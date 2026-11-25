@echo off
rem Same as tools.bat, as Administrator (setip, fw-*, scannet, reset-nic)
where pwsh.exe >nul 2>&1
if %errorlevel%==0 (
    powershell.exe -NoProfile -Command "Start-Process pwsh.exe -Verb RunAs -ArgumentList '-NoLogo','-NoExit','-Command','tools'"
) else (
    powershell.exe -NoProfile -Command "Start-Process powershell.exe -Verb RunAs -ArgumentList '-NoLogo','-NoExit','-Command','tools'"
)
