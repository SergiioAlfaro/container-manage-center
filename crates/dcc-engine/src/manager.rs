//! Gestor de la conexión con Docker Engine.
//!
//! - Resuelve los sockets candidatos y conecta con el primero que responde como Docker.
//! - Si ninguno sirve, construye un diagnóstico con la causa más relevante.
//! - Vigila la conexión y reconecta automáticamente.
//!
//! Política de registro (D-018): las conexiones pedidas por el usuario y las
//! operaciones de los apartados se registran siempre. Las comprobaciones
//! automáticas periódicas (latido cada 10 s y reintentos de reconexión) no se
//! registran una a una para no inundar el panel; sí se registra la comprobación
//! que detecta la pérdida de conexión.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use dcc_activity::ActivityLog;
use tokio::sync::{watch, Mutex, RwLock};

use crate::client::{self, Engine, MIN_API_VERSION};
use crate::endpoint::{resolve, HostEnv};
use crate::error::EngineError;
use crate::model::*;
use crate::probe;
use crate::sys;

pub const HEARTBEAT: Duration = Duration::from_secs(10);
const RETRY_MIN: Duration = Duration::from_secs(2);
const RETRY_MAX: Duration = Duration::from_secs(30);

type EnvFn = Arc<dyn Fn() -> HostEnv + Send + Sync>;

struct Inner {
    activity: ActivityLog,
    mode: RwLock<ConnectionMode>,
    engine: RwLock<Option<Engine>>,
    state: watch::Sender<ConnectionState>,
    /// Serializa los intentos de conexión.
    connect_lock: Mutex<()>,
    /// Despierta al monitor (p. ej. tras un error de conexión en una operación).
    wake: tokio::sync::Notify,
    env: EnvFn,
    timing: Timing,
    locks: crate::resources::ResourceLocks,
    /// Última detección correcta de Compose (se renueva con `compose_status`).
    compose: parking_lot::Mutex<Option<crate::compose::project::ComposeCli>>,
    /// Rutas y secretos para conectar por SSH (servidores remotos, 0.10.0).
    ssh: Option<crate::ssh::SshContext>,
}

/// Intervalos del monitor. Configurables para las pruebas.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    pub heartbeat: Duration,
    pub retry_min: Duration,
    pub retry_max: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            heartbeat: HEARTBEAT,
            retry_min: RETRY_MIN,
            retry_max: RETRY_MAX,
        }
    }
}

#[derive(Clone)]
pub struct EngineManager {
    inner: Arc<Inner>,
}

/// Resultado de intentar conectar con una configuración.
pub enum Attempt {
    Connected(Box<Engine>, EngineSummary),
    Failed(Box<Diagnosis>),
}

impl EngineManager {
    pub fn new(activity: ActivityLog, mode: ConnectionMode) -> Self {
        Self::with_env(activity, mode, Arc::new(HostEnv::from_process))
    }

    pub fn with_env(activity: ActivityLog, mode: ConnectionMode, env: EnvFn) -> Self {
        Self::with_options(activity, mode, env, Timing::default())
    }

    pub fn with_options(
        activity: ActivityLog,
        mode: ConnectionMode,
        env: EnvFn,
        timing: Timing,
    ) -> Self {
        Self::build(activity, mode, env, timing, None)
    }

    /// Gestor de un servidor remoto por SSH (0.10.0). Empieza sin conectar:
    /// solo se conecta cuando se pide.
    pub fn remote(
        activity: ActivityLog,
        profile: crate::ssh::SshProfile,
        ssh: crate::ssh::SshContext,
        timing: Timing,
    ) -> Self {
        Self::build(
            activity,
            ConnectionMode::Ssh { profile },
            Arc::new(HostEnv::from_process),
            timing,
            Some(ssh),
        )
    }

    fn build(
        activity: ActivityLog,
        mode: ConnectionMode,
        env: EnvFn,
        timing: Timing,
        ssh: Option<crate::ssh::SshContext>,
    ) -> Self {
        let initial = if ssh.is_some() {
            ConnectionState::Idle
        } else {
            ConnectionState::Connecting
        };
        let (state, _) = watch::channel(initial);
        Self {
            inner: Arc::new(Inner {
                activity,
                mode: RwLock::new(mode),
                engine: RwLock::new(None),
                state,
                connect_lock: Mutex::new(()),
                wake: tokio::sync::Notify::new(),
                env,
                timing,
                locks: Default::default(),
                compose: Default::default(),
                ssh,
            }),
        }
    }

