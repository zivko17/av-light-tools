# ============================================================
#  AVCalc.ps1 - AV / IT planning calculators (offline)
#  Screen sizing (AVIXA viewing ratios), LED wall, subnet, bitrate/NDI.
#  These compute a design on paper - they do NOT touch the network or hardware.
#  Formulas are from public standards; implementation is original.
# ============================================================

# ---- shared helpers ----
function ConvertTo-IPInt([string]$ip) {
    $o = $ip -split '\.'
    return ((([int64]$o[0]) -shl 24) -bor (([int64]$o[1]) -shl 16) -bor (([int64]$o[2]) -shl 8) -bor ([int64]$o[3]))
}
function ConvertFrom-IPInt([int64]$n) {
    return "{0}.{1}.{2}.{3}" -f (($n -shr 24) -band 255), (($n -shr 16) -band 255), (($n -shr 8) -band 255), ($n -band 255)
}
function Test-IPv4Octets([string]$ip) {
    if ($ip -notmatch '^\d{1,3}(\.\d{1,3}){3}$') { return $false }
    foreach ($o in ($ip -split '\.')) { if ([int]$o -gt 255) { return $false } }
    return $true
}
function Get-GCD([int]$a, [int]$b) { $a=[math]::Abs($a); $b=[math]::Abs($b); while ($b -ne 0) { $t=$b; $b=$a % $b; $a=$t }; return $a }

# ---- calc-subnet: CIDR -> address structure + optional assignment table ----
function Get-Subnet {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true, Position=0)][string]$Cidr,
        [int]$Devices = 0,
        [int]$StartHost = 1
    )
    if ($Cidr -notmatch '^(\d{1,3}(?:\.\d{1,3}){3})/(\d{1,2})$') {
        Write-Host "`n  Usage: calc-subnet <ip>/<prefix>   e.g. calc-subnet 192.168.10.0/24" -ForegroundColor Yellow
        Write-Host "         table:  calc-subnet 192.168.10.0/24 -Devices 30 -StartHost 10`n" -ForegroundColor DarkGray
        return
    }
    $ipStr = $matches[1]; $prefix = [int]$matches[2]
    if (-not (Test-IPv4Octets $ipStr)) { Write-Host "  Invalid IP: $ipStr" -ForegroundColor Red; return }
    if ($prefix -lt 0 -or $prefix -gt 32) { Write-Host "  Invalid prefix /$prefix (0-32)" -ForegroundColor Red; return }

    $all = [int64]4294967295                    # 0xFFFFFFFF
    $ipInt = ConvertTo-IPInt $ipStr
    if ($prefix -eq 0) { $maskInt = [int64]0 } else { $maskInt = (($all -shl (32 - $prefix)) -band $all) }
    $network   = $ipInt -band $maskInt
    $wildcard  = $all - $maskInt
    $broadcast = $network -bor $wildcard
    $total     = [int64][math]::Pow(2, 32 - $prefix)

    Write-Host "`n=== SUBNET ===" -ForegroundColor Cyan
    Write-Host ("  CIDR:        {0}/{1}" -f (ConvertFrom-IPInt $network), $prefix) -ForegroundColor White
    Write-Host ("  Netmask:     {0}" -f (ConvertFrom-IPInt $maskInt))
    Write-Host ("  Wildcard:    {0}" -f (ConvertFrom-IPInt $wildcard)) -ForegroundColor DarkGray
    Write-Host ("  Network:     {0}" -f (ConvertFrom-IPInt $network))
    Write-Host ("  Broadcast:   {0}" -f (ConvertFrom-IPInt $broadcast))
    if ($prefix -ge 31) {
        $firstUsable = $network; $lastUsable = $broadcast
        Write-Host ("  Usable:      {0} - {1}  ({2} addresses)" -f (ConvertFrom-IPInt $firstUsable), (ConvertFrom-IPInt $lastUsable), $total) -ForegroundColor Green
    } else {
        $firstUsable = $network + 1; $lastUsable = $broadcast - 1
        Write-Host ("  Usable:      {0} - {1}" -f (ConvertFrom-IPInt $firstUsable), (ConvertFrom-IPInt $lastUsable)) -ForegroundColor Green
        Write-Host ("  Hosts:       {0}" -f ($total - 2)) -ForegroundColor Green
    }
    if ($Devices -gt 0) {
        Write-Host "`n  --- Assignment ($Devices devices from host $StartHost) ---" -ForegroundColor Yellow
        $startAddr = $network + $StartHost
        for ($i = 0; $i -lt $Devices; $i++) {
            $addr = $startAddr + $i
            if ($addr -gt $lastUsable) { Write-Host "    ... exceeds usable range at device $($i+1)" -ForegroundColor Red; break }
            Write-Host ("    Device {0,-4} {1}" -f ($i + 1), (ConvertFrom-IPInt $addr))
        }
    }
    Write-Host ""
}
Set-Alias -Name calc-subnet -Value Get-Subnet

