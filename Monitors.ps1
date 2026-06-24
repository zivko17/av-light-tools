function Get-Monitors {
    Write-Host "`n=== CONNECTED MONITORS ===" -ForegroundColor Cyan
    Add-Type -AssemblyName System.Windows.Forms
    $screens = [System.Windows.Forms.Screen]::AllScreens
    $i = 1
    foreach ($s in $screens) {
        $primary = if ($s.Primary) { " (PRIMARY)" } else { "" }
        Write-Host "`n[Monitor $i]$primary" -ForegroundColor Yellow
        Write-Host "  Name:        $($s.DeviceName)" -ForegroundColor White
        Write-Host "  Resolution:  $($s.Bounds.Width) x $($s.Bounds.Height)" -ForegroundColor Green
        Write-Host "  Position:    X=$($s.Bounds.X)  Y=$($s.Bounds.Y)" -ForegroundColor DarkGray
        Write-Host "  Work area:   $($s.WorkingArea.Width) x $($s.WorkingArea.Height)" -ForegroundColor DarkGray
        $i++
    }
    Write-Host "`n=== REFRESH RATE ===" -ForegroundColor Cyan
    Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorListedSupportedSourceModes -ErrorAction SilentlyContinue | ForEach-Object {
        $activeMode = $_.MonitorSourceModes | Where-Object { $_.HorizontalActivePixels -gt 0 } | Sort-Object VerticalRefreshRateNumerator -Descending | Select-Object -First 1
        if ($activeMode -and $activeMode.VerticalRefreshRateDenominator -gt 0) {
            $hz = [math]::Round($activeMode.VerticalRefreshRateNumerator / $activeMode.VerticalRefreshRateDenominator, 0)
            Write-Host "  $($activeMode.HorizontalActivePixels) x $($activeMode.VerticalActivePixels) @ ${hz}Hz" -ForegroundColor Green
        }
    }
    Write-Host "`n=== PHYSICAL INFO ===" -ForegroundColor Cyan
    Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction SilentlyContinue | ForEach-Object {
        $manufacturer = -join ($_.ManufacturerName | Where-Object {$_ -ne 0} | ForEach-Object {[char]$_})
        $model = -join ($_.UserFriendlyName | Where-Object {$_ -ne 0} | ForEach-Object {[char]$_})
        $serial = -join ($_.SerialNumberID | Where-Object {$_ -ne 0} | ForEach-Object {[char]$_})
        Write-Host "  Manufacturer: $manufacturer  |  Model: $model  |  S/N: $serial" -ForegroundColor White
    }
    Write-Host ""
}
Set-Alias -Name monitors -Value Get-Monitors
