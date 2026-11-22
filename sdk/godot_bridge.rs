// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// Embedded Godot bridge, compiled inside kyclient::capi.
use crate::platform::player::CpuOutput;
use kynput_rs::{
    Config, GamepadAxis, GamepadButton, GamepadControl, GamepadEvent, InputTarget, KeyboardKey,
    MouseButton, MouseButtonType, MouseMove, MouseWheel, Pipeline,
};
use serde::Deserialize;
use serde_json::json;
use std::ffi::{CStr, c_char, c_void};
use std::sync::{Arc, Mutex, Once};

type EventCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize);
type AudioCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
type FrameCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize, u32, u32, u64);

#[derive(Deserialize)]
struct ConnectOptions {
    #[serde(default)]
    auto_reconnection: bool,
    #[serde(default)]
    clipboard_enabled: bool,
    host: String,
    port: u16,
    username: String,
    password: String,
    fingerprint: String,
}

struct Sink {
    gamepads: Arc<Mutex<std::collections::BTreeSet<u32>>>,
    started: Arc<Mutex<bool>>,
    auto_reconnection: bool,
    callback: EventCallback,
    userdata: usize,
}

impl crate::event_manager::EventSink for Sink {
    fn on_event(&self, event: &crate::Event) {
        use crate::Event;
        let displays = |items: &[crate::Display]| {
            items
                .iter()
                .map(|d| {
                    json!({
                        "id": d.id, "width": d.width, "height": d.height
                    })
                })
                .collect::<Vec<_>>()
        };
        if matches!(
            event,
            Event::Reconnecting { .. }
                | Event::Reconnected { .. }
                | Event::DataPlaneClosed
                | Event::StreamingStopped
        ) {
            if let Ok(mut pads) = self.gamepads.lock() {
                pads.clear();
            }
            if let Ok(mut started) = self.started.lock() {
                *started = false;
            }
        }
        let value = match event {
            Event::Reconnecting {
                attempt,
                max_attempts,
            } => json!({"kind":"reconnecting", "attempt":attempt, "max_attempts":max_attempts}),
            Event::Reconnected {
                should_restart_streaming,
            } => json!({"kind":"reconnected", "should_restart_streaming":should_restart_streaming}),
            Event::DataPlaneClosed if self.auto_reconnection => {
                json!({"kind":"transport_interrupted"})
            }
            Event::ControlPlaneConnected { displays: items } => {
                json!({"kind": "connected", "displays": displays(items)})
            }
            Event::DisplayListUpdated { displays: items } => {
                json!({"kind": "displays", "displays": displays(items)})
            }
            Event::StreamingStarted => json!({"kind": "streaming"}),
            Event::StreamingStopped => json!({"kind": "stream_stopped"}),
            Event::Stopped { .. } => json!({"kind": "stopped"}),
            Event::ControlPlaneConnectFailed { reason } => {
                json!({"kind": "error", "message": format!("Connection failed: {reason:?}")})
            }
            Event::AuthenticationRequired { .. } => {
                json!({"kind": "error", "message": "This server requires OIDC; the Godot bridge currently supports basic authentication"})
            }
            Event::DataPlaneConnectFailed
            | Event::DataPlaneClosed
            | Event::StreamingStartFailed
            | Event::ReconnectionFailed => {
                json!({"kind": "error", "message": format!("Kyber: {event:?}")})
            }
            _ => return,
        };
        let bytes = value.to_string();
        unsafe {
            (self.callback)(self.userdata as *mut c_void, bytes.as_ptr(), bytes.len());
        }
    }
}

