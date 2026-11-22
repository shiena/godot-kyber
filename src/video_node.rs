// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

use crate::{
    file_video::{Mode, Session},
    frame::Mailbox,
};
use godot::{
    classes::{INode, Image, ImageTexture, Node, RenderingServer, Texture2D, Viewport},
    prelude::*,
};
use std::sync::Arc;

#[derive(GodotClass)]
#[class(base = Node)]
pub(crate) struct KyberFileVideo {
    base: Base<Node>,
    #[export]
    audio_enabled: bool,
    #[export(enum = (Reliable = 0, Unreliable = 1, UnreliableFec = 2))]
    transport_mode: i32,
    #[export(enum = (Jpeg = 0, H264 = 1))]
    video_codec: i32,
    #[export(enum = (Auto = 0, Software = 1, HardwareRequired = 2))]
    pub(crate) acceleration: i32,
    #[export]
    pub(crate) gpu_transfer: bool,
    #[cfg(windows)]
    gpu: crate::gpu_texture::Presenter,
    session: Option<Session>,
    audio_output: Option<crate::audio::Output>,
    pub(crate) video_player_audio: bool,
    frames: Arc<Mailbox>,
    texture: Option<Gd<ImageTexture>>,
    latest_image: Option<Gd<Image>>,
    message: String,
    code: String,
    failed: bool,
    count: u64,
    viewport: Option<Gd<Viewport>>,
    capture_send: Option<tokio::sync::mpsc::Sender<crate::file_video::SourceEvent>>,
    audio_capture: Option<crate::audio_capture::Capture>,
    capture_origin: std::time::Instant,
    next_capture: std::time::Instant,
    capture_period: std::time::Duration,
    #[cfg(target_os = "ios")]
    picker_open: bool,
}

#[godot_api]
impl INode for KyberFileVideo {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            audio_enabled: true,
            transport_mode: 0,
            video_codec: 0,
            acceleration: 0,
            gpu_transfer: true,
            #[cfg(windows)]
            gpu: Default::default(),
            session: None,
            audio_output: None,
            video_player_audio: false,
            frames: Arc::default(),
            texture: None,
            latest_image: None,
            message: "Stopped".into(),
            code: String::new(),
            failed: false,
            count: 0,
            viewport: None,
            capture_send: None,
            audio_capture: None,
            capture_origin: std::time::Instant::now(),
            next_capture: std::time::Instant::now(),
            capture_period: std::time::Duration::from_secs_f64(1.0 / 24.0),
            #[cfg(target_os = "ios")]
            picker_open: false,
        }
    }

    fn process(&mut self, _delta: f64) {
        #[cfg(target_os = "ios")]
        if self.picker_open {
            let result = unsafe { kg_ios_picker_result() };
            if !result.is_null() {
                self.picker_open = false;
                let result = unsafe { std::ffi::CStr::from_ptr(result) }.to_string_lossy();
                if let Some(path) = result.strip_prefix("ok:") {
                    self.base_mut()
                        .emit_signal("video_file_selected", &[GString::from(path).to_variant()]);
                } else {
                    let error = result.strip_prefix("error:").unwrap_or("");
                    self.base_mut().emit_signal(
                        "video_file_dialog_closed",
                        &[GString::from(error).to_variant()],
                    );
                }
            }
        }
        if let (Some(capture), Some(send)) = (&mut self.audio_capture, &self.capture_send)
            && let Err(error) = capture.pump(
                send,
                self.session.as_ref().is_some_and(|s| {
                    s.status.lock().unwrap_or_else(|e| e.into_inner()).clients > 0
                }),
            )
        {
            self.stop();
            self.fail(error);
            return;
        }
        if let Some(session) = &self.session {
            let status = session
                .status
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            self.message = status.message;
            self.code = status.code;
            self.failed = status.failed;
            if status.finished {
                self.session.take();
                self.clear_capture();
            }
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
            let output = crate::audio::Output::new(&mut self.base_mut());
            self.audio_output = Some(output);
        }
        if let Some(output) = &mut self.audio_output {
            output.pump(&self.frames.audio);
        }
        let upload_start = std::time::Instant::now();
        #[cfg(windows)]
        if self.gpu.pump() {
            self.count += 1;
            self.frames.presented(upload_start.elapsed());
        }
        let Some(frame) = self.frames.take() else {
            return;
        };
        #[cfg(windows)]
        if let Some(gpu) = frame.gpu {
            self.gpu.submit(gpu);
            return;
        }
        #[cfg(windows)]
        if self.gpu.texture().is_some() {
            self.gpu = Default::default();
        }
        if let Some(image) = Image::create_from_data(
            frame.width as i32,
            frame.height as i32,
            false,
            godot::classes::image::Format::RGBA8,
            &PackedByteArray::from(frame.rgba.as_slice()),
        ) {
            if let Some(texture) = &mut self.texture {
                texture.update(&image);
            } else {
                self.texture = ImageTexture::create_from_image(&image);
            }
            self.latest_image = Some(image);
            self.count += 1;
            self.frames.presented(upload_start.elapsed());
        }
    }

    fn exit_tree(&mut self) {
        self.stop();
    }
}

