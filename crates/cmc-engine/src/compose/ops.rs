// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Operaciones de Compose sobre un proyecto, ejecutando el CLI oficial sin
//! shell (D-005, D-053). Cada ejecución es una operación del panel de
//! actividad con el programa y los argumentos exactos.

use std::path::PathBuf;
use std::time::Duration;

use cmc_activity::Category;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::project::{
    action_args, action_args_with, config_args, logs_args_with, parse_model, projects_from_with,
    valid_service_name, ComposeAction, ComposeCli, ComposeProject, ProjectModel, ProjectRef,
};
use crate::cli::{run_streaming, CliRequest, OutputLine, StreamOptions, StreamedRun};
use crate::client::Engine;
use crate::error::EngineError;

/// Límite de una acción (descargas y construcciones pueden ser largas).
const ACTION_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const CONFIG_TIMEOUT: Duration = Duration::from_secs(60);
const LOGS_TIMEOUT: Duration = Duration::from_secs(7 * 24 * 60 * 60);
pub const MAX_LOG_TAIL: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComposeConfig {
    /// Modelo de servicios si la configuración es válida.
    pub model: Option<ProjectModel>,
    /// Configuración resuelta (JSON) tal como la devuelve Compose. Puede incluir
    /// valores de variables: no se guarda en el registro de actividad.
    pub resolved: Option<String>,
    /// Mensaje de Compose si no es válida.
    pub error: Option<String>,
    /// Avisos de Compose aunque sea válida (p. ej. `version` obsoleto).
    pub warnings: Vec<String>,
    /// Comando ejecutado (para mostrarlo).
    pub command: String,
}

/// Directorio vacío en el que se ejecuta Compose, para que nunca cargue por
/// accidente un `compose.yaml` del directorio actual de la aplicación.
fn neutral_dir() -> Result<PathBuf, EngineError> {
    use crate::fsx::DirBuilderExt;
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("cmc-compose-{}", crate::sys::uid()));
    match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => {
            return Err(EngineError::Other {
                message: format!("{}: {e}", dir.display()),
            })
        }
    }
    let meta = std::fs::symlink_metadata(&dir).map_err(|e| EngineError::Other {
        message: format!("{}: {e}", dir.display()),
    })?;
    let empty = std::fs::read_dir(&dir)
        .map(|mut r| r.next().is_none())
        .unwrap_or(false);
    #[cfg(unix)]
    let own = {
        use std::os::unix::fs::MetadataExt;
        meta.uid() == crate::sys::uid()
    };
    // Windows: la carpeta temporal está en el perfil del usuario.
    #[cfg(windows)]
    let own = true;
    if !meta.is_dir() || !own || !empty {
        return Err(EngineError::Other {
            message: format!(
                "{} no es una carpeta vacía propia; no se ejecuta Compose desde ella",
                dir.display()
            ),
        });
    }
    Ok(dir)
}

fn summary_for(action: &ComposeAction, project: &str) -> String {
    match action {
        ComposeAction::Up { .. } => format!("Proyecto {project} desplegado"),
        ComposeAction::Down { .. } => format!("Proyecto {project} eliminado"),
        ComposeAction::Start { .. } => format!("Proyecto {project} iniciado"),
        ComposeAction::Stop { .. } => format!("Proyecto {project} detenido"),
        ComposeAction::Restart { .. } => format!("Proyecto {project} reiniciado"),
        ComposeAction::Pull { .. } => format!("Imágenes de {project} descargadas"),
        ComposeAction::Build { .. } => format!("Imágenes de {project} construidas"),
    }
}

impl Engine {
    /// Orden de Compose: local (con `DOCKER_HOST` del motor) o, en un servidor
    /// remoto, ejecutada allí por la sesión SSH (D-080). `tty`: solo para
    /// órdenes largas sin entrada estándar (acciones y registros).
    fn compose_request(
        &self,
        cli: &ComposeCli,
        kind: String,
        target: &str,
        args: Vec<String>,
        timeout: Duration,
        tty: bool,
    ) -> Result<CliRequest, EngineError> {
        if let Some(session) = self.ssh_session().cloned() {
            return Ok(CliRequest {
                category: Category::Compose,
                kind,
                target: Some(target.to_owned()),
                program: cli.program.clone(),
                args,
                env: super::remote_env(&session.profile().docker),
                cwd: None,
                timeout,
                remote: Some(crate::cli::RemoteExec { session, tty }),
            });
        }
        Ok(CliRequest {
            category: Category::Compose,
            kind,
            target: Some(target.to_owned()),
            program: cli.program.clone(),
            args,
            env: vec![("DOCKER_HOST".into(), self.endpoint().host_uri())],
            cwd: Some(neutral_dir()?),
            timeout,
            remote: None,
        })
    }

