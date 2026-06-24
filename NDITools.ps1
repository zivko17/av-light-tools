# ============================================================
#  NDITools.ps1 - NDI utilities for AV technicians
# ============================================================

# NDI Tools install path. Leave $NDIBase empty to auto-detect the newest
# "NDI N Tools" folder; set it explicitly to force a specific install.
$script:NDIBase = $null

function Resolve-NDIBase {
    if ($script:NDIBase -and (Test-Path $script:NDIBase)) { return $script:NDIBase }
    $found = Get-ChildItem "C:\Program Files\NDI" -Directory -Filter "NDI * Tools" -ErrorAction SilentlyContinue |
             Sort-Object { [int]($_.Name -replace '\D','') } -Descending | Select-Object -First 1
    if ($found) { return $found.FullName }
    return "C:\Program Files\NDI\NDI 6 Tools"  # fallback
}

# ---- ndi-launch: open NDI Tools apps from console ----
function Invoke-NDILaunch {
    param(
        [Parameter(Position=0)][ValidateSet('monitor','router','test','bridge','manager','screen','launcher','help')]
        [string]$App = 'help'
    )

    $base = Resolve-NDIBase
    $apps = @{
        monitor  = "$base\Studio Monitor\Application.Network.StudioMonitor.x64.exe"
        router   = "$base\Router\Application.NDI.Router.exe"
        test     = "$base\Test Patterns\Application.Network.TestPatterns.exe"
        bridge   = "$base\Bridge\Application.NDI.Bridge.UI.exe"
        manager  = "$base\Access Manager\Application.NdiGroupEditor.exe"
        screen   = "$base\Screen Capture\Application.Network.ScanConverter2.x64.exe"
        launcher = "$base\NDI Launcher.exe"
    }

    if ($App -eq 'help') {
        Write-Host "`n  Usage: ndi-launch <app>`n" -ForegroundColor Cyan
        Write-Host "  Available apps:" -ForegroundColor Yellow
        Write-Host "    monitor   - Studio Monitor (view NDI sources)"
        Write-Host "    router    - NDI Router (route sources)"
        Write-Host "    test      - Test Patterns (generate test signal)"
        Write-Host "    bridge    - NDI Bridge (LAN to WAN tunneling)"
        Write-Host "    manager   - Access Manager (groups + remote servers)"
        Write-Host "    screen    - Screen Capture (desktop to NDI)"
        Write-Host "    launcher  - NDI Launcher (main hub)"
        Write-Host ""
        return
    }

    $exe = $apps[$App]
    if (Test-Path $exe) {
        Start-Process $exe
        Write-Host "  Launched: $App" -ForegroundColor Green
    } else {
        Write-Host "  ERROR: $exe not found." -ForegroundColor Red
    }
}
Set-Alias -Name ndi-launch -Value Invoke-NDILaunch