# ---- calc-screen: screen size vs viewing distance (AVIXA 4/6/8 ratios) ----
function Get-ScreenSize {
    [CmdletBinding()]
    param(
        [Parameter(Position=0)][double]$Distance = 0,   # farthest viewer distance (forward mode)
        [double]$Diagonal = 0,                           # screen diagonal (reverse mode)
        [ValidateSet('detailed','basic','passive')][string]$Content = 'basic',
        [string]$Aspect = '16:9',
        [string]$Unit = 'm'
    )
    $ratio = switch ($Content) { 'detailed' { 4 } 'basic' { 6 } 'passive' { 8 } }
    $ap = $Aspect -split ':'
    if ($ap.Count -ne 2 -or [double]$ap[1] -eq 0) { Write-Host "  Invalid aspect '$Aspect' (use w:h, e.g. 16:9)" -ForegroundColor Red; return }
    $arWH = [double]$ap[0] / [double]$ap[1]              # width / height
    $diagFactor = [math]::Sqrt($arWH * $arWH + 1)        # diagonal = height * sqrt((w/h)^2 + 1)

    Write-Host ("`n=== SCREEN SIZE (AVIXA {0}:1 guideline, {1}) ===" -f $ratio, $Content) -ForegroundColor Cyan
    if ($Diagonal -gt 0) {
        $height = $Diagonal / $diagFactor
        Write-Host ("  Diagonal:         {0} {1}" -f $Diagonal, $Unit) -ForegroundColor White
        Write-Host ("  Image height:     {0:N2} {1}" -f $height, $Unit)
        Write-Host ("  Max view distance:{0:N2} {1}   (farthest viewer)" -f ($height * $ratio), $Unit) -ForegroundColor Green
    } elseif ($Distance -gt 0) {
        $height = $Distance / $ratio
        $width  = $height * $arWH
        $diag   = $height * $diagFactor
        $inch   = if ($Unit -eq 'm') { $diag * 39.37 } else { $diag * 12 }
        Write-Host ("  Farthest viewer:  {0} {1}" -f $Distance, $Unit) -ForegroundColor White
        Write-Host ("  Min image height: {0:N2} {1}" -f $height, $Unit) -ForegroundColor Green
        Write-Host ("  Min image width:  {0:N2} {1}" -f $width, $Unit)
        Write-Host ("  Min diagonal:     {0:N2} {1}  (~{2:N0} in)" -f $diag, $Unit, $inch) -ForegroundColor Green
    } else {
        Write-Host "  Usage: calc-screen <distance> [-Content detailed|basic|passive] [-Aspect 16:9]" -ForegroundColor Yellow
        Write-Host "         reverse: calc-screen -Diagonal 2.5 -Content basic`n" -ForegroundColor DarkGray
        return
    }
    Write-Host "  (Guideline only - verify with full AVIXA DISCAS for critical designs.)" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name calc-screen -Value Get-ScreenSize

# ---- calc-ledwall: rental LED wall - resolution, power, NovaStar signal, diagrams ----
#
# Cabinet database, three tiers. Geometry (size + pitch -> px) is exact; electrical /
# weight / brightness are TYPICAL rental values - always confirm on the datasheet.
# Fields: PxW PxH Wmm Hmm Env(indoor/outdoor) Nits MaxW Kg RecvPerCab [Brand Model].
#
# --- Tier 1: GENERIC standards (by environment / pitch / cabinet size) ---
# MaxWm2 = PEAK power density in W/m^2 (all-white); per-cabinet watts derive from area,
# so a 500x1000 cabinet draws double a 500x500 one. Average draw ~= 1/3 of peak (AvgFactor).
# Generic peaks are datasheet-representative medians: indoor ~550, outdoor ~700 W/m^2.
$Script:LEDGeneric = [ordered]@{
    'in-p1.9'      = @{ Brand='Generic'; Pitch='1.9'; PxW=256; PxH=256; Wmm=500; Hmm=500;  Env='indoor';  Nits=1000; MaxWm2=550; Kg=8.0;  RecvPerCab=1 }
    'in-p2.6'      = @{ Brand='Generic'; Pitch='2.6'; PxW=192; PxH=192; Wmm=500; Hmm=500;  Env='indoor';  Nits=1200; MaxWm2=550; Kg=7.5;  RecvPerCab=1 }
    'in-p2.9'      = @{ Brand='Generic'; Pitch='2.9'; PxW=168; PxH=168; Wmm=500; Hmm=500;  Env='indoor';  Nits=1500; MaxWm2=550; Kg=7.5;  RecvPerCab=1 }
    'in-p3.9'      = @{ Brand='Generic'; Pitch='3.9'; PxW=128; PxH=128; Wmm=500; Hmm=500;  Env='indoor';  Nits=1500; MaxWm2=550; Kg=7.0;  RecvPerCab=1 }
    'in-p4.8'      = @{ Brand='Generic'; Pitch='4.8'; PxW=104; PxH=104; Wmm=500; Hmm=500;  Env='indoor';  Nits=1500; MaxWm2=550; Kg=7.0;  RecvPerCab=1 }
    'out-p3.9'     = @{ Brand='Generic'; Pitch='3.9'; PxW=128; PxH=128; Wmm=500; Hmm=500;  Env='outdoor'; Nits=5000; MaxWm2=700; Kg=9.0;  RecvPerCab=1 }
    'out-p4.8'     = @{ Brand='Generic'; Pitch='4.8'; PxW=104; PxH=104; Wmm=500; Hmm=500;  Env='outdoor'; Nits=5500; MaxWm2=700; Kg=9.0;  RecvPerCab=1 }
    'in-p2.6-510'  = @{ Brand='Generic'; Pitch='2.6'; PxW=192; PxH=384; Wmm=500; Hmm=1000; Env='indoor';  Nits=1200; MaxWm2=550; Kg=15.0; RecvPerCab=2 }
    'in-p2.9-510'  = @{ Brand='Generic'; Pitch='2.9'; PxW=168; PxH=336; Wmm=500; Hmm=1000; Env='indoor';  Nits=1500; MaxWm2=550; Kg=15.0; RecvPerCab=2 }
    'in-p3.9-510'  = @{ Brand='Generic'; Pitch='3.9'; PxW=128; PxH=256; Wmm=500; Hmm=1000; Env='indoor';  Nits=1500; MaxWm2=550; Kg=14.0; RecvPerCab=2 }
    'out-p3.9-510' = @{ Brand='Generic'; Pitch='3.9'; PxW=128; PxH=256; Wmm=500; Hmm=1000; Env='outdoor'; Nits=5000; MaxWm2=700; Kg=18.0; RecvPerCab=2 }
    'out-p4.8-510' = @{ Brand='Generic'; Pitch='4.8'; PxW=104; PxH=208; Wmm=500; Hmm=1000; Env='outdoor'; Nits=5500; MaxWm2=700; Kg=18.0; RecvPerCab=2 }
}
# --- Tier 2: BRAND models used in rental. Values are from the manufacturers' own
# datasheets (Absen usabsen.com, Traulux/Charmex, Unilumin) - pitch/size/res/nits/weight
# and peak power density (MaxWm2, W/m^2 all-white). Average draw ~= 1/3 of peak (AvgFactor).
# Fields: B=brand K=key-prefix M=model Px=cab px-wide P=pitch label E=env T=tall(500x1000)
#         W=peak W/m^2  N=nits  Kg=cabinet weight.
function New-LEDEntry($Brand, $Model, $Px, $Pitch, $Env, $Tall, $W, $N, $Kg) {
    if ($Tall) { $hmm = 1000; $ph = $Px * 2; $recv = 2 } else { $hmm = 500; $ph = $Px; $recv = 1 }
    return @{ Brand=$Brand; Model=$Model; Pitch=$Pitch; Env=$Env; PxW=$Px; PxH=$ph; Wmm=500; Hmm=$hmm; Nits=$N; MaxWm2=$W; Kg=$Kg; RecvPerCab=$recv }
}
$Script:LEDBrandDefs = @(
    # Absen (NT Pro V2 / PL V2 / PR series - usabsen.com datasheets).  S = series slug.
    @{ B='Absen';    K='absen';    S='nt'; M='NT1.9 Pro V2';  Px=256; P='1.9'; E='indoor';  T=$false; W=645; N=1000; Kg=7.1 }
    @{ B='Absen';    K='absen';    S='nt'; M='NT2.6 Pro V2';  Px=192; P='2.6'; E='indoor';  T=$false; W=600; N=1000; Kg=6.8 }
    @{ B='Absen';    K='absen';    S='pl'; M='PL2.5 Pro V2';  Px=200; P='2.5'; E='indoor';  T=$false; W=680; N=1500; Kg=7.8 }
    @{ B='Absen';    K='absen';    S='pl'; M='PL2.9 Plus V2'; Px=168; P='2.9'; E='indoor';  T=$false; W=560; N=1000; Kg=7.8 }
    @{ B='Absen';    K='absen';    S='pl'; M='PL3.9 Plus V2'; Px=128; P='3.9'; E='indoor';  T=$false; W=560; N=1000; Kg=7.8 }
    @{ B='Absen';    K='absen';    S='pl'; M='PL2.9 Pro V2';  Px=168; P='2.9'; E='outdoor'; T=$false; W=610; N=3000; Kg=8.0 }
    @{ B='Absen';    K='absen';    S='pl'; M='PL3.9 Pro V2';  Px=128; P='3.9'; E='outdoor'; T=$false; W=800; N=4500; Kg=8.0 }
    @{ B='Absen';    K='absen';    S='pl'; M='PL3.9 Pro V2 500x1000'; Px=128; P='3.9'; E='outdoor'; T=$true; W=800; N=4500; Kg=13.0 }
    @{ B='Absen';    K='absen';    S='pl'; M='PL4.8 Pro V2';  Px=104; P='4.8'; E='outdoor'; T=$false; W=800; N=4500; Kg=8.0 }
    @{ B='Absen';    K='absen';    S='pr'; M='PR3.9 (IP65)';  Px=128; P='3.9'; E='outdoor'; T=$false; W=600; N=6000; Kg=9.5 }
    # Traulux (T-Rent LX / L - Charmex datasheets)
    @{ B='Traulux';  K='traulux';  S='lx'; M='T-Rent-LX 1.9'; Px=256; P='1.9'; E='indoor';  T=$false; W=580; N=1000; Kg=7.5 }
    @{ B='Traulux';  K='traulux';  S='l';  M='T-Rent-L 2.6';  Px=192; P='2.6'; E='indoor';  T=$false; W=490; N=800;  Kg=7.9 }
    @{ B='Traulux';  K='traulux';  S='lx'; M='T-Rent-LX 2.9'; Px=168; P='2.9'; E='outdoor'; T=$false; W=650; N=3800; Kg=7.3 }
    @{ B='Traulux';  K='traulux';  S='lx'; M='T-Rent-LX 3.9'; Px=128; P='3.9'; E='outdoor'; T=$false; W=630; N=4000; Kg=7.5 }
    @{ B='Traulux';  K='traulux';  S='blk';M='T-Rent 3.9 Black 500x1000'; Px=128; P='3.9'; E='outdoor'; T=$true; W=700; N=3500; Kg=12.2 }
    # Unilumin (UpadIV / URMIII / UGMII - datasheets)
    @{ B='Unilumin'; K='unilumin'; S='upad'; M='UpadIV 2.6';  Px=192; P='2.6'; E='indoor';  T=$false; W=480; N=1000; Kg=6.3 }
    @{ B='Unilumin'; K='unilumin'; S='urm';  M='URMIII 3.9';  Px=128; P='3.9'; E='indoor';  T=$false; W=496; N=800;  Kg=8.0 }
    @{ B='Unilumin'; K='unilumin'; S='ugm';  M='UGMII 2.6';   Px=192; P='2.6'; E='indoor';  T=$false; W=480; N=1000; Kg=6.0 }
    @{ B='Unilumin'; K='unilumin'; S='urm';  M='URMIII 03 3.9 500x1000'; Px=128; P='3.9'; E='outdoor'; T=$true; W=670; N=3000; Kg=13.9 }
)
$Script:LEDBrands = [ordered]@{}
foreach ($d in $Script:LEDBrandDefs) {
    $ec = 'in'; if ($d.E -eq 'outdoor') { $ec = 'out' }
    $suf = ''; if ($d.T) { $suf = '-510' }
    $Script:LEDBrands["$($d.K)-$($d.S)-$ec-$($d.P)$suf"] = New-LEDEntry $d.B $d.M $d.Px $d.P $d.E $d.T $d.W $d.N $d.Kg
}
# Varied-geometry brands (non-500 cabinets, datasheet power in W/cabinet). The helper
# converts W/cab -> W/m^2 using the real cabinet area so the power model stays uniform.
function New-LEDEntry2($Brand, $Model, $PxW, $PxH, $Wmm, $Hmm, $Pitch, $Env, $Wcab, $N, $Kg, $Recv) {
    $area = $Wmm * $Hmm / 1e6
    $wm2 = [int][math]::Round($Wcab / $area)
    return @{ Brand=$Brand; Model=$Model; Pitch=$Pitch; Env=$Env; PxW=$PxW; PxH=$PxH; Wmm=$Wmm; Hmm=$Hmm; Nits=$N; MaxWm2=$wm2; Kg=$Kg; RecvPerCab=$Recv }
}
$Script:LEDBrandDefs2 = @(
    # ROE Visual (datasheet power is per-cabinet; many cabinets are NOT 500x500)
    @{ B='ROE Visual'; K='roe'; S='bp'; M='Black Pearl BP2 V2'; PxW=176; PxH=176; Wmm=500;   Hmm=500;  P='2.8';  E='indoor';  Wcab=160; N=1500; Kg=9.35; R=1 }
    @{ B='ROE Visual'; K='roe'; S='dm'; M='Diamond DM2.6';      PxW=192; PxH=192; Wmm=500;   Hmm=500;  P='2.6';  E='indoor';  Wcab=180; N=1500; Kg=5.76; R=1 }
    @{ B='ROE Visual'; K='roe'; S='dm'; M='Diamond DM3.9';      PxW=128; PxH=128; Wmm=500;   Hmm=500;  P='3.9';  E='indoor';  Wcab=150; N=1500; Kg=5.76; R=1 }
    @{ B='ROE Visual'; K='roe'; S='rb'; M='Ruby RB1.9B V2';     PxW=256; PxH=256; Wmm=500;   Hmm=500;  P='1.9';  E='indoor';  Wcab=160; N=1500; Kg=9.2;  R=1 }
    @{ B='ROE Visual'; K='roe'; S='rb'; M='Ruby RB2.6';         PxW=192; PxH=192; Wmm=500;   Hmm=500;  P='2.6';  E='indoor';  Wcab=160; N=1500; Kg=8.5;  R=1 }
    @{ B='ROE Visual'; K='roe'; S='cb'; M='Carbon CB3 MKII';    PxW=160; PxH=320; Wmm=600;   Hmm=1200; P='3.75'; E='outdoor'; Wcab=600; N=4500; Kg=14;   R=2 }
    @{ B='ROE Visual'; K='roe'; S='cb'; M='Carbon CB5 MKII';    PxW=104; PxH=208; Wmm=600;   Hmm=1200; P='5.77'; E='outdoor'; Wcab=480; N=6000; Kg=12.2; R=2 }
    @{ B='ROE Visual'; K='roe'; S='cb'; M='Carbon CB8 MKII';    PxW=72;  PxH=144; Wmm=600;   Hmm=1200; P='8.33'; E='outdoor'; Wcab=530; N=6000; Kg=11;   R=2 }
    @{ B='ROE Visual'; K='roe'; S='bm'; M='Black Marble BM4';   PxW=128; PxH=128; Wmm=609.6; Hmm=609.6;P='4.8';  E='outdoor'; Wcab=290; N=2000; Kg=17.5; R=1 }
    @{ B='ROE Visual'; K='roe'; S='vn'; M='Vanish V5S MKII';    PxW=176; PxH=176; Wmm=1000;  Hmm=1000; P='5.6';  E='indoor';  Wcab=420; N=2000; Kg=10.9; R=1 }
    @{ B='ROE Visual'; K='roe'; S='vn'; M='Vanish V8S MKII';    PxW=112; PxH=112; Wmm=1000;  Hmm=1000; P='8.9';  E='indoor';  Wcab=460; N=3500; Kg=9.6;  R=1 }
    # INFiLED (AR 500x1000 rental, DB 500x500 rental, X 500x500 - infiled.com datasheets, W/cab)
    @{ B='INFiLED'; K='infiled'; S='ar'; M='AR IL-RSS-IRAR2.97'; PxW=168; PxH=336; Wmm=500; Hmm=1000; P='2.9'; E='indoor';  Wcab=270; N=1200; Kg=14.0; R=2 }
    @{ B='INFiLED'; K='infiled'; S='ar'; M='AR IL-RSS-IRAR3.91'; PxW=128; PxH=256; Wmm=500; Hmm=1000; P='3.9'; E='indoor';  Wcab=270; N=1200; Kg=12.5; R=2 }
    @{ B='INFiLED'; K='infiled'; S='ar'; M='AR IL-RSS-IRAR4.81'; PxW=104; PxH=208; Wmm=500; Hmm=1000; P='4.8'; E='indoor';  Wcab=270; N=1200; Kg=12.3; R=2 }
    @{ B='INFiLED'; K='infiled'; S='ar'; M='AR IL-RSS-ORAR3.91'; PxW=128; PxH=256; Wmm=500; Hmm=1000; P='3.9'; E='outdoor'; Wcab=270; N=5000; Kg=12.5; R=2 }
    @{ B='INFiLED'; K='infiled'; S='ar'; M='AR IL-RSS-ORAR4.63'; PxW=108; PxH=216; Wmm=500; Hmm=1000; P='4.6'; E='outdoor'; Wcab=270; N=5000; Kg=12.7; R=2 }
    @{ B='INFiLED'; K='infiled'; S='db'; M='DB IL-RSS-IRDB1.9';  PxW=256; PxH=256; Wmm=500; Hmm=500;  P='1.9'; E='indoor';  Wcab=150; N=800;  Kg=8.8;  R=1 }
    @{ B='INFiLED'; K='infiled'; S='db'; M='DB IL-RSS-IRDB2.6';  PxW=192; PxH=192; Wmm=500; Hmm=500;  P='2.6'; E='indoor';  Wcab=150; N=1200; Kg=8.8;  R=1 }
    @{ B='INFiLED'; K='infiled'; S='db'; M='DB IL-RSS-IRDB3.4';  PxW=144; PxH=144; Wmm=500; Hmm=500;  P='3.4'; E='indoor';  Wcab=150; N=1500; Kg=8.8;  R=1 }
    @{ B='INFiLED'; K='infiled'; S='x';  M='X-Series 2.9 Flex';  PxW=168; PxH=168; Wmm=500; Hmm=500;  P='2.9'; E='indoor';  Wcab=180; N=1200; Kg=8.9;  R=1 }
)
foreach ($d in $Script:LEDBrandDefs2) {
    $ec = 'in'; if ($d.E -eq 'outdoor') { $ec = 'out' }
    $Script:LEDBrands["$($d.K)-$($d.S)-$ec-$($d.P)"] = New-LEDEntry2 $d.B $d.M $d.PxW $d.PxH $d.Wmm $d.Hmm $d.P $d.E $d.Wcab $d.N $d.Kg $d.R
}
# --- Tier 3: YOUR cabinets - EDIT THIS BLOCK with your own panels ---
# Copy a line above, rename the key, and set your real datasheet values.
# MaxWm2 = peak W/m^2 (all-white); or use MaxW=<watts> for explicit per-cabinet peak.
$Script:LEDUser = [ordered]@{
    # 'mywall' = @{ PxW=168; PxH=168; Wmm=500; Hmm=500; Env='indoor'; Nits=1500; MaxWm2=560; Kg=7.8; RecvPerCab=1; Brand='...'; Model='...' }
}
# Merged lookup (user entries override brand/generic on key clash).
$Script:LEDCabinets = [ordered]@{}
foreach ($grp in @($Script:LEDGeneric, $Script:LEDBrands, $Script:LEDUser)) {
    foreach ($k in $grp.Keys) { $Script:LEDCabinets[$k] = $grp[$k] }
}

# NovaStar receiving cards. Coex=$true marks Armor-series cards (A10s Pro and newer)
# required by the COEX/MX platform; legacy A5s/A8s are NOT COEX-compatible.
$Script:NovaRecvCards = @(
    @{ Name='A5s Plus'; PxCap=512000;  Coex=$false }
    @{ Name='A8s';      PxCap=1024000; Coex=$false }
    @{ Name='A8s Plus'; PxCap=1300000; Coex=$false }
    @{ Name='A10s Pro'; PxCap=1300000; Coex=$true }
    @{ Name='CA50E';    PxCap=2600000; Coex=$true }
)
# NovaStar senders/processors by series, with Gigabit-port count and max loading (MP),
# from official datasheets. Listed smallest-first within each series. VX2000 Pro is the
# top of the VX line (20 ports / 13 MP); above it -> H-series splicing processors.
# COEX/MX controllers require A10s Pro (Armor) receiving cards.
$Script:NovaSenders = @(
    @{ Name='MCTRL300';     Series='MCTRL'; Ports=2;   MaxMP=1.3 }
    @{ Name='MCTRL660';     Series='MCTRL'; Ports=4;   MaxMP=2.6 }
    @{ Name='MCTRL660 Pro'; Series='MCTRL'; Ports=6;   MaxMP=3.9 }
    @{ Name='MCTRL R5';     Series='MCTRL'; Ports=8;   MaxMP=4.1 }
    @{ Name='MCTRL4K';      Series='MCTRL'; Ports=16;  MaxMP=8.8 }
    @{ Name='VX400 Pro';    Series='VX';    Ports=4;   MaxMP=2.6 }
    @{ Name='VX600 Pro';    Series='VX';    Ports=6;   MaxMP=3.9 }
    @{ Name='VX1000 Pro';   Series='VX';    Ports=10;  MaxMP=6.5 }
    @{ Name='VX2000 Pro';   Series='VX';    Ports=20;  MaxMP=13 }
    @{ Name='MX40 Pro';     Series='COEX';  Ports=20;  MaxMP=9 }
    @{ Name='MX2000 Pro';   Series='COEX';  Ports=80;  MaxMP=35.4 }
    @{ Name='H2';           Series='H';     Ports=40;  MaxMP=26 }
    @{ Name='H5';           Series='H';     Ports=60;  MaxMP=39 }
    @{ Name='H9';           Series='H';     Ports=200; MaxMP=65 }
    @{ Name='H15';          Series='H';     Ports=320; MaxMP=208 }
)
# Smallest sender in a series that has enough ports AND loading (MP) for the wall.
function Find-NovaSender($Series, $NeedPorts, $TotalPx) {
    foreach ($k in $Script:NovaSenders) {
        if ($k.Series -ne $Series) { continue }
        if ($k.Ports -ge $NeedPorts -and ($k.MaxMP * 1e6) -ge $TotalPx) { return $k }
    }
    return $null
}

# Assign cabinets to groups (circuits / ports) in serpentine order, chunked by
# capacity. Distribution 'vertical' snakes column-by-column, 'horizontal' row-by-row.
# Returns a Rows x Cols jagged array of group ids (1-based).
function Get-LEDGroupMap([int]$Cols, [int]$Rows, [int]$ChunkCap, [string]$Distribution = 'vertical') {
    if ($ChunkCap -lt 1) { $ChunkCap = 1 }
    $map = New-Object 'object[]' $Rows
    for ($r = 0; $r -lt $Rows; $r++) { $map[$r] = New-Object 'int[]' $Cols }
    $idx = 0
    if ($Distribution -eq 'horizontal') {
        for ($r = 0; $r -lt $Rows; $r++) {
            if (($r % 2) -eq 0) { $cSeq = 0..($Cols - 1) } else { $cSeq = ($Cols - 1)..0 }
            foreach ($c in $cSeq) {
                $map[$r][$c] = [int]([math]::Floor($idx / $ChunkCap)) + 1
                $idx++
            }
        }
    } else {
        for ($c = 0; $c -lt $Cols; $c++) {
            if (($c % 2) -eq 0) { $rSeq = 0..($Rows - 1) } else { $rSeq = ($Rows - 1)..0 }
            foreach ($r in $rSeq) {
                $map[$r][$c] = [int]([math]::Floor($idx / $ChunkCap)) + 1
                $idx++
            }
        }
    }
    return $map
}

# Draw the cabinet grid as coloured blocks (background per group id) framed with
# box-drawing borders, or summarise with coloured chips if too large to draw.
function Write-LEDMap([string]$Title, [string]$Prefix, [int]$Cols, [int]$Rows, $Map, [int]$Groups, [string]$Note) {
    $bg = @('DarkCyan','DarkGreen','DarkYellow','DarkMagenta','DarkBlue','DarkRed','Blue','DarkGray')
    $TL=[char]0x250C; $TR=[char]0x2510; $BL=[char]0x2514; $BR=[char]0x2518
    $HZ=[char]0x2500; $VT=[char]0x2502; $TT=[char]0x252C; $BT=[char]0x2534
    Write-Host ("  --- {0} ---" -f $Title) -ForegroundColor Yellow
    if ($Cols -le 20 -and $Rows -le 18) {
        $seg = ([string]$HZ) * 4                      # 4-wide cells (" P12")
        Write-Host ("  " + $TL + ((($seg + $TT) * ($Cols - 1)) + $seg) + $TR) -ForegroundColor DarkGray
        for ($r = 0; $r -lt $Rows; $r++) {
            Write-Host ("  " + $VT) -NoNewline -ForegroundColor DarkGray
            for ($c = 0; $c -lt $Cols; $c++) {
                $id = $Map[$r][$c]
                Write-Host (" {0}{1,2}" -f $Prefix, $id) -NoNewline -ForegroundColor White -BackgroundColor $bg[($id - 1) % $bg.Count]
                Write-Host $VT -NoNewline -ForegroundColor DarkGray
            }
            Write-Host ""
        }
        Write-Host ("  " + $BL + ((($seg + $BT) * ($Cols - 1)) + $seg) + $BR) -ForegroundColor DarkGray
    } else {
        # too large to draw - coloured chip + cabinet count per group
        $counts = New-Object 'int[]' ($Groups + 1)
        for ($r = 0; $r -lt $Rows; $r++) { for ($c = 0; $c -lt $Cols; $c++) { $counts[$Map[$r][$c]]++ } }
        for ($gid = 1; $gid -le $Groups; $gid++) {
            Write-Host "    " -NoNewline
            Write-Host (" {0}{1,2} " -f $Prefix, $gid) -NoNewline -ForegroundColor White -BackgroundColor $bg[($gid - 1) % $bg.Count]
            Write-Host ("  {0} cabinets" -f $counts[$gid])
        }
    }
    if ($Note) { Write-Host ("  {0}" -f $Note) -ForegroundColor DarkGray }
}

function Get-LEDWall {
    [CmdletBinding()]
    param(
        [Parameter(Position=0)][int]$Cols = 0,
        [Parameter(Position=1)][int]$Rows = 0,
        [string]$Cabinet = '',
        [int]$CabPxW = 0,
        [int]$CabPxH = 0,
        [double]$CabWmm = 0,
        [double]$CabHmm = 0,
        [string]$Env = '',
        [double]$MaxWatt = 0,
        [double]$MaxWm2 = 0,
        [double]$AvgFactor = 0.33,
        [double]$WeightKg = 0,
        [double]$Nits = 0,
        [int]$RecvPerCab = 0,
        [string]$RecvCard = '',
        [string]$Sender = '',
        [double]$Voltage = 230,
        [double]$BreakerA = 16,
        [int]$MaxChain = 10,
        [ValidateSet('avg','max')][string]$PowerBasis = 'avg',
        [double]$PxPerPort = 650000,
        [ValidateSet('vertical','horizontal')][string]$Distribution = 'vertical',
        [switch]$Redundant,
        [switch]$NoDiagram,
        [switch]$List
    )
    if ($List) { Show-LEDCabinets; return }
    if ($Cols -lt 1 -or $Rows -lt 1) {
        if (-not $Cabinet -and $CabPxW -le 0) { Invoke-LEDWizard; return }   # no args -> guided wizard
        Write-Host "`n  Usage: calc-ledwall <cols> <rows> -Cabinet <preset> [-Redundant] [-Distribution horizontal]" -ForegroundColor Yellow
        Write-Host "         manual:  calc-ledwall 10 5 -CabPxW 168 -CabPxH 168 -CabWmm 500 -CabHmm 500 -MaxWatt 190" -ForegroundColor DarkGray
        Write-Host "         list:    calc-ledwall -List    |    wizard: calc-ledwall (no args)`n" -ForegroundColor DarkGray
        return
    }

    $brand = ''; $model = ''; $presetW = 0; $presetWm2 = 0
    if ($Cabinet) {
        $key = $Cabinet.ToLower()
        if (-not $Script:LEDCabinets.Contains($key)) {
            Write-Host ("  Unknown cabinet '{0}'. Run  calc-ledwall -List  to see presets." -f $Cabinet) -ForegroundColor Red
            return
        }
        $c = $Script:LEDCabinets[$key]
        if ($CabPxW     -le 0) { $CabPxW     = $c.PxW }
        if ($CabPxH     -le 0) { $CabPxH     = $c.PxH }
        if ($CabWmm     -le 0) { $CabWmm     = $c.Wmm }
        if ($CabHmm     -le 0) { $CabHmm     = $c.Hmm }
        if ($WeightKg   -le 0) { $WeightKg   = $c.Kg }
        if ($Nits       -le 0) { $Nits       = $c.Nits }
        if ($RecvPerCab -le 0) { $RecvPerCab = $c.RecvPerCab }
        if (-not $Env)         { $Env        = [string]$c.Env }
        if ($c.Contains('MaxW'))   { $presetW   = [double]$c.MaxW }    # explicit per-cabinet watts
        if ($c.Contains('MaxWm2')) { $presetWm2 = [double]$c.MaxWm2 }  # power density W/m^2
        if ($c.Contains('Brand')) { $brand = [string]$c.Brand; $model = [string]$c.Model }
    }
    if ($RecvPerCab -le 0) { $RecvPerCab = 1 }
    if ($CabPxW -le 0 -or $CabPxH -le 0) {
        Write-Host "  Need per-cabinet resolution. Use -Cabinet <preset>, or -CabPxW/-CabPxH." -ForegroundColor Yellow
        Write-Host "  Run  calc-ledwall -List  to see presets.`n" -ForegroundColor DarkGray
        return
    }

    $totalCab = $Cols * $Rows
    $resW = $Cols * $CabPxW
    $resH = $Rows * $CabPxH
    $totalPx = [double]$resW * $resH
    $cabPx = [double]$CabPxW * $CabPxH
    $cabArea = ($CabWmm * $CabHmm) / 1e6                              # m^2 (0 if mm unknown)
    # Effective per-cabinet peak watts. Precedence: -MaxWatt, then -MaxWm2 x area,
    # then preset per-cabinet watts, then preset density x area.
    if ($MaxWatt -le 0) {
        if ($MaxWm2 -gt 0 -and $cabArea -gt 0)      { $MaxWatt = $MaxWm2 * $cabArea }
        elseif ($presetW -gt 0)                     { $MaxWatt = $presetW }
        elseif ($presetWm2 -gt 0 -and $cabArea -gt 0) { $MaxWatt = $presetWm2 * $cabArea }
    }
    $effWm2 = 0; if ($cabArea -gt 0) { $effWm2 = $MaxWatt / $cabArea }
    $g = Get-GCD $resW $resH

    $hdr = "{0} x {1} = {2} cabinets" -f $Cols, $Rows, $totalCab
    if ($model)         { $hdr = "{0}  |  {1} {2}" -f $hdr, $brand, $model }
    elseif ($Cabinet)   { $hdr = "{0}  |  {1}" -f $hdr, $Cabinet }
    Write-Host ("`n=== LED WALL ({0}) ===" -f $hdr) -ForegroundColor Cyan

    # --- resolution ---
    Write-Host "  --- Resolution ---" -ForegroundColor Yellow
    Write-Host ("  Real resolution:  {0} x {1} px   ({2:N2} MP)" -f $resW, $resH, ($totalPx / 1e6)) -ForegroundColor Green
    if ($g -gt 0) { Write-Host ("  Aspect ratio:     {0}:{1}" -f ($resW / $g), ($resH / $g)) -ForegroundColor White }

    # --- physical ---
    if ($CabWmm -gt 0 -and $CabHmm -gt 0) {
        $physW = $Cols * $CabWmm; $physH = $Rows * $CabHmm
        $pitch = $CabWmm / $CabPxW
        Write-Host "  --- Physical ---" -ForegroundColor Yellow
        Write-Host ("  Size:             {0:N2} x {1:N2} m   ({2:N2} m^2)" -f ($physW / 1000), ($physH / 1000), ($physW * $physH / 1e6)) -ForegroundColor Green
        Write-Host ("  Pixel pitch:      {0:N3} mm" -f $pitch) -ForegroundColor White
        Write-Host ("  Min view dist:    {0:N2} m   (retina, ~pitch x 3.4)" -f ($pitch * 3.438)) -ForegroundColor DarkGray
        if ($Nits -gt 0) {
            $envStr = if ($Env) { "  ($Env)" } else { "" }
            Write-Host ("  Brightness:       {0:N0} nits{1}" -f $Nits, $envStr) -ForegroundColor White
        }
    }

    # --- power ---
    # MaxWatt is the PEAK per-cabinet watts (all-white). Sizing basis: 'avg' (~1/3 of peak,
    # real content) or 'max' (worst-case white). Two limits are kept separate:
    #   * circuit  = current limit of one BreakerA breaker  (how many breakers)
    #   * chain    = physical powerCON daisy-chain limit (MaxChain)  (how many cables)
    # One breaker can feed several daisy-chains, so cables >= circuits.
    $circuits = 0; $chainCap = 0; $cables = 0
    if ($MaxWatt -gt 0) {
        $maxKW = $totalCab * $MaxWatt / 1000.0
        $avgKW = $maxKW * $AvgFactor
        $basisW = $MaxWatt; if ($PowerBasis -eq 'avg') { $basisW = $MaxWatt * $AvgFactor }   # per-cabinet
        $basisKW = $totalCab * $basisW / 1000.0
        $maxA  = $totalCab * $MaxWatt / $Voltage
        $usableW = $BreakerA * 0.8 * $Voltage
        $elecChain = [int][math]::Floor($usableW / $basisW)      # cabinets one breaker can power
        if ($elecChain -lt 1) { $elecChain = 1 }
        $chainCap = $elecChain; $capNote = ""
        if ($MaxChain -gt 0 -and $MaxChain -lt $chainCap) { $chainCap = $MaxChain; $capNote = " (powerCON cap)" }   # daisy-chain limit
        $circuits = [int][math]::Ceiling($totalCab / [double]$elecChain)
        $cables   = [int][math]::Ceiling($totalCab / [double]$chainCap)
        # incoming supply / distro board: smallest standard CEE feed that covers the basis load
        # (16 A / 32 A single-phase, 63 A / 125 A three-phase), 80% derated.
        $distros = @(
            @{ Name='16 A 1-phase';  kW = ($Voltage * 16 / 1000.0);        Ph=1 }
            @{ Name='32 A 1-phase';  kW = ($Voltage * 32 / 1000.0);        Ph=1 }
            @{ Name='63 A 3-phase';  kW = ($Voltage * 63 * 3 / 1000.0);    Ph=3 }
            @{ Name='125 A 3-phase'; kW = ($Voltage * 125 * 3 / 1000.0);   Ph=3 }
        )
        $sup = $null
        foreach ($d in $distros) { if (($d.kW * 0.8) -ge $basisKW) { $sup = $d; break } }
        Write-Host "  --- Power ---" -ForegroundColor Yellow
        if ($effWm2 -gt 0) { Write-Host ("  Density:          {0:N0} W/m^2 peak / {1:N0} W/m^2 avg   ({2:N0} W/cab peak)" -f $effWm2, ($effWm2 * $AvgFactor), $MaxWatt) -ForegroundColor White }
        Write-Host ("  Peak draw:        {0:N2} kW   ({1:N1} A @ {2:N0} V, all-white)" -f $maxKW, $maxA, $Voltage) -ForegroundColor White
        Write-Host ("  Avg draw:         {0:N2} kW   (~{1:P0} of peak, real content)" -f $avgKW, $AvgFactor) -ForegroundColor Green
        Write-Host ("  Sizing basis:     {0} draw = {1:N2} kW" -f $PowerBasis, $basisKW) -ForegroundColor Yellow
        Write-Host ("  Circuits:         {0} x {1:N0} A   (up to {2} cabinets/breaker, {3:N2} kW usable)" -f $circuits, $BreakerA, $elecChain, ($usableW / 1000)) -ForegroundColor Green
        Write-Host ("  Power cables:     {0}   (max {1} cabinets/daisy-chain{2})" -f $cables, $chainCap, $capNote) -ForegroundColor Green
        if ($sup) {
            $phStr = ""
            if ($sup.Ph -eq 3) { $phStr = ("  ({0:N1} A/phase" -f ($basisKW * 1000 / (3 * $Voltage))) + ")" }
            Write-Host ("  Supply / distro:  {0}   (~{1:N1} kW usable){2}" -f $sup.Name, ($sup.kW * 0.8), $phStr) -ForegroundColor Green
        } else {
            $big = $distros[$distros.Count - 1]
            $n = [int][math]::Ceiling($basisKW / ($big.kW * 0.8))
            Write-Host ("  Supply / distro:  {0} x {1}   ({2} load {3:N1} kW exceeds one feed)" -f $n, $big.Name, $PowerBasis, $basisKW) -ForegroundColor Green
        }
    }

    # --- signal (NovaStar) ---
    $cabPerPort = [int][math]::Floor($PxPerPort / $cabPx)
    if ($cabPerPort -lt 1) { $cabPerPort = 1 }
    $ports = [int][math]::Ceiling($totalCab / [double]$cabPerPort)
    $rc = $totalCab * $RecvPerCab
    Write-Host "  --- Signal (NovaStar) ---" -ForegroundColor Yellow
    Write-Host ("  Data ports:       {0}   ({1:N0} px/port @ 8-bit/60Hz, ~{2} cab/port)" -f $ports, $PxPerPort, $cabPerPort) -ForegroundColor Green
    if ($Redundant) {
        Write-Host ("  Cat cables:       {0}   (redundant: {1} main + {1} backup)" -f (2 * $ports), $ports) -ForegroundColor Green
    } else {
        Write-Host ("  Cat cables:       {0}   (single path; x2 = {1} for redundancy)" -f $ports, (2 * $ports)) -ForegroundColor Green
    }
    # receiving cards (track whether they are COEX-capable, i.e. A10s Pro / Armor)
    $coexCard = $false; $recvName = ''
    $recvLine = "  Receiving cards:  {0}   ({1}/cabinet)" -f $rc, $RecvPerCab
    if ($RecvCard) {
        $rcInfo = $null
        foreach ($k in $Script:NovaRecvCards) { if ($k.Name.ToLower() -eq $RecvCard.ToLower()) { $rcInfo = $k; break } }
        if ($rcInfo) {
            $coexCard = [bool]$rcInfo.Coex; $recvName = $rcInfo.Name
            $tag = if ($coexCard) { ' [COEX-capable]' } else { '' }
            Write-Host ("  Receiving cards:  {0} x {1}{2}   ({3}/cabinet)" -f $rc, $rcInfo.Name, $tag, $RecvPerCab) -ForegroundColor White
            if ($cabPx -gt $rcInfo.PxCap) {
                Write-Host ("                    ! cabinet {0:N0} px > {1} cap ~{2:N0} px - needs >1 card/cabinet" -f $cabPx, $rcInfo.Name, $rcInfo.PxCap) -ForegroundColor Red
            }
        } else {
            Write-Host $recvLine -ForegroundColor White
            Write-Host ("                    (unknown card '{0}'; known: {1})" -f $RecvCard, (($Script:NovaRecvCards | ForEach-Object { $_.Name }) -join ', ')) -ForegroundColor DarkGray
        }
    } else {
        Write-Host $recvLine -ForegroundColor White
    }
    # sender: with redundancy one unit must carry BOTH loops, so it needs 2x the ports.
    $needPorts = $ports
    if ($Redundant) { $needPorts = 2 * $ports }
    $needStr = ""; if ($Redundant) { $needStr = " (redundant: 2x = $needPorts ports)" }
    if ($Sender) {
        $snd = $null
        foreach ($k in $Script:NovaSenders) { if ($k.Name.ToLower() -eq $Sender.ToLower()) { $snd = $k; break } }
        if ($snd) {
            $portsOK = $snd.Ports -ge $needPorts
            $mpOK = ($snd.MaxMP * 1e6) -ge $totalPx
            if ($portsOK -and $mpOK) {
                Write-Host ("  Sender:           {0} OK   ({1} ports / {2} MP; need {3} ports / {4:N2} MP){5}" -f $snd.Name, $snd.Ports, $snd.MaxMP, $needPorts, ($totalPx / 1e6), $needStr) -ForegroundColor Green
            } else {
                $uP = [int][math]::Ceiling($needPorts / [double]$snd.Ports)
                $uM = [int][math]::Ceiling($totalPx / ($snd.MaxMP * 1e6))
                $units = [math]::Max($uP, $uM)
                $lim = if (-not $mpOK) { "over {0} MP" -f $snd.MaxMP } else { "need {0} ports, has {1}" -f $needPorts, $snd.Ports }
                Write-Host ("  Sender:           {0} short ({1}) - ~{2} units{3}" -f $snd.Name, $lim, $units, $needStr) -ForegroundColor Red
            }
            if ($snd.Series -eq 'COEX' -and $RecvCard -and -not $coexCard) {
                Write-Host ("                    ! {0} (COEX) needs A10s Pro / Armor cards; '{1}' is not compatible" -f $snd.Name, $recvName) -ForegroundColor Red
            } elseif ($snd.Series -eq 'COEX') {
                Write-Host "                    (COEX/MX requires A10s Pro / Armor receiving cards)" -ForegroundColor DarkGray
            }
        } else {
            Write-Host ("  Sender:           unknown '{0}'; known: {1}" -f $Sender, (($Script:NovaSenders | ForEach-Object { $_.Name }) -join ', ')) -ForegroundColor DarkGray
        }
    } else {
        Write-Host ("  Sender needs:     {0} ports / {1:N2} MP{2}" -f $needPorts, ($totalPx / 1e6), $needStr) -ForegroundColor White
        $mctrl = Find-NovaSender 'MCTRL' $needPorts $totalPx
        $vx    = Find-NovaSender 'VX'    $needPorts $totalPx
        if ($mctrl) { Write-Host ("    MCTRL series:   {0}  ({1} ports, {2} MP)" -f $mctrl.Name, $mctrl.Ports, $mctrl.MaxMP) -ForegroundColor Green }
        else        { Write-Host  "    MCTRL series:   exceeds series (max MCTRL4K: 16 ports / 8.8 MP)" -ForegroundColor DarkGray }
        if ($vx) { Write-Host ("    VX series:      {0}  ({1} ports, {2} MP)" -f $vx.Name, $vx.Ports, $vx.MaxMP) -ForegroundColor Green }
        else {
            $h = Find-NovaSender 'H' $needPorts $totalPx
            if ($h) { Write-Host ("    VX -> H series: over VX2000 Pro (13 MP) - use {0} ({1} MP)" -f $h.Name, $h.MaxMP) -ForegroundColor Green }
            else    { Write-Host  "    VX -> H series: over a single H15 - multiple H-series units" -ForegroundColor Green }
        }
        if ($coexCard) {
            $coex = Find-NovaSender 'COEX' $needPorts $totalPx
            if ($coex) { Write-Host ("    COEX series:    {0}  ({1} ports, {2} MP)  [{3} is COEX-capable]" -f $coex.Name, $coex.Ports, $coex.MaxMP, $recvName) -ForegroundColor Green }
        } else {
            Write-Host "    COEX series:    possible if receiving cards are A10s Pro / Armor (-RecvCard 'A10s Pro')" -ForegroundColor DarkGray
        }
    }

    # --- weight ---
    if ($WeightKg -gt 0) {
        Write-Host "  --- Weight ---" -ForegroundColor Yellow
        Write-Host ("  Total:            {0:N0} kg" -f ($totalCab * $WeightKg)) -ForegroundColor Green
        Write-Host ("  Per hanging col:  {0:N0} kg   ({1} cabinets high)" -f ($Rows * $WeightKg), $Rows) -ForegroundColor White
    }

    # --- diagrams ---
    if (-not $NoDiagram) {
        Write-Host ""
        $sigNote = "cell colour = data port (P#); {0} serpentine wiring" -f $Distribution
        if ($Redundant) { $sigNote = "$sigNote; x2 cables for backup loop" }
        Write-LEDMap ("Signal map (NovaStar ports, {0})" -f $Distribution) "P" $Cols $Rows (Get-LEDGroupMap $Cols $Rows $cabPerPort $Distribution) $ports $sigNote
        if ($cables -gt 0) {
            Write-Host ""
            Write-LEDMap ("Power map (daisy-chains, {0})" -f $Distribution) "C" $Cols $Rows (Get-LEDGroupMap $Cols $Rows $chainCap $Distribution) $cables ("cell colour = power cable / daisy-chain (C#), max {0} cabinets" -f $chainCap)
        }
    }

    Write-Host "  (Brand presets = datasheet values; generic = typical medians. Verify your exact panel/revision.)" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name calc-ledwall -Value Get-LEDWall

# ---- list the cabinet database, grouped by tier ----
function Show-LEDCabinets {
    Write-Host "`n=== LED CABINET PRESETS ===" -ForegroundColor Cyan
    $fmt = "    {0,-16} P{1,-4} {2,4}x{3,-4}px {4,-10} {5,-8} {6,5} nits {7,4} W/m2 {8,5}kg  {9}"
    $groups = @(
        @{ Title='Generic (env/pitch/size)'; Data=$Script:LEDGeneric }
        @{ Title='Brands (from manufacturer datasheets)'; Data=$Script:LEDBrands }
        @{ Title='Your cabinets (edit AVCalc.ps1)'; Data=$Script:LEDUser }
    )
    foreach ($grp in $groups) {
        Write-Host ("  --- {0} ---" -f $grp.Title) -ForegroundColor Yellow
        if ($grp.Data.Count -eq 0) { Write-Host "    (empty)" -ForegroundColor DarkGray; continue }
        foreach ($k in $grp.Data.Keys) {
            $c = $grp.Data[$k]
            $name = if ($c.Contains('Model')) { "$($c.Brand) $($c.Model)" } else { "" }
            $pit = if ($c.Contains('Pitch')) { [string]$c.Pitch } else { "" }
            $pw  = if ($c.Contains('MaxWm2')) { $c.MaxWm2 } elseif ($c.Contains('MaxW')) { [string]$c.MaxW + '*' } else { "" }
            Write-Host ($fmt -f $k, $pit, $c.PxW, $c.PxH, ("$($c.Wmm)x$($c.Hmm)mm"), $c.Env, $c.Nits, $pw, $c.Kg, $name)
        }
    }
    Write-Host "`n  Use:  calc-ledwall <cols> <rows> -Cabinet <key> [-Redundant] [-RecvCard A8s] [-Sender VX600]" -ForegroundColor DarkGray
    Write-Host ""
}

# ---- numbered-choice helper: returns 1-based index, or 0 if cancelled/invalid ----
function Read-LEDChoice([string]$Prompt, [string[]]$Options, [string]$Extra) {
    for ($i = 0; $i -lt $Options.Count; $i++) { Write-Host ("    {0}) {1}" -f ($i + 1), $Options[$i]) }
    if ($Extra) { Write-Host ("    {0}) {1}" -f ($Options.Count + 1), $Extra) -ForegroundColor DarkGray }
    if (-not [Console]::IsInputRedirected) { try { $Host.UI.RawUI.FlushInputBuffer() } catch {} }
    $sel = Read-Host $Prompt
    $n = 0
    $max = if ($Extra) { $Options.Count + 1 } else { $Options.Count }
    if (-not [int]::TryParse($sel, [ref]$n)) { return 0 }
    if ($n -lt 1 -or $n -gt $max) { return 0 }
    return $n
}

# ---- guided LED wall wizard: brand -> env -> pitch/size -> cabinets -> options ----
function Invoke-LEDWizard {
    # 1) brand / screen type
    $brands = @()
    foreach ($k in $Script:LEDCabinets.Keys) {
        $b = [string]$Script:LEDCabinets[$k].Brand
        if ($b -and ($brands -notcontains $b)) { $brands += $b }
    }
    Write-Host "`n  Screen type / brand:" -ForegroundColor Cyan
    $sel = Read-LEDChoice "  Choose brand" $brands "Manual (enter specs by hand)"
    if ($sel -eq 0) { Write-Host "  Cancelled.`n" -ForegroundColor Yellow; return }
    if ($sel -eq $brands.Count + 1) { Invoke-LEDManual; return }
    $brand = $brands[$sel - 1]

    # 2) environment (indoor / outdoor) available for that brand
    $envs = @()
    foreach ($k in $Script:LEDCabinets.Keys) {
        $c = $Script:LEDCabinets[$k]
        if ($c.Brand -eq $brand -and ($envs -notcontains [string]$c.Env)) { $envs += [string]$c.Env }
    }
    $env = $envs[0]
    if ($envs.Count -gt 1) {
        Write-Host "`n  Environment:" -ForegroundColor Cyan
        $sel = Read-LEDChoice "  Choose" $envs ""
        if ($sel -eq 0) { Write-Host "  Cancelled.`n" -ForegroundColor Yellow; return }
        $env = $envs[$sel - 1]
    }

    # 3) pitch / cabinet size within brand + env
    $keys = @(); $labels = @()
    foreach ($k in $Script:LEDCabinets.Keys) {
        $c = $Script:LEDCabinets[$k]
        if ($c.Brand -eq $brand -and $c.Env -eq $env) {
            $keys += $k
            $mdl = if ($c.Contains('Model')) { $c.Model } else { $k }
            $labels += ("P{0}  {1}x{2}px  {3}x{4}mm  {5}" -f $c.Pitch, $c.PxW, $c.PxH, $c.Wmm, $c.Hmm, $mdl)
        }
    }
    if ($keys.Count -eq 0) { Write-Host "  No presets for that combination.`n" -ForegroundColor Yellow; return }
    Write-Host ("`n  {0} / {1} - pitch and cabinet size:" -f $brand, $env) -ForegroundColor Cyan
    $sel = Read-LEDChoice "  Choose" $labels ""
    if ($sel -eq 0) { Write-Host "  Cancelled.`n" -ForegroundColor Yellow; return }
    $cabKey = $keys[$sel - 1]

    # 4) wall size in cabinets
    if (-not [Console]::IsInputRedirected) { try { $Host.UI.RawUI.FlushInputBuffer() } catch {} }
    $cc = Read-Host "`n  Columns (cabinets wide)"; $rr = Read-Host "  Rows (cabinets high)"; $cn = 0; $rn = 0
    if (-not ([int]::TryParse($cc, [ref]$cn) -and [int]::TryParse($rr, [ref]$rn))) { Write-Host "  Invalid size.`n" -ForegroundColor Yellow; return }

    # 5) options
    $p = @{ Cols = $cn; Rows = $rn; Cabinet = $cabKey }
    Invoke-LEDFinish $p
}

# ---- manual specs branch of the wizard ----
function Invoke-LEDManual {
    if (-not [Console]::IsInputRedirected) { try { $Host.UI.RawUI.FlushInputBuffer() } catch {} }
    $cc = Read-Host "`n  Columns (cabinets wide)"; $rr = Read-Host "  Rows (cabinets high)"; $cn = 0; $rn = 0
    if (-not ([int]::TryParse($cc, [ref]$cn) -and [int]::TryParse($rr, [ref]$rn))) { Write-Host "  Invalid size.`n" -ForegroundColor Yellow; return }
    $pw = Read-Host "  Cabinet px width"; $ph = Read-Host "  Cabinet px height"; $pwn = 0; $phn = 0
    if (-not ([int]::TryParse($pw, [ref]$pwn) -and [int]::TryParse($ph, [ref]$phn))) { Write-Host "  Invalid resolution.`n" -ForegroundColor Yellow; return }
    $wm = Read-Host "  Cabinet mm width (optional)"; $hm = Read-Host "  Cabinet mm height (optional)"
    $mw = Read-Host "  Max W per cabinet (optional)"; $kg = Read-Host "  Weight kg per cabinet (optional)"
    $p = @{ Cols = $cn; Rows = $rn; CabPxW = $pwn; CabPxH = $phn }
    if ($wm -as [double]) { $p.CabWmm = [double]$wm }; if ($hm -as [double]) { $p.CabHmm = [double]$hm }
    if ($mw -as [double]) { $p.MaxWatt = [double]$mw }; if ($kg -as [double]) { $p.WeightKg = [double]$kg }
    Invoke-LEDFinish $p
}

# ---- shared tail: redundancy / sender / distribution, then run ----
function Invoke-LEDFinish($p) {
    if (-not [Console]::IsInputRedirected) { try { $Host.UI.RawUI.FlushInputBuffer() } catch {} }
    $red = Read-Host "  Redundant signal? (y/N)"
    if ($red -match '^(y|yes|s|si)$') { $p.Redundant = $true }
    $snd = Read-Host "  Sender you have (optional, e.g. VX1000 Pro)"
    if ($snd) { $p.Sender = $snd }
    $rcd = Read-Host "  Receiving card (optional, e.g. A10s Pro - enables COEX/MX)"
    if ($rcd) { $p.RecvCard = $rcd }
    $pb = Read-Host "  Power sizing: A=average (real content) / M=max peak (all-white) [A]"
    if ($pb -match '^(m|max)$') { $p.PowerBasis = 'max' } else { $p.PowerBasis = 'avg' }
    $dist = Read-Host "  Diagram distribution: V=vertical / H=horizontal [V]"
    if ($dist -match '^(h|horizontal)$') { $p.Distribution = 'horizontal' } else { $p.Distribution = 'vertical' }
    Get-LEDWall @p
}

# ---- calc-bitrate: uncompressed + NDI estimate + storage per hour ----
function Get-VideoBitrate {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true, Position=0)][int]$Width,
        [Parameter(Mandatory=$true, Position=1)][int]$Height,
        [double]$Fps = 25,
        [int]$BitDepth = 8,
        [int]$Channels = 3,
        [double]$Hours = 1
    )
    $pixels  = [double]$Width * $Height
    $rawBps  = $pixels * $Fps * $BitDepth * $Channels        # bits/sec, uncompressed
    $rawMbps = $rawBps / 1e6
    $rawGBh  = $rawBps * 3600 / 8 / 1e9

    # NDI High Bandwidth (SpeedHQ) anchored at 1080p60 ~= 125 Mbps, scaled by pixels*fps
    $anchor = 1920.0 * 1080 * 60
    $ndiHB  = 125 * ($pixels * $Fps / $anchor)
    $ndiHX  = $ndiHB / 10                                    # NDI|HX (H.264/5) order-of-magnitude

    Write-Host ("`n=== VIDEO BITRATE ({0} x {1} @ {2}fps, {3}-bit, {4}ch) ===" -f $Width, $Height, $Fps, $BitDepth, $Channels) -ForegroundColor Cyan
    Write-Host ("  Uncompressed:  {0:N0} Mbps   ({1:N1} GB/hour)" -f $rawMbps, $rawGBh) -ForegroundColor White
    Write-Host ("  NDI High BW:   ~{0:N0} Mbps   (~{1:N1} GB/hour)" -f $ndiHB, ($ndiHB * 3600 / 8 / 1000)) -ForegroundColor Green
    Write-Host ("  NDI|HX:        ~{0:N0} Mbps   (H.264/5)" -f $ndiHX) -ForegroundColor DarkGray
    if ($Hours -ne 1) { Write-Host ("  Uncompressed x {0}h: {1:N1} GB" -f $Hours, ($rawGBh * $Hours)) -ForegroundColor Yellow }
    Write-Host "  (NDI figures are estimates; real bitrate varies with content.)" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name calc-bitrate -Value Get-VideoBitrate

