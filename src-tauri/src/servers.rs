//! Registro de servidores (0.10.0, D-078): el equipo local y los servidores
//! remotos por SSH, cada uno con su propio gestor de conexión.
//!
//! Aislamiento: toda operación con Docker recibe el identificador del servidor
//! y se resuelve aquí ([`Servers::get`]); no hay un «servidor actual» en el
//! backend que una operación pueda heredar por error. Un servidor que falla no
//! afecta a los demás: cada gestor tiene su conexión, su monitor y sus tareas.
//!
//! Eliminar un perfil solo borra datos de la aplicación (perfil, secretos
//! guardados y, si nadie más la usa, la identidad del servidor en el
//! `known_hosts` propio). Nunca toca nada del servidor remoto.

use std::sync::{Arc, OnceLock};

use dcc_activity::{ActivityLog, ServerTag};
use dcc_engine::manager::Timing;
use dcc_engine::ssh::profile::{display_address, server_tag, validate, ProfileIssue};
use dcc_engine::ssh::{
    RemoteDocker, SecretKind, SshAuth, SshContext, SshOptions, SshPaths, SshProfile,
};
use dcc_engine::{ConnectionState, EngineManager};
use dcc_store::{ServerAuthMethod, ServerConfig, ServerDockerTarget, ServerSeen, ServerStore};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::error::{CmdResult, CommandError};
use crate::secrets::Secrets;

/// Identificador del equipo local.
pub const LOCAL: &str = ServerTag::LOCAL_ID;

/// Perfil de conexión del motor a partir del perfil guardado.
pub fn profile_of(c: &ServerConfig) -> SshProfile {
    let path = c.key_path.trim().to_owned();
    SshProfile {
        id: c.id.clone(),
        name: c.name.trim().to_owned(),
        host: c.host.trim().to_owned(),
        port: c.port,
        user: c.user.trim().to_owned(),
        description: c.description.trim().to_owned(),
        auth: match c.auth {
            ServerAuthMethod::Agent => SshAuth::Agent,
            ServerAuthMethod::Key => SshAuth::Key { path },
            ServerAuthMethod::KeyPassphrase => SshAuth::KeyPassphrase { path },
            ServerAuthMethod::Password => SshAuth::Password,
        },
        docker: match c.docker_target {
            ServerDockerTarget::Default => RemoteDocker::Default,
            ServerDockerTarget::Socket => RemoteDocker::Socket {
                path: c.docker_socket.trim().to_owned(),
            },
            ServerDockerTarget::Context => RemoteDocker::Context {
                name: c.docker_context.trim().to_owned(),
            },
        },
        options: SshOptions {
            connect_timeout: c.connect_timeout,
            proxy_jump: c.proxy_jump.trim().to_owned(),
        },
    }
}

/// Perfil guardado con los campos recortados (lo que se escribe en disco).
fn tidy(mut c: ServerConfig) -> ServerConfig {
    for f in [
        &mut c.name,
        &mut c.host,
        &mut c.user,
        &mut c.description,
        &mut c.key_path,
        &mut c.docker_socket,
        &mut c.docker_context,
        &mut c.proxy_jump,
    ] {
        *f = f.trim().to_owned();
    }
    if !matches!(
        c.auth,
        ServerAuthMethod::Key | ServerAuthMethod::KeyPassphrase
    ) {
        c.key_path.clear();
    }
    if c.auth.secret_kind().is_none() {
        c.remember_secret = false;
    }
    if c.docker_target != ServerDockerTarget::Socket {
        c.docker_socket.clear();
    }
    if c.docker_target != ServerDockerTarget::Context {
        c.docker_context.clear();
    }
    c
}

trait SecretKindOf {
    fn secret_kind(&self) -> Option<SecretKind>;
}

impl SecretKindOf for ServerAuthMethod {
    fn secret_kind(&self) -> Option<SecretKind> {
        match self {
            ServerAuthMethod::KeyPassphrase => Some(SecretKind::Passphrase),
            ServerAuthMethod::Password => Some(SecretKind::Password),
            _ => None,
        }
    }
}

/// Qué secreto necesita un perfil, si alguno.
pub fn secret_kind(c: &ServerConfig) -> Option<SecretKind> {
    c.auth.secret_kind()
}

/// Problemas del perfil (los mismos que comprueba el motor al conectar).
pub fn issues(c: &ServerConfig) -> Vec<ProfileIssue> {
    validate(&profile_of(c))
}

/// Etiqueta del servidor para registros y confirmaciones.
pub fn tag_of(c: &ServerConfig) -> ServerTag {
    server_tag(&profile_of(c))
}

