// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! E6 (1.6.0): enlaces entre herramientas (D-108).
//!
//! La auditoría muestra, para cada contenedor, si la monitorización lo vigila
//! y qué copia lo protege. Las herramientas no se leen entre sí: este módulo
//! cruza lo que cada una ya guarda **por identificadores** (id del servidor y
//! del contenedor), como pide el plan (§4.1):
//!
//! - monitorización: el estado del muestreador del servidor (última muestra y
//!   alertas activas), por id de contenedor;
//! - copias: los manifiestos de las copias completas del servidor (por id de
//!   contenedor desde la 1.6.0; por nombre en las copias anteriores, y así se
//!   indica) y las tareas programadas que lo incluyen.
//!
//! Solo lee. No escribe nada ni registra nada en el panel de actividad.

use std::collections::{BTreeSet, HashMap};

use cmc_engine::backup::{
    BackupEntry, BackupState, Consistency, ItemKind, Manifest, DETAIL_CONTAINER_ID,
    DETAIL_USED_BY_IDS,
};
use cmc_engine::monitor::AlertKind;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::backups::BackupJob;
use crate::monitor::MonitorStatus;

/// Un contenedor por el que se pregunta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LinkTarget {
    pub id: String,
    pub name: String,
    pub project: Option<String>,
}

/// Lo que la monitorización sabe del contenedor.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MonitorLink {
    /// La monitorización del servidor está activada.
    pub enabled: bool,
    /// El contenedor (mismo id) está en la última muestra.
    pub sampled: bool,
    /// Estado en la última muestra.
    pub state: Option<String>,
    /// Alertas activas de este contenedor.
    pub firing: Vec<AlertKind>,
}

/// Cómo se reconoce el contenedor en una copia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LinkMatch {
    /// Mismo id de contenedor (copias de la 1.6.0 en adelante).
    Id,
    /// Mismo nombre: copia anterior a la 1.6.0, o el contenedor se ha vuelto a
    /// crear desde la copia (otro id).
    Name,
    /// Proyecto Compose del contenedor.
    Project,
}

/// La copia más reciente que incluye algo del contenedor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupLink {
    pub backup_id: String,
    pub created_at: String,
    /// Carpeta de destino donde está.
    pub destination: String,
    /// Lo que incluye del contenedor (configuración, base de datos, volumen…).
    pub kinds: Vec<cmc_engine::backup::ItemKind>,
    /// La mejor coincidencia encontrada en esa copia.
    #[serde(rename = "match")]
    #[ts(rename = "match")]
    pub matched: LinkMatch,
    /// Algún volumen se copió en caliente (no garantizado).
    pub hot: bool,
    /// Tarea que la creó (`None` si fue manual).
    pub job: Option<String>,
}

/// Tarea de copias que incluye el contenedor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobLink {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolLinks {
    /// Id del contenedor (el de la petición).
    pub id: String,
    pub monitor: MonitorLink,
    pub backup: Option<BackupLink>,
    pub jobs: Vec<JobLink>,
}

/// Lo que se sabe de los volúmenes de cada contenedor (id → volúmenes con
/// nombre). Vacío si no hay conexión: entonces las tareas solo se reconocen
/// por el nombre del contenedor o su proyecto.
pub type VolumesOf = HashMap<String, Vec<String>>;

fn monitor_link(status: &MonitorStatus, id: &str) -> MonitorLink {
    let sample = status
        .last
        .as_ref()
        .and_then(|s| s.containers.iter().find(|c| c.meta.id == id));
    let mut firing: Vec<AlertKind> = status
        .firing
        .iter()
        .filter(|f| f.container_id == id)
        .map(|f| f.kind)
        .collect();
    firing.sort_by_key(|k| format!("{k:?}"));
    firing.dedup();
    MonitorLink {
        enabled: status.enabled,
        sampled: sample.is_some(),
        state: sample.map(|c| c.meta.state.clone()),
        firing,
    }
}

/// Coincidencia de un elemento de la copia con el contenedor.
fn item_match(item: &cmc_engine::backup::BackupItem, t: &LinkTarget) -> Option<LinkMatch> {
    match item.kind {
        ItemKind::Database | ItemKind::Container => {
            match item.details.get(DETAIL_CONTAINER_ID) {
                Some(id) if *id == t.id => Some(LinkMatch::Id),
                // Con otro id: el contenedor se ha vuelto a crear con el mismo
                // nombre. Sin id: copia anterior a la 1.6.0.
                _ if item.name == t.name => Some(LinkMatch::Name),
                _ => None,
            }
        }
        ItemKind::Volume => {
            let by_id = item
                .details
                .get(DETAIL_USED_BY_IDS)
                .is_some_and(|ids| ids.split(',').any(|x| x == t.id));
            if by_id {
                return Some(LinkMatch::Id);
            }
            // `usedBy`: «nombre:/destino, nombre:/destino».
            let by_name = item.details.get("usedBy").is_some_and(|u| {
                u.split(", ")
                    .any(|x| x.split_once(':').is_some_and(|(n, _)| n == t.name))
            });
            by_name.then_some(LinkMatch::Name)
        }
        ItemKind::Compose => t
            .project
            .as_deref()
            .filter(|p| *p == item.name)
            .map(|_| LinkMatch::Project),
        ItemKind::AppConfig => None,
    }
}

