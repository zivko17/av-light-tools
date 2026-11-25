# AVLT: reescritura en Rust como binario único — Plan de implementación

> **Para agentes:** SUB-SKILL OBLIGATORIA: usa `superpowers:subagent-driven-development` para ejecutar este plan tarea a tarea. Las tareas usan casillas (`- [ ]`) para el seguimiento. Lee primero las secciones "Restricciones globales" y "Agentes y orquestación".

**Objetivo:** sustituir los scripts PowerShell y las dos apps C# por un único `avlt.exe` (Windows x64) que contiene todas las herramientas, usable como CLI y como GUI, sin instalar nada en el PC.

**Arquitectura:** workspace de Cargo con tres crates. `avlt-core` contiene toda la lógica pura (cálculos, decodificación EDID, modelos, parseo) y compila y se testea en Linux. `avlt-platform` contiene todo lo que toca Windows (registro, WMI, ICMP, adaptadores, servicios) detrás de funciones con la misma firma en todas las plataformas; fuera de Windows devuelven `Error::Unsupported`. `avlt` es el binario: CLI con `clap` y GUI con `egui` sobre las mismas funciones.

**Stack:** Rust estable, `clap` (derive), `serde` + `serde_json`, `toml`, `thiserror`, `anyhow` (solo en el binario), `tokio`, `eframe`/`egui`, `windows`, `wmi`, `winreg`, `sysinfo`, `ureq`. Usa la última versión estable de cada crate en el momento de añadirlo y deja que `Cargo.lock` (versionado) la fije.

**Spec:** no hay documento de diseño aparte. La especificación de comportamiento es el código actual: cada tarea cita el `.ps1` o `.cs` que porta. Este plan fija las decisiones que ese código no contiene.

## Alcance

Dentro: todo lo que hoy exponen `Tools.ps1` y el README salvo lo listado abajo.

Fuera (no tocar, no portar, no borrar):

- Todos los `.ahk` y `.bat` (hotkeys de Resolume y de volumen).
- `CueDeck.ps1`, `Get-VentoyTools.ps1`, `New-VentoyStructure.ps1`, `Setup-NewPC.ps1`, `install.ps1`, `Portable/`.
- Los `.ps1` existentes y las carpetas `AVToolbox/` y `NetMaint/` se quedan en el repo como referencia durante toda la migración. Este plan no los elimina.

## Restricciones globales

- El binario se llama `avlt` (`avlt.exe`). Un solo ejecutable; sin DLLs ni runtime.
- Target de entrega: `x86_64-pc-windows-msvc`, compilado en GitHub Actions (`windows-latest`). En la sesión de desarrollo (Linux) se verifica con `cargo test` y con `cargo check --target x86_64-pc-windows-gnu`.
- `avlt-core` no depende de `windows`, `wmi`, `winreg` ni de nada con `cfg(windows)`. Todo lo que se pueda testear en Linux vive ahí.
- Las funciones de `avlt-core` y `avlt-platform` devuelven structs con `#[derive(Serialize)]`; nunca imprimen. Solo `avlt/src/cli/` y `avlt/src/gui/` presentan.
- Todo subcomando acepta `--json` (imprime el struct serializado) además de la salida de texto.
- Sin `unwrap()`/`expect()` fuera de tests. Sin `unsafe` fuera de `avlt-platform`.
- Color solo si stdout es un terminal; con salida redirigida, texto plano.
- Los nombres de subcomando conservan los actuales: `calc-subnet` pasa a `avlt calc subnet`, `edid-diff` a `avlt edid diff`, `scan` a `avlt net scan`, etc. (tabla completa en la Tarea 0).
- Nunca se escribe un EDID ni se toca el driver de pantalla: `edid` es solo lectura, como hoy.
- Las acciones destructivas o que cambian el sistema (`setip`, `reset-nic`, `killp`, `cleanup`, `monitors arrange`) piden confirmación salvo con `--yes`, y las que requieren administrador lo comprueban y fallan con mensaje claro si no lo son.
- Commits pequeños, uno por tarea como mínimo, en la rama de trabajo de la sesión. No abrir PR salvo que el usuario lo pida.
- Idioma: código, comentarios, mensajes de la app y README en inglés (como el repo actual).

## Correcciones respecto al código actual

Al portar, no reproducir estos comportamientos:

1. `top` ordena por segundos de CPU acumulados. La versión Rust ordena por uso de CPU instantáneo (delta entre muestras).
2. `cleanup` borra `C:\Windows\Prefetch`. No incluirlo.
3. `scan` consulta la tabla de vecinos una vez por host. Leerla una sola vez y cruzar por IP.
4. `portcheck` usa `Test-NetConnection`. Usar conexión TCP con timeout (1000 ms), igual que `portscan`.
5. `whoison` depende de `api.macvendors.com`. Buscar primero en una tabla OUI embebida y usar la API solo como segundo intento.
6. El menú de `NetTools.ps1` llama a `C:\Scripts\SetIP.ps1` con ruta fija. En Rust `setip` es un subcomando más.

## Foco de revisión

Entradas y condiciones que el código actual no deja claras y que más probablemente fallen en uso real. Cada una tiene su test en la tarea indicada.

