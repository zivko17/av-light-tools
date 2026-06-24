# ===================================================================
#  NetTools.ps1
# ===================================================================

# Default network interface. Set $NetIface = "Ethernet 2" in your profile
# to force a specific adapter; left empty it auto-resolves the active one.
$script:NetIface = $null

function Resolve-NetIface {
    param([string]$Iface)
    if ($Iface)           { return $Iface }
    if ($script:NetIface) { return $script:NetIface }
    # Prefer the adapter that owns the default route (lowest metric, real gateway).
    $route = Get-NetRoute -DestinationPrefix "0.0.0.0/0" -ErrorAction SilentlyContinue |
             Where-Object { $_.NextHop -ne "0.0.0.0" } |
             Sort-Object RouteMetric | Select-Object -First 1
    if ($route) {
        $a = Get-NetAdapter -InterfaceIndex $route.InterfaceIndex -ErrorAction SilentlyContinue
        if ($a) { return $a.Name }
    }
    return "Ethernet"
}

function netinfo {
    $iface = Resolve-NetIface
    $ip  = Get-NetIPAddress -InterfaceAlias $iface -AddressFamily IPv4 -ErrorAction SilentlyContinue
    $cfg = Get-NetIPInterface -InterfaceAlias $iface -AddressFamily IPv4 -ErrorAction SilentlyContinue
    $gw  = (Get-NetRoute -InterfaceAlias $iface -DestinationPrefix "0.0.0.0/0" -ErrorAction SilentlyContinue).NextHop
    $dns = (Get-DnsClientServerAddress -InterfaceAlias $iface -AddressFamily IPv4 -ErrorAction SilentlyContinue).ServerAddresses
    Write-Host ""
    Write-Host "  === $iface ===" -ForegroundColor Cyan
    if ($ip) { foreach ($a in $ip) { Write-Host ("  IP:       {0}/{1}" -f $a.IPAddress, $a.PrefixLength) -ForegroundColor White } }
    else { Write-Host "  IP:       (no address)" -ForegroundColor Yellow }
    Write-Host ("  Gateway:  {0}" -f $(if($gw){$gw}else{"(none)"}))
    Write-Host ("  DNS:      {0}" -f $(if($dns){$dns -join ', '}else{"(automatic)"}))
    if ($cfg) {
        $mode = if ($cfg.Dhcp -eq "Enabled") { "DHCP (automatic)" } else { "Manual (static)" }
        $color = if ($cfg.Dhcp -eq "Enabled") { "Green" } else { "Yellow" }
        Write-Host ("  Mode:     {0}" -f $mode) -ForegroundColor $color
    }
    Write-Host ""
}

function scan {
    param([string]$Subnet)
    if (-not $Subnet) {
        $iface = Resolve-NetIface
        $myip = (Get-NetIPAddress -InterfaceAlias $iface -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.PrefixOrigin -ne "WellKnown" } | Select-Object -First 1).IPAddress
        if (-not $myip) { Write-Host "  No IP on '$iface'. Specify subnet: scan 192.168.1" -ForegroundColor Yellow; return }
        $Subnet = ($myip -split '\.')[0..2] -join '.'
    }
    Write-Host "`n  Scanning $Subnet.1-254 (parallel) ..." -ForegroundColor Cyan
    $pings = 1..254 | ForEach-Object {
        $ip = "$Subnet.$_"
        $p = New-Object System.Net.NetworkInformation.Ping
        [PSCustomObject]@{ IP = $ip; Task = $p.SendPingAsync($ip, 1000) }
    }
    $total = $pings.Count
    while (($pings.Task | Where-Object { -not $_.IsCompleted }).Count -gt 0) {
        $done = ($pings.Task | Where-Object { $_.IsCompleted }).Count
        $pct = [int](($done / $total) * 100)
        Write-Progress -Activity "Scanning $Subnet.0/24" -Status "$done / $total ($pct%)" -PercentComplete $pct
        Start-Sleep -Milliseconds 200
    }
    Write-Progress -Activity "Scanning" -Completed
    $active = @()
    foreach ($p in $pings) {
        if ($p.Task.Result.Status -eq 'Success') {
            $mac = (Get-NetNeighbor -IPAddress $p.IP -ErrorAction SilentlyContinue).LinkLayerAddress
            $active += [PSCustomObject]@{ IP = $p.IP; MAC = $(if($mac){$mac}else{"-"}) }
        }
    }
    if ($active) {
        Write-Host "  Found $($active.Count) hosts:`n" -ForegroundColor Green
        $active | Sort-Object { [int]($_.IP -split '\.')[3] } | Format-Table -AutoSize
    } else { Write-Host "  No hosts responded.`n" -ForegroundColor Yellow }
}