fn backup_link(backups: &[(String, Manifest)], t: &LinkTarget) -> Option<BackupLink> {
    // `backups` va de la más reciente a la más antigua.
    for (dest, m) in backups {
        let mut kinds = BTreeSet::new();
        let mut best: Option<LinkMatch> = None;
        let mut hot = false;
        for item in &m.items {
            if let Some(x) = item_match(item, t) {
                kinds.insert(item.kind);
                best = Some(best.map_or(x, |b| b.min(x)));
                hot |= item.consistency == Consistency::Hot;
            }
        }
        if let Some(matched) = best {
            return Some(BackupLink {
                backup_id: m.id.clone(),
                created_at: m.created_at.clone(),
                destination: dest.clone(),
                kinds: kinds.into_iter().collect(),
                matched,
                hot,
                job: m.job.clone(),
            });
        }
    }
    None
}

fn job_links(jobs: &[BackupJob], server: &str, t: &LinkTarget, vols: &VolumesOf) -> Vec<JobLink> {
    let mine = vols.get(&t.id);
    jobs.iter()
        .filter(|j| j.server == server)
        .filter(|j| {
            let r = &j.request;
            r.containers.contains(&t.name)
                || r.databases.contains(&t.name)
                || t.project
                    .as_ref()
                    .is_some_and(|p| r.compose.iter().any(|c| c.project == *p))
                || mine.is_some_and(|v| r.volumes.iter().any(|s| v.contains(&s.name)))
        })
        .map(|j| JobLink {
            id: j.id.clone(),
            name: j.name.clone(),
        })
        .collect()
}

/// Copias completas de varias carpetas, ordenadas de la más reciente a la más
/// antigua (las interrumpidas no protegen nada).
pub fn complete_backups(lists: Vec<(String, Vec<BackupEntry>)>) -> Vec<(String, Manifest)> {
    let mut v: Vec<(String, Manifest)> = lists
        .into_iter()
        .flat_map(|(dest, entries)| {
            entries.into_iter().filter_map(move |e| {
                (e.state == BackupState::Complete)
                    .then_some(e.manifest)
                    .flatten()
                    .map(|m| (dest.clone(), m))
            })
        })
        .collect();
    v.sort_by(|a, b| b.1.created_at.cmp(&a.1.created_at));
    // Una misma carpeta puede aparecer dos veces (destino por defecto y tarea).
    let mut seen = BTreeSet::new();
    v.retain(|(_, m)| seen.insert(m.id.clone()));
    v
}

