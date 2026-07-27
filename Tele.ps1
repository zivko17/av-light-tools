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
    $prevRx = $null; $prevTx = $null

    while ($true) {
        $hasConsole = -not [Console]::IsOutputRedirected
        $cursorTop = 0
        if ($hasConsole) {
            try { $cursorTop = [Console]::CursorTop } catch { $hasConsole = $false }
        }

        # English counter path (also localized on non-EN Windows -> falls back to 0 there)
        $cpu = 0
        $cpuCounter = Get-Counter "\Processor(_Total)\% Processor Time" -ErrorAction SilentlyContinue
        if ($cpuCounter -and $cpuCounter.CounterSamples) {
            $cpu = [math]::Round($cpuCounter.CounterSamples[0].CookedValue, 1)
        }

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

        $net = Get-NetAdapterStatistics -ErrorAction SilentlyContinue | Where-Object { $_.ReceivedBytes -gt 0 } | Select-Object -First 1
        $rxRate = 0; $txRate = 0
        if ($net) {
            if ($null -ne $prevRx) {
                $rxRate = [math]::Round(($net.ReceivedBytes - $prevRx) / 1MB, 2)
                $txRate = [math]::Round(($net.SentBytes    - $prevTx) / 1MB, 2)
                if ($rxRate -lt 0) { $rxRate = 0 }
                if ($txRate -lt 0) { $txRate = 0 }
            }
            $prevRx = $net.ReceivedBytes
            $prevTx = $net.SentBytes
        }

        $gpu = (Get-Counter "\GPU Engine(*engtype_3D)\Utilization Percentage" -ErrorAction SilentlyContinue).CounterSamples |
               Where-Object { $_.CookedValue -gt 0 } | Measure-Object -Property CookedValue -Sum
        $gpuPct = if ($gpu) { [math]::Round($gpu.Sum, 1) } else { 0 }

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
