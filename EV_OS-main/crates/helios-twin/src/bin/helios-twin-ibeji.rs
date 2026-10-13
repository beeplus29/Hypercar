//! Hardware-free end-to-end SIL: simulation -> Ibeji-compatible registry/provider
//! -> local consumer. It accepts no vehicle commands and has no hardware interface.
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use clap::Parser;
use helios_core::DriveMode;
use helios_core::params::HELIOS_BASELINE;
use helios_ibeji_sil::digital_twin_consumer::{
    PublishRequest, PublishResponse, RespondRequest, RespondResponse,
    digital_twin_consumer_server::{DigitalTwinConsumer, DigitalTwinConsumerServer},
};
use helios_ibeji_sil::{IbejiSil, find_telemetry_entity, subscribe_to_telemetry};
use helios_twin::{DriverCmd, Sim, TwinTelemetry, twin_telemetry};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status, transport::Server};

const TWIN_PERIOD: Duration = Duration::from_millis(1);
const PUBLISH_EVERY_TICKS: u64 = 100;

#[derive(Parser, Debug)]
#[command(about = "Run a hardware-free HELIOS twin through a local Ibeji-compatible SIL stack")]
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

#[derive(Clone)]
struct LocalConsumer {
    received: tokio::sync::mpsc::UnboundedSender<TwinTelemetry>,
}

#[tonic::async_trait]
impl DigitalTwinConsumer for LocalConsumer {
    async fn publish(&self, request: Request<PublishRequest>) -> Result<Response<PublishResponse>, Status> {
        let request = request.into_inner();
        let snapshot: TwinTelemetry = serde_json::from_str(&request.value)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
        if snapshot.schema_version != 1 {
            return Err(Status::failed_precondition("unsupported telemetry schema version"));
        }
        self.received.send(snapshot).map_err(|_| Status::unavailable("local SIL consumer has stopped"))?;
        Ok(Response::new(PublishResponse {}))
    }

    async fn respond(&self, _request: Request<RespondRequest>) -> Result<Response<RespondResponse>, Status> {
        Err(Status::unimplemented("the SIL telemetry consumer has no invoke operations"))
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

async fn start_consumer() -> anyhow::Result<(
    SocketAddr,
    tokio::sync::mpsc::UnboundedReceiver<TwinTelemetry>,
    tokio::task::JoinHandle<()>,
)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        if let Err(error) = Server::builder()
            .add_service(DigitalTwinConsumerServer::new(LocalConsumer { received: sender }))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
        {
            eprintln!("local Ibeji SIL consumer stopped: {error}");
        }
    });
    Ok((address, receiver, task))
}

async fn run(args: Args) -> anyhow::Result<()> {
    let ibeji = IbejiSil::start().await?;
    let registered = find_telemetry_entity(ibeji.registry_uri())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Ibeji SIL registry did not return the telemetry entity"))?;
    anyhow::ensure!(
        registered.id == helios_ibeji_sil::TELEMETRY_ENTITY_ID,
        "Ibeji SIL registry returned an unexpected entity ID"
    );

    let (consumer_address, mut received, consumer_task) = start_consumer().await?;
    let consumer_uri = format!("http://{consumer_address}");
    subscribe_to_telemetry(ibeji.provider_uri(), &consumer_uri).await?;
    println!(
        "Ibeji SIL registered {:?}; registry={} provider={} consumer={}",
        registered.name,
        ibeji.registry_uri(),
        ibeji.provider_uri(),
        consumer_uri
    );

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
    let mut last_snapshot = None;
    while next_tick < deadline {
        tokio::time::sleep_until(next_tick.into()).await;
        sim.step();
        ticks += 1;
        if ticks % PUBLISH_EVERY_TICKS == 0 {
            let timestamp_ns = t0.elapsed().as_nanos().min(u64::MAX as u128) as u64;
            let snapshot = twin_telemetry(&sim, 1, timestamp_ns);
            published += ibeji.publish(&snapshot).await? as u64;
            last_snapshot = Some(snapshot);
        }
        while let Ok(snapshot) = received.try_recv() {
            last_snapshot = Some(snapshot);
        }
        next_tick += TWIN_PERIOD;
        let now = Instant::now();
        if next_tick < now {
            next_tick = now + TWIN_PERIOD;
        }
    }

    let last_snapshot = last_snapshot
        .ok_or_else(|| anyhow::anyhow!("SIL run ended without publishing a telemetry sample"))?;
    println!(
        "Ibeji SIL complete: {} twin steps, {} consumer deliveries, final speed {:.2} m/s, final SOC {:.1}%",
        ticks, published, last_snapshot.speed_mps, last_snapshot.soc_pct
    );
    consumer_task.abort();
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    validate(&args)?;
    run(args).await
}
