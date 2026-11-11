//! Servidores remotos (0.10.0): perfiles, conexión, prueba en siete pasos,
//! identidad del servidor, secretos, diagnóstico y vista general.
//!
//! Ninguna orden de este módulo ejecuta nada que cambie el servidor: no
//! instala Docker, no cambia permisos, no toca la configuración SSH ni abre
//! puertos. Lo único que se ejecuta en el servidor es `docker --version`,
//! `docker version` y `docker system dial-stdio` (el canal de la API).

use std::sync::Arc;
use std::time::Duration;

use dcc_activity::{OpStatus, ServerTag};
use dcc_engine::ssh::connect::TestReport;
use dcc_engine::ssh::hostkey::{self, TrustError, TrustedKey};
use dcc_engine::ssh::{HostKeyInfo, RemoteProblem, Secret, SecretKind, SecretProvider, SshContext};
use dcc_engine::{ConnectionState, Problem, SystemOverview};
use dcc_store::ServerConfig;
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};
use ts_rs::TS;

use crate::error::{CmdResult, CommandError};
use crate::secrets::{KeyringStatus, Secrets};
use crate::servers::{issues, profile_of, secret_kind, tag_of, ServerView, LOCAL};
use crate::state::AppState;

/// Plazo para la vista general de cada servidor.
const OVERVIEW_TIMEOUT: Duration = Duration::from_secs(15);
/// Errores recientes que se muestran en el diagnóstico.
const RECENT_ERRORS: usize = 10;

#[tauri::command]
pub fn servers_list(state: State<'_, AppState>) -> Vec<ServerView> {
    state.servers.list()
}

/// Valida un perfil sin guardarlo (asistente). Códigos `server.*` por campo.
#[tauri::command]
pub fn server_validate(config: ServerConfig) -> Vec<dcc_engine::ssh::profile::ProfileIssue> {
    let mut c = config;
    if c.id.is_empty() {
        c.id = "srv-00000000".into();
    }
    issues(&c)
}

/// Añade (sin identificador) o cambia un servidor.
#[tauri::command]
pub async fn server_save(
    state: State<'_, AppState>,
    config: ServerConfig,
) -> CmdResult<ServerConfig> {
    if !config.id.is_empty() {
        // Cambiar los datos de conexión cierra la conexión y sus flujos.
        state.streams.close_server(&config.id);
    }
    state.servers.save(config).await
}

