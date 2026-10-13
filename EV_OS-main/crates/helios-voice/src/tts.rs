use crate::ElevenLabsClient;
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

pub struct ElevenLabsWsClient {
    api_key: String,
    voice_id: String,
    model_id: String,
}

impl ElevenLabsWsClient {
    pub fn new(api_key: String, voice_id: &str, model_id: &str) -> Self {
        Self {
            api_key,
            voice_id: voice_id.to_string(),
            model_id: model_id.to_string(),
        }
    }
}

#[async_trait]
impl ElevenLabsClient for ElevenLabsWsClient {
    async fn synthesize_stream(
        &self,
        mut text_rx: mpsc::Receiver<String>,
        audio_tx: mpsc::Sender<Vec<u8>>,
        cancel: CancellationToken,
    ) -> Result<()> {
        // We use output_format=pcm_44100 so the AudioDriver can cast it to f32 or play it natively
        let ws_url = format!(
            "wss://api.elevenlabs.io/v1/text-to-speech/{}/stream-input?model_id={}&output_format=pcm_16000",
            self.voice_id, self.model_id
        );

        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .context("Failed to connect to ElevenLabs WebSocket")?;

        info!("Connected to ElevenLabs WebSocket");
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Send Initial Configuration (BOS) with tuned K.I.T.T. voice parameters
        let bos_msg = json!({
            "text": " ",
            "voice_settings": {
                "stability": 0.55,
                "similarity_boost": 0.80,
                "style": 0.15,
                "use_speaker_boost": true
            },
            "xi_api_key": self.api_key,
        });
        ws_sender.send(Message::Text(bos_msg.to_string())).await?;

        // 1. Task to read from Gemini text channel and forward to WebSocket
        let mut sender_cancel = cancel.clone();
        let handle_sender = tokio::spawn(async move {
            loop {
                tokio::select! {
                    text_opt = text_rx.recv() => {
                        match text_opt {
                            Some(text) => {
                                let msg = json!({ "text": format!("{} ", text), "try_trigger_generation": true });
                                if ws_sender.send(Message::Text(msg.to_string())).await.is_err() {
                                    break;
                                }
                            }
                            None => {
                                // EOS (End of Stream)
                                let eos_msg = json!({ "text": "" });
                                let _ = ws_sender.send(Message::Text(eos_msg.to_string())).await;
                                break;
                            }
                        }
                    }
                    _ = sender_cancel.cancelled() => {
                        debug!("ElevenLabs sender loop cancelled");
                        break;
                    }
                }
            }
        });

        // 2. Loop to read audio chunks back from WebSocket and send to AudioDriver
        loop {
            tokio::select! {
                msg_opt = ws_receiver.next() => {
                    match msg_opt {
                        Some(Ok(Message::Text(text))) => {
                            if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                                if let Some(audio_base64) = parsed["audio"].as_str() {
                                    if let Ok(audio_bytes) = BASE64.decode(audio_base64) {
                                        // Forward PCM bytes directly to hardware driver
                                        if audio_tx.send(audio_bytes).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                                if parsed["isFinal"].as_bool().unwrap_or(false) {
                                    break; // Generation complete
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(e)) => {
                            error!("ElevenLabs WebSocket error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
                _ = cancel.cancelled() => {
                    debug!("ElevenLabs receiver loop cancelled");
                    break;
                }
            }
        }

        // Wait for sender task to finish
        let _ = handle_sender.await;
        Ok(())
    }
}