# ---- calc-avoip: AV-over-IP bandwidth per codec (estimates) ----
function Get-AVoIPBandwidth {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true, Position=0)][int]$Width,
        [Parameter(Mandatory=$true, Position=1)][int]$Height,
        [double]$Fps = 60,
        [int]$BitDepth = 8
    )
    $px = [double]$Width * $Height
    $rawMbps = $px * $Fps * $BitDepth * 3 / 1e6
    $sf = $px * $Fps / (1920.0 * 1080 * 60)
    Write-Host ("`n=== AVoIP BANDWIDTH ({0}x{1}@{2}) ===" -f $Width, $Height, $Fps) -ForegroundColor Cyan
    Write-Host ("  Uncompressed:   {0:N0} Mbps" -f $rawMbps) -ForegroundColor White
    Write-Host  "  SDVoE (10GbE):  visually lossless, ~10.000 Mbps class" -ForegroundColor Green
    Write-Host ("  JPEG-XS:        ~{0:N0} Mbps  (light compression)" -f (800 * $sf))
    Write-Host ("  NDI High BW:    ~{0:N0} Mbps" -f (125 * $sf)) -ForegroundColor Green
    Write-Host ("  NDI|HX / H.26x: ~{0:N0} Mbps  (heavy compression)" -f (12 * $sf)) -ForegroundColor DarkGray
    Write-Host  "  (Estimates; SDVoE is a fixed 10G class, others scale with pixels x fps.)" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name calc-avoip -Value Get-AVoIPBandwidth

