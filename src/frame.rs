// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

pub const MAX_DIMENSION: u32 = 4096;

#[derive(Debug)]
pub struct Frame {
    #[cfg(windows)]
    pub gpu: Option<std::sync::Arc<kyber_media::gpu::Frame>>,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn frame_size(width: u32, height: u32) -> Option<usize> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return None;
    }
    (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)
}

#[derive(Default)]
pub struct Mailbox {
    #[cfg(windows)]
    pub gpu_context: Mutex<Option<kyber_media::gpu::Context>>,
    #[cfg(windows)]
    pub gpu_pending: std::sync::atomic::AtomicBool,
    #[cfg(windows)]
    pub gpu_status: Mutex<String>,
    pub closed: std::sync::atomic::AtomicBool,
    pub audio: crate::audio::AudioMailbox,
    scheduled: Mutex<std::collections::VecDeque<(u64, Frame)>>,
    inner: Mutex<(Option<Frame>, u64, Option<Instant>)>,
    timing: Mutex<Timing>,
}

#[derive(Default)]
struct Timing {
    statistics: crate::statistics::Statistics,
    ready: Option<Instant>,
    displayed: Option<Instant>,
    clock_offset_us: Option<i64>,
    clock_rtt_us: i64,
}

impl Mailbox {
    pub fn put(&self, frame: Frame) {
        let now = Instant::now();
        let mut timing = self.timing.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(previous) = timing.ready {
            timing
                .statistics
                .record("ready_interval", now.duration_since(previous));
        }
        timing.ready = Some(now);
        drop(timing);
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.2 = Some(now);
        if inner.0.replace(frame).is_some() {
            inner.1 += 1;
        }
    }

    pub fn put_timed(&self, frame: Frame, pts: u64) {
        if self
            .audio
            .received
            .load(std::sync::atomic::Ordering::Relaxed)
            == 0
        {
            self.put(frame);
            return;
        }
        let mut queue = self.scheduled.lock().unwrap_or_else(|e| e.into_inner());
        if queue.len() >= 24 {
            queue.pop_front();
            self.discarded(1);
        }
        queue.push_back((pts, frame));
    }
    pub fn take(&self) -> Option<Frame> {
        let position = self
            .audio
            .position
            .load(std::sync::atomic::Ordering::Relaxed);
        let mut scheduled = self.scheduled.lock().unwrap_or_else(|e| e.into_inner());
        if position == 0 && !scheduled.is_empty() {
            let (_, newest) = scheduled.pop_back().unwrap();
            self.discarded(scheduled.len() as u64);
            scheduled.clear();
            return Some(newest);
        }
        let mut due = None;
        let mut due_pts = 0;
        while scheduled.front().is_some_and(|(pts, _)| *pts <= position) {
            if due.is_some() {
                self.discarded(1);
            }
            let (pts, frame) = scheduled.pop_front().unwrap();
            due_pts = pts;
            due = Some(frame);
        }
        drop(scheduled);
        if due.is_some() {
            self.record(
                "av_sync",
                Duration::from_micros(position.saturating_sub(due_pts)),
            );
            return due;
        }

        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let frame = inner.0.take();
        if let Some(ready) = inner.2.take() {
            self.record("queue", ready.elapsed());
        }
        frame
    }

    pub fn discarded(&self, count: u64) {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).1 += count;
    }

    pub fn dropped(&self) -> u64 {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).1
    }

    pub fn record(&self, phase: &'static str, duration: Duration) {
        self.timing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .statistics
            .record(phase, duration);
    }

    pub fn presented(&self, upload: Duration) {
        let now = Instant::now();
        let mut timing = self.timing.lock().unwrap_or_else(|e| e.into_inner());
        timing.statistics.record("texture_update", upload);
        if let Some(previous) = timing.displayed {
            timing
                .statistics
                .record("display_interval", now.duration_since(previous));
        }
        timing.displayed = Some(now);
    }

    pub fn synchronize(&self, offset_us: i64, rtt_us: i64) {
        let mut timing = self.timing.lock().unwrap_or_else(|e| e.into_inner());
        timing.clock_offset_us = Some(offset_us);
        timing.clock_rtt_us = rtt_us;
    }

    pub fn statistics(&self) -> godot::prelude::VarDictionary {
        let timing = self.timing.lock().unwrap_or_else(|e| e.into_inner());
        let mut result = timing.statistics.dictionary();
        #[cfg(windows)]
        result.set("gpu_transfer", self.gpu_status.lock().unwrap().as_str());
        result.set(
            "audio_received_samples",
            self.audio
                .received
                .load(std::sync::atomic::Ordering::Relaxed) as i64,
        );
        result.set(
            "audio_playback_pts_us",
            self.audio
                .position
                .load(std::sync::atomic::Ordering::Relaxed) as i64,
        );
        result.set(
            "audio_dropped_packets",
            self.audio
                .dropped
                .load(std::sync::atomic::Ordering::Relaxed) as i64,
        );
        result.set("clock_synchronized", timing.clock_offset_us.is_some());
        if let Some(offset) = timing.clock_offset_us {
            result.set("clock_offset_ms", offset as f64 / 1000.0);
            result.set("clock_rtt_ms", timing.clock_rtt_us as f64 / 1000.0);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_and_oversized_frames() {
        assert_eq!(frame_size(0, 1), None);
        assert_eq!(frame_size(u32::MAX, u32::MAX), None);
        assert_eq!(frame_size(1920, 1080), Some(8_294_400));
    }

    #[test]
    fn slow_consumer_gets_only_the_latest_frame() {
        let mailbox = Mailbox::default();
        for value in 0..100 {
            mailbox.put(Frame {
                #[cfg(windows)]
                gpu: None,
                width: 1,
                height: 1,
                rgba: vec![value; 4],
            });
        }
        assert_eq!(mailbox.dropped(), 99);
        assert_eq!(mailbox.take().unwrap().rgba, vec![99; 4]);
        assert!(mailbox.take().is_none());
    }
}

#[cfg(test)]
mod synchronization_tests {
    use super::*;
    use std::sync::atomic::Ordering;
    #[test]
    fn video_waits_for_audio_and_discards_only_obsolete_frames() {
        let mailbox = Mailbox::default();
        mailbox.audio.received.store(1, Ordering::Relaxed);
        mailbox.audio.position.store(99, Ordering::Relaxed);
        for pts in [100, 200, 300] {
            mailbox.put_timed(
                Frame {
                    #[cfg(windows)]
                    gpu: None,
                    width: 1,
                    height: 1,
                    rgba: vec![(pts / 100) as u8; 4],
                },
                pts,
            );
        }
        assert!(mailbox.take().is_none());
        mailbox.audio.position.store(250, Ordering::Relaxed);
        assert_eq!(mailbox.take().unwrap().rgba, vec![2; 4]);
        assert_eq!(mailbox.dropped(), 1);
        assert!(mailbox.take().is_none());
        mailbox.audio.position.store(300, Ordering::Relaxed);
        assert_eq!(mailbox.take().unwrap().rgba, vec![3; 4]);
    }
}
