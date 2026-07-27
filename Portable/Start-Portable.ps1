#Requires -Version 5.1
<#
    AV Light Tools - portable launcher

    Loads the whole toolkit into the CURRENT session from wherever the
    folder happens to live (USB stick, network share, Ventoy drive).

    DOT-SOURCE IT (note the leading dot) - functions defined by a script that
    is merely *run* live in that script's scope and vanish when it returns:

        . .\Start-Portable.ps1            load everything, open the 'tools' menu
        . .\Start-Portable.ps1 -NoMenu    load everything, drop to the prompt
        . .\Start-Portable.ps1 -Only edid open straight into one tool

    The .cmd launchers next to this file already do that for you.

    It does NOT touch $PROFILE, the registry or anything on the host PC.
    Close the window and nothing is left behind. Output that scripts would
    normally write to the user's shell folders goes to <root>\_OUTPUT instead,
    so dumps and captures stay on the stick.
#>
[CmdletBinding()]
param(
    # Folder holding the .ps1 files. Defaults to the parent of this script.
    [string]$ScriptDir,

    # Load only, don't open the tools menu.
    [switch]$NoMenu,

    # Run this command instead of the menu (e.g. -Only edid).
    [string]$Only
)

# $PSScriptRoot is unreliable as a parameter DEFAULT under PS 5.1, so resolve here.
if (-not $ScriptDir) {
    $here = $null
    if     ($PSScriptRoot)                { $here = $PSScriptRoot }
    elseif ($PSCommandPath)               { $here = Split-Path -Parent $PSCommandPath }
    elseif ($MyInvocation.MyCommand.Path) { $here = Split-Path -Parent $MyInvocation.MyCommand.Path }
    else                                  { $here = (Get-Location).Path }
    $ScriptDir = Split-Path -Parent $here     # Portable\ -> toolkit root
}
$ScriptDir = (Resolve-Path -LiteralPath $ScriptDir).Path

# Everything the tools write lands here instead of the host's Documents/Music/Videos.
$env:AVLT_PORTABLE_ROOT = Join-Path $ScriptDir '_OUTPUT'
$writable = $true
try {
    if (-not (Test-Path -LiteralPath $env:AVLT_PORTABLE_ROOT)) {
        New-Item -ItemType Directory -Path $env:AVLT_PORTABLE_ROOT -Force -ErrorAction Stop | Out-Null
    }
} catch { $writable = $false }

$TL=[char]0x250C; $TR=[char]0x2510; $BL=[char]0x2514; $BR=[char]0x2518
$HZ=[char]0x2500; $VT=[char]0x2502
$boxW = 58

try { Clear-Host } catch { }   # no console handle when output is redirected
Write-Host ""
$title = "AV LIGHT TOOLS - PORTABLE"
$pad = $boxW - $title.Length; if ($pad -lt 0) { $pad = 0 }
$lp = [math]::Floor($pad / 2); $rp = $pad - $lp
Write-Host ("  " + $TL + ("$HZ" * $boxW) + $TR) -ForegroundColor DarkGray
Write-Host ("  " + $VT) -NoNewline -ForegroundColor DarkGray
Write-Host ((" " * $lp) + $title + (" " * $rp)) -NoNewline -ForegroundColor Cyan
Write-Host $VT -ForegroundColor DarkGray
Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
Write-Host ""

Write-Host ("  Source:   {0}" -f $ScriptDir) -ForegroundColor DarkGray
if ($writable) {
    Write-Host ("  Output:   {0}" -f $env:AVLT_PORTABLE_ROOT) -ForegroundColor DarkGray
} else {
    Write-Host ("  Output:   {0}  (READ-ONLY media - saves will fail)" -f $env:AVLT_PORTABLE_ROOT) -ForegroundColor Yellow
}
Write-Host ("  Host:     {0}  |  PowerShell {1}" -f $env:COMPUTERNAME, $PSVersionTable.PSVersion) -ForegroundColor DarkGray
Write-Host ""

