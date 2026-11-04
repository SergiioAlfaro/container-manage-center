# Arquitectura de Docker Control Center

Versión 0.2.0 (Fase 2).

## Visión general

```
┌──────────────────────── Interfaz (React + TypeScript, WebKitGTK) ─────────────────────┐
│ Sidebar │ Dashboard · Contenedores · Imágenes · Configuración │ Panel de actividad    │
│         TanStack Query (caché e invalidación) · Zustand (estado de UI y conexión)     │
│              ▲ invoke() tipado                ▲ eventos activity://event, engine://state
└──────────────┼────────────────────────────────┼───────────────────────────────────────┘
┌──────────────┴──────────────── Backend (Rust, tokio) ─────────────────────────────────┐
│ src-tauri: comandos IPC · AppState · puentes de eventos · negativa a root             │
│   ├── dcc-store     config.toml (UI, actividad, conexión Docker)                      │
│   ├── dcc-activity  registro de operaciones reales ──► broadcast ──► ventana          │
│   └── dcc-engine                                                                      │
│         endpoint   candidatos: DOCKER_HOST, contexto, rootless, sistema / personalizado│
│         probe      existe · es socket · permiso · alguien escucha                     │
│         manager    conexión, diagnóstico, latido, reconexión, with_engine()           │
│         client     Engine: bollard + registro (ping, version, info)                   │
│         containers listado y ciclo de vida      images  listado y uso                 │
│         resources  validación de IDs, rutas exactas, bloqueo por recurso              │
│         cli        procesos sin shell (docker compose)                                │
└──────────────┬───────────────────────────────────────────────┬───────────────────────┘
      socket Unix de Docker (API)                      docker compose (proceso CLI)
```

## Estructura de directorios

```
docker-control-center/
├── Cargo.toml                    Workspace (versión 0.2.0)
├── crates/
│   ├── dcc-activity/             Registro de operaciones
│   ├── dcc-store/                Configuración persistente
│   └── dcc-engine/
│       ├── src/{endpoint,probe,sys,client,manager,containers,images,resources,cli,error,model}.rs
│       ├── src/bin/dcc-diagnose.rs   Diagnóstico desde terminal
│       └── tests/live_docker.rs      Integración contra Docker real
├── src-tauri/src/
│   ├── commands/{app,config,activity,engine,resources}.rs
│   ├── events.rs  state.rs  error.rs  lib.rs  ipc_tests.rs
├── ui/src/
│   ├── app/                      App, Sidebar, ErrorBoundary, estado global
│   ├── features/
│   │   ├── activity/             Panel de actividad
│   │   ├── engine/               Estado de conexión, diagnóstico, indicador
│   │   ├── dashboard/  containers/  images/  settings/  pending/
│   ├── shared/                   ipc (cliente y tipos generados), i18n, query, format, ui
│   └── test/                     Dobles y fixtures SOLO para pruebas
└── scripts/                      install-build-deps.sh, verify-phase-1.sh, verify-phase-2.sh
```

## dcc-engine

### Resolución del endpoint (`endpoint.rs`)
Modo **automático**, en este orden y sin duplicados (`/var/run` ≡ `/run`):

1. `DOCKER_HOST` si es `unix://…` (con `tcp://`/`ssh://` se anota `unsupportedHost` y se siguen probando los demás).
2. Contexto activo: `DOCKER_CONTEXT` o `currentContext` de `~/.docker/config.json` (o `$DOCKER_CONFIG`), leyendo `contexts/meta/<sha256(nombre)>/meta.json`. Así se detecta el contexto `rootless` que crea el instalador de Docker rootless.
3. Rootless: `$XDG_RUNTIME_DIR/docker.sock`, o `/run/user/<uid>/docker.sock` si la variable no existe (aplicaciones lanzadas desde el menú). No se prueba para root.
4. Sistema: `/var/run/docker.sock`.

Modo **personalizado**: solo la ruta configurada (`unix:///ruta` o ruta absoluta; se rechazan rutas relativas, vacías, con `..` o con NUL).

### Sondeo (`probe.rs`) y diagnóstico (`manager.rs`)
Cada candidato se comprueba antes de hablar HTTP: `Missing`, `NotASocket`, `PermissionDenied` (EACCES/EPERM), `Refused` (archivo sin proceso), `Reachable`. Con `Reachable` se hace el *handshake*: `GET /_ping` (debe responder `OK`), `GET /version` (API ≥ 1.41) y `GET /info` (para detectar rootless por `SecurityOptions`).

