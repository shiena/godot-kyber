# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

param([switch]$Release, [switch]$BuildTests, [string]$Ndk = "$env:LOCALAPPDATA/Android/Sdk/ndk/27.2.12479018")
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$toolchain = "$Ndk/toolchains/llvm/prebuilt/windows-x86_64"
if (-not (Test-Path "$toolchain/bin/clang.exe")) { throw "Android NDK not found: $Ndk" }
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "$toolchain/bin/aarch64-linux-android24-clang.cmd"
$env:CC_aarch64_linux_android = "$toolchain/bin/aarch64-linux-android24-clang.cmd"
$env:CXX_aarch64_linux_android = "$toolchain/bin/aarch64-linux-android24-clang++.cmd"
$env:AR_aarch64_linux_android = "$toolchain/bin/llvm-ar.exe"
$env:BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android = "--sysroot=$toolchain/sysroot --target=aarch64-linux-android24"
$env:CMAKE_GENERATOR_aarch64_linux_android = 'Ninja'
$env:CMAKE_TOOLCHAIN_FILE_aarch64_linux_android = "$PSScriptRoot/android-arm64.cmake"
$env:ANDROID_NDK_HOME = $Ndk
$env:CARGO_PROFILE_DEV_DEBUG = '0'
$env:CARGO_PROFILE_TEST_DEBUG = '0'
$env:CARGO_INCREMENTAL = '0'
Push-Location $projectRoot
try {
    & python "$PSScriptRoot/prepare_sdk.py"
    if ($LASTEXITCODE -ne 0) { throw 'SDK source preparation failed' }
    $cargoArgs = @('build', '--locked', '--target', 'aarch64-linux-android')
    if ($Release) { $cargoArgs += '--release' }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw 'Android build failed' }
    if ($BuildTests) {
        & cargo test --locked --target aarch64-linux-android --test native_media --no-run
        if ($LASTEXITCODE -ne 0) { throw 'Android test build failed' }
    }
    $profile = if ($Release) { 'release' } else { 'debug' }
    $destination = "$projectRoot/demo/addons/kyber/bin/android"
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item "target/aarch64-linux-android/$profile/libgodot_kyber.so" "$destination/libgodot_kyber.so"
    $packageAddon = "$projectRoot/target/android-addon/addons/kyber"
    New-Item -ItemType Directory -Force "$packageAddon/bin/android" | Out-Null
    Copy-Item "$destination/libgodot_kyber.so" "$packageAddon/bin/android/libgodot_kyber.so"
    & python "$PSScriptRoot/install_addon.py" --native --platform android --addon-dir $packageAddon
    if ($LASTEXITCODE -ne 0) { throw 'Android add-on staging failed' }
    New-Item -ItemType Directory -Force "$destination/licenses" | Out-Null
    Copy-Item "$packageAddon/licenses/*" "$destination/licenses" -Recurse -Force
    Copy-Item "$projectRoot/sdk/kyber.gdextension.in" "$projectRoot/demo/addons/kyber/kyber.gdextension"
    Add-Content "$projectRoot/demo/addons/kyber/kyber.gdextension" "`n[dependencies]`nwindows.x86_64 = { `"res://addons/kyber/licenses`": `"`" }"
} finally { Pop-Location }