/// Un servidor tal como lo ve la interfaz.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServerView {
    pub id: String,
    pub remote: bool,
    /// Nombre a mostrar; para el equipo local, vacío (la interfaz lo traduce).
    pub name: String,
    /// `usuario@host:puerto` o el socket local.
    pub address: String,
    /// Perfil guardado (solo servidores remotos).
    pub config: Option<ServerConfig>,
    pub state: ConnectionState,
}

#[derive(Clone)]
struct Remote {
    config: ServerConfig,
    manager: EngineManager,
    /// Cancela las tareas del servidor (monitor, eventos, puentes) al
    /// eliminarlo o sustituir su gestor.
    cancel: CancellationToken,
}

/// Arranca las tareas de un servidor nuevo (lo instala la aplicación).
pub type Spawner = Arc<dyn Fn(String, EngineManager, CancellationToken) + Send + Sync>;

pub struct Servers {
    pub local: EngineManager,
    remotes: RwLock<Vec<Remote>>,
    store: ServerStore,
    activity: ActivityLog,
    pub ctx: SshContext,
    pub secrets: Arc<Secrets>,
    timing: Timing,
    spawner: OnceLock<Spawner>,
    /// Serializa las escrituras de `servers.toml`.
    write: Mutex<()>,
    pub warning: Option<String>,
}

impl Servers {
    pub fn new(
        local: EngineManager,
        store: ServerStore,
        activity: ActivityLog,
        paths: SshPaths,
        timing: Timing,
    ) -> Self {
        let secrets = Arc::new(Secrets::default());
        let ctx = SshContext {
            paths,
            secrets: secrets.clone(),
        };
        let (list, warning) = match store.load() {
            Ok(l) => (l.servers, l.warning),
            Err(e) => (
                Vec::new(),
                Some(format!("No se pudo leer la lista de servidores ({e}).")),
            ),
        };
        let remotes = list
            .into_iter()
            .map(|c| Remote {
                manager: EngineManager::remote(
                    activity.clone(),
                    profile_of(&c),
                    ctx.clone(),
                    timing,
                ),
                config: c,
                cancel: CancellationToken::new(),
            })
            .collect();
        Self {
            local,
            remotes: RwLock::new(remotes),
            store,
            activity,
            ctx,
            secrets,
            timing,
            spawner: OnceLock::new(),
            write: Mutex::new(()),
            warning,
        }
    }

    /// Instala el lanzador de tareas y lo aplica a los servidores ya cargados.
    pub fn set_spawner(&self, spawner: Spawner) {
        if self.spawner.set(spawner.clone()).is_ok() {
            for r in self.remotes.read().iter() {
                spawner(r.config.id.clone(), r.manager.clone(), r.cancel.clone());
            }
        }
    }

    /// Gestor del servidor indicado. Un identificador desconocido es un error:
    /// nunca se recurre a otro servidor.
    pub fn get(&self, id: &str) -> CmdResult<EngineManager> {
        if id == LOCAL {
            return Ok(self.local.clone());
        }
        self.remotes
            .read()
            .iter()
            .find(|r| r.config.id == id)
            .map(|r| r.manager.clone())
            .ok_or_else(|| unknown(id))
    }

    pub fn config(&self, id: &str) -> CmdResult<ServerConfig> {
        self.remotes
            .read()
            .iter()
            .find(|r| r.config.id == id)
            .map(|r| r.config.clone())
            .ok_or_else(|| unknown(id))
    }

    #[cfg(test)]
    pub fn ids(&self) -> Vec<String> {
        std::iter::once(LOCAL.to_owned())
            .chain(self.remotes.read().iter().map(|r| r.config.id.clone()))
            .collect()
    }

    /// Etiqueta del servidor (para actividad y confirmaciones).
    pub fn tag(&self, id: &str) -> CmdResult<ServerTag> {
        if id == LOCAL {
            return Ok(ServerTag::local());
        }
        Ok(tag_of(&self.config(id)?))
    }

    pub fn list(&self) -> Vec<ServerView> {
        let local_state = self.local.state();
        let local_address = match &local_state {
            ConnectionState::Connected { endpoint, .. } => endpoint.socket_path.clone(),
            _ => String::new(),
        };
        let mut out = vec![ServerView {
            id: LOCAL.into(),
            remote: false,
            name: String::new(),
            address: local_address,
            config: None,
            state: local_state,
        }];
        out.extend(self.remotes.read().iter().map(|r| ServerView {
            id: r.config.id.clone(),
            remote: true,
            name: r.config.name.clone(),
            address: display_address(&profile_of(&r.config)),
            config: Some(r.config.clone()),
            state: r.manager.state(),
        }));
        out
    }

