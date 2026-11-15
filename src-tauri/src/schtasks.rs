// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Tareas de copia programadas en Windows con el Programador de tareas
//! (1.9.0, D-114). Mismas reglas que con systemd (D-100):
//!
//! - Solo se crean, cambian o eliminan tareas con la **autorización expresa**
//!   del usuario, tras mostrarle exactamente qué se crea.
//! - Solo se tocan tareas propias: en la carpeta `\Container Manage Center\`,
//!   con el nombre `cmc-backup-<tarea>` y la marca en su descripción. Una
//!   tarea con ese nombre sin la marca nunca se sobrescribe ni se elimina.
//! - `schtasks.exe` sin shell, con los argumentos exactos registrados en el
//!   panel de actividad. Se ejecutan con el usuario, sin privilegios de
//!   administrador y solo con la sesión iniciada.
//! - Se importan en XML para poder pedir lo mismo que `Persistent=true` de
//!   systemd: si el equipo estaba apagado a la hora prevista, la copia se hace
//!   en cuanto se pueda (`StartWhenAvailable`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use cmc_activity::{ActivityLog, Category, Failure, OperationSpec, Outcome, Transport};

use super::{Systemd, TimerPreview, TimerStatus, MARKER};
use crate::backups::{BackupJob, Schedule};

const FOLDER: &str = r"\Container Manage Center\";
const TIMEOUT: Duration = Duration::from_secs(30);