#[godot_api]
impl KyberFileVideo {
    pub(crate) fn mailbox(&self) -> Arc<Mailbox> {
        self.frames.clone()
    }
    pub(crate) fn pause_audio(&mut self, paused: bool) {
        if let Some(output) = &mut self.audio_output {
            output.pause(paused);
        }
    }
    /// Capture only after rendering; a bounded channel prevents frame accumulation.
    #[func]
    fn capture_viewport_frame(&mut self) {
        let Some(send) = &self.capture_send else {
            return;
        };
        let now = std::time::Instant::now();
        if now < self.next_capture || send.capacity() == 0 {
            return;
        }
        self.next_capture = now + self.capture_period;
        let Some(viewport) = self.viewport.as_ref().filter(|v| v.is_instance_valid()) else {
            self.stop();
            self.fail("The captured Viewport was freed".into());
            return;
        };
        let Some(texture) = viewport.get_texture() else {
            return;
        };
        let Some(mut image) = texture.get_image() else {
            return;
        };
        if image.is_empty()
            || crate::frame::frame_size(image.get_width() as u32, image.get_height() as u32)
                .is_none()
        {
            return;
        }
        image.convert(godot::classes::image::Format::RGBA8);
        let frame = kyber_media::VideoFrame {
            width: image.get_width() as u32,
            height: image.get_height() as u32,
            pts_us: match kyproto::clock::now_micros() {
                Ok(time) => time,
                Err(_) => return,
            },
            rgba: image.get_data().to_vec(),
        };
        self.frames.record("capture", now.elapsed());
        let _ = send.try_send(crate::file_video::SourceEvent::Video(frame));
    }

    #[func]
    fn start_viewport_sender(
        &mut self,
        viewport: Gd<Viewport>,
        bind_ip: GString,
        port: i32,
        fps: i32,
    ) -> bool {
        self.stop();
        if godot::classes::DisplayServer::singleton().get_name() == "headless" {
            return self.fail("Viewport capture requires a rendering display server".into());
        }
        if !(1..=24).contains(&fps) || !(1..=65535).contains(&port) || !viewport.is_instance_valid()
        {
            return self
                .fail("Viewport capture requires a valid viewport, port, and 1–24 fps".into());
        }
        let Ok(ip) = bind_ip.to_string().parse::<std::net::IpAddr>() else {
            return self.fail("Invalid bind address".into());
        };
        let (send, receive) = tokio::sync::mpsc::channel(50);
        if !self.start(Mode::Viewport {
            bind: (ip, port as u16).into(),
            receive,
        }) {
            return false;
        }
        self.viewport = Some(viewport);
        self.capture_send = Some(send);
        self.capture_origin = std::time::Instant::now();
        self.next_capture = self.capture_origin;
        self.capture_period = std::time::Duration::from_secs_f64(1.0 / fps as f64);
        RenderingServer::singleton().connect(
            "frame_post_draw",
            &self.base().callable("capture_viewport_frame"),
        );
        true
    }

    /// Attach a bus to an active Viewport or audio-only sender.
    #[func]
    fn capture_audio_bus(&mut self, bus: StringName) -> bool {
        if self.capture_send.is_none() {
            return self.fail("Start a live sender before attaching audio".into());
        }
        self.audio_capture.take();
        match crate::audio_capture::Capture::bus(bus) {
            Ok(capture) => {
                self.audio_capture = Some(capture);
                true
            }
            Err(error) => self.fail(error),
        }
    }

    #[func]
    fn capture_microphone(&mut self) -> bool {
        if self.capture_send.is_none() {
            return self.fail("Start a live sender before attaching a microphone".into());
        }
        self.audio_capture.take();
        let result = crate::audio_capture::Capture::microphone(&mut self.base_mut());
        match result {
            Ok(capture) => {
                self.audio_capture = Some(capture);
                true
            }
            Err(error) => self.fail(error),
        }
    }

    #[func]
    fn stop_audio_capture(&mut self) {
        self.audio_capture.take();
    }

    #[func]
    fn start_audio_sender(&mut self, bind_ip: GString, port: i32) -> bool {
        self.stop();
        if !(1..=65535).contains(&port) {
            return self.fail("Invalid port".into());
        }
        let Ok(ip) = bind_ip.to_string().parse::<std::net::IpAddr>() else {
            return self.fail("Invalid bind address".into());
        };
        let (send, receive) = tokio::sync::mpsc::channel(50);
        if !self.start(Mode::Viewport {
            bind: (ip, port as u16).into(),
            receive,
        }) {
            return false;
        }
        self.capture_send = Some(send);
        true
    }

    fn clear_capture(&mut self) {
        let callable = self.base().callable("capture_viewport_frame");
        let mut renderer = RenderingServer::singleton();
        if renderer.is_connected("frame_post_draw", &callable) {
            renderer.disconnect("frame_post_draw", &callable);
        }
        self.audio_capture.take();
        self.capture_send.take();
        self.viewport = None;
    }
    #[signal]
    fn video_file_selected(path: GString);
    #[signal]
    fn video_file_dialog_closed(error: GString);

