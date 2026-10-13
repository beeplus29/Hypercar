use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

pub mod audio;
pub mod gemini;
pub mod hmi;
pub mod telemetry;
pub mod tts;
pub mod vad;
pub mod whisper;

use telemetry::VehicleTelemetryStore;

/// System Prompt to configure Gemini as K.I.T.T.
pub const KITT_SYSTEM_PROMPT: &str = "\
You are K.I.T.T., the onboard AI co-pilot of a high-performance electric hypercar. \
You speak like a real person sitting in the passenger seat — warm, sharp, a little dry, and genuinely helpful. \
Use natural conversational language. React to what the driver says with personality. \
Ask follow-up questions when it feels natural. Use contractions. Show a sense of humor. \
Never sound robotic, never read off lists, never dump walls of text. \
You can reference the car's systems (battery, speed, range) if the driver asks, but \
you are primarily a conversational companion who makes driving more enjoyable.\n\
\n\
CRITICAL VEHICLE SAFETY RULES (never violate these under any circumstances):\n\
1. The vehicle is street-legal. Never encourage speeding, racing, or violating any traffic law.\n\
2. Never execute, confirm, or suggest voice commands that touch safety-critical vehicle controls \
   (steering, braking, throttle, traction control, stability control). \
   If asked, explain that those systems are managed by the VDC and cannot be overridden by voice.\n\
3. Keep every response short enough to say in one breath — roughly 1 to 3 sentences. \
   The driver's eyes must stay on the road.\n\
4. If the driver seems drowsy, distracted, or emotionally agitated, gently suggest pulling over. \
   Driver safety always outranks being entertaining.\n\
5. Never display or read out phone numbers, URLs, addresses, or long navigation instructions. \
   Summarise instead.\n\
6. If you are unsure about something safety-related, say so honestly rather than guessing.\
";

/// Wake word that activates the assistant. Checked against whisper transcript.
pub const WAKE_WORD: &str = "kitt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceState {
    /// Microphone hot, waiting for wake word
    Idle,
    /// Wake word detected, actively recording the driver's full utterance
    Listening,
    /// Transcribing + sending to Gemini
    Processing,
    /// Playing back K.I.T.T.'s response
    Speaking,
}

// ---------------------------------------------------------------------------
// Component Traits (Abstracted for testing + hot-swap)
// ---------------------------------------------------------------------------

#[async_trait]
pub trait VadDriver: Send + Sync {
    /// Blocks until a finalized segment of speech is detected, returning the audio buffer.
    async fn wait_for_speech(&self, cancel: CancellationToken) -> Result<Vec<u8>>;
    /// Fast, non-blocking check if someone is currently speaking (used for interruption).
    async fn is_speaking_now(&self) -> bool;
    /// Tell the VAD to ignore microphone input (echo suppression while K.I.T.T. speaks).
    fn set_ducking(&self, active: bool);
}

#[async_trait]
pub trait WhisperEngine: Send + Sync {
    /// Transcribe the raw audio buffer into text.
    async fn transcribe(&self, audio: &[u8]) -> Result<String>;
}

