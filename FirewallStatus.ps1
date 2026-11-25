function Get-FirewallStatus {
    Write-Host "`n=== FIREWALL STATUS ===" -ForegroundColor Cyan
    Write-Host ""

    # Profile status
    Write-Host "  --- Profiles ---" -ForegroundColor Yellow
    $profiles = Get-NetFirewallProfile -ErrorAction SilentlyContinue
    if ($profiles) {
        Write-Host ("    {0,-15} {1,-10} {2,-25} {3}" -f "PROFILE", "ENABLED", "INBOUND DEFAULT", "OUTBOUND DEFAULT") -ForegroundColor DarkGray
        Write-Host ("    {0,-15} {1,-10} {2,-25} {3}" -f ("-" * 15), ("-" * 10), ("-" * 25), ("-" * 25)) -ForegroundColor DarkGray
        foreach ($p in $profiles) {
            $color = if ($p.Enabled) { "Green" } else { "Red" }
            Write-Host ("    {0,-15} {1,-10} {2,-25} {3}" -f $p.Name, $p.Enabled, $p.DefaultInboundAction, $p.DefaultOutboundAction) -ForegroundColor $color
        }
    }
    Write-Host ""

    # Active rules summary
    Write-Host "  --- Active rules summary ---" -ForegroundColor Yellow
    $allRules = Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.Enabled -eq $true }
    $inbound  = $allRules | Where-Object { $_.Direction -eq "Inbound" }
    $outbound = $allRules | Where-Object { $_.Direction -eq "Outbound" }
    $allow    = $allRules | Where-Object { $_.Action -eq "Allow" }
    $block    = $allRules | Where-Object { $_.Action -eq "Block" }

    Write-Host ("    Total enabled rules:  {0}" -f $allRules.Count) -ForegroundColor Green
    Write-Host ("      Inbound:            {0}" -f $inbound.Count) -ForegroundColor Gray
    Write-Host ("      Outbound:           {0}" -f $outbound.Count) -ForegroundColor Gray
    Write-Host ("      Allow:              {0}" -f $allow.Count) -ForegroundColor Gray
    Write-Host ("      Block:              {0}" -f $block.Count) -ForegroundColor Gray
    Write-Host ""

    # Recent custom rules (not Microsoft default)
    Write-Host "  --- Custom inbound BLOCK rules (top 10) ---" -ForegroundColor Yellow
    $blockInbound = $allRules | Where-Object { $_.Action -eq "Block" -and $_.Direction -eq "Inbound" } | Select-Object -First 10
    if ($blockInbound) {
        foreach ($r in $blockInbound) {
            $name = if ($r.DisplayName.Length -gt 70) { $r.DisplayName.Substring(0, 70) + ".." } else { $r.DisplayName }
            Write-Host ("    $name") -ForegroundColor Red
        }
    } else {
        Write-Host "    (no custom block rules)" -ForegroundColor DarkGray
    }
    Write-Host ""
    Write-Host "  Tip: open firewall panel -> start wf.msc" -ForegroundColor DarkGray
    Write-Host ""
}
Set-Alias -Name firewall-status -Value Get-FirewallStatus
