# AV Light Tools

**by A. Zivkovic**

A personal toolkit of PowerShell, AutoHotkey and batch scripts for live AV
production on Windows — networking, NDI, audio, media (FFmpeg / yt-dlp),
system diagnostics and Resolume automation, all behind one `tools` menu.

> Built and used daily on a live-events rig. Loads from the same source under
> both **Windows PowerShell 5.1** and **PowerShell 7**.

## Quick start

```powershell
# 1. Clone into an empty folder (any path; this example uses your home folder)
git clone https://github.com/zivko17/av-light-tools.git $HOME\av-light-tools
cd $HOME\av-light-tools

# 2. Run the installer — unblocks the files and adds a loader to your PowerShell profile
.\install.ps1

# 3. Reload your profile (or open a new terminal), then open the menu
. $PROFILE
tools
```

> Windows PowerShell 5.1 and PowerShell 7 use separate profiles — run
> `install.ps1` from each edition you want the tools in. Re-running it is safe.
> Already have a `C:\Scripts`? Clone into a *different* empty folder; `git clone`
> won't write into an existing directory.

### Manual setup

Prefer to wire it up by hand? Unblock the files and add a loader to your `$PROFILE`:

```powershell
Get-ChildItem . -Recurse | Unblock-File
'foreach ($f in (Get-ChildItem "$HOME\av-light-tools\*.ps1" | Where-Object { $_.Name -ne "install.ps1" })) { . $f.FullName }' | Add-Content $PROFILE
```

## The menu

`tools` opens an interactive, numbered launcher. It is **data-driven**: the
`$sections` array at the top of `Tools.ps1` defines every entry, and the
numbering and dispatch update automatically when you add a tool.

## What's inside

### Networking — `NetTools.ps1`
`netinfo`, `scan`, `portcheck` / `portscan`, `whoison`, `pingmulti`, `latency`,
`speed`, `connections`, `speedtest`, `flushdns`, `reset-nic`, plus the
`nettools` menu. Static IP profiles via `setip`.

### NDI — `NDITools.ps1`
`ndi` menu, `ndi-launch`, `ndi-check`, `ndi-services`, `ndi-config` (NDI 6 Tools).

### Audio — `AudioTools.ps1`
FFmpeg audio toolkit: convert presets (mp3 / aac / opus / flac / wav),
`normalize` (two-pass EBU R128 −23 LUFS), `trim`, `extract-audio`, `resample`,
mono/stereo, `probe-audio`, and the `audiotools` menu.

### System & utilities
`sysinfo`, `tele`, `top`, `killp`, `usb`, `disks`, `monitors`,
`monitors-arrange`, `battery`, `startup`, `audio`, `services-audio`, `myip`,
`cleanup`, `backup`, `search`, `weather`, `timer`.

### Resolume / hotkeys
`.ahk` and `.bat` helpers for Resolume Arena and common shortcuts.

## External tools (optional)

Each is resolved automatically if installed; an install hint is shown if missing:

| Tool | Install |
|------|---------|
| FFmpeg | `winget install Gyan.FFmpeg` |
| yt-dlp | `winget install yt-dlp.yt-dlp` |
| Ookla Speedtest | `winget install Ookla.Speedtest.CLI` |
| nircmd (display config) | <https://www.nirsoft.net/utils/nircmd.html> |

> The **AVTools** FFmpeg video module (`av` / `convert` / `probe` / `dl` —
> HAP / ProRes / NVENC) is a separate PowerShell 7 module and is not part of
> this repository.

## Customize for your setup

A few scripts default to the author's rig — **edit these before relying on them**:

- **Network interface** — `NetTools.ps1` auto-detects the active adapter (the one
  owning the default route). To force a specific NIC, set `$NetIface = "Ethernet 2"`
  in your profile.
- **Static IP profiles** — `SetIP.ps1` ships with `192.168.0.99` / `192.168.1.99`
  profiles. Edit the `$profiles` table for your own subnets; pass `-Iface` to target
  a specific adapter.
- **Weather location** — `Weather.ps1` defaults to Ibiza; change the default `-City`.
- **NDI version** — `NDITools.ps1` auto-detects the newest `NDI N Tools` install,
  falling back to NDI 6.

## License

Apache License 2.0 — see [LICENSE](LICENSE).
