#Requires -Version 5.1
<#
    AV Light Tools - new-PC bootstrap
    =================================
    One-shot setup for a fresh Windows machine. Run it from the folder that holds
    the scripts (USB stick, cloned repo, copied folder):

        # if scripts are blocked / execution policy stops you, first run:
        Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass

        .\Setup-NewPC.ps1                 # copy to C:\Scripts + set up both profiles + check deps
        .\Setup-NewPC.ps1 -InstallDeps    # also winget-install the missing dependencies
        .\Setup-NewPC.ps1 -Target D:\Tools -SkipProfile -InstallDeps

    What it does:
      1. Copies the tool files (*.ps1 / *.ahk / *.bat + README) to -Target (default C:\Scripts).
      2. Unblocks them (Windows blocks files copied from the network / internet).
      3. Runs install.ps1 for BOTH Windows PowerShell 5.1 and PowerShell 7 so every
         new session loads the tools (they have separate $PROFILE files).
      4. Checks external dependencies and prints the winget commands; with -InstallDeps
         it installs the missing ones for you.
    Re-running is safe. Works in Windows PowerShell 5.1 and PowerShell 7.
#>
[CmdletBinding()]
param(
    [string]$Target = 'C:\Scripts',
    [switch]$InstallDeps,
    [switch]$SkipProfile,
    [switch]$SkipCopy
)

# --- resolve the source folder (where this script lives) ---
if     ($PSScriptRoot)                { $src = $PSScriptRoot }
elseif ($PSCommandPath)               { $src = Split-Path -Parent $PSCommandPath }
elseif ($MyInvocation.MyCommand.Path) { $src = Split-Path -Parent $MyInvocation.MyCommand.Path }
else                                  { $src = (Get-Location).Path }
$src = (Resolve-Path -LiteralPath $src).Path

Write-Host ""
Write-Host "  === AV Light Tools - new-PC bootstrap ===" -ForegroundColor Cyan
Write-Host "  Source: $src" -ForegroundColor DarkGray
Write-Host "  Target: $Target" -ForegroundColor DarkGray
Write-Host ""

# --- 1. copy tool files to the target folder ---
if (-not (Test-Path -LiteralPath $Target)) {
    New-Item -ItemType Directory -Path $Target -Force | Out-Null
}
$Target = (Resolve-Path -LiteralPath $Target).Path

if ($SkipCopy) {
    Write-Host "  [1/4] Copy skipped (-SkipCopy)." -ForegroundColor DarkGray
} elseif ($src -eq $Target) {
    Write-Host "  [1/4] Already running from the target folder - nothing to copy." -ForegroundColor DarkGray
} else {
    Write-Host "  [1/4] Copying tool files..." -ForegroundColor Yellow
    $patterns = @('*.ps1', '*.ahk', '*.bat')
    $extra    = @('README.md', '.gitattributes')
    $copied = 0
    foreach ($pat in $patterns) {
        foreach ($f in (Get-ChildItem -LiteralPath $src -Filter $pat -File -ErrorAction SilentlyContinue)) {
            Copy-Item -LiteralPath $f.FullName -Destination $Target -Force
            $copied++
        }
    }
    foreach ($name in $extra) {
        $p = Join-Path $src $name
        if (Test-Path -LiteralPath $p) { Copy-Item -LiteralPath $p -Destination $Target -Force; $copied++ }
    }
    Write-Host "        $copied files -> $Target" -ForegroundColor Green
}

# --- 2. unblock the copied files ---
Write-Host "  [2/4] Unblocking files..." -ForegroundColor Yellow
Get-ChildItem -LiteralPath $Target -Recurse -File |
    Where-Object { $_.Extension -in '.ps1', '.psm1', '.psd1', '.ahk', '.bat' } |
    Unblock-File -ErrorAction SilentlyContinue
Write-Host "        OK" -ForegroundColor Green

