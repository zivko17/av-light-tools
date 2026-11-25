# ===================================================================
#  NetTools.ps1 - simple network tools for the show network
#
#    netinfo                 every active adapter: IP, gateway, DNS, MAC, speed
#    scan [192.168.1]        who is on the network (IP + MAC), current subnet by default
#    pingmulti IP IP ...     live monitor of several devices (online / latency / lost)
#    portcheck IP PORT       is that TCP port open?
#    whoison IP              MAC and vendor of a device
#    speed                   live bandwidth of the active adapter
#    speedtest               Internet speed test (Ookla CLI)
#    flushdns / reset-nic    quick fixes
#
#  To force an adapter, set  $NetIface = "Ethernet 2"  in your profile.
# ===================================================================

function Resolve-NetIface {
    param([string]$Iface)
    if ($Iface)          { return $Iface }
    if ($global:NetIface) { return $global:NetIface }
    # The adapter that owns the default route...
    $route = Get-NetRoute -DestinationPrefix "0.0.0.0/0" -ErrorAction SilentlyContinue |
             Where-Object { $_.NextHop -ne "0.0.0.0" } |
             Sort-Object RouteMetric | Select-Object -First 1
    if ($route) {
        $a = Get-NetAdapter -InterfaceIndex $route.InterfaceIndex -ErrorAction SilentlyContinue
        if ($a) { return $a.Name }
    }
    # ...or, on an isolated show network with no gateway, the first wired adapter that is up.
    $a = Get-NetAdapter -Physical -ErrorAction SilentlyContinue |
         Where-Object { $_.Status -eq 'Up' } | Sort-Object { $_.MediaType -ne '802.3' }, ifIndex |
         Select-Object -First 1
    if ($a) { return $a.Name }
    return $null
}

function netinfo {
    $configs = Get-NetIPConfiguration -ErrorAction SilentlyContinue |
               Where-Object { $_.NetAdapter.Status -eq 'Up' -and $_.IPv4Address }
    if (-not $configs) { Write-Host "`n  No active network adapter.`n" -ForegroundColor Yellow; return }

    foreach ($c in $configs) {
        Write-Host "`n  [$($c.InterfaceAlias)]" -ForegroundColor Cyan
        foreach ($a in $c.IPv4Address) {
            Write-Host ("    IP:       {0}/{1}" -f $a.IPAddress, $a.PrefixLength) -ForegroundColor White
        }
        $dhcp = $c.NetIPv4Interface.Dhcp -eq 'Enabled'
        Write-Host ("    Mode:     {0}" -f $(if ($dhcp) { 'DHCP (automatic)' } else { 'Static' })) -ForegroundColor $(if ($dhcp) { 'Green' } else { 'Yellow' })
        $gw = $c.IPv4DefaultGateway.NextHop
        Write-Host ("    Gateway:  {0}" -f $(if ($gw) { $gw -join ', ' } else { '(none)' }))
        $dns = $c.DNSServer | Where-Object { $_.AddressFamily -eq 2 } | ForEach-Object { $_.ServerAddresses }
        Write-Host ("    DNS:      {0}" -f $(if ($dns) { $dns -join ', ' } else { '(none)' }))
        Write-Host ("    MAC:      {0}" -f $c.NetAdapter.MacAddress)
        $speed = $c.NetAdapter.LinkSpeed
        $color = if ($speed -match 'Gbps') { 'Green' } elseif ($speed -match '^100 Mbps') { 'Yellow' } else { 'Red' }
        Write-Host ("    Link:     {0}" -f $speed) -ForegroundColor $color
    }

    # Public IP only when there is a way out (saves a timeout on isolated show networks)
    if ($configs | Where-Object { $_.IPv4DefaultGateway }) {
        try {
            $pub = Invoke-RestMethod -Uri "https://api.ipify.org" -TimeoutSec 3
            Write-Host "`n  Public IP:  $pub" -ForegroundColor Green
        } catch {
            Write-Host "`n  Public IP:  (no Internet)" -ForegroundColor DarkGray
        }
    }
    Write-Host ""
}

