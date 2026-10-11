#include <errno.h>
#include <stdint.h>

#include <zephyr/device.h>
#include <zephyr/devicetree.h>
#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/sys/atomic.h>
#include <zephyr/sys/reboot.h>
#include <zephyr/sys/util.h>
#include <zephyr/task_wdt/task_wdt.h>

#include "helios_control_ffi.h"

LOG_MODULE_REGISTER(helios_control, CONFIG_LOG_DEFAULT_LEVEL);

BUILD_ASSERT(sizeof(struct helios_control_input) == 152,
	     "C and Rust control input ABI layouts differ");
BUILD_ASSERT(sizeof(struct helios_control_output) == 72,
	     "C and Rust control output ABI layouts differ");

#define CONTROL_STACK_SIZE 8192
#define CONTROL_THREAD_PRIORITY 1
#define CONTROL_PERIOD_MS 1
#define CONTROL_WATCHDOG_MS CONFIG_HELIOS_CONTROL_WATCHDOG_MS
#define CONTROL_CONTEXT_MAX_SIZE 2048
#define CONTROL_CONTEXT_MAX_ALIGN 16

static uint8_t control_context[CONTROL_CONTEXT_MAX_SIZE]
	__aligned(CONTROL_CONTEXT_MAX_ALIGN);
static struct helios_control_input control_input;
static struct helios_control_output control_output;
static struct k_timer control_timer;
static struct k_sem control_start_sem;
static int control_watchdog_channel = -1;
static const struct device *watchdog_device;
static atomic_t safe_outputs_latched;
static bool invalid_input_reported;

static void force_safe_outputs(void)
{
	/*
	 * Board-specific production code must drive independent hardware safe-state
	 * lines here. No GPIO outputs are assumed until board signals are validated.
	 */
	control_output.contactor_main_pos = 0;
	control_output.contactor_main_neg = 0;
	control_output.contactor_precharge = 0;
	control_output.active_discharge = 0;
	control_output.torque_enable = 0;
	control_output.regen_enable = 0;
	control_output.motor_enable[0] = 0;
	control_output.motor_enable[1] = 0;
	control_output.motor_enable[2] = 0;
	control_output.motor_enable[3] = 0;
	control_output.charge_enable = 0;
	control_output.power_scale = 0.0f;
	control_output.decel_request = 0.0f;
	control_output.motor_torque_nm[0] = 0.0f;
	control_output.motor_torque_nm[1] = 0.0f;
	control_output.motor_torque_nm[2] = 0.0f;
	control_output.motor_torque_nm[3] = 0.0f;
	control_output.brake_torque_nm[0] = 0.0f;
	control_output.brake_torque_nm[1] = 0.0f;
	control_output.brake_torque_nm[2] = 0.0f;
	control_output.brake_torque_nm[3] = 0.0f;
}

static void watchdog_expired(int channel_id, void *user_data)
{
	ARG_UNUSED(channel_id);
	ARG_UNUSED(user_data);
	atomic_set(&safe_outputs_latched, 1);
	LOG_ERR("control watchdog expired; rebooting");
	sys_reboot(SYS_REBOOT_COLD);
}

static int init_watchdog(void)
{
	int ret;
	int task_channel;

#if DT_NODE_HAS_STATUS_OKAY(DT_ALIAS(watchdog0))
	watchdog_device = DEVICE_DT_GET(DT_ALIAS(watchdog0));
#else
	watchdog_device = NULL;
#endif

	if (watchdog_device == NULL || !device_is_ready(watchdog_device)) {
		LOG_WRN("no watchdog0 devicetree alias; only task watchdog is active");
		watchdog_device = NULL;
	}

	ret = task_wdt_init(watchdog_device);
	if (ret != 0) {
		return ret;
	}
	task_channel = task_wdt_add(CONTROL_WATCHDOG_MS, watchdog_expired, NULL);
	if (task_channel < 0) {
		return task_channel;
	}
	control_watchdog_channel = task_channel;
	return 0;
}

static void control_thread(void *arg1, void *arg2, void *arg3)
{
	int ret;

	ARG_UNUSED(arg1);
	ARG_UNUSED(arg2);
	ARG_UNUSED(arg3);
	k_sem_take(&control_start_sem, K_FOREVER);

	while (true) {
		uint32_t expirations = k_timer_status_sync(&control_timer);

		if (expirations > 1U) {
			atomic_set(&safe_outputs_latched, 1);
			force_safe_outputs();
			LOG_ERR("control loop missed %u timer periods", expirations);
		}
		if (atomic_get(&safe_outputs_latched) != 0) {
			force_safe_outputs();
			continue;
		}

		/*
		 * Safe placeholder until validated board drivers are supplied. Marking
		 * the measurement invalid makes the Rust facade reject the cycle and
		 * zero its control outputs; this scaffold never commands vehicle I/O.
		 */
		control_input.vx = __builtin_nan("");
		ret = helios_control_step(control_context, &control_input, &control_output);
		if (ret != 0) {
			force_safe_outputs();
			if (!invalid_input_reported) {
				LOG_ERR("control input rejected (%d); outputs remain safe", ret);
				invalid_input_reported = true;
			}
		}

		ret = task_wdt_feed(control_watchdog_channel);
		if (ret != 0) {
			atomic_set(&safe_outputs_latched, 1);
			force_safe_outputs();
			LOG_ERR("task watchdog feed failed (%d)", ret);
		}
	}
}

K_THREAD_DEFINE(control_thread_id, CONTROL_STACK_SIZE, control_thread,
		NULL, NULL, NULL, CONTROL_THREAD_PRIORITY, 0, 0);

int main(void)
{
	int ret;

	k_sem_init(&control_start_sem, 0, 1);
	k_timer_init(&control_timer, NULL, NULL);

	if (helios_control_context_size() > sizeof(control_context) ||
	    helios_control_context_align() > CONTROL_CONTEXT_MAX_ALIGN) {
		LOG_ERR("Rust controller context exceeds static storage");
		return -ENOMEM;
	}
	ret = helios_control_init(control_context, sizeof(control_context));
	if (ret != 0) {
		LOG_ERR("Rust controller init failed (%d)", ret);
		return ret;
	}
	control_input.gear_request_aosp = -1;
	control_input.mode_request = -1;
	control_input.tv_enable = 1;
	ret = init_watchdog();
	if (ret != 0) {
		LOG_ERR("watchdog init failed (%d)", ret);
		force_safe_outputs();
		return ret;
	}

	k_timer_start(&control_timer, K_MSEC(CONTROL_PERIOD_MS),
		      K_MSEC(CONTROL_PERIOD_MS));
	k_sem_give(&control_start_sem);
	LOG_INF("HELIOS Zephyr control scaffold initialized; vehicle I/O is not connected");
	return 0;
}
