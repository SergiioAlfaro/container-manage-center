# Registro de decisiones técnicas

Cada decisión tiene un identificador estable. Si una decisión se revisa, se marca
como **sustituida** y se añade una nueva; no se borra.

| ID | Fecha | Estado |
|---|---|---|
| D-001 … D-017 | 2026-11-04 | Vigentes (Fase 1) |
| D-018 … D-030 | 2026-11-04 | Vigentes (Fase 2) |
| D-031 … D-033 | 2026-11-04 | Vigentes (0.2.1, correcciones de la Fase 2) |

---

### D-001 · Tauri 2 + Rust + React/TypeScript
**Aprobada por el propietario del proyecto.** Tauri usa el WebKitGTK del sistema
(sin Chromium embebido), lo que reduce memoria y tamaño de paquete. Toda la
lógica de Docker vive en Rust; la interfaz nunca accede al socket.
Descartados: Electron (consumo), Python/Qt o GTK (concurrencia de streams y
empaquetado menos fiables).

### D-002 · Tauri 2 estable, no Tauri 3
En noviembre de 2026 Tauri 3 sigue en alfa (`3.0.0-alpha.5`). Se fija `tauri = "2"`
y `@tauri-apps/api ^2`. Revisar cuando Tauri 3 sea estable.

### D-003 · Plataformas mínimas: Debian 12 y Ubuntu 22.04
**Aprobada.** Tauri 2 requiere `libwebkit2gtk-4.1`, disponible desde esas versiones.

### D-004 · Comunicación con Docker por la Engine API (socket Unix)
**Aprobada.** Se usará `bollard` desde la Fase 2. No se ejecutan comandos de shell
para operaciones que la API permite.

### D-005 · Docker Compose mediante el CLI oficial, sin shell
**Aprobada.** Compose no tiene API en Docker Engine. Se invoca `docker compose`
con `std::process::Command`, argumentos uno a uno y sin `sh -c`. En el panel de
actividad estas operaciones se marcan como **CLI** y muestran el comando real.

### D-006 · Docker rootless desde la Fase 2
**Aprobada.** Detección automática de `$DOCKER_HOST`, `$XDG_RUNTIME_DIR/docker.sock`
y `/var/run/docker.sock`, con configuración manual de la conexión.

### D-007 · Español por defecto, inglés como alternativa
**Aprobada.** `i18next` con `locales/es.json` y `locales/en.json`. Una prueba
automatizada exige que ambos archivos tengan exactamente las mismas claves.

### D-008 · Registro de actividad como crate independiente (`dcc-activity`)
En la propuesta el registro formaba parte de `dcc-engine`. Se separa porque lo
usarán tanto el cliente de la API como el envoltorio del CLI de Compose, y así
ninguna de las dos capas depende de la otra.
Invariantes:
- Un `OperationRecord` solo se crea con `ActivityLog::begin` al iniciar una
  operación real. La interfaz no puede crear registros.
- `transport` describe la operación **ejecutada** (petición API o proceso CLI).
  `equivalent_command` es solo una traducción orientativa al CLI.
- Si el `OperationHandle` se descarta sin cerrarse (p. ej. tarea cancelada), la
  operación queda como `cancelled`; nunca se queda "en curso" para siempre.
- Las salidas capturadas se recortan a 64 KiB indicando el tamaño original.

### D-009 · Tipos compartidos generados con ts-rs
Los tipos del IPC se definen una sola vez en Rust y `cargo test` genera
`ui/src/shared/ipc/bindings/*.ts`. Evita que frontend y backend diverjan.
Los archivos generados se versionan para que el frontend compile sin Rust.

### D-010 · Configuración en TOML con escritura atómica
`$XDG_CONFIG_HOME/docker-control-center/config.toml`, permisos `0600`, escritura
en archivo temporal + `rename`. Un archivo corrupto se aparta a
`config.toml.corrupt` y se arranca con valores por defecto, avisando al usuario.
Todos los campos tienen valor por defecto para que añadir opciones no rompa
archivos antiguos. Valores fuera de rango se corrigen en el backend.

### D-011 · Credenciales nunca en el archivo de configuración
Las credenciales de registros irán al llavero del sistema (Secret Service) o a
los *credential helpers* de Docker (Fase 4/9). `config.toml` no las admite.

### D-012 · La aplicación se niega a ejecutarse como root
`geteuid() == 0` termina con un mensaje. `DCC_ALLOW_ROOT=1` lo permite solo para
entornos de prueba aislados (se usa en el entorno de desarrollo de Claude, que
corre como root dentro de un contenedor). No está documentado para usuarios finales.

### D-013 · Apartados no implementados: estado informativo sin acciones
Requisito "sin botones decorativos ni datos ficticios". Los apartados de fases
futuras muestran qué harán y en qué fase llegan, sin botones ni datos. Una prueba
automatizada comprueba que el área central de esos apartados no contiene botones.

### D-014 · Estado de interfaz con Zustand; sin router
Nueve apartados fijos no justifican un router. La sección activa es estado en
memoria. TanStack Query se incorporará en la Fase 2, cuando haya datos remotos
que cachear e invalidar.

