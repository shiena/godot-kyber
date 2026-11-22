<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Features and limitations

[Documentation](README.md) | [譌･譛ｬ隱枉(overview_ja.md)

This guide distinguishes addon features from upstream Kyber capabilities that are not yet integrated.

## Features and limitations

- Standard `VideoStreamPlayer` reception through `KyberVideoStream`
- Shared connections, input, detailed status, and statistics through `KyberSession`
- Video file transmission, reception, sender preview, stop, and loop playback through `KyberFileVideo`
- Godot RPC through `KyberMultiplayerPeer`, a `MultiplayerPeerExtension` backed by KyProto
- Connection management and state signals through `KyberClient`
- Basic authentication, TLS verification with system CAs, and certificate fingerprint pinning
- Display discovery and H.264/Kymux stream requests
- Texture updates from software-decoded RGBA frames
- USB HID keyboard input, relative mouse movement, mouse buttons and wheel, and standard gamepad buttons and axes
- Viewport capture, multi-client video broadcasting, local performance statistics, and opt-in text clipboard exchange
- A local display demo that doesn't require an external SDK runtime

Video-only consumers retain the latest frame. Audio playback uses a bounded timestamped video queue. Network threads don't call Godot APIs. Texture updates and signal emission run on the main thread.

Most video paths involve CPU copies and GPU uploads. Windows D3D12 H.264 reception supports GPU transfer; see [H.264](h264.md) for its scope. Touch, gamepad rumble, and OIDC aren't implemented. Windows, macOS, Linux, Android, and iOS file senders support Opus audio with synchronized Godot video playback. Godot bus/microphone capture and `KyberAudioStream` are available; existing-host Opus audio requests are implemented, with full host interoperability still unverified. See [live audio](audio.md).

## Platform media backends

The shared media interface separates `VideoDecoder`, `VideoSource`, encoded packets, and RGBA frames from Godot and Kyber networking.
Windows uses Media Foundation for H.264 decoding and file reading. File and Viewport streams offer optional H.264 encoding and acceleration selection. D3D12 reception can avoid CPU readback; Compatibility retains CPU transfer. Encoding still accepts CPU frames. See [H.264](h264.md).
Linux x86_64 uses system GStreamer 1.20 or later for H.264 decoding and file reading. H.264 reception requires `h264parse` and either `avdec_h264` or `openh264dec`. File support depends on installed demuxers and decoders. File audio also requires `libgstpbutils-1.0`, `audioconvert`, `audioresample`, and a decoder for the audio track. GStreamer is loaded on demand; unavailable libraries or plugins leave RPC and JPEG reception usable.
macOS arm64 uses VideoToolbox for H.264 decoding and AVFoundation for video files. The SDK is embedded in one dylib; system frameworks are not bundled. Frames are copied to RGBA on the CPU. Intel Macs are unverified. File audio uses AVFoundation to decode stereo PCM at 48 kHz. The H.264 option supports VideoToolbox acceleration selection. GPU sharing is not implemented.
Android arm64 uses system MediaCodec, MediaExtractor, and ImageReader (API 24 or later). H.264 reception, file video, and RPC use the same Godot APIs as desktop platforms. YUV420 frames are copied to RGBA; device codec support determines available file formats. Hardware decoding falls back to Android software H.264 codecs when configuration fails. HDR/BT.2020, background streaming, and direct GPU sharing are unsupported. The native file picker in the demo copies content URIs into app storage; it needs space for one selected video. File audio supports mono/stereo tracks decoded to PCM16 or float PCM and resamples to stereo 48 kHz. Android 14 is tested; other Android versions are unverified.
iOS arm64 uses AVFoundation and VideoToolbox through a shared Apple backend. The SDK is embedded in `GodotKyber.framework`; Apple frameworks remain OS dependencies. File audio uses the same AVFoundation PCM path as macOS. The deployment target is iOS 15.0; iPhone 15 Pro with iOS 27.2 is tested. The demo uses a native UIKit document picker and copies the selected movie into the app cache. Simulator slices, background streaming, and direct GPU sharing are not implemented. App Store distribution and other iOS versions are unverified. Web is outside the target scope.
`get_media_capabilities()` on `KyberSession` or `KyberFileVideo` reports backend availability. A missing decoder does not disable JPEG reception or RPC.
VLC and FFmpeg are not bundled. System codecs reduce redistributed dependencies; they do not establish patent clearance for every application.

## Kyber capabilities not yet integrated

This comparison was checked on November 21, 2026 against official documentation, release notes through 0.29, and the pinned SDK. It describes integration gaps, not a committed roadmap. Upstream availability depends on the platform, backend, and SDK version. A host-side capability can work independently without having a corresponding Godot API. Existing-host interoperability remains unverified.

| Capability | Upstream Kyber | Current Godot integration |
|---|---|---|
| Video codecs | The client SDK defines H.264, HEVC, and AV1 options. [Client SDK](https://gitlab.com/kyber/core/kyctl/-/blob/92c632f9e889917a34aaa7697fb88b4991054b1c/kyclient/src/lib.rs) | Existing-host requests select H.264. Codec selection isn't exposed. The file demo offers JPEG or H.264 through a separate application protocol. HEVC and AV1 remain unsupported. |
| Capture and hardware encoding | Server backends include NVENC and x264, with experimental AMF, QSV, and VAAPI options. [Server configuration](https://docs.kyber.tech/docs/GettingStarted/KyberServerConfig/) | Viewport capture is available through `KyberFileVideo.start_viewport_sender()`. It uses CPU readback with JPEG or optional H.264 encoding. H.264 acceleration depends on the platform and installed codecs; see [H.264](h264.md). It does not implement a full Kyber host. Hardware encoding on an external host is separate from GPU texture sharing in Godot. |
| Audio | Kyber carries audio; the SDK includes playback and Opus support. [0.29 release notes](https://kyber.tech/article/v-029/) | Windows, macOS, Linux, Android, and iOS file audio use system PCM decoding, statically linked Opus, KyProto audio endpoints, and kyaudioreg. Godot VideoStreamPlayer handles playback. Bus/microphone capture and a standalone KyberAudioStream resource are supported. Existing-host Opus audio reception is implemented; full host interoperability remains unverified. |
| Extended input | Gamepads are supported; the input SDK also defines mouse wheel events. [Windows setup](https://docs.kyber.tech/docs/GettingStarted/Kyber-installation-notes-windows/), [input SDK](https://gitlab.com/kyber/core/kynput) | Keyboard, mouse movement, three buttons, wheel, and standard gamepad buttons and axes are forwarded through SDK packets. `KyberInputRelay` supports SDK rumble reception and touch-to-mouse conversion. Godot peers exchange multitouch, paddle buttons, and rumble through standard RPC. Native host multitouch/paddles remain unsupported. See [extended input](extensions.md#forward-extended-input). |
| Clipboard | Official clients support clipboard exchange with Windows and X11 hosts. [0.27 release notes](https://kyber.tech/article/v-027/) | Opt-in text exchange uses upstream clipboard packets and Godot `DisplayServer`. Reads, requests, and local writes are explicit. HTML, images, and automatic synchronization are unsupported. See [extended features](extensions.md). |
| OIDC and session renewal | Official clients support OIDC; 0.29 adds token exchange and renewal. [0.26](https://kyber.tech/article/v-026/), [0.29](https://kyber.tech/article/v-029/) | Host access exposes Basic credentials and TLS options. OIDC login and token lifecycle controls aren't exposed. Demo connection codes are a separate authentication mechanism. |
| Multiple remote viewers | A host can distribute a shared encoded stream to multiple clients. [0.27 release notes](https://kyber.tech/article/v-027/) | File and Viewport sending support up to 16 concurrent receiver/handshake tasks with independent writers and shared encoding. RPC still supports one remote client. Multiple local players sharing `KyberSession` are a separate feature. Joining an upstream multi-client host hasn't been verified. |
| Automatic reconnection | Official clients provide automatic retry controls. [0.27 release notes](https://kyber.tech/article/v-027/) | Partially integrated: SDK host sessions can enable official SDK reconnection through `auto_reconnect`. File transfer and RPC are excluded. See the [SDK reconnection guide](video.md#enable-kyber-sdk-reconnection). |
| Detailed latency metrics | The web client displays per-stage latency measurements. [0.27 release notes](https://kyber.tech/article/v-027/) | Local stage durations, frame intervals, queue delays, and estimated source-to-ready age are exposed. File/Viewport reception uses official KyProto clock synchronization. Native-host stage timings and physical display latency are unavailable. |
| Alternative transports | Native WebSocket support is documented for 0.29. [0.29 changelog](https://docs.kyber.tech/blog/) | No WebSocket selection or Godot Web export integration. The custom file and RPC paths use QUIC. |
| Host management | Upstream adds availability heartbeats and desktop `kyber://` links. [0.29 release notes](https://kyber.tech/article/v-029/) | No agent management or URL handler. The management portal is described upstream as future work, not an available addon feature. |
| Virtual displays | Windows host configuration includes virtual monitor settings, requiring a driver described as not yet public. [Server configuration](https://docs.kyber.tech/docs/GettingStarted/KyberServerConfig/) | Existing displays can be listed and selected; creating virtual displays or managing the driver isn't implemented. |

The [technology overview](https://kyber.tech/technology/) also describes touch/gesture input, synchronized sensor telemetry and metadata, adaptive quality, and data-channel uses such as USB forwarding. This addon doesn't expose these integrations or configuration controls. KyProto Unreliable and RaptorQ FEC are available for file/Viewport/live audio streams through `transport_mode`; RPC remains Reliable. These product-level descriptions don't establish that every feature is available through the pinned public SDK. Godot RPC provides application messages, not a ready-made USB, file-transfer, or telemetry integration.

The file/Viewport path keeps the latest compressed frame when decoding falls behind. Clock synchronization supplies a shared time basis and stale-frame rejection; it does not guarantee simultaneous presentation across devices. See [timing and clipboard details](extensions.md).

## Reference implementation and license

Godot integration and documentation organization draw on [godot-ndi](https://github.com/unvermuthet/godot-ndi). Kyber-specific control and detailed status remain accessible through the shared session.

The architecture draws on [Kyber for Unity](https://gitlab.com/mfkl/kyber-unity). Its D3D11 sender implementation wasn't reused. The existing-host receiver uses `kyclient` from the [Kyber SDK](https://gitlab.com/kyber/core/kysdk).

Original project files are MIT-licensed. SDK-derived definitions are extracted at build time into the ignored SDK tree and retain their upstream licenses, and native binaries built with the public SDK are combined AGPL works. An applicable Kyber commercial agreement may provide an alternative for covered Kyber material; this project grants no commercial rights. See [license scope](../NOTICE.md). Other dependencies retain their own licenses.
