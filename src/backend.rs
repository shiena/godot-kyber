// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

use crate::frame::{Frame, Mailbox, frame_size};
use serde::{Deserialize, Serialize};
use std::ffi::{CString, c_char, c_void};
use std::sync::{Arc, mpsc};

type Handle = *mut c_void;
type EventCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize);
type AudioCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
type FrameCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize, u32, u32, u64);
type Create = unsafe extern "C" fn(
    *const c_char,
    EventCallback,
    FrameCallback,
    AudioCallback,
    Handle,
) -> Handle;
type Start = unsafe extern "C" fn(Handle, u32, u32, u32, u32, bool) -> i32;
type Input = unsafe extern "C" fn(Handle, u32, i32, i32, bool) -> i32;
type Destroy = unsafe extern "C" fn(Handle);

// For this input kind, a/b carry IEEE-754 f32 bits in normalized wheel units.
pub const INPUT_WHEEL: u32 = 3;
// Kinds 4/5: a = device << 16 | Godot button/axis, b = signed axis value.
// Kind 6: a = device, pressed = connected. Devices are limited to 0..31.

#[derive(Serialize)]
pub struct Config {
    pub auto_reconnection: bool,
    pub clipboard_enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Display {
    pub id: u32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Deserialize)]
pub struct Event {
    pub kind: String,
    #[serde(default)]
    pub device: i32,
    #[serde(default)]
    pub strong: u16,
    #[serde(default)]
    pub weak: u16,
    #[serde(default)]
    pub duration_ms: i16,
    #[serde(default)]
    pub attempt: i32,
    #[serde(default)]
    pub max_attempts: i32,
    #[serde(default)]
    pub should_restart_streaming: bool,
    #[serde(default)]
    pub displays: Vec<Display>,
    #[serde(default)]
    pub message: String,
}

struct Callbacks {
    events: mpsc::SyncSender<Event>,
    frames: Arc<Mailbox>,
    overflow: std::sync::atomic::AtomicBool,
    audio: std::sync::Mutex<Option<crate::audio::PacketDecoder>>,
}

