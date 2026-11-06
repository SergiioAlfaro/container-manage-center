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

pub fn risk_of(path: &str) -> Option<BindRisk> {
    let p = path.trim_end_matches('/');
    let p = if p.is_empty() { "/" } else { p };
    if p.ends_with("/docker.sock") || p == "docker.sock" {
        return Some(BindRisk::DockerSocket);
    }
    if p == "/" {
        return Some(BindRisk::HostRoot);
    }
    if crate::spec::SENSITIVE_BINDS.contains(&p)
        || ["/usr", "/bin", "/sbin", "/lib", "/lib64", "/var/lib/docker"].contains(&p)
    {
        return Some(BindRisk::System);
    }
    None
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
        tokio::task::spawn_blocking(move || binds_from(&list, host_path_info))
            .await
            .map_err(|e| EngineError::Other {
                message: e.to_string(),
            })
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
        assert_eq!(
            risk_of("/etc/nginx"),
            None,
            "una subcarpeta concreta no se marca"
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