#[derive(Default)]
struct ClipboardState {
    offered: Option<String>,
    requested: bool,
}
struct ClipboardHandler {
    state: Arc<Mutex<ClipboardState>>,
    sink: std::sync::Weak<Mutex<Option<kynput_rs::Consumer>>>,
    callback: EventCallback,
    userdata: usize,
}
impl kynput_rs::HandlerInterface for ClipboardHandler {
    fn start(&self) {}
    fn stop(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = ClipboardState::default();
        }
    }
    fn produced_types(&self) -> Option<Vec<kynput_rs::InputType>> {
        Some(vec![kynput_rs::InputType::Clipboard])
    }
}
struct RumbleHandler {
    callback: EventCallback,
    userdata: usize,
}
impl kynput_rs::HandlerInterface for RumbleHandler {
    fn start(&self) {}
    fn stop(&self) {}
    fn produced_types(&self) -> Option<Vec<kynput_rs::InputType>> {
        Some(vec![])
    }
}
impl kynput_rs::HandlerConsumerInterface for RumbleHandler {
    fn handled_types(&self) -> Vec<kynput_rs::InputType> {
        vec![kynput_rs::InputType::GamepadEvent]
    }
}
impl kynput_rs::ConsumerInterface for RumbleHandler {
    fn consume(&self, packet: kynput_rs::Packet) {
        if let Some((device, strong, weak, duration)) = kynput_rs::read_rumble(&packet) {
            let value = json!({"kind":"gamepad_rumble", "device":device,
                "strong":strong, "weak":weak, "duration_ms":duration})
            .to_string();
            unsafe {
                (self.callback)(self.userdata as *mut c_void, value.as_ptr(), value.len());
            }
        }
    }
}
impl kynput_rs::HandlerConsumerInterface for ClipboardHandler {
    fn handled_types(&self) -> Vec<kynput_rs::InputType> {
        vec![kynput_rs::InputType::Clipboard]
    }
}
impl kynput_rs::ConsumerInterface for ClipboardHandler {
    fn consume(&self, packet: kynput_rs::Packet) {
        use kynput_rs::TextClipboard;
        let value = match TextClipboard::from_packet(&packet) {
            Some(TextClipboard::Changed) => json!({"kind":"clipboard_available"}),
            Some(TextClipboard::Text(text)) => {
                let Ok(mut state) = self.state.lock() else {
                    return;
                };
                if !std::mem::take(&mut state.requested) {
                    return;
                }
                json!({"kind":"clipboard_text", "message":text})
            }
            Some(TextClipboard::Unavailable) => {
                if let Ok(mut state) = self.state.lock() {
                    state.requested = false;
                }
                json!({"kind":"clipboard_unavailable"})
            }
            Some(TextClipboard::Request) => {
                let offered = self.state.lock().ok().and_then(|s| s.offered.clone());
                let response = offered
                    .map(TextClipboard::Text)
                    .unwrap_or(TextClipboard::Unavailable);
                if let Some(sink) = self.sink.upgrade() {
                    if let Ok(sink) = sink.lock() {
                        if let (Some(sink), Some(packet)) =
                            (sink.as_ref(), response.to_packet(InputTarget::Host))
                        {
                            sink.consume(packet);
                        }
                    }
                }
                return;
            }
            _ => return,
        };
        let bytes = value.to_string();
        unsafe {
            (self.callback)(self.userdata as *mut c_void, bytes.as_ptr(), bytes.len());
        }
    }
}

