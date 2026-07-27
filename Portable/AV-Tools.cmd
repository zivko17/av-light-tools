@echo off
rem ===========================================================
rem  AV Light Tools - portable launcher (double-click me)
rem  Loads the whole toolkit from this folder. Installs nothing.
rem  Prefers PowerShell 7, falls back to Windows PowerShell 5.1.
rem ===========================================================
setlocal
set "HERE=%~dp0"

rem  The launcher must be DOT-SOURCED, not run with -File: functions defined by
rem  a -File script live in that script's scope and are gone by the time you get
rem  back to the prompt. Dot-sourcing loads them into the session itself.
where pwsh.exe >nul 2>&1
if %errorlevel%==0 (
    pwsh.exe -NoLogo -NoExit -ExecutionPolicy Bypass -Command ". '%HERE%Start-Portable.ps1' %*"
) else (
    powershell.exe -NoLogo -NoExit -ExecutionPolicy Bypass -Command ". '%HERE%Start-Portable.ps1' %*"
)
