<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Live audio and transports

[Documentation](README.md) | [日本語](audio_ja.md)

## Choose an audio source

| Source | Use this path |
| --- | --- |
| Video file soundtrack | Start the file sender; the file must contain an audio track |
| Game audio or generated sound | Capture a dedicated Godot audio bus |
| Microphone | Enable audio input and configure platform recording permissions |

Use the same `KyberSession` for received video and audio when they belong to one connection. Keep the session referenced while both players are active. Microphone permissions are not needed for file audio or reception.

## Send a Godot audio bus

```gdscript
var sender := KyberFileVideo.new()
add_child(sender)
if sender.start_audio_sender("0.0.0.0", 4433):
    sender.capture_audio_bus("Music")
```

Create the bus before capture. To send video too, call `start_viewport_sender()` instead of `start_audio_sender()`, then attach the bus. Read `get_connection_code()` once available and give it to receivers. File senders use their file track instead of live capture.

Capture appends an `AudioEffectCapture` to the bus. It converts stereo PCM to 48 kHz using stateful linear interpolation and encodes 20 ms Opus packets on the worker. Video and audio use the KyProto clock. Stop with `stop_audio_capture()` or stop the sender. The addon removes its own effect; bus reordering is supported. Removing the captured bus/effect stops the sender with an error. Capture queues are bounded; overflow skips old audio rather than accumulating latency. No receiver means captured samples are discarded.

Use a dedicated source bus when the same app also receives audio, to avoid sending received sound back. Capture doesn't add local monitoring. Bus capture and playback use Godot APIs shared across the five target platforms; the new live path is automated on desktop, not yet verified with physical mobile microphones.

<!-- capture: audio-bus-en | Godot Audio bus layout showing the dedicated Music bus used by the example. -->

## Send a microphone

After starting an audio-only or Viewport sender, call `capture_microphone()` instead of `capture_audio_bus()`. The addon uses `AudioStreamMicrophone`, an internal muted bus, and the same capture path. It doesn't provide echo cancellation or noise suppression.

Enable `audio/driver/enable_input` in Project Settings before starting the app. The application owns microphone permission requests and export settings; the addon doesn't prompt or change project settings. A successful capture call confirms setup, not permission or non-silent input. Godot supports microphone recording on Windows, macOS, Linux, Android, and iOS. On iOS, also select **Record** or **Play and Record** for the audio session category. See [Godot microphone setup](https://docs.godotengine.org/en/stable/tutorials/audio/recording_with_microphone.html) and [input permissions](https://docs.godotengine.org/en/4.2/classes/class_projectsettings.html#class-projectsettings-property-audio-driver-enable-input).

Desktop privacy settings and the selected input device must permit recording. Mobile exports need the corresponding microphone permission/usage description and runtime authorization. A headless dummy audio driver cannot validate a physical microphone.

### Configure macOS microphone access

Enable **Project Settings > Audio > Driver > Enable Input** (`audio/driver/enable_input`) before launching the app.

1. Open **Project > Export** and select the macOS preset.
2. Enable **Codesign > Entitlements > Audio Input** (`codesign/entitlements/audio_input`).
3. Set **Privacy > Microphone Usage Description** (`privacy/microphone_usage_description`) to an explanation such as `Send microphone audio to the connected receiver.`
4. Export and sign the app with the updated preset.
5. Launch the exported app and allow microphone access when macOS prompts.

