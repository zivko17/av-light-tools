# ============================================================
#  AudioTools.ps1 - Audio toolkit for AV (FFmpeg)
# ============================================================

$script:AudioCodecMeta = @{
    mp3_320    = @{ ext = '.mp3';  args = @('-c:a','libmp3lame','-b:a','320k'); label = 'MP3-320' }
    mp3_192    = @{ ext = '.mp3';  args = @('-c:a','libmp3lame','-b:a','192k'); label = 'MP3-192' }
    aac_256    = @{ ext = '.m4a';  args = @('-c:a','aac','-b:a','256k'); label = 'AAC-256' }
    opus_192   = @{ ext = '.opus'; args = @('-c:a','libopus','-b:a','192k'); label = 'Opus-192' }
    flac       = @{ ext = '.flac'; args = @('-c:a','flac','-compression_level','5'); label = 'FLAC' }
    wav_16     = @{ ext = '.wav';  args = @('-c:a','pcm_s16le'); label = 'WAV-16' }
    wav_24     = @{ ext = '.wav';  args = @('-c:a','pcm_s24le'); label = 'WAV-24' }
}

function Resolve-FFmpegAudio {
    $cmd = Get-Command ffmpeg.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    throw "ffmpeg not found. Install with: winget install Gyan.FFmpeg"
}
function Resolve-FFprobeAudio {
    $cmd = Get-Command ffprobe.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    throw "ffprobe not found."
}

function Get-AudioInputs {
    param([Parameter(Mandatory=$true)][string]$Path)
    $clean = ($Path -replace '^"|"$','').Trim()
    if (-not (Test-Path -LiteralPath $clean)) {
        Write-Host "  Path not found: $clean" -ForegroundColor Red
        return @()
    }
    $audioExts = @('.mp3','.flac','.wav','.m4a','.aac','.ogg','.opus','.wma','.aif','.aiff','.mp4','.mkv','.mov','.webm')
    $item = Get-Item -LiteralPath $clean
    if ($item.PSIsContainer) {
        return @(Get-ChildItem -LiteralPath $clean -File -Recurse | Where-Object { $audioExts -contains $_.Extension.ToLower() })
    } else {
        return @($item)
    }
}

