#![cfg(any(
// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

    windows,
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
))]
use kyber_media::{EncodedPacket, VideoFrame};

#[cfg(target_os = "linux")]
#[test]
fn missing_gstreamer_plugins_report_unavailable_without_aborting() {
    if std::env::var_os("KYBER_TEST_NO_PLUGINS").is_some() {
        let capabilities = kyber_media::capabilities();
        assert!(!capabilities.h264_decode);
        assert!(!capabilities.file_video);
        assert!(!capabilities.reason.is_empty());
        assert!(kyber_media::h264_decoder().is_err());
        return;
    }
    let temporary = std::env::temp_dir().join(format!("kyber-gst-{}", std::process::id()));
    std::fs::create_dir_all(&temporary).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "missing_gstreamer_plugins_report_unavailable_without_aborting",
        ])
        .env("KYBER_TEST_NO_PLUGINS", "1")
        .env("GST_PLUGIN_SYSTEM_PATH_1_0", &temporary)
        .env("GST_PLUGIN_PATH_1_0", &temporary)
        .env("GST_REGISTRY_1_0", temporary.join("registry.bin"))
        .output()
        .unwrap();
    std::fs::remove_dir_all(&temporary).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn check_frame(frame: &VideoFrame) {
    assert_eq!((frame.width, frame.height), (64, 48));
    let pixel = |x: usize, y: usize| &frame.rgba[(y * 64 + x) * 4..][..4];
    let top = pixel(32, 8);
    let bottom = pixel(32, 40);
    assert!(top[0] > 200 && top[1] < 30 && top[2] < 30, "top: {top:?}");
    assert!(
        bottom[0] < 30 && bottom[1] < 30 && bottom[2] > 200,
        "bottom: {bottom:?}"
    );
    assert_eq!(top[3], 255);
}

#[test]
fn system_file_decoder_preserves_color_orientation_and_timestamps() {
    let fixtures = std::env::var_os("KYBER_MEDIA_FIXTURES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media"));
    let path = fixtures.join("colors.mp4");
    let mut source = kyber_media::open_video(&path).unwrap();
    let mut count = 0;
    let mut previous = -1;
    while let Some(frame) = source.next_frame().unwrap() {
        check_frame(&frame);
        assert!(frame.pts_us >= previous);
        previous = frame.pts_us;
        count += 1;
    }
    assert_eq!(count, 6);
    assert!(kyber_media::open_video(std::path::Path::new("missing-video.mp4")).is_err());
}

#[test]
fn system_file_decoder_reads_cpu_pixels_at_hardware_supported_dimensions() {
    let fixtures = std::env::var_os("KYBER_MEDIA_FIXTURES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media"));
    let mut source = kyber_media::open_video(&fixtures.join("surface.mp4")).unwrap();
    let mut count = 0;
    while let Some(frame) = source.next_frame().unwrap() {
        assert_eq!((frame.width, frame.height), (640, 360));
        let center = &frame.rgba[(180 * 640 + 320) * 4..][..4];
        assert!(center[0] > 200 && center[1] < 30 && center[2] < 30);
        assert_eq!(center[3], 255);
        count += 1;
    }
    assert_eq!(count, 6);
}

#[test]
fn h264_access_units_decode_and_reset_without_external_runtime() {
    let data = include_bytes!("media/colors.h264");
    let starts: Vec<_> = data
        .windows(5)
        .enumerate()
        .filter_map(|(i, w)| (w[..4] == [0, 0, 0, 1] && w[4] & 31 == 9).then_some(i))
        .chain(std::iter::once(data.len()))
        .collect();
    assert_eq!(starts.len(), 7);
    let mut decoder = kyber_media::h264_decoder().unwrap();
    for _ in 0..2 {
        let mut frames = Vec::new();
        for (i, pair) in starts.windows(2).enumerate() {
            frames.extend(
                decoder
                    .decode(EncodedPacket {
                        data: &data[pair[0]..pair[1]],
                        pts_us: i as i64 * 41667,
                        keyframe: true,
                    })
                    .unwrap(),
            );
        }
        frames.extend(decoder.drain().unwrap());
        assert_eq!(frames.len(), 6);
        for (i, frame) in frames.iter().enumerate() {
            check_frame(frame);
            assert_eq!(frame.pts_us, i as i64 * 41667);
        }
        decoder.reset().unwrap();
    }
}

