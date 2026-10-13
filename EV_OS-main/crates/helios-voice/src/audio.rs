use crate::AudioDriver;
use anyhow::{Context, Result};
use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, StreamConfig};
use crossbeam_queue::ArrayQueue;
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use std::thread;
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
        Self { rb: Arc::new(ArrayQueue::new(buffer_size)), sample_rate, channels }
    }
}

#[async_trait]
impl AudioDriver for CpalAudioDriver {
    async fn play_stream(
        &self,
        mut audio_rx: mpsc::Receiver<Vec<u8>>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let (ready_tx, ready_rx) = std_mpsc::sync_channel(1);
        let (stop_tx, stop_rx) = std_mpsc::channel();
        let rb = self.rb.clone();
        let sample_rate = self.sample_rate;
        let channels = self.channels;
        let worker_cancel = cancel.clone();
        let worker = thread::Builder::new()
            .name("helios-audio-output".to_string())
            .spawn(move || {
                Self::run_audio_thread(rb, sample_rate, channels, worker_cancel, stop_rx, ready_tx)
            })
            .context("Failed to start audio output thread")?;

        let startup = tokio::task::spawn_blocking(move || ready_rx.recv())
            .await
            .context("Audio output startup task failed")?
            .context("Audio output thread exited during startup")?;
        if let Err(message) = startup {
            let _ = stop_tx.send(());
            let worker_result = tokio::task::spawn_blocking(move || worker.join())
                .await
                .context("Audio output join task failed")?;
            let _worker_result =
                worker_result.map_err(|_| anyhow::anyhow!("Audio output thread panicked"))?;
            return Err(anyhow::anyhow!(message));
        }
        info!("Audio stream playing...");

        let mut playback_result: Result<()> = Ok(());
        'playback: loop {
            tokio::select! {
                chunk_opt = audio_rx.recv() => {
                    match chunk_opt {
                        Some(chunk) => {
                            let samples: &[f32] = match bytemuck::try_cast_slice(&chunk) {
                                Ok(samples) => samples,
                                Err(error) => {
                                    playback_result =
                                        Err(anyhow::anyhow!("Invalid f32 PCM audio chunk: {error}"));
                                    break 'playback;
                                }
                            };

                            for &sample in samples {
                                while self.rb.push(sample).is_err() {
                                    if cancel.is_cancelled() {
                                        break 'playback;
                                    }
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

        while !self.rb.is_empty() && !cancel.is_cancelled() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let _ = stop_tx.send(());
        let worker_result = tokio::task::spawn_blocking(move || worker.join())
            .await
            .context("Audio output join task failed")?
            .map_err(|_| anyhow::anyhow!("Audio output thread panicked"))?;
        playback_result?;
        worker_result.map_err(anyhow::Error::msg)
    }

    fn flush_buffers(&self) {
        // Drain all samples instantly
        while self.rb.pop().is_some() {}
        debug!("Hardware audio buffer flushed.");
    }
}

impl CpalAudioDriver {
    fn run_audio_thread(
        rb: Arc<ArrayQueue<f32>>,
        sample_rate: u32,
        channels: u16,
        cancel: CancellationToken,
        stop_rx: std_mpsc::Receiver<()>,
        ready_tx: std_mpsc::SyncSender<std::result::Result<(), String>>,
    ) -> std::result::Result<(), String> {
        let setup = (|| -> Result<cpal::Stream> {
            let host = cpal::default_host();
            let device = host.default_output_device().context("No output device available")?;
            let config = StreamConfig {
                channels,
                sample_rate: cpal::SampleRate(sample_rate),
                buffer_size: BufferSize::Default,
            };
            let rb_consumer = rb;
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
            Ok(stream)
        })();

        let stream = match setup {
            Ok(stream) => stream,
            Err(error) => {
                let message = format!("{error:#}");
                let _ = ready_tx.send(Err(message.clone()));
                return Err(message);
            }
        };

        if ready_tx.send(Ok(())).is_err() {
            return Err("Audio output startup receiver was dropped".to_string());
        }
        while !cancel.is_cancelled()
            && stop_rx.recv_timeout(std::time::Duration::from_millis(100))
                == Err(std_mpsc::RecvTimeoutError::Timeout)
        {}
        stream.pause().map_err(|error| error.to_string())
    }
}