# Cross-version arg passing for ffmpeg/ffprobe.
# ProcessStartInfo.ArgumentList only exists on .NET Core (PS7+). On Windows
# PowerShell 5.1 (.NET Framework) we fall back to a correctly-quoted Arguments string.
function ConvertTo-NativeArg {
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
function Set-PSIArguments {
    param([System.Diagnostics.ProcessStartInfo]$Psi, [string[]]$ArgList)
    if ($Psi.PSObject.Properties['ArgumentList']) {
        foreach ($a in $ArgList) { $Psi.ArgumentList.Add([string]$a) }            # PS7 / .NET Core
    } else {
        $Psi.Arguments = (($ArgList | ForEach-Object { ConvertTo-NativeArg $_ }) -join ' ')  # PS5.1 / .NET Framework
    }
}

function Invoke-AudioFF {
    param([string]$Ffmpeg, [string[]]$FFArgs)
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $Ffmpeg
    Set-PSIArguments -Psi $psi -ArgList $FFArgs
    $psi.UseShellExecute = $false
    $psi.RedirectStandardError = $true
    $p = [System.Diagnostics.Process]::Start($psi)
    $p.StandardError.ReadToEnd() | Out-Null
    $p.WaitForExit()
    return $p.ExitCode
}

# Like Invoke-AudioFF but also returns stderr (loudnorm prints its measurements there).
function Invoke-AudioFFErr {
    param([string]$Ffmpeg, [string[]]$FFArgs)
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $Ffmpeg
    Set-PSIArguments -Psi $psi -ArgList $FFArgs
    $psi.UseShellExecute = $false
    $psi.RedirectStandardError = $true
    $p = [System.Diagnostics.Process]::Start($psi)
    $err = $p.StandardError.ReadToEnd()
    $p.WaitForExit()
    return [pscustomobject]@{ ExitCode = $p.ExitCode; Err = $err }
}

# Avoid silently overwriting when -Recurse flattens same-named files from different
# folders into one output dir. De-duplicates WITHIN a run (re-runs still overwrite).
function Get-NonCollidingPath {
    param([string]$Path, [System.Collections.Generic.HashSet[string]]$Used)
    if ($Used.Add($Path.ToLower())) { return $Path }
    $dir  = Split-Path $Path -Parent
    $name = [IO.Path]::GetFileNameWithoutExtension($Path)
    $ext  = [IO.Path]::GetExtension($Path)
    $n = 2
    do { $cand = Join-Path $dir "${name}_$n$ext"; $n++ } while (-not $Used.Add($cand.ToLower()))
    return $cand
}

function Invoke-AudioConvert {
    param(
        [Parameter(Mandatory=$true)][string]$Path,
        [Parameter(Mandatory=$true)][string]$Codec,
        [int]$SampleRate,
        [ValidateSet('mono','stereo')][string]$Channels
    )
    $ffmpeg = Resolve-FFmpegAudio
    $files = Get-AudioInputs -Path $Path
    if ($files.Count -eq 0) { Write-Host "  No audio files found." -ForegroundColor Yellow; return }
    $outDir = [Environment]::GetFolderPath('MyMusic')
    if (-not (Test-Path $outDir)) { New-Item -Path $outDir -ItemType Directory -Force | Out-Null }
    $meta = $script:AudioCodecMeta[$Codec]
    if (-not $meta) { Write-Host "  Unknown codec: $Codec" -ForegroundColor Red; return }

    Write-Host "`n  Converting $($files.Count) file(s) to $($meta.label) -> $outDir`n" -ForegroundColor Cyan
    $ok=0;$fail=0;$i=0;$total=$files.Count
    $used = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($f in $files) {
        $i++
        $base = [IO.Path]::GetFileNameWithoutExtension($f.Name)
        $output = Join-Path $outDir "${base}_$($meta.label)$($meta.ext)"
        $output = Get-NonCollidingPath $output $used
        $ffArgs = @('-hide_banner','-y','-i', $f.FullName) + $meta.args
        if ($SampleRate) { $ffArgs += @('-ar', "$SampleRate") }
        if ($Channels -eq 'mono') { $ffArgs += @('-ac','1') }
        if ($Channels -eq 'stereo') { $ffArgs += @('-ac','2') }
        $ffArgs += @($output)
        Write-Host ("  [$i/$total] " + $f.Name) -ForegroundColor DarkGray
        $exit = Invoke-AudioFF -Ffmpeg $ffmpeg -FFArgs $ffArgs
        if ($exit -eq 0 -and (Test-Path -LiteralPath $output)) {
            $sizeMB = [math]::Round((Get-Item $output).Length / 1MB, 1)
            Write-Host ("        OK -> $(Split-Path $output -Leaf)  ($sizeMB MB)") -ForegroundColor Green
            $ok++
        } else { Write-Host "        FAILED" -ForegroundColor Red; $fail++ }
    }
    Write-Host "`n  Done: $ok ok, $fail failed" -ForegroundColor Cyan
    Write-Host "  Output: $outDir`n" -ForegroundColor DarkGray
}

function ConvertTo-Mp3   { param([Parameter(Mandatory)][string]$Path) Invoke-AudioConvert -Path $Path -Codec mp3_320 }
function ConvertTo-Mp3Lo { param([Parameter(Mandatory)][string]$Path) Invoke-AudioConvert -Path $Path -Codec mp3_192 }
function ConvertTo-Aac   { param([Parameter(Mandatory)][string]$Path) Invoke-AudioConvert -Path $Path -Codec aac_256 }
function ConvertTo-Opus  { param([Parameter(Mandatory)][string]$Path) Invoke-AudioConvert -Path $Path -Codec opus_192 }
function ConvertTo-Flac  { param([Parameter(Mandatory)][string]$Path) Invoke-AudioConvert -Path $Path -Codec flac }
function ConvertTo-Wav16 { param([Parameter(Mandatory)][string]$Path) Invoke-AudioConvert -Path $Path -Codec wav_16 }
function ConvertTo-Wav24 { param([Parameter(Mandatory)][string]$Path) Invoke-AudioConvert -Path $Path -Codec wav_24 }
Set-Alias -Name mp3 -Value ConvertTo-Mp3
Set-Alias -Name mp3-low -Value ConvertTo-Mp3Lo
Set-Alias -Name aac -Value ConvertTo-Aac
Set-Alias -Name opus -Value ConvertTo-Opus
Set-Alias -Name flac -Value ConvertTo-Flac
Set-Alias -Name wav16 -Value ConvertTo-Wav16
Set-Alias -Name wav24 -Value ConvertTo-Wav24

function Invoke-AudioNormalize {
    param([Parameter(Mandatory=$true)][string]$Path)
    $ffmpeg = Resolve-FFmpegAudio
    $files = Get-AudioInputs -Path $Path
    if ($files.Count -eq 0) { return }
    $outDir = [Environment]::GetFolderPath('MyMusic')
    Write-Host "`n  Normalizing $($files.Count) file(s) to -23 LUFS (two-pass) -> $outDir`n" -ForegroundColor Cyan
    $used = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($f in $files) {
        $base = [IO.Path]::GetFileNameWithoutExtension($f.Name)
        # Output extension MUST match the chosen codec, or we'd write e.g. MP3 data into a .m4a/.opus container
        if     ($f.Extension -ieq '.flac') { $codec = 'flac';       $outExt = '.flac' }
        elseif ($f.Extension -ieq '.wav')  { $codec = 'pcm_s16le';  $outExt = '.wav'  }
        else                               { $codec = 'libmp3lame'; $outExt = '.mp3'  }
        $output = Join-Path $outDir "${base}_norm$outExt"
        $output = Get-NonCollidingPath $output $used
        Write-Host "  $($f.Name)" -ForegroundColor DarkGray

        # Pass 1: measure loudness (loudnorm prints JSON stats to stderr)
        $measure = Invoke-AudioFFErr -Ffmpeg $ffmpeg -FFArgs @('-hide_banner','-i', $f.FullName, '-af','loudnorm=I=-23:LRA=7:TP=-2:print_format=json','-f','null','-')
        $stats = $null
        $jsonText = [regex]::Match($measure.Err, '(?s)\{.*\}').Value
        if ($jsonText) { try { $stats = $jsonText | ConvertFrom-Json } catch { $stats = $null } }

        # Pass 2: apply with measured values (true EBU R128); fall back to single-pass if measuring failed
        if ($stats -and ($null -ne ($stats.input_i -as [double]))) {
            $lf = "loudnorm=I=-23:LRA=7:TP=-2:measured_I=$($stats.input_i):measured_TP=$($stats.input_tp):measured_LRA=$($stats.input_lra):measured_thresh=$($stats.input_thresh):offset=$($stats.target_offset):linear=true"
        } else {
            Write-Host "        (measurement failed, single-pass fallback)" -ForegroundColor Yellow
            $lf = 'loudnorm=I=-23:LRA=7:TP=-2'
        }
        $ffArgs = @('-hide_banner','-y','-i', $f.FullName, '-af', $lf, '-c:a', $codec, $output)
        $exit = Invoke-AudioFF -Ffmpeg $ffmpeg -FFArgs $ffArgs
        if ($exit -eq 0) { Write-Host "        OK -> $(Split-Path $output -Leaf)" -ForegroundColor Green }
        else { Write-Host "        FAILED" -ForegroundColor Red }
    }
    Write-Host ""
}
Set-Alias -Name normalize -Value Invoke-AudioNormalize

function Invoke-AudioTrim {
    param([Parameter(Mandatory=$true)][string]$Path, [string]$Start="00:00:00", [string]$End)
    $ffmpeg = Resolve-FFmpegAudio
    $clean = ($Path -replace '^"|"$','').Trim()
    if (-not (Test-Path -LiteralPath $clean)) { Write-Host "  Not found." -ForegroundColor Red; return }
    $file = Get-Item -LiteralPath $clean
    $outDir = [Environment]::GetFolderPath('MyMusic')
    $output = Join-Path $outDir "$([IO.Path]::GetFileNameWithoutExtension($file.Name))_trim$($file.Extension)"
    $ffArgs = @('-hide_banner','-y','-i', $file.FullName, '-ss', $Start)
    if ($End) { $ffArgs += @('-to', $End) }
    $ffArgs += @('-c','copy', $output)
    Write-Host "`n  Trimming $($file.Name)..." -ForegroundColor Cyan
    $exit = Invoke-AudioFF -Ffmpeg $ffmpeg -FFArgs $ffArgs
    if ($exit -eq 0) { Write-Host "  OK -> $output`n" -ForegroundColor Green }
    else { Write-Host "  FAILED`n" -ForegroundColor Red }
}
Set-Alias -Name trim-audio -Value Invoke-AudioTrim

function Invoke-ExtractAudio {
    param([Parameter(Mandatory=$true)][string]$Path, [ValidateSet('mp3','flac','wav','copy')][string]$Format='mp3')
    $ffmpeg = Resolve-FFmpegAudio
    $files = Get-AudioInputs -Path $Path
    if ($files.Count -eq 0) { return }
    $outDir = [Environment]::GetFolderPath('MyMusic')
    Write-Host "`n  Extracting audio from $($files.Count) file(s) -> $outDir`n" -ForegroundColor Cyan
    $used = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($f in $files) {
        $base = [IO.Path]::GetFileNameWithoutExtension($f.Name)
        $ext = switch ($Format) { 'mp3' {'.mp3'} 'flac' {'.flac'} 'wav' {'.wav'} 'copy' {'.m4a'} }
        $output = Join-Path $outDir "${base}_audio$ext"
        $output = Get-NonCollidingPath $output $used
        $codecArgs = switch ($Format) {
            'mp3' { @('-vn','-c:a','libmp3lame','-b:a','320k') }
            'flac' { @('-vn','-c:a','flac') }
            'wav' { @('-vn','-c:a','pcm_s16le') }
            'copy' { @('-vn','-c:a','copy') }
        }
        $ffArgs = @('-hide_banner','-y','-i', $f.FullName) + $codecArgs + @($output)
        Write-Host "  $($f.Name)" -ForegroundColor DarkGray
        $exit = Invoke-AudioFF -Ffmpeg $ffmpeg -FFArgs $ffArgs
        if ($exit -eq 0) { Write-Host "        OK -> $(Split-Path $output -Leaf)" -ForegroundColor Green }
        else { Write-Host "        FAILED" -ForegroundColor Red }
    }
    Write-Host ""
}
Set-Alias -Name extract-audio -Value Invoke-ExtractAudio

function Invoke-AudioResample {
    param([Parameter(Mandatory=$true)][string]$Path, [ValidateSet('44100','48000','96000','192000')][string]$SampleRate='48000')
    $ffmpeg = Resolve-FFmpegAudio
    $files = Get-AudioInputs -Path $Path
    if ($files.Count -eq 0) { return }
    $outDir = [Environment]::GetFolderPath('MyMusic')
    Write-Host "`n  Resampling $($files.Count) file(s) to $SampleRate Hz -> $outDir`n" -ForegroundColor Cyan
    $used = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($f in $files) {
        $base = [IO.Path]::GetFileNameWithoutExtension($f.Name)
        $output = Join-Path $outDir "${base}_${SampleRate}$($f.Extension)"
        $output = Get-NonCollidingPath $output $used
        $codec = switch ($f.Extension.ToLower()) {
            '.flac' { @('-c:a','flac') }
            '.wav' { @('-c:a','pcm_s16le') }
            '.mp3' { @('-c:a','libmp3lame','-b:a','320k') }
            default { @('-c:a','aac','-b:a','256k') }
        }
        $ffArgs = @('-hide_banner','-y','-i', $f.FullName, '-ar', $SampleRate) + $codec + @($output)
        Write-Host "  $($f.Name)" -ForegroundColor DarkGray
        $exit = Invoke-AudioFF -Ffmpeg $ffmpeg -FFArgs $ffArgs
        if ($exit -eq 0) { Write-Host "        OK" -ForegroundColor Green }
        else { Write-Host "        FAILED" -ForegroundColor Red }
    }
    Write-Host ""
}
Set-Alias -Name resample -Value Invoke-AudioResample

function Invoke-AudioChannels {
    param([Parameter(Mandatory=$true)][string]$Path, [Parameter(Mandatory=$true)][ValidateSet('mono','stereo')][string]$Mode)
    $ffmpeg = Resolve-FFmpegAudio
    $files = Get-AudioInputs -Path $Path
    if ($files.Count -eq 0) { return }
    $outDir = [Environment]::GetFolderPath('MyMusic')
    Write-Host "`n  Converting $($files.Count) file(s) to $Mode -> $outDir`n" -ForegroundColor Cyan
    $ch = if ($Mode -eq 'mono') {'1'} else {'2'}
    $used = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($f in $files) {
        $base = [IO.Path]::GetFileNameWithoutExtension($f.Name)
        $output = Join-Path $outDir "${base}_$Mode$($f.Extension)"
        $output = Get-NonCollidingPath $output $used
        $codec = switch ($f.Extension.ToLower()) {
            '.flac' { @('-c:a','flac') }
            '.wav' { @('-c:a','pcm_s16le') }
            '.mp3' { @('-c:a','libmp3lame','-b:a','320k') }
            default { @('-c:a','aac','-b:a','256k') }
        }
        $ffArgs = @('-hide_banner','-y','-i', $f.FullName, '-ac', $ch) + $codec + @($output)
        Write-Host "  $($f.Name)" -ForegroundColor DarkGray
        $exit = Invoke-AudioFF -Ffmpeg $ffmpeg -FFArgs $ffArgs
        if ($exit -eq 0) { Write-Host "        OK" -ForegroundColor Green }
        else { Write-Host "        FAILED" -ForegroundColor Red }
    }
    Write-Host ""
}

function Get-AudioProbe {
    param([Parameter(Mandatory=$true)][string]$Path)
    $ffprobe = Resolve-FFprobeAudio
    $clean = ($Path -replace '^"|"$','').Trim()
    if (-not (Test-Path -LiteralPath $clean)) { Write-Host "  Not found." -ForegroundColor Red; return }
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $ffprobe
    Set-PSIArguments -Psi $psi -ArgList @('-v','quiet','-print_format','json','-show_format','-show_streams','--', $clean)
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $p = [System.Diagnostics.Process]::Start($psi)
    $out = $p.StandardOutput.ReadToEnd()
    $p.WaitForExit()
    if (-not $out) { Write-Host "  No probe data." -ForegroundColor Red; return }
    $json = $out | ConvertFrom-Json
    $a = $json.streams | Where-Object codec_type -eq 'audio' | Select-Object -First 1
    Write-Host "`n=== AUDIO PROBE ===" -ForegroundColor Cyan
    Write-Host ("  File:         $(Split-Path $clean -Leaf)") -ForegroundColor Green
    Write-Host ("  Container:    $(($json.format.format_name -split ',')[0])")
    if ($json.format.duration) { Write-Host ("  Duration:     $([timespan]::FromSeconds([double]$json.format.duration).ToString('hh\:mm\:ss\.ff'))") }
    if ($json.format.size)     { Write-Host ("  Size:         {0:N1} MB" -f ([double]$json.format.size / 1MB)) }
    if ($a) {
        Write-Host ""
        Write-Host ("  Codec:        $($a.codec_name)") -ForegroundColor Yellow
        Write-Host ("  Sample rate:  {0:N1} kHz" -f ([double]$a.sample_rate / 1000))
        Write-Host ("  Channels:     $($a.channels)  ($($a.channel_layout))")
        if ($a.bit_rate) { Write-Host ("  Bitrate:      {0:N0} kbps" -f ([double]$a.bit_rate / 1000)) }
        if ($a.bits_per_sample -and $a.bits_per_sample -gt 0) { Write-Host ("  Bit depth:    $($a.bits_per_sample)-bit") }
    }
    Write-Host ""
}
Set-Alias -Name probe-audio -Value Get-AudioProbe

function Read-AudioFile { $p = Read-Host "  File or folder"; return ($p -replace '^"|"$','').Trim() }

function Invoke-AudioToolsMenu {
    $TL=[char]0x250C; $TR=[char]0x2510; $BL=[char]0x2514; $BR=[char]0x2518
    $HZ=[char]0x2500; $VT=[char]0x2502
    $boxW = 52
    while ($true) {
        Clear-Host
        Write-Host ""
        $title = "AUDIO TOOLS  -  FFMPEG"
        $pad = $boxW - $title.Length; if ($pad -lt 0) { $pad = 0 }
        $lp = [math]::Floor($pad / 2); $rp = $pad - $lp
        Write-Host ("  " + $TL + ("$HZ" * $boxW) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT) -NoNewline -ForegroundColor DarkGray
        Write-Host ((" " * $lp) + $title + (" " * $rp)) -NoNewline -ForegroundColor Cyan
        Write-Host $VT -ForegroundColor DarkGray
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Compression" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - ("Compression").Length - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   1)  MP3 320         (high quality universal)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   2)  MP3 192         (standard)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   3)  AAC 256         (better than MP3)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   4)  OGG/Opus 192    (best quality, open)"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Lossless" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - ("Lossless").Length - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   5)  FLAC            (lossless compressed)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   6)  WAV 16-bit      (CD quality)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   7)  WAV 24-bit      (master)"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Special" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - ("Special").Length - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   8)  Normalize       (EBU R128 -23 LUFS)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "   9)  Trim            (cut start/end)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  10)  Extract audio   (from video)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  11)  Resample        (44.1k / 48k / 96k / 192k)"
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  12)  Mono / Stereo"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host ("  " + $TL + $HZ + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "Info" -NoNewline -ForegroundColor Yellow
        $fill = $boxW - ("Info").Length - 3; if ($fill -lt 0) { $fill = 0 }
        Write-Host (" " + ("$HZ" * $fill) + $TR) -ForegroundColor DarkGray
        Write-Host ("  " + $VT + " ") -NoNewline -ForegroundColor DarkGray
        Write-Host "  13)  probe audio"
        Write-Host ("  " + $BL + ("$HZ" * $boxW) + $BR) -ForegroundColor DarkGray
        Write-Host ""
        Write-Host "    0)  Exit" -ForegroundColor DarkGray
        Write-Host ""
        $op = Read-Host "  Choose option"
        switch ($op) {
            "1"  { $p = Read-AudioFile; if ($p) { Invoke-AudioConvert -Path $p -Codec mp3_320 } }
            "2"  { $p = Read-AudioFile; if ($p) { Invoke-AudioConvert -Path $p -Codec mp3_192 } }
            "3"  { $p = Read-AudioFile; if ($p) { Invoke-AudioConvert -Path $p -Codec aac_256 } }
            "4"  { $p = Read-AudioFile; if ($p) { Invoke-AudioConvert -Path $p -Codec opus_192 } }
            "5"  { $p = Read-AudioFile; if ($p) { Invoke-AudioConvert -Path $p -Codec flac } }
            "6"  { $p = Read-AudioFile; if ($p) { Invoke-AudioConvert -Path $p -Codec wav_16 } }
            "7"  { $p = Read-AudioFile; if ($p) { Invoke-AudioConvert -Path $p -Codec wav_24 } }
            "8"  { $p = Read-AudioFile; if ($p) { Invoke-AudioNormalize -Path $p } }
            "9"  {
                $p = Read-AudioFile
                if ($p) {
                    $s = Read-Host "  Start (HH:MM:SS, default 00:00:00)"
                    if (-not $s) { $s = "00:00:00" }
                    $e = Read-Host "  End (HH:MM:SS, empty = to end)"
                    if ($e) { Invoke-AudioTrim -Path $p -Start $s -End $e } else { Invoke-AudioTrim -Path $p -Start $s }
                }
            }
            "10" {
                $p = Read-AudioFile
                if ($p) {
                    $fmt = Read-Host "  Format (mp3/flac/wav/copy, default mp3)"
                    if (-not $fmt) { $fmt = "mp3" }
                    Invoke-ExtractAudio -Path $p -Format $fmt
                }
            }
            "11" {
                $p = Read-AudioFile
                if ($p) {
                    $sr = Read-Host "  Sample rate (44100/48000/96000/192000, default 48000)"
                    if (-not $sr) { $sr = "48000" }
                    Invoke-AudioResample -Path $p -SampleRate $sr
                }
            }
            "12" {
                $p = Read-AudioFile
                if ($p) {
                    $m = Read-Host "  Mode (mono/stereo)"
                    if ($m -in @('mono','stereo')) { Invoke-AudioChannels -Path $p -Mode $m }
                }
            }
            "13" { $p = Read-AudioFile; if ($p) { Get-AudioProbe -Path $p } }
            "0"  { return }
            default { Write-Host "  Invalid option" -ForegroundColor Yellow }
        }
        Write-Host ""
        Read-Host "  [Enter] to return"
    }
}
Set-Alias -Name audiotools -Value Invoke-AudioToolsMenu
