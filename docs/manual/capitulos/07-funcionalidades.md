# Funcionalidades de la aplicación

Cada apartado se describe con la misma ficha: qué hace, dónde está, cómo se
usa, qué operaciones hace sobre Docker, qué componentes intervienen, qué
errores pueden aparecer y qué limitaciones tiene. El uso paso a paso está en
el capítulo [Uso de la aplicación](#uso-de-la-aplicación).

## Dashboard (panel de inicio)

| | |
|---|---|
| Qué hace | Resume el Docker del servidor elegido: versión del motor y de la API, socket y origen de la conexión, contenedores (en marcha, en pausa, detenidos), imágenes, equipo, almacenamiento y avisos del daemon. Sin conexión, muestra el diagnóstico |
| Dónde | Barra lateral › Inicio · `features/dashboard`, `features/engine` |
| Operaciones | `GET /_ping`, `/version`, `/info`, lista de contenedores e imágenes |
| Componentes | `engine_overview`, `engine_state`, `manager.rs`, `probe.rs` |
| Errores | Sin conexión (diagnóstico con causa y pasos) |
| Limitaciones | Los datos del equipo son los que da `docker info`, no una supervisión del sistema operativo |

## Servidores remotos

| | |
|---|---|
| Qué hace | Administra el Docker de servidores Linux por SSH, a la vez que el local, con todas las funciones |
| Dónde | Barra lateral › Servidores · selector de servidor de la barra superior · `features/servers` |
| Uso | Asistente (General, Autenticación, Docker), **Probar conexión** (siete comprobaciones), confirmación de la huella, **Conectar** |
| Operaciones | `ssh` con verificación estricta; `docker system dial-stdio`; Compose con `ssh … -- docker compose` |
| Componentes | `servers.rs`, `secrets.rs`, `cmc-engine/src/ssh/` |
| Errores | Nombre que no resuelve, servidor inaccesible, SSH rechazado, huella desconocida o cambiada, autenticación fallida, Docker ausente o detenido, sin permiso en el socket, API antigua; cada uno con su explicación |
| Limitaciones | Un solo salto `ProxyJump` (no disponible en Windows); el usuario remoto necesita una shell POSIX; las rutas son las del servidor y se escriben a mano |

## Contenedores

| | |
|---|---|
| Qué hace | Lista (como `docker ps -a`), filtros, búsqueda y acciones según el estado: iniciar, detener, reiniciar, pausar, reanudar, eliminar |
| Dónde | Barra lateral › Contenedores · `features/containers` |
| Operaciones | `GET /containers/json`, `POST /containers/{id}/start|stop|restart|pause|unpause`, `DELETE /containers/{id}` |
| Componentes | `containers_list`, `container_action`, `container_remove` · `containers.rs` |
| Errores | Rechazo de Docker (se muestra su mensaje); operación en curso sobre el mismo contenedor (`engine.busy`) |
| Limitaciones | Detener y reiniciar esperan 10 s antes de forzar, como Docker |

### Crear un contenedor

| | |
|---|---|
| Qué hace | Asistente de seis pestañas (General, Entorno, Red, Almacenamiento, Recursos, Avanzado) con vista previa, orden `docker run` equivalente, revisión de errores y **riesgos para el equipo** |
| Operaciones | Solo lectura para rellenar (imagen, redes, volúmenes); `POST /containers/create` y, si se pide, `start` |
| Componentes | `CreateWizard.tsx`, `container_preview`, `container_create`, `spec.rs`, `risk.rs` |
| Errores | Validación por campo; riesgos altos o críticos sin aceptar (`RisksNotAccepted`); fallo al iniciar (el contenedor queda creado y se muestra el error) |
| Limitaciones | La imagen debe estar en el equipo (se descarga en Imágenes) |

### Detalle: registros, estadísticas, terminal e inspección

| | |
|---|---|
| Qué hace | Resumen, **registros** en directo (10 000 líneas, filtro, marcas de tiempo), **estadísticas** cada segundo (como `docker stats`), **terminal** (`docker exec -it`) e **inspección JSON** |
| Operaciones | `GET /containers/{id}/logs?follow`, `/stats`, `POST /containers/{id}/exec` + `start` (TTY) + `resize`, `GET /containers/{id}/json` |
| Componentes | `streams.rs`, `logs_open`, `stats_open`, `terminal_open` · xterm.js |
| Limitaciones | La terminal necesita una shell en el contenedor (`bash` o `sh`) |

### Editar la configuración y recreación segura

| | |
|---|---|
| Qué hace | Cambia nombre, política de reinicio, CPU y memoria al momento (`docker rename`, `docker update`); conecta y desconecta redes; para variables, puertos, montajes y descripción **recrea** el contenedor de forma segura (copia con `-cmc-backup-<fecha>`, nuevo contenedor con toda la configuración, vuelta atrás si falla) |
| Componentes | `edit.rs`, `edit_ops.rs`, `container_edit_plan`, `container_edit_apply`, `container_restore_backup` |
| Errores | Recreación bloqueada si no es segura (contenedor de Compose o Swarm, `--rm`, en pausa, imagen inexistente) |
| Limitaciones | Imagen, comando, usuario y opciones avanzadas no se pueden cambiar en Docker: se usa **Duplicar** |

### Crear una imagen y duplicar

- **Crear imagen** (`docker commit`): repositorio, etiqueta, autor y mensaje;
  no incluye volúmenes ni carpetas montadas; sí la configuración, incluidas
  las variables secretas (se avisa).
- **Duplicar**: abre el asistente con la configuración del contenedor y avisa
  de lo que no se traslada.

## Gestión masiva (1.10.0)

| | |
|---|---|
| Qué hace | Reúne los contenedores de todos los servidores conectados en una tabla con indicadores (total, en marcha, detenidos, en pausa, con error), filtros (nombre o ID, estado, imagen, etiqueta, servidor), orden, páginas y selección (individual, página, todo lo filtrado), y aplica una misma operación a muchos a la vez: iniciar, detener, reiniciar, pausar, reanudar, eliminar y cambiar la política de reinicio |
| Dónde | Barra lateral › Recursos › Gestión masiva · `features/bulk` |
| Uso | Seleccionar → botón de la barra (indica a cuántos se aplica) → confirmación con la lista de afectados y omitidos, consecuencias y operaciones a la vez (1–16) → progreso y resultado por contenedor; «Ver en actividad» |
| Operaciones | Lectura silenciosa (`GET /containers/json`, `GET /containers/{id}/json`); por contenedor, las mismas órdenes que «Contenedores» (`start`, `stop`, `restart`, `pause`, `unpause`, `DELETE ?v=false`) o `POST /containers/{id}/update` para la política, con confirmación por `inspect` |
| Componentes | `bulk_inventory`, `bulk_run_open`, `stream_close` · `src-tauri/src/bulk.rs`, `commands/bulk.rs`, `cmc-engine/src/bulk.rs` |
| Errores | Antes de empezar: sin confirmar, número mal escrito al eliminar, destinos repetidos o más de 1000, servidor desconocido, contenedor ya en otra operación masiva (`bulk.busy`). Por contenedor: rechazo de Docker, contenedor ocupado (`engine.busy`), sin conexión, resultado desconocido si la conexión se corta |
| Limitaciones | Las etiquetas no se pueden cambiar (Docker exige recrear el contenedor; D-119). No hay transacción global: los resultados pueden ser parciales y nada se deshace ni se reintenta. Nunca elimina volúmenes ni imágenes. No gestiona proyectos Compose, plantillas ni automatizaciones |

## Imágenes

| | |
|---|---|
| Qué hace | Lista con uso, detalle (resumen, capas, contenedores, JSON), **descarga** con progreso por capa y credenciales, **búsqueda** en Docker Hub, **construcción** (BuildKit o clásica) con pasos y salida, etiquetar, eliminar y limpiar |
| Operaciones | `GET /images/json`, `POST /images/create`, `GET /images/search`, `POST /build`, `POST /images/{name}/tag`, `DELETE /images/{name}`, `POST /images/prune` |
| Componentes | `images.rs`, `pull.rs`, `build/`, `registry_auth.rs`, `reference.rs` |
| Errores | Referencia no válida (con las reglas de Docker), sin acceso al registro, credenciales rechazadas, imagen en uso |
| Limitaciones | No se puede limpiar mientras hay descargas (con el almacén containerd fallarían); en un servidor remoto, las credenciales locales solo se envían si se marca expresamente |

## Redes

| | |
|---|---|
| Qué hace | Lista con contenedores, detalle, crear (bridge, macvlan, ipvlan, overlay; IPAM, IPv6, opciones), conectar (alias, IP fija) y desconectar, eliminar y limpiar |
| Operaciones | `/networks`, `/networks/create`, `/networks/{id}/connect|disconnect`, `/networks/prune` |
| Componentes | `networks.rs`, `features/networks` |
| Errores | Subred no válida, puerta de enlace fuera de la subred, red en uso por contenedores en marcha |
| Limitaciones | `bridge`, `host` y `none` no se pueden eliminar; overlay necesita Swarm |

## Volúmenes

| | |
|---|---|
| Qué hace | Lista con contenedores que los montan, tamaños bajo demanda, crear (local, tmpfs, NFS, carpeta, otros drivers), detalle, eliminar y limpiar |
| Operaciones | `/volumes`, `/volumes/create`, `GET /system/df` para tamaños, `/volumes/prune` |
| Componentes | `volumes.rs`, `features/volumes` |
| Errores | Volumen en uso (Docker no lo elimina, ni con contenedores detenidos) |
| Limitaciones | Con Docker anterior a la 23, la limpieza elimina también los volúmenes con nombre (se avisa) |

## Carpetas vinculadas

| | |
|---|---|
| Qué hace | Carpetas y archivos del equipo montados en contenedores, agrupados por ruta, con tipo, permisos, existencia, contenedores y **riesgos** (socket de Docker, raíz, carpetas del sistema, rutas inexistentes) |
| Componentes | `binds.rs`, `features/binds` |
| Limitaciones | Docker no permite añadir una carpeta a un contenedor existente: **Vincular una carpeta…** abre el asistente de un contenedor nuevo |

## Docker Compose

| | |
|---|---|
| Qué hace | Proyectos desplegados y guardados; abrir o crear proyectos; servicios con su estado; desplegar (perfiles, construir, descargar, recrear, huérfanos), iniciar, detener, reiniciar, construir, descargar imágenes, eliminar (`down`); salida en vivo con interrupción; registros; **editor** con validación y protección frente a cambios externos; configuración resuelta |
| Operaciones | `docker compose … up|down|start|stop|restart|pull|build|logs|config` |
| Componentes | `compose/`, `features/compose`, `compose_run_open`, `compose_file_save` |
| Errores | Compose no instalado, archivo no válido (mensaje de Compose), archivo cambiado por otro programa (`engine.fileChanged`), ruta no permitida, riesgos sin aceptar |
| Limitaciones | Una operación a la vez por proyecto; solo se editan `.yml`/`.yaml` fuera de carpetas ocultas, de configuración y del sistema |

## Terminales

| | |
|---|---|
| Qué hace | Hasta 12 terminales a la vez en contenedores (local y remotos) y en servidores (**Shell SSH del servidor**), con distribución automática en paneles y separadores ajustables |
| Componentes | `features/terminals` (`sessions.ts`, `layout.ts`, `Splitter.tsx`), `ssh/shell.rs` |
| Seguridad | Los enlaces OSC 8 se confirman antes de abrirlos y solo `http`/`https`; el panel de actividad registra apertura y cierre, nunca lo escrito |
| Limitaciones | En Windows la terminal del servidor no cambia de tamaño |

## Auditoría de seguridad

| | |
|---|---|
| Qué hace | Revisa sin cambiar nada contenedores, imágenes y proyectos Compose: riesgos para el equipo (socket, carpetas, capacidades, privilegiado, red y PID del equipo, seccomp/AppArmor/SELinux desactivados, dispositivos, puertos abiertos) y endurecimiento (root, sistema de archivos modificable, límites, `no-new-privileges`, imágenes antiguas) |
| Extras | Correcciones propuestas con vista previa (abren la edición), vulnerabilidades con **Trivy** o **Grype** si están instalados, exportación a JSON, Markdown o HTML (archivo 0600), historial de 200 auditorías, enlaces con monitorización y copias |
| Componentes | `audit/`, `features/audit`, `links.rs` |
| Limitaciones | Sin Trivy o Grype (o en un servidor remoto) las vulnerabilidades figuran como «No analizado», nunca «0» |

## Copias de seguridad

| | |
|---|---|
| Qué hace | Copias de volúmenes (con contenedor auxiliar que nunca arranca; en caliente o deteniendo), bases de datos (MariaDB, MySQL, PostgreSQL, Redis, MongoDB, con su herramienta de volcado), configuración de contenedores, Compose y la aplicación; manifiesto SHA-256; **Comprobar**; restauración completa o selectiva con vista previa; retención; tareas programadas |
| Programación | Linux: temporizador de usuario de systemd; Windows: Programador de tareas. Solo con autorización expresa |
| Componentes | `backup/`, `backups.rs`, `systemd.rs`, `schtasks.rs`, `features/backup` |
| Errores | Destino no válido o del sistema, volumen en uso, copia alterada o incompleta, base de datos de otro motor |
| Limitaciones | Las copias **no están cifradas**; los temporizadores de usuario de systemd solo se ejecutan con la sesión iniciada (salvo `loginctl enable-linger`) |

## Monitorización

| | |
|---|---|
| Qué hace | CPU, memoria, red, disco, procesos, tiempo en marcha y reinicios por contenedor; gráficas de 5 minutos a 24 horas; comparación de hasta 4 contenedores; huecos visibles sin conexión; **alertas** con histéresis y silencio, y notificaciones de escritorio |
| Componentes | `monitor/` (motor), `monitor.rs`, `notify.rs`, `features/monitor` |
| Limitaciones | No es una supervisión del sistema operativo; el disco solo si Docker lo da; el historial por minuto ocupa como mucho 64 MiB |

## Eventos e historial

| | |
|---|---|
| Qué hace | **En directo** (eventos de esta sesión), **Guardados** (20 000 más recientes), **Consultar a Docker** (`docker events --since … --until …`) e **Historial de operaciones** (10 000 más recientes) |
| Componentes | `events.rs`, `journal.rs`, `features/events` |
| Limitaciones | Docker solo conserva 256 eventos y los pierde al reiniciarse; el historial no guarda cuerpos, órdenes equivalentes ni salidas |

## Panel de actividad

| | |
|---|---|
| Qué hace | Cada operación con hora, transporte (**API**, **CLI** o **APP**), tipo, recurso, duración, estado, petición real, orden equivalente, resultado y error |
| Componentes | `cmc-activity`, `features/activity` |
| Seguridad | Secretos ocultos (`********`) antes de guardarse o mostrarse |
| Limitaciones | Las comprobaciones automáticas de la conexión no se registran una a una |

## Configuración

| Subapartado | Contenido |
|---|---|
| Apariencia | Tema (sistema, claro, oscuro) e idioma |
| Panel de actividad | Abrir ante errores, operaciones conservadas (50–10 000), guardar historial |
| Conexión con Docker | Automático o ruta personalizada, probar sin guardar, volver a conectar, Compose |
| Actualizaciones | Fuente (GitHub o JSON por HTTPS), comprobar ahora o al iniciar; nunca descarga ni instala |
| Sistema | Versiones, sistema operativo, rutas e informe para copiar |
| Componentes y desinstalación | Linux: análisis con dpkg/APT y desinstalación selectiva con `pkexec`; Windows: instrucciones |
| Legal | Licencia, copyright y avisos de terceros |
| Ayuda | 30 secciones con índice y búsqueda |
| Restablecer | Preferencias a sus valores iniciales (no toca la conexión ni Compose) |

## Migración desde el nombre anterior

La 1.8.0 sustituyó todos los identificadores del nombre anterior del producto.
La aplicación (o `container-manage-center --migrate`, que ejecuta
`install.sh`) pasa sin pérdidas las carpetas de configuración y estado, las
tareas programadas, la carpeta de copias, las contraseñas del llavero y el
servidor elegido. Detalles en `docs/MIGRATION-CMC.md`.