1. Entrada inválida en calculadoras: CIDR mal formado, prefijos `/0`, `/31`, `/32`, aspect ratio `16:0`, valores negativos o cero. Esperado: error con mensaje de uso, código de salida 2, sin pánico. (Tarea 1)
2. EDID corrupto o truncado: archivo de menos de 128 bytes, cabecera incorrecta, número de extensiones mayor que los bloques presentes, checksum malo. Esperado: `edid info` decodifica lo que pueda y marca el problema; `edid check` lo reporta; nunca pánico. (Tarea 2)
3. PC sin red: ningún adaptador activo, sin puerta de enlace, sin internet. Esperado: `net info`, `net scan`, `myip`, `whoison`, `weather` devuelven un resultado parcial o un error legible. (Tarea 3)
4. Binario externo ausente (`ffmpeg`, `ffprobe`, `yt-dlp`, `speedtest`). Esperado: mensaje con el comando `winget install` correspondiente, código de salida 3. (Tarea 5)
5. Rutas con espacios, acentos y caracteres no ASCII en archivos de entrada y salida, y colisión de nombre de salida. Esperado: funcionan; nunca se sobrescribe un archivo existente (sufijo `_1`, `_2`…). (Tareas 2 y 5)

## Agentes y orquestación

La sesión principal (Opus) solo orquesta: reparte tareas, revisa informes, integra y lleva el seguimiento con `TaskCreate`/`TaskUpdate`. No implementa.

| Rol | Agente | Modelo | Cuándo |
|---|---|---|---|
| Implementador | `general-purpose`, con `isolation: "worktree"` si hay otra tarea en paralelo | `opus` para Tareas 0, 2, 3, 4 y 6; `sonnet` para Tareas 1, 5 y 7 | Uno nuevo por tarea (o por subtarea donde se indica) |
| Revisor de paridad | `general-purpose` nuevo, sin el contexto del implementador | `opus` | Tras cada tarea, antes de integrarla |
| Revisión de fase | skill `code-review` sobre el diff de la rama | — | Al cerrar las Tareas 3, 4 y 6 |
| Verificación final | skill `superpowers:verification-before-completion` | — | Antes de dar el plan por terminado |

No hace falta `Explore` ni `Plan`: cada implementador lee directamente los archivos fuente que su tarea cita, y este documento ya es el plan.

**Brief del implementador** (incluye siempre): el texto completo de su tarea, las secciones "Restricciones globales" y "Correcciones respecto al código actual", las rutas de los `.ps1`/`.cs` que porta, y la orden de seguir el ciclo test → fallo → implementación → verde → commit. Debe devolver: archivos tocados, comandos de verificación ejecutados con su salida, y cualquier comportamiento del original que no haya podido portar.

**Brief del revisor de paridad:** recibe el diff de la tarea y las rutas del código original, sin el informe del implementador. Debe comprobar (a) que cada fórmula, tabla y umbral del original está en el Rust con los mismos valores, (b) que los tests del plan existen y afirman lo que el plan dice, (c) las restricciones globales. Devuelve hallazgos con archivo y línea; el orquestador los pasa al implementador o a uno nuevo.

**Paralelismo:** la Tarea 0 crea los stubs de todos los módulos precisamente para que las tareas siguientes no toquen los mismos archivos.

- Tarea 0: sola, primero.
- Tareas 1, 2 y 3: en paralelo, cada una en su worktree.
- Tareas 4 y 5: en paralelo, tras integrar la 3 (usan sus helpers de proceso y de admin).
- Tarea 6: tras integrar 1–5.
- Tarea 7: al final.

Dentro de la Tarea 1 las calculadoras son independientes entre sí; si se quiere más velocidad, repartirlas entre dos implementadores `sonnet` (1A y 1B, indicado en la tarea). No usar la herramienta `Workflow` salvo que el usuario lo pida expresamente.

**Límite de verificación en Linux:** el código `cfg(windows)` solo se puede comprobar con `cargo check --target x86_64-pc-windows-gnu` y con el job de CI en `windows-latest`. Lo que no cubre ninguno de los dos (hardware real: monitores, adaptadores, audio) se lista al final en "Validación manual en Windows" y lo ejecuta el usuario. Ningún informe debe afirmar que esas partes "funcionan"; solo que compilan y pasan CI.

---

## Estructura de archivos

```
Cargo.toml                      workspace
Cargo.lock
crates/
  avlt-core/src/
    lib.rs
    error.rs                    Error, Result
    config.rs                   Config (avlt.toml)
    calc/{mod,subnet,screen,ledwall,led_cabinets,bitrate,avoip,projector,
          pixelpitch,brightness,viewangle,camera,audiobw,speaker,cable,dsp}.rs
    edid/{mod,base,cta,timing,check,diff}.rs
    net/{mod,ipv4,ports,oui,model}.rs
    media/{mod,presets,ffargs,paths}.rs
    sys/model.rs                structs de sysinfo, discos, monitores, etc.
  avlt-core/tests/fixtures/edid/*.bin
  avlt-platform/src/
    lib.rs
    admin.rs  process.rs  registry.rs  wmi.rs
    edid_source.rs
    net/{mod,adapters,icmp,neighbors,tcp,traffic,setip}.rs
    sys/{mod,info,telemetry,processes,usb,disks,monitors,battery,startup,
         audio,services,cleanup}.rs
    ndi.rs
  avlt/src/
    main.rs                     arranque, detección doble clic
    cli/{mod,output,calc,edid,net,sys,media,ndi,util}.rs
    gui/{mod,theme,calc,edid,net,sys,media,ndi}.rs
.github/workflows/ci.yml
.github/workflows/release.yml
```

---

## Tarea 0: Esqueleto, contrato de CLI y CI

**Archivos:** crear todo el árbol anterior con módulos vacíos; `.github/workflows/ci.yml`; añadir `target/` a `.gitignore`.

**Produce (lo usan todas las tareas):**

