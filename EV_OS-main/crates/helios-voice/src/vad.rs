use crate::VadDriver;
use anyhow::{Context, Result};
use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, StreamConfig};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};
use webrtc_vad::{Vad, VadMode};

/// Energy threshold multiplier when ducking is active.
/// The VAD still listens, but requires significantly louder input to trigger,
/// filtering out K.I.T.T.'s own speakers bleeding into the cabin mic.
const DUCKING_ENERGY_MULTIPLIER: f32 = 4.0;

thread_local! {
    static THREAD_VAD: RefCell<Vad> = RefCell::new({
        let mut vad = Vad::new();
        vad.set_mode(VadMode::VeryAggressive);
        vad
    });
}

pub struct CpalVadDriver {
    /// Atomic flag updated by the audio callback for instant interruption checks
    is_speaking: Arc<AtomicBool>,
    /// When true, raise the energy threshold so the VAD ignores speaker bleed
    ducking: Arc<AtomicBool>,
    /// Channel to receive finalized speech buffers
    speech_rx: tokio::sync::Mutex<mpsc::Receiver<Vec<u8>>>,
    /// Signals the audio thread to release its thread-confined CPAL stream.
    stop_tx: std_mpsc::Sender<()>,
}

impl CpalVadDriver {
    pub fn new() -> Result<Self> {
        let sample_rate = 16000; // Whisper and WebRTC VAD prefer 16kHz
        let is_speaking = Arc::new(AtomicBool::new(false));
        let is_speaking_cb = is_speaking.clone();

        let ducking = Arc::new(AtomicBool::new(false));
        let ducking_cb = ducking.clone();

        let rms_energy = Arc::new(AtomicU32::new(0));
        let rms_cb = rms_energy.clone();

        let (speech_tx, speech_rx) = mpsc::channel(4);

        let (ready_tx, ready_rx) = std_mpsc::sync_channel(1);
        let (stop_tx, stop_rx) = std_mpsc::channel();
        let _audio_thread = thread::Builder::new()
            .name("helios-vad-audio".to_string())
            .spawn(move || {
                if let Err(e) = Self::run_audio_thread(
                    sample_rate,
                    is_speaking_cb,
                    ducking_cb,
                    rms_cb,
                    speech_tx,
                    stop_rx,
                    ready_tx,
                ) {
                    error!("Microphone VAD thread failed: {e:#}");
                }
            })
            .context("Failed to start microphone audio thread")?;

        ready_rx
            .recv()
            .context("Microphone audio thread exited during startup")?
            .map_err(anyhow::Error::msg)?;
        info!("Microphone VAD stream active (VeryAggressive, ducking-aware)");

        Ok(Self { is_speaking, ducking, speech_rx: tokio::sync::Mutex::new(speech_rx), stop_tx })
    }

    fn run_audio_thread(
        sample_rate: u32,
        is_speaking: Arc<AtomicBool>,
        ducking: Arc<AtomicBool>,
        rms_energy: Arc<AtomicU32>,
        speech_tx: mpsc::Sender<Vec<u8>>,
        stop_rx: std_mpsc::Receiver<()>,
        ready_tx: std_mpsc::SyncSender<std::result::Result<(), String>>,
    ) -> Result<()> {
        let host = cpal::default_host();
        let device = match host.default_input_device().context("No input device available") {
            Ok(device) => device,
            Err(error) => {
                let _ = ready_tx.send(Err(error.to_string()));
                return Err(error);
            }
        };
        let config = StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: BufferSize::Default,
        };

        let mut active_buffer = Vec::<i16>::new();
        let mut silence_frames: u32 = 0;
        let silence_threshold: u32 = 60;
        let min_speech_frames: usize = 30 * 160;
        let is_speaking_cb = is_speaking.clone();
        let ducking_cb = ducking.clone();
        let rms_cb = rms_energy.clone();

        let stream = match device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let i16_data: Vec<i16> =
                    data.iter().map(|&sample| (sample * i16::MAX as f32) as i16).collect();
                let sum_sq: f64 = data.iter().map(|&sample| f64::from(sample).powi(2)).sum();
                let rms = (sum_sq / data.len().max(1) as f64).sqrt() as f32;
                rms_cb.store(rms.to_bits(), Ordering::Relaxed);

                let ducking_active = ducking_cb.load(Ordering::Relaxed);
                let energy_floor = if ducking_active { 0.02 * DUCKING_ENERGY_MULTIPLIER } else { 0.0 };

                const FRAME_SIZE: usize = 160;
                for chunk in i16_data.chunks(FRAME_SIZE) {
                    if chunk.len() < FRAME_SIZE {
                        continue;
                    }

                    let is_voice = if rms < energy_floor {
                        false
                    } else {
                        THREAD_VAD.with(|vad| vad.borrow_mut().is_voice_segment(chunk).unwrap_or(false))
                    };

                    if is_voice {
                        silence_frames = 0;
                        if !is_speaking_cb.swap(true, Ordering::Relaxed) {
                            debug!("VAD: speech onset");
                        }
                        active_buffer.extend_from_slice(chunk);
                    } else if is_speaking_cb.load(Ordering::Relaxed) {
                        active_buffer.extend_from_slice(chunk);
                        silence_frames += 1;

                        if silence_frames >= silence_threshold {
                            is_speaking_cb.store(false, Ordering::Relaxed);
                            if active_buffer.len() >= min_speech_frames {
                                debug!(
                                    "VAD: speech finalized — {} samples ({:.1} s)",
                                    active_buffer.len(),
                                    active_buffer.len() as f32 / sample_rate as f32
                                );
                                let byte_buffer = bytemuck::cast_slice(&active_buffer).to_vec();
                                let _ = speech_tx.try_send(byte_buffer);
                            } else {
                                debug!("VAD: rejected short utterance ({} samples)", active_buffer.len());
                            }
                            active_buffer.clear();
                            silence_frames = 0;
                        }
                    }
                }
            },
            |err| error!("CPAL input stream error: {}", err),
            None,
        ) {
            Ok(stream) => stream,
            Err(error) => {
                let error = anyhow::Error::from(error);
                let _ = ready_tx.send(Err(error.to_string()));
                return Err(error);
            }
        };

        if let Err(error) = stream.play() {
            let error = anyhow::Error::from(error);
            let _ = ready_tx.send(Err(error.to_string()));
            return Err(error);
        }
        ready_tx
            .send(Ok(()))
            .map_err(|_| anyhow::anyhow!("Microphone audio startup receiver was dropped"))?;
        while stop_rx.recv_timeout(std::time::Duration::from_millis(100))
            == Err(std_mpsc::RecvTimeoutError::Timeout)
        {}
        drop(stream);
        Ok(())
    }
}

impl Drop for CpalVadDriver {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(());
    }
}

#[async_trait]
impl VadDriver for CpalVadDriver {
    async fn wait_for_speech(&self, cancel: CancellationToken) -> Result<Vec<u8>> {
        let mut rx = self.speech_rx.lock().await;

        tokio::select! {
            speech_opt = rx.recv() => {
                speech_opt.context("VAD channel closed unexpectedly")
            }
            _ = cancel.cancelled() => {
                anyhow::bail!("wait_for_speech cancelled by orchestrator");
            }
        }
    }

    async fn is_speaking_now(&self) -> bool {
        self.is_speaking.load(Ordering::Relaxed)
    }

    fn set_ducking(&self, active: bool) {
        self.ducking.store(active, Ordering::Relaxed);
        debug!("VAD ducking {}", if active { "ENABLED" } else { "DISABLED" });
    }
}
