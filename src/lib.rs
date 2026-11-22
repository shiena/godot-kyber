// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

mod audio;
mod audio_capture;
mod audio_stream;
mod backend;
mod file_video;
mod frame;
#[cfg(windows)]
mod gpu_texture;
mod input_event;
mod media_capabilities;
mod multiplayer;
mod multiplayer_transport;
mod session;
mod statistics;
mod video_codec;
mod video_node;
mod video_stream;

use backend::{Config, Display, Session};
use frame::{Frame, Mailbox};
use godot::classes::{INode, Image, ImageTexture, Node};
use godot::prelude::*;
use std::sync::Arc;

struct KyberExtension;

// Native callbacks decode/copy into mailboxes without accessing Godot objects.
#[gdextension]
unsafe impl ExtensionLibrary for KyberExtension {}

#[derive(GodotClass)]
#[class(base = Node)]
pub(crate) struct KyberClient {
    base: Base<Node>,
    session: Option<Session>,
    auto_reconnect: bool,
    clipboard_enabled: bool,
    reconnect_attempt: i32,
    reconnect_max: i32,
    stream_request: Option<(i64, i32, i32, i64, bool)>,
    frames: Arc<Mailbox>,
    pub(crate) video_player_audio: bool,
    audio_output: Option<audio::Output>,
    texture: Option<Gd<ImageTexture>>,
    latest_image: Option<Gd<Image>>,
    displays: Vec<Display>,
    state: i32,
    error: String,
    demo_time: Option<f64>,
    received_frames: u64,
}

#[godot_api]
impl INode for KyberClient {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            session: None,
            auto_reconnect: false,
            clipboard_enabled: false,
            reconnect_attempt: 0,
            reconnect_max: 0,
            stream_request: None,
            frames: Arc::default(),
            video_player_audio: false,
            audio_output: None,
            texture: None,
            latest_image: None,
            displays: vec![],
            state: 0,
            error: String::new(),
            demo_time: None,
            received_frames: 0,
        }
    }

    fn process(&mut self, delta: f64) {
        let events = self
            .session
            .as_ref()
            .map(Session::events)
            .unwrap_or_default();
        for event in events {
            match event.kind.as_str() {
                "gamepad_rumble" => {
                    self.base_mut().emit_signal(
                        "gamepad_rumble_received",
                        &[
                            event.device.to_variant(),
                            (f64::from(event.weak) / 65535.0).to_variant(),
                            (f64::from(event.strong) / 65535.0).to_variant(),
                            (if event.duration_ms < 0 {
                                5.0
                            } else {
                                f64::from(event.duration_ms) / 1000.0
                            })
                            .to_variant(),
                        ],
                    );
                }
                "clipboard_text" => {
                    self.base_mut().emit_signal(
                        "clipboard_text_received",
                        &[GString::from(&event.message).to_variant()],
                    );
                }
                "clipboard_available" => {
                    self.base_mut().emit_signal("clipboard_available", &[]);
                }
                "clipboard_unavailable" => {
                    self.base_mut().emit_signal("clipboard_unavailable", &[]);
                }
                "connected" => {
                    self.displays = event.displays;
                    self.set_state(2);
                    self.base_mut().emit_signal("connected", &[]);
                }
                "displays" => {
                    self.displays = event.displays;
                    self.base_mut().emit_signal("displays_changed", &[]);
                }
                "reconnecting" => {
                    self.reconnect_attempt = event.attempt;
                    self.reconnect_max = event.max_attempts;
                    self.set_state(8);
                }
                "transport_interrupted" => self.set_state(8),
                "reconnected" => {
                    self.error.clear();
                    self.set_state(2);
                    if event.should_restart_streaming {
                        if let Some((display, width, height, bitrate, audio)) = self.stream_request
                        {
                            if !self.start_stream(display, width, height, bitrate, audio) {
                                self.fail(self.error.clone());
                                break;
                            }
                        } else {
                            self.fail(
                                "SDK requested stream restoration without saved settings".into(),
                            );
                            break;
                        }
                    } else {
                        self.base_mut().emit_signal("connected", &[]);
                    }
                }
                "streaming" => self.set_state(4),
                "stopped" => {
                    self.close();
                    break;
                }
                "stream_stopped" => {
                    self.close();
                    break;
                }
                "error" => {
                    self.fail(event.message);
                    break;
                }
                _ => {}
            }
        }
        if let Some(time) = &mut self.demo_time {
            *time += delta;
            let width = 320;
            let height = 180;
            let shift = (*time * 60.0) as u32;
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for y in 0..height {
                for x in 0..width {
                    rgba.extend_from_slice(&[
                        ((x + shift) % 256) as u8,
                        (y * 255 / height) as u8,
                        160,
                        255,
                    ]);
                }
            }
            self.frames.put(Frame {
                #[cfg(windows)]
                gpu: None,
                width,
                height,
                rgba,
            });
        }
        if !self.video_player_audio
            && self.audio_output.is_none()
            && self
                .frames
                .audio
                .received
                .load(std::sync::atomic::Ordering::Relaxed)
                > 0
        {
            let output = audio::Output::new(&mut self.base_mut());
            self.audio_output = Some(output);
        }
        if let Some(output) = &mut self.audio_output {
            output.pump(&self.frames.audio);
        }
        let upload_start = std::time::Instant::now();
        if let Some(frame) = self.frames.take() {
            let bytes = PackedByteArray::from(frame.rgba.as_slice());
            if let Some(image) = Image::create_from_data(
                frame.width as i32,
                frame.height as i32,
                false,
                godot::classes::image::Format::RGBA8,
                &bytes,
            ) {
                match &mut self.texture {
                    Some(texture)
                        if texture.get_width() == frame.width as i32
                            && texture.get_height() == frame.height as i32 =>
                    {
                        texture.update(&image)
                    }
                    Some(texture) => texture.set_image(&image),
                    None => self.texture = ImageTexture::create_from_image(&image),
                }
                self.latest_image = Some(image);
                self.received_frames += 1;
                self.frames.presented(upload_start.elapsed());
                self.base_mut().emit_signal("frame_received", &[]);
            }
        }
    }

    fn exit_tree(&mut self) {
        self.close();
    }
}