    fn persist(&self, list: &[Remote]) -> CmdResult<()> {
        let configs: Vec<ServerConfig> = list.iter().map(|r| r.config.clone()).collect();
        self.store.save(&configs)?;
        Ok(())
    }

    fn record(&self, tag: ServerTag, kind: &str, action: &str, summary: Option<String>) {
        let log = self.activity.with_server(tag.clone());
        let h = log.begin(dcc_activity::OperationSpec {
            category: dcc_activity::Category::System,
            kind: kind.to_owned(),
            target: Some(tag.name.clone()),
            transport: dcc_activity::Transport::App {
                action: action.to_owned(),
            },
            request_body: None,
            equivalent_command: None,
        });
        h.succeed(dcc_activity::Outcome {
            summary,
            ..Default::default()
        });
    }

    /// Añade un servidor o cambia uno existente. Valida el perfil completo.
    /// Si cambian los datos de conexión, la conexión actual se cierra (sin
    /// tocar nada del servidor) y hay que volver a conectar.
    pub async fn save(&self, incoming: ServerConfig) -> CmdResult<ServerConfig> {
        let mut c = tidy(incoming);
        let adding = c.id.is_empty();
        if adding {
            c.id = format!("srv-{}", dcc_engine::ssh::askpass::random_hex(8));
        }
        let problems = issues(&c);
        if !problems.is_empty() {
            return Err(CommandError {
                code: "server.invalid".into(),
                message: problems
                    .iter()
                    .map(|i| format!("{}: {}", i.field, i.code))
                    .collect::<Vec<_>>()
                    .join(", "),
                http_status: None,
                issues: None,
            });
        }
        let (old_manager, old_cancel) = {
            let _w = self.write.lock();
            let mut list = self.remotes.write();
            let mut next = list.clone();
            let pos = next.iter().position(|r| r.config.id == c.id);
            if !adding && pos.is_none() {
                return Err(unknown(&c.id));
            }
            if adding && next.len() >= dcc_store::servers::MAX_SERVERS {
                return Err(CommandError::new(
                    "server.tooMany",
                    format!("como mucho {} servidores", dcc_store::servers::MAX_SERVERS),
                ));
            }
            let replaced = match pos {
                Some(i) => {
                    // Los datos de la última conexión los pone la aplicación.
                    c.last_seen = next[i].config.last_seen.clone();
                    let same_connection = profile_of(&next[i].config).auth == profile_of(&c).auth
                        && strip(&next[i].config) == strip(&c);
                    if same_connection {
                        next[i].config = c.clone();
                        None
                    } else {
                        let old = next[i].clone();
                        next[i] = self.fresh(c.clone());
                        Some(old)
                    }
                }
                None => {
                    c.last_seen = None;
                    next.push(self.fresh(c.clone()));
                    None
                }
            };
            self.persist(&next)?;
            if let (Some(sp), Some(new)) = (
                self.spawner.get(),
                next.iter().find(|r| r.config.id == c.id),
            ) {
                if adding || replaced.is_some() {
                    sp(
                        new.config.id.clone(),
                        new.manager.clone(),
                        new.cancel.clone(),
                    );
                }
            }
            *list = next;
            match replaced {
                Some(old) => (Some(old.manager), Some(old.cancel)),
                None => (None, None),
            }
        };
        if let Some(m) = old_manager {
            m.disconnect().await;
        }
        if let Some(t) = old_cancel {
            t.cancel();
        }
        if c.auth.secret_kind().is_none() || !c.remember_secret {
            // Ya no se usa o no se quiere recordar: fuera del keyring.
            let _ = self.secrets.delete(&c.id).await;
        }
        self.record(
            tag_of(&c),
            if adding { "server.add" } else { "server.edit" },
            if adding {
                "Alta de un servidor remoto"
            } else {
                "Cambio de un servidor remoto"
            },
            Some(display_address(&profile_of(&c))),
        );
        Ok(c)
    }

    fn fresh(&self, c: ServerConfig) -> Remote {
        Remote {
            manager: EngineManager::remote(
                self.activity.clone(),
                profile_of(&c),
                self.ctx.clone(),
                self.timing,
            ),
            config: c,
            cancel: CancellationToken::new(),
        }
    }

