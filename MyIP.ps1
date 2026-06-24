function Get-MyIP {
    Write-Host "`n=== NETWORK INFO ===" -ForegroundColor Cyan

    $adapters = Get-NetIPConfiguration | Where-Object { $_.IPv4Address -and $_.NetAdapter.Status -eq "Up" }

    foreach ($a in $adapters) {
        Write-Host "`n  [$($a.InterfaceAlias)]" -ForegroundColor Yellow
        Write-Host ("    Local IP:    {0}" -f $a.IPv4Address.IPAddress) -ForegroundColor Green
        if ($a.IPv4DefaultGateway) {
            Write-Host ("    Gateway:     {0}" -f $a.IPv4DefaultGateway.NextHop)
        }
        Write-Host ("    DNS:         {0}" -f ($a.DNSServer.ServerAddresses -join ", "))
        Write-Host ("    MAC:         {0}" -f $a.NetAdapter.MacAddress)

        $speed = $a.NetAdapter.LinkSpeed
        $color = if ($speed -match "^1 Gbps|^2.5 Gbps|^10 Gbps") { "Green" } elseif ($speed -match "^100 Mbps") { "Yellow" } else { "Red" }
        Write-Host ("    Link Speed:  {0}" -f $speed) -ForegroundColor $color
    }

    Write-Host "`n  [Public IP]" -ForegroundColor Yellow
    try {
        $publicIP = Invoke-RestMethod -Uri "https://api.ipify.org" -TimeoutSec 3
        Write-Host ("    {0}" -f $publicIP) -ForegroundColor Green
    } catch {
        Write-Host "    Could not retrieve (no internet?)" -ForegroundColor Red
    }
    Write-Host ""
}
Set-Alias -Name myip -Value Get-MyIP
