function Get-SystemTelemetry {
    Clear-Host
    Write-Host "=== SYSTEM TELEMETRY ===" -ForegroundColor Cyan
    Write-Host "Press Ctrl+C to exit`n" -ForegroundColor DarkGray

    function Bar($pct, $color) {
        # Clamp 0..100 so the bar width can never go negative
        if ($pct -lt 0)   { $pct = 0 }
        if ($pct -gt 100) { $pct = 100 }
        $filled = [math]::Floor($pct / 5)
        $empty = 20 - $filled
        Write-Host ("[" + ("#" * $filled) + ("." * $empty) + "] $pct%") -ForegroundColor $color
    }

    # Track previous NET counters so we can show a rate, not a since-boot total
    $prevRx = $null; $prevTx = $null; $prevT = $null
    $netName = if (Get-Command Resolve-NetIface -ErrorAction SilentlyContinue) { Resolve-NetIface } else { $null }

    while ($true) {
        $hasConsole = -not [Console]::IsOutputRedirected
        $cursorTop = 0
        if ($hasConsole) {
            try { $cursorTop = [Console]::CursorTop } catch { $hasConsole = $false }
        }

        # WMI performance classes: same names on every Windows language (Get-Counter
        # paths are translated, so they return nothing on a Spanish Windows).
        $cpu = 0
        $cpuRow = Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor -Filter "Name='_Total'" -ErrorAction SilentlyContinue
        if ($cpuRow) { $cpu = [double]$cpuRow.PercentProcessorTime }

        $os = Get-CimInstance Win32_OperatingSystem -ErrorAction SilentlyContinue
        $ramTotal = 0; $ramFree = 0; $ramUsed = 0; $ramPct = 0
        if ($os -and $os.TotalVisibleMemorySize -gt 0) {
            $ramTotal = [math]::Round($os.TotalVisibleMemorySize / 1MB, 2)
            $ramFree  = [math]::Round($os.FreePhysicalMemory / 1MB, 2)
            $ramUsed  = [math]::Round($ramTotal - $ramFree, 2)
            if ($ramTotal -gt 0) { $ramPct = [math]::Round(($ramUsed / $ramTotal) * 100, 1) }
        }

        $disk = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='C:'" -ErrorAction SilentlyContinue
        $diskTotal = 0; $diskFree = 0; $diskPct = 0
        if ($disk -and $disk.Size -gt 0) {
            $diskTotal = [math]::Round($disk.Size / 1GB, 1)
            $diskFree  = [math]::Round($disk.FreeSpace / 1GB, 1)
            $diskPct   = [math]::Round((($diskTotal - $diskFree) / $diskTotal) * 100, 1)
        }

        $net = if ($netName) { Get-NetAdapterStatistics -Name $netName -ErrorAction SilentlyContinue }
        if (-not $net) { $net = Get-NetAdapterStatistics -ErrorAction SilentlyContinue | Where-Object { $_.ReceivedBytes -gt 0 } | Select-Object -First 1 }
        $rxRate = 0; $txRate = 0
        $now = Get-Date
        if ($net) {
            if ($null -ne $prevRx) {
                $dt = ($now - $prevT).TotalSeconds; if ($dt -le 0) { $dt = 1 }
                $rxRate = [math]::Round(($net.ReceivedBytes - $prevRx) / 1MB / $dt, 2)
                $txRate = [math]::Round(($net.SentBytes    - $prevTx) / 1MB / $dt, 2)
                if ($rxRate -lt 0) { $rxRate = 0 }
                if ($txRate -lt 0) { $txRate = 0 }
            }
            $prevRx = $net.ReceivedBytes
            $prevTx = $net.SentBytes
            $prevT  = $now
        }

        $gpu = Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine -ErrorAction SilentlyContinue |
               Where-Object { $_.Name -like '*engtype_3D' } | Measure-Object -Property UtilizationPercentage -Sum
        $gpuPct = if ($gpu -and $gpu.Sum) { [math]::Round([double]$gpu.Sum, 1) } else { 0 }

        $colorCPU = if ($cpu -gt 80) {"Red"} elseif ($cpu -gt 50) {"Yellow"} else {"Green"}
        $colorRAM = if ($ramPct -gt 80) {"Red"} elseif ($ramPct -gt 50) {"Yellow"} else {"Green"}
        $colorDSK = if ($diskPct -gt 85) {"Red"} elseif ($diskPct -gt 70) {"Yellow"} else {"Green"}
        $colorGPU = if ($gpuPct -gt 80) {"Red"} elseif ($gpuPct -gt 50) {"Yellow"} else {"Green"}
        Write-Host ("CPU  ") -NoNewline; Bar $cpu $colorCPU
        Write-Host ("RAM  ") -NoNewline; Bar $ramPct $colorRAM
        Write-Host ("     $ramUsed GB / $ramTotal GB") -ForegroundColor DarkGray
        Write-Host ("GPU  ") -NoNewline; Bar $gpuPct $colorGPU
        Write-Host ("DISK ") -NoNewline; Bar $diskPct $colorDSK
        Write-Host ("     $($diskTotal - $diskFree) GB / $diskTotal GB used") -ForegroundColor DarkGray
        Write-Host ("NET  down: $rxRate MB/s  |  up: $txRate MB/s") -ForegroundColor Cyan
        Write-Host ("`nUpdated: $(Get-Date -Format 'HH:mm:ss')") -ForegroundColor DarkGray
        Start-Sleep -Seconds 1
        if ($hasConsole) {
            try { [Console]::SetCursorPosition(0, $cursorTop) } catch {}
        }
    }
}
Set-Alias -Name tele -Value Get-SystemTelemetry