# 1. Unblock (files copied from a download or a share are marked). Read-only
#    media will refuse; that is harmless, the files still load.
try {
    Get-ChildItem -LiteralPath $ScriptDir -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Extension -in '.ps1', '.psm1', '.psd1' } |
        Unblock-File -ErrorAction SilentlyContinue
} catch { }

# 2. Load every tool.
#    A LIBRARY script (functions + Set-Alias) is dot-sourced.
#    A COMMAND script has a top-level param() block - dot-sourcing it would RUN
#    it, or prompt for a mandatory parameter, so it gets an alias instead.
#    The distinction is read from the AST, not from a hand-kept list, so extra
#    scripts dropped in the folder can't ambush the loader.
$skip   = @('install.ps1', 'Start-Portable.ps1')
$loaded = 0
$cmds   = New-Object System.Collections.Generic.List[string]
$failed = New-Object System.Collections.Generic.List[string]

foreach ($f in (Get-ChildItem -LiteralPath $ScriptDir -Filter '*.ps1' -File -ErrorAction SilentlyContinue |
                Where-Object { $_.Name -notin $skip -and -not $_.Name.StartsWith('_') } | Sort-Object Name)) {
    try {
        $tok = $null; $perr = $null
        $ast = [System.Management.Automation.Language.Parser]::ParseFile($f.FullName, [ref]$tok, [ref]$perr)
        if ($ast.ParamBlock) {
            $name = [System.IO.Path]::GetFileNameWithoutExtension($f.Name).ToLower()
            Set-Alias -Name $name -Value $f.FullName -Scope Global
            $cmds.Add($name)
        }
        else {
            . $f.FullName
            $loaded++
        }
    }
    catch { $failed.Add(('{0}: {1}' -f $f.Name, $_.Exception.Message)) }
}

Write-Host ("  Loaded {0} script(s)." -f $loaded) -ForegroundColor Green
if ($cmds.Count) {
    Write-Host ("  Run-only scripts, available as: {0}" -f ($cmds -join ', ')) -ForegroundColor DarkGray
}
foreach ($e in $failed) { Write-Host ("  FAILED  {0}" -f $e) -ForegroundColor Red }

# If this script was RUN instead of dot-sourced, everything it just loaded dies
# with it. Say so plainly rather than leaving a session with no commands in it.
if ($MyInvocation.InvocationName -ne '.') {
    Write-Host ""
    Write-Host "  WARNING: this launcher was run, not dot-sourced." -ForegroundColor Yellow
    Write-Host "  The commands will disappear when it finishes. Use AV-Tools.cmd," -ForegroundColor Yellow
    Write-Host "  or start it with a leading dot:  . .\Start-Portable.ps1" -ForegroundColor Yellow
}
Write-Host ""

# 3. What the host is missing (portable runs land on machines that have nothing)
$optional = @(
    @{ Cmd='ffmpeg';    For='convert / audiotools / probe' }
    @{ Cmd='ffprobe';   For='probe-audio' }
    @{ Cmd='yt-dlp';    For='dl' }
    @{ Cmd='speedtest'; For='speedtest' }
)
$missing = @($optional | Where-Object { -not (Get-Command $_.Cmd -ErrorAction SilentlyContinue) })
if ($missing.Count) {
    Write-Host "  Not on this PC (those tools will tell you how to install):" -ForegroundColor Yellow
    foreach ($m in $missing) { Write-Host ("    {0,-10} needed by {1}" -f $m.Cmd, $m.For) -ForegroundColor DarkGray }
    Write-Host ""
}
Write-Host "  Nothing was installed and `$PROFILE was not touched." -ForegroundColor DarkGray
Write-Host "  Close this window to leave no trace on the host." -ForegroundColor DarkGray
Write-Host ""

# 4. Go
if ($Only) {
    if (Get-Command $Only -ErrorAction SilentlyContinue) { & $Only }
    else { Write-Host ("  Unknown command '{0}'. Type 'tools' for the menu." -f $Only) -ForegroundColor Red }
}
elseif (-not $NoMenu -and (Get-Command 'tools' -ErrorAction SilentlyContinue)) {
    tools
}
else {
    Write-Host "  Type 'tools' for the menu." -ForegroundColor Yellow
    Write-Host ""
}