function portcheck {
    param([Parameter(Mandatory=$true)][string]$IP, [Parameter(Mandatory=$true)][int]$Port)
    Write-Host "`n  Testing $IP : $Port ..." -ForegroundColor Cyan
    $r = Test-NetConnection -ComputerName $IP -Port $Port -WarningAction SilentlyContinue
    if ($r.TcpTestSucceeded) { Write-Host "  OPEN    ($IP $Port responds)`n" -ForegroundColor Green }
    else { Write-Host "  CLOSED / no response  ($IP $Port)`n" -ForegroundColor Yellow }
}

function flushdns {
    Write-Host "`n  Flushing DNS cache..." -ForegroundColor Cyan
    Clear-DnsClientCache
    ipconfig /flushdns | Out-Null
    Write-Host "  OK -> DNS cache cleared`n" -ForegroundColor Green
}

function reset-nic {
    param([string]$iface)
    $iface = Resolve-NetIface $iface
    $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $isAdmin) {
        Write-Host "`n  reset-nic needs admin rights. Run PowerShell as Administrator.`n" -ForegroundColor Yellow
        return
    }
    Write-Host "`n  Restarting adapter '$iface'..." -ForegroundColor Cyan
    Disable-NetAdapter -Name $iface -Confirm:$false
    Start-Sleep -Seconds 2
    Enable-NetAdapter -Name $iface -Confirm:$false
    Write-Host "  OK -> adapter restarted`n" -ForegroundColor Green
}

function whoison {
    param([Parameter(Mandatory=$true)][string]$IP)
    Write-Host "`n  Querying $IP ..." -ForegroundColor Cyan
    try {
        (New-Object System.Net.NetworkInformation.Ping).Send($IP,1000) | Out-Null
    } catch {
        Write-Host "  Invalid or unreachable host: $IP`n" -ForegroundColor Yellow
        return
    }
    $n = Get-NetNeighbor -IPAddress $IP -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $n -or -not $n.LinkLayerAddress) {
        Write-Host "  No ARP response (host offline or out of subnet)`n" -ForegroundColor Yellow
        return
    }
    $mac = $n.LinkLayerAddress
    Write-Host "  IP:           $IP"
    Write-Host "  MAC:          $mac"
    $oui = ($mac -replace '[-:]','').Substring(0,6)
    try {
        $vendor = Invoke-RestMethod -Uri "https://api.macvendors.com/$oui" -TimeoutSec 4 -ErrorAction Stop
        Write-Host "  Vendor:       $vendor" -ForegroundColor Green
    } catch {
        Write-Host "  Vendor:       (unavailable / no internet)" -ForegroundColor DarkGray
    }
    Write-Host ""
}

