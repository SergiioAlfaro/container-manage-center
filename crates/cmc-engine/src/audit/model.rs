// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Modelo de la auditoría de seguridad (E3, D-093).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::risk::RiskLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "AuditTargetKind")]
pub enum TargetKind {
    Container,
    Service,
    Image,
}

/// Lo que se ha auditado: un contenedor, un servicio de Compose o una imagen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "AuditTarget")]
pub struct TargetRef {
    pub kind: TargetKind,
    /// Id del contenedor o de la imagen; `proyecto/servicio` en Compose.
    pub id: String,
    pub name: String,
    /// Proyecto Compose (servicios, y contenedores creados por Compose).
    pub project: Option<String>,
}

/// Corrección que se puede proponer en «Editar configuración» (con vista
/// previa y confirmación; la auditoría nunca cambia nada).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum AuditFix {
    /// Publicar el puerto solo en 127.0.0.1.
    PortLocalhost {
        container_port: u16,
        protocol: String,
        host_port: String,
    },
    /// Montar la carpeta en solo lectura.
    MountReadOnly { target: String },
    /// Fijar límites de CPU y memoria (los elige el usuario).
    Resources,
}

/// Un hallazgo. `code` se traduce en la interfaz (`issues.<code>` para los
/// riesgos, `audit.check.<code>` para las explicaciones y recomendaciones).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditFinding {
    pub target: TargetRef,
    pub code: String,
    pub level: RiskLevel,
    pub field: String,
    pub key: String,
    pub params: BTreeMap<String, String>,
    pub fix: Option<AuditFix>,
}

/// Seguridad del daemon (`docker info`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DaemonSecurity {
    pub rootless: bool,
    pub userns: bool,
    pub seccomp: bool,
    pub seccomp_profile: Option<String>,
    pub apparmor: bool,
    pub selinux: bool,
    pub cgroup_version: Option<String>,
}

/// Qué auditar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditRequest {
    /// Un solo contenedor (id o nombre); si falta, todos.
    pub container: Option<String>,
    pub include_compose: bool,
    pub include_images: bool,
    /// Buscar vulnerabilidades con Trivy o Grype (solo en este equipo).
    pub scan_vulnerabilities: bool,
    /// Herramienta preferida (`trivy` o `grype`); si falta, la primera disponible.
    pub tool: Option<String>,
}

/// Algo que no se pudo auditar, con el motivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditSkipped {
    pub target: TargetRef,
    /// Código traducible (`audit.skipped.composeUnavailable`…).
    pub reason: String,
    pub detail: Option<String>,
}

/// Resumen de un contenedor, servicio o imagen auditado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditedTarget {
    pub target: TargetRef,
    /// Estado del contenedor (running, exited…), vacío en otros.
    pub state: String,
    pub image: String,
}

// ------------------------------------------------------------ vulnerabilidades

/// Una herramienta de vulnerabilidades encontrada en este equipo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VulnTool {
    /// `trivy` o `grype`.
    pub name: String,
    pub path: String,
    pub version: Option<String>,
    /// Fecha de la base de vulnerabilidades (ISO 8601), si se conoce.
    pub db_updated: Option<String>,
    /// La base existe y la herramienta la puede usar sin conexión.
    pub db_ready: bool,
    /// Si no está lista, por qué.
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VulnCounts {
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub unknown: u32,
}

impl VulnCounts {
    pub fn total(&self) -> u32 {
        self.critical + self.high + self.medium + self.low + self.unknown
    }
    pub fn add(&mut self, severity: &str) {
        match severity.to_ascii_lowercase().as_str() {
            "critical" => self.critical += 1,
            "high" => self.high += 1,
            "medium" => self.medium += 1,
            "low" | "negligible" => self.low += 1,
            _ => self.unknown += 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Vulnerability {
    pub id: String,
    pub package: String,
    pub installed: String,
    pub fixed: Option<String>,
    pub severity: String,
}

/// Resultado del análisis de una imagen. **Nunca** se informa de «sin
/// vulnerabilidades» si el análisis no terminó: entonces es `NotAnalyzed` o
/// `Failed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ImageVulns {
    /// Análisis completo con la base de esa fecha.
    Done {
        image: String,
        tool: String,
        tool_version: Option<String>,
        db_updated: Option<String>,
        counts: VulnCounts,
        /// Las más graves (como mucho 50).
        top: Vec<Vulnerability>,
    },
    /// No se analizó (código traducible y detalle).
    NotAnalyzed { image: String, reason: String },
    /// El análisis empezó y no terminó.
    Failed {
        image: String,
        tool: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VulnSection {
    /// Se pidió el análisis.
    pub requested: bool,
    /// Herramientas encontradas (aunque no se usaran).
    pub tools: Vec<VulnTool>,
    /// Si no se analizó nada: por qué (código traducible).
    pub not_analyzed: Option<String>,
    pub images: Vec<ImageVulns>,
}

// ------------------------------------------------------------------ informe

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditReport {
    pub id: String,
    pub server: String,
    pub started_at: String,
    pub finished_at: String,
    pub request: AuditRequest,
    pub daemon: DaemonSecurity,
    pub docker_version: Option<String>,
    pub targets: Vec<AuditedTarget>,
    pub findings: Vec<AuditFinding>,
    pub skipped: Vec<AuditSkipped>,
    pub vulnerabilities: VulnSection,
    /// Hallazgos por nivel (`critical`, `high`, `medium`, `low`).
    pub counts: BTreeMap<String, u32>,
}

/// Entrada del historial (sin los hallazgos, para listar deprisa).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditSummary {
    pub id: String,
    pub server: String,
    pub started_at: String,
    pub finished_at: String,
    pub container: Option<String>,
    pub targets: u32,
    pub counts: BTreeMap<String, u32>,
    pub vulns_analyzed: bool,
}

impl AuditReport {
    pub fn summary(&self) -> AuditSummary {
        AuditSummary {
            id: self.id.clone(),
            server: self.server.clone(),
            started_at: self.started_at.clone(),
            finished_at: self.finished_at.clone(),
            container: self.request.container.clone(),
            targets: self.targets.len() as u32,
            counts: self.counts.clone(),
            vulns_analyzed: self
                .vulnerabilities
                .images
                .iter()
                .any(|i| matches!(i, ImageVulns::Done { .. })),
        }
    }
}

pub fn count_levels(findings: &[AuditFinding]) -> BTreeMap<String, u32> {
    let mut m = BTreeMap::new();
    for l in [
        RiskLevel::Critical,
        RiskLevel::High,
        RiskLevel::Medium,
        RiskLevel::Low,
    ] {
        m.insert(l.as_str().to_owned(), 0);
    }
    for f in findings {
        *m.entry(f.level.as_str().to_owned()).or_insert(0) += 1;
    }
    m
}

/// Progreso de una auditoría en curso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditProgress {
    /// `containers`, `images`, `compose`, `vulnerabilities`.
    pub step: String,
    pub done: u32,
    pub total: u32,
    pub current: Option<String>,
}
