# Control rapido del Firewall de Windows para directo
#
#   fw-info                  estado de perfiles + reglas creadas por estas herramientas
#   fw-off [-Perfil X] [-Minutos N]   desactiva el firewall (N = se reactiva solo a los N min)
#   fw-on  [-Perfil X]       reactiva el firewall
#   fw-allow <app> [-Arreglar]   abre el firewall SOLO para ese software (entrada, todos los perfiles)
#   fw-remove <app|all>      quita las reglas que creo fw-allow
#   fw-blocks                lista las reglas de BLOQUEO de entrada activas (las del aviso de Windows)
#
# <app> puede ser: un preset (vmix, resolume, obs, ndi...), la ruta a un .exe,
# una carpeta (se abren todos sus .exe) o el nombre de un proceso en marcha.
# Las funciones que cambian algo piden admin y, si hace falta, abren una consola elevada.

# Variables en global: 'reload' recarga dentro de una funcion y solo copia funciones y alias
$global:AVFwGroup = 'AV-Scripts'
$global:AVFwTask  = 'AV-Scripts-FirewallOn'

# Presets: carpeta(s) de instalacion. Se abren todos los .exe que haya dentro
$global:AVFwPresets = [ordered]@{
    vmix       = @('C:\Program Files (x86)\vMix', 'C:\Program Files\vMix')
    resolume   = @('C:\Program Files\Resolume Arena', 'C:\Program Files\Resolume Avenue', 'C:\Program Files\Resolume Wire', 'C:\Program Files\Resolume Alley')
    arena      = @('C:\Program Files\Resolume Arena')
    avenue     = @('C:\Program Files\Resolume Avenue')
    wire       = @('C:\Program Files\Resolume Wire')
    obs        = @('C:\Program Files\obs-studio')
    ndi        = @('C:\Program Files\NDI')
    companion  = @('C:\Program Files\Companion', "$env:LOCALAPPDATA\Programs\companion")
    blackmagic = @('C:\Program Files\Blackmagic Design', 'C:\Program Files (x86)\Blackmagic Design')
    qsys       = @('C:\Program Files\QSC')
    ledset     = @('C:\Program Files\Linsn')
    # netaudio (pip, Python de la Store): el que habla con la red es bin\netaudio-daemon.exe
    netaudio   = @("$env:LOCALAPPDATA\Packages\PythonSoftwareFoundation.Python.3.13_qbz5n2kfra8p0\LocalCache\local-packages\Python313\site-packages\netaudio\bin",
                   "$env:APPDATA\Python\Python313\site-packages\netaudio\bin")
}

# Ejecutables que nunca necesitan red de entrada (instaladores, desinstaladores, crash reporters)
$global:AVFwSkipExe = '^(unins\d*|vc_?redist.*|vcredist.*|BsSndRpt.*|.*setup.*|.*update.*|.*upgrade.*|InstallCheck|LicenseViewer|Devcon)\.exe$'

function Test-AVFwAdmin {
    $p = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# "AV-Scripts [vmix] vMix64.exe" -> "vmix"
function Get-AVFwTag([string]$DisplayName) {
    if ($DisplayName -match '^AV-Scripts \[([^\]]+)\]') { return $Matches[1] }
    return '?'
}

function ConvertTo-AVFwArg([string]$s) { "'" + ($s -replace "'", "''") + "'" }

# Si no somos admin, relanza el comando en una consola elevada y devuelve $true
function Invoke-AVFwElevated([string]$Line) {
    if (Test-AVFwAdmin) { return $false }
    Write-Host "`n  Hace falta administrador: abriendo consola elevada para '$Line'..." -ForegroundColor Yellow
    $exe = (Get-Process -Id $PID).Path
    # Lo que salga en la consola elevada queda tambien en %TEMP%\AV-Scripts-fw.log
    $log = Join-Path $env:TEMP 'AV-Scripts-fw.log'
    $full = "Start-Transcript -LiteralPath '$log' -Force | Out-Null; try { $Line } finally { Stop-Transcript | Out-Null }"
    $enc = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($full))
    try {
        Start-Process -FilePath $exe -Verb RunAs -ArgumentList "-NoExit -EncodedCommand $enc" -ErrorAction Stop
    } catch {
        Write-Host "  Cancelado (no se acepto el UAC)`n" -ForegroundColor Red
    }
    return $true
}

function Get-AVFwProfileList([string]$Perfil) {
    if ($Perfil -eq 'All') { return @('Domain', 'Private', 'Public') }
    return @($Perfil)
}