# ---- calc-projector: throw distance + required brightness ----
function Get-Projector {
    [CmdletBinding()]
    param(
        [double]$Width = 0,            # image width (m)
        [double]$Diagonal = 0,         # or diagonal (m) + aspect
        [string]$Aspect = '16:9',
        [double]$ThrowRatio = 0,       # ratio -> distance
        [double]$Distance = 0,         # or distance -> ratio
        [double]$Gain = 1.0,
        [double]$TargetFL = 16         # foot-lamberts (16 dim, 30-50 with ambient light)
    )
    $ap = $Aspect -split ':'
    if ($ap.Count -ne 2 -or [double]$ap[1] -eq 0) { Write-Host "  Invalid aspect '$Aspect'" -ForegroundColor Red; return }
    $arWH = [double]$ap[0] / [double]$ap[1]
    if ($Width -le 0 -and $Diagonal -gt 0) { $Width = $Diagonal * $arWH / [math]::Sqrt($arWH * $arWH + 1) }
    if ($Width -le 0) { Write-Host "  Usage: calc-projector -Width <m> [-ThrowRatio r | -Distance d] [-Gain g] [-TargetFL fL]" -ForegroundColor Yellow; return }
    $height = $Width / $arWH
    $areaFt2 = ($Width * 3.28084) * ($height * 3.28084)
    $lumens = $TargetFL * $areaFt2 / $Gain
    Write-Host "`n=== PROJECTOR ===" -ForegroundColor Cyan
    Write-Host ("  Image:          {0:N2} x {1:N2} m  ({2:N1} ft^2)" -f $Width, $height, $areaFt2) -ForegroundColor White
    if ($ThrowRatio -gt 0)   { Write-Host ("  Throw distance: {0:N2} m   (ratio {1})" -f ($ThrowRatio * $Width), $ThrowRatio) -ForegroundColor Green }
    elseif ($Distance -gt 0) { Write-Host ("  Throw ratio:    {0:N2}   (at {1} m)" -f ($Distance / $Width), $Distance) -ForegroundColor Green }
    Write-Host ("  Brightness:     ~{0:N0} ANSI lumens   (target {1} fL, gain {2})" -f $lumens, $TargetFL, $Gain) -ForegroundColor Green
    Write-Host "  (Brightness is a guideline; raise target fL for high ambient light.)" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name calc-projector -Value Get-Projector

# ---- calc-pixelpitch: DvLED pitch <-> viewing distance + resolution ----
function Get-PixelPitch {
    [CmdletBinding()]
    param(
        [double]$Pitch = 0,       # mm
        [double]$Distance = 0,    # m (reverse: recommend pitch)
        [double]$WidthM = 0,
        [double]$HeightM = 0
    )
    Write-Host "`n=== DvLED PIXEL PITCH ===" -ForegroundColor Cyan
    if ($Pitch -le 0 -and $Distance -gt 0) {
        Write-Host ("  Viewing distance:      {0} m" -f $Distance) -ForegroundColor White
        Write-Host ("  Recommended max pitch: {0:N2} mm   (retina, ~pitch x 3.44 m)" -f ($Distance / 3.438)) -ForegroundColor Green
        Write-Host ""
        return
    }
    if ($Pitch -le 0) { Write-Host "  Usage: calc-pixelpitch -Pitch <mm> [-WidthM w -HeightM h]   or  -Distance <m>`n" -ForegroundColor Yellow; return }
    Write-Host ("  Pixel pitch:      {0} mm" -f $Pitch) -ForegroundColor White
    Write-Host ("  Optimal min dist: {0:N2} m   (retina, 1px/arcmin)" -f ($Pitch * 3.438)) -ForegroundColor Green
    if ($WidthM -gt 0 -and $HeightM -gt 0) {
        $resW = [math]::Round($WidthM * 1000 / $Pitch); $resH = [math]::Round($HeightM * 1000 / $Pitch)
        Write-Host ("  Resolution:       {0} x {1} px  ({2:N1} MP)" -f $resW, $resH, ($resW * $resH / 1e6)) -ForegroundColor Green
    }
    Write-Host ""
}
Set-Alias -Name calc-pixelpitch -Value Get-PixelPitch

# ---- calc-brightness: required display luminance vs ambient (guideline) ----
function Get-DisplayBrightness {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true, Position=0)][double]$AmbientLux,
        [double]$Reflectance = 0.15,                          # display diffuse reflectance
        [ValidateSet('basic','detailed')][string]$Content = 'basic'
    )
    $cr = if ($Content -eq 'detailed') { 15 } else { 7 }      # AVIXA contrast-ratio targets
    $lReflect = $AmbientLux * $Reflectance / [math]::PI       # cd/m^2 (lambertian)
    Write-Host ("`n=== DISPLAY BRIGHTNESS (guideline, {0} {1}:1) ===" -f $Content, $cr) -ForegroundColor Cyan
    Write-Host ("  Ambient:          {0} lux" -f $AmbientLux) -ForegroundColor White
    Write-Host ("  Reflected light:  {0:N1} nits" -f $lReflect)
    Write-Host ("  Required display: ~{0:N0} nits   (for {1}:1 contrast)" -f ($lReflect * ($cr - 1)), $cr) -ForegroundColor Green
    Write-Host "  (Estimate from ambient reflection; verify with AVIXA ambient-light method.)" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name calc-brightness -Value Get-DisplayBrightness

