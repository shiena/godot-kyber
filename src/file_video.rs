// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! File video demo over the SDK's KyProto reliable video endpoint.
use crate::frame::{Frame, Mailbox};
use image::{
    ImageDecoder,
    codecs::jpeg::{JpegDecoder, JpegEncoder},
};
use kymux_types::{
    AVPacket, CodecPacket, CodecPacketHeader, DataPacket, MediaPacket, MediaPacketHeader,
};
use kynet::{
    Server,
    common::{CommonServer, CommonServerOptions},
    quinn::QuinnClientOptions,
};
use kyproto::{AudioProtocol, ClientAuth, Connection, VideoProtocol};
use sha2::{Digest, Sha256};
use std::{
    io::Cursor,
    net::SocketAddr,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tokio::{sync::oneshot, time::timeout};

pub const WIDTH: u32 = 640;
pub const HEIGHT: u32 = 360;
const FPS: u64 = 24;
const H264: u32 = u32::from_be_bytes(*b"h264");
const JPEG: u32 = u32::from_be_bytes(*b"MJPG");
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
pub struct Status {
    pub message: String,
    pub code: String,
    pub finished: bool,
    pub failed: bool,
    pub clients: usize,
    pub skipped: u64,
}

pub enum Mode {
    Viewport {
        bind: SocketAddr,
        receive: tokio::sync::mpsc::Receiver<SourceEvent>,
    },
    Send {
        file: String,
        bind: SocketAddr,
        repeat: bool,
        audio_enabled: bool,
    },
    Receive {
        host: String,
        port: u16,
        code: String,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Transport {
    #[default]
    Reliable,
    Unreliable,
    Fec,
}
impl Transport {
    pub fn from_id(value: i32) -> Result<Self> {
        match value {
            0 => Ok(Self::Reliable),
            1 => Ok(Self::Unreliable),
            2 => Ok(Self::Fec),
            _ => Err("Unknown media transport".into()),
        }
    }
    fn video(self) -> VideoProtocol {
        match self {
            Self::Reliable => VideoProtocol::Reliable,
            Self::Unreliable => VideoProtocol::Unreliable,
            Self::Fec => VideoProtocol::UnreliableFec,
        }
    }
    fn audio(self) -> AudioProtocol {
        match self {
            Self::Reliable => AudioProtocol::Reliable,
            Self::Unreliable => AudioProtocol::Unreliable,
            Self::Fec => AudioProtocol::UnreliableFec,
        }
    }
    fn code(self, hash: &str, token: &str) -> String {
        match self {
            Self::Reliable => format!("{hash}:{token}"),
            Self::Unreliable => format!("k3:1:{hash}:{token}"),
            Self::Fec => format!("k3:2:{hash}:{token}"),
        }
    }
}
struct SendOptions {
    bind: SocketAddr,
    repeat: bool,
    audio_enabled: bool,
    transport: Transport,
    video: VideoOptions,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct VideoOptions {
    pub h264: bool,
    pub acceleration: kyber_media::Acceleration,
}

pub struct Session {
    cancel: Option<oneshot::Sender<()>>,
    worker: Option<thread::JoinHandle<()>>,
    pub status: Arc<Mutex<Status>>,
}

impl Session {
    pub fn start(mode: Mode, frames: Arc<Mailbox>) -> Result<Self> {
        Self::start_with_transport(mode, frames, Transport::Reliable)
    }
    pub fn start_with_transport(
        mode: Mode,
        frames: Arc<Mailbox>,
        transport: Transport,
    ) -> Result<Self> {
        Self::start_with_video(mode, frames, transport, VideoOptions::default())
    }
    pub fn start_with_video(
        mode: Mode,
        frames: Arc<Mailbox>,
        transport: Transport,
        video: VideoOptions,
    ) -> Result<Self> {
        match &mode {
            Mode::Viewport { .. } => {}
            Mode::Send { file, .. } => {
                if !Path::new(file).is_absolute() || !Path::new(file).is_file() {
                    return Err("Provide an absolute path to the video file.".into());
                }
                let capabilities = kyber_media::capabilities();
                if !capabilities.file_video {
                    return Err(capabilities.reason.into());
                }
            }
            Mode::Receive { host, code, .. } => {
                if host.trim().is_empty() {
                    return Err("Provide the sender's host name.".into());
                }
                parse_code(code)?;
            }
        }
        let status = Arc::new(Mutex::new(Status {
            message: "Preparing…".into(),
            ..Status::default()
        }));
        let shared = status.clone();
        let (cancel, cancelled) = oneshot::channel();
        let worker = thread::Builder::new().name("kyber-file-video".into()).spawn(move || {
            let result = (|| -> Result<()> {
                let _ = rustls::crypto::ring::default_provider().install_default();
                let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
                runtime.block_on(async {
                    tokio::select! {
                        _ = cancelled => Ok(()),
                        result = async {
                            match mode {
                                Mode::Send { file, bind, repeat, audio_enabled } => send(Some(&file), None, SendOptions { bind, repeat, audio_enabled, transport, video }, &frames, &shared).await,
                                Mode::Viewport { bind, receive } => send(None, Some(receive), SendOptions { bind, repeat: false, audio_enabled: false, transport, video }, &frames, &shared).await,
                                Mode::Receive { host, port, code } => receive_with_video(&host, port, &code, &frames, &shared, video).await,
                            }
                        } => result,
                    }
                })
            })();
            frames.closed.store(true, std::sync::atomic::Ordering::Release);
            let mut state = shared.lock().unwrap_or_else(|e| e.into_inner());
            state.finished = true;
            state.failed = result.is_err();
            state.message = match result { Ok(()) => "Stopped.".into(), Err(error) => format!("Error: {error}") };
        })?;
        Ok(Self {
            cancel: Some(cancel),
            worker: Some(worker),
            status,
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn message(status: &Mutex<Status>, value: &str) {
    status.lock().unwrap_or_else(|e| e.into_inner()).message = value.into();
}

fn parse_code(code: &str) -> Result<(&str, &str, Transport)> {
    let code = code.trim();
    let (code, transport) = if let Some(rest) = code.strip_prefix("k3:1:") {
        (rest, Transport::Unreliable)
    } else if let Some(rest) = code.strip_prefix("k3:2:") {
        (rest, Transport::Fec)
    } else {
        (code, Transport::Reliable)
    };
    let (hash, token) = code
        .trim()
        .split_once(':')
        .ok_or("Copy the connection code from the sender.")?;
    if [hash, token]
        .iter()
        .any(|part| part.len() != 64 || !part.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return Err("Invalid connection code format.".into());
    }
    Ok((hash, token, transport))
}

async fn send(
    file: Option<&str>,
    mut live: Option<tokio::sync::mpsc::Receiver<SourceEvent>>,
    options: SendOptions,
    frames: &Mailbox,
    status: &Arc<Mutex<Status>>,
) -> Result<()> {
    let SendOptions {
        bind,
        repeat,
        audio_enabled,
        transport,
        video,
    } = options;
    let certificate = rcgen::generate_simple_self_signed(vec!["godot-kyber.local".into()])?;
    let key = certificate.signing_key.serialize_der();
    let hash = hex::encode(Sha256::digest(certificate.cert.der()));
    let token = hex::encode(Sha256::digest(&key));
    let server = CommonServer::start_on_addr(
        bind,
        vec![certificate.cert.der().clone()],
        kynet::cert::PrivateKey::Pkcs8(key.into()),
        &CommonServerOptions {
            max_idle_timeout: Some(Duration::from_secs(15)),
            keep_alive_interval: Some(Duration::from_secs(2)),
        },
    )?;
    status.lock().unwrap_or_else(|e| e.into_inner()).code = transport.code(&hash, &token);
    message(
        status,
        "Waiting for a receiver (paste the connection code on the receiving device)",
    );
    // Every receiver has an independent writer and a latest-frame mailbox.
    let (packets, _) = tokio::sync::watch::channel(None::<(u64, AVPacket)>);
    let (audio_packets, _) = tokio::sync::broadcast::channel::<AVPacket>(50);
    let (ready, mut readiness) = tokio::sync::mpsc::channel(1);
    let (finished, mut finish) = oneshot::channel::<()>();
    let transport_mode = transport;
    let acceptor = async {
        let mut clients = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                _ = &mut finish => break,
                _ = clients.join_next(), if !clients.is_empty() => {},
                incoming = server.accept() => {
                    let Some(transport) = incoming? else { break };
                    // Bound simultaneous handshakes and connected receivers.
                    if clients.len() >= 16 { continue; }
                    let token = token.clone();
                    let receive = packets.subscribe();
                    let audio_receive = audio_packets.subscribe();
                    let ready = ready.clone();
                    let status = status.clone();
                    let options_transport = transport_mode;
                    clients.spawn(async move {
                        let _ = serve_viewer(transport, token, receive, audio_receive, ready, status, (options_transport, video.h264)).await;
                    });
                }
            }
        }
        let _ = timeout(Duration::from_secs(12), async {
            while clients.join_next().await.is_some() {}
        })
        .await;
        clients.abort_all();
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    };
    let producer = async {
        let result = async {
            readiness.recv().await.ok_or("No receiver connected")?;
            message(
                status,
                "Streaming · KyProto / QUIC · 640×360 / up to 24 fps · Video / audio (supported files)",
            );
            let is_live = live.is_some();
            let (video_send, mut video_receive) =
                tokio::sync::mpsc::channel::<(kyber_media::VideoFrame, u64)>(1);
            let encoding = async {
                let h264 = video
                    .h264
                    .then(|| crate::video_codec::encoder(video.acceleration))
                    .transpose()?;
                let mut sequence = 0;
                while let Some((frame, pts)) = video_receive.recv().await {
                    let start = std::time::Instant::now();
                    let (jpeg, preview) = if let Some(h264) = &h264 {
                        let mut frame = frame;
                        frame.pts_us = i64::try_from(pts)?;
                        let (packet, preview, hardware) = h264.process(frame).await?;
                        message(
                            status,
                            if hardware {
                                "Sending H.264 / GPU"
                            } else {
                                "Sending H.264 / CPU"
                            },
                        );
                        (packet.data, preview)
                    } else {
                        tokio::task::spawn_blocking(move || encode(frame)).await??
                    };
                    let elapsed = start.elapsed();
                    frames.record("encode", elapsed);
                    sequence += 1;
                    packets.send_replace(Some((
                        sequence,
                        AVPacket::Media(MediaPacket {
                            header: MediaPacketHeader {
                                is_config: false,
                                is_key: true,
                                pts,
                                size: jpeg.len() as u32,
                            },
                            payload: jpeg.into(),
                        }),
                    )));
                    frames.put_timed(preview, pts);
                }
                Ok::<u64, Box<dyn std::error::Error + Send + Sync>>(sequence)
            };
            let producing = async {
                let mut audio_encoder = crate::audio::Encoder::new()?;
                loop {
                    let mut source = match live.take() {
                        Some(receive) => Source::Live(receive),
                        None => Source::File(FileSource::start(
                            file.ok_or("Missing video source")?.to_owned(),
                            audio_enabled,
                        )?),
                    };
                    let start = tokio::time::Instant::now();
                    let timestamp_base = kyproto::clock::now_micros()? as u64;
                    let mut first_pts = None;
                    let mut next_pts = 0u64;
                    let mut pass_count = 0;
                    while let Some(event) = source.next().await? {
                        let captured_pts = event.pts();
                        let origin = *first_pts.get_or_insert(captured_pts);
                        let pts = captured_pts.saturating_sub(origin).max(0) as u64;
                        if let SourceEvent::Audio(mut audio) = event {
                            if !is_live {
                                tokio::time::sleep_until(start + Duration::from_micros(pts)).await;
                                audio.pts_us = (timestamp_base + pts) as i64;
                            }
                            if !is_live {
                                frames
                                    .audio
                                    .push(audio.pts_us.max(0) as u64, audio.samples.clone());
                            }
                            for packet in audio_encoder.encode(audio)? {
                                let _ = audio_packets.send(packet);
                            }
                            continue;
                        }
                        let SourceEvent::Video(frame) = event else {
                            unreachable!()
                        };
                        // Never delay audio to encode video that is already late.
                        if !is_live && start.elapsed().as_micros() > u128::from(pts) + 60_000 {
                            continue;
                        }
                        if !is_live && pts < next_pts {
                            continue;
                        }
                        if !is_live {
                            tokio::time::sleep_until(start + Duration::from_micros(pts)).await;
                        }
                        next_pts = pts + 1_000_000 / FPS;
                        let packet_pts = if is_live {
                            u64::try_from(captured_pts)?
                        } else {
                            timestamp_base + pts
                        };
                        // Video encoding cannot block the audio timeline.
                        let _ = video_send.try_send((frame, packet_pts));
                        pass_count += 1;
                    }
                    for packet in audio_encoder.flush()? {
                        let _ = audio_packets.send(packet);
                    }
                    if pass_count == 0 {
                        return Err("No readable video frames in this file.".into());
                    }
                    if !repeat {
                        break;
                    }
                }
                drop(video_send);
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            };
            let (_, mut sequence) = tokio::try_join!(producing, encoding)?;
            // Allow queued audio and the last video frame to reach presentation.
            tokio::time::sleep(Duration::from_millis(250)).await;
            sequence += 1;
            packets.send_replace(Some((
                sequence,
                AVPacket::Media(MediaPacket {
                    header: MediaPacketHeader {
                        is_config: true,
                        is_key: false,
                        pts: 0,
                        size: 0,
                    },
                    payload: Vec::new().into(),
                }),
            )));
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        }
        .await;
        let _ = finished.send(());
        result
    };
    tokio::pin!(producer, acceptor);
    tokio::select! {
        produced = &mut producer => { produced?; acceptor.await?; }
        accepted = &mut acceptor => { accepted?; return Err("Video listener stopped".into()); }
    }
    timeout(Duration::from_secs(5), server.wait_idle()).await?;
    Ok(())
}

struct ViewerCount(Arc<Mutex<Status>>);
impl Drop for ViewerCount {
    fn drop(&mut self) {
        let mut status = self.0.lock().unwrap_or_else(|e| e.into_inner());
        status.clients = status.clients.saturating_sub(1);
    }
}

async fn serve_viewer(
    transport: kynet::Connection,
    token: String,
    mut packets: tokio::sync::watch::Receiver<Option<(u64, AVPacket)>>,
    mut audio_packets: tokio::sync::broadcast::Receiver<AVPacket>,
    ready: tokio::sync::mpsc::Sender<()>,
    status: Arc<Mutex<Status>>,
    (media_transport, h264): (Transport, bool),
) -> Result<()> {
    let pending = timeout(
        Duration::from_secs(10),
        Connection::accept_with_auth(transport),
    )
    .await??;
    if pending.get_auth().token() != token {
        pending.reject_authentication();
        return Ok(());
    }
    let connection = pending.accept_authentication().await?;
    let result = async {
        let (_, endpoint) = connection
            .register_video_endpoint(media_transport.video())
            .await?;
        let mut output = timeout(Duration::from_secs(10), endpoint.ready()).await??;
        let (_, completion) = connection.register_data_endpoint().await?;
        let (mut completion_send, mut recv) =
            timeout(Duration::from_secs(10), completion.ready()).await??;
        let (_, clock_endpoint) = connection.register_clock_sync_endpoint().await?;
        let mut clock_protocol = timeout(Duration::from_secs(10), clock_endpoint.ready()).await??;
        let _clock = AbortTask(tokio::spawn(async move {
            let _ = clock_protocol.serve().await;
        }));
        timeout(
            Duration::from_secs(10),
            output.send(AVPacket::Codec(CodecPacket {
                header: CodecPacketHeader {
                    codec: if h264 { H264 } else { JPEG },
                    rotation: 0,
                    frame_size: 2,
                },
            })),
        )
        .await??;
        // Datagram video groups require a reliable configuration before media.
        if media_transport != Transport::Reliable {
            output
                .send(AVPacket::Media(MediaPacket {
                    header: MediaPacketHeader {
                        is_config: true,
                        is_key: false,
                        pts: 0,
                        size: 4,
                    },
                    payload: Vec::from(if h264 { b"h264" } else { b"MJPG" }).into(),
                }))
                .await?;
        }
        let (_, audio_endpoint) = connection
            .register_audio_endpoint(media_transport.audio())
            .await?;
        let mut audio_output = timeout(Duration::from_secs(10), audio_endpoint.ready()).await??;
        audio_output.send(crate::audio::codec()).await?;
        audio_output.send(crate::audio::configuration()).await?;
        let (audio_failed, mut audio_failure) = oneshot::channel();
        let _audio = AbortTask(tokio::spawn(async move {
            loop {
                match audio_packets.recv().await {
                    Ok(packet) => {
                        if !matches!(
                            timeout(Duration::from_secs(2), audio_output.send(packet)).await,
                            Ok(Ok(()))
                        ) {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            let _ = audio_failed.send(());
        }));
        status.lock().unwrap_or_else(|e| e.into_inner()).clients += 1;
        let _count = ViewerCount(status.clone());
        let _ = ready.try_send(());
        let mut previous = None;
        packets.mark_changed();
        loop {
            tokio::select! {
                changed = packets.changed() => { changed?; },
                _ = connection.closed() => return Ok(()),
                _ = &mut audio_failure => return Err("Audio sender stopped".into()),
            }
            let Some((sequence, packet)) = packets.borrow_and_update().clone() else {
                continue;
            };
            if let Some(last) = previous {
                status.lock().unwrap_or_else(|e| e.into_inner()).skipped +=
                    sequence.saturating_sub(last + 1);
            }
            previous = Some(sequence);
            let eof =
                matches!(&packet, AVPacket::Media(p) if p.header.is_config && p.payload.is_empty());
            if eof && media_transport != Transport::Reliable {
                // A final datagram can be lost; finish on the reliable control channel.
                completion_send
                    .send(DataPacket {
                        payload: Vec::from(b"eof").into(),
                    })
                    .await?;
            } else {
                timeout(Duration::from_secs(10), output.send(packet)).await??;
            }
            if eof {
                let ack = timeout(Duration::from_secs(10), recv.recv()).await??;
                if ack.is_none_or(|p| p.payload.as_ref() != b"eof") {
                    return Err("Invalid EOF acknowledgement".into());
                }
                return Ok(());
            }
        }
    }
    .await;
    connection.close();
    result
}

// COM objects stay on a dedicated thread; bounded delivery limits decoded memory.
pub(crate) enum SourceEvent {
    Video(kyber_media::VideoFrame),
    Audio(kyber_media::AudioFrame),
}
impl SourceEvent {
    fn pts(&self) -> i64 {
        match self {
            Self::Video(f) => f.pts_us,
            Self::Audio(f) => f.pts_us,
        }
    }
}

enum Source {
    File(FileSource),
    Live(tokio::sync::mpsc::Receiver<SourceEvent>),
}

impl Source {
    async fn next(&mut self) -> Result<Option<SourceEvent>> {
        match self {
            Self::File(source) => source.next().await,
            Self::Live(receive) => Ok(receive.recv().await),
        }
    }
}

struct FileSource {
    receive: Option<tokio::sync::mpsc::Receiver<kyber_media::Result<SourceEvent>>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl FileSource {
    fn start(path: String, audio_enabled: bool) -> Result<Self> {
        let (send, receive) = tokio::sync::mpsc::channel(2);
        let worker = thread::Builder::new()
            .name("kyber-file-source".into())
            .spawn(move || {
                let run = || -> kyber_media::Result<()> {
                    let mut source = kyber_media::open_video(Path::new(&path))?;
                    let mut audio = if audio_enabled {
                        kyber_media::open_audio(Path::new(&path))?
                    } else {
                        None
                    };
                    let mut video_next = source.next_frame()?;
                    let mut audio_pending = std::collections::VecDeque::new();
                    let mut audio_next = next_audio_chunk(&mut audio, &mut audio_pending)?;
                    while !send.is_closed() && (video_next.is_some() || audio_next.is_some()) {
                        let audio_first = audio_next.as_ref().is_some_and(|a| {
                            video_next.as_ref().is_none_or(|v| a.pts_us <= v.pts_us)
                        });
                        let event = if audio_first {
                            SourceEvent::Audio(audio_next.take().unwrap())
                        } else {
                            SourceEvent::Video(video_next.take().unwrap())
                        };
                        if send.blocking_send(Ok(event)).is_err() {
                            break;
                        }
                        if audio_first {
                            audio_next = next_audio_chunk(&mut audio, &mut audio_pending)?;
                        } else {
                            video_next = source.next_frame()?;
                        }
                    }
                    Ok(())
                };
                if let Err(error) = run() {
                    let _ = send.blocking_send(Err(error));
                }
            })?;
        Ok(Self {
            receive: Some(receive),
            worker: Some(worker),
        })
    }
    async fn next(&mut self) -> Result<Option<SourceEvent>> {
        self.receive
            .as_mut()
            .unwrap()
            .recv()
            .await
            .transpose()
            .map_err(Into::into)
    }
}
// Native readers may return large PCM blocks. Interleave 20 ms chunks with video
// instead of publishing future audio timestamps in one burst.
fn next_audio_chunk(
    source: &mut Option<Box<dyn kyber_media::AudioSource>>,
    pending: &mut std::collections::VecDeque<kyber_media::AudioFrame>,
) -> kyber_media::Result<Option<kyber_media::AudioFrame>> {
    if pending.is_empty()
        && let Some(source) = source
        && let Some(frame) = source.next_frame()?
    {
        for (index, samples) in frame.samples.chunks(960 * 2).enumerate() {
            pending.push_back(kyber_media::AudioFrame {
                pts_us: frame.pts_us + index as i64 * 20_000,
                samples: samples.to_vec(),
            });
        }
    }
    Ok(pending.pop_front())
}

impl Drop for FileSource {
    fn drop(&mut self) {
        self.receive.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn decode(payload: &[u8]) -> Result<Frame> {
    if payload.len() > 2_000_000 {
        return Err("Video packet is too large.".into());
    }
    let decoder = JpegDecoder::new(Cursor::new(payload))?;
    if decoder.dimensions() != (WIDTH, HEIGHT) {
        return Err("Unsupported video dimensions.".into());
    }
    let rgba = image::DynamicImage::from_decoder(decoder)?
        .into_rgba8()
        .into_raw();
    Ok(Frame {
        #[cfg(windows)]
        gpu: None,
        width: WIDTH,
        height: HEIGHT,
        rgba,
    })
}

pub(crate) fn scale(frame: kyber_media::VideoFrame) -> Result<Frame> {
    let image = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba)
        .ok_or("Invalid decoded video frame")?;
    let scaled = image::DynamicImage::ImageRgba8(image)
        .resize(WIDTH, HEIGHT, image::imageops::FilterType::Triangle)
        .to_rgba8();
    let mut rgba = image::RgbaImage::from_pixel(WIDTH, HEIGHT, image::Rgba([0, 0, 0, 255]));
    image::imageops::replace(
        &mut rgba,
        &scaled,
        ((WIDTH - scaled.width()) / 2) as i64,
        ((HEIGHT - scaled.height()) / 2) as i64,
    );
    Ok(Frame {
        #[cfg(windows)]
        gpu: None,
        width: WIDTH,
        height: HEIGHT,
        rgba: rgba.into_raw(),
    })
}
fn encode(frame: kyber_media::VideoFrame) -> Result<(Vec<u8>, Frame)> {
    let preview = scale(frame)?;
    let rgb: Vec<u8> = preview
        .rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 75).encode(
        &rgb,
        WIDTH,
        HEIGHT,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok((jpeg, preview))
}

// Aborting an owning session must also stop its clock responder.
struct AbortTask(tokio::task::JoinHandle<()>);
impl Drop for AbortTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn source_age(pts: u64, offset: i64) -> Result<Duration> {
    let now = i128::from(kyproto::clock::now_micros()?) + i128::from(offset);
    let age = (now - i128::from(pts)).max(0).min(u64::MAX as i128) as u64;
    Ok(Duration::from_micros(age))
}

async fn receive_with_video(
    host: &str,
    port: u16,
    code: &str,
    frames: &Arc<Mailbox>,
    status: &Mutex<Status>,
    video: VideoOptions,
) -> Result<()> {
    let (hash, token, media_transport) = parse_code(code)?;
    message(status, "Connecting to the sender…");
    let address = timeout(
        Duration::from_secs(10),
        tokio::net::lookup_host((host, port)),
    )
    .await??
    .next()
    .ok_or("Cannot resolve the host.")?;
    let transport = timeout(
        Duration::from_secs(10),
        kynet::Connection::quinn_connect(
            address,
            "godot-kyber.local",
            None,
            &QuinnClientOptions {
                certificate_hash: Some(hash.into()),
                max_idle_timeout: Some(Duration::from_secs(15)),
                keep_alive_interval: Some(Duration::from_secs(2)),
            },
        ),
    )
    .await??;
    let connection = timeout(
        Duration::from_secs(10),
        Connection::connect_with_auth(transport, &ClientAuth::new(token)?),
    )
    .await??;
    let endpoint = connection.connect_video_endpoint(0, media_transport.video())?;
    let mut input = timeout(Duration::from_secs(10), endpoint.ready()).await??;
    let completion = connection.connect_data_endpoint(2)?;
    let (mut completion_send, mut completion_recv) =
        timeout(Duration::from_secs(10), completion.ready()).await??;
    message(
        status,
        "Receiving · KyProto / QUIC · 640×360 / up to 24 fps · Video / audio (supported files)",
    );
    let clock_endpoint = connection.connect_clock_sync_endpoint(4)?;
    let mut clock_protocol = timeout(Duration::from_secs(10), clock_endpoint.ready()).await??;
    let initial = timeout(Duration::from_secs(5), clock_protocol.sync_average(3)).await??;
    frames.synchronize(initial.offset_micros(), initial.delay_micros());
    let (clock_send, clock_receive) = tokio::sync::watch::channel(initial.offset_micros());
    let synchronize = async {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let result = timeout(Duration::from_secs(5), clock_protocol.sync_average(3)).await??;
            frames.synchronize(result.offset_micros(), result.delay_micros());
            clock_send.send_replace(result.offset_micros());
        }
        #[allow(unreachable_code)]
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    };
    let codec = timeout(Duration::from_secs(10), input.recv())
        .await??
        .ok_or("Missing video codec")?;
    let (revision, h264) = match codec {
        AVPacket::Codec(p) if p.header.codec == JPEG || p.header.codec == H264 => {
            (p.header.frame_size, p.header.codec == H264)
        }
        _ => return Err("Unsupported video codec".into()),
    };
    if revision != 1 && revision != 2 {
        return Err("Unsupported video protocol revision".into());
    }
    let audio_input = if revision == 2 {
        Some(
            timeout(
                Duration::from_secs(10),
                connection
                    .connect_audio_endpoint(6, media_transport.audio())?
                    .ready(),
            )
            .await??,
        )
    } else {
        None
    };
    let audio_reading = crate::audio::receive(audio_input, &frames.audio);
    let (compressed, pending) = tokio::sync::watch::channel(None);
    let reading = async {
        let result = async {
            let mut codec_valid = true;
            let mut last_received: Option<std::time::Instant> = None;
            let mut sequence = 0;
            let mut last_audio = frames.audio.received.load(std::sync::atomic::Ordering::Relaxed);
            loop {
                let packet = tokio::select! {
                    packet = timeout(Duration::from_secs(15), input.recv()) => match packet {
                        Ok(packet) => packet?,
                        Err(error) => {
                            let audio = frames.audio.received.load(std::sync::atomic::Ordering::Relaxed);
                            if audio == last_audio { return Err(error.into()); }
                            last_audio = audio;
                            continue;
                        }
                    },
                    control = completion_recv.recv(), if media_transport != Transport::Reliable => {
                        if control?.is_some_and(|p| p.payload.as_ref() == b"eof") {
                            return Ok(());
                        }
                        return Err("Invalid video completion message".into());
                    }
                };
                let Some(packet) = packet else { break; };
                match packet {
                    AVPacket::Codec(packet)
                        if packet.header.codec == (if h264 { H264 } else { JPEG })
                            && (packet.header.frame_size == 1 || packet.header.frame_size == 2) =>
                    {
                        codec_valid = true
                    }
                    AVPacket::Media(packet) if codec_valid => {
                        if packet.header.is_config && packet.payload.is_empty() {
                            return Ok(());
                        }
                        if packet.header.is_config && (packet.payload.as_ref() == b"MJPG" || packet.payload.as_ref() == b"h264") {
                            continue;
                        }
                        if packet.payload.len() > 2_000_000 {
                            return Err("Video packet is too large".into());
                        }
                        let now = std::time::Instant::now();
                        if let Some(previous) = last_received {
                            frames.record("receive_interval", now.duration_since(previous));
                        }
                        last_received = Some(now);
                        frames.record(
                            "source_to_receive",
                            source_age(packet.header.pts, *clock_receive.borrow())?,
                        );
                        sequence += 1;
                        // Drain the transport independently of decoding to avoid replaying a backlog.
                        compressed.send_replace(Some(CompressedFrame {
                            sequence,
                            packet,
                            arrived: now,
                        }));
                    }
                    AVPacket::Hole(_) if codec_valid => {
                        frames.discarded(1);
                    }
                    _ => return Err("Unsupported video protocol.".into()),
                }
            }
            Err("Disconnected from the sender.".into())
        }
        .await;
        drop(compressed);
        result
    };
    let receive_and_decode = async {
        tokio::try_join!(reading, async {
            if h264 {
                decode_h264_latest(pending, frames, &clock_receive, video.acceleration, status)
                    .await
            } else {
                decode_latest(pending, frames, decode, Some(&clock_receive)).await
            }
        })?;
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    };
    tokio::select! {
        result = receive_and_decode => result?,
        result = synchronize => { result?; return Err("Clock synchronization stopped".into()); },
        result = audio_reading => { result?; return Err("Audio stream stopped".into()); },
    }
    completion_send
        .send(DataPacket {
            payload: Vec::from(b"eof").into(),
        })
        .await?;
    timeout(Duration::from_secs(10), connection.closed()).await??;
    Ok(())
}

#[derive(Clone)]
struct CompressedFrame {
    sequence: u64,
    packet: MediaPacket,
    arrived: std::time::Instant,
}

async fn decode_h264_latest(
    mut pending: tokio::sync::watch::Receiver<Option<CompressedFrame>>,
    frames: &Arc<Mailbox>,
    clock: &tokio::sync::watch::Receiver<i64>,
    acceleration: kyber_media::Acceleration,
    status: &Mutex<Status>,
) -> Result<()> {
    let decoder = crate::video_codec::decoder(acceleration, frames.clone())?;
    let mut previous = 0;
    while pending.changed().await.is_ok() {
        let Some(input) = pending.borrow_and_update().clone() else {
            continue;
        };
        frames.discarded(input.sequence.saturating_sub(previous + 1));
        previous = input.sequence;
        frames.record("decode_queue", input.arrived.elapsed());
        let pts = input.packet.header.pts;
        if source_age(pts, *clock.borrow())? > Duration::from_millis(500) {
            frames.discarded(1);
            continue;
        }
        if !input.packet.header.is_key {
            return Err("H.264 demo requires independent keyframes".into());
        }
        let start = std::time::Instant::now();
        let (decoded, hardware) = decoder
            .process((input.packet.payload.to_vec(), i64::try_from(pts)?))
            .await?;
        frames.record("decode", start.elapsed());
        frames.record("receive_to_ready", input.arrived.elapsed());
        for (frame, pts) in decoded {
            if (frame.width, frame.height) != (WIDTH, HEIGHT) {
                return Err("Unsupported H.264 frame dimensions".into());
            }
            frames.record("source_to_ready", source_age(pts as u64, *clock.borrow())?);
            frames.put_timed(frame, pts as u64);
        }
        message(
            status,
            if hardware {
                "Receiving H.264 / GPU"
            } else {
                "Receiving H.264 / CPU or platform default"
            },
        );
    }
    let (tail, _) = decoder.process((Vec::new(), 0)).await?;
    for (frame, pts) in tail {
        if (frame.width, frame.height) != (WIDTH, HEIGHT) {
            return Err("Unsupported H.264 frame dimensions".into());
        }
        frames.put_timed(frame, pts as u64);
    }
    Ok(())
}

async fn decode_latest(
    mut pending: tokio::sync::watch::Receiver<Option<CompressedFrame>>,
    frames: &Mailbox,
    decoder: fn(&[u8]) -> Result<Frame>,
    clock: Option<&tokio::sync::watch::Receiver<i64>>,
) -> Result<()> {
    let mut previous = 0;
    while pending.changed().await.is_ok() {
        let Some(input) = pending.borrow_and_update().clone() else {
            continue;
        };
        frames.discarded(input.sequence.saturating_sub(previous + 1));
        previous = input.sequence;
        frames.record("decode_queue", input.arrived.elapsed());
        let pts = input.packet.header.pts;
        if let Some(clock) = clock
            && source_age(pts, *clock.borrow())? > Duration::from_millis(500)
        {
            frames.discarded(1);
            continue;
        }
        let (frame, elapsed) = tokio::task::spawn_blocking(move || {
            let start = std::time::Instant::now();
            decoder(&input.packet.payload).map(|frame| (frame, start.elapsed()))
        })
        .await??;
        frames.record("decode", elapsed);
        frames.record("receive_to_ready", input.arrived.elapsed());
        if let Some(clock) = clock {
            frames.record("source_to_ready", source_age(pts, *clock.borrow())?);
        }
        frames.put_timed(frame, pts);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_native_audio_blocks_are_interleaved_in_twenty_ms_chunks() {
        struct Source(Option<kyber_media::AudioFrame>);
        impl kyber_media::AudioSource for Source {
            fn next_frame(&mut self) -> kyber_media::Result<Option<kyber_media::AudioFrame>> {
                Ok(self.0.take())
            }
        }
        let mut source: Option<Box<dyn kyber_media::AudioSource>> =
            Some(Box::new(Source(Some(kyber_media::AudioFrame {
                pts_us: 123_456,
                samples: vec![0.25; 4800 * 2 + 14],
            }))));
        let mut pending = std::collections::VecDeque::new();
        let mut total = 0;
        let mut count = 0;
        while let Some(frame) = next_audio_chunk(&mut source, &mut pending).unwrap() {
            assert_eq!(frame.pts_us, 123_456 + count * 20_000);
            assert!(frame.samples.len() <= 1920);
            total += frame.samples.len();
            count += 1;
        }
        assert_eq!(count, 6);
        assert_eq!(total, 9614);
    }

    #[test]
    fn slow_decoder_skips_backlog_and_drains_the_final_frame() {
        fn slow(payload: &[u8]) -> Result<Frame> {
            thread::sleep(Duration::from_millis(40));
            Ok(Frame {
                #[cfg(windows)]
                gpu: None,
                width: 1,
                height: 1,
                rgba: vec![payload[0]; 4],
            })
        }
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let frames = Mailbox::default();
                let (send, receive) = tokio::sync::watch::channel(None);
                let producer = async {
                    for sequence in 1..=30 {
                        send.send_replace(Some(CompressedFrame {
                            sequence,
                            arrived: std::time::Instant::now(),
                            packet: MediaPacket {
                                header: MediaPacketHeader {
                                    is_config: false,
                                    is_key: true,
                                    pts: sequence,
                                    size: 1,
                                },
                                payload: vec![sequence as u8].into(),
                            },
                        }));
                        tokio::time::sleep(Duration::from_millis(3)).await;
                    }
                    drop(send);
                };
                let (_, result) =
                    tokio::join!(producer, decode_latest(receive, &frames, slow, None));
                result.unwrap();
                assert_eq!(frames.take().unwrap().rgba, vec![30; 4]);
                assert_eq!(frames.dropped(), 29);
            });
    }

    #[test]
    fn broadcasts_to_late_joiners_and_survives_a_receiver_disconnect() {
        for transport in [Transport::Reliable, Transport::Unreliable, Transport::Fec] {
            broadcast_and_finish(transport, VideoOptions::default());
        }
    }
    #[cfg(windows)]
    #[test]
    fn h264_broadcast_survives_late_join_and_disconnect() {
        for transport in [Transport::Reliable, Transport::Unreliable, Transport::Fec] {
            broadcast_and_finish(
                transport,
                VideoOptions {
                    h264: true,
                    acceleration: kyber_media::Acceleration::Software,
                },
            );
        }
    }
    struct TestFrameProducer {
        stop: Arc<std::sync::atomic::AtomicBool>,
        worker: Option<thread::JoinHandle<()>>,
    }

    impl TestFrameProducer {
        fn start(send: tokio::sync::mpsc::Sender<SourceEvent>) -> Self {
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let stopping = stop.clone();
            let worker = thread::spawn(move || {
                // Model capture independently of receiver setup and teardown.
                while !stopping.load(std::sync::atomic::Ordering::Acquire) && !send.is_closed() {
                    let _ = send.try_send(SourceEvent::Video(kyber_media::VideoFrame {
                        width: 64,
                        height: 36,
                        pts_us: kyproto::clock::now_micros().unwrap(),
                        rgba: [10, 160, 90, 255].repeat(64 * 36),
                    }));
                    thread::sleep(Duration::from_millis(50));
                }
            });
            Self {
                stop,
                worker: Some(worker),
            }
        }
    }

    impl Drop for TestFrameProducer {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::Release);
            self.worker.take().unwrap().join().unwrap();
        }
    }

    fn broadcast_and_finish(transport: Transport, video: VideoOptions) {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let address = socket.local_addr().unwrap();
        drop(socket);
        let (send, receive) = tokio::sync::mpsc::channel(1);
        let host = Session::start_with_video(
            Mode::Viewport {
                bind: address,
                receive,
            },
            Arc::default(),
            transport,
            video,
        )
        .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let code = loop {
            let code = host.status.lock().unwrap().code.clone();
            if !code.is_empty() {
                break code;
            }
            assert!(std::time::Instant::now() < deadline, "sender did not bind");
            thread::sleep(Duration::from_millis(10));
        };
        let connect = || {
            let frames = Arc::new(Mailbox::default());
            let session = Session::start(
                Mode::Receive {
                    host: "127.0.0.1".into(),
                    port: address.port(),
                    code: code.clone(),
                },
                frames.clone(),
            )
            .unwrap();
            (session, frames)
        };
        let producer = TestFrameProducer::start(send);
        let pump_until = |phase: &str,
                          receiver: &Session,
                          frames: &Mailbox,
                          condition: &dyn Fn() -> bool| {
            // Each handshake/teardown gets its own budget on shared CI runners.
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            while !condition() {
                let status = host.status.lock().unwrap();
                assert!(!status.failed, "{phase}: {}", status.message);
                let receiving = receiver.status.lock().unwrap();
                assert!(!receiving.failed, "{phase}: receiver {}", receiving.message);
                assert!(
                    std::time::Instant::now() < deadline,
                    "{phase} timed out ({transport:?}, h264={}): clients={}, dropped={}, sender={}, receiver={}",
                    video.h264,
                    status.clients,
                    frames.dropped(),
                    status.message,
                    receiving.message
                );
                drop(receiving);
                drop(status);
                thread::sleep(Duration::from_millis(50));
            }
        };
        let (first, first_frames) = connect();
        pump_until("first receiver", &first, &first_frames, &|| {
            first_frames.take().is_some()
        });
        let (second, second_frames) = connect();
        pump_until("late receiver", &second, &second_frames, &|| {
            second_frames.take().is_some()
        });
        assert_eq!(host.status.lock().unwrap().clients, 2);
        drop(first);
        second_frames.take();
        pump_until("receiver disconnect", &second, &second_frames, &|| {
            second_frames.take().is_some() && host.status.lock().unwrap().clients == 1
        });
        let (third, third_frames) = connect();
        pump_until("replacement receiver", &third, &third_frames, &|| {
            third_frames.take().is_some()
        });
        assert_eq!(host.status.lock().unwrap().clients, 2);
        drop(producer);
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while !host.status.lock().unwrap().finished {
            assert!(std::time::Instant::now() < deadline, "EOF did not complete");
            thread::sleep(Duration::from_millis(20));
        }
        assert!(!host.status.lock().unwrap().failed);
        assert!(!second.status.lock().unwrap().failed);
        assert!(!third.status.lock().unwrap().failed);
    }

    #[test]
    fn rejects_invalid_code_and_image_dimensions() {
        assert!(parse_code("").is_err());
        assert!(parse_code(&format!("{}:{}", "x".repeat(64), "a".repeat(64))).is_err());
        assert!(parse_code(&format!("{}:{}", "a".repeat(64), "b".repeat(64))).is_ok());
        for (id, expected) in [(1, Transport::Unreliable), (2, Transport::Fec)] {
            let code = format!("k3:{id}:{}:{}", "a".repeat(64), "b".repeat(64));
            assert_eq!(parse_code(&code).unwrap().2, expected);
        }
        assert!(parse_code(&format!("k3:3:{}:{}", "a".repeat(64), "b".repeat(64))).is_err());
        let mut jpeg = vec![];
        JpegEncoder::new(&mut jpeg)
            .encode(&[0, 0, 0], 1, 1, image::ExtendedColorType::Rgb8)
            .unwrap();
        assert!(decode(&jpeg).is_err());
    }
}
