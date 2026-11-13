# Arquitectura de Docker Control Center

Versión 0.7.1 (Fase 7 con correcciones).

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
│         compose    detección (D-032), proyectos, acciones, archivos (Fase 7)          │
│         events     suscripción en vivo y consulta de eventos (Fase 8, D-057)          │
│         cli        procesos sin shell, salida en vivo y cancelación por grupo         │
│         ssh        servidores remotos: sesión maestra OpenSSH, canales dial-stdio     │
└──────────────┬───────────────────────────────────────────────┬───────────────────────┘
      socket Unix de Docker (API)                      docker compose (proceso CLI)
      ssh … docker system dial-stdio (servidor remoto)   ssh … -- docker compose …
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
| Estado confirmado tras cada acción (D-031) | `GET /containers/{id}/json?size=false` | `docker container inspect --format '{{.State.Status}}' {id}` |
| Versión de Compose (D-032) | proceso `docker compose version` y, si falta el plugin, `docker-compose version` (CLI, sin opciones) | — |
| Crear contenedor (D-034) | `POST /containers/create?name=…&platform=` con el cuerpo de `ContainerSpec::to_create_body` | `docker create …` (`ContainerSpec::cli_args`, secretos ocultos, D-039) |
| Inspeccionar | `GET /containers/{id}/json?size=false` | `docker container inspect {id}` |
| Configuración de la imagen | `GET /images/{ref}/json` | `docker image inspect {ref}` |
| Redes / volúmenes (para el asistente) | `GET /networks` · `GET /volumes` | `docker network ls` · `docker volume ls` |
| Logs (flujo, D-036) | `GET /containers/{id}/logs?follow=true&stdout=true&stderr=true&timestamps=true&tail=N` | `docker logs --follow --timestamps --tail N {id}` |
| Estadísticas (flujo) | `GET /containers/{id}/stats?stream=true&one-shot=false` | `docker stats {id}` |
| Terminal (D-036, D-037) | `POST /containers/{id}/exec` → `POST /exec/{id}/start` (conexión secuestrada) → `POST /exec/{id}/resize` (no registrado, contado) → `GET /exec/{id}/json` | `docker exec --interactive --tty {id} <shell>` |
| Renombrar (0.9.2, D-070) | `POST /containers/{id}/rename?name=…` | `docker rename {id} {nombre}` |
| Cambiar reinicio y límites (0.9.2) | `POST /containers/{id}/update` con `RestartPolicy`, `NanoCpus`/`CpuQuota`, `Memory`, `MemorySwap`, `MemoryReservation` | `docker update …` |
| Recrear (0.9.2, D-070) | `POST /containers/{id}/stop` → `GET /containers/{id}/json?size=true` → `rename` a la copia → `POST /containers/create` (cuerpo construido desde el `inspect` completo, registrado con los secretos ocultos) → `POST /networks/{red}/connect` → `start` → comprobación; vuelta atrás con `DELETE /containers/{nuevo}?v=false&force=true`, `rename` y `start` | resumen `container.recreate` (APP) y cada paso como operación API |
| Crear imagen (0.9.2, D-069) | `POST /commit?container=…&repo=…&tag=…&author=…&comment=…&pause=…` | `docker commit [--author] [--message] [--pause=false] {id} {repo}:{tag}` |

### Imágenes (Fase 4)
| Operación | Petición real | Comando equivalente |
|---|---|---|
| Descargar (flujo, D-042) | `POST /images/create?fromImage=<nombre completo>&tag=<etiqueta>&platform=` + `X-Registry-Auth` si hay credenciales | `docker pull <ref>` |
| Obtener credenciales | proceso `docker-credential-<helper> get` (sin shell; la salida no se registra) | — |
| Buscar | `GET /images/search?term=…&limit=N[&filters=…]` | `docker search --limit N [--filter is-official=true] <término>` |
| Inspeccionar | `GET /images/{ref}/json` | `docker image inspect <ref>` |
| Capas | `GET /images/{ref}/history` | `docker image history --no-trunc <ref>` |
| Etiquetar | `POST /images/{ref}/tag?repo=…&tag=…` | `docker tag <ref> <repo>:<tag>` |
| Eliminar | `DELETE /images/{ref}?force=…&noprune=false` | `docker image rm [--force] <ref>` |
| Limpiar | `POST /images/prune?filters={"dangling":["true"\|"false"]}` | `docker image prune --force [--all]` |
| Construir (flujo, D-041) | `POST /build?dockerfile=…&t=…&buildargs=…&labels=…&target=…&nocache=1&pull=1&rm=1[&forcerm=1]&version=2\|1`, cuerpo tar en flujo (petición propia, `build/http.rs`) | `docker build …` / `DOCKER_BUILDKIT=0 docker build … --force-rm` |

Módulos: `reference.rs` (gramática de referencias de `distribution/reference`), `registry_auth.rs` (credenciales, D-042), `pull.rs` (`PullTracker`: progreso por capa), `build/` (`ignore.rs` .dockerignore, `context.rs` recorrido y tar en flujo, `progress.rs` protobuf de BuildKit y salida plana, `http.rs` petición HTTP/1.1 sobre el socket, `mod.rs` `BuildSpec`).

