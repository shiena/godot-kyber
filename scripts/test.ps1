# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

param(
    [string]$Godot = $(if ($env:GODOT) { $env:GODOT } else { Join-Path $env:USERPROFILE '.local/godot/4.7.2-stable' }),
    [switch]$PrepareOnly,
    [switch]$Release,
    [switch]$MockOnly
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (Test-Path -LiteralPath $Godot -PathType Container) {
    $Godot = Join-Path $Godot 'Godot_v4.7.2-stable_win64_console.exe'
}
if (-not (Test-Path -LiteralPath $Godot -PathType Leaf)) { throw "Godot executable not found: $Godot" }
$Godot = (Resolve-Path -LiteralPath $Godot).Path
Push-Location $projectRoot
try {
    if (-not $MockOnly) { & "$PSScriptRoot/build.ps1" -Release:$Release }
    if (-not $PrepareOnly) {
        & cargo fmt --check
        if ($LASTEXITCODE -ne 0) { throw 'Formatting failed' }
        & cargo fmt --manifest-path media/Cargo.toml --check
        if ($LASTEXITCODE -ne 0) { throw 'Media formatting failed' }
        & cargo clippy --manifest-path media/Cargo.toml --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Media Clippy failed' }
        $testFeatures = if ($MockOnly) { @('--no-default-features', '--features', 'mock-sdk') } else { @() }
        & cargo clippy @testFeatures --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Clippy failed' }
        & cargo test @testFeatures --locked
        if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed' }

    }
    & "$PSScriptRoot/build.ps1" -Release:$Release -MockSdk
    New-Item -ItemType Directory -Force -Path demo/tests/fixtures | Out-Null
    $ffmpeg = if ($env:FFMPEG) { $env:FFMPEG } else { 'ffmpeg' }
    & $ffmpeg -hide_banner -loglevel error -y -f lavfi -i 'color=c=red:s=640x360:r=24' -t 1.5 -an -c:v libx264 -pix_fmt yuv420p 'demo/tests/fixtures/video sample.mp4'
    if ($LASTEXITCODE -ne 0) { throw 'FFmpeg video fixture generation failed' }
    # Register before startup so a clean import can resolve extension classes.
    # Preserve other registrations when testing an existing editor project.
    New-Item -ItemType Directory -Force demo/.godot | Out-Null
    $extensionList = 'demo/.godot/extension_list.cfg'
    $extension = 'res://addons/kyber/kyber.gdextension'
    if (-not (Test-Path $extensionList) -or $extension -notin @(Get-Content $extensionList)) {
        Add-Content -LiteralPath $extensionList -Value $extension -Encoding utf8
    }
    $importLog = Join-Path $projectRoot 'target/test-results/godot-import.log'
    New-Item -ItemType Directory -Force (Split-Path $importLog -Parent) | Out-Null
    & $Godot --headless --path demo --editor --import --quit *> $importLog
    $importExitCode = $LASTEXITCODE
    Get-Content -LiteralPath $importLog
    if ($importExitCode -ne 0) { throw "Godot import failed with exit code $importExitCode`: $importLog" }
    if ($PrepareOnly) {
        Write-Host 'Ready: open demo/project.godot and run tests/test_runner.tscn with F6.'
        return
    }
    $reportDirectory = Join-Path $projectRoot 'target/test-results'
    New-Item -ItemType Directory -Force -Path $reportDirectory | Out-Null
    # A unique path prevents an old successful report from masking a failed run.
    $reportPath = Join-Path $reportDirectory ("gut-{0}.xml" -f [guid]::NewGuid().ToString('N'))
    & $Godot --headless --path demo --script addons/gut/gut_cmdln.gd -gexit -gdisable_colors "-gjunit_xml_file=$reportPath"
    if ($LASTEXITCODE -ne 0) { throw 'GUT tests failed' }
    if (-not (Test-Path -LiteralPath $reportPath)) { throw 'GUT did not produce a test report' }
    [xml]$report = Get-Content -LiteralPath $reportPath -Raw
    $testCases = @($report.SelectNodes('//testcase'))
    $expectedTests = @(Get-ChildItem demo/tests/unit,demo/tests/integration -Filter 'test_*.gd' -Recurse |
        Select-String -Pattern '^func test_').Count
    if ($expectedTests -eq 0 -or $testCases.Count -lt $expectedTests) {
        throw "Incomplete test discovery: expected $expectedTests tests, ran $($testCases.Count)"
    }
    if ($report.SelectNodes('//failure|//error|//skipped').Count -ne 0) {
        throw 'GUT report contains failed, errored, or skipped tests'
    }
    Copy-Item -LiteralPath $reportPath -Destination (Join-Path $reportDirectory 'gut.xml')
    Write-Host "GUT: $($testCases.Count) tests passed. Report: target/test-results/gut.xml"
    & "$PSScriptRoot/test_video_processes.ps1" -Godot $Godot
    & "$PSScriptRoot/test_video_processes.ps1" -Godot $Godot -Scenario rpc
    if (-not $MockOnly) {
        & "$PSScriptRoot/build.ps1" -Release:$Release
        foreach ($mode in 0, 1, 2) {
            python scripts/test_audio_processes.py --godot $Godot --transport $mode
            if ($LASTEXITCODE -ne 0) { throw "Audio/video transport $mode failed" }
        }
        python scripts/test_audio_processes.py --godot $Godot --transport 2 --drop-every 11 --audio-stream
        if ($LASTEXITCODE -ne 0) { throw 'FEC loss test failed' }
        python scripts/test_audio_processes.py --godot $Godot --codec h264 --acceleration 0
        if ($LASTEXITCODE -ne 0) { throw 'H.264 audio/video test failed' }
        python scripts/test_live_audio.py --godot $Godot
        if ($LASTEXITCODE -ne 0) { throw 'Live audio test failed' }
        python scripts/test_native_runtime.py --godot $Godot
        if ($LASTEXITCODE -ne 0) { throw 'Embedded SDK smoke test failed' }
        @{
            sha256 = (Get-FileHash demo/addons/kyber/bin/godot_kyber.dll -Algorithm SHA256).Hash.ToLowerInvariant()
            runtime = (Get-FileHash demo/addons/kyber/runtime-manifest.json -Algorithm SHA256).Hash
        } | ConvertTo-Json | Set-Content target/test-results/native-runtime.json
    }
} finally { Pop-Location }
