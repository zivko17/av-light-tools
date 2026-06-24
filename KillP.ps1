function Stop-ProcessByName {
    param(
        [Parameter(Mandatory=$true)]
        [string]$Name,
        # By default match the EXACT process name. Use -Wildcard to match substrings
        # (e.g. 'chrome' -> chrome.exe). Exact-by-default avoids 'svc' killing svchost.
        [switch]$Wildcard
    )

    $pattern = if ($Wildcard) { "*$Name*" } else { $Name }
    $procs = Get-Process -Name $pattern -ErrorAction SilentlyContinue
    if (-not $procs) {
        Write-Host "No process found matching '$Name'." -ForegroundColor Yellow
        if (-not $Wildcard) { Write-Host "(exact name match; add -Wildcard for partial matches)" -ForegroundColor DarkGray }
        return
    }

    $critical = @('svchost','csrss','wininit','services','lsass','winlogon','smss','dwm','explorer','System','Idle','fontdrvhost','ctfmon')

    Write-Host "`nProcesses matching '$Name':" -ForegroundColor Cyan
    Write-Host ("  {0,-30} {1,-10} {2,-12}" -f "PROCESS", "PID", "MEMORY (MB)") -ForegroundColor Yellow
    Write-Host ("  {0,-30} {1,-10} {2,-12}" -f "------------------------------", "----------", "------------") -ForegroundColor DarkGray

    $hasCritical = $false
    foreach ($p in $procs) {
        $memMB  = [math]::Round($p.WorkingSet64 / 1MB, 1)
        $isCrit = $critical -contains $p.ProcessName
        if ($isCrit) { $hasCritical = $true }
        $color  = if ($isCrit) { 'Red' } else { 'White' }
        $tag    = if ($isCrit) { '   <- CRITICAL' } else { '' }
        Write-Host ("  {0,-30} {1,-10} {2,-12}{3}" -f $p.ProcessName, $p.Id, $memMB, $tag) -ForegroundColor $color
    }

    if ($hasCritical) {
        Write-Host "`n  WARNING: this matches critical system processes. Killing them can crash Windows." -ForegroundColor Red
    }

    Write-Host "`nKill all $($procs.Count) processes? (y/n): " -NoNewline -ForegroundColor Cyan
    $confirm = Read-Host

    if ($confirm -eq 'y' -or $confirm -eq 'Y') {
        foreach ($p in $procs) {
            try {
                Stop-Process -Id $p.Id -Force -ErrorAction Stop
                Write-Host "  Killed: $($p.ProcessName) (PID $($p.Id))" -ForegroundColor Green
            } catch {
                Write-Host "  Failed: $($p.ProcessName) (PID $($p.Id)) - $($_.Exception.Message)" -ForegroundColor Red
            }
        }
    } else {
        Write-Host "Cancelled." -ForegroundColor DarkGray
    }
    Write-Host ""
}
Set-Alias -Name killp -Value Stop-ProcessByName
