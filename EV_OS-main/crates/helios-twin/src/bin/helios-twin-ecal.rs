//! SIL-only eCAL publisher for HPC-local telemetry exploration.
//! This process has no vehicle sensor or actuator interface.
use clap::Parser;
use helios_core::DriveMode;
use helios_core::params::HELIOS_BASELINE;
use helios_twin::{DriverCmd, Sim, TwinTelemetry, twin_telemetry};
use rustecal::pubsub::publisher::Timestamp;
use rustecal::{Ecal, EcalComponents, TypedPublisher};
use rustecal_types_serde::JsonMessage;
use std::time::{Duration, Instant};

const TWIN_PERIOD: Duration = Duration::from_millis(1);
const PUBLISH_EVERY_TICKS: u64 = 10;
const ECAL_TOPIC: &str = "helios/sil/twin_telemetry";

#[derive(Parser, Debug)]
#[command(about = "Run the HELIOS digital twin and publish SIL telemetry over eCAL")]
struct Args {
    #[arg(long, default_value_t = 10.0)]
    duration_s: f64,
    #[arg(long, default_value_t = 0.0)]
    throttle: f64,
    #[arg(long, default_value_t = 0.0)]
    brake: f64,
    #[arg(long, default_value_t = 0.0)]
    steer_rad: f64,
}

struct EcalSession;

impl EcalSession {
    fn start() -> anyhow::Result<Self> {
        Ecal::initialize(Some("helios-twin-sil"), EcalComponents::DEFAULT, None)
            .map_err(anyhow::Error::msg)?;
        Ok(Self)
    }
}

impl Drop for EcalSession {
    fn drop(&mut self) {
        Ecal::finalize();
    }
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
    Ok(())
}

fn run(args: Args) -> anyhow::Result<()> {
    let _ecal = EcalSession::start()?;
    let publisher =
        TypedPublisher::<JsonMessage<TwinTelemetry>>::new(ECAL_TOPIC).map_err(anyhow::Error::msg)?;

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
    let mut published = 0_u64;
    let mut missed_deadlines = 0_u64;
    while next_tick < deadline {
        std::thread::sleep(next_tick.saturating_duration_since(Instant::now()));
        sim.step();
        ticks += 1;
        if ticks % PUBLISH_EVERY_TICKS == 0 {
            let timestamp_ns = t0.elapsed().as_nanos().min(u64::MAX as u128) as u64;
            let message = JsonMessage::new(twin_telemetry(&sim, 1, timestamp_ns));
            anyhow::ensure!(
                publisher.send(&message, Timestamp::Auto),
                "eCAL failed to publish telemetry at tick {ticks}"
            );
            published += 1;
        }
        next_tick += TWIN_PERIOD;
        let now = Instant::now();
        if next_tick < now {
            missed_deadlines += 1;
            next_tick = now + TWIN_PERIOD;
        }
    }

    println!(
        "SIL eCAL complete: {ticks} twin steps, {published} telemetry samples, \
         {missed_deadlines} missed 1ms deadlines in {:.3}s",
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    validate(&args)?;
    run(args)
}
