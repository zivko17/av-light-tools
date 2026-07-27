<#
.SYNOPSIS
    Crea la estructura de carpetas del USB toolkit (Ventoy).
.EXAMPLE
    .\New-VentoyStructure.ps1
    .\New-VentoyStructure.ps1 -DriveLetter F -Force
#>
[CmdletBinding()]
param(
    [ValidatePattern('^[A-Za-z]$')]
    [string]$DriveLetter = 'E',

    # Omite la comprobacion de la etiqueta del volumen
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$Root = "$($DriveLetter.ToUpper()):\"

# --- Validaciones -----------------------------------------------------------
if (-not (Test-Path $Root)) {
    throw "La unidad $Root no existe o no esta conectada."
}

$vol = Get-Volume -DriveLetter $DriveLetter.ToUpper()
Write-Host "Unidad detectada : $Root" -ForegroundColor Cyan
Write-Host "Etiqueta         : $($vol.FileSystemLabel)"
Write-Host "Sistema archivos : $($vol.FileSystem)"
Write-Host "Libre / Total    : $([math]::Round($vol.SizeRemaining/1GB,1)) GB / $([math]::Round($vol.Size/1GB,1)) GB"
Write-Host ""
if (-not $Force -and $vol.FileSystemLabel -notmatch 'Ventoy') {
    $r = Read-Host "La etiqueta no contiene 'Ventoy'. Continuar de todos modos? (s/N)"
    if ($r -notmatch '^[sSyY]$') { Write-Host "Cancelado."; return }
}

# --- Estructura -------------------------------------------------------------
$Folders = @(
    '01_BOOT_ISOS'
    '02_PORTABLE_APPS'
    '02_PORTABLE_APPS\CRU_CustomResolution'
    '02_PORTABLE_APPS\ShutterEncoderPortable'
    '02_PORTABLE_APPS\Advanced_IP_Scanner'
    '02_PORTABLE_APPS\WiresharkPortable'
    '02_PORTABLE_APPS\NovaLCT'
    '03_MEDIA_TEST'
    '03_MEDIA_TEST\IMAGENES_TEST'
    '03_MEDIA_TEST\VIDEO_CODECS'
    '03_MEDIA_TEST\AUDIO_TEST'
)

$created = 0; $existed = 0
foreach ($f in $Folders) {
    $path = Join-Path $Root $f
    if (Test-Path $path) {
        Write-Host "  [=] $f" -ForegroundColor DarkGray
        $existed++
    } else {
        New-Item -Path $path -ItemType Directory -Force | Out-Null
        Write-Host "  [+] $f" -ForegroundColor Green
        $created++
    }
}

# --- Notas de contenido (solo si no existen) --------------------------------
$Notes = @{
    '01_BOOT_ISOS'                = "ISOs arrancables para Ventoy (raiz de la particion Ventoy).`r`n- HirensBootCD.iso`r`n- Ubuntu_24.04_Live.iso`r`n- Win10_PE.iso"
    '02_PORTABLE_APPS'            = "Apps que se ejecutan sin instalar.`r`nUna carpeta por herramienta. Anota la version en un TXT dentro de cada una."
    '03_MEDIA_TEST\IMAGENES_TEST' = "Patrones: SMPTE, PLUGE, grid 1:1, solidos RGB/blanco/negro.`r`nGuardar en 1920x1080, 3840x2160 y resoluciones nativas del LED."
    '03_MEDIA_TEST\VIDEO_CODECS'  = "Clips de prueba: ProRes 422, DXV3, H.264, HAP / HAP Q.`r`nIncluir uno con movimiento rapido para juzgar latencia."
    '03_MEDIA_TEST\AUDIO_TEST'    = "Pink noise, sweeps 20 Hz - 20 kHz, tono 1 kHz -20 dBFS, test de polaridad y canales L/R."
}

foreach ($k in $Notes.Keys) {
    $file = Join-Path (Join-Path $Root $k) '_LEEME.txt'
    if (-not (Test-Path $file)) {
        $Notes[$k] | Out-File -FilePath $file -Encoding UTF8
    }
}

Write-Host ""
Write-Host "Listo. Creadas: $created | Ya existian: $existed" -ForegroundColor Cyan
Write-Host ""
Get-ChildItem $Root -Directory | Select-Object Name, LastWriteTime | Format-Table -AutoSize
