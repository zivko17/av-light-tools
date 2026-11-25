# AV Light Tools

**by A. Zivkovic**

A small PowerShell + AutoHotkey toolkit for live AV production on Windows:
network checks, EDID, audio and system diagnostics, all behind one `tools` menu.
Works in **Windows PowerShell 5.1** and **PowerShell 7**.

## Quick start

```powershell
git clone https://github.com/zivko17/av-light-tools.git C:\Scripts
cd C:\Scripts
.\install.ps1        # unblocks the files and adds a loader to your $PROFILE
. $PROFILE
tools                # the menu
```

On a new PC, `.\Setup-NewPC.ps1` does all of that for both PowerShell editions
and lists what is missing (`-InstallDeps` installs it with winget).

On someone else's PC, without touching anything: copy the folder to a USB stick
and double-click `Portable\AV-Tools.cmd` (or `Portable\EDID.cmd` to go straight
to EDID). Nothing is installed and output goes to `<stick>\_OUTPUT`.

## What's inside

### Network — `NetTools.ps1`, `SetIP.ps1`

| Command | What it does |
|---------|--------------|
| `netinfo` | IP, mode (DHCP/static), gateway, DNS, MAC and link speed of every active adapter |
| `scan` | Who is on the network: IP + MAC. `scan` uses this PC's network, `scan 192.168.10` another one. Devices that block ping still show up (from the ARP table) |
| `pingmulti 192.168.1.10 192.168.1.20` | Live monitor: online/offline, latency, lost pings |
| `portcheck IP PORT` | Is a TCP port open? |
| `whoison IP` | MAC and vendor of a device |
| `speed` | Live download/upload of the active adapter |
| `speedtest` | Internet speed test (Ookla CLI) |
| `flushdns`, `reset-nic` | Quick fixes |
| `setip 0` / `setip 1` / `setip dhcp` / `setip 192.168.1.25/24 [gateway]` | Change the IP (Administrator) |

### Displays — `EDIDTools.ps1`, `Monitors.ps1`, `MonitorsArrange.ps1`

- `edid` menu, `edid-list`, `edid-info 1`, `edid-check`, `edid-dump 1`, `edid-diff 1 projector.bin`.
  Reads and decodes what each display really reports (timings, VICs, audio, HDMI, HDR).
  `edid-diff` shows what gets lost through a switcher or extender. **Read-only**: it never writes an EDID.
- `monitors`: every output with its current resolution and refresh rate.
- `monitors-arrange`: change resolution / Hz from a menu (needs `nircmd.exe`).

### Audio — `AudioTools.ps1`, `Audio.ps1`, `ServicesAudio.ps1`

- `audiotools` menu (FFmpeg): convert to MP3 / AAC / Opus / FLAC / WAV, `normalize`
  (two-pass EBU R128 −23 LUFS), trim, extract audio from video, resample, mono/stereo, `probe-audio`.
  Works on a file or a whole folder; output goes to the Music folder.
- `audio`: playback / recording devices and volume (needs the `AudioDeviceCmdlets` module).
- `services-audio`: Windows audio and Dante services.

### System and utilities

`sysinfo`, `tele` (live CPU/RAM/GPU/disk/net), `top`, `killp <name>`, `usb`, `disks`,
`startup`, `battery`, `cleanup`, `backup "src" "dest"`, `search "text"`, `timer 5m`, `weather Ibiza`.

### AutoHotkey helpers (`.ahk`)

Resolume Arena shortcuts (save, open, copy/paste, snapshot, test card...) and general ones
(volume, copy/paste, screenshot), for a Stream Deck or a hotkey. They need AutoHotkey v2.
`volume_up.bat`, `volume_down.bat` and `nvidia_control_panel.bat` are small launchers.

### USB toolkit (Ventoy)

`New-VentoyStructure.ps1` creates the folder layout on the stick and
`Get-VentoyTools.ps1` downloads the portable apps (CRU, Sysinternals, PuTTY, FFmpeg, iPerf3, Wireshark...).

### NetMaint (C#, .NET 8)

A WinForms network app (adapter info, IP config, ping, ports, scan, speedtest, traffic).
Source only: build it with `dotnet build NetMaint`.

## External tools (optional)

| Tool | Used by | Install |
|------|---------|---------|
| FFmpeg | `audiotools` | `winget install Gyan.FFmpeg` |
| Ookla Speedtest | `speedtest` | `winget install Ookla.Speedtest.CLI` |
| NirCmd | `monitors-arrange` | <https://www.nirsoft.net/utils/nircmd.html> (put `nircmd.exe` next to the scripts) |
| AudioDeviceCmdlets | `audio` | `Install-Module AudioDeviceCmdlets -Scope CurrentUser` |

## Adjust for your setup

- **Network adapter**: detected automatically. To force one, put `$NetIface = "Ethernet 2"` in your profile.
- **Static IP profiles**: edit the `$profiles` table at the top of `SetIP.ps1`.
- **Weather city**: `weather Madrid`, or change the default in `Weather.ps1`.

## Rust (`avlt`)

The Rust rewrite is parked in the hidden `.rust/` folder (CI disabled). See `.rust/README.md`.

## License

Apache License 2.0 — see [LICENSE](LICENSE).
