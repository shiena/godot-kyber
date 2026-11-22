#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Materialize the pinned Kyber SDK and install the Godot bridge sources."""
import argparse
import pathlib
import os
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
SDK_REV = "ebca32a08f6488ea23750943ded52119efd584d6"
CTL_REV = "92c632f9e889917a34aaa7697fb88b4991054b1c"


def git(path, *args):
    return subprocess.check_output(["git", "-C", str(path), *args], text=True, encoding="utf-8").strip()


def generate_player(ctl):
    """Extract compatibility types from immutable SDK blobs, preserving notices."""
    sources = {}
    for name, expected in (
        ("player.rs", "d3454d70f89756f64b77ca351c4ee7ca7974d79c"),
        ("window_handle.rs", "5c599c58b4e6d8bdab6fe8dbef780afe7627599d"),
    ):
        ref = f"{CTL_REV}:kyvlcplayer/src/{name}"
        if git(ctl, "rev-parse", ref) != expected:
            raise RuntimeError(f"Review SDK player extraction for {ref}")
        sources[name] = git(ctl, "show", ref)
    player = sources["player.rs"]
    window = sources["window_handle.rs"]
    # These boundaries are valid only for the exact blobs checked above.
    events = player[player.index("#[derive(Clone, Debug, PartialEq, Eq)]"):
                    player.index("#[allow(dead_code)]\nfn setup_vlc_logger")]
    handles = window[window.index("#[derive(Clone, Copy, Debug)]"):
                     window.index("impl From<RawWindowHandle>")]
    handles = replace_once(handles, "pub enum WindowHandle {",
                           "pub enum WindowHandle {\n    Cpu(CpuOutput),")
    notice = player[:player.index("use std::collections::HashMap;")]
    adapter = (ROOT / "sdk/native_player.rs").read_text(encoding="utf-8")
    return (notice + f"// Generated from KyCtl {CTL_REV}; do not edit.\n"
            "// Includes MIT-licensed additions from sdk/native_player.rs.\n\n"
            + events + handles + adapter)