### D-015 · Identidad visual "casco y baliza"
Fondo gris niebla, barra lateral azul casco, acento verde azulado (baliza) y
colores de señal para el estado de las operaciones. IBM Plex Sans para la
interfaz y JetBrains Mono solo para comandos y datos técnicos. Fuentes
empaquetadas localmente (`@fontsource`), sin dependencias de red. El panel de
actividad se diseña como un cuaderno de bitácora: franja de estado por
operación, distintivo API/CLI y comando como línea principal.

### D-016 · Licencia provisional GPL-3.0-or-later
Declarada en `Cargo.toml` y `package.json` como provisional. **Pendiente de
confirmación por el propietario** antes de la Fase 10. "Docker" es una marca
registrada de Docker, Inc.; antes de distribuir públicamente conviene revisar
el nombre del producto.

### D-017 · Nombre interno del paquete sin espacios
`productName` en `tauri.conf.json` es `docker-control-center`, de modo que el
paquete se llama `docker-control-center_<versión>_amd64.deb` y el lanzador
`docker-control-center.desktop` (convención freedesktop). El nombre visible
"Docker Control Center" se mantiene en el título de la ventana y en el menú
mediante la plantilla `packaging/docker-control-center.desktop`, que añade
traducción al español y palabras clave de búsqueda. Las dependencias de
ejecución (`libwebkit2gtk-4.1-0`, `libgtk-3-0`) las añade Tauri.

---

## Fase 2

### D-018 · Política de registro de las comprobaciones automáticas
Se registran siempre: las conexiones pedidas por el usuario o al arrancar
(ping, versión, información), toda operación de los apartados y la comprobación
que detecta una pérdida de conexión. **No** se registran una a una las
comprobaciones automáticas periódicas (latido cada 10 s con la conexión activa y
reintentos con espera creciente de 2 a 30 s sin ella), para no inundar el panel.
Cuando un reintento automático consigue reconectar, sus peticiones de versión e
información sí quedan registradas, porque fundamentan lo que se muestra después.

### D-019 · La ruta registrada es la ruta enviada, verificado por tráfico
Comprobado con un proxy sobre el socket: bollard 0.21 envía las rutas **sin**
prefijo de versión (`GET /_ping`, `GET /containers/json?all=true&size=false`).
Las query strings se generan con `serde_urlencoded` sobre el mismo struct de
opciones que usa bollard (`resources::path_with_query`). Las pruebas
`recorded_requests_match_what_travels_on_the_socket`,
`container_lifecycle_against_real_docker_matches_wire` e
`images_list_reports_real_usage` interceptan el socket real y exigen que lo
registrado coincida, en orden, con lo transmitido.

### D-020 · Alcance de la Fase 2 ampliado
A petición del propietario, la Fase 2 incluye: listado de contenedores con las
acciones iniciar, detener, reiniciar, pausar, reanudar y eliminar, y listado de
imágenes con su uso. Creación, logs, inspección y terminal siguen en la Fase 3.
Descarga, construcción, etiquetado y eliminación de imágenes, en la Fase 4.

### D-021 · Confirmación de operaciones destructivas en dos capas
La interfaz pide confirmación con un diálogo (y obliga a marcar "forzar" si el
contenedor está activo). Además, el comando `container_remove` exige
`confirmed = true`; sin él devuelve `engine.notConfirmed` sin tocar Docker.

### D-022 · Validación de identificadores y bloqueo por recurso
Los IDs y nombres se validan (`[A-Za-z0-9][A-Za-z0-9_.-]{0,254}`) antes de
formar una ruta HTTP; uno inválido nunca llega a Docker (probado con proxy).
Dos operaciones simultáneas sobre el mismo contenedor no se permiten: la
segunda devuelve `engine.busy`. Los bloqueos sobreviven a las reconexiones.

### D-023 · Actualización de datos
(Complementada por D-031.) Los datos se vuelven a pedir a Docker tras cada operación, al reconectar, al
volver a la ventana y con "Actualizar". Las consultas no se reintentan solas.
Los cambios hechos fuera de la aplicación (CLI) aparecen en la siguiente
actualización; la actualización en tiempo real con el flujo de eventos de Docker
llega en la Fase 8.

### D-024 · API mínima 1.41 (Docker Engine 20.10)
Debian 12 incluye `docker.io` 20.10.24. Con una API anterior la conexión se
rechaza con el diagnóstico `apiTooOld`.

### D-025 · Solo sockets Unix locales
`DOCKER_HOST` o contextos con `tcp://`, `ssh://`, etc. se diagnostican como
`unsupportedHost`, pero se siguen probando el resto de candidatos locales.

### D-026 · Imagen de prueba local, sin Internet
Las pruebas de integración compilan un binario estático mínimo en C que espera a
SIGTERM y lo importan como `dcc-test/sleeper:1` (`POST /images/create?fromSrc=-`).
No dependen de Docker Hub. Requiere `build-essential` y `libc6-dev`, ya
necesarios para compilar el proyecto. Los contenedores de prueba llevan la
etiqueta `dcc.test=true` y se eliminan al terminar cada prueba.

