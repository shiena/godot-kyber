# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

param([switch]$Release, [switch]$MockSdk)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Push-Location $projectRoot
try {
    & python "$PSScriptRoot/prepare_sdk.py"
    if ($LASTEXITCODE -ne 0) { throw 'SDK source preparation failed' }
    $buildProfile = if ($Release) { 'release' } else { 'debug' }
    $cargoArgs = @('build', '--locked')
    $target = 'target'
    if ($MockSdk) {
        $target = 'target/mock-sdk'
        $cargoArgs += @('--no-default-features', '--features', 'mock-sdk', '--target-dir', $target)
    }
    if ($Release) { $cargoArgs += '--release' }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw 'Cargo build failed' }
    $destination = Join-Path $projectRoot 'demo/addons/kyber/bin'
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item -LiteralPath "$target/$buildProfile/godot_kyber.dll" -Destination "$destination/godot_kyber.dll"
    & python "$PSScriptRoot/install_addon.py" $(if ($MockSdk) { '--mock' } else { '--native' })
    if ($LASTEXITCODE -ne 0) { throw 'Add-on staging failed' }
    Write-Host "Built $destination/godot_kyber.dll"
} finally { Pop-Location }