def replace_once(text, old, new):
    if new in text:
        return text
    if text.count(old) != 1:
        raise RuntimeError(f"SDK source does not match the pinned version: {old[:80]}")
    return text.replace(old, new, 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk", type=pathlib.Path, default=ROOT / ".deps/kysdk")
    args = parser.parse_args()
    sdk = args.sdk.resolve()
    if not sdk.exists():
        sdk.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "clone", "https://gitlab.com/kyber/core/kysdk.git", str(sdk)], check=True)
        git(sdk, "checkout", "--detach", SDK_REV)
    if git(sdk, "rev-parse", "HEAD") != SDK_REV:
        raise RuntimeError(f"Expected SDK {SDK_REV}; use a separate checkout")
    git(sdk, "submodule", "update", "--init", "--depth", "1")
    git(sdk / "kynput", "submodule", "update", "--init", "--recursive", "--depth", "1")
    ctl = sdk / "kyctl"
    if git(ctl, "rev-parse", "HEAD") != CTL_REV:
        raise RuntimeError(f"Unexpected revision in {ctl}")

    # Prepare every edit first so a mismatched source cannot leave half a patch.
    edits = {}
    # cbindgen invokes Cargo from inside the SDK. Its upstream patch must not
    # require the unused VLC submodule on a clean checkout.
    path = sdk / ".cargo/config.toml"
    edits[path] = "\n".join(line for line in path.read_text(encoding="utf-8").splitlines()
        if not line.lstrip().startswith("vlc-rs ")) + "\n"
    path = ctl / "kyclient/src/capi.rs"
    content = path.read_text(encoding="utf-8")
    marker = '\n#[cfg(all(feature = "kymux", not(target_family = "wasm")))]\n#[path = "godot_bridge.rs"]\nmod godot_bridge;\n'
    edits[path] = content if marker in content else content + marker
    edits[ctl / "kyclient/src/godot_bridge.rs"] = (ROOT / "sdk/godot_bridge.rs").read_text(encoding="utf-8")
    path = ctl / "kyclient/Cargo.toml"
    edits[path] = replace_once(git(ctl, "show", "HEAD:kyclient/Cargo.toml") + "\n", "[dependencies]", '[dependencies]\nkeycode = "1.0"')
    # External engines inject input explicitly; the SDK dummy client emits test keys.
    path = sdk / "kynput/kynput/src/capi/mod.rs"
    content = path.read_text(encoding="utf-8")
    external_config = '''
/// Create an input pipeline without OS hooks or synthetic input producers.
#[unsafe(no_mangle)]
pub extern "C" fn kynput_get_external_client_config() -> Box<PipelineConfig> {
    Box::new(PipelineConfig {
        config: crate::InputPipelineConfig {
            target: crate::InputTarget::Client,
            cursor_format: Default::default(),
            builders: Vec::new(),
        },
    })
}
'''
    edits[path] = content if external_config in content else content + external_config
    path = sdk / "kynput/kynput-rs/src/lib.rs"
    edits[path] = replace_once(path.read_text(encoding="utf-8"), "impl Config {", '''impl Config {
    /// Receives input explicitly from an embedding application.
    pub fn external_client() -> Self {
        Self { ptr: unsafe { sys::kynput_get_external_client_config() } }
    }
''')
    # Upstream maps the right stick click to the right shoulder by mistake.
    edits[path] = edits[path].replace(
        'Self::RightStick => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_RIGHT_SHOULDER,',
        'Self::RightStick => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_RIGHT_STICK,')
    path = sdk / "kynput/kynput/src/capi/mod.rs"
    marker = '\n#[path = "godot_clipboard.rs"]\nmod godot_clipboard;\n'
    if marker not in edits[path]: edits[path] += marker
    edits[path.parent / "godot_clipboard.rs"] = (ROOT / "sdk/kynput_clipboard.rs").read_text(encoding="utf-8")
    path = sdk / "kynput/kynput-rs/src/lib.rs"
    marker = '\nmod godot_clipboard;\npub use godot_clipboard::TextClipboard;\n'
    if marker not in edits[path]: edits[path] += marker
    edits[path.parent / "godot_clipboard.rs"] = (ROOT / "sdk/kynput_clipboard_wrapper.rs").read_text(encoding="utf-8")
    marker = '\nmod godot_rumble;\npub use godot_rumble::read_rumble;\n'
    if marker not in edits[path]: edits[path] += marker
    edits[path.parent / "godot_rumble.rs"] = (ROOT / "sdk/kynput_rumble_wrapper.rs").read_text(encoding="utf-8")
    # Keep the SDK monotonic clock valid on Windows after long system uptimes.
    path = sdk / "kymux/kyproto/src/clock.rs"
    edits[path] = replace_once(path.read_text(encoding="utf-8"),
        'Ok(counter * 1_000_000 / frequency)',
        'Ok((i128::from(counter) * 1_000_000 / i128::from(frequency)) as i64)')
    # Route media packets to the embedding application's native backend.
    path = ctl / "kyclient/Cargo.toml"
    content = edits[path]
    media_path = pathlib.Path(os.path.relpath(ROOT / "media", ctl / "kyclient")).as_posix()
    content = replace_once(content, 'kyvlcplayer = { path = "../kyvlcplayer" }',
        f'kyber-media = {{ path = "{media_path}" }}')
    edits[path] = content
    path = ctl / "kyclient/src/platform/desktop/mod.rs"
    content = path.read_text(encoding="utf-8")
    edits[path] = content.replace('pub use kyvlcplayer as player;', 'pub mod player;')
    edits[ctl / "kyclient/src/platform/desktop/player.rs"] = generate_player(ctl)
    edits[ctl / "kyclient/src/kymux_backend/godot_audio.rs"] = (ROOT / "sdk/audio_forward.rs").read_text(encoding="utf-8")
    # Keep upstream input/network lifecycle; replace only the media adapter.
    path = ctl / "kyclient/src/kymux_backend/desktop.rs"
    original = git(ctl, "show", "HEAD:kyclient/src/kymux_backend/desktop.rs")
    tail = original[original.index('fn start_input('):]
    tail = tail.replace('        let player = create_player(\n            &mut kycom,', '        let player = create_player(')
    tail = tail.replace('            endpoints.audio,\n        )?;', '            endpoints.audio,\n        ).await?;')
    tail = tail.replace('player.0.audio_set_volume(volume)?;', 'let _ = (player, volume);\n        return Err(Error::NoAudioPlayer);')
    tail = tail.replace('player.0.audio_set_mute(mute)?;', 'let _ = (player, mute);\n        return Err(Error::NoAudioPlayer);')
    tail = tail.replace('return Err(Error::NoAudioPlayer);\n\n        Ok(())', 'Err(Error::NoAudioPlayer)')
    edits[path] = original[:original.index("use async_trait")] + (ROOT / "sdk/native_desktop.rs").read_text(encoding="utf-8") + "\n" + tail + "\n"
    # Link Rust SDK/input code into the extension instead of shipping SDK DLLs.
    sys = sdk / "kynput/kynput-rs/kynput-sys"
    path = sys / "Cargo.toml"
    content = path.read_text(encoding="utf-8")
    content = replace_once(content, '[dependencies]', '[dependencies]\nkynput = { path = "../../kynput", default-features = false, features = ["capi"] }')
    edits[path] = replace_once(content, '[build-dependencies]', '[build-dependencies]\ncbindgen = "0.29"')
    path = sys / "src/lib.rs"
    content = path.read_text(encoding="utf-8")
    edits[path] = replace_once(content, '#![no_std]', '#![no_std]\n\n// Retain the statically linked C entry points used by these bindings.\nextern crate kynput;')
    edits[sys / "build.rs"] = (ROOT / "sdk/kynput_build.rs").read_text(encoding="utf-8")
    for path, content in edits.items():
        if not path.exists() or path.read_text(encoding="utf-8") != content:
            path.write_text(content, encoding="utf-8", newline="\n")
    print(f"Prepared embedded Kyber SDK: {sdk}")


if __name__ == "__main__":
    main()
