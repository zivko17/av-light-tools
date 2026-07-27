# ============================================================
#  EDIDTools.ps1 - EDID read / decode / compare for AV
#
#  READ-ONLY BY DESIGN. Nothing here writes an EDID override or
#  touches the display driver. To FORCE an EDID use CRU (ToastyX)
#  or a hardware EDID emulator; this tool tells you what is
#  actually being reported so you know what to force.
#
#  Sources, in order: registry (Enum\DISPLAY, full blob incl.
#  extensions) then WMI WmiMonitorDescriptorMethods as fallback.
# ============================================================

# ------------------------------------------------------------
#  Reference tables
# ------------------------------------------------------------

# Common PnP manufacturer IDs. Unknown codes are shown as-is.
$script:EDIDVendor = @{
    'GSM'='LG Electronics'; 'LGD'='LG Display';  'SAM'='Samsung';       'SEC'='Samsung / Seiko Epson'
    'DEL'='Dell';           'ACI'='ASUS';        'AUS'='ASUS';          'BNQ'='BenQ'
    'ACR'='Acer';           'HWP'='HP';          'HPN'='HP';            'LEN'='Lenovo'
    'VSC'='ViewSonic';      'AOC'='AOC';         'MSI'='MSI';           'GBT'='Gigabyte'
    'PHL'='Philips';        'SNY'='Sony';        'NEC'='NEC';           'EIZ'='EIZO'
    'SHP'='Sharp';          'TSB'='Toshiba';     'APP'='Apple';         'CMN'='Chimei Innolux'
    'BOE'='BOE';            'AUO'='AU Optronics';'EPS'='Epson';         'VIZ'='Vizio'
    'HIT'='Hitachi';        'MEI'='Panasonic';   'PIO'='Pioneer';       'BBY'='Insignia'
}

# CTA-861 Video Identification Codes. Unlisted codes print as "VIC n".
$script:EDIDVic = @{
     1='640x480p60';      2='720x480p60 4:3';   3='720x480p60 16:9';  4='1280x720p60'
     5='1920x1080i60';    6='720x480i60 4:3';   7='720x480i60 16:9';  8='720x240p60 4:3'
     9='720x240p60 16:9';14='1440x480p60 4:3'; 15='1440x480p60 16:9';16='1920x1080p60'
    17='720x576p50 4:3'; 18='720x576p50 16:9'; 19='1280x720p50';     20='1920x1080i50'
    21='720x576i50 4:3'; 22='720x576i50 16:9'; 29='1440x576p50 4:3'; 30='1440x576p50 16:9'
    31='1920x1080p50';   32='1920x1080p24';    33='1920x1080p25';    34='1920x1080p30'
    35='2880x480p60 4:3';36='2880x480p60 16:9';37='2880x576p50 4:3'; 38='2880x576p50 16:9'
    39='1920x1080i50';   40='1920x1080i100';   41='1280x720p100';    42='720x576p100 4:3'
    43='720x576p100 16:9';44='720x576i100 4:3';45='720x576i100 16:9';46='1920x1080i120'
    47='1280x720p120';   48='720x480p120 4:3'; 49='720x480p120 16:9';50='720x480i120 4:3'
    51='720x480i120 16:9';52='720x576p200 4:3';53='720x576p200 16:9';54='720x576i200 4:3'
    55='720x576i200 16:9';56='720x480p240 4:3';57='720x480p240 16:9';58='720x480i240 4:3'
    59='720x480i240 16:9';60='1280x720p24';    61='1280x720p25';     62='1280x720p30'
    63='1920x1080p120';  64='1920x1080p100'
    93='3840x2160p24';   94='3840x2160p25';    95='3840x2160p30';    96='3840x2160p50'
    97='3840x2160p60';   98='4096x2160p24';    99='4096x2160p25';   100='4096x2160p30'
   101='4096x2160p50';  102='4096x2160p60';   103='3840x2160p24';   104='3840x2160p25'
   105='3840x2160p30';  106='3840x2160p50';   107='3840x2160p60'
}

# CTA-861 audio format codes (short audio descriptors)
$script:EDIDAudioFormat = @{
     1='LPCM';    2='AC-3';      3='MPEG-1';   4='MP3';       5='MPEG-2'
     6='AAC LC';  7='DTS';       8='ATRAC';    9='DSD';      10='E-AC-3'
    11='DTS-HD'; 12='MLP/TrueHD';13='DST';    14='WMA Pro';  15='Extended'
}

# Established timings bitmaps (bytes 35..37 of the base block)
$script:EDIDEstablished = @(
    @{ B=0; Bit=7; T='720x400@70'   }, @{ B=0; Bit=6; T='720x400@88'   }
    @{ B=0; Bit=5; T='640x480@60'   }, @{ B=0; Bit=4; T='640x480@67'   }
    @{ B=0; Bit=3; T='640x480@72'   }, @{ B=0; Bit=2; T='640x480@75'   }
    @{ B=0; Bit=1; T='800x600@56'   }, @{ B=0; Bit=0; T='800x600@60'   }
    @{ B=1; Bit=7; T='800x600@72'   }, @{ B=1; Bit=6; T='800x600@75'   }
    @{ B=1; Bit=5; T='832x624@75'   }, @{ B=1; Bit=4; T='1024x768@87i' }
    @{ B=1; Bit=3; T='1024x768@60'  }, @{ B=1; Bit=2; T='1024x768@70'  }
    @{ B=1; Bit=1; T='1024x768@75'  }, @{ B=1; Bit=0; T='1280x1024@75' }
    @{ B=2; Bit=7; T='1152x870@75'  }
)


# ------------------------------------------------------------
#  Helpers
# ------------------------------------------------------------

# Where dumps go. On a portable/USB run Start-Portable.ps1 sets
# AVLT_PORTABLE_ROOT so everything stays on the stick; otherwise
# fall back to the user's Documents shell folder.
function Get-EDIDOutputPath {
    if ($env:AVLT_PORTABLE_ROOT) {
        # The folder itself may not exist yet - check that its parent (the
        # stick) is reachable, so a portable run doesn't silently fall back
        # to the host PC's Documents folder.
        $parent = Split-Path -Parent $env:AVLT_PORTABLE_ROOT
        if ($parent -and (Test-Path -LiteralPath $parent)) {
            return (Join-Path $env:AVLT_PORTABLE_ROOT 'EDID')
        }
    }
    return (Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'EDID')
}

function Write-EDIDField {
    param([string]$Label, [string]$Value, [string]$Color = 'White')
    Write-Host ("    {0,-22}" -f $Label) -NoNewline
    Write-Host $Value -ForegroundColor $Color
}

function ConvertFrom-EDIDManufacturerId {
    param([byte]$B0, [byte]$B1)
    $v  = (([int]$B0) -shl 8) -bor ([int]$B1)
    $c1 = (($v -shr 10) -band 0x1F)
    $c2 = (($v -shr 5)  -band 0x1F)
    $c3 = ($v -band 0x1F)
    return (-join @($c1, $c2, $c3 | ForEach-Object { [char](64 + $_) }))
}

# Descriptor text fields (monitor name / serial / free text): bytes 5..17,
# terminated by 0x0A and space-padded.
function ConvertFrom-EDIDDescriptorText {
    param([byte[]]$D)
    $sb = New-Object System.Text.StringBuilder
    for ($i = 5; $i -le 17; $i++) {
        if ($D[$i] -eq 0x0A) { break }
        [void]$sb.Append([char]$D[$i])
    }
    return $sb.ToString().Trim()
}

# 18-byte Detailed Timing Descriptor -> object (or $null if not a DTD)
function ConvertFrom-EDIDDetailedTiming {
    param([byte[]]$D)

    $pclk = ((([int]$D[1]) -shl 8) -bor ([int]$D[0])) * 10000.0   # Hz
    if ($pclk -le 0) { return $null }

    $hAct = ((([int]$D[4]) -shr 4) -shl 8) -bor ([int]$D[2])
    $hBl  = ((([int]$D[4]) -band 0x0F) -shl 8) -bor ([int]$D[3])
    $vAct = ((([int]$D[7]) -shr 4) -shl 8) -bor ([int]$D[5])
    $vBl  = ((([int]$D[7]) -band 0x0F) -shl 8) -bor ([int]$D[6])

    $hSyncOff = (((([int]$D[11]) -shr 6) -band 0x03) -shl 8) -bor ([int]$D[8])
    $hSyncW   = (((([int]$D[11]) -shr 4) -band 0x03) -shl 8) -bor ([int]$D[9])
    $vSyncOff = (((([int]$D[11]) -shr 2) -band 0x03) -shl 4) -bor ((([int]$D[10]) -shr 4) -band 0x0F)
    $vSyncW   = ((([int]$D[11]) -band 0x03) -shl 4) -bor (([int]$D[10]) -band 0x0F)

    $hSize = ((([int]$D[14]) -shr 4) -shl 8) -bor ([int]$D[12])
    $vSize = ((([int]$D[14]) -band 0x0F) -shl 8) -bor ([int]$D[13])

    $flags      = [int]$D[17]
    $interlaced = (($flags -band 0x80) -ne 0)
    $digSep     = ((($flags -shr 3) -band 0x03) -eq 0x03)
    $hPol = '?'; $vPol = '?'
    if ($digSep) {
        $vPol = if (($flags -band 0x04) -ne 0) { '+' } else { '-' }
        $hPol = if (($flags -band 0x02) -ne 0) { '+' } else { '-' }
    }

    $hTotal = $hAct + $hBl
    $vTotal = $vAct + $vBl
    $refresh = 0.0
    if ($hTotal -gt 0 -and $vTotal -gt 0) { $refresh = $pclk / ($hTotal * $vTotal) }

    return [PSCustomObject]@{
        PixelClockMHz = [math]::Round($pclk / 1000000.0, 3)
        HActive       = $hAct
        HBlank        = $hBl
        HFrontPorch   = $hSyncOff
        HSyncWidth    = $hSyncW
        HPolarity     = $hPol
        VActive       = $vAct
        VBlank        = $vBl
        VFrontPorch   = $vSyncOff
        VSyncWidth    = $vSyncW
        VPolarity     = $vPol
        Interlaced    = $interlaced
        Refresh       = [math]::Round($refresh, 3)
        WidthMM       = $hSize
        HeightMM      = $vSize
        Summary       = ('{0}x{1}{2} @ {3} Hz' -f $hAct, $vAct,
                          $(if ($interlaced) { 'i' } else { 'p' }), [math]::Round($refresh, 2))
    }
}

