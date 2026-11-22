// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! Bounded local timing samples; no cross-machine clock assumptions.
use godot::prelude::*;
use std::{
    collections::{BTreeMap, VecDeque},
    time::Duration,
};

const SAMPLES: usize = 120;

#[derive(Default)]
pub struct Statistics {
    samples: BTreeMap<&'static str, VecDeque<f64>>,
}

impl Statistics {
    pub fn record(&mut self, name: &'static str, duration: Duration) {
        let values = self.samples.entry(name).or_default();
        if values.len() == SAMPLES {
            values.pop_front();
        }
        values.push_back(duration.as_secs_f64() * 1000.0);
    }

    pub fn dictionary(&self) -> VarDictionary {
        let mut result = VarDictionary::new();
        for name in [
            "av_sync",
            "capture",
            "encode",
            "decode",
            "decode_queue",
            "receive_to_ready",
            "source_to_receive",
            "source_to_ready",
            "receive_interval",
            "ready_interval",
            "display_interval",
            "queue",
            "texture_update",
        ] {
            let mut entry = VarDictionary::new();
            let values = self.samples.get(name);
            entry.set("available", values.is_some_and(|v| !v.is_empty()));
            entry.set("samples", values.map_or(0, |v| v.len()) as i64);
            if let Some(values) = values.filter(|v| !v.is_empty()) {
                let mut sorted: Vec<_> = values.iter().copied().collect();
                sorted.sort_by(f64::total_cmp);
                let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
                entry.set("last_ms", *values.back().unwrap());
                entry.set("mean_ms", mean);
                entry.set("max_ms", *sorted.last().unwrap());
                entry.set("p95_ms", sorted[(sorted.len() * 95).div_ceil(100) - 1]);
                if name.ends_with("_interval") {
                    entry.set("fps", if mean > 0.0 { 1000.0 / mean } else { 0.0 });
                }
            }
            result.set(name, &entry);
        }
        result.set("end_to_end_latency_available", false);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_only_the_latest_window() {
        let mut statistics = Statistics::default();
        for value in 0..200 {
            statistics.record("decode", Duration::from_millis(value));
        }
        let values = &statistics.samples["decode"];
        assert_eq!(values.len(), SAMPLES);
        assert_eq!(values.front(), Some(&80.0));
        assert_eq!(values.back(), Some(&199.0));
        assert!(!statistics.samples.contains_key("encode"));
    }
}
