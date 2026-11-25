function Get-Monitors {
    Write-Host "`n=== CONNECTED MONITORS ===" -ForegroundColor Cyan

    # Current mode (resolution + refresh) of each output, straight from the display driver.
    if (-not ('AVLT.DisplayMode' -as [type])) {
        Add-Type -Namespace AVLT -Name DisplayMode -MemberDefinition @'
[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
public struct DEVMODE {
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
    public short dmSpecVersion, dmDriverVersion, dmSize, dmDriverExtra;
    public int dmFields, dmPositionX, dmPositionY, dmDisplayOrientation, dmDisplayFixedOutput;
    public short dmColor, dmDuplex, dmYResolution, dmTTOption, dmCollate;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
    public short dmLogPixels;
    public int dmBitsPerPel, dmPelsWidth, dmPelsHeight, dmDisplayFlags, dmDisplayFrequency;
    public int dmICMMethod, dmICMIntent, dmMediaType, dmDitherType, dmReserved1, dmReserved2, dmPanningWidth, dmPanningHeight;
}
[DllImport("user32.dll", CharSet = CharSet.Unicode)]
public static extern bool EnumDisplaySettings(string deviceName, int modeNum, ref DEVMODE devMode);
'@
    }
    Add-Type -AssemblyName System.Windows.Forms

    $i = 1
    foreach ($s in [System.Windows.Forms.Screen]::AllScreens) {
        $dm = New-Object AVLT.DisplayMode+DEVMODE
        $dm.dmSize = [System.Runtime.InteropServices.Marshal]::SizeOf($dm)
        $hz = if ([AVLT.DisplayMode]::EnumDisplaySettings($s.DeviceName, -1, [ref]$dm)) { "$($dm.dmDisplayFrequency) Hz" } else { "? Hz" }
        $primary = if ($s.Primary) { " (PRIMARY)" } else { "" }
        Write-Host "`n[Monitor $i]$primary" -ForegroundColor Yellow
        Write-Host "  Output:      $($s.DeviceName)" -ForegroundColor White
        Write-Host "  Mode:        $($s.Bounds.Width) x $($s.Bounds.Height) @ $hz" -ForegroundColor Green
        Write-Host "  Position:    X=$($s.Bounds.X)  Y=$($s.Bounds.Y)" -ForegroundColor DarkGray
        $i++
    }

    Write-Host "`n=== MODELS ===" -ForegroundColor Cyan
    Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction SilentlyContinue | ForEach-Object {
        $manufacturer = -join ($_.ManufacturerName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ })
        $model  = -join ($_.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ })
        $serial = -join ($_.SerialNumberID   | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ })
        Write-Host "  $manufacturer  $model  (S/N: $serial)" -ForegroundColor White
    }
    Write-Host ""
}
Set-Alias -Name monitors -Value Get-Monitors
