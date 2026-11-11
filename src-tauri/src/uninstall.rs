//! Desinstalación y componentes (0.9.1, D-065).
//!
//! Muestra qué instaló la aplicación y permite quitarlo de forma selectiva con
//! APT. Reglas, de más a menos importante:
//!
//! - Docker Engine, el CLI, Compose y su motor (containerd, runc…) son
//!   herramientas externas: se muestran y **nunca** se quitan.
//! - Los datos y la configuración del usuario (y todo lo de Docker: imágenes,
//!   contenedores, redes, volúmenes, proyectos Compose) no se tocan.
//! - Una dependencia solo se ofrece para quitar si se instaló automáticamente
//!   **en la misma operación de APT** que la aplicación (historial de APT),
//!   sigue marcada como automática y APT la quitaría al desinstalar la
//!   aplicación (ningún otro paquete la necesita). Si no se puede saber con
//!   seguridad, se conserva y se explica por qué.
//! - Antes de ejecutar se simula exactamente la orden; si APT fuera a quitar
//!   algo más de lo elegido, no se ejecuta.
//! - Solo la orden de APT se ejecuta con privilegios, con `pkexec` (diálogo de
//!   autenticación del sistema), nunca la aplicación entera.
//!
//! Todo se ejecuta sin shell, con argumentos separados, y queda en el panel de
//! actividad (`packages.query`, `packages.simulate`, `packages.remove`).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dcc_activity::{ActivityLog, Category};
use dcc_engine::cli::{self, CliOutput, CliRequest};
use serde::Serialize;
use ts_rs::TS;

use crate::error::CommandError;

/// Nombre del paquete de la aplicación.
pub const OWN_PACKAGE: &str = "docker-control-center";

const QUERY_TIMEOUT: Duration = Duration::from_secs(60);
/// Desinstalar puede tardar (disparadores de dpkg, cachés de iconos…).
const REMOVE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

// ------------------------------------------------------------------ modelo

/// Clasificación de un paquete en la vista de componentes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PackageClass {
    /// El paquete de la aplicación.
    Own,
    /// Instalado automáticamente con la aplicación y nadie más lo usa: se puede quitar.
    Removable,
    /// Instalado con la aplicación, pero otros paquetes lo necesitan.
    Shared,
    /// Instalado de forma explícita (por el usuario o por otro programa).
    Manual,
    /// Ya estaba instalado antes que la aplicación.
    PreExisting,
    /// No se puede saber con seguridad quién lo instaló (sin historial de APT).
    Uncertain,
    /// Componente del sistema o de Docker: se conserva siempre.
    Protected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackageEntry {
    pub name: String,
    pub version: String,
    /// Espacio que ocupa instalado (KiB, campo `Installed-Size`).
    #[ts(type = "number")]
    pub installed_kib: u64,
    pub class: PackageClass,
    /// Dependencia directa de la aplicación (si no, la trajo otra dependencia).
    pub direct: bool,
    /// Paquetes instalados que dependen de este (además de la aplicación).
    pub used_by: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExternalKind {
    Engine,
    Cli,
    Compose,
}

/// Docker Engine, CLI o Compose: se muestran y se conservan siempre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExternalTool {
    pub kind: ExternalKind,
    /// Paquetes instalados que lo proporcionan ("nombre versión").
    pub packages: Vec<String>,
    /// Ejecutable encontrado (aunque no venga de un paquete).
    pub binary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UserPathKind {
    Config,
    State,
    WebData,
    WebCache,
}

/// Datos del usuario que **no** se borran (se indican para que sepa dónde están).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserPath {
    pub kind: UserPathKind,
    pub path: String,
    pub exists: bool,
    #[ts(type = "number")]
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HistoryState {
    /// Se encontró la operación de APT que instaló la aplicación.
    Found,
    /// Hay historial, pero no la instalación de la aplicación (p. ej. `dpkg -i`
    /// o historial rotado): no se atribuye ninguna dependencia a la aplicación.
    NotFound,
    /// No se pudo leer `/var/log/apt/history.log`.
    Unreadable,
}

/// Simulación de APT tal como se ejecutó.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Simulation {
    /// Orden ejecutada (sin privilegios).
    pub command: String,
    /// Paquetes que APT quitaría.
    pub removes: Vec<String>,
    /// Salida de APT.
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComponentsReport {
    /// El sistema usa dpkg/APT (Debian, Ubuntu…).
    pub supported: bool,
    /// El paquete de la aplicación, si está instalada como paquete.
    pub own: Option<PackageEntry>,
    /// Dependencias consideradas (directas y las instaladas con la aplicación).
    pub packages: Vec<PackageEntry>,
    pub external: Vec<ExternalTool>,
    pub user_data: Vec<UserPath>,
    pub history: HistoryState,
    /// Qué quitaría APT al desinstalar la aplicación con `--auto-remove`.
    pub simulation: Option<Simulation>,
    /// Programa para pedir privilegios (`pkexec`), si está instalado.
    pub elevation: Option<String>,
}

/// Plan comprobado de una desinstalación: lo que se ejecutaría.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemovalPlan {
    pub packages: Vec<String>,
    /// Orden que se ejecutará con privilegios.
    pub command: String,
    pub simulation: Simulation,
    /// Paquetes que APT quitaría y que no se eligieron (si hay, no se ejecuta).
    pub extra: Vec<String>,
    /// Espacio que se libera (KiB).
    #[ts(type = "number")]
    pub freed_kib: u64,
    /// La aplicación se quita a sí misma.
    pub removes_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemovalResult {
    pub removed: Vec<String>,
    /// Elegidos que siguen instalados después (no debería ocurrir).
    pub still_installed: Vec<String>,
    pub output: String,
    pub removed_self: bool,
}

// ------------------------------------------------------------------ análisis (puro)

/// Paquete según `dpkg-query`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DpkgPackage {
    pub name: String,
    pub version: String,
    pub installed_kib: u64,
    pub installed: bool,
    pub essential: bool,
    pub priority: String,
    /// Dependencias (Depends y Pre-Depends); cada elemento son alternativas.
    pub depends: Vec<Vec<String>>,
}

/// Formato de `dpkg-query -W -f`: una línea por paquete, campos separados por tabuladores.
pub const DPKG_FORMAT: &str =
    "${Package}\t${Version}\t${Installed-Size}\t${db:Status-Abbrev}\t${Essential}\t${Priority}\t${Depends}\t${Pre-Depends}\n";

/// `libgtk-3-0 (>= 3.21), a | b:any` → `[["libgtk-3-0"], ["a", "b"]]`.
pub fn parse_depends(field: &str) -> Vec<Vec<String>> {
    field
        .split(',')
        .map(|group| {
            group
                .split('|')
                .filter_map(|alt| {
                    let name = alt.trim().split([' ', '(']).next().unwrap_or("").trim();
                    let name = name.split(':').next().unwrap_or("");
                    (!name.is_empty()).then(|| name.to_owned())
                })
                .collect::<Vec<_>>()
        })
        .filter(|g| !g.is_empty())
        .collect()
}

