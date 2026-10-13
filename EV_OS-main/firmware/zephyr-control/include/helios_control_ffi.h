#ifndef HELIOS_CONTROL_FFI_H
#define HELIOS_CONTROL_FFI_H

#include <stddef.h>
#include <stdint.h>

struct helios_control_input {
	uint8_t wake;
	uint8_t start_button;
	uint8_t brake_pressed;
	float accel_pedal;
	/* Use -1 for no request; otherwise use AOSP VehicleGear bits. */
	int32_t gear_request_aosp;
	/* Use -1 for no request; otherwise use the HELIOS DriveMode value. */
	int32_t mode_request;
	float speed_mps;
	float pack_voltage;
	float dc_link_voltage;
	float pack_current;
	uint8_t hvil_closed;
	float isolation_ohm;
	uint8_t isolation_valid;
	uint8_t crash;
	uint8_t charge_connector;
	uint8_t charge_request;
	uint8_t bms_critical;
	uint8_t bms_warning;
	uint8_t motor_fault[4];
	float lv_voltage;
	uint32_t heartbeat_age_ms;
	uint8_t thermal_derate;
	uint8_t steering_sensor_fault;
	uint8_t ground_speed_invalid;
	uint8_t service_reset;
	float vx;
	float vy;
	float yaw_rate;
	float ax;
	float ay;
	float steer_road;
	float wheel_omega[4];
	float brake_pedal;
	float motor_drive_limit[4];
	float motor_regen_limit[4];
	float p_discharge_limit;
	float p_charge_limit;
	uint8_t tv_enable;
};

struct helios_control_output {
	int32_t status;
	uint8_t state;
	uint8_t drive_mode;
	int32_t gear_aosp;
	uint8_t contactor_main_pos;
	uint8_t contactor_main_neg;
	uint8_t contactor_precharge;
	uint8_t active_discharge;
	uint8_t torque_enable;
	uint8_t regen_enable;
	uint8_t motor_enable[4];
	uint8_t charge_enable;
	float power_scale;
	float decel_request;
	uint64_t faults;
	float motor_torque_nm[4];
	float brake_torque_nm[4];
};

size_t helios_control_context_size(void);
size_t helios_control_context_align(void);
int32_t helios_control_init(uint8_t *storage, size_t storage_len);
int32_t helios_control_step(uint8_t *storage,
			    const struct helios_control_input *input,
			    struct helios_control_output *output);

_Static_assert(sizeof(struct helios_control_input) == 152,
	       "C and Rust control input ABI layouts differ");
_Static_assert(sizeof(struct helios_control_output) == 72,
	       "C and Rust control output ABI layouts differ");

#endif
