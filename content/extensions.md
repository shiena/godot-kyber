<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Capture, input, and performance

[Documentation](README.md) | [日本語](extensions_ja.md)

## Choose the feature and connection

| Task | Connection |
| --- | --- |
| Send a rendered Godot scene to multiple receivers | File/Viewport protocol with `KyberFileVideo` |
| Control an existing host or exchange clipboard text | Streaming SDK host session |
| Exchange application RPCs | `KyberMultiplayerPeer`; see the [RPC guide](multiplayer.md) |

A video connection does not enable every input or clipboard feature. Check the connection requirements in each section before wiring your scene.

## Send a Viewport to multiple receivers

```gdscript
var sender := KyberFileVideo.new()
add_child(sender)
sender.start_viewport_sender($SubViewport, "0.0.0.0", 4433, 24)
```

Use **Viewport streaming** in the demo to send an animated scene. Set a SubViewport to `UPDATE_ALWAYS` when it isn't otherwise displayed. Capture requires a rendering display server; headless capture returns an error.

Capture runs after `RenderingServer.frame_post_draw`. GPU readback runs on the Godot main thread. Resizing and JPEG encoding run on a worker. Output is 640×360 at up to 24 fps, with letterboxing. The source size must fit the 4096-pixel dimension limit. This describes the JPEG capture path. For H.264 acceleration and GPU transfer requirements, see the [H.264 guide](h264.md).

Each receiver uses the same address, port, and connection code. File and Viewport sending share encoding across receivers. Up to 16 connected or pending receiver tasks are accepted. Each connection retains the latest pending encoded frame. Slow receivers don't block other receivers. Audio uses a bounded queue of 50 packets; a receiver that falls behind skips overwritten packets; a timed-out writer is disconnected. `get_connected_clients()` counts authenticated receivers with ready endpoints. `get_skipped_client_frames()` sums per-client skipped encoded frames. A disconnected receiver can join again while the sender remains active. There is no automatic reconnect on this path.