# --- 3. set up the profile loader for both PowerShell editions ---
if ($SkipProfile) {
    Write-Host "  [3/4] Profile setup skipped (-SkipProfile)." -ForegroundColor DarkGray
} else {
    $installer = Join-Path $Target 'install.ps1'
    if (-not (Test-Path -LiteralPath $installer)) {
        Write-Host "  [3/4] install.ps1 not found in target - skipping profile setup." -ForegroundColor Red
    } else {
        Write-Host "  [3/4] Configuring PowerShell profiles..." -ForegroundColor Yellow
        foreach ($edition in @(@{ Exe='powershell.exe'; Name='Windows PowerShell 5.1' }, @{ Exe='pwsh.exe'; Name='PowerShell 7' })) {
            $cmd = Get-Command $edition.Exe -ErrorAction SilentlyContinue
            if (-not $cmd) { Write-Host ("        - {0}: not installed, skipped" -f $edition.Name) -ForegroundColor DarkGray; continue }
            & $cmd.Source -NoProfile -ExecutionPolicy Bypass -File $installer -ScriptDir $Target | Out-Null
            if ($LASTEXITCODE -eq 0 -or $null -eq $LASTEXITCODE) {
                Write-Host ("        - {0}: profile updated" -f $edition.Name) -ForegroundColor Green
            } else {
                Write-Host ("        - {0}: installer exit {1}" -f $edition.Name, $LASTEXITCODE) -ForegroundColor Red
            }
        }
    }
}

# --- 4. external dependencies ---
Write-Host "  [4/4] Checking external dependencies..." -ForegroundColor Yellow
$deps = @(
    @{ Name='FFmpeg (ffmpeg + ffprobe)'; Cmd='ffmpeg.exe';    Winget='Gyan.FFmpeg';           Used='AudioTools, video convert/probe' }
    @{ Name='Ookla Speedtest';           Cmd='speedtest.exe'; Winget='Ookla.Speedtest.CLI';   Used='nettools speedtest' }
    @{ Name='yt-dlp';                    Cmd='yt-dlp.exe';    Winget='yt-dlp.yt-dlp';         Used='dl / media download' }
    @{ Name='NirCmd';                    Cmd='nircmd.exe';    Winget=$null;                   Used='monitors-arrange'; Url='https://www.nirsoft.net/utils/nircmd.html' }
)
$winget = Get-Command winget.exe -ErrorAction SilentlyContinue
$missing = @()
foreach ($d in $deps) {
    $have = Get-Command $d.Cmd -ErrorAction SilentlyContinue
    if ($have) {
        Write-Host ("        [ok]      {0}" -f $d.Name) -ForegroundColor Green
    } else {
        $missing += $d
        Write-Host ("        [missing] {0}   ({1})" -f $d.Name, $d.Used) -ForegroundColor Red
    }
}

if ($missing.Count -gt 0) {
    Write-Host ""
    if ($InstallDeps -and $winget) {
        Write-Host "  Installing missing dependencies via winget..." -ForegroundColor Yellow
        foreach ($d in $missing) {
            if (-not $d.Winget) {
                Write-Host ("        - {0}: no winget package; download from {1}" -f $d.Name, $d.Url) -ForegroundColor DarkGray
                continue
            }
            Write-Host ("        - winget install {0}" -f $d.Winget) -ForegroundColor White
            & winget.exe install --id $d.Winget -e --accept-source-agreements --accept-package-agreements
        }
        Write-Host "  Done. Open a NEW terminal so freshly-installed tools are on PATH." -ForegroundColor Green
    } else {
        Write-Host "  To install the missing dependencies:" -ForegroundColor Yellow
        foreach ($d in $missing) {
            if ($d.Winget) { Write-Host ("        winget install {0}" -f $d.Winget) -ForegroundColor White }
            else           { Write-Host ("        {0}: download from {1}" -f $d.Name, $d.Url) -ForegroundColor DarkGray }
        }
        if (-not $winget) { Write-Host "        (winget not found - install 'App Installer' from the Microsoft Store)" -ForegroundColor DarkGray }
        # Only suggest -InstallDeps when at least one missing dep actually has a winget package
        if (@($missing | Where-Object { $_.Winget }).Count -gt 0) {
            Write-Host "  Or re-run this with:  .\Setup-NewPC.ps1 -InstallDeps" -ForegroundColor DarkGray
        }
    }
}

Write-Host ""
Write-Host "  === Done ===" -ForegroundColor Cyan
Write-Host "  Open a NEW PowerShell window (or run  . `$PROFILE ) and type 'tools'." -ForegroundColor Yellow
Write-Host ""
