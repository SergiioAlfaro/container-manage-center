# Backend Rust y Tauri

## Organización de módulos

El backend se divide en cuatro crates con dependencias en un solo sentido:

```text
src-tauri (aplicación)  ──►  cmc-engine  ──►  cmc-activity
        │                        │
        └──────────────►  cmc-store ◄──┘ (solo tipos de configuración)
```

- `cmc-activity` no depende de nadie: cualquier módulo puede registrar
  operaciones.
- `cmc-engine` no conoce Tauri: recibe un registro de actividad y devuelve
  resultados o `EngineError`. Se prueba sin interfaz y contra Docker real.
- `src-tauri` une todo: estado, órdenes, eventos y servicios del sistema.

El capítulo [Estructura del proyecto](#estructura-del-proyecto) enumera los
módulos de cada crate.

## Órdenes Tauri

La aplicación registra **130 órdenes** en `lib.rs` (`generate_handler!`),
agrupadas en `src-tauri/src/commands/`. Todas siguen el mismo esquema:

1. reciben el `AppState` y argumentos tipados (deserializados por Tauri; un
   argumento mal formado es un error antes de ejecutar nada);
2. las de Docker reciben el **servidor** y obtienen su gestor con
   `state.server(&server)?` (error `server.unknown` si no existe);
3. validan en Rust (identificadores, rutas, referencias, nombres);
4. llaman al motor con `with_engine`;
5. devuelven `CmdResult<T>` = `Result<T, CommandError>`.

| Grupo | Ejemplos de órdenes |
|---|---|
| Aplicación y configuración | `app_info`, `config_get`, `config_set`, `system_info`, `updates_check` |
| Conexión | `engine_state`, `engine_reconnect`, `engine_test`, `engine_overview` |
| Servidores remotos | `servers_list`, `server_save`, `server_connect`, `server_test`, `server_trust`, `server_shell_open` |
| Contenedores | `containers_list`, `container_action`, `container_preview`, `container_create`, `container_edit_plan`, `container_edit_apply` |
| Flujos | `logs_open`, `stats_open`, `terminal_open`, `terminal_input`, `terminal_resize`, `stream_close` |
| Imágenes | `images_list`, `image_pull_open`, `build_preview`, `build_open`, `image_remove`, `images_prune` |
| Redes y volúmenes | `network_create`, `network_connect`, `volumes_list`, `volume_sizes`, `binds_list` |
| Compose | `compose_projects`, `compose_config`, `compose_run_open`, `compose_file_save` |
| Eventos e historial | `events_recent`, `events_journal`, `events_query`, `history_list` |
| Herramientas | Auditoría, copias, monitorización y enlaces entre ellas |

El detalle de cada orden (argumentos, errores) está en `ARCHITECTURE.md`.

## Servicios

- **Servidores** (`servers.rs`): un `Manager` del motor por servidor (el
  local y cada remoto), creado bajo demanda. Un servidor caído no afecta a
  los demás.
- **Flujos** (`streams.rs`): registro de flujos abiertos con su tarea; al
  cerrar la ventana se cierran todos y se espera a que terminen (máximo 8 s).
- **Diarios** (`journal.rs`): historial de operaciones, eventos de Docker y
  auditorías en JSONL, con límite de entradas.
- **Monitorización** (`monitor.rs`): muestreo periódico, resumen por minuto
  en disco, alertas y notificaciones.
- **Copias programadas** (`systemd.rs` y `schtasks.rs`): crea, consulta,
  activa y elimina los temporizadores o tareas, siempre con autorización
  expresa del usuario y una marca que identifica los propios.
- **Llavero** (`secrets.rs`): Secret Service (D-Bus) en Linux,
  Administrador de credenciales en Windows; caché de sesión para los secretos
  que no se quieren recordar.

## Modelos de datos

Los tipos que cruzan el puente IPC derivan `Serialize`, `Deserialize` y `TS`
(se exportan a TypeScript). Algunos importantes:

| Tipo | Crate | Contenido |
|---|---|---|
| `ContainerSpec` | cmc-engine | Especificación completa de un contenedor (imagen, comando, entorno, puertos, montajes, recursos, opciones avanzadas) |
| `MountSpec` | cmc-engine | Montaje: tipo (volumen o bind), origen, destino, solo lectura |
| `Risk` | cmc-engine | Riesgo para el equipo: código, nivel, campo, clave para aceptarlo y parámetros |
| `ConnectionState`, `Diagnosis` | cmc-engine | Estado de la conexión y diagnóstico con candidatos y pasos |
| `ProjectRef`, `ComposeAction` | cmc-engine | Proyecto Compose (archivos, carpeta) y acción |
| `ServerConfig` | cmc-engine | Perfil SSH (host, puerto, usuario, autenticación, Docker remoto) |
| `OperationRecord` | cmc-activity | Operación registrada: transporte, petición, orden equivalente, resultado |
| `AppConfig` | cmc-store | Configuración persistente |

## Comunicación con Docker

`with_engine` obtiene el `Engine` conectado del servidor (o
`engine.notConnected`) y, si la operación falla por la conexión, avisa al
gestor para que pase a desconectado y empiece a reconectar:

```rust
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
```

Cada operación del `Engine` abre una entrada en el registro de actividad con
el **transporte** (`API` con método, ruta y socket; `CLI` con programa y
argumentos), el cuerpo de la petición (sin secretos) y la orden equivalente, y
la cierra con el resultado.

## Gestión de procesos

Ver [Gestión de procesos](#gestión-de-procesos) en el capítulo de
arquitectura. En el backend esto se traduce en:

- `cli::run` para órdenes cortas (con tiempo máximo, salida completa) y
  `cli::run_streaming` para órdenes largas con salida en vivo y cancelación
  (Compose);
- un `ProcessTree` por proceso: grupo de procesos en Linux, *job object* en
  Windows;
- procesos de SSH gestionados por la sesión (`ssh/session.rs`), que se
  cierran al desconectar sin dejar procesos huérfanos.

## Gestión de errores

`EngineError` distingue: `NotConnected`, `Docker { status, message }` (error
HTTP de Docker), `Timeout`, `Io`, `InvalidResponse`, `Process`,
`InvalidInput`, `InvalidSpec { issues }`, `RisksNotAccepted { risks }`,
`Busy` (otra operación sobre el mismo recurso), `NotConfirmed`,
`FileChanged`, `AlreadyExists`, `PathNotAllowed`, `ComposeUnavailable` y
`Other`. Las órdenes lo
convierten en `CommandError` con un **código estable** que la interfaz traduce.

Las operaciones sobre un mismo recurso se **serializan** (`ResourceLocks`):
dos acciones simultáneas sobre el mismo contenedor no se pisan; la segunda
recibe `engine.busy`.

## Seguridad en el backend

- **Validación**: identificadores de recursos (`resources.rs`), referencias de
  imagen con las reglas de Docker (`reference.rs`), rutas (`hostpath.rs`,
  reglas de Compose), nombres de servicios y perfiles, perfiles SSH.
- **Confirmación** en el backend de toda operación destructiva.
- **Riesgos** (`risk.rs`) calculados en Rust y aceptados uno a uno.
- **Secretos**: ocultación en el registro (`cmc-activity/redact.rs`),
  `zeroize` para las contraseñas en memoria, llavero del sistema.
- **Negativa a ejecutarse como root** en Linux y como administrador en Windows (`refuse_root`; en Windows consulta `TokenElevation`).

Reglas de ocultación (`crates/cmc-activity/src/redact.rs`):

```rust
/// Palabras que, en cualquier parte del nombre, lo hacen secreto.
pub const SECRET_NAME_WORDS: [&str; 7] = [
    "PASSWORD", "PASSWD", "SECRET", "TOKEN", "KEY", "CREDENTIAL", "PRIVATE",
];
/// Terminaciones de una parte del nombre que lo hacen secreto:
/// `DB_PASS`, `PGPASS`, `MYSQL_PWD`, `REDIS_AUTH`, `SENTRY_DSN`…
pub const SECRET_NAME_PART_ENDINGS: [&str; 5] = ["PASS", "PWD", "AUTH", "DSN", "COOKIE"];
```

## Persistencia

`cmc-store` lee y escribe `config.toml` y `servers.toml` con `toml` y
`serde`. Si el archivo está dañado, la aplicación arranca con los valores por
defecto y avisa (`startup_warning`) sin sobrescribirlo hasta que el usuario
cambie algo. Los diarios son JSONL (una entrada JSON por línea) con un máximo
de entradas: al superarlo un 25 %, se reescriben con las más recientes. La
escritura es atómica y, en Windows, se cierra el archivo abierto antes de
sustituirlo.

## Integración con el frontend

- Los tipos se exportan con `ts-rs` a `ui/src/shared/ipc/bindings/`; una
  prueba comprueba que están al día.
- Los eventos se emiten desde puentes (`events.rs`) que escuchan los canales
  internos del motor (`watch` y `broadcast` de tokio) y los reenvían a la
  ventana.
- Las pruebas de `ipc_tests.rs` ejercitan las órdenes como lo hace la
  interfaz, con el runtime simulado de Tauri (161 pruebas, también compiladas
  para Windows).

## Flujos importantes, paso a paso

### Arranque

1. `main.rs` comprueba los modos sin ventana: askpass (lo invoca `ssh`),
   `--migrate`, `--remove-scheduled-tasks` (Windows) y `--run-backup`.
2. `run()` se niega a seguir como root (Linux) o como administrador (Windows), migra los datos del nombre
   anterior si los hay, carga la configuración, crea el `AppState` y la
   ventana.
3. El gestor del equipo local empieza a conectar con Docker (siguiente
   capítulo) y emite `engine://state`.

### Crear un contenedor

![Creación de un contenedor: vista previa sin Docker y creación validada.](diagramas/06-crear-contenedor.pdf){width=92%}

### Registros y terminal

![Flujos de registros y de terminal entre la interfaz, Rust y Docker.](diagramas/07-registros-terminal.pdf){width=92%}