function scan {
    param([string]$Subnet)

    if (-not $Subnet) {
        $iface = Resolve-NetIface
        $myip = if ($iface) {
            Get-NetIPAddress -InterfaceAlias $iface -AddressFamily IPv4 -ErrorAction SilentlyContinue |
                Select-Object -First 1 -ExpandProperty IPAddress
        }
        if (-not $myip) { Write-Host "`n  No IP on this PC. Say which network:  scan 192.168.1`n" -ForegroundColor Yellow; return }
        $Subnet = ($myip -split '\.')[0..2] -join '.'
    }
    # Accept 192.168.1  /  192.168.1.0  /  192.168.1.0/24  /  192.168.1.*
    $Subnet = $Subnet.Trim() -replace '(\.(0|\*))?(/\d+)?$', ''
    if ($Subnet -notmatch '^\d{1,3}\.\d{1,3}\.\d{1,3}$') {
        Write-Host "`n  Use the first three numbers, e.g.  scan 192.168.1`n" -ForegroundColor Yellow
        return
    }

    Write-Host "`n  Scanning $Subnet.1 - $Subnet.254 ..." -ForegroundColor Cyan
    $pingers = 1..254 | ForEach-Object { New-Object System.Net.NetworkInformation.Ping }
    $tasks   = for ($i = 0; $i -lt 254; $i++) { $pingers[$i].SendPingAsync("$Subnet.$($i + 1)", 800) }
    try { [System.Threading.Tasks.Task]::WaitAll([System.Threading.Tasks.Task[]]$tasks) } catch { }
    $pingers | ForEach-Object { $_.Dispose() }

    # One ARP-table read. Devices that block ping still answer ARP, so they show up too.
    $arp = @{}
    Get-NetNeighbor -AddressFamily IPv4 -ErrorAction SilentlyContinue |
        Where-Object { $_.IPAddress -like "$Subnet.*" -and $_.State -ne 'Unreachable' -and
                       $_.LinkLayerAddress -and $_.LinkLayerAddress -notmatch '^(00-){5}00$|^(FF-){5}FF$' } |
        ForEach-Object { $arp[$_.IPAddress] = $_.LinkLayerAddress }
    $mine = @(Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | ForEach-Object { $_.IPAddress })

    $found = for ($i = 0; $i -lt 254; $i++) {
        $ip = "$Subnet.$($i + 1)"
        $t  = $tasks[$i]
        $ok = $t.Status -eq 'RanToCompletion' -and $t.Result.Status -eq 'Success'
        if ($ok -or $arp.ContainsKey($ip)) {
            [PSCustomObject]@{
                IP   = $ip
                MAC  = if ($mine -contains $ip) { '(this PC)' } elseif ($arp[$ip]) { $arp[$ip] } else { '-' }
                Ping = if ($ok) { "$($t.Result.RoundtripTime) ms" } else { 'no reply' }
            }
        }
    }

    if ($found) {
        Write-Host "  $(@($found).Count) devices found:" -ForegroundColor Green
        $found | Format-Table -AutoSize
    } else {
        Write-Host "  Nothing answered on $Subnet.x`n" -ForegroundColor Yellow
    }
}

function pingmulti {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$IPs)
    $IPs = @($IPs | ForEach-Object { $_ -split '[,\s]+' } | Where-Object { $_ })
    if (-not $IPs) {
        $entry = Read-Host "  IPs separated by spaces or commas"
        $IPs = @($entry -split '[,\s]+' | Where-Object { $_ })
        if (-not $IPs) { return }
    }
    $lost = @{}; $sent = 0
    foreach ($ip in $IPs) { $lost[$ip] = 0 }

    while ($true) {
        $pingers = @($IPs | ForEach-Object { New-Object System.Net.NetworkInformation.Ping })
        $tasks = for ($i = 0; $i -lt $IPs.Count; $i++) { $pingers[$i].SendPingAsync($IPs[$i], 1000) }
        try { [System.Threading.Tasks.Task]::WaitAll([System.Threading.Tasks.Task[]]@($tasks)) } catch { }
        $sent++

        Clear-Host
        Write-Host "`n  Ping monitor - $(Get-Date -Format 'HH:mm:ss')   (Ctrl+C to stop)`n" -ForegroundColor Cyan
        Write-Host ("  {0,-18} {1,-9} {2,-9} {3}" -f 'IP', 'STATUS', 'LATENCY', 'LOST') -ForegroundColor DarkGray
        for ($i = 0; $i -lt $IPs.Count; $i++) {
            $t = @($tasks)[$i]
            $ok = $t.Status -eq 'RanToCompletion' -and $t.Result.Status -eq 'Success'
            if (-not $ok) { $lost[$IPs[$i]]++ }
            $line = "  {0,-18} {1,-9} {2,-9} {3}/{4}" -f $IPs[$i],
                    $(if ($ok) { 'ONLINE' } else { 'OFFLINE' }),
                    $(if ($ok) { "$($t.Result.RoundtripTime) ms" } else { '-' }),
                    $lost[$IPs[$i]], $sent
            Write-Host $line -ForegroundColor $(if ($ok) { 'Green' } else { 'Red' })
        }
        $pingers | ForEach-Object { $_.Dispose() }
        Start-Sleep -Seconds 1
    }
}