# ------------------------------------------------------------
#  Base block (bytes 0..127)
# ------------------------------------------------------------
function ConvertFrom-EDIDBaseBlock {
    param([byte[]]$B)

    $mfg     = ConvertFrom-EDIDManufacturerId -B0 $B[8] -B1 $B[9]
    # Product code and serial are little-endian; BitConverter avoids the
    # int/uint overflow traps of -shl on a 32-bit value under PS 5.1.
    $product = [System.BitConverter]::ToUInt16($B, 10)
    $serial  = [System.BitConverter]::ToUInt32($B, 12)

    $week = [int]$B[16]
    $year = 1990 + [int]$B[17]
    $ver  = '{0}.{1}' -f $B[18], $B[19]
    $rev  = [int]$B[19]

    # --- video input ---
    $vin       = [int]$B[20]
    $isDigital = (($vin -band 0x80) -ne 0)
    $inputDesc = 'Analog'
    $bitDepth  = $null
    $interface = $null
    if ($isDigital) {
        # Bit depth and interface only exist from EDID 1.4 on. In 1.3 the low
        # bits mean "DFP 1.x compatible", so don't invent a depth/interface.
        if ($rev -ge 4) {
            $depthMap = @{ 0='undefined bpc'; 1='6 bpc'; 2='8 bpc'; 3='10 bpc'; 4='12 bpc'; 5='14 bpc'; 6='16 bpc'; 7='reserved' }
            $ifaceMap = @{ 0='undefined interface'; 1='DVI'; 2='HDMI-a'; 3='HDMI-b'; 4='MDDI'; 5='DisplayPort' }
            $bitDepth  = $depthMap[(($vin -shr 4) -band 0x07)]
            $interface = $ifaceMap[($vin -band 0x0F)]
            if (-not $interface) { $interface = 'reserved interface' }
            $inputDesc = 'Digital, {0}, {1}' -f $interface, $bitDepth
        }
        else {
            $inputDesc = 'Digital'
            if (($vin -band 0x01) -ne 0) { $inputDesc += ', DFP 1.x compatible' }
            $inputDesc += '   (EDID 1.3: no depth/interface field)'
        }
    }

    # --- physical size / gamma / features ---
    $hCm = [int]$B[21]; $vCm = [int]$B[22]
    $diagIn = $null
    if ($hCm -gt 0 -and $vCm -gt 0) {
        $diagIn = [math]::Round([math]::Sqrt(($hCm * $hCm) + ($vCm * $vCm)) / 2.54, 1)
    }
    $gamma = $null
    if ($B[23] -ne 0xFF) { $gamma = [math]::Round((([int]$B[23]) + 100) / 100.0, 2) }

    $feat = [int]$B[24]
    $features = New-Object System.Collections.Generic.List[string]
    if (($feat -band 0x80) -ne 0) { $features.Add('DPMS standby') }
    if (($feat -band 0x40) -ne 0) { $features.Add('DPMS suspend') }
    if (($feat -band 0x20) -ne 0) { $features.Add('DPMS active-off') }
    if (($feat -band 0x04) -ne 0) { $features.Add('sRGB default') }
    if (($feat -band 0x02) -ne 0) { $features.Add('preferred timing is native') }
    if (($feat -band 0x01) -ne 0) { $features.Add('continuous frequency') }

    $colourType = ''
    if ($isDigital) {
        $ct = @{ 0='RGB 4:4:4'; 1='RGB 4:4:4 + YCrCb 4:4:4'; 2='RGB 4:4:4 + YCrCb 4:2:2'
                 3='RGB 4:4:4 + YCrCb 4:4:4 + YCrCb 4:2:2' }
        $colourType = $ct[(($feat -shr 3) -band 0x03)]
    } else {
        $ct = @{ 0='Monochrome'; 1='RGB colour'; 2='Non-RGB colour'; 3='Undefined' }
        $colourType = $ct[(($feat -shr 3) -band 0x03)]
    }

    # --- established timings ---
    $established = New-Object System.Collections.Generic.List[string]
    foreach ($e in $script:EDIDEstablished) {
        if ((([int]$B[35 + $e.B]) -band (1 -shl $e.Bit)) -ne 0) { $established.Add($e.T) }
    }

    # --- standard timings (bytes 38..53) ---
    $standard = New-Object System.Collections.Generic.List[string]
    for ($i = 38; $i -lt 54; $i += 2) {
        if ($B[$i] -eq 0x01 -and $B[$i+1] -eq 0x01) { continue }
        if ($B[$i] -eq 0x00 -or $B[$i] -eq 0xFF)    { continue }
        $h  = (([int]$B[$i]) + 31) * 8
        $ar = (([int]$B[$i+1]) -shr 6) -band 0x03
        $hz = (([int]$B[$i+1]) -band 0x3F) + 60
        switch ($ar) {
            0 { if ($rev -lt 3) { $v = $h; $arT = '1:1' } else { $v = [int]($h * 10 / 16); $arT = '16:10' } }
            1 { $v = [int]($h * 3 / 4);  $arT = '4:3'  }
            2 { $v = [int]($h * 4 / 5);  $arT = '5:4'  }
            3 { $v = [int]($h * 9 / 16); $arT = '16:9' }
        }
        $standard.Add(('{0}x{1}@{2} ({3})' -f $h, $v, $hz, $arT))
    }

    # --- the four 18-byte descriptors (bytes 54..125) ---
    $timings = New-Object System.Collections.Generic.List[object]
    $monitorName = $null; $serialText = $null
    $textFields  = New-Object System.Collections.Generic.List[string]
    $range = $null

    for ($d = 54; $d -le 108; $d += 18) {
        $desc = $B[$d..($d + 17)]
        if ($desc[0] -eq 0 -and $desc[1] -eq 0 -and $desc[2] -eq 0) {
            switch ([int]$desc[3]) {
                0xFC { $monitorName = ConvertFrom-EDIDDescriptorText $desc }
                0xFF { $serialText  = ConvertFrom-EDIDDescriptorText $desc }
                0xFE { $t = ConvertFrom-EDIDDescriptorText $desc; if ($t) { $textFields.Add($t) } }
                0xFD {
                    $offV = 0; $offH = 0
                    if ($rev -ge 4) {
                        if ((([int]$desc[4]) -band 0x02) -ne 0) { $offV = 255 }
                        if ((([int]$desc[4]) -band 0x08) -ne 0) { $offH = 255 }
                    }
                    $range = [PSCustomObject]@{
                        VMin        = [int]$desc[5]
                        VMax        = [int]$desc[6] + $offV
                        HMin        = [int]$desc[7]
                        HMax        = [int]$desc[8] + $offH
                        MaxClockMHz = ([int]$desc[9]) * 10
                    }
                }
            }
        }
        else {
            $t = ConvertFrom-EDIDDetailedTiming $desc
            if ($t) { $timings.Add($t) }
        }
    }

    return [PSCustomObject]@{
        HeaderValid  = ($B[0] -eq 0x00 -and $B[1] -eq 0xFF -and $B[2] -eq 0xFF -and $B[3] -eq 0xFF -and
                        $B[4] -eq 0xFF -and $B[5] -eq 0xFF -and $B[6] -eq 0xFF -and $B[7] -eq 0x00)
        Manufacturer = $mfg
        VendorName   = $(if ($script:EDIDVendor.ContainsKey($mfg)) { $script:EDIDVendor[$mfg] } else { $null })
        ProductCode  = $product
        SerialNumber = $serial
        Week         = $week
        Year         = $year
        Version      = $ver
        Revision     = $rev
        IsDigital    = $isDigital
        InputDesc    = $inputDesc
        BitDepth     = $bitDepth
        Interface    = $interface
        WidthCm      = $hCm
        HeightCm     = $vCm
        DiagonalIn   = $diagIn
        Gamma        = $gamma
        Features     = $features
        ColourType   = $colourType
        Established  = $established
        Standard     = $standard
        Timings      = $timings
        MonitorName  = $monitorName
        SerialText   = $serialText
        TextFields   = $textFields
        Range        = $range
        ExtCount     = [int]$B[126]
    }
}

