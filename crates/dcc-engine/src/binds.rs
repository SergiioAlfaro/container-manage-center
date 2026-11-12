//! Fase 6: directorios vinculados (bind mounts). Se obtienen de los montajes de
//! la lista de contenedores (una petición real registrada) y se agrupan por la
//! ruta del equipo. El estado de cada ruta (existe, tipo, permisos) se consulta
//! en el sistema de archivos local: el socket de Docker es local, así que el
//! daemon ve las mismas rutas (también con Docker rootless).
//!
//! Docker no permite añadir montajes a un contenedor que ya existe: vincular
//! una carpeta es crear un contenedor (el asistente de contenedores).

use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::Engine;
use crate::containers::ContainerSummary;
use crate::error::EngineError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PathKind {
    Directory,
    File,
    Socket,
    Other,
    /// No existe (o no se puede consultar) en este equipo.
    Missing,
}

/// Por qué una vinculación merece atención.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BindRisk {
    /// El socket de Docker: quien lo tiene controla Docker y, en la práctica,
    /// el equipo (salvo con Docker rootless).
    DockerSocket,
    /// La raíz del sistema de archivos.
    HostRoot,
    /// Un directorio del sistema (/etc, /root, /boot, /proc, /sys, /dev…).
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HostPathInfo {
    pub kind: PathKind,
    /// Propietario (uid) y permisos (`0o755`) si existe.
    pub uid: Option<u32>,
    pub mode: Option<u32>,
    /// El usuario de la aplicación puede leerla.
    pub readable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BindUser {
    pub id: String,
    pub name: String,
    pub state: String,
    pub destination: String,
    pub rw: bool,
    pub propagation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BindSummary {
    /// Ruta en el equipo.
    pub source: String,
    pub host: HostPathInfo,
    pub risk: Option<BindRisk>,
    /// Algún contenedor la monta con escritura.
    pub writable: bool,
    pub containers: Vec<BindUser>,
}

/// Riesgo de una carpeta del equipo en la pantalla de directorios vinculados.
/// Mismas reglas que el asistente y la edición (`crate::risk`, D-086): sin
/// saber cómo la monta cada contenedor, se evalúa como montaje con escritura.
pub fn risk_of(path: &str) -> Option<BindRisk> {
    let r = crate::risk::mount_risk(path, false, false, "source")?;
    Some(match r.code.as_str() {
        "risk.runtimeSocket" => BindRisk::DockerSocket,
        "risk.hostRoot" => BindRisk::HostRoot,
        _ => BindRisk::System,
    })
}

pub fn host_path_info(path: &str) -> HostPathInfo {
    let p = Path::new(path);
    match std::fs::symlink_metadata(p).and_then(|m| {
        // Se sigue el enlace como hace Docker al montar.
        if m.file_type().is_symlink() {
            std::fs::metadata(p)
        } else {
            Ok(m)
        }
    }) {
        Ok(m) => {
            let ft = m.file_type();
            let kind = if ft.is_dir() {
                PathKind::Directory
            } else if ft.is_file() {
                PathKind::File
            } else if ft.is_socket() {
                PathKind::Socket
            } else {
                PathKind::Other
            };
            let readable = if ft.is_dir() {
                std::fs::read_dir(p).is_ok()
            } else if ft.is_file() {
                std::fs::File::open(p).is_ok()
            } else {
                // Sockets y dispositivos: se informa solo de permisos.
                false
            };
            HostPathInfo {
                kind,
                uid: Some(m.uid()),
                mode: Some(m.permissions().mode() & 0o7777),
                readable,
            }
        }
        Err(_) => HostPathInfo {
            kind: PathKind::Missing,
            uid: None,
            mode: None,
            readable: false,
        },
    }
}

/// Agrupa por ruta del equipo los montajes `bind` de la lista de contenedores.
pub fn binds_from(
    containers: &[ContainerSummary],
    inspect: impl Fn(&str) -> HostPathInfo,
) -> Vec<BindSummary> {
    let mut map: std::collections::BTreeMap<String, Vec<BindUser>> = Default::default();
    for c in containers {
        for m in c.mount_points.iter().filter(|m| m.kind == "bind") {
            map.entry(m.source.clone()).or_default().push(BindUser {
                id: c.id.clone(),
                name: c.name.clone(),
                state: c.state.clone(),
                destination: m.destination.clone(),
                rw: m.rw,
                propagation: m.propagation.clone(),
            });
        }
    }
    map.into_iter()
        .map(|(source, mut users)| {
            users.sort_by(|a, b| (&a.name, &a.destination).cmp(&(&b.name, &b.destination)));
            BindSummary {
                host: inspect(&source),
                risk: risk_of(&source),
                writable: users.iter().any(|u| u.rw),
                containers: users,
                source,
            }
        })
        .collect()
}

impl Engine {
    /// Directorios vinculados de todos los contenedores (`GET /containers/json`).
    pub async fn list_binds(
        &self,
        containers: Option<&[ContainerSummary]>,
    ) -> Result<Vec<BindSummary>, EngineError> {
        let owned;
        let containers = match containers {
            Some(c) => c,
            None => {
                owned = self.list_containers().await?;
                &owned
            }
        };
        let list = containers.to_vec();
        if self.ssh_session().is_none() {
            return tokio::task::spawn_blocking(move || binds_from(&list, host_path_info))
                .await
                .map_err(|e| EngineError::Other {
                    message: e.to_string(),
                });
        }
        // Servidor remoto: las rutas son del servidor y se consultan allí.
        let mut paths: Vec<String> = list
            .iter()
            .flat_map(|c| c.mount_points.iter().filter(|m| m.kind == "bind"))
            .map(|m| m.source.clone())
            .collect();
        paths.sort();
        paths.dedup();
        let info = self.host_paths(&paths).await?;
        Ok(binds_from(&list, |p| {
            info.get(p).cloned().unwrap_or(HostPathInfo {
                kind: PathKind::Missing,
                uid: None,
                mode: None,
                readable: false,
            })
        }))
    }

    /// Estado de unas rutas **del equipo donde está Docker**: este equipo o,
    /// con un servidor remoto, el servidor (0.10.0). En remoto son dos
    /// órdenes de solo lectura (`stat` y `find -readable`) por la sesión SSH,
    /// registradas en actividad; las rutas van entrecomilladas una a una.
    pub async fn host_paths(
        &self,
        paths: &[String],
    ) -> Result<std::collections::HashMap<String, HostPathInfo>, EngineError> {
        let mut out = std::collections::HashMap::new();
        let Some(session) = self.ssh_session().cloned() else {
            for p in paths {
                out.insert(p.clone(), host_path_info(p));
            }
            return Ok(out);
        };
        let paths: Vec<String> = paths
            .iter()
            .filter(|p| p.starts_with('/') && !p.contains('\0'))
            .cloned()
            .collect();
        if paths.is_empty() {
            return Ok(out);
        }
        // `before` + rutas + `after`, cada una como argumento separado.
        let run = |program: &str, before: Vec<String>, after: Vec<String>, kind: &str| {
            let mut args = before;
            args.extend(paths.iter().cloned());
            args.extend(after);
            crate::cli::run(
                self.activity(),
                crate::cli::CliRequest {
                    category: dcc_activity::Category::Bind,
                    kind: kind.into(),
                    target: paths.first().cloned(),
                    program: program.into(),
                    args,
                    env: vec![],
                    cwd: None,
                    timeout: std::time::Duration::from_secs(30),
                    remote: Some(crate::cli::RemoteExec {
                        session: session.clone(),
                        tty: false,
                    }),
                },
            )
        };
        let stat = run(
            "env",
            ["LC_ALL=C", "stat", "-L", "--printf", "%F|%u|%a|%n\\0", "--"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            vec![],
            "bind.inspect",
        )
        .await?;
        // `find … -maxdepth 0 -readable`: las que puede leer el usuario SSH.
        let readable_out = run(
            "find",
            vec![],
            ["-maxdepth", "0", "-readable", "-print0"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            "bind.inspect",
        )
        .await;
        let readable: std::collections::HashSet<String> = match readable_out {
            Ok(o) => o.stdout.split('\0').map(str::to_owned).collect(),
            Err(_) => Default::default(),
        };
        for entry in stat.stdout.split('\0').filter(|e| !e.is_empty()) {
            let mut it = entry.splitn(4, '|');
            let (Some(kind), Some(uid), Some(mode), Some(name)) =
                (it.next(), it.next(), it.next(), it.next())
            else {
                continue;
            };
            let kind = match kind {
                "directory" => PathKind::Directory,
                "regular file" | "regular empty file" => PathKind::File,
                "socket" => PathKind::Socket,
                _ => PathKind::Other,
            };
            out.insert(
                name.to_owned(),
                HostPathInfo {
                    readable: matches!(kind, PathKind::Directory | PathKind::File)
                        && readable.contains(name),
                    kind,
                    uid: uid.parse().ok(),
                    mode: u32::from_str_radix(mode, 8).ok(),
                },
            );
        }
        for p in &paths {
            out.entry(p.clone()).or_insert(HostPathInfo {
                kind: PathKind::Missing,
                uid: None,
                mode: None,
                readable: false,
            });
        }
        Ok(out)
    }

    /// Problemas de una especificación de contenedor comprobando las carpetas
    /// de los bind mounts **donde está Docker** (este equipo o el servidor).
    pub async fn spec_issues(
        &self,
        spec: &crate::spec::ContainerSpec,
    ) -> Result<Vec<crate::spec::SpecIssue>, EngineError> {
        if self.ssh_session().is_none() {
            return Ok(spec.validate(true));
        }
        let mut issues = spec.validate(false);
        let binds: Vec<(usize, String)> = spec
            .mounts
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                m.kind == crate::spec::MountKind::Bind && m.source.trim().starts_with('/')
            })
            .map(|(i, m)| (i, m.source.trim().to_owned()))
            .collect();
        issues.extend(self.missing_bind_issues(&binds).await?);
        Ok(issues)
    }

    /// `bind.missing` para las carpetas que no existen en el servidor remoto.
    pub(crate) async fn missing_bind_issues(
        &self,
        binds: &[(usize, String)],
    ) -> Result<Vec<crate::spec::SpecIssue>, EngineError> {
        if binds.is_empty() {
            return Ok(vec![]);
        }
        let paths: Vec<String> = binds.iter().map(|(_, p)| p.clone()).collect();
        let info = self.host_paths(&paths).await?;
        Ok(binds
            .iter()
            .filter(|(_, p)| info.get(p).map_or(true, |i| i.kind == PathKind::Missing))
            .map(|(i, p)| {
                crate::spec::issue(
                    crate::spec::Severity::Error,
                    format!("mounts.{i}.source"),
                    "bind.missing",
                    &[("path", p.clone())],
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::MountPoint;

    #[test]
    fn risks() {
        assert_eq!(
            risk_of("/var/run/docker.sock"),
            Some(BindRisk::DockerSocket)
        );
        assert_eq!(
            risk_of("/run/user/1000/docker.sock"),
            Some(BindRisk::DockerSocket)
        );
        assert_eq!(risk_of("/"), Some(BindRisk::HostRoot));
        assert_eq!(risk_of("/etc/"), Some(BindRisk::System));
        assert_eq!(risk_of("/var/lib/docker"), Some(BindRisk::System));
        // E1 (D-086): sin saber cómo se monta, se evalúa con escritura, y una
        // carpeta de /etc con escritura cambia la configuración del equipo.
        assert_eq!(risk_of("/etc/nginx"), Some(BindRisk::System));
        assert_eq!(risk_of("//etc"), Some(BindRisk::System));
        assert_eq!(risk_of("/home/ana/.ssh"), Some(BindRisk::System));
        assert_eq!(
            risk_of("/run/containerd/containerd.sock"),
            Some(BindRisk::DockerSocket)
        );
        assert_eq!(risk_of("/home/ana/proyecto"), None);
    }

    #[test]
    fn host_paths_are_inspected_for_real() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("carpeta");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o750)).unwrap();
        let file = d.path().join("archivo.conf");
        std::fs::write(&file, "x").unwrap();
        let link = d.path().join("enlace");
        std::os::unix::fs::symlink(&dir, &link).unwrap();
        let i = host_path_info(dir.to_str().unwrap());
        assert_eq!(i.kind, PathKind::Directory);
        assert_eq!(i.mode, Some(0o750));
        assert!(i.readable);
        assert_eq!(host_path_info(file.to_str().unwrap()).kind, PathKind::File);
        assert_eq!(
            host_path_info(link.to_str().unwrap()).kind,
            PathKind::Directory,
            "sigue el enlace"
        );
        let missing = host_path_info(d.path().join("no-existe").to_str().unwrap());
        assert_eq!(
            missing,
            HostPathInfo {
                kind: PathKind::Missing,
                uid: None,
                mode: None,
                readable: false
            }
        );
        let l = std::os::unix::net::UnixListener::bind(d.path().join("s.sock")).unwrap();
        assert_eq!(
            host_path_info(d.path().join("s.sock").to_str().unwrap()).kind,
            PathKind::Socket
        );
        drop(l);
    }

    #[test]
    fn binds_are_grouped_by_host_path() {
        let mp = |kind: &str, src: &str, dest: &str, rw: bool| MountPoint {
            kind: kind.into(),
            name: None,
            source: src.into(),
            destination: dest.into(),
            rw,
            mode: String::new(),
            propagation: "rprivate".into(),
        };
        let c = |id: &str, name: &str, state: &str, mounts: Vec<MountPoint>| ContainerSummary {
            id: id.into(),
            name: name.into(),
            image: String::new(),
            image_id: String::new(),
            command: String::new(),
            created: 0,
            state: state.into(),
            status: String::new(),
            ports: vec![],
            networks: vec![],
            mounts: mounts.len() as u32,
            mount_points: mounts,
            compose_project: None,
            labels: Default::default(),
        };
        let list = [
            c(
                "1",
                "web",
                "running",
                vec![
                    mp("bind", "/srv/web", "/usr/share/nginx/html", false),
                    mp("volume", "/var/lib/docker/volumes/x/_data", "/x", true),
                ],
            ),
            c(
                "2",
                "editor",
                "exited",
                vec![mp("bind", "/srv/web", "/work", true)],
            ),
            c(
                "3",
                "ci",
                "running",
                vec![mp(
                    "bind",
                    "/var/run/docker.sock",
                    "/var/run/docker.sock",
                    true,
                )],
            ),
        ];
        let b = binds_from(&list, |p| HostPathInfo {
            kind: if p.ends_with(".sock") {
                PathKind::Socket
            } else {
                PathKind::Missing
            },
            uid: None,
            mode: None,
            readable: false,
        });
        assert_eq!(b.len(), 2, "los volúmenes no son directorios vinculados");
        assert_eq!(b[0].source, "/srv/web");
        assert!(b[0].writable);
        assert_eq!(b[0].risk, None);
        assert_eq!(
            b[0].containers
                .iter()
                .map(|u| (u.name.as_str(), u.destination.as_str(), u.rw))
                .collect::<Vec<_>>(),
            [
                ("editor", "/work", true),
                ("web", "/usr/share/nginx/html", false)
            ]
        );
        assert_eq!(b[1].risk, Some(BindRisk::DockerSocket));
        assert_eq!(b[1].host.kind, PathKind::Socket);
    }
}
