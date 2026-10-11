#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]

use core::mem::{align_of, size_of};
use core::ptr;
use helios_core::params::HELIOS_BASELINE;
use helios_core::{DriveMode, Gear};
use helios_vdc::{Vdc, VdcInputs};
use helios_vsm::{Vsm, VsmConfig, VsmInputs};

const CONTEXT_MAGIC: u32 = 0x4845_4c49;
const STATUS_OK: i32 = 0;
const STATUS_NULL: i32 = -1;
const STATUS_STORAGE: i32 = -2;
const STATUS_INVALID_INPUT: i32 = -3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ControlInput {
    pub wake: u8,
    pub start_button: u8,
    pub brake_pressed: u8,
    pub accel_pedal: f32,
    pub gear_request_aosp: i32,
    pub mode_request: i32,
    pub speed_mps: f32,
    pub pack_voltage: f32,
    pub dc_link_voltage: f32,
    pub pack_current: f32,
    pub hvil_closed: u8,
    pub isolation_ohm: f32,
    pub isolation_valid: u8,
    pub crash: u8,
    pub charge_connector: u8,
    pub charge_request: u8,
    pub bms_critical: u8,
    pub bms_warning: u8,
    pub motor_fault: [u8; 4],
    pub lv_voltage: f32,
    pub heartbeat_age_ms: u32,
    pub thermal_derate: u8,
    pub steering_sensor_fault: u8,
    pub ground_speed_invalid: u8,
    pub service_reset: u8,
    pub vx: f32,
    pub vy: f32,
    pub yaw_rate: f32,
    pub ax: f32,
    pub ay: f32,
    pub steer_road: f32,
    pub wheel_omega: [f32; 4],
    pub brake_pedal: f32,
    pub motor_drive_limit: [f32; 4],
    pub motor_regen_limit: [f32; 4],
    pub p_discharge_limit: f32,
    pub p_charge_limit: f32,
    pub tv_enable: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ControlOutput {
    pub status: i32,
    pub state: u8,
    pub drive_mode: u8,
    pub gear_aosp: i32,
    pub contactor_main_pos: u8,
    pub contactor_main_neg: u8,
    pub contactor_precharge: u8,
    pub active_discharge: u8,
    pub torque_enable: u8,
    pub regen_enable: u8,
    pub motor_enable: [u8; 4],
    pub charge_enable: u8,
    pub power_scale: f32,
    pub decel_request: f32,
    pub faults: u64,
    pub motor_torque_nm: [f32; 4],
    pub brake_torque_nm: [f32; 4],
}

impl Default for ControlOutput {
    fn default() -> Self {
        Self {
            status: STATUS_INVALID_INPUT,
            state: 8,
            drive_mode: DriveMode::Road as u8,
            gear_aosp: Gear::Park.aosp(),
            contactor_main_pos: 0,
            contactor_main_neg: 0,
            contactor_precharge: 0,
            active_discharge: 0,
            torque_enable: 0,
            regen_enable: 0,
            motor_enable: [0; 4],
            charge_enable: 0,
            power_scale: 0.0,
            decel_request: 0.0,
            faults: 0,
            motor_torque_nm: [0.0; 4],
            brake_torque_nm: [0.0; 4],
        }
    }
}

struct Controller {
    magic: u32,
    vsm: Vsm,
    vdc: Vdc,
}

fn valid_bool(value: u8) -> bool {
    value <= 1
}

fn finite_input(i: &ControlInput) -> bool {
    let scalars = [
        i.accel_pedal,
        i.speed_mps,
        i.pack_voltage,
        i.dc_link_voltage,
        i.pack_current,
        i.isolation_ohm,
        i.lv_voltage,
        i.vx,
        i.vy,
        i.yaw_rate,
        i.ax,
        i.ay,
        i.steer_road,
        i.brake_pedal,
        i.p_discharge_limit,
        i.p_charge_limit,
    ];
    scalars.iter().all(|v| v.is_finite())
        && i.wheel_omega.iter().all(|v| v.is_finite())
        && i.motor_drive_limit.iter().all(|v| v.is_finite())
        && i.motor_regen_limit.iter().all(|v| v.is_finite())
}

fn valid_input(i: &ControlInput) -> bool {
    let bools = [
        i.wake,
        i.start_button,
        i.brake_pressed,
        i.hvil_closed,
        i.isolation_valid,
        i.crash,
        i.charge_connector,
        i.charge_request,
        i.bms_critical,
        i.bms_warning,
        i.thermal_derate,
        i.steering_sensor_fault,
        i.ground_speed_invalid,
        i.service_reset,
        i.tv_enable,
    ];
    bools.iter().all(|v| valid_bool(*v))
        && i.motor_fault.iter().all(|v| valid_bool(*v))
        && i.accel_pedal >= 0.0
        && i.accel_pedal <= 1.0
        && i.brake_pedal >= 0.0
        && i.brake_pedal <= 1.0
        && i.pack_voltage >= 0.0
        && i.dc_link_voltage >= 0.0
        && i.isolation_ohm >= 0.0
        && i.lv_voltage >= 0.0
        && i.p_discharge_limit >= 0.0
        && i.p_charge_limit >= 0.0
        && i.motor_drive_limit.iter().all(|v| *v >= 0.0)
        && i.motor_regen_limit.iter().all(|v| *v >= 0.0)
        && (i.gear_request_aosp == -1 || Gear::from_aosp(i.gear_request_aosp).is_some())
        && (i.mode_request == -1
            || (i.mode_request >= 0
                && i.mode_request <= u8::MAX as i32
                && DriveMode::from_u8(i.mode_request as u8).is_some_and(DriveMode::driver_selectable)))
        && finite_input(i)
}

fn make_vsm_input(i: &ControlInput) -> VsmInputs {
    VsmInputs {
        wake: i.wake != 0,
        start_button: i.start_button != 0,
        brake_pressed: i.brake_pressed != 0,
        accel_pedal: i.accel_pedal,
        gear_request: if i.gear_request_aosp == -1 { None } else { Gear::from_aosp(i.gear_request_aosp) },
        mode_request: if i.mode_request == -1 { None } else { DriveMode::from_u8(i.mode_request as u8) },
        speed_mps: i.speed_mps,
        pack_voltage: i.pack_voltage,
        dc_link_voltage: i.dc_link_voltage,
        pack_current: i.pack_current,
        hvil_closed: i.hvil_closed != 0,
        isolation_ohm: i.isolation_ohm,
        isolation_valid: i.isolation_valid != 0,
        crash: i.crash != 0,
        charge_connector: i.charge_connector != 0,
        charge_request: i.charge_request != 0,
        bms_critical: i.bms_critical != 0,
        bms_warning: i.bms_warning != 0,
        motor_fault: i.motor_fault.map(|v| v != 0),
        lv_voltage: i.lv_voltage,
        heartbeat_age_ms: i.heartbeat_age_ms,
        thermal_derate: i.thermal_derate != 0,
        steering_sensor_fault: i.steering_sensor_fault != 0,
        ground_speed_invalid: i.ground_speed_invalid != 0,
        service_reset: i.service_reset != 0,
    }
}

fn make_vdc_input(i: &ControlInput, mode: DriveMode, gear: Gear, vsm: &helios_vsm::VsmOutputs) -> VdcInputs {
    VdcInputs {
        vx: i.vx,
        vy: i.vy,
        yaw_rate: i.yaw_rate,
        ax: i.ax,
        ay: i.ay,
        steer_road: i.steer_road,
        wheel_omega: i.wheel_omega,
        accel_pedal: i.accel_pedal,
        brake_pedal: i.brake_pedal,
        mode,
        reverse: gear == Gear::Reverse,
        torque_enable: vsm.torque_enable,
        regen_enable: vsm.regen_enable,
        motor_enable: vsm.motor_enable,
        power_scale: vsm.power_scale,
        motor_drive_limit: i.motor_drive_limit,
        motor_regen_limit: i.motor_regen_limit,
        p_discharge_limit: i.p_discharge_limit,
        p_charge_limit: i.p_charge_limit,
        decel_request: vsm.decel_request,
        tv_enable: i.tv_enable != 0,
    }
}

fn set_safe_output(output: &mut ControlOutput, status: i32) {
    *output = ControlOutput::default();
    output.status = status;
}

#[unsafe(no_mangle)]
pub extern "C" fn helios_control_context_size() -> usize {
    size_of::<Controller>()
}

#[unsafe(no_mangle)]
pub extern "C" fn helios_control_context_align() -> usize {
    align_of::<Controller>()
}

/// The caller provides `helios_control_context_size()` writable bytes aligned to
/// `helios_control_context_align()`. Only the Zephyr control thread may use it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helios_control_init(storage: *mut u8, storage_len: usize) -> i32 {
    if storage.is_null() {
        return STATUS_NULL;
    }
    if (storage as usize) % align_of::<Controller>() != 0 || storage_len < size_of::<Controller>() {
        return STATUS_STORAGE;
    }
    let mut vsm_config = VsmConfig::helios_baseline();
    vsm_config.heartbeat_stale_ms = 5;
    vsm_config.heartbeat_lost_ms = 9;
    let controller =
        Controller { magic: CONTEXT_MAGIC, vsm: Vsm::new(vsm_config), vdc: Vdc::new(HELIOS_BASELINE, 0.001) };
    // SAFETY: The API contract requires an aligned, writable storage region of the
    // reported size. The region is exclusively owned by this controller instance.
    unsafe { ptr::write(storage.cast::<Controller>(), controller) };
    STATUS_OK
}

