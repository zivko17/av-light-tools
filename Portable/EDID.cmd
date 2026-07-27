@echo off
rem ===========================================================
rem  EDID Tools - portable, straight into the EDID menu.
rem  For the on-site "what is this screen actually reporting?"
rem  moment. Read-only; dumps land in ..\_OUTPUT\EDID.
rem ===========================================================
setlocal
set "HERE=%~dp0"

rem  Dot-sourced (not -File) so the edid* commands are still there when you
rem  leave the menu and land at the prompt.
where pwsh.exe >nul 2>&1
if %errorlevel%==0 (
    pwsh.exe -NoLogo -NoExit -ExecutionPolicy Bypass -Command ". '%HERE%Start-Portable.ps1' -Only edid"
) else (
    powershell.exe -NoLogo -NoExit -ExecutionPolicy Bypass -Command ". '%HERE%Start-Portable.ps1' -Only edid"
)
