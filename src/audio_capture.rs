// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! Main-thread Godot bus capture; only owned PCM crosses into the transport worker.
use crate::file_video::SourceEvent;
use godot::{
    classes::{
        AudioEffectCapture, AudioServer, AudioStreamMicrophone, AudioStreamPlayer, Node,
        ProjectSettings,
    },
    prelude::*,
};

#[derive(Default)]
struct Resampler {
    input: u64,
    output: u64,
    previous: [f32; 2],
    pending: Vec<f32>,
    origin: i64,
    rate: u32,
}
impl Resampler {
    fn push(&mut self, samples: &[[f32; 2]], rate: u32, pts: i64) -> Vec<kyber_media::AudioFrame> {
        if self.rate != rate {
            *self = Self {
                rate,
                origin: pts,
                ..Self::default()
            };
        }
        let mut frames = Vec::new();
        for sample in samples {
            if self.input == 0 {
                self.previous = *sample;
            }
            while self.output * u64::from(rate) <= self.input * 48000 {
                let position = (self.output * u64::from(rate)) as f64 / 48000.0;
                let fraction = if self.input == 0 {
                    1.0
                } else {
                    (position - (self.input - 1) as f64) as f32
                };
                for (previous, current) in self.previous.iter().zip(sample) {
                    self.pending
                        .push(previous + fraction * (current - previous));
                }
                self.output += 1;
                if self.pending.len() == 1920 {
                    frames.push(kyber_media::AudioFrame {
                        pts_us: self.origin + ((self.output - 960) * 1_000_000 / 48000) as i64,
                        samples: std::mem::take(&mut self.pending),
                    });
                }
            }
            self.previous = *sample;
            self.input += 1;
        }
        frames
    }
}

pub struct Capture {
    effect: Gd<AudioEffectCapture>,
    microphone: Option<Gd<AudioStreamPlayer>>,
    resampler: Resampler,
    discarded: i64,
}
impl Capture {
    pub fn bus(name: StringName) -> Result<Self, String> {
        let mut server = AudioServer::singleton();
        let index = server.get_bus_index(&name);
        if index < 0 {
            return Err("Audio bus does not exist".into());
        }
        let mut effect = AudioEffectCapture::new_gd();
        effect.set_buffer_length(0.1);
        server.add_bus_effect(index, &effect);
        Ok(Self {
            effect,
            microphone: None,
            resampler: Resampler::default(),
            discarded: 0,
        })
    }
    pub fn microphone(parent: &mut Gd<Node>) -> Result<Self, String> {
        if !ProjectSettings::singleton()
            .get_setting("audio/driver/enable_input")
            .try_to::<bool>()
            .unwrap_or(false)
        {
            return Err(
                "Enable audio/driver/enable_input and grant microphone permission before capture"
                    .into(),
            );
        }
        let mut server = AudioServer::singleton();
        server.add_bus();
        let index = server.get_bus_count() - 1;
        let name = StringName::from(format!("KyberMicrophone{}", parent.instance_id()).as_str());
        server.set_bus_name(index, &GString::from(&name));
        server.set_bus_mute(index, true);
        let mut capture = Self::bus(name.clone())?;
        let mut player = AudioStreamPlayer::new_alloc();
        player.set_stream(&AudioStreamMicrophone::new_gd());
        player.set_bus(&name);
        parent.add_child(&player);
        player.play();
        capture.microphone = Some(player);
        Ok(capture)
    }
    fn location(&self) -> Option<(i32, i32)> {
        let server = AudioServer::singleton();
        for bus in 0..server.get_bus_count() {
            for effect in 0..server.get_bus_effect_count(bus) {
                if server
                    .get_bus_effect(bus, effect)
                    .is_some_and(|e| e.instance_id() == self.effect.instance_id())
                {
                    return Some((bus, effect));
                }
            }
        }
        None
    }
    pub fn pump(
        &mut self,
        send: &tokio::sync::mpsc::Sender<SourceEvent>,
        connected: bool,
    ) -> Result<(), String> {
        if self.location().is_none() {
            return Err("Captured audio bus or effect was removed".into());
        }
        let dropped = self.effect.get_discarded_frames();
        if !connected || dropped != self.discarded {
            self.effect.clear_buffer();
            self.resampler = Resampler::default();
            self.discarded = dropped;
            return Ok(());
        }
        let count = self.effect.get_frames_available();
        if count <= 0 {
            return Ok(());
        }
        let rate = AudioServer::singleton().get_mix_rate() as u32;
        if !(8000..=192000).contains(&rate) {
            return Err("Unsupported Godot audio mix rate".into());
        }
        let pts = kyproto::clock::now_micros().map_err(|e| e.to_string())?
            - i64::from(count) * 1_000_000 / i64::from(rate);
        let samples: Vec<_> = self
            .effect
            .get_buffer(count)
            .as_slice()
            .iter()
            .map(|v| [v.x, v.y])
            .collect();
        for frame in self.resampler.push(&samples, rate, pts) {
            if send.try_send(SourceEvent::Audio(frame)).is_err() {
                self.resampler = Resampler::default();
                self.effect.clear_buffer();
                break;
            }
        }
        Ok(())
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let owned = self.microphone.is_some();
        if let Some(mut player) = self.microphone.take().filter(|p| p.is_instance_valid()) {
            player.stop();
            player.queue_free();
        }
        if let Some((bus, effect)) = self.location() {
            let mut server = AudioServer::singleton();
            server.remove_bus_effect(bus, effect);
            if owned {
                server.remove_bus(bus);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_chunks_preserve_time_across_input_blocks() {
        for rate in [44100, 48000, 96000] {
            let input: Vec<_> = (0..rate)
                .map(|i| {
                    let x = (i as f32 * 880.0 * std::f32::consts::TAU / rate as f32).sin();
                    [x, -x]
                })
                .collect();
            let mut converter = Resampler::default();
            let output: Vec<_> = input
                .chunks(137)
                .flat_map(|s| converter.push(s, rate, 1_000_000))
                .collect();
            assert!((49..=50).contains(&output.len()));
            for (index, frame) in output.iter().enumerate() {
                assert_eq!(frame.pts_us, 1_000_000 + index as i64 * 20000);
                assert_eq!(frame.samples.len(), 1920);
                assert!(
                    frame
                        .samples
                        .chunks_exact(2)
                        .all(|s| (s[0] + s[1]).abs() < 0.0001)
                );
                assert!(frame.samples.iter().map(|x| x * x).sum::<f32>() > 100.0);
            }
        }
    }
}
