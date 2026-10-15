# ============================================================
#  REATools.ps1 - REA (Reverse Engineer Anything) front-end
#
#  Thin menu over the 'rea' CLI (npm: rea-agents). REA itself is
#  READ-ONLY on the target: it never executes the artifact being
#  analyzed, and the imported analysis database is ephemeral.
#
#  Engines are optional and caller-supplied. REA never downloads
#  them. They are picked up from the environment:
#    GHIDRA_INSTALL_DIR  native binaries (PE/ELF/Mach-O)
#    JAVA_HOME           full JDK 21+, required by Ghidra and JADX
#    REA_JADX_MCP_JAR    Android APK analysis
#  Static JavaScript/Electron, .NET and SQLite need no engine.
# ============================================================

# ------------------------------------------------------------
#  Helpers
# ------------------------------------------------------------

# REA ships as an npm global: a .ps1/.cmd launcher next to the package.
# Resolve node + the package entry point instead of the launcher, so
# arguments bypass cmd.exe parsing entirely (see Invoke-REA).
function Resolve-REACommand {
    $node = Get-Command node.exe -ErrorAction SilentlyContinue
    if (-not $node) {
        throw "node not found. Install with: winget install OpenJS.NodeJS.LTS"
    }
    $launcher = Get-Command rea -ErrorAction SilentlyContinue
    if (-not $launcher) {
        throw "rea not found. Install with: npm install --global rea-agents"
    }
    $dir = Split-Path -Parent $launcher.Source
    $entry = Join-Path $dir 'node_modules\rea-agents\scripts\rea.mjs'
    if (-not (Test-Path -LiteralPath $entry)) {
        # Launcher present but package entry missing - fall back to it.
        return @{ File = $launcher.Source; Prefix = @() }
    }
    return @{ File = $node.Source; Prefix = @($entry) }
}