function speed {
    param([string]$iface)
    $iface = Resolve-NetIface $iface
    Write-Host "`n  Bandwidth monitor on '$iface' - Ctrl+C to stop`n" -ForegroundColor Cyan
    Write-Host "  Time       Download       Upload         Total down     Total up" -ForegroundColor DarkGray
    Write-Host "  ---------  -------------  -------------  -------------  -------------" -ForegroundColor DarkGray

    $stats0 = Get-NetAdapterStatistics -Name $iface
    $prevRx = $stats0.ReceivedBytes
    $prevTx = $stats0.SentBytes
    $baseRx = $prevRx
    $baseTx = $prevTx
    $start = Get-Date

    function Format-Bps($bytes) {
        $bits = $bytes * 8
        if ($bits -gt 1e9) { return ("{0,8:N2} Gbps" -f ($bits / 1e9)) }
        if ($bits -gt 1e6) { return ("{0,8:N2} Mbps" -f ($bits / 1e6)) }
        if ($bits -gt 1e3) { return ("{0,8:N2} Kbps" -f ($bits / 1e3)) }
        return ("{0,8:N0}  bps" -f $bits)
    }
    function Format-Bytes($bytes) {
        if ($bytes -gt 1e9) { return ("{0,8:N2} GB" -f ($bytes / 1e9)) }
        if ($bytes -gt 1e6) { return ("{0,8:N2} MB" -f ($bytes / 1e6)) }
        if ($bytes -gt 1e3) { return ("{0,8:N2} KB" -f ($bytes / 1e3)) }
        return ("{0,8:N0}  B" -f $bytes)
    }

    while ($true) {
        Start-Sleep -Seconds 1
        $s = Get-NetAdapterStatistics -Name $iface
        $rxBps = $s.ReceivedBytes - $prevRx
        $txBps = $s.SentBytes - $prevTx
        $totRx = $s.ReceivedBytes - $baseRx
        $totTx = $s.SentBytes - $baseTx
        $t = (Get-Date) - $start
        $tStr = "{0:mm\:ss}" -f $t
        Write-Host ("  {0}     {1}  {2}  {3}  {4}" -f $tStr, (Format-Bps $rxBps), (Format-Bps $txBps), (Format-Bytes $totRx), (Format-Bytes $totTx))
        $prevRx = $s.ReceivedBytes
        $prevTx = $s.SentBytes
    }
}

function connections {
    Write-Host "`n  Active TCP connections`n" -ForegroundColor Cyan
    $conn = Get-NetTCPConnection -State Established -ErrorAction SilentlyContinue
    if (-not $conn) { Write-Host "  No established connections.`n" -ForegroundColor Yellow; return }
    $list = foreach ($c in $conn) {
        $proc = (Get-Process -Id $c.OwningProcess -ErrorAction SilentlyContinue).ProcessName
        [PSCustomObject]@{
            Process       = $(if($proc){$proc}else{"-"})
            "Local IP"    = $c.LocalAddress
            "Local Port"  = $c.LocalPort
            "Remote IP"   = $c.RemoteAddress
            "Remote Port" = $c.RemotePort
        }
    }
    $list | Sort-Object Process, "Remote IP" | Format-Table -AutoSize
}

function speedtest {
    $cli = Get-Command speedtest.exe -ErrorAction SilentlyContinue
    if (-not $cli) {
        Write-Host "`n  Ookla Speedtest CLI not installed." -ForegroundColor Yellow
        Write-Host "  Install with: winget install Ookla.Speedtest.CLI`n" -ForegroundColor DarkGray
        return
    }
    Write-Host "`n  Speed test (Ookla)... approx 30 seconds`n" -ForegroundColor Cyan
    & speedtest.exe --accept-license --accept-gdpr
    Write-Host ""
}

