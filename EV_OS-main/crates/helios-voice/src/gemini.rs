use crate::{GeminiClient, KITT_SYSTEM_PROMPT};
use anyhow::{Context, Result};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

pub struct GeminiLiveClient {
    api_key: String,
    client: Client,
    model: String,
    // Conversation history to make it conversational like a real person
    history: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl GeminiLiveClient {
    pub fn new(api_key: String, model: &str) -> Self {
        Self {
            api_key,
            client: Client::new(),
            model: model.to_string(),
            history: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub async fn clear_history(&self) {
        let mut hist = self.history.lock().await;
        hist.clear();
    }
}

#[async_trait]
impl GeminiClient for GeminiLiveClient {
    async fn generate_stream(
        &self,
        prompt: &str,
        system_prompt: &str,
        tx: mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.model, self.api_key
        );

        // 1. Prepare and append user prompt to history
        let user_msg = json!({
            "role": "user",
            "parts": [ { "text": prompt } ]
        });
        
        let mut contents = {
            let mut hist = self.history.lock().await;
            hist.push(user_msg);
            
            // Limit history to last 10 messages (5 turns) to save context window 
            // and keep responses snappy for a voice assistant.
            if hist.len() > 10 {
                let excess = hist.len() - 10;
                hist.drain(0..excess);
            }
            hist.clone()
        };

        let body = json!({
            "systemInstruction": {
                "parts": [ { "text": system_prompt } ]
            },
            "contents": contents
        });

        let request = self.client.post(&url).json(&body).send();

        // Allow instant cancellation while waiting for initial connection
        let mut response = tokio::select! {
            res = request => res.context("Failed to connect to Gemini API")?,
            _ = cancel.cancelled() => {
                debug!("Gemini request cancelled during connection phase");
                return Ok(());
            }
        };

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Gemini API error: {}", error_text);
        }

        let mut stream = response.bytes_stream().eventsource();
        let mut full_response = String::new();

        while let Some(event_result) = tokio::select! {
            ev = stream.next() => ev,
            _ = cancel.cancelled() => {
                debug!("Gemini request cancelled during streaming phase");
                None // Break loop early
            }
        } {
            match event_result {
                Ok(event) => {
                    let data = event.data;
                    if data.is_empty() { continue; }

                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                        if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                            full_response.push_str(text);
                            if tx.send(text.to_string()).await.is_err() {
                                // Downstream receiver dropped (e.g. TTS pipeline closed), abort
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("SSE stream error: {}", e);
                    break;
                }
            }
        }

        // 2. Save KITT's response to history so it remembers what it just said
        if !full_response.trim().is_empty() {
            let mut hist = self.history.lock().await;
            hist.push(json!({
                "role": "model",
                "parts": [ { "text": full_response } ]
            }));
        }

        Ok(())
    }
}
