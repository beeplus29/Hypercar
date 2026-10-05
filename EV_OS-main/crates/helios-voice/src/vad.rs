use crate::VadDriver;
use anyhow::{Context, Result};
use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, StreamConfig};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};
use webrtc_vad::{Vad, VadMode};

/// Energy threshold multiplier when ducking is active.
/// The VAD still listens, but requires significantly louder input to trigger,
/// filtering out K.I.T.T.'s own speakers bleeding into the cabin mic.
const DUCKING_ENERGY_MULTIPLIER: f32 = 4.0;

pub struct CpalVadDriver {
    /// Atomic flag updated by the audio callback for instant interruption checks
    is_speaking: Arc<AtomicBool>,
    /// When true, raise the energy threshold so the VAD ignores speaker bleed
    ducking: Arc<AtomicBool>,
    /// Running RMS energy estimate (stored as f32 bits for atomic access)
    rms_energy: Arc<AtomicU32>,
    /// Channel to receive finalized speech buffers
    speech_rx: tokio::sync::Mutex<mpsc::Receiver<Vec<u8>>>,
    /// We keep the CPAL stream alive by holding its handle
    _stream: cpal::Stream,
}

impl CpalVadDriver {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_input_device().context("No input device available")?;

        let sample_rate = 16000; // Whisper and WebRTC VAD prefer 16kHz
        let config = StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: BufferSize::Default,
        };

        let mut vad = Vad::new();
        // Very Aggressive mode is best for in-cabin automotive environments
        // where road noise, wind, and HVAC constantly hit the microphone.
        vad.set_mode(VadMode::VeryAggressive);

        let is_speaking = Arc::new(AtomicBool::new(false));
        let is_speaking_cb = is_speaking.clone();

        let ducking = Arc::new(AtomicBool::new(false));
        let ducking_cb = ducking.clone();

        let rms_energy = Arc::new(AtomicU32::new(0));
        let rms_cb = rms_energy.clone();

        let (speech_tx, speech_rx) = mpsc::channel(4);

        // Callback state
        let mut active_buffer = Vec::<i16>::new();
        let mut silence_frames: u32 = 0;
        // 60 frames × 10 ms = 600 ms of silence to finalize.
        // Slightly longer than typical to prevent clipping the driver mid-sentence
        // at highway speeds where road noise can cause brief VAD dropouts.
        let silence_threshold: u32 = 60;
        // Minimum speech length: 300 ms (30 frames). Reject pops/clicks.
        let min_speech_frames: usize = 30 * 160;

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Convert f32 → i16 for WebRTC VAD
                let i16_data: Vec<i16> = data.iter().map(|&s| (s * i16::MAX as f32) as i16).collect();

                // Compute RMS energy of this callback buffer
                let sum_sq: f64 = data.iter().map(|&s| (s as f64) * (s as f64)).sum();
                let rms = (sum_sq / data.len().max(1) as f64).sqrt() as f32;
                rms_cb.store(rms.to_bits(), Ordering::Relaxed);

                // When ducking is active, require the RMS energy to be well above
                // the speaker bleed floor before even asking the VAD.
                let ducking_active = ducking_cb.load(Ordering::Relaxed);
                let energy_floor = if ducking_active { 0.02 * DUCKING_ENERGY_MULTIPLIER } else { 0.0 };

                const FRAME_SIZE: usize = 160; // 10 ms at 16 kHz

                for chunk in i16_data.chunks(FRAME_SIZE) {
                    if chunk.len() < FRAME_SIZE {
                        continue; // Partial frame — skip
                    }

                    let is_voice = if rms < energy_floor {
                        false // Below ducking floor, suppress
                    } else {
                        vad.is_voice_segment(chunk).unwrap_or(false)
                    };

                    if is_voice {
                        silence_frames = 0;
                        if !is_speaking_cb.load(Ordering::Relaxed) {
                            debug!("VAD: speech onset");
                            is_speaking_cb.store(true, Ordering::Relaxed);
                        }
                        active_buffer.extend_from_slice(chunk);
                    } else if is_speaking_cb.load(Ordering::Relaxed) {
                        // Still recording — append silence to preserve natural cadence
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
                                let byte_buffer: Vec<u8> =
                                    bytemuck::cast_slice(&active_buffer).to_vec();
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
        )?;

        stream.play()?;
        info!("Microphone VAD stream active (VeryAggressive, ducking-aware)");

        Ok(Self {
            is_speaking,
            ducking,
            rms_energy,
            speech_rx: tokio::sync::Mutex::new(speech_rx),
            _stream: stream,
        })
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