    /// Archivos que no existen (o no son archivos normales) **donde está el
    /// proyecto**: en este equipo o, con un servidor remoto, en el servidor
    /// (0.10.0: nunca se supone que un archivo local existe allí). En remoto
    /// es una sola orden `stat` de solo lectura, registrada en actividad.
    pub async fn files_missing(&self, files: &[String]) -> Result<Vec<String>, EngineError> {
        if files.is_empty() {
            return Ok(vec![]);
        }
        let Some(session) = self.ssh_session().cloned() else {
            return Ok(files
                .iter()
                .filter(|f| !std::path::Path::new(f).is_file())
                .cloned()
                .collect());
        };
        let mut args: Vec<String> = vec![
            "LC_ALL=C".into(),
            "stat".into(),
            "-L".into(),
            "--printf".into(),
            "%F|%n\\0".into(),
            "--".into(),
        ];
        args.extend(files.iter().cloned());
        let out = crate::cli::run(
            self.activity(),
            CliRequest {
                category: Category::Compose,
                kind: "compose.files".into(),
                target: files.first().cloned(),
                program: "env".into(),
                args,
                env: vec![],
                cwd: None,
                timeout: Duration::from_secs(30),
                remote: Some(crate::cli::RemoteExec {
                    session,
                    tty: false,
                }),
            },
        )
        .await?;
        // Código 1 si falta alguno: la salida sigue listando los que existen.
        if !matches!(out.exit_code, Some(0) | Some(1)) {
            return Err(EngineError::Process {
                message: format!(
                    "no se pudieron comprobar los archivos en el servidor: {}",
                    out.stderr.trim()
                ),
            });
        }
        let present: std::collections::HashSet<&str> = out
            .stdout
            .split('\0')
            .filter_map(|e| e.split_once('|'))
            .filter(|(kind, _)| *kind == "regular file" || *kind == "regular empty file")
            .map(|(_, name)| name)
            .collect();
        Ok(files
            .iter()
            .filter(|f| !present.contains(f.as_str()))
            .cloned()
            .collect())
    }

    /// Proyectos: los que tienen contenedores (por sus etiquetas, con la lista
    /// real de contenedores) más los registrados por el usuario.
    pub async fn compose_projects(
        &self,
        registered: &[ProjectRef],
    ) -> Result<Vec<ComposeProject>, EngineError> {
        let containers = self.list_containers().await?;
        let reg = registered.to_vec();
        let mut projects = tokio::task::spawn_blocking(move || {
            projects_from_with(&containers, &reg, |_| Vec::new())
        })
        .await
        .map_err(|e| EngineError::Other {
            message: e.to_string(),
        })?;
        // Archivos de todos los proyectos (añadidos y desplegados, cuyos
        // archivos vienen de las etiquetas), comprobados de una vez donde están.
        let mut all: Vec<String> = projects
            .iter()
            .flat_map(|p| p.reference.files.iter().cloned())
            .collect();
        all.sort();
        all.dedup();
        let missing = self.files_missing(&all).await?;
        for p in &mut projects {
            p.missing_files = p
                .reference
                .files
                .iter()
                .filter(|f| missing.contains(f))
                .cloned()
                .collect();
        }
        Ok(projects)
    }

    /// `docker compose config --format json` con todos los perfiles. Con
    /// `unsaved` se valida el contenido de un archivo sin guardarlo (por la
    /// entrada estándar, en la posición de ese archivo).
    pub async fn compose_config(
        &self,
        cli: &ComposeCli,
        project: &ProjectRef,
        unsaved: Option<(usize, String)>,
    ) -> Result<ComposeConfig, EngineError> {
        project.validate()?;
        if project.files.is_empty() {
            return Err(EngineError::InvalidInput {
                message: "el proyecto no tiene archivos Compose".into(),
            });
        }
        let stdin_index = unsaved.as_ref().map(|(i, _)| *i);
        if let Some(i) = stdin_index {
            if i >= project.files.len() {
                return Err(EngineError::InvalidInput {
                    message: "índice de archivo fuera de rango".into(),
                });
            }
        }
        let missing: Vec<_> = self
            .files_missing(&project.files)
            .await?
            .into_iter()
            .filter(|f| stdin_index.map(|i| &project.files[i] != f).unwrap_or(true))
            .collect();
        if !missing.is_empty() {
            return Err(EngineError::InvalidInput {
                message: format!("no existe: {}", missing.join(", ")),
            });
        }
        let args = config_args(cli, project, stdin_index);
        self.config_run(cli, &project.name, args, unsaved.map(|(_, c)| c))
            .await
    }

