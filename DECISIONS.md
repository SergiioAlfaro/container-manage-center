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

## Fase 3 · Gestión de contenedores

### D-034 · Una sola especificación para validar, crear y traducir
El asistente trabaja sobre `ContainerSpec` (`dcc-engine/src/spec.rs`), un tipo
propio, compartido con la interfaz mediante ts-rs. De él salen, sin duplicar
lógica: la validación (`validate`, con códigos de problema estables y
traducibles, y comprobación en disco de las carpetas de los bind mounts), el
cuerpo de `POST /containers/create` (`to_create_body`, que solo envía lo
establecido) y los argumentos de la CLI equivalente (`cli_args`). La interfaz
pide una vista previa (`container_preview`) que valida y traduce **sin hablar con
Docker** (no aparece en el panel de actividad); al crear, el backend vuelve a
validar y rechaza la especificación con `engine.invalidSpec` y la lista de
problemas sin enviar nada a Docker. Las pruebas reales comparan el tráfico HTTP
capturado con el registro de actividad.

Decisiones derivadas: los bind mounts se envían con la API `Mounts` (no `Binds`)
para que una carpeta inexistente sea un error y no una carpeta creada por root;
la imagen debe existir localmente (la descarga llega en la Fase 4, y el asistente
lo explica sin consultar Docker por imágenes que no están en la lista local).

### D-035 · Entrypoint con varios elementos
La API admite un entrypoint con varios elementos, pero `docker run --entrypoint`
solo acepta un ejecutable. Para que el comando equivalente sea fiel, el primer
elemento va a `--entrypoint` y el resto se antepone al comando, que es como lo
ejecuta Docker. El asistente lo advierte cuando ocurre.

### D-036 · Flujos (logs, estadísticas, terminal)
Cada flujo es **una petición larga y un único registro** en el panel de actividad,
que pasa a "Correcto" al cerrarse con un resumen real (líneas o muestras
recibidas, bytes enviados y recibidos, código de salida). Los datos viajan a la
interfaz por un `Channel` de Tauri; la interfaz guarda el identificador del
flujo y lo cierra al salir de la vista o al cerrar la ventana (`close_all`).
Los cambios de tamaño de la terminal (`POST /exec/{id}/resize`) no se registran
uno a uno: se cuentan en el resumen de la sesión. `exec.create` y `exec.inspect`
sí se registran (no tienen equivalente exacto en la CLI y así se indica).
Los logs se agrupan cada 60 ms o 500 líneas y la interfaz conserva como máximo
10 000 líneas.

### D-037 · Cierre de la terminal
Docker no termina el proceso de un `exec` cuando se desconecta el cliente. Al
cerrar la sesión se envía Ctrl+C, se esperan 200 ms (la TTY descarta la entrada
pendiente al recibir la interrupción, por lo que enviarlos juntos perdía el
Ctrl+D) y se envía Ctrl+D; después se espera hasta 3 s a que el proceso termine y
se consulta `GET /exec/{id}/json`. Si aún sigue en ejecución, el resumen lo dice
expresamente.

### D-038 · Duplicar contenedores
`duplicate_spec` lee la configuración con inspect del contenedor y de su imagen
(por ID) y descarta lo heredado de la imagen (CMD, ENTRYPOINT, ENV, etiquetas,
directorio de trabajo, usuario), las etiquetas `com.docker.compose.*` y el
nombre. Lo que el asistente no puede trasladar (dispositivos, DNS, opciones de
seguridad, tmpfs, ulimits, cpuset, límite de procesos, configuración de logs,
healthcheck y señal de parada personalizados, redes adicionales…) se devuelve
como avisos que la interfaz muestra antes de crear. La prueba real comprueba que
duplicar un contenedor creado con el asistente devuelve exactamente la misma
especificación.

### D-039 · Valores sensibles
Las variables cuyo nombre contiene PASSWORD, PASSWD, SECRET, TOKEN, KEY,
CREDENTIAL o PRIVATE se ocultan en la interfaz (con opción de mostrarlas) y se
sustituyen por `********` en la vista previa del comando y en el comando y el
cuerpo que quedan en el panel de actividad (`ContainerSpec::redacted`). A Docker
se le envían completas: no es cifrado, y la interfaz avisa de que cualquiera con
acceso a Docker puede leerlas. La pestaña Inspección JSON muestra la respuesta de
Docker tal cual, igual que `docker inspect`.

