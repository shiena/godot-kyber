// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! Godot AudioStream adapter for live Kyber audio, including spatial players.
use crate::{
    audio::PullOutput,
    file_video::{Mode, Session},
    frame::Mailbox,
    session::KyberSession,
};
use godot::{
    classes::{
        AudioServer, AudioStream, AudioStreamPlayback, AudioStreamPlaybackResampled, IAudioStream,
        IAudioStreamPlaybackResampled, native::AudioFrame,
    },
    prelude::*,
};
use godot_core::meta::RawPtr;
use std::sync::{Arc, Mutex, atomic::Ordering};

#[derive(GodotClass)]
#[class(tool, base=AudioStream)]
struct KyberAudioStream {
    base: Base<AudioStream>,
    #[export]
    host: GString,
    #[export(range=(1.0,65535.0))]
    port: i32,
    code: String,
    shared: Option<Arc<Mailbox>>,
    error: Arc<Mutex<String>>,
}
#[godot_api]
impl IAudioStream for KyberAudioStream {
    fn init(base: Base<AudioStream>) -> Self {
        Self {
            base,
            host: "127.0.0.1".into(),
            port: 4433,
            code: String::new(),
            shared: None,
            error: Arc::default(),
        }
    }
    fn instantiate_playback(&self) -> Option<Gd<AudioStreamPlayback>> {
        let mut playback = KyberAudioStreamPlayback::new_gd();
        {
            let mut p = playback.bind_mut();
            p.host = self.host.to_string();
            p.port = self.port;
            p.code = self.code.clone();
            p.shared = self.shared.is_some();
            p.frames = self.shared.clone().unwrap_or_default();
            p.error = self.error.clone();
            p.latency =
                (AudioServer::singleton().get_output_latency().max(0.0) * 1_000_000.0) as u64;
        }
        Some(playback.upcast())
    }
    fn get_stream_name(&self) -> GString {
        "Kyber live audio".into()
    }
    fn get_length(&self) -> f64 {
        0.0
    }
    fn is_monophonic(&self) -> bool {
        true
    }
}
#[godot_api]
impl KyberAudioStream {
    #[func]
    fn set_connection_code(&mut self, code: GString) {
        self.code = code.to_string();
    }
    /// Call after starting the scene-owned receiver; no Godot nodes enter the mixer.
    #[func]
    fn set_session(&mut self, session: Option<Gd<KyberSession>>) {
        self.shared = session
            .filter(|s| s.is_instance_valid())
            .and_then(|s| s.bind().audio_frames());
    }
    #[func]
    fn get_last_error(&self) -> GString {
        self.error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_str()
            .into()
    }
}
#[derive(GodotClass)]
#[class(base=AudioStreamPlaybackResampled)]
struct KyberAudioStreamPlayback {
    base: Base<AudioStreamPlaybackResampled>,
    host: String,
    port: i32,
    code: String,
    frames: Arc<Mailbox>,
    shared: bool,
    session: Option<Session>,
    output: Option<PullOutput>,
    playing: bool,
    claimed: bool,
    claim_wait: usize,
    position: u64,
    latency: u64,
    error: Arc<Mutex<String>>,
}
#[godot_api]
impl IAudioStreamPlaybackResampled for KyberAudioStreamPlayback {
    fn init(base: Base<AudioStreamPlaybackResampled>) -> Self {
        Self {
            base,
            host: String::new(),
            port: 4433,
            code: String::new(),
            frames: Arc::default(),
            shared: false,
            session: None,
            output: None,
            playing: false,
            claimed: false,
            claim_wait: 0,
            position: 0,
            latency: 0,
            error: Arc::default(),
        }
    }
    fn start(&mut self, _from_pos: f64) {
        self.shutdown();
        self.error.lock().unwrap_or_else(|e| e.into_inner()).clear();
        if !self.shared {
            self.frames = Arc::default();
        }
        self.claim_wait = 0;
        self.claimed = !self.frames.audio.claimed.swap(true, Ordering::AcqRel);
        self.playing = true;
        if !self.shared {
            let result = if (1..=65535).contains(&self.port) {
                Session::start(
                    Mode::Receive {
                        host: self.host.clone(),
                        port: self.port as u16,
                        code: self.code.clone(),
                    },
                    self.frames.clone(),
                )
            } else {
                Err("Invalid audio port".into())
            };
            match result {
                Ok(session) => self.session = Some(session),
                Err(error) => {
                    *self.error.lock().unwrap_or_else(|e| e.into_inner()) = error.to_string();
                    self.shutdown();
                    return;
                }
            }
        }
        self.position = 0;
        self.output = Some(PullOutput::default());
        self.base_mut().begin_resample();
    }
    fn stop(&mut self) {
        self.shutdown();
    }
    fn is_playing(&self) -> bool {
        self.playing
    }
    fn get_loop_count(&self) -> i32 {
        0
    }
    fn get_playback_position(&self) -> f64 {
        self.position as f64 / 48000.0
    }
    fn seek(&mut self, _position: f64) {}
    fn get_stream_sampling_rate(&self) -> f32 {
        48000.0
    }
    unsafe fn mix_resampled_rawptr(
        &mut self,
        dst_buffer: RawPtr<*mut AudioFrame>,
        frame_count: i32,
    ) -> i32 {
        if frame_count <= 0 || dst_buffer.ptr().is_null() {
            return 0;
        }
        let output =
            unsafe { std::slice::from_raw_parts_mut(dst_buffer.ptr(), frame_count as usize) };
        for f in output.iter_mut() {
            f.left = 0.0;
            f.right = 0.0;
        }
        if !self.playing {
            return 0;
        }
        if !self.claimed {
            self.claimed = !self.frames.audio.claimed.swap(true, Ordering::AcqRel);
            if !self.claimed {
                // Godot finishes a stopped player's fade on a later mixer block.
                // Allow that owner to release before rejecting a new play call.
                self.claim_wait += frame_count as usize;
                if self.claim_wait >= 4800 {
                    *self.error.lock().unwrap_or_else(|e| e.into_inner()) =
                        "Audio session already has a consumer".into();
                    self.playing = false;
                    return 0;
                }
                return frame_count;
            }
        }
        if let Some(session) = &self.session {
            let state = session.status.lock().unwrap_or_else(|e| e.into_inner());
            if state.failed {
                *self.error.lock().unwrap_or_else(|e| e.into_inner()) = state.message.clone();
                self.frames.audio.claimed.store(false, Ordering::Release);
                self.claimed = false;
                self.playing = false;
                return 0;
            }
        }
        if self.frames.closed.load(Ordering::Acquire)
            && self
                .output
                .as_ref()
                .is_none_or(|o| o.drained(&self.frames.audio))
        {
            self.frames.audio.claimed.store(false, Ordering::Release);
            self.claimed = false;
            self.playing = false;
            return 0;
        }
        if let Some(mixer) = &mut self.output {
            mixer.mix(&self.frames.audio, output, self.latency);
        }
        self.position += frame_count as u64;
        frame_count
    }
}
impl KyberAudioStreamPlayback {
    fn shutdown(&mut self) {
        if self.claimed {
            self.frames.audio.claimed.store(false, Ordering::Release);
            self.claimed = false;
        }
        self.playing = false;
        self.session.take();
        self.output.take();
    }
}
impl Drop for KyberAudioStreamPlayback {
    fn drop(&mut self) {
        self.shutdown();
    }
}
