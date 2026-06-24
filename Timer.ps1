function Start-Timer {
    param(
        [Parameter(Mandatory=$true)][string]$Duration,
        [string]$Label = "Timer"
    )

    # Parse duration: "5m", "30s", "1h", "1h30m", "90s"
    $totalSec = 0
    if ($Duration -match '(\d+)h') { $totalSec += [int]$matches[1] * 3600 }
    if ($Duration -match '(\d+)m') { $totalSec += [int]$matches[1] * 60 }
    if ($Duration -match '(\d+)s') { $totalSec += [int]$matches[1] }
    if ($totalSec -eq 0 -and $Duration -match '^\d+$') { $totalSec = [int]$Duration * 60 }

    if ($totalSec -eq 0) {
        Write-Host "  Invalid duration. Examples: 5m, 30s, 1h30m, 90s" -ForegroundColor Red
        return
    }

    $endTime = (Get-Date).AddSeconds($totalSec)
    Write-Host "`n  $Label started - ends at $($endTime.ToString('HH:mm:ss'))" -ForegroundColor Cyan
    Write-Host "  Press Ctrl+C to cancel`n" -ForegroundColor DarkGray

    while ((Get-Date) -lt $endTime) {
        $remaining = $endTime - (Get-Date)
        $h = $remaining.Hours
        $m = $remaining.Minutes
        $s = $remaining.Seconds

        if ($h -gt 0) {
            $display = "{0:D2}:{1:D2}:{2:D2}" -f $h, $m, $s
        } else {
            $display = "{0:D2}:{1:D2}" -f $m, $s
        }

        $color = if ($remaining.TotalSeconds -lt 10) { "Red" } elseif ($remaining.TotalSeconds -lt 60) { "Yellow" } else { "Green" }

        Write-Host ("`r  $Label : $display    ") -NoNewline -ForegroundColor $color
        Start-Sleep -Seconds 1
    }

    Write-Host ("`r  $Label : DONE!        ") -ForegroundColor Green
    Write-Host ""

    # Beep three times
    for ($i = 0; $i -lt 3; $i++) {
        try {
            [console]::beep(1000, 300)
        } catch {}
        Start-Sleep -Milliseconds 200
    }
}
Set-Alias -Name timer -Value Start-Timer
