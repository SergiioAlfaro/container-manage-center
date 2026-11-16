# Arquitectura general

## Visión de conjunto

CMC es una aplicación **Tauri 2**: un único proceso de Rust que abre una
ventana con el motor web del sistema y sirve en ella una interfaz web (React).
La interfaz no tiene acceso al sistema: todo lo que necesita lo pide a Rust por
el puente **IPC** de Tauri, y Rust es quien habla con Docker, con OpenSSH y con
los servicios del sistema.

![Arquitectura general de Container Manage Center. Las cajas corresponden a carpetas y crates reales del repositorio.](diagramas/01-arquitectura.pdf){width=85%}

Las piezas son:

| Pieza | Ubicación | Responsabilidad |
|---|---|---|
| Interfaz | `ui/` | Pantallas, formularios, estado de la interfaz y traducciones. No ejecuta nada en el sistema |
| Aplicación | `src-tauri/` | 130 órdenes IPC, estado global (`AppState`), servidores, flujos, diarios en disco, servicios del sistema (temporizadores, llavero, notificaciones) |
| Motor | `crates/cmc-engine/` | Conexión y diagnóstico de Docker, recursos, Compose, SSH, copias, auditoría, monitorización, riesgos |
| Almacén | `crates/cmc-store/` | `config.toml`, perfiles de servidores, diarios JSONL, migración desde el nombre anterior |
| Actividad | `crates/cmc-activity/` | Registro de cada operación, ocultación de secretos, orden equivalente |

## Tauri

Tauri aporta tres cosas:

1. **La ventana** con el motor web del sistema: WebKitGTK 4.1 en Linux y
   Microsoft Edge WebView2 en Windows. La interfaz compilada (`dist/`) se
   incluye dentro del ejecutable.
2. **El puente IPC**: la interfaz llama a `invoke("orden", argumentos)` y Rust
   ejecuta la función marcada con `#[tauri::command]`; además, Rust emite
   **eventos** (`engine://state`, `activity://event`…) y **canales** para los
   flujos (registros, terminales).
3. **El empaquetado**: paquete `.deb` en Linux e instalador NSIS en Windows.

Los permisos de la ventana están reducidos al mínimo
(`src-tauri/capabilities/default.json`): escuchar eventos, el diálogo de abrir
archivos, el portapapeles y abrir un único enlace (el perfil del autor). Las
órdenes propias validan sus argumentos en Rust.

## Rust

Todo el trabajo con el sistema está en Rust, con **tokio** como motor
asíncrono. Las ventajas para este proyecto:

- tipos estrictos en todo el recorrido (petición de la interfaz → validación →
  petición a Docker), con los tipos de la interfaz **generados** desde Rust con
  `ts-rs` (carpeta `ui/src/shared/ipc/bindings/`);
- concurrencia sin bloquear la interfaz: cada operación es una tarea asíncrona;
- procesos externos sin shell (`Command` con argumentos separados).

## React, TypeScript y Vite