pub fn parse_dpkg(out: &str) -> HashMap<String, DpkgPackage> {
    let mut map = HashMap::new();
    for line in out.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 8 || f[0].is_empty() {
            continue;
        }
        let mut depends = parse_depends(f[6]);
        depends.extend(parse_depends(f[7]));
        let p = DpkgPackage {
            name: f[0].to_owned(),
            version: f[1].to_owned(),
            installed_kib: f[2].trim().parse().unwrap_or(0),
            // "ii " instalado; "iU", "rc"… no cuentan como instalados.
            installed: f[3].starts_with("ii") || f[3].starts_with("hi"),
            essential: f[4].trim() == "yes",
            priority: f[5].trim().to_owned(),
            depends,
        };
        // Multiarquitectura: un mismo nombre puede aparecer varias veces; se
        // conserva el que está instalado.
        match map.get(&p.name) {
            Some(prev) if prev_is_better(prev, &p) => {}
            _ => {
                map.insert(p.name.clone(), p);
            }
        }
    }
    map
}

fn prev_is_better(prev: &DpkgPackage, new: &DpkgPackage) -> bool {
    prev.installed && !new.installed
}

/// Una operación del historial de APT.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Transaction {
    pub commandline: String,
    /// (paquete, instalado como automático)
    pub install: Vec<(String, bool)>,
    pub upgrade: Vec<String>,
    pub remove: Vec<String>,
}

/// `pkg:amd64 (1.0, automatic), otro:amd64 (2.0)` → nombres (sin arquitectura) y si son automáticos.
fn parse_history_list(value: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut cur = String::new();
    for c in value.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                cur.push(c);
            }
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter()
        .filter_map(|item| {
            let item = item.trim();
            let name = item.split([' ', '(']).next()?.split(':').next()?.trim();
            if name.is_empty() {
                return None;
            }
            Some((name.to_owned(), item.contains("automatic)")))
        })
        .collect()
}

/// Lee el historial de APT (uno o varios archivos ya concatenados en orden cronológico).
pub fn parse_history(text: &str) -> Vec<Transaction> {
    let mut out = Vec::new();
    let mut cur: Option<Transaction> = None;
    for line in text.lines() {
        if line.trim().is_empty() {
            if let Some(t) = cur.take() {
                out.push(t);
            }
            continue;
        }
        let Some((key, value)) = line.split_once(": ") else {
            continue;
        };
        let t = cur.get_or_insert_with(Transaction::default);
        match key {
            "Commandline" => t.commandline = value.to_owned(),
            "Install" | "Reinstall" => t.install.extend(parse_history_list(value)),
            "Upgrade" | "Downgrade" => t
                .upgrade
                .extend(parse_history_list(value).into_iter().map(|(n, _)| n)),
            "Remove" | "Purge" => t
                .remove
                .extend(parse_history_list(value).into_iter().map(|(n, _)| n)),
            _ => {}
        }
    }
    if let Some(t) = cur {
        out.push(t);
    }
    out
}

/// Paquetes instalados automáticamente en las operaciones de APT que
/// instalaron o actualizaron la aplicación (desde su última desinstalación).
/// `None` si el historial no muestra cómo se instaló la aplicación.
pub fn installed_with_app(transactions: &[Transaction]) -> Option<BTreeSet<String>> {
    // La última vez que se quitó la aplicación: lo anterior ya no cuenta.
    let start = transactions
        .iter()
        .rposition(|t| t.remove.iter().any(|n| n == OWN_PACKAGE))
        .map(|i| i + 1)
        .unwrap_or(0);
    let relevant = &transactions[start..];
    let installed_here = relevant
        .iter()
        .any(|t| t.install.iter().any(|(n, _)| n == OWN_PACKAGE));
    if !installed_here {
        return None;
    }
    let mut set = BTreeSet::new();
    for t in relevant {
        let has_app = t.install.iter().any(|(n, _)| n == OWN_PACKAGE)
            || t.upgrade.iter().any(|n| n == OWN_PACKAGE);
        if has_app {
            set.extend(
                t.install
                    .iter()
                    .filter(|(n, auto)| *auto && n != OWN_PACKAGE)
                    .map(|(n, _)| n.clone()),
            );
        }
    }
    Some(set)
}

/// Líneas `Remv paquete [versión]` de una simulación de `apt-get -s`.
pub fn parse_simulation(out: &str) -> Vec<String> {
    let mut v: Vec<String> = out
        .lines()
        .filter_map(|l| l.strip_prefix("Remv "))
        .filter_map(|r| r.split_whitespace().next())
        .map(|n| n.split(':').next().unwrap_or(n).to_owned())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Salida de `apt-cache rdepends --installed`: paquete → quién depende de él.
pub fn parse_rdepends(out: &str) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut current: Option<String> = None;
    for line in out.lines() {
        if line.is_empty() {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('|') {
            if line == "Reverse Depends:" {
                continue;
            }
            let name = line.split(':').next().unwrap_or(line).trim().to_owned();
            map.entry(name.clone()).or_default();
            current = Some(name);
        } else if let Some(c) = &current {
            let dep = line.trim().trim_start_matches('|').trim();
            let dep = dep.split(':').next().unwrap_or(dep).to_owned();
            if !dep.is_empty() {
                let list = map.entry(c.clone()).or_default();
                if !list.contains(&dep) {
                    list.push(dep);
                }
            }
        }
    }
    map
}

/// Docker y su motor, la gestión de paquetes y la autenticación: nunca se ofrecen
/// para quitar, aunque APT los considere prescindibles.
pub fn is_protected(p: &DpkgPackage) -> bool {
    const NAMES: &[&str] = &[
        "containerd",
        "containerd.io",
        "runc",
        "tini",
        "apt",
        "dpkg",
        "pkexec",
        "policykit-1",
        "polkitd",
        "sudo",
        "libc6",
    ];
    let n = p.name.as_str();
    (n.starts_with("docker") && n != OWN_PACKAGE)
        || n.starts_with("moby-")
        || n.starts_with("podman")
        || NAMES.contains(&n)
        || p.essential
        || matches!(p.priority.as_str(), "required" | "important")
}

/// Clasifica las dependencias. `auto`: paquetes marcados como automáticos;
/// `with_app`: instalados con la aplicación (`None`: no se sabe);
/// `autoremove`: lo que APT quitaría al desinstalar la aplicación.
pub fn classify(
    pkgs: &HashMap<String, DpkgPackage>,
    auto: &HashSet<String>,
    with_app: Option<&BTreeSet<String>>,
    autoremove: &[String],
    rdepends: &HashMap<String, Vec<String>>,
) -> Vec<PackageEntry> {
    let Some(own) = pkgs.get(OWN_PACKAGE).filter(|p| p.installed) else {
        return Vec::new();
    };
    // Dependencias directas: la primera alternativa instalada de cada grupo.
    let direct: BTreeSet<String> = own
        .depends
        .iter()
        .filter_map(|alts| {
            alts.iter()
                .find(|a| pkgs.get(*a).is_some_and(|p| p.installed))
                .cloned()
        })
        .collect();
    let removable: HashSet<&str> = autoremove.iter().map(String::as_str).collect();
    let mut names: BTreeSet<String> = direct.clone();
    if let Some(w) = with_app {
        names.extend(
            w.iter()
                .filter(|n| pkgs.get(*n).is_some_and(|p| p.installed))
                .cloned(),
        );
    }
    names.remove(OWN_PACKAGE);

    let mut out: Vec<PackageEntry> = names
        .into_iter()
        .filter_map(|name| {
            let p = pkgs.get(&name)?;
            let used_by: Vec<String> = rdepends
                .get(&name)
                .map(|v| {
                    let mut v: Vec<String> = v
                        .iter()
                        .filter(|d| *d != OWN_PACKAGE && !removable.contains(d.as_str()))
                        .filter(|d| pkgs.get(*d).is_some_and(|p| p.installed))
                        .cloned()
                        .collect();
                    v.sort();
                    v.dedup();
                    v
                })
                .unwrap_or_default();
            let class = if is_protected(p) {
                PackageClass::Protected
            } else if !auto.contains(&name) {
                PackageClass::Manual
            } else {
                match with_app {
                    None => PackageClass::Uncertain,
                    Some(w) if !w.contains(&name) => PackageClass::PreExisting,
                    Some(_) if removable.contains(name.as_str()) => PackageClass::Removable,
                    Some(_) => PackageClass::Shared,
                }
            };
            Some(PackageEntry {
                direct: direct.contains(&name),
                name,
                version: p.version.clone(),
                installed_kib: p.installed_kib,
                class,
                used_by,
            })
        })
        .collect();
    out.sort_by(|a, b| (!a.direct, &a.name).cmp(&(!b.direct, &b.name)));
    out
}

/// Nombre de paquete Debian válido (Debian Policy 5.6.1).
pub fn valid_package_name(n: &str) -> bool {
    n.len() >= 2
        && n.len() <= 128
        && n.chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '+' | '-' | '.'))
}