### Redes (Fase 5, `networks.rs`)
| Operación | Petición real | Comando equivalente |
|---|---|---|
| Listar | `GET /networks` + `GET /containers/json?all=true` (para saber qué contenedores, en marcha o no, tienen cada red, D-047) | `docker network ls` |
| Inspeccionar | `GET /networks/{id}` + lista de contenedores (los detenidos no aparecen en el inspect) | `docker network inspect <nombre>` |
| Crear | `POST /networks/create` con el cuerpo de `NetworkSpec::wire_body()` (incluye `CheckDuplicate: true`, petición propia por `build/http.rs`, D-048) | `docker network create --driver … [--subnet … --ip-range … --gateway …] [--label …] [--opt …] [--ipv6] [--internal] [--attachable] <nombre>` |
| Eliminar | `DELETE /networks/{id}` | `docker network rm <nombre>` |
| Limpiar | `POST /networks/prune?` (filtro de etiqueta solo en pruebas) | `docker network prune --force` |
| Conectar | `POST /networks/{id}/connect` con `{Container, EndpointConfig{Aliases, IPAMConfig}}` | `docker network connect [--alias …] [--ip …] [--ip6 …] <red> <contenedor>` |
| Desconectar | `POST /networks/{id}/disconnect` con `{Container[, Force]}` | `docker network disconnect [--force] <red> <contenedor>` |

Las operaciones usan siempre el ID de la red (Docker acepta nombres con espacios creados por otras herramientas); el nombre solo se usa para mostrar y en el comando equivalente. `NetworkSpec` es la fuente única de la validación (nombre, driver, subredes CIDR con la misma regla de dirección de red que Docker, puerta de enlace y rango dentro de la subred, familia IPv4/IPv6, etiquetas y opciones), del cuerpo y del comando, como `ContainerSpec` (D-034). `ConnectOptions` valida alias e IP fija (dentro de las subredes conocidas de la red). Eliminar y desconectar exigen confirmación; conectar y desconectar bloquean el contenedor (`container:{id}`), eliminar bloquea la red (`network:{id}`).

### Volúmenes y directorios vinculados (Fase 6, `volumes.rs`, `binds.rs`)
| Operación | Petición real | Comando equivalente |
|---|---|---|
| Listar volúmenes | `GET /volumes` + `GET /containers/json?all=true` (montajes de cada contenedor) | `docker volume ls` |
| Inspeccionar | `GET /volumes/{nombre}` + lista de contenedores | `docker volume inspect <nombre>` |
| Tamaños (bajo demanda) | `GET /system/df?type=volume` (petición propia, lee los formatos de Docker ≤ 28 y ≥ 29, D-051) | `docker system df --verbose` |
| Crear | `GET /volumes?filters={"name":[…]}` (comprobar si existe) + `POST /volumes/create` | `docker volume create --driver … [--opt …] [--label …] [nombre]` |
| Eliminar | `DELETE /volumes/{nombre}` | `docker volume rm <nombre>` |
| Limpiar | `POST /volumes/prune?filters={"all":["true"]}` (sin filtro: solo anónimos) | `docker volume prune --force [--all]` |
| Directorios vinculados | `GET /containers/json?all=true`; el estado de cada ruta se consulta en el sistema de archivos local (no es una operación de Docker) | — |

`ContainerSummary` incluye ahora `mountPoints` (los `Mounts` de la lista de contenedores). `VolumeSpec` es la fuente única de la validación (regla de nombres del driver local, driver, opciones y etiquetas), del cuerpo y del comando. `binds.rs` agrupa los montajes `bind` por ruta del equipo, comprueba si existen (tipo, propietario, permisos, lectura) y marca los riesgos (socket de Docker, raíz, directorios del sistema). Docker no permite añadir montajes a un contenedor existente: vincular una carpeta abre el asistente de contenedores con el montaje (D-052).

### Docker Compose (Fase 7, `compose/`, D-053, D-054, D-055)
Todas las operaciones ejecutan el CLI oficial (`docker compose …` o `docker-compose …`, según la detección de D-032, guardada en `EngineManager::compose_cli`) con `cli::run_streaming`: sin shell, argumentos separados, `DOCKER_HOST` del Docker conectado, directorio de trabajo vacío y propio, proceso en su propio grupo (cancelar = SIGTERM al grupo y SIGKILL a los 8 s), salida línea a línea en lotes y **un** registro de actividad por ejecución.

| Operación | Ejecución real |
|---|---|
| Proyectos | `GET /containers/json?all=true` (etiquetas `com.docker.compose.*`) + proyectos guardados en la configuración |
| Modelo y validación | `compose --ansi never -p P -f … --project-directory D --profile "*" config --format json` (con `-f -` y el contenido por la entrada estándar para validar sin guardar). La salida no se guarda en el registro: contiene los valores de las variables |
| Nombre al añadir | `compose --ansi never -f … --project-directory D config --format json` (sin `-p`) |
| Desplegar | `… [--profile X] up --detach [--build] [--pull always\|never] [--force-recreate] [--remove-orphans] [-- servicios]` |
| Eliminar | `… --profile "*" down [--volumes] [--rmi local\|all] [--remove-orphans]` (exige `confirmed`) |
| Iniciar / detener / reiniciar | `… --profile "*" start\|stop\|restart [-- servicios]` (sin `-f` si los archivos ya no existen) |
| Descargar / construir | `… pull [-- servicios]` · `… build [--no-cache] [--pull] [-- servicios]` |
| Registros | `… logs --follow --no-color --tail N [--timestamps] [-- servicios]` (las líneas no se guardan en el registro) |
| Archivos | lectura, guardado atómico con huella SHA-256 y creación con `O_EXCL` en `compose/file.rs` (no son operaciones de Docker) |

Tras desplegar, iniciar, detener o reiniciar, `compose_run` confirma con inspect cada contenedor del proyecto (`compose_settle`, operaciones "Comprobar estado del contenedor"): con Docker 26 el listado puede ir un instante por detrás de la respuesta de Docker (D-056, como D-031).

