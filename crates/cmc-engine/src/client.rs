// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Cliente de Docker Engine (socket Unix local o canal SSH remoto, 0.10.0)
//! con registro de actividad.
//!
//! Cada método público que habla con Docker declara la petición HTTP real que
//! `bollard` envía (método y ruta exactos, verificados por las pruebas de
//! integración con un proxy que captura el tráfico del socket) y su comando
//! `docker` equivalente.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use bollard::Docker;
use cmc_activity::{
    quote::render_argv, ActivityLog, Category, Failure, OperationSpec, Outcome, Transport,
};

use crate::endpoint::SYSTEM_SOCKET;
use crate::error::EngineError;
use crate::model::{ComponentVersion, Endpoint, EngineInfo, EngineSummary, EngineVersion};
use crate::resources::ResourceLocks;

/// Versión mínima de la API: Docker Engine 20.10 (Debian 12 incluye 20.10.24).
pub const MIN_API_VERSION: (u32, u32) = (1, 41);
/// Tiempo máximo para comprobaciones rápidas.
pub const QUICK_TIMEOUT: Duration = Duration::from_secs(5);
/// Tiempo máximo por petición de bollard (las operaciones largas usarán streams).
const CLIENT_TIMEOUT_SECS: u64 = 120;

/// Descripción de una petición a la API.
#[derive(Debug, Clone)]
pub struct ApiOp {
    pub category: Category,
    pub kind: &'static str,
    pub target: Option<String>,
    pub method: &'static str,
    /// Ruta exacta enviada, con query string.
    pub path: String,
    pub body: Option<String>,
    /// Argumentos del comando `docker` equivalente (sin el programa), o `None`.
    pub equivalent: Option<Vec<String>>,
    pub timeout: Option<Duration>,
}

impl ApiOp {
    pub fn get(category: Category, kind: &'static str, path: impl Into<String>) -> Self {
        Self {
            category,
            kind,
            target: None,
            method: "GET",
            path: path.into(),
            body: None,
            equivalent: None,
            timeout: None,
        }
    }
    pub fn equivalent<S: Into<String>>(mut self, args: impl IntoIterator<Item = S>) -> Self {
        self.equivalent = Some(args.into_iter().map(Into::into).collect());
        self
    }
    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = Some(t);
        self
    }
}

/// Flujo de bytes hacia la API de Docker (para las peticiones que no pasan por
/// bollard: construcción, eventos, tamaños de volúmenes, creación de redes).
pub trait RawStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin> RawStream for T {}

/// Cómo se abre una conexión nueva con el Docker Engine de este motor.
#[derive(Clone, Debug)]
pub enum Dialer {
    /// Socket Unix local.
    Unix(String),
    /// Canal `docker system dial-stdio` por SSH (0.10.0).
    Ssh(Arc<crate::ssh::SshSession>),
}

impl Dialer {
    pub async fn connect(&self) -> std::io::Result<Box<dyn RawStream>> {
        match self {
            #[cfg(unix)]
            Dialer::Unix(path) => Ok(Box::new(tokio::net::UnixStream::connect(path).await?)),
            // Windows: la tubería con nombre de Docker Desktop (1.9.0, D-114).
            #[cfg(windows)]
            Dialer::Unix(path) => Ok(Box::new(open_pipe(path).await?)),
            Dialer::Ssh(s) => Ok(Box::new(s.dial().await?)),
        }
    }
}

/// Abre la tubería con nombre de Docker Desktop. Si todas sus instancias están
/// ocupadas (`ERROR_PIPE_BUSY`), lo vuelve a intentar durante 2 s como mucho
/// (1.9.1), como recomienda la documentación de tokio y como hace bollard.
#[cfg(windows)]
async fn open_pipe(
    path: &str,
) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    const ERROR_PIPE_BUSY: i32 = 231;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        match tokio::net::windows::named_pipe::ClientOptions::new().open(path) {
            Err(e)
                if e.raw_os_error() == Some(ERROR_PIPE_BUSY)
                    && tokio::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            r => return r,
        }
    }
}

