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
    [string]$ScriptDir = $PSScriptRoot
)

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
$loaderCmd = "foreach (`$__avlt in (Get-ChildItem `"$ScriptDir\*.ps1`" -ErrorAction SilentlyContinue | Where-Object { `$_.Name -ne 'install.ps1' })) { . `$__avlt.FullName }"

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
$out.Add($loaderCmd)
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
