function Set-MonitorMode {
    Clear-Host
    Write-Host "`n=== MONITOR CONFIGURATOR ===" -ForegroundColor Cyan
    Write-Host ""

    # Get all monitors via WMI
    $monitors = Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction SilentlyContinue
    $modesAll = Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorListedSupportedSourceModes -ErrorAction SilentlyContinue

    if (-not $monitors -or -not $modesAll) {
        Write-Host "  Could not enumerate monitors via WMI." -ForegroundColor Red
        return
    }

    # Match monitors with their modes
    $list = @()
    $i = 1
    foreach ($m in $monitors) {
        $name = -join ($m.UserFriendlyName | Where-Object {$_ -ne 0} | ForEach-Object {[char]$_})
        if (-not $name) { $name = "Monitor $i" }
        $modes = $modesAll | Where-Object { $_.InstanceName -eq $m.InstanceName } | Select-Object -First 1
        $list += [PSCustomObject]@{
            Index = $i
            Name  = $name
            Modes = $modes
        }
        $i++
    }

    # Step 1: pick monitor
    Write-Host "  --- Step 1: Select monitor ---`n" -ForegroundColor Yellow
    foreach ($mon in $list) {
        Write-Host ("    {0}) {1}" -f $mon.Index, $mon.Name) -ForegroundColor Green
    }
    Write-Host ""
    $pick = Read-Host "  Monitor number"
    $pickNum = 0
    if (-not [int]::TryParse($pick, [ref]$pickNum)) { Write-Host "  Invalid selection." -ForegroundColor Red; return }
    $selected = $list | Where-Object { $_.Index -eq $pickNum } | Select-Object -First 1
    if (-not $selected) { Write-Host "  Invalid selection." -ForegroundColor Red; return }

    if (-not $selected.Modes) {
        Write-Host "  No supported modes available for this monitor." -ForegroundColor Yellow
        return
    }

    # Step 2: list resolutions
    $supported = $selected.Modes.MonitorSourceModes | Where-Object { $_.HorizontalActivePixels -gt 0 }
    $resolutions = $supported | Group-Object { "$($_.HorizontalActivePixels)x$($_.VerticalActivePixels)" } |
                   Sort-Object { [int](($_.Name -split 'x')[0]) } -Descending

    Clear-Host
    Write-Host "`n  Monitor: $($selected.Name)" -ForegroundColor Cyan
    Write-Host "  --- Step 2: Select resolution ---`n" -ForegroundColor Yellow
    $r = 1
    $resMap = @{}
    foreach ($res in $resolutions) {
        Write-Host ("    {0,2}) {1}" -f $r, $res.Name) -ForegroundColor Green
        $resMap[$r] = $res
        $r++
    }
    Write-Host ""
    $pickRes = Read-Host "  Resolution number"
    $rNum = 0
    if (-not [int]::TryParse($pickRes, [ref]$rNum)) { Write-Host "  Invalid selection." -ForegroundColor Red; return }
    $selectedRes = $resMap[$rNum]
    if (-not $selectedRes) { Write-Host "  Invalid selection." -ForegroundColor Red; return }

    # Step 3: list refresh rates for that resolution
    $rates = $selectedRes.Group | Where-Object { $_.VerticalRefreshRateDenominator -gt 0 } | ForEach-Object {
        [math]::Round($_.VerticalRefreshRateNumerator / $_.VerticalRefreshRateDenominator, 0)
    } | Sort-Object -Unique -Descending

    Clear-Host
    Write-Host "`n  Monitor: $($selected.Name)" -ForegroundColor Cyan
    Write-Host "  Resolution: $($selectedRes.Name)" -ForegroundColor Cyan
    Write-Host "  --- Step 3: Select refresh rate ---`n" -ForegroundColor Yellow
    $h = 1
    $hzMap = @{}
    foreach ($hz in $rates) {
        Write-Host ("    {0}) {1} Hz" -f $h, $hz) -ForegroundColor Green
        $hzMap[$h] = $hz
        $h++
    }
    Write-Host ""
    $pickHz = Read-Host "  Refresh rate number"
    $hNum = 0
    if (-not [int]::TryParse($pickHz, [ref]$hNum)) { Write-Host "  Invalid selection." -ForegroundColor Red; return }
    $selectedHz = $hzMap[$hNum]
    if (-not $selectedHz) { Write-Host "  Invalid selection." -ForegroundColor Red; return }

    # Step 4: apply with nircmd
    $resParts = $selectedRes.Name -split 'x'
    $width = $resParts[0]
    $height = $resParts[1]

    Write-Host ""
    Write-Host "  Applying: $($selected.Name) -> ${width}x${height} @ ${selectedHz}Hz" -ForegroundColor Cyan

    $nircmd = Get-Command nircmd.exe -ErrorAction SilentlyContinue
    if (-not $nircmd) {
        $local = Join-Path $PSScriptRoot 'nircmd.exe'
        if (Test-Path -LiteralPath $local) {
            $nircmd = $local
        } else {
            Write-Host "  ERROR: nircmd.exe not found in PATH or $PSScriptRoot." -ForegroundColor Red
            Write-Host "  Get it from https://www.nirsoft.net/utils/nircmd.html and drop nircmd.exe in $PSScriptRoot" -ForegroundColor DarkGray
            return
        }
    } else {
        $nircmd = $nircmd.Source
    }

    # nircmd setdisplay monitor:<index> width height bpp(32) refresh-rate
    # 'monitor:N' targets a specific display (N = display number as Windows numbers them).
    & $nircmd setdisplay "monitor:$($selected.Index)" $width $height 32 $selectedHz | Out-Null
    $code = $LASTEXITCODE
    Start-Sleep -Seconds 2

    if ($code -ne 0) {
        Write-Host "  nircmd returned exit code $code - the change may not have applied." -ForegroundColor Red
    } else {
        Write-Host "  Sent to nircmd. Verify '$($selected.Name)' now shows ${width}x${height} @ ${selectedHz}Hz." -ForegroundColor Green
    }
    Write-Host ""
}
Set-Alias -Name monitors-arrange -Value Set-MonitorMode