# ---- calc-viewangle: usable seating width from off-axis limit (geometry) ----
function Get-ViewingAngle {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true, Position=0)][double]$Distance,   # viewer distance (m)
        [double]$MaxAngle = 45,                                      # max off-axis half-angle (deg)
        [double]$ScreenWidth = 0
    )
    $halfWidth = $Distance * [math]::Tan($MaxAngle * [math]::PI / 180)
    Write-Host ("`n=== VIEWING ANGLE (max +/-{0} deg off-axis) ===" -f $MaxAngle) -ForegroundColor Cyan
    Write-Host ("  Viewer distance:   {0} m" -f $Distance) -ForegroundColor White
    Write-Host ("  Usable width:      {0:N2} m   (+/-{1:N2} m from centre)" -f (2 * $halfWidth), $halfWidth) -ForegroundColor Green
    if ($ScreenWidth -gt 0) {
        Write-Host ("  Screen-edge angle: {0:N1} deg   (from a centre seat)" -f ([math]::Atan(($ScreenWidth / 2) / $Distance) * 180 / [math]::PI))
    }
    Write-Host "  (AVIXA viewing-cone guideline; check the display's own spec angle.)" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name calc-viewangle -Value Get-ViewingAngle

# ---- calc-camera: FOV + DORI distances (IEC 62676-4) ----
function Get-CameraDistance {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true, Position=0)][double]$FocalLength,   # mm
        [double]$SensorWidth = 36,                                      # mm (full-frame; 1/2.8" ~= 5.37)
        [int]$HResolution = 1920,
        [double]$Distance = 0                                           # optional: PPM at this distance
    )
    if ($FocalLength -le 0) { Write-Host "  Focal length must be > 0" -ForegroundColor Red; return }
    $hfov = 2 * [math]::Atan($SensorWidth / (2 * $FocalLength)) * 180 / [math]::PI
    Write-Host ("`n=== CAMERA / DORI ({0}mm, {1}mm sensor, {2}px) ===" -f $FocalLength, $SensorWidth, $HResolution) -ForegroundColor Cyan
    Write-Host ("  Horizontal FOV:   {0:N1} deg" -f $hfov) -ForegroundColor White
    if ($Distance -gt 0) {
        $fovW = $Distance * $SensorWidth / $FocalLength
        Write-Host ("  FOV width @ {0}m:  {1:N2} m  ->  {2:N0} px/m" -f $Distance, $fovW, ($HResolution / $fovW)) -ForegroundColor Green
    }
    Write-Host "  --- DORI max distance (px/m threshold) ---" -ForegroundColor Yellow
    foreach ($d in @([pscustomobject]@{n='Detect';p=25}, [pscustomobject]@{n='Observe';p=62.5}, [pscustomobject]@{n='Recognize';p=125}, [pscustomobject]@{n='Identify';p=250})) {
        Write-Host ("    {0,-10} ({1,5} px/m): up to {2:N1} m" -f $d.n, $d.p, ($HResolution * $FocalLength / ($d.p * $SensorWidth)))
    }
    Write-Host ""
}
Set-Alias -Name calc-camera -Value Get-CameraDistance

