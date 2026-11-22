<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Native media fixtures

These synthetic fixtures contain no third-party media. They test system decoding without requiring FFmpeg during `cargo test`.
`colors.h264` and `colors.mp4` contain six frames with a red upper half and blue lower half.
`resized.h264` contains three green frames with non-aligned display dimensions.
`surface.mp4` contains six red 640x360 frames to exercise CPU-readable output at dimensions accepted by Android hardware decoders.

`tone.wav` contains a 250 ms, 880 Hz mono tone at 44.1 kHz. It tests Windows, macOS, Linux, and Android conversion to stereo PCM at 48 kHz.

Regenerate from the repository root with FFmpeg:

```powershell
ffmpeg -y -f lavfi -i "color=c=red:s=64x48:r=24" -vf "drawbox=x=0:y=24:w=64:h=24:color=blue:t=fill" -t 0.25 -c:v libx264 -x264-params "aud=1:keyint=1" -pix_fmt yuv420p tests/media/colors.h264
ffmpeg -y -i tests/media/colors.h264 -c:v copy tests/media/colors.mp4
ffmpeg -y -f lavfi -i "color=c=green:s=62x46:r=24" -t 0.125 -c:v libx264 -x264-params "aud=1:keyint=1" -pix_fmt yuv420p tests/media/resized.h264
ffmpeg -y -f lavfi -i "color=c=red:s=640x360:r=24" -t 0.25 -c:v libx264 -x264-params "aud=1:keyint=1" -pix_fmt yuv420p tests/media/surface.mp4
ffmpeg -y -f lavfi -i "sine=frequency=880:sample_rate=44100:duration=0.25" -ac 1 tests/media/tone.wav
```
