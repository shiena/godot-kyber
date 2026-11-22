// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! Kyber audio transport and Godot playback. Each regulator stays with its consumer.
use godot::{
    classes::{
        AudioServer, AudioStreamGenerator, AudioStreamGeneratorPlayback, AudioStreamPlayer, Node,
    },
    prelude::*,
};
use kyaudioreg::{AudioRegulatorPull, AudioRegulatorPush, memory::AudioRegulatorSharedMemory};
use kymux_types::{AVPacket, CodecPacket, CodecPacketHeader, MediaPacket, MediaPacketHeader};
use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const RATE: u32 = 48000;
const FRAME: usize = 960;
const OPUS: u32 = u32::from_be_bytes(*b"opus");
const TARGET: u32 = 3840;
const OUTPUT_TARGET: u32 = 1920;
const CAPACITY: u32 = 48000;

pub fn codec() -> AVPacket {
    AVPacket::Codec(CodecPacket {
        header: CodecPacketHeader {
            codec: OPUS,
            rotation: 0,
            frame_size: FRAME as u16,
        },
    })
}
pub fn configuration() -> AVPacket {
    AVPacket::Media(MediaPacket {
        header: MediaPacketHeader {
            is_config: true,
            is_key: false,
            pts: 0,
            size: 0,
        },
        payload: Vec::new().into(),
    })
}
pub struct Encoder {
    encoder: opus::Encoder,
    pending: VecDeque<f32>,
    pts: Option<i64>,
    delay: i64,
    input_end: Option<i64>,
}
impl Encoder {
    pub fn new() -> Result<Self> {
        let mut encoder =
            opus::Encoder::new(RATE, opus::Channels::Stereo, opus::Application::Audio)?;
        encoder.set_bitrate(opus::Bitrate::Bits(128000))?;
        let delay = encoder.get_lookahead()? as i64 * 1_000_000 / RATE as i64;
        Ok(Self {
            encoder,
            pending: VecDeque::new(),
            pts: None,
            delay,
            input_end: None,
        })
    }
    pub fn flush(&mut self) -> Result<Vec<AVPacket>> {
        if self.pts.is_none() {
            return Ok(Vec::new());
        }
        let delayed = self.delay as usize * RATE as usize / 1_000_000 * 2;
        let size =
            (self.pending.len() + delayed).div_ceil(FRAME * 2) * FRAME * 2 - self.pending.len();
        self.encode(kyber_media::AudioFrame {
            pts_us: self.input_end.unwrap_or(0),
            samples: vec![0.0; size],
        })
    }
    pub fn encode(&mut self, frame: kyber_media::AudioFrame) -> Result<Vec<AVPacket>> {
        if !frame.samples.len().is_multiple_of(2) || frame.samples.iter().any(|v| !v.is_finite()) {
            return Err("Invalid stereo PCM".into());
        }
        let discontinuity = self
            .input_end
            .is_some_and(|end| frame.pts_us.abs_diff(end) > 1000);
        if discontinuity {
            self.pending.clear();
            self.pts = None;
        }
        self.input_end =
            Some(frame.pts_us + (frame.samples.len() / 2) as i64 * 1_000_000 / RATE as i64);
        self.pts.get_or_insert(frame.pts_us - self.delay);
        self.pending.extend(frame.samples);
        let mut packets = Vec::new();
        while self.pending.len() >= FRAME * 2 {
            let pcm: Vec<f32> = self.pending.drain(..FRAME * 2).collect();
            let mut payload = vec![0; 4000];
            let size = self.encoder.encode_float(&pcm, &mut payload)?;
            payload.truncate(size);
            let pts = self.pts.as_mut().unwrap();
            packets.push(AVPacket::Media(MediaPacket {
                header: MediaPacketHeader {
                    is_config: false,
                    is_key: true,
                    pts: (*pts).max(0) as u64,
                    size: size as u32,
                },
                payload: payload.into(),
            }));
            *pts += 20_000;
        }
        Ok(packets)
    }
}
#[derive(Default)]
pub struct AudioMailbox {
    pub claimed: AtomicBool,
    queue: Mutex<VecDeque<(u64, Vec<f32>)>>,
    pub position: AtomicU64,
    pub received: AtomicU64,
    pub dropped: AtomicU64,
}
impl AudioMailbox {
    pub fn push(&self, pts: u64, pcm: Vec<f32>) {
        self.received
            .fetch_add((pcm.len() / 2) as u64, Ordering::Relaxed);
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        if queue.len() >= 50 {
            queue.pop_front();
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        queue.push_back((pts, pcm));
    }
}
pub async fn receive(
    input: Option<kymux_types::AudioClientProtocol>,
    mailbox: &AudioMailbox,
) -> Result<()> {
    let Some(mut input) = input else {
        return std::future::pending().await;
    };
    let mut decoder = PacketDecoder::new()?;
    while let Some(packet) = input.recv().await? {
        decoder.push(packet, mailbox)?;
    }
    std::future::pending().await
}

/// Shared Opus decoder for file transports and native SDK audio callbacks.
pub(crate) struct PacketDecoder {
    decoder: opus::Decoder,
    valid: bool,
    frame_size: usize,
    next_pts: Option<u64>,
    skip: usize,
    gain: f32,
}
impl PacketDecoder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            decoder: opus::Decoder::new(RATE, opus::Channels::Stereo)?,
            valid: false,
            frame_size: FRAME,
            next_pts: None,
            skip: 0,
            gain: 1.0,
        })
    }
    pub fn push(&mut self, packet: AVPacket, mailbox: &AudioMailbox) -> Result<()> {
        match packet {
            AVPacket::Codec(c)
                if c.header.codec == OPUS
                    && matches!(
                        c.header.frame_size,
                        0 | 120 | 240 | 480 | 960 | 1920 | 2880 | 5760
                    ) =>
            {
                self.decoder.reset_state()?;
                self.valid = true;
                self.frame_size = if c.header.frame_size == 0 {
                    FRAME
                } else {
                    c.header.frame_size as usize
                };
                self.next_pts = None;
                self.skip = 0;
                self.gain = 1.0;
            }
            AVPacket::Media(p) if self.valid && p.header.is_config => {
                if !p.payload.is_empty() {
                    let h = &p.payload;
                    if h.len() < 19
                        || h.len() > 256
                        || &h[..8] != b"OpusHead"
                        || h[8] > 15
                        || !matches!(h[9], 1 | 2)
                        || h[18] != 0
                    {
                        return Err(
                            "Unsupported Opus configuration (only mono/stereo mapping family 0)"
                                .into(),
                        );
                    }
                    self.skip = u16::from_le_bytes([h[10], h[11]]) as usize;
                    self.gain =
                        10.0_f32.powf(i16::from_le_bytes([h[16], h[17]]) as f32 / (256.0 * 20.0));
                }
                self.decoder.reset_state()?;
                self.next_pts = None;
            }
            AVPacket::Media(p)
                if self.valid
                    && !p.header.is_config
                    && !p.payload.is_empty()
                    && p.payload.len() <= 4000 =>
            {
                let mut pcm = vec![0.0; 5760 * 2];
                let count = self.decoder.decode_float(&p.payload, &mut pcm, false)?;
                pcm.truncate(count * 2);
                self.frame_size = count;
                self.next_pts = Some(
                    p.header
                        .pts
                        .saturating_add(count as u64 * 1_000_000 / u64::from(RATE)),
                );
                let skip = self.skip.min(count);
                self.skip -= skip;
                pcm.drain(..skip * 2);
                for sample in &mut pcm {
                    *sample *= self.gain;
                }
                if !pcm.is_empty() {
                    mailbox.push(
                        p.header
                            .pts
                            .saturating_add(skip as u64 * 1_000_000 / u64::from(RATE)),
                        pcm,
                    );
                }
            }
            AVPacket::Hole(hole) if self.valid => {
                let missing = (hole.header.missing_audio_samples as usize).min(RATE as usize);
                mailbox
                    .dropped
                    .fetch_add(missing.div_ceil(self.frame_size) as u64, Ordering::Relaxed);
                if let Some(pts) = &mut self.next_pts {
                    for count in (0..missing)
                        .step_by(self.frame_size)
                        .map(|i| (missing - i).min(self.frame_size))
                    {
                        // Opus PLC accepts integral 2.5 ms blocks.
                        let count = count.div_ceil(120) * 120;
                        let mut pcm = vec![0.0; count * 2];
                        self.decoder.decode_float(&[], &mut pcm, false)?;
                        for sample in &mut pcm {
                            *sample *= self.gain;
                        }
                        mailbox.push(*pts, pcm);
                        *pts = pts.saturating_add(count as u64 * 1_000_000 / u64::from(RATE));
                    }
                }
            }
            _ => {
                self.valid = false;
                return Err("Unsupported audio packet".into());
            }
        }
        Ok(())
    }
}

