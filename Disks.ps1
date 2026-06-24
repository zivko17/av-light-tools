function Get-DisksInfo {
    Write-Host "`n=== DISKS INFO ===" -ForegroundColor Cyan
    Write-Host ""

    $physicalDisks = Get-PhysicalDisk -ErrorAction SilentlyContinue

    if (-not $physicalDisks) {
        Write-Host "  Could not enumerate physical disks." -ForegroundColor Yellow
        return
    }

    Write-Host "  --- Physical Disks ---" -ForegroundColor Yellow
    Write-Host ("    {0,-25} {1,-10} {2,-10} {3,-12} {4,-10}" -f "MODEL", "TYPE", "SIZE", "HEALTH", "TEMP") -ForegroundColor DarkGray
    Write-Host ("    {0,-25} {1,-10} {2,-10} {3,-12} {4,-10}" -f "-------------------------", "----------", "----------", "------------", "----------") -ForegroundColor DarkGray

    foreach ($d in $physicalDisks) {
        $size = [math]::Round($d.Size / 1GB, 0)
        $type = $d.MediaType
        $health = $d.HealthStatus

        $healthColor = switch ($health) {
            "Healthy" { "Green" }
            "Warning" { "Yellow" }
            "Unhealthy" { "Red" }
            default { "DarkGray" }
        }

        # Try to get temperature via reliability counter
        $temp = "-"
        try {
            $reliability = $d | Get-StorageReliabilityCounter -ErrorAction SilentlyContinue
            if ($reliability -and $reliability.Temperature) {
                $temp = "$($reliability.Temperature) C"
            }
        } catch {}

        $fname = if ($d.FriendlyName) { $d.FriendlyName } else { "(unknown)" }
        $model = if ($fname.Length -gt 23) { $fname.Substring(0, 23) + ".." } else { $fname }

        Write-Host ("    {0,-25} {1,-10} {2,-10} {3,-12} {4,-10}" -f $model, $type, "$size GB", $health, $temp) -ForegroundColor $healthColor
    }

    Write-Host ""
    Write-Host "  --- Logical Volumes ---" -ForegroundColor Yellow
    Write-Host ("    {0,-6} {1,-15} {2,-12} {3,-12} {4,-8}" -f "DRIVE", "LABEL", "USED", "FREE", "USAGE") -ForegroundColor DarkGray
    Write-Host ("    {0,-6} {1,-15} {2,-12} {3,-12} {4,-8}" -f "------", "---------------", "------------", "------------", "--------") -ForegroundColor DarkGray

    Get-CimInstance Win32_LogicalDisk -Filter "DriveType=3" | ForEach-Object {
        $total = [math]::Round($_.Size / 1GB, 1)
        $free  = [math]::Round($_.FreeSpace / 1GB, 1)
        $used  = [math]::Round($total - $free, 1)
        $pct   = if ($total -gt 0) { [math]::Round(($used / $total) * 100, 0) } else { 0 }

        $color = if ($pct -gt 85) { "Red" } elseif ($pct -gt 70) { "Yellow" } else { "Green" }
        $label = if ($_.VolumeName) { $_.VolumeName } else { "(no label)" }

        Write-Host ("    {0,-6} {1,-15} {2,-12} {3,-12} {4,-8}" -f $_.DeviceID, $label, "$used GB", "$free GB", "$pct%") -ForegroundColor $color
    }
    Write-Host ""
}
Set-Alias -Name disks -Value Get-DisksInfo
