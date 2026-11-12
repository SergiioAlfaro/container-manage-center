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

### D-056 · Estado de los proyectos Compose confirmado con inspect (corrección 0.7.1)
**Problema observado** en Debian 13 (Docker 26.1.5, Compose 2.26.1):
`full_lifecycle_with_build_profiles_and_confirmation` y
`lifecycle_without_files_and_refusal_of_actions_that_need_them` esperaban
`Stopped` tras detener el proyecto y obtuvieron `Partial`.

**Causa:** la misma carrera de Docker que D-031. `docker compose stop` termina
cuando Docker ha respondido a cada `POST /containers/{id}/stop`, pero Docker
escribe el estado en la réplica de la que lee `GET /containers/json` *después*
de responder. La aplicación calculaba el estado del proyecto con ese listado
justo después de la acción y, en esa ventana, un contenedor seguía `running`. El
cálculo del estado era correcto: los datos estaban desfasados. Afectaba también a
la interfaz (tras detener podía verse "Parcial").

**Reproducido** aquí con Docker 26.1.3 (Ubuntu, misma rama 26.1): las dos pruebas
fallaron en 2 de 3 ejecuciones. Con un segundo proyecto como carga, en 2 de 30
ciclos `GET /containers/json` daba un contenedor `running` justo después de
`docker compose stop`, mientras inspect y `docker compose ps -a` daban los dos
`exited`. No se reprodujo con Docker 24.0.7 ni 29.9.2.