/// Cruce puro: lo prueban las pruebas unitarias sin Docker ni disco.
pub fn compute(
    server: &str,
    targets: &[LinkTarget],
    status: &MonitorStatus,
    backups: &[(String, Manifest)],
    jobs: &[BackupJob],
    vols: &VolumesOf,
) -> Vec<ToolLinks> {
    targets
        .iter()
        .map(|t| ToolLinks {
            id: t.id.clone(),
            monitor: monitor_link(status, &t.id),
            backup: backup_link(backups, t),
            jobs: job_links(jobs, server, t, vols),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::FiringAlert;
    use cmc_engine::backup::{
        BackupItem, BackupRequest, ComposeSelection, VolumeMode, VolumeSelection,
    };
    use cmc_engine::monitor::{ContainerMeta, ContainerSample, Counts, ServerSample};
    use std::collections::BTreeMap;

    const WEB: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const OTHER: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn target(id: &str, name: &str, project: Option<&str>) -> LinkTarget {
        LinkTarget {
            id: id.into(),
            name: name.into(),
            project: project.map(Into::into),
        }
    }

    fn meta(id: &str, name: &str) -> ContainerSample {
        ContainerSample {
            meta: ContainerMeta {
                id: id.into(),
                name: name.into(),
                image: "img".into(),
                state: "running".into(),
                compose_project: None,
                started_at: None,
                restart_count: None,
            },
            point: None,
        }
    }

    fn status(
        enabled: bool,
        containers: Vec<ContainerSample>,
        firing: Vec<FiringAlert>,
    ) -> MonitorStatus {
        MonitorStatus {
            server: "local".into(),
            enabled,
            paused: false,
            running: enabled,
            interval_secs: 5,
            host: None,
            last: Some(ServerSample {
                t: 1,
                connected: true,
                counts: Counts::default(),
                cpu_total: None,
                mem_total: 0,
                containers,
                errors: 0,
                took_ms: 1,
            }),
            gap: false,
            last_error: None,
            firing,
        }
    }

    fn item(
        kind: ItemKind,
        name: &str,
        details: &[(&str, &str)],
        consistency: Consistency,
    ) -> BackupItem {
        BackupItem {
            kind,
            name: name.into(),
            files: vec![],
            consistency,
            details: details
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect::<BTreeMap<_, _>>(),
            warnings: vec![],
        }
    }

    fn manifest(id: &str, at: &str, items: Vec<BackupItem>) -> Manifest {
        Manifest {
            format: 1,
            id: id.into(),
            server: "local".into(),
            created_at: at.into(),
            finished_at: at.into(),
            app_version: "1.6.0".into(),
            job: None,
            items,
        }
    }

    fn job(id: &str, server: &str, request: BackupRequest) -> BackupJob {
        let mut j = BackupJob::new_for_tests(id);
        j.server = server.into();
        j.name = format!("tarea {id}");
        j.request = request;
        j
    }

    #[test]
    fn monitoring_is_matched_by_container_id() {
        let st = status(
            true,
            vec![meta(WEB, "web"), meta(OTHER, "db")],
            vec![
                FiringAlert {
                    container_id: WEB.into(),
                    kind: AlertKind::Cpu,
                },
                FiringAlert {
                    container_id: OTHER.into(),
                    kind: AlertKind::Memory,
                },
            ],
        );
        // «db» se llama igual que un contenedor de la muestra, pero el id es
        // otro: no se confunde.
        let out = compute(
            "local",
            &[target(WEB, "web", None), target(DB, "db", None)],
            &st,
            &[],
            &[],
            &HashMap::new(),
        );
        assert!(out[0].monitor.enabled && out[0].monitor.sampled);
        assert_eq!(out[0].monitor.state.as_deref(), Some("running"));
        assert_eq!(out[0].monitor.firing, vec![AlertKind::Cpu]);
        assert!(out[1].monitor.enabled);
        assert!(
            !out[1].monitor.sampled,
            "otro id, aunque el nombre coincida"
        );
        assert!(out[1].monitor.firing.is_empty());
        // Monitorización desactivada.
        let off = MonitorStatus {
            last: None,
            ..status(false, vec![], vec![])
        };
        let out = compute(
            "local",
            &[target(WEB, "web", None)],
            &off,
            &[],
            &[],
            &HashMap::new(),
        );
        assert_eq!(out[0].monitor, MonitorLink::default());
    }

    #[test]
    fn backups_match_by_id_then_by_name_and_newest_first() {
        let backups = vec![
            // La más reciente: solo la configuración de otro contenedor.
            (
                "/d".to_owned(),
                manifest(
                    "c3",
                    "2026-11-15T10:00:00Z",
                    vec![item(
                        ItemKind::Container,
                        "otro",
                        &[(DETAIL_CONTAINER_ID, OTHER)],
                        Consistency::NotApplicable,
                    )],
                ),
            ),
            // 1.6.0: base de datos con id y volumen usado por «web» en caliente.
            (
                "/d".to_owned(),
                manifest(
                    "c2",
                    "2026-11-15T09:00:00Z",
                    vec![
                        item(
                            ItemKind::Database,
                            "db",
                            &[(DETAIL_CONTAINER_ID, DB)],
                            Consistency::Dump,
                        ),
                        item(
                            ItemKind::Volume,
                            "datos",
                            &[("usedBy", "web:/datos"), (DETAIL_USED_BY_IDS, WEB)],
                            Consistency::Hot,
                        ),
                        item(ItemKind::Compose, "tienda", &[], Consistency::NotApplicable),
                    ],
                ),
            ),
            // Anterior a la 1.6.0: sin ids.
            (
                "/e".to_owned(),
                manifest(
                    "c1",
                    "2026-11-14T09:00:00Z",
                    vec![item(
                        ItemKind::Container,
                        "viejo",
                        &[],
                        Consistency::NotApplicable,
                    )],
                ),
            ),
        ];
        let st = status(false, vec![], vec![]);
        let t = [
            target(WEB, "web", Some("tienda")),
            target(DB, "db", None),
            target("dddd", "viejo", None),
            target("eeee", "nadie", None),
        ];
        let out = compute("local", &t, &st, &backups, &[], &HashMap::new());
        let web = out[0].backup.as_ref().unwrap();
        assert_eq!(web.backup_id, "c2");
        assert_eq!(web.matched, LinkMatch::Id, "el volumen lo montaba este id");
        assert_eq!(web.kinds, vec![ItemKind::Volume, ItemKind::Compose]);
        assert!(web.hot);
        let db = out[1].backup.as_ref().unwrap();
        assert_eq!(
            (db.backup_id.as_str(), db.matched, db.hot),
            ("c2", LinkMatch::Id, false)
        );
        assert_eq!(db.kinds, vec![ItemKind::Database]);
        let old = out[2].backup.as_ref().unwrap();
        assert_eq!(
            (
                old.backup_id.as_str(),
                old.matched,
                old.destination.as_str()
            ),
            ("c1", LinkMatch::Name, "/e")
        );
        assert!(out[3].backup.is_none());
    }

    #[test]
    fn a_recreated_container_is_only_matched_by_name() {
        let backups = vec![(
            "/d".to_owned(),
            manifest(
                "c1",
                "2026-11-15T09:00:00Z",
                vec![item(
                    ItemKind::Container,
                    "web",
                    &[(DETAIL_CONTAINER_ID, OTHER)],
                    Consistency::NotApplicable,
                )],
            ),
        )];
        let out = compute(
            "local",
            &[target(WEB, "web", None)],
            &status(false, vec![], vec![]),
            &backups,
            &[],
            &HashMap::new(),
        );
        assert_eq!(out[0].backup.as_ref().unwrap().matched, LinkMatch::Name);
    }

    #[test]
    fn jobs_include_the_container_by_name_project_or_volume_of_this_server() {
        let jobs = vec![
            job(
                "job-00000001",
                "local",
                BackupRequest {
                    containers: vec!["web".into()],
                    ..Default::default()
                },
            ),
            job(
                "job-00000002",
                "local",
                BackupRequest {
                    databases: vec!["db".into()],
                    ..Default::default()
                },
            ),
            job(
                "job-00000003",
                "local",
                BackupRequest {
                    compose: vec![ComposeSelection {
                        project: "tienda".into(),
                        include_env: false,
                    }],
                    ..Default::default()
                },
            ),
            job(
                "job-00000004",
                "local",
                BackupRequest {
                    volumes: vec![VolumeSelection {
                        name: "datos".into(),
                        mode: VolumeMode::Hot,
                    }],
                    ..Default::default()
                },
            ),
            // Otro servidor con un contenedor del mismo nombre.
            job(
                "job-00000005",
                "srv-otro",
                BackupRequest {
                    containers: vec!["web".into()],
                    ..Default::default()
                },
            ),
        ];
        let vols: VolumesOf = HashMap::from([(WEB.to_owned(), vec!["datos".to_owned()])]);
        let st = status(false, vec![], vec![]);
        let out = compute(
            "local",
            &[target(WEB, "web", Some("tienda")), target(DB, "db", None)],
            &st,
            &[],
            &jobs,
            &vols,
        );
        let ids: Vec<&str> = out[0].jobs.iter().map(|j| j.id.as_str()).collect();
        assert_eq!(ids, vec!["job-00000001", "job-00000003", "job-00000004"]);
        let ids: Vec<&str> = out[1].jobs.iter().map(|j| j.id.as_str()).collect();
        assert_eq!(ids, vec!["job-00000002"]);
        // Sin conexión no se sabe qué volúmenes monta: solo nombre y proyecto.
        let out = compute(
            "local",
            &[target(WEB, "web", Some("tienda"))],
            &st,
            &[],
            &jobs,
            &HashMap::new(),
        );
        assert_eq!(out[0].jobs.len(), 2);
    }

    #[test]
    fn only_complete_backups_count_and_duplicates_are_removed() {
        let m = |id: &str, at: &str| manifest(id, at, vec![]);
        let entry = |id: &str, at: &str, state: BackupState| BackupEntry {
            id: id.into(),
            path: format!("/d/{id}"),
            state,
            manifest: (state == BackupState::Complete).then(|| m(id, at)),
            size: 0,
        };
        let v = complete_backups(vec![
            (
                "/d".into(),
                vec![
                    entry("a", "2026-11-14T09:00:00Z", BackupState::Complete),
                    entry("b", "2026-11-15T09:00:00Z", BackupState::Incomplete),
                ],
            ),
            (
                "/e".into(),
                vec![entry("c", "2026-11-15T10:00:00Z", BackupState::Complete)],
            ),
            (
                "/d".into(),
                vec![entry("a", "2026-11-14T09:00:00Z", BackupState::Complete)],
            ),
        ]);
        let ids: Vec<&str> = v.iter().map(|(_, m)| m.id.as_str()).collect();
        assert_eq!(ids, vec!["c", "a"]);
    }
}