# ---- calc-audiobw: digital audio data rate + wavelength ----
function Get-AudioBandwidth {
    [CmdletBinding()]
    param(
        [int]$Channels = 64,
        [int]$SampleRate = 48000,
        [int]$BitDepth = 24,
        [double]$Freq = 0            # optional: wavelength for this frequency
    )
    $bps = [double]$Channels * $SampleRate * $BitDepth
    Write-Host "`n=== AUDIO BANDWIDTH ===" -ForegroundColor Cyan
    Write-Host ("  {0} ch x {1} Hz x {2}-bit" -f $Channels, $SampleRate, $BitDepth) -ForegroundColor White
    Write-Host ("  Payload data rate: {0:N1} Mbps" -f ($bps / 1e6)) -ForegroundColor Green
    Write-Host "  (Dante/AES67 add packet + protocol overhead on top.)" -ForegroundColor DarkGray
    if ($Freq -gt 0) { Write-Host ("  Wavelength @ {0} Hz: {1:N3} m   (343 m/s)" -f $Freq, (343 / $Freq)) -ForegroundColor Green }
    Write-Host ""
}
Set-Alias -Name calc-audiobw -Value Get-AudioBandwidth

# ---- calc-speaker: ceiling coverage layout + amp sizing ----
function Get-SpeakerDesign {
    [CmdletBinding()]
    param(
        [double]$CeilingHeight = 0,    # m
        [double]$EarHeight = 1.2,      # m (seated listener)
        [double]$CoverageAngle = 90,   # speaker -6dB coverage angle (deg)
        [double]$RoomWidth = 0,
        [double]$RoomLength = 0,
        [int]$Speakers = 0,            # amp mode
        [double]$WattsEach = 0,
        [double]$HeadroomDb = 6
    )
    Write-Host "`n=== SPEAKER DESIGN ===" -ForegroundColor Cyan
    if ($CeilingHeight -gt 0) {
        $dia = 2 * ($CeilingHeight - $EarHeight) * [math]::Tan($CoverageAngle * [math]::PI / 180 / 2)
        $spacing = $dia * 0.707
        Write-Host ("  Coverage diameter @ ear: {0:N2} m" -f $dia) -ForegroundColor Green
        Write-Host ("  Spacing (edge):          {0:N2} m" -f $dia)
        Write-Host ("  Spacing (0.707 overlap): {0:N2} m   (even coverage)" -f $spacing) -ForegroundColor Green
        if ($RoomWidth -gt 0 -and $RoomLength -gt 0 -and $spacing -gt 0) {
            $cols = [math]::Ceiling($RoomWidth / $spacing); $rows = [math]::Ceiling($RoomLength / $spacing)
            Write-Host ("  Room {0}x{1} m -> {2} x {3} = {4} speakers" -f $RoomWidth, $RoomLength, $cols, $rows, ($cols * $rows)) -ForegroundColor Green
        }
    }
    if ($Speakers -gt 0 -and $WattsEach -gt 0) {
        $total = $Speakers * $WattsEach
        Write-Host ("  Amp: {0} x {1} W = {2} W, +{3} dB headroom -> {4:N0} W" -f $Speakers, $WattsEach, $total, $HeadroomDb, ($total * [math]::Pow(10, $HeadroomDb / 10))) -ForegroundColor Green
    }
    if ($CeilingHeight -le 0 -and $Speakers -le 0) {
        Write-Host "  Usage: calc-speaker -CeilingHeight 3 -CoverageAngle 90 [-RoomWidth w -RoomLength l]" -ForegroundColor Yellow
        Write-Host "         amp:  calc-speaker -Speakers 12 -WattsEach 10 [-HeadroomDb 6]" -ForegroundColor DarkGray
    }
    Write-Host ""
}
Set-Alias -Name calc-speaker -Value Get-SpeakerDesign

