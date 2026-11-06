# Arquitectura de Docker Control Center

Versión 0.2.1 (Fase 2 con correcciones).

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
│         compose    detección de Compose v2+ (plugin o binario)                        │
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
| Estado confirmado tras cada acción (D-031) | `GET /containers/{id}/json?size=false` | `docker container inspect --format '{{.State.Status}}' {id}` |
| Versión de Compose (D-032) | proceso `docker compose version` y, si falta el plugin, `docker-compose version` (CLI, sin opciones) | — |
| Crear contenedor (D-034) | `POST /containers/create?name=…&platform=` con el cuerpo de `ContainerSpec::to_create_body` | `docker create …` (`ContainerSpec::cli_args`, secretos ocultos, D-039) |
| Inspeccionar | `GET /containers/{id}/json?size=false` | `docker container inspect {id}` |
| Configuración de la imagen | `GET /images/{ref}/json` | `docker image inspect {ref}` |
| Redes / volúmenes (para el asistente) | `GET /networks` · `GET /volumes` | `docker network ls` · `docker volume ls` |
| Logs (flujo, D-036) | `GET /containers/{id}/logs?follow=true&stdout=true&stderr=true&timestamps=true&tail=N` | `docker logs --follow --timestamps --tail N {id}` |
| Estadísticas (flujo) | `GET /containers/{id}/stats?stream=true&one-shot=false` | `docker stats {id}` |
| Terminal (D-036, D-037) | `POST /containers/{id}/exec` → `POST /exec/{id}/start` (conexión secuestrada) → `POST /exec/{id}/resize` (no registrado, contado) → `GET /exec/{id}/json` | `docker exec --interactive --tty {id} <shell>` |

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

### Especificación de contenedor (`spec.rs`, D-034)
`ContainerSpec` → `validate(check_fs)` (problemas `SpecIssue { severity, field, code, params }`), `to_create_body()`, `cli_args(run)`, `redacted()`. `from_inspect()` construye el `DuplicateSpec` (D-038). `details.rs` convierte la respuesta de inspect en `ContainerDetails` (incluye el JSON completo). `streams.rs` implementa los flujos: `LineAssembler` (fragmentos, UTF-8 y marca de tiempo), `sample_from` (fórmulas de `docker stats`) y `run_terminal`.

### Flujos en la aplicación (`src-tauri/src/streams.rs`)
`StreamRegistry` guarda, por identificador, el `CancellationToken` y, para la terminal, el canal de entrada. `logs_open`/`stats_open`/`terminal_open` devuelven el identificador y envían los eventos por un `Channel` de Tauri; al terminar mandan `end` con `StreamClosed { end: StreamEnd { reason, summary, exitCode } | error }` y se retiran del registro. `stream_close` cancela; al destruirse la ventana se cancelan todos.

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
| `containers_list`, `container_action(id, name, action)` | Contenedores; la acción devuelve el estado confirmado (`ContainerState`) |
| `container_remove(id, name, force, volumes, confirmed)` | Exige `confirmed = true` |
| `images_list` | Imágenes con su uso |
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

Errores: `CommandError { code, message, httpStatus, issues }` con códigos `engine.invalidSpec` (con `issues`), `stream.closed`, `engine.notConnected`, `engine.docker`, `engine.timeout`, `engine.io`, `engine.invalidResponse`, `engine.invalidInput`, `engine.busy`, `engine.notConfirmed`, `engine.process`, `engine.other`, `config.io`.

Eventos: `activity://event`, `activity://resync`, `engine://state`. Canales por flujo: `LogEvent` (`lines` | `end`), `StatsEvent` (`sample` | `end`), `TerminalEvent` (`output` | `end`).

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
- `features/images/`: `store.ts` (lista, detalle, búsqueda, construcción), `pull/pulls.ts` y `build/buildStore.ts` (descargas y construcción en almacenes globales para que sigan al cambiar de apartado), `dialogs.tsx` (etiquetar, eliminar, limpiar), `useRefCheck.ts` (validación de referencias en el backend).

## Pruebas

| Capa | Herramienta | Cantidad |
|---|---|---|
| Rust unitarias | `cargo test` | 22 actividad · 196 motor · 15 configuración |
| Integración IPC (runtime simulado de Tauri) | `cargo test -p docker-control-center` | 46 (seis contra Docker real: inspección, capas y construcción; descarga completa y cancelada; redes; volúmenes) |
| Integración contra Docker real | `DCC_DOCKER_TESTS=1 cargo test -p dcc-engine --test live_docker --test live_containers --test live_images --test live_networks --test live_volumes` | 11 + 7 + 9 + 4 + 4 (redes y volúmenes: probados con Docker 29 y 24; 2 requieren usuario no root; Compose se omite si no está instalado; la descarga usa un registro local con una imagen nueva en cada prueba, D-043, D-045; probadas con los almacenes de imágenes clásico y containerd) |
| Carrera de estado de Docker (determinista) | `cargo test -p dcc-engine --test stale_list` | 1 |
| Interfaz | Vitest + Testing Library | 179 |
| Prototipo congelado (D-040) | `node scripts/dev/check-frozen-prototype.cjs` (Playwright) | 1 |
| Estática | clippy `-D warnings`, `cargo fmt --check`, `tsc` estricto | — |
| Manual | `scripts/verify-phase-6.sh` | M1–M15 |