# PS5.1 has no ProcessStartInfo.ArgumentList, so quote by hand there.
function ConvertTo-REANativeArg {
    param([string]$Arg)
    if ($Arg.Length -gt 0 -and $Arg -notmatch '[\s"]') { return $Arg }
    $sb = [System.Text.StringBuilder]::new()
    [void]$sb.Append('"')
    $bs = 0
    foreach ($c in $Arg.ToCharArray()) {
        if ($c -eq '\') { $bs++ }
        elseif ($c -eq '"') { [void]$sb.Append('\' * ($bs * 2 + 1)); [void]$sb.Append('"'); $bs = 0 }
        else { if ($bs) { [void]$sb.Append('\' * $bs); $bs = 0 }; [void]$sb.Append($c) }
    }
    if ($bs) { [void]$sb.Append('\' * ($bs * 2)) }
    [void]$sb.Append('"')
    return $sb.ToString()
}

function Set-REAPsiArguments {
    param([System.Diagnostics.ProcessStartInfo]$Psi, [string[]]$ArgList)
    if ($Psi.PSObject.Properties['ArgumentList']) {
        foreach ($a in $ArgList) { $Psi.ArgumentList.Add([string]$a) }                        # PS7 / .NET Core
    } else {
        $Psi.Arguments = (($ArgList | ForEach-Object { ConvertTo-REANativeArg $_ }) -join ' ') # PS5.1 / .NET Framework
    }
}

# Run rea with an exact argument vector. Output is inherited by this
# console (not redirected) so long analyses stream as they go.
function Invoke-REA {
    param([Parameter(Mandatory=$true)][string[]]$ReaArgs)
    try { $exe = Resolve-REACommand }
    catch { Write-Host ("  " + $_.Exception.Message) -ForegroundColor Red; return }

    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $exe.File
    Set-REAPsiArguments -Psi $psi -ArgList (@($exe.Prefix) + $ReaArgs)
    $psi.UseShellExecute = $false
    Write-Host ("  rea " + ($ReaArgs -join ' ')) -ForegroundColor DarkGray
    Write-Host ""
    $p = [System.Diagnostics.Process]::Start($psi)
    $p.WaitForExit()
    if ($p.ExitCode -ne 0) {
        Write-Host ""
        Write-Host ("  rea exited with code {0}" -f $p.ExitCode) -ForegroundColor Yellow
    }
}
Set-Alias -Name rea-run -Value Invoke-REA

# Prompt for an artifact path, strip stray quotes, verify it exists.
# Returns $null when the user cancels or the path is wrong.
function Read-REAPath {
    param([string]$Prompt = "  Path to artifact")
    $raw = Read-Host $Prompt
    if ([string]::IsNullOrWhiteSpace($raw)) { return $null }
    $clean = ($raw -replace '^"|"$','').Trim()
    if (-not (Test-Path -LiteralPath $clean)) {
        Write-Host ("  Path not found: {0}" -f $clean) -ForegroundColor Red
        return $null
    }
    return $clean
}

# One-line engine summary for the menu footer.
function Get-REAEngineStatus {
    $parts = @()
    if ($env:GHIDRA_INSTALL_DIR) { $parts += 'Ghidra' }
    if ($env:REA_JADX_MCP_JAR)   { $parts += 'JADX' }
    if ($env:REA_IDA_MCP_CONFIG) { $parts += 'IDA' }
    if ($parts.Count -eq 0) { return 'none (static JS / .NET / SQLite only)' }
    return ($parts -join ', ')
}

function Show-REAStatus {
    Invoke-REA -ReaArgs @('doctor')
}
Set-Alias -Name rea-doctor -Value Show-REAStatus

# ------------------------------------------------------------
#  Menu
# ------------------------------------------------------------
function Invoke-REAMenu {
    $TL=[char]0x250C; $TR=[char]0x2510; $BL=[char]0x2514; $BR=[char]0x2518
    $HZ=[char]0x2500; $VT=[char]0x2502
    $boxW = 62

    while ($true) {
        Clear-Host
        Write-Host ""
        $title = "REA - REVERSE ENGINEER ANYTHING"
        $pad = $boxW - $title.Length; if ($pad -lt 0) { $pad = 0 }
        $lp = [math]::Floor($pad / 2); $rp = $pad - $lp
        Write-Host ("  " + $TL + ("$HZ" * $boxW) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT) -NoNewline -ForegroundColor DarkGray
        Write-Host ((" " * $lp) + $title + (" " * $rp)) -NoNewline -ForegroundColor Cyan
        Write-Host $VT -ForegroundColor DarkGray
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""

        Write-Host "  --- Status ---" -ForegroundColor DarkGray
        Write-Host "    1)  Doctor               engines, JDK, agent registrations"
        Write-Host "    2)  Capabilities         what can be analyzed on this PC"
        Write-Host ""
        Write-Host "  --- Any artifact ---" -ForegroundColor DarkGray
        Write-Host "    3)  Overview             app, binary or bundle at a glance"
        Write-Host "    4)  Artifact contents    inventory of a file, bundle or archive"
        Write-Host ""
        Write-Host "  --- Native code (needs Ghidra) ---" -ForegroundColor DarkGray
        Write-Host "    5)  Decompile            pseudocode of one procedure"
        Write-Host ""
        Write-Host "  --- Android APK (needs JADX) ---" -ForegroundColor DarkGray
        Write-Host "    6)  APK info             package, SDK, permissions, manifest"
        Write-Host "    7)  Search classes       by case-sensitive substring"
        Write-Host "    8)  Decompile method     one method of one class"
        Write-Host ""
        Write-Host "  --- Managed / data (no engine needed) ---" -ForegroundColor DarkGray
        Write-Host "    9)  .NET identity        PE/CLI without loading target code"
        Write-Host "   10)  SQLite database      schema and optional table rows"
        Write-Host ""
        Write-Host "  --- JavaScript / Electron (no engine needed) ---" -ForegroundColor DarkGray
        Write-Host "   11)  Analyze JS app       statically reconstruct a local app"
        Write-Host ""
        Write-Host "  --- Free form ---" -ForegroundColor DarkGray
        Write-Host "   12)  Any rea command      type the arguments yourself"
        Write-Host "   13)  Command list         all rea commands"
        Write-Host ""
        Write-Host ("    Engines configured: {0}" -f (Get-REAEngineStatus)) -ForegroundColor DarkGray
        Write-Host "    Read-only: REA never executes the artifact it analyzes." -ForegroundColor DarkGray
        Write-Host ""
        Write-Host "    0)  Exit" -ForegroundColor DarkGray
        Write-Host ""

        if (-not [Console]::IsInputRedirected) { try { $Host.UI.RawUI.FlushInputBuffer() } catch {} }
        $op = Read-Host "  Choose option"
        switch ($op) {
            "1" { Invoke-REA -ReaArgs @('doctor') }
            "2" { Invoke-REA -ReaArgs @('capabilities') }
            "3" {
                $p = Read-REAPath
                if ($p) { Invoke-REA -ReaArgs @('analyze', $p) }
            }
            "4" {
                $p = Read-REAPath
                if ($p) { Invoke-REA -ReaArgs @('inspect-artifact', $p) }
            }
            "5" {
                $p = Read-REAPath "  Path to binary"
                if ($p) {
                    Write-Host "  Procedure name or 0x address (Enter = entry point)" -ForegroundColor DarkGray
                    $proc = Read-Host "  Procedure"
                    if ([string]::IsNullOrWhiteSpace($proc)) { $proc = 'entry' }
                    Invoke-REA -ReaArgs @('decompile', $p, $proc.Trim())
                }
            }
            "6" {
                $p = Read-REAPath "  Path to .apk"
                if ($p) { Invoke-REA -ReaArgs @('inspect-android-package', $p) }
            }
            "7" {
                $p = Read-REAPath "  Path to .apk"
                if ($p) {
                    Write-Host "  Case-sensitive substring (Enter = list all classes)" -ForegroundColor DarkGray
                    $q = Read-Host "  Query"
                    if ($null -eq $q) { $q = '' }
                    Invoke-REA -ReaArgs @('search-android-classes', $p, $q)
                }
            }
            "8" {
                $p = Read-REAPath "  Path to .apk"
                if ($p) {
                    $cls = Read-Host "  Fully qualified class name"
                    $mth = Read-Host "  Method name"
                    if ($cls -and $mth) {
                        Invoke-REA -ReaArgs @('inspect-android-method', $p, $cls.Trim(), $mth.Trim())
                    } else {
                        Write-Host "  Class and method are both required." -ForegroundColor Yellow
                    }
                }
            }
            "9" {
                $p = Read-REAPath "  Path to .exe / .dll"
                if ($p) { Invoke-REA -ReaArgs @('inspect-managed-artifact', $p) }
            }
            "10" {
                $p = Read-REAPath "  Path to .db / .sqlite"
                if ($p) { Invoke-REA -ReaArgs @('inspect-sqlite-database', $p) }
            }
            "11" {
                $p = Read-REAPath "  Path to app folder or bundle"
                if ($p) { Invoke-REA -ReaArgs @('analyze-javascript-application', $p) }
            }
            "12" {
                Write-Host "  Everything after 'rea', e.g. inspect-artifact C:\app.zip" -ForegroundColor DarkGray
                $line = Read-Host "  rea"
                if (-not [string]::IsNullOrWhiteSpace($line)) {
                    # Tokenize respecting double quotes, so paths with spaces stay intact
                    $argArr = @([regex]::Matches($line, '"([^"]*)"|(\S+)') | ForEach-Object {
                        if ($_.Groups[1].Success) { $_.Groups[1].Value } else { $_.Value }
                    })
                    Invoke-REA -ReaArgs $argArr
                }
            }
            "13" { Invoke-REA -ReaArgs @('--help') }
            "0" { return }
            default { Write-Host "  Invalid option" -ForegroundColor Yellow }
        }
        Write-Host ""
        Read-Host "  [Enter] to return" | Out-Null
    }
}
Set-Alias -Name reatools -Value Invoke-REAMenu