# ---- ndi-check: network diagnostic for NDI ----
function Test-NDINetwork {
    Write-Host "`n=== NDI NETWORK DIAGNOSTIC ===" -ForegroundColor Cyan
    Write-Host ""

    # 1. Active adapter and link speed
    Write-Host "  --- Network adapter ---" -ForegroundColor Yellow
    $adapters = Get-NetAdapter | Where-Object { $_.Status -eq "Up" -and $_.InterfaceDescription -notmatch "Loopback|Virtual" }
    foreach ($a in $adapters) {
        $speed = $a.LinkSpeed
        $color = if ($speed -match "^1 Gbps|^2.5 Gbps|^10 Gbps") { "Green" }
                 elseif ($speed -match "^100 Mbps") { "Red" }
                 else { "Yellow" }
        Write-Host ("    {0,-25} {1,-15} {2}" -f $a.Name, $speed, $a.MacAddress) -ForegroundColor $color
        if ($speed -match "^100 Mbps") {
            Write-Host "      WARNING: 100 Mbps is NOT enough for NDI Full Bandwidth (need 1 Gbps+)" -ForegroundColor Red
        }
    }
    Write-Host ""

    # 2. mDNS port (5353)
    Write-Host "  --- mDNS service (port 5353) ---" -ForegroundColor Yellow
    $mdnsService = Get-Service -Name "Dnscache" -ErrorAction SilentlyContinue
    if ($mdnsService) {
        $color = if ($mdnsService.Status -eq "Running") { "Green" } else { "Red" }
        Write-Host ("    DNS Client service:    {0}" -f $mdnsService.Status) -ForegroundColor $color
    }

    $mdnsListener = Get-NetUDPEndpoint -LocalPort 5353 -ErrorAction SilentlyContinue
    if ($mdnsListener) {
        Write-Host "    UDP 5353 listening:    YES (mDNS is active)" -ForegroundColor Green
    } else {
        Write-Host "    UDP 5353 listening:    NO (mDNS may not work)" -ForegroundColor Red
    }
    Write-Host ""

    # 3. Firewall rules for NDI
    Write-Host "  --- Firewall ---" -ForegroundColor Yellow
    $ndiRules = Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -match "NDI" -and $_.Enabled -eq $true }
    if ($ndiRules) {
        $allow = ($ndiRules | Where-Object { $_.Action -eq "Allow" }).Count
        $block = ($ndiRules | Where-Object { $_.Action -eq "Block" }).Count
        Write-Host ("    NDI firewall rules:    {0} allow / {1} block" -f $allow, $block) -ForegroundColor $(if ($block -gt 0) { "Yellow" } else { "Green" })
        if ($block -gt 0) {
            Write-Host "      WARNING: $block blocking rules detected, check Windows Firewall" -ForegroundColor Yellow
        }
    } else {
        Write-Host "    NDI firewall rules:    none found (NDI may be blocked)" -ForegroundColor Yellow
    }
    Write-Host ""

    # 4. NDI port range (typically 5960-5990)
    Write-Host "  --- NDI listening ports (5960-5990) ---" -ForegroundColor Yellow
    $ndiPorts = Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object { $_.LocalPort -ge 5960 -and $_.LocalPort -le 5990 }
    if ($ndiPorts) {
        Write-Host ("    Found {0} NDI ports listening:" -f $ndiPorts.Count) -ForegroundColor Green
        foreach ($p in $ndiPorts) {
            $proc = (Get-Process -Id $p.OwningProcess -ErrorAction SilentlyContinue).ProcessName
            Write-Host ("      Port {0,5}  <-  {1}" -f $p.LocalPort, $proc) -ForegroundColor Gray
        }
    } else {
        Write-Host "    No NDI ports listening (no NDI app is sending right now)" -ForegroundColor DarkGray
    }
    Write-Host ""

    # 5. Multicast capability
    Write-Host "  --- Multicast ---" -ForegroundColor Yellow
    $multicastRoutes = Get-NetRoute -DestinationPrefix "224.0.0.0/4" -ErrorAction SilentlyContinue
    if ($multicastRoutes) {
        Write-Host ("    Multicast routes:      {0} configured" -f $multicastRoutes.Count) -ForegroundColor Green
    } else {
        Write-Host "    Multicast routes:      NONE (multicast disabled)" -ForegroundColor Red
    }
    Write-Host ""
}
Set-Alias -Name ndi-check -Value Test-NDINetwork


# ---- ndi-services: NDI services and running processes ----
function Get-NDIServices {
    Write-Host "`n=== NDI SERVICES & PROCESSES ===" -ForegroundColor Cyan
    Write-Host ""

    # NDI Discovery Service
    Write-Host "  --- Services ---" -ForegroundColor Yellow
    $svc = Get-Service -DisplayName "*NDI*" -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -match "^NDI |Newtek" }
    if ($svc) {
        foreach ($s in $svc) {
            $color = if ($s.Status -eq "Running") { "Green" } else { "Red" }
            Write-Host ("    {0,-40} {1}" -f $s.DisplayName, $s.Status) -ForegroundColor $color
        }
    } else {
        Write-Host "    No NDI services registered." -ForegroundColor DarkGray
    }
    Write-Host ""

    # Running NDI processes
    Write-Host "  --- Running NDI processes ---" -ForegroundColor Yellow
    # "NDI" already matches the Router/Bridge processes (Application.NDI.*),
    # so the generic "Router|Bridge" tokens are dropped to avoid false positives.
    $processes = Get-Process | Where-Object {
        $_.ProcessName -match "NDI|StudioMonitor|ScanConverter|TestPatterns|GroupEditor"
    } | Sort-Object ProcessName

    if ($processes) {
        Write-Host ("    {0,-40} {1,-10} {2}" -f "PROCESS", "PID", "MEMORY (MB)") -ForegroundColor DarkGray
        Write-Host ("    {0,-40} {1,-10} {2}" -f "----------------------------------------", "----------", "------------") -ForegroundColor DarkGray
        foreach ($p in $processes) {
            $memMB = [math]::Round($p.WorkingSet64 / 1MB, 1)
            Write-Host ("    {0,-40} {1,-10} {2}" -f $p.ProcessName, $p.Id, $memMB) -ForegroundColor Green
        }
    } else {
        Write-Host "    No NDI processes running." -ForegroundColor DarkGray
    }
    Write-Host ""
}
Set-Alias -Name ndi-services -Value Get-NDIServices


# Normalise an NDI config field that may be a comma-separated string OR a JSON array.
function ConvertTo-NDIList {
    param($Value)
    if ($null -eq $Value)    { return @() }
    if ($Value -is [array])  { return @($Value | ForEach-Object { "$_" }) }
    return ($Value -split ',')
}