**Solución:** tras desplegar, iniciar, detener o reiniciar (también si la
acción falla o se interrumpe), `compose_run` confirma con inspect cada contenedor
del proyecto (`compose_settle`, que reutiliza `container_state` de D-031 sin
cambiarlo; cada consulta es una operación registrada "Comprobar estado del
contenedor"; un contenedor que ya no existe se omite). Inspect espera al bloqueo
del contenedor, así que al responder el listado ya es coherente; el evento de fin
llega a la interfaz después, y su recarga ve el estado real. `down` no la
necesita (elimina los contenedores) y descargar o construir no tocan
contenedores.

**Pruebas:** `tests/stale_compose.rs` (servidor que responde como Docker 26 en
la ventana: falla sin la confirmación), pruebas unitarias del cálculo del estado
para cada caso, y `project_state_matches_docker_after_every_operation`, que tras
cada operación individual y global compara el estado de la aplicación con
`docker compose ps -a --format json` e inspect de cada contenedor (con Docker 26
falla sin la corrección y pasa con ella).


### D-057 · Eventos de Docker en tiempo real (Fase 8)
* **Una sola suscripción** a `GET /events` mientras la aplicación está conectada
  (`spawn_events_watcher`). Se abre al conectar, se cancela al desconectar y,
  si Docker la corta, se reintenta a los 3 s (y se avisa de la pérdida de
  conexión si procede). Se usa la petición HTTP propia (`build::http::get_stream`)
  porque bollard no entrega nada hasta el primer evento y así se sabe cuándo
  Docker la ha aceptado.
* **Registro en el panel de actividad**: la suscripción se registra como una
  operación terminada correctamente en cuanto Docker la acepta ("Suscripción a
  los eventos de Docker abierta", con el estado HTTP), y una operación fallida
  aparte si se interrumpe. No se deja una operación "en curso" permanente, que
  ocultaría las operaciones reales y nunca terminaría. Cada evento **no** es una
  operación (no lo inicia la aplicación); se ve en "Eventos y registros".
* **Actualización de la interfaz**: cada evento se envía a la ventana
  (`docker://event`) y se traduce en las consultas que hay que repetir
  (`invalidationsFor`): contenedor → contenedores, panel, Compose, directorios
  vinculados, redes y volúmenes; imagen → imágenes; volumen → volúmenes y
  tamaños; red → redes y contenedores; daemon → panel. Las acciones `exec_*`
  (las genera la propia terminal y las comprobaciones de salud) no recargan
  nada. Las recargas se agrupan 250 ms (un `compose up` produce decenas de
  eventos). La recarga se hace pidiendo los datos a Docker de nuevo: el evento
  nunca se usa como fuente del estado.
* **Consultar a Docker** (`events_query`): `GET /events?since&until&filters`,
  con `until` obligatorio para que Docker cierre. Docker solo conserva en memoria
  los **últimos 256 eventos** y los pierde al reiniciarse (comprobado con 29.9.2):
  la interfaz lo explica. Con `until` entero Docker excluye los eventos del
  segundo en curso, así que si `until` ≥ ahora se envía con nanosegundos
  (`1794068958.714492246`). Filtros validados (tipos conocidos, acciones y
  recurso sin espacios ni `-` inicial, `since` ≤ `until`, como mucho 5000
  resultados) **antes** de llamar a Docker. Se muestra el `docker events`
  equivalente.
* `timeNano` supera la precisión entera de JavaScript; la interfaz suma 1 µs
  antes de truncar a milisegundos.

### D-058 · Historial de operaciones y diario de eventos persistentes
* Dos archivos JSONL en la carpeta de estado: `historial.jsonl` (operaciones
  terminadas, máximo 10 000) y `eventos.jsonl` (eventos recibidos con la
  aplicación abierta, máximo 20 000). Carpeta: `DCC_STATE_DIR`, si no
  `$XDG_STATE_HOME/docker-control-center` (por defecto
  `~/.local/state/docker-control-center`) y, si se usa otra configuración
  (`DCC_CONFIG`), la carpeta `estado` junto a ella.
* Permisos 0700 (carpeta) y 0600 (archivos), también si existían con otros. Se
  rechazan enlaces simbólicos y carpetas en lugar del archivo. Las líneas dañadas
  se descartan y se avisa al iniciar; el archivo se compacta al abrir y al
  superar el máximo en un 25 %.
* **Sin secretos**: del historial se guarda tipo, recurso, categoría, estado,
  fechas, duración, resumen, error, código HTTP o de salida, y el método y la
  ruta o el programa y los argumentos. **No** se guardan el cuerpo de la
  petición (variables de entorno, comandos de `exec`), el comando equivalente
  (`-e CLAVE=valor`, `--build-arg`) ni las salidas. En la ruta se ocultan los
  valores de `buildargs` (se conservan los nombres). Los eventos de Docker no
  contienen variables de entorno. Comprobado buscando el valor de una variable
  secreta en los archivos tras crear un contenedor con ella (prueba IPC y prueba
  manual).
* Opción **Guardar historial** (`activity.keep_history`, activada por defecto):
  si se desactiva no se escribe nada nuevo (lo guardado se puede borrar desde la
  vista). El panel de actividad de la sesión no cambia.
* Lo guardado es un **registro**, no el estado actual: el estado siempre se pide
  a Docker. Si la aplicación estaba cerrada no hay eventos de ese periodo en el
  diario (para eso está "Consultar a Docker", limitado a los 256 últimos).

### D-059 · Compatibilidad del paquete con Debian 12+ y Ubuntu 22.04+ (Fase 9)
* **Problema comprobado**:
  * El binario de la 0.8.0, compilado en Ubuntu 24.04, exige `GLIBC_2.39`, por las funciones `pidfd_spawnp` y `pidfd_getpid`, que la biblioteca estándar de Rust usa al lanzar procesos. Esas referencias son débiles, pero la dependencia de versión no (`Flags: none`), así que el cargador rechaza el binario.
  * Ejecutado en un Ubuntu 22.04 real (con debootstrap): ``version `GLIBC_2.39' not found``.
  * Encima, el paquete de Tauri declaraba solo `Depends: libwebkit2gtk-4.1-0, libgtk-3-0`, así que apt lo instalaba igualmente en Debian 12 o Ubuntu 22.04 y la aplicación no arrancaba.
  * Lo mismo pasa con un paquete compilado en Debian 13 (glibc 2.41).
* **Solución, primera parte: dependencias reales**:
  * `scripts/deb-set-depends.sh` calcula las dependencias con `dpkg-shlibdeps` (la herramienta estándar de Debian) a partir del propio binario y las escribe en el paquete. Así apt se niega a instalarlo donde no puede funcionar.
  * Ejemplo en Ubuntu 24.04: `libc6 (>= 2.39), libgtk-3-0t64 (>= 3.21.5), libwebkit2gtk-4.1-0 (>= 2.41.90)…`.
  * `verify-phase-9.sh --deb` lo aplica y comprueba que el `libc6 (>= X)` declarado cubre la glibc más nueva que exige el binario (`objdump -T`).
* **Solución, segunda parte: paquete para distribuir**:
  * `scripts/build-deb-compat.sh` compila dentro de un contenedor de Docker con Ubuntu 22.04 (`packaging/build/Dockerfile.ubuntu2204`, con Rust 1.91 de `jammy-updates`).
  * La compilación se hace sin red (`cargo fetch` previo en el equipo y `cargo build --offline --locked`) y con el usuario que lo ejecuta. Empaqueta con Tauri y calcula `Depends` contra las bibliotecas de Ubuntu 22.04, que además tienen los nombres anteriores a la transición `t64`.
  * No necesita sudo ni cambia el sistema.
  * El binario resultante exige `GLIBC_2.34` como mucho: vale para Ubuntu 22.04 (2.35) y Debian 12 (2.36).
  * Verificado arrancándolo en el Ubuntu 22.04 real, conectado a Docker 29.
* **Debian 12 no se ha podido probar en ejecución** (no hay acceso a sus repositorios desde el entorno de desarrollo). Su compatibilidad se apoya en dos hechos comprobados: la versión de glibc que exige el binario y que WebKitGTK 4.1 está en Debian 12.

### D-060 · Comprobación de versiones nuevas (Configuración → Actualizaciones)
* **Qué hace**:
  * La aplicación **solo consulta y avisa**: nunca descarga ni instala nada. Actualizar es instalar el paquete nuevo con apt, que necesita los permisos del usuario.
  * Por defecto **no se conecta a Internet**: hace falta indicar una fuente y pulsar «Comprobar ahora», o activar «Comprobar al iniciar». Esa comprobación al iniciar está desactivada por defecto y se hace 5 s después de arrancar.
* **Fuentes admitidas**:
  * `propietario/repositorio` de GitHub, que se consulta en `api.github.com/repos/…/releases/latest`.
  * La dirección `https://` de un JSON con `version` y, opcionalmente, `url`, `notes` y `date`.
  * `http://` solo hacia la propia máquina, y sin proxy.
  * La fuente oficial queda **pendiente de decidir** junto con el nombre y el lugar de publicación (Fase 10).
* **Límites y seguridad**:
  * Solo HTTPS, con `reqwest` + `rustls` (motor `ring`, sin dependencias de sistema nuevas), los certificados del sistema y `HTTPS_PROXY`.
  * Las redirecciones también deben ser HTTPS.
  * 15 s de espera como máximo y respuestas de 1 MiB como máximo.
  * Se ignoran las versiones preliminares y los borradores. Las versiones se comparan con semver.
  * Solo se ofrece abrir enlaces `https://`, con `xdg-open` y sin shell.
* **Qué no hace**: la consulta no es una operación sobre Docker, así que no aparece en el panel de actividad. El resultado se ve en Configuración y, si hay una versión nueva al iniciar, en un aviso discreto arriba.

### D-061 · Rendimiento: medido antes y después
* **Medición**:
  * Release, Xvfb sin aceleración, Docker 29, con 200 contenedores (40 en marcha), 60 volúmenes y 30 redes.
  * Un proxy de socket cuenta las peticiones a Docker.
  * La CPU sale de `/proc/PID/stat` y la memoria es PSS (`smaps_rollup`).
* **Hallazgos**:
  * En reposo, la aplicación hacía 6 peticiones por minuto (`/_ping`) y cada apartado hacía 1 o 2 peticiones, sin patrones de una petición por recurso. Ahí no había nada que mejorar.
  * El problema estaba en las **ráfagas**: arrancar 100 contenedores desde la terminal con la lista abierta costaba **23 s de CPU** en la interfaz, más de media CPU durante toda la ráfaga.
  * El motivo es que se recargaba con cada evento y se volvía a pintar la tabla completa, y además cada fila creaba formateadores de `Intl` nuevos.
* **Cambios**:
  * Formateadores de `Intl` creados una vez y reutilizados.
  * Filas memorizadas en Contenedores y Eventos. TanStack Query conserva los objetos que no cambian, así que solo se repintan las filas que cambian.
  * Como mucho una recarga por segundo durante las ráfagas; la primera sigue llegando a los 250 ms.
  * Cada apartado y xterm se cargan al abrirlos y se precargan 2 s después del arranque. El JavaScript inicial pasa de 1044 KB a 500 KB.
* **Resultado** (versión final, mismas condiciones):
  * En la misma ráfaga, la CPU de la interfaz baja de 23,0 s a **3,1 s**, la del proceso principal de 1,4 s a 0,7 s y las peticiones de 77 a 25.
  * Las primeras visitas a los apartados tardan 400–510 ms hasta que la pantalla queda estable, frente a 430–530 ms en la 0.8.0. Docker Compose baja de 1374 a 399 ms.
  * En reposo no cambia nada: 6 peticiones por minuto, y la primera petición a Docker sale a los 0,26 s del arranque.
  * La memoria total (PSS) sube un 4 %, de 231 a 240 MB: 99 MB el proceso principal, 120 MB WebKit y 19 MB la red de WebKit.
* **Corrección durante la medición**: con `React.lazy`, la precarga no bastaba. Un componente diferido suspende siempre la primera vez aunque su código ya esté descargado, y mostraba «Cargando…» un instante: la primera visita pasó a 650–800 ms. Ahora, una vez precargado, cada apartado se pinta con su componente directo.
* **Fallo encontrado al revisar**:
  * La lista de eventos en directo usaba la posición en la clave de cada fila. Con cada evento nuevo se volvían a crear las 2000 filas y se cerraba el detalle que el usuario tuviera desplegado.
  * Corregido con claves estables y cubierto por una prueba que fallaba antes del cambio.

### D-062 · Errores en el idioma de la interfaz y formato de fechas único
* **Errores**:
  * Los errores con texto fijo que genera la propia aplicación se muestran en el idioma de la interfaz: `engine.notConnected`, `engine.timeout`, `engine.notConfirmed`, `engine.composeUnavailable`, los de la fuente de actualizaciones y el de la interfaz ejecutada fuera de la aplicación de escritorio. La traducción se hace en un único punto (`toCommandError`).
  * Los que llevan detalle técnico se muestran tal cual, porque ese detalle es lo útil: el mensaje de Docker, una ruta, un valor no válido.
  * Una prueba exige texto en español y en inglés para cada código traducido.
* **Fechas**:
  * Las marcas de tiempo de registros (panel de actividad, eventos, historial, logs) usan siempre el formato de 24 horas, con fecha y milisegundos cuando hace falta. Antes, en inglés, el historial mostraba «06:08:33 PM.904».
  * Las fechas de objetos de Docker usan el mismo formato de fecha.
  * Los campos de fecha que se escriben siguen siendo el selector nativo del sistema.
* **Configuración**:
  * Sección **Sistema**: aplicación, paquete instalado (leído de la base de datos de dpkg, solo lectura), WebKitGTK, glibc, sistema operativo, escritorio, Docker y Compose. Incluye un informe para copiar sin proyectos, variables ni credenciales.
  * **Restablecer preferencias**: apariencia, panel de actividad y actualizaciones, con confirmación. No toca la conexión con Docker ni los proyectos Compose.

## Actualización 0.9.1 (antes de la Fase 10)

### D-063 · Terminales: cada sesión limpia sus procesos y sabe qué le pasó al contenedor
* **Problema**: al cerrar una sesión, `docker exec` solo termina la shell. Un proceso que la shell dejó en segundo plano (`sleep 100 &`) o un programa que ignora Ctrl+C seguía vivo dentro del contenedor. Además, cuando la sesión terminaba porque el contenedor se detenía o se eliminaba, la interfaz no lo sabía.
* **Solución**:
  * La shell imprime su PID al empezar con una secuencia OSC propia (`ESC]6973;PID BEL`). El backend la retira de la salida (`PidMark`, busca en los primeros 4 KB), así que el usuario no la ve.
  * Al cerrar: Ctrl+C, Ctrl+D y espera (D-037). Si quedan procesos de esa sesión (mismo identificador de sesión del núcleo), se terminan con un `exec` sin TTY que solo usa órdenes internas de `sh` (`KILL_SESSION_SCRIPT`). Primero mueren los que no son el líder; después, el líder si sigue vivo. Los zombis se ignoran. Queda registrado como `container.exec.cleanup`.
  * Cuando la sesión termina sola, se inspecciona el contenedor. Si se detuvo o se eliminó (`removing` cuenta como eliminado), el final lo indica (`StreamEnd.container`). Si el código de salida es ≥128, se espera hasta 3 s a que Docker actualice el estado (la misma carrera que en D-031).
  * Al cerrar la ventana se cierran todos los flujos y se espera a que terminen (como mucho 8 s) antes de cerrarla, para que no queden shells ni procesos.
* **Cambio de comportamiento deliberado**: `ShellChoice::argv()` ya no ejecuta la shell directamente, sino `sh -c 'printf …; exec bash'`. La prueba unitaria `shell_choices` se actualizó en consecuencia. La terminal del detalle del contenedor (Fase 3) usa el mismo motor y gana esta limpieza, sin cambios en su interfaz.

### D-064 · Apartado «Terminales»: sesiones fuera de React
* **Decisión**: xterm y el identificador del flujo de cada sesión viven en un gestor fuera de React (`features/terminals/sessions.ts`); el estado visible vive en un store de Zustand. El panel de cada sesión solo «presta» su elemento a la terminal mientras está montado.
* **Por qué**: así, cambiar de apartado no cierra las sesiones ni pierde su contenido, y recolocar los paneles al abrir o cerrar uno no reinicia ninguna sesión.
* **Alternativa descartada**: un componente por sesión dueño de xterm (como el panel del detalle). Cada cambio de distribución desmontaría los paneles y cerraría las sesiones.
* **Distribución**:
  * 1 sesión ocupa todo; 2 van en columnas; 3, una a la izquierda y dos a la derecha; 4, en cuadrícula 2×2.
  * **Más de 4**: filas de dos con desplazamiento vertical, en lugar de pestañas. Con pestañas solo se vería una sesión a la vez, y lo que se pidió es ver varias.
  * Los separadores se arrastran o se mueven con el teclado (20–80 %). Maximizar muestra una sola sesión sin cerrar las demás.
* **Límite**: 12 sesiones a la vez. Cada una mantiene una shell dentro de un contenedor y una terminal en memoria.
* **Sin sesiones huérfanas**: cerrar una sesión que aún se está abriendo cierra el flujo en cuanto llega su identificador. Los eventos de una apertura anterior se ignoran gracias a un contador de generación.
* **Panel de actividad**: registra la apertura, el cierre, los errores y la limpieza, nunca la entrada ni la salida de la terminal.

### D-065 · Desinstalación selectiva con APT
* **Qué se ofrece quitar**: solo el paquete de la aplicación y las dependencias que cumplen las tres condiciones siguientes:
  1. APT las instaló automáticamente **en una operación que también instaló o actualizó la aplicación**, según `/var/log/apt/history.log`, incluidos los archivos rotados `.gz`, y desde su última desinstalación.
  2. Siguen marcadas como automáticas (`apt-mark showauto`).
  3. APT las quitaría al desinstalar la aplicación (`apt-get -s remove --auto-remove`).
* **Todo lo demás se conserva y se explica**:
  * Compartidas: se indica qué paquetes las usan, según `apt-cache rdepends --installed`.
  * Instaladas de forma explícita.
  * Ya existentes antes de la aplicación.
  * Origen incierto: si el historial no muestra la instalación, por ejemplo tras `dpkg -i`, no se atribuye nada a la aplicación.
  * Protegidas: Docker y su motor (`docker*`, `containerd*`, `runc`, `moby-*`), APT, dpkg, polkit y los paquetes esenciales, `required` o `important`.
* **Antes de ejecutar**:
  * Se repite el análisis y se simula exactamente `apt-get remove --no-auto-remove <selección>`. Si APT quitaría algo no elegido, no se ejecuta.
  * Las dependencias solo se pueden quitar junto con la aplicación.
  * Los nombres se validan según Debian Policy.
  * No se usa `purge`. Nunca se borran los datos del usuario ni nada de Docker.
* **Privilegios**: solo la orden de APT se ejecuta con privilegios, con `pkexec /usr/bin/apt-get remove -y --no-auto-remove …`, que muestra el diálogo de autenticación del sistema. Si el usuario cancela (código 126) o no se conceden los permisos (127), se informa y no cambia nada.
* **Sin pkexec**: se muestra la orden equivalente con `sudo` para ejecutarla a mano. La aplicación nunca guarda ni pide contraseñas.
* **Registro**: cada consulta (`packages.query`), simulación (`packages.simulate`) y desinstalación (`packages.remove`) es un proceso CLI sin shell que queda en el panel de actividad, con `LC_ALL=C` para poder interpretar su salida.
* **Pruebas reales**: sobre un Ubuntu 22.04 creado con `debootstrap` (`scripts/prepare-uninstall-chroot.sh`), con Docker y Compose instalados antes. La prueba instala la aplicación con APT, instala zenity, que comparte GTK, comprueba la clasificación, prueba una cancelación y desinstala de verdad.
* **Alternativas descartadas**:
  * `apt-get autoremove`: quitaría paquetes prescindibles que no tienen nada que ver con la aplicación.
  * Ejecutar la aplicación con privilegios.
  * Un ayudante con setuid.

### D-066 · Ayuda en la aplicación y enlace a GitHub
* **Documento**: un archivo Markdown por idioma (`ui/src/features/help/docs/*.md`) con 24 secciones identificadas por `{#id}`. Para ampliarlo basta con editar esos archivos. Una prueba exige las mismas secciones, en el mismo orden, en los cuatro idiomas.
* **Lectura**: un lector propio y mínimo (párrafos, listas, tablas, código, `código` y **negrita**) que produce elementos de React y nunca inserta HTML. Se descartó una biblioteca de Markdown con `innerHTML`.
* **Interfaz**: índice, búsqueda, anterior y siguiente, como subpágina de Configuración.
* **Enlace a GitHub**: plugin oficial `tauri-plugin-opener`, con permiso limitado en `capabilities/default.json` a `https://github.com/SergiioAlfaro`. La ventana no puede abrir ninguna otra dirección por esta vía.
* **Sin cambios**: `open_release_page` (Fase 9, validada) sigue igual con `xdg-open`.

### D-067 · Francés e italiano
* **Idiomas**: `Language` admite `fr` e `it` en Rust y TypeScript; se guardan en `config.toml` como el resto y los formatos de fecha y número siguen el idioma.
* **Traducción**: `fr.json` e `it.json` traducen todas las claves (unas 1700).
* **Pruebas**:
  * Las claves coinciden en los cuatro idiomas y ningún texto está vacío.
  * Cada traducción conserva las variables `{{…}}`.
  * Ninguna cadena de francés o italiano es igual a la inglesa, salvo una lista explícita por idioma de nombres propios, términos técnicos y palabras que se escriben igual.
* **Límite conocido (sin cambios respecto a la 0.9.0)**:
  * Los resúmenes del panel de actividad que escribe el backend y los errores con datos concretos se generan en español en todos los idiomas.
  * Los errores con texto fijo sí se traducen (D-062, ampliado con los de la desinstalación).
  * Traducir el backend exigiría claves y argumentos en todos los mensajes de las fases 1–9, que están validadas; queda como mejora posible y se indica en la ayuda.

### D-068 · Estados de interacción
* **`Button` compartido**: estados de encima, pulsado y ocupado (`loading`), con transiciones de 150 ms.
* **Cambio deliberado en `Button`**: un botón desactivado ya no usa `pointer-events: none`, para que siga mostrando su descripción emergente (que suele explicar por qué no se puede usar). Los estilos de encima solo se aplican si está activo (`enabled:`). Toda la suite anterior sigue pasando.
* **Reglas globales de baja especificidad** (`:where`): filas de tabla y desplegables con estado de encima, y transiciones de 150 ms. La preferencia «reducir movimiento» ya las anulaba (Fase 1).
* **Prueba**: recorre todos los apartados y exige nombre accesible y descripción emergente en cada botón de solo icono.

### D-069 · Crear una imagen a partir de un contenedor
* **Operación**: `POST /commit` de la API (equivalente a `docker commit`), con `Engine::call` y bloqueo del contenedor como el resto de operaciones. El panel de actividad muestra la orden `docker commit` equivalente.
* **Qué entra**: solo la capa de escritura del contenedor. Antes de crear, el diálogo enumera los volúmenes, carpetas y tmpfs montados, cuyo contenido **no** entra. La prueba real lo comprueba escribiendo en la capa, en un volumen y en una carpeta.
* **Variables de entorno**: `docker commit` guarda en la imagen la configuración del contenedor, incluidas sus variables, también las secretas. El diálogo lo advierte (lo detecté al revisar la imagen creada en la aplicación real). No se eliminan automáticamente, porque cambiaría el resultado de `docker commit` y la imagen podría dejar de funcionar.
* **Contenedor en ejecución**: por defecto se pausa durante la creación (como `docker commit`) para que la imagen sea coherente. El usuario puede desmarcarlo. Nunca se detiene ni se reinicia el contenedor.
* **Nombre existente**: `commit_preview` comprueba si ya hay una imagen con ese nombre. Sin `replace_existing`, `commit_image` responde `AlreadyExists` y no crea nada. Con él, la imagen anterior no se elimina: se queda sin etiqueta y sus contenedores siguen funcionando.
* **Validación en Rust** (y la misma, inmediata, en la interfaz): repositorio con la gramática de referencias de Docker (minúsculas, separadores, registro con puerto), etiqueta, longitud máxima, y autor y mensaje sin caracteres de control.
* **Después**: el diálogo abre la imagen nueva en Imágenes.

### D-070 · Editar la configuración y recreación segura
* **Clasificación de cada cambio** (la hace el backend, `edit.rs`; la interfaz solo la anticipa):
  * **Directos**:
    * Nombre: `rename`.
    * Política de reinicio, CPU, memoria y memoria reservada: `update`, sin reiniciar.
    * Redes: conectar y desconectar se aplica al momento con las operaciones de la Fase 5.
  * **Exigen recrear**:
    * Variables de entorno, puertos y montajes.
    * Descripción: es la etiqueta `dcc.description`, y las etiquetas no se pueden cambiar en un contenedor existente.
    * Quitar un límite: `docker update` no sabe quitarlo.
  * **Solo lectura**: imagen, comando, usuario, directorio de trabajo, modo de red, privilegios, capacidades, etc. Se muestran en la pestaña «Avanzado» sin campos editables y con un enlace a «Duplicar».
* **Por qué recrear desde el `inspect` completo**: el cuerpo del contenedor nuevo se construye a partir de toda la configuración real del original (Config y HostConfig) y solo se cambia lo editado. Así no se pierden opciones que la aplicación no edita (capacidades, ulimits, logs, healthcheck, dispositivos, etiquetas…). La prueba real compara el JSON crudo de `inspect` antes y después: solo cambian los campos editados y los que Docker genera.
* **Montajes**:
  * Los binds y montajes se copian tal cual.
  * Los volúmenes anónimos se vuelven a montar por su nombre, para conservar el contenido en lugar de crear uno vacío.
* **Redes**:
  * Se conservan IPAM, aliases propios, links y opciones.
  * Se descartan los aliases que Docker genera con el ID antiguo.
  * La red principal va en la creación y las demás se conectan después.
* **Secuencia**:
  1. Detener (si estaba en ejecución).
  2. Volver a leer el contenedor detenido.
  3. Renombrarlo a `<nombre>-dcc-backup-<fecha>` (la copia).
  4. Crear el nuevo y conectar sus redes.
  5. Iniciar y comprobar durante 3 s que sigue en ejecución sin reinicios.
* **Nombre de la copia**: `<nombre que tendrá el contenedor nuevo>-dcc-backup-<fecha>`. Si la misma recreación cambia el nombre, la copia lleva el nombre nuevo, para que aparezca entre las copias del contenedor y restaurarla lo sustituya. Se corrigió al probar la aplicación real: con el nombre antiguo, la copia no aparecía. Lo cubre la prueba real `recreation_with_rename_…`.
* **Vuelta atrás**: si algo falla, se elimina el contenedor nuevo **sin sus volúmenes**, el original recupera su nombre y su estado y se informa paso a paso. Si la vuelta atrás tampoco termina, se explica dónde está cada cosa.
* **Protección de datos** (requisito 2.4):
  * El original nunca se elimina: queda como copia detenida que se puede restaurar o eliminar a mano.
  * Restaurar aparta el actual como `-dcc-replaced-`, sin borrarlo.
  * Nunca se borran volúmenes, carpetas ni imágenes.
* **Se niega a recrear** (bloqueos):
  * Estados distintos de running, exited o created.
  * Contenedores con `--rm`, de Compose o de Swarm.
  * Imagen que ya no existe.
  * Configuración que no se puede leer entera.
* **Riesgos que se aceptan uno a uno**:
  * Archivos escritos fuera de los volúmenes, o tamaño desconocido.
  * Contenedores que usan la red de este (`--network container:`).
  * Montajes que se quitan.
  * Etiqueta de la imagen que ahora apunta a otra imagen.
* **Confirmaciones**: además de los riesgos, se pide una confirmación explícita de la recreación. Los cambios directos no piden confirmaciones innecesarias.
* **Puertos**: antes de aplicar se comprueba que no los use otro contenedor. Un puerto ocupado por un programa ajeno a Docker solo se detecta al iniciar, y entonces se vuelve al original.
* **Alternativas descartadas**:
  * Recrear con el modelo del asistente de creación: perdería todo lo que no edita.
  * Eliminar el original tras comprobar el nuevo: la orden pide no hacerlo automáticamente, y una copia detenida no ocupa más que su capa.

### D-071 · Dirección MAC al recrear
* **Hallazgos (Docker 29)**: un contenedor detenido muestra la MAC del endpoint vacía; uno creado y nunca iniciado muestra la configurada. Docker 24 (API 1.43) no admite fijar la MAC del endpoint al crear.
* **Decisión**: no se fija la MAC automáticamente, porque si el original tenía una asignada por Docker, fijarla convertiría una MAC dinámica en estática.
  * Con el contenedor en ejecución y API ≥ 1.44, se ofrece la casilla «Conservar las direcciones MAC actuales» (para quien la fijó con `--mac-address`).
  * Un contenedor creado y nunca iniciado conserva la suya.
  * Detenido: se avisa de que recibirá una nueva.
  * API < 1.44: se avisa y la opción se rechaza.
* **Comprobación**: tras iniciar, `verify_macs` comprueba que la MAC es la pedida.

### D-072 · Color principal morado y acento de la barra lateral
* **Tema**: el morado se aplica con `data-accent="purple"` en `<html>`, que solo redefine los tokens del acento (`--beacon*` y `--focus`) en claro y en oscuro. Todos los componentes ya usaban esos tokens, así que no hay reglas por componente.
* **Aplicación**: se aplica al momento, incluidas las terminales abiertas, que cambian su cursor sin reiniciarse. Se guarda en `config.toml` (`ui.accent`) y su cambio queda en el panel de actividad.
* **`--beacon-nav`** (nuevo): la barra lateral es siempre oscura y usaba el acento general. El icono activo en verde azulado tenía un contraste de 1,9–2,6:1, por debajo de 3:1, que es un defecto de accesibilidad demostrado. Ahora usa un acento propio para fondos oscuros.
* **Ajuste del verde azulado claro**: el texto blanco de los botones primarios tenía un contraste de 4,43:1, por debajo de 4,5:1 (AA). Se oscureció de `#0e858a` a `#0d8287` (4,61:1): la diferencia no se aprecia, pero cumple AA. Es el único cambio en el color existente.
* **Contrastes medidos** (prueba `styles/accent.test.ts`, que los calcula con la fórmula de WCAG a partir de app.css):
  * Botón primario: verde azulado 4,61 (claro) y 7,12 (oscuro); morado 5,97 y 6,38.
  * Texto del acento fuerte sobre las superficies: ≥ 5,4 en los cuatro casos.
  * Foco: ≥ 3 en los cuatro casos.
  * Icono de la barra lateral: ≥ 4,5 sobre el casco y ≥ 3 sobre la fila seleccionada.

### D-073 · Catalán, gallego, euskera y portugués
* **Idiomas**: `Language` en Rust y TypeScript admite `ca`, `gl`, `eu` y `pt`.
  * El selector, ahora una lista desplegable, sigue el orden pedido: English, Español, Català, Galego, Euskara, Français, Italiano, Português.
  * Cada idioma aparece con su propio nombre (endónimo) y con `lang` en la opción.
  * Español sigue siendo el idioma por defecto.
* **Variante**: portugués europeo (pt-PT), con «contentor», «registos» y «transferir». Las fechas y los números usan ca-ES, gl-ES, eu-ES y pt-PT.
* **Traducción**:
  * Todas las claves de la interfaz (unas 1900), incluidas las de esta actualización.
  * La ayuda completa: 26 secciones en los ocho idiomas, con las dos nuevas («Editar la configuración» y «Crear una imagen»).
  * Las pruebas exigen las mismas claves y variables `{{…}}` en los ocho idiomas, ningún texto vacío y ninguna cadena igual a la inglesa, salvo una lista explícita por idioma.
* **Revisión nativa recomendada**: las traducciones son completas y coherentes, pero no las ha revisado un hablante nativo, sobre todo el euskera. Conviene hacerlo antes de distribuir.
* **Sin cambios (D-067)**: los resúmenes que el backend escribe en el panel de actividad y los errores con datos concretos siguen en español en todos los idiomas.

### D-074 · Grupos de opciones con flechas y operaciones de la aplicación
* **`Segmented`** (tema, color, filtros): además de Tab, Intro y Espacio, las flechas eligen la opción anterior o la siguiente (patrón de grupo de opciones de WAI-ARIA). No cambia el orden de tabulación ni el aspecto. Toda la suite anterior sigue pasando.
* **Panel de actividad**: nuevo transporte `app` (distintivo APP) para operaciones que no son peticiones a Docker ni procesos:
  * Cambio de idioma y de color.
  * Resumen de una recreación o restauración, cuyos pasos aparecen a la vez como operaciones API.
  * No ofrece copiar una orden, porque no existe.
* **Secretos**: el cuerpo de creación del contenedor nuevo se registra con los valores sensibles ocultos.

## Actualización 0.10.0 — Administración remota y varios servidores

### D-075 · Transporte: OpenSSH del sistema y `docker system dial-stdio`
* **Cómo**: la aplicación lanza el cliente `ssh` del sistema, que ejecuta `docker system dial-stdio` en el servidor (el mismo mecanismo que `docker -H ssh://…`). Una **conexión maestra** por servidor (`ControlMaster`) y un **canal multiplexado** por cada conexión HTTP de bollard (`connect_with_custom_transport`), sin repetir la autenticación.
* **Descartado**:
  * Docker por TCP (2375/2376): exige abrir un puerto en el servidor y, sin TLS, es acceso de root sin autenticar.
  * La función `ssh` de bollard: usa `KnownHosts::Add` (acepta cualquier clave nueva) y no permite fijar opciones.
  * Una biblioteca SSH propia: duplicaría la configuración, el agente y las claves del usuario, que OpenSSH ya gestiona bien.
* **Opciones de seguridad, siempre las primeras** (en `ssh` gana el primer valor, también frente a `~/.ssh/config`): `StrictHostKeyChecking=yes`, `UpdateHostKeys=no`, `ForwardAgent=no`, `ForwardX11=no`, `ClearAllForwardings=yes`, `PermitLocalCommand=no`, tiempo de conexión y `ServerAlive`. El destino va tras `--`.
* **Sin shell local**: `ssh` se lanza con argumentos separados. La orden remota es fija o se compone con argumentos entrecomillados uno a uno (`shell_quote`, probado con entradas hostiles contra `/bin/sh` real).
* **Requisito del servidor**: Docker Engine 20.10 o posterior (API 1.41); se comprueba al conectar.
* **Cierre**: al salir se cierran todas las sesiones. Los sockets de control llevan el PID de la instancia; al arrancar se cierran solo los de instancias que ya no existen (cierre forzado), nunca los de otra en marcha.

### D-076 · Identidad del servidor
* **Primera conexión**: se muestra la huella (`SHA256:…`) y su tipo; no se conecta hasta que el usuario la confirma. Al confirmar, la clave se captura en un archivo temporal y **solo se guarda si su huella es exactamente la mostrada**.
* **Dónde**: en un `known_hosts` propio (0600). `~/.ssh/known_hosts` se lee pero **nunca se modifica**.
* **Cambio de identidad**: la conexión se detiene, no se reintenta sola y se avisa de un posible ataque de intermediario. Solo se sustituye con una confirmación expresa tras revisarla; si la clave anterior está en el archivo del usuario, se le da la orden `ssh-keygen -R` para que lo haga él.

### D-077 · Credenciales
* **Métodos**: agente SSH (recomendado), clave privada, clave con frase de paso y contraseña (desaconsejada, decisión del propietario). De una clave solo se guarda la ruta: **nunca se copia ni se lee**.
* **Entrega a `ssh`**: por `SSH_ASKPASS`, con la propia aplicación como programa askpass. Se comunica por un socket efímero (carpeta 0700, socket 0600, uid del otro extremo comprobado) que solo responde a la pregunta esperada. El secreto nunca va en argumentos, variables de entorno ni archivos, y en memoria se borra al liberarse (`zeroize`).
* **Recordar**: en el keyring del sistema (Secret Service: GNOME Keyring, KWallet…), en Rust puro (`secret-service`/zbus). **Sin keyring no hay alternativa**: se avisa y se pide en cada conexión; nunca en texto plano.
* **Separación**: perfiles en `servers.toml`, secretos en el keyring, y ninguno en el registro de actividad, los diagnósticos ni los errores. Se pueden borrar al editar el servidor y se borran al eliminarlo.
* **Credencial incorrecta**: se olvida de la sesión (y del keyring si se acaba de guardar) y se vuelve a pedir.

### D-078 · Registro de servidores y operación explícita
* **Perfiles**: van en `servers.toml`, aparte de `config.toml` (0600, escritura atómica, copia del archivo corrupto). Así la pantalla de preferencias, que guarda la configuración completa, no puede pisar la lista de servidores.
  * Los datos que pone la aplicación (última conexión y proyectos Compose del servidor) no se pueden sobrescribir con la copia que envía la interfaz.
* **Un gestor por servidor**: el equipo local y cada servidor remoto tienen su propio gestor de conexión, con su monitor, sus eventos y sus tareas.
  * Los remotos empiezan sin conectar.
  * Uno caído no afecta a los demás.
  * El monitor no reintenta lo que necesita al usuario (identidad, credenciales, permisos).
* **Servidor explícito**: las 59 órdenes IPC que actúan sobre un servidor (Docker, Compose y sus archivos, terminales) reciben `server`. Un identificador ausente o desconocido es un error y **nunca se recurre a otro servidor**; no hay un «servidor actual» en el backend.
  * Los flujos (logs, terminales, Compose…) llevan su servidor.
  * Desconectar o eliminar un servidor cierra solo los suyos.
* **Eliminar un perfil** borra solo datos de la aplicación: perfil, credenciales y, si ningún otro perfil la usa, la identidad guardada. No ejecuta nada en el servidor.

### D-079 · Interfaz por servidor
* **Ámbito**: todo lo que cuelga de `ServerScope` trabaja con un servidor.
  * `useApi()` da las órdenes de Docker ligadas a ese servidor.
  * Cada servidor tiene su propia caché de consultas.
  * Al cambiar de servidor, el área principal se monta de nuevo: nada de uno puede mostrarse ni usarse en otro.
* **Operaciones que siguen solas**: descargas, construcciones, Compose y terminales guardan su servidor al empezar y siguen yendo a él aunque se cambie de servidor.
  * El cliente global `api` ya no tiene órdenes de Docker, así que TypeScript impide usarlas sin servidor.
* **Visibilidad**:
  * Selector con estado e indicador «Remoto» en la barra superior.
  * Línea «Servidor: nombre · dirección» en los diálogos de un ámbito (también en las confirmaciones destructivas).
  * Servidor en cada operación del panel de actividad, del historial y de los eventos, con filtro por servidor.

### D-080 · Compose y órdenes del CLI en el servidor
* **Dónde se ejecuta**: en un servidor remoto, `docker compose` (o `docker-compose`) **se ejecuta en el servidor** por la sesión SSH verificada.
  * Se descartó el CLI local con `DOCKER_HOST=ssh://…`: usaría su propio SSH, sin las opciones de la aplicación, y además los archivos del proyecto están en el servidor.
  * La detección también se hace allí.
* **Registro**: en el panel de actividad aparece la orden `ssh` equivalente. El socket o contexto del perfil se pasa con `env DOCKER_HOST=…`/`DOCKER_CONTEXT=…`.
* **Terminal remota**: las acciones y los registros usan terminal remota (`-tt`). Al cancelar se cierra el canal, el servidor envía SIGHUP a `compose` y no quedan procesos en el servidor. Las órdenes con entrada estándar (validar el contenido sin guardar) van sin terminal.
* **Archivos del proyecto** (comprobar, leer, guardar y crear): siempre en el servidor, nunca en este equipo.
  * Lectura exacta con `base64`.
  * Guardado atómico con `dd conv=excl` en un temporal más `chmod` y `mv`, con comprobación de versión.
  * Creación sin pisar nada.
  * El contenido no se registra, y la interfaz confirma el destino antes de escribir.
* **Proyectos añadidos**: una lista por servidor, con rutas del servidor.
* **Bind mounts y carpetas**: se comprueban y describen en el servidor (`stat`, `find -readable`) al crear, editar, previsualizar y listar.
  * Los textos «de este equipo» tienen una variante «del servidor», usada solo en remoto.
* **Construcción**: el contexto sigue siendo una carpeta de este equipo, que Docker envía al servidor; la pantalla lo dice.

### D-081 · Terminal SSH del servidor
* **Tipo de sesión aparte**: una shell interactiva en el servidor, distinta de la terminal de un contenedor y solo para servidores remotos.
* **Sobre una PTY local**: `ssh -tt` (por la sesión verificada) se ejecuta sobre una PTY propia como terminal de control, así que los cambios de tamaño llegan al servidor.
* **Cierre**: envía SIGHUP a la sesión y la shell del servidor termina. Se comprueba que no quedan ni la shell remota ni procesos `ssh` del canal.
* **Lo que se ejecuta**: no hay ninguna orden propia; solo la shell de inicio de sesión del usuario.
* **Registro y etiquetas**: queda en el panel de actividad con su servidor, y el panel de la terminal lo indica.

## Fase 10 · Pruebas finales y distribución (1.0.0)

### D-082 · El morado es el único color principal
* **Decisión del propietario** (2026-11-12): eliminar el verde azulado como color principal y mantener el morado. Con un solo color, el selector de Configuración desaparece: un control con una sola opción no haría nada (requisito de calidad §14).
* **Dónde**: en el sistema central de temas. Los tokens base `--beacon*` y `--focus` de `styles/app.css` son los morados en claro y oscuro (los valores que tenía la variante morada de la 0.9.2, D-072); se eliminan las reglas `data-accent`. Ningún componente tiene colores propios.
* **Lo que no cambia**: los colores de señal (`--signal-ok/run/warn/err/off`): el verde de «en ejecución», «conectado» o «correcto» es un indicador funcional, no el color principal.
* **Preferencias guardadas**: `Accent` solo tiene `Purple`. Al leer `config.toml`, cualquier valor de `accent` —el `"teal"` de la 0.9.2–0.10.0, uno desconocido o de otro tipo— es morado, sin invalidar el resto de la configuración ni apartarla como dañada. Se sigue escribiendo `accent = "purple"`, así que una versión anterior (0.9.2–0.10.0) lee el mismo color.
* **Actividad**: ya no se registra un «cambio del color principal» (no puede cambiar). Los registros `ui.accent` de versiones anteriores se siguen mostrando con su texto.
* **Logotipo e iconos**: el verde azulado del logotipo (`#14A3A8`) pasa a `#A184F2` (el acento morado para fondos oscuros, `--beacon-nav`), con la misma forma; se regeneran los iconos.
* **Pruebas**: migración desde `teal` y valores no válidos conservando el resto; tono morado (250°–290°) de todos los tokens del acento y ausencia de los valores del verde en el código, el CSS compilado y el logotipo; colores semánticos sin cambios; contraste WCAG AA; Configuración sin selector.

### D-083 · Distribución: paquete, metadatos y script de instalación
* **Dos paquetes de la misma versión** (como en la Fase 9, D-059): el normal (Debian 13 / Ubuntu 24.04, glibc 2.39) y el compatible (compilado en Ubuntu 22.04, glibc 2.34+).
* **Contenido del `.deb`**: programa, lanzador, iconos de 32 a 512 px en carpetas estándar de `hicolor` (el de 256 px estaba en `256x256@2`, que los escritorios no buscan), y en `/usr/share/doc/docker-control-center/` el `copyright` (formato DEP-5), `changelog.gz`, README, INSTALL, USER_GUIDE y `THIRD-PARTY.md`.
* **Componentes de terceros**: `scripts/third-party-licenses.py` genera `packaging/THIRD-PARTY.md` desde `Cargo.lock` (solo las bibliotecas que se enlazan en el programa) y `package-lock.json` (solo los paquetes de producción); `--check` comprueba que está al día. Todas las licencias son permisivas y compatibles con GPL-3.0-or-later.
* **Licencia y nombre**: siguen siendo provisionales (D-016), por decisión del propietario para la 1.0.0. `LICENSE` contiene el texto de la GPL-3.0. No hay fuente de actualizaciones predefinida (D-060).
* **`scripts/install.sh`**: con el usuario normal y `sudo` solo para `apt-get`. Comprueba el sistema, elige el paquete por la glibc que declara cada uno, simula con apt, instala y comprueba **sin cambiar nada** Docker, el acceso al socket, Compose, el cliente SSH y el almacén de contraseñas. Nunca instala Docker ni cambia grupos, permisos o la configuración de SSH.
* **`scripts/run.sh`**: abre la versión instalada o la compilación del código fuente; no se ejecuta como root.
* **Instalación limpia**: `scripts/dev/clean-install-test.sh` (solo máquinas de prueba, como root) crea Ubuntu 24.04 y 22.04 con debootstrap y prueba instalación, arranque con un usuario normal y desinstalación. No se puede crear Debian aquí (sin acceso a sus repositorios): Debian 12 y 13 se comprueban en el equipo del propietario.

### D-084 · Pasos de verificación que no ejecutan ninguna prueba
* **Problema**: un paso que filtra pruebas por nombre (`cargo test … nombre`) termina bien aunque el filtro no coincida con nada. Al renombrar una prueba, el paso seguía «correcto» sin probar nada (pasó con `language_and_accent_changes_are_recorded_once`).
* **Decisión**: en `scripts/verify-phase-10.sh`, los pasos con filtro usan `run_tests`, que falla si no se ha ejecutado ninguna prueba.

## Evolución · Etapa 1: correcciones de seguridad prioritarias (1.1.0)

Plan: `docs/roadmap/PROJECT_EVOLUTION_PLAN.md` (etapas E0–E6, aprobadas por el
propietario el 2026-11-12). Hallazgos: `docs/security/SECURITY_AUDIT.md`.

### D-085 · Ocultación de secretos centralizada (DCC-SEC-05)
* **Un solo lugar decide qué es un secreto**: `dcc-activity::redact`, la capa más baja del grafo de módulos, para que la usen todas las demás sin ciclos. La interfaz aplica las mismas reglas (`ui/src/shared/secrets.ts`); una prueba de Rust lee ese archivo y comprueba que las listas son idénticas.
* **Nombres secretos**: los que contienen `PASSWORD`, `PASSWD`, `SECRET`, `TOKEN`, `KEY`, `CREDENTIAL` o `PRIVATE` (como antes), y ahora también los que tienen una parte (separada por `_`, `-` o `.`) que termina en `PASS`, `PWD`, `AUTH`, `DSN` o `COOKIE` (`MYSQL_PWD`, `DB_PASS`, `PGPASS`, `REDIS_AUTH`, `SMTP_PASS`, `SENTRY_DSN`…). Excepciones: `PWD`/`OLDPWD` (carpeta actual) y los nombres terminados en `_FILE` (ruta de un secreto de Docker, no el secreto).
* **Valores secretos con cualquier nombre**: la contraseña de una URL o DSN (`postgres://app:clave@db` → `postgres://app:********@db`), los bloques PEM de claves privadas y las cabeceras `Authorization`.
* **Formatos**: `NOMBRE=valor` en cuerpos JSON, órdenes con cualquier entrecomillado (`'it'\''s'` incluido), consultas `?token=…`, campos JSON `"password": "…"`, líneas YAML `NOMBRE: valor` (`docker compose config`) y opciones `--password`/`--token` (no `--password-stdin` ni `--secret id=…,src=…`, que son referencias).
* **Dónde se aplica**: en `ActivityLog`, antes de guardar o emitir cualquier operación (cuerpo, orden equivalente, argumentos del CLI, ruta de la API, destino, resumen, salidas y error): así cubre el panel, sus eventos, el historial persistente y la interfaz desde un único punto. Primero se oculta y después se recorta la salida, para no partir un secreto. También en los mensajes de error que recibe la interfaz (al serializar `CommandError`, cubre todas las formas de construirlo) y en los registros de la aplicación que incluyen texto de errores externos.
* **Qué se conserva**: solo se oculta el valor. El nombre de la variable, el usuario y el servidor de una URL, los códigos de error y los valores triviales (`true`, `false`, `yes`, `no`…) siguen visibles para poder diagnosticar.
* **Límite (decisión del propietario)**: los registros de los contenedores y la salida de las terminales **no se alteran**: son datos del usuario y modificarlos dificultaría el diagnóstico. A Docker se le envían siempre los valores reales.

### D-086 · Riesgos para el equipo y aceptación comprobada en el motor (DCC-SEC-01, DCC-SEC-03)
* **Reglas únicas** en `dcc-engine::risk`, usadas por el asistente de creación, la edición, el despliegue de Compose y la pantalla de directorios vinculados (antes `spec.rs` y `binds.rs` aplicaban criterios distintos). Solo avisan: nunca se cambia lo que configura el usuario.
* **Montajes** (con la ruta normalizada: `//etc`, `/./etc`; y, con Docker local, también la ruta real de un enlace simbólico):
  * Crítico: la raíz `/` y cualquier socket de un motor de contenedores (`docker.sock`, `containerd.sock`, `podman.sock`, el de rootless en `/run/user/<uid>`) o una carpeta que lo contiene (`/run`, `/var/run`, `/var`). **También en solo lectura**: `:ro` impide escribir en el archivo del socket, no usar la API.
  * Alto, también en solo lectura: `/etc` entero, `/etc/shadow`, `/etc/gshadow`, `/etc/sudoers*`, `/etc/ssh`, `/root`, `/home`, `/home/<usuario>` y sus carpetas `.ssh`, `.gnupg`, `.docker`, `.kube` y `.aws`.
  * Otra carpeta de `/etc`: alto con escritura y sin aviso en solo lectura (`/etc/nginx:ro`, `/etc/localtime:ro` son habituales).
  * `/boot`, `/proc`, `/sys`, `/dev`, `/var/lib/docker`, `/var/lib/containerd` (y las carpetas que los contienen): alto con escritura, medio en solo lectura. `/usr`, `/bin`, `/sbin`, `/lib*`: medio con escritura.
* **Capacidades** (`cap_add`, con o sin `CAP_` y en cualquier mayúscula): crítico `ALL` y `SYS_ADMIN`; alto `SYS_PTRACE`, `SYS_MODULE`, `SYS_RAWIO`, `DAC_READ_SEARCH`, `NET_ADMIN`, `BPF`, `PERFMON`, `MAC_ADMIN`, `MAC_OVERRIDE`, `SYS_BOOT`, `SYS_TIME`, `SYSLOG`, `LINUX_IMMUTABLE`; medio `DAC_OVERRIDE`, `SYS_RESOURCE`, `SYS_NICE`, `IPC_LOCK`; bajo `NET_RAW` (Docker ya concede `DAC_OVERRIDE` y `NET_RAW` por defecto; se avisa porque la configuración las pide). `cap_drop` no reduce el aviso: en Docker, lo añadido prevalece también con `cap_drop: [ALL]`.
* **Otros**: modo privilegiado (crítico) y red del equipo `host` (alto).
* **Aceptación**: los riesgos altos y críticos se aceptan uno a uno por su clave (`código:sujeto`; en Compose, `…@servicio`). El motor **recalcula** los riesgos y rechaza la operación (`engine.risksNotAccepted`, sin enviar nada a Docker) si falta alguno: `container_create` (`ContainerSpec.acceptedRisks`), `container_edit_apply` (solo los riesgos que **añade** el cambio, en `ApplyOptions.acceptedRisks`) y `compose_run_open` con `up` (`acceptedRisks`, a partir de `docker compose config`). La interfaz no puede saltarse el aviso. Una copia de un contenedor vuelve a pedir la aceptación.
* **Sin falsos positivos**: se comprueba con las configuraciones legítimas del plan (§9.5): carpeta de contenido en solo lectura, volumen con nombre, carpeta del proyecto, `cap_drop: [ALL]` + `NET_BIND_SERVICE`, `tmpfs`, puertos en 127.0.0.1 y redes propias.
* **Fuera de esta etapa** (E2, R-05): en Compose, `pid`/`ipc`/`userns_mode: host`, `security_opt`, `devices` y `cgroup_parent`.

### D-087 · Servidor intermedio (ProxyJump) con las reglas del destino (DCC-SEC-02)
* **Problema confirmado**: con `ssh -J`, OpenSSH lanza el `ssh` del salto pasándole solo `-l`, `-p` y `-v`: sin `StrictHostKeyChecking`, sin el `known_hosts` de la aplicación y sin `BatchMode`. Además hereda `SSH_ASKPASS`. Reproducido en el laboratorio: con un salto que pide contraseña, el registro del salto mostraba `Failed password for …`, con la contraseña del servidor de destino.
* **Decisión**: el salto se construye con un `ProxyCommand` propio que repite las opciones de seguridad (`StrictHostKeyChecking=yes`, `known_hosts` de la aplicación, `UpdateHostKeys=no`, `BatchMode=yes`, sin contraseña ni teclado interactivo, sin reenvíos, sin `ControlMaster`, `ProxyCommand=none`). El proceso del salto se lanza con `env -u SSH_ASKPASS -u DCC_ASKPASS_SOCKET SSH_ASKPASS_REQUIRE=never`: nunca puede pedir ni recibir un secreto. OpenSSH ejecuta el `ProxyCommand` con `$SHELL -c`: la aplicación fija `SHELL=/bin/sh` en sus procesos `ssh`, entrecomilla cada argumento y escapa los `%`.
* **Autenticación en el salto** (decisión del propietario): solo con clave o agente SSH. La clave del perfil (sin frase de paso) también se ofrece al salto. Si el salto solo admite contraseña, la conexión se detiene con `jumpAuthFailed` antes de leer el secreto del destino.
* **Identidad del salto**: se comprueba y se confirma igual que la del destino (`jumpHostKeyUnknown`/`jumpHostKeyChanged`, `server_trust` con `jump`), en el `known_hosts` de la aplicación. Después se comprueba la del destino a través del salto.
* **askpass**: solo responde a la pregunta de **este** destino (usuario y nombre del servidor, también el `HostName` efectivo de `~/.ssh/config`) o de **esta** clave. `Password:` sin destino se sigue aceptando, porque solo puede venir del `ssh` del destino. El número de respuestas (64) no se reduce: con muchos canales, OpenSSH abre conexiones adicionales que vuelven a autenticarse.
* **Un solo salto**: la identidad de cada salto se verifica con las reglas de la aplicación y eso solo es fiable para el primero. Los perfiles con varios saltos (`a,b`) se marcan como no válidos (`proxyJumpChain`).
* **Salto definido en `~/.ssh/config`**: si el perfil usa contraseña o frase de paso y `ssh -G` indica un `ProxyJump` o `ProxyCommand` del usuario, no se conecta (`jumpFromSshConfig`) y se pide indicar el salto en el perfil. Con agente o clave sin frase se mantiene el comportamiento anterior, porque no hay secreto que pueda filtrarse.
* **Laboratorio de pruebas**: el servidor A de `scripts/dev/remote-lab.sh` permite reenvíos locales solo hacia B y C (`PermitOpen`) para hacer de servidor intermedio.