/// Pull adapter for Godot's audio mixing callback. It never calls scene APIs.
pub struct PullOutput {
    memory: AudioRegulatorSharedMemory,
    push: AudioRegulatorPush,
    pull: AudioRegulatorPull,
    end_pts: u64,
}
impl Default for PullOutput {
    fn default() -> Self {
        let memory = AudioRegulatorSharedMemory::native(CAPACITY, 8);
        Self {
            push: AudioRegulatorPush::new(memory.clone(), RATE, TARGET),
            pull: AudioRegulatorPull::new(memory.clone(), TARGET),
            memory,
            end_pts: 0,
        }
    }
}
impl PullOutput {
    pub fn drained(&self, mailbox: &AudioMailbox) -> bool {
        self.memory.get_buffer_head(Ordering::Relaxed)
            == self.memory.get_buffer_tail(Ordering::Relaxed)
            && mailbox
                .queue
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_empty()
    }
    pub fn mix(
        &mut self,
        mailbox: &AudioMailbox,
        output: &mut [godot::classes::native::AudioFrame],
        latency: u64,
    ) {
        let packets: Vec<_> = mailbox
            .queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        for (pts, pcm) in packets {
            let missing = if self.end_pts == 0 {
                0
            } else {
                (pts.saturating_sub(self.end_pts) * u64::from(RATE) / 1_000_000)
                    .min(u64::from(RATE)) as u32
            };
            self.end_pts = pts + (pcm.len() / 2) as u64 * 1_000_000 / u64::from(RATE);
            let bytes: Vec<u8> = pcm.iter().flat_map(|v| v.to_ne_bytes()).collect();
            self.push.push(missing, &bytes);
        }
        let buffered = (CAPACITY + self.memory.get_buffer_head(Ordering::Relaxed)
            - self.memory.get_buffer_tail(Ordering::Relaxed))
            % CAPACITY;
        if self.memory.get_played(Ordering::Relaxed) && output.len() > buffered as usize {
            self.end_pts += (output.len() - buffered as usize) as u64 * 1_000_000 / u64::from(RATE);
        }
        let mut bytes = vec![0; output.len() * 8];
        self.pull.pull(&mut bytes);
        for (frame, bytes) in output.iter_mut().zip(bytes.chunks_exact(8)) {
            frame.left = f32::from_ne_bytes(bytes[..4].try_into().unwrap());
            frame.right = f32::from_ne_bytes(bytes[4..].try_into().unwrap());
        }
        if self.memory.get_played(Ordering::Relaxed) {
            let buffered = (CAPACITY + self.memory.get_buffer_head(Ordering::Relaxed)
                - self.memory.get_buffer_tail(Ordering::Relaxed))
                % CAPACITY;
            let delay =
                (u64::from(buffered) + output.len() as u64) * 1_000_000 / u64::from(RATE) + latency;
            mailbox
                .position
                .store(self.end_pts.saturating_sub(delay), Ordering::Relaxed);
        }
    }
}

