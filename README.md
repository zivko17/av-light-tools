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

Prefer to wire it up by hand? Unblock the files, then add this block to your
`$PROFILE` (edit the path). It dot-sources **library** scripts only: files whose
name starts with `_` and *command* scripts — the ones with a top-level `param()`
block, like `SetIP.ps1` or `Setup-NewPC.ps1` — are skipped, because sourcing
those would execute them at every startup.

```powershell
Get-ChildItem . -Recurse | Unblock-File     # run once
```

```powershell
$__avltDir = "$HOME\av-light-tools"
foreach ($__avltF in (Get-ChildItem "$__avltDir\*.ps1" -ErrorAction SilentlyContinue)) {
    if ($__avltF.Name -like '_*') { continue }
    $__avltH = Get-Content -LiteralPath $__avltF.FullName -TotalCount 60 -ErrorAction SilentlyContinue
    if ($__avltH -match '^param\s*\(') { continue }
    . $__avltF.FullName
}
if (Test-Path "$__avltDir\SetIP.ps1") { Set-Alias -Name setip -Value "$__avltDir\SetIP.ps1" }
```

### New machine

`Setup-NewPC.ps1` does the whole bootstrap in one go — copies the tools to
`C:\Scripts`, unblocks them, configures **both** PowerShell profiles and reports
which external binaries are missing:

