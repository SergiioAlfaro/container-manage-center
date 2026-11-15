// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Auditoría de seguridad (E3, 1.3.0, D-093 y D-094).
//!
//! **Solo lectura**: lee `docker info`, los contenedores (`inspect`), las
//! imágenes y la configuración de Compose, y nunca cambia nada. Cada lectura
//! real queda en el panel de actividad como siempre. Las correcciones se
//! proponen en «Editar configuración», con vista previa y confirmación.

pub mod checks;
pub mod model;
pub mod vuln;

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use cmc_activity::{Category, Failure, OperationSpec, Outcome, Transport};
use tokio_util::sync::CancellationToken;

pub use model::*;

use crate::client::Engine;
use crate::compose::project::{ComposeCli, ProjectRef};
use crate::error::EngineError;

fn cancelled() -> EngineError {
    EngineError::Other {
        message: "auditoría cancelada".into(),
    }
}

/// Imagen que se pasa a la herramienta de vulnerabilidades: el nombre si lo
/// tiene, si no el id.
fn scan_ref(name: &str, id: &str) -> String {
    if name.is_empty() || name.starts_with("sha256:") {
        id.to_owned()
    } else {
        name.to_owned()
    }
}

impl Engine {
    /// Ejecuta una auditoría. `compose`: el CLI de Compose y los proyectos
    /// conocidos (si se piden). `progress` recibe el avance.
    pub async fn audit<F>(
        &self,
        server: &str,
        req: &AuditRequest,
        compose: Option<(ComposeCli, Vec<ProjectRef>)>,
        cancel: CancellationToken,
        mut progress: F,
    ) -> Result<AuditReport, EngineError>
    where
        F: FnMut(AuditProgress) + Send,
    {
        if let Some(c) = &req.container {
            if c.trim().is_empty() || c.len() > 256 {
                return Err(EngineError::InvalidInput {
                    message: "contenedor no válido".into(),
                });
            }
        }
        let started = Utc::now();
        let handle = self.activity().begin(OperationSpec {
            category: Category::System,
            kind: "audit.run".into(),
            target: req.container.clone(),
            transport: Transport::App {
                action: "Auditoría de seguridad (solo lectura)".into(),
            },
            request_body: None,
            equivalent_command: None,
        });
        let r = self
            .audit_inner(server, req, compose, &cancel, &mut progress)
            .await;
        match &r {
            Ok(rep) => handle.succeed(Outcome {
                summary: Some(format!(
                    "{} elementos, {} hallazgos ({} críticos, {} altos)",
                    rep.targets.len(),
                    rep.findings.len(),
                    rep.counts.get("critical").copied().unwrap_or(0),
                    rep.counts.get("high").copied().unwrap_or(0)
                )),
                ..Default::default()
            }),
            Err(e) => {
                let f = Failure {
                    error: e.to_string(),
                    stdout: None,
                    stderr: None,
                    http_status: None,
                    exit_code: None,
                };
                if cancel.is_cancelled() {
                    handle.cancel(f)
                } else {
                    handle.fail(f)
                }
            }
        }
        r.map(|mut rep| {
            rep.started_at = started.to_rfc3339();
            rep
        })
    }

