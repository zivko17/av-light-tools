function Get-Weather {
    param(
        [string]$City = "Ibiza",
        [ValidateSet("full","short","mini")]
        [string]$Format = "short"
    )
    try {
        switch ($Format) {
            "full"  { $url = "https://wttr.in/${City}?lang=en&M" }
            "short" { $url = "https://wttr.in/${City}?lang=en&M&format=v2" }
            "mini"  { $url = "https://wttr.in/${City}?lang=en&M&format=%l:+%c+%t+%w+%h" }
        }
        $result = Invoke-RestMethod -Uri $url -TimeoutSec 5
        Write-Host ""
        Write-Host $result
        Write-Host ""
    } catch {
        Write-Host "Could not fetch weather. Check internet connection." -ForegroundColor Red
    }
}
Set-Alias -Name weather -Value Get-Weather
