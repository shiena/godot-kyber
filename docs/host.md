<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Existing Kyber hosts

[Documentation](README.md) | [日本語](host_ja.md)

Run commands from the repository root. Paths in code examples are relative to that root.

## Embedded SDK

The SDK and input library are statically linked into the extension. Windows decodes H.264 through Media Foundation. macOS and iOS use VideoToolbox; Android uses MediaCodec. Linux uses system GStreamer plugins. Existing-host Opus audio can be requested with `audio = true` and played through Godot. See [host audio](audio.md#receive-audio-from-an-existing-kyber-host). Copy the entire `addons/kyber` directory. No SDK installation, library path, or `PATH` change is required. See the [SDK integration specification](sdk.md) for build details.

## Connect to an existing host

You need the Kyber host address and Basic authentication credentials. For a self-signed certificate, obtain the SHA-256 fingerprint from the host.

1. Enter the hostname, username, and password. This demo uses port `8080`.
2. For a self-signed certificate, enter its fingerprint.
3. Click **接続** (Connect).

The demo requests the first display at 1280×720 and 20 Mbps. Its audio checkbox controls whether host audio is requested. Connection details aren't saved to disk.

To forward input, enable the input checkbox and click the video. Press Esc or move focus away from the window to release capture. The demo uses `send_input_event()` for keyboard, mouse, wheel, and standard gamepad events. See [input and clipboard APIs](extensions.md).
## Use the extension from GDScript

```gdscript
extends Node

@onready var receiver: KyberClient = $KyberClient
@onready var screen: TextureRect = $TextureRect

func _ready() -> void:
    receiver.connected.connect(_connected, CONNECT_DEFERRED)
    receiver.frame_received.connect(_frame, CONNECT_DEFERRED)
    receiver.error_occurred.connect(push_error, CONNECT_DEFERRED)
    receiver.connect_to_host(
        "stream.example.net", 8080,
        OS.get_environment("KYBER_USERNAME"),
        OS.get_environment("KYBER_PASSWORD"), ""
    )

func _connected() -> void:
    var displays := receiver.get_displays()
    if not displays.is_empty():
        receiver.start_stream(displays[0].id, 1280, 720, 20000000, false)

func _frame() -> void:
    screen.texture = receiver.get_texture()
```

Use `CONNECT_DEFERRED` when a signal handler calls back into the same Rust node. The return values of `connect_to_host()` and `start_stream()` indicate request acceptance. Use signals or `get_state()` to detect connection and streaming completion.

To display video in 3D, attach `demo/spatial_screen.gd` to a `MeshInstance3D` with a `PlaneMesh`. Set `client_path` to the `KyberClient` node.

| API | Description |
|---|---|
| `connect_to_host(host, port, username, password, fingerprint)` | Uses the embedded SDK to request a connection, closing any existing connection. |
| `start_stream(display_id, width, height, bitrate, audio)` | Requests a stream after connecting. Each output dimension must be 1–4096 pixels. |
| `close()` | Stops the SDK and discards frames and textures. Waits for shutdown to finish. |
| `get_texture()` | Returns the latest `ImageTexture`, or `null` before receiving a frame and after disconnecting. |
| `get_displays()` | Returns an array of dictionaries containing `id`, `width`, and `height`. |
| `get_received_frames()` / `get_dropped_frames()` | Returns the number of displayed frames / pending frames replaced by newer frames. |
| `send_key(usage, pressed)` | Sends a USB HID Keyboard Usage ID. For example, A is `4`. |
| `send_mouse_motion(dx, dy)` | Sends relative movement in the range -32768 to 32767. |
| `send_mouse_button(button, pressed)` | Sends a button event: `0` = left, `1` = right, `2` = middle. |

Input methods returning `true` indicate local acceptance, not completion on the host. Enable official SDK reconnection before connecting with `set_auto_reconnect(true)`, or call `connect_to_host()` again manually. See [SDK reconnection](video.md#enable-kyber-sdk-reconnection).
