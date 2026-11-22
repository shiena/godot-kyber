#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

set -euo pipefail
cd "$(dirname "$0")/.."
if [[ $(uname -s) != Darwin ]]; then
    echo 'Build iOS on a Mac with Xcode installed.' >&2
    exit 1
fi
profile=debug
arguments=(build --locked --target aarch64-apple-ios)
if [[ ${1:-} == --release ]]; then
    profile=release
    arguments+=(--release)
elif [[ $# != 0 ]]; then
    echo 'Usage: scripts/build_ios.sh [--release]' >&2
    exit 1
fi
export IPHONEOS_DEPLOYMENT_TARGET=${IPHONEOS_DEPLOYMENT_TARGET:-15.0}
export CARGO_PROFILE_DEV_DEBUG=${CARGO_PROFILE_DEV_DEBUG:-0}
export CARGO_INCREMENTAL=0
python3 scripts/prepare_sdk.py
rustup target add aarch64-apple-ios
cargo "${arguments[@]}"
framework=demo/addons/kyber/bin/ios/GodotKyber.framework
mkdir -p "$framework"
cp "target/aarch64-apple-ios/$profile/libgodot_kyber.dylib" "$framework/GodotKyber"
install_name_tool -id '@rpath/GodotKyber.framework/GodotKyber' "$framework/GodotKyber"
python3 - "$framework" <<'PY'
import os, pathlib, plistlib, sys
framework = pathlib.Path(sys.argv[1])
with (framework / "Info.plist").open("wb") as output:
    plistlib.dump({
        "CFBundleExecutable": "GodotKyber",
        "CFBundleIdentifier": "org.godotengine.kyber.framework",
        "CFBundleName": "GodotKyber",
        "CFBundlePackageType": "FMWK",
        "CFBundleShortVersionString": "0.1.0",
        "CFBundleVersion": "1",
        "CFBundleSupportedPlatforms": ["iPhoneOS"],
        "MinimumOSVersion": os.environ["IPHONEOS_DEPLOYMENT_TARGET"],
    }, output)
PY
codesign --force --sign - "$framework"
package=target/ios-addon/addons/kyber
mkdir -p "$package/bin/ios"
ditto "$framework" "$package/bin/ios/GodotKyber.framework"
python3 scripts/install_addon.py --native --platform ios --addon-dir "$package"
mkdir -p demo/addons/kyber/bin/ios/licenses
ditto "$package/licenses" demo/addons/kyber/bin/ios/licenses
cp sdk/kyber.gdextension.in demo/addons/kyber/kyber.gdextension
echo "Built $framework; Xcode will sign it with the application's development identity."