function Write-AVFwProfiles {
    Write-Host "  --- Perfiles ---" -ForegroundColor Yellow
    $activos = @(Get-NetConnectionProfile -ErrorAction SilentlyContinue)
    foreach ($p in Get-NetFirewallProfile) {
        $redes = @($activos | Where-Object { "$($_.NetworkCategory)" -eq $p.Name -or ("$($_.NetworkCategory)" -eq 'DomainAuthenticated' -and $p.Name -eq 'Domain') } |
                 ForEach-Object { $_.InterfaceAlias })
        $txtRed = ''
        if ($redes.Count) { $txtRed = '  <- ' + ($redes -join ', ') }
        if ($p.Enabled) {
            Write-Host ("    {0,-8} ACTIVO     {1}" -f $p.Name, $txtRed) -ForegroundColor Green
        } else {
            Write-Host ("    {0,-8} DESACTIVADO{1}" -f $p.Name, $txtRed) -ForegroundColor Red
        }
    }
    $t = Get-ScheduledTask -TaskName $AVFwTask -ErrorAction SilentlyContinue
    if ($t) {
        $next = (Get-ScheduledTaskInfo -TaskName $AVFwTask -ErrorAction SilentlyContinue).NextRunTime
        Write-Host "    Reactivacion automatica programada: $next" -ForegroundColor DarkYellow
    }
}

# Devuelve objetos @{Tag; Exe} para lo que pida el usuario
function Resolve-AVFwApp([string]$App) {
    $out = @()
    $key = $App.ToLower()
    $folders = @()
    $tag = $null

    if ($AVFwPresets.Contains($key)) {
        $tag = $key
        $folders = @($AVFwPresets[$key] | Where-Object { Test-Path $_ -PathType Container })
        if (-not $folders.Count) { Write-Host "  Preset '$key': no encuentro su carpeta de instalacion" -ForegroundColor Red; return @() }
    } elseif (Test-Path -LiteralPath $App -PathType Leaf) {
        $f = Get-Item -LiteralPath $App
        return ,([pscustomobject]@{ Tag = $f.BaseName.ToLower(); Exe = $f.FullName })
    } elseif (Test-Path -LiteralPath $App -PathType Container) {
        $f = Get-Item -LiteralPath $App
        $tag = $f.Name.ToLower()
        $folders = @($f.FullName)
    } else {
        $procs = @(Get-Process -Name $App -ErrorAction SilentlyContinue | Where-Object { $_.Path } |
                   Select-Object -ExpandProperty Path -Unique)
        if (-not $procs.Count) {
            Write-Host "  No se que es '$App': no es preset, ni ruta, ni proceso en marcha" -ForegroundColor Red
            Write-Host "  Presets: $($AVFwPresets.Keys -join ', ')" -ForegroundColor DarkGray
            return @()
        }
        foreach ($p in $procs) { $out += [pscustomobject]@{ Tag = $key; Exe = $p } }
        return $out
    }

    foreach ($dir in $folders) {
        Get-ChildItem -LiteralPath $dir -Recurse -Filter *.exe -File -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -notmatch $AVFwSkipExe -and $_.FullName -notmatch '\\redist\\' } |
            ForEach-Object { $out += [pscustomobject]@{ Tag = $tag; Exe = $_.FullName } }
    }
    return $out
}

# Reglas de BLOQUEO de entrada activas, con su programa
function Get-AVFwBlockRules {
    $rules = @(Get-NetFirewallRule -Direction Inbound -Action Block -Enabled True -ErrorAction SilentlyContinue)
    if (-not $rules.Count) { return @() }
    $byId = @{}
    foreach ($f in ($rules | Get-NetFirewallApplicationFilter)) { $byId[$f.InstanceID] = $f.Program }
    foreach ($r in $rules) {
        $prog = $byId[$r.Name]
        if ($prog) { $prog = [Environment]::ExpandEnvironmentVariables($prog) }
        [pscustomobject]@{ Rule = $r; Name = $r.DisplayName; Profile = "$($r.Profile)"; Program = $prog }
    }
}

function Show-AVFirewall {
    Write-Host "`n=== FIREWALL (AV) ===" -ForegroundColor Cyan
    Write-AVFwProfiles
    Write-Host ""
    Write-Host "  --- Software abierto con fw-allow ---" -ForegroundColor Yellow
    $mine = @(Get-NetFirewallRule -Group $AVFwGroup -ErrorAction SilentlyContinue)
    if ($mine.Count) {
        $mine | Group-Object { Get-AVFwTag $_.DisplayName } | Sort-Object Name | ForEach-Object {
            $on = @($_.Group | Where-Object { $_.Enabled -eq 'True' }).Count
            Write-Host ("    {0,-12} {1} ejecutables" -f $_.Name, $on) -ForegroundColor Green
        }
    } else {
        Write-Host "    (ninguno)" -ForegroundColor DarkGray
    }
    Write-Host ""
    Write-Host "  Presets: $($AVFwPresets.Keys -join ', ')" -ForegroundColor DarkGray
    Write-Host "  Comandos: fw-info  fw-off  fw-on  fw-allow <app>  fw-remove <app|all>  fw-blocks" -ForegroundColor DarkGray
    Write-Host ""
}

