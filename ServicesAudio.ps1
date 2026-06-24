function Get-AudioServices {
    Write-Host "`n=== AUDIO SERVICES ===" -ForegroundColor Cyan
    Write-Host ""

    $audioServices = @(
        @{ Name = "Audiosrv";           Desc = "Windows Audio" },
        @{ Name = "AudioEndpointBuilder"; Desc = "Audio Endpoint Builder" },
        @{ Name = "MMCSS";              Desc = "Multimedia Class Scheduler" }
    )

    # Add Dante services if present
    $danteServices = Get-Service -Name "Dante*" -ErrorAction SilentlyContinue
    foreach ($d in $danteServices) {
        $audioServices += @{ Name = $d.Name; Desc = $d.DisplayName }
    }

    Write-Host ("  {0,-30} {1,-15} {2}" -f "SERVICE", "STATUS", "DESCRIPTION") -ForegroundColor Yellow
    Write-Host ("  {0,-30} {1,-15} {2}" -f "------------------------------", "---------------", "------------") -ForegroundColor DarkGray

    foreach ($s in $audioServices) {
        $svc = Get-Service -Name $s.Name -ErrorAction SilentlyContinue
        if ($svc) {
            $status = $svc.Status
            $color = if ($status -eq "Running") { "Green" } elseif ($status -eq "Stopped") { "Red" } else { "Yellow" }
            Write-Host ("  {0,-30} {1,-15} {2}" -f $svc.Name, $status, $s.Desc) -ForegroundColor $color
        }
    }

    Write-Host ""
    Write-Host "  Tip: to restart audio, run as Admin:" -ForegroundColor DarkGray
    Write-Host "       Restart-Service Audiosrv -Force" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name services-audio -Value Get-AudioServices