    /// Nombre de proyecto que resulta de unos archivos (su `name:` o la
    /// carpeta), con `config --format json` sin `-p`. Para añadir un proyecto.
    pub async fn compose_resolve(
        &self,
        cli: &ComposeCli,
        files: &[String],
    ) -> Result<ComposeConfig, EngineError> {
        if files.is_empty() || files.len() > super::project::MAX_FILES {
            return Err(EngineError::InvalidInput {
                message: "indica entre 1 y 16 archivos Compose".into(),
            });
        }
        let probe = ProjectRef {
            name: "x".into(),
            files: files.to_vec(),
            working_dir: None,
            env_files: vec![],
        };
        probe.validate()?;
        let missing = self.files_missing(&probe.files).await?;
        if !missing.is_empty() {
            return Err(EngineError::InvalidInput {
                message: format!("no existe: {}", missing.join(", ")),
            });
        }
        let mut args = cli.prefix.clone();
        args.extend(["--ansi".into(), "never".into()]);
        for f in files {
            args.push("-f".into());
            args.push(f.clone());
        }
        if let Some(d) = probe.project_dir() {
            args.push("--project-directory".into());
            args.push(d);
        }
        args.extend(["config".into(), "--format".into(), "json".into()]);
        self.config_run(cli, &files[0], args, None).await
    }

    async fn config_run(
        &self,
        cli: &ComposeCli,
        target: &str,
        args: Vec<String>,
        stdin: Option<String>,
    ) -> Result<ComposeConfig, EngineError> {
        let command = cmc_activity::quote::render_argv(&cli.program, &args);
        let req = self.compose_request(
            cli,
            "compose.config".into(),
            target,
            args,
            CONFIG_TIMEOUT,
            false,
        )?;
        let mut opts = StreamOptions::new(CancellationToken::new());
        opts.stdin = stdin.map(String::into_bytes);
        opts.collect_stdout = true;
        // La configuración resuelta incluye los valores de las variables.
        opts.record_stdout = false;
        opts.summary = Some(
            "Configuración válida (la configuración resuelta no se guarda en el registro)".into(),
        );
        let run = run_streaming(self.activity(), req, opts, |_| true).await?;
        let stderr: Vec<String> = run
            .stderr_tail
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect();
        if run.success() {
            let model = parse_model(&run.stdout)?;
            Ok(ComposeConfig {
                model: Some(model),
                resolved: Some(run.stdout),
                error: None,
                warnings: stderr,
                command,
            })
        } else {
            Ok(ComposeConfig {
                model: None,
                resolved: None,
                error: Some(if stderr.is_empty() {
                    format!("Compose terminó con {:?}", run.end)
                } else {
                    stderr.join("\n")
                }),
                warnings: vec![],
                command,
            })
        }
    }

    /// Ejecuta una acción con su salida en vivo. Las acciones que eliminan
    /// recursos exigen `confirmed`. Solo una acción a la vez por proyecto.
    pub async fn compose_run<F>(
        &self,
        cli: &ComposeCli,
        project: &ProjectRef,
        action: &ComposeAction,
        confirmed: bool,
        cancel: CancellationToken,
        on_lines: F,
    ) -> Result<StreamedRun, EngineError>
    where
        F: FnMut(Vec<OutputLine>) -> bool,
    {
        let missing = self.files_missing(&project.files).await?;
        self.compose_check(project, action, confirmed, &missing)?;
        let _guard = self.locks().acquire(&format!("compose:{}", project.name))?;
        let args = action_args_with(cli, project, action, missing.is_empty());
        let req = self.compose_request(
            cli,
            action.kind(),
            &project.name,
            args,
            ACTION_TIMEOUT,
            true,
        )?;
        let mut opts = StreamOptions::new(cancel);
        opts.summary = Some(summary_for(action, &project.name));
        let run = run_streaming(self.activity(), req, opts, on_lines).await?;
        // También si falló o se interrumpió: Compose puede haber cambiado algo.
        if action.changes_state() {
            let _ = self.compose_settle(&project.name).await;
        }
        Ok(run)
    }

