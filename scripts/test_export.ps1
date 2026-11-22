# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

param(
    [string]$Godot = $(if ($env:GODOT) { $env:GODOT } else { Join-Path $env:USERPROFILE '.local/godot/4.7.2-stable/Godot_v4.7.2-stable_win64_console.exe' })
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$buildInfo = Get-Content "$projectRoot/demo/addons/kyber/bin/build-info.json" -Raw | ConvertFrom-Json
if ($buildInfo.backend -ne 'native') { throw 'Run scripts/build.ps1 to install the native build before export testing' }
$testRoot = Join-Path $projectRoot ("target/export-smoke/{0}" -f [guid]::NewGuid().ToString('N'))
$project = Join-Path $testRoot 'project'
$game = Join-Path $testRoot 'game'
New-Item -ItemType Directory -Force "$project/addons", $game | Out-Null
Copy-Item -LiteralPath "$projectRoot/demo/addons/kyber" -Destination "$project/addons" -Recurse
if ((Get-FileHash "$project/addons/kyber/bin/godot_kyber.dll" -Algorithm SHA256).Hash.ToLowerInvariant() -ne $buildInfo.sha256) {
    throw 'The add-on changed while preparing the export test'
}
@'
config_version=5
[application]
config/name="Kyber export smoke test"
run/main_scene="res://main.tscn"
[rendering]
renderer/rendering_method="gl_compatibility"
'@ | Set-Content "$project/project.godot"
@'
[gd_scene load_steps=2 format=3]
[ext_resource type="Script" path="res://main.gd" id="1"]
[node name="Main" type="Node"]
script = ExtResource("1")
'@ | Set-Content "$project/main.tscn"
$script = Get-Content "$projectRoot/demo/tests/process/native_sdk.gd" -Raw
$script.Replace('extends SceneTree', 'extends Node').Replace('_initialize()', '_ready()').Replace('root.add_child', 'get_tree().root.add_child').Replace('await process_frame', 'await get_tree().process_frame').Replace('quit(', 'get_tree().quit(') | Set-Content "$project/main.gd"
@'
[preset.0]
name="Windows"
platform="Windows Desktop"
runnable=true
export_filter="all_resources"
include_filter=""
exclude_filter=""
export_path="../game/kyber.exe"
[preset.0.options]
binary_format/architecture="x86_64"
binary_format/embed_pck=false
debug/export_console_wrapper=0
codesign/enable=false
application/modify_resources=false
'@ | Set-Content "$project/export_presets.cfg"
# Load the extension at startup, before the editor discovers typed GDScripts.
# A frame-count shutdown can race the initial filesystem scan on CI runners.
New-Item -ItemType Directory -Force "$project/.godot" | Out-Null
'res://addons/kyber/kyber.gdextension' | Set-Content "$project/.godot/extension_list.cfg" -Encoding utf8
$importLog = Join-Path $testRoot 'import.log'
& $Godot --headless --path $project --editor --import --quit *> $importLog
$importExitCode = $LASTEXITCODE
Get-Content -LiteralPath $importLog
if ($importExitCode -ne 0) { throw "Export test import failed with exit code $importExitCode`: $importLog" }
$exportLog = Join-Path $testRoot 'export.log'
& $Godot --headless --path $project --export-release Windows "$game/kyber.exe" *> $exportLog
$exportExitCode = $LASTEXITCODE
Get-Content -LiteralPath $exportLog
if ($exportExitCode -ne 0) { throw "Windows export failed with exit code $exportExitCode`: $exportLog (check matching export templates)" }
if (-not (Test-Path "$game/licenses/rust-packages.json")) { throw 'Export omitted dependency license notices' }
Copy-Item -LiteralPath "$projectRoot/demo/tests/fixtures/video sample.mp4" -Destination "$game/media-test.mp4"
$process = Start-Process -FilePath "$game/kyber.exe" -ArgumentList @('--headless', '--', "`"$game/media-test.mp4`"") -WindowStyle Hidden -PassThru -RedirectStandardOutput "$testRoot/game.log" -RedirectStandardError "$testRoot/game-error.log"
if (-not $process.WaitForExit(30000)) {
    $process.Kill()
    throw 'Exported SDK startup/shutdown timed out'
}
if ($process.ExitCode -ne 0) { throw "Exported SDK startup/shutdown failed: $testRoot/game-error.log" }
if (-not (Select-String -LiteralPath "$testRoot/game.log" -Pattern '^KYBER_NATIVE_RUNTIME_OK$' -Quiet)) {
    throw "Exported SDK test did not complete: $testRoot/game.log"
}
$dlls = @(Get-ChildItem -LiteralPath $game -Filter '*.dll' -Recurse)
if ($dlls.Count -ne 1 -or $dlls[0].Name -ne 'godot_kyber.dll') { throw 'Unexpected redistributable DLL in export' }
Write-Host "Exported game verified with Windows system media: $game"