#[godot_api]
impl KyberClient {
    /// Applies to the next SDK connection; the SDK owns retries and cancellation.
    #[func]
    pub(crate) fn set_auto_reconnect(&mut self, enabled: bool) {
        self.auto_reconnect = enabled;
    }
    #[func]
    pub(crate) fn get_reconnect_attempts(&self) -> i32 {
        self.reconnect_attempt
    }
    #[func]
    pub(crate) fn get_reconnect_limit(&self) -> i32 {
        self.reconnect_max
    }
    #[constant]
    const RECONNECTING: i32 = 8;
    pub(crate) fn latest_image(&self) -> Option<Gd<Image>> {
        self.latest_image.clone()
    }
    #[constant]
    const DISCONNECTED: i32 = 0;
    #[constant]
    const CONNECTING: i32 = 1;
    #[constant]
    const CONNECTED: i32 = 2;
    #[constant]
    const STARTING: i32 = 3;
    #[constant]
    const STREAMING: i32 = 4;
    #[constant]
    const FAILED: i32 = 5;
    #[constant]
    const DEMO: i32 = 6;

    #[signal]
    fn connected();
    #[signal]
    fn disconnected();
    #[signal]
    fn displays_changed();
    #[signal]
    fn state_changed(state: i32);
    #[signal]
    fn error_occurred(message: GString);
    #[signal]
    fn frame_received();

    /// Uses the embedded Kyber SDK and queues an authenticated connection.
    #[func]
    pub(crate) fn connect_to_host(
        &mut self,
        host: GString,
        port: i32,
        username: GString,
        password: GString,
        fingerprint: GString,
    ) -> bool {
        self.close();
        self.error.clear();
        let Ok(port) = u16::try_from(port) else {
            self.fail("Invalid port".into());
            return false;
        };
        let config = Config {
            clipboard_enabled: self.clipboard_enabled,
            auto_reconnection: self.auto_reconnect,
            host: host.to_string(),
            port,
            username: username.to_string(),
            password: password.to_string(),
            fingerprint: fingerprint.to_string(),
        };
        self.frames = Arc::default();
        match Session::connect(config, self.frames.clone()) {
            Ok(session) => {
                self.session = Some(session);
                self.set_state(1);
                true
            }
            Err(error) => {
                self.fail(error);
                false
            }
        }
    }

