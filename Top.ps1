function Get-TopProcesses {
    param([int]$Count = 15)

    Clear-Host
    Write-Host "=== TOP PROCESSES (CPU right now) ===" -ForegroundColor Cyan
    Write-Host "Press Ctrl+C to exit`n" -ForegroundColor DarkGray

    # Per-process % is summed over all cores: divide by the logical CPU count to get 0-100 %.
    $cores = [Environment]::ProcessorCount
    $hasConsole = -not [Console]::IsOutputRedirected
    $cursorTop = 0
    if ($hasConsole) { try { $cursorTop = [Console]::CursorTop } catch { $hasConsole = $false } }

    while ($true) {
        # Same class name on every Windows language, and it gives CPU % now (not total seconds).
        $procs = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -ErrorAction SilentlyContinue |
                 Where-Object { $_.Name -ne '_Total' -and $_.Name -ne 'Idle' } |
                 Sort-Object PercentProcessorTime, WorkingSetPrivate -Descending |
                 Select-Object -First $Count

        Write-Host ("  {0,-30} {1,-8} {2,-8} {3,-12}" -f "PROCESS", "PID", "CPU %", "RAM (MB)") -ForegroundColor Yellow
        foreach ($p in $procs) {
            $cpu = [math]::Round($p.PercentProcessorTime / $cores, 1)
            $mem = [math]::Round($p.WorkingSetPrivate / 1MB, 0)
            $name = $p.Name -replace '#\d+$', ''
            if ($name.Length -gt 28) { $name = $name.Substring(0, 28) + '..' }
            $color = if ($cpu -ge 50) { 'Red' } elseif ($cpu -ge 15) { 'Yellow' } else { 'Green' }
            Write-Host ("  {0,-30} {1,-8} {2,-8} {3,-12}" -f $name, $p.IDProcess, $cpu, $mem) -ForegroundColor $color
        }
        Write-Host ("`n  Updated: {0}   |   killp <name> to close one   " -f (Get-Date -Format 'HH:mm:ss')) -ForegroundColor DarkGray

        Start-Sleep -Seconds 2
        if ($hasConsole) { try { [Console]::SetCursorPosition(0, $cursorTop) } catch {} }
    }
}
Set-Alias -Name top -Value Get-TopProcesses