    /// Servidor remoto (SSH).
    pub fn is_remote(&self) -> bool {
        self.inner.ssh.is_some()
    }

    /// Cierra la conexión (servidores remotos: la sesión SSH y sus canales).
    /// No toca nada del servidor. Queda sin conectar hasta que se pida.
    pub async fn disconnect(&self) {
        let _guard = self.inner.connect_lock.lock().await;
        let old = self.inner.engine.write().await.take();
        self.inner.state.send_replace(ConnectionState::Idle);
        if let Some(e) = old {
            if let Some(s) = e.ssh_session().cloned() {
                drop(e);
                s.close().await;
            }
        }
    }

    /// Prueba la conexión SSH en siete pasos sin sustituir la actual (0.10.0).
    pub async fn test_ssh(
        &self,
        profile: &crate::ssh::SshProfile,
    ) -> Result<crate::ssh::connect::TestReport, EngineError> {
        let ctx = self.inner.ssh.as_ref().ok_or(EngineError::Other {
            message: "este gestor no es de un servidor remoto".into(),
        })?;
        Ok(crate::ssh::connect::test(ctx, profile, &self.inner.activity).await)
    }

    /// Contexto SSH (rutas y secretos), si el gestor es remoto.
    pub fn ssh_context(&self) -> Option<&crate::ssh::SshContext> {
        self.inner.ssh.as_ref()
    }