`project.rs` contiene la identidad (`ProjectRef`), las reglas de nombres (proyecto, servicio y perfil: nunca empiezan por `-` y los servicios van tras `--`), el descubrimiento (`projects_from`), la lectura del modelo (`parse_model`, solo nombres de variables) y los argumentos de cada acción (`action_args`), todo con pruebas unitarias. `ops.rs` ejecuta: `compose_config`, `compose_resolve`, `compose_check` (validación, confirmación, archivos y cerrojo antes de ejecutar nada), `compose_run` (cerrojo `compose:NOMBRE`) y `compose_logs`.

### Actualizaciones y sistema (Fase 9, `src-tauri/src/updates.rs`, D-060)

- `UpdateSource::parse`: `propietario/repositorio` de GitHub (a `api.github.com/…/releases/latest`), `https://…` o `http://` hacia la propia máquina. Todo lo demás se rechaza sin conectarse.
- `fetch`: `reqwest` 0.12 con `rustls` (motor `ring`), certificados del sistema y `HTTPS_PROXY`. Sin proxy para la propia máquina. Las redirecciones solo a `https`, 15 s de espera y respuestas de 1 MiB como máximo.
- `parse_release`: lee el formato de GitHub (`tag_name`, `html_url`, `body`) o el propio (`version`, `url`, `notes`). Rechaza borradores y versiones preliminares, y solo admite enlaces `https`.
- `compare`: compara versiones con semver.
- `commands/app.rs`:
  - `spawn_startup_check`: si está activada, comprueba 5 s después de arrancar y envía `updates://result`.
  - `system_info`: lee `/etc/os-release`, `/proc/sys/kernel/osrelease`, `gnu_get_libc_version`, `tauri::webview_version` y la base de datos de dpkg (solo lectura).
  - `open_release_page`: abre el enlace con `xdg-open`, sin shell y solo para `https`.

### Paquete (D-059)

- `tauri build --bundles deb` genera el paquete.
- `scripts/deb-set-depends.sh` sustituye `Depends` por lo que calcula `dpkg-shlibdeps` sobre el binario.
- `scripts/build-deb-compat.sh` compila en un contenedor de Ubuntu 22.04 (`packaging/build/Dockerfile.ubuntu2204`):
  - En el equipo: `npm run build` y `cargo fetch` a `target/compat/cargo-home`.
  - En el contenedor: `cargo-1.91 build --offline --locked`, sin red y con el usuario que lo ejecuta.
  - En el equipo: `tauri bundle` con ese binario.
  - En el contenedor: `deb-set-depends.sh` contra las bibliotecas de Ubuntu 22.04.

### Eventos (Fase 8, `events.rs`, D-057) e historial (D-058)

| Operación | Llamada real |
|---|---|
| Suscripción en vivo (`events_watch`) | `GET /events` con la petición HTTP propia (`build::http::get_stream`): un objeto JSON por evento; se registra cuando Docker la acepta y, si se interrumpe, una operación fallida aparte |
| Consulta (`events_query`) | `GET /events?since=S&until=U&filters={type,event,…}` (con `until` en nanosegundos si es ahora o futuro); validada antes de llamar a Docker; máximo 5000 |

`src-tauri/src/journal.rs`:
- `spawn_events_watcher` mantiene una suscripción mientras el estado es `Connected` (se cancela al desconectar y se reintenta a los 3 s si Docker la corta). Cada evento se guarda en memoria (últimos 2000), en el diario si `keep_history`, y se emite como `docker://event`.
- `spawn_history_bridge` escucha el registro de actividad y guarda cada operación terminada como `HistoryEntry` (`dcc-activity/history.rs`: sin cuerpo, comando equivalente ni salidas; `buildargs` ocultos).
- Los diarios son `dcc_store::journal::Journal<T>`: JSONL, carpeta 0700 y archivos 0600, líneas dañadas descartadas y contadas, compactación al abrir y al superar el máximo en un 25 %, rechazo de enlaces simbólicos. Carpeta: `DCC_STATE_DIR`, `$XDG_STATE_HOME/docker-control-center` o `estado/` junto a una configuración indicada con `DCC_CONFIG`.

### Auditoría de seguridad (1.3.0, `audit/`, D-093–D-095)
`Engine::audit` lee `docker info`, los contenedores (`inspect_raw`), las imágenes (`image_details`) y `compose config` de los proyectos, y devuelve un `AuditReport`. Las comprobaciones son funciones puras (`audit/checks.rs`) que reutilizan `risk` y añaden el endurecimiento. `audit/vuln.rs` detecta Trivy y Grype, lee la fecha de su base y analiza sin red. En la aplicación, `commands/audit.rs` ejecuta la auditoría como un flujo cancelable (`audit_run_open`), oculta los secretos del informe y lo guarda en `auditorias.jsonl`. `audit_export_save` abre el diálogo de guardar desde Rust y escribe el archivo con permisos 0600. La interfaz (`ui/src/features/audit/`) genera el Markdown y el HTML en su idioma y propone correcciones a «Editar configuración» (`applyAuditFixes`).

### Especificación de contenedor (`spec.rs`, D-034)
`ContainerSpec` → `validate(check_fs)` (problemas `SpecIssue { severity, field, code, params }`), `to_create_body()`, `cli_args(run)`, `redacted()`. `from_inspect()` construye el `DuplicateSpec` (D-038). `details.rs` convierte la respuesta de inspect en `ContainerDetails` (incluye el JSON completo). `streams.rs` implementa los flujos: `LineAssembler` (fragmentos, UTF-8 y marca de tiempo), `sample_from` (fórmulas de `docker stats`) y `run_terminal`.