function pingmulti {
    param([string[]]$IPs)
    if (-not $IPs) {
        $entry = Read-Host "  IPs separated by comma (e.g. 192.168.1.1,192.168.1.10,8.8.8.8)"
        if (-not $entry) { return }
        $IPs = $entry -split ',' | ForEach-Object { $_.Trim() }
    }
    Write-Host "`n  Monitoring $($IPs.Count) IPs - Ctrl+C to stop`n" -ForegroundColor Cyan
    while ($true) {
        Clear-Host
        Write-Host "`n  Multi-ping monitor - $(Get-Date -Format 'HH:mm:ss')`n" -ForegroundColor Cyan
        Write-Host ("  {0,-20} {1,-10} {2,-10}" -f "IP", "STATUS", "LATENCY") -ForegroundColor DarkGray
        Write-Host ("  {0,-20} {1,-10} {2,-10}" -f "--------------------", "----------", "----------") -ForegroundColor DarkGray
        $pings = foreach ($ip in $IPs) {
            $p = New-Object System.Net.NetworkInformation.Ping
            [PSCustomObject]@{ IP = $ip; Task = $p.SendPingAsync($ip, 1500) }
        }
        foreach ($p in $pings) {
            try {
                $r = $p.Task.Result
                if ($r.Status -eq 'Success') {
                    Write-Host ("  {0,-20} {1,-10} {2,-10}" -f $p.IP, "ONLINE", "$($r.RoundtripTime) ms") -ForegroundColor Green
                } else {
                    Write-Host ("  {0,-20} {1,-10} {2,-10}" -f $p.IP, "OFFLINE", "-") -ForegroundColor Red
                }
            } catch {
                Write-Host ("  {0,-20} {1,-10} {2,-10}" -f $p.IP, "ERROR", "-") -ForegroundColor Yellow
            }
        }
        Write-Host "`n  Refreshing every 2s..." -ForegroundColor DarkGray
        Start-Sleep -Seconds 2
    }
}

function latency {
    param([Parameter(Mandatory=$true)][string]$IP)
    Write-Host "`n  Latency monitor to $IP - Ctrl+C to stop`n" -ForegroundColor Cyan
    Write-Host "  Time       Latency    Graph" -ForegroundColor DarkGray
    Write-Host "  ---------  ---------  ----------------------------------------" -ForegroundColor DarkGray
    while ($true) {
        $time = Get-Date -Format "HH:mm:ss"
        try {
            $p = New-Object System.Net.NetworkInformation.Ping
            $r = $p.Send($IP, 1500)
            if ($r.Status -eq 'Success') {
                $ms = $r.RoundtripTime
                $bars = [math]::Min([math]::Floor($ms / 2), 40)
                $graph = "#" * $bars
                $color = if ($ms -lt 10) { "Green" } elseif ($ms -lt 30) { "Yellow" } else { "Red" }
                Write-Host ("  {0}  {1,5} ms   {2}" -f $time, $ms, $graph) -ForegroundColor $color
            } else {
                Write-Host ("  {0}  TIMEOUT    -" -f $time) -ForegroundColor Red
            }
        } catch {
            Write-Host ("  {0}  ERROR      -" -f $time) -ForegroundColor Yellow
        }
        Start-Sleep -Seconds 1
    }
}

function portscan {
    param([Parameter(Mandatory=$true)][string]$IP)
    $ports = @{
        21="FTP"; 22="SSH"; 23="Telnet"; 53="DNS"; 80="HTTP"
        123="NTP"; 161="SNMP"; 443="HTTPS"; 445="SMB"; 3389="RDP"
        5353="mDNS/Bonjour"; 8000="HTTP-Alt"; 8080="HTTP-Proxy"; 8443="HTTPS-Alt"
    }
    Write-Host "`n  Scanning common ports on $IP (parallel) ..." -ForegroundColor Cyan
    Write-Host ("  {0,-8} {1,-12} {2}" -f "PORT", "STATUS", "SERVICE") -ForegroundColor DarkGray
    Write-Host ("  {0,-8} {1,-12} {2}" -f "--------", "------------", "------------") -ForegroundColor DarkGray
    $timeout = 1000
    $checks = foreach ($p in $ports.Keys) {
        $client = New-Object System.Net.Sockets.TcpClient
        [PSCustomObject]@{
            Port   = [int]$p
            Svc    = $ports[$p]
            Client = $client
            Task   = $client.ConnectAsync($IP, [int]$p)
        }
    }
    $deadline = (Get-Date).AddMilliseconds($timeout)
    while ((Get-Date) -lt $deadline -and ($checks.Task | Where-Object { -not $_.IsCompleted }).Count -gt 0) {
        Start-Sleep -Milliseconds 50
    }
    foreach ($c in ($checks | Sort-Object Port)) {
        $open = $c.Task.IsCompleted -and -not $c.Task.IsFaulted -and $c.Client.Connected
        if ($open) {
            Write-Host ("  {0,-8} {1,-12} {2}" -f $c.Port, "OPEN", $c.Svc) -ForegroundColor Green
        } else {
            Write-Host ("  {0,-8} {1,-12} {2}" -f $c.Port, "closed", $c.Svc) -ForegroundColor DarkGray
        }
        $c.Client.Close()
    }
    Write-Host ""
}