#[derive(Clone)]
pub struct Engine {
    docker: Docker,
    endpoint: Endpoint,
    activity: ActivityLog,
    locks: ResourceLocks,
    dialer: Dialer,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

impl Engine {
    pub fn new(endpoint: Endpoint, activity: ActivityLog) -> Result<Self, EngineError> {
        #[cfg(unix)]
        let docker = Docker::connect_with_unix(
            &endpoint.socket_path,
            CLIENT_TIMEOUT_SECS,
            bollard::API_DEFAULT_VERSION,
        )?;
        // Windows: Docker Desktop por su tubería con nombre (1.9.0, D-114).
        #[cfg(windows)]
        let docker = Docker::connect_with_named_pipe(
            &endpoint.socket_path,
            CLIENT_TIMEOUT_SECS,
            bollard::API_DEFAULT_VERSION,
        )?;
        Ok(Self {
            dialer: Dialer::Unix(endpoint.socket_path.clone()),
            docker,
            endpoint,
            activity,
            locks: ResourceLocks::default(),
        })
    }

    /// Motor sobre una sesión SSH ya autenticada (0.10.0, D-075): bollard
    /// habla HTTP por canales `docker system dial-stdio` de esa sesión.
    pub fn new_ssh(
        endpoint: Endpoint,
        session: Arc<crate::ssh::SshSession>,
        activity: ActivityLog,
    ) -> Result<Self, EngineError> {
        use hyper_util::client::legacy::Client;
        use hyper_util::rt::TokioExecutor;
        let client = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(60))
            // Cada conexión es un proceso `ssh`: pocas en reposo.
            .pool_max_idle_per_host(2)
            .build(crate::ssh::session::SshConnector {
                session: session.clone(),
            });
        let docker = Docker::connect_with_custom_transport(
            move |req: bollard::BollardRequest| {
                let client = client.clone();
                async move {
                    client
                        .request(req)
                        .await
                        .map_err(|e| bollard::errors::Error::IOError {
                            err: std::io::Error::other(e),
                        })
                }
            },
            Some("http://docker"),
            CLIENT_TIMEOUT_SECS,
            bollard::API_DEFAULT_VERSION,
        )?;
        Ok(Self {
            docker,
            endpoint,
            activity,
            locks: ResourceLocks::default(),
            dialer: Dialer::Ssh(session),
        })
    }

    /// Cómo abrir una conexión directa con la API (peticiones fuera de bollard).
    pub fn dialer(&self) -> Dialer {
        self.dialer.clone()
    }

    /// Sesión SSH, si el motor es remoto.
    pub fn ssh_session(&self) -> Option<&Arc<crate::ssh::SshSession>> {
        match &self.dialer {
            Dialer::Ssh(s) => Some(s),
            Dialer::Unix(_) => None,
        }
    }

    /// Comparte los bloqueos por recurso entre reconexiones.
    pub fn with_locks(mut self, locks: ResourceLocks) -> Self {
        self.locks = locks;
        self
    }