# ---- calc-cable: speaker wire loss / coax (SDI,RF) attenuation ----
function Get-CableSize {
    [CmdletBinding()]
    param(
        [ValidateSet('speaker','coax')][string]$Type = 'speaker',
        [int]$AWG = 14,
        [double]$LengthM = 0,
        [double]$Impedance = 8,          # speaker load ohms
        [double]$AttenPer100m = 0,       # coax dB/100m at working frequency
        [double]$MaxDb = 0               # coax: budget -> max length
    )
    $ohmPerM = @{ 10 = 0.00328; 12 = 0.00521; 14 = 0.00829; 16 = 0.01317; 18 = 0.02095; 20 = 0.03331 }
    Write-Host "`n=== CABLE ===" -ForegroundColor Cyan
    if ($Type -eq 'speaker') {
        if (-not $ohmPerM.ContainsKey($AWG)) { Write-Host "  AWG must be one of: $(($ohmPerM.Keys | Sort-Object) -join ', ')" -ForegroundColor Red; return }
        if ($LengthM -le 0) { Write-Host "  Usage: calc-cable -Type speaker -AWG 14 -LengthM 30 [-Impedance 8]" -ForegroundColor Yellow; return }
        $rc = $ohmPerM[$AWG] * 2 * $LengthM               # round trip
        $lossDb = 10 * [math]::Log10(($Impedance + $rc) / $Impedance)
        $col = if ($lossDb -le 0.5) { 'Green' } elseif ($lossDb -le 1) { 'Yellow' } else { 'Red' }
        Write-Host ("  AWG {0}, {1} m, {2} ohm load" -f $AWG, $LengthM, $Impedance) -ForegroundColor White
        Write-Host ("  Cable resistance: {0:N3} ohm (round trip)" -f $rc)
        Write-Host ("  Power loss:       {0:N2} dB  ({1:N1}%)" -f $lossDb, ($rc / ($rc + $Impedance) * 100)) -ForegroundColor $col
        Write-Host "  (Aim for <= 0.5 dB; go thicker for long runs.)" -ForegroundColor DarkGray
    } else {
        if ($AttenPer100m -le 0) { Write-Host "  Usage: calc-cable -Type coax -AttenPer100m 20 -LengthM 80   (atten at working freq)" -ForegroundColor Yellow; return }
        if ($LengthM -gt 0) { Write-Host ("  Attenuation over {0} m: {1:N1} dB" -f $LengthM, ($AttenPer100m * $LengthM / 100)) -ForegroundColor Green }
        if ($MaxDb -gt 0)   { Write-Host ("  Max length for {0} dB budget: {1:N0} m" -f $MaxDb, ($MaxDb * 100 / $AttenPer100m)) -ForegroundColor Green }
        Write-Host "  (SDI receivers tolerate ~20 dB; use your cable's dB/100m at the SDI clock.)" -ForegroundColor DarkGray
    }
    Write-Host ""
}
Set-Alias -Name calc-cable -Value Get-CableSize