### Flujos en la aplicación (`src-tauri/src/streams.rs`)
`StreamRegistry` guarda, por identificador, el `CancellationToken` y, para la terminal, el canal de entrada. `logs_open`/`stats_open`/`terminal_open` devuelven el identificador y envían los eventos por un `Channel` de Tauri; al terminar mandan `end` con `StreamClosed { end: StreamEnd { reason, summary, exitCode } | error }` y se retiran del registro. `stream_close` cancela; al destruirse la ventana se cancelan todos.

## Administración remota (0.10.0, D-075–D-081)

### Transporte (`dcc-engine/src/ssh/`, D-075)
- `SshSession` (`session.rs`): una sesión maestra de OpenSSH por servidor (`ControlMaster=yes`, `ControlPersist=no`) con un socket de control `c-<pid>-<servidor>-<aleatorio>` en `$XDG_RUNTIME_DIR/docker-control-center/ssh` (0700). Cada canal de Docker es `ssh -o ControlMaster=no -o ControlPath=… -T -- host docker system dial-stdio` (argumentos entrecomillados uno a uno cuando hace falta) multiplexado por esa sesión; bollard lo usa como transporte propio (`connect.rs`). `close_orphans()` cierra al arrancar solo las sesiones de PID que ya no existen.
- `args.rs`: las opciones de seguridad (`StrictHostKeyChecking=yes` con la lista de huellas de la aplicación y, solo para leer, la del usuario; `UpdateHostKeys=no`; `BatchMode`/`IdentitiesOnly` según el método; sin reenvíos de agente, X11 ni puertos; `PermitLocalCommand=no`; `ServerAlive*` para detectar cortes) van siempre antes del destino y de `--`; los argumentos remotos se entrecomillan uno a uno (`shell_quote`). Nunca se construye una orden concatenando entradas del usuario.
- `problem.rs` clasifica los fallos (`RemoteProblem`: DNS, inaccesible, rechazado, tiempo, autenticación, huella desconocida o cambiada, Docker ausente, detenido, sin permiso, API incompatible…); `connect.rs` hace las siete comprobaciones de **Probar conexión** y el diagnóstico.

### Identidad del servidor (`hostkey.rs`, D-076)
Primer contacto: la clave se captura en un archivo temporal y se muestra la huella; solo se guarda si el usuario confirma y la huella coincide. Lista de huellas propia (`known_hosts` de la aplicación, 0600). Una huella cambiada detiene la conexión (`HostKeyChanged`) y solo se sustituye con `server_trust(replace=true)`. `~/.ssh/known_hosts` se consulta pero nunca se modifica.

### Secretos (`src-tauri/src/secrets.rs`, `ssh/askpass.rs`, D-077)
`SSH_ASKPASS` es el propio binario (`main.rs` llama primero a `askpass::client_main()`), que pide el secreto por un socket Unix privado de un solo uso. Los secretos viven en memoria (`zeroize`) durante la sesión y, si el usuario lo pide, en Secret Service (crate `secret-service`). Sin almacén seguro se informa (`KeyringStatus::Unavailable`) y se piden cada vez; nunca se escriben en archivos ni en el registro.

### Registro de servidores (`src-tauri/src/servers.rs`, `dcc-store/src/servers.rs`, D-078)
`servers.toml` (0600, escritura atómica) guarda los perfiles, separado de `config.toml` y sin secretos. `Servers` mantiene el `EngineManager` local y uno por servidor remoto (`EngineManager::remote`), cada uno con su propio registro de actividad etiquetado (`ServerTag`), sus tareas canceladas con un `CancellationToken` propio y su estado (`server://state`). Los 59 comandos de Docker reciben `server`; `state.server(&id)` devuelve el gestor o `server.unknown`. Los flujos se registran por servidor (`StreamRegistry::close_server`).

### Interfaz (D-079)
`app/server.tsx`: `ServerScope` da a cada pantalla su servidor; `useApi()` devuelve `dockerApi(server)` (todas las órdenes de Docker llevan el servidor) y `queryClientFor(server)` separa las cachés; el área principal se vuelve a montar al cambiar de servidor (`key`). Los diálogos muestran el servidor de destino (`DialogTarget`). `features/servers/`: lista, asistente (`ServerEditor.tsx`), revisión de identidad, petición de secretos, diagnóstico, selector y vista general.

### CLI y Compose remotos (D-080)
`CliRequest.remote = RemoteExec { session, tty }` ejecuta el mismo programa en el servidor por un canal de la sesión (`[env K=V…] programa args`, entrecomillado); la orden equivalente registrada es `ssh [-p] [-J] usuario@host -- …`. Compose usa los archivos del servidor: existencia (`stat`), lectura (`base64`), guardado atómico (`dd conv=excl` + `chmod` + `mv`) y creación sin sobrescribir (`compose/remote_file.rs`). Los proyectos registrados son por servidor. Las carpetas vinculadas se comprueban en el servidor (`binds::host_paths`).

### Shell SSH del servidor (`ssh/shell.rs`, D-081)
`ssh -tt` por la sesión verificada sobre una PTY local (`openpty`, `setsid` + `TIOCSCTTY`, `TIOCSWINSZ`). Al cerrar: SIGHUP al grupo y, si no termina en 5 s, SIGKILL.

## Comandos IPC