```rust
// avlt-core/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid input: {0}")]            InvalidInput(String),   // exit 2
    #[error("'{tool}' not found. Install with: {hint}")]
                                              MissingTool { tool: String, hint: String }, // exit 3
    #[error("administrator rights required")] NeedsAdmin,             // exit 4
    #[error("not supported on this platform")] Unsupported,           // exit 5
    #[error("{0}")]                           Other(String),          // exit 1
    #[error(transparent)]                     Io(#[from] std::io::Error), // exit 1
}
pub type Result<T> = std::result::Result<T, Error>;
impl Error { pub fn exit_code(&self) -> i32; }

// avlt-core/src/config.rs
#[derive(Deserialize, Default)]
pub struct Config {
    pub iface: Option<String>,
    pub weather_city: Option<String>,          // por defecto "Madrid"
    pub output_dir: Option<PathBuf>,
    pub ip_profiles: BTreeMap<String, IpProfile>,
}
#[derive(Deserialize, Clone)]
pub struct IpProfile { pub address: String /* "192.168.0.99/24" */, pub gateway: Option<String> }
impl Config {
    pub fn load(exe_dir: &Path) -> Result<Config>;   // exe_dir/avlt.toml; si no existe, Default
    pub fn output_dir(&self, exe_dir: &Path, portable: bool) -> PathBuf;
}

// avlt/src/cli/output.rs
pub struct Out { pub json: bool, pub color: bool }
impl Out {
    pub fn emit<T: Serialize>(&self, value: &T, text: impl FnOnce(&T, &Out)); // json o texto
    pub fn title(&self, s: &str);  pub fn field(&self, label: &str, value: &str);
    pub fn ok(&self, s: &str);     pub fn warn(&self, s: &str);   pub fn err(&self, s: &str);
}
```

**Decisiones:**

- Modo portable: si existe `avlt.toml` junto al exe **o** una carpeta `_OUTPUT` junto al exe, `portable = true` y la salida va a `<exe_dir>/_OUTPUT`. Si no, a `Documents\AVLT` (y `Music`/`Videos` para media, como hoy).
- Perfiles de IP por defecto cuando no hay config: los de la tabla `$profiles` de `SetIP.ps1` (copiar valores exactos, incluidas máscaras y gateway).
- Arranque: el exe es de subsistema consola. Sin argumentos y si el proceso es el único dueño de su consola (`GetConsoleProcessList` devuelve 1, es decir, doble clic), llama a `FreeConsole` y abre la GUI. Sin argumentos desde una terminal, imprime la ayuda. `avlt gui` abre la GUI siempre.
- El enum de `clap` declara ya **todos** los subcomandos; cada uno llama a una función de su módulo `cli/*.rs` que de momento devuelve `Error::Other("not implemented")`.

Árbol de comandos (nombres definitivos):

```
avlt calc   subnet|screen|ledwall|bitrate|avoip|projector|pixelpitch|brightness|
            viewangle|camera|audiobw|speaker|cable|dsp
avlt edid   list|info|check|diff|dump
avlt net    info|myip|scan|portcheck|portscan|whoison|pingmulti|latency|speed|
            connections|speedtest|flushdns|reset-nic|setip
avlt sys    info|tele|top|killp|usb|disks|monitors|monitors-arrange|battery|
            startup|audio|services-audio|cleanup
avlt media  convert|normalize|trim|extract|resample|channels|probe
avlt ndi    launch|check|services|config
avlt util   backup|search|weather|timer
avlt gui
```

- [ ] **Paso 1:** crear workspace y los tres crates con sus `Cargo.toml` y módulos vacíos.
- [ ] **Paso 2:** tests de `error.rs` (`exit_code` devuelve 2, 3, 4, 5, 1, 1 en el orden del enum) y de `config.rs` (`load` sobre directorio sin `avlt.toml` devuelve `Default`; con un `avlt.toml` que define `weather_city = "Ibiza"` y un perfil `"0"`, los lee). Verlos fallar, implementar, verlos pasar.
- [ ] **Paso 3:** `Out` y el enum de `clap` completo con stubs. Test de integración en `avlt/tests/cli.rs`: `avlt --help` sale con 0 y lista `calc`, `edid`, `net`, `sys`, `media`, `ndi`, `util`, `gui`; `avlt calc subnet 1.2.3.4/24` sale con 1 y "not implemented".
- [ ] **Paso 4:** `ci.yml` con dos jobs: `linux` (`cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`) y `windows` (`cargo test --workspace`, `cargo build --release -p avlt`, subir `avlt.exe` como artifact).
- [ ] **Paso 5:** verificar en local: `cargo test --workspace` (verde), `rustup target add x86_64-pc-windows-gnu && cargo check --workspace --target x86_64-pc-windows-gnu` (sin errores). Commit.

---

## Tarea 1: Calculadoras (`avlt calc`)

**Porta:** `AVCalc.ps1` completo (1.064 líneas) y `AVToolbox/CalcTools.cs` como segunda referencia.

**Archivos:** `avlt-core/src/calc/*.rs`, `avlt/src/cli/calc.rs`.

**Reparto opcional:** 1A = `subnet`, `screen`, `ledwall`, `led_cabinets`; 1B = las diez restantes.

**Produce:** por cada calculadora, un struct de entrada, uno de salida y una función pura:

```rust
pub fn subnet(cidr: &str, devices: u32, start_host: u32) -> Result<SubnetResult>;
pub fn screen(input: &ScreenInput) -> Result<ScreenResult>;
pub fn ledwall(input: &LedWallInput) -> Result<LedWallResult>;
pub fn led_cabinets() -> &'static [LedCabinet];
// y bitrate, avoip, projector, pixelpitch, brightness, viewangle, camera,
// audiobw, speaker, cable, dsp con la misma forma: fn x(input: &XInput) -> Result<XResult>
```