    async fn audit_inner<F>(
        &self,
        server: &str,
        req: &AuditRequest,
        compose: Option<(ComposeCli, Vec<ProjectRef>)>,
        cancel: &CancellationToken,
        progress: &mut F,
    ) -> Result<AuditReport, EngineError>
    where
        F: FnMut(AuditProgress) + Send,
    {
        let now = Utc::now();
        let local = self.ssh_session().is_none();
        let info = self.info().await?;
        let daemon = checks::daemon_security(
            &info.security_options,
            info.rootless,
            info.cgroup_version.as_deref(),
        );
        let mut findings = Vec::new();
        let mut targets = Vec::new();
        let mut skipped = Vec::new();
        // Imágenes ya leídas: id → (usuario, fecha de creación, nombre).
        let mut images: BTreeMap<String, (String, Option<String>, String)> = BTreeMap::new();
        let mut to_scan: BTreeSet<String> = BTreeSet::new();

        // --- contenedores
        let all = self.list_containers().await?;
        let selected: Vec<_> = match &req.container {
            Some(c) => {
                let c = c.trim();
                let found: Vec<_> = all
                    .into_iter()
                    .filter(|x| x.id == c || x.name == c || (c.len() >= 12 && x.id.starts_with(c)))
                    .collect();
                if found.is_empty() {
                    return Err(EngineError::Docker {
                        status: 404,
                        message: format!("no existe el contenedor {c}"),
                    });
                }
                found
            }
            None => all,
        };
        let total = selected.len() as u32;
        for (i, s) in selected.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(cancelled());
            }
            progress(AuditProgress {
                step: "containers".into(),
                done: i as u32,
                total,
                current: Some(s.name.clone()),
            });
            let insp = match self.inspect_raw(&s.id, Some(&s.name)).await {
                Ok(x) => x,
                // Eliminado mientras tanto.
                Err(EngineError::Docker { status: 404, .. }) => continue,
                Err(e) => return Err(e),
            };
            if !images.contains_key(&s.image_id) {
                let entry = match self.image_details(&s.image_id).await {
                    Ok(d) => (d.user, d.created, s.image.clone()),
                    Err(_) => (String::new(), None, s.image.clone()),
                };
                images.insert(s.image_id.clone(), entry);
            }
            let created = images.get(&s.image_id).and_then(|x| x.1.clone());
            findings.extend(checks::check_container(
                &insp,
                created.as_deref(),
                &daemon,
                local,
                now,
            ));
            targets.push(AuditedTarget {
                target: TargetRef {
                    kind: TargetKind::Container,
                    id: s.id.clone(),
                    name: s.name.clone(),
                    project: s.compose_project.clone(),
                },
                state: s.state.clone(),
                image: s.image.clone(),
            });
            to_scan.insert(scan_ref(&s.image, &s.image_id));
        }

        let whole = req.container.is_none();
        // --- imágenes
        if whole && req.include_images {
            let list = self.list_images(None).await?;
            let total = list.len() as u32;
            for (i, im) in list.iter().enumerate() {
                if cancel.is_cancelled() {
                    return Err(cancelled());
                }
                let name = im
                    .repo_tags
                    .iter()
                    .find(|t| !t.ends_with(":<none>"))
                    .cloned()
                    .unwrap_or_else(|| im.id.clone());
                progress(AuditProgress {
                    step: "images".into(),
                    done: i as u32,
                    total,
                    current: Some(name.clone()),
                });
                let (user, created) = match images.get(&im.id) {
                    Some((u, c, _)) => (u.clone(), c.clone()),
                    None => match self.image_details(&im.id).await {
                        Ok(d) => (d.user, d.created),
                        Err(_) => continue,
                    },
                };
                findings.extend(checks::check_image(
                    &im.id,
                    &name,
                    &user,
                    created.as_deref(),
                    &daemon,
                    now,
                ));
                targets.push(AuditedTarget {
                    target: TargetRef {
                        kind: TargetKind::Image,
                        id: im.id.clone(),
                        name: name.clone(),
                        project: None,
                    },
                    state: String::new(),
                    image: name.clone(),
                });
                if !name.starts_with("sha256:") {
                    to_scan.insert(name);
                }
            }
        }

        // --- Compose
        if whole && req.include_compose {
            match compose {
                None => skipped.push(AuditSkipped {
                    target: TargetRef {
                        kind: TargetKind::Service,
                        id: String::new(),
                        name: String::new(),
                        project: None,
                    },
                    reason: "audit.skipped.composeUnavailable".into(),
                    detail: None,
                }),
                Some((cli, projects)) => {
                    let total = projects.len() as u32;
                    for (i, p) in projects.iter().enumerate() {
                        if cancel.is_cancelled() {
                            return Err(cancelled());
                        }
                        progress(AuditProgress {
                            step: "compose".into(),
                            done: i as u32,
                            total,
                            current: Some(p.name.clone()),
                        });
                        let project_target = TargetRef {
                            kind: TargetKind::Service,
                            id: p.name.clone(),
                            name: String::new(),
                            project: Some(p.name.clone()),
                        };
                        if p.files.is_empty() {
                            skipped.push(AuditSkipped {
                                target: project_target,
                                reason: "audit.skipped.noFiles".into(),
                                detail: None,
                            });
                            continue;
                        }
                        let cfg = match self.compose_config(&cli, p, None).await {
                            Ok(c) => c,
                            Err(e) => {
                                skipped.push(AuditSkipped {
                                    target: project_target,
                                    reason: "audit.skipped.composeConfig".into(),
                                    detail: Some(e.to_string()),
                                });
                                continue;
                            }
                        };
                        let (Some(model), Some(resolved)) = (cfg.model, cfg.resolved) else {
                            skipped.push(AuditSkipped {
                                target: project_target,
                                reason: "audit.skipped.composeConfig".into(),
                                detail: cfg.error,
                            });
                            continue;
                        };
                        let json: serde_json::Value =
                            serde_json::from_str(&resolved).unwrap_or_default();
                        for svc in &model.services {
                            let raw = json
                                .pointer(&format!(
                                    "/services/{}",
                                    svc.name.replace('~', "~0").replace('/', "~1")
                                ))
                                .cloned()
                                .unwrap_or_default();
                            findings.extend(checks::check_compose_service(
                                &p.name, &svc.name, &raw, &svc.risks, &daemon,
                            ));
                            targets.push(AuditedTarget {
                                target: TargetRef {
                                    kind: TargetKind::Service,
                                    id: format!("{}/{}", p.name, svc.name),
                                    name: svc.name.clone(),
                                    project: Some(p.name.clone()),
                                },
                                state: String::new(),
                                image: svc.image.clone().unwrap_or_default(),
                            });
                        }
                    }
                }
            }
        }

        // --- vulnerabilidades
        let mut vulns = VulnSection {
            requested: req.scan_vulnerabilities,
            tools: Vec::new(),
            not_analyzed: None,
            images: Vec::new(),
        };
        if !req.scan_vulnerabilities {
            vulns.not_analyzed = Some("audit.vuln.notRequested".into());
        } else if !local {
            vulns.not_analyzed = Some("audit.vuln.remote".into());
        } else {
            vulns.tools = vuln::detect_tools().await;
            let wanted = req.tool.as_deref();
            let tool = vulns
                .tools
                .iter()
                .filter(|t| wanted.map_or(true, |w| w == t.name))
                .find(|t| t.db_ready)
                .cloned();
            match tool {
                None if vulns.tools.is_empty() => {
                    vulns.not_analyzed = Some("audit.vuln.noTool".into())
                }
                None => vulns.not_analyzed = Some("audit.vuln.noDatabase".into()),
                Some(tool) => {
                    let host = format!("unix://{}", self.endpoint().socket_path);
                    let total = to_scan.len() as u32;
                    for (i, image) in to_scan.iter().enumerate() {
                        if cancel.is_cancelled() {
                            return Err(cancelled());
                        }
                        progress(AuditProgress {
                            step: "vulnerabilities".into(),
                            done: i as u32,
                            total,
                            current: Some(image.clone()),
                        });
                        vulns.images.push(
                            vuln::scan_image(self.activity(), &tool, image, &host, cancel).await,
                        );
                        if cancel.is_cancelled() {
                            return Err(cancelled());
                        }
                    }
                }
            }
        }

        findings.sort_by(|a, b| {
            b.level
                .cmp(&a.level)
                .then(a.target.name.cmp(&b.target.name))
                .then(a.code.cmp(&b.code))
        });
        let counts = count_levels(&findings);
        let id = format!(
            "{}-{}",
            now.format("%Y%m%dT%H%M%S"),
            crate::ssh::askpass::random_hex(3)
        );
        Ok(AuditReport {
            id,
            server: server.to_owned(),
            started_at: now.to_rfc3339(),
            finished_at: Utc::now().to_rfc3339(),
            request: req.clone(),
            daemon,
            docker_version: info.server_version,
            targets,
            findings,
            skipped,
            vulnerabilities: vulns,
            counts,
        })
    }
}
