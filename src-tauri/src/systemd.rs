// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Temporizadores de usuario de systemd para las copias programadas (E4,
//! D-100, decisión 2 del propietario).
//!
//! - Solo se crean, cambian o eliminan archivos de systemd con la
//!   **autorización expresa** del usuario, tras mostrarle su contenido exacto.
//! - Solo se tocan archivos propios: `cmc-backup-<tarea>.service/.timer` en
//!   `~/.config/systemd/user/` con la marca [`MARKER`]. Un archivo con ese
//!   nombre sin la marca nunca se sobrescribe ni se elimina.
//! - `systemctl --user` sin shell, con los argumentos exactos registrados en el
//!   panel de actividad. Nunca `sudo` ni unidades del sistema.

use std::path::{Path, PathBuf};
use std::time::Duration;

use cmc_activity::{ActivityLog, Category, Failure, OperationSpec, Outcome, Transport};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::backups::{BackupJob, Schedule};

/// Marca que identifica los archivos creados por la aplicación.
pub const MARKER: &str = "# cmc-managed-backup-job:";
const TIMEOUT: Duration = Duration::from_secs(30);

/// Estado del temporizador de una tarea.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "BackupTimerStatus")]
pub struct TimerStatus {
    /// Los dos archivos existen y son de la aplicación.
    pub installed: bool,
    /// Hay un archivo con ese nombre que no es de la aplicación: no se toca.
    pub foreign: bool,
    /// El contenido ya no coincide con la tarea (cambió la programación).
    pub outdated: bool,
    /// `enabled`, `disabled`… (`UnitFileState`).
    pub unit_file_state: Option<String>,
    /// `active`, `inactive`… (`ActiveState`).
    pub active_state: Option<String>,
    pub next_run: Option<String>,
    pub last_run: Option<String>,
    /// No se pudo consultar systemd (no está, o no hay sesión de usuario).
    pub problem: Option<String>,
}