pub struct Output {
    player: Gd<AudioStreamPlayer>,
    playback: Gd<AudioStreamGeneratorPlayback>,
    memory: AudioRegulatorSharedMemory,
    push: AudioRegulatorPush,
    pull: AudioRegulatorPull,
    capacity: i32,
    end_pts: u64,
    paused: bool,
}
impl Output {
    pub fn new(parent: &mut Gd<Node>) -> Self {
        let mut generator = AudioStreamGenerator::new_gd();
        generator.set_mix_rate(RATE as f32);
        generator.set_buffer_length(0.08);
        let mut player = AudioStreamPlayer::new_alloc();
        player.set_stream(&generator);
        parent.add_child(&player);
        player.play();
        let playback = player
            .get_stream_playback()
            .unwrap()
            .cast::<AudioStreamGeneratorPlayback>();
        let capacity = playback.get_frames_available();
        let memory = AudioRegulatorSharedMemory::native(CAPACITY, 8);
        Self {
            player,
            playback,
            push: AudioRegulatorPush::new(memory.clone(), RATE, TARGET),
            pull: AudioRegulatorPull::new(memory.clone(), TARGET),
            memory,
            capacity,
            end_pts: 0,
            paused: false,
        }
    }
    pub fn pause(&mut self, paused: bool) {
        self.paused = paused;
        self.player.set_stream_paused(paused);
    }
    pub fn pump(&mut self, mailbox: &AudioMailbox) {
        let packets: Vec<_> = mailbox
            .queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        for (pts, pcm) in packets {
            let missing = if self.end_pts != 0 && pts > self.end_pts {
                ((pts - self.end_pts) * RATE as u64 / 1_000_000).min(RATE as u64) as u32
            } else {
                0
            };
            self.end_pts = pts + (pcm.len() / 2) as u64 * 1_000_000 / RATE as u64;
            let bytes: Vec<u8> = pcm.iter().flat_map(|v| v.to_ne_bytes()).collect();
            self.push.push(missing, &bytes);
        }
        if self.paused || self.end_pts == 0 {
            return;
        }
        let available = self.playback.get_frames_available();
        let queued = self.capacity - available;
        // Keep the Godot queue short; Kyber owns jitter and drift regulation.
        let wanted = (OUTPUT_TARGET as i32 - queued).max(0).min(available) as usize;
        if wanted > 0 {
            let mut bytes = vec![0; wanted * 8];
            let buffered = (CAPACITY + self.memory.get_buffer_head(Ordering::Relaxed)
                - self.memory.get_buffer_tail(Ordering::Relaxed))
                % CAPACITY;
            if self.memory.get_played(Ordering::Relaxed) && wanted > buffered as usize {
                self.end_pts += (wanted - buffered as usize) as u64 * 1_000_000 / RATE as u64;
            }
            self.pull.pull(&mut bytes);
            let samples: Vec<Vector2> = bytes
                .chunks_exact(8)
                .map(|b| {
                    Vector2::new(
                        f32::from_ne_bytes(b[..4].try_into().unwrap()),
                        f32::from_ne_bytes(b[4..].try_into().unwrap()),
                    )
                })
                .collect();
            self.playback
                .push_buffer(&PackedVector2Array::from(samples.as_slice()));
        }
        if self.memory.get_played(Ordering::Relaxed) {
            let buffered = (CAPACITY + self.memory.get_buffer_head(Ordering::Relaxed)
                - self.memory.get_buffer_tail(Ordering::Relaxed))
                % CAPACITY;
            let queued = (self.capacity - self.playback.get_frames_available()).max(0) as u64;
            let latency = AudioServer::singleton().get_output_latency().max(0.0);
            let delay = (buffered as u64 + queued) * 1_000_000 / RATE as u64
                + (latency * 1_000_000.0) as u64;
            mailbox
                .position
                .store(self.end_pts.saturating_sub(delay), Ordering::Relaxed);
        }
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        if self.player.is_instance_valid() {
            self.player.stop();
            self.player.queue_free();
        }
    }
}