function portcheck {
    param([Parameter(Mandatory = $true)][string]$IP, [Parameter(Mandatory = $true)][int]$Port)
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $open = $false
        try { $open = $client.ConnectAsync($IP, $Port).Wait(1500) -and $client.Connected } catch { }
        if ($open) { Write-Host "`n  $IP : $Port  OPEN`n" -ForegroundColor Green }
        else       { Write-Host "`n  $IP : $Port  closed / no answer`n" -ForegroundColor Yellow }
    } finally { $client.Dispose() }
}

function whoison {
    param([Parameter(Mandatory = $true)][string]$IP)
    $p = New-Object System.Net.NetworkInformation.Ping
    try { $null = $p.Send($IP, 1000) } catch { } finally { $p.Dispose() }

    $n = Get-NetNeighbor -IPAddress $IP -AddressFamily IPv4 -ErrorAction SilentlyContinue |
         Where-Object { $_.State -ne 'Unreachable' -and $_.LinkLayerAddress -notmatch '^(00-){5}00$' } |
         Select-Object -First 1
    if (-not $n) {
        Write-Host "`n  $IP did not answer (offline, or not on this PC's network)`n" -ForegroundColor Yellow
        return
    }
    $mac = $n.LinkLayerAddress
    Write-Host "`n  IP:      $IP"
    Write-Host "  MAC:     $mac"
    try {
        $vendor = Invoke-RestMethod -Uri "https://api.macvendors.com/$(($mac -replace '[-:]', '').Substring(0, 6))" -TimeoutSec 4
        Write-Host "  Vendor:  $vendor" -ForegroundColor Green
    } catch {
        Write-Host "  Vendor:  (unknown or no Internet)" -ForegroundColor DarkGray
    }
    Write-Host ""
}

function speed {
    param([string]$Iface)
    $Iface = Resolve-NetIface $Iface
    $s0 = if ($Iface) { Get-NetAdapterStatistics -Name $Iface -ErrorAction SilentlyContinue }
    if (-not $s0) { Write-Host "`n  No active adapter found. Try:  speed 'Ethernet 2'`n" -ForegroundColor Yellow; return }

    function Format-Rate([double]$bytesPerSec) {
        $bits = $bytesPerSec * 8
        if ($bits -ge 1e9) { return ("{0,7:N2} Gbps" -f ($bits / 1e9)) }
        if ($bits -ge 1e6) { return ("{0,7:N2} Mbps" -f ($bits / 1e6)) }
        return ("{0,7:N0} kbps" -f ($bits / 1e3))
    }

    Write-Host "`n  Bandwidth on '$Iface'  (Ctrl+C to stop)`n" -ForegroundColor Cyan
    Write-Host "  Time      Download       Upload" -ForegroundColor DarkGray
    $prev = $s0; $t0 = Get-Date
    while ($true) {
        Start-Sleep -Seconds 1
        $s = Get-NetAdapterStatistics -Name $Iface -ErrorAction SilentlyContinue
        if (-not $s) { continue }
        $now = Get-Date; $dt = ($now - $t0).TotalSeconds; if ($dt -le 0) { $dt = 1 }
        Write-Host ("  {0}  {1}   {2}" -f $now.ToString('HH:mm:ss'),
            (Format-Rate (($s.ReceivedBytes - $prev.ReceivedBytes) / $dt)),
            (Format-Rate (($s.SentBytes - $prev.SentBytes) / $dt)))
        $prev = $s; $t0 = $now
    }
}

function speedtest {
    if (-not (Get-Command speedtest.exe -ErrorAction SilentlyContinue)) {
        Write-Host "`n  Ookla Speedtest CLI not installed:  winget install Ookla.Speedtest.CLI`n" -ForegroundColor Yellow
        return
    }
    Write-Host "`n  Internet speed test (about 30 s)...`n" -ForegroundColor Cyan
    & speedtest.exe --accept-license --accept-gdpr @args
    Write-Host ""
}

function flushdns {
    Clear-DnsClientCache
    Write-Host "`n  DNS cache cleared`n" -ForegroundColor Green
}

function reset-nic {
    param([string]$Iface)
    $Iface = Resolve-NetIface $Iface
    if (-not $Iface) { Write-Host "`n  No adapter found. Try:  reset-nic 'Ethernet 2'`n" -ForegroundColor Yellow; return }
    $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $isAdmin) { Write-Host "`n  reset-nic needs PowerShell as Administrator.`n" -ForegroundColor Yellow; return }
    Write-Host "`n  Restarting '$Iface'..." -ForegroundColor Cyan
    Disable-NetAdapter -Name $Iface -Confirm:$false
    Start-Sleep -Seconds 2
    Enable-NetAdapter -Name $Iface -Confirm:$false
    Write-Host "  Done`n" -ForegroundColor Green
}
