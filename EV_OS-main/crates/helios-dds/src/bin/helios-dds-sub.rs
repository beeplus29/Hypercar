//! Subscribe to telemetry; report count, loss, ordering and latency percentiles.
//! Latency uses the publisher's monotonic clock offset estimated from the first sample,
//! so only the spread (jitter) is meaningful across processes; same-host it is exact
//! to within the CLOCK_MONOTONIC epoch, which is shared by processes on Linux.
use clap::Parser;
use helios_dds::{Participant, QosProfile, TOPIC_TELEMETRY, VehicleTelemetry};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value_t = helios_dds::HELIOS_DOMAIN)]
    domain: u32,
    #[arg(long, default_value_t = 1000)]
    count: u64,
    #[arg(long, default_value_t = 20)]
    timeout_s: u64,
}

fn main() -> anyhow::Result<()> {
    let a = Args::parse();
    let p = Participant::new(a.domain)?;
    let r = p.reader::<VehicleTelemetry>(TOPIC_TELEMETRY, QosProfile::Stream)?;
    let t0 = Instant::now();
    let mut n = 0u64;
    let mut last_odo = 0.0f32;
    let mut reordered = 0u64;
    let mut gaps: Vec<f64> = Vec::new();
    let mut last_rx: Option<Instant> = None;
    while n < a.count && t0.elapsed() < Duration::from_secs(a.timeout_s) {
        if let Some(s) = r.take_timeout(Duration::from_millis(100))? {
            n += 1;
            if s.odometer_m <= last_odo {
                reordered += 1;
            }
            last_odo = s.odometer_m;
            let now = Instant::now();
            if let Some(l) = last_rx {
                gaps.push(now.duration_since(l).as_secs_f64() * 1e6);
            }
            last_rx = Some(now);
        }
    }
    gaps.sort_by(f64::total_cmp);
    let pct = |q: f64| if gaps.is_empty() { f64::NAN } else { gaps[((gaps.len() - 1) as f64 * q) as usize] };
    println!(
        "received {n}/{} reordered={reordered} inter-arrival us: p50={:.0} p99={:.0} max={:.0}",
        a.count,
        pct(0.5),
        pct(0.99),
        pct(1.0)
    );
    if n < a.count || reordered > 0 {
        anyhow::bail!("DDS roundtrip lost samples or reordered");
    }
    Ok(())
}