fn xml_escape(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn trigger(s: &Schedule) -> String {
    const DAYS: [&str; 7] = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let (hour, minute, by) = match *s {
        Schedule::Daily { hour, minute } => (
            hour,
            minute,
            "<ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>".to_owned(),
        ),
        Schedule::Weekly {
            weekday,
            hour,
            minute,
        } => (
            hour,
            minute,
            format!(
                "<ScheduleByWeek><WeeksInterval>1</WeeksInterval><DaysOfWeek><{}/></DaysOfWeek></ScheduleByWeek>",
                DAYS[usize::from(weekday.clamp(1, 7) - 1)]
            ),
        ),
        Schedule::Monthly { day, hour, minute } => (
            hour,
            minute,
            format!(
                "<ScheduleByMonth><DaysOfMonth><Day>{day}</Day></DaysOfMonth><Months>{}</Months></ScheduleByMonth>",
                MONTHS.iter().map(|m| format!("<{m}/>")).collect::<String>()
            ),
        ),
    };
    format!(
        "<CalendarTrigger><StartBoundary>2026-01-01T{hour:02}:{minute:02}:00</StartBoundary><Enabled>true</Enabled>{by}</CalendarTrigger>"
    )
}

impl Systemd {
    /// `schtasks.exe` del sistema. Las tareas no tienen archivos propios:
    /// `unit_dir` es la carpeta temporal donde se deja el XML al importarlo.
    pub fn from_env() -> Option<Self> {
        let program = cmc_engine::sys::find_executable("schtasks")?;
        Some(Self {
            program,
            unit_dir: std::env::temp_dir(),
        })
    }

    pub fn unit_base(job_id: &str) -> String {
        format!("cmc-backup-{job_id}")
    }

    fn task_name(job_id: &str) -> String {
        format!("{FOLDER}{}", Self::unit_base(job_id))
    }

    /// XML de la tarea y la orden que ejecuta.
    pub fn render(job: &BackupJob, exe: &Path) -> (String, String) {
        let desc = format!(
            "{MARKER} {}\nContainer Manage Center: copia «{}». Creada con tu autorización; se gestiona desde la aplicación.",
            job.id, job.name
        );
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n\
<Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n\
  <RegistrationInfo><Description>{}</Description></RegistrationInfo>\n\
  <Triggers>{}</Triggers>\n\
  <Principals><Principal id=\"Author\"><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>\n\
  <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><StartWhenAvailable>true</StartWhenAvailable><Enabled>true</Enabled><Hidden>false</Hidden><ExecutionTimeLimit>PT72H</ExecutionTimeLimit><Priority>7</Priority></Settings>\n\
  <Actions Context=\"Author\"><Exec><Command>{}</Command><Arguments>--run-backup {}</Arguments></Exec></Actions>\n\
</Task>\n",
            xml_escape(&desc),
            trigger(&job.schedule),
            xml_escape(&exe.display().to_string()),
            xml_escape(&job.id),
        );
        let action = format!("\"{}\" --run-backup {}", exe.display(), job.id);
        (xml, action)
    }

    fn xml_path(&self, job_id: &str) -> PathBuf {
        self.unit_dir
            .join(format!("{}.xml", Self::unit_base(job_id)))
    }

    pub fn preview(&self, job: &BackupJob, exe: &Path) -> TimerPreview {
        let (xml, action) = Self::render(job, exe);
        let p = self.program.display();
        let name = Self::task_name(&job.id);
        TimerPreview {
            service_path: name.clone(),
            service_text: action,
            timer_path: self.xml_path(&job.id).display().to_string(),
            timer_text: xml,
            commands: vec![format!(
                "{p} /Create /F /TN \"{name}\" /XML \"{}\"",
                self.xml_path(&job.id).display()
            )],
        }
    }

    async fn schtasks(
        &self,
        log: &ActivityLog,
        args: &[String],
        record: bool,
    ) -> Result<String, String> {
        let handle = record.then(|| {
            log.begin(OperationSpec {
                category: Category::Backup,
                kind: "backup.timer.schtasks".into(),
                target: args
                    .iter()
                    .position(|a| a == "/TN")
                    .and_then(|i| args.get(i + 1))
                    .cloned(),
                transport: Transport::Cli {
                    program: self.program.display().to_string(),
                    args: args.to_vec(),
                },
                request_body: None,
                equivalent_command: None,
            })
        });
        let mut cmd = tokio::process::Command::new(&self.program);
        cmc_engine::sys::no_console(&mut cmd);
        let child = cmd
            .args(args)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output();
        let out = match tokio::time::timeout(TIMEOUT, child).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => {
                let msg = format!("no se pudo ejecutar schtasks: {e}");
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
                return Err("schtasks no respondió".into());
            }
        };
        let stdout = decode(&out.stdout);
        let stderr = decode(&out.stderr);
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
                    error: format!("schtasks terminó con {}", out.status),
                    stderr: Some(stderr.chars().take(8 * 1024).collect()),
                    exit_code: out.status.code(),
                    ..Default::default()
                });
            }
            Err(stderr.trim().to_owned())
        }
    }

    /// XML de la tarea instalada, si existe.
    async fn installed_xml(&self, log: &ActivityLog, job_id: &str) -> Option<String> {
        self.schtasks(
            log,
            &[
                "/Query".into(),
                "/TN".into(),
                Self::task_name(job_id),
                "/XML".into(),
            ],
            false,
        )
        .await
        .ok()
    }

    fn ours(xml: &str, job_id: &str) -> bool {
        xml.contains(&format!("{MARKER} {job_id}"))
    }

    pub async fn status(&self, log: &ActivityLog, job: &BackupJob, exe: &Path) -> TimerStatus {
        let mut st = TimerStatus::default();
        let Some(xml) = self.installed_xml(log, &job.id).await else {
            return st;
        };
        if !Self::ours(&xml, &job.id) {
            st.foreign = true;
            return st;
        }
        st.installed = true;
        // Lo que se compara: el disparador y la orden (Windows reescribe el
        // resto del XML al importarlo).
        let compact = |s: &str| s.split_whitespace().collect::<String>();
        let (want, _) = Self::render(job, exe);
        let x = compact(&xml);
        let trig = compact(&trigger(&job.schedule));
        let cmd = compact(&format!(
            "<Command>{}</Command>",
            xml_escape(&exe.display().to_string())
        ));
        let by = trig
            .split("</Enabled>")
            .nth(1)
            .unwrap_or_default()
            .trim_end_matches("</CalendarTrigger>")
            .to_owned();
        if !x.contains(&by) || !x.contains(&cmd) || want.is_empty() {
            st.outdated = true;
        }
        let enabled = !x.contains("<Settings>") || {
            let settings = x.split("<Settings>").nth(1).unwrap_or_default();
            let settings = settings.split("</Settings>").next().unwrap_or_default();
            !settings.contains("<Enabled>false</Enabled>")
        };
        st.unit_file_state = Some(if enabled { "enabled" } else { "disabled" }.into());
        st
    }

    fn write_xml(&self, path: &Path, xml: &str) -> std::io::Result<()> {
        // El Programador de tareas espera UTF-16 con BOM, como indica la cabecera.
        let mut bytes = vec![0xFF, 0xFE];
        for u in xml.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        std::fs::write(path, bytes)
    }

    /// Crea (o actualiza) la tarea y la deja activada. Solo con autorización
    /// expresa; nunca sobre una tarea ajena.
    pub async fn install(
        &self,
        log: &ActivityLog,
        job: &BackupJob,
        exe: &Path,
    ) -> Result<(), String> {
        if let Some(xml) = self.installed_xml(log, &job.id).await {
            if !Self::ours(&xml, &job.id) {
                return Err(format!(
                    "{} existe y no es de Container Manage Center: no se toca",
                    Self::task_name(&job.id)
                ));
            }
        }
        let (xml, _) = Self::render(job, exe);
        let path = self.xml_path(&job.id);
        let h = log.begin(OperationSpec {
            category: Category::Backup,
            kind: "backup.timer.write".into(),
            target: Some(Self::task_name(&job.id)),
            transport: Transport::App {
                action: format!("Preparar {}", path.display()),
            },
            request_body: Some(xml.clone()),
            equivalent_command: None,
        });
        if let Err(e) = self.write_xml(&path, &xml) {
            h.fail(Failure {
                error: e.to_string(),
                ..Default::default()
            });
            return Err(e.to_string());
        }
        h.succeed(Outcome {
            summary: Some("Definición de la tarea preparada".into()),
            ..Default::default()
        });
        let r = self
            .schtasks(
                log,
                &[
                    "/Create".into(),
                    "/F".into(),
                    "/TN".into(),
                    Self::task_name(&job.id),
                    "/XML".into(),
                    path.display().to_string(),
                ],
                true,
            )
            .await;
        let _ = std::fs::remove_file(&path);
        r.map(|_| ())
    }

    pub async fn set_enabled(
        &self,
        log: &ActivityLog,
        job_id: &str,
        enabled: bool,
    ) -> Result<(), String> {
        match self.installed_xml(log, job_id).await {
            Some(xml) if Self::ours(&xml, job_id) => {}
            _ => return Err("la tarea programada no está creada".into()),
        }
        self.schtasks(
            log,
            &[
                "/Change".into(),
                "/TN".into(),
                Self::task_name(job_id),
                if enabled { "/ENABLE" } else { "/DISABLE" }.into(),
            ],
            true,
        )
        .await
        .map(|_| ())
    }

    /// Elimina la tarea programada propia.
    pub async fn remove(&self, log: &ActivityLog, job_id: &str) -> Result<(), String> {
        match self.installed_xml(log, job_id).await {
            None => return Ok(()),
            Some(xml) if !Self::ours(&xml, job_id) => {
                return Err(
                    "hay una tarea con ese nombre que no es de Container Manage Center: no se toca"
                        .into(),
                )
            }
            Some(_) => {}
        }
        self.schtasks(
            log,
            &[
                "/Delete".into(),
                "/F".into(),
                "/TN".into(),
                Self::task_name(job_id),
            ],
            true,
        )
        .await
        .map(|_| ())
    }

    /// En Windows no hubo versiones con el nombre anterior.
    pub async fn migrate_legacy(
        &self,
        _log: &ActivityLog,
        _jobs: &[BackupJob],
        _exe: &Path,
    ) -> Vec<String> {
        Vec::new()
    }
}