| Comando | Descripción |
|---|---|
| `app_info`, `config_get`, `config_set` | Información y configuración. `config_set` reconecta en segundo plano si cambia la conexión |
| `activity_list`, `activity_clear` | Registro de actividad |
| `engine_state` | Estado de la conexión (`connecting` / `connected` / `disconnected` con diagnóstico) |
| `engine_reconnect` | Detectar y conectar de nuevo (registrado) |
| `engine_test(docker)` | Probar una configuración sin guardarla ni sustituir la conexión |
| `engine_overview` | Versión e información del motor |
| `servers_list`, `server_validate`, `server_save`, `server_delete(confirmed)` | Perfiles de servidores remotos (0.10.0). Eliminar un perfil no toca el servidor |
| `server_connect(secret, remember)`, `server_disconnect`, `server_test`, `server_diagnose`, `servers_overview` | Conexión, prueba de siete pasos, diagnóstico y vista general |
| `server_host_key`, `server_trusted_keys`, `server_trust(key, replace)` | Identidad del servidor (huella) |
| `keyring_status`, `server_secret_status`, `server_forget_secret` | Almacén seguro de credenciales |
| `server_shell_open` | Shell SSH del servidor (solo servidores remotos) |
| `compose_status` | Versión del plugin Compose |
| `containers_list`, `container_action(id, name, action)` | Contenedores; la acción devuelve el estado confirmado (`ContainerState`) |
| `container_remove(id, name, force, volumes, confirmed)` | Exige `confirmed = true` |
| `images_list` | Imágenes con su uso |
| `container_edit_config(id)` | 0.9.2 (D-070): `EditableConfig` (lo editable, lo de solo lectura, redes, copias de seguridad, tamaño de la capa) |
| `container_edit_plan(id, request)` | Valida en Rust y compara → `EditPlan` (cambios directos o que exigen recrear, lo que se mantiene, bloqueos, riesgos, avisos, órdenes equivalentes, cuerpo final sin secretos). No cambia nada |
| `container_edit_apply(id, request, options)` | Aplica: directos o recreación segura (exige `confirmRecreate` y que cada riesgo esté en `acceptedRisks`) → `EditOutcome` con cada paso |
| `container_restore_backup(id, start, confirmed)` | Exige `confirmed`; restaura una copia y aparta el contenedor actual sin borrarlo |
| `container_commit_preview(id, request)` · `container_commit(id, request)` | 0.9.2 (D-069): referencia validada, imagen existente, montajes excluidos y orden equivalente; crear la imagen (`engine.alreadyExists` si el nombre existe y no se aceptó) |
| `container_preview(spec)` | Valida y traduce sin hablar con Docker: problemas, `docker run`/`docker create` equivalentes, cuerpo JSON y nº de secretos ocultos |
| `container_create(spec, start)` | Crea (y opcionalmente inicia); si no arranca devuelve el contenedor creado con `startError` |
| `container_inspect(id, name)` · `container_duplicate_spec(id, name)` | Detalle (`ContainerDetails`) y especificación para duplicar (`DuplicateSpec`) |
| `image_defaults(image)` · `networks_brief` · `volumes_brief` | Datos reales para ayudar a rellenar el asistente |
| `logs_open(id, name, tail, onEvent)` · `stats_open(id, name, onEvent)` | Flujos; devuelven el identificador |
| `terminal_open(id, name, shell, cols, rows, onEvent)` · `terminal_input(stream, data)` · `terminal_resize(stream, cols, rows)` | Terminal interactiva (salida en base64) |
| `stream_close(stream)` | Cierra un flujo; `false` si ya había terminado |
| `image_details(reference)` · `image_history(reference)` | Detalle (`ImageDetails`, con el JSON completo) y capas |
| `image_tag(source, repo, tag)` · `image_remove(reference, force, confirmed)` · `images_prune(all, confirmed)` | Etiquetar, eliminar y limpiar (las dos últimas exigen `confirmed`) |
| `images_search(term, limit, officialOnly)` | Búsqueda en Docker Hub a través del daemon |
| `image_ref_parse(reference)` | Analiza una referencia como Docker, sin hablar con Docker (errores `ref.*`) |
| `image_pull_open(reference, auth, onEvent)` | Descarga en flujo: `PullEvent` (`progress` \| `end`); `auth` = `PullAuth` (`dockerConfig` \| `anonymous` \| `manual`) |
| `build_preview(spec)` | Valida la construcción, resume el contexto y devuelve el comando y la petición (secretos ocultos), sin hablar con Docker |
| `build_open(spec, onEvent)` | Construcción en flujo: `BuildEvent` (`progress` con pasos y líneas \| `end` con `BuildEnd` o error) |
| `networks_list` · `network_details(id, name)` | Redes con sus contenedores (`NetworkSummary`) y detalle (`NetworkDetails`: IPAM, opciones, JSON completo) |
| `network_preview(spec)` | Valida la red y devuelve problemas, comando equivalente, petición y cuerpo exacto, sin hablar con Docker |
| `network_create(spec)` · `network_remove(id, name, confirmed)` · `networks_prune(confirmed)` | Crear (valida antes, también sin conexión), eliminar y limpiar (las dos últimas exigen `confirmed`) |
| `network_connect(network, networkName, container, containerName, options, subnets)` · `network_disconnect(…, force, confirmed)` | Conectar (alias e IP fija) y desconectar (exige `confirmed`) |
| `volumes_list` · `volume_details(name)` · `volume_sizes` | Volúmenes con los contenedores que los montan, detalle (con JSON completo) y tamaños bajo demanda |
| `volume_preview(spec)` · `volume_create(spec)` | Vista previa sin hablar con Docker; creación (error `engine.invalidSpec` con `volumeName.exists` si ya existe) |
| `volume_remove(name, confirmed)` · `volumes_prune(all, confirmed)` | Eliminar y limpiar (exigen `confirmed`) |
| `binds_list` | Directorios vinculados agrupados por ruta, con su estado en el equipo y sus riesgos |
| `compose_projects` | Proyectos desplegados (etiquetas) y guardados, con contenedores y estado |
| `compose_config(project, unsaved)` | Modelo, configuración resuelta, error o avisos de Compose; `unsaved = {index, content}` valida sin guardar |
| `compose_preview(project, action)` | Comando exacto de una acción (valida la entrada, no ejecuta nada) |
| `compose_run_open(project, action, confirmed, onEvent)` | Acción con salida en vivo: `ComposeEvent` (`lines` \| `end` con `RunClosed`); los errores de validación, confirmación, archivos u operación en curso se devuelven antes de abrir nada |
| `compose_logs_open(project, services, tail, timestamps, onEvent)` | Registros en vivo |
| `compose_register(files, name)` · `compose_unregister(name)` | Añadir (nombre decidido por Compose salvo que se indique; `compose.duplicate`) y quitar de la lista |
| `compose_file_read(path)` · `compose_file_save(path, content, version)` · `compose_file_create(dir, content)` · `compose_find_file(dir)` | Archivos (`engine.fileChanged`, `engine.alreadyExists`) |
| `system_info()` | `SystemInfo`: versión, paquete instalado, ejecutable, sistema, núcleo, arquitectura, glibc, WebKitGTK, escritorio, rutas |
| `updates_check(source?)` · `updates_last()` | Consulta la fuente (la escrita o la guardada) → `UpdateCheck`; última comprobación de la sesión (`UpdateOutcome`). Errores `updates.noSource`, `updates.invalidSource`, `updates.insecureSource`, `updates.network`, `updates.response` |
| `open_release_page(url)` | `xdg-open` con un enlace `https://` (`app.open`) |
| `components_inspect()` | 0.9.1 (D-065): `ComponentsReport` (paquete propio, dependencias clasificadas, herramientas externas, datos del usuario, historial de APT, simulación). Solo consulta y simula, sin privilegios |
| `components_plan(packages)` | Comprueba la selección contra un análisis nuevo y simula la orden exacta → `RemovalPlan` (`extra` no vacío = no se ejecutará). Errores `packages.invalid`, `packages.notAllowed`, `packages.notInstalled` |
| `components_remove(packages, confirmed)` | Exige `confirmed`; repite el plan y ejecuta `pkexec /usr/bin/apt-get remove -y --no-auto-remove …` → `RemovalResult`. Errores `packages.extra`, `packages.cancelled`, `packages.denied`, `packages.noElevation`, `packages.busy`, `packages.remove` |
| `events_recent(filter)` | Eventos de esta sesión en memoria (`EventPage`, los más recientes primero) |
| `events_journal(filter)` · `events_journal_clear()` | Eventos guardados (también de sesiones anteriores) y borrarlos (`history.io`) |
| `events_query(query)` | Consulta a Docker (`EventQueryResult` con eventos, `truncated` y comando equivalente; `engine.invalidInput` sin llamar a Docker) |
| `history_list(filter)` · `history_clear()` | Historial de operaciones guardado (`HistoryPage` con inicio de la sesión y carpeta) y borrarlo |

