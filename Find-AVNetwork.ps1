#requires -Version 5.1
<#
.SYNOPSIS
  Descubre qué hay en una red desconocida (sin DHCP, IPs fijas, varias subredes).

.DESCRIPTION
  1. Escucha la red en silencio con tshark (ARP, LLDP, CDP, mDNS, SSDP, DHCP, PTP, broadcasts).
  2. Deduce las subredes en uso y, si el switch es gestionable, su nombre, IP y tu puerto.
  3. Se pone una IP libre en cada subred (comprobando duplicados) solo en memoria (ActiveStore).
  4. Barre cada /24 por ARP: encuentra también equipos que no responden a ping.
  5. Identifica el fabricante por MAC (lista IEEE cacheada + tabla AV de respaldo).
  6. Exporta CSV al escritorio y deja la tarjeta como estaba (salvo que elijas mantener las IPs).

.EXAMPLE
  .\Find-AVNetwork.ps1
  .\Find-AVNetwork.ps1 -Seconds 90           # más tiempo de escucha (CDP anuncia cada 60 s)
  .\Find-AVNetwork.ps1 -Interface "Ethernet 2"
  .\Find-AVNetwork.ps1 -PassiveOnly          # solo escuchar, no toca la configuración IP
  .\Find-AVNetwork.ps1 -Defaults             # barre además subredes típicas de fábrica
  .\Find-AVNetwork.ps1 -Keep                 # deja las IPs puestas para trabajar
#>
[CmdletBinding()]
param(
    [string]$Interface,
    [int]$Seconds = 60,
    [switch]$PassiveOnly,
    [switch]$Defaults,
    [switch]$Keep,
    [switch]$Force,
    [string]$OutDir = [Environment]::GetFolderPath('Desktop')
)

$ErrorActionPreference = 'Stop'
$DefaultSubnets = '192.168.0', '192.168.1', '192.168.2', '192.168.10', '192.168.100', '10.0.0', '10.10.10'
$HostCandidates = 237, 238, 239, 236, 235, 234, 233, 199, 198, 197, 149, 148

function Write-Step([string]$t) { Write-Host "`n>> $t" -ForegroundColor Cyan }
function Write-Info([string]$t) { Write-Host "   $t" -ForegroundColor Gray }
function Write-Warn2([string]$t) { Write-Host "   $t" -ForegroundColor Yellow }

# ---------------------------------------------------------------- comprobaciones
$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "Ejecuta PowerShell como administrador." -ForegroundColor Red; return
}