# ------------------------------------------------------------
#  CTA-861 extension block (tag 0x02)
# ------------------------------------------------------------
function ConvertFrom-EDIDCtaBlock {
    param([byte[]]$B)

    $rev = [int]$B[1]
    $d   = [int]$B[2]
    $f   = [int]$B[3]

    $res = [PSCustomObject]@{
        Revision       = $rev
        UnderscanIT    = (($f -band 0x80) -ne 0)
        BasicAudio     = (($f -band 0x40) -ne 0)
        YCbCr444       = (($f -band 0x20) -ne 0)
        YCbCr422       = (($f -band 0x10) -ne 0)
        NativeDTDs     = ($f -band 0x0F)
        Video          = (New-Object System.Collections.Generic.List[object])
        Video420       = (New-Object System.Collections.Generic.List[object])
        Audio          = (New-Object System.Collections.Generic.List[string])
        Speakers       = (New-Object System.Collections.Generic.List[string])
        Colorimetry    = (New-Object System.Collections.Generic.List[string])
        HdrEotf        = (New-Object System.Collections.Generic.List[string])
        HdrMaxLum      = $null
        HdrAvgLum      = $null
        HdrMinLum      = $null
        Hdmi           = $null
        HdmiForum      = $null
        QuantRgb       = $false
        QuantYcc       = $false
        Timings        = (New-Object System.Collections.Generic.List[object])
        UnknownBlocks  = (New-Object System.Collections.Generic.List[string])
    }

    # --- data block collection (bytes 4 .. d-1) ---
    if ($d -gt 4) {
        $i = 4
        while ($i -lt $d -and $i -lt 127) {
            $tag = (([int]$B[$i]) -shr 5) -band 0x07
            $len = ([int]$B[$i]) -band 0x1F
            if ($len -eq 0) { $i += 1; continue }
            $end = $i + $len
            if ($end -gt 126) { break }
            $p = @($B[($i + 1)..$end])  # payload, without the header byte

            switch ($tag) {
                1 {   # Audio Data Block - 3-byte SADs
                    for ($k = 0; ($k + 2) -lt $p.Count; $k += 3) {
                        $fmt = (([int]$p[$k]) -shr 3) -band 0x0F
                        $ch  = (([int]$p[$k]) -band 0x07) + 1
                        $name = if ($script:EDIDAudioFormat.ContainsKey($fmt)) { $script:EDIDAudioFormat[$fmt] } else { "format $fmt" }
                        $rates = New-Object System.Collections.Generic.List[string]
                        $rateTab = @('32','44.1','48','88.2','96','176.4','192')
                        for ($r = 0; $r -lt 7; $r++) {
                            if ((([int]$p[$k+1]) -band (1 -shl $r)) -ne 0) { $rates.Add($rateTab[$r]) }
                        }
                        $extra = ''
                        if ($fmt -eq 1) {
                            $bits = New-Object System.Collections.Generic.List[string]
                            if ((([int]$p[$k+2]) -band 0x01) -ne 0) { $bits.Add('16') }
                            if ((([int]$p[$k+2]) -band 0x02) -ne 0) { $bits.Add('20') }
                            if ((([int]$p[$k+2]) -band 0x04) -ne 0) { $bits.Add('24') }
                            if ($bits.Count) { $extra = ('  {0} bit' -f ($bits -join '/')) }
                        }
                        elseif ([int]$p[$k+2] -gt 0) {
                            $extra = ('  max {0} kbit/s' -f (([int]$p[$k+2]) * 8))
                        }
                        $res.Audio.Add(('{0,-10} {1} ch   {2} kHz{3}' -f $name, $ch, ($rates -join '/'), $extra))
                    }
                }
                2 {   # Video Data Block - 1-byte SVDs
                    foreach ($svd in $p) {
                        $v = [int]$svd
                        $native = $false
                        if ($v -ge 129 -and $v -le 192) { $native = $true; $v = $v - 128 }
                        elseif ($v -ge 1 -and $v -le 64 -and (([int]$svd) -band 0x80) -ne 0) { $native = $true }
                        $res.Video.Add([PSCustomObject]@{ Vic = $v; Native = $native })
                    }
                }
                3 {   # Vendor Specific Data Block
                    if ($p.Count -ge 3) {
                        $oui = '{0:X2}-{1:X2}-{2:X2}' -f $p[2], $p[1], $p[0]
                        if ($oui -eq '00-0C-03') {
                            $h = [PSCustomObject]@{
                                PhysicalAddress = $null; MaxTmdsMHz = $null
                                DeepColour = (New-Object System.Collections.Generic.List[string])
                                DviDual = $false; SupportsAI = $false
                            }
                            if ($p.Count -ge 5) {
                                $h.PhysicalAddress = '{0}.{1}.{2}.{3}' -f
                                    ((([int]$p[3]) -shr 4) -band 0x0F), (([int]$p[3]) -band 0x0F),
                                    ((([int]$p[4]) -shr 4) -band 0x0F), (([int]$p[4]) -band 0x0F)
                            }
                            if ($p.Count -ge 6) {
                                $x = [int]$p[5]
                                if (($x -band 0x80) -ne 0) { $h.SupportsAI = $true }
                                if (($x -band 0x40) -ne 0) { $h.DeepColour.Add('48-bit') }
                                if (($x -band 0x20) -ne 0) { $h.DeepColour.Add('36-bit') }
                                if (($x -band 0x10) -ne 0) { $h.DeepColour.Add('30-bit') }
                                if (($x -band 0x08) -ne 0) { $h.DeepColour.Add('YCbCr 4:4:4') }
                                if (($x -band 0x01) -ne 0) { $h.DviDual = $true }
                            }
                            if ($p.Count -ge 7 -and [int]$p[6] -gt 0) { $h.MaxTmdsMHz = ([int]$p[6]) * 5 }
                            $res.Hdmi = $h
                        }
                        elseif ($oui -eq 'C4-5D-D8') {
                            $hf = [PSCustomObject]@{
                                Version = $(if ($p.Count -ge 4) { [int]$p[3] } else { $null })
                                MaxTmdsCharRateMHz = $null
                                Scdc = $false
                                MaxFrlRate = $null
                            }
                            if ($p.Count -ge 5 -and [int]$p[4] -gt 0) { $hf.MaxTmdsCharRateMHz = ([int]$p[4]) * 5 }
                            if ($p.Count -ge 6) { $hf.Scdc = (((([int]$p[5]) -shr 7) -band 0x01) -ne 0) }
                            if ($p.Count -ge 8) { $hf.MaxFrlRate = ((([int]$p[7]) -shr 4) -band 0x0F) }
                            $res.HdmiForum = $hf
                        }
                        else {
                            $res.UnknownBlocks.Add(('Vendor block OUI {0} ({1} bytes)' -f $oui, $p.Count))
                        }
                    }
                }
                4 {   # Speaker Allocation Data Block
                    if ($p.Count -ge 1) {
                        $s = [int]$p[0]
                        $map = @(
                            @{ Bit=0; N='FL/FR' },   @{ Bit=1; N='LFE' },      @{ Bit=2; N='FC' }
                            @{ Bit=3; N='RL/RR' },   @{ Bit=4; N='RC' },       @{ Bit=5; N='FLC/FRC' }
                            @{ Bit=6; N='RLC/RRC' }, @{ Bit=7; N='FLW/FRW' }
                        )
                        foreach ($m in $map) { if (($s -band (1 -shl $m.Bit)) -ne 0) { $res.Speakers.Add($m.N) } }
                    }
                }
                7 {   # Use Extended Tag
                    $ext = [int]$p[0]
                    switch ($ext) {
                        0 {   # Video Capability Data Block
                            if ($p.Count -ge 2) {
                                $res.QuantYcc = (((([int]$p[1]) -shr 7) -band 0x01) -ne 0)
                                $res.QuantRgb = (((([int]$p[1]) -shr 6) -band 0x01) -ne 0)
                            }
                        }
                        5 {   # Colorimetry Data Block
                            if ($p.Count -ge 2) {
                                $c = [int]$p[1]
                                $map = @(
                                    @{ Bit=0; N='xvYCC601' },   @{ Bit=1; N='xvYCC709' }
                                    @{ Bit=2; N='sYCC601' },    @{ Bit=3; N='opYCC601' }
                                    @{ Bit=4; N='opRGB' },      @{ Bit=5; N='BT.2020 cYCC' }
                                    @{ Bit=6; N='BT.2020 YCC' },@{ Bit=7; N='BT.2020 RGB' }
                                )
                                foreach ($m in $map) { if (($c -band (1 -shl $m.Bit)) -ne 0) { $res.Colorimetry.Add($m.N) } }
                            }
                        }
                        6 {   # HDR Static Metadata Data Block
                            if ($p.Count -ge 2) {
                                $e = [int]$p[1]
                                if (($e -band 0x01) -ne 0) { $res.HdrEotf.Add('SDR (traditional gamma)') }
                                if (($e -band 0x02) -ne 0) { $res.HdrEotf.Add('HDR (traditional gamma)') }
                                if (($e -band 0x04) -ne 0) { $res.HdrEotf.Add('PQ (SMPTE ST 2084)') }
                                if (($e -band 0x08) -ne 0) { $res.HdrEotf.Add('HLG') }
                            }
                            # Optional desired luminance, coded per CTA-861:
                            #   max = 50 * 2^(v/32);  min = max * (v/255)^2 / 100
                            if ($p.Count -ge 4 -and [int]$p[3] -gt 0) {
                                $res.HdrMaxLum = [math]::Round(50 * [math]::Pow(2, ([int]$p[3]) / 32.0), 0)
                            }
                            if ($p.Count -ge 5 -and [int]$p[4] -gt 0) {
                                $res.HdrAvgLum = [math]::Round(50 * [math]::Pow(2, ([int]$p[4]) / 32.0), 0)
                            }
                            if ($p.Count -ge 6 -and $res.HdrMaxLum) {
                                $res.HdrMinLum = [math]::Round($res.HdrMaxLum * [math]::Pow((([int]$p[5]) / 255.0), 2) / 100.0, 4)
                            }
                        }
                        14 {  # YCbCr 4:2:0 Video Data Block (these VICs are 4:2:0 ONLY)
                            for ($k = 1; $k -lt $p.Count; $k++) {
                                $res.Video420.Add([PSCustomObject]@{ Vic = [int]$p[$k]; Native = $false })
                            }
                        }
                        default { $res.UnknownBlocks.Add(('Extended tag {0} ({1} bytes)' -f $ext, $p.Count)) }
                    }
                }
                default { $res.UnknownBlocks.Add(('Data block tag {0} ({1} bytes)' -f $tag, $p.Count)) }
            }
            $i = $end + 1
        }
    }

    # --- detailed timings (from byte d to 126) ---
    if ($d -ge 4) {
        for ($t = $d; ($t + 17) -le 126; $t += 18) {
            if ($B[$t] -eq 0 -and $B[$t+1] -eq 0) { break }
            $dtd = ConvertFrom-EDIDDetailedTiming $B[$t..($t + 17)]
            if ($dtd) { $res.Timings.Add($dtd) }
        }
    }

    return $res
}