Si ningún candidato conecta, el `Problem` elegido es, por prioridad: ruta inválida → host no soportado → el problema del primer candidato que existe → `noSocket`. El `Diagnosis` incluye todos los candidatos, la instalación detectada (`docker`, `dockerd`, herramientas rootless) y el usuario. Para `permissionDenied` se leen `/etc/passwd` y `/etc/group` y `getgroups()` para distinguir "no está en el grupo" de "está, pero la sesión no lo tiene activo". No se ejecuta ningún programa para diagnosticar.

### Monitor
- Conectado: latido `GET /_ping` silencioso cada 10 s. Si falla, se confirma con un ping registrado y se rediagnostica.
- Desconectado: reintento silencioso con espera de 2 s a 30 s.
- Una operación que falla por conexión (`Io`, `Timeout`) despierta al monitor (`with_engine`).
- El estado se publica por `tokio::sync::watch` solo si cambia de forma significativa.

### Cliente y registro (`client.rs`)
`Engine::call(ApiOp, futuro, resumen)` abre el registro con la petición exacta (método, ruta, socket, comando equivalente con `--host` si el socket no es el del sistema), ejecuta y cierra con el resultado o el error de Docker (código HTTP incluido). bollard no expone el código de las respuestas correctas, así que no se registra uno inventado. La validación de la respuesta (p. ej. ping distinto de `OK`) ocurre **dentro** de la operación registrada.

### Contenedores e imágenes
| Operación | Petición real | Comando equivalente |
|---|---|---|
| Listar contenedores | `GET /containers/json?all=true&size=false` | `docker ps --all` |
| Iniciar | `POST /containers/{id}/start` | `docker start {id}` |
| Detener | `POST /containers/{id}/stop?t=10` | `docker stop --time 10 {id}` |
| Reiniciar | `POST /containers/{id}/restart?t=10` | `docker restart --time 10 {id}` |
| Pausar / reanudar | `POST /containers/{id}/pause` · `/unpause` | `docker pause/unpause {id}` |
| Eliminar | `DELETE /containers/{id}?v=…&force=…&link=false` | `docker rm [--force] [--volumes] {id}` |
| Listar imágenes | `GET /images/json?all=false&shared-size=false&digests=false&manifests=false` + listado de contenedores para el uso | `docker image ls` |
| Versión de Compose | proceso `docker compose version --format json` (CLI) | — |

## Comandos IPC

| Comando | Descripción |
|---|---|
| `app_info`, `config_get`, `config_set` | Información y configuración. `config_set` reconecta en segundo plano si cambia la conexión |
| `activity_list`, `activity_clear` | Registro de actividad |
| `engine_state` | Estado de la conexión (`connecting` / `connected` / `disconnected` con diagnóstico) |
| `engine_reconnect` | Detectar y conectar de nuevo (registrado) |
| `engine_test(docker)` | Probar una configuración sin guardarla ni sustituir la conexión |
| `engine_overview` | Versión e información del motor |
| `compose_status` | Versión del plugin Compose |
| `containers_list`, `container_action(id, name, action)` | Contenedores |
| `container_remove(id, name, force, volumes, confirmed)` | Exige `confirmed = true` |
| `images_list` | Imágenes con su uso |

Errores: `CommandError { code, message, httpStatus }` con códigos `engine.notConnected`, `engine.docker`, `engine.timeout`, `engine.io`, `engine.invalidResponse`, `engine.invalidInput`, `engine.busy`, `engine.notConfirmed`, `engine.process`, `engine.other`, `config.io`.

Eventos: `activity://event`, `activity://resync`, `engine://state`.

## Interfaz
- `features/engine/store.ts`: estado de conexión; `App` lo alimenta con `engine_state` y el evento `engine://state`. Al pasar a conectado se invalidan todas las consultas; al perder la conexión se descartan.
- `RequireConnection` envuelve los apartados que necesitan Docker.
- `diagnosis.ts` traduce el diagnóstico en pasos concretos (función pura, probada caso a caso).
- Todas las peticiones son asíncronas; una operación en curso bloquea solo su fila.

## Pruebas

| Capa | Herramienta | Cantidad |
|---|---|---|
| Rust unitarias | `cargo test` | 22 actividad · 55 motor · 15 configuración |
| Integración IPC (runtime simulado de Tauri) | `cargo test -p docker-control-center` | 18 (una contra Docker real) |
| Integración contra Docker real | `DCC_DOCKER_TESTS=1 cargo test -p dcc-engine --test live_docker` | 11 (2 requieren usuario no root) |
| Interfaz | Vitest + Testing Library | 72 |
| Estática | clippy `-D warnings`, `cargo fmt --check`, `tsc` estricto, shellcheck | — |
| Manual | `scripts/verify-phase-2.sh` | M1–M15 |
