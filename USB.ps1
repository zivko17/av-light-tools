function Get-USBDevices {
    Write-Host "`n=== USB DEVICES ===" -ForegroundColor Cyan
    Write-Host ""

    $devices = Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue |
               Where-Object { $_.InstanceId -like "USB*" -and $_.Status -eq "OK" } |
               Sort-Object FriendlyName

    if (-not $devices) {
        Write-Host "  No USB devices detected." -ForegroundColor Yellow
        return
    }

    $grouped = $devices | Group-Object Class

    foreach ($g in $grouped | Sort-Object Name) {
        $class = if ($g.Name) { $g.Name } else { "Other" }
        Write-Host "  --- $class ---" -ForegroundColor Yellow
        foreach ($d in $g.Group) {
            $name = if ($d.FriendlyName) { $d.FriendlyName } else { "(no name)" }
            Write-Host "    $name" -ForegroundColor Green
        }
        Write-Host ""
    }

    Write-Host "  Total: $($devices.Count) USB devices`n" -ForegroundColor Cyan
}
Set-Alias -Name usb -Value Get-USBDevices
