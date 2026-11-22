// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

use super::*;
use std::{ffi::c_void, marker::PhantomData, os::windows::ffi::OsStrExt, rc::Rc};

type Callback = unsafe extern "C" fn(*mut c_void, *const u8, usize, u32, u32, i64);
unsafe extern "C" {
    fn km_available() -> i32;
    fn km_decoder_create(result: *mut *mut c_void) -> i32;
    fn km_decoder_create_mode(mode: i32, result: *mut *mut c_void) -> i32;
    fn km_decoder_hardware(decoder: *mut c_void) -> bool;
    fn km_decoder_destroy(decoder: *mut c_void);
    fn km_decoder_push(
        decoder: *mut c_void,
        data: *const u8,
        len: usize,
        pts: i64,
        key: bool,
        callback: Callback,
        user: *mut c_void,
    ) -> i32;
    fn km_decoder_drain(decoder: *mut c_void, callback: Callback, user: *mut c_void) -> i32;
    fn km_decoder_reset(decoder: *mut c_void) -> i32;
    fn km_source_create(path: *const u16, result: *mut *mut c_void) -> i32;
    fn km_source_next(source: *mut c_void, callback: Callback, user: *mut c_void) -> i32;
    fn km_source_destroy(source: *mut c_void);
}
fn check(code: i32) -> Result<()> {
    if code < 0 {
        Err(MediaError(format!(
            "Media Foundation error 0x{:08X}",
            code as u32
        )))
    } else {
        Ok(())
    }
}
unsafe extern "C" fn collect(
    user: *mut c_void,
    data: *const u8,
    len: usize,
    width: u32,
    height: u32,
    pts_us: i64,
) {
    // The native backend owns this buffer for the duration of the callback.
    let frames = unsafe { &mut *user.cast::<Vec<VideoFrame>>() };
    let rgba = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
    frames.push(VideoFrame {
        width,
        height,
        pts_us,
        rgba,
    });
}
pub fn available() -> bool {
    unsafe { km_available() >= 0 }
}

type PacketCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize, i64, bool);
unsafe extern "C" {
    fn km_encoder_create(
        w: u32,
        h: u32,
        fps: u32,
        bitrate: u32,
        mode: i32,
        result: *mut *mut c_void,
    ) -> i32;
    fn km_encoder_destroy(handle: *mut c_void);
    fn km_encoder_hardware(handle: *mut c_void) -> bool;
    fn km_encoder_push(
        handle: *mut c_void,
        rgba: *const u8,
        len: usize,
        pts: i64,
        callback: PacketCallback,
        user: *mut c_void,
    ) -> i32;
    fn km_encoder_drain(handle: *mut c_void, callback: PacketCallback, user: *mut c_void) -> i32;
}
unsafe extern "C" fn collect_packet(
    user: *mut c_void,
    data: *const u8,
    len: usize,
    pts_us: i64,
    keyframe: bool,
) {
    let packets = unsafe { &mut *user.cast::<Vec<EncodedFrame>>() };
    packets.push(EncodedFrame {
        data: unsafe { std::slice::from_raw_parts(data, len) }.to_vec(),
        pts_us,
        keyframe,
    });
}
pub struct Encoder {
    handle: *mut c_void,
    width: u32,
    height: u32,
    _thread: PhantomData<Rc<()>>,
}
impl Encoder {
    pub fn new(settings: &EncoderSettings) -> Result<Self> {
        let mut handle = std::ptr::null_mut();
        check(unsafe {
            km_encoder_create(
                settings.width,
                settings.height,
                settings.fps,
                settings.bitrate,
                settings.acceleration as i32,
                &mut handle,
            )
        })?;
        Ok(Self {
            handle,
            width: settings.width,
            height: settings.height,
            _thread: PhantomData,
        })
    }
}
impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { km_encoder_destroy(self.handle) };
    }
}
impl VideoEncoder for Encoder {
    fn encode(&mut self, frame: &VideoFrame) -> Result<Vec<EncodedFrame>> {
        if frame.width != self.width || frame.height != self.height {
            return Err(MediaError("H.264 frame dimensions changed".into()));
        }
        let mut result = Vec::<EncodedFrame>::new();
        check(unsafe {
            km_encoder_push(
                self.handle,
                frame.rgba.as_ptr(),
                frame.rgba.len(),
                frame.pts_us,
                collect_packet,
                (&mut result as *mut Vec<EncodedFrame>).cast(),
            )
        })?;
        Ok(result)
    }
    fn drain(&mut self) -> Result<Vec<EncodedFrame>> {
        let mut result = Vec::<EncodedFrame>::new();
        check(unsafe {
            km_encoder_drain(
                self.handle,
                collect_packet,
                (&mut result as *mut Vec<EncodedFrame>).cast(),
            )
        })?;
        Ok(result)
    }
    fn hardware_active(&self) -> bool {
        unsafe { km_encoder_hardware(self.handle) }
    }
}