    pub fn locks(&self) -> &ResourceLocks {
        &self.locks
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// El mismo motor (conexión, bloqueos) con otra vista del registro de
    /// actividad, p. ej. etiquetada con una operación masiva (1.10.0, D-117).
    pub fn with_activity(&self, activity: ActivityLog) -> Self {
        let mut e = self.clone();
        e.activity = activity;
        e
    }

    pub fn activity(&self) -> &ActivityLog {
        &self.activity
    }

    /// Acceso directo a bollard para los módulos de recursos (siempre mediante `call`).
    pub fn docker(&self) -> &Docker {
        &self.docker
    }

    /// Comando `docker` equivalente, apuntando al mismo socket si no es el del sistema.
    pub fn docker_cli(&self, args: &[String]) -> String {
        let mut full = Vec::with_capacity(args.len() + 2);
        if self.endpoint.socket_path != SYSTEM_SOCKET
            && self.endpoint.socket_path != "/run/docker.sock"
        {
            full.push("--host".to_owned());
            full.push(self.endpoint.host_uri());
        }
        full.extend_from_slice(args);
        render_argv("docker", &full)
    }

    /// Ejecuta una petición registrándola en el panel de actividad.
    pub async fn call<T, E, Fut>(
        &self,
        op: ApiOp,
        fut: Fut,
        summary: impl FnOnce(&T) -> Option<String>,
    ) -> Result<T, EngineError>
    where
        E: Into<EngineError>,
        Fut: Future<Output = Result<T, E>>,
    {
        let handle = self.activity.begin(OperationSpec {
            category: op.category,
            kind: op.kind.to_owned(),
            target: op.target.clone(),
            transport: Transport::Api {
                method: op.method.to_owned(),
                path: op.path.clone(),
                endpoint: self.endpoint.socket_path.clone(),
            },
            request_body: op.body.clone(),
            equivalent_command: op.equivalent.as_deref().map(|a| self.docker_cli(a)),
        });
        match Self::run(op.timeout, fut).await {
            Ok(v) => {
                // bollard no expone el código HTTP de las respuestas correctas:
                // no se registra uno inventado.
                handle.succeed(Outcome {
                    summary: summary(&v),
                    ..Default::default()
                });
                Ok(v)
            }
            Err(e) => {
                handle.fail(Failure {
                    error: e.to_string(),
                    http_status: e.http_status(),
                    ..Default::default()
                });
                Err(e)
            }
        }
    }

    /// Petición sin registro. Solo para comprobaciones automáticas periódicas (D-018).
    pub async fn call_silent<T, E, Fut>(
        &self,
        timeout: Option<Duration>,
        fut: Fut,
    ) -> Result<T, EngineError>
    where
        E: Into<EngineError>,
        Fut: Future<Output = Result<T, E>>,
    {
        Self::run(timeout, fut).await
    }

    async fn run<T, E, Fut>(timeout: Option<Duration>, fut: Fut) -> Result<T, EngineError>
    where
        E: Into<EngineError>,
        Fut: Future<Output = Result<T, E>>,
    {
        match timeout {
            Some(t) => match tokio::time::timeout(t, fut).await {
                Ok(r) => r.map_err(Into::into),
                Err(_) => Err(EngineError::Timeout),
            },
            None => fut.await.map_err(Into::into),
        }
    }

    // ---------------------------------------------------------------- sistema

    pub fn ping_op() -> ApiOp {
        ApiOp::get(Category::System, "system.ping", "/_ping").timeout(QUICK_TIMEOUT)
    }

    /// La respuesta se valida dentro de la operación registrada: un socket que
    /// responde algo distinto de "OK" queda registrado como fallo.
    pub async fn ping(&self) -> Result<(), EngineError> {
        let d = self.docker.clone();
        self.call(
            Self::ping_op(),
            async move { check_ping(d.ping().await?) },
            |s| Some(format!("Respuesta: {s}")),
        )
        .await
        .map(drop)
    }

    pub async fn ping_silent(&self) -> Result<(), EngineError> {
        let d = self.docker.clone();
        self.call_silent(
            Some(QUICK_TIMEOUT),
            async move { check_ping(d.ping().await?) },
        )
        .await
        .map(drop)
    }

    pub async fn version(&self) -> Result<EngineVersion, EngineError> {
        let d = self.docker.clone();
        let op = ApiOp::get(Category::System, "system.version", "/version")
            .equivalent(["version"])
            .timeout(QUICK_TIMEOUT);
        let v = self
            .call(op, async move { d.version().await }, |v| {
                Some(format!(
                    "Docker Engine {} (API {})",
                    v.version.as_deref().unwrap_or("?"),
                    v.api_version.as_deref().unwrap_or("?")
                ))
            })
            .await?;
        Ok(EngineVersion {
            version: v.version,
            api_version: v.api_version,
            min_api_version: v.min_api_version,
            go_version: v.go_version,
            git_commit: v.git_commit,
            build_time: v.build_time,
            os: v.os,
            arch: v.arch,
            kernel_version: v.kernel_version,
            components: v
                .components
                .unwrap_or_default()
                .into_iter()
                .map(|c| ComponentVersion {
                    name: c.name,
                    version: c.version,
                })
                .collect(),
        })
    }

    pub async fn info(&self) -> Result<EngineInfo, EngineError> {
        let d = self.docker.clone();
        let op = ApiOp::get(Category::System, "system.info", "/info")
            .equivalent(["info"])
            .timeout(Duration::from_secs(15));
        let i = self
            .call(op, async move { d.info().await }, |i| {
                Some(format!(
                    "{} contenedores, {} imágenes",
                    i.containers.unwrap_or(0),
                    i.images.unwrap_or(0)
                ))
            })
            .await?;
        let enum_str = |v: Option<serde_json::Value>| {
            v.and_then(|v| v.as_str().map(str::to_owned))
                .filter(|s| !s.is_empty())
        };
        let security_options = i.security_options.unwrap_or_default();
        let rootless = security_options
            .iter()
            .any(|o| o.split(',').any(|p| p == "name=rootless"));
        Ok(EngineInfo {
            name: i.name,
            server_version: i.server_version,
            operating_system: i.operating_system,
            os_type: i.os_type,
            kernel_version: i.kernel_version,
            architecture: i.architecture,
            ncpu: i.ncpu,
            mem_total: i.mem_total,
            docker_root_dir: i.docker_root_dir,
            storage_driver: i.driver,
            logging_driver: i.logging_driver,
            cgroup_driver: enum_str(i.cgroup_driver.and_then(|v| serde_json::to_value(v).ok())),
            cgroup_version: enum_str(i.cgroup_version.and_then(|v| serde_json::to_value(v).ok())),
            containers: i.containers,
            containers_running: i.containers_running,
            containers_paused: i.containers_paused,
            containers_stopped: i.containers_stopped,
            images: i.images,
            swarm_active: i
                .swarm
                .and_then(|s| s.local_node_state)
                .and_then(|s| serde_json::to_value(s).ok())
                .and_then(|v| v.as_str().map(|s| s == "active"))
                .unwrap_or(false),
            security_options,
            warnings: i.warnings.unwrap_or_default(),
            rootless,
        })
    }
}

fn check_ping(r: String) -> Result<String, EngineError> {
    if r.trim() == "OK" {
        Ok(r)
    } else {
        Err(EngineError::InvalidResponse {
            message: format!("respuesta inesperada a /_ping: {r:?}"),
        })
    }
}

/// Compara "1.41" con el mínimo soportado.
pub fn api_at_least(api: &str, min: (u32, u32)) -> bool {
    let mut it = api.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let v = (it.next().unwrap_or(0), it.next().unwrap_or(0));
    v >= min
}

pub fn summary(version: &EngineVersion, info: &EngineInfo) -> EngineSummary {
    EngineSummary {
        version: version
            .version
            .clone()
            .or_else(|| info.server_version.clone())
            .unwrap_or_default(),
        api_version: version.api_version.clone().unwrap_or_default(),
        os: info
            .operating_system
            .clone()
            .or_else(|| version.os.clone())
            .unwrap_or_default(),
        arch: version
            .arch
            .clone()
            .or_else(|| info.architecture.clone())
            .unwrap_or_default(),
        rootless: info.rootless,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use crate::model::EndpointSource;

    #[test]
    fn api_version_comparison() {
        assert!(api_at_least("1.41", (1, 41)));
        assert!(api_at_least("1.56", (1, 41)));
        assert!(!api_at_least("1.40", (1, 41)));
        assert!(!api_at_least("basura", (1, 41)));
        assert!(api_at_least("2.0", (1, 41)));
    }

    #[cfg(unix)]
    #[test]
    fn equivalent_includes_host_only_for_non_system_socket() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("docker.sock");
        let _l = std::os::unix::net::UnixListener::bind(&p).unwrap();
        let other = Engine::new(
            Endpoint {
                socket_path: p.display().to_string(),
                source: EndpointSource::Rootless,
            },
            ActivityLog::new(10),
        )
        .unwrap();
        assert_eq!(
            other.docker_cli(&["info".into()]),
            format!("docker --host unix://{} info", p.display())
        );
        // Con el socket del sistema no hace falta --host (se prueba la regla, sin crear cliente).
        let mut sys = other.clone();
        sys.endpoint.socket_path = SYSTEM_SOCKET.into();
        assert_eq!(sys.docker_cli(&["info".into()]), "docker info");
    }
}
