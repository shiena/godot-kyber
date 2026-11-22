// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! Platform media services, independent of Godot and Kyber transport.
use std::fmt;
#[cfg(windows)]
pub mod gpu;

#[derive(Debug, Clone)]
pub struct MediaError(pub String);
impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for MediaError {}
pub type Result<T> = std::result::Result<T, MediaError>;

pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub pts_us: i64,
    pub rgba: Vec<u8>,
}

/// A complete compressed access unit. H.264 uses Annex B start codes.
pub struct EncodedPacket<'a> {
    pub data: &'a [u8],
    pub pts_us: i64,
    pub keyframe: bool,
}

/// Objects remain on their creating worker thread, including destruction.
pub trait VideoDecoder {
    fn decode(&mut self, packet: EncodedPacket<'_>) -> Result<Vec<VideoFrame>>;
    fn drain(&mut self) -> Result<Vec<VideoFrame>>;
    fn reset(&mut self) -> Result<()>;
    fn hardware_active(&self) -> bool {
        false
    }
}

/// Auto may fall back to software. HardwareRequired fails instead of falling back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum Acceleration {
    #[default]
    Auto = 0,
    Software = 1,
    HardwareRequired = 2,
}

pub struct EncoderSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    pub acceleration: Acceleration,
}
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub pts_us: i64,
    pub keyframe: bool,
}
pub trait VideoEncoder {
    fn encode(&mut self, frame: &VideoFrame) -> Result<Vec<EncodedFrame>>;
    fn drain(&mut self) -> Result<Vec<EncodedFrame>>;
    fn hardware_active(&self) -> bool;
}
pub fn h264_encoder(settings: &EncoderSettings) -> Result<Box<dyn VideoEncoder>> {
    if settings.width == 0
        || settings.height == 0
        || settings.width > 4096
        || settings.height > 4096
        || !settings.width.is_multiple_of(2)
        || !settings.height.is_multiple_of(2)
        || !(1..=120).contains(&settings.fps)
        || !(64_000..=100_000_000).contains(&settings.bitrate)
    {
        return Err(MediaError("Invalid H.264 encoder settings".into()));
    }
    #[cfg(windows)]
    {
        Ok(Box::new(windows::Encoder::new(settings)?))
    }
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android"
    ))]
    {
        Ok(Box::new(native_unix::Encoder::new(settings)?))
    }
    #[cfg(not(any(
        windows,
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android"
    )))]
    {
        Err(MediaError(
            "H.264 encoding is not implemented on this platform".into(),
        ))
    }
}
pub fn h264_decoder_with_acceleration(mode: Acceleration) -> Result<Box<dyn VideoDecoder>> {
    #[cfg(windows)]
    {
        Ok(Box::new(windows::Decoder::with_acceleration(mode)?))
    }
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android"
    ))]
    {
        Ok(Box::new(native_unix::Decoder::with_acceleration(mode)?))
    }
    #[cfg(not(any(
        windows,
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android"
    )))]
    {
        let _ = mode;
        h264_decoder()
    }
}

pub trait VideoSource {
    fn next_frame(&mut self) -> Result<Option<VideoFrame>>;
}

/// Interleaved stereo float PCM on the file's original microsecond timeline.
pub struct AudioFrame {
    pub pts_us: i64,
    pub samples: Vec<f32>,
}
pub trait AudioSource {
    fn next_frame(&mut self) -> Result<Option<AudioFrame>>;
}
pub fn open_audio(path: &std::path::Path) -> Result<Option<Box<dyn AudioSource>>> {
    #[cfg(windows)]
    {
        windows::AudioReader::open(path)
    }
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android"
    ))]
    {
        native_unix::AudioReader::open(path)
    }
    #[cfg(not(any(
        windows,
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android"
    )))]
    {
        let _ = path;
        Ok(None)
    }
}

/// Stateful packet handling shared by transport adapters and tests.
pub struct H264Stream {
    decoder: Box<dyn VideoDecoder>,
    configuration: Vec<u8>,
    configuration_pending: bool,
    waiting_for_keyframe: bool,
}
impl H264Stream {
    pub fn new() -> Result<Self> {
        Ok(Self {
            decoder: h264_decoder()?,
            configuration: Vec::new(),
            configuration_pending: false,
            waiting_for_keyframe: true,
        })
    }
    pub fn configure(&mut self, data: &[u8]) -> Result<()> {
        if !self.configuration_pending {
            self.configuration.clear();
        }
        if self.configuration.len().saturating_add(data.len()) > 1024 * 1024 {
            return Err(MediaError("H.264 configuration exceeds limit".into()));
        }
        self.configuration.extend_from_slice(data);
        self.configuration_pending = true;
        Ok(())
    }
    pub fn discontinuity(&mut self) -> Result<()> {
        self.decoder.reset()?;
        self.waiting_for_keyframe = true;
        Ok(())
    }
    pub fn decode(&mut self, packet: EncodedPacket<'_>) -> Result<Vec<VideoFrame>> {
        if packet.data.len() > 16 * 1024 * 1024 {
            return Err(MediaError("Video packet exceeds limit".into()));
        }
        if self.waiting_for_keyframe && !packet.keyframe {
            return Ok(Vec::new());
        }
        let mut bytes = Vec::new();
        if packet.keyframe {
            bytes.extend_from_slice(&self.configuration);
        }
        bytes.extend_from_slice(packet.data);
        self.configuration_pending = false;
        self.waiting_for_keyframe = false;
        self.decoder.decode(EncodedPacket {
            data: &bytes,
            ..packet
        })
    }
    pub fn drain(&mut self) -> Result<Vec<VideoFrame>> {
        self.decoder.drain()
    }
}

pub struct Capabilities {
    pub backend: &'static str,
    pub h264_decode: bool,
    pub file_video: bool,
    pub reason: String,
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
))]
mod native_unix;
#[cfg(windows)]
mod windows;

pub fn capabilities() -> Capabilities {
    #[cfg(windows)]
    {
        match windows::Decoder::new() {
            Ok(_) => Capabilities {
                backend: "media_foundation",
                h264_decode: true,
                file_video: true,
                reason: String::new(),
            },
            Err(error) => Capabilities {
                backend: "media_foundation",
                h264_decode: false,
                file_video: windows::available(),
                reason: error.to_string(),
            },
        }
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    {
        native_unix::capabilities()
    }
    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    {
        Capabilities {
            backend: "unavailable",
            h264_decode: false,
            file_video: false,
            reason: "No native media backend for this platform yet".into(),
        }
    }
}

pub fn h264_decoder() -> Result<Box<dyn VideoDecoder>> {
    #[cfg(windows)]
    {
        Ok(Box::new(windows::Decoder::new()?))
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    {
        Ok(Box::new(native_unix::Decoder::new()?))
    }
    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    {
        Err(MediaError(capabilities().reason))
    }
}

pub fn open_video(path: &std::path::Path) -> Result<Box<dyn VideoSource>> {
    #[cfg(windows)]
    {
        Ok(Box::new(windows::Source::new(path)?))
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    {
        Ok(Box::new(native_unix::Source::new(path)?))
    }
    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    {
        let _ = path;
        Err(MediaError(capabilities().reason))
    }
}