pub struct Decoder {
    handle: *mut c_void,
    automatic: bool,
    delivered: bool,
    _thread: PhantomData<Rc<()>>,
}
impl Decoder {
    pub fn with_acceleration(mode: Acceleration) -> Result<Self> {
        let mut handle = std::ptr::null_mut();
        let result = check(unsafe { km_decoder_create_mode(mode as i32, &mut handle) });
        if result.is_err() && mode == Acceleration::Auto {
            return Self::with_acceleration(Acceleration::Software);
        }
        result?;
        Ok(Self {
            handle,
            automatic: mode == Acceleration::Auto,
            delivered: false,
            _thread: PhantomData,
        })
    }
    pub fn new() -> Result<Self> {
        let mut handle = std::ptr::null_mut();
        check(unsafe { km_decoder_create(&mut handle) })?;
        Ok(Self {
            handle,
            automatic: false,
            delivered: false,
            _thread: PhantomData,
        })
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe { km_decoder_destroy(self.handle) };
    }
}
impl VideoDecoder for Decoder {
    fn hardware_active(&self) -> bool {
        unsafe { km_decoder_hardware(self.handle) }
    }
    fn decode(&mut self, packet: EncodedPacket<'_>) -> Result<Vec<VideoFrame>> {
        let mut frames = Vec::new();
        let result = check(unsafe {
            km_decoder_push(
                self.handle,
                packet.data.as_ptr(),
                packet.data.len(),
                packet.pts_us,
                packet.keyframe,
                collect,
                (&mut frames as *mut Vec<VideoFrame>).cast(),
            )
        });
        if result.is_err() && self.automatic && !self.delivered {
            *self = Self::with_acceleration(Acceleration::Software)?;
            return self.decode(packet);
        }
        result?;
        self.delivered |= !frames.is_empty();
        Ok(frames)
    }
    fn drain(&mut self) -> Result<Vec<VideoFrame>> {
        let mut frames = Vec::new();
        check(unsafe {
            km_decoder_drain(
                self.handle,
                collect,
                (&mut frames as *mut Vec<VideoFrame>).cast(),
            )
        })?;
        Ok(frames)
    }
    fn reset(&mut self) -> Result<()> {
        check(unsafe { km_decoder_reset(self.handle) })
    }
}
pub struct Source {
    handle: *mut c_void,
    _thread: PhantomData<Rc<()>>,
}
impl Source {
    pub fn new(path: &std::path::Path) -> Result<Self> {
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err(MediaError("Video path contains NUL".into()));
        }
        wide.push(0);
        let mut handle = std::ptr::null_mut();
        check(unsafe { km_source_create(wide.as_ptr(), &mut handle) })?;
        Ok(Self {
            handle,
            _thread: PhantomData,
        })
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        unsafe { km_source_destroy(self.handle) };
    }
}
impl VideoSource for Source {
    fn next_frame(&mut self) -> Result<Option<VideoFrame>> {
        let mut frames = Vec::new();
        check(unsafe {
            km_source_next(
                self.handle,
                collect,
                (&mut frames as *mut Vec<VideoFrame>).cast(),
            )
        })?;
        Ok(frames.pop())
    }
}

unsafe extern "C" {
    fn km_audio_create(path: *const u16, result: *mut *mut c_void) -> i32;
    fn km_audio_next(source: *mut c_void, callback: Callback, user: *mut c_void) -> i32;
    fn km_audio_destroy(source: *mut c_void);
}
pub struct AudioReader {
    handle: *mut c_void,
    _thread: PhantomData<Rc<()>>,
}
impl AudioReader {
    pub fn open(path: &std::path::Path) -> Result<Option<Box<dyn AudioSource>>> {
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err(MediaError("Audio path contains NUL".into()));
        }
        wide.push(0);
        let mut handle = std::ptr::null_mut();
        check(unsafe { km_audio_create(wide.as_ptr(), &mut handle) })?;
        Ok(if handle.is_null() {
            None
        } else {
            Some(Box::new(Self {
                handle,
                _thread: PhantomData,
            }))
        })
    }
}
impl Drop for AudioReader {
    fn drop(&mut self) {
        unsafe { km_audio_destroy(self.handle) };
    }
}
impl AudioSource for AudioReader {
    fn next_frame(&mut self) -> Result<Option<AudioFrame>> {
        unsafe extern "C" fn collect_audio(
            user: *mut c_void,
            data: *const u8,
            len: usize,
            _rate: u32,
            _channels: u32,
            pts_us: i64,
        ) {
            let result = unsafe { &mut *user.cast::<Option<AudioFrame>>() };
            let bytes = unsafe { std::slice::from_raw_parts(data, len) };
            *result = Some(AudioFrame {
                pts_us,
                samples: bytes
                    .chunks_exact(4)
                    .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                    .collect(),
            });
        }
        let mut result: Option<AudioFrame> = None;
        check(unsafe {
            km_audio_next(
                self.handle,
                collect_audio,
                (&mut result as *mut Option<AudioFrame>).cast(),
            )
        })?;
        Ok(result)
    }
}