Errores: `CommandError { code, message, httpStatus, issues }` con códigos `engine.invalidSpec` (con `issues`), `stream.closed`, `engine.notConnected`, `engine.docker`, `engine.timeout`, `engine.io`, `engine.invalidResponse`, `engine.invalidInput`, `engine.busy`, `engine.notConfirmed`, `engine.process`, `engine.fileChanged`, `engine.alreadyExists`, `engine.composeUnavailable`, `compose.duplicate`, `history.io`, `engine.other`, `config.io`.

El plugin `opener` de Tauri (0.9.1, D-066) solo puede abrir `https://github.com/SergiioAlfaro` (permiso limitado en `capabilities/default.json`). Desde la 1.2.0 (D-091), `capabilities/default.json` ya no incluye `core:default`: solo `core:event:allow-listen` y `core:event:allow-unlisten`, el diálogo de abrir y el portapapeles. Los enlaces de las terminales (OSC 8) y la página de una versión se abren desde Rust con `xdg-open` tras validarlos (`open_terminal_link`, `open_release_page`).

Al cerrar la ventana (0.9.1, D-063) se cierran los flujos abiertos y se espera a que terminen (como mucho 8 s): `StreamRegistry::close_all_and_wait`.

Eventos: `activity://event`, `activity://resync`, `engine://state`, `server://state` (0.10.0), `docker://event` (cada evento de Docker), `updates://result` (comprobación al iniciar). Canales por flujo: `LogEvent` (`lines` | `end`), `StatsEvent` (`sample` | `end`), `TerminalEvent` (`output` | `end`), `ComposeEvent` (`lines` | `end`).

