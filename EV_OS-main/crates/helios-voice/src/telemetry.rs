use anyhow::Result;
use helios_dds::{Participant, QosProfile, VehicleTelemetry, HELIOS_DOMAIN, TOPIC_TELEMETRY};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

#[derive(Clone)]
pub struct VehicleTelemetryStore {
    latest: Arc<Mutex<Option<VehicleTelemetry>>>,
}

impl VehicleTelemetryStore {
    pub fn new() -> Self {
        Self {
            latest: Arc::new(Mutex::new(None)),
        }
    }

    /// Spawns a background thread that continuously polls the DDS `TOPIC_TELEMETRY`
    pub fn start_subscriber(&self) {
        let store = self.latest.clone();

        tokio::task::spawn_blocking(move || {
            let participant = match Participant::new(HELIOS_DOMAIN) {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to create DDS participant for voice telemetry: {}", e);
                    return;
                }
            };

            let reader = match participant.reader::<VehicleTelemetry>(TOPIC_TELEMETRY, QosProfile::Telemetry) {
                Ok(r) => r,
                Err(e) => {
                    warn!("Failed to create DDS reader for voice telemetry: {}", e);
                    return;
                }
            };

            info!("Subscribed to {} for K.I.T.T. live telemetry context", TOPIC_TELEMETRY);

            loop {
                match reader.take_timeout(std::time::Duration::from_millis(200)) {
                    Ok(Some(sample)) => {
                        let mut lock = store.blocking_lock();
                        *lock = Some(sample);
                    }
                    Ok(None) => {}
                    Err(e) => {
                        debug!("DDS telemetry poll error: {:?}", e);
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
            }
        });
    }

    /// Formats the current vehicle state into a clean prompt context for Gemini
    pub async fn build_prompt_context(&self) -> String {
        let lock = self.latest.lock().await;
        match &*lock {
            Some(t) => {
                let speed_mph = t.speed_mps * 2.23694;
                let mode_str = match t.drive_mode {
                    0 => "Road",
                    1 => "Sport",
                    2 => "Track",
                    3 => "Drift",
                    _ => "Unknown",
                };
                let gear_str = match t.gear {
                    0 => "Neutral",
                    1 => "Drive",
                    2 => "Reverse",
                    3 => "Park",
                    _ => "Drive",
                };

                format!(
                    "\n[LIVE VEHICLE TELEMETRY CONTEXT]\n\
                     - Speed: {:.1} mph ({:.1} m/s)\n\
                     - Battery SoC: {:.1}%\n\
                     - Battery Cell Temp: {:.1} °C\n\
                     - Drive Mode: {}\n\
                     - Current Gear: {}\n\
                     - Wheel Torques (FL/FR/RL/RR): [{:.1}, {:.1}, {:.1}, {:.1}] Nm\n\
                     - Active Fault Code: 0x{:08X}\n",
                    speed_mph,
                    t.speed_mps,
                    t.soc_pct,
                    t.cell_temp_c,
                    mode_str,
                    gear_str,
                    t.motor_torque_nm[0],
                    t.motor_torque_nm[1],
                    t.motor_torque_nm[2],
                    t.motor_torque_nm[3],
                    t.fault_bits
                )
            }
            None => {
                "\n[LIVE VEHICLE TELEMETRY CONTEXT]\n\
                 - Telemetry state: Standby / Initializing (No active DDS telemetry feed)\n"
                    .to_string()
            }
        }
    }
}
