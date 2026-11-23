<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Kyber SDK integration

This reference describes the embedded SDK, internal interfaces, and worker lifetimes. For platform build commands, see [Build, test, and distribute](development.md).

## Pinned sources

The extension embeds the Kyber 0.29 SDK series.

| Repository | Commit |
|---|---|
| kysdk | `ebca32a08f6488ea23750943ded52119efd584d6` |
| kyctl | `92c632f9e889917a34aaa7697fb88b4991054b1c` |

The kysdk submodules pin kymux, kynput, and kyutil. `scripts/prepare_sdk.py` checks the versions and applies patches without duplicating changes on subsequent runs.

The build extracts playback event, listener, and window handle definitions from the pinned KyCtl commit. It checks source file hashes before extraction. It combines those definitions with the project's MIT-licensed adapters under `.deps/`, preserving upstream copyright and license notices. Run `python scripts/test_prepare_sdk.py` to validate generation.

## Common media interfaces

The `kyber-media` crate in `media/` is independent of Godot and Kyber transport.

| Interface | Role |
|---|---|
| `VideoDecoder` | Decode compressed frames, drain remaining frames, and reset the decoder |
| `VideoSource` | Read the next video frame from a file |
| `AudioSource` / `open_audio()` | Read the first audio track, or return `None` when no audio track exists |
| `AudioFrame` | Preserve the source PTS with 48 kHz stereo floating-point PCM |
| `EncodedPacket` | Carry an Annex B H.264 access unit, a PTS in microseconds, and keyframe information |
| `VideoFrame` | Carry display dimensions, a PTS, and top-to-bottom RGBA8 pixels |
| `H264Stream` | Retain configuration packets, wait for a keyframe after packet loss, and feed the decoder |
| `capabilities()` | Report the backend, H.264 decoding support, file reading support, and reasons for unavailable features |

### Platform backends

Windows uses Media Foundation for H.264 decoding, color conversion, and Source Reader access. The decoder handles resolution changes, display cropping, row stride, and image orientation.

File and Viewport streaming support H.264 encoding and decoding with acceleration selection. The Windows D3D12 receive path supports GPU transfer. Other paths use CPU memory. See [H.264 and GPU acceleration](h264.md).

Linux loads system GStreamer libraries when required for H.264 decoding and file reading. RPC and JPEG reception remain available without those libraries or codecs. macOS uses VideoToolbox and AVFoundation. Android arm64 uses MediaCodec, MediaExtractor, and ImageReader. iOS arm64 shares the AVFoundation and VideoToolbox implementation with macOS.

File audio also uses platform media APIs. Linux uses GStreamer audio conversion plugins. Android converts mono and stereo PCM. Godot plays Opus audio received from existing Kyber hosts.

## SDK connection path

The build places `sdk/native_desktop.rs` in the SDK's Kymux media path. It receives compressed packets from the official video endpoint without VLC IPC.

The official SDK handles connection setup, authentication, TLS, input transport, and automatic reconnection. After the SDK reports a successful reconnection, the Godot adapter requests the last streaming configuration again.

The native host path supports H.264 without rotation. Unsupported codecs or rotation produce an error. After a loss notification, the adapter resets the decoder and resumes at the next keyframe.

File streaming uses a separate path. Platform APIs read the file, then separate KyProto endpoints carry JPEG or H.264 video and Opus audio within one connection. Viewport streaming can also capture a Godot audio bus or microphone. See [Audio and microphones](audio.md).

## Internal bridge

`sdk/godot_bridge.rs` and `src/backend.rs` link in the same build. They use internal C-style callbacks, without an external SDK path or ABI version number. Update both sides together when changing an interface.

| Function | Role |
|---|---|
| `kyclient_godot_create` | Accept configuration JSON and callbacks, then request a connection |
| `kyclient_godot_start` | Request H.264 video and optional Opus audio; return `-3` when video decoding is unavailable |
| `kyclient_godot_input` | Send keys, relative mouse motion, buttons, wheel events, and gamepad input |
| `kyclient_godot_clipboard` | Set offered text and request text from the peer |
| `kyclient_godot_destroy` | Stop the connection, wait for workers, and release resources |

The public Godot API converts numeric errors to descriptions. It retains connection state, frame counts, and drop counts.