# ---- calc-dsp: gain conversion + latency budget + matrix ----
function Get-DSPSystem {
    [CmdletBinding()]
    param(
        [double]$Dbu = [double]::NaN,     # level to convert
        [double[]]$LatenciesMs = @(),     # latency chain to sum
        [int]$Inputs = 0,
        [int]$Outputs = 0
    )
    Write-Host "`n=== DSP SYSTEM ===" -ForegroundColor Cyan
    if (-not [double]::IsNaN($Dbu)) {
        $vrms = 0.7746 * [math]::Pow(10, $Dbu / 20)           # 0 dBu = 0.7746 Vrms
        Write-Host ("  {0} dBu = {1:N4} Vrms = {2:N2} dBV" -f $Dbu, $vrms, (20 * [math]::Log10($vrms))) -ForegroundColor Green
    }
    if ($LatenciesMs.Count -gt 0) {
        Write-Host ("  Latency chain: {0} = {1:N2} ms total" -f ($LatenciesMs -join ' + '), (($LatenciesMs | Measure-Object -Sum).Sum)) -ForegroundColor Green
    }
    if ($Inputs -gt 0 -and $Outputs -gt 0) {
        Write-Host ("  Matrix: {0} in x {1} out = {2} crosspoints" -f $Inputs, $Outputs, ($Inputs * $Outputs)) -ForegroundColor Green
    }
    if ([double]::IsNaN($Dbu) -and $LatenciesMs.Count -eq 0 -and $Inputs -le 0) {
        Write-Host "  Usage: calc-dsp -Dbu 4  |  -LatenciesMs 0.25,1.0,2.0  |  -Inputs 16 -Outputs 8" -ForegroundColor Yellow
    }
    Write-Host "  (Simplified helper; not a full DSP design tool.)" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name calc-dsp -Value Get-DSPSystem

# ---- calc: interactive menu ----
function Invoke-CalcMenu {
    $TL=[char]0x250C; $TR=[char]0x2510; $BL=[char]0x2514; $BR=[char]0x2518; $HZ=[char]0x2500; $VT=[char]0x2502; $boxW=52
    while ($true) {
        Clear-Host
        Write-Host ""
        $title = 'AV / IT CALCULATORS'
        $pad = $boxW - $title.Length; if ($pad -lt 0) { $pad = 0 }
        $lp = [math]::Floor($pad / 2); $rp = $pad - $lp
        Write-Host ("  " + $TL + ("$HZ" * $boxW) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT) -NoNewline -ForegroundColor DarkGray
        Write-Host ((" " * $lp) + $title + (" " * $rp)) -NoNewline -ForegroundColor Cyan
        Write-Host $VT -ForegroundColor DarkGray
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        $sec  = { param($t) $f = $boxW - $t.Length - 3; if ($f -lt 0) { $f = 0 }; Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray; Write-Host $t -NoNewline -ForegroundColor Yellow; Write-Host (" " + ("$HZ" * $f) + $TR) -ForegroundColor DarkGray }
        $item = { param($s) Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray; Write-Host $s }
        $bott = { Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray }

        & $sec 'Video'
        & $item '  1)  Screen size       (AVIXA viewing distance)'
        & $item '  2)  LED wall         (res, power, NovaStar signal + diagrams)'
        & $item '  3)  Video bitrate     (RAW + NDI estimate)'
        & $item '  4)  AVoIP bandwidth   (per codec)'
        & $item '  5)  Projector         (throw + brightness)'
        & $item '  6)  DvLED pixel pitch'
        & $item '  7)  Display brightness'
        & $item '  8)  Viewing angle'
        & $item '  9)  Camera / DORI'
        & $bott
        Write-Host ""
        & $sec 'Audio'
        & $item ' 10)  Audio bandwidth   (Dante/AES67, wavelength)'
        & $item ' 11)  Speaker design    (ceiling coverage + amp)'
        & $item ' 12)  Cable size        (speaker loss / coax SDI)'
        & $item ' 13)  DSP               (gain / latency / matrix)'
        & $bott
        Write-Host ""
        & $sec 'IT'
        & $item ' 14)  Subnet / IPs      (CIDR -> range + table)'
        & $bott
        Write-Host ""
        Write-Host "    0)  Exit" -ForegroundColor DarkGray
        Write-Host ""
        if (-not [Console]::IsInputRedirected) { try { $Host.UI.RawUI.FlushInputBuffer() } catch {} }
        $op = Read-Host "  Choose option"
        switch ($op) {
            "1"  { $d=Read-Host "  Farthest viewer distance"; $c=Read-Host "  Content (detailed/basic/passive)"; if(-not $c){$c='basic'}; if(($d -as [double]) -and $c -in 'detailed','basic','passive'){ Get-ScreenSize -Distance ([double]$d) -Content $c } else { Write-Host "  Invalid input." -ForegroundColor Yellow } }
            "2"  { Invoke-LEDWizard }
            "3"  { $w=Read-Host "  Width px"; $h=Read-Host "  Height px"; $f=Read-Host "  Fps (default 25)"; if(-not $f){$f=25}; $wn=0;$hn=0; if([int]::TryParse($w,[ref]$wn) -and [int]::TryParse($h,[ref]$hn) -and ($f -as [double])){ Get-VideoBitrate -Width $wn -Height $hn -Fps ([double]$f) } else { Write-Host "  Invalid input." -ForegroundColor Yellow } }
            "4"  { $w=Read-Host "  Width px"; $h=Read-Host "  Height px"; $f=Read-Host "  Fps (default 60)"; if(-not $f){$f=60}; $wn=0;$hn=0; if([int]::TryParse($w,[ref]$wn) -and [int]::TryParse($h,[ref]$hn) -and ($f -as [double])){ Get-AVoIPBandwidth -Width $wn -Height $hn -Fps ([double]$f) } else { Write-Host "  Invalid input." -ForegroundColor Yellow } }
            "5"  { $wd=Read-Host "  Image width (m)"; $tr=Read-Host "  Throw ratio (e.g. 1.5)"; if(($wd -as [double]) -and ($tr -as [double])){ Get-Projector -Width ([double]$wd) -ThrowRatio ([double]$tr) } else { Write-Host "  Invalid input." -ForegroundColor Yellow } }
            "6"  { $pp=Read-Host "  Pixel pitch (mm)"; $ww=Read-Host "  Wall width m (optional)"; $hh=Read-Host "  Wall height m (optional)"; if($pp -as [double]){ if(($ww -as [double]) -and ($hh -as [double])){ Get-PixelPitch -Pitch ([double]$pp) -WidthM ([double]$ww) -HeightM ([double]$hh) } else { Get-PixelPitch -Pitch ([double]$pp) } } else { Write-Host "  Invalid input." -ForegroundColor Yellow } }
            "7"  { $lx=Read-Host "  Ambient light (lux)"; $c=Read-Host "  Content (basic/detailed)"; if(-not $c){$c='basic'}; if(($lx -as [double]) -and $c -in 'basic','detailed'){ Get-DisplayBrightness -AmbientLux ([double]$lx) -Content $c } else { Write-Host "  Invalid input." -ForegroundColor Yellow } }
            "8"  { $ds=Read-Host "  Viewer distance (m)"; $ma=Read-Host "  Max off-axis angle deg (default 45)"; if(-not $ma){$ma=45}; if(($ds -as [double]) -and ($ma -as [double])){ Get-ViewingAngle -Distance ([double]$ds) -MaxAngle ([double]$ma) } else { Write-Host "  Invalid input." -ForegroundColor Yellow } }
            "9"  { $fl=Read-Host "  Focal length (mm)"; $sw=Read-Host "  Sensor width mm (default 36)"; if(-not $sw){$sw=36}; $ds=Read-Host "  Distance m (optional)"; if(($fl -as [double]) -and ($sw -as [double])){ if($ds -as [double]){ Get-CameraDistance -FocalLength ([double]$fl) -SensorWidth ([double]$sw) -Distance ([double]$ds) } else { Get-CameraDistance -FocalLength ([double]$fl) -SensorWidth ([double]$sw) } } else { Write-Host "  Invalid input." -ForegroundColor Yellow } }
            "10" { $ch=Read-Host "  Channels (default 64)"; if(-not $ch){$ch=64}; $sr=Read-Host "  Sample rate (default 48000)"; if(-not $sr){$sr=48000}; $bd=Read-Host "  Bit depth (default 24)"; if(-not $bd){$bd=24}; $cn=0;$sn=0;$bn=0; if([int]::TryParse("$ch",[ref]$cn) -and [int]::TryParse("$sr",[ref]$sn) -and [int]::TryParse("$bd",[ref]$bn)){ Get-AudioBandwidth -Channels $cn -SampleRate $sn -BitDepth $bn } else { Write-Host "  Invalid input." -ForegroundColor Yellow } }
            "11" { $chg=Read-Host "  Ceiling height (m)"; $ca=Read-Host "  Coverage angle deg (default 90)"; if(-not $ca){$ca=90}; $rw=Read-Host "  Room width m (optional)"; $rl=Read-Host "  Room length m (optional)"; if(($chg -as [double]) -and ($ca -as [double])){ if(($rw -as [double]) -and ($rl -as [double])){ Get-SpeakerDesign -CeilingHeight ([double]$chg) -CoverageAngle ([double]$ca) -RoomWidth ([double]$rw) -RoomLength ([double]$rl) } else { Get-SpeakerDesign -CeilingHeight ([double]$chg) -CoverageAngle ([double]$ca) } } else { Write-Host "  Invalid input." -ForegroundColor Yellow } }
            "12" { $ty=Read-Host "  Type (speaker/coax, default speaker)"; if(-not $ty){$ty='speaker'}; if($ty -eq 'speaker'){ $g=Read-Host "  AWG (default 14)"; if(-not $g){$g=14}; $ln=Read-Host "  Length (m)"; $gn=0; if([int]::TryParse("$g",[ref]$gn) -and ($ln -as [double])){ Get-CableSize -Type speaker -AWG $gn -LengthM ([double]$ln) } else { Write-Host "  Invalid input." -ForegroundColor Yellow } } elseif($ty -eq 'coax'){ $at=Read-Host "  Attenuation dB/100m"; $ln=Read-Host "  Length (m)"; if(($at -as [double]) -and ($ln -as [double])){ Get-CableSize -Type coax -AttenPer100m ([double]$at) -LengthM ([double]$ln) } else { Write-Host "  Invalid input." -ForegroundColor Yellow } } else { Write-Host "  Invalid type." -ForegroundColor Yellow } }
            "13" { $db=Read-Host "  Level dBu (blank to skip)"; $io=Read-Host "  Matrix in x out (e.g. 16x8, blank to skip)"; $did=$false; if($db -as [double]){ Get-DSPSystem -Dbu ([double]$db); $did=$true }; if($io -match '^\d+x\d+$'){ $p=$io -split 'x'; Get-DSPSystem -Inputs ([int]$p[0]) -Outputs ([int]$p[1]); $did=$true }; if(-not $did){ Write-Host "  Nothing to compute." -ForegroundColor Yellow } }
            "14" { $cidr=Read-Host "  CIDR (e.g. 192.168.10.0/24)"; if($cidr){ Get-Subnet $cidr } }
            "0"  { return }
            default { Write-Host "  Invalid option" -ForegroundColor Yellow }
        }
        Write-Host ""
        Read-Host "  [Enter] to return" | Out-Null
    }
}
Set-Alias -Name calc -Value Invoke-CalcMenu
