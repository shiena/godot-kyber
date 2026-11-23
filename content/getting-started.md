<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Get started

Send a video between two Godot instances using the included demo. Start on one computer before connecting devices on your network.

## Download the demo

Open [GitHub Releases](https://github.com/shiena/godot-kyber/releases/latest) and download the **demo** ZIP. Extract it, then open `project.godot` in Godot **4.7.2** (the version used to test the full demo).

The addon supports Godot 4.2 and later 4.x releases. The full demo and test suite use Godot 4.7.2. Mobile projects also need platform export tools and signing.

## Connect two instances

1. Run two instances of the demo.
2. In the first instance, select the file video sender and open a local video with the native file dialog.
3. Start sending, then copy the connection code.
4. In the second instance, select the receiver. Use `127.0.0.1` and the same port for this local test.
5. Paste the connection code and connect. Both instances display the video.
6. Stop the session when you finish.

For a LAN connection, listen on `0.0.0.0` and use the sender's LAN address on the receiver. Allow the selected UDP port through the firewall. A new sending session generates a new connection code.

<!-- capture: file-sender | Sender preview with the connection code hidden -->

<!-- capture: file-receiver | Received video and frame statistics -->

## Install in your own project

Download the **addon-only** ZIP from the same release. Copy its entire `addons/kyber` directory into your project, including `bin/` and `licenses/`. Open the project again so Godot loads the GDExtension. You don't need to enable an editor plugin.

```text
my-project/
├── project.godot
└── addons/
    └── kyber/
        ├── kyber.gdextension
        ├── icon.svg
        ├── bin/
        └── licenses/
```

## Choose a Godot API

| Task | Start here |
| --- | --- |
| Play received video | `VideoStreamPlayer` with `KyberVideoStream` |
| Share a connection and inspect its status | `KyberSession` |
| Play received audio | `AudioStreamPlayer` with `KyberAudioStream` |
| Send a Viewport | `KyberFileVideo.start_viewport_sender()` |
| Exchange Godot RPCs | `KyberMultiplayerPeer` |

Read the [video guide](../../docs/video.md), [audio guide](../../docs/audio.md), and [RPC guide](../../docs/multiplayer.md) for complete examples and limits.

## Check platform requirements

Windows and Apple platforms use system media frameworks. Android uses MediaCodec. Linux requires GStreamer and the appropriate codec plugins. No separate Kyber SDK installation is needed. Web is not supported.

See [features and limitations](../../docs/overview.md) and the [build guide](../../docs/development.md).

## Before distributing

Original project code is MIT-licensed. Public binaries combine it with the Kyber SDK and are distributed under AGPL-3.0-or-later. A qualifying Kyber SAS commercial agreement can provide an alternative for covered Kyber components. Other dependencies retain their terms.

Read the [binary licensing guide](../../BINARY_LICENSE.md) before redistributing the addon or demo.