`get_media_capabilities()` describes OS media support. It differs from `get_capabilities()`, which describes capabilities for the connection state. Its `audio_playback` and `file_audio` fields describe implemented support. `file_audio` does not guarantee that every file codec is available.

`KyberSession.get_capabilities()` reports `godot_audio` and `native_audio_request` as `true`. The adapter requests Opus audio from the existing host and forwards it to the common decoder.

Input integration adds `kynput_get_external_client_config()`. It does not start global OS input hooks or a dummy input generator. The `keycode` crate converts USB HID Usage IDs to Kyber's Windows Scan Code Set 1 representation.

## Worker threads and lifetimes

The native host video path is:

```text
Official Kyber SDK video endpoint
  → queue of at most four packets
  → VideoDecoder on a dedicated thread
  → RGBA callback
  → mailbox holding the latest frame
  → ImageTexture on the Godot main thread
```

COM and media objects are created, used, and destroyed on their dedicated thread. File reading also uses a dedicated thread. Its video and audio event queue is limited to two entries, with audio split into 20 ms blocks. JPEG compression has a separate worker and a queue limited to one frame.

File reception separates network reading from JPEG decoding. During audio playback, it selects a video frame matching the estimated audio playback position from at most 24 timestamped frames. `kyaudioreg` regulates the audio buffer and playback-rate differences. Godot receives audio through `VideoStreamPlayback.mix_audio()`.

`close()` waits for the SDK and decoder workers to stop before releasing callback state. This synchronous shutdown also blocks Godot while it completes. The maximum shutdown time after extended operation has not been verified.

## Build and distribution

On Windows, `scripts/build.ps1 -Release` fetches the SDK, builds the extension, and installs it into the demo. It requires C++ build tools, the Windows SDK, and CMake. It does not require WSL or a VLC build.

The Windows build statically links the SDK, input support, Opus, and the C/C++ runtime into `godot_kyber.dll`. It resolves Media Foundation libraries from System32 at runtime. Missing Media Foundation components do not prevent the extension itself from loading.

The addon does not bundle VLC, FFmpeg, or external SDK DLLs. The installer removes legacy VLC files recorded in its previous manifest. It also regenerates license documents and export settings. Install the complete `addons/kyber` directory.

See [Build, test, and distribute](development.md) for the other platforms and [Licensing](../BINARY_LICENSE.md) for binary distribution terms.

## Verification scope

Rust tests cover real Media Foundation and GStreamer decoding, color, orientation, PTS, resolution changes, cropping, and reset. GUT and two-process tests cover file streaming and RPC. SDK startup, connection failure, shutdown, and Godot exports are also tested.

`mock-sdk` is for tests only. It cannot be enabled with the default `native-sdk` feature, and distribution scripts reject mock builds.

Authentication, video, input, and reconnection against a real external Kyber host remain unverified. See the development guide for platform-specific video, audio, and RPC coverage. Extended input and clipboard exchange still require external-host and mobile-device verification.

## Input and clock patches

`sdk/kynput_clipboard.rs` and `sdk/kynput_clipboard_wrapper.rs` expose the SDK's existing text clipboard packets. They do not introduce a separate clipboard protocol.

Notifications use the existing event callback. Its payload limit is 512 KiB to allow for JSON escaping. Clipboard text itself is limited to 60 KiB.

A build-time patch corrects right-stick-click mapping that otherwise becomes a right shoulder button. Another patch uses 128-bit intermediate arithmetic for the Windows SDK monotonic clock to prevent multiplication overflow after long uptime.

## Existing-host audio adapter

`kyclient_godot_create()` includes an audio packet callback, and the video callback includes a PTS. These are internal interfaces updated together in `sdk/` and `src/backend.rs`.

The build installs `sdk/audio_forward.rs` into the SDK. It forwards AVPacket headers and payloads from the official audio endpoint. Audio payloads are limited to 4,000 bytes.

The common Opus decoder converts packets to PCM and passes them to a bounded mailbox. SDK workers do not access Godot nodes. Shutdown stops the audio forwarding task and retains callback data until SDK destruction completes.

Audio requests accompany screen streaming and support mono or stereo Opus. Interoperability with an official host remains unverified. See [Receive audio from an existing Kyber host](audio.md#receive-audio-from-an-existing-kyber-host).
