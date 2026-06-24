function Get-SystemInfo {
    Write-Host "`n=== SYSTEM INFO ===" -ForegroundColor Cyan
    Write-Host ""

    $cs   = Get-CimInstance Win32_ComputerSystem
    $bios = Get-CimInstance Win32_BIOS
    $os   = Get-CimInstance Win32_OperatingSystem
    $cpu  = Get-CimInstance Win32_Processor | Select-Object -First 1
    $gpus = Get-CimInstance Win32_VideoController | Where-Object { $_.Name -notmatch "Basic|Remote" }

    Write-Host "  --- Identity ---" -ForegroundColor Yellow
    Write-Host ("    Hostname:       {0}" -f $env:COMPUTERNAME)
    Write-Host ("    Manufacturer:   {0}" -f $cs.Manufacturer)
    Write-Host ("    Model:          {0}" -f $cs.Model)
    Write-Host ("    Serial Number:  {0}" -f $bios.SerialNumber)
    Write-Host ("    BIOS Version:   {0}" -f $bios.SMBIOSBIOSVersion)
    Write-Host ""

    Write-Host "  --- Operating System ---" -ForegroundColor Yellow
    Write-Host ("    OS:             {0}" -f $os.Caption)
    Write-Host ("    Version:        {0}  (build {1})" -f $os.Version, $os.BuildNumber)
    Write-Host ("    Architecture:   {0}" -f $os.OSArchitecture)
    Write-Host ("    Install Date:   {0}" -f $os.InstallDate)
    $uptime = (Get-Date) - $os.LastBootUpTime
    Write-Host ("    Uptime:         {0}d {1}h {2}m" -f $uptime.Days, $uptime.Hours, $uptime.Minutes)
    Write-Host ""

    Write-Host "  --- CPU ---" -ForegroundColor Yellow
    Write-Host ("    Name:           {0}" -f $cpu.Name.Trim())
    Write-Host ("    Cores:          {0} physical / {1} logical" -f $cpu.NumberOfCores, $cpu.NumberOfLogicalProcessors)
    Write-Host ("    Base Speed:     {0} MHz" -f $cpu.MaxClockSpeed)
    Write-Host ""

    Write-Host "  --- RAM ---" -ForegroundColor Yellow
    $ramTotal = [math]::Round($cs.TotalPhysicalMemory / 1GB, 1)
    Write-Host ("    Total:          {0} GB" -f $ramTotal)
    $sticks = Get-CimInstance Win32_PhysicalMemory
    foreach ($s in $sticks) {
        $cap = [math]::Round($s.Capacity / 1GB, 0)
        Write-Host ("    Slot:           {0} GB @ {1} MHz  ({2})" -f $cap, $s.Speed, $s.Manufacturer) -ForegroundColor DarkGray
    }
    Write-Host ""

    # Read accurate VRAM from the registry: AdapterRAM is a UInt32 that caps at 4 GB
    # and under-reports large GPUs. qwMemorySize (Int64) is the real value when present.
    $gpuRegMem = @(Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0*' -ErrorAction SilentlyContinue |
                   ForEach-Object { $_.'HardwareInformation.qwMemorySize' } |
                   Where-Object { $_ -and $_ -gt 0 })

    Write-Host "  --- GPU ---" -ForegroundColor Yellow
    $gpuIndex = 0
    foreach ($g in $gpus) {
        $vram = "?"
        if ($gpuIndex -lt $gpuRegMem.Count -and $gpuRegMem[$gpuIndex] -gt 0) {
            $vram = [math]::Round([int64]$gpuRegMem[$gpuIndex] / 1GB, 1)
        } elseif ($g.AdapterRAM) {
            $vram = [math]::Round($g.AdapterRAM / 1GB, 1)
        }
        Write-Host ("    Name:           {0}" -f $g.Name)
        Write-Host ("    VRAM:           {0} GB" -f $vram) -ForegroundColor DarkGray
        Write-Host ("    Driver:         {0}  ({1})" -f $g.DriverVersion, $g.DriverDate) -ForegroundColor DarkGray
        $gpuIndex++
    }
    Write-Host ""

    Write-Host "  --- Storage Summary ---" -ForegroundColor Yellow
    Get-CimInstance Win32_LogicalDisk -Filter "DriveType=3" | ForEach-Object {
        $total = [math]::Round($_.Size / 1GB, 1)
        $free  = [math]::Round($_.FreeSpace / 1GB, 1)
        $used  = [math]::Round($total - $free, 1)
        $pct   = if ($total -gt 0) { [math]::Round(($used / $total) * 100, 0) } else { 0 }
        Write-Host ("    {0}  {1} GB used / {2} GB total  ({3}%)" -f $_.DeviceID, $used, $total, $pct) -ForegroundColor DarkGray
    }
    Write-Host ""
}
Set-Alias -Name sysinfo -Value Get-SystemInfo
