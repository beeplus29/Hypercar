//! Canonical SIL telemetry snapshot shared by the middleware publisher targets.
use crate::Sim;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TwinTelemetry {
    pub schema_version: u16,
    pub vehicle_id: u32,
    pub timestamp_ns: u64,
    pub speed_mps: f32,
    pub longitudinal_accel_mps2: f32,
    pub lateral_accel_mps2: f32,
    pub yaw_rate_rps: f32,
    pub sideslip_deg: f32,
    pub steering_angle_rad: f32,
    pub throttle: f32,
    pub brake: f32,
    pub vehicle_state: u8,
    pub gear: i32,
    pub drive_mode: u8,
    pub aero_mode: u8,
    pub fault_bits: u32,
    pub soc_pct: f32,
    pub pack_voltage_v: f32,
    pub pack_current_a: f32,
    pub battery_power_kw: f32,
    pub discharge_limit_kw: f32,
    pub regen_limit_kw: f32,
    pub battery_temperature_c: f32,
    pub coolant_temperature_c: f32,
    pub motor_torque_nm: [f32; 4],
    pub motor_winding_temperature_c: [f32; 4],
    pub wheel_slip: [f32; 4],
    pub tyre_load_n: [f32; 4],
    pub downforce_n: f32,
    pub yaw_moment_nm: f32,
    pub odometer_m: f32,
}

pub fn twin_telemetry(sim: &Sim, vehicle_id: u32, timestamp_ns: u64) -> TwinTelemetry {
    TwinTelemetry {
        schema_version: 1,
        vehicle_id,
        timestamp_ns,
        speed_mps: sim.plant.s.vx as f32,
        longitudinal_accel_mps2: sim.plant.aux.ax as f32,
        lateral_accel_mps2: sim.plant.aux.ay as f32,
        yaw_rate_rps: sim.plant.s.r as f32,
        sideslip_deg: sim.vdc_out.sideslip.to_degrees(),
        steering_angle_rad: sim.driver.steer_road as f32,
        throttle: sim.driver.accel_pedal as f32,
        brake: sim.driver.brake_pedal as f32,
        vehicle_state: sim.vsm.state() as u8,
        gear: sim.vsm_out.gear.aosp(),
        drive_mode: sim.vsm_out.drive_mode as u8,
        aero_mode: sim.vdc_out.aero_mode as u8,
        fault_bits: sim.vsm.faults().0 as u32,
        soc_pct: sim.battery.s.soc * 100.0,
        pack_voltage_v: sim.battery.terminal_voltage(sim.i_batt as f32),
        pack_current_a: sim.i_batt as f32,
        battery_power_kw: (sim.p_batt / 1000.0) as f32,
        discharge_limit_kw: sim.sop.p_discharge_w / 1000.0,
        regen_limit_kw: sim.sop.p_charge_w / 1000.0,
        battery_temperature_c: sim.battery.s.t_cell_c,
        coolant_temperature_c: sim.battery.s.t_coolant_c,
        motor_torque_nm: sim.vdc_out.motor_torque,
        motor_winding_temperature_c: sim.motor_th.map(|motor| motor.t_winding_c),
        wheel_slip: sim.plant.aux.slip_ss.map(|value| value as f32),
        tyre_load_n: sim.plant.aux.fz.map(|value| value as f32),
        downforce_n: sim.plant.aux.downforce as f32,
        yaw_moment_nm: sim.vdc_out.mz_demand,
        odometer_m: sim.plant.s.x as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helios_core::DriveMode;
    use helios_core::params::HELIOS_BASELINE;

    #[test]
    fn twin_snapshot_has_stable_identity_and_mapping() {
        let mut sim = Sim::new(HELIOS_BASELINE, 0.8, 25.0);
        sim.set_speed(12.0);
        sim.driver.steer_road = 0.1;
        let snapshot = twin_telemetry(&sim, 7, 42);

        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.vehicle_id, 7);
        assert_eq!(snapshot.timestamp_ns, 42);
        assert_eq!(snapshot.gear, sim.vsm_out.gear.aosp());
        assert_eq!(snapshot.drive_mode, DriveMode::Road as u8);
        assert_eq!(snapshot.speed_mps, 12.0);
        assert!((snapshot.steering_angle_rad - 0.1).abs() < 1e-6);
        assert_eq!(snapshot.motor_torque_nm, sim.vdc_out.motor_torque);
        assert_eq!(snapshot.motor_winding_temperature_c[0], 25.0);
    }

    #[test]
    fn twin_snapshot_serializes_with_versioned_structured_fields() {
        let sim = Sim::new(HELIOS_BASELINE, 0.8, 25.0);
        let snapshot = twin_telemetry(&sim, 1, 100);
        let json = serde_json::to_value(&snapshot).expect("serialize snapshot");

        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["vehicle_id"], 1);
        assert!(json["motor_torque_nm"].is_array());
        assert!(json["battery_temperature_c"].is_number());
    }
}
