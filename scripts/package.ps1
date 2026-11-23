# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

# Package the existing release binary after scripts/test.ps1 -Release succeeds.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Push-Location $projectRoot
try {
    $releaseDll = Join-Path $projectRoot 'target/release/godot_kyber.dll'
    $testedDll = Join-Path $projectRoot 'demo/addons/kyber/bin/godot_kyber.dll'
    foreach ($binary in @($releaseDll, $testedDll)) {
        if (-not (Test-Path -LiteralPath $binary)) { throw 'Run scripts/test.ps1 -Release before packaging' }
    }
    if ((Get-FileHash -LiteralPath $releaseDll).Hash -ne (Get-FileHash -LiteralPath $testedDll).Hash) {
        throw 'The demo DLL does not match the release DLL; run scripts/test.ps1 -Release'
    }
    $buildInfo = Get-Content demo/addons/kyber/bin/build-info.json -Raw | ConvertFrom-Json
    if ($buildInfo.backend -ne 'native' -or $buildInfo.sha256 -ne (Get-FileHash $testedDll -Algorithm SHA256).Hash.ToLowerInvariant()) {
        throw 'Only the native SDK build can be packaged; mock builds are rejected'
    }
    $verification = Get-Content target/test-results/native-runtime.json -Raw | ConvertFrom-Json
    if ($verification.sha256 -ne $buildInfo.sha256 -or $verification.runtime -ne (Get-FileHash demo/addons/kyber/runtime-manifest.json -Algorithm SHA256).Hash) {
        throw 'Run the full test suite against this native build before packaging'
    }
    $runtimeManifest = Get-Content demo/addons/kyber/runtime-manifest.json -Raw | ConvertFrom-Json
    foreach ($entry in $runtimeManifest.files.PSObject.Properties) {
        if ($entry.Name.StartsWith('bin/') -and (Get-FileHash (Join-Path 'demo/addons/kyber' $entry.Name) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.Value) {
            throw "Runtime file changed after testing: $($entry.Name)"
        }
    }
    $metadataJson = & cargo metadata --no-deps --format-version 1 --locked
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read Cargo package metadata' }
    $metadata = $metadataJson | ConvertFrom-Json
    $version = ($metadata.packages | Where-Object name -EQ 'godot-kyber').version
    if ($version -notmatch '^\d+\.\d+\.\d+([+-][0-9A-Za-z.-]+)?$') { throw 'Invalid package version' }
    $packages = Join-Path $projectRoot 'target/packages'
    $staging = Join-Path $projectRoot ("target/package-staging/{0}/godot-kyber" -f [guid]::NewGuid().ToString('N'))
    $addon = Join-Path $staging 'addons/kyber'
    New-Item -ItemType Directory -Force -Path "$addon/bin", "$staging/docs", $packages | Out-Null
    Copy-Item -LiteralPath $testedDll -Destination "$addon/bin/godot_kyber.dll"
    foreach ($entry in $runtimeManifest.files.PSObject.Properties) {
        if ($entry.Name.StartsWith('bin/')) {
            $destination = Join-Path $addon $entry.Name
            New-Item -ItemType Directory -Force (Split-Path $destination -Parent) | Out-Null
            Copy-Item -LiteralPath (Join-Path 'demo/addons/kyber' $entry.Name) -Destination $destination
        }
    }
    Copy-Item -LiteralPath demo/addons/kyber/kyber.gdextension,demo/addons/kyber/runtime-manifest.json,demo/addons/kyber/input_relay.gd,demo/addons/kyber/input_relay.gd.uid,demo/addons/kyber/icon.svg -Destination $addon
    Copy-Item -LiteralPath demo/addons/kyber/licenses -Destination $addon -Recurse
    Copy-Item -LiteralPath README.md,README_ja.md,LICENSE,NOTICE.md,REUSE.toml,BINARY_LICENSE.md -Destination $staging
    Copy-Item -LiteralPath LICENSES -Destination $staging -Recurse
    Get-ChildItem -LiteralPath docs -Filter '*.md' -File | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination "$staging/docs"
    }
    $archive = Join-Path $packages "godot-kyber-$version-windows-x86_64.zip"
    Compress-Archive -LiteralPath $staging -DestinationPath $archive -Force
    Write-Host "Packaged $archive (embedded SDK; Windows system media)"
} finally { Pop-Location }
