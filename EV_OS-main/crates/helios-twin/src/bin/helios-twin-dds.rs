//! Software-in-the-loop bridge: run the digital twin and publish telemetry at
//! 100 Hz over Cyclone DDS. This process has no vehicle actuator interface.
use clap::Parser;
use helios_core::DriveMode;
use helios_core::params::HELIOS_BASELINE;
use helios_dds::{Participant, QosProfile, TOPIC_TELEMETRY, VehicleTelemetry};
use helios_twin::{DriverCmd, Sim, twin_telemetry};
use std::time::{Duration, Instant};

const TWIN_PERIOD: Duration = Duration::from_millis(1);
const PUBLISH_EVERY_TICKS: u64 = 10;

#[derive(Parser, Debug)]
#[command(about = "Run the HELIOS digital twin and publish SIL telemetry over Cyclone DDS")]
struct Args {
    #[arg(long, default_value_t = helios_dds::HELIOS_DOMAIN)]
    domain: u32,
    #[arg(long, default_value_t = 10.0)]
    duration_s: f64,
    #[arg(long, default_value_t = 0.0)]
    throttle: f64,
    #[arg(long, default_value_t = 0.0)]
    brake: f64,
    #[arg(long, default_value_t = 0.0)]
    steer_rad: f64,
}

fn validate(args: &Args) -> anyhow::Result<()> {
    anyhow::ensure!(
        args.duration_s.is_finite() && args.duration_s > 0.0,
        "duration must be finite and positive"
    );
    anyhow::ensure!(
        args.throttle.is_finite() && (0.0..=1.0).contains(&args.throttle),
        "throttle must be in [0, 1]"
    );
    anyhow::ensure!(args.brake.is_finite() && (0.0..=1.0).contains(&args.brake), "brake must be in [0, 1]");
    anyhow::ensure!(
        args.steer_rad.is_finite() && args.steer_rad.abs() <= 0.5,
        "steer_rad must be finite and within +/-0.5 rad"
    );
    anyhow::Ok(())
}

fn dds_telemetry(snapshot: helios_twin::TwinTelemetry) -> VehicleTelemetry {
    VehicleTelemetry {
        vehicle_id: snapshot.vehicle_id,
        t_ns: snapshot.timestamp_ns,
        speed_mps: snapshot.speed_mps,
        ax_mps2: snapshot.longitudinal_accel_mps2,
        ay_mps2: snapshot.lateral_accel_mps2,
        yaw_rate_rps: snapshot.yaw_rate_rps,
        sideslip_deg: snapshot.sideslip_deg,
        steer_road_deg: snapshot.steering_angle_rad.to_degrees(),
        accel_pedal: snapshot.throttle,
        brake_pedal: snapshot.brake,
        vehicle_state: snapshot.vehicle_state,
        gear: snapshot.gear as u8,
        drive_mode: snapshot.drive_mode,
        aero_mode: snapshot.aero_mode,
        fault_bits: snapshot.fault_bits,
        soc_pct: snapshot.soc_pct,
        pack_voltage_v: snapshot.pack_voltage_v,
        pack_current_a: snapshot.pack_current_a,
        battery_power_kw: snapshot.battery_power_kw,
        discharge_limit_kw: snapshot.discharge_limit_kw,
        regen_limit_kw: snapshot.regen_limit_kw,
        cell_temp_c: snapshot.battery_temperature_c,
        coolant_temp_c: snapshot.coolant_temperature_c,
        motor_torque_nm: snapshot.motor_torque_nm,
        motor_winding_c: snapshot.motor_winding_temperature_c,
        wheel_slip: snapshot.wheel_slip,
        tyre_load_n: snapshot.tyre_load_n,
        downforce_n: snapshot.downforce_n,
        yaw_moment_nm: snapshot.yaw_moment_nm,
        odometer_m: snapshot.odometer_m,
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    validate(&args)?;

    let participant = Participant::new(args.domain)?;
    let writer = participant.writer::<VehicleTelemetry>(TOPIC_TELEMETRY, QosProfile::Telemetry)?;
    let mut sim = Sim::new(HELIOS_BASELINE, 0.8, 25.0);
    let t0 = Instant::now();
    sim.power_up(DriveMode::Road)?;
    sim.driver = DriverCmd {
        accel_pedal: args.throttle,
        brake_pedal: args.brake,
        steer_road: args.steer_rad,
        ..sim.driver
    };

    let deadline = Instant::now() + Duration::from_secs_f64(args.duration_s);
    let mut next_tick = Instant::now();
    let mut ticks = 0_u64;
    let mut missed_deadlines = 0_u64;
    while next_tick < deadline {
        std::thread::sleep(next_tick.saturating_duration_since(Instant::now()));
        sim.step();
        ticks += 1;
        if ticks % PUBLISH_EVERY_TICKS == 0 {
            let timestamp_ns = t0.elapsed().as_nanos().min(u64::MAX as u128) as u64;
            writer.write(&dds_telemetry(twin_telemetry(&sim, 1, timestamp_ns)))?;
        }
        next_tick += TWIN_PERIOD;
        let now = Instant::now();
        if next_tick < now {
            missed_deadlines += 1;
            next_tick = now + TWIN_PERIOD;
        }
    }

    println!(
        "SIL complete: {} twin steps, {} telemetry samples, {} missed 1ms deadlines in {:.3}s",
        ticks,
        ticks / PUBLISH_EVERY_TICKS,
        missed_deadlines,
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}