# ------------------------------------------------------------
#  Full blob -> parsed object
# ------------------------------------------------------------
function ConvertFrom-EDID {
    param([byte[]]$Bytes)

    if ($null -eq $Bytes -or $Bytes.Count -lt 128) {
        throw "EDID blob too short ($(if ($Bytes) { $Bytes.Count } else { 0 }) bytes, need at least 128)."
    }

    $blockCount = [math]::Floor($Bytes.Count / 128)
    $checks = New-Object System.Collections.Generic.List[object]
    for ($b = 0; $b -lt $blockCount; $b++) {
        $sum = 0
        for ($k = 0; $k -lt 128; $k++) { $sum += [int]$Bytes[($b * 128) + $k] }
        $checks.Add([PSCustomObject]@{ Block = $b; Valid = (($sum % 256) -eq 0) })
    }

    $base = ConvertFrom-EDIDBaseBlock $Bytes[0..127]
    $exts = New-Object System.Collections.Generic.List[object]
    for ($b = 1; $b -lt $blockCount; $b++) {
        $blk = $Bytes[($b * 128)..(($b * 128) + 127)]
        if ($blk[0] -eq 0x02) {
            $cta = ConvertFrom-EDIDCtaBlock $blk
            $exts.Add([PSCustomObject]@{ Index = $b; Type = 'CTA-861'; Tag = $blk[0]; Data = $cta })
        }
        else {
            $tagName = switch ([int]$blk[0]) {
                0x10    { 'Video Timing Block' }
                0x40    { 'Display Information' }
                0x50    { 'Localised String' }
                0x60    { 'Microdisplay' }
                0x70    { 'DisplayID' }
                0xF0    { 'Block Map' }
                default { 'Unknown' }
            }
            $exts.Add([PSCustomObject]@{ Index = $b; Type = $tagName; Tag = $blk[0]; Data = $null })
        }
    }

    return [PSCustomObject]@{
        Raw        = $Bytes
        BlockCount = $blockCount
        Base       = $base
        Extensions = $exts
        Checksums  = $checks
    }
}


# ------------------------------------------------------------
#  Device enumeration
# ------------------------------------------------------------

# Read the full EDID blob (base + extensions) for one PnP instance.
function Get-EDIDBytesFromRegistry {
    param([string]$InstanceId)   # e.g. DISPLAY\GSM5B09\5&2b6e2b7&0&UID4353
    $key = "HKLM:\SYSTEM\CurrentControlSet\Enum\$InstanceId\Device Parameters"
    try {
        $v = Get-ItemProperty -LiteralPath $key -Name 'EDID' -ErrorAction Stop
        if ($v.EDID) { return [byte[]]$v.EDID }
    } catch { }
    return $null
}

# Fallback: ask the display driver for each 128-byte block.
function Get-EDIDBytesFromWmi {
    param([string]$InstanceName)  # WMI form, keeps the _0 suffix
    try {
        $m = Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorDescriptorMethods -ErrorAction Stop |
             Where-Object { $_.InstanceName -eq $InstanceName } | Select-Object -First 1
        if (-not $m) { return $null }
        $out = New-Object System.Collections.Generic.List[byte]
        $r0 = Invoke-CimMethod -InputObject $m -MethodName 'WmiGetMonitorRawEEdidV1Block' `
                               -Arguments @{ BlockId = [uint32]0 } -ErrorAction Stop
        if (-not $r0.BlockContent) { return $null }
        $out.AddRange([byte[]]$r0.BlockContent)
        $extCount = [int]$out[126]
        for ($b = 1; $b -le $extCount; $b++) {
            $rb = Invoke-CimMethod -InputObject $m -MethodName 'WmiGetMonitorRawEEdidV1Block' `
                                   -Arguments @{ BlockId = [uint32]$b } -ErrorAction Stop
            if ($rb.BlockContent) { $out.AddRange([byte[]]$rb.BlockContent) }
        }
        return $out.ToArray()
    } catch { }
    return $null
}

# Enumerate displays. Default = currently attached (via WmiMonitorID).
# -Remembered also returns every display still recorded in the registry
# (useful to look up a projector that was plugged in yesterday).
function Get-EDIDDevice {
    [CmdletBinding()]
    param([switch]$Remembered)

    $list = New-Object System.Collections.Generic.List[object]
    $seen = New-Object System.Collections.Generic.HashSet[string]

    $ids = @()
    try {
        $ids = @(Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction Stop |
                 Sort-Object InstanceName)
    } catch { }

    foreach ($id in $ids) {
        $instId = $id.InstanceName -replace '_\d+$', ''
        $bytes  = Get-EDIDBytesFromRegistry -InstanceId $instId
        $src    = 'registry'
        if (-not $bytes) {
            $bytes = Get-EDIDBytesFromWmi -InstanceName $id.InstanceName
            $src   = 'wmi'
        }
        if (-not $bytes) { $src = 'unavailable' }
        [void]$seen.Add($instId.ToUpper())
        $list.Add([PSCustomObject]@{
            InstanceId = $instId
            Connected  = $true
            Source     = $src
            Bytes      = $bytes
        })
    }

    if ($Remembered) {
        try {
            $root = 'HKLM:\SYSTEM\CurrentControlSet\Enum\DISPLAY'
            foreach ($mon in (Get-ChildItem -LiteralPath $root -ErrorAction Stop)) {
                foreach ($inst in (Get-ChildItem -LiteralPath $mon.PSPath -ErrorAction SilentlyContinue)) {
                    $instId = 'DISPLAY\{0}\{1}' -f $mon.PSChildName, $inst.PSChildName
                    if ($seen.Contains($instId.ToUpper())) { continue }
                    $bytes = Get-EDIDBytesFromRegistry -InstanceId $instId
                    if (-not $bytes) { continue }
                    [void]$seen.Add($instId.ToUpper())
                    $list.Add([PSCustomObject]@{
                        InstanceId = $instId
                        Connected  = $false
                        Source     = 'registry'
                        Bytes      = $bytes
                    })
                }
            }
        } catch { }
    }

    $i = 1
    foreach ($item in $list) {
        Add-Member -InputObject $item -NotePropertyName 'Index' -NotePropertyValue $i -Force
        $edid = $null
        if ($item.Bytes) { try { $edid = ConvertFrom-EDID $item.Bytes } catch { } }
        Add-Member -InputObject $item -NotePropertyName 'Edid' -NotePropertyValue $edid -Force
        $i++
    }
    return $list
}

# Accepts "1" (live display index) or a path to a .bin/.dat EDID file.
function Resolve-EDIDSource {
    param([Parameter(Mandatory)][string]$Source, [switch]$Remembered)

    $s = $Source.Trim().Trim('"')
    $n = 0
    if ([int]::TryParse($s, [ref]$n)) {
        $devs = Get-EDIDDevice -Remembered:$Remembered
        $dev  = $devs | Where-Object { $_.Index -eq $n } | Select-Object -First 1
        if (-not $dev)      { throw "No display with index $n. Run 'edid-list' to see them." }
        if (-not $dev.Bytes){ throw "Display $n reported no EDID (source: $($dev.Source))." }
        return [PSCustomObject]@{ Name = "display $n"; Bytes = $dev.Bytes; Edid = $dev.Edid; Device = $dev }
    }

    if (-not (Test-Path -LiteralPath $s)) { throw "File not found: $s" }
    $bytes = [System.IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $s).Path)
    return [PSCustomObject]@{ Name = (Split-Path -Leaf $s); Bytes = $bytes; Edid = (ConvertFrom-EDID $bytes); Device = $null }
}