function Disable-AVFirewall {
    param(
        [ValidateSet('All', 'Domain', 'Private', 'Public')][string]$Perfil = 'All',
        [int]$Minutos = 0
    )
    $line = "Disable-AVFirewall -Perfil $Perfil -Minutos $Minutos"
    if (Invoke-AVFwElevated $line) { return }

    $list = Get-AVFwProfileList $Perfil
    Set-NetFirewallProfile -Profile $list -Enabled False
    Write-Host "`n  FIREWALL DESACTIVADO  ($($list -join ', '))" -ForegroundColor Red

    if ($Minutos -gt 0) {
        $cmd = "Set-NetFirewallProfile -Profile $($list -join ',') -Enabled True; Unregister-ScheduledTask -TaskName '$AVFwTask' -Confirm:`$false"
        $act = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -WindowStyle Hidden -Command `"$cmd`""
        $trg = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes($Minutos)
        $usr = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -RunLevel Highest
        Register-ScheduledTask -TaskName $AVFwTask -Action $act -Trigger $trg -Principal $usr -Force | Out-Null
        Write-Host "  Se reactivara solo a las $((Get-Date).AddMinutes($Minutos).ToString('HH:mm'))" -ForegroundColor Yellow
    } else {
        Unregister-ScheduledTask -TaskName $AVFwTask -Confirm:$false -ErrorAction SilentlyContinue
        Write-Host "  Acuerdate de 'fw-on' al terminar. Mas seguro: 'fw-allow <app>'" -ForegroundColor Yellow
    }
    Write-Host ""
    Write-AVFwProfiles
    Write-Host ""
}

function Enable-AVFirewall {
    param([ValidateSet('All', 'Domain', 'Private', 'Public')][string]$Perfil = 'All')
    if (Invoke-AVFwElevated "Enable-AVFirewall -Perfil $Perfil") { return }

    $list = Get-AVFwProfileList $Perfil
    Set-NetFirewallProfile -Profile $list -Enabled True
    if ($Perfil -eq 'All') { Unregister-ScheduledTask -TaskName $AVFwTask -Confirm:$false -ErrorAction SilentlyContinue }
    Write-Host "`n  FIREWALL ACTIVADO  ($($list -join ', '))`n" -ForegroundColor Green
    Write-AVFwProfiles
    Write-Host ""
}

function Grant-AVFirewallApp {
    param(
        [Parameter(Mandatory = $true, Position = 0)][string[]]$App,
        [switch]$Arreglar
    )
    $line = 'Grant-AVFirewallApp -App ' + (($App | ForEach-Object { ConvertTo-AVFwArg $_ }) -join ',')
    if ($Arreglar) { $line += ' -Arreglar' }
    if (Invoke-AVFwElevated $line) { return }

    $targets = @()
    foreach ($a in $App) { $targets += Resolve-AVFwApp $a }
    if (-not $targets.Count) { Write-Host ""; return }

    $already = @{}
    Get-NetFirewallRule -Group $AVFwGroup -ErrorAction SilentlyContinue | Get-NetFirewallApplicationFilter |
        ForEach-Object { $already[$_.Program.ToLower()] = $true }

    Write-Host "`n  Abriendo firewall (entrada, todos los perfiles) para:" -ForegroundColor Cyan
    $nuevas = 0
    $fallos = 0
    foreach ($t in $targets) {
        $leaf = Split-Path $t.Exe -Leaf
        if ($already.ContainsKey($t.Exe.ToLower())) {
            Write-Host "    =  [$($t.Tag)] $leaf  (ya estaba)" -ForegroundColor DarkGray
            continue
        }
        # Ojo: el firewall no admite '|' en los nombres (lo usa como separador interno)
        try {
            New-NetFirewallRule -DisplayName "$AVFwGroup [$($t.Tag)] $leaf" -Group $AVFwGroup `
                -Direction Inbound -Action Allow -Profile Any -Program $t.Exe `
                -Description "Creada por fw-allow (C:\scripts\Firewall.ps1)" -ErrorAction Stop | Out-Null
            $already[$t.Exe.ToLower()] = $true
            $nuevas++
            Write-Host "    +  [$($t.Tag)] $leaf" -ForegroundColor Green
        } catch {
            $fallos++
            Write-Host "    X  [$($t.Tag)] $leaf  -> $($_.Exception.Message)" -ForegroundColor Red
        }
    }
    Write-Host "  $nuevas reglas nuevas" -ForegroundColor Green
    if ($fallos) {
        Write-Host "  $fallos reglas fallaron: no toco las reglas de bloqueo`n" -ForegroundColor Red
        return
    }

    # Un BLOQUEO gana a cualquier PERMITIR: hay que revisarlos
    $exes = @{}
    foreach ($t in $targets) { $exes[$t.Exe.ToLower()] = $true }
    $blocks = @(Get-AVFwBlockRules | Where-Object { $_.Program -and $exes.ContainsKey($_.Program.ToLower()) })
    if ($blocks.Count) {
        Write-Host ""
        if ($Arreglar) {
            Write-Host "  Desactivando reglas de BLOQUEO que anulaban el permiso:" -ForegroundColor Yellow
            foreach ($b in $blocks) {
                $b.Rule | Disable-NetFirewallRule
                Write-Host "    -  $($b.Name)  [$($b.Profile)]" -ForegroundColor Yellow
            }
        } else {
            Write-Host "  OJO: hay reglas de BLOQUEO para este software y ganan al permiso:" -ForegroundColor Red
            foreach ($b in $blocks) { Write-Host "    !  $($b.Name)  [$($b.Profile)]  $($b.Program)" -ForegroundColor Red }
            Write-Host "  Repite con -Arreglar para desactivarlas" -ForegroundColor Yellow
        }
    }
    Write-Host ""
}