    /// iOS fallback until Godot implements native file dialogs on that platform.
    #[func]
    fn open_native_video_dialog(&mut self) -> bool {
        #[cfg(target_os = "ios")]
        {
            if self.picker_open {
                return false;
            }
            self.picker_open = unsafe { kg_ios_picker_open() };
            self.picker_open
        }
        #[cfg(not(target_os = "ios"))]
        {
            false
        }
    }
    #[func]
    fn get_media_capabilities(&self) -> VarDictionary {
        crate::media_capabilities::get()
    }
    pub(crate) fn latest_image(&self) -> Option<Gd<Image>> {
        #[cfg(windows)]
        if let Some(texture) = self.gpu.texture() {
            return texture.get_image();
        }
        self.latest_image.clone()
    }
    #[func]
    fn start_sender(&mut self, file: GString, bind_ip: GString, port: i32, repeat: bool) -> bool {
        self.stop();
        if !(1..=65535).contains(&port) {
            return self.fail("Port must be between 1 and 65535.".into());
        }
        let port = port as u16;
        let Ok(ip) = bind_ip.to_string().parse::<std::net::IpAddr>() else {
            return self.fail("Invalid listen address.".into());
        };
        self.start(Mode::Send {
            audio_enabled: self.audio_enabled,
            file: file.to_string(),
            bind: (ip, port).into(),
            repeat,
        })
    }

    #[func]
    pub(crate) fn start_receiver(&mut self, host: GString, port: i32, code: GString) -> bool {
        self.stop();
        if !(1..=65535).contains(&port) {
            return self.fail("Port must be between 1 and 65535.".into());
        }
        self.start(Mode::Receive {
            host: host.to_string(),
            port: port as u16,
            code: code.to_string(),
        })
    }

    #[func]
    pub(crate) fn stop(&mut self) {
        self.clear_capture();
        self.session.take();
        self.audio_output.take();
        self.frames = Arc::default();
        self.texture = None;
        #[cfg(windows)]
        {
            self.gpu = Default::default();
        }
        self.latest_image = None;
        self.message = "Stopped".into();
        self.code.clear();
        self.failed = false;
        self.count = 0;
    }

    #[func]
    pub(crate) fn get_status(&self) -> GString {
        self.message.as_str().into()
    }
    #[func]
    fn get_connection_code(&self) -> GString {
        self.code.as_str().into()
    }
    #[func]
    pub(crate) fn get_performance_statistics(&self) -> VarDictionary {
        self.frames.statistics()
    }
    #[func]
    fn get_connected_clients(&self) -> i64 {
        self.session.as_ref().map_or(0, |s| {
            s.status.lock().unwrap_or_else(|e| e.into_inner()).clients as i64
        })
    }
    #[func]
    fn get_skipped_client_frames(&self) -> i64 {
        self.session.as_ref().map_or(0, |s| {
            s.status.lock().unwrap_or_else(|e| e.into_inner()).skipped as i64
        })
    }
    #[func]
    pub(crate) fn get_dropped_frames(&self) -> i64 {
        self.frames.dropped() as i64
    }
    #[func]
    pub(crate) fn is_active(&self) -> bool {
        self.session.is_some()
    }
    #[func]
    pub(crate) fn has_failed(&self) -> bool {
        self.failed
    }
    #[func]
    pub(crate) fn get_frame_count(&self) -> i64 {
        self.count as i64
    }
    #[func]
    pub(crate) fn get_texture(&self) -> Option<Gd<Texture2D>> {
        #[cfg(windows)]
        if let Some(texture) = self.gpu.texture() {
            return Some(texture);
        }
        self.texture.clone().map(|t| t.upcast())
    }

    fn start(&mut self, mode: Mode) -> bool {
        #[cfg(windows)]
        if self.gpu_transfer && self.acceleration != 1 && matches!(mode, Mode::Receive { .. }) {
            crate::gpu_texture::request(self.frames.clone());
        }
        let transport = match crate::file_video::Transport::from_id(self.transport_mode) {
            Ok(value) => value,
            Err(error) => return self.fail(error.to_string()),
        };
        let acceleration = match self.acceleration {
            0 => kyber_media::Acceleration::Auto,
            1 => kyber_media::Acceleration::Software,
            2 => kyber_media::Acceleration::HardwareRequired,
            _ => return self.fail("Unknown acceleration mode".into()),
        };
        if !(0..=1).contains(&self.video_codec) {
            return self.fail("Unknown video codec".into());
        }
        match Session::start_with_video(
            mode,
            self.frames.clone(),
            transport,
            crate::file_video::VideoOptions {
                h264: self.video_codec == 1,
                acceleration,
            },
        ) {
            Ok(session) => {
                self.session = Some(session);
                self.message = "Preparing…".into();
                true
            }
            Err(error) => self.fail(error.to_string()),
        }
    }
    fn fail(&mut self, message: String) -> bool {
        self.message = message;
        self.failed = true;
        false
    }
}

#[cfg(target_os = "ios")]
unsafe extern "C" {
    fn kg_ios_picker_open() -> bool;
    fn kg_ios_picker_result() -> *const std::ffi::c_char;
}
