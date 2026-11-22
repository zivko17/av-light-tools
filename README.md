# AV Light Tools

**by A. Zivkovic**

## avlt (single binary)

`avlt.exe` is one portable Windows x64 executable that replaces the PowerShell
tools and both C# apps below: AV / IT calculators, EDID, network, system,
media (FFmpeg), NDI and utilities, as a CLI **and** as a GUI. Nothing to
install, no PowerShell profile, no runtime, no DLLs.

### Download

Grab `avlt.exe` (and `avlt.exe.sha256`) from the
[latest release](https://github.com/zivko17/av-light-tools/releases/latest).
To verify it:

```powershell
(Get-FileHash .\avlt.exe -Algorithm SHA256).Hash.ToLower()   # compare with avlt.exe.sha256
```

> **SmartScreen warning.** The executable is not code-signed, so Windows
> SmartScreen may show "Windows protected your PC" the first time. Click
> **More info**, then **Run anyway** (or tick *Unblock* in the file's
> Properties). Check the SHA-256 first if you want to be sure.

### Use

```powershell
avlt --help                       # groups: calc, edid, net, sys, media, ndi, util, gui
avlt calc subnet 192.168.1.0/24
avlt edid list
avlt net scan
avlt sys top
avlt gui                          # open the graphical interface
avlt net info --json              # every command can print JSON instead of text
```

- **GUI.** Double-clicking `avlt.exe` opens the GUI, and so does any launcher
  that starts it without a console (for example Task Scheduler with no
  arguments). Always pass a subcommand in scheduled tasks and shortcuts, e.g.
  `avlt.exe net scan --json`. From a terminal, `avlt` with no arguments prints
  the help; `avlt gui` opens the GUI.
- **Output.** Colors only when stdout is a terminal; redirected output is plain
  text. `--json` prints the result as JSON.
- **Exit codes.**

  | Code | Meaning |
  |------|---------|
  | 0 | OK |
  | 1 | Error |
  | 2 | Usage or invalid input |
  | 3 | A required external tool is missing (a `winget install` hint is printed) |
  | 4 | Needs administrator rights |
  | 5 | Not supported on this system |

- **Safety.** Commands that change the system (`net setip`, `net reset-nic`,
  `sys killp`, `sys cleanup`, `sys monitors-arrange`, `sys services-audio
  --restart`) ask for confirmation
  unless you pass `--yes`, and the ones that need administrator rights check
  that first. `sys killp` never kills protected system processes (svchost,
  csrss, explorer...) unless you pass `--include-critical` **and** confirm
  interactively; `--yes` does not answer that question. `sys cleanup` no longer
  touches `C:\Windows\Prefetch`. `edid` is read-only: it never writes an EDID or
  touches the display driver.
- **Existing files are never overwritten.** Outputs get a `_1`, `_2`... suffix
  on name collisions.

### Portable mode and output folder

If `avlt.toml` or an `_OUTPUT` folder sits next to `avlt.exe`, avlt runs in
**portable mode** and writes everything to `<exe folder>\_OUTPUT` (handy on a
USB stick). Otherwise output goes to `Documents\AVLT` (media output to
`Music\AVLT`), resolved through the Windows known folders, so redirected
folders (OneDrive, etc.) work. `output_dir` in `avlt.toml` overrides both, and
`--out` overrides it per command where available. Nothing is written to the
registry and no profile is touched.

### Configuration

Everything is optional. Copy [`avlt.example.toml`](avlt.example.toml) next to the
exe as `avlt.toml` to set the default network adapter (`iface`), the weather
city, the output folder and named static-IP profiles for `avlt net setip`.

### External tools (optional)

Only some commands need them. If one is missing, avlt prints the install
command and exits with code 3.

| Tool | Used by | Install |
|------|---------|---------|
| FFmpeg / ffprobe | `avlt media ...` | `winget install Gyan.FFmpeg` |
| Ookla Speedtest CLI | `avlt net speedtest` | `winget install Ookla.Speedtest.CLI` |
| NDI Tools | `avlt ndi ...` | <https://ndi.video/tools/> |

### Old command to `avlt`

| PowerShell (legacy) | avlt |
|---------------------|------|
| `calc-subnet` | `avlt calc subnet` |
| `calc-screen` | `avlt calc screen` |
| `calc-ledwall` | `avlt calc ledwall` |
| `calc-bitrate` | `avlt calc bitrate` |
| `calc-avoip` | `avlt calc avoip` |
| `calc-projector` | `avlt calc projector` |
| `calc-pixelpitch` | `avlt calc pixelpitch` |
| `calc-brightness` | `avlt calc brightness` |
| `calc-viewangle` | `avlt calc viewangle` |
| `calc-camera` | `avlt calc camera` |
| `calc-audiobw` | `avlt calc audiobw` |
| `calc-speaker` | `avlt calc speaker` |
| `calc-cable` | `avlt calc cable` |
| `calc-dsp` | `avlt calc dsp` |
| `edid-list` | `avlt edid list` |
| `edid-info` | `avlt edid info` |
| `edid-check` | `avlt edid check` |
| `edid-diff` | `avlt edid diff` |
| `edid-dump` | `avlt edid dump` |
| `netinfo` | `avlt net info` |
| `myip` | `avlt net myip` |
| `scan` | `avlt net scan` |
| `portcheck IP PORT` | `avlt net portcheck IP PORT` |
| `portscan` | `avlt net portscan` |
| `whoison` | `avlt net whoison` |
| `pingmulti` | `avlt net pingmulti` |
| `latency` | `avlt net latency` |
| `speed` | `avlt net speed` |
| `connections` | `avlt net connections` |
| `speedtest` | `avlt net speedtest` |
| `flushdns` | `avlt net flushdns` |
| `reset-nic` | `avlt net reset-nic` |
| `setip 0` / `setip dhcp` / `setip 192.168.1.25/24 192.168.1.1` | `avlt net setip 0` / `dhcp` / `192.168.1.25/24 192.168.1.1` |
| `sysinfo` | `avlt sys info` |
| `tele` | `avlt sys tele` |
| `top` | `avlt sys top` |
| `killp` | `avlt sys killp` |
| `usb` | `avlt sys usb` |
| `disks` | `avlt sys disks` |
| `monitors` | `avlt sys monitors` |
| `monitors-arrange` | `avlt sys monitors-arrange` |
| `audio` | `avlt sys audio` |
| `services-audio` | `avlt sys services-audio` |
| `startup` | `avlt sys startup` |
| `battery` | `avlt sys battery` |
| `cleanup` | `avlt sys cleanup` |
| `backup` | `avlt util backup` |
| `search` | `avlt util search` |
| `weather` | `avlt util weather` |
| `timer` | `avlt util timer` |
| `mp3`, `mp3-low`, `aac`, `opus`, `flac`, `wav16`, `wav24` | `avlt media convert --preset <preset>` |
| `normalize` | `avlt media normalize` |
| `trim-audio` | `avlt media trim` |
| `extract-audio` | `avlt media extract` |
| `resample` | `avlt media resample` |
| mono / stereo (audiotools menu) | `avlt media channels` |
| `probe-audio`, `probe` | `avlt media probe` |
| `ndi-launch` | `avlt ndi launch` |
| `ndi-check` | `avlt ndi check` |
| `ndi-services` | `avlt ndi services` |
| `ndi-config` | `avlt ndi config` |

**Not ported:**

- The interactive menus (`tools`, `nettools`, `calc`, `edid`, `ndi`, `audiotools`)
  are replaced by the GUI (`avlt gui`).
- `av`, `convert` (video) and `dl` (yt-dlp) belonged to the separate AVTools
  module, which was never part of this repository. `avlt media` covers audio only.
- `REATools.ps1`, `CueDeck.ps1`, the Ventoy scripts, `Setup-NewPC.ps1`,
  `install.ps1`, `Portable/` and the Resolume `.ahk` / `.bat` helpers stay as scripts.
- The C# apps `AVToolbox/` and `NetMaint/` are superseded by `avlt gui`.

### Build from source

```powershell
cargo build --release -p avlt     # target\release\avlt.exe
```

Pushing a `v*` tag runs `.github/workflows/release.yml`, which builds on
`windows-latest` and publishes `avlt.exe` plus its SHA-256 as a GitHub Release.

---

## Legacy: PowerShell scripts

> Everything below documents the original PowerShell toolkit. It still works,
> but the calculators, EDID, network, system, media, NDI and utility tools are
> now covered by `avlt.exe` above.

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