## Interfaz
- `features/engine/store.ts`: estado de conexión; `App` lo alimenta con `engine_state` y el evento `engine://state`. Al pasar a conectado se invalidan todas las consultas; al perder la conexión se descartan.
- `RequireConnection` envuelve los apartados que necesitan Docker.
- `diagnosis.ts` traduce el diagnóstico en pasos concretos (función pura, probada caso a caso).
- Todas las peticiones son asíncronas; una operación en curso bloquea solo su fila.
- `features/containers/store.ts`: navegación interna (lista, detalle con pestaña, asistente con especificación prellenada).
- `create/CreateWizard.tsx`: pestañas General, Entorno, Red, Almacenamiento, Recursos y Avanzado; vista previa con retardo de 200 ms; `shellwords.ts` divide Comando y Entrypoint con reglas de comillas POSIX sin ejecutar ninguna shell.
- `detail/`: `useStream` abre y cierra flujos ligados al ciclo de vida del componente; las pestañas con flujos se mantienen montadas tras visitarlas. La terminal usa xterm.js 5.5 (D-040).
- `public/boot-guard.js` muestra los errores anteriores al arranque de la interfaz.
- `features/networks/`: `store.ts` (lista, detalle, creación), `model.ts` (filtros, estimación de la limpieza, motivos por los que Docker rechazaría una conexión, comandos equivalentes; funciones puras), `dialogs.tsx` (eliminar, limpiar, conectar, desconectar: consultan a Docker al abrirse y no permiten confirmar con datos desactualizados, D-049), `CreateNetwork.tsx` (asistente con vista previa del backend), `NetworkDetail.tsx`.
- `features/volumes/`: lista con tamaños bajo demanda, detalle, asistente de creación (tipos habituales del driver local: tmpfs, NFS, carpeta), diálogos de eliminación y limpieza que consultan a Docker al abrirse; `model.ts` con la estimación de la limpieza según la versión de la API.
- `features/binds/BindsView.tsx`: carpetas vinculadas con su estado, riesgos y contenedores; "Vincular una carpeta…" usa el selector de carpetas del sistema y abre el asistente de contenedores.
- `features/compose/`: `store.ts` (lista, detalle con pestaña y filtro de registros, proyecto nuevo), `model.ts` (servicios del archivo unidos a sus contenedores, filtros, acciones; funciones puras), `runs.ts` (última operación de cada proyecto en un almacén global, para que siga al cambiar de apartado; al terminar recarga proyectos, contenedores, volúmenes, redes e imágenes), `RunPanel.tsx` (salida en vivo, interrumpir), `dialogs.tsx` (desplegar, eliminar con confirmación, abrir proyecto; el comando exacto lo calcula el backend), `FileEditor.tsx` (validar sin guardar, guardar con protección frente a cambios externos), `LogsTab.tsx`, `NewProject.tsx`. Si Compose no está disponible se explica cómo instalarlo.
- `app/App.tsx` (Fase 9): los apartados se cargan con `React.lazy` y se precargan 2 s después del arranque (`usePreloadSections`). Una vez cargados se pintan directamente (`View`), sin la suspensión de `lazy`. La terminal (xterm) también se carga al abrirla.
- `shared/format.ts`: formateadores de `Intl` en caché (`numberFormat`, `dateTimeFormat`) y `formatTimestamp`, que da el único formato de marca de tiempo: 24 h, con fecha y milisegundos opcionales.
- `shared/ipc/client.ts`: `localizeError` traduce los errores de texto fijo (`LOCALIZED_ERRORS`) al idioma de la interfaz.
- `features/settings/`: `UpdatesSettings.tsx`, `SystemSettings.tsx` (con `systemReport`), restablecer en `SettingsView.tsx` y `layout.tsx` (`Group`, `Row`).
- `features/events/`: `EventsView.tsx` (pestañas En directo, Guardados, Consultar a Docker, Historial), `store.ts` (eventos en directo, pausa, últimos 2000), `liveRefresh.ts` (`useLiveRefresh`, montado en `App.tsx`: cada evento recarga las consultas afectadas, agrupadas 250 ms y como mucho una vez por segundo durante las ráfagas, `refreshDelay`), `EventRow.tsx` (filas memorizadas con claves estables, `eventKeys`), `model.ts` (`invalidationsFor`, fechas con precisión de `timeNano`, filtros locales), `EventRow.tsx`, `HistoryTab.tsx`, `filters.tsx`.
- `features/terminals/` (0.9.1, D-064): `sessions.ts` (gestor de sesiones fuera de React: una xterm y un flujo por sesión, generación por apertura, cierre sin huérfanos; estado visible en Zustand), `TerminalsView.tsx` (distribución según `layout.ts`, `Splitter.tsx` con ratón y teclado), `TerminalPane.tsx`, `NewTerminalDialog.tsx`.
- `features/settings/ComponentsSettings.tsx` (D-065) y `features/help/` (D-066: `docs/{en,es,ca,gl,eu,fr,it,pt}.md` con 27 secciones (0.10.0), `markdown.tsx` sin `innerHTML`, `HelpView.tsx`), como subpáginas de `SettingsView.tsx`.
- `shared/i18n`: `en`, `es`, `ca`, `gl`, `eu`, `fr`, `it`, `pt` en el orden del selector (D-067, D-073); `LANGUAGE_NAMES` con los endónimos.
- `features/containers/edit/` (0.9.2, D-069, D-070): `EditContainer.tsx` (pestañas, barra de cambios pendientes, ciclo de vida, copias de seguridad, redes con los diálogos de la Fase 5, Avanzado de solo lectura), `ReviewPanel.tsx` (comparación, asistente de recreación con riesgos y confirmación, resultado paso a paso), `model.ts` (borrador y clasificación para los indicadores; decide el backend), `CommitDialog.tsx`. `create/editors.tsx` reúne los editores de puertos, montajes, memoria y clave/valor que comparten el asistente de creación y la edición.
- `shared/ui/Menu.tsx` (0.9.2): menú de acciones accesible (botón ⋯ y botón derecho en la fila).
- Color principal (D-072, D-082): desde la 1.0.0 solo morado. Los tokens base `--beacon*` y `--focus` de `styles/app.css` son morados en claro y oscuro, sin variantes por atributo; `--beacon-nav` para la barra lateral; `shared/ui/terminalTheme.ts` usa el mismo morado en las terminales. Los colores de señal (`--signal-*`) son independientes.
- `features/images/`: `store.ts` (lista, detalle, búsqueda, construcción), `pull/pulls.ts` y `build/buildStore.ts` (descargas y construcción en almacenes globales para que sigan al cambiar de apartado), `dialogs.tsx` (etiquetar, eliminar, limpiar), `useRefCheck.ts` (validación de referencias en el backend).