### D-040 · xterm.js 5.5 y `freezePrototype`
La configuración de seguridad de Tauri congela `Object.prototype`
(`freezePrototype: true`). xterm.js 6 incluye código heredado de VS Code que
asigna `toString` a objetos planos, lo que con el prototipo congelado lanza
`TypeError` al cargar el módulo y deja la ventana en blanco (reproducido en
Chromium y en la aplicación real). En lugar de desactivar la protección se usa
xterm.js 5.5.0 (`@xterm/addon-fit` 0.10.0), comprobado con el prototipo
congelado. Además, `public/boot-guard.js` muestra el error si la interfaz falla
antes de arrancar, y `scripts/dev/check-frozen-prototype.cjs` carga la
interfaz compilada con el prototipo congelado (lo ejecuta `verify-phase-3.sh`
si Playwright está disponible).

## Fase 4 · Gestión de imágenes

### D-041 · Construcción: petición `/build` propia, BuildKit por defecto
bollard 0.21 solo construye con BuildKit si se activa su integración gRPC
(sesión, decenas de dependencias) y no admite varias etiquetas (`t` repetido).
Se comprobó contra Docker 29 que `POST /build?version=2` con el contexto en tar
en el cuerpo funciona **sin sesión** (Docker lo trata como contexto remoto
subido), así que la aplicación hace esa petición directamente sobre el socket
(`build/http.rs`: HTTP/1.1 con *chunked transfer encoding*, sin shell ni CLI).

* El progreso de BuildKit llega como mensajes `moby.buildkit.trace` con un
  `StatusResponse` en protobuf (base64). Se decodifica con un lector mínimo de
  protobuf propio (`build/progress.rs`) que solo extrae los campos de
  `api/services/control/control.proto` necesarios (vértices, estados, logs y
  avisos). Se prueba con mensajes construidos a mano y con uno capturado del
  daemon real.
* El constructor clásico (`version=1`, obsoleto) queda como alternativa. Su
  equivalente se muestra como `DOCKER_BUILDKIT=0 docker build …`, y se envía
  `forcerm=1` (`--force-rm`) para que un paso fallido no deje su contenedor.
* La salida se presenta como `docker build --progress=plain` (`#N …`) y como
  lista de pasos con estado y duración.
* El contexto se empaqueta en tar **en flujo** (bloques de 64 KiB) desde un hilo,
  sin cargarlo en memoria. Como el CLI: rutas relativas, propietario 0:0,
  enlaces simbólicos como enlaces y sin archivos especiales.
