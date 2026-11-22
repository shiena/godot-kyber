<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Video reception and file transfer

[Documentation](README.md) | [日本語](video_ja.md)

Run commands from the repository root. Paths in code examples are relative to that root.

## Send and receive a video file

The sender reads video through Media Foundation on Windows, AVFoundation on macOS/iOS, system GStreamer on Linux, and MediaExtractor on Android. No FFmpeg executable or external SDK runtime is required. File format support depends on installed system codecs.

The file-video demo sends JPEG frames over KyProto/QUIC. On every platform, JPEG encoding and decoding run on blocking worker threads, leaving QUIC I/O and acknowledgements responsive. The existing Kyber-host H.264 path uses a separate implementation.

The demo UI is in Japanese. Instructions include the visible labels with English translations.

### Start the demo

1. Build from the repository root:

   ```powershell
   pwsh -File scripts/build.ps1 -Release
   ```

2. Open `demo/project.godot` in Godot.
3. Launch two instances of the game. To launch from PowerShell, run this command twice, adjusting the executable path for your installation:

   ```powershell
   & 'C:/Tools/Godot/4.7.2-stable/Godot_v4.7.2-stable_win64.exe' --path demo
   ```

### Connect the sender and receiver

1. Select **送信モード** (Sender mode) in one instance.
2. Click **動画を開く…** (Open video) to select a file. Use a format supported by your system, such as an H.264 MP4 file. Other containers and codecs depend on the system.
3. Click **送信開始** (Start sending). Playback starts from the beginning when the receiver connects.
4. Copy the connection code displayed by the sender.
5. Select **受信モード** (Receiver mode) in the other instance. For a local test, use `127.0.0.1` and port `4433` on both sides.
6. Paste the connection code into the receiver's connection code field.
7. Click **接続して受信** (Connect and receive). The sender shows a preview, and the receiver shows the received video.
8. Click **停止** (Stop) or switch modes to end the session.

For another PC on your LAN, bind the sender to `0.0.0.0` and connect the receiver to the sender's IPv4 address. Allow the selected UDP port through the firewall. Each new sending session generates a connection code containing the certificate's SHA-256 fingerprint and an authentication token.

Video is scaled to 640×360 while preserving its aspect ratio, then sent as JPEG frames at up to 24 fps through a KyProto video endpoint (Reliable by default). Multiple receivers can join with the same connection code while the sender remains active. Windows, macOS, Linux, Android, and iOS file senders include audio when the file has an audio track. Seeking and automatic reconnection aren't supported. See [capture and multi-client video](extensions.md). With looping disabled, the session ends at the end of the file and keeps the final frame visible.

The payload format and connection procedure are specific to this demo. They aren't compatible with an existing Kyber host's H.264 stream. The transport uses the official Kyber sources pinned by the [SDK integration specification](sdk.md).

| `KyberFileVideo` API | Description |
|---|---|
| `start_sender(file, bind_ip, port, repeat)` | Starts listening. Specify an absolute path to the video file. |
| `get_connection_code()` | Returns the connection code after the listener starts. |
| `start_receiver(host, port, code)` | Connects to the sender. |
| `get_texture()` / `get_frame_count()` | Returns the latest texture / number of displayed frames. |
| `is_active()` / `has_failed()` / `get_status()` | Reports asynchronous operation status. A start method returning `true` means the request was accepted. |
| `stop()` | Stops the worker and clears the texture and connection code. |
## Receive video with VideoStreamPlayer

`KyberVideoStream` extends Godot's `VideoStream`. Assign it to a standard `VideoStreamPlayer` to receive video. You can use it alone or attach a shared `KyberSession`.

### Share a connection with input and status controls

Add a `KyberSession` node to the scene before connecting. The file demo uses this mode and preserves the video's aspect ratio.

```gdscript
@onready var session: KyberSession = $KyberSession
@onready var player: VideoStreamPlayer = $VideoStreamPlayer

func receive(host: String, port: int, connection_code: String) -> void:
    if not session.start_receiver(host, port, connection_code):
        $Status.text = session.get_last_error()
        return
    var stream := KyberVideoStream.new()
    stream.set_session(session)
    player.stream = stream
    player.play()

func _process(_delta: float) -> void:
    $Status.text = session.get_status()
    if player.paused:
        $Status.text += " | Display paused"
```

For SDK reception, call `session.connect_to_host()` with the same arguments as `KyberClient`. After `connected`, select an entry from `get_displays()` and call `start_stream(display_id, width, height, bitrate, audio)`. Attach the session to a video resource. Input methods, including `send_input_event()`, use this same connection. The existing `KyberClient` and `KyberFileVideo` APIs remain available; the session reuses their implementations.

