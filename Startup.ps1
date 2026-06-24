function Get-StartupPrograms {
    Write-Host "`n=== STARTUP PROGRAMS ===" -ForegroundColor Cyan
    Write-Host ""

    $sources = @(
        @{ Name = "HKLM Run";       Path = "HKLM:\Software\Microsoft\Windows\CurrentVersion\Run" },
        @{ Name = "HKLM RunOnce";   Path = "HKLM:\Software\Microsoft\Windows\CurrentVersion\RunOnce" },
        @{ Name = "HKCU Run";       Path = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" },
        @{ Name = "HKCU RunOnce";   Path = "HKCU:\Software\Microsoft\Windows\CurrentVersion\RunOnce" },
        @{ Name = "HKLM x86 Run";   Path = "HKLM:\Software\Wow6432Node\Microsoft\Windows\CurrentVersion\Run" }
    )

    $found = @()
    foreach ($src in $sources) {
        if (Test-Path $src.Path) {
            $items = Get-ItemProperty -Path $src.Path -ErrorAction SilentlyContinue
            if ($items) {
                $items.PSObject.Properties | Where-Object { $_.Name -notmatch '^PS' } | ForEach-Object {
                    $found += [PSCustomObject]@{
                        Source  = $src.Name
                        Name    = $_.Name
                        Command = $_.Value
                    }
                }
            }
        }
    }

    # Startup folders
    $startupFolders = @(
        "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup",
        "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Startup"
    )
    foreach ($f in $startupFolders) {
        if (Test-Path $f) {
            Get-ChildItem -Path $f -File -ErrorAction SilentlyContinue | ForEach-Object {
                $found += [PSCustomObject]@{
                    Source  = "Startup folder"
                    Name    = $_.Name
                    Command = $_.FullName
                }
            }
        }
    }

    if ($found.Count -eq 0) {
        Write-Host "  No startup programs found.`n" -ForegroundColor Yellow
        return
    }

    $grouped = $found | Group-Object Source
    foreach ($g in $grouped) {
        Write-Host "  --- $($g.Name) ---" -ForegroundColor Yellow
        foreach ($item in $g.Group) {
            Write-Host "    $($item.Name)" -ForegroundColor Green
            $cmd = if ($item.Command.Length -gt 100) { $item.Command.Substring(0, 100) + "..." } else { $item.Command }
            Write-Host "      $cmd" -ForegroundColor DarkGray
        }
        Write-Host ""
    }

    Write-Host "  Total: $($found.Count) startup entries`n" -ForegroundColor Cyan
    Write-Host "  Tip: use Task Manager > Startup tab to enable/disable items.`n" -ForegroundColor DarkGray
}
Set-Alias -Name startup -Value Get-StartupPrograms