# ---- ndi-config: read NDI Access Manager configuration ----
function Get-NDIConfig {
    Write-Host "`n=== NDI ACCESS MANAGER CONFIG ===" -ForegroundColor Cyan
    Write-Host ""

    # NDI Access Manager stores config in roaming profile
    $configPath = "$env:APPDATA\NDI\ndi-config.v1.json"

    if (-not (Test-Path $configPath)) {
        Write-Host "  No config file found at:" -ForegroundColor Yellow
        Write-Host "    $configPath" -ForegroundColor DarkGray
        Write-Host ""
        Write-Host "  Open Access Manager once to generate it: ndi-launch manager" -ForegroundColor DarkGray
        Write-Host ""
        return
    }

    try {
        $config = Get-Content $configPath -Raw -Encoding UTF8 | ConvertFrom-Json

        # Groups
        Write-Host "  --- Groups (sending) ---" -ForegroundColor Yellow
        $sendGroups = $config.ndi.groups.send
        if ($sendGroups) {
            foreach ($g in (ConvertTo-NDIList $sendGroups)) {
                Write-Host "    $($g.Trim())" -ForegroundColor Green
            }
        } else {
            Write-Host "    (default: public)" -ForegroundColor DarkGray
        }
        Write-Host ""

        Write-Host "  --- Groups (receiving) ---" -ForegroundColor Yellow
        $recvGroups = $config.ndi.groups.recv
        if ($recvGroups) {
            foreach ($g in (ConvertTo-NDIList $recvGroups)) {
                Write-Host "    $($g.Trim())" -ForegroundColor Green
            }
        } else {
            Write-Host "    (default: public)" -ForegroundColor DarkGray
        }
        Write-Host ""

        # Discovery servers
        Write-Host "  --- Discovery Servers ---" -ForegroundColor Yellow
        $discoServers = $config.ndi.networks.discovery
        if ($discoServers) {
            Write-Host "    $discoServers" -ForegroundColor Green
        } else {
            Write-Host "    (none configured - using mDNS)" -ForegroundColor DarkGray
        }
        Write-Host ""

        # External IPs (manual sources)
        Write-Host "  --- External IPs (manual sources) ---" -ForegroundColor Yellow
        $extIPs = $config.ndi.networks.ips
        if ($extIPs) {
            foreach ($ip in (ConvertTo-NDIList $extIPs)) {
                Write-Host "    $($ip.Trim())" -ForegroundColor Green
            }
        } else {
            Write-Host "    (none)" -ForegroundColor DarkGray
        }
        Write-Host ""

    } catch {
        Write-Host "  ERROR reading config: $($_.Exception.Message)" -ForegroundColor Red
    }
}
Set-Alias -Name ndi-config -Value Get-NDIConfig


# ---- ndi: interactive NDI menu (diagnostics + launch apps) ----
function Invoke-NDIMenu {
    $TL=[char]0x250C; $TR=[char]0x2510; $BL=[char]0x2514; $BR=[char]0x2518
    $HZ=[char]0x2500; $VT=[char]0x2502
    $boxW = 52
    while ($true) {
        Clear-Host
        Write-Host ""
        $title = "NDI TOOLS"
        $pad = $boxW - $title.Length; if ($pad -lt 0) { $pad = 0 }
        $lp = [math]::Floor($pad / 2); $rp = $pad - $lp
        Write-Host ("  " + $TL + ("$HZ" * $boxW) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT) -NoNewline -ForegroundColor DarkGray
        Write-Host ((" " * $lp) + $title + (" " * $rp)) -NoNewline -ForegroundColor Cyan
        Write-Host $VT -ForegroundColor DarkGray
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Diagnostics" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - ("Diagnostics").Length - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   1)  Network check    (link / mDNS / firewall / ports / multicast)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   2)  Services         (NDI services + running processes)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   3)  Config           (Access Manager: groups / discovery / IPs)"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Launch apps" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - ("Launch apps").Length - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   4)  Studio Monitor   (view NDI sources)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   5)  Router           (route sources)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   6)  Test Patterns    (generate test signal)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   7)  Bridge           (LAN to WAN tunneling)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   8)  Access Manager   (groups + remote servers)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   9)  Screen Capture   (desktop to NDI)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  10)  NDI Launcher     (main hub)"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host "    0)  Exit" -ForegroundColor DarkGray
        Write-Host ""
        $op = Read-Host "  Choose option"
        switch ($op) {
            "1"  { Test-NDINetwork }
            "2"  { Get-NDIServices }
            "3"  { Get-NDIConfig }
            "4"  { Invoke-NDILaunch monitor }
            "5"  { Invoke-NDILaunch router }
            "6"  { Invoke-NDILaunch test }
            "7"  { Invoke-NDILaunch bridge }
            "8"  { Invoke-NDILaunch manager }
            "9"  { Invoke-NDILaunch screen }
            "10" { Invoke-NDILaunch launcher }
            "0"  { return }
            default { Write-Host "  Invalid option" -ForegroundColor Yellow }
        }
        Write-Host ""
        Read-Host "  [Enter] to return" | Out-Null
    }
}
Set-Alias -Name ndi -Value Invoke-NDIMenu