Los campos de cada `XInput` son los parámetros del `param()` de la función PowerShell correspondiente, con los mismos valores por defecto (por ejemplo `LedWallInput`: `avg_factor = 0.33`, `voltage = 230.0`, `breaker_a = 16.0`, `max_chain = 10`, `px_per_port = 650000.0`, `power_basis = Avg`, `distribution = Vertical`). Las tablas de datos (cabinets LED, procesadores NovaStar, secciones de cable, presets de códec) se copian valor a valor a `const`/`static`.

Los asistentes interactivos (`Invoke-LEDWizard`, `Invoke-CalcMenu`) no se portan a la CLI: la GUI cubre ese uso. La CLI sí porta `--list` de cabinets y el diagrama de reparto (`Get-LEDGroupMap` en core como `Vec<Vec<u16>>`; el dibujo, en `cli/calc.rs`).

**Tests obligatorios** (además de los que el implementador derive):

```rust
// subnet
let r = subnet("192.168.10.37/24", 0, 1)?;
assert_eq!((r.network.as_str(), r.netmask.as_str(), r.wildcard.as_str(), r.broadcast.as_str()),
           ("192.168.10.0", "255.255.255.0", "0.0.0.255", "192.168.10.255"));
assert_eq!((r.first_usable.as_str(), r.last_usable.as_str(), r.hosts), ("192.168.10.1", "192.168.10.254", 254));
let r = subnet("10.0.0.0/30", 0, 1)?;  assert_eq!(r.hosts, 2);
let r = subnet("10.0.0.0/31", 0, 1)?;  assert_eq!((r.first_usable.as_str(), r.last_usable.as_str(), r.hosts), ("10.0.0.0", "10.0.0.1", 2));
let r = subnet("10.0.0.5/32", 0, 1)?;  assert_eq!(r.hosts, 1);
let r = subnet("0.0.0.0/0", 0, 1)?;    assert_eq!(r.netmask, "0.0.0.0");
let r = subnet("192.168.1.0/24", 3, 10)?;
assert_eq!(r.assignments, vec!["192.168.1.10", "192.168.1.11", "192.168.1.12"]);
let r = subnet("192.168.1.0/30", 5, 1)?; assert_eq!(r.assignments.len(), 2); assert!(r.assignments_truncated);
for bad in ["192.168.1.0", "192.168.1.0/33", "300.1.1.1/24", "a.b.c.d/24", ""] {
    assert!(matches!(subnet(bad, 0, 1), Err(Error::InvalidInput(_))));
}
// screen
assert!(matches!(screen(&ScreenInput { aspect: "16:0".into(), ..d }), Err(Error::InvalidInput(_))));
```

Para el resto de calculadoras: mínimo tres tests por función con valores esperados calculados a mano desde la fórmula del `.ps1`, con la fórmula y la línea de origen citadas en un comentario del test, más un test de entrada inválida (cero, negativo) que espere `InvalidInput`. Tolerancia `1e-6` en flotantes; el redondeo de presentación se hace en `cli/calc.rs`, no en core.

- [ ] **Paso 1:** tests de `subnet` (los de arriba). Fallan. Implementar. Pasan.
- [ ] **Paso 2:** repetir el ciclo por cada calculadora restante, una por commit.
- [ ] **Paso 3:** `cli/calc.rs`: subcomandos con los mismos nombres de opción que los parámetros PowerShell en kebab-case (`-CabPxW` → `--cab-px-w`). Test de integración: `avlt calc subnet 192.168.10.37/24 --json` devuelve JSON con `"hosts":254`; `avlt calc subnet nope` sale con código 2.
- [ ] **Paso 4:** `cargo test --workspace` y `cargo clippy --workspace -- -D warnings` en verde. Commit.

---

## Tarea 2: EDID (`avlt edid`)

**Porta:** `EDIDTools.ps1` (1.340 líneas).

**Archivos:** `avlt-core/src/edid/*.rs`, `avlt-platform/src/edid_source.rs`, `avlt/src/cli/edid.rs`, `avlt-core/tests/fixtures/edid/`.

**Produce:**

```rust
// avlt-core::edid
pub fn decode(bytes: &[u8]) -> Edid;               // nunca falla: lo no decodificable va a Edid.problems
pub fn check(edid: &Edid) -> Vec<Finding>;         // checksums, timings vs límites declarados
pub fn diff(a: &Edid, b: &Edid) -> Vec<DiffLine>;
pub struct Edid { pub raw: Vec<u8>, pub base: Option<BaseBlock>, pub cta: Vec<CtaBlock>, pub problems: Vec<String> }
pub struct Finding { pub severity: Severity, pub message: String }   // Severity: Ok | Warn | Error
pub struct DiffLine { pub field: String, pub a: String, pub b: String }

// avlt-platform::edid_source
pub struct DisplayDevice { pub index: usize, pub name: String, pub instance_id: String, pub active: bool, pub edid: Vec<u8> }
pub fn displays() -> Result<Vec<DisplayDevice>>;   // registro primero, WMI como alternativa (igual que el original)
```

`BaseBlock` y `CtaBlock` contienen los mismos campos que devuelven `ConvertFrom-EDIDBaseBlock` (líneas 177–335) y `ConvertFrom-EDIDCtaBlock` (líneas 336–535): fabricante, producto, serie, fecha, versión, tamaño, gamma, timing nativo, descriptores, límites de rango, VICs, SADs de audio, bloques HDMI y HDMI Forum, metadatos HDR, colorimetría.

