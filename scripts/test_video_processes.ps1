# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

param(
    [Parameter(Mandatory)][string]$Godot,
    [ValidateSet('video', 'rpc')][string]$Scenario = 'video'
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$directory = Join-Path $projectRoot ("target/$Scenario-processes/" + [guid]::NewGuid().ToString('N'))
$peerScript = if ($Scenario -eq 'rpc') { 'res://tests/multiplayer_peer.gd' } else { 'res://tests/video_peer.gd' }
New-Item -ItemType Directory -Path $directory -Force | Out-Null
$socket = [System.Net.Sockets.UdpClient]::new([System.Net.IPEndPoint]::new([System.Net.IPAddress]::Loopback, 0))
$port = $socket.Client.LocalEndPoint.Port
$socket.Dispose()
$processes = @()
$logs = @()
try {
    foreach ($role in @('sender', 'receiver')) {
        $info = [System.Diagnostics.ProcessStartInfo]::new()
        $info.FileName = $Godot
        $info.UseShellExecute = $false
        $info.CreateNoWindow = $true
        $info.RedirectStandardOutput = $true
        $info.RedirectStandardError = $true
        foreach ($argument in @('--headless', '--path', (Join-Path $projectRoot 'demo'), '--script', $peerScript, '--', $role, $directory, "$port", (Join-Path $projectRoot 'demo/tests/fixtures/video sample.mp4'))) {
            $info.ArgumentList.Add($argument)
        }
        $process = [System.Diagnostics.Process]::Start($info)
        $processes += $process
        $logs += [pscustomobject]@{
            Role = $role
            Output = $process.StandardOutput.ReadToEndAsync()
            Error = $process.StandardError.ReadToEndAsync()
        }
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(40)
    while (@($processes | Where-Object { -not $_.HasExited }).Count -gt 0) {
        if ([DateTime]::UtcNow -gt $deadline) { throw "Two-process $Scenario test timed out" }
        Start-Sleep -Milliseconds 100
    }
    foreach ($process in $processes) {
        if ($process.ExitCode -ne 0) { throw "Godot peer exited with code $($process.ExitCode). Reports: $directory" }
    }
    foreach ($role in @('sender', 'receiver')) {
        $report = Get-Content -LiteralPath (Join-Path $directory "$role.json") -Raw | ConvertFrom-Json
        if (-not $report.success) { throw "$role failed: $($report.status)" }
        Write-Host "$role : $($report.status)"
    }
    Write-Host "Two Godot processes completed the $Scenario scenario through KyProto / QUIC successfully."
} finally {
    foreach ($process in $processes) {
        if (-not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        $process.Dispose()
    }
    foreach ($log in $logs) {
        $log.Output.GetAwaiter().GetResult() | Set-Content -LiteralPath (Join-Path $directory "$($log.Role).stdout.log")
        $log.Error.GetAwaiter().GetResult() | Set-Content -LiteralPath (Join-Path $directory "$($log.Role).stderr.log")
    }
}