/// Lo que se va a crear, para mostrarlo antes de autorizarlo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "BackupTimerPreview")]
pub struct TimerPreview {
    pub service_path: String,
    pub service_text: String,
    pub timer_path: String,
    pub timer_text: String,
    /// Órdenes que se ejecutarán.
    pub commands: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Systemd {
    pub program: PathBuf,
    pub unit_dir: PathBuf,
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// `[%$\"]` escapados para una línea de unidad de systemd.
fn unit_escape(s: &str) -> String {
    s.replace('%', "%%")
}

fn exec_quote(s: &str) -> String {
    let e = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
        .replace('$', "$$");
    format!("\"{e}\"")
}

/// Texto de una línea de descripción: sin saltos ni caracteres de control.
fn one_line(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(120)
        .collect();
    unit_escape(t.trim())
}

pub fn on_calendar(s: &Schedule) -> String {
    match *s {
        Schedule::Daily { hour, minute } => format!("*-*-* {hour:02}:{minute:02}:00"),
        Schedule::Weekly {
            weekday,
            hour,
            minute,
        } => {
            let d = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
                [usize::from(weekday.clamp(1, 7) - 1)];
            format!("{d} *-*-* {hour:02}:{minute:02}:00")
        }
        Schedule::Monthly { day, hour, minute } => format!("*-*-{day:02} {hour:02}:{minute:02}:00"),
    }
}

impl Systemd {
    /// systemd del usuario: `systemctl` del `PATH` y `$XDG_CONFIG_HOME/systemd/user`.
    /// En las compilaciones de desarrollo `CMC_SYSTEMCTL` sustituye el
    /// programa (pruebas con un `systemctl` simulado).
    pub fn from_env() -> Option<Self> {
        let program = if cfg!(debug_assertions) {
            std::env::var_os("CMC_SYSTEMCTL")
                .map(PathBuf::from)
                .or_else(|| find_in_path("systemctl"))
        } else {
            find_in_path("systemctl")
        }?;
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| dirs::home_dir().map(|h| h.join(".config")))?;
        Some(Self {
            program,
            unit_dir: config.join("systemd/user"),
        })
    }

    pub fn unit_base(job_id: &str) -> String {
        format!("cmc-backup-{job_id}")
    }

    fn paths(&self, job_id: &str) -> (PathBuf, PathBuf) {
        let b = Self::unit_base(job_id);
        (
            self.unit_dir.join(format!("{b}.service")),
            self.unit_dir.join(format!("{b}.timer")),
        )
    }

    /// Contenido de las dos unidades.
    pub fn render(job: &BackupJob, exe: &Path) -> (String, String) {
        let desc = one_line(&format!("Container Manage Center: copia «{}»", job.name));
        let head = format!(
            "{MARKER} {}\n# Creado por Container Manage Center con tu autorización. Se gestiona desde la aplicación.\n",
            job.id
        );
        let service = format!(
            "{head}[Unit]\nDescription={desc}\n\n[Service]\nType=oneshot\nExecStart={} --run-backup {}\nNice=10\nIOSchedulingClass=idle\n",
            exec_quote(&exe.display().to_string()),
            job.id
        );
        let timer = format!(
            "{head}[Unit]\nDescription={desc}\n\n[Timer]\nOnCalendar={}\nPersistent=true\nUnit={}.service\n\n[Install]\nWantedBy=timers.target\n",
            on_calendar(&job.schedule),
            Self::unit_base(&job.id)
        );
        (service, timer)
    }

    fn timer_unit(job_id: &str) -> String {
        format!("{}.timer", Self::unit_base(job_id))
    }

    pub fn preview(&self, job: &BackupJob, exe: &Path) -> TimerPreview {
        let (s, t) = Self::render(job, exe);
        let (sp, tp) = self.paths(&job.id);
        let p = self.program.display();
        TimerPreview {
            service_path: sp.display().to_string(),
            service_text: s,
            timer_path: tp.display().to_string(),
            timer_text: t,
            commands: vec![
                format!("{p} --user daemon-reload"),
                format!("{p} --user enable --now {}", Self::timer_unit(&job.id)),
            ],
        }
    }

    /// ¿Es nuestro este archivo, y de esta tarea?
    fn ours(path: &Path, job_id: &str) -> Option<bool> {
        let text = std::fs::read_to_string(path).ok()?;
        Some(text.lines().next() == Some(format!("{MARKER} {job_id}").as_str()))
    }

    async fn systemctl(
        &self,
        log: &ActivityLog,
        args: &[&str],
        record: bool,
    ) -> Result<String, String> {
        let mut full = vec!["--user".to_owned()];
        full.extend(args.iter().map(|s| s.to_string()));
        let handle = record.then(|| {
            log.begin(OperationSpec {
                category: Category::Backup,
                kind: "backup.timer.systemctl".into(),
                target: args.last().map(|s| s.to_string()),
                transport: Transport::Cli {
                    program: self.program.display().to_string(),
                    args: full.clone(),
                },
                request_body: None,
                equivalent_command: None,
            })
        });
        let child = tokio::process::Command::new(&self.program)
            .args(&full)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output();
        let out = match tokio::time::timeout(TIMEOUT, child).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => {
                let msg = format!("no se pudo ejecutar systemctl: {e}");
                if let Some(h) = handle {
                    h.fail(Failure {
                        error: msg.clone(),
                        ..Default::default()
                    });
                }
                return Err(msg);
            }
            Err(_) => {
                if let Some(h) = handle {
                    h.fail(Failure {
                        error: "tiempo agotado".into(),
                        ..Default::default()
                    });
                }
                return Err("systemctl no respondió".into());
            }
        };
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if out.status.success() {
            if let Some(h) = handle {
                h.succeed(Outcome {
                    summary: Some("Correcto".into()),
                    ..Default::default()
                });
            }
            Ok(stdout)
        } else {
            if let Some(h) = handle {
                h.fail(Failure {
                    error: format!("systemctl terminó con {}", out.status),
                    stderr: Some(stderr.chars().take(8 * 1024).collect()),
                    exit_code: out.status.code(),
                    ..Default::default()
                });
            }
            Err(stderr.trim().to_owned())
        }
    }

