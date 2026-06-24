param(
    [Parameter(Mandatory=$true, Position=0)][string]$Target,
    [Parameter(Position=1)][string]$Gateway,
    [string]$Iface = "Ethernet"
)

# ---- Predefined profiles ----
$profiles = @{
    "0"    = @{ IP="192.168.0.99"; Mask="255.255.255.0"; GW="192.168.0.1" }
    "1"    = @{ IP="192.168.1.99"; Mask="255.255.255.0"; GW="192.168.1.1" }
    "dhcp" = @{ DHCP=$true }
}

# ---- Admin check ----
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { Write-Host "ERROR: run as Administrator." -ForegroundColor Red; exit 1 }

# ---- Helper: CIDR prefix to dotted mask ----
function Get-Mask([int]$prefix) {
    $octets = @(0,0,0,0)
    for ($i = 0; $i -lt 4; $i++) {
        $bits = [math]::Min(8, [math]::Max(0, $prefix - 8*$i))
        $octets[$i] = 256 - (1 -shl (8 - $bits))
    }
    return ($octets -join '.')
}

# ---- Helper: validate a dotted IPv4 address (4 octets, each 0-255) ----
function Test-IPv4([string]$Addr) {
    if ($Addr -notmatch '^\d{1,3}(\.\d{1,3}){3}$') { return $false }
    foreach ($o in ($Addr -split '\.')) { if ([int]$o -gt 255) { return $false } }
    return $true
}

# ---- Helper: run netsh as an arg array and verify it actually succeeded ----
# (array args keep interface names with spaces intact, e.g. "Ethernet 2")
function Invoke-NetSh {
    param([Parameter(Mandatory)][string[]]$NetShArgs)
    $out = & netsh @NetShArgs 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: netsh failed -> $($out -join ' ')" -ForegroundColor Red
        return $false
    }
    return $true
}

$key = $Target.ToLower()

# ============================================================
# MODE 1: predefined profile (0, 1, dhcp)
# ============================================================
if ($profiles.ContainsKey($key)) {
    $p = $profiles[$key]

    if ($p.DHCP) {
        $okAddr = Invoke-NetSh @('interface','ip','set','address',"name=$Iface",'source=dhcp')
        Invoke-NetSh @('interface','ip','set','dns',"name=$Iface",'source=dhcp') | Out-Null
        if (-not $okAddr) { exit 1 }
        Write-Host "OK -> DHCP (automatic)" -ForegroundColor Green

        $maxSec = 8
        $assigned = $null
        for ($i = 1; $i -le $maxSec; $i++) {
            Start-Sleep -Seconds 1
            Write-Progress -Activity "Waiting for DHCP server" -Status "$i / $maxSec s" -PercentComplete (($i / $maxSec) * 100)
            $assigned = Get-NetIPAddress -InterfaceAlias $iface -AddressFamily IPv4 -ErrorAction SilentlyContinue |
                        Where-Object { $_.PrefixOrigin -eq "Dhcp" } | Select-Object -First 1
            if ($assigned) { break }
        }
        Write-Progress -Activity "Waiting for DHCP server" -Completed
    }
    else {
        $okAddr = Invoke-NetSh @('interface','ip','set','address',"name=$Iface",'static',$p.IP,$p.Mask,$p.GW)
        Invoke-NetSh @('interface','ip','set','dns',"name=$Iface",'source=dhcp') | Out-Null
        if (-not $okAddr) { exit 1 }
        Write-Host "OK -> Profile $key : $($p.IP) / $($p.Mask)  GW $($p.GW)" -ForegroundColor Green
        Start-Sleep -Milliseconds 800
    }
}

# ============================================================
# MODE 2: custom IP/CIDR (with optional gateway)
# ============================================================
elseif ($Target -match '^(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})/(\d{1,2})$') {
    $ip   = $matches[1]
    $cidr = [int]$matches[2]

    if (-not (Test-IPv4 $ip)) {
        Write-Host "ERROR: invalid IP address '$ip' (octets must be 0-255)" -ForegroundColor Red
        exit 1
    }
    if ($cidr -lt 0 -or $cidr -gt 32) {
        Write-Host "ERROR: invalid CIDR prefix /$cidr (must be 0-32)" -ForegroundColor Red
        exit 1
    }

    $mask = Get-Mask $cidr

    if ($Gateway) {
        if (-not (Test-IPv4 $Gateway)) {
            Write-Host "ERROR: invalid gateway '$Gateway'" -ForegroundColor Red
            exit 1
        }
        $okAddr = Invoke-NetSh @('interface','ip','set','address',"name=$Iface",'static',$ip,$mask,$Gateway)
        if (-not $okAddr) { exit 1 }
        Write-Host "OK -> $ip/$cidr  ($mask)  GW $Gateway" -ForegroundColor Green
    }
    else {
        $okAddr = Invoke-NetSh @('interface','ip','set','address',"name=$Iface",'static',$ip,$mask)
        if (-not $okAddr) { exit 1 }
        Write-Host "OK -> $ip/$cidr  ($mask)  (no gateway)" -ForegroundColor Green
    }

    Invoke-NetSh @('interface','ip','set','dns',"name=$Iface",'source=dhcp') | Out-Null
    Start-Sleep -Milliseconds 800
}

# ============================================================
# MODE 3: invalid input
# ============================================================
else {
    Write-Host "Invalid input '$Target'." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Usage:" -ForegroundColor DarkGray
    Write-Host "  setip 0                              # profile 0"
    Write-Host "  setip 1                              # profile 1"
    Write-Host "  setip dhcp                           # automatic"
    Write-Host "  setip 192.168.1.25/24                # custom IP, no gateway"
    Write-Host "  setip 192.168.1.25/24 192.168.1.1    # custom IP + gateway"
    exit 1
}

# ---- Show resulting configuration ----
$ips = Get-NetIPAddress -InterfaceAlias $iface -AddressFamily IPv4 -ErrorAction SilentlyContinue
if ($ips) {
    $ips | Select-Object IPAddress, PrefixLength, PrefixOrigin | Format-Table -AutoSize
} else {
    Write-Host "(No IP yet, wait and run: netinfo)" -ForegroundColor DarkGray
}