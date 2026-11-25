# avlt (Rust) — aparcado

> **Estado: aparcado.** Este árbol está aquí guardado, no en uso. Los workflows de CI/release
> están desactivados (movidos a `github-workflows/`, GitHub solo lee `.github/workflows/`).
>
> Para reactivarlo: devolver `crates/`, `Cargo.toml`, `Cargo.lock`, `.cargo/`, `clippy.toml`,
> `avlt.example.toml` a la raíz y `github-workflows/*.yml` a `.github/workflows/`.
> O compilar desde aquí: `cd .rust && cargo build --release -p avlt`.

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