| API | Purpose |
|---|---|
| `get_state()` | `DISCONNECTED`, `CONNECTING`, `CONNECTED`, `STARTING`, `STREAMING`, `FAILED`, `DEMO`, `FINISHED`, or `RECONNECTING` |
| `get_status()` / `get_last_error()` | Current detailed message / retained error, including file transport messages |
| `get_statistics()` | `received_frames`, `dropped_frames`, `width`, and `height`; received frames count those consumed by the receiving node |
| `get_capabilities()` | Current support for input, display selection, seeking, and audio; Godot file audio is supported; native host Opus audio requests are supported |
| `state_changed`, `status_changed`, `error_occurred` | Deferred notifications; callbacks can safely query or control the session |
| `connected`, `disconnected`, `displays_changed`, `frame_received` | Connection and media notifications; `connected` indicates SDK display discovery is complete |
| `close()` | Disconnect all consumers; removing the session from the tree also closes the connection |

The session processes reception independently of its players. Pausing freezes that player's image while reception, status updates, and input continue. Stopping or freeing a player doesn't disconnect a shared session. Video-only sessions support multiple players with independent pause controls. Use one player for an audio session because its audio queue has one consumer. Share that player’s texture to display it in multiple places. Close the session explicitly when finished. As a regular Node, the session follows its `process_mode` when the scene tree is paused.

`set_session()` is runtime-only; node references aren't saved in `.tres`. Call it before assigning the stream to the player. In shared mode, connection settings come from the session, so the resource's host and source fields aren't used. Each player keeps its own texture and presentation count; the session owns receive statistics. No GPU readback is needed to distribute received frames.

`FINISHED` identifies normal file-transfer completion. Authentication progress is reported only where the underlying transport exposes it; no authentication state is inferred from playback. Errors and reception status remain visible while presentation is paused. File audio uses Godot's video player. `get_capabilities().godot_audio` is `true`; `.native_audio_request` is `true`, and `.seek` remains `false`. Existing-host `start_stream(..., true)` requests Opus audio.

### Enable Kyber SDK reconnection

Set `session.auto_reconnect = true` before calling `connect_to_host()`. The existing-host demo exposes this option. For direct `KyberClient` use, call `set_auto_reconnect(true)` before connecting. Changes apply to the next connection; the default is disabled.

The bridge passes this flag to the official SDK's `ConnectConfig.auto_reconnection`. The pinned SDK schedules up to ten attempts at one-second intervals after connection loss and resets its attempt counter after successful reconnection. These values are private SDK defaults, not Godot settings. Initial connection failure is not retried by this mechanism. Godot does not run a retry timer or recreate the SDK client.

SDK `Reconnecting` events update `RECONNECTING` and `get_reconnect_attempts()`. Shared video players remain active during recovery. On `Reconnected`, Godot reissues the last accepted stream request only when the SDK reports `should_restart_streaming = true`. `ReconnectionFailed` becomes `FAILED`. Closing the session destroys the native client and lets the SDK cancel pending work; stopping a shared player still only stops presentation.

The embedded SDK provides this feature. The bridge forwards `DataPlaneClosed` as an interruption when SDK reconnection is enabled, rather than destroying the client before it can reconnect.

File-transfer receivers, RPC peers, and standalone video resources do not enable this feature. In particular, the custom file protocol isn't an official SDK host session. Tests emulate SDK callbacks on the same fixture handle and verify restoration, terminal failure, and cancellation. The updated bridge also passes a type check inside the pinned SDK. Reconnection against a real Kyber host has not been verified.