    /// Confirma con inspect el estado de cada contenedor del proyecto (D-031, D-056).
    ///
    /// Docker responde a la parada (o al arranque) de un contenedor *antes* de
    /// actualizar la réplica de la que lee `GET /containers/json`; justo después
    /// de `docker compose stop` el listado puede decir "running" (observado con
    /// Docker 26). Inspect toma el bloqueo del contenedor y espera a que termine
    /// esa escritura: al responder, el listado ya es coherente. Cada consulta se
    /// registra. Un contenedor que ya no existe (huérfano eliminado) se omite.
    pub async fn compose_settle(
        &self,
        project: &str,
    ) -> Result<Vec<crate::containers::ContainerState>, EngineError> {
        let list = self.list_containers().await?;
        let mut out = Vec::new();
        for c in list.iter().filter(|c| {
            c.labels
                .get(super::project::LABEL_PROJECT)
                .map(String::as_str)
                == Some(project)
        }) {
            match self.container_state(&c.id, Some(&c.name)).await {
                Ok(s) => out.push(s),
                Err(e) if e.http_status() == Some(404) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Riesgos para el equipo de los servicios que arrancaría `up` (E1, D-086),
    /// a partir de la configuración resuelta por Compose. Otras acciones no
    /// crean contenedores: sin riesgos. Si la configuración no es válida, `up`
    /// fallará igualmente y no hay riesgos que mostrar.
    pub async fn compose_up_risks(
        &self,
        cli: &ComposeCli,
        project: &ProjectRef,
        action: &ComposeAction,
    ) -> Result<Vec<crate::risk::Risk>, EngineError> {
        let ComposeAction::Up { services, .. } = action else {
            return Ok(Vec::new());
        };
        let cfg = self.compose_config(cli, project, None).await?;
        Ok(cfg
            .model
            .map(|m| super::project::risks_for(&m, services))
            .unwrap_or_default())
    }

    /// Comprobaciones previas de una acción, sin ejecutar nada (también las usa
    /// la interfaz para responder de inmediato antes de abrir la salida en vivo).
    /// `missing`: archivos del proyecto que no existen donde está el proyecto
    /// ([`Engine::files_missing`]).
    pub fn compose_check(
        &self,
        project: &ProjectRef,
        action: &ComposeAction,
        confirmed: bool,
        missing: &[String],
    ) -> Result<(), EngineError> {
        project.validate()?;
        action.validate()?;
        if action.destructive() && !confirmed {
            return Err(EngineError::NotConfirmed);
        }
        if self.locks().is_busy(&format!("compose:{}", project.name)) {
            return Err(EngineError::Busy {
                resource: format!("compose:{}", project.name),
            });
        }
        if action.needs_files() {
            if project.files.is_empty() {
                return Err(EngineError::InvalidInput {
                    message: "esta acción necesita los archivos Compose del proyecto".into(),
                });
            }
            if !missing.is_empty() {
                return Err(EngineError::InvalidInput {
                    message: format!("no existe: {}", missing.join(", ")),
                });
            }
        }
        Ok(())
    }

    /// `docker compose logs --follow`: una operación que se cierra cuando el
    /// usuario deja de mirar. Las líneas no se guardan en el registro.
    #[allow(clippy::too_many_arguments)]
    pub async fn compose_logs<F>(
        &self,
        cli: &ComposeCli,
        project: &ProjectRef,
        services: &[String],
        tail: u32,
        timestamps: bool,
        cancel: CancellationToken,
        on_lines: F,
    ) -> Result<StreamedRun, EngineError>
    where
        F: FnMut(Vec<OutputLine>) -> bool,
    {
        project.validate()?;
        if let Some(s) = services.iter().find(|s| !valid_service_name(s)) {
            return Err(EngineError::InvalidInput {
                message: format!("nombre de servicio no válido: {s:?}"),
            });
        }
        let missing = self.files_missing(&project.files).await?;
        let args = logs_args_with(
            cli,
            project,
            services,
            tail.min(MAX_LOG_TAIL),
            timestamps,
            missing.is_empty(),
        );
        let req = self.compose_request(
            cli,
            "compose.logs".into(),
            &project.name,
            args,
            LOGS_TIMEOUT,
            true,
        )?;
        let mut opts = StreamOptions::new(cancel);
        opts.record_stdout = false;
        opts.close_is_success = true;
        opts.summary = Some("Los contenedores del proyecto terminaron".into());
        run_streaming(self.activity(), req, opts, on_lines).await
    }
}

/// Comando mostrado para una acción (sin ejecutarla).
pub fn action_command(cli: &ComposeCli, project: &ProjectRef, action: &ComposeAction) -> String {
    cmc_activity::quote::render_argv(&cli.program, &action_args(cli, project, action))
}

// Pruebas con permisos, enlaces o sockets de Unix: solo en Linux (D-114).
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn compose_runs_from_an_empty_private_folder() {
        let d = neutral_dir().unwrap();
        assert!(d.is_dir());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&d).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(neutral_dir().unwrap(), d, "se reutiliza");
    }
}