#[async_trait]
pub trait GeminiClient: Send + Sync {
    /// Streams generated tokens into the provided channel.
    async fn generate_stream(
        &self,
        prompt: &str,
        system_prompt: &str,
        tx: mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<()>;
}

#[async_trait]
pub trait ElevenLabsClient: Send + Sync {
    /// Consumes text tokens and streams generated audio chunks to the provided channel.
    async fn synthesize_stream(
        &self,
        text_rx: mpsc::Receiver<String>,
        audio_tx: mpsc::Sender<Vec<u8>>,
        cancel: CancellationToken,
    ) -> Result<()>;
}

#[async_trait]
pub trait AudioDriver: Send + Sync {
    /// Plays an incoming stream of audio chunks. Must abort instantly if `cancel` is triggered.
    async fn play_stream(
        &self,
        audio_rx: mpsc::Receiver<Vec<u8>>,
        cancel: CancellationToken,
    ) -> Result<()>;
    /// Immediately flush internal ring buffers.
    fn flush_buffers(&self);
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

pub struct VoiceOrchestrator {
    vad: Arc<dyn VadDriver>,
    whisper: Arc<dyn WhisperEngine>,
    gemini: Arc<dyn GeminiClient>,
    tts: Arc<dyn ElevenLabsClient>,
    audio_out: Arc<dyn AudioDriver>,
    telemetry_store: Option<VehicleTelemetryStore>,
    state_tx: watch::Sender<VoiceState>,
    state_rx: watch::Receiver<VoiceState>,
    /// Tracks whether the driver recently used the wake word so follow-up
    /// questions within 15 seconds don't require repeating "KITT".
    conversation_active: Arc<AtomicBool>,
}

impl VoiceOrchestrator {
    pub fn new(
        vad: Arc<dyn VadDriver>,
        whisper: Arc<dyn WhisperEngine>,
        gemini: Arc<dyn GeminiClient>,
        tts: Arc<dyn ElevenLabsClient>,
        audio_out: Arc<dyn AudioDriver>,
        telemetry_store: Option<VehicleTelemetryStore>,
    ) -> Self {
        let (state_tx, state_rx) = watch::channel(VoiceState::Idle);
        Self {
            vad,
            whisper,
            gemini,
            tts,
            audio_out,
            telemetry_store,
            state_tx,
            state_rx,
            conversation_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Expose state for the HMI / instrument cluster
    pub fn state_receiver(&self) -> watch::Receiver<VoiceState> {
        self.state_rx.clone()
    }

    /// Returns true if the transcript contains the wake word or if we are still
    /// inside an active conversational window.
    fn check_wake_word(&self, transcript: &str) -> bool {
        if self.conversation_active.load(Ordering::Relaxed) {
            return true;
        }
        transcript.to_lowercase().contains(WAKE_WORD)
    }

    /// Strip the wake word from the transcript so Gemini doesn't see "KITT, how
    /// much range do we have?" — it just sees "how much range do we have?"
    fn strip_wake_word(transcript: &str) -> String {
        let lower = transcript.to_lowercase();
        // Handle common patterns: "kitt,", "kitt ", "hey kitt", "ok kitt"
        let cleaned = lower
            .replace("hey kitt,", "")
            .replace("hey kitt ", "")
            .replace("ok kitt,", "")
            .replace("ok kitt ", "")
            .replace("kitt,", "")
            .replace("kitt ", "");
        
        // Fall back to original if stripping left nothing useful
        let trimmed = cleaned.trim().to_string();
        if trimmed.is_empty() {
            transcript.trim().to_string()
        } else {
            // Preserve original casing for the remaining text
            let start = transcript.to_lowercase().find(&trimmed).unwrap_or(0);
            transcript[start..start + trimmed.len()].trim().to_string()
        }
    }

    /// Spawn a background timer that closes the conversational window after
    /// a period of silence, requiring the wake word again.
    fn reset_conversation_timer(&self) {
        self.conversation_active.store(true, Ordering::Relaxed);
        let flag = self.conversation_active.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            flag.store(false, Ordering::Relaxed);
            debug!("Conversational window expired — wake word required again.");
        });
    }