    pub async fn status(&self, log: &ActivityLog, job: &BackupJob, exe: &Path) -> TimerStatus {
        let (sp, tp) = self.paths(&job.id);
        let mut st = TimerStatus::default();
        match (Self::ours(&sp, &job.id), Self::ours(&tp, &job.id)) {
            (Some(true), Some(true)) => st.installed = true,
            (None, None) => return st,
            (Some(false), _) | (_, Some(false)) => {
                st.foreign = true;
                return st;
            }
            // Falta uno de los dos: instalación incompleta.
            _ => {
                st.installed = true;
                st.outdated = true;
            }
        }
        let (s, t) = Self::render(job, exe);
        if std::fs::read_to_string(&sp).ok().as_deref() != Some(s.as_str())
            || std::fs::read_to_string(&tp).ok().as_deref() != Some(t.as_str())
        {
            st.outdated = true;
        }
        let unit = Self::timer_unit(&job.id);
        match self
            .systemctl(
                log,
                &[
                    "show",
                    "--property=ActiveState,UnitFileState,NextElapseUSecRealtime,LastTriggerUSec",
                    "--no-pager",
                    &unit,
                ],
                false,
            )
            .await
        {
            Ok(text) => {
                for line in text.lines() {
                    let Some((k, v)) = line.split_once('=') else {
                        continue;
                    };
                    let v = v.trim();
                    let val = (!v.is_empty() && v != "n/a").then(|| v.to_owned());
                    match k {
                        "ActiveState" => st.active_state = val,
                        "UnitFileState" => st.unit_file_state = val,
                        "NextElapseUSecRealtime" => st.next_run = val,
                        "LastTriggerUSec" => st.last_run = val,
                        _ => {}
                    }
                }
            }
            Err(e) => st.problem = Some(e),
        }
        st
    }

