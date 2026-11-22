#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

set -euo pipefail
cd "$(dirname "$0")/.."
case "$(uname -s)" in
    Darwin) platform=macos; binary=libgodot_kyber.dylib ;;
    Linux) platform=linux; binary=libgodot_kyber.so ;;
    *) echo "Unsupported build host" >&2; exit 2 ;;
esac
profile=debug
target=target
args=(build --locked)
mode=--native
for option in "$@"; do
    case "$option" in
        --release) profile=release; args+=(--release) ;;
        --mock-sdk) target=target/mock-sdk; mode=--mock; args+=(--no-default-features --features mock-sdk --target-dir "$target") ;;
        *) echo "Unknown argument: $option" >&2; exit 2 ;;
    esac
done
python3 scripts/prepare_sdk.py
cargo "${args[@]}"
mkdir -p demo/addons/kyber/bin
cp "$target/$profile/$binary" demo/addons/kyber/bin/
python3 scripts/install_addon.py "$mode" --platform "$platform"
echo "Built demo/addons/kyber/bin/$binary"