El argumento de origen de `info`, `check`, `dump` y `diff` es un número de display (1-based) o una ruta a `.bin`, como hoy (`Resolve-EDIDSource`). `dump` escribe en `<output_dir>/EDID/` y no sobrescribe.

**Fixtures:** construir en el código de test un bloque base sintético válido de 128 bytes (función `fn synthetic_base() -> [u8; 128]` que fija cabecera, fabricante, un detailed timing 1920×1080@60 y recalcula el checksum). Si el usuario aporta volcados reales, van en `tests/fixtures/edid/` y se añaden tests de snapshot sobre ellos.

**Tests obligatorios:**

```rust
let e = decode(&synthetic_base());
assert!(e.problems.is_empty());
let b = e.base.unwrap();
assert_eq!(b.manufacturer, "DEL");                 // bytes 8-9 = 0x10 0xAC
assert_eq!((b.native.h_active, b.native.v_active), (1920, 1080));
assert!(check(&e).iter().all(|f| f.severity != Severity::Error));

// bytes 8-9 = 0x4C 0x2D
assert_eq!(decode(&with_mfg(0x4C, 0x2D)).base.unwrap().manufacturer, "SAM");

// corrupto / truncado: nunca pánico
assert!(!decode(&[]).problems.is_empty());
assert!(!decode(&synthetic_base()[..100]).problems.is_empty());
let mut bad = synthetic_base(); bad[0] = 0x12;      // cabecera rota
assert!(!decode(&bad).problems.is_empty());
let mut sum = synthetic_base(); sum[127] = sum[127].wrapping_add(1);
assert!(check(&decode(&sum)).iter().any(|f| f.severity == Severity::Error && f.message.contains("checksum")));
let mut ext = synthetic_base(); ext[126] = 2; fix_checksum(&mut ext);   // declara 2 extensiones, no hay ninguna
assert!(!decode(&ext).problems.is_empty());

assert!(diff(&e, &e).is_empty());
```

Añadir un test con bloque CTA sintético que incluya al menos un VIC (16), un SAD LPCM de 2 canales y un bloque vendor HDMI, y que afirme que los tres aparecen decodificados.

- [ ] **Paso 1:** `synthetic_base` y tests de bloque base. Ciclo fallo → implementación → verde. Commit.
- [ ] **Paso 2:** bloque CTA con sus tests. Commit.
- [ ] **Paso 3:** `check` y `diff` con sus tests. Commit.
- [ ] **Paso 4:** `edid_source.rs` (Windows: registro `HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY\...\Device Parameters\EDID`, alternativa WMI `root\wmi` `WmiMonitorDescriptorMethods`; otras plataformas: `Unsupported`).
- [ ] **Paso 5:** `cli/edid.rs`. Tests de integración sobre archivo: `avlt edid info <fixture.bin> --json` contiene `"manufacturer":"DEL"`; `avlt edid check <fixture_corrupto.bin>` sale con código distinto de 0; `avlt edid dump` con ruta de salida que contiene espacios y un carácter acentuado; segunda ejecución crea `_1` en vez de sobrescribir.
- [ ] **Paso 6:** tests, clippy y `cargo check --target x86_64-pc-windows-gnu` en verde. Commit.

---

## Tarea 3: Red (`avlt net`)

**Porta:** `NetTools.ps1`, `MyIP.ps1`, `SetIP.ps1`, `NetMaint/NetHelpers.cs`, `NetMaint/Pages/{Ping,Ports,Scan,Traffic,IpConfig}Page.cs`.

**Archivos:** `avlt-core/src/net/*.rs`, `avlt-platform/src/{admin,process}.rs`, `avlt-platform/src/net/*.rs`, `avlt/src/cli/net.rs`.

**Produce:**

```rust
// avlt-core::net
pub fn parse_cidr(s: &str) -> Result<(Ipv4Addr, u8)>;
pub fn prefix_to_mask(prefix: u8) -> Ipv4Addr;
pub fn hosts_in(net: Ipv4Addr, prefix: u8) -> impl Iterator<Item = Ipv4Addr>;
pub const COMMON_PORTS: &[(u16, &str)];            // los 14 de portscan en NetTools.ps1
pub fn oui_vendor(mac: &str) -> Option<&'static str>;

// avlt-platform (lo reutilizan las Tareas 4 y 5)
pub fn admin::is_admin() -> bool;
pub fn admin::require_admin() -> Result<()>;
pub fn process::find_tool(name: &str, winget_id: &str) -> Result<PathBuf>;   // MissingTool si no está
pub fn process::run(exe: &Path, args: &[&str]) -> Result<ExitStatus>;        // hereda stdio
pub fn process::capture(exe: &Path, args: &[&str]) -> Result<String>;

// avlt-platform::net
pub fn adapters() -> Result<Vec<Adapter>>;
pub fn default_adapter(preferred: Option<&str>) -> Result<Adapter>;   // el de la ruta por defecto, como Resolve-NetIface
pub async fn ping(addr: Ipv4Addr, timeout: Duration) -> Option<Duration>;   // IcmpSendEcho, no requiere admin
pub fn neighbors() -> Result<HashMap<Ipv4Addr, String>>;              // una sola lectura de la tabla
pub async fn tcp_open(addr: Ipv4Addr, port: u16, timeout: Duration) -> bool;
pub fn traffic(adapter: &Adapter) -> Result<TrafficSample>;
pub fn connections() -> Result<Vec<Connection>>;
pub fn set_static(adapter: &str, cidr: &str, gateway: Option<&str>) -> Result<()>;  // netsh, como SetIP.ps1
pub fn set_dhcp(adapter: &str) -> Result<()>;
pub fn restart_adapter(adapter: &str) -> Result<()>;
pub fn flush_dns() -> Result<()>;
```

