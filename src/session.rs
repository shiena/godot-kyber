// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! A scene-owned connection shared by presentation and control clients.
use crate::{KyberClient, video_node::KyberFileVideo};
use godot::{
    classes::{INode, InputEvent, Node, Texture2D},
    prelude::*,
};

#[derive(GodotClass)]
#[class(base = Node)]
pub(crate) struct KyberSession {
    base: Base<Node>,
    client: Option<Gd<KyberClient>>,
    file: Option<Gd<KyberFileVideo>>,
    #[export]
    auto_reconnect: bool,
    #[export(enum = (Auto = 0, Software = 1, HardwareRequired = 2))]
    acceleration: i32,
    #[export]
    gpu_transfer: bool,
    #[export]
    clipboard_enabled: bool,
    mode: i32,
    state: i32,
    message: GString,
    error: GString,
    count: i64,
    serial: i64,
    displays: Array<VarDictionary>,
}

#[godot_api]
impl INode for KyberSession {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            client: None,
            file: None,
            auto_reconnect: false,
            acceleration: 0,
            gpu_transfer: true,
            clipboard_enabled: false,
            mode: 0,
            state: 0,
            message: "Disconnected".into(),
            error: GString::new(),
            count: 0,
            serial: 0,
            displays: Array::new(),
        }
    }
    fn ready(&mut self) {
        if self.client.is_some() {
            return;
        }
        let mut client = KyberClient::new_alloc();
        client.bind_mut().video_player_audio = true;
        client.connect(
            "clipboard_text_received",
            &self.base().callable("forward_clipboard_text"),
        );
        client.connect(
            "clipboard_available",
            &self.base().callable("forward_clipboard_available"),
        );
        client.connect(
            "clipboard_unavailable",
            &self.base().callable("forward_clipboard_unavailable"),
        );
        client.connect(
            "gamepad_rumble_received",
            &self.base().callable("forward_gamepad_rumble"),
        );
        let mut file = KyberFileVideo::new_alloc();
        file.bind_mut().video_player_audio = true;
        self.base_mut().add_child(&client);
        self.base_mut().add_child(&file);
        self.client = Some(client);
        self.file = Some(file);
    }
    fn process(&mut self, _delta: f64) {
        self.refresh();
    }
    fn exit_tree(&mut self) {
        self.close();
    }
}

