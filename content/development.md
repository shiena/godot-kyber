<!-- SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro -->
<!-- SPDX-License-Identifier: MIT -->

# Build, test, and package

[Documentation](README.md) | [日本語](development_ja.md)

Run commands from the repository root. Paths in code examples are relative to that root.

## When to use this guide

Use this guide when changing the addon, building native libraries, or preparing a release. To use a prebuilt addon or run the demo in Godot, follow [installation](../content/installation.md) and [video streaming](video.md).

## Choose a build target

| Target | Instructions |
| --- | --- |
| Windows | [Build and run locally](#run-the-local-display-demo) |
| Linux | [Linux build](#build-on-linux) |
| macOS | [macOS build](#build-on-macos) |
| Android | See the Android build and device-test section below |
| iOS | See the iOS build and device-test section below |

Use the matching platform toolchain. Mobile builds also need export tools and signing or device configuration. After changing native code, rebuild the library before running the demo or tests.

## Run the local display demo

Use Windows x86_64, Rust 1.94 or later, Visual Studio C++ build tools with the Windows SDK, Python 3, Git, CMake, and LLVM (libclang).
WSL and a separate codec build are not required. The extension retains the Godot 4.2 API baseline; the full suite runs on Godot 4.7.2.


1. Build from the repository root:

   ```powershell
   pwsh -File scripts/build.ps1
   ```

2. Open `demo/project.godot` in Godot.
3. Run the project.
4. In the **Existing Kyber server** tab, click **Local display demo**.

An animated gradient appears without using the network. The build script places the DLL at `demo/addons/kyber/bin/godot_kyber.dll`. To use the extension in another project, copy `demo/addons/kyber` to that project's `addons/kyber` directory. You don't need to enable an editor plugin.

## Build on Linux

Use Ubuntu 24.04 x86_64, Rust 1.96.0, and Python 3.11 or later. WSL Ubuntu can build Linux binaries for deployment to another Linux computer.

```bash
sudo bash scripts/install_linux_deps.sh
bash scripts/build.sh --release
```

The installer adds compiler tools, SDK dependencies, GStreamer development packages, and test codecs. The build embeds the SDK in `libgodot_kyber.so`.
On the destination, install GStreamer 1.20 or later, its app/video/pbutils libraries, container plugins, and the required decoders. The addon doesn't bundle these system components.
H.264 reception uses `h264parse` with `avdec_h264` or `openh264dec`. File reading uses `uridecodebin` and installed plugins. Missing codecs are reported through `get_media_capabilities()`.
RPC and JPEG reception don't require GStreamer. File audio requires audio decoders, `audioconvert`, and `audioresample`. See [H.264](h264.md) for acceleration options. GPU sharing is not implemented.

Run the Linux suite and create a tested distribution:

```bash
python3 scripts/install_godot.py --destination "$HOME/.local/godot/4.7.2-stable"
export GODOT="$HOME/.local/godot/4.7.2-stable/Godot_v4.7.2-stable_linux.x86_64"
bash scripts/test_linux.sh
python3 scripts/package_linux.py
```

The output is `target/packages/godot-kyber-<version>-linux-x86_64.tar.gz`. Copy the packaged `addons/kyber` directory into your project.
FFmpeg generates test fixtures only. The Linux backend can use a system GStreamer plugin backed by FFmpeg; that plugin is not included in the package.
For limited disk space, set `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, and `CARGO_INCREMENTAL=0` before running the suite.

Validation on November 22, 2026 used WSL Ubuntu 24.04 and Steam Deck with SteamOS 3.8.28, GStreamer 1.26.4, and Godot 4.7.2.
The initial Deck video verification passed five native media tests, 44 GUT tests with 391 assertions, and separate-process video and RPC tests.
The native SDK build also passed startup/failure/shutdown checks and rendered sent and received video in two Wayland windows. That earlier video test did not exercise audio. See the file audio validation below.
These checks don't verify an existing Kyber host, the Steam gaming session, or native file-picker interaction. The demo retains `use_native_dialog = true`.
Packages built on Ubuntu 24.04 target its system ABI; older distributions are not verified.

## Build on macOS

Use an Apple Silicon Mac, Xcode command-line tools, Rust 1.96.0, Python 3.11 or later, CMake, and Git. Run `bash scripts/build.sh --release` from the repository root.
The build embeds the SDK in `libgodot_kyber.dylib` and links Apple system frameworks. No codec package is required at runtime.
If Rust was installed through Homebrew, add its command directory to your shell's `PATH`:

```bash
export PATH="$(brew --prefix rustup)/bin:$PATH"
```

For tests, install FFmpeg to generate fixtures, then run:

```bash
export GODOT="$HOME/.local/godot/4.7.2-stable/Godot.app/Contents/MacOS/Godot"
bash scripts/test_macos.sh
python3 scripts/package_macos.py
```

The distribution is `target/packages/godot-kyber-<version>-macos-arm64.tar.gz`. It contains a single-architecture library, not a universal binary.
The package is for development integration; signing and notarizing an exported application remain the application's responsibility. Intel Macs are unverified.
H.264 reception uses VideoToolbox. File reading uses AVFoundation and supports formats available through the OS. File audio uses AVFoundation; zero-copy GPU output is not implemented.
The Cargo configuration disables Rust's Mach-O stripping on macOS to avoid the [Rust 1.96 LINKEDIT alignment bug](https://github.com/rust-lang/rust/issues/157750).

Validation on November 22, 2026 used an arm64 Mac with macOS 27.2.1 and Godot 4.7.2.
The initial video verification passed formatting, Clippy, eight Rust tests, 44 GUT tests with 391 assertions, separate-process video/RPC, and native SDK startup/failure/shutdown. See the later audio validation for the expanded suite.
Windows-to-Mac and Mac-to-Windows video transfer were also visually confirmed with the demo running on both computers.
This verifies the demo's JPEG-over-QUIC protocol and OS file readers, not interoperability with an existing Kyber host.

## Run tests

`demo/project.godot` supports manual checks and automated tests. It includes [GUT 9.6.1](https://github.com/bitwes/Gut/tree/v9.6.1).

### Run the full suite

Install FFmpeg before running the tests. On Windows, close this project's Godot editor and running game instances before rebuilding.

From the repository root, run:

```powershell
pwsh -File scripts/test.ps1
```

The script defaults to the console executable in `%USERPROFILE%/.local/godot/4.7.2-stable`. Override it with the `GODOT` environment variable or `-Godot`. The parameter accepts an executable path or a Godot 4.7.2 installation directory:

```powershell
pwsh -File scripts/test.ps1 -Godot "C:/Tools/Godot/4.7.2-stable"
```

The script checks Rust formatting, runs Clippy, and runs Rust tests, including real Media Foundation file reading and H.264 decoding. It then builds the DLL and runs GUT headlessly. Results are written to `target/test-results/gut.xml`. Failures, skipped tests, missing tests, or an incomplete test count produce exit code `1`.

### Run tests in the editor

1. Run `pwsh -File scripts/test.ps1 -PrepareOnly` to prepare the test extension. Skip this step if you've already run the full suite.
2. Open `demo/project.godot` in Godot 4.7.2.
3. Open `tests/test_runner.tscn`.
4. Press F6 to run the current scene.

The GUT interface displays the results. Press F8 to stop. This scene uses the same `.gutconfig.json` as the command-line runner.

You can also use the **GUT** panel at the bottom of the editor. Add `res://tests/unit` and `res://tests/integration` to its settings on first use. GUT saves these settings in the user data directory.

### Test coverage

| Tests | Count | Coverage |
|---|---:|---|
| Godot API | 8 | Initial state, local video, texture reuse, disconnect, errors, and input rejection |
| Demo scenes | 2 | TextureRect and 3D material updates and clearing on disconnect |
| SDK bridge integration | 8 | Connection events, pixel values, Godot input events, input ranges, five reconnects, and worker shutdown |
| Video file transfer | 5 | Pixel values over real QUIC, end of file, authentication rejection and reconnection, invalid settings, and mode switching |
| Multiplayer peer | 4 | Packet routing and limits, SceneMultiplayer RPC, connection signals, authentication rejection, refusal, and shutdown |
| Standard video player | 6 | Pause/resume, texture identity, resource persistence, credential exclusion, SDK fixture, validation, network pixels, and finished signal |
| Shared session | 14 | Independent players, input on the same SDK connection, callback reentry, error recovery, node lifetime, detailed demo status, aspect ratio, SDK reconnect callbacks, stream restoration, cancellation, extended input, clipboard, and performance statistics |

All 47 GUT tests passed on Godot 4.7.2. Video tests send an FFmpeg-generated red video over a real KyProto/QUIC connection and verify the received texture's pixels. After GUT, the script launches separate Godot processes to verify video transfer into `VideoStreamPlayer`, end-of-file handling, and RPC round trips. Reports are saved to `target/video-processes` and `target/rpc-processes`.

SDK integration tests use a statically linked fixture in a separate `mock-sdk` build. They don't verify communication with an existing Kyber host. Missing fixtures or FFmpeg cause failures rather than skipped tests.

GUT is a development dependency. Copy only `addons/kyber` when integrating the extension into another game.
## GitHub Actions

`.github/workflows/ci.yaml` runs on pushes to `main`, pull requests, and manual dispatches.

| Job | Tasks |
|---|---|
| Rust formatting | Runs root/media formatting checks and release packaging tests on Ubuntu. |
| Windows build and Godot tests | Runs Clippy, Rust tests, native media tests, and GUT with the release DLL. Also checks audio/video loopback and exported games, then creates a distribution ZIP. |
| Linux build and Godot tests | Runs Clippy, Rust and GStreamer tests, GUT, separate-process video and RPC tests, and native SDK startup/shutdown. Also checks audio/video loopback and packages the tested shared library. |
| macOS build and Godot tests | Runs native media, GUT, video, and RPC tests on an Apple Silicon runner plus audio/video loopback and packages the tested dylib. |
| Android arm64 cross-build | Builds the release addon and native device test executables; no physical-device execution. |
| iOS arm64 cross-build | Builds the release framework; no signing for distribution or physical-device execution. |

CI pins Rust to 1.96.0 and Godot to 4.7.2-stable. It downloads Godot from the official release location and verifies SHA-512 checksums. FFmpeg is installed through Chocolatey on Windows, apt on Linux, and Homebrew on macOS. CI builds the SDK and Media Foundation adapter on Windows. FFmpeg only generates test fixtures; it is not used by the addon. Connections to existing hosts remain unverified.

The following artifacts are retained for 14 days:

- `godot-kyber-windows-x86_64`: A ZIP containing the tested DLL, GDExtension configuration, license, and documentation. It includes one extension DLL with the embedded SDK and dependency license notices.
- `windows-test-results`: GUT JUnit XML and execution logs. Available results are uploaded even when tests fail.
- `godot-kyber-linux-x86_64`: A `.tar.gz` containing the tested Linux extension, configuration, license notices, and documentation.
- `linux-test-results`: Linux GUT reports and build, video, and RPC logs.

- `godot-kyber-macos-arm64`: The tested macOS addon archive.
- `macos-test-results`: macOS test reports and logs.
- `godot-kyber-android-arm64-build`: Android addon directory with dependency notices.
- `godot-kyber-ios-arm64-build`: iPhoneOS framework addon directory with dependency notices.

To create the distribution ZIP locally, run:

```powershell
pwsh -File scripts/test.ps1 -Release
pwsh -File scripts/package.ps1
```

The output is `target/packages/godot-kyber-<version>-windows-x86_64.zip`. The CI workflow doesn't publish GitHub Releases or change version numbers.

### Publish addon and demo packages

The separate **Release** workflow follows godot-servo's package layout. It builds optimized addons for Windows/Linux x86_64 and macOS/Android/iOS arm64, then combines all five platforms into two ZIP files:

- `godot-kyber-v<version>.zip`: Starts at `addons/kyber/`, ready to extract into an existing project. Includes the project license and dependency notices.
- `godot-kyber-demo-v<version>.zip`: Contains a `godot-kyber-demo-v<version>/` project folder with the same addon, demo scenes, and documentation.

The release flow builds and packages addons; it doesn't export APKs, IPAs, or desktop applications, or run device tests. Normal CI retains its test jobs. An absent platform binary, incorrect architecture, mock build, or checksum mismatch fails packaging.

Update the crate version and lockfile before starting a release. In GitHub Actions, run **Release** with the matching version, such as `v0.1.0`. After all builds succeed, the workflow publishes both ZIPs and tags the source commit. It rejects an existing tag and doesn't create version-bump commits.

To assemble downloaded build artifacts locally, use Python 3.11 or later:

```sh
python3 scripts/package_release.py --staged staged --version v0.1.0
```

The `staged` directory must contain `addon-windows`, `addon-linux`, `addon-macos`, `addon-android`, and `addon-ios`, each holding the built addon contents. Output goes to `target/packages/release`. Mobile export still requires the platform SDK, Godot export templates, and signing configuration. For iOS, select the Mobile renderer and a physical arm64 device; the package doesn't contain simulator slices.

## Native build tools

Install Visual Studio C++ build tools with the Windows SDK, Rust, Python, Git, CMake, and LLVM.
Run `scripts/build.ps1 -Release` to fetch and patch the pinned SDK, compile the extension, and install it into the demo.
The build statically links the SDK and C/C++ runtime. Media Foundation is resolved from Windows system libraries.
No VLC, FFmpeg, MinGW runtime, or external SDK DLL is packaged.

Run `python scripts/prepare_sdk.py` before direct Cargo commands on a fresh checkout.

SDK preparation extracts player compatibility types from immutable, hash-checked
Kyber sources and appends the MIT CPU adapter. Generated SDK code stays under
`.deps/` and retains upstream notices; do not copy it into tracked sources.
All platform build scripts run this preparation automatically. After changing
SDK extraction, run `python scripts/test_prepare_sdk.py` after preparation.
CI checks generation, provenance notices, and rejection of unexpected sources
before Rust formatting. Public-SDK binaries remain subject to AGPL requirements.

The export smoke test pre-registers the GDExtension before the first editor import
and waits for import completion. A frame-count exit can race initial scanning.
Broadcast tests generate frames independently of receiver setup and teardown so
synchronous disconnects cannot pause capture.

The shared backend lives in `media/`. Run `cargo fmt --manifest-path media/Cargo.toml --check` and `cargo clippy --manifest-path media/Cargo.toml --all-targets -- -D warnings` when changing it.

The full test script finishes with the native build installed. It verifies system decoding, SDK connection failure, and shutdown.
`-MockOnly` skips native SDK checks and cannot produce a distributable package. `-PrepareOnly` leaves the test build installed for GUT.
Run `scripts/build.ps1` to restore the native SDK build.

Install matching Windows export templates and run `pwsh -File scripts/test_export.ps1` to verify an exported game.
The test checks SDK startup, native file decoding, and the absence of bundled codec DLLs. CI runs this before packaging.
The package script checks the tested DLL hash. Copy the entire addon directory, including license notices.

Migration: remove the first `library` argument from `connect_to_host()` and remove `KyberVideoStream.sdk_library` assignments.
Remove the `ffmpeg` argument from `KyberFileVideo.start_sender()`. Set host streaming `audio` to `true` to receive Opus audio, or `false` for video only.
The SDK still owns automatic reconnection.

Godot polls `VideoStreamPlayback` from its audio thread even for silent video. The extension uses godot-rust's `experimental-threads` to serialize those queries.
Scene and texture operations remain on the main thread. First-time headless imports on Godot 4.7.2 allow editor frames before shutdown.

## Build and compatibility maintenance

These notes preserve development constraints from the commit history before it was consolidated.

### Decode Cargo metadata as UTF-8

Keep `encoding="utf-8"` on the `cargo metadata` subprocess in `scripts/install_addon.py`. Python's default Windows encoding previously failed to decode dependency metadata. This stopped Windows and Android package generation, even though compilation was not the failing step.

Use explicit UTF-8 decoding in new tooling that reads Cargo's JSON output. Don't rely on the machine's locale or discard undecodable characters. The installer uses this metadata to collect dependency licenses for distribution.

### Rebuild both sides of SDK interface changes

Update `sdk/` and `src/backend.rs` together when changing callback signatures or the internal SDK interface. Rebuild and restage the complete addon before testing or packaging.

The old reconnection commit required rebuilding a separate SDK DLL for bridge ABI 2. That instruction applied to the former dynamic-loading design. The current build embeds the SDK and has no external SDK path or public bridge ABI number. See the [SDK integration guide](sdk.md) for the current interface.

### Check sender and receiver protocol compatibility

The file-streaming audio change introduced application protocol revision 2. Its senders require updated receivers; current receivers also accept revision 1 video-only senders. A successful local rebuild doesn't update an already installed remote receiver.

Check both endpoint versions when diagnosing failures after deployment. Protocol revision compatibility and codec support are separate: revision 2 alone doesn't guarantee H.264 support. See [video streaming](video.md) and [H.264 compatibility](h264.md) before changing the wire format or selecting a codec.

## Build and test on Android

The Android target is arm64, API 24 or later. The desktop editor still needs its host-platform addon binary. The demo uses Godot 4.7.2; native Android file selection requires Android 10 or later.

On Windows, install Android SDK, NDK 27.2.12479018, JDK 17, CMake, Ninja, and matching Godot Android export templates. Configure the Android SDK and Java SDK paths in Godot's editor settings, then run:

```powershell
rustup target add aarch64-linux-android
./scripts/build_android.ps1 -BuildTests
```

Pass `-Ndk <directory>` for another NDK location and `-Release` for an optimized library. The build statically links the SDK and C++ runtime. Android system media libraries remain OS dependencies. ELF segments are aligned for 16 KB pages; this does not replace testing on a 16 KB device.

The script stages the addon, dependency notices, and build metadata in `target/android-addon/addons/kyber`. It also adds the Android binary to the demo without replacing the Windows binary. Export the demo using its **Android** preset with Internet permission enabled. For another project, include `addons/kyber/licenses/*` in the export include filter; Android GDExtension dependencies accept shared libraries, not license directories. Godot's Android debug keystore is for development only.

For a USB-connected device and a Windows machine on the same LAN:

```powershell
python scripts/test_android.py --adb <adb.exe> --serial <USB-serial> --godot <Godot.exe> --windows-ip <Windows-LAN-IP> --device-ip <Android-LAN-IP>
```

This installs a separate `org.godotengine.kybertest` APK. It requires a native Windows build in the demo and the generated MP4 fixture from the desktop test setup. The test checks native SDK startup/failure/shutdown, loopback video, video in both LAN directions, and RPC with Android as both server and client. Reports and logs are saved in `target/android-test`. Ports 14434 and 14435 must be reachable over UDP. The test app is stopped afterward.

Run the cross-compiled `native_media-*` test executable from `/data/local/tmp` with `KYBER_MEDIA_FIXTURES` pointing to a pushed copy of `tests/media`. It checks decoded colors, orientation, timestamps, all six frames, reset, resolution changes, and configuration replay. Test APK success is checked separately from process exit status.

The initial video verification on November 22, 2026 passed five native media tests and the Godot video/RPC tests on an Android 14 arm64 device. A real external Kyber host, Android versions other than 14, 16 KB devices and background operation remain unverified or unsupported as described in [Features and limitations](overview.md).

The Android CI job cross-compiles the release addon and device test executables. It does not run device tests; those require the ADB procedure above.

For visual inspection, add `--manual windows-to-android --video <video.mp4>` to the device test command. It leaves both applications running and records the Windows process ID under `target/manual-windows-to-android`. Stop that Windows process before switching to `--manual android-to-windows --skip-export`. Stop the Windows process and `adb -s <serial> shell am force-stop org.godotengine.kybertest` when finished. The selected video loops until stopped.

Windows ↔ Android loop video was also visually confirmed in both directions on November 22, 2026. The unoptimized Android sender was slow and the Windows receiver reported dropped frames; the later release-build audio tests passed in both directions. Sustained throughput remains unmeasured.

## Build and test on iPhone

Use an Apple Silicon Mac with Xcode, Rust, Python 3.11 or later, CMake, and Godot 4.7.2. Pair your iPhone with Xcode and enable Developer Mode. Signing requires an Apple development team and a valid development certificate.

```sh
python3 scripts/install_ios_templates.py
bash scripts/build_ios.sh
python3 scripts/export_ios.py --godot /path/to/Godot --team YOURTEAMID
```

The build embeds the SDK in `GodotKyber.framework`. The addon package is in `target/ios-addon/addons/kyber`. Pass `--release` to the build script for an optimized framework. This package contains only an arm64 iPhoneOS slice; iOS Simulator is unsupported.

Open `target/ios/kyberdemo/export/kyberdemo.xcodeproj` in Xcode. Select the connected physical iPhone as the run destination, then choose **Run**. Allow signing-key access when Keychain asks. The export limits supported platforms to iPhoneOS. Close this generated project before exporting it again.

The demo uses Metal and includes a local-network usage description. Allow local-network access on the iPhone. Its native document picker copies a selected movie into the app cache, requiring space for another copy. File audio is supported; background streaming is unsupported.

To prepare the separate test app, export with `--test`. Open `target/ios/kybertest/export/kybertest.xcodeproj` and run it on the iPhone. Then run:

```sh
python3 scripts/test_ios.py --device <iPhone-UDID> --godot /path/to/Godot --mac-ip <Mac-LAN-IP> --device-ip <iPhone-LAN-IP>
```

This requires a native macOS addon in the demo and the MP4 fixture from the desktop test setup. It tests video and RPC with the iPhone as both sender/server and receiver/client. UDP ports 14500–14503 must be reachable. Reports are saved under `target/ios-tests`; the test app is stopped afterward.

On November 22, 2026, an iPhone 15 Pro running iOS 27.2 passed the native SDK and loopback test. Mac↔iPhone video and RPC also passed in both directions. Each video test received 36 frames at 640×360 with no reported drops. Other iOS versions, iPad, App Store distribution, and sustained performance remain unverified. CI builds the release framework but does not run physical-device tests.

Windows↔iPhone loop video was visually confirmed in both directions on the same date. The final Windows receiver log showed 308 received frames and no drops. These checks used an unoptimized build; they do not establish sustained frame rate. The UIKit file picker is implemented, but selecting an external file on a physical iPhone remains unverified.

A subsequent Windows→iPhone timing check reproduced periodic stalls despite zero dropped frames. Moving JPEG encoding and decoding off the QUIC runtime thread resolved the visible stalls. In the captured samples, the maximum display interval fell from 1,300 ms to 187 ms; the 95th percentile fell from 267 ms to 50 ms. The corrected sample contained 489 display observations and no intervals above 200 ms. This is a short device check, not a latency guarantee. The shared change applies to all native platforms; this timing comparison covers Windows→iPhone only.


## Extended feature validation

On November 22, 2026, Windows sent a rendered Viewport to a Mac and Steam Deck simultaneously. Both receivers displayed the scene. Pausing the Deck process left Mac reception running. Restarting the Deck receiver joined the active broadcast.

The first run exposed a receiver backlog: Deck JPEG decoding took about 55 ms, exceeding the sender's roughly 50 ms frame interval. Network reading now runs independently of decoding and retains the latest compressed frame. Official KyProto clock synchronization supplies the sender-time basis for age metrics and stale-frame rejection. The user confirmed that the large delay after restarting the Deck receiver was resolved.

In the final debug-build run, Mac received/displayed over 2,000 frames and the restarted Deck over 980 frames. The final 120-sample mean source-to-ready ages were about 71 ms and 130 ms, respectively. These are estimated capture-to-decoded-frame ages, not physical display latency. Different device processing times remain visible; frame-accurate simultaneous presentation isn't tested or guaranteed.

The reusable LAN harness is `demo/tests/process/multi_video.gd`. Arguments are role, address, port, an existing result directory, duration in seconds, and an optional movie path. Role `sender` without a movie captures the animated Viewport. Receivers read `code.txt` copied from the sender's result directory. Each process writes timing samples and a rendered PNG. Create a `stop` file in its result directory to end it.

Rust tests cover late joining, disconnect isolation, EOF, slow decoding, native clipboard packet round trips, and gamepad mapping. GUT covers the extended input API, clipboard callbacks and UTF-8/JSON size handling, and performance availability. Real Kyber-host input/clipboard interoperability, mobile clipboard permission flows, and the new features on mobile hardware remain unverified.

Before audio support was added, Windows passed formatting, Clippy, 16 Rust tests, 47 GUT tests, separate-process video/RPC tests, and the native runtime smoke test. Linux and macOS passed formatting, Clippy, and Rust tests. Android arm64 and iOS arm64 built successfully. Release packaging validation passed five tests. Existing CI discovers the added Rust and GUT tests; GitHub Actions was not run as part of that verification.

### File audio validation

Windows, macOS, Linux, Android, and iOS file senders decode audio to stereo PCM at 48 kHz. The native test includes a 44.1 kHz mono WAV fixture.
The Windows, Linux, and macOS test scripts run `test_audio_processes.py` after staging the native addon. This checks Godot bus output, looping, and pause/resume.
For LAN tests, `tests/process/audio_video.gd` accepts optional address and duration arguments after the port. Copy the sender's `code.txt` to the receiver's result directory.
Reports contain captured audio energy, sample counts, frame counts, and estimated A/V timing. These are software measurements, not physical speaker/display measurements.

### Mobile audio tests

Android source builds require CMake and Ninja for statically linked Opus. The build script fixes the NDK ABI to arm64-v8a and API 24.
After building, add `--audio` to the `test_android.py` command above to export and install the audio test app and test Windows ↔ Android. UDP ports 14600–14601 must be reachable.
For iOS, rebuild and export with `--test`, install the app, then add `--audio` to the `test_ios.py` command above to test Mac ↔ iPhone on UDP ports 14610–14611.
Both tests generate a 44.1 kHz AAC fixture with FFmpeg, exercise looping and pause/resume, and verify received Godot bus samples and estimated A/V timing. FFmpeg is only a test dependency.

CMake is a build dependency on every platform because Opus is built from source. On macOS, install it with `brew install cmake`.

### Cross-platform audio validation

On November 22, 2026, file audio passed bidirectional LAN tests for Windows ↔ macOS, Windows ↔ Steam Deck, Windows ↔ Android, and macOS ↔ iPhone. Each direction captured at least 830,000 PCM frames on the receiver’s Godot bus over 20 seconds and passed looping and pause/resume checks. The maximum sampled A/V p95 values were 76.4 ms, 25.8 ms, 16.1 ms, and 8.2 ms, respectively. These are estimated playback timing metrics, not physical speaker/display measurements or listening tests. Android also passed all six native media tests, including 44.1 kHz mono to 48 kHz stereo conversion.

The final Linux suite passed formatting, Clippy, 22 Rust tests, 47 GUT tests, separate-process video/RPC, audio/video loopback, and the native SDK smoke test. The audio loopback captured 528,384 samples with a maximum sampled A/V p95 of 6.512 ms. Windows passed formatting, Clippy, and 21 Rust tests; macOS passed its full suite including 47 GUT tests and audio loopback. Release packaging tests passed all five cases. GitHub Actions itself was not run.

### Live audio and transport tests

Desktop test scripts now run `scripts/test_live_audio.py` and audio/video process tests in all three transport modes. The live test generates a 44.1 kHz tone on a Godot bus and captures the receiver's output, exercises shared/standalone AudioStream resources, immediate stop/play, duplicate-consumer rejection, and an audio-only connection longer than the video inactivity timeout. No physical microphone is recorded.

`python scripts/test_audio_processes.py --godot <executable> --transport 2 --drop-every 11` uses a local UDP relay to drop every eleventh downstream datagram after a two-second handshake window. The test checks video/audio output, looping, pause/resume, and estimated synchronization. It validates continued playback under that loss pattern, not recovery of every missing packet or physical audiovisual synchronization.

GUT also checks microphone input settings, bus reordering/cleanup, resource credential storage, and invalid connection errors. CI runs these tests through the standard Windows/Linux/macOS scripts. Mobile builds compile the common implementation; physical microphone permissions and capture on Android/iOS require separate device validation.

On November 22, 2026, this implementation passed the Windows standard suite (formatting, Clippy, 22 Rust/native tests, 52 GUT tests, video/RPC processes, and native SDK smoke test). Linux passed its standard suite (23 Rust/native tests and 51 GUT tests), followed by all five current audio GUT tests, including the added invalid-connection case. Both passed all three audio/video transports, packet-loss playback, live bus capture, and standalone/shared audio playback. A separate shared AudioStream/video test with every eleventh downstream datagram dropped passed with a maximum sampled A/V p95 of 101.668 ms on Windows and 40.720 ms on Linux. Android arm64 release compilation passed. Physical microphone capture and the new path on macOS/iOS devices were not tested in this run; GitHub Actions was not run.