Official implementation: [configuration and reconnection state machine](https://gitlab.com/kyber/core/kyctl/-/blob/92c632f9e889917a34aaa7697fb88b4991054b1c/kyclient/src/lib.rs).

### Use a standalone video resource

```gdscript
var stream := KyberVideoStream.new()
stream.source = KyberVideoStream.FILE_TRANSFER
stream.host = "127.0.0.1"
stream.port = 4433
stream.set_connection_code(connection_code)
$VideoStreamPlayer.stream = stream
$VideoStreamPlayer.play()
# Pause presentation; resume at the latest received frame.
$VideoStreamPlayer.paused = true
$VideoStreamPlayer.paused = false
```

Without a shared session, the `VideoStreamPlayback` implementation owns the connection and `stop()` closes it. Normal end of stream emits the player's `finished` signal. Stopping retains the last texture. Set `player.stream = null` to clear the display.

Use `player.get_video_texture()` with a `TextureRect`, `Sprite2D`, or 3D material. The texture object stays the same throughout playback. Keep the player in the scene tree so it updates the texture.

You can create the resource in the Inspector and save settings as `.tres` using `ResourceSaver`. Set credentials before assigning the stream to the player. Passwords and connection codes are runtime-only and aren't serialized. Settings are copied when the player instantiates playback; reassign the resource after changing them. Use a separate resource per player: `get_last_error()` and `get_frame_count()` describe its latest playback instance.

| Source | Settings |
|---|---|
| `FILE_TRANSFER` | `host`, `port`, and `set_connection_code()`; receives the file demo's JPEG stream |
| `KYBER_HOST` | `host`, `port`, `username`, `fingerprint`, and `set_password()`; requests the first display by default (`display_id = -1`) |
| `LOCAL_DEMO` | Animated local frames without a network connection |

For `KYBER_HOST`, set `width`, `height`, and `bitrate` before playback. This path uses the same SDK bridge as `KyberClient`; state transitions are tested with the embedded fixture. Real-host reception remains unverified.

These are live streams: duration is unknown, seeking is unsupported, and pausing doesn't pause the sender. Video-only reception keeps the latest frame. Streams with audio use a bounded video queue and select frames against the estimated audio playback position. File audio uses `VideoStreamPlayer` volume and bus settings. Separate-process video reception was verified on Godot 4.2.2 and 4.7.2.

### Forward Godot input events

Call `KyberClient.send_input_event(event)` from `_input()` while input capture is active. It accepts keyboard, mouse movement, three mouse buttons, wheel presses, and standard gamepad button/axis events. See [extended input](extensions.md#forward-wheel-and-gamepad-input). Physical keycodes take priority over logical keycodes. Echoes, unsupported keys, wheel releases, and unsupported gamepad controls return `false`. Mouse deltas are rounded and clamped to the SDK's signed 16-bit range. Modifier keys map to left-side HID usages.

The existing host demo uses this method and sends releases when capture ends. Keep using `send_key()` for HID usages outside the supported mapping. `KyberSession` exposes the same input methods for a connection shared with `VideoStreamPlayer`.

## Media capabilities

Call `get_media_capabilities()` on `KyberSession` or `KyberFileVideo` to inspect `backend`, `h264_decode`, `file_video`, `h264_encode`, `audio_playback`, `file_audio`, `jpeg_receive`, and `reason`.
`file_video` reports the system video reader’s availability. `file_audio` indicates a compiled audio reader for the platform, not installed codec availability. Unsupported codecs or missing Linux audio plugins cause an asynchronous error when opening a file.
Migration: remove the `ffmpeg` argument from `start_sender()`. Use `audio = true` to request host audio, or `false` for video only.

## File audio

A Windows sender reads the first audio track through Media Foundation and converts it to stereo float PCM at 48 kHz.
macOS and iOS use AVFoundation; Linux uses GStreamer with `audioconvert` and `audioresample`. Android uses MediaExtractor and MediaCodec, then converts mono/stereo PCM16 or float PCM to stereo 48 kHz with a stateful linear resampler. Other Android channel layouts are unsupported. Native PCM blocks are split into 20 ms chunks before interleaving with video.
Opus uses 20 ms packets at 128 kbit/s. The codec is statically linked; no extra codec DLL is required.
KyProto carries audio on a separate endpoint, using the selected transport mode, in the same connection as video.
Kyber's `kyaudioreg` manages the audio jitter buffer and compensates playback-rate differences.

`KyberVideoStream` feeds PCM through `VideoStreamPlayback.mix_audio()`. Use the normal `VideoStreamPlayer` volume, bus, and pause controls.
The low-level `KyberFileVideo` node plays preview/received audio through an internal Godot `AudioStreamGenerator` and `AudioStreamPlayer`.
Use one video player per session: the audio queue has a single consumer.

The receiver compares video PTS with an estimated audio playback position, including queued PCM and Godot's reported output latency.
The default buffers hold about 80 ms in Kyber and up to 40 ms in Godot, plus device latency.
`performance.av_sync` reports the selected video frame's age relative to that estimate; it does not measure physical speaker/display synchronization.
Video encoding has its own bounded queue so a slow encoder cannot hold back audio.

The custom file protocol is revision 2. New receivers also accept revision 1 video-only senders; old receivers cannot receive revision 2 streams.
File audio extraction is implemented on Windows, macOS, Linux, Android, and iOS. Existing-host Opus audio reception is implemented; full host interoperability remains unverified. For bus/microphone capture, `KyberAudioStream`, and transport selection, see [live audio](audio.md).

Set `KyberFileVideo.audio_enabled = false` before starting a sender to keep video-only behavior, including files whose audio codec is unsupported by the sending OS.

## H.264

File and Viewport senders support optional H.264. JPEG remains the default. See the [H.264 guide](h264.md) for settings, platform limits, and protocol compatibility.
