# === CueDeck.ps1 — launcher for cuedeck (terminal QLab-style cue player) ===
# The app lives at C:\Projects\cuelab (Python venv). This exposes it as `cuedeck`.
#   cuedeck                 -> opens shows\demo.json
#   cuedeck <show.json>     -> opens a show file
#   cuedeck <media_folder>  -> builds a show by scanning a folder
# Keys in-app: Space=GO  Enter=GO(no advance)  Esc=STOP ALL  s=stop+fade  w=save  0-9=screen  q=quit

function Invoke-CueDeck {
    $proj = 'C:\Projects\cuelab'
    $py = Join-Path $proj '.venv\Scripts\python.exe'
    if (-not (Test-Path $py)) {
        Write-Host "cuedeck no encontrado en $proj (.venv sin crear)." -ForegroundColor Red
        return
    }
    Push-Location $proj
    try { & $py (Join-Path $proj 'cuedeck.py') @args }
    finally { Pop-Location }
}

Set-Alias cuedeck Invoke-CueDeck