```powershell
.\Setup-NewPC.ps1                 # copy + both profiles + dependency check
.\Setup-NewPC.ps1 -InstallDeps    # also winget-install what's missing
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

### EDID — `EDIDTools.ps1`
`edid` menu, plus `edid-list`, `edid-info`, `edid-check`, `edid-diff`, `edid-dump`.
Reads the raw EDID of every display (registry, falling back to WMI) and decodes
the base block and the CTA-861 extension: native timing, range limits, VICs,
audio SADs, HDMI / HDMI Forum vendor blocks, HDR static metadata and colorimetry.

```powershell
edid-list                       # what every connected display reports
edid-info 1                     # full decode of display 1
edid-check                      # checksums + timings vs declared limits
edid-dump 1                     # save the raw .bin
edid-diff 1 projector.bin       # direct vs through the switcher/extender
```

`edid-diff` is the one that earns its keep on site: dump the projector plugged
in directly, dump it again through the switcher or extender, and the diff shows
exactly which VICs, audio blocks or TMDS limits went missing on the way.

**Read-only by design** — it never writes an EDID override or touches the
display driver. To *force* an EDID use [CRU](https://www.monitortests.com/forum/Thread-Custom-Resolution-Utility-CRU)
(ToastyX), [AW EDID Editor](https://www.analogway.com) or a hardware EDID emulator;
this tool tells you what is actually being reported so you know what to force.

### Audio — `AudioTools.ps1`
FFmpeg audio toolkit: convert presets (mp3 / aac / opus / flac / wav),
`normalize` (two-pass EBU R128 −23 LUFS), `trim`, `extract-audio`, `resample`,
mono/stereo, `probe-audio`, and the `audiotools` menu.

### AV / IT calculators — `AVCalc.ps1`
Offline design maths, `calc` for the menu or the commands directly:
`calc-ledwall` (real resolution, power, NovaStar cabling, weight),
`calc-screen`, `calc-projector` (throw + lumens), `calc-pixelpitch`,
`calc-bitrate`, `calc-avoip`, `calc-camera` (FOV + DORI), `calc-brightness`,
`calc-viewangle`, `calc-audiobw`, `calc-speaker`, `calc-cable`, `calc-dsp`,
`calc-subnet`.

> Where a figure comes from a standard's *guideline* (AVIXA viewing ratios,
> ambient-light contrast) or is a codec **estimate** (NDI / AVoIP), the output
> says so. Throw, pitch, DORI, subnet and wall geometry are exact maths.

### Reverse engineering — `REATools.ps1`
`reatools` menu, plus `rea-doctor` and `rea-run`. A thin front-end over the
[REA](https://github.com/morluto/rea) CLI (`npm install --global rea-agents`)
for inspecting software you only have as a shipped artifact: native binaries,
Android APKs, JavaScript / Electron apps, .NET assemblies, SQLite snapshots.

```powershell
reatools                               # the menu
rea-doctor                             # which engines are configured and healthy
rea analyze app.exe                    # overview of an app or binary
rea decompile app.exe entry            # pseudocode of one procedure (Ghidra)
rea inspect-android-package app.apk    # package, SDK, permissions, manifest (JADX)
```

Deep-analysis engines are optional and caller-supplied — REA never downloads
them. They are picked up from the environment:

| Variable | Engine | What it unlocks |
|----------|--------|-----------------|
| `JAVA_HOME` | full JDK 21+ (64-bit) | required by both Ghidra and JADX |
| `GHIDRA_INSTALL_DIR` | [Ghidra](https://github.com/NationalSecurityAgency/ghidra) 12.1.x | native PE / ELF / Mach-O decompilation |
| `REA_JADX_MCP_JAR` | `jadx-headless-mcp` 0.7.1 JAR | Android APK classes and methods |

The menu footer shows which of the three are live, and `rea-doctor` explains
what is missing. Static JavaScript / Electron, .NET and SQLite inspection needs
none of them.

**Read-only on the target** — REA never executes the artifact it analyzes, and
the imported analysis database is ephemeral. Every result carries an Evidence
record with the artifact's SHA-256, the engine identity and the limitations of
that specific observation.

### System & utilities
`sysinfo`, `tele`, `top`, `killp`, `usb`, `disks`, `monitors`,
`monitors-arrange`, `battery`, `startup`, `audio`, `services-audio`, `myip`,
`cleanup`, `backup`, `search`, `weather`, `timer`, `cuedeck`.

### Resolume / hotkeys
`.ahk` and `.bat` helpers for Resolume Arena and common shortcuts.

## GUI companions (C#, .NET 8)

Two small WinForms apps that cover the same ground with a mouse, for when a
console isn't practical. Source only — build them with `dotnet build`:

| Project | What it is |
|---------|------------|
| `AVToolbox/` | Audio / NDI / network / calculator front-end over the same workflows |
| `NetMaint/`  | Network maintenance: adapter info, IP config, ping, port scan, subnet sweep, speedtest, traffic graph |

## Portable mode (USB stick)

The toolkit also runs with **nothing installed on the host PC** — useful when
you are on someone else's rig and don't want to touch their profile.

```powershell
# Copy the whole folder to the stick, then on the target PC:
E:\av-light-tools\Portable\AV-Tools.cmd     # everything, opens the tools menu
E:\av-light-tools\Portable\EDID.cmd         # straight into the EDID menu
```

Both `.cmd` files are double-clickable. They prefer PowerShell 7 and fall back
to Windows PowerShell 5.1, which is on every Windows machine. From an already
open prompt, **dot-source** the launcher (the leading dot matters — without it
the commands vanish when the script returns):

```powershell
. E:\av-light-tools\Portable\Start-Portable.ps1
```

What portable mode does:

- Loads every tool into that one session only. **`$PROFILE` is never touched**,
  nothing is written to the registry, nothing is installed.
- Sends output that would normally go to the host's Documents / Music / Videos
  folders to `<stick>\_OUTPUT` instead, so EDID dumps stay with you.
- Tells you up front which optional binaries (FFmpeg, yt-dlp, Speedtest) are
  missing on that PC.
- Loads library scripts by dot-sourcing them, and exposes scripts that have a
  top-level `param()` block (`setip`, `setup-newpc`, …) as aliases instead — the
  distinction is read from the script's AST, so dropping extra `.ps1` files into
  the folder can't make the loader run them by accident.

Close the window and nothing is left behind.

> If a machine enforces an execution policy through Group Policy, the
> `-ExecutionPolicy Bypass` in the launcher is ignored and the scripts won't
> run — that's a locked-down PC, not a bug.

## External tools (optional)

Each is resolved automatically if installed; an install hint is shown if missing:

| Tool | Install |
|------|---------|
| FFmpeg | `winget install Gyan.FFmpeg` |
| yt-dlp | `winget install yt-dlp.yt-dlp` |
| Ookla Speedtest | `winget install Ookla.Speedtest.CLI` |
| nircmd (display config) | <https://www.nirsoft.net/utils/nircmd.html> |
| Node.js 22+ (for REA) | `winget install OpenJS.NodeJS.LTS` |
| REA CLI | `npm install --global rea-agents` |

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
- **Weather location** — `Weather.ps1` defaults to Madrid; change the default `-City`.
- **NDI version** — `NDITools.ps1` auto-detects the newest `NDI N Tools` install,
  falling back to NDI 6.

## License

Apache License 2.0 — see [LICENSE](LICENSE).