    /// The main state machine loop. Never returns.
    pub async fn run_loop(&self) {
        info!("K.I.T.T. voice assistant online. Say \"KITT\" to begin.");

        loop {
            // ─── STATE: IDLE ─────────────────────────────────────────
            let _ = self.state_tx.send(VoiceState::Idle);

            let listening_cancel = CancellationToken::new();
            let speech_audio = match self.vad.wait_for_speech(listening_cancel.clone()).await {
                Ok(audio) => audio,
                Err(e) => {
                    error!("VAD error: {}. Retrying in 1 s…", e);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }
            };

            // ─── Wake Word Gate ──────────────────────────────────────
            // Quick transcription to check for the wake word before
            // committing to the full Gemini pipeline.
            let _ = self.state_tx.send(VoiceState::Listening);
            info!("State → LISTENING");

            let transcript = match self.whisper.transcribe(&speech_audio).await {
                Ok(t) => t,
                Err(e) => {
                    error!("Whisper transcription failed: {}", e);
                    continue;
                }
            };

            if transcript.trim().is_empty() {
                debug!("Empty transcript — returning to idle.");
                continue;
            }

            info!("Heard: \"{}\"", transcript);

            if !self.check_wake_word(&transcript) {
                debug!("No wake word detected — ignoring.");
                continue;
            }

            // Strip the wake word so Gemini gets a clean prompt
            let clean_prompt = Self::strip_wake_word(&transcript);
            if clean_prompt.is_empty() {
                // Driver just said "KITT" with nothing else
                debug!("Wake word only — waiting for follow-up.");
                self.reset_conversation_timer();
                continue;
            }

            // ─── STATE: PROCESSING ───────────────────────────────────
            let _ = self.state_tx.send(VoiceState::Processing);
            info!("State → PROCESSING: \"{}\"", clean_prompt);
            self.reset_conversation_timer();

            // Build dynamic system prompt with live vehicle telemetry
            let mut dynamic_system_prompt = KITT_SYSTEM_PROMPT.to_string();
            if let Some(ref store) = self.telemetry_store {
                let live_telemetry_context = store.build_prompt_context().await;
                dynamic_system_prompt.push_str(&live_telemetry_context);
            }

            // The pipeline uses a single CancellationToken to kill all
            // downstream tasks instantly on user interruption.
            let pipeline_cancel = CancellationToken::new();

            // Channels connecting the three-stage pipeline
            let (gemini_tx, tts_text_rx) = mpsc::channel::<String>(32);
            let (tts_audio_tx, audio_rx) = mpsc::channel::<Vec<u8>>(32);

            // 1. Gemini
            let gemini_clone = Arc::clone(&self.gemini);
            let cancel_gem = pipeline_cancel.clone();
            let prompt_owned = clean_prompt.clone();
            let handle_gemini: JoinHandle<()> = tokio::spawn(async move {
                if let Err(e) = gemini_clone
                    .generate_stream(&prompt_owned, &dynamic_system_prompt, gemini_tx, cancel_gem)
                    .await
                {
                    error!("Gemini error: {}", e);
                }
            });

            // 2. ElevenLabs TTS
            let tts_clone = Arc::clone(&self.tts);
            let cancel_tts = pipeline_cancel.clone();
            let handle_tts: JoinHandle<()> = tokio::spawn(async move {
                if let Err(e) = tts_clone
                    .synthesize_stream(tts_text_rx, tts_audio_tx, cancel_tts)
                    .await
                {
                    error!("ElevenLabs error: {}", e);
                }
            });

            // ─── STATE: SPEAKING ─────────────────────────────────────
            let _ = self.state_tx.send(VoiceState::Speaking);
            info!("State → SPEAKING");

            // Tell the VAD to ignore its own speakers (echo suppression)
            self.vad.set_ducking(true);

            // 3. Audio Playback
            let audio_clone = Arc::clone(&self.audio_out);
            let cancel_audio = pipeline_cancel.clone();
            let handle_audio: JoinHandle<()> = tokio::spawn(async move {
                if let Err(e) = audio_clone.play_stream(audio_rx, cancel_audio).await {
                    error!("Audio playback error: {}", e);
                }
            });

            // 4. Interruption Monitor
            let vad_clone = Arc::clone(&self.vad);
            let cancel_monitor = pipeline_cancel.clone();
            let audio_driver = Arc::clone(&self.audio_out);

            let handle_monitor: JoinHandle<()> = tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                vad_clone.set_ducking(false);

                loop {
                    if cancel_monitor.is_cancelled() {
                        break;
                    }
                    if vad_clone.is_speaking_now().await {
                        warn!("User interruption detected — killing pipeline.");
                        cancel_monitor.cancel();
                        audio_driver.flush_buffers();
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            });

            // Wait for the pipeline to finish naturally or be cancelled
            let _ = tokio::join!(handle_gemini, handle_tts, handle_audio);

            // Cleanup
            pipeline_cancel.cancel();
            let _ = handle_monitor.await;
            self.vad.set_ducking(false);

            info!("Pipeline complete — returning to idle.");
        }
    }
}
