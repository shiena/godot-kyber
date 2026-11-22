# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

param(
    [string]$Version = '4.7.2-stable',
    [Parameter(Mandatory=$true)][string]$Destination,
    [switch]$ExportTemplates
)
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^\d+\.\d+(\.\d+)?-stable$') { throw 'Expected a stable Godot version' }
New-Item -ItemType Directory -Force -Path $Destination | Out-Null
$Destination = (Resolve-Path -LiteralPath $Destination).Path
$archiveName = "Godot_v${Version}_win64.exe.zip"
$releaseUrl = "https://github.com/godotengine/godot-builds/releases/download/$Version"
$archivePath = Join-Path $Destination $archiveName
$checksumsPath = Join-Path $Destination 'SHA512-SUMS.txt'
Invoke-WebRequest "$releaseUrl/$archiveName" -OutFile $archivePath
Invoke-WebRequest "$releaseUrl/SHA512-SUMS.txt" -OutFile $checksumsPath
$pattern = '^([0-9a-fA-F]{128})\s+\*?' + [regex]::Escape($archiveName) + '$'
$matchesFound = @(Get-Content -LiteralPath $checksumsPath | Select-String -Pattern $pattern)
if ($matchesFound.Count -ne 1) { throw "Missing or ambiguous checksum for $archiveName" }
$expectedHash = $matchesFound[0].Matches[0].Groups[1].Value
$actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA512).Hash
if ($actualHash -ne $expectedHash) { throw 'Godot archive checksum mismatch' }
Expand-Archive -LiteralPath $archivePath -DestinationPath $Destination -Force
$executable = Join-Path $Destination "Godot_v${Version}_win64_console.exe"
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) { throw 'Godot console executable is missing' }
$engineVersion = & $executable --version
if ($LASTEXITCODE -ne 0 -or $engineVersion -notlike "$($Version.Replace('-stable', '.stable'))*") {
    throw "Unexpected Godot version: $engineVersion"
}
Write-Host "Installed $engineVersion in $Destination"
if ($ExportTemplates) {
    $templateName = "Godot_v${Version}_export_templates.tpz"
    $templateArchive = Join-Path $Destination 'export_templates.zip'
    Invoke-WebRequest "$releaseUrl/$templateName" -OutFile $templateArchive
    $pattern = '^([0-9a-fA-F]{128})\s+\*?' + [regex]::Escape($templateName) + '$'
    $matchesFound = @(Get-Content -LiteralPath $checksumsPath | Select-String -Pattern $pattern)
    if ($matchesFound.Count -ne 1 -or (Get-FileHash $templateArchive -Algorithm SHA512).Hash -ne $matchesFound[0].Matches[0].Groups[1].Value) {
        throw 'Godot export templates checksum mismatch'
    }
    Expand-Archive -LiteralPath $templateArchive -DestinationPath "$Destination/export" -Force
    $templateDirectory = Join-Path $env:APPDATA "Godot/export_templates/$($Version.Replace('-stable', '.stable'))"
    New-Item -ItemType Directory -Force $templateDirectory | Out-Null
    Copy-Item "$Destination/export/templates/windows_*x86_64.exe" -Destination $templateDirectory
}