#[godot_api]
impl KyberSession {
    pub(crate) fn audio_frames(&self) -> Option<std::sync::Arc<crate::frame::Mailbox>> {
        match self.mode {
            1 => self.file.as_ref().map(|f| f.bind().mailbox()),
            2 => self.client.as_ref().map(|c| c.bind().mailbox()),
            _ => None,
        }
    }
    pub(crate) fn pause_audio(&mut self, paused: bool) {
        if let Some(file) = &mut self.file {
            file.bind_mut().pause_audio(paused);
        }
    }
    #[func]
    fn get_media_capabilities(&self) -> VarDictionary {
        crate::media_capabilities::get()
    }
    #[constant]
    const RECONNECTING: i32 = 8;
    #[func]
    fn get_reconnect_attempts(&self) -> i32 {
        self.client
            .as_ref()
            .map(|c| c.bind().get_reconnect_attempts())
            .unwrap_or(0)
    }
    #[func]
    pub(crate) fn latest_image(&self) -> Option<Gd<godot::classes::Image>> {
        match self.mode {
            1 => self.file.as_ref()?.bind().latest_image(),
            2 => self.client.as_ref()?.bind().latest_image(),
            _ => None,
        }
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
    #[constant]
    const FINISHED: i32 = 7;

    #[signal]
    fn state_changed(state: i32);
    #[signal]
    fn status_changed(message: GString);
    #[signal]
    fn error_occurred(message: GString);
    #[signal]
    fn displays_changed();
    #[signal]
    fn frame_received();
    #[signal]
    fn connected();
    #[signal]
    fn disconnected();

    #[func]
    fn start_receiver(&mut self, host: GString, port: i32, code: GString) -> bool {
        if !self.available() {
            return false;
        }
        self.close();
        self.mode = 1;
        self.file.as_mut().unwrap().bind_mut().gpu_transfer = self.gpu_transfer;
        self.file.as_mut().unwrap().bind_mut().acceleration = self.acceleration;
        let ok = self
            .file
            .as_mut()
            .unwrap()
            .bind_mut()
            .start_receiver(host, port, code);
        self.refresh();
        ok
    }
    #[func]
    fn connect_to_host(
        &mut self,
        host: GString,
        port: i32,
        username: GString,
        password: GString,
        fingerprint: GString,
    ) -> bool {
        if !self.available() {
            return false;
        }
        self.close();
        self.mode = 2;
        self.client
            .as_mut()
            .unwrap()
            .bind_mut()
            .set_auto_reconnect(self.auto_reconnect);
        self.client
            .as_mut()
            .unwrap()
            .bind_mut()
            .set_clipboard_enabled(self.clipboard_enabled);
        let ok = self.client.as_mut().unwrap().bind_mut().connect_to_host(
            host,
            port,
            username,
            password,
            fingerprint,
        );
        self.refresh();
        ok
    }
    #[func]
    fn start_stream(
        &mut self,
        display_id: i64,
        width: i32,
        height: i32,
        bitrate: i64,
        audio: bool,
    ) -> bool {
        if self.mode != 2 || !self.available() {
            return false;
        }
        let ok = self
            .client
            .as_mut()
            .unwrap()
            .bind_mut()
            .start_stream(display_id, width, height, bitrate, audio);
        self.refresh();
        ok
    }
    #[func]
    fn start_demo(&mut self) -> bool {
        if !self.available() {
            return false;
        }
        self.close();
        self.mode = 2;
        self.client.as_mut().unwrap().bind_mut().start_demo();
        self.refresh();
        true
    }
    #[func]
    pub(crate) fn close(&mut self) {
        if let Some(client) = &mut self.client {
            client.bind_mut().close();
        }
        if let Some(file) = &mut self.file {
            file.bind_mut().stop();
        }
        self.mode = 0;
        self.count = 0;
        self.error = GString::new();
        if !self.displays.is_empty() {
            self.displays.clear();
            self.notify("displays_changed", &[]);
        }
        self.publish(0, "Disconnected".into());
    }
    #[func]
    pub(crate) fn get_state(&self) -> i32 {
        self.state
    }
    #[func]
    pub(crate) fn get_status(&self) -> GString {
        self.message.clone()
    }
    #[func]
    pub(crate) fn get_last_error(&self) -> GString {
        self.error.clone()
    }
    #[func]
    pub(crate) fn get_frame_count(&self) -> i64 {
        self.count
    }
    #[func]
    pub(crate) fn is_active(&self) -> bool {
        matches!(self.state, 1..=4 | 6 | 8)
    }
    #[func]
    fn get_displays(&self) -> Array<VarDictionary> {
        self.displays.clone()
    }
    #[func]
    pub(crate) fn get_texture(&self) -> Option<Gd<Texture2D>> {
        match self.mode {
            1 => self.file.as_ref()?.bind().get_texture(),
            2 => self
                .client
                .as_ref()?
                .bind()
                .get_texture()
                .map(|t| t.upcast()),
            _ => None,
        }
    }
    #[func]
    fn get_statistics(&self) -> VarDictionary {
        let mut result = VarDictionary::new();
        result.set(
            "performance",
            &match self.mode {
                1 => self
                    .file
                    .as_ref()
                    .map(|v| v.bind().get_performance_statistics()),
                2 => self
                    .client
                    .as_ref()
                    .map(|v| v.bind().get_performance_statistics()),
                _ => None,
            }
            .unwrap_or_default(),
        );
        result.set("received_frames", self.count);
        result.set(
            "dropped_frames",
            match self.mode {
                1 => self
                    .file
                    .as_ref()
                    .map(|v| v.bind().get_dropped_frames())
                    .unwrap_or(0),
                2 => self
                    .client
                    .as_ref()
                    .map(|v| v.bind().get_dropped_frames())
                    .unwrap_or(0),
                _ => 0,
            },
        );
        if let Some(texture) = self.get_texture() {
            result.set("width", texture.get_width());
            result.set("height", texture.get_height());
        } else {
            result.set("width", 0);
            result.set("height", 0);
        }
        result
    }
    #[func]
    fn get_capabilities(&self) -> VarDictionary {
        let mut result = VarDictionary::new();
        result.set("input", self.mode == 2 && self.state == 4);
        result.set("display_selection", self.mode == 2 && self.state == 2);
        result.set("mouse_wheel", self.mode == 2 && self.state == 4);
        result.set("gamepad", self.mode == 2 && self.state == 4);
        result.set("system_clipboard", self.is_system_clipboard_supported());
        result.set(
            "clipboard_requested",
            self.mode == 2 && self.clipboard_enabled,
        );
        result.set("seek", false);
        result.set("godot_audio", true);
        result.set("native_audio_request", true);
        result
    }
    #[func]
    fn send_input_event(&self, event: Gd<InputEvent>) -> bool {
        self.mode == 2
            && self
                .client
                .as_ref()
                .is_some_and(|c| c.bind().send_input_event(event))
    }
    #[func]
    fn send_key(&self, code: i32, pressed: bool) -> bool {
        self.mode == 2
            && self
                .client
                .as_ref()
                .is_some_and(|c| c.bind().send_key(code, pressed))
    }
    #[func]
    fn send_mouse_motion(&self, dx: i32, dy: i32) -> bool {
        self.mode == 2
            && self
                .client
                .as_ref()
                .is_some_and(|c| c.bind().send_mouse_motion(dx, dy))
    }
    #[func]
    fn send_mouse_button(&self, button: i32, pressed: bool) -> bool {
        self.mode == 2
            && self
                .client
                .as_ref()
                .is_some_and(|c| c.bind().send_mouse_button(button, pressed))
    }

    #[signal]
    fn clipboard_text_received(text: GString);
    #[signal]
    fn gamepad_rumble_received(device: i32, weak: f64, strong: f64, duration: f64);
    #[func]
    fn forward_gamepad_rumble(&mut self, device: i32, weak: f64, strong: f64, duration: f64) {
        self.base_mut().emit_signal(
            "gamepad_rumble_received",
            &[
                device.to_variant(),
                weak.to_variant(),
                strong.to_variant(),
                duration.to_variant(),
            ],
        );
    }
    #[signal]
    fn clipboard_available();
    #[signal]
    fn clipboard_unavailable();
    #[func]
    fn forward_clipboard_text(&mut self, text: GString) {
        self.base_mut()
            .emit_signal("clipboard_text_received", &[text.to_variant()]);
    }
    #[func]
    fn forward_clipboard_available(&mut self) {
        self.base_mut().emit_signal("clipboard_available", &[]);
    }
    #[func]
    fn forward_clipboard_unavailable(&mut self) {
        self.base_mut().emit_signal("clipboard_unavailable", &[]);
    }
    #[func]
    fn send_clipboard_text(&self, text: GString) -> bool {
        self.mode == 2
            && self
                .client
                .as_ref()
                .is_some_and(|c| c.bind().send_clipboard_text(text))
    }
    #[func]
    fn request_clipboard_text(&self) -> bool {
        self.mode == 2
            && self
                .client
                .as_ref()
                .is_some_and(|c| c.bind().request_clipboard_text())
    }
    #[func]
    fn send_system_clipboard(&self) -> bool {
        self.mode == 2
            && self
                .client
                .as_ref()
                .is_some_and(|c| c.bind().send_system_clipboard())
    }
    #[func]
    fn is_system_clipboard_supported(&self) -> bool {
        self.client
            .as_ref()
            .is_some_and(|c| c.bind().is_system_clipboard_supported())
    }
    #[func]
    fn apply_clipboard_text(&self, text: GString) -> bool {
        self.client
            .as_ref()
            .is_some_and(|c| c.bind().apply_clipboard_text(text))
    }
    #[func]
    fn send_gamepad_button(&self, device: i32, button: i32, pressed: bool) -> bool {
        self.mode == 2
            && self
                .client
                .as_ref()
                .is_some_and(|c| c.bind().send_gamepad_button(device, button, pressed))
    }
    #[func]
    fn send_gamepad_axis(&self, device: i32, axis: i32, value: f32) -> bool {
        self.mode == 2
            && self
                .client
                .as_ref()
                .is_some_and(|c| c.bind().send_gamepad_axis(device, axis, value))
    }
    #[func]
    fn send_gamepad_connection(&self, device: i32, connected: bool) -> bool {
        self.mode == 2
            && self
                .client
                .as_ref()
                .is_some_and(|c| c.bind().send_gamepad_connection(device, connected))
    }
    #[func]
    fn send_mouse_wheel(&self, dx: f32, dy: f32) -> bool {
        self.mode == 2
            && self
                .client
                .as_ref()
                .is_some_and(|c| c.bind().send_mouse_wheel(dx, dy))
    }
}

impl KyberSession {
    pub(crate) fn serial(&self) -> i64 {
        self.serial
    }
    fn available(&mut self) -> bool {
        if self.base().is_inside_tree() && self.client.is_some() {
            return true;
        }
        self.error = "Add KyberSession to the scene tree before connecting".into();
        self.notify("error_occurred", &[self.error.to_variant()]);
        self.publish(5, self.error.clone());
        false
    }
    fn notify(&mut self, signal: &str, arguments: &[Variant]) {
        // Deferred delivery lets callbacks safely query or control this session.
        let mut args = vec![signal.to_variant()];
        args.extend_from_slice(arguments);
        self.base_mut().call_deferred("emit_signal", &args);
    }
    fn publish(&mut self, state: i32, message: GString) {
        if self.state != state {
            self.state = state;
            self.notify("state_changed", &[state.to_variant()]);
            if state == 2 {
                self.notify("connected", &[]);
            }
            if state == 0 {
                self.notify("disconnected", &[]);
            }
        }
        if self.message != message {
            self.message = message.clone();
            self.notify("status_changed", &[message.to_variant()]);
        }
    }
    fn refresh(&mut self) {
        let (state, message, error, count) = match self.mode {
            1 => {
                let v = self.file.as_ref().unwrap().bind();
                let state = if v.has_failed() {
                    5
                } else if !v.is_active() {
                    7
                } else if v.get_frame_count() > 0 {
                    4
                } else {
                    1
                };
                (
                    state,
                    v.get_status(),
                    if v.has_failed() {
                        v.get_status()
                    } else {
                        GString::new()
                    },
                    v.get_frame_count(),
                )
            }
            2 => {
                let c = self.client.as_ref().unwrap().bind();
                let state = c.get_state();
                let displays = c.get_displays();
                let values = (
                    state,
                    GString::from(
                        [
                            "Disconnected",
                            "Connecting",
                            "Connected; select a display",
                            "Starting stream",
                            "Receiving",
                            "Failed",
                            "Local demo",
                            "Finished",
                            "Reconnecting",
                        ][state as usize],
                    ),
                    c.get_last_error(),
                    c.get_received_frames(),
                );
                drop(c);
                if displays != self.displays {
                    self.displays = displays;
                    self.notify("displays_changed", &[]);
                }
                values
            }
            _ => return,
        };
        if self.error != error {
            self.error = error.clone();
            if !error.is_empty() {
                self.notify("error_occurred", &[error.to_variant()]);
            }
        }
        let message = if state == 8 {
            let client = self.client.as_ref().unwrap().bind();
            format!(
                "Kyber SDK reconnecting: {}/{}",
                client.get_reconnect_attempts(),
                client.get_reconnect_limit()
            )
            .as_str()
            .into()
        } else {
            message
        };
        self.publish(state, if state == 5 { error } else { message });
        if count != self.count {
            self.count = count;
            self.serial += 1;
            self.notify("frame_received", &[]);
        }
    }
}