# ------------------------------------------------------------
#  edid-list : one-line summary of every display
# ------------------------------------------------------------
function Get-EDIDList {
    [CmdletBinding()]
    param([switch]$Remembered)

    Write-Host "`n=== EDID - DISPLAYS ===" -ForegroundColor Cyan
    Write-Host ""

    $devs = Get-EDIDDevice -Remembered:$Remembered
    if (-not $devs -or $devs.Count -eq 0) {
        Write-Host "  No displays found (WmiMonitorID returned nothing)." -ForegroundColor Yellow
        Write-Host "  Some virtual / remote sessions do not expose an EDID." -ForegroundColor DarkGray
        Write-Host ""
        return
    }

    Write-Host ("   {0,-2} {1,-4} {2,-22} {3,-6} {4,-5} {5,-20} {6}" -f
        '#', 'MFG', 'MODEL', 'YEAR', 'EDID', 'PREFERRED TIMING', 'CAPS') -ForegroundColor DarkGray
    Write-Host ("   " + ("-" * 92)) -ForegroundColor DarkGray

    foreach ($d in $devs) {
        if (-not $d.Edid) {
            Write-Host ("   {0,-2} {1}" -f $d.Index, "EDID unavailable ($($d.Source))") -ForegroundColor Red
            Write-Host ("      {0}" -f $d.InstanceId) -ForegroundColor DarkGray
            continue
        }
        $b = $d.Edid.Base
        $model = $b.MonitorName
        if (-not $model) { $model = ('0x{0:X4}' -f $b.ProductCode) }
        if ($model.Length -gt 22) { $model = $model.Substring(0, 21) + '.' }

        $pref = ''
        if ($b.Timings.Count -gt 0) { $pref = $b.Timings[0].Summary }

        # Capability shorthand that matters on site
        $caps = New-Object System.Collections.Generic.List[string]
        $cta = ($d.Edid.Extensions | Where-Object { $_.Type -eq 'CTA-861' } | Select-Object -First 1)
        if ($cta) {
            $c = $cta.Data
            $caps.Add('CTA' + $c.Revision)
            if ($c.HdmiForum -and $c.HdmiForum.MaxTmdsCharRateMHz) { $caps.Add("HDMI2 $($c.HdmiForum.MaxTmdsCharRateMHz)MHz") }
            elseif ($c.Hdmi -and $c.Hdmi.MaxTmdsMHz)               { $caps.Add("HDMI $($c.Hdmi.MaxTmdsMHz)MHz") }
            elseif ($c.Hdmi)                                       { $caps.Add('HDMI') }
            if ($c.BasicAudio)      { $caps.Add('audio') }
            if ($c.HdrEotf.Count -gt 0) {
                $hdr = @($c.HdrEotf | Where-Object { $_ -match 'PQ|HLG' } | ForEach-Object { ($_ -split ' ')[0] })
                if ($hdr.Count) { $caps.Add('HDR ' + ($hdr -join '/')) }
            }
        } else {
            $caps.Add('no CTA (DVI/VGA-style)')
        }

        $colour = if ($d.Connected) { 'Green' } else { 'DarkGray' }
        Write-Host ("   {0,-2} {1,-4} {2,-22} {3,-6} {4,-5} {5,-20} {6}" -f
            $d.Index, $b.Manufacturer, $model, $b.Year, $b.Version, $pref, ($caps -join ', ')) -ForegroundColor $colour

        $tail = $d.InstanceId
        if (-not $d.Connected) { $tail += '   [not connected]' }
        Write-Host ("      {0}" -f $tail) -ForegroundColor DarkGray
    }

    Write-Host ""
    Write-Host "  edid-info <n>   full decode        edid-dump <n>   save .bin" -ForegroundColor DarkGray
    Write-Host "  edid-check <n>  validate           edid-diff a b   compare" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name edid-list -Value Get-EDIDList


# ------------------------------------------------------------
#  edid-info : full decode of one display or .bin file
# ------------------------------------------------------------
function Show-EDIDInfo {
    [CmdletBinding()]
    param([Parameter(Position = 0)][string]$Source = '1', [switch]$Remembered)

    try { $src = Resolve-EDIDSource -Source $Source -Remembered:$Remembered }
    catch { Write-Host "`n  ERROR: $($_.Exception.Message)`n" -ForegroundColor Red; return }

    $e = $src.Edid
    $b = $e.Base

    Write-Host ("`n=== EDID - {0} ===" -f $src.Name.ToUpper()) -ForegroundColor Cyan
    Write-Host ""

    # --- identity ---
    Write-Host "  --- Identity ---" -ForegroundColor Yellow
    $mfgLine = $b.Manufacturer
    if ($b.VendorName) { $mfgLine += "  ($($b.VendorName))" }
    Write-EDIDField 'Manufacturer:' $mfgLine
    if ($b.MonitorName) { Write-EDIDField 'Model name:' $b.MonitorName }
    Write-EDIDField 'Product code:'  ('0x{0:X4}  ({0})' -f $b.ProductCode)
    Write-EDIDField 'Serial number:' ("$($b.SerialNumber)")
    if ($b.SerialText) { Write-EDIDField 'Serial (text):' $b.SerialText }
    foreach ($t in $b.TextFields) { Write-EDIDField 'Text:' $t 'DarkGray' }
    $mfgDate = if ($b.Week -ge 1 -and $b.Week -le 54) { "week $($b.Week) / $($b.Year)" } else { "$($b.Year)" }
    Write-EDIDField 'Manufactured:'  $mfgDate
    Write-EDIDField 'EDID version:'  $b.Version
    Write-EDIDField 'Blocks:'        ("{0}  (base + {1} extension)" -f $e.BlockCount, ($e.BlockCount - 1))
    Write-Host ""

    # --- display parameters ---
    Write-Host "  --- Display parameters ---" -ForegroundColor Yellow
    Write-EDIDField 'Input:' $b.InputDesc
    if ($b.WidthCm -gt 0) {
        Write-EDIDField 'Screen size:' ('{0} x {1} cm   ({2}")' -f $b.WidthCm, $b.HeightCm, $b.DiagonalIn)
    } else {
        Write-EDIDField 'Screen size:' 'not reported' 'DarkGray'
    }
    if ($b.Gamma) { Write-EDIDField 'Gamma:' ('{0}' -f $b.Gamma) }
    Write-EDIDField 'Colour formats:' $b.ColourType
    if ($b.Features.Count) { Write-EDIDField 'Features:' ($b.Features -join ', ') 'DarkGray' }
    Write-Host ""

    # --- timings ---
    if ($b.Timings.Count -gt 0) {
        Write-Host "  --- Preferred timing (base block) ---" -ForegroundColor Yellow
        $t = $b.Timings[0]
        Write-Host ("    {0}      pixel clock {1} MHz" -f $t.Summary, $t.PixelClockMHz) -ForegroundColor Green
        Write-Host ("    H: {0,5} active {1,4} blank   front {2,4}  sync {3,4}  ({4})" -f
            $t.HActive, $t.HBlank, $t.HFrontPorch, $t.HSyncWidth, $t.HPolarity) -ForegroundColor DarkGray
        Write-Host ("    V: {0,5} active {1,4} blank   front {2,4}  sync {3,4}  ({4})" -f
            $t.VActive, $t.VBlank, $t.VFrontPorch, $t.VSyncWidth, $t.VPolarity) -ForegroundColor DarkGray
        if ($t.WidthMM -gt 0) {
            Write-Host ("    Image size: {0} x {1} mm" -f $t.WidthMM, $t.HeightMM) -ForegroundColor DarkGray
        }
        Write-Host ""

        if ($b.Timings.Count -gt 1) {
            Write-Host "  --- Other detailed timings (base block) ---" -ForegroundColor Yellow
            for ($i = 1; $i -lt $b.Timings.Count; $i++) {
                Write-Host ("    {0,-28} {1} MHz" -f $b.Timings[$i].Summary, $b.Timings[$i].PixelClockMHz)
            }
            Write-Host ""
        }
    }

    if ($b.Range) {
        Write-Host "  --- Range limits ---" -ForegroundColor Yellow
        Write-Host ("    V: {0}-{1} Hz    H: {2}-{3} kHz    Max pixel clock: {4} MHz" -f
            $b.Range.VMin, $b.Range.VMax, $b.Range.HMin, $b.Range.HMax, $b.Range.MaxClockMHz)
        Write-Host ""
    }

    if ($b.Established.Count -or $b.Standard.Count) {
        Write-Host "  --- Established / standard timings ---" -ForegroundColor Yellow
        if ($b.Established.Count) { Write-Host ("    " + ($b.Established -join ', ')) -ForegroundColor DarkGray }
        if ($b.Standard.Count)    { Write-Host ("    " + ($b.Standard    -join ', ')) }
        Write-Host ""
    }

    # --- extensions ---
    foreach ($ext in $e.Extensions) {
        if ($ext.Type -ne 'CTA-861') {
            Write-Host ("  --- Extension {0}: {1} (tag 0x{2:X2}) ---" -f $ext.Index, $ext.Type, $ext.Tag) -ForegroundColor Yellow
            Write-Host "    Not decoded by this tool." -ForegroundColor DarkGray
            Write-Host ""
            continue
        }
        $c = $ext.Data
        Write-Host ("  --- CTA-861 extension (block {0}, rev {1}) ---" -f $ext.Index, $c.Revision) -ForegroundColor Yellow
        Write-EDIDField 'Basic audio:'   $(if ($c.BasicAudio) { 'yes' } else { 'no' })  $(if ($c.BasicAudio) { 'Green' } else { 'DarkGray' })
        Write-EDIDField 'YCbCr:'         ('4:4:4 {0}   4:2:2 {1}' -f
            $(if ($c.YCbCr444) { 'yes' } else { 'no' }), $(if ($c.YCbCr422) { 'yes' } else { 'no' }))
        Write-EDIDField 'Underscan IT:'  $(if ($c.UnderscanIT) { 'yes' } else { 'no' }) 'DarkGray'
        if ($c.QuantRgb -or $c.QuantYcc) {
            Write-EDIDField 'Quant. range:' ('RGB selectable: {0}   YCC selectable: {1}' -f
                $(if ($c.QuantRgb) { 'yes' } else { 'no' }), $(if ($c.QuantYcc) { 'yes' } else { 'no' }))
        }

        if ($c.Video.Count) {
            Write-Host "    Video (SVDs, * = native):" -ForegroundColor DarkGray
            $line = New-Object System.Collections.Generic.List[string]
            foreach ($v in $c.Video) {
                $name = if ($script:EDIDVic.ContainsKey($v.Vic)) { $script:EDIDVic[$v.Vic] } else { "VIC $($v.Vic)" }
                $line.Add(('{0}{1,-3} {2}' -f $(if ($v.Native) { '*' } else { ' ' }), $v.Vic, $name))
            }
            for ($i = 0; $i -lt $line.Count; $i += 3) {
                $chunk = $line[$i..([math]::Min($i + 2, $line.Count - 1))]
                Write-Host ("      " + (($chunk | ForEach-Object { '{0,-26}' -f $_ }) -join '')) -ForegroundColor Green
            }
        }
        if ($c.Video420.Count) {
            Write-Host "    YCbCr 4:2:0 only:" -ForegroundColor DarkGray
            $l420 = @($c.Video420 | ForEach-Object {
                $n = if ($script:EDIDVic.ContainsKey($_.Vic)) { $script:EDIDVic[$_.Vic] } else { "VIC $($_.Vic)" }
                '{0} {1}' -f $_.Vic, $n
            })
            Write-Host ("      " + ($l420 -join ',  ')) -ForegroundColor DarkYellow
        }
        if ($c.Audio.Count) {
            Write-Host "    Audio (SADs):" -ForegroundColor DarkGray
            foreach ($a in $c.Audio) { Write-Host ("      $a") }
        }
        if ($c.Speakers.Count) { Write-EDIDField 'Speakers:' ($c.Speakers -join ', ') }

        if ($c.Hdmi) {
            Write-Host "    HDMI vendor block (00-0C-03):" -ForegroundColor DarkGray
            if ($c.Hdmi.PhysicalAddress) { Write-Host ("      Source physical address: {0}" -f $c.Hdmi.PhysicalAddress) }
            if ($c.Hdmi.MaxTmdsMHz)      { Write-Host ("      Max TMDS clock:          {0} MHz" -f $c.Hdmi.MaxTmdsMHz) }
            if ($c.Hdmi.DeepColour.Count){ Write-Host ("      Deep colour:             {0}" -f ($c.Hdmi.DeepColour -join ', ')) }
        }
        if ($c.HdmiForum) {
            Write-Host "    HDMI Forum block (C4-5D-D8, HDMI 2.x):" -ForegroundColor DarkGray
            if ($c.HdmiForum.MaxTmdsCharRateMHz) {
                Write-Host ("      Max TMDS character rate: {0} MHz" -f $c.HdmiForum.MaxTmdsCharRateMHz) -ForegroundColor Green
            }
            Write-Host ("      SCDC:                    {0}" -f $(if ($c.HdmiForum.Scdc) { 'yes' } else { 'no' }))
            if ($c.HdmiForum.MaxFrlRate) { Write-Host ("      Max FRL rate:            {0}" -f $c.HdmiForum.MaxFrlRate) }
        }
        if ($c.HdrEotf.Count) {
            Write-Host "    HDR static metadata:" -ForegroundColor DarkGray
            Write-Host ("      EOTF: {0}" -f ($c.HdrEotf -join ', ')) -ForegroundColor Green
            if ($c.HdrMaxLum) {
                $lum = "      Luminance: max $($c.HdrMaxLum) cd/m2"
                if ($c.HdrAvgLum) { $lum += "   frame-avg $($c.HdrAvgLum)" }
                if ($c.HdrMinLum) { $lum += "   min $($c.HdrMinLum)" }
                Write-Host $lum
            }
        }
        if ($c.Colorimetry.Count) { Write-EDIDField 'Colorimetry:' ($c.Colorimetry -join ', ') }

        if ($c.Timings.Count) {
            Write-Host "    Detailed timings in this block:" -ForegroundColor DarkGray
            foreach ($t in $c.Timings) { Write-Host ("      {0,-28} {1} MHz" -f $t.Summary, $t.PixelClockMHz) }
        }
        if ($c.UnknownBlocks.Count) {
            foreach ($u in $c.UnknownBlocks) { Write-Host ("    (not decoded) $u") -ForegroundColor DarkGray }
        }
        Write-Host ""
    }

    # --- integrity ---
    Write-Host "  --- Integrity ---" -ForegroundColor Yellow
    Write-EDIDField 'Header:' $(if ($b.HeaderValid) { 'OK' } else { 'INVALID' }) `
                              $(if ($b.HeaderValid) { 'Green' } else { 'Red' })
    foreach ($chk in $e.Checksums) {
        $lbl = if ($chk.Block -eq 0) { 'Base checksum:' } else { "Ext $($chk.Block) checksum:" }
        Write-EDIDField $lbl $(if ($chk.Valid) { 'OK' } else { 'BAD' }) $(if ($chk.Valid) { 'Green' } else { 'Red' })
    }
    Write-Host ""
}
Set-Alias -Name edid-info -Value Show-EDIDInfo


# ------------------------------------------------------------
#  edid-dump : save the raw EDID to a .bin
# ------------------------------------------------------------
function Export-EDIDBinary {
    [CmdletBinding()]
    param(
        [Parameter(Position = 0)][string]$Source = '1',
        [Parameter(Position = 1)][string]$Path,
        [switch]$All,
        [switch]$Remembered
    )

    if (-not $Path) { $Path = Get-EDIDOutputPath }
    $Path = $Path.Trim().Trim('"')
    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path -Force | Out-Null
    }

    $targets = @()
    if ($All -or $Remembered) {
        $targets = @(Get-EDIDDevice -Remembered:$Remembered | Where-Object { $_.Bytes } | ForEach-Object {
            [PSCustomObject]@{ Name = "display $($_.Index)"; Tag = "$($_.Index)"; Bytes = $_.Bytes; Edid = $_.Edid }
        })
    } else {
        try {
            $one = Resolve-EDIDSource -Source $Source -Remembered:$Remembered
            $tag = if ($one.Device) { "$($one.Device.Index)" } else { $null }
            $targets = @([PSCustomObject]@{ Name = $one.Name; Tag = $tag; Bytes = $one.Bytes; Edid = $one.Edid })
        }
        catch { Write-Host "`n  ERROR: $($_.Exception.Message)`n" -ForegroundColor Red; return }
    }

    Write-Host "`n=== EDID DUMP ===" -ForegroundColor Cyan
    Write-Host ""
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'

    foreach ($t in $targets) {
        $mfg = 'UNKNOWN'; $model = 'display'
        if ($t.Edid) {
            $mfg = $t.Edid.Base.Manufacturer
            if ($t.Edid.Base.MonitorName) { $model = $t.Edid.Base.MonitorName }
            else { $model = '{0:X4}' -f $t.Edid.Base.ProductCode }
        }
        # The display index goes in the name: the same panel can appear on two
        # outputs, and without it a multi-dump would silently overwrite itself.
        $safe = ('{0}_{1}' -f $mfg, $model) -replace '[^\w\-\.]', '_'
        if ($t.Tag) { $safe = '{0}_out{1}' -f $safe, $t.Tag }
        $file = Join-Path $Path ('{0}_{1}.bin' -f $safe, $stamp)
        $n = 2
        while (Test-Path -LiteralPath $file) {
            $file = Join-Path $Path ('{0}_{1}_{2}.bin' -f $safe, $stamp, $n)
            $n++
        }

        try {
            [System.IO.File]::WriteAllBytes($file, [byte[]]$t.Bytes)
            Write-Host ("  {0,-14} -> {1}  ({2} bytes)" -f $t.Name, $file, $t.Bytes.Count) -ForegroundColor Green
        } catch {
            Write-Host ("  {0,-14} FAILED: {1}" -f $t.Name, $_.Exception.Message) -ForegroundColor Red
        }
    }
    Write-Host ""
    Write-Host "  Load these in CRU / AW EDID Editor, or push them to a matrix or EDID emulator." -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name edid-dump -Value Export-EDIDBinary


# ------------------------------------------------------------
#  edid-check : sanity / integrity validation
# ------------------------------------------------------------
function Test-EDIDIntegrity {
    [CmdletBinding()]
    param([Parameter(Position = 0)][string]$Source, [switch]$Remembered)

    $targets = @()
    if ($Source) {
        try { $targets = @(Resolve-EDIDSource -Source $Source -Remembered:$Remembered) }
        catch { Write-Host "`n  ERROR: $($_.Exception.Message)`n" -ForegroundColor Red; return }
    } else {
        $targets = @(Get-EDIDDevice -Remembered:$Remembered | Where-Object { $_.Bytes } | ForEach-Object {
            [PSCustomObject]@{ Name = "display $($_.Index)"; Bytes = $_.Bytes; Edid = $_.Edid }
        })
    }

    Write-Host "`n=== EDID CHECK ===" -ForegroundColor Cyan

    foreach ($t in $targets) {
        Write-Host ""
        Write-Host ("  --- {0} ---" -f $t.Name) -ForegroundColor Yellow
        $e = $t.Edid
        if (-not $e) { Write-Host "    Could not parse this EDID." -ForegroundColor Red; continue }
        $b = $e.Base
        $issues = New-Object System.Collections.Generic.List[string]
        $warns  = New-Object System.Collections.Generic.List[string]

        if (-not $b.HeaderValid) { $issues.Add('Invalid 00 FF FF FF FF FF FF 00 header') }
        foreach ($chk in $e.Checksums) {
            if (-not $chk.Valid) { $issues.Add("Block $($chk.Block) checksum is wrong") }
        }
        if ($b.ExtCount -ne ($e.BlockCount - 1)) {
            $issues.Add("Byte 126 says $($b.ExtCount) extension block(s) but $($e.BlockCount - 1) are present")
        }
        if ($b.Version -notin @('1.3', '1.4')) { $warns.Add("Unusual EDID version $($b.Version)") }
        if ($b.Year -lt 1990 -or $b.Year -gt ((Get-Date).Year + 1)) {
            $warns.Add("Implausible manufacture year $($b.Year)")
        }
        if ($b.Timings.Count -eq 0) { $issues.Add('No detailed timing in the base block (no native resolution declared)') }
        if (-not $b.MonitorName)    { $warns.Add('No monitor name descriptor (0xFC)') }
        if ($b.WidthCm -eq 0)       { $warns.Add('Physical size not reported (scaling/DPI may be wrong)') }

        # Preferred timing vs declared limits - the classic "why no 4K60" case
        if ($b.Timings.Count -gt 0) {
            $pref = $b.Timings[0]
            if ($b.Range -and $b.Range.MaxClockMHz -gt 0 -and $pref.PixelClockMHz -gt $b.Range.MaxClockMHz) {
                $issues.Add(("Preferred timing needs {0} MHz but range limits cap at {1} MHz" -f
                    $pref.PixelClockMHz, $b.Range.MaxClockMHz))
            }
            if ($b.Range -and ($pref.Refresh -lt $b.Range.VMin -or $pref.Refresh -gt $b.Range.VMax)) {
                $warns.Add(("Preferred refresh {0} Hz is outside the declared {1}-{2} Hz range" -f
                    $pref.Refresh, $b.Range.VMin, $b.Range.VMax))
            }
            $cta = ($e.Extensions | Where-Object { $_.Type -eq 'CTA-861' } | Select-Object -First 1)
            if ($cta) {
                $maxRate = $null
                if ($cta.Data.HdmiForum -and $cta.Data.HdmiForum.MaxTmdsCharRateMHz) { $maxRate = $cta.Data.HdmiForum.MaxTmdsCharRateMHz }
                elseif ($cta.Data.Hdmi -and $cta.Data.Hdmi.MaxTmdsMHz)               { $maxRate = $cta.Data.Hdmi.MaxTmdsMHz }
                if ($maxRate -and $pref.PixelClockMHz -gt $maxRate) {
                    $warns.Add(("Preferred timing is {0} MHz but the HDMI block declares max {1} MHz - " +
                                "needs 4:2:0, or the sink expects DisplayPort/FRL") -f $pref.PixelClockMHz, $maxRate)
                }
                # 600 MHz is the HDMI 2.0 TMDS ceiling; anything above it in a
                # TMDS field is a lying dongle, a splitter or a corrupted block.
                if ($maxRate -and $maxRate -gt 600) {
                    $warns.Add("HDMI block declares $maxRate MHz max TMDS, above the 600 MHz spec limit - suspect EDID")
                }
            }
        }

        if ($issues.Count -eq 0 -and $warns.Count -eq 0) {
            Write-Host "    No problems found." -ForegroundColor Green
        }
        foreach ($i in $issues) { Write-Host ("    [ERROR] {0}" -f $i) -ForegroundColor Red }
        foreach ($w in $warns)  { Write-Host ("    [WARN ] {0}" -f $w) -ForegroundColor Yellow }
    }
    Write-Host ""
}
Set-Alias -Name edid-check -Value Test-EDIDIntegrity


# ------------------------------------------------------------
#  edid-diff : compare two EDIDs (display index and/or .bin file)
# ------------------------------------------------------------
function Compare-EDIDSource {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory, Position = 0)][string]$A,
        [Parameter(Mandatory, Position = 1)][string]$B,
        [switch]$Bytes,
        [switch]$Remembered
    )

    try { $x = Resolve-EDIDSource -Source $A -Remembered:$Remembered } catch { Write-Host "`n  ERROR (A): $($_.Exception.Message)`n" -ForegroundColor Red; return }
    try { $y = Resolve-EDIDSource -Source $B -Remembered:$Remembered } catch { Write-Host "`n  ERROR (B): $($_.Exception.Message)`n" -ForegroundColor Red; return }

    Write-Host "`n=== EDID DIFF ===" -ForegroundColor Cyan
    Write-Host ("  A: {0}   ({1} bytes)" -f $x.Name, $x.Bytes.Count) -ForegroundColor DarkGray
    Write-Host ("  B: {0}   ({1} bytes)" -f $y.Name, $y.Bytes.Count) -ForegroundColor DarkGray
    Write-Host ""

    # Field-level comparison of the things that actually break a show
    $rows = New-Object System.Collections.Generic.List[object]
    function Add-DiffRow { param($Label, $ValA, $ValB)
        $rows.Add([PSCustomObject]@{ Label = $Label; A = "$ValA"; B = "$ValB" })
    }

    $ba = $x.Edid.Base; $bb = $y.Edid.Base
    Add-DiffRow 'Manufacturer'    $ba.Manufacturer $bb.Manufacturer
    Add-DiffRow 'Model'           $ba.MonitorName  $bb.MonitorName
    Add-DiffRow 'Product code'    ('0x{0:X4}' -f $ba.ProductCode) ('0x{0:X4}' -f $bb.ProductCode)
    Add-DiffRow 'Serial'          $ba.SerialNumber $bb.SerialNumber
    Add-DiffRow 'EDID version'    $ba.Version      $bb.Version
    Add-DiffRow 'Extension blocks' ($x.Edid.BlockCount - 1) ($y.Edid.BlockCount - 1)
    Add-DiffRow 'Input'           $ba.InputDesc    $bb.InputDesc
    Add-DiffRow 'Colour formats'  $ba.ColourType   $bb.ColourType
    Add-DiffRow 'Preferred timing' $(if ($ba.Timings.Count) { $ba.Timings[0].Summary } else { '-' }) `
                                   $(if ($bb.Timings.Count) { $bb.Timings[0].Summary } else { '-' })
    Add-DiffRow 'Preferred clock' $(if ($ba.Timings.Count) { "$($ba.Timings[0].PixelClockMHz) MHz" } else { '-' }) `
                                  $(if ($bb.Timings.Count) { "$($bb.Timings[0].PixelClockMHz) MHz" } else { '-' })
    Add-DiffRow 'Range limits'    $(if ($ba.Range) { "$($ba.Range.VMin)-$($ba.Range.VMax) Hz, max $($ba.Range.MaxClockMHz) MHz" } else { '-' }) `
                                  $(if ($bb.Range) { "$($bb.Range.VMin)-$($bb.Range.VMax) Hz, max $($bb.Range.MaxClockMHz) MHz" } else { '-' })

    $ca = ($x.Edid.Extensions | Where-Object { $_.Type -eq 'CTA-861' } | Select-Object -First 1)
    $cb = ($y.Edid.Extensions | Where-Object { $_.Type -eq 'CTA-861' } | Select-Object -First 1)
    $da = $(if ($ca) { $ca.Data } else { $null })
    $db = $(if ($cb) { $cb.Data } else { $null })

    Add-DiffRow 'CTA-861'      $(if ($da) { "rev $($da.Revision)" } else { 'absent' }) `
                               $(if ($db) { "rev $($db.Revision)" } else { 'absent' })
    Add-DiffRow 'Basic audio'  $(if ($da) { $da.BasicAudio } else { '-' })  $(if ($db) { $db.BasicAudio } else { '-' })
    Add-DiffRow 'YCbCr 4:4:4'  $(if ($da) { $da.YCbCr444 }  else { '-' })  $(if ($db) { $db.YCbCr444 }  else { '-' })
    Add-DiffRow 'YCbCr 4:2:2'  $(if ($da) { $da.YCbCr422 }  else { '-' })  $(if ($db) { $db.YCbCr422 }  else { '-' })
    Add-DiffRow 'Max TMDS'     $(if ($da -and $da.HdmiForum -and $da.HdmiForum.MaxTmdsCharRateMHz) { "$($da.HdmiForum.MaxTmdsCharRateMHz) MHz (HF)" }
                                 elseif ($da -and $da.Hdmi -and $da.Hdmi.MaxTmdsMHz) { "$($da.Hdmi.MaxTmdsMHz) MHz" } else { '-' }) `
                               $(if ($db -and $db.HdmiForum -and $db.HdmiForum.MaxTmdsCharRateMHz) { "$($db.HdmiForum.MaxTmdsCharRateMHz) MHz (HF)" }
                                 elseif ($db -and $db.Hdmi -and $db.Hdmi.MaxTmdsMHz) { "$($db.Hdmi.MaxTmdsMHz) MHz" } else { '-' })
    Add-DiffRow 'HDR EOTF'     $(if ($da -and $da.HdrEotf.Count) { $da.HdrEotf -join ', ' } else { 'none' }) `
                               $(if ($db -and $db.HdrEotf.Count) { $db.HdrEotf -join ', ' } else { 'none' })
    Add-DiffRow 'Colorimetry'  $(if ($da -and $da.Colorimetry.Count) { $da.Colorimetry -join ', ' } else { 'none' }) `
                               $(if ($db -and $db.Colorimetry.Count) { $db.Colorimetry -join ', ' } else { 'none' })

    $vicA = @(); $vicB = @()
    if ($da) { $vicA = @($da.Video | ForEach-Object { $_.Vic }) }
    if ($db) { $vicB = @($db.Video | ForEach-Object { $_.Vic }) }

    Write-Host ("  {0,-18} {1,-32} {2}" -f 'FIELD', 'A', 'B') -ForegroundColor DarkGray
    Write-Host ("  " + ("-" * 88)) -ForegroundColor DarkGray
    $diffCount = 0
    foreach ($r in $rows) {
        $same = ($r.A -eq $r.B)
        if (-not $same) { $diffCount++ }
        $colour = if ($same) { 'DarkGray' } else { 'Yellow' }
        $mark   = if ($same) { ' ' } else { '!' }
        $va = $r.A; if ($va.Length -gt 31) { $va = $va.Substring(0, 30) + '.' }
        $vb = $r.B; if ($vb.Length -gt 31) { $vb = $vb.Substring(0, 30) + '.' }
        Write-Host ("{0} {1,-18} {2,-32} {3}" -f $mark, $r.Label, $va, $vb) -ForegroundColor $colour
    }
    Write-Host ""

    # Video modes present on one side only - the reason a resolution "disappears"
    $onlyA = @($vicA | Where-Object { $vicB -notcontains $_ })
    $onlyB = @($vicB | Where-Object { $vicA -notcontains $_ })
    if ($onlyA.Count -or $onlyB.Count) {
        Write-Host "  --- Video modes (VIC) present on one side only ---" -ForegroundColor Yellow
        if ($onlyA.Count) {
            $names = @($onlyA | ForEach-Object { if ($script:EDIDVic.ContainsKey($_)) { "$_ $($script:EDIDVic[$_])" } else { "VIC $_" } })
            Write-Host ("    only in A:  " + ($names -join ',  ')) -ForegroundColor Red
        }
        if ($onlyB.Count) {
            $names = @($onlyB | ForEach-Object { if ($script:EDIDVic.ContainsKey($_)) { "$_ $($script:EDIDVic[$_])" } else { "VIC $_" } })
            Write-Host ("    only in B:  " + ($names -join ',  ')) -ForegroundColor Red
        }
        $diffCount += $onlyA.Count + $onlyB.Count
        Write-Host ""
    }

    # Byte-level diff
    $len = [math]::Max($x.Bytes.Count, $y.Bytes.Count)
    $byteDiffs = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -lt $len; $i++) {
        $av = if ($i -lt $x.Bytes.Count) { $x.Bytes[$i] } else { $null }
        $bv = if ($i -lt $y.Bytes.Count) { $y.Bytes[$i] } else { $null }
        if ($av -ne $bv) {
            $byteDiffs.Add(('    byte {0,3} (blk {1} off {2,3}):  {3}  ->  {4}' -f
                $i, [math]::Floor($i / 128), ($i % 128),
                $(if ($null -ne $av) { '0x{0:X2}' -f $av } else { '--' }),
                $(if ($null -ne $bv) { '0x{0:X2}' -f $bv } else { '--' })))
        }
    }

    Write-Host ("  --- Raw bytes: {0} of {1} differ ---" -f $byteDiffs.Count, $len) -ForegroundColor Yellow
    if ($byteDiffs.Count -eq 0) {
        Write-Host "    Identical." -ForegroundColor Green
    }
    elseif ($Bytes) {
        foreach ($d in $byteDiffs) { Write-Host $d -ForegroundColor DarkGray }
    }
    else {
        foreach ($d in ($byteDiffs | Select-Object -First 20)) { Write-Host $d -ForegroundColor DarkGray }
        if ($byteDiffs.Count -gt 20) {
            Write-Host ("    ... {0} more. Use -Bytes to list them all." -f ($byteDiffs.Count - 20)) -ForegroundColor DarkGray
        }
    }
    Write-Host ""
    if ($diffCount -eq 0 -and $byteDiffs.Count -eq 0) {
        Write-Host "  The two EDIDs are byte-identical." -ForegroundColor Green
    } else {
        Write-Host ("  {0} decoded field(s) differ." -f $diffCount) -ForegroundColor Cyan
    }
    Write-Host ""
}
Set-Alias -Name edid-diff -Value Compare-EDIDSource


# ------------------------------------------------------------
#  edid : interactive menu
# ------------------------------------------------------------
function Invoke-EDIDMenu {
    $TL=[char]0x250C; $TR=[char]0x2510; $BL=[char]0x2514; $BR=[char]0x2518
    $HZ=[char]0x2500; $VT=[char]0x2502
    $boxW = 62

    while ($true) {
        Clear-Host
        Write-Host ""
        $title = "EDID TOOLS"
        $pad = $boxW - $title.Length; if ($pad -lt 0) { $pad = 0 }
        $lp = [math]::Floor($pad / 2); $rp = $pad - $lp
        Write-Host ("  " + $TL + ("$HZ" * $boxW) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT) -NoNewline -ForegroundColor DarkGray
        Write-Host ((" " * $lp) + $title + (" " * $rp)) -NoNewline -ForegroundColor Cyan
        Write-Host $VT -ForegroundColor DarkGray
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""

        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Read" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - 4 - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  1)  List displays        (summary of every connected display)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  2)  List + remembered    (also displays seen in the past)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  3)  Full decode          (one display or a .bin file)"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""

        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Diagnose" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - 8 - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  4)  Validate             (checksums + timing vs declared limits)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  5)  Compare two EDIDs    (direct vs through switcher/extender)"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""

        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Export" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - 6 - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  6)  Dump one display     (.bin for CRU / AW Editor / matrix)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  7)  Dump all connected"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  8)  Open dump folder"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host ("    Dumps go to: {0}" -f (Get-EDIDOutputPath)) -ForegroundColor DarkGray
        Write-Host "    Read-only tool. To FORCE an EDID use CRU (ToastyX) or an EDID emulator." -ForegroundColor DarkGray
        Write-Host ""
        Write-Host "    0)  Exit" -ForegroundColor DarkGray
        Write-Host ""

        if (-not [Console]::IsInputRedirected) { try { $Host.UI.RawUI.FlushInputBuffer() } catch {} }
        $op = Read-Host "  Choose option"
        switch ($op) {
            "1" { Get-EDIDList }
            "2" { Get-EDIDList -Remembered }
            "3" {
                Get-EDIDList -Remembered
                $s = Read-Host "  Display number or path to a .bin"
                if ($s) { Show-EDIDInfo -Source $s -Remembered }
            }
            "4" {
                $s = Read-Host "  Display number / path (Enter = check all connected)"
                if ($s) { Test-EDIDIntegrity -Source $s } else { Test-EDIDIntegrity }
            }
            "5" {
                Get-EDIDList -Remembered
                Write-Host "  Each side can be a display number or a path to a .bin" -ForegroundColor DarkGray
                $a = Read-Host "  A"
                $b = Read-Host "  B"
                if ($a -and $b) { Compare-EDIDSource -A $a -B $b -Remembered }
            }
            "6" {
                Get-EDIDList -Remembered
                $s = Read-Host "  Display number to dump"
                if ($s) { Export-EDIDBinary -Source $s -Remembered }
            }
            "7" { Export-EDIDBinary -All }
            "8" {
                $p = Get-EDIDOutputPath
                if (-not (Test-Path -LiteralPath $p)) { New-Item -ItemType Directory -Path $p -Force | Out-Null }
                Start-Process explorer.exe $p
            }
            "0" { return }
            default { Write-Host "  Invalid option" -ForegroundColor Yellow }
        }
        Write-Host ""
        Read-Host "  [Enter] to return" | Out-Null
    }
}
Set-Alias -Name edid -Value Invoke-EDIDMenu
