function Get-BatteryStatus {
    $battery = Get-CimInstance -ClassName Win32_Battery -ErrorAction SilentlyContinue

    if (-not $battery) {
        Write-Host "`n  No battery detected (desktop PC?).`n" -ForegroundColor Yellow
        return
    }

    Write-Host "`n=== BATTERY STATUS ===" -ForegroundColor Cyan
    Write-Host ""

    $statusMap = @{
        1  = "Discharging"
        2  = "Plugged in (AC)"
        3  = "Fully charged"
        4  = "Low"
        5  = "Critical"
        6  = "Charging"
        7  = "Charging and High"
        8  = "Charging and Low"
        9  = "Charging and Critical"
        10 = "Undefined"
        11 = "Partially Charged"
    }

    $pct = $battery.EstimatedChargeRemaining
    $status = $statusMap[[int]$battery.BatteryStatus]
    if (-not $status) { $status = "Unknown ($($battery.BatteryStatus))" }

    # Color based on percentage
    $color = if ($pct -lt 20) { "Red" } elseif ($pct -lt 50) { "Yellow" } else { "Green" }

    # Bar
    $filled = [math]::Floor($pct / 5)
    $empty = 20 - $filled
    $bar = "[" + ("#" * $filled) + ("." * $empty) + "]"

    Write-Host "  Charge:   $bar $pct%" -ForegroundColor $color
    Write-Host "  Status:   $status"

    # Time remaining
    if ($battery.EstimatedRunTime -and $battery.EstimatedRunTime -lt 71582788) {
        $hours = [math]::Floor($battery.EstimatedRunTime / 60)
        $minutes = $battery.EstimatedRunTime % 60
        Write-Host "  Time:     ${hours}h ${minutes}m remaining"
    } else {
        Write-Host "  Time:     (calculating or plugged in)"
    }

    # Battery health via designed vs full capacity
    try {
        $static = Get-CimInstance -Namespace root\wmi -ClassName BatteryStaticData -ErrorAction SilentlyContinue | Select-Object -First 1
        $full   = Get-CimInstance -Namespace root\wmi -ClassName BatteryFullChargedCapacity -ErrorAction SilentlyContinue | Select-Object -First 1

        if ($static -and $full) {
            $designed = $static.DesignedCapacity
            $current  = $full.FullChargedCapacity
            if ($designed -gt 0) {
                $health = [math]::Round(($current / $designed) * 100, 1)
                $healthColor = if ($health -lt 70) { "Red" } elseif ($health -lt 85) { "Yellow" } else { "Green" }
                Write-Host "  Health:   $health%  (designed: $designed mWh / current: $current mWh)" -ForegroundColor $healthColor
            }
        }
    } catch {}

    Write-Host ""
    Write-Host "  Tip: full detailed report -> powercfg /batteryreport" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name battery -Value Get-BatteryStatus