// ------------------------------------------------------------------ sistema

/// Dónde están los programas y archivos de APT. En producción, el sistema;
/// en las pruebas, un sistema Ubuntu real dentro de un `chroot`.
#[derive(Debug, Clone)]
pub struct Host {
    /// Programa que antecede a cada orden (`chroot DIR` en pruebas).
    pub wrapper: Vec<String>,
    /// Raíz del sistema para leer archivos.
    pub root: PathBuf,
    /// Cómo se piden privilegios para quitar paquetes.
    pub elevation: Elevation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Elevation {
    /// `pkexec` (diálogo de autenticación del sistema).
    Pkexec(String),
    /// La orden ya se ejecuta como root (solo pruebas).
    #[cfg_attr(not(test), allow(dead_code))]
    AlreadyRoot,
    /// No hay forma de pedir privilegios desde la aplicación.
    Unavailable,
}

impl Host {
    pub fn system() -> Self {
        let pkexec = Path::new("/usr/bin/pkexec");
        Host {
            wrapper: Vec::new(),
            root: PathBuf::from("/"),
            elevation: if pkexec.exists() {
                Elevation::Pkexec(pkexec.display().to_string())
            } else {
                Elevation::Unavailable
            },
        }
    }

    fn path(&self, p: &str) -> PathBuf {
        self.root.join(p.trim_start_matches('/'))
    }

    fn request(
        &self,
        kind: &str,
        target: Option<String>,
        program: &str,
        args: Vec<String>,
        timeout: Duration,
    ) -> CliRequest {
        let (program, args) = match self.wrapper.split_first() {
            None => (program.to_owned(), args),
            Some((first, rest)) => {
                let mut a: Vec<String> = rest.to_vec();
                a.push(program.to_owned());
                a.extend(args);
                (first.clone(), a)
            }
        };
        CliRequest {
            category: Category::System,
            kind: kind.into(),
            target,
            program,
            args,
            // Salida en inglés y sin barras de progreso: se interpreta.
            env: vec![("LC_ALL".into(), "C".into()), ("LANG".into(), "C".into())],
            cwd: None,
            timeout,
            // Componentes del equipo local: nunca en un servidor remoto.
            remote: None,
        }
    }

    async fn run(
        &self,
        log: &ActivityLog,
        kind: &str,
        target: Option<String>,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<CliOutput, CommandError> {
        let req = self.request(
            kind,
            target,
            program,
            args.iter().map(|s| s.to_string()).collect(),
            timeout,
        );
        cli::run(log, req).await.map_err(CommandError::from)
    }

    pub fn supported(&self) -> bool {
        self.path("/usr/bin/dpkg-query").exists() && self.path("/usr/bin/apt-get").exists()
    }

    /// Historial de APT, del más antiguo al más reciente (incluye los rotados `.N.gz`).
    pub fn read_history(&self) -> Option<String> {
        let dir = self.path("/var/log/apt");
        let mut parts: Vec<(u32, PathBuf)> = std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let n = if name == "history.log" {
                    0
                } else {
                    name.strip_prefix("history.log.")?
                        .trim_end_matches(".gz")
                        .parse::<u32>()
                        .ok()?
                };
                Some((n, e.path()))
            })
            .collect();
        if parts.is_empty() {
            return None;
        }
        parts.sort_by_key(|p| std::cmp::Reverse(p.0));
        let mut text = String::new();
        let mut any = false;
        for (_, p) in parts {
            let Ok(bytes) = std::fs::read(&p) else {
                continue;
            };
            let content = if p.extension().is_some_and(|e| e == "gz") {
                let mut s = String::new();
                if flate2::read::GzDecoder::new(&bytes[..])
                    .read_to_string(&mut s)
                    .is_err()
                {
                    continue;
                }
                s
            } else {
                String::from_utf8_lossy(&bytes).into_owned()
            };
            any = true;
            text.push_str(&content);
            text.push_str("\n\n");
        }
        any.then_some(text)
    }

    async fn dpkg(&self, log: &ActivityLog) -> Result<HashMap<String, DpkgPackage>, CommandError> {
        let out = self
            .run(
                log,
                "packages.query",
                None,
                "/usr/bin/dpkg-query",
                &["-W", "-f", DPKG_FORMAT],
                QUERY_TIMEOUT,
            )
            .await?;
        if !out.success() {
            return Err(CommandError::new(
                "packages.query",
                format!("dpkg-query falló: {}", out.stderr.trim()),
            ));
        }
        Ok(parse_dpkg(&out.stdout))
    }

    async fn simulate(&self, log: &ActivityLog, args: &[&str]) -> Result<Simulation, CommandError> {
        let mut full = vec!["-s"];
        full.extend_from_slice(args);
        let out = self
            .run(
                log,
                "packages.simulate",
                Some(OWN_PACKAGE.into()),
                "/usr/bin/apt-get",
                &full,
                QUERY_TIMEOUT,
            )
            .await?;
        if !out.success() {
            return Err(CommandError::new(
                "packages.simulate",
                format!("La simulación de APT falló: {}", out.stderr.trim()),
            ));
        }
        let mut argv = vec!["apt-get".to_owned()];
        argv.extend(full.iter().map(|s| s.to_string()));
        Ok(Simulation {
            command: dcc_activity::quote::render_argv("apt-get", &argv[1..]),
            removes: parse_simulation(&out.stdout),
            output: out.stdout,
        })
    }
}

fn dir_size(p: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(p) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    std::fs::read_dir(p)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| dir_size(&e.path())).sum())
        .unwrap_or(0)
}

