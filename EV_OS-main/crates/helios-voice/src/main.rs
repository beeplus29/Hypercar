use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use helios_voice::audio::CpalAudioDriver;
use helios_voice::gemini::GeminiLiveClient;
use helios_voice::hmi::VoiceHmiBroadcaster;
use helios_voice::telemetry::VehicleTelemetryStore;
use helios_voice::tts::ElevenLabsWsClient;
use helios_voice::vad::CpalVadDriver;
use helios_voice::whisper::LocalWhisperEngine;
use helios_voice::VoiceOrchestrator;

#[tokio::main]
async fn main() -> Result<()> {
    // ── Structured Logging ───────────────────────────────────────────
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to set tracing subscriber");

    info!("╔══════════════════════════════════════════╗");
    info!("║   K.I.T.T. Voice Assistant — HELIOS OS   ║");
    info!("╚══════════════════════════════════════════╝");

    // ── Configuration ────────────────────────────────────────────────
    let gemini_api_key = std::env::var("GEMINI_API_KEY")
        .context("Set GEMINI_API_KEY environment variable")?;
    let elevenlabs_api_key = std::env::var("ELEVENLABS_API_KEY")
        .context("Set ELEVENLABS_API_KEY environment variable")?;

    let whisper_model = std::env::var("WHISPER_MODEL_PATH")
        .unwrap_or_else(|_| "/opt/helios/models/ggml-base.en.bin".to_string());
    let elevenlabs_voice_id = std::env::var("ELEVENLABS_VOICE_ID")
        .unwrap_or_else(|_| "21m00Tcm4TlvDq8ikWAM".to_string());

    // ── DDS Vehicle Telemetry Subscriber ─────────────────────────────
    info!("Initializing DDS Vehicle Telemetry store for live context…");
    let telemetry_store = VehicleTelemetryStore::new();
    telemetry_store.start_subscriber();

    // ── Component Assembly ───────────────────────────────────────────

    // 1. Microphone + VAD (16 kHz mono, VeryAggressive, ducking-aware)
    info!("Initializing microphone VAD…");
    let vad = Arc::new(CpalVadDriver::new().context("Failed to initialize microphone")?);

    // 2. Local Whisper (runs on Jetson GPU via whisper.cpp)
    info!("Loading Whisper model from {}…", whisper_model);
    let whisper = Arc::new(
        LocalWhisperEngine::new(&whisper_model)
            .context("Failed to load Whisper model")?,
    );

    // 3. Gemini (streaming SSE with conversation history)
    let gemini = Arc::new(GeminiLiveClient::new(
        gemini_api_key,
        "gemini-2.5-flash",
    ));

    // 4. ElevenLabs TTS (WebSocket streaming, Turbo v2.5)
    let tts = Arc::new(ElevenLabsWsClient::new(
        elevenlabs_api_key,
        &elevenlabs_voice_id,
        "eleven_turbo_v2_5",
    ));

    // 5. Audio Output (lock-free ring buffer → CPAL hardware)
    let audio_out = Arc::new(CpalAudioDriver::new(16000, 1, 32000));

    // ── Create Orchestrator & Wire HMI Broadcaster ────────────────────
    let orchestrator = VoiceOrchestrator::new(
        vad,
        whisper,
        gemini,
        tts,
        audio_out,
        Some(telemetry_store),
    );

    // Start HMI broadcaster task to sync visualizer / cluster UI with VoiceState
    VoiceHmiBroadcaster::start(orchestrator.state_receiver());

    // ── Run ──────────────────────────────────────────────────────────
    info!("All systems nominal. Say \"KITT\" to begin.");
    orchestrator.run_loop().await;

    Ok(())
}
