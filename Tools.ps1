function Show-Tools {

    # ----------------------------------------------------------------------
    #  Menu definition (data-driven: edit here, numbers auto-assign)
    #  - Items with 'Name' get a number and can be launched.
    #  - Items with 'Info' are reference-only (no number).
    # ----------------------------------------------------------------------
    $sections = @(
        @{ Title = "Network"; Items = @(
            @{ Name="netinfo";   Desc="IP, gateway, DNS, MAC, link speed of every adapter" }
            @{ Name="scan";      Desc="Who is on the network  (scan / scan 192.168.10)" }
            @{ Name="pingmulti"; Desc="Live monitor of devices  (pingmulti IP IP ...)" }
            @{ Name="portcheck"; Desc="Is a TCP port open?  (portcheck IP PORT)" }
            @{ Name="whoison";   Desc="MAC + vendor of a device  (whoison IP)" }
            @{ Name="speed";     Desc="Live bandwidth (down/up)" }
            @{ Name="speedtest"; Desc="Internet speed test (Ookla CLI)" }
            @{ Name="flushdns";  Desc="Clear DNS cache" }
            @{ Name="reset-nic"; Desc="Restart the network adapter (Admin)" }
        )}
        @{ Title = "IP Configuration (run as Admin)"; Items = @(
            @{ Info="setip 0                              Profile 0  (192.168.0.99)" }
            @{ Info="setip 1                              Profile 1  (192.168.1.99)" }
            @{ Info="setip dhcp                           Switch to DHCP" }
            @{ Info="setip 192.168.1.25/24                Static IP, no gateway" }
            @{ Info="setip 192.168.1.25/24 192.168.1.1    Static IP + gateway" }
        )}
        @{ Title = "Displays"; Items = @(
            @{ Name="edid";             Desc="EDID menu (read / decode / compare / dump)" }
            @{ Name="edid-list";        Desc="What every display reports" }
            @{ Name="edid-diff";        Desc="Compare two EDIDs  (edid-diff 1 projector.bin)" }
            @{ Name="edid-dump";        Desc="Save raw EDID to .bin" }
            @{ Name="monitors";         Desc="Connected displays: resolution, Hz, model" }
            @{ Name="monitors-arrange"; Desc="Change resolution / refresh rate (needs nircmd)" }
        )}
        @{ Title = "Audio"; Items = @(
            @{ Name="audiotools";     Desc="FFmpeg audio menu (convert / normalize / trim / extract)" }
            @{ Name="audio";          Desc="Audio devices + default + volume" }
            @{ Name="services-audio"; Desc="Windows audio + Dante services status" }
        )}
        @{ Title = "System"; Items = @(
            @{ Name="sysinfo"; Desc="PC info: model, CPU, RAM, GPU, S/N, OS, uptime" }
            @{ Name="tele";    Desc="Live CPU / RAM / GPU / disk / network" }
            @{ Name="top";     Desc="Processes using the most CPU right now" }
            @{ Name="killp";   Desc="Kill a process by name  (killp resolume)" }
            @{ Name="usb";     Desc="Connected USB devices" }
            @{ Name="disks";   Desc="Disks: health, temperature, free space" }
            @{ Name="startup"; Desc="Programs that launch at boot" }
            @{ Name="battery"; Desc="Battery charge, health, time left" }
            @{ Name="cleanup"; Desc="Delete temp files, caches, recycle bin" }
        )}
        @{ Title = "Utilities"; Items = @(
            @{ Name="backup";  Desc='Copy with date  (backup "src" "dest")' }
            @{ Name="search";  Desc='Find files by name  (search "text")' }
            @{ Name="timer";   Desc="Countdown with beep  (timer 5m / 1h30m / 90s)" }
            @{ Name="weather"; Desc="Weather  (weather Ibiza)" }
        )}
    )

    # Box-drawing chars built from code points: keeps this file ASCII-safe and renders
    # correctly under both PS5.1 and PS7 consoles.
    $TL=[char]0x250C; $TR=[char]0x2510; $BL=[char]0x2514; $BR=[char]0x2518
    $HZ=[char]0x2500; $VT=[char]0x2502
    $boxW = 52

    while ($true) {
        Clear-Host
        Write-Host ""

        # Framed title
        $title = "AVAILABLE TOOLS"
        $pad = $boxW - $title.Length; if ($pad -lt 0) { $pad = 0 }
        $lp = [math]::Floor($pad / 2); $rp = $pad - $lp
        Write-Host ("  " + $TL + ("$HZ" * $boxW) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT) -NoNewline -ForegroundColor DarkGray
        Write-Host ((" " * $lp) + $title + (" " * $rp)) -NoNewline -ForegroundColor Cyan
        Write-Host $VT -ForegroundColor DarkGray
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""

        # Render + build the number -> command map (each section framed)
        $map = @{}
        $i = 1
        foreach ($sec in $sections) {
            $fill = $boxW - $sec.Title.Length - 3; if ($fill -lt 0) { $fill = 0 }
            Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
            Write-Host $sec.Title -NoNewline -ForegroundColor Yellow
            Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
            foreach ($item in $sec.Items) {
                Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
                if ($item.ContainsKey('Name')) {
                    $num = "{0,2}" -f $i
                    Write-Host (" [{0}] " -f $num) -NoNewline -ForegroundColor DarkCyan
                    Write-Host ("{0,-16}" -f $item.Name) -NoNewline -ForegroundColor Green
                    Write-Host (" {0}" -f $item.Desc)
                    $map[$i] = $item.Name
                    $i++
                }
                else {
                    Write-Host ("      {0}" -f $item.Info) -ForegroundColor DarkGray
                }
            }
            Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
            Write-Host ""
        }

        Write-Host "  --- This menu ---" -ForegroundColor Yellow
        Write-Host "    tools            " -NoNewline -ForegroundColor Green; Write-Host "Show this list"
        Write-Host ""
        Write-Host "  Type a NUMBER to launch, or press Enter to exit." -ForegroundColor DarkGray
        Write-Host "  You can also still type any command name directly." -ForegroundColor DarkGray
        Write-Host ""

        # Drain any stray/type-ahead keystrokes so a leftover Enter isn't read as an empty choice
        if (-not [Console]::IsInputRedirected) { try { $Host.UI.RawUI.FlushInputBuffer() } catch {} }
        $choice = Read-Host "  Select"
        if ([string]::IsNullOrWhiteSpace($choice)) { break }

        $n = 0
        if ([int]::TryParse($choice, [ref]$n) -and $map.ContainsKey($n)) {
            $target = $map[$n]
            Write-Host ""
            $argline = Read-Host ("  Arguments for '{0}' (Enter for none)" -f $target)
            Write-Host ""
            Write-Host ("  Launching '{0}'..." -f $target) -ForegroundColor Cyan
            Write-Host ""
            try {
                if ([string]::IsNullOrWhiteSpace($argline)) {
                    & $target
                } else {
                    # Tokenize respecting double quotes, so paths with spaces stay intact
                    # e.g. backup "C:\My Folder" "D:\bak"
                    $argArr = @([regex]::Matches($argline, '"([^"]*)"|(\S+)') | ForEach-Object {
                        if ($_.Groups[1].Success) { $_.Groups[1].Value } else { $_.Value }
                    })
                    & $target @argArr
                }
            }
            catch {
                Write-Host ("  Error running '{0}': {1}" -f $target, $_.Exception.Message) -ForegroundColor Red
            }
            Write-Host ""
            Read-Host "  Press Enter to return to the menu" | Out-Null
        }
        else {
            Write-Host ("  Invalid option: '{0}'" -f $choice) -ForegroundColor Red
            Start-Sleep -Milliseconds 900
        }
    }
}

Set-Alias -Name tools -Value Show-Tools