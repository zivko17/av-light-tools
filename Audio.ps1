function Get-AudioInfo {
    if (-not (Get-Module -ListAvailable -Name AudioDeviceCmdlets)) {
        Write-Host "AudioDeviceCmdlets module not installed. Install with:" -ForegroundColor Red
        Write-Host "  Install-Module -Name AudioDeviceCmdlets -Force -Scope CurrentUser" -ForegroundColor Yellow
        return
    }
    Import-Module AudioDeviceCmdlets
    Write-Host "`n=== AUDIO DEVICES ===" -ForegroundColor Cyan
    $devices = Get-AudioDevice -List
    Write-Host "`n--- PLAYBACK (Output) ---" -ForegroundColor Yellow
    $devices | Where-Object { $_.Type -eq "Playback" } | ForEach-Object {
        $mark = if ($_.Default) { "[DEFAULT] " } else { "          " }
        $color = if ($_.Default) { "Green" } else { "DarkGray" }
        Write-Host "$mark$($_.Name)" -ForegroundColor $color
    }
    Write-Host "`n--- RECORDING (Input) ---" -ForegroundColor Yellow
    $devices | Where-Object { $_.Type -eq "Recording" } | ForEach-Object {
        $mark = if ($_.Default) { "[DEFAULT] " } else { "          " }
        $color = if ($_.Default) { "Green" } else { "DarkGray" }
        Write-Host "$mark$($_.Name)" -ForegroundColor $color
    }
    try {
        $volume = (Get-AudioDevice -PlaybackVolume) -replace '[^\d.]',''
        $mute = Get-AudioDevice -PlaybackMute
        $muteStatus = if ($mute) { " (MUTED)" } else { "" }
        Write-Host "`nDefault output volume: $volume%$muteStatus" -ForegroundColor Cyan
    } catch {}
    Write-Host ""
}
Set-Alias -Name audio -Value Get-AudioInfo
