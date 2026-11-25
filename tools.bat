@echo off
rem Opens PowerShell straight into the tools menu (PowerShell 7 if installed, else 5.1)
where pwsh.exe >nul 2>&1
if %errorlevel%==0 (
    start "AV Light Tools" pwsh.exe -NoLogo -NoExit -Command "tools"
) else (
    start "AV Light Tools" powershell.exe -NoLogo -NoExit -Command "tools"
)
