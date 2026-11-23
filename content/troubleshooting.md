<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Troubleshooting

## The receiver cannot connect

1. Confirm that the sender has started and displays a connection code.
2. Copy the current code again. Restarting the sending session generates a new code.
3. Check that both ends use the same port.
4. On one computer, use `127.0.0.1`. On separate devices, bind the sender to `0.0.0.0` and enter the sender's LAN address on the receiver.
5. Allow the selected UDP port through the firewall.

Video and RPC use separate connections and codes. Don't paste an RPC connection code into the video receiver.

## The connection starts but no video appears

Check the sender preview first. If it is blank, check the selected file and the sender's status message. Supported file formats depend on system codecs.

If the sender preview works, inspect **Received**, **Displayed**, and **Dropped** on the receiver. Record these values and the status message when reporting a problem. A successful start request means the asynchronous operation was accepted; it does not prove that frames are arriving.

<!-- capture: file-receiver | Received video and status -->

## H.264 fails to start

Try JPEG to isolate codec availability from connection problems. The receiver must have an H.264 decoder; codec negotiation and automatic switching to JPEG are not implemented.

Use **H.264 acceleration: Auto** first. **Hardware required** fails when hardware support is unavailable or cannot be verified. On Linux, also check the installed GStreamer plugins. See the [H.264 guide](../../docs/h264.md).

<!-- capture: h264-settings | H.264 and acceleration settings -->

## Video plays but audio is missing

Check that the file has an audio track and that output volume is audible. For a custom scene, connect `KyberAudioStream` to the same `KyberSession` as the video and assign it to an `AudioStreamPlayer`.

For microphone input, check the OS permission and export settings. macOS and iOS require platform-specific configuration. Follow the [audio guide](../../docs/audio.md) rather than changing video settings.

## Report a reproducible problem

Include the addon version on both devices, Godot version, OS and CPU architecture, codec, acceleration setting, connection direction, and exact status or error text. State whether the problem also occurs on the same computer with JPEG.

Remove connection codes, authentication tokens, private file paths, and personal media from logs and screenshots before posting to [GitHub Issues](https://github.com/shiena/godot-kyber/issues).
