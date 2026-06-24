function Invoke-SystemCleanup {
    Write-Host "`n=== SYSTEM CLEANUP ===" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "  This deletes temp files, browser caches and empties the Recycle Bin." -ForegroundColor Yellow
    Write-Host "  Files currently in use are skipped automatically." -ForegroundColor DarkGray
    Write-Host ""
    Write-Host "  Continue? (y/N): " -NoNewline -ForegroundColor Cyan
    $confirm = Read-Host
    if ($confirm -notmatch '^(y|yes)$') { Write-Host "  Cancelled.`n" -ForegroundColor DarkGray; return }
    Write-Host ""

    $targets = @(
        @{ Name = "User Temp";        Path = "$env:TEMP\*" },
        @{ Name = "Windows Temp";     Path = "C:\Windows\Temp\*" },
        @{ Name = "Prefetch";         Path = "C:\Windows\Prefetch\*" },
        @{ Name = "Browser caches";   Path = "$env:LOCALAPPDATA\Google\Chrome\User Data\Default\Cache\*" },
        @{ Name = "Edge cache";       Path = "$env:LOCALAPPDATA\Microsoft\Edge\User Data\Default\Cache\*" }
    )

    $totalFreed = 0

    foreach ($t in $targets) {
        # Measure before, delete, then re-measure: 'freed' = what actually went away.
        # Locked / in-use files survive the delete and are NOT counted as freed.
        $before = (Get-ChildItem -Path $t.Path -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
        if (-not $before) { $before = 0 }

        Remove-Item -Path $t.Path -Recurse -Force -ErrorAction SilentlyContinue

        $after = (Get-ChildItem -Path $t.Path -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
        if (-not $after) { $after = 0 }

        $freed = $before - $after
        if ($freed -lt 0) { $freed = 0 }
        $totalFreed += $freed
        $freedMB = [math]::Round($freed / 1MB, 1)

        if ($freed -gt 0) {
            Write-Host ("  {0,-25} {1,8} MB freed" -f $t.Name, $freedMB) -ForegroundColor Green
        } elseif ($before -gt 0) {
            Write-Host ("  {0,-25} in use / locked (skipped)" -f $t.Name) -ForegroundColor Yellow
        } else {
            Write-Host ("  {0,-25} already clean" -f $t.Name) -ForegroundColor DarkGray
        }
    }

    # Empty Recycle Bin
    try {
        Clear-RecycleBin -Force -ErrorAction Stop
        Write-Host ("  {0,-25} {1}" -f "Recycle Bin", "emptied") -ForegroundColor Green
    } catch {
        Write-Host ("  {0,-25} {1}" -f "Recycle Bin", "already empty or skipped") -ForegroundColor DarkGray
    }

    $totalMB = [math]::Round($totalFreed / 1MB, 1)
    $totalGB = [math]::Round($totalFreed / 1GB, 2)

    Write-Host ""
    if ($totalGB -ge 1) {
        Write-Host "  TOTAL FREED: $totalGB GB" -ForegroundColor Cyan
    } else {
        Write-Host "  TOTAL FREED: $totalMB MB" -ForegroundColor Cyan
    }
    Write-Host ""
}
Set-Alias -Name cleanup -Value Invoke-SystemCleanup