/// Carpetas de datos de la aplicación del usuario actual.
pub fn user_paths(config_path: &Path) -> Vec<UserPath> {
    let mut v: Vec<(UserPathKind, PathBuf)> = Vec::new();
    if let Some(dir) = config_path.parent() {
        v.push((UserPathKind::Config, dir.to_path_buf()));
    }
    if let Some(d) = crate::journal::state_dir(config_path) {
        v.push((UserPathKind::State, d));
    }
    // Datos y caché de WebKit (identificador de la aplicación).
    const ID: &str = "org.dockercontrolcenter.app";
    if let Some(d) = dirs::data_dir() {
        v.push((UserPathKind::WebData, d.join(ID)));
    }
    if let Some(d) = dirs::cache_dir() {
        v.push((UserPathKind::WebCache, d.join(ID)));
    }
    v.into_iter()
        .map(|(kind, p)| UserPath {
            kind,
            exists: p.exists(),
            bytes: dir_size(&p),
            path: p.display().to_string(),
        })
        .collect()
}

fn external_tools(pkgs: &HashMap<String, DpkgPackage>) -> Vec<ExternalTool> {
    let groups: [(ExternalKind, &[&str], &str); 3] = [
        (
            ExternalKind::Engine,
            &[
                "docker-ce",
                "docker.io",
                "moby-engine",
                "containerd",
                "containerd.io",
                "runc",
            ],
            "dockerd",
        ),
        (
            ExternalKind::Cli,
            &["docker-ce-cli", "docker-cli", "moby-cli"],
            "docker",
        ),
        (
            ExternalKind::Compose,
            &[
                "docker-compose-plugin",
                "docker-compose",
                "docker-compose-v2",
            ],
            "docker-compose",
        ),
    ];
    groups
        .iter()
        .map(|(kind, names, bin)| ExternalTool {
            kind: *kind,
            packages: names
                .iter()
                .filter_map(|n| pkgs.get(*n).filter(|p| p.installed))
                .map(|p| format!("{} {}", p.name, p.version))
                .collect(),
            binary: dcc_engine::sys::find_executable(bin)
                .or_else(|| {
                    (*kind == ExternalKind::Compose)
                        .then(|| {
                            [
                                "/usr/libexec/docker/cli-plugins/docker-compose",
                                "/usr/lib/docker/cli-plugins/docker-compose",
                            ]
                            .iter()
                            .map(PathBuf::from)
                            .find(|p| p.exists())
                        })
                        .flatten()
                })
                .map(|p| p.display().to_string()),
        })
        .collect()
}

/// Analiza los componentes instalados. Solo consulta y simula: no cambia nada.
pub async fn inspect(
    host: &Host,
    log: &ActivityLog,
    config_path: &Path,
) -> Result<ComponentsReport, CommandError> {
    let user_data = user_paths(config_path);
    let elevation = match &host.elevation {
        Elevation::Pkexec(p) => Some(p.clone()),
        Elevation::AlreadyRoot => Some("root".into()),
        Elevation::Unavailable => None,
    };
    if !host.supported() {
        return Ok(ComponentsReport {
            supported: false,
            own: None,
            packages: Vec::new(),
            external: external_tools(&HashMap::new()),
            user_data,
            history: HistoryState::Unreadable,
            simulation: None,
            elevation,
        });
    }
    let pkgs = host.dpkg(log).await?;
    let external = external_tools(&pkgs);
    let own_pkg = pkgs.get(OWN_PACKAGE).filter(|p| p.installed);
    let Some(own_pkg) = own_pkg else {
        return Ok(ComponentsReport {
            supported: true,
            own: None,
            packages: Vec::new(),
            external,
            user_data,
            history: HistoryState::NotFound,
            simulation: None,
            elevation,
        });
    };

    let auto_out = host
        .run(
            log,
            "packages.query",
            None,
            "/usr/bin/apt-mark",
            &["showauto"],
            QUERY_TIMEOUT,
        )
        .await?;
    let auto: HashSet<String> = auto_out
        .stdout
        .lines()
        .map(|l| l.trim().split(':').next().unwrap_or("").to_owned())
        .filter(|l| !l.is_empty())
        .collect();

    let (history, with_app) = match host.read_history() {
        None => (HistoryState::Unreadable, None),
        Some(text) => match installed_with_app(&parse_history(&text)) {
            None => (HistoryState::NotFound, None),
            Some(set) => (HistoryState::Found, Some(set)),
        },
    };

    let simulation = host
        .simulate(log, &["remove", "--auto-remove", OWN_PACKAGE])
        .await?;
    let autoremove: Vec<String> = simulation
        .removes
        .iter()
        .filter(|n| *n != OWN_PACKAGE)
        .cloned()
        .collect();

    // Quién usa cada dependencia (solo para explicarlo; quién se quita lo decide APT).
    let mut candidates: BTreeSet<String> = own_pkg.depends.iter().flatten().cloned().collect();
    if let Some(w) = &with_app {
        candidates.extend(w.iter().cloned());
    }
    candidates.retain(|n| pkgs.get(n).is_some_and(|p| p.installed) && valid_package_name(n));
    let rdepends = if candidates.is_empty() {
        HashMap::new()
    } else {
        let mut args: Vec<&str> = vec![
            "rdepends",
            "--installed",
            "--no-suggests",
            "--no-conflicts",
            "--no-breaks",
            "--no-replaces",
            "--no-enhances",
        ];
        args.extend(candidates.iter().map(String::as_str));
        let out = host
            .run(
                log,
                "packages.query",
                None,
                "/usr/bin/apt-cache",
                &args,
                QUERY_TIMEOUT,
            )
            .await?;
        parse_rdepends(&out.stdout)
    };

    let packages = classify(&pkgs, &auto, with_app.as_ref(), &autoremove, &rdepends);
    Ok(ComponentsReport {
        supported: true,
        own: Some(PackageEntry {
            name: OWN_PACKAGE.into(),
            version: own_pkg.version.clone(),
            installed_kib: own_pkg.installed_kib,
            class: PackageClass::Own,
            direct: true,
            used_by: Vec::new(),
        }),
        packages,
        external,
        user_data,
        history,
        simulation: Some(simulation),
        elevation,
    })
}

fn removal_argv(host: &Host, packages: &[String]) -> Option<(String, Vec<String>)> {
    let mut apt: Vec<String> = vec![
        "/usr/bin/apt-get".into(),
        "remove".into(),
        "-y".into(),
        "--no-auto-remove".into(),
    ];
    apt.extend(packages.iter().cloned());
    match &host.elevation {
        Elevation::Pkexec(p) => Some((p.clone(), apt)),
        Elevation::AlreadyRoot => {
            let (first, rest) = apt.split_first()?;
            Some((first.clone(), rest.to_vec()))
        }
        Elevation::Unavailable => None,
    }
}

