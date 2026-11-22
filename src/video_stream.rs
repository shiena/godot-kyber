// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! Standard Godot video resources for live Kyber reception.
use crate::{
    backend, file_video,
    frame::{Frame, Mailbox, frame_size},
    session::KyberSession,
};
use godot::{
    classes::{
        CanvasTexture, IVideoStream, IVideoStreamPlayback, Image, ImageTexture, Texture2D,
        VideoStream, VideoStreamPlayback,
    },
    prelude::*,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[derive(Default)]
struct PlaybackStatus {
    error: String,
    frames: i64,
}

#[derive(Clone, Default)]
struct Settings {
    audio_enabled: bool,
    source: i32,
    host: String,
    port: i32,
    code: String,
    username: String,
    password: String,
    fingerprint: String,
    display: i32,
    width: i32,
    height: i32,
    bitrate: i32,
}

#[derive(GodotClass)]
#[class(tool, base = VideoStream)]
struct KyberVideoStream {
    base: Base<VideoStream>,
    #[export(enum = (FileTransfer = 0, KyberHost = 1, LocalDemo = 2))]
    source: i32,
    #[export]
    audio_enabled: bool,
    #[export]
    host: GString,
    #[export(range = (1.0, 65535.0))]
    port: i32,
    #[export]
    username: GString,
    #[export]
    fingerprint: GString,
    #[export]
    display_id: i32,
    #[export(range = (1.0, 4096.0))]
    width: i32,
    #[export(range = (1.0, 4096.0))]
    height: i32,
    #[export]
    bitrate: i32,
    // Credentials are runtime-only and never serialized to .tres files.
    code: String,
    password: String,
    status: Rc<RefCell<PlaybackStatus>>,
    shared_session: Option<Gd<KyberSession>>,
}

#[godot_api]
impl IVideoStream for KyberVideoStream {
    fn init(base: Base<VideoStream>) -> Self {
        Self {
            base,
            source: 0,
            audio_enabled: true,
            host: "127.0.0.1".into(),
            port: 4433,
            username: GString::new(),
            fingerprint: GString::new(),
            display_id: -1,
            width: 1280,
            height: 720,
            bitrate: 20_000_000,
            code: String::new(),
            password: String::new(),
            status: Rc::default(),
            shared_session: None,
        }
    }
    fn instantiate_playback(&mut self) -> Option<Gd<VideoStreamPlayback>> {
        self.status = Rc::default();
        if self
            .shared_session
            .as_ref()
            .is_some_and(|s| !s.is_instance_valid())
        {
            self.status.borrow_mut().error = "Shared KyberSession was freed".into();
            return None;
        }
        let mut playback = KyberVideoStreamPlayback::new_gd();
        {
            let mut instance = playback.bind_mut();
            instance.settings = Settings {
                source: self.source,
                audio_enabled: self.audio_enabled,
                host: self.host.to_string(),
                port: self.port,
                code: self.code.clone(),
                username: self.username.to_string(),
                password: self.password.clone(),
                fingerprint: self.fingerprint.to_string(),
                display: self.display_id,
                width: self.width,
                height: self.height,
                bitrate: self.bitrate,
            };
            instance.status = self.status.clone();
            instance.shared_session = self.shared_session.clone();
        }
        Some(playback.upcast())
    }
}

#[godot_api]
impl KyberVideoStream {
    /// Runtime-only node reference; the scene owns the connection lifetime.
    #[func]
    fn set_session(&mut self, session: Option<Gd<KyberSession>>) {
        self.shared_session = session;
    }
    #[func]
    fn get_session(&self) -> Option<Gd<KyberSession>> {
        self.shared_session
            .as_ref()
            .filter(|s| s.is_instance_valid())
            .cloned()
    }
    #[constant]
    const FILE_TRANSFER: i32 = 0;
    #[constant]
    const KYBER_HOST: i32 = 1;
    #[constant]
    const LOCAL_DEMO: i32 = 2;
    #[func]
    fn set_connection_code(&mut self, code: GString) {
        self.code = code.to_string();
    }
    #[func]
    fn set_password(&mut self, password: GString) {
        self.password = password.to_string();
    }
    #[func]
    fn get_last_error(&self) -> GString {
        self.status.borrow().error.as_str().into()
    }
    #[func]
    fn get_frame_count(&self) -> i64 {
        self.status.borrow().frames
    }
}

enum Session {
    File(file_video::Session),
    Host(backend::Session),
    Demo,
}

#[derive(GodotClass)]
#[class(tool, base = VideoStreamPlayback)]
struct KyberVideoStreamPlayback {
    base: Base<VideoStreamPlayback>,
    settings: Settings,
    status: Rc<RefCell<PlaybackStatus>>,
    session: Option<Session>,
    frames: Arc<Mailbox>,
    texture: Gd<ImageTexture>,
    shared_texture: Option<Gd<CanvasTexture>>,
    shared_session: Option<Gd<KyberSession>>,
    last_serial: i64,
    playing: bool,
    paused: bool,
    position: f64,
    audio: Option<crate::audio::VideoOutput>,
    audio_owner: Option<Arc<Mailbox>>,
}

#[godot_api]
impl IVideoStreamPlayback for KyberVideoStreamPlayback {
    fn init(base: Base<VideoStreamPlayback>) -> Self {
        Self {
            base,
            settings: Settings::default(),
            status: Rc::default(),
            session: None,
            frames: Arc::default(),
            texture: ImageTexture::new_gd(),
            shared_texture: Some(CanvasTexture::new_gd()),
            shared_session: None,
            last_serial: -1,
            playing: false,
            paused: false,
            position: 0.0,
            audio: None,
            audio_owner: None,
        }
    }
    fn stop(&mut self) {
        if let Some(mailbox) = self.audio_owner.take() {
            mailbox
                .audio
                .claimed
                .store(false, std::sync::atomic::Ordering::Release);
        }
        self.session.take();
        self.audio = None;
        self.frames = Arc::default();
        self.playing = false;
        self.paused = false;
        self.position = 0.0;
    }
    fn play(&mut self) {
        self.stop();
        *self.status.borrow_mut() = PlaybackStatus::default();
        self.last_serial = -1;
        if let Some(session) = &self.shared_session {
            if session.is_instance_valid() {
                self.playing = session.bind().is_active();
                self.status.borrow_mut().error = session.bind().get_last_error().to_string();
            } else {
                self.status.borrow_mut().error = "Shared KyberSession was freed".into();
            }
            return;
        }
        match self.connect() {
            Ok(session) => {
                self.session = Some(session);
                self.playing = true;
            }
            Err(error) => self.status.borrow_mut().error = error,
        }
    }
    fn is_playing(&self) -> bool {
        self.playing
    }
    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if let Some(session) = &mut self.shared_session {
            session.bind_mut().pause_audio(paused);
        }
    }
    fn is_paused(&self) -> bool {
        self.paused
    }
    fn get_length(&self) -> f64 {
        0.0
    }
    fn get_playback_position(&self) -> f64 {
        self.position
    }
    fn seek(&mut self, _time: f64) {
        self.status.borrow_mut().error = "Live Kyber streams do not support seeking".into();
    }
    fn set_audio_track(&mut self, _idx: i32) {}
    fn get_texture(&self) -> Option<Gd<Texture2D>> {
        self.shared_texture
            .clone()
            .map(|texture| texture.upcast())
            .or_else(|| Some(self.texture.clone().upcast()))
    }
    fn get_channels(&self) -> i32 {
        2
    }
    fn get_mix_rate(&self) -> i32 {
        48000
    }
    fn update(&mut self, delta: f64) {
        if !self.playing || self.paused {
            return;
        }
        self.position += delta.max(0.0);
        let mailbox = self
            .shared_session
            .as_ref()
            .filter(|s| s.is_instance_valid())
            .and_then(|s| s.bind().audio_frames())
            .unwrap_or_else(|| self.frames.clone());
        if self.settings.audio_enabled
            && mailbox
                .audio
                .received
                .load(std::sync::atomic::Ordering::Relaxed)
                > 0
        {
            if self
                .audio_owner
                .as_ref()
                .is_some_and(|owned| !Arc::ptr_eq(owned, &mailbox))
            {
                if let Some(owned) = self.audio_owner.take() {
                    owned
                        .audio
                        .claimed
                        .store(false, std::sync::atomic::Ordering::Release);
                }
                self.audio = None;
            }
            if self.audio_owner.is_none() {
                if mailbox
                    .audio
                    .claimed
                    .swap(true, std::sync::atomic::Ordering::AcqRel)
                {
                    self.status.borrow_mut().error = "Audio session already has a consumer; disable audio_enabled for texture sharing".into();
                    self.playing = false;
                    return;
                }
                self.audio_owner = Some(mailbox.clone());
            }
            let mut audio = self.audio.take().unwrap_or_default();
            audio.pump(&mailbox.audio, delta, |samples| {
                self.base_mut()
                    .mix_audio_ex((samples.len() / 2) as i32)
                    .buffer(&PackedFloat32Array::from(samples))
                    .done()
                    .max(0) as usize
            });
            self.audio = Some(audio);
        }
        if let Some(session) = &self.shared_session {
            if !session.is_instance_valid() {
                self.status.borrow_mut().error = "Shared KyberSession was freed".into();
                self.playing = false;
                return;
            }
            let (serial, image, error, active) = {
                let session = session.bind();
                (
                    session.serial(),
                    session.get_texture(),
                    session.get_last_error().to_string(),
                    session.is_active(),
                )
            };
            if serial != self.last_serial
                && let Some(image) = image
            {
                if let Some(mut texture) = self.shared_texture.clone() {
                    let _base = self.base_mut();
                    texture.set_diffuse_texture(&image);
                    texture.emit_changed();
                } else if let Some(image) = image.get_image() {
                    self.update_texture(&image);
                }
                self.last_serial = serial;
                self.status.borrow_mut().frames += 1;
            }
            self.status.borrow_mut().error = error;
            self.playing = active;
            return;
        }
        let mut finished = false;
        match &self.session {
            Some(Session::Demo) => {
                let shift = (self.position * 60.0) as u32;
                let rgba = (0..180)
                    .flat_map(|y| {
                        (0..320).flat_map(move |x| {
                            [((x + shift) % 256) as u8, (y * 255 / 180) as u8, 160, 255]
                        })
                    })
                    .collect();
                self.frames.put(Frame {
                    #[cfg(windows)]
                    gpu: None,
                    width: 320,
                    height: 180,
                    rgba,
                });
            }
            Some(Session::File(session)) => {
                let state = session.status.lock().unwrap_or_else(|e| e.into_inner());
                finished = state.finished;
                if state.failed {
                    self.status.borrow_mut().error = state.message.clone();
                }
            }
            Some(Session::Host(session)) => {
                for event in session.events() {
                    match event.kind.as_str() {
                        "connected" => {
                            let display = if self.settings.display < 0 {
                                event.displays.first()
                            } else {
                                event
                                    .displays
                                    .iter()
                                    .find(|d| d.id == self.settings.display as u32)
                            };
                            let result = display
                                .ok_or_else(|| "Requested Kyber display is unavailable".to_string())
                                .and_then(|display| {
                                    session.start(
                                        display.id,
                                        self.settings.width as u32,
                                        self.settings.height as u32,
                                        self.settings.bitrate as u32,
                                        self.settings.audio_enabled,
                                    )
                                });
                            if let Err(error) = result {
                                self.status.borrow_mut().error = error;
                                finished = true;
                            }
                        }
                        "error" => {
                            self.status.borrow_mut().error = event.message;
                            finished = true;
                        }
                        "stopped" | "stream_stopped" => finished = true,
                        _ => {}
                    }
                }
            }
            None => finished = true,
        }
        if let Some(frame) = self.frames.take()
            && let Some(image) = Image::create_from_data(
                frame.width as i32,
                frame.height as i32,
                false,
                godot::classes::image::Format::RGBA8,
                &PackedByteArray::from(frame.rgba.as_slice()),
            )
        {
            self.update_texture(&image);
            self.status.borrow_mut().frames += 1;
        }
        if finished {
            self.session.take();
            self.playing = false;
        }
    }
}