    /// Requests H.264/Kymux playback after the connected signal.
    #[func]
    pub(crate) fn start_stream(
        &mut self,
        display_id: i64,
        width: i32,
        height: i32,
        bitrate: i64,
        audio: bool,
    ) -> bool {
        if self.state != 2 {
            return self.reject("Connect before starting a stream");
        }
        let (Ok(display_id), Ok(width), Ok(height), Ok(bitrate)) = (
            u32::try_from(display_id),
            u32::try_from(width),
            u32::try_from(height),
            u32::try_from(bitrate),
        ) else {
            return self.reject("Invalid stream parameters");
        };
        if !self.displays.iter().any(|d| d.id == display_id) {
            return self.reject("Unknown display ID");
        }
        let result = self
            .session
            .as_ref()
            .ok_or("No native session".to_owned())
            .and_then(|s| s.start(display_id, width, height, bitrate, audio));
        match result {
            Ok(()) => {
                self.stream_request = Some((
                    display_id as i64,
                    width as i32,
                    height as i32,
                    bitrate as i64,
                    audio,
                ));
                self.set_state(3);
                true
            }
            Err(error) => self.reject(&error),
        }
    }

    #[func]
    pub(crate) fn close(&mut self) {
        self.audio_output = None;
        self.frames
            .closed
            .store(true, std::sync::atomic::Ordering::Release);
        self.session = None;
        self.stream_request = None;
        self.reconnect_attempt = 0;
        self.reconnect_max = 0;
        self.demo_time = None;
        self.frames = Arc::default();
        self.texture = None;
        self.latest_image = None;
        self.displays.clear();
        self.received_frames = 0;
        if self.state != 0 {
            self.set_state(0);
            self.base_mut().emit_signal("disconnected", &[]);
        }
    }

    /// Generates local frames only; this does not open a network connection.
    #[func]
    pub(crate) fn start_demo(&mut self) {
        self.close();
        self.error.clear();
        self.demo_time = Some(0.0);
        self.set_state(6);
    }

    #[func]
    pub(crate) fn get_texture(&self) -> Option<Gd<ImageTexture>> {
        self.texture.clone()
    }
    #[func]
    pub(crate) fn get_state(&self) -> i32 {
        self.state
    }
    #[func]
    pub(crate) fn get_last_error(&self) -> GString {
        GString::from(&self.error)
    }
    #[func]
    pub(crate) fn get_received_frames(&self) -> i64 {
        self.received_frames as i64
    }
    pub(crate) fn mailbox(&self) -> Arc<Mailbox> {
        self.frames.clone()
    }
    #[func]
    pub(crate) fn get_performance_statistics(&self) -> VarDictionary {
        self.frames.statistics()
    }
    #[func]
    pub(crate) fn get_dropped_frames(&self) -> i64 {
        self.frames.dropped() as i64
    }

    #[func]
    pub(crate) fn get_displays(&self) -> Array<VarDictionary> {
        self.displays
            .iter()
            .map(|d| {
                let mut value = VarDictionary::new();
                value.set("id", d.id as i64);
                value.set("width", d.width);
                value.set("height", d.height);
                value
            })
            .collect()
    }

    /// Translates supported Godot keyboard and mouse events to Kyber input.
    #[func]
    pub(crate) fn send_input_event(&self, event: Gd<godot::classes::InputEvent>) -> bool {
        input_event::translate(event)
            .is_some_and(|(kind, a, b, pressed)| self.send_input(kind, a, b, pressed))
    }

    /// Scancodes use USB HID keyboard usage IDs, not Godot keycodes.
    #[func]
    pub(crate) fn send_key(&self, scancode: i32, pressed: bool) -> bool {
        (0..=u16::MAX as i32).contains(&scancode) && self.send_input(0, scancode, 0, pressed)
    }

