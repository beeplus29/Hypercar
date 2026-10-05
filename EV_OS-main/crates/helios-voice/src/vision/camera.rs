use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug, Default)]
pub struct DriverStatus {
    pub is_drowsy: bool,
    pub is_distracted: bool,
    pub eyes_on_road: bool,
    pub passenger_detected: bool,
}

#[derive(Clone)]
pub struct CabinVisionStore {
    latest_frame_jpeg: Arc<Mutex<Option<Vec<u8>>>>,
    driver_status: Arc<Mutex<DriverStatus>>,
}

impl CabinVisionStore {
    pub fn new() -> Self {
        Self {
            latest_frame_jpeg: Arc::new(Mutex::new(None)),
            driver_status: Arc::new(Mutex::new(DriverStatus {
                eyes_on_road: true,
                ..Default::default()
            })),
        }
    }

    /// Update the latest cabin camera snapshot (JPEG format)
    pub async fn set_latest_frame(&self, jpeg_data: Vec<u8>) {
        let mut lock = self.latest_frame_jpeg.lock().await;
        *lock = Some(jpeg_data);
    }

    /// Retrieve the current cabin camera snapshot for Gemini VLM inspection
    pub async fn get_latest_frame(&self) -> Option<Vec<u8>> {
        let lock = self.latest_frame_jpeg.lock().await;
        lock.clone()
    }

    /// Update real-time Driver Monitoring System (DMS) status metrics
    pub async fn update_driver_status(&self, status: DriverStatus) {
        let mut lock = self.driver_status.lock().await;
        *lock = status;
    }

    /// Returns a text summary of the cabin state for system prompt injection
    pub async fn get_cabin_status_prompt(&self) -> String {
        let status = self.driver_status.lock().await;
        format!(
            "\n[CABIN INTERIOR VISION STATUS]\n\
             - Eyes on Road: {}\n\
             - Driver Drowsiness Alert: {}\n\
             - Driver Distraction (Phone/Other): {}\n\
             - Front Passenger Occupancy: {}\n",
            if status.eyes_on_road { "Yes" } else { "No" },
            if status.is_drowsy { "WARNING (Drowsy)" } else { "Normal" },
            if status.is_distracted { "WARNING (Distracted)" } else { "Clear" },
            if status.passenger_detected { "Occupied" } else { "Empty" }
        )
    }
}
