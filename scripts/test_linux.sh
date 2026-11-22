#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

set -euo pipefail
cd "$(dirname "$0")/.."
: "${GODOT:?Set GODOT to the Godot 4.7.2 executable}"
python3 scripts/prepare_sdk.py
cargo fmt --check
cargo fmt --manifest-path media/Cargo.toml --check
cargo clippy --all-targets -- -D warnings
cargo clippy --manifest-path media/Cargo.toml --all-targets -- -D warnings
cargo test --locked
mkdir -p target/test-results demo/tests/fixtures
ffmpeg -hide_banner -loglevel error -y -f lavfi -i color=c=red:s=640x360:r=24 \
    -t 1.5 -an -c:v libx264 -pix_fmt yuv420p 'demo/tests/fixtures/video sample.mp4'
bash scripts/build.sh --release --mock-sdk
"$GODOT" --headless --path demo --editor --quit-after 60
"$GODOT" --headless --path demo --editor --import --quit
report="$(pwd)/target/test-results/gut-$(date +%s)-$$.xml"
"$GODOT" --headless --path demo --script addons/gut/gut_cmdln.gd -gexit -gdisable_colors "-gjunit_xml_file=$report"
python3 scripts/check_gut.py "$report"
cp "$report" target/test-results/gut.xml
python3 scripts/test_processes.py --godot "$GODOT" --scenario video
python3 scripts/test_processes.py --godot "$GODOT" --scenario rpc
bash scripts/build.sh --release
for mode in 0 1 2; do
    python3 scripts/test_audio_processes.py --godot "$GODOT" --transport "$mode"
done
python3 scripts/test_audio_processes.py --godot "$GODOT" --transport 2 --drop-every 11 --audio-stream
python3 scripts/test_audio_processes.py --godot "$GODOT" --codec h264 --acceleration 0
python3 scripts/test_live_audio.py --godot "$GODOT"
python3 scripts/test_native_runtime.py --godot "$GODOT"
python3 - <<'PY'
import hashlib, json, pathlib
addon = pathlib.Path('demo/addons/kyber')
pathlib.Path('target/test-results/native-runtime.json').write_text(json.dumps({
    'sha256': hashlib.sha256((addon / 'bin/libgodot_kyber.so').read_bytes()).hexdigest(),
    'runtime': hashlib.sha256((addon / 'runtime-manifest.json').read_bytes()).hexdigest(),
}))
PY