**Decisiones:**

- `scan`: 254 pings concurrentes (límite 64 simultáneos), timeout 1000 ms, después una lectura de `neighbors()` y resolución de fabricante con `oui_vendor`. Acepta `scan`, `scan 192.168.1` y `scan 192.168.1.0/24`.
- `portscan`: `COMMON_PORTS` en paralelo, timeout 1000 ms. `portcheck <ip> <port>`: una conexión, mismo timeout.
- Tabla OUI embebida: fabricantes habituales en AV e IT (lista en un `const` ordenado; mínimo los que aparecen en el propio repo y los de redes de eventos: Blackmagic, NewTek/Vizrt, Barco, Christie, Panasonic, Sony, Novastar, Analog Way, Roland, Yamaha, Audinate, Netgear, Cisco, Ubiquiti, TP-Link, Intel, Realtek, Apple, Dell, HP, Lenovo). Los prefijos se toman del registro público del IEEE; si la sesión no tiene acceso a esa fuente, dejar la tabla con los que se puedan verificar y anotarlo en el informe, sin inventar prefijos.
- Bucles en vivo (`pingmulti`, `latency`, `speed`): refresco cada 1 s, terminan con Ctrl+C; con `--json` emiten una línea JSON por muestra.
- `setip` acepta `0`, `1` (perfiles de config), `dhcp`, `<ip>/<prefijo>` y `<ip>/<prefijo> <gateway>`; requiere admin y confirma salvo `--yes`.
- `speedtest` envuelve la CLI de Ookla con `find_tool("speedtest", "Ookla.Speedtest.CLI")`.

**Tests obligatorios** (en `avlt-core`):

```rust
assert_eq!(parse_cidr("192.168.1.25/24")?, ("192.168.1.25".parse()?, 24));
assert!(parse_cidr("192.168.1.25").is_err());
assert_eq!(prefix_to_mask(24).to_string(), "255.255.255.0");
assert_eq!(prefix_to_mask(0).to_string(), "0.0.0.0");
assert_eq!(prefix_to_mask(32).to_string(), "255.255.255.255");
assert_eq!(hosts_in("192.168.1.0".parse()?, 24).count(), 254);
assert_eq!(hosts_in("10.0.0.0".parse()?, 30).map(|a| a.to_string()).collect::<Vec<_>>(), ["10.0.0.1", "10.0.0.2"]);
assert_eq!(COMMON_PORTS.len(), 14);
assert!(COMMON_PORTS.contains(&(3389, "RDP")));
assert_eq!(oui_vendor("zz"), None);
assert_eq!(oui_vendor("AA-BB-CC-00-11-22"), oui_vendor("aa:bb:cc:00:11:22"));   // formato indiferente
```

Test de `tcp_open` (corre en Linux): abrir un `TcpListener` en `127.0.0.1:0`, comprobar `true` en ese puerto y `false` tras cerrarlo. Test de `find_tool("definitely-not-a-tool", "X.Y")` devuelve `MissingTool` cuyo mensaje contiene `winget install X.Y`.

Sin red: en Windows, `default_adapter` sin ruta por defecto devuelve `Error::Other("no active network adapter")` y `net info` lo muestra como aviso con código 1; `myip` muestra las IPs locales y marca la pública como no disponible sin fallar.

- [ ] **Paso 1:** `avlt-core::net` con sus tests. Commit.
- [ ] **Paso 2:** `admin.rs`, `process.rs` con sus tests. Commit.
- [ ] **Paso 3:** `avlt-platform::net` (Windows + stubs). `cargo check --target x86_64-pc-windows-gnu`. Commit.
- [ ] **Paso 4:** `cli/net.rs`, un subcomando por commit lógico. Test de integración: `avlt net portcheck 127.0.0.1 <puerto de un listener de test> --json` devuelve `"open":true`.
- [ ] **Paso 5:** añadir al job `windows` de CI un smoke test: `avlt net info --json` y `avlt net portscan 127.0.0.1 --json` salen con 0. Commit.

---

## Tarea 4: Sistema y utilidades (`avlt sys`, `avlt util`)

**Porta:** `SysInfo.ps1`, `Tele.ps1`, `Top.ps1`, `KillP.ps1`, `USB.ps1`, `Disks.ps1`, `Monitors.ps1`, `MonitorsArrange.ps1`, `Battery.ps1`, `Startup.ps1`, `Audio.ps1`, `ServicesAudio.ps1`, `Cleanup.ps1`, `Backup.ps1`, `Search.ps1`, `Weather.ps1`, `Timer.ps1`, `AVToolbox/SystemTools.cs`, `NetMaint/Pages/MaintenancePage.cs`.

**Archivos:** `avlt-core/src/sys/model.rs`, `avlt-platform/src/{registry,wmi}.rs`, `avlt-platform/src/sys/*.rs`, `avlt/src/cli/{sys,util}.rs`.

**Consume:** `admin::*`, `process::*` de la Tarea 3.

**Produce:** una función por subcomando en `avlt-platform::sys` con la forma `pub fn info() -> Result<SystemInfo>`, `pub fn disks() -> Result<Vec<Disk>>`, etc., y los structs en `avlt-core::sys::model`. Los campos de cada struct son los que el script original muestra.

**Decisiones:**

