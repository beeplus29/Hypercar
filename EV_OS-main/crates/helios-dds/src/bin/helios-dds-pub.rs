//! Publish N telemetry samples on the HELIOS domain (cross-process smoke test).
use clap::Parser;
use helios_dds::{Participant, QosProfile, TOPIC_TELEMETRY, VehicleTelemetry};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value_t = helios_dds::HELIOS_DOMAIN)]
    domain: u32,
    #[arg(long, default_value_t = 1000)]
    count: u64,
    #[arg(long, default_value_t = 1000.0)]
    rate_hz: f64,
    #[arg(long, default_value_t = 10)]
    wait_s: u64,
}

fn main() -> anyhow::Result<()> {
    let a = Args::parse();
    let p = Participant::new(a.domain)?;
    let w = p.writer::<VehicleTelemetry>(TOPIC_TELEMETRY, QosProfile::Stream)?;
    w.wait_for_readers(1, Duration::from_secs(a.wait_s))?;
    let period = Duration::from_secs_f64(1.0 / a.rate_hz);
    let t0 = Instant::now();
    for k in 1..=a.count {
        let s = VehicleTelemetry {
            vehicle_id: 1,
            t_ns: t0.elapsed().as_nanos() as u64,
            odometer_m: k as f32,
            speed_mps: (k as f32 * 0.01).sin().abs() * 90.0,
            ..Default::default()
        };
        w.write(&s)?;
        let next = period * k as u32;
        if let Some(d) = next.checked_sub(t0.elapsed()) {
            std::thread::sleep(d);
        }
    }
    std::thread::sleep(Duration::from_millis(300));
    println!("published {} samples in {:.3} s", a.count, t0.elapsed().as_secs_f64());
    Ok(())
}