For translated permission text, use `privacy/microphone_usage_description_localized`. If access was denied, enable the app under **System Settings > Privacy & Security > Microphone**, then restart it. Test the exported app as well as the editor: they have separate permission identities. See [Godot macOS export settings](https://docs.godotengine.org/en/stable/classes/class_editorexportplatformmacos.html).

### Configure iOS microphone access

1. Enable **Project Settings > Audio > Driver > Enable Input** (`audio/driver/enable_input`).
2. Turn on **Advanced Settings** in Project Settings.
3. Set **Audio > General > iOS > Session Category** (`audio/general/ios/session_category`) to **Play and Record** for simultaneous playback and capture. Use **Record** only for recording without playback.
4. In the iOS export preset, set **Privacy > Microphone Usage Description** (`privacy/microphone_usage_description`) to explain why audio is captured and sent.
5. Export the updated project and rebuild/install it through Xcode.
6. Allow microphone access when iOS prompts on the device.

Use `privacy/microphone_usage_description_localized` for translated permission text. If access was denied, enable the app's microphone permission in iOS Settings and restart it. Changing the export description requires a new export and build. See [Godot iOS export settings](https://docs.godotengine.org/en/stable/classes/class_editorexportplatformios.html) and [Godot microphone recording](https://docs.godotengine.org/en/stable/tutorials/audio/recording_with_microphone.html).

### Verify microphone transmission

Start a live sender, call `capture_microphone()`, and connect a receiver. Speak into the selected microphone and check the receiver's audio output. Use headphones to avoid acoustic feedback. If output is silent, check OS permission, input-device selection, and the settings for your platform. `capture_microphone()` returning `true` alone doesn't verify microphone access.

These settings apply to microphone capture. Bus capture of generated/game audio, file-track transmission, and audio reception don't require microphone permission. The addon doesn't change these settings automatically. Physical microphone capture on macOS/iOS remains unverified for this implementation.

## Play through AudioStreamPlayer

```gdscript
var stream := KyberAudioStream.new()
stream.host = "192.168.0.10"
stream.port = 4433
stream.set_connection_code(connection_code)
$AudioStreamPlayer.stream = stream
$AudioStreamPlayer.play()
```

The resource creates its own receiver. `AudioStreamPlayer`, `AudioStreamPlayer2D`, and `AudioStreamPlayer3D` can use the standard AudioStream interface for volume, routing, and spatial playback. Stop disconnects a standalone receiver; play creates a new connection. Inspect `stream.get_last_error()` for failures. The connection code is runtime-only and isn't saved in the resource. Seeking and automatic reconnection aren't supported.

To share reception with video:

```gdscript
session.start_receiver(host, port, connection_code)
var audio := KyberAudioStream.new()
audio.set_session(session)
$AudioStreamPlayer.stream = audio
var video := KyberVideoStream.new()
video.audio_enabled = false
video.set_session(session)
$VideoStreamPlayer.stream = video
$AudioStreamPlayer.play()
$VideoStreamPlayer.play()
```

Call `set_session()` after starting the receiver, before playback. It snapshots that connection's audio mailbox; reattach after restarting the session. There is one audio consumer per session. A second audio consumer is rejected instead of splitting PCM between players. Disable video resource audio as shown when using a separate audio player. Video follows the estimated audio playback position. Stopping the audio player doesn't close the shared session; close it explicitly. Pause doesn't stop network reception and old audio is bounded. Separate audio/video player pause controls are independent; pause both when needed.

The audio mixer pulls through Kyber's `kyaudioreg` and Godot's `AudioStreamPlaybackResampled`. It doesn't access scene nodes from the audio thread. Existing-host Opus audio uses the same playback path; see the host section in this guide.

## Receive audio from an existing Kyber host

Connect a `KyberSession` to the host. After its `connected` signal, request the display with audio enabled:

```gdscript
session.start_stream(display_id, 1280, 720, 20000000, true)
var stream := KyberVideoStream.new()
stream.set_session(session)
$VideoStreamPlayer.stream = stream
$VideoStreamPlayer.play()
```

Use the shared `KyberAudioStream` example in this guide to route audio to a separate player. Attach it after `connect_to_host()` has created the session. Disable `KyberVideoStream.audio_enabled` when another player consumes its audio. A standalone `KyberVideoStream` with `source = KyberHost` uses `audio_enabled` to control the host request. A low-level `KyberClient` plays audio through its internal generator. Standalone `KyberAudioStream` connection codes still address the addon protocol; use `set_session()` for a host connection.

The SDK handles host authentication, endpoint negotiation, and reconnection. Its audio endpoint feeds the same Opus decoder and `kyaudioreg` used by addon streams. Host video PTS is retained for selection against the audio playback position. Audio is requested on the SDK Reliable endpoint; the file sender's `transport_mode` doesn't configure host streams. SDK volume/mute commands aren't used: control the Godot player instead.

Supported audio is Opus decoded to stereo 48 kHz, including mono/stereo `OpusHead` mapping family 0, pre-skip, and output gain. Packet durations from 2.5 to 120 ms are accepted. Other codecs and multichannel mappings report an error. Only audio accompanying a display request is supported. Audio-only host requests are not exposed. The host may reject an audio request if its audio backend is unavailable; use `false` for video-only reception.

The integration is implemented for the common desktop/mobile code path. Verification covers real KyProto audio endpoints through the SDK forwarding adapter, plus Godot playback and reconnection using an SDK callback fixture. It doesn't establish interoperability with a deployed Kyber controller. The pinned SDK README references a `kymux/examples` directory that isn't present in that revision. The [Kyber for Unity demo](https://gitlab.com/mfkl/kyber-unity/-/blob/464b6dd69b3f69cce5bf445f55e3c5784b548d7e/README.md) supplies a host-side Unity worker, controller, and official client; it isn't an embedded receiver demo. Running it as a Godot test host requires building that separate stack.

## Transport and protocol compatibility

Set `KyberFileVideo.transport_mode` before starting a sender:

| Value | Transport | Behavior |
|---|---|---|
| `0` | Reliable (default) | KyProto reliable video/audio; retransmission can add delay under loss. |
| `1` | Unreliable | KyProto datagrams; missing JPEG frames are skipped and audio holes use Opus packet-loss concealment. |
| `2` | Unreliable + FEC | KyProto RaptorQ recovery for video/audio, plus concealment for unrecovered audio. |

This selects the official SDK drivers, not a new FEC implementation. Recovery has bandwidth overhead and cannot recover every loss pattern. The choice applies to both audio and video; RPC retains its Reliable data endpoint. The file and Viewport demos expose this setting. Viewport also offers bus/microphone capture.

Reliable retains the existing `certificate_hash:token` code and revision 2 payload/endpoints (video 0, completion 2, clock 4, audio 6). Unreliable and FEC use `k3:1:certificate_hash:token` and `k3:2:certificate_hash:token`; receivers select the transport from the code. `k3` identifies the new code format, not an SDK ABI. Older receivers reject these new codes. Use Reliable for compatibility with the preceding revision 2 addon. Revision 1 video-only senders remain accepted. This application protocol isn't compatible with an official Kyber host.

`audio_dropped_packets` counts mailbox overwrites and missing audio packets reported by the transport. It doesn't count losses already recovered by FEC. Existing performance statistics remain available through `KyberSession` and `KyberFileVideo`.