### D-027 · Herramienta `dcc-diagnose`
Binario de terminal que usa el mismo código de conexión que la aplicación y
muestra el diagnóstico y las peticiones realizadas. Sirve para soporte y para la
verificación automática. No modifica nada.

### D-028 · Barrera de errores de la interfaz
Un error de renderizado muestra el error y un botón para recargar, en lugar de
una ventana en blanco.

### D-029 · Contrato IPC sin `bigint` ni snake_case
Los enums con variantes struct usan `rename_all_fields = "camelCase"` y los
enteros de 64 bits se anotan como `number` para ts-rs. Una prueba de interfaz
(`bindings.test.ts`) impide que reaparezcan `bigint` o campos snake_case.

### D-030 · Instrucciones del diagnóstico: el usuario las ejecuta
El diagnóstico propone órdenes concretas (iniciar el servicio, `usermod`,
contexto por defecto…) para copiar. La aplicación nunca las ejecuta, no pide
sudo, no cambia permisos ni grupos, y advierte de que el grupo `docker` equivale
a privilegios de administrador.

---

## Versión 0.2.1 (correcciones tras la verificación en Debian 13)

### D-031 · Estado confirmado con inspect tras cada acción
**Problema observado** en Debian 13 (Docker 26.1.5): `container_lifecycle_against_real_docker_matches_wire`
esperaba `exited` tras detener y el listado devolvió `running`.

**Causa** (código de moby, `daemon/monitor.go`, `handleContainerExit`): con el
contenedor bloqueado, el daemon llama a `SetStopped`, que despierta al manejador
de `POST /containers/{id}/stop` y hace que responda 204, y *después* ejecuta
`CheckpointTo(containersReplica)`, que escribe el estado en disco y en la réplica
de la que lee `GET /containers/json`. Mientras dura esa escritura el listado está
desfasado. `GET /containers/{id}/json` (inspect) toma el bloqueo del contenedor y
espera a que termine, así que devuelve el estado real y, al responder, el
listado ya es coherente. La misma causa y solución están documentadas de forma
independiente en otro proyecto (AgentSpore PR #129).

No se reprodujo en el entorno de desarrollo (Docker 29.9.2, disco rápido) en 120
ciclos con `curl` y con un cliente de conexión persistente; tampoco en el
proyecto citado tras 680 ciclos. Afectaba también a la aplicación: la tabla se
refrescaba con el listado justo después de la acción.

**Solución:** `Engine::container_action` ejecuta la acción y luego
`GET /containers/{id}/json?size=false` (operación registrada "Comprobar estado
del contenedor", equivalente `docker container inspect --format '{{.State.Status}}' <id>`)
y devuelve `ContainerState`. La interfaz aplica ese estado a la fila de
inmediato y después vuelve a pedir el listado. Pruebas: `tests/stale_list.rs`
(servidor que responde como Docker durante la ventana: listado `running`,
inspect `exited`), prueba de interfaz equivalente (falla sin la corrección) y la
prueba de ciclo de vida real, que comprueba estado confirmado y listado.

### D-032 · Detección de Docker Compose
**Problema observado:** el script mostraba `unknown flag: --short` y la app
`unknown flag: --format`. Reproducido ocultando el plugin: cuando el CLI de
Docker **no encuentra el plugin Compose**, `compose` no es un subcomando y el
propio `docker` interpreta `--short`/`--format`, que no conoce (salida 125). El
mensaje indica, por tanto, que Compose no está disponible para el CLI, no que la
versión no admita esas opciones.

**Solución:** se ejecuta solo `docker compose version` (sin opciones, válido en
toda Compose v2+) y, si falta el plugin, `docker-compose version`. Estados:
`available` (plugin o binario independiente v2+), `notInstalled` (con las rutas
buscadas), `pluginUnusable` (el archivo existe pero el CLI no lo usa), `legacy`
(solo Compose v1, no admitido), `unrecognized` y `error`. Versiones admitidas:
Compose v2 o posterior. Compose **no es necesario para la Fase 2**; lo será para
el apartado Docker Compose (Fase 7). Instalación: Debian 13 `docker-compose`;
Ubuntu `docker-compose-v2`; repositorio oficial de Docker `docker-compose-plugin`;
en Debian 12 el paquete `docker-compose` es la v1 (no admitida).

### D-033 · Semántica del script de verificación
Cuatro categorías: correctas, **fallos** (obligatorias), **omitidas
justificadamente** (con motivo, nunca cuentan como superadas) y avisos.
Veredicto explícito y código de salida 1 ante cualquier fallo. Obligatorio: la
conexión con Docker y las pruebas contra Docker real. Omisiones aceptadas:
Compose no instalado, pruebas que no aplican al usuario (permisos siendo
root/miembro del grupo), prueba sin red si el sistema no permite espacios de
nombres sin privilegios, arranque sin sesión gráfica. Al salir (también ante
errores o Ctrl+C) se eliminan solo los recursos con la etiqueta `dcc.test=true`
y se comprueba que no queda ninguno. `DCC_VERIFY_ALLOW_ROOT=1` permite
ejecutarlo como root solo en entornos de desarrollo aislados.
