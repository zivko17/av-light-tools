function Invoke-CustomSearch {
    param(
        [Parameter(Mandatory=$true)]
        [string]$Pattern,
        [string]$Path = (Get-Location).Path
    )
    Write-Host "Searching files named like '$Pattern' under '$Path'..." -ForegroundColor Cyan
    $results = New-Object System.Collections.Generic.List[object]
    # -Filter is applied by the file system itself: far faster than filtering afterwards.
    Get-ChildItem -LiteralPath $Path -Filter "*$Pattern*" -File -Recurse -ErrorAction SilentlyContinue |
    ForEach-Object {
        $results.Add($_)
        $index = $results.Count
        Write-Host "[$index] " -NoNewline -ForegroundColor Yellow
        Write-Host $_.FullName -ForegroundColor Green
        Write-Host "     Size: $([math]::Round($_.Length / 1KB, 2)) KB  |  Modified: $($_.LastWriteTime)" -ForegroundColor DarkGray
    }
    if ($results.Count -eq 0) {
        Write-Host "No files found." -ForegroundColor Red
        return
    }
    Write-Host "`nType the number of the file to open (or Enter to exit): " -ForegroundColor Cyan -NoNewline
    $selection = Read-Host
    if ($selection -match '^\d+$') {
        $num = [int]$selection
        if ($num -ge 1 -and $num -le $results.Count) {
            Start-Process $results[$num - 1].FullName
        } else {
            Write-Host "Number out of range." -ForegroundColor Red
        }
    }
}
Set-Alias -Name search -Value Invoke-CustomSearch