    #[func]
    pub(crate) fn send_mouse_motion(&self, dx: i32, dy: i32) -> bool {
        i16::try_from(dx).is_ok() && i16::try_from(dy).is_ok() && self.send_input(1, dx, dy, false)
    }

    /// Buttons: 0=left, 1=right, 2=middle.
    #[func]
    pub(crate) fn send_mouse_button(&self, button: i32, pressed: bool) -> bool {
        (0..=2).contains(&button) && self.send_input(2, button, 0, pressed)
    }

    #[signal]
    fn clipboard_text_received(text: GString);
    #[signal]
    fn gamepad_rumble_received(device: i32, weak: f64, strong: f64, duration: f64);
    #[signal]
    fn clipboard_available();
    #[signal]
    fn clipboard_unavailable();
    #[func]
    pub(crate) fn set_clipboard_enabled(&mut self, enabled: bool) -> bool {
        if self.session.is_some() {
            return false;
        }
        self.clipboard_enabled = enabled;
        true
    }
    #[func]
    pub(crate) fn send_clipboard_text(&self, text: GString) -> bool {
        self.state == 4
            && self.clipboard_enabled
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.clipboard(0, &text.to_string()))
    }
    #[func]
    pub(crate) fn request_clipboard_text(&self) -> bool {
        self.state == 4
            && self.clipboard_enabled
            && self.session.as_ref().is_some_and(|s| s.clipboard(1, ""))
    }
    #[func]
    pub(crate) fn is_system_clipboard_supported(&self) -> bool {
        godot::classes::DisplayServer::singleton()
            .has_feature(godot::classes::display_server::Feature::CLIPBOARD)
    }
    #[func]
    pub(crate) fn send_system_clipboard(&self) -> bool {
        if self.state != 4 || !self.clipboard_enabled || !self.is_system_clipboard_supported() {
            return false;
        }
        self.send_clipboard_text(godot::classes::DisplayServer::singleton().clipboard_get())
    }
    #[func]
    pub(crate) fn apply_clipboard_text(&self, text: GString) -> bool {
        if !self.is_system_clipboard_supported() || text.to_string().len() > 60 * 1024 {
            return false;
        }
        godot::classes::DisplayServer::singleton().clipboard_set(&text);
        true
    }

    #[func]
    pub(crate) fn send_gamepad_button(&self, device: i32, button: i32, pressed: bool) -> bool {
        input_event::gamepad_button(device, button, pressed)
            .is_some_and(|(k, a, b, p)| self.send_input(k, a, b, p))
    }
    #[func]
    pub(crate) fn send_gamepad_axis(&self, device: i32, axis: i32, value: f32) -> bool {
        input_event::gamepad_axis(device, axis, value)
            .is_some_and(|(k, a, b, p)| self.send_input(k, a, b, p))
    }
    #[func]
    pub(crate) fn send_gamepad_connection(&self, device: i32, connected: bool) -> bool {
        (0..32).contains(&device) && self.send_input(6, device, 0, connected)
    }

    fn send_input(&self, kind: u32, a: i32, b: i32, pressed: bool) -> bool {
        self.state == 4
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.input(kind, a, b, pressed))
    }

    #[func]
    pub(crate) fn send_mouse_wheel(&self, dx: f32, dy: f32) -> bool {
        input_event::wheel(dx, dy)
            .is_some_and(|(kind, a, b, pressed)| self.send_input(kind, a, b, pressed))
    }

    fn set_state(&mut self, state: i32) {
        if self.state != state {
            self.state = state;
            self.base_mut()
                .emit_signal("state_changed", &[state.to_variant()]);
        }
    }

    fn reject(&mut self, message: &str) -> bool {
        self.error = message.to_owned();
        self.base_mut()
            .emit_signal("error_occurred", &[GString::from(message).to_variant()]);
        false
    }

    fn fail(&mut self, message: String) {
        self.close();
        self.set_state(5);
        self.reject(&message);
    }
}
