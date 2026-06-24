#SingleInstance Off
Run "explorer.exe"
WinWait "ahk_class CabinetWClass"
WinActivate "ahk_class CabinetWClass"
WinMoveTop "ahk_class CabinetWClass"
ExitApp