La interfaz es una aplicación de una sola página (React 19 + TypeScript 5.8),
compilada con **Vite 7** y estilada con **Tailwind CSS 4**. Los datos de Docker
se guardan en caché con **TanStack Query**; el estado propio de la interfaz
(apartado, servidor elegido, sesiones de terminal) con **Zustand**. El
capítulo [Frontend](#frontend) la describe en detalle.

## Comunicación entre la interfaz y Rust

![Recorrido de una operación desde la interfaz hasta Docker y vuelta.](diagramas/02-ipc.pdf){width=95%}

1. Un componente pide datos o lanza una acción con TanStack Query.
2. El cliente IPC (`ui/src/shared/ipc/client.ts`) llama a `invoke()` con el
   nombre de la orden y sus argumentos; todas las órdenes de Docker llevan el
   **servidor** (`local` o el identificador de un servidor remoto).
3. La orden de Rust valida los argumentos, obtiene el gestor de ese servidor y
   ejecuta la operación con `with_engine`.
4. El motor registra la operación en el **registro de actividad**, hace la
   petición a Docker y registra el resultado.
5. La orden devuelve el valor o un `CommandError` (código estable, mensaje,
   estado HTTP y problemas de validación).
6. La interfaz traduce el error y actualiza o invalida las consultas.

Fragmento real de una orden (`src-tauri/src/commands/resources.rs`):

```rust
#[tauri::command]
pub async fn container_action(
    state: State<'_, AppState>,
    server: String,
    id: String,
    name: Option<String>,
    action: ContainerAction,
) -> CmdResult<ContainerState> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.container_action(&id, name.as_deref(), action).await })
        .await?)
}
```

Y su llamada desde la interfaz (`ui/src/shared/ipc/client.ts`):

```typescript
containerAction: (id: string, name: string | null, action: ContainerAction) =>
  call<ContainerState>("container_action", { id, name, action }),
```

## Integración con Docker Engine

El motor usa la **API de Docker Engine** (HTTP sobre un socket Unix, una
tubería con nombre de Windows o un canal SSH) con la biblioteca **bollard**, y
un *dialer* propio para las peticiones que bollard no cubre como necesita la
aplicación (construcción, eventos, tamaños de volúmenes, creación de redes).
Solo se usa un **proceso** externo para lo que no tiene API: **Docker Compose**
(`docker compose …`) y, en los servidores remotos, `ssh`. El capítulo
[Integración con Docker](#integración-con-docker) lo detalla.

## Gestión de procesos

Los procesos externos (`docker compose`, `ssh`, `ssh-keygen`, `trivy`/`grype`,
`systemctl`, `schtasks`) se lanzan desde `cmc-engine/src/cli.rs` y módulos
similares, siempre:

- **sin shell**: el programa y cada argumento por separado, así que `$HOME`,
  `*` o `;` llegan literalmente;
- con la ruta completa del programa (buscada en el `PATH` y, en Windows, con
  `PATHEXT` y la carpeta de Docker Desktop);
- con **tiempo máximo** y **cancelación**: en Linux, el proceso es líder de su
  grupo y al detenerlo se avisa a todo el grupo (SIGTERM y, tras un plazo,
  SIGKILL); en Windows, el proceso entra en un *job object* y se termina el
  árbol completo;
- en Windows, sin ventana de consola (`CREATE_NO_WINDOW`);
- registrados en el panel de actividad como una operación **CLI** con su
  salida (con los secretos ocultos).

## Sistema de eventos

Rust envía a la interfaz:

| Evento o canal | Contenido |
|---|---|
| `engine://state` · `server://state` | Estado de la conexión de cada servidor (conectando, conectado, desconectado con diagnóstico) |
| `activity://event` · `activity://resync` | Altas y cambios del registro de actividad |
| `docker://event` | Cada evento de Docker (contenedor creado, iniciado, imagen descargada…) |
| `updates://result` | Resultado de la comprobación de versiones al iniciar |
| Canales por flujo | `LogEvent`, `StatsEvent`, `TerminalEvent`, `ComposeEvent`, `PullEvent`, `BuildEvent` |

Los eventos de Docker sirven también para **actualizar la interfaz sola**:
cada evento invalida las consultas afectadas (agrupadas cada 250 ms y como
mucho una vez por segundo durante las ráfagas), de modo que lo que se hace
desde la terminal aparece al momento.

## Gestión de errores

- En Rust, cada módulo tiene su tipo de error (`EngineError`, `StoreError`,
  `KeyringError`…) con `thiserror`. Las órdenes los convierten en
  `CommandError { code, message, httpStatus, issues }`.
- Los **códigos** son estables (`engine.notConnected`, `engine.invalidSpec`,
  `engine.notConfirmed`, `engine.fileChanged`…) y la interfaz los traduce a
  los ocho idiomas; el mensaje de Docker se muestra tal cual cuando aporta
  información.
- Las validaciones devuelven una lista de **problemas** (`issues`) con el
  campo y un código, para marcar el campo concreto del formulario.
- Las operaciones destructivas exigen `confirmed = true` en el backend, además
  del diálogo de la interfaz (defensa en profundidad).
- Una `ErrorBoundary` de React impide que un error de la interfaz deje la
  ventana en blanco.

## Persistencia

| Archivo | Carpeta (Linux / Windows) | Contenido |
|---|---|---|
| `config.toml` | `~/.config/container-manage-center/` · `%APPDATA%\container-manage-center\` | Preferencias, conexión con Docker, proyectos Compose, monitorización, actualizaciones |
| `servers.toml` | Igual | Perfiles de servidores remotos (sin secretos) |
| `known_hosts` | Igual | Huellas de los servidores aceptadas en la aplicación |
| `copias.json` | Igual | Tareas de copia y carpeta de destino |
| `historial.jsonl` · `eventos.jsonl` · `auditorias.jsonl` | `~/.local/state/container-manage-center/` · `%LOCALAPPDATA%\container-manage-center\` | Historial de operaciones, diario de eventos y auditorías |
| `monitor/<servidor>/…` | Igual | Resumen por minuto y alertas de la monitorización |
| Contraseñas y frases de clave | Llavero del sistema (Secret Service) · Administrador de credenciales | Nunca en archivos |

Los archivos se escriben de forma **atómica** (archivo temporal y cambio de
nombre) y, en Linux, solo para el usuario (carpetas 0700, archivos 0600).
Los diarios tienen un número máximo de entradas y descartan las líneas
corruptas al leerlos.

## Seguridad

La seguridad forma parte de la arquitectura, no es una capa aparte: validación
en Rust de todo lo que llega de la interfaz, ocultación de secretos en el
registro de actividad, credenciales en el almacén del sistema, verificación
estricta de la clave de los servidores SSH, detección de riesgos antes de
crear contenedores y negativa a ejecutarse como administrador. El capítulo
[Seguridad](#seguridad) lo desarrolla.