pub struct GodotSession {
    client: crate::Client,
    pipeline: Pipeline,
    gamepads: Arc<Mutex<std::collections::BTreeSet<u32>>>,
    clipboard_enabled: bool,
    clipboard: Arc<Mutex<ClipboardState>>,
    _clipboard_sink: Arc<Mutex<Option<kynput_rs::Consumer>>>,
    frame_callback: FrameCallback,
    audio_callback: AudioCallback,
    event_callback: EventCallback,
    userdata: usize,
    started: Arc<Mutex<bool>>,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kyclient_godot_create(
    config: *const c_char,
    event: EventCallback,
    frame: FrameCallback,
    audio: AudioCallback,
    userdata: *mut c_void,
) -> *mut GodotSession {
    if config.is_null() || userdata.is_null() {
        return std::ptr::null_mut();
    }
    let result = std::panic::catch_unwind(|| -> Option<GodotSession> {
        let options: ConnectOptions =
            serde_json::from_slice(unsafe { CStr::from_ptr(config) }.to_bytes()).ok()?;
        if options.host.trim().is_empty() || options.port == 0 {
            return None;
        }
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            // File video and RPC may have installed the same process-wide provider.
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
        let mut config = crate::ConnectConfig {
            host: options.host,
            port: options.port,
            credentials: crate::AuthCredentials::Basic {
                username: options.username,
                password: options.password,
            },
            tls_host: None,
            verify_mode: None,
            auto_reconnection: options.auto_reconnection,
        };
        if !options.fingerprint.is_empty() {
            config.set_tls_fingerprint(&options.fingerprint).ok()?;
        }
        let client = crate::Client::new(config, false).ok()?;
        let started = Arc::new(Mutex::new(false));
        let gamepads = Arc::new(Mutex::new(Default::default()));
        client.get_event_manager().add_event_sink(Box::new(Sink {
            gamepads: gamepads.clone(),
            started: started.clone(),
            auto_reconnection: options.auto_reconnection,
            callback: event,
            userdata: userdata as usize,
        }));
        let pipeline = Pipeline::new(Config::external_client());
        pipeline
            .plug_handler(kynput_rs::create_handler_consumer_instance(Arc::new(
                RumbleHandler {
                    callback: event,
                    userdata: userdata as usize,
                },
            )))
            .ok()?;
        let clipboard = Arc::new(Mutex::new(ClipboardState::default()));
        let clipboard_sink = Arc::new(Mutex::new(Some(pipeline.get_input_sink())));
        if options.clipboard_enabled {
            pipeline
                .plug_handler(kynput_rs::create_handler_consumer_instance(Arc::new(
                    ClipboardHandler {
                        state: clipboard.clone(),
                        sink: Arc::downgrade(&clipboard_sink),
                        callback: event,
                        userdata: userdata as usize,
                    },
                )))
                .ok()?;
        }
        let session = GodotSession {
            client,
            pipeline,
            gamepads,
            clipboard_enabled: options.clipboard_enabled,
            clipboard,
            _clipboard_sink: clipboard_sink,
            frame_callback: frame,
            audio_callback: audio,
            event_callback: event,
            userdata: userdata as usize,
            started,
        };
        session.client.connect().ok()?;
        Some(session)
    });
    match result {
        Ok(Some(session)) => Box::into_raw(Box::new(session)),
        _ => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kyclient_godot_start(
    session: *mut GodotSession,
    display: u32,
    width: u32,
    height: u32,
    bitrate: u32,
    audio: bool,
) -> i32 {
    if !kyber_media::capabilities().h264_decode {
        return -3;
    }
    if session.is_null()
        || width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || bitrate == 0
    {
        return -1;
    }
    let session = unsafe { &*session };
    let Ok(mut started) = session.started.lock() else {
        return -1;
    };
    if *started {
        return -1;
    }
    let config = crate::StreamingSessionConfig {
        protocol: crate::StreamingProtocol::Kymux(kyctl_common::kymux::KymuxConfig::KyProto(
            kyctl_common::kymux::KyProtoConfig {
                video_protocol: kyctl_common::kymux::VideoProtocol::GopStream,
                audio_protocol: kyctl_common::kymux::AudioProtocol::Reliable,
            },
        )),
        bitrate,
        video: vec![crate::VideoSessionConfig {
            display_id: display,
            codec: crate::VideoCodec::H264,
            yuv444: false,
            intra_refresh: None,
            player: crate::VideoPlayerConfig {
                window_handle: crate::platform::player::WindowHandle::Cpu(CpuOutput {
                    width,
                    height,
                    callback: session.frame_callback,
                    audio: session.audio_callback,
                    error: session.event_callback,
                    userdata: session.userdata,
                }),
                software_decode: true,
                video_buffer: 0,
            },
        }],
        audio: audio.then_some(crate::AudioSessionConfig { audio_buffer: None }),
        input: Some(crate::InputSessionConfig {
            pipeline: session.pipeline.clone(),
            gamepad_force_xbox360: None,
            clipboard: session.clipboard_enabled,
        }),
    };
    if session.client.start_streaming(config).is_err() {
        return -1;
    }
    *started = true;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kyclient_godot_input(
    session: *mut GodotSession,
    kind: u32,
    a: i32,
    b: i32,
    pressed: bool,
) -> i32 {
    if session.is_null() {
        return -1;
    }
    let session = unsafe { &*session };
    let target = InputTarget::Host;
    let packet = match kind {
        0 if u16::try_from(a).is_ok() => {
            let Ok(key) = keycode::KeyMap::from_usb_code(7, a as u16) else {
                return -1;
            };
            KeyboardKey {
                scancode: key.win,
                pressed,
            }
            .to_packet(target)
        }
        1 if i16::try_from(a).is_ok() && i16::try_from(b).is_ok() => MouseMove {
            dx: a as i16,
            dy: b as i16,
        }
        .to_packet(target),
        2 => {
            let button = match a {
                0 => MouseButtonType::Left,
                1 => MouseButtonType::Right,
                2 => MouseButtonType::Middle,
                _ => return -1,
            };
            MouseButton { button, pressed }.to_packet(target)
        }
        3 => {
            // Match src/backend.rs: a/b contain normalized wheel f32 bits.
            let dx = f32::from_bits(a as u32);
            let dy = f32::from_bits(b as u32);
            if !dx.is_finite() || !dy.is_finite() || dx.abs() > 1.0 || dy.abs() > 1.0 {
                return -1;
            }
            MouseWheel { dx, dy }.to_packet(target)
        }
        4 | 5 => {
            let id = (a as u32) >> 16;
            let control = (a as u32) & 0xffff;
            if a < 0 || id >= 32 {
                return -1;
            }
            let packet = if kind == 4 {
                let button = match control {
                    0 => GamepadButton::A,
                    1 => GamepadButton::B,
                    2 => GamepadButton::X,
                    3 => GamepadButton::Y,
                    4 => GamepadButton::Back,
                    5 => GamepadButton::Guide,
                    6 => GamepadButton::Start,
                    7 => GamepadButton::LeftStick,
                    8 => GamepadButton::RightStick,
                    9 => GamepadButton::LeftShoulder,
                    10 => GamepadButton::RightShoulder,
                    11 => GamepadButton::DPadUp,
                    12 => GamepadButton::DPadDown,
                    13 => GamepadButton::DPadLeft,
                    14 => GamepadButton::DPadRight,
                    _ => return -1,
                };
                GamepadEvent::Button {
                    id,
                    type_: button,
                    pressed,
                }
                .to_packet(target)
            } else {
                let axis = match control {
                    0 => GamepadAxis::LeftX,
                    1 => GamepadAxis::LeftY,
                    2 => GamepadAxis::RightX,
                    3 => GamepadAxis::RightY,
                    4 => GamepadAxis::TriggerLeft,
                    5 => GamepadAxis::TriggerRight,
                    _ => return -1,
                };
                let Ok(value) = i16::try_from(b) else {
                    return -1;
                };
                GamepadEvent::Axis {
                    id,
                    type_: axis,
                    value,
                }
                .to_packet(target)
            };
            let Ok(mut pads) = session.gamepads.lock() else {
                return -1;
            };
            if pads.insert(id) {
                session.pipeline.get_input_sink().consume(
                    GamepadControl::Plug {
                        id,
                        vendor_id: None,
                        product_id: None,
                    }
                    .to_packet(target)
                    .expect("Valid gamepad control"),
                );
            }
            let Some(packet) = packet else { return -1 };
            packet
        }
        6 if (0..32).contains(&a) => {
            let id = a as u32;
            let Ok(mut pads) = session.gamepads.lock() else {
                return -1;
            };
            if pressed {
                if !pads.insert(id) {
                    return 0;
                }
                GamepadControl::Plug {
                    id,
                    vendor_id: None,
                    product_id: None,
                }
                .to_packet(target)
                .expect("Valid gamepad control")
            } else {
                if !pads.remove(&id) {
                    return 0;
                }
                GamepadControl::Unplug { id }
                    .to_packet(target)
                    .expect("Valid gamepad control")
            }
        }
        _ => return -1,
    };
    session.pipeline.get_input_sink().consume(packet);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kyclient_godot_clipboard(
    session: *mut GodotSession,
    operation: u32,
    text: *const u8,
    len: usize,
) -> i32 {
    if session.is_null() {
        return -1;
    }
    let session = unsafe { &*session };
    if !session.clipboard_enabled {
        return -1;
    }
    let Ok(mut state) = session.clipboard.lock() else {
        return -1;
    };
    let event = match operation {
        0 => {
            if len > 60 * 1024 || text.is_null() {
                return -1;
            }
            let Ok(text) = std::str::from_utf8(unsafe { std::slice::from_raw_parts(text, len) })
            else {
                return -1;
            };
            state.offered = Some(text.to_owned());
            kynput_rs::TextClipboard::Changed
        }
        1 => {
            state.requested = true;
            kynput_rs::TextClipboard::Request
        }
        _ => return -1,
    };
    drop(state);
    let Some(packet) = event.to_packet(InputTarget::Host) else {
        return -1;
    };
    session.pipeline.get_input_sink().consume(packet);
    0
}

impl Drop for GodotSession {
    fn drop(&mut self) {
        if let Ok(pads) = self.gamepads.lock() {
            for &id in pads.iter() {
                self.pipeline.get_input_sink().consume(
                    GamepadControl::Unplug { id }
                        .to_packet(InputTarget::Host)
                        .expect("Valid gamepad control"),
                );
            }
        }
        let _ = self.pipeline.flush_keys();
        // Disconnect waits for media teardown; stop then joins the owned runtime.
        let _ = self.client.disconnect_and_stop(false);
        let _ = self.client.stop();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kyclient_godot_destroy(session: *mut GodotSession) {
    if !session.is_null() {
        drop(unsafe { Box::from_raw(session) });
    }
}
