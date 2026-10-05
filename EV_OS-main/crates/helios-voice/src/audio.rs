use crate::AudioDriver;
use anyhow::{Context, Result};
use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, StreamConfig};
use crossbeam_queue::ArrayQueue;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

pub struct CpalAudioDriver {
    rb: Arc<ArrayQueue<f32>>,
    sample_rate: u32,
    channels: u16,
}

impl CpalAudioDriver {
    pub fn new(sample_rate: u32, channels: u16, buffer_size: usize) -> Self {
        Self { 
            rb: Arc::new(ArrayQueue::new(buffer_size)), 
            sample_rate, 
            channels 
        }
    }
}

#[async_trait]
impl AudioDriver for CpalAudioDriver {
    async fn play_stream(
        &self,
        mut audio_rx: mpsc::Receiver<Vec<u8>>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let host = cpal::default_host();
        let device = host.default_output_device().context("No output device available")?;
        
        let config = StreamConfig {
            channels: self.channels,
            sample_rate: cpal::SampleRate(self.sample_rate),
            buffer_size: BufferSize::Default,
        };

        let rb_consumer = self.rb.clone();

        // The audio callback pulls samples from the lock-free ring buffer
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for sample in data.iter_mut() {
                    *sample = rb_consumer.pop().unwrap_or(0.0);
                }
            },
            |err| error!("CPAL audio stream error: {}", err),
            None,
        )?;

        stream.play()?;
        info!("Audio stream playing...");

        loop {
            tokio::select! {
                chunk_opt = audio_rx.recv() => {
                    match chunk_opt {
                        Some(chunk) => {
                            // Convert incoming raw PCM bytes to f32 samples
                            let samples: &[f32] = bytemuck::cast_slice(&chunk);
                            
                            // Push to ring buffer, waiting if full
                            for &sample in samples {
                                while self.rb.push(sample).is_err() {
                                    if cancel.is_cancelled() {
                                        return Ok(());
                                    }
                                    // Yield to allow CPAL to consume samples
                                    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                                }
                            }
                        }
                        None => {
                            // Upstream closed (TTS finished generating)
                            break;
                        }
                    }
                }
                _ = cancel.cancelled() => {
                    debug!("Audio playback task cancelled");
                    break;
                }
            }
        }

        // Keep stream alive until buffer drains, unless cancelled
        while !self.rb.is_empty() && !cancel.is_cancelled() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        stream.pause()?;
        Ok(())
    }

    fn flush_buffers(&self) {
        // Drain all samples instantly
        while self.rb.pop().is_some() {}
        debug!("Hardware audio buffer flushed.");
    }
}
