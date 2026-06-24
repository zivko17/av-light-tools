function Invoke-Backup {
    param(
        [Parameter(Mandatory=$true)][string]$Source,
        [Parameter(Mandatory=$true)][string]$Destination
    )

    $Source      = ($Source      -replace '^"|"$','').Trim()
    $Destination = ($Destination -replace '^"|"$','').Trim()

    if (-not (Test-Path -LiteralPath $Source)) {
        Write-Host "  ERROR: Source path '$Source' does not exist." -ForegroundColor Red
        return
    }

    if (-not (Test-Path -LiteralPath $Destination)) {
        Write-Host "  Creating destination folder: $Destination" -ForegroundColor Yellow
        New-Item -Path $Destination -ItemType Directory -Force | Out-Null
    }

    $isFile     = -not (Get-Item -LiteralPath $Source).PSIsContainer
    $sourceName = Split-Path $Source -Leaf
    $timestamp  = Get-Date -Format "yyyy-MM-dd_HH-mm"
    $backupName = "${sourceName}_backup_${timestamp}"
    $backupPath = Join-Path $Destination $backupName

    Write-Host "`n  Backing up:" -ForegroundColor Cyan
    Write-Host "    Source:      $Source"
    Write-Host "    Destination: $backupPath"
    Write-Host ""

    $sizeBefore = (Get-ChildItem -LiteralPath $Source -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    $sizeMB = [math]::Round($sizeBefore / 1MB, 1)
    Write-Host "    Size:        $sizeMB MB" -ForegroundColor DarkGray
    Write-Host ""

    $start = Get-Date
    # robocopy is an external exe: it does NOT throw. Exit codes 0-7 = success/partial,
    # 8+ = failure. A single file needs <srcDir> <dstDir> <fileName>; a folder uses /E.
    if ($isFile) {
        $srcDir   = Split-Path $Source -Parent
        $fileName = Split-Path $Source -Leaf
        $robocopyArgs = @($srcDir, $backupPath, $fileName, "/COPY:DAT", "/R:1", "/W:1", "/NFL", "/NDL", "/NJH", "/NJS", "/NC", "/NS", "/NP")
    } else {
        $robocopyArgs = @($Source, $backupPath, "/E", "/COPY:DAT", "/R:1", "/W:1", "/NFL", "/NDL", "/NJH", "/NJS", "/NC", "/NS", "/NP")
    }
    & robocopy @robocopyArgs | Out-Null
    $code = $LASTEXITCODE
    $elapsed = (Get-Date) - $start

    if ($code -ge 8) {
        Write-Host "  ERROR: backup failed (robocopy exit code $code). Files may be incomplete." -ForegroundColor Red
        Write-Host ""
        return
    }

    $h = [math]::Floor($elapsed.TotalHours)
    Write-Host ("  Done in {0}h {1}m {2}s  (robocopy code {3})" -f $h, $elapsed.Minutes, $elapsed.Seconds, $code) -ForegroundColor Green
    Write-Host "  Backup saved to: $backupPath" -ForegroundColor Cyan
    Write-Host ""
}
Set-Alias -Name backup -Value Invoke-Backup