$tshark = @(
    (Get-Command tshark.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -First 1),
    "$env:ProgramFiles\Wireshark\tshark.exe",
    "${env:ProgramFiles(x86)}\Wireshark\tshark.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $tshark) { Write-Host "No encuentro tshark.exe (viene con Wireshark)." -ForegroundColor Red; return }

# ---------------------------------------------------------------- interfaz
if ($Interface) {
    $adapter = Get-NetAdapter -Name $Interface
} else {
    $cands = @(Get-NetAdapter -Physical | Where-Object { $_.Status -eq 'Up' -and $_.NdisPhysicalMedium -eq 14 })
    if ($cands.Count -eq 0) { Write-Host "No hay ninguna tarjeta Ethernet con cable conectado." -ForegroundColor Red; return }
    if ($cands.Count -eq 1) { $adapter = $cands[0] }
    else {
        for ($i = 0; $i -lt $cands.Count; $i++) { Write-Host ("  [{0}] {1}  ({2}, {3})" -f $i, $cands[$i].Name, $cands[$i].InterfaceDescription, $cands[$i].LinkSpeed) }
        $adapter = $cands[[int](Read-Host "Elige interfaz")]
    }
}
$ifIndex = $adapter.ifIndex
$ifName  = $adapter.Name

function Format-Mac([string]$m) {
    $h = ($m -replace '[^0-9A-Fa-f]', '').ToUpper()
    if ($h.Length -ne 12) { return $m }
    ($h -split '(..)' | Where-Object { $_ }) -join '-'
}
$myMac = Format-Mac $adapter.MacAddress

function Test-UsableIp([string]$ip) {
    $a = $null
    if (-not [System.Net.IPAddress]::TryParse($ip, [ref]$a)) { return $false }
    if ($a.AddressFamily -ne [System.Net.Sockets.AddressFamily]::InterNetwork) { return $false }
    $b = $a.GetAddressBytes()
    if ($b[0] -eq 0 -or $b[0] -eq 127 -or $b[0] -ge 224) { return $false }
    if ($b[3] -eq 0 -or $b[3] -eq 255) { return $false }
    $true
}
function Get-Prefix([string]$ip) {
    if ($ip -like '169.254.*') { return '169.254' }
    (($ip -split '\.')[0..2]) -join '.'
}

$found = @{}
function Add-Found([string]$Ip, [string]$Mac, [string]$Via, [string]$Name) {
    if (-not (Test-UsableIp $Ip)) { return }
    if ($Mac) {
        $Mac = Format-Mac $Mac
        if ($Mac -eq $myMac -or $Mac -eq 'FF-FF-FF-FF-FF-FF' -or $Mac -eq '00-00-00-00-00-00' -or $Mac -like '01-00-5E*') { return }
    }
    if (-not $found.ContainsKey($Ip)) {
        $found[$Ip] = [pscustomobject]@{ IP = $Ip; MAC = $Mac; Nombre = $Name; Via = New-Object System.Collections.Generic.List[string] }
    }
    $h = $found[$Ip]
    if (-not $h.MAC -and $Mac) { $h.MAC = $Mac }
    if (-not $h.Nombre -and $Name) { $h.Nombre = $Name }
    if ($Via -and -not $h.Via.Contains($Via)) { $h.Via.Add($Via) }
}

Write-Host ("`nInterfaz: {0}  ({1})  MAC {2}  {3}" -f $ifName, $adapter.InterfaceDescription, $myMac, $adapter.LinkSpeed) -ForegroundColor White

# Estado original
$ipIf = Get-NetIPInterface -InterfaceIndex $ifIndex -AddressFamily IPv4
$wasDhcp = ($ipIf.Dhcp.ToString() -eq 'Enabled')
$origAddrs = @(Get-NetIPAddress -InterfaceIndex $ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue)
foreach ($a in $origAddrs) { Write-Info ("IP actual: {0}/{1}  [{2}, {3}]" -f $a.IPAddress, $a.PrefixLength, $a.PrefixOrigin, $a.AddressState) }
Write-Info ("DHCP: {0}" -f $ipIf.Dhcp)

$script:added = @()
$script:dhcpDisabled = $false
$stamp = Get-Date -Format 'yyyyMMdd_HHmmss'

# ---------------------------------------------------------------- 1. escucha
Write-Step "Escuchando $Seconds s (no envío nada)..."
$pcap = Join-Path $env:TEMP "avdiscover_$stamp.pcapng"
$errF = Join-Path $env:TEMP "avdiscover_$stamp.err"
$bpf  = 'arp or ether proto 0x88cc or ether dst 01:00:0c:cc:cc:cc or ether broadcast or igmp or udp port 5353 or udp port 67 or udp port 68 or udp port 1900 or udp port 319 or udp port 320'
$capArgs = @('-i', "`"\Device\NPF_$($adapter.InterfaceGuid)`"", '-f', "`"$bpf`"", '-a', "duration:$Seconds", '-w', "`"$pcap`"", '-q')
$proc = Start-Process -FilePath $tshark -ArgumentList $capArgs -PassThru -NoNewWindow -RedirectStandardError $errF
$sw = [Diagnostics.Stopwatch]::StartNew()
while (-not $proc.HasExited) {
    $el = [int]$sw.Elapsed.TotalSeconds
    Write-Progress -Activity "Escuchando en $ifName" -Status "$el / $Seconds s" -PercentComplete ([math]::Min(100, [int]($el * 100 / [math]::Max(1, $Seconds))))
    Start-Sleep -Milliseconds 500
    if ($el -gt $Seconds + 20) { $proc.Kill(); break }
}
Write-Progress -Activity "Escuchando" -Completed
if (-not (Test-Path $pcap)) {
    Write-Host "La captura ha fallado:" -ForegroundColor Red
    if (Test-Path $errF) { Get-Content $errF | Write-Host }
    Write-Host "¿Está instalado Npcap? (se instala con Wireshark)" -ForegroundColor Red
    return
}

# Campos: valida contra esta versión de tshark (los nombres cambian entre versiones)
$wanted = 'eth.src', 'ip.src', 'arp.src.hw_mac', 'arp.src.proto_ipv4',
          'lldp.tlv.system.name', 'lldp.mgn.addr.ip4', 'lldp.port.id', 'lldp.port.desc',
          'cdp.deviceid', 'cdp.platform', 'cdp.portid',
          'dhcp.option.dhcp', 'bootp.option.dhcp',
          'dns.flags.response', 'dns.resp.name', 'udp.srcport'
$rx = "`t(" + (($wanted | ForEach-Object { [regex]::Escape($_) }) -join '|') + ")`t"
$known = @{}
foreach ($l in ((& $tshark -G fields 2>$null) -match $rx)) { $known[($l -split "`t")[2]] = $true }
$fields = @($wanted | Where-Object { $known.ContainsKey($_) })

$rdArgs = @('-r', $pcap, '-T', 'fields', '-E', 'separator=|', '-E', 'aggregator=;', '-E', 'occurrence=a')
foreach ($f in $fields) { $rdArgs += '-e'; $rdArgs += $f }
$rows = @(& $tshark @rdArgs 2>$null)
Write-Info ("{0} paquetes relevantes capturados" -f $rows.Count)

$switches = @{}
$dhcpServers = @{}
foreach ($row in $rows) {
    $v = $row -split '\|'
    $F = @{}
    for ($i = 0; $i -lt $fields.Count; $i++) { $F[$fields[$i]] = if ($i -lt $v.Count) { $v[$i] } else { '' } }
    $first = { param($k) if ($F.ContainsKey($k) -and $F[$k]) { ($F[$k] -split ';')[0] } else { '' } }

    $ethSrc = & $first 'eth.src'
    $ipSrc  = & $first 'ip.src'

    $arpIp = & $first 'arp.src.proto_ipv4'
    if ($arpIp) { Add-Found $arpIp (& $first 'arp.src.hw_mac') 'ARP' $null }

    if ($ipSrc) { Add-Found $ipSrc $ethSrc 'Pasivo' $null }

    $lldpName = & $first 'lldp.tlv.system.name'
    $lldpIp   = & $first 'lldp.mgn.addr.ip4'
    $cdpName  = & $first 'cdp.deviceid'
    if ($lldpName -or $lldpIp -or $cdpName) {
        $key = Format-Mac $ethSrc
        $switches[$key] = [pscustomobject]@{
            Protocolo = if ($cdpName) { 'CDP' } else { 'LLDP' }
            Nombre    = if ($cdpName) { $cdpName } else { $lldpName }
            IP_gestion = $lldpIp
            Tu_puerto = (@((& $first 'cdp.portid'), (& $first 'lldp.port.desc'), (& $first 'lldp.port.id')) | Where-Object { $_ } | Select-Object -First 1)
            Modelo    = & $first 'cdp.platform'
            MAC       = $key
        }
        if ($lldpIp) { Add-Found $lldpIp $ethSrc 'LLDP' ("Switch: " + $lldpName) }
    }

    $dhcpType = @((& $first 'dhcp.option.dhcp'), (& $first 'bootp.option.dhcp')) | Where-Object { $_ } | Select-Object -First 1
    if ($dhcpType -in '2', '5' -and $ipSrc) { $dhcpServers[$ipSrc] = $true; Add-Found $ipSrc $ethSrc 'DHCP' 'Servidor DHCP' }

    if ((& $first 'udp.srcport') -eq '5353' -and (& $first 'dns.flags.response') -in '1', 'True') {
        $names = @(($F['dns.resp.name'] -split ';') | Where-Object { $_ -match '\.local$' -and $_ -notmatch '^_' -and $_ -notmatch '\._' } | Sort-Object Length)
        if ($names.Count -and $ipSrc) { Add-Found $ipSrc $ethSrc 'mDNS' ($names[0] -replace '\.local$', '') }
    }
}

$passiveSubnets = @($found.Keys | ForEach-Object { Get-Prefix $_ } | Sort-Object -Unique)
Write-Info ("Equipos vistos en escucha: {0}" -f $found.Count)
if ($passiveSubnets.Count) { Write-Info ("Subredes detectadas: " + (($passiveSubnets | ForEach-Object { if ($_ -eq '169.254') { '169.254.0.0/16 (link-local)' } else { "$_.0/24" } }) -join ', ')) }
if ($dhcpServers.Count) { Write-Info ("Hay DHCP en la red: " + ($dhcpServers.Keys -join ', ')) }

# ---------------------------------------------------------------- funciones de config
function Add-FreeAddress([string]$Prefix, [int]$Len = 24) {
    $base = if ($Prefix -eq '169.254') { '169.254.237' } else { $Prefix }
    foreach ($o in $HostCandidates) {
        $ip = "$base.$o"
        if ($found.ContainsKey($ip)) { continue }
        try { New-NetIPAddress -InterfaceIndex $ifIndex -IPAddress $ip -PrefixLength $Len -PolicyStore ActiveStore -ErrorAction Stop | Out-Null }
        catch { continue }
        $state = 'Tentative'
        for ($i = 0; $i -lt 25 -and $state -eq 'Tentative'; $i++) {
            Start-Sleep -Milliseconds 300
            $st = Get-NetIPAddress -InterfaceIndex $ifIndex -IPAddress $ip -ErrorAction SilentlyContinue
            if ($st) { $state = $st.AddressState.ToString() }
        }
        if ($state -eq 'Preferred') { $script:added += $ip; return $ip }
        Remove-NetIPAddress -InterfaceIndex $ifIndex -IPAddress $ip -PolicyStore ActiveStore -Confirm:$false -ErrorAction SilentlyContinue
        Write-Warn2 "$ip ocupada ($state), pruebo otra"
    }
    return $null
}

function Restore-Net {
    foreach ($ip in $script:added) {
        Remove-NetIPAddress -InterfaceIndex $ifIndex -IPAddress $ip -PolicyStore ActiveStore -Confirm:$false -ErrorAction SilentlyContinue
    }
    if ($script:dhcpDisabled) { Set-NetIPInterface -InterfaceIndex $ifIndex -Dhcp Enabled -ErrorAction SilentlyContinue }
}

function Invoke-Sweep([string[]]$Ips) {
    $ok = @{}
    for ($i = 0; $i -lt $Ips.Count; $i += 128) {
        $chunk = $Ips[$i..([math]::Min($i + 128, $Ips.Count) - 1)]
        $jobs = foreach ($ip in $chunk) {
            $p = New-Object System.Net.NetworkInformation.Ping
            [pscustomobject]@{ Ip = $ip; Task = $p.SendPingAsync($ip, 500) }
        }
        try { [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($jobs | ForEach-Object { $_.Task })) } catch {}
        foreach ($j in $jobs) {
            if ($j.Task.Status -eq 'RanToCompletion' -and $j.Task.Result.Status -eq 'Success') { $ok[$j.Ip] = $true }
        }
    }
    $ok
}

# ---------------------------------------------------------------- 2-4. config + barrido
try {
    if (-not $PassiveOnly) {
        $targets = New-Object System.Collections.Generic.List[string]
        foreach ($s in $passiveSubnets) { if (-not $targets.Contains($s)) { $targets.Add($s) } }
        if ($Defaults -or $passiveSubnets.Count -eq 0) {
            if ($passiveSubnets.Count -eq 0) { Write-Warn2 "No se ha oído a nadie: pruebo subredes típicas de fábrica." }
            foreach ($s in $DefaultSubnets) { if (-not $targets.Contains($s)) { $targets.Add($s) } }
        }

        # No pisar subredes que ya usan otras interfaces (Wi-Fi, etc.)
        $otherPrefixes = @(Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
            Where-Object { $_.InterfaceIndex -ne $ifIndex -and $_.IPAddress -notlike '169.254.*' -and $_.IPAddress -ne '127.0.0.1' } |
            ForEach-Object { Get-Prefix $_.IPAddress })

        Write-Step "Configurando IPs temporales en $ifName"
        if ($wasDhcp) {
            $lease = @($origAddrs | Where-Object { $_.IPAddress -notlike '169.254.*' -and $_.AddressState.ToString() -eq 'Preferred' })
            Set-NetIPInterface -InterfaceIndex $ifIndex -Dhcp Disabled
            $script:dhcpDisabled = $true
            Start-Sleep -Seconds 1
            foreach ($l in $lease) {
                New-NetIPAddress -InterfaceIndex $ifIndex -IPAddress $l.IPAddress -PrefixLength $l.PrefixLength -PolicyStore ActiveStore -ErrorAction SilentlyContinue | Out-Null
                $script:added += $l.IPAddress
                Write-Info "Mantengo la IP de DHCP $($l.IPAddress) durante el escaneo"
            }
        }

        $mine = @{}
        foreach ($a in @(Get-NetIPAddress -InterfaceIndex $ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.AddressState.ToString() -eq 'Preferred' })) {
            $mine[(Get-Prefix $a.IPAddress)] = $a.IPAddress
        }

        $sweepList = New-Object System.Collections.Generic.List[string]
        foreach ($s in $targets) {
            if ($s -ne '169.254' -and $otherPrefixes -contains $s -and -not $Force) {
                Write-Warn2 "$s.0/24 ya la usa otra interfaz (¿Wi-Fi?). La salto; usa -Force o desconecta la otra red."
                continue
            }
            if (-not $mine.ContainsKey($s)) {
                $len = if ($s -eq '169.254') { 16 } else { 24 }
                $ip = Add-FreeAddress $s $len
                if ($ip) { $mine[$s] = $ip; Write-Info "+ $ip/$len" } else { Write-Warn2 "No encuentro IP libre en $s"; continue }
            } else { Write-Info "Ya tengo $($mine[$s]) en $s" }

            if ($s -eq '169.254') {
                # /16 es demasiado grande para barrer: solo confirmo los oídos
                $found.Keys | Where-Object { $_ -like '169.254.*' } | ForEach-Object { $sweepList.Add($_) }
            } else {
                1..254 | ForEach-Object { $ip2 = "$s.$_"; if ($ip2 -ne $mine[$s]) { $sweepList.Add($ip2) } }
            }
        }

        if ($sweepList.Count) {
            Write-Step ("Barriendo {0} direcciones por ARP/ping..." -f $sweepList.Count)
            $pingOk = Invoke-Sweep $sweepList.ToArray()
            Start-Sleep -Seconds 1
            $myIps = @($mine.Values)
            Get-NetNeighbor -InterfaceIndex $ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue |
                Where-Object { $_.State.ToString() -in 'Reachable', 'Stale', 'Delay', 'Probe' -and $_.LinkLayerAddress -and $myIps -notcontains $_.IPAddress } |
                ForEach-Object { Add-Found $_.IPAddress $_.LinkLayerAddress 'ARP-scan' $null }
            foreach ($ip in $pingOk.Keys) { if ($found.ContainsKey($ip)) { Add-Found $ip $null 'Ping' $null } }
        }
    }

    # ------------------------------------------------------------ 5. fabricantes
    Write-Step "Identificando fabricantes"
    $oui = @{
        '000EDD' = 'Shure'; '001DC1' = 'Audinate (Dante)'; '001CAB' = 'Meyer Sound'; '00A0DE' = 'Yamaha'
        '7C2E0D' = 'Blackmagic Design'; '00107F' = 'Crestron'; '0005A6' = 'Extron'; '006074' = 'QSC'
        '008045' = 'Panasonic'; '080046' = 'Sony'; '000E58' = 'Sonos'
        'B827EB' = 'Raspberry Pi'; 'DCA632' = 'Raspberry Pi'; 'E45F01' = 'Raspberry Pi'
        '00155D' = 'Microsoft Hyper-V'; '000C29' = 'VMware'
        'F09FC2' = 'Ubiquiti'; '788A20' = 'Ubiquiti'; '24A43C' = 'Ubiquiti'
    }
    $ouiDir = Join-Path $env:LOCALAPPDATA 'av-discover'
    $ouiCsv = Join-Path $ouiDir 'oui.csv'
    $stale = -not (Test-Path $ouiCsv) -or ((Get-Item $ouiCsv).LastWriteTime -lt (Get-Date).AddDays(-180))
    if ($stale) {
        try {
            New-Item -ItemType Directory -Force -Path $ouiDir | Out-Null
            Invoke-WebRequest -Uri 'https://standards-oui.ieee.org/oui/oui.csv' -OutFile "$ouiCsv.tmp" -UseBasicParsing -TimeoutSec 20 -UserAgent 'Mozilla/5.0'
            Move-Item -Force "$ouiCsv.tmp" $ouiCsv
            Write-Info "Lista IEEE actualizada"
        } catch { Write-Info "Sin internet para la lista IEEE; uso la caché o la tabla interna" }
    }
    if (Test-Path $ouiCsv) {
        foreach ($r in (Import-Csv $ouiCsv)) { if (-not $oui.ContainsKey($r.Assignment)) { $oui[$r.Assignment] = $r.'Organization Name' } }
    }
    function Get-Vendor([string]$mac) {
        if (-not $mac) { return '' }
        $h = $mac -replace '-', ''
        if ($oui.ContainsKey($h.Substring(0, 6))) { return $oui[$h.Substring(0, 6)] }
        if (([Convert]::ToInt32($h.Substring(0, 2), 16) -band 2) -ne 0) { return '(MAC local/aleatoria)' }
        '?'
    }

    $result = @($found.Values | Sort-Object { [version]$_.IP } | ForEach-Object {
        [pscustomobject]@{
            Subred     = if ($_.IP -like '169.254.*') { '169.254.0.0/16' } else { (Get-Prefix $_.IP) + '.0/24' }
            IP         = $_.IP
            MAC        = $_.MAC
            Fabricante = Get-Vendor $_.MAC
            Nombre     = $_.Nombre
            Visto      = ($_.Via -join ',')
        }
    })

    # ------------------------------------------------------------ informe
    Write-Step "RESULTADO"
    if ($switches.Count) {
        Write-Host "`n  Switch gestionable detectado:" -ForegroundColor Green
        $switches.Values | Format-Table Protocolo, Nombre, IP_gestion, Tu_puerto, Modelo, MAC -AutoSize | Out-Host
    }
    if ($result.Count -eq 0) {
        Write-Warn2 "No he encontrado ningún equipo. Prueba -Seconds 120 y -Defaults, otro puerto, o revisa VLANs."
    } else {
        foreach ($g in ($result | Group-Object Subred)) {
            Write-Host ("`n  {0}  ({1} equipos)" -f $g.Name, $g.Count) -ForegroundColor Green
            $g.Group | Format-Table IP, MAC, Fabricante, Nombre, Visto -AutoSize | Out-Host
        }
        $csv = Join-Path $OutDir "av-discover_$stamp.csv"
        $result | Export-Csv -Path $csv -NoTypeInformation -Encoding UTF8 -Delimiter ';'
        Write-Info "CSV: $csv"
    }
    Write-Info "Captura: $pcap  (ábrela en Wireshark si quieres más detalle)"

    if (-not $PassiveOnly -and -not $Keep -and $script:added.Count) {
        $ans = Read-Host "`n¿Mantener las IPs temporales para trabajar ahora? (s/N)"
        if ($ans -match '^[sSyY]') { $Keep = $true }
    }
}
finally {
    if (-not $PassiveOnly) {
        if ($Keep) {
            Write-Host "`nIPs mantenidas hasta reiniciar: $($script:added -join ', ')" -ForegroundColor Yellow
            if ($script:dhcpDisabled) { Write-Host "Para volver a DHCP:  Set-NetIPInterface -InterfaceAlias '$ifName' -Dhcp Enabled" -ForegroundColor Yellow }
        } else {
            Restore-Net
            Write-Host "`nTarjeta $ifName restaurada a su configuración original." -ForegroundColor DarkGray
        }
    }
    Remove-Item $errF -ErrorAction SilentlyContinue
}