/// Adapter for VideoStreamPlayback::mix_audio, retaining partial writes.
pub struct VideoOutput {
    memory: AudioRegulatorSharedMemory,
    push: AudioRegulatorPush,
    pull: AudioRegulatorPull,
    end_pts: u64,
    queued: f64,
    pending: Vec<f32>,
}
impl Default for VideoOutput {
    fn default() -> Self {
        let memory = AudioRegulatorSharedMemory::native(CAPACITY, 8);
        Self {
            push: AudioRegulatorPush::new(memory.clone(), RATE, TARGET),
            pull: AudioRegulatorPull::new(memory.clone(), TARGET),
            memory,
            end_pts: 0,
            queued: 0.0,
            pending: Vec::new(),
        }
    }
}
impl VideoOutput {
    pub fn pump(
        &mut self,
        mailbox: &AudioMailbox,
        delta: f64,
        mut write: impl FnMut(&[f32]) -> usize,
    ) {
        self.queued = (self.queued - delta.max(0.0) * RATE as f64).max(0.0);
        let packets: Vec<_> = mailbox
            .queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        for (pts, pcm) in packets {
            let missing = if self.end_pts != 0 {
                pts.saturating_sub(self.end_pts)
                    .saturating_mul(RATE as u64)
                    .checked_div(1_000_000)
                    .unwrap()
                    .min(RATE as u64) as u32
            } else {
                0
            };
            self.end_pts = pts + (pcm.len() / 2) as u64 * 1_000_000 / RATE as u64;
            let bytes: Vec<u8> = pcm.iter().flat_map(|v| v.to_ne_bytes()).collect();
            self.push.push(missing, &bytes);
        }
        if self.end_pts == 0 {
            return;
        }
        let wanted = (OUTPUT_TARGET as f64 - self.queued).max(0.0) as usize;
        if self.pending.is_empty() && wanted > 0 {
            let mut bytes = vec![0; wanted * 8];
            let buffered = (CAPACITY + self.memory.get_buffer_head(Ordering::Relaxed)
                - self.memory.get_buffer_tail(Ordering::Relaxed))
                % CAPACITY;
            if self.memory.get_played(Ordering::Relaxed) && wanted > buffered as usize {
                self.end_pts += (wanted - buffered as usize) as u64 * 1_000_000 / RATE as u64;
            }
            self.pull.pull(&mut bytes);
            self.pending = bytes
                .chunks_exact(4)
                .map(|b| f32::from_ne_bytes(b.try_into().unwrap()))
                .collect();
        }
        if !self.pending.is_empty() {
            let written = write(&self.pending).min(self.pending.len() / 2);
            self.pending.drain(..written * 2);
            self.queued += written as f64;
        }
        if self.memory.get_played(Ordering::Relaxed) {
            let buffered = (CAPACITY + self.memory.get_buffer_head(Ordering::Relaxed)
                - self.memory.get_buffer_tail(Ordering::Relaxed))
                % CAPACITY;
            let delay = (buffered as u64 + self.pending.len() as u64 / 2 + self.queued as u64)
                * 1_000_000
                / RATE as u64
                + (AudioServer::singleton().get_output_latency().max(0.0) * 1_000_000.0) as u64;
            mailbox
                .position
                .store(self.end_pts.saturating_sub(delay), Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opus_preserves_timeline_and_flushes_short_tail() {
        let mut encoder = Encoder::new().unwrap();
        let delay = encoder.delay;
        let count = 4800 + 137;
        let samples: Vec<f32> = (0..count)
            .flat_map(|i| {
                let v = (i as f32 * std::f32::consts::TAU * 880.0 / RATE as f32).sin() * 0.25;
                [v, v]
            })
            .collect();
        let mut packets = encoder
            .encode(kyber_media::AudioFrame {
                pts_us: 1_000_000,
                samples,
            })
            .unwrap();
        packets.extend(encoder.flush().unwrap());
        let mut decoder = opus::Decoder::new(RATE, opus::Channels::Stereo).unwrap();
        let mut decoded = Vec::new();
        for (index, packet) in packets.into_iter().enumerate() {
            let AVPacket::Media(packet) = packet else {
                panic!()
            };
            assert_eq!(
                packet.header.pts,
                (1_000_000 - delay) as u64 + index as u64 * 20_000
            );
            let mut pcm = vec![0.0; FRAME * 2];
            assert_eq!(
                decoder
                    .decode_float(&packet.payload, &mut pcm, false)
                    .unwrap(),
                FRAME
            );
            decoded.extend(pcm);
        }
        let skip = delay as usize * RATE as usize / 1_000_000;
        assert!(decoded.len() / 2 >= count + skip);
        let error: f32 = (1000..count)
            .map(|i| {
                let expected =
                    (i as f32 * std::f32::consts::TAU * 880.0 / RATE as f32).sin() * 0.25;
                (decoded[(i + skip) * 2] - expected).powi(2)
            })
            .sum::<f32>()
            / (count - 1000) as f32;
        assert!(error < 0.003, "aligned PCM MSE {error}");
    }
    #[test]
    fn host_opus_header_applies_preskip_and_rejects_channel_mapping() {
        let mut decoder = PacketDecoder::new().unwrap();
        let mailbox = AudioMailbox::default();
        decoder.push(codec(), &mailbox).unwrap();
        let mut head = b"OpusHead".to_vec();
        head.extend_from_slice(&[1, 2, 120, 0, 128, 187, 0, 0, 0, 0, 0]);
        let config = |head: Vec<u8>| {
            AVPacket::Media(MediaPacket {
                header: MediaPacketHeader {
                    is_config: true,
                    is_key: false,
                    pts: 0,
                    size: head.len() as u32,
                },
                payload: head.into(),
            })
        };
        decoder.push(config(head.clone()), &mailbox).unwrap();
        let mut encoder = Encoder::new().unwrap();
        let packets = encoder
            .encode(kyber_media::AudioFrame {
                pts_us: 1_000_000,
                samples: vec![0.1; FRAME * 2],
            })
            .unwrap();
        let AVPacket::Media(first) = &packets[0] else {
            panic!()
        };
        let pts = first.header.pts;
        for packet in packets {
            decoder.push(packet, &mailbox).unwrap();
        }
        let queue = mailbox.queue.lock().unwrap();
        assert_eq!(queue[0].0, pts + 2500);
        assert_eq!(queue[0].1.len(), (FRAME - 120) * 2);
        drop(queue);
        head[18] = 1;
        assert!(decoder.push(config(head), &mailbox).is_err());
        assert!(decoder.push(config(vec![0; 18]), &mailbox).is_err());
    }

    #[test]
    fn audio_queue_is_bounded_for_stopped_players() {
        let queue = AudioMailbox::default();
        for i in 0..100 {
            queue.push(i * 20_000, vec![0.0; FRAME * 2]);
        }
        assert_eq!(queue.queue.lock().unwrap().len(), 50);
        assert_eq!(queue.dropped.load(Ordering::Relaxed), 50);
    }
}
