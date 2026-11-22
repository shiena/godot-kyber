// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! Thread-affine codecs. The bounded worker never calls Godot or blocks audio I/O.
use crate::frame::Frame;
use kyber_media::{Acceleration, EncodedFrame, EncoderSettings, VideoFrame};
use std::thread;
use tokio::sync::{mpsc, oneshot};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
type Reply<I, O> = (I, oneshot::Sender<Result<O>>);
pub(crate) struct Worker<I, O> {
    send: Option<mpsc::Sender<Reply<I, O>>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl<I: Send + 'static, O: Send + 'static> Worker<I, O> {
    fn new<F, C>(create: F) -> Result<Self>
    where
        F: FnOnce() -> Result<C> + Send + 'static,
        C: FnMut(I) -> Result<O> + 'static,
    {
        let (send, mut receive) = mpsc::channel::<Reply<I, O>>(1);
        let thread = thread::Builder::new()
            .name("kyber-codec".into())
            .spawn(move || {
                let mut codec = create();
                while let Some((input, reply)) = receive.blocking_recv() {
                    let result = match &mut codec {
                        Ok(codec) => codec(input),
                        Err(error) => Err(error.to_string().into()),
                    };
                    let _ = reply.send(result);
                }
            })?;
        Ok(Self {
            send: Some(send),
            thread: Some(thread),
        })
    }
    pub async fn process(&self, input: I) -> Result<O> {
        let (send, receive) = oneshot::channel();
        self.send
            .as_ref()
            .unwrap()
            .send((input, send))
            .await
            .map_err(|_| "Codec worker stopped")?;
        receive.await?
    }
}
impl<I, O> Drop for Worker<I, O> {
    fn drop(&mut self) {
        self.send.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub(crate) type Encoder = Worker<VideoFrame, (EncodedFrame, Frame, bool)>;
pub(crate) fn encoder(acceleration: Acceleration) -> Result<Encoder> {
    Worker::new(move || {
        let mut encoder = kyber_media::h264_encoder(&EncoderSettings {
            width: super::file_video::WIDTH,
            height: super::file_video::HEIGHT,
            fps: 24,
            bitrate: 4_000_000,
            acceleration,
        })?;
        Ok(move |frame: VideoFrame| {
            let pts = frame.pts_us;
            let preview = super::file_video::scale(frame)?;
            let mut packets = encoder.encode(&VideoFrame {
                width: preview.width,
                height: preview.height,
                pts_us: pts,
                rgba: preview.rgba.clone(),
            })?;
            if packets.len() != 1 || !packets[0].keyframe {
                return Err(
                    "H.264 transport requires one independently decodable frame per input".into(),
                );
            }
            Ok((packets.remove(0), preview, encoder.hardware_active()))
        })
    })
}
pub(crate) type Decoder = Worker<(Vec<u8>, i64), (Vec<(Frame, i64)>, bool)>;
pub(crate) fn decoder(
    acceleration: Acceleration,
    mailbox: std::sync::Arc<crate::frame::Mailbox>,
) -> Result<Decoder> {
    Worker::new(move || {
        #[cfg(windows)]
        let mut gpu = {
            let start = std::time::Instant::now();
            while mailbox
                .gpu_pending
                .load(std::sync::atomic::Ordering::Acquire)
                && start.elapsed().as_millis() < 1000
            {
                thread::sleep(std::time::Duration::from_millis(1));
            }
            mailbox
                .gpu_context
                .lock()
                .unwrap()
                .take()
                .and_then(|context| match kyber_media::gpu::Decoder::new(context) {
                    Ok(decoder) => Some(decoder),
                    Err(error) => {
                        *mailbox.gpu_status.lock().unwrap() = error.to_string();
                        None
                    }
                })
        };
        #[cfg(not(windows))]
        let _ = mailbox;
        let mut decoder = kyber_media::h264_decoder_with_acceleration(acceleration)?;
        let mut timestamps = std::collections::VecDeque::new();
        Ok(move |(bytes, pts): (Vec<u8>, i64)| {
            if cfg!(windows) && !bytes.is_empty() {
                timestamps.push_back(pts);
            }
            #[cfg(windows)]
            if let Some(shared) = &mut gpu {
                if bytes.is_empty() {
                    return Ok((Vec::new(), true));
                }
                match shared.decode(kyber_media::EncodedPacket {
                    data: &bytes,
                    pts_us: pts,
                    keyframe: true,
                }) {
                    Ok(frames) => {
                        let mut output = Vec::new();
                        for frame in frames {
                            let pts = timestamps.pop_front().ok_or("Unexpected GPU frame")?;
                            output.push((
                                Frame {
                                    width: frame.width,
                                    height: frame.height,
                                    rgba: Vec::new(),
                                    gpu: Some(std::sync::Arc::new(frame)),
                                },
                                pts,
                            ));
                        }
                        *mailbox.gpu_status.lock().unwrap() = "d3d12_shared".into();
                        return Ok((output, true));
                    }
                    Err(error) => {
                        *mailbox.gpu_status.lock().unwrap() =
                            format!("CPU transfer fallback: {error}");
                        gpu = None;
                        timestamps.clear();
                        timestamps.push_back(pts);
                    }
                }
            }
            let mut frames = if bytes.is_empty() {
                decoder.drain()?
            } else {
                decoder.decode(kyber_media::EncodedPacket {
                    data: &bytes,
                    pts_us: pts,
                    keyframe: true,
                })?
            };
            // Windows hardware decoders may synthesize a local timeline.
            if cfg!(windows) {
                for frame in &mut frames {
                    frame.pts_us = timestamps
                        .pop_front()
                        .ok_or("Decoder emitted an unexpected frame")?;
                }
            }
            if timestamps.len() > 24 {
                return Err("H.264 decoder output stalled".into());
            }
            let frames = frames
                .into_iter()
                .map(|f| {
                    (
                        Frame {
                            width: f.width,
                            height: f.height,
                            rgba: f.rgba,
                            #[cfg(windows)]
                            gpu: None,
                        },
                        f.pts_us,
                    )
                })
                .collect();
            Ok((frames, decoder.hardware_active()))
        })
    })
}