    /// Elimina el perfil: cierra su conexión, borra sus secretos guardados y,
    /// si ningún otro perfil la usa, su identidad en el `known_hosts` propio.
    /// No se ejecuta nada en el servidor.
    pub async fn delete(&self, id: &str) -> CmdResult<ServerConfig> {
        let removed = {
            let _w = self.write.lock();
            let mut list = self.remotes.write();
            let pos = list
                .iter()
                .position(|r| r.config.id == id)
                .ok_or_else(|| unknown(id))?;
            let mut next = list.clone();
            let removed = next.remove(pos);
            self.persist(&next)?;
            *list = next;
            removed
        };
        removed.manager.disconnect().await;
        removed.cancel.cancel();
        let _ = self.secrets.delete(id).await;
        let p = profile_of(&removed.config);
        let pattern = dcc_engine::ssh::hostkey::host_pattern(&p);
        let shared = self
            .remotes
            .read()
            .iter()
            .any(|r| dcc_engine::ssh::hostkey::host_pattern(&profile_of(&r.config)) == pattern);
        if !shared {
            let _ = dcc_engine::ssh::hostkey::forget(&self.ctx.paths, &p).await;
        }
        self.record(
            tag_of(&removed.config),
            "server.delete",
            "Eliminación de un perfil de servidor (no se toca nada del servidor)",
            Some(display_address(&p)),
        );
        Ok(removed.config)
    }

    /// Anota los datos de la última conexión correcta.
    pub fn mark_seen(&self, id: &str) {
        let Ok(m) = self.get(id) else { return };
        let ConnectionState::Connected { engine, since, .. } = m.state() else {
            return;
        };
        let seen = ServerSeen {
            at: since.to_rfc3339(),
            docker_version: engine.version,
            api_version: engine.api_version,
            os: engine.os,
        };
        let _w = self.write.lock();
        let mut list = self.remotes.write();
        let mut next = list.clone();
        let Some(r) = next.iter_mut().find(|r| r.config.id == id) else {
            return;
        };
        r.config.last_seen = Some(seen);
        if let Err(e) = self.persist(&next) {
            tracing::warn!(error = %e, "no se pudo guardar la última conexión");
            return;
        }
        *list = next;
    }

    /// Registra en actividad una acción sobre la identidad o los secretos.
    pub fn record_action(&self, id: &str, kind: &str, action: &str, summary: Option<String>) {
        if let Ok(tag) = self.tag(id) {
            self.record(tag, kind, action, summary);
        }
    }

    /// Hay algún servidor remoto conectado o conectando.
    pub fn any_remote_open(&self) -> bool {
        self.remotes
            .read()
            .iter()
            .any(|r| !matches!(r.manager.state(), ConnectionState::Idle))
    }

    /// Cierra todas las conexiones remotas (al salir de la aplicación).
    pub async fn close_all(&self) {
        let managers: Vec<EngineManager> = self
            .remotes
            .read()
            .iter()
            .map(|r| r.manager.clone())
            .collect();
        for m in managers {
            m.disconnect().await;
        }
    }
}

/// Perfil sin los campos que no afectan a la conexión.
fn strip(c: &ServerConfig) -> ServerConfig {
    ServerConfig {
        name: String::new(),
        description: String::new(),
        auto_connect: false,
        remember_secret: false,
        last_seen: None,
        ..c.clone()
    }
}

fn unknown(id: &str) -> CommandError {
    CommandError::new("server.unknown", format!("servidor desconocido: {id}"))
}