- Fuente de datos: `sysinfo` para CPU, RAM, procesos y volúmenes; `wmi` para lo que `sysinfo` no da, usando las mismas clases que los scripts (`Win32_BIOS`, `Win32_ComputerSystem`, `Win32_VideoController`, `Win32_PhysicalMemory`, `Win32_Battery`, `root\wmi` `BatteryStaticData`/`BatteryFullChargedCapacity`, `WmiMonitorID`, `WmiMonitorListedSupportedSourceModes`); registro para `startup` (las cinco claves `Run`/`RunOnce` de `Startup.ps1`).
- `monitors-arrange`: cambiar modo con `ChangeDisplaySettingsExW` directamente. Elimina la dependencia de `nircmd.exe`. Primero `CDS_TEST`, y solo si devuelve `DISP_CHANGE_SUCCESSFUL` se aplica.
- `killp`: coincidencia exacta por defecto, `--wildcard` para subcadena, misma lista de procesos críticos que `KillP.ps1`, confirmación salvo `--yes`.
- `cleanup`: objetivos de `Cleanup.ps1` menos Prefetch. Un solo recorrido por objetivo que suma los bytes de lo que se borra con éxito. Confirmación salvo `--yes`.
- `top`: muestreo cada 2 s, orden por CPU instantánea, `--count` por defecto 10.
- `audio`: dispositivos y volumen con la API Core Audio (`IMMDeviceEnumerator`, `IAudioEndpointVolume`).
- `timer` acepta `5m`, `1h30m`, `90s`. `weather` usa `wttr.in` con ciudad de config y formatos `full|short|mini`. `backup <src> <dest>` copia a `<dest>/<nombre>_<yyyy-MM-dd_HHmm>`.

**Tests obligatorios** (lógica pura, en `avlt-core`):

```rust
assert_eq!(parse_duration("5m")?, Duration::from_secs(300));
assert_eq!(parse_duration("1h30m")?, Duration::from_secs(5400));
assert_eq!(parse_duration("90s")?, Duration::from_secs(90));
assert!(parse_duration("").is_err());  assert!(parse_duration("5x").is_err());
assert!(is_critical_process("svchost"));  assert!(!is_critical_process("resolume"));
assert!(matches_process("chrome", "chrome", false));
assert!(!matches_process("svc", "svchost", false));    // exacto por defecto
assert!(matches_process("svc", "svchost", true));
assert_eq!(cpu_percent(/*delta_proc*/ 1.0, /*delta_wall*/ 2.0, /*cores*/ 4), 12.5);
```

Test en directorio temporal para `cleanup` (borra lo borrable, devuelve bytes liberados exactos, no falla con un archivo bloqueado/inexistente) y para `backup` (crea carpeta con sufijo de fecha, copia árbol con nombres con espacios y acentos).

- [ ] **Paso 1:** modelos y lógica pura con tests. Commit.
- [ ] **Paso 2:** `registry.rs`, `wmi.rs` y `sys/*.rs`, un archivo por commit. `cargo check --target x86_64-pc-windows-gnu` tras cada uno.
- [ ] **Paso 3:** `cli/sys.rs` y `cli/util.rs`.
- [ ] **Paso 4:** smoke en CI Windows: `avlt sys info --json`, `avlt sys disks --json`, `avlt sys startup --json`, `avlt util timer 1s` salen con 0. Commit.

---

## Tarea 5: Media y NDI (`avlt media`, `avlt ndi`)

**Porta:** `AudioTools.ps1`, `NDITools.ps1`, `AVToolbox/{AudioTools,NdiTools}.cs`.

**Archivos:** `avlt-core/src/media/*.rs`, `avlt-platform/src/ndi.rs`, `avlt/src/cli/{media,ndi}.rs`.

**Consume:** `process::find_tool`, `process::run`, `process::capture` de la Tarea 3.

**Produce:**

```rust
// avlt-core::media
pub enum Preset { Mp3, Mp3Low, Aac, Opus, Flac, Wav16, Wav24 }
pub fn convert_args(input: &Path, output: &Path, preset: Preset) -> Vec<String>;
pub fn normalize_pass1_args(input: &Path) -> Vec<String>;
pub fn normalize_pass2_args(input: &Path, output: &Path, measured: &LoudnormMeasure) -> Vec<String>;
pub fn parse_loudnorm_json(stderr: &str) -> Result<LoudnormMeasure>;
pub fn trim_args(..) / extract_args(..) / resample_args(..) / channels_args(..) -> Vec<String>;
pub fn non_colliding(path: &Path) -> PathBuf;      // a.mp3 -> a_1.mp3 -> a_2.mp3
```

Los argumentos de FFmpeg de cada preset y de `normalize` (dos pasadas EBU R128, objetivo −23 LUFS) se copian exactamente de `AudioTools.ps1`. Los argumentos se pasan como lista, nunca como una cadena concatenada.

NDI: `launch <app>` con los mismos alias de `Invoke-NDILaunch`, detección de la instalación `NDI N Tools` más reciente (como `Resolve-NDIBase`), `check` (puertos y mDNS que comprueba `Test-NDINetwork`), `services`, `config` (lectura del JSON de Access Manager).

El menú interactivo (`audiotools`, `ndi`) no se porta: lo cubre la GUI.

**Tests obligatorios:**

```rust
let a = convert_args(Path::new("in file.wav"), Path::new("out é.mp3"), Preset::Mp3);
assert!(a.contains(&"in file.wav".to_string()) && a.contains(&"out é.mp3".to_string()));  // sin comillas ni escapes
// non_colliding en tempdir: con a.mp3 existente -> a_1.mp3; con a.mp3 y a_1.mp3 -> a_2.mp3
// parse_loudnorm_json: sobre un stderr de ejemplo que incluye el bloque JSON de loudnorm, extrae
// input_i, input_tp, input_lra, input_thresh, target_offset; sobre texto sin JSON -> Err
// normalize_pass2_args contiene "I=-23"
```

