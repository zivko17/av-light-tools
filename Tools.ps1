# ===================================================================
#  Tools.ps1 - the 'tools' menu
#
#  Edit $sections to add or move a tool: numbers are assigned in order.
#    Name  command to run
#    Desc  short description (keep it under ~30 characters)
#    Ask   what to ask before running it. "[...]" = optional (Enter skips it).
#          No Ask = runs straight away.
#  Commands that are not loaded on this PC are hidden, so the menu never
#  offers something that does not work.
# ===================================================================

function Show-Tools {
    $sections = @(
        @{ Title = 'NETWORK'; Color = 'Cyan'; Items = @(
            @{ Name = 'netinfo';   Desc = 'IP, gateway, DNS, MAC' }
            @{ Name = 'scan';      Desc = 'Who is on the network';      Ask = '[network, e.g. 192.168.10]' }
            @{ Name = 'scannet';   Desc = 'Discover unknown network'; Ask = '[-Seconds 30] [-PassiveOnly] [-Keep]' }
            @{ Name = 'pingmulti'; Desc = 'Live ping of devices'; Ask = 'IPs separated by spaces' }
            @{ Name = 'portcheck'; Desc = 'Is a TCP port open?';          Ask = 'IP PORT' }
            @{ Name = 'whoison';   Desc = 'MAC + vendor of a device';     Ask = 'IP' }
            @{ Name = 'speed';     Desc = 'Live bandwidth down / up' }
            @{ Name = 'speedtest'; Desc = 'Internet speed test' }
            @{ Name = 'setip';     Desc = 'Change IP (Admin)';            Ask = '0 | 1 | dhcp | 192.168.1.25/24 [gateway]' }
            @{ Name = 'flushdns';  Desc = 'Clear DNS cache' }
            @{ Name = 'reset-nic'; Desc = 'Restart adapter (Admin)' }
        )}
        @{ Title = 'FIREWALL'; Color = 'Red'; Items = @(
            @{ Name = 'fw-info';   Desc = 'Status + opened programs' }
            @{ Name = 'fw-allow';  Desc = 'Open it for one program';  Ask = 'vmix | dante | resolume | obs | qsys | path.exe  [-Arreglar]' }
            @{ Name = 'fw-remove'; Desc = 'Undo fw-allow';            Ask = 'program | all' }
            @{ Name = 'fw-blocks'; Desc = 'Active block rules' }
            @{ Name = 'fw-off';    Desc = 'Turn it off (Admin)';      Ask = '[-Minutos 60]' }
            @{ Name = 'fw-on';     Desc = 'Turn it back on (Admin)' }
        )}
        @{ Title = 'DISPLAYS'; Color = 'Magenta'; Items = @(
            @{ Name = 'edid';             Desc = 'EDID: read, compare, dump' }
            @{ Name = 'edid-list';        Desc = 'What every display reports' }
            @{ Name = 'edid-diff';        Desc = 'Compare two EDIDs'; Ask = 'A B   (e.g. 1 projector.bin)' }
            @{ Name = 'monitors';         Desc = 'Outputs: resolution and Hz' }
            @{ Name = 'monitors-arrange'; Desc = 'Change resolution / Hz' }
        )}
        @{ Title = 'AUDIO'; Color = 'Green'; Items = @(
            @{ Name = 'audiotools';     Desc = 'Convert / normalize audio' }
            @{ Name = 'audio';          Desc = 'Audio devices and volume' }
            @{ Name = 'services-audio'; Desc = 'Audio + Dante services' }
        )}
        @{ Title = 'SYSTEM'; Color = 'Yellow'; Items = @(
            @{ Name = 'sysinfo'; Desc = 'Model, CPU, RAM, GPU, OS' }
            @{ Name = 'tele';    Desc = 'Live CPU / RAM / GPU / net' }
            @{ Name = 'top';     Desc = 'What is using the CPU now' }
            @{ Name = 'killp';   Desc = 'Close a program by name'; Ask = 'name (e.g. resolume)' }
            @{ Name = 'usb';     Desc = 'Connected USB devices' }
            @{ Name = 'disks';   Desc = 'Disk health and free space' }
            @{ Name = 'startup'; Desc = 'Programs at boot' }
            @{ Name = 'battery'; Desc = 'Battery charge and health' }
        )}
        @{ Title = 'UTILITIES'; Color = 'DarkYellow'; Items = @(
            @{ Name = 'cleanup'; Desc = 'Delete temp and caches' }
            @{ Name = 'backup';  Desc = 'Copy with date';        Ask = '"source" "destination"' }
            @{ Name = 'search';  Desc = 'Find files by name';    Ask = 'text' }
            @{ Name = 'timer';   Desc = 'Countdown with beep';   Ask = 'time (5m, 1h30m, 90s)' }
            @{ Name = 'weather'; Desc = 'Weather';               Ask = '[city]' }
        )}
    )

    # Rounded box characters, built from code points so this file stays ASCII
    $TL = [char]0x256D; $TR = [char]0x256E; $BL = [char]0x2570; $BR = [char]0x256F
    $HZ = [char]0x2500; $VT = [char]0x2502; $DOT = [char]0x00B7

    # Header info, read once
    $iface = if (Get-Command Resolve-NetIface -ErrorAction SilentlyContinue) { Resolve-NetIface } else { $null }
    $ip = if ($iface) {
        Get-NetIPAddress -InterfaceAlias $iface -AddressFamily IPv4 -ErrorAction SilentlyContinue |
            Select-Object -First 1 | ForEach-Object { "$($_.IPAddress)/$($_.PrefixLength)" }
    }
    $netLine = if ($ip) { "$ip  $DOT  $iface" } else { 'no network' }
    $psLine  = "PowerShell $($PSVersionTable.PSVersion.Major).$($PSVersionTable.PSVersion.Minor)"

    while ($true) {
        try { Clear-Host } catch { }

        $width = 100
        try { $width = $Host.UI.RawUI.WindowSize.Width } catch { }
        $twoCols = $width -ge 108
        $cellW   = 50                                   # one menu cell: number + name + description
        $boxW    = if ($twoCols) { 2 * $cellW + 2 } else { $cellW }

        # ---- header ---------------------------------------------------
        $left  = '  AV LIGHT TOOLS'
        $right = "$env:COMPUTERNAME  "
        $gap = $boxW - $left.Length - $right.Length; if ($gap -lt 1) { $gap = 1 }
        Write-Host ""
        Write-Host ("  $TL" + ("$HZ" * $boxW) + $TR) -ForegroundColor DarkGray
        Write-Host "  $VT" -NoNewline -ForegroundColor DarkGray
        Write-Host $left -NoNewline -ForegroundColor White
        Write-Host ((' ' * $gap) + $right) -NoNewline -ForegroundColor DarkGray
        Write-Host $VT -ForegroundColor DarkGray
        $sub = "  $netLine   $DOT   $psLine"
        if ($sub.Length -gt $boxW) { $sub = $sub.Substring(0, $boxW) }
        Write-Host "  $VT" -NoNewline -ForegroundColor DarkGray
        Write-Host $sub.PadRight($boxW) -NoNewline -ForegroundColor $(if ($ip) { 'Cyan' } else { 'Yellow' })
        Write-Host $VT -ForegroundColor DarkGray
        Write-Host ("  $BL" + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray

        # ---- sections -------------------------------------------------
        $map = @{}
        $n = 1
        foreach ($sec in $sections) {
            $items = @($sec.Items | Where-Object { Get-Command $_.Name -ErrorAction SilentlyContinue })
            if (-not $items) { continue }

            Write-Host ""
            Write-Host ("  " + $sec.Title + ' ') -NoNewline -ForegroundColor $sec.Color
            Write-Host ("$HZ" * ($boxW - $sec.Title.Length)) -ForegroundColor DarkGray

            $perRow = if ($twoCols) { 2 } else { 1 }
            for ($r = 0; $r -lt $items.Count; $r += $perRow) {
                for ($c = 0; $c -lt $perRow -and ($r + $c) -lt $items.Count; $c++) {
                    $it = $items[$r + $c]
                    $map[$n] = $it
                    $desc = $it.Desc
                    if ($desc.Length -gt ($cellW - 24)) { $desc = $desc.Substring(0, $cellW - 25) + '.' }
                    Write-Host ("  {0,3}  " -f $n) -NoNewline -ForegroundColor $sec.Color
                    Write-Host ("{0,-17}" -f $it.Name) -NoNewline -ForegroundColor White
                    Write-Host ("{0,-$($cellW - 24)}" -f $desc) -NoNewline -ForegroundColor DarkGray
                    $n++
                }
                Write-Host ""
            }
        }

        Write-Host ""
        Write-Host ("  " + ("$HZ" * $boxW)) -ForegroundColor DarkGray
        Write-Host "  Number + Enter to run   $DOT   Enter alone to exit   $DOT   or type any command" -ForegroundColor DarkGray
        Write-Host ""

        if (-not [Console]::IsInputRedirected) { try { $Host.UI.RawUI.FlushInputBuffer() } catch { } }
        $choice = Read-Host "  >"
        if ([string]::IsNullOrWhiteSpace($choice)) { break }

        $num = 0
        if (-not ([int]::TryParse($choice.Trim(), [ref]$num) -and $map.ContainsKey($num))) {
            Write-Host "  '$choice' is not on the list" -ForegroundColor Red
            Start-Sleep -Milliseconds 900
            continue
        }

        $it = $map[$num]
        $argline = ''
        if ($it.Ask) {
            $optional = $it.Ask.StartsWith('[')
            $argline = Read-Host ("  {0}  {1}" -f $it.Name, $it.Ask)
            if (-not $optional -and [string]::IsNullOrWhiteSpace($argline)) { continue }
        }

        Write-Host ""
        try {
            if ([string]::IsNullOrWhiteSpace($argline)) {
                & $it.Name
            } else {
                # Split on spaces but keep "quoted paths" together
                $argArr = @([regex]::Matches($argline, '"([^"]*)"|(\S+)') | ForEach-Object {
                    if ($_.Groups[1].Success) { $_.Groups[1].Value } else { $_.Value }
                })
                & $it.Name @argArr
            }
        } catch {
            Write-Host ("  Error in '{0}': {1}" -f $it.Name, $_.Exception.Message) -ForegroundColor Red
        }
        Write-Host ""
        Read-Host "  Enter to go back to the menu" | Out-Null
    }
}

Set-Alias -Name tools -Value Show-Tools