/// Salida de `schtasks`: UTF-8 si lo es; si no, la página de códigos OEM
/// (se muestra tal cual, con los caracteres no ASCII sustituidos).
fn decode(b: &[u8]) -> String {
    String::from_utf8(b.to_vec()).unwrap_or_else(|_| {
        b.iter()
            .map(|&c| if c.is_ascii() { c as char } else { '?' })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_xml_has_the_marker_the_schedule_and_the_exact_command() {
        let mut j = BackupJob::new_for_tests("job-0a1b2c3d");
        j.name = "Diaria <x> & \"y\"".into();
        j.schedule = Schedule::Weekly {
            weekday: 7,
            hour: 4,
            minute: 5,
        };
        let exe =
            Path::new(r"C:\Program Files\Container Manage Center\container-manage-center.exe");
        let (xml, action) = Systemd::render(&j, exe);
        assert!(xml.contains("# cmc-managed-backup-job: job-0a1b2c3d"));
        assert!(xml.contains("Diaria &lt;x&gt; &amp; &quot;y&quot;"));
        assert!(xml.contains("<StartBoundary>2026-01-01T04:05:00</StartBoundary>"));
        assert!(xml.contains("<DaysOfWeek><Sunday/></DaysOfWeek>"));
        assert!(xml.contains("<StartWhenAvailable>true</StartWhenAvailable>"));
        assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
        assert!(xml.contains("<Arguments>--run-backup job-0a1b2c3d</Arguments>"));
        assert_eq!(
            action,
            r#""C:\Program Files\Container Manage Center\container-manage-center.exe" --run-backup job-0a1b2c3d"#
        );
    }
}