/// Comprueba una selección: solo la aplicación y dependencias «quitables» del
/// análisis actual, y simula exactamente la orden.
pub async fn plan(
    host: &Host,
    log: &ActivityLog,
    config_path: &Path,
    selection: &[String],
) -> Result<RemovalPlan, CommandError> {
    let mut packages: Vec<String> = selection.to_vec();
    packages.sort();
    packages.dedup();
    if packages.is_empty() {
        return Err(CommandError::new(
            "packages.invalid",
            "No se ha elegido nada para desinstalar.",
        ));
    }
    if let Some(bad) = packages.iter().find(|p| !valid_package_name(p)) {
        return Err(CommandError::new(
            "packages.invalid",
            format!("Nombre de paquete no válido: {bad}"),
        ));
    }
    let report = inspect(host, log, config_path).await?;
    let Some(own) = &report.own else {
        return Err(CommandError::new(
            "packages.notInstalled",
            "La aplicación no está instalada como paquete .deb: no hay nada que desinstalar con APT.",
        ));
    };
    let allowed: HashMap<&str, u64> = std::iter::once((own.name.as_str(), own.installed_kib))
        .chain(
            report
                .packages
                .iter()
                .filter(|p| p.class == PackageClass::Removable)
                .map(|p| (p.name.as_str(), p.installed_kib)),
        )
        .collect();
    if let Some(p) = packages.iter().find(|p| !allowed.contains_key(p.as_str())) {
        return Err(CommandError::new(
            "packages.notAllowed",
            format!("{p} no se puede quitar desde aquí: no se instaló con la aplicación o lo usan otros programas."),
        ));
    }
    if !packages.iter().any(|p| p == OWN_PACKAGE) {
        return Err(CommandError::new(
            "packages.notAllowed",
            "Las dependencias solo se pueden quitar junto con la aplicación (la aplicación las necesita).",
        ));
    }
    let mut args: Vec<&str> = vec!["remove", "--no-auto-remove"];
    args.extend(packages.iter().map(String::as_str));
    let simulation = host.simulate(log, &args).await?;
    let chosen: HashSet<&str> = packages.iter().map(String::as_str).collect();
    let extra: Vec<String> = simulation
        .removes
        .iter()
        .filter(|n| !chosen.contains(n.as_str()))
        .cloned()
        .collect();
    let command = match removal_argv(host, &packages) {
        Some((p, a)) => dcc_activity::quote::render_argv(&p, &a),
        None => {
            let mut a = vec![
                "apt-get".to_owned(),
                "remove".into(),
                "--no-auto-remove".into(),
            ];
            a.extend(packages.iter().cloned());
            dcc_activity::quote::render_argv("sudo", &a)
        }
    };
    Ok(RemovalPlan {
        freed_kib: packages
            .iter()
            .filter_map(|p| allowed.get(p.as_str()))
            .sum(),
        removes_self: chosen.contains(OWN_PACKAGE),
        packages,
        command,
        simulation,
        extra,
    })
}

