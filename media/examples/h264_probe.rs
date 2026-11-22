// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! Run with --release to compare software and hardware codec CPU time.
use kyber_media::*;
use std::time::Instant;

#[cfg(windows)]
fn cpu_seconds() -> f64 {
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn GetProcessTimes(
            process: *mut std::ffi::c_void,
            creation: *mut u64,
            exit: *mut u64,
            kernel: *mut u64,
            user: *mut u64,
        ) -> i32;
    }
    let (mut creation, mut exit, mut kernel, mut user) = (0, 0, 0, 0);
    assert_ne!(
        unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        },
        0
    );
    (kernel + user) as f64 / 10_000_000.0
}
#[cfg(unix)]
fn cpu_seconds() -> f64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut time) },
        0
    );
    time.tv_sec as f64 + time.tv_nsec as f64 / 1_000_000_000.0
}
#[cfg(not(any(windows, unix)))]
fn cpu_seconds() -> f64 {
    f64::NAN
}

fn main() -> Result<()> {
    let modes = match std::env::args().nth(1).as_deref() {
        Some("auto") => vec![Acceleration::Auto],
        Some("software") => vec![Acceleration::Software],
        Some("hardware") => vec![Acceleration::HardwareRequired],
        _ => vec![Acceleration::Software, Acceleration::HardwareRequired],
    };
    for mode in modes {
        let settings = EncoderSettings {
            width: 640,
            height: 360,
            fps: 24,
            bitrate: 4_000_000,
            acceleration: mode,
        };
        eprintln!("Creating {mode:?} encoder");
        let mut encoder = h264_encoder(&settings)?;
        eprintln!("Creating {mode:?} decoder");
        let mut decoder = h264_decoder_with_acceleration(mode)?;
        let mut frame = VideoFrame {
            width: settings.width,
            height: settings.height,
            pts_us: 0,
            rgba: vec![0; (settings.width * settings.height * 4) as usize],
        };
        let mut packets = Vec::new();
        let start = Instant::now();
        let cpu = cpu_seconds();
        for n in 0..120 {
            frame.pts_us = n * 1_000_000 / 24;
            for (index, pixel) in frame.rgba.chunks_exact_mut(4).enumerate() {
                pixel.copy_from_slice(&[200, ((index / 640 + n as usize) % 256) as u8, 30, 255]);
            }
            packets.extend(encoder.encode(&frame)?);
        }
        packets.extend(encoder.drain()?);
        let encode_cpu = cpu_seconds() - cpu;
        let encode_wall = start.elapsed().as_secs_f64();
        assert_eq!(packets.len(), 120);
        assert!(packets.iter().all(|p| p.keyframe));
        let bytes: usize = packets.iter().map(|p| p.data.len()).sum();
        let start = Instant::now();
        let cpu = cpu_seconds();
        let mut decoded = Vec::new();
        for packet in &packets {
            decoded.extend(decoder.decode(EncodedPacket {
                data: &packet.data,
                pts_us: packet.pts_us,
                keyframe: packet.keyframe,
            })?);
        }
        decoded.extend(decoder.drain()?);
        let decode_cpu = cpu_seconds() - cpu;
        assert_eq!(decoded.len(), 120);
        assert!(decoded.iter().all(|f| f.width == 640 && f.height == 360));
        assert!(
            decoded.iter().all(|f| f.rgba[0].abs_diff(200) < 15),
            "red pixels: {:?}",
            decoded
                .iter()
                .map(|f| &f.rgba[..4])
                .take(8)
                .collect::<Vec<_>>()
        );
        if mode == Acceleration::HardwareRequired {
            assert!(encoder.hardware_active());
            assert!(decoder.hardware_active());
        }
        println!(
            "{mode:?}: frames={}, bytes={bytes}, encode_cpu={encode_cpu:.3}s, encode_wall={encode_wall:.3}s, decode_cpu={decode_cpu:.3}s, decode_wall={:.3}s, hardware_encode={}, hardware_decode={}",
            decoded.len(),
            start.elapsed().as_secs_f64(),
            encoder.hardware_active(),
            decoder.hardware_active()
        );
    }
    Ok(())
}