/// The input and output pointers must reference valid C-compatible structs. This
/// function is single-thread-owned; Zephyr must call it only from its control task.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helios_control_step(
    storage: *mut u8,
    input: *const ControlInput,
    output: *mut ControlOutput,
) -> i32 {
    if storage.is_null() || input.is_null() || output.is_null() {
        return STATUS_NULL;
    }
    if (storage as usize) % align_of::<Controller>() != 0 {
        // SAFETY: A non-null output pointer is required by the C ABI contract.
        unsafe { set_safe_output(&mut *output, STATUS_STORAGE) };
        return STATUS_STORAGE;
    }
    // SAFETY: The ABI contract requires valid pointers and a context previously
    // initialized with helios_control_init. The dedicated Zephyr task owns them.
    let (controller, input, output) = unsafe { (&mut *storage.cast::<Controller>(), *input, &mut *output) };
    if controller.magic != CONTEXT_MAGIC {
        set_safe_output(output, STATUS_STORAGE);
        return STATUS_STORAGE;
    }
    if !valid_input(&input) {
        set_safe_output(output, STATUS_INVALID_INPUT);
        return STATUS_INVALID_INPUT;
    }
    let vsm = controller.vsm.step(&make_vsm_input(&input));
    let vdc = controller.vdc.step(&make_vdc_input(&input, vsm.drive_mode, vsm.gear, &vsm));
    output.status = STATUS_OK;
    output.state = vsm.state as u8;
    output.drive_mode = vsm.drive_mode as u8;
    output.gear_aosp = vsm.gear.aosp();
    output.contactor_main_pos = vsm.contactors.main_pos as u8;
    output.contactor_main_neg = vsm.contactors.main_neg as u8;
    output.contactor_precharge = vsm.contactors.precharge as u8;
    output.active_discharge = vsm.active_discharge as u8;
    output.torque_enable = vsm.torque_enable as u8;
    output.regen_enable = vsm.regen_enable as u8;
    output.motor_enable = vsm.motor_enable.map(|v| v as u8);
    output.charge_enable = vsm.charge_enable as u8;
    output.power_scale = vsm.power_scale;
    output.decel_request = vsm.decel_request;
    output.faults = vsm.faults.0;
    output.motor_torque_nm = vdc.motor_torque;
    output.brake_torque_nm = vdc.brake_torque;
    STATUS_OK
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    // The Zephyr hardware watchdog is expected to reset the target if Rust panics.
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(C, align(16))]
    struct Storage([u8; 2048]);

    #[test]
    fn context_fits_the_documented_zephyr_storage() {
        assert!(size_of::<Controller>() <= 2048);
        assert!(align_of::<Controller>() <= 16);
        assert_eq!(size_of::<ControlInput>(), 152);
        assert_eq!(size_of::<ControlOutput>(), 72);
    }

    #[test]
    fn invalid_sensor_value_returns_zero_torque_and_open_contactors() {
        let mut storage = Storage([0; 2048]);
        let mut input = ControlInput {
            hvil_closed: 1,
            isolation_valid: 1,
            tv_enable: 1,
            gear_request_aosp: -1,
            mode_request: -1,
            isolation_ohm: 5.0e6,
            pack_voltage: 760.0,
            lv_voltage: 13.8,
            p_discharge_limit: 100_000.0,
            p_charge_limit: 100_000.0,
            motor_drive_limit: [500.0; 4],
            motor_regen_limit: [500.0; 4],
            ..Default::default()
        };
        let mut output = ControlOutput::default();
        // SAFETY: The test storage meets the exported size/alignment contract.
        unsafe {
            assert_eq!(helios_control_init(storage.0.as_mut_ptr(), storage.0.len()), STATUS_OK);
            input.vx = f32::NAN;
            assert_eq!(
                helios_control_step(storage.0.as_mut_ptr(), &input, &mut output),
                STATUS_INVALID_INPUT
            );
        }
        assert_eq!(output.status, STATUS_INVALID_INPUT);
        assert_eq!(output.contactor_main_pos, 0);
        assert_eq!(output.contactor_main_neg, 0);
        assert_eq!(output.motor_torque_nm, [0.0; 4]);
    }

    #[test]
    fn valid_standby_input_starts_with_safe_outputs() {
        let mut storage = Storage([0; 2048]);
        let input = ControlInput {
            hvil_closed: 1,
            isolation_valid: 1,
            tv_enable: 1,
            gear_request_aosp: -1,
            mode_request: -1,
            isolation_ohm: 5.0e6,
            pack_voltage: 760.0,
            lv_voltage: 13.8,
            p_discharge_limit: 100_000.0,
            p_charge_limit: 100_000.0,
            motor_drive_limit: [500.0; 4],
            motor_regen_limit: [500.0; 4],
            ..Default::default()
        };
        let mut output = ControlOutput::default();
        // SAFETY: The test storage meets the exported size/alignment contract.
        unsafe {
            assert_eq!(helios_control_init(storage.0.as_mut_ptr(), storage.0.len()), STATUS_OK);
            assert_eq!(helios_control_step(storage.0.as_mut_ptr(), &input, &mut output), STATUS_OK);
        }
        assert_eq!(output.contactor_main_pos, 0);
        assert_eq!(output.contactor_main_neg, 0);
        assert_eq!(output.motor_torque_nm, [0.0; 4]);
    }

    #[test]
    fn lost_hpc_heartbeat_is_reported_by_vsm_and_blocks_torque() {
        let mut storage = Storage([0; 2048]);
        let input = ControlInput {
            hvil_closed: 1,
            isolation_valid: 1,
            tv_enable: 1,
            gear_request_aosp: -1,
            mode_request: -1,
            isolation_ohm: 5.0e6,
            pack_voltage: 760.0,
            lv_voltage: 13.8,
            wake: 1,
            heartbeat_age_ms: 10,
            p_discharge_limit: 100_000.0,
            p_charge_limit: 100_000.0,
            motor_drive_limit: [500.0; 4],
            motor_regen_limit: [500.0; 4],
            ..Default::default()
        };
        let mut output = ControlOutput::default();
        // SAFETY: The test storage meets the exported size/alignment contract.
        unsafe {
            assert_eq!(helios_control_init(storage.0.as_mut_ptr(), storage.0.len()), STATUS_OK);
            assert_eq!(helios_control_step(storage.0.as_mut_ptr(), &input, &mut output), STATUS_OK);
        }
        assert_ne!(output.faults & (1 << helios_core::Fault::CentralHeartbeatLost as u8), 0);
        assert_eq!(output.torque_enable, 0);
        assert_eq!(output.motor_torque_nm, [0.0; 4]);
    }
}
