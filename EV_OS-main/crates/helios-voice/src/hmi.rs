use crate::VoiceState;
use tokio::sync::watch;
use tracing::info;

pub struct VoiceHmiBroadcaster;

impl VoiceHmiBroadcaster {
    /// Listens to VoiceState changes and broadcasts status to HMI / Instrument Cluster
    pub fn start(mut rx: watch::Receiver<VoiceState>) {
        tokio::spawn(async move {
            let mut last_state = *rx.borrow();

            while rx.changed().await.is_ok() {
                let new_state = *rx.borrow();
                if new_state != last_state {
                    last_state = new_state;

                    let (status_code, status_name, visualizer_animation) = match new_state {
                        VoiceState::Idle => (0, "IDLE", "scanner_off"),
                        VoiceState::Listening => (1, "LISTENING", "scanner_pulse_red"),
                        VoiceState::Processing => (2, "PROCESSING", "scanner_sweep_fast"),
                        VoiceState::Speaking => (3, "SPEAKING", "audio_waveform_active"),
                    };

                    info!(
                        "HMI Voice State Update -> State: {} ({}), Visualizer: '{}'",
                        status_name, status_code, visualizer_animation
                    );

                    // Here we can also publish to gRPC / VHAL property or socket for Android Automotive
                }
            }
        });
    }
}