RPC remains limited to one remote peer. The video broadcast protocol is separate from an official Kyber host. The current protocol is revision 2, with clock endpoint 4 and audio endpoint 6. New receivers also accept revision 1 video-only senders. Old receivers cannot receive revision 2 streams. Viewport capture can include opt-in [bus or microphone audio](audio.md). Unreliable/FEC connection codes require the new receiver; see [protocol compatibility](audio.md#transport-and-protocol-compatibility).

<!-- capture: viewport-sender | Viewport sender with one receiver -->

<!-- capture: viewport-receiver | Viewport receiver -->

## Read performance statistics

Use `KyberFileVideo.get_performance_statistics()`, `KyberClient.get_performance_statistics()`, or `KyberSession.get_statistics().performance`.

| Measurement | Meaning |
|---|---|
| `capture` | Viewport readback and RGBA conversion |
| `encode`, `decode` | CPU JPEG conversion/compression or decompression time |
| `receive_interval` | Time between received video packets |
| `decode_queue` | Time from receipt to starting JPEG decoding |
| `receive_to_ready` | Time from receipt to decoded RGBA availability |
| `ready_interval` | Time between decoded or preview frames |
| `queue` | Time from RGBA availability to consumption by Godot |
| `texture_update` | CPU time spent creating/updating the texture |
| `display_interval` | Interval between those texture updates, not physical display refreshes |
| `source_to_receive`, `source_to_ready` | Estimated age relative to the sender's monotonic clock |
| `av_sync` | Selected video frame’s age relative to estimated audio playback position; not physical speaker/display synchronization |

Each measurement has `available` and `samples`. Available measurements also have `last_ms`, `mean_ms`, `max_ms`, and `p95_ms`. Intervals include `fps`. The window retains 120 samples. `audio_received_samples` counts stereo PCM frames delivered to the audio mailbox. `audio_playback_pts_us` reports the estimated playback position in microseconds. Unsupported stages are unavailable, not zero.

File/Viewport reception uses KyProto's `ClockSyncClientProtocol.sync_average(3)` and server `serve()` API. It updates the offset every 10 seconds. Initial synchronization and refresh attempts time out after five seconds. Failure ends the receive session. `clock_synchronized`, `clock_offset_ms`, and `clock_rtt_ms` describe the latest successful estimate. Network asymmetry affects accuracy.

Viewport timestamps describe capture. File timestamps describe scheduled playback on the sender. `source_to_ready` includes encoding, transport, and decoding but excludes presentation. `end_to_end_latency_available` remains false. Clock synchronization doesn't guarantee frame-accurate simultaneous display across clients. Native-host playback doesn't yet apply a shared clock to presentation.

The network reader drains packets independently of decoding. A slow decoder skips to the latest compressed frame. Frames already over 500 ms old before decoding are discarded using the synchronized clock. Receiver drop counts include skipped compressed frames and overwritten decoded frames.

## Forward wheel and gamepad input

`KyberSession` and `KyberClient` accept `InputEventMouseButton`, `InputEventJoypadButton`, and `InputEventJoypadMotion` through `send_input_event(event)`.

- `send_mouse_wheel(dx, dy)`: finite values in −1..1; positive X is right, positive Y is down. Wheel releases are ignored.
- `send_gamepad_button(device, button, pressed)`: device 0..31 and Godot standard button 0..14.
- `send_gamepad_axis(device, axis, value)`: Godot axis 0..5. Sticks use −1..1; triggers use 0..1.
- `send_gamepad_connection(device, connected)`: plug or unplug a remote gamepad. The first input event also plugs it automatically.

Call the disconnect method when `Input.joy_connection_changed` reports removal and when releasing input capture. The demo releases gamepads on focus loss and Escape. Inputs require a streaming SDK host session. Host OS injection and permissions still determine usability. Use `KyberInputRelay` for the extended input described below. Existing-host input interoperability is unverified.

## Exchange text through Godot's clipboard

Set `KyberSession.clipboard_enabled = true` before connecting. For `KyberClient`, call `set_clipboard_enabled(true)` before connecting. The default is false.

```gdscript
session.clipboard_enabled = true
session.clipboard_text_received.connect(func(text): received_text.text = text)
# Call these from explicit UI actions after starting the host stream.
session.send_system_clipboard()
session.request_clipboard_text()
# A separate user action applies the received text locally.
session.apply_clipboard_text(received_text.text)
```

`send_clipboard_text(text)` offers explicit text without reading the OS clipboard. The bridge uses Kyber's Change, DataRequest, Data, and DataUnavailable packets. `clipboard_available` announces remote text; `request_clipboard_text()` requests it. `clipboard_text_received(text)` delivers the response. `clipboard_unavailable` reports unavailable data. Receiving never overwrites the local clipboard automatically. Offers remain available to the connected host until replaced or disconnected. Text is limited to 60 KiB of UTF-8; larger text is rejected.

Only SDK host sessions support this exchange. File/Viewport and RPC connections don't automatically carry clipboard messages. HTML and images aren't implemented. A successful send means the local SDK accepted the operation, not that the host pasted it.

`is_system_clipboard_supported()` checks `DisplayServer.FEATURE_CLIPBOARD` at runtime. `send_system_clipboard()` uses `clipboard_get()` and `apply_clipboard_text()` uses `clipboard_set()` on the main thread.

| Platform | Godot API and OS restrictions |
|---|---|
| Windows | Clipboard read/write API supported |
| macOS | Supported; OS versions and pasteboard access policy can require permission |
| Linux | X11/Wayland supported by current Godot; compositor/session access still applies |
| Android | Supported; Android 10+ restricts background clipboard reads |
| iOS | Supported; iOS 16+ can ask permission for programmatic pasteboard reads |
| Headless | Check runtime support; no OS clipboard is assumed |

A supported API doesn't imply permission was granted. Godot's string-returning read API doesn't distinguish an empty clipboard from every denied/unavailable read. Mobile permission prompts and real host clipboard exchange remain unverified.

Sources: [Godot DisplayServer](https://docs.godotengine.org/en/4.6/classes/class_displayserver.html), [Android clipboard restrictions](https://developer.android.com/about/versions/10/privacy/changes), [Apple UIPasteControl](https://developer.apple.com/documentation/uikit/uipastecontrol), [macOS pasteboard access](https://developer.apple.com/documentation/appkit/nspasteboard/accessbehavior-swift.enum).

## Forward extended input

Attach `res://addons/kyber/input_relay.gd` to a Node. Call `send_input_event(event)` from `_input()` or `_unhandled_input()`, not both.

For an official Kyber host, set `session` to a streaming `KyberSession`. The default `touch_as_mouse` converts the first finger's press/release to the left mouse button and its drag to relative movement, like a touchpad. Other fingers are ignored. This is not absolute positioning or native multitouch. Disable Godot's mouse-from-touch emulation when forwarding both mouse and touch events to avoid duplicate clicks.

Kyber's existing rumble packets emit `gamepad_rumble_received(device, weak, strong, duration)` on `KyberClient` and `KyberSession`. Set the relay's `local_gamepad_device` to the controller ID sent to the host. Matching-device notifications emit `rumble_received`; enable `vibration_enabled` to apply them with `Input.start_joy_vibration()` and `stop_joy_vibration()`. Negative SDK durations use the upstream five-second fallback.

For Godot-to-Godot input, leave `session` unset. Connect `KyberMultiplayerPeer` as `multiplayer.multiplayer_peer`. Use the same relay node path on both peers and set `remote_peer_id` to the other peer (server: 2; client: 1). Zero disables RPC input.

```gdscript
func _unhandled_input(event: InputEvent) -> void:
    $InputRelay.send_input_event(event)

func on_remote_collision() -> void:
    $InputRelay.send_rumble(0.25, 0.75, 0.1)
```

The RPC path carries `InputEventScreenTouch`, `InputEventScreenDrag`, and `JOY_BUTTON_PADDLE1` through `JOY_BUTTON_PADDLE4`. Touch indices are limited to 0–31. Positions and deltas are normalized to the sending viewport and scaled to the receiving viewport. Map letterboxed video coordinates before forwarding. Standard keyboard, mouse, and gamepad events still use the session input path or your application's RPC logic.

Received events emit `input_received(event)`. Enable `inject_received_input` to call `Input.parse_input_event()` automatically. Events use reserved device ID `-2`; sending rejects this ID to prevent feedback loops. InputMap actions must accept all devices or this ID. Vibration is opt-in through `vibration_enabled` and targets `local_gamepad_device`. Local `Input.start_joy_vibration()` calls are not intercepted; request remote vibration with `send_rumble()`.

Reliable Godot RPC uses Kyber's existing QUIC transport. Both peers need the relay script. The official Kyber input packets and file-video protocol are unchanged. Only the configured peer may send input. Focus loss, disconnection, and node removal release tracked inputs and applied vibration. Call `reset_input()` before changing peer IDs, sessions, or input settings.

The implementation uses shared Godot APIs on Windows, macOS, Linux, Android, and iOS. Touch, paddle, and vibration availability depend on the platform backend and hardware. Handheld vibration is not substituted for controller vibration. Tests cover upstream rumble packets, touch-to-mouse state, rejected requests, and bidirectional RPC through Kyber. Physical vibration and mobile touch are not yet verified for this addition.