/// Rutas SSH de la aplicación. Si no hay cliente `ssh`, las conexiones
/// remotas lo indicarán (`sshMissing`) sin afectar al equipo local.
pub fn ssh_paths(config_dir: std::path::PathBuf) -> SshPaths {
    // La propia aplicación hace de SSH_ASKPASS (ver `main.rs`).
    #[allow(unused_mut)]
    let mut askpass =
        std::env::current_exe().unwrap_or_else(|_| "/usr/bin/docker-control-center".into());
    // En las pruebas unitarias el ejecutable es el de las pruebas: se usa
    // `dcc-askpass`, que ejecuta el mismo código.
    #[cfg(test)]
    if let Some(p) = std::env::var_os("DCC_TEST_ASKPASS") {
        askpass = p.into();
    }
    SshPaths::detect(config_dir.clone(), askpass.clone()).unwrap_or_else(|| SshPaths {
        ssh: "/usr/bin/ssh".into(),
        ssh_keygen: "/usr/bin/ssh-keygen".into(),
        runtime_dir: dcc_engine::ssh::runtime_dir(),
        app_known_hosts: config_dir.join("known_hosts"),
        user_known_hosts: dirs::home_dir()
            .unwrap_or_else(|| "/".into())
            .join(".ssh")
            .join("known_hosts"),
        askpass,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ServerConfig {
        ServerConfig {
            name: "  Producción ".into(),
            host: "docker-a.example.com".into(),
            user: "deploy".into(),
            auth: ServerAuthMethod::Key,
            key_path: " ~/.ssh/id_ed25519 ".into(),
            docker_socket: "/run/x.sock".into(),
            remember_secret: true,
            ..Default::default()
        }
    }

    fn servers(dir: &std::path::Path) -> Servers {
        let paths = ssh_paths(dir.to_path_buf());
        Servers::new(
            EngineManager::new(ActivityLog::new(100), dcc_engine::ConnectionMode::Auto),
            ServerStore::at(dir.join("servers.toml")),
            ActivityLog::new(100),
            paths,
            Timing::default(),
        )
    }

    #[test]
    fn profile_mapping_and_tidy() {
        let c = tidy(cfg());
        assert_eq!(c.name, "Producción");
        assert_eq!(c.key_path, "~/.ssh/id_ed25519");
        assert!(c.docker_socket.is_empty(), "socket sin usar no se guarda");
        assert!(!c.remember_secret, "una clave sin frase no tiene secreto");
        let p = profile_of(&c);
        assert_eq!(
            p.auth,
            SshAuth::Key {
                path: "~/.ssh/id_ed25519".into()
            }
        );
        assert!(issues(&ServerConfig {
            id: "srv-0123abcd".into(),
            ..c
        })
        .is_empty());
    }

    #[tokio::test]
    async fn add_edit_delete_are_isolated_and_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let s = servers(dir.path());
        let a = s.save(cfg()).await.unwrap();
        assert!(dcc_store::servers::valid_id(&a.id));
        let mut b = cfg();
        b.name = "Pruebas".into();
        b.host = "docker-b.example.com".into();
        let b = s.save(b).await.unwrap();
        assert_ne!(a.id, b.id);
        // Cada identificador da su propio gestor; uno desconocido es un error.
        assert!(s.get(&a.id).unwrap().is_remote());
        assert!(!s.get(LOCAL).unwrap().is_remote());
        assert_eq!(s.get("srv-ffffffff").unwrap_err().code, "server.unknown");
        assert_eq!(s.get("").unwrap_err().code, "server.unknown");
        assert!(matches!(
            s.get(&a.id).unwrap().state(),
            ConnectionState::Idle
        ));
        // Cambiar solo el nombre conserva el gestor; cambiar el host lo sustituye.
        let m1 = s.get(&a.id).unwrap();
        let mut a2 = a.clone();
        a2.name = "Producción A".into();
        s.save(a2.clone()).await.unwrap();
        assert!(m1.same(&s.get(&a.id).unwrap()));
        a2.host = "10.0.0.5".into();
        s.save(a2).await.unwrap();
        assert!(!m1.same(&s.get(&a.id).unwrap()));
        // Persistido y recargado en el mismo orden.
        let again = servers(dir.path());
        let names: Vec<String> = again.list().into_iter().map(|v| v.name).collect();
        assert_eq!(names, ["", "Producción A", "Pruebas"]);
        // Eliminar solo quita ese perfil.
        s.delete(&a.id).await.unwrap();
        assert_eq!(s.get(&a.id).unwrap_err().code, "server.unknown");
        assert!(s.get(&b.id).is_ok());
        assert_eq!(servers(dir.path()).ids(), [LOCAL.to_string(), b.id.clone()]);
        // Registrado en actividad con el servidor.
        let kinds: Vec<(String, String)> = s
            .activity
            .snapshot()
            .iter()
            .map(|r| (r.kind.clone(), r.server.clone().unwrap().name))
            .collect();
        assert!(kinds.contains(&("server.delete".into(), "Producción A".into())));
        assert!(kinds.contains(&("server.add".into(), "Pruebas".into())));
    }

    #[tokio::test]
    async fn invalid_or_unknown_profiles_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let s = servers(dir.path());
        let mut bad = cfg();
        bad.host = "-oProxyCommand=sh".into();
        assert_eq!(s.save(bad).await.unwrap_err().code, "server.invalid");
        let mut ghost = cfg();
        ghost.id = "srv-0123abcd".into();
        assert_eq!(s.save(ghost).await.unwrap_err().code, "server.unknown");
        assert_eq!(s.ids(), [LOCAL.to_string()]);
        assert!(!dir.path().join("servers.toml").exists());
    }

    #[test]
    fn last_seen_cannot_come_from_the_interface() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let s = servers(dir.path());
        let mut c = cfg();
        c.last_seen = Some(ServerSeen {
            at: "falso".into(),
            ..Default::default()
        });
        let saved = rt.block_on(s.save(c)).unwrap();
        assert!(saved.last_seen.is_none());
    }
}