    fn write_unit(path: &Path, text: &str) -> std::io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let tmp = path.with_extension("cmc-tmp");
        let _ = std::fs::remove_file(&tmp);
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    }

    /// Crea (o actualiza) las unidades y activa el temporizador. Solo con
    /// autorización expresa; nunca sobre archivos ajenos.
    pub async fn install(
        &self,
        log: &ActivityLog,
        job: &BackupJob,
        exe: &Path,
    ) -> Result<(), String> {
        let (sp, tp) = self.paths(&job.id);
        for p in [&sp, &tp] {
            if Self::ours(p, &job.id) == Some(false) {
                return Err(format!(
                    "{} existe y no es de Container Manage Center: no se toca",
                    p.display()
                ));
            }
        }
        std::fs::create_dir_all(&self.unit_dir)
            .map_err(|e| format!("{}: {e}", self.unit_dir.display()))?;
        let (s, t) = Self::render(job, exe);
        let h = log.begin(OperationSpec {
            category: Category::Backup,
            kind: "backup.timer.write".into(),
            target: Some(Self::timer_unit(&job.id)),
            transport: Transport::App {
                action: format!("Crear {} y {}", sp.display(), tp.display()),
            },
            request_body: Some(format!("{s}\n{t}")),
            equivalent_command: None,
        });
        if let Err(e) = Self::write_unit(&sp, &s).and_then(|()| Self::write_unit(&tp, &t)) {
            h.fail(Failure {
                error: e.to_string(),
                ..Default::default()
            });
            return Err(e.to_string());
        }
        h.succeed(Outcome {
            summary: Some("Unidades escritas".into()),
            ..Default::default()
        });
        self.systemctl(log, &["daemon-reload"], true).await?;
        self.systemctl(log, &["enable", "--now", &Self::timer_unit(&job.id)], true)
            .await?;
        Ok(())
    }

    pub async fn set_enabled(
        &self,
        log: &ActivityLog,
        job_id: &str,
        enabled: bool,
    ) -> Result<(), String> {
        let (_, tp) = self.paths(job_id);
        if Self::ours(&tp, job_id) != Some(true) {
            return Err("el temporizador no está creado".into());
        }
        let verb = if enabled { "enable" } else { "disable" };
        self.systemctl(log, &[verb, "--now", &Self::timer_unit(job_id)], true)
            .await
            .map(|_| ())
    }

    /// Pasa al nombre nuevo las unidades creadas antes de la 1.8.0
    /// (`dcc-backup-<tarea>`, con la marca y la ruta del programa antiguos,
    /// D-113). Solo toca archivos con la marca antigua de la propia tarea.
    /// - Tarea existente: se crean las unidades nuevas (que ejecutan `exe`) y,
    ///   si el temporizador antiguo estaba activado, se activa el nuevo; solo
    ///   después se desactiva y se borra el antiguo. Así no se pierde ninguna
    ///   programación.
    /// - Tarea que ya no existe: la unidad antigua no podría hacer nada; se
    ///   desactiva y se borra.
    ///
    /// Devuelve los problemas (vacío si todo fue bien). Las tareas con
    /// problemas conservan sus unidades antiguas.
    pub async fn migrate_legacy(
        &self,
        log: &ActivityLog,
        jobs: &[BackupJob],
        exe: &Path,
    ) -> Vec<String> {
        use cmc_store::legacy;
        let mut ids = std::collections::BTreeSet::new();
        if let Ok(rd) = std::fs::read_dir(&self.unit_dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let id = name.strip_prefix(legacy::UNIT_PREFIX).and_then(|r| {
                    r.strip_suffix(".service")
                        .or_else(|| r.strip_suffix(".timer"))
                });
                if let Some(id) = id {
                    ids.insert(id.to_owned());
                }
            }
        }
        let mut problems = Vec::new();
        for id in ids {
            let base = format!("{}{id}", legacy::UNIT_PREFIX);
            let (osp, otp) = (
                self.unit_dir.join(format!("{base}.service")),
                self.unit_dir.join(format!("{base}.timer")),
            );
            let marker = format!("{} {id}", legacy::UNIT_MARKER);
            let ours = |p: &Path| {
                std::fs::read_to_string(p)
                    .ok()
                    .map(|t| t.lines().next() == Some(marker.as_str()))
            };
            if [&osp, &otp].iter().any(|p| ours(p) == Some(false)) {
                // Un archivo con ese nombre que no es de la aplicación: no se toca.
                continue;
            }
            let old_timer = format!("{base}.timer");
            let was_enabled = std::fs::symlink_metadata(
                self.unit_dir.join("timers.target.wants").join(&old_timer),
            )
            .is_ok();
            if let Some(job) = jobs.iter().find(|j| j.id == id) {
                if let Err(e) = self.install(log, job, exe).await {
                    problems.push(format!("tarea {id}: {e}"));
                    continue;
                }
                if !was_enabled {
                    if let Err(e) = self.set_enabled(log, &id, false).await {
                        problems.push(format!("tarea {id}: {e}"));
                    }
                }
            }
            let _ = self
                .systemctl(log, &["disable", "--now", &old_timer], true)
                .await;
            let h = log.begin(OperationSpec {
                category: Category::Backup,
                kind: "backup.timer.delete".into(),
                target: Some(old_timer.clone()),
                transport: Transport::App {
                    action: format!(
                        "Eliminar {} y {} (nombre anterior)",
                        osp.display(),
                        otp.display()
                    ),
                },
                request_body: None,
                equivalent_command: None,
            });
            let removed = [&osp, &otp]
                .iter()
                .filter(|p| p.exists())
                .try_for_each(std::fs::remove_file);
            match removed {
                Ok(()) => h.succeed(Outcome {
                    summary: Some("Unidades del nombre anterior eliminadas".into()),
                    ..Default::default()
                }),
                Err(e) => {
                    h.fail(Failure {
                        error: e.to_string(),
                        ..Default::default()
                    });
                    problems.push(format!("tarea {id}: {e}"));
                }
            }
            let _ = self.systemctl(log, &["daemon-reload"], true).await;
        }
        problems
    }

    /// Desactiva y elimina las unidades propias de la tarea.
    pub async fn remove(&self, log: &ActivityLog, job_id: &str) -> Result<(), String> {
        let (sp, tp) = self.paths(job_id);
        let mine: Vec<&PathBuf> = [&sp, &tp]
            .into_iter()
            .filter(|p| Self::ours(p, job_id) == Some(true))
            .collect();
        if [&sp, &tp]
            .iter()
            .any(|p| Self::ours(p, job_id) == Some(false))
        {
            return Err(
                "hay un archivo con ese nombre que no es de Container Manage Center: no se toca"
                    .into(),
            );
        }
        if mine.is_empty() {
            return Ok(());
        }
        // Si systemd no lo conoce, se sigue: lo importante es quitar los archivos.
        let _ = self
            .systemctl(log, &["disable", "--now", &Self::timer_unit(job_id)], true)
            .await;
        let h = log.begin(OperationSpec {
            category: Category::Backup,
            kind: "backup.timer.delete".into(),
            target: Some(Self::timer_unit(job_id)),
            transport: Transport::App {
                action: format!(
                    "Eliminar {}",
                    mine.iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(" y ")
                ),
            },
            request_body: None,
            equivalent_command: None,
        });
        for p in &mine {
            if let Err(e) = std::fs::remove_file(p) {
                h.fail(Failure {
                    error: e.to_string(),
                    ..Default::default()
                });
                return Err(e.to_string());
            }
        }
        h.succeed(Outcome {
            summary: Some("Unidades eliminadas".into()),
            ..Default::default()
        });
        self.systemctl(log, &["daemon-reload"], true)
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backups::BackupJob;
    use std::os::unix::fs::PermissionsExt;

    /// `systemctl` simulado: registra sus argumentos y responde a `show`.
    fn fake(dir: &Path) -> PathBuf {
        let p = dir.join("systemctl");
        std::fs::write(
            &p,
            format!(
                "#!/bin/sh\necho \"$*\" >> {}/calls\ncase \"$2\" in show) printf 'ActiveState=active\\nUnitFileState=enabled\\nNextElapseUSecRealtime=Sat 2026-11-15 03:15:00 CET\\nLastTriggerUSec=n/a\\n';; esac\n",
                dir.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn job() -> BackupJob {
        let mut j = BackupJob::new_for_tests("job-0a1b2c3d");
        j.name = "Diaria %h \"x\"\nlínea".into();
        j.schedule = Schedule::Daily {
            hour: 3,
            minute: 15,
        };
        j
    }

    #[tokio::test]
    async fn timers_are_created_listed_disabled_and_removed_only_when_ours() {
        let d = tempfile::Builder::new()
            .prefix("cmc-test")
            .tempdir()
            .unwrap();
        let sd = Systemd {
            program: fake(d.path()),
            unit_dir: d.path().join("user"),
        };
        let log = ActivityLog::new(100);
        let j = job();
        let exe = Path::new("/usr/bin/container manage center");
        // Sin crear: nada instalado y ninguna orden ejecutada.
        let st = sd.status(&log, &j, exe).await;
        assert!(!st.installed && !st.foreign);
        assert!(!d.path().join("calls").exists());
        let pv = sd.preview(&j, exe);
        assert!(pv
            .service_text
            .starts_with(&format!("{MARKER} job-0a1b2c3d\n")));
        assert!(pv.service_text.contains(
            "ExecStart=\"/usr/bin/container manage center\" --run-backup job-0a1b2c3d\n"
        ));
        assert!(pv
            .service_text
            .contains("Description=Container Manage Center: copia «Diaria %%h \"x\" línea»"));
        assert!(pv
            .timer_text
            .contains("OnCalendar=*-*-* 03:15:00\nPersistent=true\n"));
        sd.install(&log, &j, exe).await.unwrap();
        let (sp, tp) = sd.paths(&j.id);
        assert_eq!(std::fs::read_to_string(&sp).unwrap(), pv.service_text);
        assert_eq!(
            std::fs::metadata(&tp).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let st = sd.status(&log, &j, exe).await;
        assert!(st.installed && !st.outdated, "{st:?}");
        assert_eq!(st.unit_file_state.as_deref(), Some("enabled"));
        assert_eq!(st.next_run.as_deref(), Some("Sat 2026-11-15 03:15:00 CET"));
        assert_eq!(st.last_run, None);
        // Cambia la programación: el temporizador queda desactualizado.
        let mut j2 = j.clone();
        j2.schedule = Schedule::Weekly {
            weekday: 7,
            hour: 4,
            minute: 0,
        };
        assert!(sd.status(&log, &j2, exe).await.outdated);
        assert!(Systemd::render(&j2, exe)
            .1
            .contains("OnCalendar=Sun *-*-* 04:00:00"));
        sd.set_enabled(&log, &j.id, false).await.unwrap();
        sd.remove(&log, &j.id).await.unwrap();
        assert!(!sp.exists() && !tp.exists());
        let calls = std::fs::read_to_string(d.path().join("calls")).unwrap();
        let expected = [
            "--user daemon-reload",
            "--user enable --now cmc-backup-job-0a1b2c3d.timer",
            "--user disable --now cmc-backup-job-0a1b2c3d.timer",
            "--user disable --now cmc-backup-job-0a1b2c3d.timer",
            "--user daemon-reload",
        ];
        let got: Vec<&str> = calls.lines().filter(|l| !l.contains(" show ")).collect();
        assert_eq!(got, expected);
        // Cada orden queda registrada con sus argumentos exactos.
        assert!(log
            .snapshot()
            .iter()
            .any(|r| r.kind == "backup.timer.systemctl"));

        // Un archivo ajeno con el mismo nombre nunca se toca.
        std::fs::write(&tp, "[Timer]\nOnCalendar=daily\n").unwrap();
        let st = sd.status(&log, &j, exe).await;
        assert!(st.foreign && !st.installed);
        assert!(sd.install(&log, &j, exe).await.is_err());
        assert!(sd.remove(&log, &j.id).await.is_err());
        assert_eq!(
            std::fs::read_to_string(&tp).unwrap(),
            "[Timer]\nOnCalendar=daily\n"
        );
        assert!(!sp.exists());
    }

    #[tokio::test]
    async fn units_from_before_1_8_0_are_moved_to_the_new_name_keeping_their_state() {
        let d = tempfile::Builder::new()
            .prefix("cmc-test")
            .tempdir()
            .unwrap();
        let ud = d.path().join("user");
        std::fs::create_dir_all(ud.join("timers.target.wants")).unwrap();
        let sd = Systemd {
            program: fake(d.path()),
            unit_dir: ud.clone(),
        };
        let log = ActivityLog::new(100);
        let mut off = BackupJob::new_for_tests("job-desactivada");
        off.schedule = Schedule::Daily { hour: 1, minute: 0 };
        let on = job();
        let legacy_unit = |id: &str, ext: &str| {
            let p = ud.join(format!("dcc-backup-{id}.{ext}"));
            std::fs::write(
                &p,
                format!("# dcc-managed-backup-job: {id}\n[Service]\nExecStart=/usr/bin/docker-control-center --run-backup {id}\n"),
            )
            .unwrap();
            p
        };
        let mut old = Vec::new();
        for id in ["job-0a1b2c3d", "job-desactivada", "job-borrada"] {
            old.push(legacy_unit(id, "service"));
            old.push(legacy_unit(id, "timer"));
        }
        // Activadas: la primera y la de una tarea que ya no existe.
        for id in ["job-0a1b2c3d", "job-borrada"] {
            std::os::unix::fs::symlink(
                ud.join(format!("dcc-backup-{id}.timer")),
                ud.join(format!("timers.target.wants/dcc-backup-{id}.timer")),
            )
            .unwrap();
        }
        // Un archivo ajeno con el prefijo antiguo no se toca.
        std::fs::write(ud.join("dcc-backup-ajena.timer"), "[Timer]\n").unwrap();
        let exe = Path::new("/usr/bin/container-manage-center");
        let problems = sd
            .migrate_legacy(&log, &[on.clone(), off.clone()], exe)
            .await;
        assert!(problems.is_empty(), "{problems:?}");
        for p in &old {
            assert!(!p.exists(), "{} sigue", p.display());
        }
        assert!(ud.join("dcc-backup-ajena.timer").exists());
        for j in [&on, &off] {
            let (sp, tp) = sd.paths(&j.id);
            let (s, t) = Systemd::render(j, exe);
            assert_eq!(std::fs::read_to_string(sp).unwrap(), s);
            assert_eq!(std::fs::read_to_string(tp).unwrap(), t);
        }
        assert!(Systemd::render(&on, exe).0.contains(
            "ExecStart=\"/usr/bin/container-manage-center\" --run-backup job-0a1b2c3d\n"
        ));
        let calls = std::fs::read_to_string(d.path().join("calls")).unwrap();
        let got: Vec<&str> = calls.lines().collect();
        let pos = |s: &str| {
            got.iter()
                .position(|l| *l == s)
                .unwrap_or_else(|| panic!("falta {s}: {got:?}"))
        };
        // El nuevo se activa antes de desactivar el antiguo.
        assert!(
            pos("--user enable --now cmc-backup-job-0a1b2c3d.timer")
                < pos("--user disable --now dcc-backup-job-0a1b2c3d.timer")
        );
        // La desactivada sigue desactivada con el nombre nuevo.
        assert!(got.contains(&"--user disable --now cmc-backup-job-desactivada.timer"));
        // La de una tarea borrada solo se quita.
        assert!(got.contains(&"--user disable --now dcc-backup-job-borrada.timer"));
        assert!(!got.iter().any(|l| l.contains("cmc-backup-job-borrada")));
        // Repetirla no hace nada.
        std::fs::remove_file(d.path().join("calls")).unwrap();
        assert!(sd.migrate_legacy(&log, &[on, off], exe).await.is_empty());
        assert!(!d.path().join("calls").exists());
    }

    #[test]
    fn calendar_and_quoting() {
        assert_eq!(
            on_calendar(&Schedule::Monthly {
                day: 5,
                hour: 23,
                minute: 59
            }),
            "*-*-05 23:59:00"
        );
        assert_eq!(
            on_calendar(&Schedule::Weekly {
                weekday: 1,
                hour: 0,
                minute: 5
            }),
            "Mon *-*-* 00:05:00"
        );
        assert_eq!(exec_quote("/a b/$x%\"\\"), "\"/a b/$$x%%\\\"\\\\\"");
    }
}
