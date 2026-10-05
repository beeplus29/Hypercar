use crate::WhisperEngine;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct LocalWhisperEngine {
    // The context is thread-safe, but state generation needs mutability. 
    // We'll wrap it in an Arc to share across blocking tasks.
    ctx: Arc<WhisperContext>,
}

impl LocalWhisperEngine {
    pub fn new(model_path: &str) -> Result<Self> {
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path, params)
            .context("Failed to load whisper model")?;
        
        Ok(Self {
            ctx: Arc::new(ctx),
        })
    }
}

#[async_trait]
impl WhisperEngine for LocalWhisperEngine {
    async fn transcribe(&self, audio: &[u8]) -> Result<String> {
        // Assume incoming audio is 16-bit PCM (s16le).
        // Whisper expects 32-bit floats at 16kHz.
        let samples_f32: Vec<f32> = bytemuck::cast_slice::<u8, i16>(audio)
            .iter()
            .map(|&s| s as f32 / 32768.0)
            .collect();

        let ctx = self.ctx.clone();

        // Whisper inference is heavy and synchronous. 
        // We offload it to a tokio blocking thread to avoid starving the async reactor.
        let transcript = tokio::task::spawn_blocking(move || -> Result<String> {
            let mut state = ctx.create_state().context("Failed to create whisper state")?;
            
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some("en"));
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_timestamps(false);

            state.full(params, &samples_f32).context("Failed to run model")?;

            let num_segments = state.full_n_segments().context("Failed to get segments")?;
            let mut text = String::new();
            for i in 0..num_segments {
                if let Ok(segment) = state.full_get_segment_text(i) {
                    text.push_str(&segment);
                }
            }
            Ok(text.trim().to_string())
        })
        .await??;

        Ok(transcript)
    }
}
