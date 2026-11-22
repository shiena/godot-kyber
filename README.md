<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# godot-kyber

English | [Japanese](README_ja.md)

> Integrates [Kyber](https://kyber.tech/) video transport and multiplayer RPC with Godot.

Receive video with a standard `VideoStreamPlayer` using `KyberVideoStream`. Add a `KyberSession` node to share the connection with input controls, detailed status, and statistics. Use `KyberMultiplayerPeer` with Godot's multiplayer API for RPC.

The [demo project](https://github.com/shiena/godot-kyber/tree/main/demo) includes file audio/video streaming, Viewport transmission, an existing-host receiver, and an RPC chat. Video reception keeps its aspect ratio and reports connection status independently of display pause.

## Install

- The extension targets Godot 4.2. The full demo and test suite use Godot 4.7.2.
- Windows/Linux x86_64, macOS arm64, Android arm64, and iOS arm64 have native media backends. Web is not a target.
- Windows video reception and file sending use Media Foundation. No FFmpeg executable is required.
- macOS uses VideoToolbox and AVFoundation from the operating system.
- iOS uses the same Apple media frameworks. See the [iPhone build and test guide](docs/development.md#build-and-test-on-iphone).
- Android uses system MediaCodec and MediaExtractor. See the [Android build and device test guide](docs/development.md#build-and-test-on-android).
- Linux uses system GStreamer and its codec plugins. See the [Linux setup](docs/development.md#build-on-linux).
- The Kyber SDK is embedded. No VLC, FFmpeg libraries, SDK installation, or library path is required. Real-host interoperability remains unverified.

Build from source using the instructions below, then copy `demo/addons/kyber` into your project's `addons/kyber` directory. You don't need to enable an editor plugin.

The Release workflow produces a five-platform addon ZIP and a demo ZIP. See [release packaging](docs/development.md#publish-addon-and-demo-packages). Mobile use still requires platform export tools and signing.

For a prebuilt Windows DLL, download the `godot-kyber-windows-x86_64` artifact from a successful [GitHub Actions run](https://github.com/shiena/godot-kyber/actions/workflows/ci.yaml). Extract the distribution ZIP and copy its `addons/kyber` directory into your project.

## Try the demo

Open `demo/project.godot` after building the extension. Run two instances to send a video file or exchange RPC messages. The video dialog uses the operating system's native file picker.

For a preview without a network connection, open the **Kyber host receiver** tab and click **Local display demo**.

See the [video guide](docs/video.md) and [RPC guide](docs/multiplayer.md) for connection steps.

## Documentation

Start with the [documentation index](docs/README.md):

- [Video playback, shared sessions, and input](docs/video.md)
- [Godot multiplayer RPC](docs/multiplayer.md)
- [Existing Kyber hosts](docs/host.md)
- [Supported features and limitations](docs/overview.md)
- [Build, test, CI, and packaging](docs/development.md)

Viewport transmission and multi-client video are available. See [capture, input, and performance](docs/extensions.md). Windows, macOS, Linux, Android, and iOS file streaming includes synchronized Opus audio through Godot's video player. [Live audio](docs/audio.md) supports Godot buses, microphones, and `KyberAudioStream`. Video/audio transport can use Reliable, Unreliable, or FEC. RPC currently supports one client, reliable delivery, and channel 0. See the guides for the full limits.

## Build from source

For Windows source builds, prepare the build tools in the [development guide](docs/development.md). From the repository root, run:

```powershell
pwsh -File scripts/build.ps1 -Release
```

This fetches the pinned SDK, compiles the SDK and Windows media adapter, and installs the complete add-on into the demo project. Follow the [development guide](docs/development.md) to run tests or create a distribution ZIP.

For Linux, run `bash scripts/build.sh --release` after installing the dependencies in the [Linux setup guide](docs/development.md#build-on-linux).

For macOS, use the same build command after following the [macOS setup guide](docs/development.md#build-on-macos).

## License

Original project code, including SDK extraction scripts, is available under [MIT](LICENSE).
Build preparation extracts compatibility types from the pinned Kyber SDK into
the ignored build tree. The SDK and generated modules retain their upstream licenses.
File headers and [REUSE.toml](REUSE.toml) identify individual file licenses.

Native binaries built with the public Kyber SDK are combined works distributed
under AGPL-3.0-or-later. Distributors must provide the complete Corresponding
Source and retain all applicable notices. The MIT license for original files
does not make the combined binary MIT-licensed.

If you hold a [Kyber commercial license](https://kyber.tech/), you may use the
covered Kyber material under that agreement, subject to its permissions for
modification, building, and redistribution. This project does not grant Kyber
commercial rights. Other dependencies retain their respective licenses.
See [native addon licensing](BINARY_LICENSE.md) for the AGPL and commercial
options, and [license scope and third-party notices](NOTICE.md) for details.