## Distribución (Fase 10, D-083)

| Pieza | Función |
|---|---|
| `src-tauri/tauri.conf.json` (`bundle`) | Metadatos del `.deb`: versión, descripción, `Recommends: openssh-client`, iconos y archivos de `/usr/share/doc/docker-control-center/` |
| `scripts/deb-set-depends.sh` | Dependencias reales del binario con `dpkg-shlibdeps` (D-059) |
| `scripts/build-deb-compat.sh` | Paquete compatible compilado en un contenedor de Ubuntu 22.04 |
| `packaging/copyright`, `LICENSE` | Licencia (DEP-5) |
| `scripts/third-party-licenses.py` → `packaging/THIRD-PARTY.md` | Componentes de terceros incluidos y sus licencias |
| `scripts/install.sh` | Instalación, comprobación y desinstalación para el usuario final |
| `scripts/run.sh` | Ejecutar la versión instalada o la compilación del código |
| `scripts/dev/clean-install-test.sh` | Instalación limpia en Ubuntu 24.04 y 22.04 (solo máquinas de prueba) |
| `scripts/verify-phase-10.sh` | Verificación final: regresión completa, color, paquetes e instalador |

## Pruebas

| Capa | Herramienta | Cantidad |
|---|---|---|
| Rust unitarias | `cargo test` | 27 actividad · 301 motor · 37 configuración · 124 aplicación (1.0.0) |
| Integración IPC (runtime simulado de Tauri) | `cargo test -p docker-control-center` | 75: incluye las de actualizaciones (servidor local, límites, redirecciones; HTTPS real solo con `DCC_NET_TESTS=1`) y la de la Fase 9 por IPC; ocho contra Docker real (inspección, capas y construcción; descarga completa y cancelada; redes; volúmenes; Compose; eventos) |
| Integración contra Docker real | `DCC_DOCKER_TESTS=1 cargo test -p dcc-engine --test live_docker --test live_containers --test live_images --test live_networks --test live_volumes --test live_compose --test live_events` | 11 + 7 + 9 + 4 + 4 + 6 + 2 (redes, volúmenes y Compose: probados con Docker 29 y 24; Compose con 5.6.1 y, con `DCC_TEST_COMPOSE`, con el binario 2.26.1; 2 requieren usuario no root; la descarga usa un registro local con una imagen nueva en cada prueba, D-043, D-045; probadas con los almacenes de imágenes clásico y containerd) |
| Carrera de estado de Docker (determinista) | `cargo test -p dcc-engine --test stale_list --test stale_compose` | 1 + 1 |
| Interrupción de la suscripción a eventos (servidor simulado) | `cargo test -p dcc-engine --test events_interrupted` | 3 |
| Interfaz | Vitest + Testing Library (zona horaria fija Europe/Madrid) | 376 |
| Administración remota contra servidores SSH reales (0.10.0) | `DCC_DOCKER_TESTS=1 DCC_SSH_LAB=lab.json cargo test -p dcc-engine --test live_ssh -- --test-threads=1`; IPC con `DCC_TEST_ASKPASS`; las suites de recursos con `DCC_SSH_TEST=usuario@host:puerto` (laboratorio de `scripts/dev/remote-lab.sh`, solo en una máquina de pruebas) | 12 + 1 IPC; contenedores, imágenes, redes, volúmenes, terminales, edición y eventos repetidos por SSH |
| Llavero real (0.10.0) | `DCC_KEYRING_TEST=1` dentro de una sesión D-Bus con un llavero temporal | 1 |
| Edición, recreación y commit contra Docker real (0.9.2) | `DCC_DOCKER_TESTS=1 cargo test -p dcc-engine --test live_edit` | 8 (Docker 29, 26 y 24) |
| Terminales múltiples contra Docker real (0.9.1) | `DCC_DOCKER_TESTS=1 cargo test -p dcc-engine --test live_terminals` | 5 |
| Desinstalación real (0.9.1) | `DCC_TEST_CHROOT=… DCC_TEST_DEB=… cargo test -p docker-control-center --lib uninstall::tests::live_` (root, chroot preparado con `scripts/prepare-uninstall-chroot.sh`) | 1 |
| Prototipo congelado (D-040) | `node scripts/dev/check-frozen-prototype.cjs` (Playwright) | 1 |
| Estática | clippy `-D warnings`, `cargo fmt --check`, `tsc` estricto | — |
| Paquete | `scripts/verify-phase-10.sh --deb --compat` | Versión, contenido y lanzador; `Depends` real; `libc6` cubre la glibc del binario; paquete compatible con glibc ≤ 2.35; el instalador elige el paquete |
| Instalación limpia | `scripts/dev/clean-install-test.sh` (root, máquina de pruebas) | Ubuntu 24.04 y 22.04: instalación con un usuario normal, archivos, arranque y desinstalación |
| Manual | `scripts/verify-phase-10.sh` | F1–F8 (solo lo que cambia en la 1.0.0) |