#[test]
fn stream_recovers_after_loss_and_accepts_resolution_changes() {
    let mut stream = kyber_media::H264Stream::new().unwrap();
    for data in [
        include_bytes!("media/colors.h264").as_slice(),
        include_bytes!("media/resized.h264").as_slice(),
    ] {
        let starts: Vec<_> = data
            .windows(5)
            .enumerate()
            .filter_map(|(i, w)| (w[..4] == [0, 0, 0, 1] && w[4] & 31 == 9).then_some(i))
            .chain(std::iter::once(data.len()))
            .collect();
        stream.discontinuity().unwrap();
        assert!(
            stream
                .decode(EncodedPacket {
                    data: &[1, 2, 3],
                    pts_us: 0,
                    keyframe: false
                })
                .unwrap()
                .is_empty()
        );
        let mut frames = Vec::new();
        for pair in starts.windows(2) {
            frames.extend(
                stream
                    .decode(EncodedPacket {
                        data: &data[pair[0]..pair[1]],
                        pts_us: 0,
                        keyframe: true,
                    })
                    .unwrap(),
            );
        }
        frames.extend(stream.drain().unwrap());
        assert_eq!(frames.len(), starts.len() - 1);
        let expected = if starts.len() == 7 {
            (64, 48)
        } else {
            (62, 46)
        };
        for frame in frames {
            assert_eq!((frame.width, frame.height), expected);
        }
    }
}

#[test]
fn configuration_packets_are_reused_after_discontinuity() {
    let data = include_bytes!("media/colors.h264");
    let end = data
        .windows(5)
        .enumerate()
        .skip(1)
        .find_map(|(i, w)| (w[..4] == [0, 0, 0, 1] && w[4] & 31 == 9).then_some(i))
        .unwrap();
    let first = &data[..end];
    let slice = first
        .windows(4)
        .enumerate()
        .find_map(|(i, w)| (w[..3] == [0, 0, 1] && w[3] & 31 == 5).then_some(i))
        .unwrap();
    let mut stream = kyber_media::H264Stream::new().unwrap();
    stream.configure(&first[..slice / 2]).unwrap();
    stream.configure(&first[slice / 2..slice]).unwrap();
    for _ in 0..2 {
        let mut frames = stream
            .decode(EncodedPacket {
                data: &first[slice..],
                pts_us: 12345,
                keyframe: true,
            })
            .unwrap();
        frames.extend(stream.drain().unwrap());
        assert_eq!(frames.len(), 1);
        check_frame(&frames[0]);
        stream.discontinuity().unwrap();
    }
    assert!(stream.configure(&vec![0; 1024 * 1024 + 1]).is_err());
}

#[cfg(any(
    windows,
    target_os = "macos",
    target_os = "linux",
    target_os = "android"
))]
#[test]
fn native_audio_resamples_to_stereo_48khz_and_preserves_time() {
    let root = std::env::var_os("KYBER_MEDIA_FIXTURES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media"));
    assert!(
        kyber_media::open_audio(&root.join("colors.mp4"))
            .unwrap()
            .is_none()
    );
    let mut source = kyber_media::open_audio(&root.join("tone.wav"))
        .unwrap()
        .unwrap();
    let mut count = 0;
    let mut energy = 0.0f64;
    let mut previous = -1;
    while let Some(frame) = source.next_frame().unwrap() {
        assert!(frame.pts_us >= previous);
        previous = frame.pts_us;
        assert_eq!(frame.samples.len() % 2, 0);
        for pair in frame.samples.chunks_exact(2) {
            assert!((pair[0] - pair[1]).abs() < 0.0001);
            energy += f64::from(pair[0]).powi(2);
        }
        count += frame.samples.len() / 2;
    }
    assert!((11900..=12100).contains(&count), "sample count {count}");
    assert!(energy / count as f64 > 0.001);
}