unsafe extern "C" fn on_event(userdata: Handle, data: *const u8, len: usize) {
    // The SDK keeps userdata alive until destroy has joined its workers.
    if userdata.is_null() || data.is_null() || len > 512 * 1024 {
        return;
    }
    let state = unsafe { &*userdata.cast::<Callbacks>() };
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    if let Ok(event) = serde_json::from_slice(bytes)
        && state.events.try_send(event).is_err()
    {
        state
            .overflow
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

unsafe extern "C" fn on_frame(
    userdata: Handle,
    data: *const u8,
    len: usize,
    width: u32,
    height: u32,
    pts: u64,
) {
    if userdata.is_null() || data.is_null() || frame_size(width, height) != Some(len) {
        return;
    }
    let state = unsafe { &*userdata.cast::<Callbacks>() };
    let rgba = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
    state.frames.put_timed(
        Frame {
            #[cfg(windows)]
            gpu: None,
            width,
            height,
            rgba,
        },
        pts,
    );
}

unsafe extern "C" fn on_audio(userdata: Handle, data: *const u8, len: usize) -> i32 {
    if userdata.is_null() || data.is_null() || !(12..=4012).contains(&len) {
        return -1;
    }
    let state = unsafe { &*userdata.cast::<Callbacks>() };
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    let result = (|| -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use kymux_types::{AVPacket, AVPacketHeader, CodecPacket, HolePacket, MediaPacket};
        let packet = match AVPacketHeader::deserialize(&bytes[..12]) {
            AVPacketHeader::Codec(header) if len == 12 => AVPacket::Codec(CodecPacket { header }),
            AVPacketHeader::Hole(header) if len == 12 => AVPacket::Hole(HolePacket { header }),
            AVPacketHeader::Media(header) if header.size as usize == len - 12 => {
                AVPacket::Media(MediaPacket {
                    header,
                    payload: bytes[12..].to_vec().into(),
                })
            }
            _ => return Err("Invalid SDK audio packet size".into()),
        };
        let mut decoder = state.audio.lock().unwrap_or_else(|e| e.into_inner());
        if decoder.is_none() {
            *decoder = Some(crate::audio::PacketDecoder::new()?);
        }
        decoder.as_mut().unwrap().push(packet, &state.frames.audio)
    })();
    if let Err(error) = result {
        let event = serde_json::json!({"kind":"error", "message":format!("Host audio: {error}")})
            .to_string();
        unsafe {
            on_event(userdata, event.as_ptr(), event.len());
        }
        return -1;
    }
    0
}

pub struct Session {
    handle: Handle,
    start: Start,
    input: Input,
    destroy: Destroy,
    callbacks: Box<Callbacks>,
    events: mpsc::Receiver<Event>,
}

impl Session {
    pub fn connect(config: Config, frames: Arc<Mailbox>) -> Result<Self, String> {
        if config.host.trim().is_empty() || config.port == 0 {
            return Err("Host and port are required".into());
        }
        let json = CString::new(serde_json::to_vec(&config).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        // Both implementations are linked at build time, never loaded from user paths.
        unsafe {
            let create: Create = linked::kyclient_godot_create;
            let start: Start = linked::kyclient_godot_start;
            let input: Input = linked::kyclient_godot_input;
            let destroy: Destroy = linked::kyclient_godot_destroy;
            let (sender, events) = mpsc::sync_channel(128);
            let mut callbacks = Box::new(Callbacks {
                events: sender,
                frames,
                overflow: false.into(),
                audio: std::sync::Mutex::new(None),
            });
            let handle = create(
                json.as_ptr(),
                on_event,
                on_frame,
                on_audio,
                (&mut *callbacks as *mut Callbacks).cast(),
            );
            if handle.is_null() {
                return Err("Kyber rejected the connection configuration".into());
            }
            Ok(Self {
                handle,
                start,
                input,
                destroy,
                callbacks,
                events,
            })
        }
    }

    pub fn events(&self) -> Vec<Event> {
        if self
            .callbacks
            .overflow
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            return vec![Event {
                kind: "error".into(),
                device: 0,
                strong: 0,
                weak: 0,
                duration_ms: 0,
                attempt: 0,
                max_attempts: 0,
                should_restart_streaming: false,
                displays: vec![],
                message: "SDK event queue overflow".into(),
            }];
        }
        self.events.try_iter().take(128).collect()
    }

    pub fn start(
        &self,
        display: u32,
        width: u32,
        height: u32,
        bitrate: u32,
        audio: bool,
    ) -> Result<(), String> {
        frame_size(width, height)
            .ok_or("Output size must be between 1 and 4096 pixels per axis")?;
        if bitrate == 0 {
            return Err("Bitrate must be positive".into());
        }
        match unsafe { (self.start)(self.handle, display, width, height, bitrate, audio) } {
            0 => {}
            -3 => {
                return Err(format!(
                    "H.264 decoding is unavailable: {}",
                    kyber_media::capabilities().reason
                ));
            }
            _ => return Err("Kyber rejected the stream configuration".into()),
        }
        Ok(())
    }

    pub fn clipboard(&self, operation: u32, text: &str) -> bool {
        text.len() <= 60 * 1024
            && unsafe {
                linked::kyclient_godot_clipboard(self.handle, operation, text.as_ptr(), text.len())
                    == 0
            }
    }

    pub fn input(&self, kind: u32, a: i32, b: i32, pressed: bool) -> bool {
        unsafe { (self.input)(self.handle, kind, a, b, pressed) == 0 }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // This call must stop playback and join all SDK threads before returning.
        unsafe { (self.destroy)(self.handle) };
        self.callbacks
            .frames
            .closed
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(all(feature = "native-sdk", feature = "mock-sdk"))]
compile_error!("native-sdk and mock-sdk are mutually exclusive");
#[cfg(not(any(feature = "native-sdk", feature = "mock-sdk")))]
compile_error!("Select native-sdk (default), or mock-sdk for tests");

#[cfg(feature = "native-sdk")]
mod linked {
    use super::*;
    // Ensure the SDK rlib and its exported entry points participate in linking.
    use kyclient as _;
    unsafe extern "C" {
        pub fn kyclient_godot_create(
            config: *const c_char,
            event: EventCallback,
            frame: FrameCallback,
            audio: AudioCallback,
            userdata: Handle,
        ) -> Handle;
        pub fn kyclient_godot_start(
            handle: Handle,
            display: u32,
            width: u32,
            height: u32,
            bitrate: u32,
            audio: bool,
        ) -> i32;
        pub fn kyclient_godot_input(
            handle: Handle,
            kind: u32,
            a: i32,
            b: i32,
            pressed: bool,
        ) -> i32;
        pub fn kyclient_godot_clipboard(
            handle: Handle,
            operation: u32,
            text: *const u8,
            len: usize,
        ) -> i32;
        pub fn kyclient_godot_destroy(handle: Handle);
    }
}

#[cfg(all(feature = "mock-sdk", not(feature = "native-sdk")))]
#[path = "../tests/fixtures/mock_sdk.rs"]
mod linked;

#[cfg(test)]
mod audio_tests {
    use super::*;
    use kymux_types as types;
    use kynet::Server;
    use sha2::{Digest, Sha256};
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    include!("../sdk/audio_forward.rs");

    #[test]
    fn sdk_audio_callback_decodes_official_quic_packets() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(15), async {
                    let cert = rcgen::generate_simple_self_signed(vec!["godot-kyber.local".into()])
                        .unwrap();
                    let hash = hex::encode(Sha256::digest(cert.cert.der()));
                    let token = "aa".repeat(32);
                    let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
                    let address = socket.local_addr().unwrap();
                    drop(socket);
                    let server = kynet::common::CommonServer::start_on_addr(
                        address,
                        vec![cert.cert.der().clone()],
                        kynet::cert::PrivateKey::Pkcs8(cert.signing_key.serialize_der().into()),
                        &Default::default(),
                    )
                    .unwrap();
                    let (events, _receive) = mpsc::sync_channel(128);
                    let frames = Arc::new(Mailbox::default());
                    let mut state = Box::new(Callbacks {
                        events,
                        frames: frames.clone(),
                        overflow: false.into(),
                        audio: std::sync::Mutex::new(None),
                    });
                    let userdata = (&mut *state as *mut Callbacks).cast();
                    let (done_send, done_recv) = tokio::sync::oneshot::channel();
                    let sending = async {
                        let pending = kyproto::Connection::accept_with_auth(
                            server.accept().await.unwrap().unwrap(),
                        )
                        .await
                        .unwrap();
                        assert_eq!(pending.get_auth().token(), token);
                        let connection = pending.accept_authentication().await.unwrap();
                        let (_, endpoint) = connection
                            .register_audio_endpoint(kyproto::AudioProtocol::Reliable)
                            .await
                            .unwrap();
                        let mut output = endpoint.ready().await.unwrap();
                        output
                            .send(types::AVPacket::Codec(types::CodecPacket {
                                header: types::CodecPacketHeader {
                                    codec: u32::from_be_bytes(*b"opus"),
                                    rotation: 0,
                                    frame_size: 0,
                                },
                            }))
                            .await
                            .unwrap();
                        let mut head = b"OpusHead".to_vec();
                        head.extend_from_slice(&[1, 2, 0, 0, 128, 187, 0, 0, 0, 0, 0]);
                        output
                            .send(types::AVPacket::Media(types::MediaPacket {
                                header: types::MediaPacketHeader {
                                    is_config: true,
                                    is_key: false,
                                    pts: 0,
                                    size: head.len() as u32,
                                },
                                payload: head.into(),
                            }))
                            .await
                            .unwrap();
                        let mut encoder = opus::Encoder::new(
                            48000,
                            opus::Channels::Stereo,
                            opus::Application::Audio,
                        )
                        .unwrap();
                        // Native hosts need not use the file protocol's 20 ms packets.
                        for i in 0..120 {
                            let pcm: Vec<f32> = (0..480)
                                .flat_map(|j| {
                                    let value =
                                        (((i * 480 + j) as f32) * 880.0 * std::f32::consts::TAU
                                            / 48000.0)
                                            .sin()
                                            * 0.2;
                                    [value, value]
                                })
                                .collect();
                            let mut payload = vec![0; 4000];
                            let size = encoder.encode_float(&pcm, &mut payload).unwrap();
                            payload.truncate(size);
                            output
                                .send(types::AVPacket::Media(types::MediaPacket {
                                    header: types::MediaPacketHeader {
                                        is_config: false,
                                        is_key: true,
                                        pts: 1_000_000 + i * 10_000,
                                        size: size as u32,
                                    },
                                    payload: payload.into(),
                                }))
                                .await
                                .unwrap();
                        }
                        done_recv.await.unwrap();
                        connection.close();
                    };
                    let receiving = async {
                        let transport = kynet::Connection::quinn_connect(
                            address,
                            "godot-kyber.local",
                            None,
                            &kynet::quinn::QuinnClientOptions {
                                certificate_hash: Some(hash),
                                ..Default::default()
                            },
                        )
                        .await
                        .unwrap();
                        let connection = kyproto::Connection::connect_with_auth(
                            transport,
                            &kyproto::ClientAuth::new(&token).unwrap(),
                        )
                        .await
                        .unwrap();
                        let endpoint = connection
                            .connect_audio_endpoint(0, kyproto::AudioProtocol::Reliable)
                            .unwrap();
                        let protocol = endpoint.ready().await.unwrap();
                        forward_audio(protocol, |bytes| {
                            assert_eq!(
                                unsafe { on_audio(userdata, bytes.as_ptr(), bytes.len()) },
                                0
                            );
                            frames.audio.received.load(Ordering::Relaxed) < 57600
                        })
                        .await
                        .unwrap();
                        done_send.send(()).unwrap();
                    };
                    tokio::join!(sending, receiving);
                    assert_eq!(frames.audio.received.load(Ordering::Relaxed), 57600);
                    let mut output = crate::audio::PullOutput::default();
                    let mut pcm: Vec<_> = (0..480)
                        .map(|_| godot::classes::native::AudioFrame {
                            left: 0.0,
                            right: 0.0,
                        })
                        .collect();
                    let mut energy = 0.0;
                    for _ in 0..60 {
                        output.mix(&frames.audio, &mut pcm, 0);
                        energy += pcm
                            .iter()
                            .map(|f| f.left * f.left + f.right * f.right)
                            .sum::<f32>();
                    }
                    assert!(energy > 10.0, "received Opus must produce non-silent PCM");
                    assert!(!state.overflow.load(Ordering::Relaxed));
                })
                .await
                .unwrap();
            });
    }
}