function Revoke-AVFirewallApp {
    param([Parameter(Mandatory = $true, Position = 0)][string[]]$App)
    $line = 'Revoke-AVFirewallApp -App ' + (($App | ForEach-Object { ConvertTo-AVFwArg $_ }) -join ',')
    if (Invoke-AVFwElevated $line) { return }

    $mine = @(Get-NetFirewallRule -Group $AVFwGroup -ErrorAction SilentlyContinue)
    if (-not $mine.Count) { Write-Host "`n  No hay reglas creadas por fw-allow`n" -ForegroundColor DarkGray; return }

    $quitar = @()
    foreach ($a in $App) {
        if ($a -eq 'all') { $quitar = $mine; break }
        $key = $a.ToLower()
        $porTag = @($mine | Where-Object { (Get-AVFwTag $_.DisplayName) -eq $key })
        if ($porTag.Count) { $quitar += $porTag; continue }
        # Ruta o proceso: buscar por programa
        $exes = @{}
        foreach ($t in (Resolve-AVFwApp $a)) { $exes[$t.Exe.ToLower()] = $true }
        if ($exes.Count) {
            $quitar += @($mine | Where-Object {
                $p = ($_ | Get-NetFirewallApplicationFilter).Program
                $p -and $exes.ContainsKey($p.ToLower())
            })
        }
    }
    $quitar = @($quitar | Sort-Object Name -Unique)
    if (-not $quitar.Count) { Write-Host "`n  Nada que quitar para: $($App -join ', ')`n" -ForegroundColor DarkGray; return }

    Write-Host ""
    foreach ($r in $quitar) {
        $r | Remove-NetFirewallRule
        Write-Host "    -  $($r.DisplayName -replace "^$AVFwGroup ", '')" -ForegroundColor Yellow
    }
    Write-Host "  $($quitar.Count) reglas eliminadas`n" -ForegroundColor Green
}

function Show-AVFirewallBlocks {
    Write-Host "`n  --- Reglas de BLOQUEO de entrada activas ---" -ForegroundColor Yellow
    $b = @(Get-AVFwBlockRules)
    if (-not $b.Count) { Write-Host "    (ninguna)`n" -ForegroundColor DarkGray; return }
    $b | Sort-Object Name, Profile -Unique | ForEach-Object {
        Write-Host ("    {0,-30} {1,-16} {2}" -f $_.Name, $_.Profile, $_.Program) -ForegroundColor Red
    }
    Write-Host "  Suelen venir del aviso de Windows al pulsar Cancelar. Para quitar las de un programa:" -ForegroundColor DarkGray
    Write-Host "  fw-allow <ruta.exe> -Arreglar" -ForegroundColor DarkGray
    Write-Host ""
}

Set-Alias -Name fw-info   -Value Show-AVFirewall   # 'fw' no se puede: es alias interno de Format-Wide
Set-Alias -Name fw-off    -Value Disable-AVFirewall
Set-Alias -Name fw-on     -Value Enable-AVFirewall
Set-Alias -Name fw-allow  -Value Grant-AVFirewallApp
Set-Alias -Name fw-remove -Value Revoke-AVFirewallApp
Set-Alias -Name fw-blocks -Value Show-AVFirewallBlocks
