// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// Native media adapter for the pinned Kyber SDK. Input lifecycle remains upstream.
use super::{KymuxBackendConfig, PlatformEndpoints};
use crate::metrics::MetricsProducer;
use crate::platform::message::MessageForwarder;
use crate::platform::player::{self, PlayerListener};
use crate::platform::{InputClient, InputPipeline};
use crate::{BackendDriver, Error, MessageSender, Result};
use async_trait::async_trait;
use kymux::{kycom, types};
use log::info;

struct VideoWorker {
    forward: tokio::task::JoinHandle<()>,
    decode: Option<std::thread::JoinHandle<()>>,
    closing: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl Drop for VideoWorker {
    fn drop(&mut self) {
        self.closing
            .store(true, std::sync::atomic::Ordering::Release);
        self.forward.abort();
        if let Some(worker) = self.decode.take() {
            let _ = worker.join();
        }
    }
}
struct Player(Vec<VideoWorker>);
impl Player {
    async fn stop(self) -> Result<()> {
        // The SDK teardown must join callbacks before releasing Godot userdata.
        for mut worker in self.0 {
            worker
                .closing
                .store(true, std::sync::atomic::Ordering::Release);
            worker.forward.abort();
            let _ = (&mut worker.forward).await;
            if let Some(thread) = worker.decode.take() {
                tokio::task::spawn_blocking(move || thread.join())
                    .await?
                    .map_err(|_| Error::ThreadPanicked)?;
            }
        }
        Ok(())
    }
}
async fn create_player(
    backend_config: &KymuxBackendConfig<'_>,
    metrics: Option<&MetricsProducer>,
    endpoints: Vec<types::VideoClientEndpoint>,
    audio: Option<types::AudioClientEndpoint>,
) -> Result<Option<Player>> {
    if endpoints.is_empty() {
        return Ok(None);
    }
    if endpoints.len() != backend_config.streaming_config.video.len() {
        return Err(Error::Rejected);
    }
    let mut workers = Vec::new();
    for (config, endpoint) in backend_config.streaming_config.video.iter().zip(endpoints) {
        let player::WindowHandle::Cpu(output) = config.player.window_handle else {
            return Err(Error::Rejected);
        };
        let mut protocol = endpoint
            .ready()
            .await
            .map_err(|e| Error::KymuxReadyFailed { reason: e.0 })?;
        let listener = MessageForwarder {
            message_sender: backend_config.msg_sender.clone(),
            metrics: metrics.cloned(),
        };
        let (send, mut receive) = tokio::sync::mpsc::channel::<types::AVPacket>(4);
        let closing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopped = closing.clone();
        let decode = std::thread::Builder::new()
            .name("kyber-native-decoder".into())
            .spawn(move || {
                let mut run = || -> kyber_media::Result<()> {
                    let mut decoder = kyber_media::H264Stream::new()?;
                    let mut codec_valid = false;
                    listener.on_event(player::PlayerEvent::PlayerStarted);
                    while !stopped.load(std::sync::atomic::Ordering::Acquire) {
                        // Polling allows Drop to join safely even if the runtime is shutting down.
                        let packet = match receive.try_recv() {
                            Ok(packet) => packet,
                            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                                std::thread::sleep(std::time::Duration::from_millis(2));
                                continue;
                            }
                            Err(_) => break,
                        };
                        match packet {
                            types::AVPacket::Codec(packet) => {
                                if packet.header.codec != u32::from_be_bytes(*b"h264")
                                    || packet.header.rotation != 0
                                {
                                    return Err(kyber_media::MediaError(
                                        "Unsupported Kyber video codec or rotation".into(),
                                    ));
                                }
                                decoder = kyber_media::H264Stream::new()?;
                                codec_valid = true;
                            }
                            types::AVPacket::Media(packet) if codec_valid => {
                                if packet.payload.len() > 16 * 1024 * 1024 {
                                    return Err(kyber_media::MediaError(
                                        "Video packet exceeds limit".into(),
                                    ));
                                }
                                if packet.header.is_config {
                                    decoder.configure(&packet.payload)?;
                                    continue;
                                }
                                let pts_us = i64::try_from(packet.header.pts).map_err(|_| {
                                    kyber_media::MediaError("Invalid timestamp".into())
                                })?;
                                for frame in decoder.decode(kyber_media::EncodedPacket {
                                    data: &packet.payload,
                                    pts_us,
                                    keyframe: packet.header.is_key,
                                })? {
                                    if !stopped.load(std::sync::atomic::Ordering::Acquire) {
                                        output.emit(frame);
                                    }
                                }
                            }
                            types::AVPacket::Hole(_) => {
                                decoder.discontinuity()?;
                            }
                            _ => {
                                return Err(kyber_media::MediaError(
                                    "Media arrived before codec configuration".into(),
                                ));
                            }
                        }
                    }
                    Ok(())
                };
                if let Err(error) = run() {
                    if !stopped.load(std::sync::atomic::Ordering::Acquire) {
                        output.fail(&error.to_string());
                    }
                }
                listener.on_event(player::PlayerEvent::PlayerStopped);
            })?;
        let stopped = closing.clone();
        let forward = tokio::spawn(async move {
            loop {
                match protocol.recv().await {
                    Ok(Some(packet)) => {
                        if send.send(packet).await.is_err() {
                            break;
                        }
                    }
                    result => {
                        if !stopped.load(std::sync::atomic::Ordering::Acquire) {
                            // Transport reconnection remains owned by the SDK connection lifecycle.
                            log::warn!("Kyber video stream ended: {result:?}");
                        }
                        break;
                    }
                }
            }
        });
        workers.push(VideoWorker {
            forward,
            decode: Some(decode),
            closing,
        });
    }
    if let Some(endpoint) = audio {
        let config = backend_config
            .streaming_config
            .video
            .first()
            .ok_or(Error::NoAudioPlayer)?;
        let player::WindowHandle::Cpu(output) = config.player.window_handle else {
            return Err(Error::NoAudioPlayer);
        };
        let protocol = endpoint
            .ready()
            .await
            .map_err(|e| Error::KymuxReadyFailed { reason: e.0 })?;
        let closing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopped = closing.clone();
        let forward = tokio::spawn(async move {
            let result = forward_audio(protocol, |bytes| {
                !stopped.load(std::sync::atomic::Ordering::Acquire)
                    && unsafe {
                        (output.audio)(output.userdata as _, bytes.as_ptr(), bytes.len()) == 0
                    }
            })
            .await;
            if let Err(error) = result {
                // Upstream owns transport loss and reconnection. Packet validation
                // failures are reported by the callback or this size check.
                if error == "Host audio packet exceeds limit" {
                    output.fail(&error);
                } else {
                    log::warn!("Kyber audio stream ended: {error}");
                }
            }
        });
        workers.push(VideoWorker {
            forward,
            decode: None,
            closing,
        });
    }
    Ok(Some(Player(workers)))
}

include!("godot_audio.rs");
