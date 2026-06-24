function Get-TopProcesses {
    param([int]$Count = 10)

    Clear-Host
    Write-Host "=== TOP PROCESSES (CPU/RAM) ===" -ForegroundColor Cyan
    Write-Host "Press Ctrl+C to exit`n" -ForegroundColor DarkGray

    # Total RAM is static; query it once before the loop, not every tick
    $totalRamMB = (Get-CimInstance Win32_OperatingSystem).TotalVisibleMemorySize / 1KB

    while ($true) {
        $hasConsole = -not [Console]::IsOutputRedirected
        $cursorTop = 0
        if ($hasConsole) {
            try { $cursorTop = [Console]::CursorTop } catch { $hasConsole = $false }
        }

        # Get processes with CPU and Memory usage
        $procs = Get-Process | Where-Object { $_.CPU -ne $null } |
                 Sort-Object CPU -Descending |
                 Select-Object -First $Count

        Write-Host ("  {0,-30} {1,-10} {2,-12} {3,-8}" -f "PROCESS", "PID", "MEMORY (MB)", "CPU (s)") -ForegroundColor Yellow
        Write-Host ("  {0,-30} {1,-10} {2,-12} {3,-8}" -f "------------------------------", "----------", "------------", "--------") -ForegroundColor DarkGray

        foreach ($p in $procs) {
            $memMB = [math]::Round($p.WorkingSet64 / 1MB, 1)
            $cpuSec = [math]::Round($p.CPU, 1)
            $memPct = if ($totalRamMB -gt 0) { ($memMB / $totalRamMB) * 100 } else { 0 }

            $color = if ($memPct -gt 10) { "Red" } elseif ($memPct -gt 5) { "Yellow" } else { "Green" }
            $name = if ($p.ProcessName.Length -gt 28) { $p.ProcessName.Substring(0, 28) + ".." } else { $p.ProcessName }

            Write-Host ("  {0,-30} {1,-10} {2,-12} {3,-8}" -f $name, $p.Id, $memMB, $cpuSec) -ForegroundColor $color
        }

        Write-Host ("`n  Updated: {0}   |   Tip: killp <name>  to terminate" -f (Get-Date -Format 'HH:mm:ss')) -ForegroundColor DarkGray

        Start-Sleep -Seconds 2
        if ($hasConsole) {
            try { [Console]::SetCursorPosition(0, $cursorTop) } catch {}
        }
    }
}
Set-Alias -Name top -Value Get-TopProcesses