impl KyberVideoStreamPlayback {
    fn update_texture(&mut self, image: &Gd<Image>) {
        let mut texture = self.texture.clone();
        let same_size =
            texture.get_width() == image.get_width() && texture.get_height() == image.get_height();
        let shared = self.shared_texture.clone();
        // Texture changes can synchronously query this playback through Godot.
        let _base = self.base_mut();
        if same_size {
            texture.update(image);
        } else {
            texture.set_image(image);
        }
        if let Some(mut shared) = shared {
            shared.set_diffuse_texture(&texture);
            shared.emit_changed();
        }
    }

    fn connect(&self) -> Result<Session, String> {
        let s = &self.settings;
        if s.source == 2 {
            return Ok(Session::Demo);
        }
        if !(1..=65535).contains(&s.port) {
            return Err("Port must be between 1 and 65535".into());
        }
        match s.source {
            0 => file_video::Session::start(
                file_video::Mode::Receive {
                    host: s.host.clone(),
                    port: s.port as u16,
                    code: s.code.clone(),
                },
                self.frames.clone(),
            )
            .map(Session::File)
            .map_err(|e| e.to_string()),
            1 => {
                if frame_size(s.width as u32, s.height as u32).is_none()
                    || s.bitrate <= 0
                    || s.display < -1
                {
                    return Err("Invalid stream dimensions, bitrate, or display ID".into());
                }
                backend::Session::connect(
                    backend::Config {
                        auto_reconnection: false,
                        clipboard_enabled: false,
                        host: s.host.clone(),
                        port: s.port as u16,
                        username: s.username.clone(),
                        password: s.password.clone(),
                        fingerprint: s.fingerprint.clone(),
                    },
                    self.frames.clone(),
                )
                .map(Session::Host)
            }
            _ => Err("Unknown Kyber stream source".into()),
        }
    }
}

impl Drop for KyberVideoStreamPlayback {
    fn drop(&mut self) {
        if let Some(mailbox) = self.audio_owner.take() {
            mailbox
                .audio
                .claimed
                .store(false, std::sync::atomic::Ordering::Release);
        }
    }
}