Test de integración: con `PATH` vacío, `avlt media convert x.wav --preset mp3` sale con código 3 y el mensaje contiene `winget install Gyan.FFmpeg`.

- [ ] **Paso 1:** `avlt-core::media` con tests. Commit.
- [ ] **Paso 2:** `cli/media.rs`. Commit.
- [ ] **Paso 3:** `ndi.rs` y `cli/ndi.rs`; `cargo check --target x86_64-pc-windows-gnu`. Commit.

---

## Tarea 6: GUI (`avlt gui`)

**Sustituye a:** `AVToolbox/` y `NetMaint/` (como referencia de pantallas y flujos; no se borran).

**Archivos:** `avlt/src/gui/*.rs`, `avlt/src/main.rs`.

**Consume:** todas las funciones públicas de las Tareas 1–5. La GUI no contiene lógica: llama a las mismas funciones que la CLI.

**Decisiones:**

- `eframe` con una barra lateral de secciones: Network, EDID, Calculators, System, Media, NDI. Tema oscuro (referencia: `NetMaint/Theme.cs`).
- Las operaciones largas (scan, portscan, ping continuo, ffmpeg, speedtest, cleanup) corren en un runtime `tokio` aparte y envían resultados por canal; la interfaz nunca se bloquea y toda operación larga tiene botón de cancelar.
- Cada pantalla de resultados tiene "Copy as text" y "Copy as JSON".
- Network incluye gráfica de tráfico y de latencia (referencia: `NetMaint/GraphPanel.cs`).
- Calculators: un formulario por calculadora generado a partir de su `XInput`, con recálculo al cambiar un valor.
- Las acciones que requieren admin muestran el aviso y un botón para relanzar elevado (`ShellExecuteW` con `runas`).

**Tests:** la lógica de estado de cada pantalla (struct de estado + función `update(msg)`) se separa del dibujo y se testea sin ventana: por ejemplo, el estado de Scan pasa de `Idle` a `Running` a `Done(n)` al recibir los mensajes correspondientes, y `Cancel` en `Running` vuelve a `Idle`.

- [ ] **Paso 1:** ventana, navegación, tema, canal de tareas en segundo plano con cancelación. Test de estado. Commit.
- [ ] **Paso 2:** una sección por commit, en este orden: Calculators, EDID, Network, System, Media, NDI.
- [ ] **Paso 3:** detección de doble clic en `main.rs` (ver Tarea 0). `cargo check --target x86_64-pc-windows-gnu`. Commit.
- [ ] **Paso 4:** CI Windows: `cargo build --release -p avlt` y comprobar que el `.exe` resultante existe y `avlt --version` sale con 0.

---

## Tarea 7: Release y documentación

**Archivos:** `.github/workflows/release.yml`, `README.md`, `avlt.example.toml`.

- [ ] **Paso 1:** perfil `release` en `Cargo.toml`: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`. Enlazado estático del CRT (`-C target-feature=+crt-static` en `.cargo/config.toml` para el target msvc).
- [ ] **Paso 2:** `release.yml`: al empujar un tag `v*`, compila en `windows-latest`, calcula el SHA-256 y publica `avlt.exe` y `avlt.exe.sha256` en un GitHub Release.
- [ ] **Paso 3:** `avlt.example.toml` con todos los campos de `Config` comentados.
- [ ] **Paso 4:** README: nueva sección inicial "avlt (single binary)" con descarga, uso CLI y GUI, modo portable, tabla de equivalencias comando antiguo → `avlt ...`, y el aviso de SmartScreen para exe sin firmar. La documentación de los scripts PowerShell se conserva debajo, marcada como legacy.
- [ ] **Paso 5:** `superpowers:verification-before-completion`: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `cargo check --workspace --target x86_64-pc-windows-gnu`, y el último run de CI en verde. Commit y push.

---

## Validación manual en Windows

Lo que ni los tests ni CI pueden demostrar. El informe final debe entregar esta lista al usuario sin marcar; solo el usuario la completa.

- [ ] Doble clic en `avlt.exe` abre la GUI sin dejar una ventana de consola.
- [ ] `avlt edid list` muestra los displays conectados; `avlt edid info 1` coincide con `edid-info 1` del script.
- [ ] `avlt edid diff 1 <volcado.bin>` contra un volcado a través de un switcher.
- [ ] `avlt net scan` encuentra los mismos hosts que `scan`, con MAC y fabricante.
- [ ] `avlt net setip 0`, `setip dhcp` y `reset-nic` como administrador; y su mensaje de error sin serlo.
- [ ] `avlt sys monitors-arrange` cambia resolución y refresco sin `nircmd`.
- [ ] `avlt sys audio`, `disks` (SMART y temperatura), `battery` en un portátil, `usb`.
- [ ] `avlt media normalize` sobre un archivo real; comparar LUFS con el resultado del script.
- [ ] `avlt ndi launch monitor` y `ndi check` con NDI Tools instalado.
- [ ] Modo portable desde un USB: la salida cae en `_OUTPUT` y no queda nada en el PC.

## Qué queda fuera de este plan

- Eliminar los `.ps1`, `AVToolbox/` y `NetMaint/`. Se decide después de completar la validación manual.
- Firma de código del `.exe`.
- Auto-actualización.
- Hotkeys de Resolume.