    pub fn state(&self) -> ConnectionState {
        self.inner.state.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<ConnectionState> {
        self.inner.state.subscribe()
    }

    pub async fn mode(&self) -> ConnectionMode {
        self.inner.mode.read().await.clone()
    }

    /// Cliente activo, o error si no hay conexión.
    pub async fn engine(&self) -> Result<Engine, EngineError> {
        self.inner
            .engine
            .read()
            .await
            .clone()
            .ok_or(EngineError::NotConnected)
    }

    /// Conexión pedida por el usuario (o al arrancar): se registra en el panel.
    pub async fn connect(&self) -> ConnectionState {
        self.connect_inner(true).await
    }

    /// Cambia el modo de conexión y vuelve a conectar.
    pub async fn set_mode(&self, mode: ConnectionMode) -> ConnectionState {
        *self.inner.mode.write().await = mode;
        self.connect().await
    }

    /// Prueba una configuración sin sustituir la conexión actual.
    pub async fn test(
        &self,
        mode: ConnectionMode,
    ) -> Result<(Endpoint, EngineSummary), Box<Diagnosis>> {
        match attempt(&self.inner.activity, &mode, &(self.inner.env)(), true).await {
            Attempt::Connected(e, s) => Ok((e.endpoint().clone(), s)),
            Attempt::Failed(d) => Err(d),
        }
    }

    /// Informa de que una operación falló por un problema de conexión.
    pub fn report_connection_error(&self) {
        self.inner.wake.notify_one();
    }

    async fn connect_inner(&self, tracked: bool) -> ConnectionState {
        let _guard = self.inner.connect_lock.lock().await;
        let mode = self.inner.mode.read().await.clone();
        let env = (self.inner.env)();
        if tracked {
            self.inner.state.send_replace(ConnectionState::Connecting);
        }
        let (result, latency_ms) = match (&mode, &self.inner.ssh) {
            (ConnectionMode::Ssh { profile }, Some(ctx)) => {
                attempt_ssh(&self.inner.activity, profile, ctx).await
            }
            _ => (
                attempt(&self.inner.activity, &mode, &env, tracked).await,
                None,
            ),
        };
        let new_state = match result {
            Attempt::Connected(engine, summary) => {
                let endpoint = engine.endpoint().clone();
                let previous = self
                    .inner
                    .engine
                    .write()
                    .await
                    .replace((*engine).with_locks(self.inner.locks.clone()));
                drop(previous);
                ConnectionState::Connected {
                    endpoint,
                    engine: summary,
                    since: Utc::now(),
                    latency_ms,
                }
            }
            Attempt::Failed(diagnosis) => {
                *self.inner.engine.write().await = None;
                ConnectionState::Disconnected {
                    diagnosis,
                    since: Utc::now(),
                }
            }
        };
        self.publish(new_state.clone());
        new_state
    }

    /// Publica el estado solo si cambia de forma significativa (evita eventos repetidos).
    fn publish(&self, new: ConnectionState) {
        self.inner.state.send_if_modified(|cur| {
            let changed = !same_state(cur, &new);
            if changed {
                *cur = new;
            }
            changed
        });
    }

    /// Arranca la vigilancia periódica. Debe llamarse una vez, dentro de un runtime tokio.
    pub fn spawn_monitor(&self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(self.clone().run_monitor())
    }

    /// Bucle de vigilancia; para lanzarlo con el ejecutor que prefiera la aplicación.
    pub async fn run_monitor(self) {
        let t = self.inner.timing;
        let mut retry = t.retry_min;
        let mut changes = self.inner.state.subscribe();
        loop {
            let state = changes.borrow_and_update().clone();
            // Servidor remoto sin conectar, o con un problema que necesita al
            // usuario (identidad, credenciales…): no se reintenta solo.
            let waiting = match &state {
                ConnectionState::Idle => true,
                ConnectionState::Disconnected { diagnosis, .. } => matches!(
                    &diagnosis.problem,
                    Problem::Remote { remote, .. } if remote.needs_user()
                ),
                _ => false,
            };
            if waiting {
                let _ = changes.changed().await;
                retry = t.retry_min;
                continue;
            }
            let connected = matches!(state, ConnectionState::Connected { .. });
            if connected {
                retry = t.retry_min;
                tokio::select! {
                    _ = tokio::time::sleep(t.heartbeat) => {}
                    _ = self.inner.wake.notified() => {}
                }
                let Ok(engine) = self.engine().await else {
                    continue;
                };
                if engine.ping_silent().await.is_err() {
                    // Se confirma con una comprobación registrada antes de declarar la pérdida.
                    if engine.ping().await.is_err() {
                        tracing::warn!("conexión con Docker perdida");
                        self.connect_inner(false).await;
                    }
                }
            } else {
                tokio::select! {
                    _ = tokio::time::sleep(retry) => {}
                    _ = self.inner.wake.notified() => {}
                }
                if let ConnectionState::Connected { .. } = self.connect_inner(false).await {
                    tracing::info!("conexión con Docker restablecida");
                } else {
                    retry = (retry * 2).min(t.retry_max);
                }
            }
        }
    }

    /// Ejecuta una operación con el cliente activo. Si falla por un problema de
    /// conexión, despierta al monitor para que lo diagnostique de inmediato.
    pub async fn with_engine<T, F, Fut>(&self, f: F) -> Result<T, EngineError>
    where
        F: FnOnce(Engine) -> Fut,
        Fut: std::future::Future<Output = Result<T, EngineError>>,
    {
        let engine = self.engine().await?;
        let r = f(engine).await;
        if let Err(e) = &r {
            if e.is_connection_error() {
                self.report_connection_error();
            }
        }
        r
    }

    // ------------------------------------------------------------ información

    pub async fn overview(&self) -> Result<SystemOverview, EngineError> {
        self.with_engine(|engine| async move {
            let version = engine.version().await?;
            let info = engine.info().await?;
            Ok(SystemOverview {
                endpoint: engine.endpoint().clone(),
                version,
                info,
            })
        })
        .await
    }

    /// Docker Compose v2+ (plugin o binario independiente), ejecutando el CLI
    /// oficial sin shell (D-005, D-032).
    pub async fn compose_status(&self) -> crate::compose::ComposeStatus {
        if self.is_remote() {
            // El CLI local no debe hablar con un servidor remoto por su cuenta
            // (sin las opciones de seguridad de la aplicación). Compose remoto
            // se ejecuta en el propio servidor (D-080).
            return crate::compose::ComposeStatus::remote_pending();
        }
        let mut env = Vec::new();
        if let Ok(e) = self.engine().await {
            env.push(("DOCKER_HOST".to_owned(), e.endpoint().host_uri()));
        }
        let status = crate::compose::detect(
            &self.inner.activity,
            sys::find_executable("docker").map(|p| p.display().to_string()),
            sys::find_executable("docker-compose").map(|p| p.display().to_string()),
            &env,
        )
        .await;
        *self.inner.compose.lock() = crate::compose::project::ComposeCli::from_status(&status);
        status
    }

    /// Cómo invocar Compose. Usa la última detección correcta o detecta ahora.
    pub async fn compose_cli(&self) -> Result<crate::compose::project::ComposeCli, EngineError> {
        if let Some(c) = self.inner.compose.lock().clone() {
            if std::path::Path::new(&c.program).is_file() {
                return Ok(c);
            }
        }
        let status = self.compose_status().await;
        crate::compose::project::ComposeCli::from_status(&status)
            .ok_or(EngineError::ComposeUnavailable)
    }
}

/// Dos estados equivalentes (ignora las marcas de tiempo).
fn same_state(a: &ConnectionState, b: &ConnectionState) -> bool {
    match (a, b) {
        (ConnectionState::Connecting, ConnectionState::Connecting) => true,
        (ConnectionState::Idle, ConnectionState::Idle) => true,
        (
            ConnectionState::Connected {
                endpoint: e1,
                engine: g1,
                ..
            },
            ConnectionState::Connected {
                endpoint: e2,
                engine: g2,
                ..
            },
        ) => e1 == e2 && g1 == g2,
        (
            ConnectionState::Disconnected { diagnosis: d1, .. },
            ConnectionState::Disconnected { diagnosis: d2, .. },
        ) => d1.problem == d2.problem && d1.mode == d2.mode,
        _ => false,
    }
}

/// Conexión con un servidor remoto: la secuencia de siete pasos.
async fn attempt_ssh(
    activity: &ActivityLog,
    profile: &crate::ssh::SshProfile,
    ctx: &crate::ssh::SshContext,
) -> (Attempt, Option<u64>) {
    let out = crate::ssh::connect::connect(ctx, profile, activity).await;
    match out.result {
        Ok(c) => (
            Attempt::Connected(Box::new(c.engine), c.summary),
            Some(c.latency_ms),
        ),
        Err(remote) => (
            Attempt::Failed(Box::new(Diagnosis {
                problem: Problem::Remote {
                    remote,
                    steps: out.steps,
                },
                candidates: Vec::new(),
                installation: Installation::default(),
                user: sys::user_context(),
                mode: ConnectionMode::Ssh {
                    profile: profile.clone(),
                },
            })),
            None,
        ),
    }
}

/// Intenta conectar con la configuración dada.
pub async fn attempt(
    activity: &ActivityLog,
    mode: &ConnectionMode,
    env: &HostEnv,
    tracked: bool,
) -> Attempt {
    let resolution = resolve(mode, env);
    let mut reports = Vec::new();
    let mut first_problem: Option<Problem> = None;

    for endpoint in &resolution.candidates {
        let result = probe::probe(&endpoint.socket_path).await;
        reports.push(CandidateReport {
            endpoint: endpoint.clone(),
            result: result.clone(),
        });
        let path = endpoint.socket_path.clone();
        let problem = match result {
            ProbeResult::Reachable => match handshake(activity, endpoint.clone(), tracked).await {
                Ok((engine, summary)) => return Attempt::Connected(Box::new(engine), summary),
                Err(p) => {
                    if let Some(r) = reports.last_mut() {
                        r.result = ProbeResult::Error {
                            message: problem_text(&p),
                        };
                    }
                    Some(p)
                }
            },
            ProbeResult::Missing => None,
            ProbeResult::NotASocket => Some(Problem::NotASocket { socket_path: path }),
            ProbeResult::PermissionDenied => Some(Problem::PermissionDenied {
                permissions: probe::permissions(&path),
            }),
            ProbeResult::Refused => Some(Problem::DaemonNotRunning { socket_path: path }),
            ProbeResult::Error { message } => Some(Problem::Other {
                socket_path: Some(path),
                message,
            }),
        };
        if first_problem.is_none() {
            first_problem = problem;
        }
    }

    let problem = if let Some((path, reason)) = resolution.invalid {
        Problem::InvalidPath { path, reason }
    } else if let Some(host) = resolution.unsupported {
        Problem::UnsupportedHost { host }
    } else {
        first_problem.unwrap_or(Problem::NoSocket)
    };
    Attempt::Failed(Box::new(Diagnosis {
        problem,
        candidates: reports,
        installation: sys::installation(),
        user: sys::user_context(),
        mode: mode.clone(),
    }))
}

/// Comprueba que el socket es Docker y que la API es compatible.
async fn handshake(
    activity: &ActivityLog,
    endpoint: Endpoint,
    tracked: bool,
) -> Result<(Engine, EngineSummary), Problem> {
    let path = endpoint.socket_path.clone();
    let engine = Engine::new(endpoint, activity.clone()).map_err(|e| Problem::Other {
        socket_path: Some(path.clone()),
        message: e.to_string(),
    })?;
    let to_problem = |e: EngineError| match e {
        EngineError::Timeout => Problem::Timeout {
            socket_path: path.clone(),
        },
        EngineError::InvalidResponse { message } | EngineError::Io { message } => {
            Problem::NotDocker {
                socket_path: path.clone(),
                message,
            }
        }
        other => Problem::Other {
            socket_path: Some(path.clone()),
            message: other.to_string(),
        },
    };
    let ping = if tracked {
        engine.ping().await
    } else {
        engine.ping_silent().await
    };
    ping.map_err(to_problem)?;
    // version e info se registran siempre que la conexión se establece: son las
    // peticiones que fundamentan lo que la interfaz muestra a continuación.
    let version = engine.version().await.map_err(to_problem)?;
    let api = version.api_version.clone().unwrap_or_default();
    if !client::api_at_least(&api, MIN_API_VERSION) {
        return Err(Problem::ApiTooOld {
            socket_path: path.clone(),
            api_version: api,
            minimum: format!("{}.{}", MIN_API_VERSION.0, MIN_API_VERSION.1),
        });
    }
    let info = engine.info().await.map_err(to_problem)?;
    let summary = client::summary(&version, &info);
    Ok((engine, summary))
}

fn problem_text(p: &Problem) -> String {
    match p {
        Problem::NotDocker { message, .. } => format!("no responde como Docker Engine: {message}"),
        Problem::Timeout { .. } => "tiempo de espera agotado".into(),
        Problem::ApiTooOld {
            api_version,
            minimum,
            ..
        } => format!("API {api_version} (mínimo {minimum})"),
        Problem::Other { message, .. } => message.clone(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_with(dir: &std::path::Path) -> HostEnv {
        HostEnv {
            uid: 1000,
            xdg_runtime_dir: Some(dir.into()),
            docker_host: None,
            docker_context: None,
            docker_config_dir: None,
        }
    }

    #[tokio::test]
    async fn custom_missing_socket_reports_no_socket() {
        let dir = tempfile::tempdir().unwrap();
        let log = ActivityLog::new(10);
        let mode = ConnectionMode::Custom {
            socket_path: dir.path().join("x.sock").display().to_string(),
        };
        match attempt(&log, &mode, &env_with(dir.path()), true).await {
            Attempt::Failed(d) => {
                assert_eq!(d.problem, Problem::NoSocket);
                assert_eq!(d.candidates.len(), 1);
                assert_eq!(d.candidates[0].result, ProbeResult::Missing);
            }
            Attempt::Connected(..) => panic!("no debería conectar"),
        }
        assert!(log.is_empty(), "un socket inexistente no genera peticiones");
    }

    #[tokio::test]
    async fn invalid_custom_path_is_reported() {
        let log = ActivityLog::new(10);
        let mode = ConnectionMode::Custom {
            socket_path: "relativa.sock".into(),
        };
        match attempt(&log, &mode, &HostEnv::default(), true).await {
            Attempt::Failed(d) => assert!(matches!(d.problem, Problem::InvalidPath { .. })),
            Attempt::Connected(..) => panic!(),
        }
    }

    #[tokio::test]
    async fn stale_socket_reports_daemon_not_running() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("d.sock");
        drop(std::os::unix::net::UnixListener::bind(&p).unwrap());
        let mode = ConnectionMode::Custom {
            socket_path: p.display().to_string(),
        };
        match attempt(&ActivityLog::new(10), &mode, &env_with(dir.path()), true).await {
            Attempt::Failed(d) => assert_eq!(
                d.problem,
                Problem::DaemonNotRunning {
                    socket_path: p.display().to_string()
                }
            ),
            Attempt::Connected(..) => panic!(),
        }
    }

    #[tokio::test]
    async fn socket_that_is_not_docker_is_reported_and_recorded() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("fake.sock");
        let listener = tokio::net::UnixListener::bind(&p).unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    let _ = s.read(&mut buf).await;
                    let _ = s
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello")
                        .await;
                });
            }
        });
        let log = ActivityLog::new(10);
        let mode = ConnectionMode::Custom {
            socket_path: p.display().to_string(),
        };
        match attempt(&log, &mode, &env_with(dir.path()), true).await {
            Attempt::Failed(d) => assert!(
                matches!(d.problem, Problem::NotDocker { .. }),
                "{:?}",
                d.problem
            ),
            Attempt::Connected(..) => panic!(),
        }
        let r = &log.snapshot()[0];
        assert_eq!(r.kind, "system.ping");
        assert_eq!(r.status, dcc_activity::OpStatus::Failed);
    }
}