/// Elimina el perfil. Solo datos de la aplicación: nada del servidor.
#[tauri::command]
pub async fn server_delete(
    state: State<'_, AppState>,
    server: String,
    confirmed: bool,
) -> CmdResult<()> {
    if !confirmed {
        return Err(dcc_engine::EngineError::NotConfirmed.into());
    }
    if server == LOCAL {
        return Err(CommandError::new(
            "server.localFixed",
            "el equipo local no se puede eliminar",
        ));
    }
    state.servers.config(&server)?;
    state.streams.close_server(&server);
    state.servers.delete(&server).await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectOutcome {
    pub state: ConnectionState,
    /// Aviso si no se pudo usar el keyring (el secreto vale solo para esta sesión).
    pub keyring_notice: Option<String>,
}

/// Conecta. `secret`: frase de paso o contraseña escrita por el usuario (si
/// el perfil la necesita). `remember`: guardarla en el keyring del sistema.
#[tauri::command]
pub async fn server_connect(
    state: State<'_, AppState>,
    server: String,
    secret: Option<String>,
    remember: bool,
) -> CmdResult<ConnectOutcome> {
    connect_server(&state, &server, secret.map(Secret::new), remember).await
}

pub async fn connect_server(
    state: &AppState,
    id: &str,
    secret: Option<Secret>,
    remember: bool,
) -> CmdResult<ConnectOutcome> {
    let m = state.server(id)?;
    if id == LOCAL {
        return Ok(ConnectOutcome {
            state: m.connect().await,
            keyring_notice: None,
        });
    }
    let mut c = state.servers.config(id)?;
    let secrets = &state.servers.secrets;
    let mut notice = None;
    let typed = secret.is_some();
    if let Some(kind) = secret_kind(&c) {
        match secret {
            Some(s) => {
                secrets.remember_for_session(id, kind, s.clone());
                if remember {
                    if let Err(e) = secrets.store(id, &c.name, kind, &s).await {
                        notice = Some(e.to_string());
                    }
                }
                if c.remember_secret != (remember && notice.is_none()) {
                    c.remember_secret = remember && notice.is_none();
                    state.servers.save(c.clone()).await?;
                }
            }
            None if c.remember_secret => {
                if let Err(e) = secrets.prefetch(id, kind).await {
                    notice = Some(e.to_string());
                }
            }
            None => {}
        }
    }
    let m = state.server(id)?;
    let st = m.connect().await;
    if let ConnectionState::Disconnected { diagnosis, .. } = &st {
        if let Problem::Remote {
            remote: RemoteProblem::AuthFailed { .. },
            ..
        } = &diagnosis.problem
        {
            // Un secreto incorrecto no se vuelve a usar: se pedirá de nuevo.
            secrets.forget_session(id);
            if typed && remember {
                let _ = secrets.delete(id).await;
            }
        }
    }
    Ok(ConnectOutcome {
        state: st,
        keyring_notice: notice,
    })
}

/// Desconecta: cierra sus flujos y la sesión SSH. No toca el servidor ni sus
/// contenedores (siguen como estén).
#[tauri::command]
pub async fn server_disconnect(state: State<'_, AppState>, server: String) -> CmdResult<()> {
    if server == LOCAL {
        return Err(CommandError::new(
            "server.localFixed",
            "el equipo local no se desconecta",
        ));
    }
    let m = state.server(&server)?;
    state.streams.close_server(&server);
    m.disconnect().await;
    state.servers.secrets.forget_session(&server);
    Ok(())
}

/// Proveedor que añade el secreto escrito en el asistente a los de la sesión.
struct WithSecret {
    id: String,
    kind: SecretKind,
    secret: Secret,
    rest: Arc<Secrets>,
}

impl SecretProvider for WithSecret {
    fn secret(&self, id: &str, kind: SecretKind) -> Option<Secret> {
        if id == self.id && kind == self.kind {
            Some(self.secret.clone())
        } else {
            self.rest.secret(id, kind)
        }
    }
}

/// Perfil con identificador (uno provisional si aún no se ha guardado).
fn with_id(mut c: ServerConfig) -> ServerConfig {
    if c.id.is_empty() {
        c.id = format!("srv-{}", dcc_engine::ssh::askpass::random_hex(8));
    }
    c
}

fn tag_for(c: &ServerConfig) -> ServerTag {
    tag_of(c)
}

/// «Probar conexión» en siete pasos, con un perfil guardado o no. No conserva
/// la conexión, no guarda nada y no sustituye la conexión actual.
#[tauri::command]
pub async fn server_test(
    state: State<'_, AppState>,
    config: ServerConfig,
    secret: Option<String>,
) -> CmdResult<TestReport> {
    let c = with_id(config);
    let mut ctx: SshContext = state.servers.ctx.clone();
    if let Some(kind) = secret_kind(&c) {
        match secret {
            Some(s) => {
                ctx.secrets = Arc::new(WithSecret {
                    id: c.id.clone(),
                    kind,
                    secret: Secret::new(s),
                    rest: state.servers.secrets.clone(),
                })
            }
            None if c.remember_secret => {
                let _ = state.servers.secrets.prefetch(&c.id, kind).await;
            }
            None => {}
        }
    }
    let log = state.activity.with_server(tag_for(&c));
    Ok(dcc_engine::ssh::connect::test(&ctx, &profile_of(&c), &log).await)
}

/// Identidad que presenta ahora el servidor (para revisarla antes de confiar).
#[tauri::command]
pub async fn server_host_key(
    state: State<'_, AppState>,
    config: ServerConfig,
) -> CmdResult<HostKeyInfo> {
    let c = with_id(config);
    hostkey::presented(&state.servers.ctx.paths, &profile_of(&c))
        .await
        .map_err(remote_error)
}

/// Identidades guardadas para el servidor (propias y de `~/.ssh/known_hosts`).
#[tauri::command]
pub async fn server_trusted_keys(
    state: State<'_, AppState>,
    config: ServerConfig,
) -> CmdResult<Vec<TrustedKey>> {
    let c = with_id(config);
    Ok(hostkey::trusted(&state.servers.ctx.paths, &profile_of(&c)).await)
}

/// Confía en la identidad confirmada por el usuario. Solo se guarda si la que
/// presenta el servidor ahora es exactamente esa. `replace` sustituye una
/// identidad anterior (cambio de clave revisado por el usuario).
#[tauri::command]
pub async fn server_trust(
    state: State<'_, AppState>,
    config: ServerConfig,
    key: HostKeyInfo,
    replace: bool,
) -> CmdResult<HostKeyInfo> {
    let c = with_id(config);
    let p = profile_of(&c);
    let r = hostkey::trust(&state.servers.ctx.paths, &p, &key, replace).await;
    match r {
        Ok(k) => {
            let log = state.activity.with_server(tag_for(&c));
            let h = log.begin(dcc_activity::OperationSpec {
                category: dcc_activity::Category::System,
                kind: if replace {
                    "server.hostKeyReplace".into()
                } else {
                    "server.hostKeyTrust".into()
                },
                target: Some(c.name.clone()),
                transport: dcc_activity::Transport::App {
                    action: if replace {
                        "Sustitución de la identidad del servidor (revisada por el usuario)".into()
                    } else {
                        "Confianza en la identidad del servidor (huella confirmada)".into()
                    },
                },
                request_body: None,
                equivalent_command: None,
            });
            h.succeed(dcc_activity::Outcome {
                summary: Some(format!("{} {}", k.key_type, k.fingerprint)),
                ..Default::default()
            });
            Ok(k)
        }
        Err(TrustError::Mismatch { presented }) => Err(CommandError::new(
            "server.hostKeyMismatch",
            format!(
                "el servidor presenta ahora otra identidad ({}); no se ha guardado nada",
                presented
                    .iter()
                    .map(|k| k.fingerprint.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )),
        Err(TrustError::InUserFile { file, pattern }) => Err(CommandError::new(
            "server.hostKeyInUserFile",
            format!(
                "la identidad anterior está en {file}; la aplicación no modifica ese archivo. \
                 Revísala y, si es correcta, quítala con: ssh-keygen -R '{pattern}'"
            ),
        )),
        Err(TrustError::Problem(p)) => Err(remote_error(p)),
        Err(TrustError::Io(e)) => Err(CommandError::new("server.io", e)),
    }
}

fn remote_error(p: RemoteProblem) -> CommandError {
    let code = serde_json::to_value(&p)
        .ok()
        .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "other".into());
    CommandError::new(format!("server.remote.{code}"), format!("{p:?}"))
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SecretStatus {
    /// Secreto que necesita el perfil (ninguno con agente o clave sin frase).
    pub kind: Option<SecretKind>,
    /// Disponible en memoria para esta sesión.
    pub in_session: bool,
    /// Guardado en el keyring (`None` si no se pudo consultar).
    pub stored: Option<bool>,
    pub keyring: KeyringStatus,
}

#[tauri::command]
pub async fn server_secret_status(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<SecretStatus> {
    let c = state.servers.config(&server)?;
    let secrets = &state.servers.secrets;
    let keyring = secrets.keyring_status().await;
    let kind = secret_kind(&c);
    let stored = match (kind, &keyring) {
        (Some(k), KeyringStatus::Available) => secrets.stored(&server, k).await.ok(),
        (None, _) => Some(false),
        _ => None,
    };
    Ok(SecretStatus {
        kind,
        in_session: kind.is_some_and(|k| secrets.in_session(&server, k)),
        stored,
        keyring,
    })
}

#[tauri::command]
pub async fn keyring_status(state: State<'_, AppState>) -> CmdResult<KeyringStatus> {
    Ok(state.servers.secrets.keyring_status().await)
}

/// Borra los secretos guardados del servidor (keyring y sesión). La conexión
/// abierta, si la hay, sigue: el secreto se pedirá la próxima vez.
#[tauri::command]
pub async fn server_forget_secret(state: State<'_, AppState>, server: String) -> CmdResult<u32> {
    let mut c = state.servers.config(&server)?;
    let n = state
        .servers
        .secrets
        .delete(&server)
        .await
        .map_err(|e| CommandError::new("server.keyring", e))?;
    if c.remember_secret {
        c.remember_secret = false;
        state.servers.save(c).await?;
    }
    state.servers.record_action(
        &server,
        "server.secretForget",
        "Eliminación de las credenciales guardadas",
        Some(format!("{n}")),
    );
    Ok(n as u32)
}

/// Operación reciente fallida (sin cuerpos ni salidas: solo lo necesario).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecentError {
    pub kind: String,
    pub target: Option<String>,
    pub error: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServerDiagnosis {
    pub server: ServerView,
    /// Prueba completa recién hecha (servidores remotos).
    pub report: Option<TestReport>,
    pub recent_errors: Vec<RecentError>,
}

/// Diagnóstico de un servidor: prueba en siete pasos y errores recientes.
#[tauri::command]
pub async fn server_diagnose(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<ServerDiagnosis> {
    state.server(&server)?;
    let view = state
        .servers
        .list()
        .into_iter()
        .find(|v| v.id == server)
        .ok_or_else(|| CommandError::new("server.unknown", server.clone()))?;
    let report = match &view.config {
        Some(c) => {
            if let Some(k) = secret_kind(c).filter(|_| c.remember_secret) {
                let _ = state.servers.secrets.prefetch(&server, k).await;
            }
            let log = state.activity.with_server(tag_for(c));
            Some(dcc_engine::ssh::connect::test(&state.servers.ctx, &profile_of(c), &log).await)
        }
        None => None,
    };
    let recent_errors = state
        .activity
        .snapshot()
        .into_iter()
        .filter(|r| {
            r.status == OpStatus::Failed
                && r.server.as_ref().map(|s| s.id.as_str()).unwrap_or(LOCAL) == server
        })
        .take(RECENT_ERRORS)
        .map(|r| RecentError {
            kind: r.kind,
            target: r.target,
            error: r.error,
            at: r.started_at.to_rfc3339(),
        })
        .collect();
    Ok(ServerDiagnosis {
        server: view,
        report,
        recent_errors,
    })
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServerOverview {
    pub server: ServerView,
    pub overview: Option<SystemOverview>,
    pub error: Option<CommandError>,
    /// Fecha RFC 3339 de la consulta.
    pub updated_at: String,
}

/// Vista general de todos los servidores conectados, cada uno por separado y
/// con su plazo: uno lento o caído no retrasa ni afecta a los demás.
#[tauri::command]
pub async fn servers_overview(state: State<'_, AppState>) -> CmdResult<Vec<ServerOverview>> {
    let views = state.servers.list();
    let tasks = views.into_iter().map(|v| {
        let m = state.server(&v.id);
        async move {
            let connected = matches!(v.state, ConnectionState::Connected { .. });
            let (overview, error) = match (m, connected) {
                (Ok(m), true) => match tokio::time::timeout(OVERVIEW_TIMEOUT, m.overview()).await {
                    Ok(Ok(o)) => (Some(o), None),
                    Ok(Err(e)) => (None, Some(e.into())),
                    Err(_) => (None, Some(dcc_engine::EngineError::Timeout.into())),
                },
                (Err(e), _) => (None, Some(e)),
                _ => (None, None),
            };
            ServerOverview {
                server: v,
                overview,
                error,
                updated_at: chrono::Utc::now().to_rfc3339(),
            }
        }
    });
    Ok(futures_join(tasks).await)
}

async fn futures_join<F>(tasks: impl Iterator<Item = F>) -> Vec<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let handles: Vec<_> = tasks.map(tokio::spawn).collect();
    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        if let Ok(v) = h.await {
            out.push(v);
        }
    }
    out
}

/// Al arrancar, conecta los servidores marcados como «conectar al abrir»,
/// cada uno por su cuenta.
pub fn spawn_auto_connect<R: Runtime>(app: AppHandle<R>) {
    let state = app.state::<AppState>();
    let ids: Vec<String> = state
        .servers
        .list()
        .into_iter()
        .filter(|v| v.config.as_ref().is_some_and(|c| c.auto_connect))
        .map(|v| v.id)
        .collect();
    for id in ids {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            if let Err(e) = connect_server(&state, &id, None, false).await {
                tracing::warn!(server = %id, error = %e, "no se pudo conectar al abrir");
            }
        });
    }
}
