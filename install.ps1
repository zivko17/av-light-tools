#Requires -Version 5.1
<#
    AV Light Tools - installer

    Run once from the cloned repo folder:

        .\install.ps1

    It unblocks the scripts and adds a loader block to your PowerShell profile,
    so every new session has the tools available. Re-running is safe (it simply
    refreshes the loader block). Works in Windows PowerShell 5.1 and PowerShell 7.
#>
[CmdletBinding()]
param(
    # Folder holding the scripts. Defaults to where this installer lives.
    [string]$ScriptDir
)

# Resolve the scripts folder in the body: $PSScriptRoot is unreliable as a
# parameter DEFAULT under Windows PowerShell 5.1, so compute it here with fallbacks.
if (-not $ScriptDir) {
    if     ($PSScriptRoot)                { $ScriptDir = $PSScriptRoot }
    elseif ($PSCommandPath)               { $ScriptDir = Split-Path -Parent $PSCommandPath }
    elseif ($MyInvocation.MyCommand.Path) { $ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path }
    else                                  { $ScriptDir = (Get-Location).Path }
}
$ScriptDir = (Resolve-Path -LiteralPath $ScriptDir).Path

Write-Host ""
Write-Host "  === AV Light Tools - installer ===" -ForegroundColor Cyan
Write-Host "  Scripts folder: $ScriptDir" -ForegroundColor DarkGray
Write-Host ""

# 1. Unblock files (Windows blocks files that came from the internet)
Write-Host "  Unblocking script files..." -ForegroundColor Yellow
Get-ChildItem -LiteralPath $ScriptDir -Recurse -File |
    Where-Object { $_.Extension -in '.ps1', '.psm1', '.psd1', '.ahk', '.bat' } |
    Unblock-File -ErrorAction SilentlyContinue
Write-Host "  OK" -ForegroundColor Green

# 2. Add / refresh an idempotent loader block in the current profile.
#    A foreach STATEMENT (not ForEach-Object) is used so the dot-sourced
#    functions land in the profile's scope. install.ps1 is skipped.
$marker    = "# >>> AV Light Tools >>>"
$endMarker = "# <<< AV Light Tools <<<"
# The loader dot-sources LIBRARY scripts only (files that just define functions).
# COMMAND scripts - the ones with a top-level param() block, like SetIP.ps1,
# Setup-NewPC.ps1 or the Ventoy helpers - must be RUN, not dot-sourced: sourcing
# them executes their body at every startup (prompts, downloads, hangs). They are
# skipped automatically, so dropping a new command script in the folder is safe.
# Files starting with "_" (backups, disabled) are skipped too.
$loaderLines = @(
    "`$__avltDir = `"$ScriptDir`""
    'foreach ($__avltF in (Get-ChildItem "$__avltDir\*.ps1" -ErrorAction SilentlyContinue)) {'
    '    if ($__avltF.Name -like ''_*'') { continue }                                  # backup / disabled'
    '    $__avltH = Get-Content -LiteralPath $__avltF.FullName -TotalCount 60 -ErrorAction SilentlyContinue'
    '    if ($__avltH -match ''^param\s*\('') { continue }                             # command script: run it, do not source it'
    '    . $__avltF.FullName'
    '}'
    'if (Test-Path "$__avltDir\SetIP.ps1") { Set-Alias -Name setip -Value "$__avltDir\SetIP.ps1" }'
    'Remove-Variable __avltDir, __avltF, __avltH -ErrorAction SilentlyContinue'
)

if (-not (Test-Path -LiteralPath $PROFILE)) {
    New-Item -ItemType File -Path $PROFILE -Force | Out-Null
}

$existing = @()
if (Test-Path -LiteralPath $PROFILE) { $existing = @(Get-Content -LiteralPath $PROFILE) }

# Strip any previous AV Light Tools block, then append a fresh one.
$out  = New-Object System.Collections.Generic.List[string]
$skip = $false
foreach ($line in $existing) {
    if ($line.Trim() -eq $marker)    { $skip = $true;  continue }
    if ($line.Trim() -eq $endMarker) { $skip = $false; continue }
    if (-not $skip) { $out.Add($line) }
}
$out.Add($marker)
foreach ($l in $loaderLines) { $out.Add($l) }
$out.Add($endMarker)
Set-Content -LiteralPath $PROFILE -Value $out -Encoding UTF8

Write-Host ""
Write-Host "  Loader added to your profile:" -ForegroundColor Green
Write-Host "    $PROFILE" -ForegroundColor DarkGray
Write-Host ""
Write-Host "  Reload it now with:  " -NoNewline -ForegroundColor Yellow
Write-Host ". `$PROFILE" -ForegroundColor White
Write-Host "  then type 'tools' to open the menu." -ForegroundColor Yellow
Write-Host ""
Write-Host "  Note: Windows PowerShell 5.1 and PowerShell 7 have separate profiles." -ForegroundColor DarkGray
Write-Host "  Run this installer from each edition you use." -ForegroundColor DarkGray
Write-Host ""
