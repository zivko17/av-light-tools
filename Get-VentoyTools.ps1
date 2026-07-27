<#
.SYNOPSIS
    Descarga las herramientas portables del USB toolkit a 02_PORTABLE_APPS.
.DESCRIPTION
    Resuelve la ultima version de cada herramienta (URL directa, GitHub API
    o indice HTTP), la descarga y la extrae en su carpeta.
    Las que no tienen descarga automatizable se listan al final.
.EXAMPLE
    .\Get-VentoyTools.ps1 -ListOnly
    .\Get-VentoyTools.ps1
    .\Get-VentoyTools.ps1 -Only CRU,Wireshark -Force
#>
[CmdletBinding()]
param(
    [ValidatePattern('^[A-Za-z]$')]
    [string]$DriveLetter = 'E',

    # Descargar solo estas herramientas (por Name)
    [string[]]$Only,

    # Mostrar el catalogo y las URLs resueltas sin descargar nada
    [switch]$ListOnly,

    # Volver a descargar aunque la carpeta ya tenga contenido
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$ProgressPreference = 'SilentlyContinue'   # acelera mucho Invoke-WebRequest

$Root    = "$($DriveLetter.ToUpper()):\02_PORTABLE_APPS"
$TempDir = Join-Path $env:TEMP "VentoyTools"

# ============================================================================
#  CATALOGO
#  Type: Direct | GitHub | Index | Manual
# ============================================================================
$Catalog = @(
    @{ Name='CRU'; Folder='CRU_CustomResolution'; Type='Direct'
       Url='https://www.monitortests.com/download/cru/cru-1.5.3.zip'
       Note='Editor EDID. Version fijada: revisar monitortests.com al actualizar.' }

    @{ Name='Sysinternals'; Folder='Sysinternals'; Type='Direct'
       Url='https://download.sysinternals.com/files/SysinternalsSuite.zip'
       Note='TCPView, PsExec, Autoruns, Process Explorer... (~190 MB)' }

    @{ Name='PuTTY'; Folder='PuTTY'; Type='Direct'
       Url='https://the.earth.li/~sgtatham/putty/latest/w64/putty.zip'
       Note='Consola serie/SSH a switches.' }

    @{ Name='PingInfoView'; Folder='PingInfoView'; Type='Direct'
       Url='https://www.nirsoft.net/utils/pinginfoview.zip'
       Note='Monitoriza N IPs a la vez. Ojo: el antivirus lo marca (falso positivo).' }

    @{ Name='FFmpeg'; Folder='FFmpeg'; Type='Direct'
       Url='https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip'
       Note='Build essentials de gyan.dev. Base de tu modulo AVTools.' }
)

$Catalog += @(
    @{ Name='iPerf3'; Folder='iPerf3'; Type='Direct'
       Url='https://iperf.fr/download/windows/iperf-3.1.3-win64.zip'
       Note='Ancho de banda real de un tramo de red.' }

    @{ Name='Wireshark'; Folder='WiresharkPortable'; Type='Index'
       IndexUrl='https://www.wireshark.org/download/win64/'
       Pattern='WiresharkPortable64_(\d+)\.(\d+)\.(\d+)\.paf\.exe'
       Extract=$false
       Note='Instalador PortableApps. Necesita Npcap (admin) para capturar.' }

    @{ Name='AngryIPScanner'; Folder='Angry_IP_Scanner'; Type='GitHub'
       Repo='angryip/ipscan'; Asset='ipscan-win64-.*\.exe$'; Extract=$false
       Note='Alternativa scriptable a Advanced IP Scanner. Necesita Java o usar el build con JRE.' }

    @{ Name='ytdlp'; Folder='yt-dlp'; Type='GitHub'
       Repo='yt-dlp/yt-dlp'; Asset='^yt-dlp\.exe$'; Extract=$false
       Note='Descarga de material de referencia.' }

    @{ Name='ShutterEncoder'; Folder='ShutterEncoderPortable'; Type='Manual'
       Page='https://www.shutterencoder.com/'
       Note='La descarga pasa por una pagina de donacion: elegir "64-BIT PORTABLE VERSION".' }

    @{ Name='NovaLCT'; Folder='NovaLCT'; Type='Manual'
       Page='https://www.novastar.tech/downloads/'
       Note='Descargar tambien ViPlex Handy/Express. Guardar la version en el nombre.' }

    @{ Name='AdvancedIPScanner'; Folder='Advanced_IP_Scanner'; Type='Manual'
       Page='https://www.advanced-ip-scanner.com/'
       Note='El .exe se ejecuta y ofrece modo portable sin instalar.' }
)

# ============================================================================
#  RESOLVERS
# ============================================================================
function Resolve-ToolUrl {
    param([hashtable]$Tool)

    switch ($Tool.Type) {
        'Direct' { return $Tool.Url }

        'GitHub' {
            $api = "https://api.github.com/repos/$($Tool.Repo)/releases/latest"
            $rel = Invoke-RestMethod -Uri $api -Headers @{ 'User-Agent'='VentoyTools' }
            $a = $rel.assets | Where-Object { $_.name -match $Tool.Asset } | Select-Object -First 1
            if (-not $a) { throw "Sin asset que case con '$($Tool.Asset)' en $($rel.tag_name)" }
            return $a.browser_download_url
        }

        'Index' {
            $html = (Invoke-WebRequest -Uri $Tool.IndexUrl -UseBasicParsing).Content
            $hits = [regex]::Matches($html, $Tool.Pattern) | ForEach-Object {
                [pscustomobject]@{
                    File = $_.Value
                    Ver  = [version]("{0}.{1}.{2}" -f $_.Groups[1].Value,$_.Groups[2].Value,$_.Groups[3].Value)
                }
            }
            # Wireshark: minor impar = rama de desarrollo, nos quedamos con la estable
            $best = $hits | Where-Object { $_.Ver.Minor % 2 -eq 0 } |
                    Sort-Object Ver -Descending | Select-Object -First 1
            if (-not $best) { throw "No se encontro version estable en $($Tool.IndexUrl)" }
            return ($Tool.IndexUrl.TrimEnd('/') + '/' + $best.File)
        }
    }
}

function Install-Tool {
    param([hashtable]$Tool, [string]$Url)

    $dest = Join-Path $Root $Tool.Folder
    New-Item -Path $dest -ItemType Directory -Force | Out-Null

    $file = Join-Path $TempDir ([IO.Path]::GetFileName(($Url -split '\?')[0]))
    Write-Host "    bajando $([IO.Path]::GetFileName($file)) ..." -ForegroundColor DarkGray
    Invoke-WebRequest -Uri $Url -OutFile $file -UseBasicParsing -UserAgent 'VentoyTools'

    $mb = [math]::Round((Get-Item $file).Length / 1MB, 1)
    $extract = if ($Tool.ContainsKey('Extract')) { $Tool.Extract } else { $true }

    if ($extract -and $file -match '\.zip$') {
        Expand-Archive -Path $file -DestinationPath $dest -Force
        Remove-Item $file -Force
        Write-Host "    extraido ($mb MB)" -ForegroundColor Green
    } else {
        Move-Item $file (Join-Path $dest ([IO.Path]::GetFileName($file))) -Force
        Write-Host "    copiado ($mb MB)" -ForegroundColor Green
    }

    # Deja constancia de que version/URL se descargo y cuando
    $stamp = "Herramienta : $($Tool.Name)`r`nOrigen      : $Url`r`n" +
             "Descargado  : $(Get-Date -Format 'yyyy-MM-dd HH:mm')`r`nNota        : $($Tool.Note)"
    $stamp | Out-File (Join-Path $dest '_VERSION.txt') -Encoding UTF8
}

# ============================================================================
#  MAIN
# ============================================================================
if (-not (Test-Path $Root)) {
    throw "No existe $Root. Ejecuta antes New-VentoyStructure.ps1."
}
New-Item -Path $TempDir -ItemType Directory -Force | Out-Null

$items = if ($Only) { $Catalog | Where-Object { $_.Name -in $Only } } else { $Catalog }
if (-not $items) { throw "Ninguna herramienta coincide con -Only $($Only -join ',')" }

$auto   = $items | Where-Object { $_.Type -ne 'Manual' }
$manual = $items | Where-Object { $_.Type -eq 'Manual' }
$ok = 0; $skip = 0; $fail = @()

Write-Host "`nDestino: $Root`n" -ForegroundColor Cyan

foreach ($t in $auto) {
    Write-Host "[$($t.Name)]" -ForegroundColor White
    $dest = Join-Path $Root $t.Folder

    if (-not $Force -and (Test-Path $dest) -and (Get-ChildItem $dest -File -Recurse -EA SilentlyContinue |
        Where-Object { $_.Name -ne '_VERSION.txt' })) {
        Write-Host "    ya tiene contenido, omitido (usa -Force)" -ForegroundColor DarkYellow
        $skip++; continue
    }

    try {
        $url = Resolve-ToolUrl -Tool $t
        if ($ListOnly) {
            Write-Host "    $url" -ForegroundColor DarkCyan
        } else {
            Install-Tool -Tool $t -Url $url
            $ok++
        }
    } catch {
        Write-Host "    ERROR: $($_.Exception.Message)" -ForegroundColor Red
        $fail += $t.Name
    }
}

# --- Descarga manual --------------------------------------------------------
if ($manual) {
    Write-Host "`n--- Requieren descarga manual ---" -ForegroundColor Yellow
    foreach ($m in $manual) {
        New-Item -Path (Join-Path $Root $m.Folder) -ItemType Directory -Force | Out-Null
        Write-Host "  $($m.Name)  ->  $($m.Page)"
        Write-Host "      $($m.Note)" -ForegroundColor DarkGray
        "Descargar manualmente desde: $($m.Page)`r`n$($m.Note)" |
            Out-File (Join-Path $Root "$($m.Folder)\_DESCARGAR_DE.txt") -Encoding UTF8
    }
}

Remove-Item $TempDir -Recurse -Force -ErrorAction SilentlyContinue
Write-Host "`nOK: $ok | Omitidas: $skip | Fallos: $($fail.Count) $(if($fail){"($($fail -join ', '))"})" -ForegroundColor Cyan