/// Desinstala la selección tras comprobarla de nuevo. Solo esta orden se
/// ejecuta con privilegios.
pub async fn remove(
    host: &Host,
    log: &ActivityLog,
    config_path: &Path,
    selection: &[String],
) -> Result<RemovalResult, CommandError> {
    let plan = plan(host, log, config_path, selection).await?;
    if !plan.extra.is_empty() {
        return Err(CommandError::new(
            "packages.extra",
            format!(
                "APT quitaría también {}: no se ejecuta. Revisa la selección.",
                plan.extra.join(", ")
            ),
        ));
    }
    let missing: Vec<&String> = plan
        .packages
        .iter()
        .filter(|p| !plan.simulation.removes.contains(p))
        .collect();
    if !missing.is_empty() {
        return Err(CommandError::new(
            "packages.simulate",
            format!(
                "La simulación no quitaría {}: no se ejecuta.",
                missing
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    let Some((program, args)) = removal_argv(host, &plan.packages) else {
        return Err(CommandError::new(
            "packages.noElevation",
            format!("No se encuentra pkexec para pedir permisos de administrador. Ejecuta tú mismo en una terminal: {}", plan.command),
        ));
    };
    let mut req = host.request(
        "packages.remove",
        Some(plan.packages.join(" ")),
        &program,
        args,
        REMOVE_TIMEOUT,
    );
    // dpkg no debe hacer preguntas: no hay terminal.
    req.env
        .push(("DEBIAN_FRONTEND".into(), "noninteractive".into()));
    let out = cli::run(log, req).await.map_err(CommandError::from)?;
    match out.exit_code {
        Some(0) => {}
        // pkexec: 126 el usuario canceló la autenticación; 127 no autorizado.
        Some(126) if matches!(host.elevation, Elevation::Pkexec(_)) => {
            return Err(CommandError::new(
                "packages.cancelled",
                "Se canceló la autenticación: no se ha desinstalado nada.",
            ));
        }
        Some(127) if matches!(host.elevation, Elevation::Pkexec(_)) => {
            return Err(CommandError::new(
                "packages.denied",
                "No se concedieron permisos de administrador: no se ha desinstalado nada.",
            ));
        }
        code => {
            let tail: String = out
                .stderr
                .lines()
                .rev()
                .take(6)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            return Err(CommandError::new(
                "packages.remove",
                format!(
                    "APT terminó con código {}: {}",
                    code.map(|c| c.to_string()).unwrap_or_else(|| "?".into()),
                    tail.trim()
                ),
            ));
        }
    }
    // Comprobación posterior: qué sigue instalado.
    let after = host.dpkg(log).await?;
    let still: Vec<String> = plan
        .packages
        .iter()
        .filter(|p| after.get(*p).is_some_and(|x| x.installed))
        .cloned()
        .collect();
    let removed: Vec<String> = plan
        .packages
        .iter()
        .filter(|p| !still.contains(p))
        .cloned()
        .collect();
    Ok(RemovalResult {
        removed_self: removed.iter().any(|p| p == OWN_PACKAGE),
        removed,
        still_installed: still,
        output: out.stdout,
    })
}

/// Agrupa por clase (para las pruebas).
#[cfg(test)]
pub fn by_class(entries: &[PackageEntry]) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut m = std::collections::BTreeMap::<String, Vec<String>>::new();
    for e in entries {
        m.entry(format!("{:?}", e.class))
            .or_default()
            .push(e.name.clone());
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    const DPKG: &str = "docker-control-center\t0.9.1\t25000\tii \t\toptional\tlibc6 (>= 2.34), libgtk-3-0 (>= 3.21.5), libwebkit2gtk-4.1-0 (>= 2.41.90) | libfake\t\n\
libc6\t2.35\t13000\tii \tno\trequired\t\t\n\
libgtk-3-0\t3.24\t9000\tii \t\toptional\tlibatk1.0-0\t\n\
libwebkit2gtk-4.1-0\t2.44\t90000\tii \t\toptional\tlibjavascriptcoregtk-4.1-0\t\n\
libjavascriptcoregtk-4.1-0\t2.44\t30000\tii \t\toptional\t\t\n\
libatk1.0-0\t2.36\t200\tii \t\toptional\t\t\n\
zenity\t3.42\t500\tii \t\toptional\tlibgtk-3-0\t\n\
vim\t9.0\t3000\tii \t\toptional\t\t\n\
docker.io\t26.1.5\t100000\tii \t\toptional\tcontainerd, runc\t\n\
containerd\t1.7\t50000\tii \t\toptional\t\t\n\
runc\t1.1\t9000\tii \t\toptional\t\t\n\
oldlib\t1\t10\trc \t\toptional\t\t\n";

    const HISTORY: &str = "Start-Date: 2026-01-01  10:00:00\n\
Commandline: apt install vim\n\
Install: vim:amd64 (9.0), libatk1.0-0:amd64 (2.36, automatic)\n\
End-Date: 2026-01-01  10:00:05\n\
\n\
Start-Date: 2026-02-01  10:00:00\n\
Commandline: apt install ./docker-control-center_0.9.0_amd64.deb\n\
Install: docker-control-center:amd64 (0.9.0), libwebkit2gtk-4.1-0:amd64 (2.44, automatic), libjavascriptcoregtk-4.1-0:amd64 (2.44, automatic), libgtk-3-0:amd64 (3.24, automatic)\n\
End-Date: 2026-02-01  10:00:30\n\
\n\
Start-Date: 2026-03-01  10:00:00\n\
Commandline: apt install zenity\n\
Install: zenity:amd64 (3.42)\n\
End-Date: 2026-03-01  10:00:05\n";

    #[test]
    fn parses_dpkg_depends_and_status() {
        let m = parse_dpkg(DPKG);
        let own = &m["docker-control-center"];
        assert_eq!(own.installed_kib, 25000);
        assert_eq!(
            own.depends,
            vec![
                vec!["libc6".to_string()],
                vec!["libgtk-3-0".into()],
                vec!["libwebkit2gtk-4.1-0".into(), "libfake".into()]
            ]
        );
        assert!(
            !m["oldlib"].installed,
            "rc = desinstalado con configuración"
        );
        assert_eq!(m["libc6"].priority, "required");
        assert_eq!(
            parse_depends("a:any (>= 1), b | c:amd64"),
            vec![vec!["a".to_string()], vec!["b".into(), "c".into()]]
        );
    }

    #[test]
    fn history_attributes_only_the_automatic_packages_installed_with_the_app() {
        let t = parse_history(HISTORY);
        assert_eq!(t.len(), 3);
        assert_eq!(
            t[1].install[0],
            ("docker-control-center".to_string(), false)
        );
        let set = installed_with_app(&t).unwrap();
        assert_eq!(
            set.into_iter().collect::<Vec<_>>(),
            vec![
                "libgtk-3-0",
                "libjavascriptcoregtk-4.1-0",
                "libwebkit2gtk-4.1-0"
            ]
        );
        // Sin la instalación en el historial (dpkg -i, historial rotado): no se sabe.
        assert_eq!(installed_with_app(&t[..1]), None);
        // Una desinstalación posterior invalida lo anterior.
        let mut t2 = t.clone();
        t2.push(Transaction {
            commandline: "apt remove docker-control-center".into(),
            remove: vec![OWN_PACKAGE.into()],
            ..Default::default()
        });
        assert_eq!(installed_with_app(&t2), None);
        // Paquetes nuevos que trae una actualización de la aplicación también cuentan.
        t2.push(Transaction {
            install: vec![(OWN_PACKAGE.into(), false), ("libsoup-3.0-0".into(), true)],
            ..Default::default()
        });
        assert_eq!(
            installed_with_app(&t2)
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["libsoup-3.0-0"]
        );
    }

    #[test]
    fn history_lists_with_versions_containing_commas_and_parentheses() {
        let l = parse_history_list("libfoo:amd64 (1:2.0-1, automatic), bar:i386 (3.0), baz (1)");
        assert_eq!(
            l,
            vec![
                ("libfoo".to_string(), true),
                ("bar".into(), false),
                ("baz".into(), false)
            ]
        );
        let up = parse_history("Upgrade: docker-control-center:amd64 (0.9.0, 0.9.1)\n");
        assert_eq!(up[0].upgrade, vec!["docker-control-center"]);
    }

    #[test]
    fn simulation_and_rdepends_parsing() {
        let sim = "NOTE: This is only a simulation!\nRemv docker-control-center [0.9.1]\nRemv libwebkit2gtk-4.1-0 [2.44] [libfoo:amd64 ]\nRemv libjavascriptcoregtk-4.1-0:amd64 [2.44]\n";
        assert_eq!(
            parse_simulation(sim),
            vec![
                "docker-control-center",
                "libjavascriptcoregtk-4.1-0",
                "libwebkit2gtk-4.1-0"
            ]
        );
        let rd = "libgtk-3-0\nReverse Depends:\n  zenity\n |docker-control-center\n  libwebkit2gtk-4.1-0:amd64\nlibatk1.0-0\nReverse Depends:\n  libgtk-3-0\n";
        let m = parse_rdepends(rd);
        assert_eq!(
            m["libgtk-3-0"],
            vec!["zenity", "docker-control-center", "libwebkit2gtk-4.1-0"]
        );
        assert_eq!(m["libatk1.0-0"], vec!["libgtk-3-0"]);
    }

    #[test]
    fn classification_keeps_everything_that_is_not_safely_removable() {
        let pkgs = parse_dpkg(DPKG);
        let auto: HashSet<String> = [
            "libgtk-3-0",
            "libwebkit2gtk-4.1-0",
            "libjavascriptcoregtk-4.1-0",
            "libatk1.0-0",
            "libc6",
            "containerd",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let with = installed_with_app(&parse_history(HISTORY)).unwrap();
        // APT quitaría webkit y javascriptcore; libgtk-3-0 no (zenity la usa).
        let autoremove = vec![
            "libjavascriptcoregtk-4.1-0".to_string(),
            "libwebkit2gtk-4.1-0".into(),
        ];
        let rd = parse_rdepends("libgtk-3-0\nReverse Depends:\n  zenity\n  docker-control-center\n  libwebkit2gtk-4.1-0\n");
        let c = classify(&pkgs, &auto, Some(&with), &autoremove, &rd);
        let g = by_class(&c);
        assert_eq!(
            g["Removable"],
            vec!["libwebkit2gtk-4.1-0", "libjavascriptcoregtk-4.1-0"]
        );
        assert_eq!(g["Shared"], vec!["libgtk-3-0"]);
        assert_eq!(g["Protected"], vec!["libc6"]);
        let gtk = c.iter().find(|e| e.name == "libgtk-3-0").unwrap();
        assert_eq!(
            gtk.used_by,
            vec!["zenity"],
            "ni la aplicación ni lo que se va a quitar cuentan como usuarios"
        );
        assert!(gtk.direct);
        assert!(
            !c.iter()
                .find(|e| e.name == "libjavascriptcoregtk-4.1-0")
                .unwrap()
                .direct
        );

        // Sin historial no se atribuye nada: todo se conserva como incierto.
        let c = classify(&pkgs, &auto, None, &autoremove, &rd);
        let g = by_class(&c);
        assert!(!g.contains_key("Removable"));
        assert_eq!(g["Uncertain"], vec!["libgtk-3-0", "libwebkit2gtk-4.1-0"]);

        // Marcado como manual: no se quita aunque viniera con la aplicación.
        let mut manual = auto.clone();
        manual.remove("libwebkit2gtk-4.1-0");
        let c = classify(&pkgs, &manual, Some(&with), &autoremove, &rd);
        assert_eq!(
            c.iter()
                .find(|e| e.name == "libwebkit2gtk-4.1-0")
                .unwrap()
                .class,
            PackageClass::Manual
        );

        // Instalado antes que la aplicación (aunque sea automático).
        let mut with_less = with.clone();
        with_less.remove("libjavascriptcoregtk-4.1-0");
        let c = classify(&pkgs, &auto, Some(&with_less), &autoremove, &rd);
        assert!(c.iter().all(
            |e| e.name != "libjavascriptcoregtk-4.1-0" || e.class == PackageClass::PreExisting
        ));
    }

    #[test]
    fn docker_and_system_packages_are_always_protected() {
        let p = |name: &str, prio: &str, ess: bool| DpkgPackage {
            name: name.into(),
            priority: prio.into(),
            essential: ess,
            installed: true,
            ..Default::default()
        };
        for n in [
            "docker.io",
            "docker-ce",
            "docker-cli",
            "docker-compose",
            "docker-compose-plugin",
            "containerd",
            "containerd.io",
            "runc",
            "moby-engine",
            "pkexec",
            "apt",
        ] {
            assert!(is_protected(&p(n, "optional", false)), "{n}");
        }
        assert!(is_protected(&p("bash", "optional", true)));
        assert!(is_protected(&p("libsystemd0", "required", false)));
        assert!(!is_protected(&p(OWN_PACKAGE, "optional", false)));
        assert!(!is_protected(&p("libwebkit2gtk-4.1-0", "optional", false)));
    }

    #[test]
    fn package_names_are_validated() {
        assert!(valid_package_name("libwebkit2gtk-4.1-0"));
        assert!(valid_package_name("libstdc++6"));
        for bad in [
            "", "a", "-rf", "Foo", "a b", "a;b", "a/b", "$(x)", "--purge",
        ] {
            assert!(!valid_package_name(bad), "{bad}");
        }
    }

    #[test]
    fn removal_uses_pkexec_only_for_apt_and_never_purges() {
        let host = Host {
            wrapper: vec![],
            root: "/".into(),
            elevation: Elevation::Pkexec("/usr/bin/pkexec".into()),
        };
        let (p, a) =
            removal_argv(&host, &["docker-control-center".into(), "libfoo".into()]).unwrap();
        assert_eq!(p, "/usr/bin/pkexec");
        assert_eq!(
            a,
            vec![
                "/usr/bin/apt-get",
                "remove",
                "-y",
                "--no-auto-remove",
                "docker-control-center",
                "libfoo"
            ]
        );
        assert!(!a.iter().any(|x| x.contains("purge") || x == "autoremove"));
        let none = Host {
            elevation: Elevation::Unavailable,
            ..host
        };
        assert!(removal_argv(&none, &["x1".into()]).is_none());
    }

    #[test]
    fn wrapper_prefixes_the_program_without_a_shell() {
        let host = Host {
            wrapper: vec!["/usr/sbin/chroot".into(), "/tmp/r".into()],
            root: "/tmp/r".into(),
            elevation: Elevation::AlreadyRoot,
        };
        let r = host.request(
            "packages.query",
            None,
            "/usr/bin/dpkg-query",
            vec!["-W".into()],
            QUERY_TIMEOUT,
        );
        assert_eq!(r.program, "/usr/sbin/chroot");
        assert_eq!(r.args, vec!["/tmp/r", "/usr/bin/dpkg-query", "-W"]);
        assert_eq!(
            host.path("/var/log/apt"),
            PathBuf::from("/tmp/r/var/log/apt")
        );
    }

    #[test]
    fn rotated_history_is_read_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let apt = dir.path().join("var/log/apt");
        std::fs::create_dir_all(&apt).unwrap();
        std::fs::write(apt.join("history.log"), "Commandline: tercero\n").unwrap();
        std::fs::write(apt.join("history.log.1"), "Commandline: segundo\n").unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut gz, b"Commandline: primero\n").unwrap();
        std::fs::write(apt.join("history.log.2.gz"), gz.finish().unwrap()).unwrap();
        std::fs::write(apt.join("eipp.log.xz"), "x").unwrap();
        let host = Host {
            wrapper: vec![],
            root: dir.path().into(),
            elevation: Elevation::Unavailable,
        };
        let t = parse_history(&host.read_history().unwrap());
        assert_eq!(
            t.iter().map(|t| t.commandline.as_str()).collect::<Vec<_>>(),
            vec!["primero", "segundo", "tercero"]
        );
    }

    // ------------------------------------------------------------ prueba real
    //
    // Se ejecuta sobre un sistema Ubuntu 22.04 real (un `chroot` preparado por
    // `scripts/prepare-uninstall-chroot.sh`), como root, con APT y dpkg de
    // verdad: instala la aplicación con APT (que trae sus dependencias),
    // instala después un programa que comparte una de ellas, comprueba la
    // clasificación y desinstala de verdad. Sin `DCC_TEST_CHROOT` no se ejecuta
    // (el script de verificación la exige y comprueba que se ejecutó).

    fn chroot_host(root: &str, elevation: Elevation) -> Host {
        Host {
            wrapper: vec!["/usr/sbin/chroot".into(), root.into()],
            root: root.into(),
            elevation,
        }
    }

    fn sh(root: &str, args: &[&str]) -> std::process::Output {
        let o = std::process::Command::new("/usr/sbin/chroot")
            .arg(root)
            .args(args)
            .env("DEBIAN_FRONTEND", "noninteractive")
            .output()
            .unwrap();
        o
    }

    fn installed(root: &str, pkg: &str) -> bool {
        let o = sh(
            root,
            &[
                "/usr/bin/dpkg-query",
                "-W",
                "-f",
                "${db:Status-Abbrev}",
                pkg,
            ],
        );
        String::from_utf8_lossy(&o.stdout).starts_with("ii")
    }

    fn apt(root: &str, args: &[&str]) {
        let mut a = vec!["/usr/bin/apt-get", "-y", "-q"];
        a.extend_from_slice(args);
        let o = sh(root, &a);
        assert!(
            o.status.success(),
            "apt-get {args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn live_uninstall_in_a_real_ubuntu_system() {
        let Ok(root) = std::env::var("DCC_TEST_CHROOT") else {
            eprintln!("SIN EJECUTAR: falta DCC_TEST_CHROOT (sistema Ubuntu de prueba)");
            return;
        };
        let deb =
            std::env::var("DCC_TEST_DEB").expect("DCC_TEST_DEB: ruta del .deb dentro del chroot");
        let root = root.as_str();

        // Estado de partida: sin la aplicación ni zenity; Docker y Compose
        // instalados antes (los instala el script de preparación).
        // (Una ejecución anterior deja zenity y GTK: se quitan con todo lo que
        // quedó como automático y sin usar.)
        let leftovers: Vec<&str> = [OWN_PACKAGE, "zenity"]
            .into_iter()
            .filter(|p| installed(root, p))
            .collect();
        if !leftovers.is_empty() {
            let mut a = vec!["remove"];
            a.extend(leftovers);
            apt(root, &a);
        }
        apt(root, &["autoremove"]);
        assert!(
            installed(root, "docker.io") && installed(root, "docker-compose-v2"),
            "el script de preparación instala Docker y Compose"
        );
        assert!(
            installed(root, "libglib2.0-0"),
            "libglib2.0-0 ya existía antes de la aplicación"
        );
        assert!(
            !installed(root, "libwebkit2gtk-4.1-0"),
            "WebKit no debe estar instalado antes"
        );

        // 1) Instalación real con APT: trae WebKit, GTK… como automáticos.
        apt(root, &["install", "--no-install-recommends", &deb]);
        // 2) Otro programa que usa GTK, instalado después (dependencia compartida).
        apt(root, &["install", "--no-install-recommends", "zenity"]);

        // Datos del usuario: nunca se tocan.
        let home = tempfile::tempdir().unwrap();
        let cfg_dir = home.path().join("docker-control-center");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let cfg = cfg_dir.join("config.toml");
        std::fs::write(&cfg, "[ui]\ntheme = \"dark\"\n").unwrap();

        let log = ActivityLog::new(500);
        let host = chroot_host(root, Elevation::AlreadyRoot);
        let r = inspect(&host, &log, &cfg).await.unwrap();
        assert!(r.supported);
        assert_eq!(r.history, HistoryState::Found);
        let own = r.own.as_ref().expect("instalada como paquete");
        assert!(own.installed_kib > 0);
        let class = |n: &str| r.packages.iter().find(|p| p.name == n).map(|p| p.class);
        let g = by_class(&r.packages);
        eprintln!("clasificación: {g:#?}");
        assert_eq!(class("libwebkit2gtk-4.1-0"), Some(PackageClass::Removable));
        assert_eq!(
            class("libjavascriptcoregtk-4.1-0"),
            Some(PackageClass::Removable)
        );
        assert_eq!(
            class("libgtk-3-0"),
            Some(PackageClass::Shared),
            "zenity la usa"
        );
        assert!(r
            .packages
            .iter()
            .find(|p| p.name == "libgtk-3-0")
            .unwrap()
            .used_by
            .iter()
            .any(|u| u == "zenity"));
        assert_eq!(class("libglib2.0-0"), Some(PackageClass::PreExisting));
        assert_eq!(class("libc6"), Some(PackageClass::Protected));
        // Docker, el CLI y Compose: herramientas externas, nunca en la lista de lo que se quita.
        assert!(r
            .packages
            .iter()
            .filter(|p| p.class == PackageClass::Removable)
            .all(|p| !p.name.starts_with("docker") && p.name != "containerd" && p.name != "runc"));
        let ext = |k: ExternalKind| {
            r.external
                .iter()
                .find(|e| e.kind == k)
                .unwrap()
                .packages
                .join(" ")
        };
        assert!(ext(ExternalKind::Engine).contains("docker.io"));
        assert!(ext(ExternalKind::Compose).contains("docker-compose-v2"));
        // La simulación de APT se muestra tal cual.
        let sim = r.simulation.as_ref().unwrap();
        assert!(sim
            .command
            .starts_with("apt-get -s remove --auto-remove docker-control-center"));
        assert!(sim.output.contains("Remv docker-control-center"));
        assert!(!sim
            .removes
            .iter()
            .any(|p| p == "libgtk-3-0" || p.starts_with("docker.") || p == "docker-compose-v2"));
        assert!(r
            .user_data
            .iter()
            .any(|u| u.kind == UserPathKind::Config && u.exists && u.bytes > 0));

        // Selecciones que no se aceptan.
        let err = |e: Result<RemovalPlan, CommandError>| e.unwrap_err().code;
        assert_eq!(
            err(plan(&host, &log, &cfg, &["docker.io".into(), OWN_PACKAGE.into()]).await),
            "packages.notAllowed"
        );
        assert_eq!(
            err(plan(
                &host,
                &log,
                &cfg,
                &["libgtk-3-0".into(), OWN_PACKAGE.into()]
            )
            .await),
            "packages.notAllowed"
        );
        assert_eq!(
            err(plan(&host, &log, &cfg, &["libwebkit2gtk-4.1-0".into()]).await),
            "packages.notAllowed",
            "sin la aplicación"
        );
        assert_eq!(
            err(plan(&host, &log, &cfg, &["$(reboot)".into()]).await),
            "packages.invalid"
        );
        assert_eq!(err(plan(&host, &log, &cfg, &[]).await), "packages.invalid");

        // Plan válido: solo la aplicación.
        let p = plan(&host, &log, &cfg, &[OWN_PACKAGE.into()])
            .await
            .unwrap();
        assert!(p.extra.is_empty());
        assert_eq!(p.simulation.removes, vec![OWN_PACKAGE.to_string()]);
        assert_eq!(p.freed_kib, own.installed_kib);
        assert!(p.removes_self);

        // Cancelar la autenticación: no se quita nada.
        std::fs::write(
            format!("{root}/usr/local/bin/pkexec-cancela"),
            "#!/bin/sh\nexit 126\n",
        )
        .unwrap();
        std::process::Command::new("chmod")
            .args(["755", &format!("{root}/usr/local/bin/pkexec-cancela")])
            .status()
            .unwrap();
        let cancel = chroot_host(
            root,
            Elevation::Pkexec("/usr/local/bin/pkexec-cancela".into()),
        );
        let e = remove(&cancel, &log, &cfg, &[OWN_PACKAGE.into()])
            .await
            .unwrap_err();
        assert_eq!(e.code, "packages.cancelled");
        assert!(installed(root, OWN_PACKAGE));

        // Desinstalación real: la aplicación y sus dependencias quitables.
        let removable: Vec<String> = r
            .packages
            .iter()
            .filter(|p| p.class == PackageClass::Removable)
            .map(|p| p.name.clone())
            .collect();
        let mut sel = removable.clone();
        sel.push(OWN_PACKAGE.into());
        let p = plan(&host, &log, &cfg, &sel).await.unwrap();
        assert!(p.extra.is_empty(), "APT no quita nada más: {:?}", p.extra);
        let freed: u64 = r
            .packages
            .iter()
            .filter(|x| x.class == PackageClass::Removable)
            .map(|x| x.installed_kib)
            .sum::<u64>()
            + own.installed_kib;
        assert_eq!(p.freed_kib, freed);
        let res = remove(&host, &log, &cfg, &sel).await.unwrap();
        assert!(res.removed_self);
        assert!(res.still_installed.is_empty());
        assert_eq!(res.removed.len(), sel.len());
        for n in &sel {
            assert!(!installed(root, n), "{n} sigue instalado");
        }
        // Se conserva todo lo demás.
        for keep in [
            "libgtk-3-0",
            "zenity",
            "libglib2.0-0",
            "docker.io",
            "docker-compose-v2",
            "containerd",
            "runc",
            "libc6",
        ] {
            assert!(installed(root, keep), "{keep} se ha quitado");
        }
        assert_eq!(
            std::fs::read_to_string(&cfg).unwrap(),
            "[ui]\ntheme = \"dark\"\n"
        );
        // Sin purga: lo comprueba la orden registrada (más abajo); la aplicación
        // no tiene archivos de configuración de dpkg, así que dpkg la olvida.

        // Todo quedó en el panel de actividad.
        let recs = log.snapshot();
        let kinds: Vec<(&str, dcc_activity::OpStatus)> =
            recs.iter().map(|r| (r.kind.as_str(), r.status)).collect();
        assert!(kinds.contains(&("packages.query", dcc_activity::OpStatus::Succeeded)));
        assert!(kinds.contains(&("packages.simulate", dcc_activity::OpStatus::Succeeded)));
        assert!(kinds.contains(&("packages.remove", dcc_activity::OpStatus::Succeeded)));
        assert!(
            kinds.contains(&("packages.remove", dcc_activity::OpStatus::Failed)),
            "la cancelación también queda registrada"
        );
        let rm = recs
            .iter()
            .rev()
            .find(|r| r.kind == "packages.remove" && r.status == dcc_activity::OpStatus::Succeeded)
            .unwrap();
        match &rm.transport {
            dcc_activity::Transport::Cli { args, .. } => {
                assert!(args.contains(&"--no-auto-remove".to_string()));
                assert!(!args.iter().any(|a| a.contains("purge")));
            }
            t => panic!("transporte inesperado: {t:?}"),
        }
        // Una segunda consulta ya no encuentra la aplicación.
        let after = inspect(&host, &log, &cfg).await.unwrap();
        assert!(after.own.is_none());
    }
}