* `.dockerignore` sigue la semántica de `moby/patternmatcher` (`build/ignore.rs`):
  `*`, `?`, `**`, clases, `\`, excepciones con `!` (gana la última) y exclusión
  de directorios completos. El Dockerfile y el propio `.dockerignore` se envían
  siempre. Las carpetas excluidas no se recorren salvo que una excepción pueda
  incluir algo dentro (p. ej. `node_modules`).
* Antes de construir se muestra el resumen del contexto (archivos, tamaño,
  excluidos, archivos ilegibles) sin hablar con Docker.
* Credenciales: solo se envían (`X-Registry-Config`) las de los registros de las
  imágenes base citadas en `FROM`, no todas las del usuario.

### D-042 · Descarga y credenciales de registro
`POST /images/create` con `fromImage=<nombre completo>&tag=<etiqueta o digest>`,
como el CLI. Las referencias se analizan con la gramática de
`distribution/reference` (`reference.rs`): se rechaza lo mismo que rechazaría
`docker pull` y se normaliza igual (`nginx` → `docker.io/library/nginx:latest`).

La aplicación **no guarda credenciales**. Modos:
* *Las de Docker* (por defecto, como `docker pull`): `credHelpers`/`credsStore`
  de `~/.docker/config.json` (o `$DOCKER_CONFIG`), ejecutando
  `docker-credential-<helper> get` sin shell y con el servidor por la entrada
  estándar; la ejecución se registra, su salida nunca. Si no, la entrada `auths`
  que escribió `docker login`.
* *Ninguna*: descarga anónima.
* *Usuario y contraseña*: solo para esa descarga; viajan en la cabecera
  `X-Registry-Auth` y se descartan al terminar. `PullAuth` no muestra la
  contraseña ni en `Debug` ni en `Display`.

El progreso se agrega por capa y se envía cada 150 ms. Cancelar cierra la
conexión y Docker interrumpe la descarga; el registro queda como fallido con
"cancelada por el usuario". Con el almacén de imágenes de containerd (Docker 29)
el estado final de una capa es "Download complete" en vez de "Pull complete":
se tienen en cuenta ambos.

Las descargas y la construcción viven en almacenes globales de la interfaz para
que continúen si el usuario cambia de apartado.

### D-043 · Pruebas de descarga con un registro local
En el entorno de desarrollo Docker no tiene acceso a Docker Hub. Las pruebas
levantan un registro mínimo (API de distribución v2, `tests/common/registry.rs`)
que sirve una imagen con la capa en gzip y un manifiesto schema 2 (desde la
0.4.1, una imagen nueva en cada prueba; ver D-045), opcionalmente con autenticación básica y
con retraso de blobs para probar la cancelación. El daemon real descarga de él:
la prueba es real de extremo a extremo. Al intentar primero TLS (byte 0x16), el
registro responde en HTTP plano como un servidor Go, que es lo que hace que
Docker pase a HTTP con registros locales.

La búsqueda (`GET /images/search`) la hace el daemon contra Docker Hub; la
prueba acepta ambos resultados (resultados reales con Internet, error de Docker
sin él) y comprueba en los dos el registro y el tráfico. Sin Internet en el
entorno de desarrollo, la búsqueda con resultados solo se ha visto en la interfaz
con datos simulados y se valida en M6.

### D-044 · Eliminación y limpieza
* Con varias etiquetas, el diálogo permite quitar solo una (`docker image rm
  <etiqueta>`) o eliminar la imagen completa por su ID, que exige `--force`.
* Si la usan contenedores, se listan antes de confirmar. Docker decide: no
  elimina imágenes de contenedores en ejecución ni forzando, y quitar una
  etiqueta en uso exige forzar en unas versiones (409) y no en otras (Docker 29
  con containerd); la interfaz muestra el mensaje de Docker tal cual.
* Tras eliminar desde el detalle no se vuelve a pedir la imagen (evita un 404
  inútil en el registro de actividad).
* `docker image prune`: solo imágenes sin etiqueta y sin uso, o todas las que no
  usa ningún contenedor (confirmación adicional). El motor admite filtrar por
  etiqueta, que usan las pruebas para no tocar imágenes ajenas.
* "Descargar de nuevo" se ofrece para toda imagen con nombre: con el almacén de
  containerd también las imágenes construidas localmente tienen digest, así que
  no sirve para saber si vienen de un registro.

### D-045 · Pruebas de descarga independientes del almacén de imágenes
**Contexto.** En Debian 13 (Docker 26.1.5, almacén clásico overlay2) fallaron
dos pruebas de descarga de la 0.4.0. El registro de prueba servía la imagen de
prueba exportada del propio daemon. El almacén clásico reconoce la imagen por su
configuración y no descarga capas; el de containerd compara capas comprimidas y
la recomprimida le parecía nueva. Las pruebas dependían, sin saberlo, del almacén.

**Decisión.**
* El registro de prueba genera en cada prueba una imagen nueva (capa aleatoria,
  etiqueta `dcc.test=true`) que ningún daemon puede tener, y puede enviar las
  capas a ritmo controlado y anotar los blobs cuya conexión cierra el cliente.
* La prueba de descarga exige progreso parcial real (`Downloading` con
  0 < actual < total y total igual al tamaño servido), una sola petición de la
  capa y "1 capas descargadas, 0 ya existían".
* La prueba de cancelación exige: fin de la operación en menos de 6 s, que
  Docker pidiera la capa y **no la recibiera entera** (el registro la envía
  despacio, así que si Docker siguiera descargando la completaría), que no
  quede imagen y que una nueva descarga baje la capa entera (no quedan restos).
  Con containerd exige además que el corte sea inmediato.
* El caso que ocurrió en Debian 13 tiene su propia prueba, con aserciones
  exactas para cada almacén.

**Comportamiento de Docker con el almacén clásico.** Al cancelar, el daemon
deja de procesar la descarga al momento (`context canceled`), pero su conexión
con el registro sigue abierta hasta que el registro empieza a responder; entonces
la corta. `docker pull` interrumpido con Ctrl+C se comporta igual (comprobado con
Docker 29 y 24). La aplicación no puede hacer más que el CLI: cerrar la petición
a Docker. No queda imagen ni capa.

**Comprobado con** Docker 29.9.2 (containerd), Docker 29.9.2 (clásico) y
Docker 24.0.7 (clásico).

### D-046 · Fechas ausentes
Docker envía la fecha cero de Go (0001-01-01T00:00:00Z, `Created: -62135596800`)
para objetos sin fecha, por ejemplo imágenes cuya configuración no la incluye.
El CLI muestra "N/A"; la interfaz mostraba "hace 2027 años". Las fechas
anteriores a 1970 se muestran como "—". El cambio está en el formateador
compartido (`shared/format.ts`), así que también afecta a la lista de
contenedores; las fechas válidas se muestran exactamente igual que antes.

### D-047 · Redes: comportamiento de Docker que determina el diseño
Comprobado con Docker 29.9.2 (containerd) y 24.0.7 (almacén clásico):

* `GET /networks` no incluye los contenedores, y `GET /networks/{id}` solo los
  que están en marcha. Los contenedores de una red (en marcha o no) se obtienen
  cruzando con la lista de contenedores por nombre de red; el inspect aporta la
  IP y la MAC de los que están en marcha.
* `DELETE /networks/{id}` falla con 403 ("has active endpoints") si hay
  contenedores en marcha, pero **elimina** la red aunque la tengan configurada
  contenedores detenidos, que después no arrancan ("network … not found").
  `docker network prune` también elimina esas redes. La interfaz avisa y exige
  una confirmación expresa en ambos casos; la estimación de la limpieza las
  incluye.
* Docker acepta nombres de red con espacios (por la API). Las operaciones usan
  el ID; la validación de la aplicación solo exige nombres sencillos al crear.
* Docker solo admite IP fija si la subred se indicó al crear la red, y alias
  solo en redes creadas por el usuario. La aplicación valida la IP contra las
  subredes conocidas y oculta los alias en `bridge`; el resto lo decide Docker,
  cuyo error se muestra tal cual.

### D-048 · Creación de redes con `CheckDuplicate`
Docker < 25 (API < 1.44; Debian 12 trae 20.10) crea una segunda red con el
mismo nombre si la petición no lleva `CheckDuplicate: true` (solo devuelve un
aviso); el CLI de esas versiones lo envía. Docker 25 retiró el campo porque
siempre comprueba el nombre, y bollard 0.21 ya no lo tiene. La creación se hace
con una petición propia (`build/http.rs`, `post_json`) con el cuerpo de
`NetworkSpec::wire_body()`: el de `to_create_body()` más `CheckDuplicate: true`.
Las versiones modernas ignoran el campo. Comprobado: con Docker 24.0.7 sin el
campo se crean dos redes `dcc-dup`; con él, 409. La prueba real exige que tras
el intento solo exista una red con ese nombre.

### D-049 · Los diálogos de redes consultan a Docker al abrirse
Visto en la aplicación real: con la lista sin actualizar (redes creadas desde la
terminal), el diálogo de limpieza estimaba "no hay redes que eliminar" cuando
Docker iba a eliminar dos. Eliminar, limpiar y conectar consultan a Docker al
abrirse (`refetchOnMount: "always"`) y no permiten confirmar hasta tener los
datos actuales; si la red ya no existe, lo dicen. El diálogo de limpieza de
imágenes (Fase 4, validada) tiene el mismo riesgo; no se ha modificado (queda
propuesto para que el propietario decida).

### D-050 · Limpiar imágenes hace fallar las descargas en curso (containerd)
**Hallazgo.** Durante la verificación de la Fase 5 falló una vez la prueba de
descarga de la Fase 4 con un error del propio Docker ("failed commit on ref …
rename … no such file or directory"). En el registro del daemon, en el mismo
instante: "Error deleting lease … not found". Otra prueba ejecutaba a la vez
`POST /images/prune`.

**Reproducción con el CLI oficial, sin la aplicación** (registro local, 15
descargas mientras otro proceso repite la operación):

| Docker | Operación concurrente | Descargas fallidas |
|---|---|---|
| 29.9.2, containerd | ninguna | 0/15 |
| 29.9.2, containerd | `docker tag` + `docker rmi` | 0/15 |
| 29.9.2, containerd | `docker image prune -f --filter label=<inexistente>` | 15/15 ("lease does not exist") |
| 24.0.7, clásico | `docker image prune -f --filter label=<inexistente>` | 0/15 |

Una construcción con BuildKit no se ve afectada.

**Decisión.**
* Pruebas: la prueba que limpia imágenes no se solapa con las que descargan
  (`RwLock` en `live_images.rs`). No se ha relajado ninguna aserción.
* Aplicación (cambio en la Fase 4, con esta causa demostrada): el diálogo
  **Limpiar** de Imágenes no permite limpiar mientras haya descargas en curso en
  la aplicación y explica el motivo. Una limpieza lanzada desde fuera (CLI)
  durante una descarga sigue haciendo fallar la descarga; la aplicación muestra
  el error real de Docker.

### D-051 · Volúmenes: comportamiento de Docker que determina el diseño
Comprobado con Docker 29.9.2 (containerd) y 24.0.7 (clásico):

* `POST /volumes/create` con un nombre existente devuelve 201 con el volumen
  existente, **sin aplicar** el driver, las opciones ni las etiquetas pedidas
  (el CLI tampoco avisa). La aplicación comprueba antes con
  `GET /volumes?filters={"name":[…]}` (coincidencia exacta en el código) y
  devuelve `engine.invalidSpec` con `volumeName.exists`; el asistente también
  avisa mientras se escribe con la lista actual.
* `DELETE /volumes/{nombre}` falla con 409 si lo monta cualquier contenedor, en
  marcha o detenido, también con `force` (que solo sirve para errores del
  driver): la aplicación no ofrece forzar.
* `POST /volumes/prune` (API ≥ 1.42) elimina por defecto solo los anónimos sin
  uso; con `all=true`, también los que tienen nombre. Antes de la API 1.42
  elimina todos los sin uso y no admite `all`: la interfaz lo detecta por la
  versión de la API, avisa, no envía `all` y estima con todos (no probado con
  un daemon real de esa versión).
* El tamaño solo lo da `GET /system/df?type=volume`. Docker ≤ 28 lo devuelve en
  `Volumes`; Docker 29 en `VolumeUsage.Items` (y `Volumes` vacío según los
  parámetros); el tipo de bollard 0.21 solo conoce el formato nuevo. Se usa una
  petición propia (`build/http.rs`, `get_json`) y se leen los dos formatos. Se
  calcula bajo demanda porque puede ser lento.
* Con `HostConfig.Mounts`, Docker rechaza una carpeta que no existe; con
  `HostConfig.Binds` (`-v`) la crea como root al arrancar. La lista de
  directorios vinculados lo explica cuando una ruta ya no existe.

### D-052 · Directorios vinculados
No son un objeto de Docker: se obtienen de los montajes `bind` de la lista de
contenedores y se agrupan por ruta del equipo. El estado de la ruta (existe,
tipo, uid, permisos, si el usuario puede leerla) se consulta en el sistema de
archivos local, que es el del daemon porque el socket es local (también con
Docker rootless); no es una operación de Docker y no se registra. Se marcan el
socket de Docker, la raíz y los directorios del sistema (la misma lista que usa
el asistente de contenedores, más /usr, /bin, /sbin, /lib y /var/lib/docker).
Como Docker no permite añadir montajes a un contenedor existente, "vincular una
carpeta" abre el asistente de contenedores con el montaje (selector de carpetas
del sistema).

### D-053 · Docker Compose: alcance y diseño (Fase 7)
Compose se ejecuta **siempre con el CLI oficial** (plugin `docker compose` o
binario `docker-compose` v2, detectados según D-032), con argumentos separados
y sin shell (`cli::run_streaming`). Cada ejecución es una operación del panel de
actividad con el programa y los argumentos exactos.

* **Proyectos**: los que tienen contenedores se descubren por las etiquetas
  `com.docker.compose.*` de la lista real de contenedores (proyecto, servicio,
  archivos, carpeta, número, `oneoff`), sin ejecutar Compose. Los que añade el
  usuario se guardan en la configuración (`[[compose.projects]]`, nombre y
  archivos) y se combinan con los desplegados por nombre. `config_set` no puede
  cambiar esa lista (la interfaz envía la configuración completa y una copia
  antigua la borraría): solo `compose_register` y `compose_unregister`.
* **Identidad**: las acciones usan siempre `-p NOMBRE` y, si existen, los
  archivos (`-f`) y `--project-directory`. El nombre de un proyecto añadido lo
  decide Compose (`config --format json` sin `-p`: el `name:` del archivo o la
  carpeta) salvo que el usuario indique otro. Si los archivos ya no existen,
  parar, iniciar, reiniciar, eliminar y ver registros siguen funcionando solo con
  las etiquetas (comprobado con Compose 2.26.1 y 5.6.1); desplegar, descargar y
  construir se rechazan sin ejecutar nada.
* **Modelo de servicios**: `config --format json` con `--profile "*"`, para ver
  también los servicios de perfiles. De las variables de entorno solo se
  conservan los nombres. La configuración resuelta (con los valores) se envía a
  la interfaz, que la oculta hasta que se pide, y **no se guarda en el registro
  de actividad** (la salida estándar de `config` no se registra).
* **Acciones**: `up --detach` (servicios, perfiles, construir, política de
  descarga, recrear, huérfanos), `down` (volúmenes, imágenes, huérfanos; exige
  confirmación explícita en el backend), `start`, `stop`, `restart`, `pull` y
  `build`, con la salida en vivo. Una sola acción a la vez por proyecto
  (cerrojo `compose:NOMBRE`). Los nombres de servicio y perfil no pueden empezar
  por `-` y van tras `--`, de modo que nunca se interpretan como opciones.
* **Cancelación**: el proceso se lanza en su propio grupo; detener envía
  SIGTERM al grupo (también al plugin, hijo de `docker`) y SIGKILL a los 8 s. La
  operación queda como cancelada con la salida capturada. Lo que Compose ya
  hubiera hecho se mantiene (se explica en la interfaz).
* **Registros**: `logs --follow --no-color --tail N [--timestamps] [-- servicios]`,
  una operación que se cierra al dejar de mirar; las líneas no se guardan en el
  registro.
* Compose se ejecuta desde una carpeta vacía propia (`$XDG_RUNTIME_DIR` o la
  temporal, `dcc-compose-UID`, 0700) para no cargar nunca por accidente un
  `compose.yaml` de la carpeta actual de la aplicación, y con `DOCKER_HOST` del
  Docker al que está conectada la aplicación.

### D-054 · Compose omite en silencio los servicios de perfiles no activos
Comprobado con Compose 2.26.1 (Debian 13) y 5.6.1: con `-f`, `down`, `stop`,
`start` y `restart` solo actúan sobre los servicios de los perfiles activos y
terminan con código 0. Tras desplegar un servicio con `--profile debug`, un
`down` dejaba ese contenedor en marcha y la red "en uso". La aplicación pasa
`--profile "*"` en esas cuatro acciones para que actúen sobre todos los
contenedores del proyecto (comprobado con y sin archivos, en las dos versiones).
Desplegar, descargar y construir siguen los perfiles que elige el usuario. Lo
detectó la prueba real `full_lifecycle_with_build_profiles_and_confirmation`.

### D-055 · Edición de archivos Compose
El editor valida el contenido **sin guardarlo**: `config` con `-f -` en la
posición de ese archivo y el contenido por la entrada estándar (los demás
archivos y la carpeta del proyecto se mantienen, así que las rutas relativas y
el `.env` se resuelven igual). Guardar valida primero y, si Compose lo rechaza,
solo guarda si el usuario insiste. El guardado es atómico (temporal en la misma
carpeta, `fsync`, mismos permisos, `rename`) y se niega si el archivo ha
cambiado desde que se leyó (huella SHA-256): `engine.fileChanged`. Crear un
proyecto escribe `compose.yaml` con `O_EXCL` solo si la carpeta no tiene ya un
archivo Compose (`engine.alreadyExists`). Leer y escribir archivos no es una
operación de Docker y no se registra en el panel de actividad. Tamaño máximo
1 MiB, UTF-8.