function nettools {
    $TL=[char]0x250C; $TR=[char]0x2510; $BL=[char]0x2514; $BR=[char]0x2518
    $HZ=[char]0x2500; $VT=[char]0x2502
    $boxW = 52
    while ($true) {
        Clear-Host
        Write-Host ""
        $title = "NETWORK TOOLS"
        $pad = $boxW - $title.Length; if ($pad -lt 0) { $pad = 0 }
        $lp = [math]::Floor($pad / 2); $rp = $pad - $lp
        Write-Host ("  " + $TL + ("$HZ" * $boxW) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT) -NoNewline -ForegroundColor DarkGray
        Write-Host ((" " * $lp) + $title + (" " * $rp)) -NoNewline -ForegroundColor Cyan
        Write-Host $VT -ForegroundColor DarkGray
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "IP Profile" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - ("IP Profile").Length - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   1)  Profile 0    (192.168.0.99)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   2)  Profile 1    (192.168.1.99)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   3)  DHCP"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   4)  Custom IP    (manual entry)"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Diagnostics" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - ("Diagnostics").Length - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   5)  netinfo      (Ethernet interface)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   6)  myip         (all adapters + public IP)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   7)  scan         (current subnet)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   8)  scan         (other subnet)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   9)  portcheck    (single port)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  10)  portscan     (common ports of an IP)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  11)  whoison"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  12)  pingmulti    (multiple IPs at once)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  13)  latency      (live latency monitor)"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Traffic" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - ("Traffic").Length - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  14)  speed        (live download/upload monitor)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  15)  connections  (who talks to whom)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  16)  speedtest    (Internet speed test)"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Utilities" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - ("Utilities").Length - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  17)  flushdns"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  18)  reset-nic"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host "    0)  Exit" -ForegroundColor DarkGray
        Write-Host ""
        $op = Read-Host "  Choose option"
        switch ($op) {
            "1"  { & C:\Scripts\SetIP.ps1 0 }
            "2"  { & C:\Scripts\SetIP.ps1 1 }
            "3"  { & C:\Scripts\SetIP.ps1 dhcp }
            "4"  {
                $addr = Read-Host "  IP/CIDR (e.g. 192.168.1.25/24)"
                if (-not $addr) { break }
                $gw = Read-Host "  Gateway (optional, leave empty for none)"
                if ($gw) { & C:\Scripts\SetIP.ps1 $addr $gw } else { & C:\Scripts\SetIP.ps1 $addr }
            }
            "5"  { netinfo }
            "6"  { myip }
            "7"  { scan }
            "8"  { $s = Read-Host "  Subnet (e.g. 192.168.10)"; if ($s) { scan $s } }
            "9"  { $ip = Read-Host "  IP"; $pt = Read-Host "  Port"; if ($ip -and $pt) { portcheck $ip ([int]$pt) } }
            "10" { $ip = Read-Host "  IP to scan"; if ($ip) { portscan $ip } }
            "11" { $ip = Read-Host "  IP"; if ($ip) { whoison $ip } }
            "12" { pingmulti }
            "13" { $ip = Read-Host "  IP to monitor"; if ($ip) { latency $ip } }
            "14" { speed }
            "15" { connections }
            "16" { speedtest }
            "17" { flushdns }
            "18" { reset-nic }
            "0"  { return }
            default { Write-Host "  Invalid option" -ForegroundColor Yellow }
        }
        Write-Host ""
        Read-Host "  [Enter] to return"
    }
}
