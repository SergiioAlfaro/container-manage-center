# Estado del proyecto

Última actualización: 2026-11-12

## Resumen

| | |
|---|---|
| **Fase actual** | 10. Pruebas finales y distribución |
| **Versión** | 1.0.0 |
| **Estado** | En curso |
| **Fases completadas (validadas)** | 1–9 y las actualizaciones 0.9.1, 0.9.2 y 0.10.0 |
| **Fases pendientes** | 10 (en curso) |

| Fase | Estado |
|---|---|
| 1. Arquitectura y estructura inicial | **Validada** (2026-11-04, Debian 13, GNOME Wayland) |
| 2. Integración con Docker Engine (ampliada con contenedores e imágenes, D-020) | **Validada** (2026-11-05, Debian 13, versión 0.2.1) |
| 3. Contenedores (creación, logs, inspección, terminal) | **Validada** (2026-11-06, versión 0.3.0) |
| 4. Imágenes (descarga, búsqueda, construcción, etiquetado, inspección, capas, eliminación) | **Validada** (2026-11-06, Debian 13, versión 0.4.1) |
| 5. Redes | **Validada** (2026-11-06, Debian 13, versión 0.5.0) |
| 6. Volúmenes y bind mounts | **Validada** (2026-11-07, Debian 13, versión 0.6.0) |
| 7. Docker Compose | **Validada** (2026-11-07, Debian 13, versión 0.7.1) |
| 8. Actividad, eventos y monitorización | **Validada** (2026-11-08, Debian 13, versión 0.8.0) |
| 9. Configuración y optimización | **Validada** (2026-11-08, Debian 13, versión 0.9.0) |
| Actualización 0.9.1 (terminales, desinstalación, ayuda, francés e italiano, interacción) | **Validada** (confirmada por el propietario en la solicitud de la 0.9.2, 2026-11-11) |
| Actualización 0.9.2 (cuatro idiomas, color morado, imagen desde contenedor, editar configuración) | **Validada** (confirmada por el propietario, 2026-11-12) |
| Actualización 0.10.0 (administración remota de servidores Docker por SSH) | **Validada** (confirmada por el propietario, 2026-11-12) |
| 10. Pruebas finales y distribución | En curso (versión 1.0.0) |

---

## Fase 1 · Arquitectura y estructura inicial

### Objetivos
Estructura del proyecto, entorno de compilación, ventana principal con las tres
zonas, identidad visual, navegación lateral contraíble, panel de actividad
desplegable conectado al backend, configuración persistente, temas e idiomas.

### Funcionalidades implementadas
- Workspace Rust (`dcc-activity`, `dcc-store`, `src-tauri`) y frontend React/TS con Vite y Tailwind 4.
- Ventana principal (1320×840, mínimo 960×600) con barra lateral, área central y panel derecho.
- Barra lateral con los nueve apartados en el orden requerido, agrupados, contraíble y con estado persistente.
- Apartados de fases futuras con estado informativo y sin acciones (D-013).
- **Registro de actividad real en el backend**: modelo de operación (API/CLI, comando equivalente, estado, duración, salidas, errores), capacidad configurable, cancelación segura, notificación a la ventana en tiempo real y resincronización.
- **Panel de actividad**: lista en tiempo real, búsqueda, filtros por área y estado, expansión de detalles, distinción explícita entre operación ejecutada y comando equivalente, copia al portapapeles, salida estándar y de error, limpieza del historial visual, apertura automática ante errores. En esta fase no hay operaciones Docker, por lo que el panel aparece vacío: no se generan registros ficticios.
- Configuración: tema (sistema/claro/oscuro, sigue al escritorio en vivo), idioma (es/en), comportamiento del panel, número de operaciones conservadas, versión y ruta de configuración.
- Persistencia en `~/.config/docker-control-center/config.toml` (atómica, `0600`, recuperación de archivos corruptos, reversión en la interfaz si falla el guardado).
- Atajos: Ctrl+B (barra lateral), Ctrl+Mayús+A (panel de actividad).
- Negativa a ejecutarse como root.
- CSP restrictiva y permisos mínimos de la ventana.
- Iconos de la aplicación y empaquetado `.deb`.

### Archivos principales
`Cargo.toml`, `crates/dcc-activity/*`, `crates/dcc-store/*`, `src-tauri/*`,
`package.json`, `vite.config.ts`, `tsconfig.json`, `ui/**`, `scripts/*`,
`packaging/logo.svg`, documentación raíz.

### Pruebas realizadas (entorno de desarrollo: Ubuntu 24.04, WebKitGTK 2.52)

| Conjunto | Resultado |
|---|---|
| Rust: `dcc-activity` (22 pruebas: registro, cancelación, capacidad, concurrencia con 200 tareas, entrecomillado, serialización) | 22/22 correctas |
| Rust: `dcc-store` (12: valores por defecto, ida y vuelta, permisos 0600, archivos parciales, valores fuera de rango, archivo corrupto) | 12/12 correctas |
| Rust: integración IPC con runtime simulado de Tauri (10: comandos reales, ACL, persistencia, payload inválido, eventos hasta la ventana) | 10/10 correctas |
| Clippy (`--all-targets`) | 0 avisos |
| TypeScript (`tsc --noEmit`, modo estricto) | Sin errores |
| Interfaz: Vitest + Testing Library (34: orden de apartados, navegación sin botones falsos, barra lateral, panel con botón y atajo, API vs CLI, errores, filtros, búsqueda, copiar, limpiar, tema, idioma, reversión ante fallo, límites, apertura ante error, paridad es/en) | 34/34 correctas |
| App real bajo Xvfb: arranque, navegación, panel con atajo, tema oscuro, inglés, contracción con Ctrl+B, persistencia tras reinicio, ruta larga sin desbordar | Correcto (capturas revisadas) |
| Negativa a ejecutarse como root | Correcto (sale con código 1 y mensaje) |
| `shellcheck` de los scripts | Sin avisos |
| Paquete `.deb` (release, 3,0 MiB): metadatos, dependencias, `.desktop` validado con `desktop-file-validate`, instalación con `apt`, arranque desde `/usr/bin` como usuario normal, desinstalación limpia | Correcto |
| `scripts/verify-phase-1.sh` ejecutado como usuario normal sobre una copia limpia del proyecto | 22 correctas, 0 fallos, 0 avisos |

### Validación externa (2026-11-04)
Equipo del propietario: Debian 13 (trixie), núcleo 6.12, GNOME sobre Wayland, WebKitGTK 2.54, Rust 1.99.1, Node 22.23.
- `scripts/verify-phase-1.sh`: 22 correctas, 0 fallos, 0 avisos (Rust 44/44, Vitest 34/34, Clippy limpio).
- Comprobaciones manuales M1–M11: todas correctas.
- D1–D3 (paquete `.deb`, opcionales): no ejecutadas; se repetirán en la Fase 10.

### Problemas detectados y resueltos durante la fase
- La ruta de configuración larga desbordaba la tarjeta de Configuración: ahora se muestra debajo de la etiqueta con ajuste de línea.
- Los mensajes de error se partían a mitad de palabra: se usa `overflow-wrap: anywhere`.
- El botón de limpiar historial era casi invisible: ahora tiene borde.
- Las pruebas IPC con `mock_context` no cargaban la ACL; se usa el contexto real generado en modo prueba.
- El `.deb` declaraba dependencias duplicadas y un archivo `.desktop` con espacios en el nombre: se corrige con nombre interno `docker-control-center` y plantilla `.desktop` propia (D-017).

### Errores conocidos
- Ninguno abierto.
- Aviso inocuo al empaquetar: Tauri no puede marcar el tipo de paquete en el binario. Solo afecta al plugin de actualización automática, que no se usa (se revisará en la Fase 9, "Configurar actualizaciones").
- Memoria del proceso principal medida bajo Xvfb sin aceleración: ~165 MB RSS (WebKitGTK usa además procesos auxiliares). Se medirá en tu equipo y se optimizará en la Fase 9 si procede.
- Limitación del entorno de desarrollo: Docker Hub no es accesible desde él; para las pruebas de integración de fases posteriores se crearán imágenes locales con `docker import`.

### Documentación actualizada
README, STATUS, DECISIONS (D-001 a D-017), ARCHITECTURE, USER_GUIDE, INSTALL.

---

## Fase 2 · Integración con Docker Engine

### Objetivos
Conexión real con Docker Engine por socket Unix (normal y rootless), detección
del endpoint, comprobación de permisos, diagnóstico con instrucciones,
información del sistema, gestión de errores y reconexión. Ampliado por el
propietario: Dashboard con datos reales, contenedores (consultar, iniciar,
detener, reiniciar, pausar, reanudar, eliminar), imágenes (consultar),
sección Docker en Configuración e indicadores de conexión.

### Funcionalidades implementadas
- Crate `dcc-engine`: resolución de candidatos (`DOCKER_HOST`, contexto de Docker, rootless, sistema; o socket personalizado), sondeo de bajo nivel, *handshake* (`/_ping`, `/version`, `/info`), API mínima 1.41, detección de rootless.
- Diagnóstico de 10 causas distintas: sin instalación, sin socket, daemon detenido, permiso denegado (distingue "fuera del grupo" y "sesión sin el grupo activo"), host no soportado, ruta inválida, no es un socket, no es Docker, tiempo agotado, API antigua.
- Monitor: latido cada 10 s, reintento con espera de 2 a 30 s, aviso inmediato ante errores de conexión de cualquier operación.
- Registro de actividad de todas las operaciones con la ruta HTTP exacta enviada (verificada con proxy), comando equivalente y error/código de Docker (D-018, D-019).
- Contenedores: listado y acciones de ciclo de vida, validación de identificadores, bloqueo por recurso, eliminación con confirmación en interfaz y backend (D-021, D-022).
- Imágenes: listado con contenedores que las usan.
- Versión de Docker Compose ejecutando el CLI oficial sin shell (registrado como CLI).
- Interfaz: indicador de conexión en la barra superior, Dashboard real, pantalla de diagnóstico, vistas de Contenedores e Imágenes, sección "Conexión con Docker" (modo, socket, probar, guardar, reconectar, versiones), barrera de errores, textos es/en.
- Configuración `[docker]` persistente (compatible con archivos de la Fase 1).
- Herramienta de terminal `dcc-diagnose` (D-027).

### Pruebas realizadas en el entorno de desarrollo
Ubuntu 24.04, WebKitGTK 2.52, Docker Engine 29.9.2 (API 1.56), Compose 5.6.1.
Pruebas como root (el entorno es un contenedor aislado) y, donde importa, como usuario normal `tester` sin acceso a Docker.

| Conjunto | Resultado |
|---|---|
| Rust deterministas (sin Docker): actividad 22, motor 55, configuración 15, IPC 18 | 110/110 correctas |
| Integración contra Docker real `live_docker.rs` como root | 9 ejecutadas y correctas; 2 omitidas por ser root |
| Las 2 de permisos (`socket_without_permission_is_diagnosed`, `system_socket_denied_for_user_outside_docker_group`) como usuario normal | 2/2 correctas |
| IPC contra Docker real (`live_app_connects_and_serves_real_data`) | Correcta |
| Comparación registro ↔ tráfico real del socket (3 pruebas con proxy, incluido el ciclo de vida completo) | Coinciden exactamente |
| Clippy `-D warnings`, `cargo fmt --check`, `tsc` estricto, shellcheck | Sin avisos |
| Interfaz (Vitest): 72 pruebas, incluidas 38 nuevas (diagnóstico por causa, Dashboard, indicador, contenedores, no bloqueo de la UI, diálogo de eliminación, imágenes, configuración Docker, formato, contrato IPC) | 72/72 correctas |
| App real bajo Xvfb con Docker real: Dashboard con datos reales, contenedores, imágenes, panel, Configuración, tema oscuro | Correcto (capturas revisadas) |
| Acciones desde la app real (detener, iniciar, eliminar forzado) comprobadas con `docker ps -a` | Correcto |
| Daemon detenido con la app abierta → diagnóstico y ping fallido registrado; daemon arrancado → reconexión automática | Correcto |
| App como usuario fuera del grupo docker → "Sin permiso…" con root:docker 660 y orden `usermod` para ese usuario | Correcto |
| App sin red (`unshare -n`, sin DNS ni rutas) con Docker local | Conecta y funciona |
| `scripts/verify-phase-2.sh` como usuario normal **sin acceso a Docker** sobre copia limpia | 18 correctas, 3 fallos esperados (sin acceso a Docker: conexión, pruebas reales y prueba sin red), 1 aviso. Compilación, Rust 120/120 (de ellas 12 reales omitidas en ese paso), Clippy, formato, tipos, Vitest 72/72 y arranque correctos. El diagnóstico mostrado fue PermissionDenied con root:docker 660 |
| Auditoría: único `Command::new` de producción en el ejecutor CLI sin shell; sin cambios de permisos/grupos; sin peticiones externas | Correcto |

### Requisitos de la Fase 2 y su verificación

| Requisito | Verificado aquí | Pendiente en Debian |
|---|---|---|
| 1. Arranca con Docker disponible | Sí | M1 |
| 2. Dashboard con datos del motor real | Sí | M2 |
| 3. Daemon detenido o inaccesible | Sí (daemon detenido, socket inexistente, socket sin proceso, socket que no es Docker) | M8 |
| 4. Errores de permisos identificados | Sí (usuario normal real) | M13 si es posible |
| 5. Reconexión | Sí (daemon real detenido y arrancado) | M8 |
| 6. Actividad = operaciones ejecutadas | Sí (proxy sobre el socket) | M3, M5 |
| 7. Interfaz no bloqueada | Sí (prueba de interfaz con operación pendiente; comandos asíncronos) | M5 |
| 8. Configuración guardada y recuperada | Sí (store, IPC, app) | M10, M11 |
| 9. Compatibilidad rootless | Parcial: detección por XDG_RUNTIME_DIR, /run/user/UID y contexto, probada con pruebas unitarias; sin daemon rootless real (experimento abandonado por indicación del propietario) | M14 |
| 10. Sin Internet | Sí (`unshare -n`) | M9 |
| 11. Sin regresiones de la Fase 1 | Sí (todas las pruebas de la Fase 1 pasan) | M15 |

### Problemas detectados y resueltos
- Un socket que respondía algo distinto de Docker registraba el ping como correcto: la validación pasa a hacerse dentro de la operación registrada.
- Campos snake_case y `bigint` en el contrato IPC (enums con variantes struct, enteros i64): corregido en Rust y protegido con prueba.
- Ciclo de importación en los dobles de prueba que colgaba Vitest.
- Maquetación con el panel abierto (indicador partido, cabecera del diagnóstico estrecha, nombres truncados): ajustada.
- Clippy: variante grande en `Attempt`, `Default` derivable.
- `verify-phase-2.sh`: aviso vacío del CLI, prueba sin red marcada como fallo sin conexión y recuento de pruebas que incluía las omitidas sin decirlo. Corregidos tras ejecutarlo.

### Limitaciones y observaciones
- **Rootless sin daemon real**: ver requisito 9.
- **Ventana en blanco una vez**: en el primer arranque de la app compilada de la Fase 2 se observó una ventana en blanco. No se reprodujo en 7 arranques posteriores y no hubo error de JavaScript. Se añadió una barrera de errores que mostraría cualquier fallo de la interfaz. Si lo ves en tu equipo, avísame.
- Los cambios hechos fuera de la aplicación (CLI) se ven al actualizar, volver a la ventana o cambiar de apartado; en tiempo real llegará con los eventos de Docker (Fase 8).
- Al conectar se registran versión e información dos veces (conexión y Dashboard): son peticiones reales, solo redundantes.
- Los comandos equivalentes usan el ID completo del contenedor, que es lo que se envía a Docker.
- `rust-version = 1.80` declarado; probado con Rust 1.97 (aquí) y 1.99 (tu equipo), no con 1.80.
- Docker Hub no es accesible desde el entorno de desarrollo; las pruebas usan una imagen local (D-026).

### Errores conocidos
Ninguno abierto.

### Verificación del propietario de la 0.2.0 (Debian 13, Docker 26.1.5) — 2026-11-04
Compilación y pruebas de interfaz correctas. Fallos automáticos:
1. `container_lifecycle_against_real_docker_matches_wire`: tras detener, el listado devolvía `running`.
2. Compose: `unknown flag: --short` en el script y error con `--format` en las pruebas.
Sin informe adjunto en la conversación; se trabajó con la descripción de los fallos.

### Correcciones de la versión 0.2.1
| Problema | Causa demostrada | Corrección | Evidencia |
|---|---|---|---|
| Estado `running` tras detener | Docker responde a `POST /stop` en `SetStopped` y actualiza la réplica del listado después (`CheckpointTo`); inspect toma el bloqueo y da el estado real (D-031) | La acción confirma el estado con `GET /containers/{id}/json`; la interfaz lo aplica a la fila | Prueba determinista `stale_list.rs` (listado `running`, inspect `exited`); prueba de interfaz que falla sin la corrección; ciclo de vida real 15/15 ejecuciones; E2E en la app real |
| `unknown flag: --short` / `--format` | El CLI de Docker no encuentra el plugin Compose y toma esas opciones como suyas (reproducido ocultando el plugin, salida 125) (D-032) | Detección con `docker compose version` sin opciones y `docker-compose version`; 6 estados distintos; instrucciones por distribución | Salidas reales con y sin plugin; 9 pruebas unitarias; prueba real con plugin y sin él; pantalla de Configuración |
| Script ambiguo | — | Omisiones separadas de fallos, veredicto, código de salida, limpieza con `trap`, versiones de herramientas (D-033) | Ejecución con fallo real (Xvfb caído): veredicto NO SUPERADA y salida 1; ejecución limpia: salida 0; sin Compose: 2 omisiones justificadas y 0 fallos |

### Pruebas de la 0.2.1 en el entorno de desarrollo
Ubuntu 24.04, Docker 29.9.2. Informe completo: `verify-phase-2-report.txt`.

| Conjunto | Resultado |
|---|---|
| `verify-phase-2.sh --deb` (root con `DCC_VERIFY_ALLOW_ROOT=1`) | 27 correctas, 0 fallos, 2 omitidas justificadamente (pruebas de permisos como root), código 0 |
| Mismo script con el plugin Compose oculto (escenario de Debian 13 sin Compose) | 25 correctas, 0 fallos, 4 omitidas (Compose no instalado y su prueba, más las 2 de permisos), código 0 |
| Pruebas Rust deterministas | 132/132 (incluye 12 reales que en ese paso no hacen nada) |
| Pruebas contra Docker real (motor) | 9 ejecutadas y correctas; 2 de permisos ejecutadas aparte como usuario normal: 2/2 correctas |
| Ciclo de vida real repetido | 15/15 ejecuciones completas de `live_docker` correctas |
| IPC contra Docker real, prueba determinista de la carrera | Correctas |
| Vitest | 74/74 |
| Clippy `-D warnings`, `cargo fmt --check`, `tsc`, shellcheck | Sin avisos |
| App real: detener desde la interfaz → fila "Detenido", panel con "Comprobar estado del contenedor", `docker ps -a` = `exited` | Correcto |
| App real sin plugin Compose: "No instalado" con instrucciones; ejecución CLI real registrada | Correcto |
| `.deb` 0.2.1: instalar, arrancar como usuario normal y como root (desarrollo), desinstalar | Correcto |
| Recursos de prueba tras todo lo anterior | Ninguno |

### Validación externa de la 0.2.1 (2026-11-05)
El propietario comunicó que en su Debian 13 "todo funciona correctamente" y autorizó pasar a la Fase 3. Se entiende que incluye `verify-phase-2.sh` y las comprobaciones manuales M1–M15. El informe no se adjuntó a la conversación, por lo que no hay detalle de omisiones (p. ej. M13 permisos, M14 rootless, Compose).

### Limitaciones de esta verificación
- La carrera de Docker 26 **no se reproduce** con Docker 29 en este entorno (120 ciclos con la API real); la corrección se apoya en el código de moby, una prueba determinista que reproduce las respuestas exactas de la ventana y la prueba real. Debe confirmarse en Debian 13 con `verify-phase-2.sh`.
- No he podido confirmar desde aquí qué rutas instala el paquete `docker-compose` de Debian 13 (las páginas de Debian bloquean el acceso automático). La detección cubre tanto el plugin como el binario independiente.
- Las herramientas del script se han comprobado con Docker CLI 29 y util-linux de Ubuntu 24.04; en Debian 13 solo se usan opciones existentes desde Docker 20.10.
- Docker rootless sigue sin probarse con un daemon real (requisito 9, M14).

---

## Fase 3 · Gestión de contenedores

### Objetivos
Crear contenedores con un asistente por pestañas, duplicar su configuración, inspeccionarlos, seguir sus logs, abrir una terminal interactiva y ver estadísticas de CPU y memoria, mostrando puertos, variables y política de reinicio. Todo con operaciones reales y registradas.

### Funcionalidades implementadas
- **Asistente de creación** (General, Entorno, Red, Almacenamiento, Recursos, Avanzado): nombre, imagen local, comando, entrypoint, directorio de trabajo, variables, etiquetas, red, nombre de host, puertos (IP, puerto del equipo, puerto del contenedor, protocolo), publicar todos, entradas de /etc/hosts, volúmenes y bind mounts (con selector de carpeta y solo lectura), CPU, memoria y reserva, política de reinicio con intentos, usuario, TTY, STDIN, init, raíz de solo lectura, eliminar al terminar, modo privilegiado y capacidades.
- Validación en vivo en el backend (sin hablar con Docker), problemas por pestaña con enlace al campo, comando `docker run`/`docker create` equivalente y cuerpo de la API; nueva validación al crear (D-034, D-035).
- Ayudas con datos reales: imágenes locales, configuración de la imagen elegida (comando, entrypoint, variables, puertos expuestos, volúmenes), redes y volúmenes existentes.
- **Detalle**: acciones, duplicar, eliminar y pestañas Resumen, Logs, Terminal, Estadísticas e Inspección JSON (D-036, D-037).
- **Duplicar** con avisos de lo que no se traslada (D-038).
- Valores sensibles ocultos en pantalla, vista previa y panel de actividad (D-039).
- Arranque robusto: mensaje visible si la interfaz falla antes de arrancar; xterm.js 5.5 por `freezePrototype` (D-040).

### Pruebas realizadas en el entorno de desarrollo (Ubuntu 24.04, Docker 29.9.2, cgroup v1, WebKitGTK 2.52)
| Prueba | Resultado |
|---|---|
| Rust unitarias | 110 motor (25 de especificación, 6 de flujos), 22 actividad, 15 configuración: todas superadas |
| IPC con el runtime simulado de Tauri | 29 superadas, de ellas 2 contra Docker real (`DCC_DOCKER_TESTS=1`) |
| Contra Docker real, contenedores (`live_containers`) | 7 superadas, 5 de 5 ejecuciones seguidas sin restos: crear con una especificación completa, inspeccionar y duplicar (la especificación vuelve idéntica); especificación inválida sin tráfico; imagen inexistente; logs stdout/stderr y cierre; estadísticas; terminal con código de salida y 2 redimensionados contados; cierre de la terminal que termina la shell; listas de redes y volúmenes. El tráfico HTTP capturado coincide con el registro de actividad, y el registro no contiene el valor de una contraseña |
| Contra Docker real, Fase 2 (`live_docker`) y carrera determinista | 11 + 1 superadas |
| Interfaz (Vitest) | 109 superadas (asistente 13, detalle 14, utilidades) |
| Estática | clippy `-D warnings`, `cargo fmt --check`, `tsc` estricto |
| Prototipo congelado | La interfaz compilada arranca en Chromium con `Object.prototype` congelado |
| `verify-phase-3.sh` (como root con `DCC_VERIFY_ALLOW_ROOT=1`) | 29 correctas, 0 fallos, 2 omitidas justificadamente (pruebas de permisos, que no aplican a root), 1 aviso (ejecución como root). Sin recursos de prueba restantes |
| Aplicación real bajo Xvfb (binario de release) | Lista → detalle; resumen con secretos ocultos; logs en directo con marcas de tiempo; terminal real (UTF-8 y colores, cierre que termina la shell confirmado con `docker top`); estadísticas en directo; panel de actividad con cada flujo; asistente completo hasta "Crear e iniciar", con la configuración confirmada por `docker inspect` (puerto, 64 MiB, 0,5 CPU, bind de solo lectura, variable); duplicado que no arranca por puerto ocupado con el error real de Docker; eliminación con confirmación |
| Paquete .deb 0.3.0 | Construido (3,95 MiB; depende de `libwebkit2gtk-4.1-0` y `libgtk-3-0`), instalado con `apt`, arrancado desde `/usr/bin` (asistente con validación en vivo) y desinstalado sin restos en `/usr/bin` |

### Problemas detectados y resueltos durante la fase
- **Ventana en blanco en la aplicación real** (no en el navegador): xterm.js 6 falla al cargar con `freezePrototype`. Reproducido y corregido sin desactivar la protección (D-040).
- Al cerrar la terminal, Ctrl+C y Ctrl+D enviados juntos perdían el Ctrl+D y la shell seguía viva (D-037).
- `inspect_exec` no se registraba y las peticiones `GET /networks?` no coincidían con el registro: corregido tras compararlo con el tráfico real.
- Logs y terminal crecían con la página en vez de desplazarse dentro de su recuadro; el selector de unidad de memoria ocupaba todo el ancho: corregidos tras verlo en la aplicación real.
- La contraseña de una variable aparecía en la vista previa y en el panel de actividad: ahora se oculta (D-039).

### Limitaciones y observaciones
- El selector de carpetas del sistema se abrió en la aplicación real, pero en Xvfb (sin gestor de ventanas) no se pudo completar una selección; cancelarlo no cambia nada. Se comprueba en M4.
- Los resúmenes de cierre de los flujos (p. ej. "Sesión cerrada por el usuario…") los redacta el backend en español y aparecen así también con la interfaz en inglés, igual que los resúmenes del panel de actividad.
- El entorno de desarrollo usa cgroup v1; las estadísticas usan las mismas fórmulas que `docker stats` para v1 y v2. Debian 13 usa cgroup v2: se comprueba en M12.
- La imagen de prueba no incluye `stty`/`tput`; el redimensionado se comprueba en la prueba real por el tráfico (`POST /exec/{id}/resize`) y en la interfaz, y en tu equipo con M10.
- La imagen debe estar en el equipo: la descarga de imágenes es de la Fase 4.
- Docker rootless sigue sin probarse con un daemon real.

### Errores conocidos
Ninguno abierto.

### Validación externa (2026-11-06)
El propietario comunicó que "está todo correcto" y autorizó pasar a la Fase 4. Se entiende que incluye `verify-phase-3.sh` y M1–M16; el informe no se adjuntó, por lo que no hay detalle de omisiones.

---

## Fase 4 · Gestión de imágenes

### Objetivos
Visualizar imágenes locales, descargarlas de registros, buscarlas, inspeccionarlas (etiquetas, identificadores, tamaño, fecha, capas, contenedores que las usan), etiquetarlas, eliminarlas y construirlas a partir de un Dockerfile con un asistente. Todo con operaciones reales y registradas.

### Funcionalidades implementadas
- **Descarga** con progreso total y por capa, cancelación y tres modos de credenciales (las de Docker con *credential helpers*, ninguna, o usuario y contraseña solo para esa descarga), sin guardar credenciales (D-042).
- **Búsqueda** en Docker Hub a través del daemon, con filtro de oficiales y descarga directa del resultado.
- **Detalle** de imagen: resumen, capas (historial), contenedores que la usan e inspección JSON; crear contenedor con la imagen, etiquetar, descargar de nuevo y eliminar.
- **Etiquetado** con validación de la referencia según la gramática de Docker.
- **Eliminación** de una etiqueta o de la imagen completa, con aviso de los contenedores que la usan, y **limpieza** (sin etiqueta o todas las sin uso) con estimación previa (D-044).
- **Asistente de construcción**: contexto con `.dockerignore`, Dockerfile, nombres, argumentos (secretos ocultos), metadatos, etapa final, sin caché, `--pull`, BuildKit o constructor clásico; resumen del contexto antes de enviar; pasos y salida en directo; cancelación (D-041).
- Descargas y construcción continúan al cambiar de apartado.

### Pruebas realizadas en el entorno de desarrollo (Ubuntu 24.04, Docker 29.9.2 con almacén containerd, cgroup v1, sin acceso a Docker Hub)
| Prueba | Resultado |
|---|---|
| Rust unitarias | 159 motor (referencias, credenciales con *helper* falso, progreso de descarga, `.dockerignore`, contexto en tar, protobuf de BuildKit, HTTP de `/build`, `BuildSpec`), 22 actividad, 15 configuración |
| IPC con el runtime simulado de Tauri | 37, de ellas 3 contra Docker real (inspección, capas y construcción a través de la IPC) |
| Contra Docker real, imágenes (`live_images`) | 8 superadas en varias ejecuciones seguidas, sin restos: descarga desde un registro local con progreso y una sola operación registrada, segunda descarga "up to date"; registro con autenticación (sin credenciales y con contraseña errónea fallan, con la correcta descarga; la contraseña no aparece en ningún registro); descarga cancelada; inspección, capas, etiquetado, eliminación (409 o desetiquetado según versión) y limpieza filtrada; búsqueda (error real de Docker por falta de Internet, registrado); construcción BuildKit con dos etiquetas, argumento, metadatos y `.dockerignore` comprobado en la salida del `RUN`; constructor clásico; `RUN` fallido con ambos constructores (sin contenedores intermedios); especificación inválida sin tráfico; construcción cancelada. Tráfico HTTP capturado igual al registro de actividad |
| Contra Docker real, fases anteriores | 11 + 7 + carrera determinista: superadas |
| Interfaz (Vitest) | 133 superadas (imágenes 27 y descargas 5 nuevas) |
| Estática | clippy `-D warnings`, `cargo fmt --check`, `tsc` estricto, shellcheck |
| `verify-phase-4.sh` (como root con `DCC_VERIFY_ALLOW_ROOT=1`) | 30 correctas, 0 fallos, 2 omitidas justificadamente (pruebas de permisos, que no aplican a root), 1 aviso (ejecución como root). Sin recursos de prueba restantes |
| Aplicación real bajo Xvfb (binario de release) | Lista con acciones; descarga desde un registro local (127.0.0.1:5555) con su operación en el panel; búsqueda con el error real de Docker (sin Internet) y apertura automática del panel; detalle y capas de la imagen descargada; asistente de construcción con contexto real (3 archivos, 2 excluidos), nombre y argumento, pasos y salida de BuildKit, "Ver imagen" con la etiqueta `dcc.version=1.2.3`; registro de la construcción con petición, equivalente y cuerpo; eliminación confirmada |
| Paquete .deb 0.4.0 | Construido (4,4 MiB; depende de `libwebkit2gtk-4.1-0` y `libgtk-3-0`), instalado con `apt`, arrancado desde `/usr/bin` (en inglés y tema oscuro: apartado Imágenes y asistente de construcción traducidos) y desinstalado sin restos en `/usr/bin` |

### Problemas detectados y resueltos durante la fase
- bollard 0.21 no permite BuildKit sin gRPC ni varias etiquetas: petición `/build` propia y decodificador protobuf mínimo (D-041).
- El daemon no pasaba a HTTP con el registro de prueba porque este cerraba la conexión al recibir TLS; ahora responde como un servidor Go (D-043).
- Con el almacén containerd la última fase de cada capa es "Download complete": el resumen contaba 0 capas descargadas.
- El constructor clásico dejaba el contenedor de un paso fallido: se envía `forcerm=1`.
- Una prueba de limpieza borraba imágenes intermedias de otra prueba en paralelo: ahora filtra por una etiqueta propia; las imágenes intermedias del constructor clásico las elimina su propia prueba.
- Vistos en la aplicación real: las excepciones internas de `.dockerignore` hacían recorrer carpetas excluidas como `node_modules`; una fila de nombre vacía mostraba "Etiqueta no válida" y enviaba `--tag ''`; tras eliminar desde el detalle se pedía la imagen borrada (404 en el registro); "Etiquetas" se usaba para nombres y para LABEL; "Descargar de nuevo" dependía del digest, que con containerd también tienen las imágenes locales.

### Limitaciones y observaciones
- Sin acceso a Docker Hub en este entorno: la descarga desde Docker Hub, la búsqueda con resultados y las credenciales de un registro real con *credential helper* se comprueban en M3–M7. La descarga se probó de verdad contra un registro local.
- El selector de archivos y carpetas del sistema no se pudo completar en Xvfb (igual que en la Fase 3); se comprueba en M11.
- Los resúmenes de las operaciones largas los redacta el backend en español.
- Docker rootless sigue sin probarse con un daemon real.

### Errores conocidos
Ninguno abierto.

### Verificación del propietario de la 0.4.0 (Debian 13) — 2026-11-06
`verify-phase-4.sh`: 28 correctas, 1 bloque fallido, 2 omitidas, 0 avisos; 260 pruebas Rust y 133 de interfaz correctas; compilación, Clippy, TypeScript y Vite correctos. Fallaron dos pruebas contra Docker real de `live_images`:
- `cancelled_pull_stops_and_is_recorded_as_cancelled` (línea 432): `Finished` en lugar de `Closed`.
- `pull_from_local_registry_reports_progress_and_records_once` (línea 225): el resumen no contenía "1 capas descargadas".

### Análisis y corrección (0.4.1)
**Reproducción.** Con un daemon con el almacén de imágenes clásico (overlay2) se reproducen exactamente los dos fallos en las mismas líneas: con Docker 29.9.2 configurado con `containerd-snapshotter=false` y con Docker 24.0.7 (paquete de Ubuntu extraído en una carpeta temporal, sin instalar). Debian 13 usa Docker 26.1.5 con ese almacén. En el entorno de desarrollo el daemon usaba el almacén de containerd y las pruebas pasaban.

**Causa.** El registro de prueba servía la imagen de prueba exportada del propio daemon. El almacén clásico identifica la imagen por su configuración, ve que ya la tiene y **no descarga ninguna capa**: solo envía "Pulling from…", "Digest: …" y "Status: Downloaded newer image" (mensajes capturados). Por eso:
- `Finished` es correcto: la descarga terminó de verdad (la imagen queda con el nuevo nombre, el mismo ID). El retraso de 20 s del blob no se aplicaba porque Docker no pidió el blob, y la cancelación a los 2 s llegaba con la descarga ya terminada.
- El resumen decía "0 capas descargadas, 0 ya existían". containerd compara las capas comprimidas y la capa recomprimida por el registro le parecía nueva, de ahí que en desarrollo sí descargara.

No había error en el ciclo de vida de la descarga, la cancelación, el progreso ni el registro de actividad: las pruebas daban por hecho un comportamiento que depende del almacén de imágenes.

**Correcciones.**
- Pruebas: el registro local sirve en cada prueba una imagen nueva (capa de 3 MiB aleatoria) que ningún daemon puede tener, enviada a ritmo controlado. Las aserciones no se han relajado; se han reforzado (ver D-045).
- Nueva prueba `pull_of_content_already_present_is_a_real_finished_pull`: el caso de Debian 13, con aserciones exactas para cada almacén.
- Producción: si Docker no informa de ninguna capa, el resumen dice "Docker ya tenía todo su contenido, no ha descargado capas" en lugar de "0 capas descargadas, 0 ya existían".
- Producción (visto en la aplicación real durante la reproducción): una imagen sin fecha de creación aparecía como "hace 2027 años", porque Docker envía la fecha cero de Go (0001-01-01). Ahora se muestra "—", como el "N/A" del CLI (D-046).
- Infraestructura de pruebas: con un daemon sin la imagen de prueba, varias pruebas en paralelo la importaban a la vez y quedaba una imagen sin etiqueta. La creación se serializa.
- Nueva prueba IPC contra Docker real `live_pull_and_cancel_via_ipc`: descarga completa y cancelada con los comandos que usa la ventana.
- `verify-phase-4.sh`: separa infraestructura y calidad (5.1), pruebas funcionales deterministas (5.2) y pruebas funcionales contra Docker real (5.3, con el resultado de cada prueba por nombre); indica el almacén de imágenes del daemon.

**Cancelación con el almacén clásico.** La operación se cancela al momento y Docker registra `Not continuing with pull after error: context canceled`, pero mantiene su conexión con el registro hasta que este empieza a responder y entonces la corta sin leer la capa. `docker pull` interrumpido con Ctrl+C hace exactamente lo mismo (comprobado). Con containerd el corte es inmediato. En ningún caso queda la imagen ni la capa: una nueva descarga baja la capa entera (D-045).

### Pruebas de la 0.4.1 en el entorno de desarrollo
Ver la tabla de abajo; se ejecutaron con tres daemons: Docker 29.9.2 con containerd, Docker 29.9.2 con almacén clásico y Docker 24.0.7 con almacén clásico.

| Prueba | Resultado |
|---|---|
| `live_images` (9) | Superadas con los tres daemons, varias veces seguidas, partiendo también de un daemon vacío; solo queda la imagen de prueba `dcc-test/sleeper:2`, que elimina el script |
| IPC contra Docker real (4) | Superadas con los tres daemons |
| `live_docker` (11) y `live_containers` (7) | Superadas con los tres daemons (sin cambios en fases anteriores) |
| Aplicación real bajo Xvfb con Docker 24.0.7 (almacén clásico) y un registro local que envía 8 MiB despacio | Progreso total y por capa ("Downloading · 1,4 MiB / 8 MiB"); descarga completa con "1 capas descargadas"; cancelación: la operación se cerró en 12 ms, el registro vio a Docker cortar la conexión a mitad de la capa, no quedó imagen ni temporales, un único registro de actividad con "cancelada por el usuario" |

---

## Fase 5 · Redes

### Objetivos
Listar, crear, eliminar e inspeccionar redes; conectar y desconectar contenedores; ver los contenedores conectados. Todo con operaciones reales y registradas.

### Funcionalidades implementadas
- **Lista** con driver, ámbito, subredes, distintivos (predefinida, interna, conectable, IPv6, Compose) y contenedores de cada red, en marcha o no (D-047); filtros y búsqueda.
- **Asistente de creación**: nombre, driver (bridge, macvlan, ipvlan, overlay u otro), red interna, conectable, subred/puerta de enlace/rango IPv4 e IPv6, opciones del driver y metadatos. Validación con las reglas de Docker antes de enviar; comando, petición y cuerpo exactos. Creación con `CheckDuplicate` (D-048).
- **Detalle**: resumen, IPAM, opciones, metadatos, contenedores con IP y MAC, inspección JSON.
- **Conectar** (alias, IPv4/IPv6 fija; contenedores que Docker no aceptaría desactivados con el motivo) y **desconectar** (con confirmación).
- **Eliminar** y **limpiar** con avisos: contenedores en marcha (Docker lo rechaza) y detenidos (Docker elimina la red y no podrán arrancar). Los diálogos consultan el estado actual a Docker al abrirse (D-049).

### Pruebas realizadas en el entorno de desarrollo (Ubuntu 24.04; Docker 29.9.2 con containerd y Docker 24.0.7 con almacén clásico, extraído en una carpeta temporal sin instalarlo)
| Prueba | Resultado |
|---|---|
| Rust unitarias | 175 motor (15 nuevas de redes: CIDR, validación, cuerpo y comando, conexión, miembros), 22 actividad, 15 configuración |
| IPC con el runtime simulado de Tauri | 42 (vista previa, sin conexión, confirmación y validación; 5 contra Docker real, una de redes) |
| Contra Docker real, redes (`live_networks`, 4) | Superadas con Docker 29 y 24: creación con subred, puerta, rango, etiqueta y opción; listado y detalle; conexión con alias e IP fija (comprobadas en el contenedor); conexión repetida rechazada; contenedor detenido conectado; eliminación rechazada con contenedor en marcha; desconexión; eliminación con contenedor detenido (que después no arranca); nombre repetido rechazado con una sola red resultante; solapamiento de subred; red predefinida; alias en bridge; limpieza que coincide con la estimación; IPv6 (en este entorno Docker lo rechaza y se comprueba el error registrado). Tráfico del socket igual al registro |
| Contra Docker real, fases anteriores | 11 + 7 + 9 superadas con ambos daemons |
| Interfaz (Vitest) | 156 (redes: 21 nuevas; imágenes: 1 nueva) |
| Estática | clippy `-D warnings`, `cargo fmt --check`, `tsc` estricto |
| `verify-phase-5.sh` | Docker 29 (containerd), con `--deb`: 32 correctas, 0 fallos, 2 omitidas justificadamente (permisos, no aplican a root), 1 aviso (root). Docker 24 (clásico): 31 correctas, 0 fallos, mismas omisiones. Sin recursos de prueba restantes |
| Aplicación real bajo Xvfb (Docker 29) | Lista real; asistente con errores de subred y puerta de enlace detectados y corregidos; red creada (comprobada con `docker network inspect`); conexión con alias e IP 10.213.250.50 (comprobada con `docker inspect`); contenedor detenido conectado; eliminación bloqueada con contenedor en marcha; desconexión; eliminación con confirmación y contenedor detenido que después no arranca; limpieza de redes creadas desde la terminal; todas las operaciones en el panel de actividad |
| Paquete .deb 0.5.0 | Construido (4,5 MiB), instalado con `apt`, arrancado en inglés y tema oscuro (apartado Redes traducido) y desinstalado sin restos |

### Problemas detectados y resueltos durante la fase
- **Docker < 25 crea redes con nombre repetido** si no se envía `CheckDuplicate` (comprobado con Docker 24: dos redes `dcc-dup`). bollard 0.21 no tiene el campo: la creación usa una petición propia con el campo (D-048).
- **Diálogos con datos desactualizados** (visto en la aplicación real): con redes creadas desde la terminal, la limpieza estimaba "no hay redes que eliminar" y Docker eliminaba dos. Ahora consultan a Docker al abrirse y no permiten confirmar hasta tener datos actuales (D-049).
- **Limpiar imágenes hace fallar las descargas en curso con containerd** (una prueba de la Fase 4 falló una vez durante la verificación). Reproducido con el CLI oficial: 15/15 descargas fallidas en Docker 29 con containerd, 0/15 en Docker 24 clásico. Corrección en la Fase 4 con esta causa demostrada: no se permite limpiar imágenes con descargas en curso; la prueba de limpieza no se solapa con las de descarga (D-050).
- La prueba de la aplicación que usaba Redes como ejemplo de apartado pendiente pasa a usar Volúmenes (Fase 6), con las mismas comprobaciones.
- Una prueba IPC dejaba la red creada si fallaba a mitad: ahora la limpia una guarda.
- Con Docker 24, una red repetida con la misma subred falla por solapamiento antes que por nombre: la prueba de nombre repetido se hace sin subred.

### Limitaciones y observaciones
- IPv6 no está disponible en este entorno (Docker no puede configurarlo); se comprueba que el error real queda registrado. Con IPv6 se comprueba en M14.
- overlay requiere Swarm; macvlan/ipvlan dependen de las interfaces del equipo. Se envía lo indicado y se muestra el error real de Docker.
- Docker 26.1.5 (Debian 13) no se ha podido probar aquí: se ha probado con 24.0.7 y 29.9.2.
- El diálogo de limpieza de imágenes (Fase 4) estima con la lista cargada, igual que hacían los de redes antes de D-049. No se ha cambiado: queda como propuesta.
- Los resúmenes de las operaciones los redacta el backend en español.

### Errores conocidos
Ninguno abierto.

---

## Fase 6 · Volúmenes y directorios vinculados

### Objetivos
Listar, crear e inspeccionar volúmenes; ver qué contenedores usan cada volumen; avisar antes de eliminar volúmenes en uso. Mostrar las carpetas vinculadas (origen, destino, contenedores, lectura/escritura) y permitir elegir carpetas con el explorador del sistema.

### Funcionalidades implementadas
- **Volúmenes**: lista con contenedores que los montan (en marcha o no), distintivo de anónimos y de Compose, tamaños bajo demanda, filtros y búsqueda.
- **Crear volumen**: nombre opcional, tipos habituales del driver local (tmpfs, NFS, carpeta) u otro driver, opciones y metadatos; validación con la regla de Docker; comando y cuerpo exactos; aviso de nombre existente mientras se escribe y comprobación en el backend (D-051).
- **Detalle**: directorio, opciones, metadatos, tamaño, contenedores con ruta y acceso, inspección JSON; crear un contenedor con el volumen.
- **Eliminar** (bloqueado si lo monta cualquier contenedor) y **limpiar** (anónimos, o todos con confirmación; aviso para la API < 1.42). Los diálogos consultan a Docker al abrirse.
- **Directorios vinculados**: carpetas y archivos montados agrupados por ruta, con tipo, permisos, propietario, si existen, contenedores, destino y acceso; avisos de socket de Docker, raíz, directorios del sistema y rutas inexistentes; vincular una carpeta con el selector del sistema o desde una tarjeta abre el asistente de contenedores con el montaje (D-052).

### Pruebas realizadas en el entorno de desarrollo
`scripts/verify-phase-6.sh` ejecutado completo con dos daemons reales, los dos con veredicto "parte automática superada", 0 fallos y sin recursos de prueba sobrantes:

| | Docker 29.9.2 (containerd, API 1.56) | Docker 24.0.7 (almacenamiento clásico, API 1.43) |
|---|---|---|
| Rust deterministas | 315 correctas | 315 correctas |
| Vitest | 179 | 179 |
| Contra Docker real (imágenes, descarga, construcción, redes, volúmenes, IPC) | 11 + 7 + 9 + 4 + 4 + 6 | 11 + 7 + 9 + 4 + 4 + 6 |
| Resultado | 33 correctas (con `--deb`), 2 omitidas, 1 aviso | 32 correctas (sin `--deb`), 2 omitidas, 1 aviso |

- Omitidas, como en fases anteriores: las dos pruebas de permiso denegado, porque root ignora los permisos de archivo. El aviso es el de ejecutar como root en desarrollo.
- `live_volumes` se ha ejecutado por primera vez con Docker 24. Con la API 1.43 se comprueban la limpieza con `all=true` y el rechazo 409 de un volumen montado solo por un contenedor detenido.
- Prueba en la aplicación real con Docker 29:
  - la lista y los tamaños coinciden con `docker system df -v`;
  - un nombre repetido se detiene en el formulario sin llegar a Docker;
  - no se puede eliminar un volumen en uso;
  - la limpieza por defecto solo borra los anónimos;
  - la vista de vinculados muestra el aviso del socket de Docker;
  - "Nuevo contenedor con ella" abre el asistente con el montaje.
- Paquete 0.6.0 instalado con `apt`, ejecutado en inglés y con tema oscuro, con Volúmenes y Directorios vinculados funcionando, y desinstalado sin dejar archivos.

### Problemas detectados y resueltos durante la fase
- **Docker ignora en silencio la creación de un volumen con un nombre existente** (201 con el volumen antiguo, sin las etiquetas ni opciones pedidas). La aplicación lo comprueba antes y lo indica en el formulario (D-051).
- **El tamaño de los volúmenes llega en formatos distintos según la versión de Docker** y el tipo de bollard solo conoce el nuevo: petición propia que lee ambos (D-051).
- Visto en la aplicación real: el asistente solo detectaba el nombre repetido al pulsar Crear; ahora avisa mientras se escribe. La estimación de la limpieza estaba mal puntuada.
- Durante la verificación: `Cargo.lock` no se había regenerado tras subir la versión (el script usa `--locked` y falló al compilar); y el disco del entorno se llenó con artefactos de compilación acumulados (26 GB). Ambos eran del entorno de desarrollo, no del producto; se corrigieron y la verificación se repitió completa.
- La prueba de la aplicación que usaba Volúmenes como ejemplo de apartado pendiente pasa a usar Docker Compose (Fase 7).

### Limitaciones y observaciones
- El selector de carpetas del sistema no se puede manejar en Xvfb (como en fases anteriores): se comprueba en M9.
- La limpieza con Docker anterior a la API 1.42 (Docker 20.10 de Debian 12) se ha implementado según la documentación de Docker pero no se ha podido probar con ese daemon.
- No se ofrece explorar el contenido de un volumen (requeriría un contenedor auxiliar).
- Docker 26.1.5 (Debian 13) no se ha podido probar aquí: se ha probado con 24.0.7 y 29.9.2.

### Errores conocidos
Ninguno abierto.

---

## Fase 7 · Docker Compose

### Objetivos
Importar y crear proyectos Docker Compose, editar el YAML con validación antes de ejecutarlo, desplegar, iniciar, detener, reconstruir y eliminar proyectos, y ver los registros por servicio, siempre con el Docker Compose oficial (sin shell) y mostrando la operación real.

### Alcance decidido (D-053)
La especificación de esta fase solo enumeraba "importar y crear proyectos", "editor YAML con validación antes de ejecutar" y "iniciar, detener, reconstruir y registros por servicio". El alcance concreto se ha definido a partir de eso y de lo que ya existía (detección de Compose, D-032; procesos sin shell, D-005) y está documentado en D-053.

### Funcionalidades implementadas
- **Lista de proyectos**: los desplegados (detectados por las etiquetas de sus contenedores, también los lanzados desde la terminal) y los añadidos a la lista (guardados en la configuración), con estado, carpeta, servicios y la última operación; filtros y búsqueda. Si Compose no está disponible, el apartado explica cómo instalarlo.
- **Abrir proyecto** (uno o varios archivos, nombre decidido por Compose o por el usuario) y **Nuevo proyecto** (crea `compose.yaml` en una carpeta sin archivo Compose, validado antes).
- **Detalle**: servicios del archivo junto a sus contenedores reales (imagen o construcción, estado, puertos, perfiles, servicios que ya no están en el archivo) con acciones por servicio.
- **Acciones**: desplegar (servicios, perfiles, construir, política de descarga, recrear, huérfanos), iniciar, detener, reiniciar, descargar imágenes, construir y eliminar (volúmenes e imágenes opcionales, confirmación obligatoria también en el backend). Comando exacto antes de ejecutar, salida en vivo que sigue al cambiar de apartado, interrupción, una operación a la vez por proyecto.
- **Registros** en vivo de todos los servicios o de los elegidos.
- **Archivo**: editor con validación de Compose sin guardar; guardar valida antes y no sobrescribe un archivo cambiado fuera de la aplicación (D-055).
- **Configuración resuelta**, oculta hasta pedirla porque incluye los valores de las variables, que nunca se guardan en el panel de actividad.
- Proyectos cuyos archivos ya no existen: detener, iniciar, reiniciar, registros y eliminar siguen funcionando con las etiquetas.

### Pruebas realizadas en el entorno de desarrollo
`scripts/verify-phase-7.sh` completo con dos daemons reales; los dos dan "parte automática superada", 0 fallos y ningún recurso de prueba sobrante:

| | Docker 29.9.2 (containerd, API 1.56) | Docker 24.0.7 (almacenamiento clásico, API 1.43) |
|---|---|---|
| Rust deterministas | 369 correctas | 369 correctas |
| Vitest | 197 | 197 |
| Pruebas contra Docker real (imágenes, descarga, construcción, redes, volúmenes, Compose, IPC) | 11 + 7 + 9 + 4 + 4 + 5 + 7 | 11 + 7 + 9 + 4 + 4 + 5 + 7 |
| Resultado | 34 correctas (con `--deb`), 2 omitidas, 1 aviso | 33 correctas (sin `--deb`), 2 omitidas, 1 aviso |

- Las 2 omitidas son las mismas de fases anteriores: las pruebas de permiso denegado, porque root ignora los permisos de archivo. El aviso es por ejecutar como root en desarrollo.
- `live_compose` se ejecutó con el plugin Compose 5.6.1 y, con `DCC_TEST_COMPOSE`, con el binario Compose 2.26.1 (el de Debian 13). Pasa con las dos versiones en los dos daemons: 5 de 5 en cada combinación.
- Prueba en la aplicación real (Docker 29, Compose 5.6.1):
  - Lista: aparecen el proyecto desplegado desde la terminal y el añadido a la lista.
  - Servicios: modelo con perfil y servicio que se construye.
  - Desplegar: comando exacto y salida en vivo hasta "Despliegue completado".
  - Registros en vivo.
  - Editor:
    - el error de Compose se muestra y el archivo no cambia en el disco;
    - un cambio externo se detecta, no se sobrescribe y se puede recargar.
  - Eliminar con volúmenes e imágenes construidas: no queda nada en Docker.
- Paquete 0.7.0 instalado con `apt`, ejecutado en inglés con tema oscuro, con un despliegue real; después se desinstaló sin dejar archivos.

### Problemas detectados y resueltos durante la fase
- **Compose omite en silencio los servicios de perfiles no activos** en `down`, `stop`, `start` y `restart` cuando se le pasan los archivos, y termina con código 0: un `down` dejaba un contenedor de perfil en marcha y la red "en uso". Lo detectó la prueba real del ciclo completo; reproducido con Compose 2.26.1 (la versión de Debian 13) y 5.6.1. Esas acciones pasan ahora `--profile "*"` (D-054).
- Visto en la aplicación real: mensajes de estado con concordancia incorrecta ("Eliminación completado"), la columna Carpeta mostraba el archivo en lugar de la carpeta, el contexto de construcción se mostraba con la ruta absoluta y "Deshacer cambios" dejaba visibles avisos de validación que ya no correspondían. Corregidos.
- Dos botones "Detener" en la misma pantalla (el proyecto y la operación en curso): el de la operación se llama ahora "Interrumpir".
- La prueba de la aplicación que usaba Docker Compose como ejemplo de apartado pendiente pasa a usar Eventos y registros (Fase 8).
- **Fallo de una prueba propia**: el paso "lento" de la prueba de cancelación usaba `sleep`, que la imagen de prueba no tiene, así que la construcción fallaba al instante (código 127). Según la versión de Compose y del daemon, la cancelación llegaba antes o después: con 5.6.1 en Docker 24 falló. La aplicación registró bien el código de salida. Ahora el paso lento es el propio `/sleeper` de la imagen, que solo termina si se cancela, con un límite de 90 s para que la prueba nunca se quede colgada. Es estable con las dos versiones en los dos daemons.
- **Scripts de verificación**:
  - En `verify-phase-7.sh`, el nombre del bloque de Compose generaba un archivo de registro de más de 255 bytes, que no se podía crear, y las pruebas no llegaban a ejecutarse (sí se marcaba como FALLO). Ahora los registros tienen un nombre corto numerado.
  - Al revisarlo apareció un error mío de la Fase 5: al generar `verify-phase-5.sh` y `verify-phase-6.sh` se perdió la última línea, `[ "$FAIL" -eq 0 ]`. Esos scripts terminaban con código 0 aunque hubiera fallos; el veredicto escrito sí era correcto. Se ha restaurado en los dos (contrato de D-033), y se ha comprobado que el script devuelve 1 con un fallo real (Docker 24, antes de corregir la prueba) y 0 sin fallos.

### Limitaciones y observaciones
- El selector de archivos y carpetas del sistema no se puede manejar en Xvfb: abrir un proyecto y crear uno nuevo se han probado con la interfaz simulada y por IPC; en la aplicación real se comprueban en M1 y M14.
- Una validación rechazada por Compose queda en el panel de actividad como operación fallida (el proceso terminó con error), y con "Abrir el panel al fallar" activado el panel se abre.
- No se ofrecen `docker compose run`, `exec`, `scale` ni la edición de archivos `.env` (fuera del alcance de esta fase).
- Docker 26.1.5 (Debian 13) no se ha podido probar aquí; sí Compose 2.26.1, la versión que trae Debian 13.

### Errores conocidos
Ninguno abierto.

---

## Fase 7 · Corrección 0.7.1

### Informe del propietario (0.7.0)
- **Entorno:** Debian 13, Docker 26.1.5, Compose 2.26.1, usuario del grupo docker.
- **Correcto:** todo excepto dos pruebas reales de Compose.
- **Fallos:** `full_lifecycle_with_build_profiles_and_confirmation` (línea 299) y `lifecycle_without_files_and_refusal_of_actions_that_need_them` (línea 443). Las dos esperaban `Stopped` tras detener el proyecto y obtuvieron `Partial`.

### Análisis (D-056)
- **Operaciones de cada prueba:**
  - La primera despliega, detiene el proyecto y comprueba el estado.
  - La segunda despliega `web`, mueve el archivo, detiene sin archivo y comprueba el estado.
  - En las dos, la comprobación lee los proyectos con `GET /containers/json` inmediatamente después de que termine `docker compose stop`.
- **Cálculo del estado (`state_of`):** es correcto. Distingue sin desplegar, en marcha, parcial y detenido. No cuenta los contenedores de `run` ni los servicios aún sin crear. Los servicios de perfiles cuentan como los demás, y sin archivo el estado se calcula igual con las etiquetas. Se ha comprobado caso a caso con pruebas nuevas.
- **Causa:** el listado de contenedores estaba desfasado. Es la carrera de Docker de D-031: Docker responde a la parada antes de actualizar la réplica del listado.
- **Reproducción con Docker 26.1.3:**
  - Las dos pruebas fallaron en 2 de 3 ejecuciones, en las mismas líneas.
  - Comparación directa, con un segundo proyecto como carga: en 2 de 30 ciclos, justo después de `docker compose stop`, el listado daba un contenedor `running`. En esos mismos ciclos, inspect y `docker compose ps -a --format json` daban los dos `exited`.
  - Con Docker 24.0.7 y 29.9.2 no se reproduce.
- **Conclusión:**
  - Era un error de producción: la interfaz también podía mostrar "Parcial" tras detener.
  - No era un error de la lógica del estado ni de la prueba.
  - Las aserciones no se han tocado.

### Corrección
- Tras desplegar, iniciar, detener o reiniciar, también si la acción falla o se interrumpe, se confirma el estado de cada contenedor del proyecto con inspect (`compose_settle`). Es la misma solución que D-031, y reutiliza `container_state` sin modificarlo.
- Cada confirmación queda en el panel de actividad.
- No se ha cambiado ningún módulo de las Fases 1–6.

### Pruebas de regresión
- **`tests/stale_compose.rs`, determinista:** un servidor que responde como Docker 26 en la ventana de la carrera. Sin la confirmación el proyecto sale "Parcial"; con ella, "Detenido". Tampoco se consultan otros proyectos.
- **Unitarias del estado:** todos detenidos (también creados sin arrancar), todos en marcha, mezcla, en pausa, contenedores de `run`, servicios de perfiles, archivos ausentes y servicios aún sin crear.
- **`project_state_matches_docker_after_every_operation`, contra Docker real:**
  - Recorre el ciclo sin desplegar → en marcha → parada individual (parcial) → parada de todos (detenido) → arranque de un servicio → arranque de todos, repetido 3 veces.
  - Después prueba un servicio de perfil, parada y reinicio con perfiles, y el proyecto sin archivo (parada individual, parada total y arranque).
  - Tras cada paso compara el estado de la aplicación con `docker compose ps -a --format json` y con inspect de cada contenedor.
- **Demostrado con Docker 26:** sin la corrección, esta prueba y la original fallan en las dos ejecuciones; con ella pasan en todas, 8 de 8 seguidas.

### Verificación completa
`scripts/verify-phase-7.sh` con tres daemons reales. En los tres: "parte automática superada", 0 fallos, código de salida 0 y ningún recurso de prueba sobrante:

| | Docker 26.1.3 (clásico, API 1.45) | Docker 29.9.2 (containerd, API 1.56) | Docker 24.0.7 (clásico, API 1.43) |
|---|---|---|---|
| Rust deterministas | 373 | 373 | 373 |
| Vitest | 197 | 197 | 197 |
| Carreras deterministas (D-031, D-056) | 1 + 1 | 1 + 1 | 1 + 1 |
| Pruebas contra Docker real (fases 1–7 + IPC) | 11 + 7 + 9 + 4 + 4 + 6 + 7 | 11 + 7 + 9 + 4 + 4 + 6 + 7 | 11 + 7 + 9 + 4 + 4 + 6 + 7 |
| Compose 2.26.1 (`DCC_TEST_COMPOSE`) | 6 de 6 | 6 de 6 | 6 de 6 |
| Resultado | 34 correctas, 2 omitidas, 1 aviso | 35 correctas (con `--deb`), 2 omitidas, 1 aviso | 34 correctas, 2 omitidas, 1 aviso |

- **Omitidas justificadamente:** las dos pruebas de permiso denegado, porque root ignora los permisos de archivo (como en fases anteriores).
- **Aviso:** ejecución como root en desarrollo.
- **No se han podido ejecutar aquí:** Docker 26.1.5 exactamente (se ha usado 26.1.3, de la misma rama), el selector de archivos del sistema (M1, M14) y las pruebas con usuario no root (las cubre tu verificación en Debian 13).
- **Paquete 0.7.1:** instalado con `apt`. Desde la interfaz se desplegó y se detuvo un proyecto real: muestra "Detenido · 0/2", igual que `docker compose ps -a`. Después se desinstaló.
- **Fallo de infraestructura durante la verificación:** en una ejecución intermedia, el arranque de la aplicación falló porque la pantalla virtual del entorno de desarrollo (Xvfb) se había detenido entre turnos (GTK no pudo iniciarse). No era un fallo del producto: cada ejecución arranca ahora su propio Xvfb y la comprobación de arranque pasa en los tres daemons.

---

## Fase 8 · Actividad, eventos y monitorización

**Validada por el propietario el 2026-11-08** (Debian 13, versión 0.8.0: parte automática y M1–M15).

### Objetivos
Ver los eventos de Docker en tiempo real, actualizar la aplicación al instante cuando algo cambia (también desde fuera), consultar los eventos que Docker conserva y mantener un historial de operaciones entre sesiones sin guardar secretos.

### Funcionalidades implementadas
- **Suscripción a los eventos de Docker** mientras hay conexión (D-057): una sola, que se registra en el panel de actividad al abrirse y, si se interrumpe, como operación fallida; se reintenta sola.
- **Actualización en tiempo real**: cada evento recarga, pidiéndolos a Docker, los datos afectados (contenedores, imágenes, volúmenes, redes, Compose, directorios vinculados, panel), agrupando las recargas 250 ms.
- Apartado **Eventos y registros**:
  - **En directo**: filtros por tipo, búsqueda, pausa con contador de nuevos, detalle y enlace al recurso.
  - **Guardados**: eventos recibidos con la aplicación abierta, también de sesiones anteriores, con filtros de fecha, acciones, tipo y texto.
  - **Consultar a Docker**: `docker events --since … --until …` con filtros validados antes de llamar a Docker y el comando equivalente.
  - **Historial de operaciones**: filtros por fecha, categoría, estado y texto; marca "Sesión anterior"; detalle con la petición ejecutada.
- **Persistencia sin secretos** (D-058): `historial.jsonl` y `eventos.jsonl` en `~/.local/state/docker-control-center/` (0700/0600), sin cuerpos, comandos equivalentes ni salidas, con `buildargs` ocultos; opción **Guardar historial**; borrar lo guardado con confirmación.

### Pruebas realizadas en el entorno de desarrollo
`scripts/verify-phase-8.sh` con tres daemons reales:

| | Docker 26.1.3 (clásico, API 1.45) | Docker 29.9.2 (containerd, API 1.56) | Docker 24.0.7 (clásico, API 1.43) |
|---|---|---|---|
| Rust deterministas (unitarias, protocolo, IPC) | 399 | 399 | 399 |
| Vitest | 207 | 207 | 207 |
| Deterministas: carreras D-031 y D-056, interrupción de eventos | 1 + 1 + 3 | 1 + 1 + 3 | 1 + 1 + 3 |
| Contra Docker real (fases 1–8 + IPC) | 11 + 7 + 9 + 4 + 4 + 6 + 2 + 8 | 11 + 7 + 9 + 4 + 4 + 6 + 2 + 8 | 11 + 7 + 9 + 4 + 4 + 6 + 2 + 8 |
| Compose 2.26.1 (`DCC_TEST_COMPOSE`) | 6 de 6 | 6 de 6 | 6 de 6 |
| Resultado | 36 correctas, 2 omitidas, 1 aviso (segunda ejecución, ver abajo) | 37 correctas (con `--deb`), 2 omitidas, 1 aviso | 36 correctas, 2 omitidas, 1 aviso |

- En los tres: veredicto "parte automática superada", 0 fallos, código de salida 0 y ningún recurso de prueba sobrante.
- **Omitidas justificadamente:** las dos de permiso denegado (root ignora los permisos de archivo), como en fases anteriores. **Aviso:** ejecución como root en desarrollo.
- **Prueba en la aplicación real** (Docker 29):
  - En directo: aparecen los eventos de antes de abrir el apartado y los nuevos (create, start, connect, kill, stop, die, disconnect, volume create), con milisegundos y atributos.
  - Contenedores y Dashboard cambian solos tras `docker start`/`docker stop` en la terminal, sin pulsar Actualizar.
  - Consultar a Docker con acciones `start, die`: devuelve también eventos de antes de abrir la aplicación, con el comando equivalente.
  - Tras cerrar y abrir la aplicación, Guardados e Historial conservan lo anterior (marca "Sesión anterior").
  - Carpeta de estado 0700 y archivos 0600; el valor de una variable de entorno secreta no aparece en ninguno de los dos archivos.
- **Paquete 0.8.0:** instalado con `apt`, en inglés y con tema oscuro, con eventos en directo e historial; después se desinstaló.

### Problemas detectados y resueltos durante la fase
- **Primera ejecución de la verificación con Docker 26: 2 fallos.**
  1. `live_app_connects_and_serves_real_data` (IPC contra Docker real) comprueba la lista **exacta** de operaciones registradas y apareció una más: `events.watch`. Es una consecuencia real de esta fase (la aplicación abre la suscripción al conectar, D-057), no un fallo del producto. Esta prueba solo se ejecuta contra Docker real, así que no salió en las ejecuciones deterministas previas. La prueba sigue exigiendo la lista exacta de las demás operaciones, en orden, y además exige **exactamente una** suscripción, correcta y posterior a la conexión. No se ha relajado: su posición entre las consultas de la prueba no es fija porque llega en paralelo.
  2. `container_lifecycle_against_real_docker_matches_wire` (Fases 1–3): Docker devolvió `500 cannot pause container …: OCI runtime pause failed: unable to freeze`. Es un error del runtime (runc) al congelar el cgroup v1 de este entorno, no de la aplicación: la Fase 8 no toca la pausa. Comprobado después con el mismo Docker 26: 30 de 30 pausas con el CLI de Docker, `live_docker` 5 de 5 seguidas y la verificación completa repetida sin fallos y sin ese error en el registro del daemon. No se ha cambiado nada para ello.
- Visto en la aplicación real: el texto de búsqueda de los eventos se cortaba ("…o image"); el campo es ahora más ancho.
- **Infraestructura:** el disco del entorno se llenó con compilaciones incrementales (el enlazador falló con "Bus error"); se borró la carpeta de compilación y las verificaciones se ejecutan sin compilación incremental.

### Limitaciones y observaciones
- Si la aplicación estaba cerrada, sus eventos no están en Guardados; "Consultar a Docker" los recupera solo dentro de los últimos 256 que Docker conserva y no tras reiniciar Docker.
- Los campos de fecha usan el selector nativo del sistema, que sigue el idioma del sistema, no el de la aplicación (aquí, sin idioma configurado, aparecen en formato inglés).
- Docker 26.1.5 exactamente no se ha podido probar aquí (sí 26.1.3), ni con un usuario no root, ni el reinicio de Docker con systemd (M6).

### Errores conocidos
Ninguno abierto.

---

## Fase 9 · Configuración y optimización

### Objetivos
Preferencias pendientes (actualizaciones, información del sistema, restablecer), rendimiento medido, gestión de errores, mejoras visuales y compatibilidad real con Debian 12+ y Ubuntu 22.04+.

### Funcionalidades implementadas
- **Compatibilidad del paquete** (D-059):
  - `Depends` calculado con `dpkg-shlibdeps` (`scripts/deb-set-depends.sh`).
  - Paquete para distribuir compilado en un contenedor de Ubuntu 22.04 (`scripts/build-deb-compat.sh`, sin sudo y con la compilación sin red).
- **Actualizaciones** (D-060): comprobación en un repositorio de GitHub o en un JSON por HTTPS, a petición o al iniciar (desactivada por defecto), con un aviso arriba si hay una versión nueva. Nunca descarga ni instala nada.
- **Sistema**: aplicación, paquete, WebKitGTK, glibc, sistema, escritorio, Docker y Compose, con un informe para copiar.
- **Restablecer las preferencias**, con confirmación.
- **Rendimiento** (D-061):
  - Formateadores reutilizados.
  - Filas memorizadas.
  - Como mucho una recarga por segundo durante las ráfagas.
  - Apartados y terminal cargados bajo demanda y precargados.
- **Errores en el idioma de la interfaz** para los de texto fijo, y un único formato de fecha y hora de 24 horas (D-062).
- La zona horaria de las pruebas de interfaz queda fijada: no dependen del equipo.

### Pruebas realizadas en el entorno de desarrollo
`scripts/verify-phase-9.sh` con tres daemons reales:

| | Docker 26.1.3 | Docker 29.9.2 (`--deb --compat --net`) | Docker 24.0.7 |
|---|---|---|---|
| Rust deterministas (unitarias, protocolo, IPC) | 412 | 412 | 412 |
| Vitest | 221 | 221 | 221 |
| Actualizaciones (servidor local) e IPC de la Fase 9 | correctas | correctas | correctas |
| HTTPS real (TLS, certificados del sistema) | — | correcta | — |
| Contra Docker real (fases 1–8 + IPC) | correctas | correctas | correctas |
| Paquete `.deb` con `Depends` real | — | correcto: `libc6 (>= 2.39)`, que es lo que exige el binario | — |
| Paquete compatible (Ubuntu 22.04) | — | correcto: el binario exige glibc 2.34 | — |
| Compose 2.26.1 | 6 de 6 | 6 de 6 | 6 de 6 |
| Resultado | 38 correctas, 2 omitidas, 1 aviso | 46 correctas, 2 omitidas, 1 aviso | 38 correctas, 2 omitidas, 1 aviso |

Las dos omitidas son las de permiso denegado, porque aquí se ejecuta como root; el aviso es por ejecutar como root en desarrollo. No quedó ningún recurso de prueba.

**Compatibilidad, en un Ubuntu 22.04 real** (debootstrap, glibc 2.35, WebKitGTK 2.50.4):
- El binario de la 0.8.0 no arranca: ``version `GLIBC_2.39' not found``.
- El paquete compatible 0.9.0 se instala con `apt` (resuelve todas sus dependencias), arranca y se conecta a Docker 29.9.2. Lo muestra Configuración → Sistema: Ubuntu 22.04 LTS, glibc 2.35, paquete 0.9.0.
- El paquete normal (compilado en Ubuntu 24.04) es rechazado por `apt` con `Depends: libc6 (>= 2.39) but 2.35 is to be installed`, en lugar de instalarse y no arrancar.

**Rendimiento** (release, Xvfb, Docker 29; 200 contenedores con 40 en marcha, 60 volúmenes y 30 redes; peticiones contadas con un proxy del socket):

| | 0.8.0 | 0.9.0 |
|---|---|---|
| Ráfaga de 100 `docker start` desde la terminal con Contenedores abierto: CPU de la interfaz (WebKit) | 23,0 s | 3,1 s |
| Esa ráfaga: CPU del proceso principal | 1,4 s | 0,7 s |
| Esa ráfaga: peticiones a Docker | 77 | 25 |
| Primera visita a cada apartado hasta pantalla estable | 330–530 ms; Compose 1374 ms | 300–510 ms; Compose 399 ms |
| Peticiones en reposo | 6/min (`/_ping`) | 6/min (`/_ping`) |
| JavaScript inicial | 1044 KB | 500 KB |
| Memoria total (PSS) en reposo | 231 MB | 240 MB |

**En la aplicación real**:
- Configuración → Actualizaciones contra un servidor local: la consulta llega al servidor, aparece la versión nueva con su enlace y se guarda la fuente.
- Con «Comprobar al iniciar», a los ~5 s llega la petición al servidor y sale el aviso.
- Configuración → Sistema muestra los datos reales.

### Problemas detectados y resueltos durante la fase
- **El paquete no servía para Debian 12 ni Ubuntu 22.04**: necesitaba glibc 2.39 y no lo declaraba. Ver la compatibilidad, arriba.
- **Ráfagas de eventos**: se recargaba con cada evento y se repintaba la tabla entera, a más de media CPU. Corregido; hay una prueba que falla sin el cambio (100 recargas frente al máximo de 39).
- **Eventos en directo**: el detalle desplegado se cerraba al llegar otro evento, porque las claves de las filas incluían su posición. Prueba que lo reproducía y corrección.
- **Fechas**: en inglés, el historial mostraba «06:08:33 PM.904». Ahora el formato es único.
- **Primera visita más lenta tras la carga diferida**: lo provocaba la suspensión de `React.lazy`. Visto al medir y corregido.
- **Proxy y fuentes locales**: con un proxy configurado, una fuente `http://127.0.0.1` se enviaba al proxy. Ahora va directa; la prueba falla sin el cambio.
- **Prueba IPC de crear y duplicar**: con una imagen importada sin comando, Docker rechazaba crear el contenedor («no command specified»). La prueba elige ahora una imagen con comando o entrypoint.
- **Errores de mis scripts, corregidos**: una comprobación con `grep -q` en una tubería dio «Broken pipe»; una comparación byte a byte del binario empaquetado no era válida, porque Tauri lo modifica al empaquetar; y el paquete compatible sobrescribía el normal.

### Limitaciones y observaciones
- **Debian 12** no se ha podido arrancar aquí, porque no hay acceso a sus repositorios. La compatibilidad se apoya en lo comprobado: el binario exige glibc ≤ 2.34 (Debian 12 tiene 2.36) y Debian 12 incluye WebKitGTK 4.1. Ubuntu 22.04 sí se ha probado en ejecución.
- **Actualizaciones**: aquí no hay acceso a GitHub. La comprobación se ha probado con servidores locales y con un HTTPS real (`registry.npmjs.org`, que en este entorno no pasa por el proxy). La fuente oficial queda pendiente de decidir.
- **Memoria**: no ha bajado. Unos 120 MB son del proceso de WebKitGTK, que no depende de la aplicación.
- **`build-deb-compat.sh` se ha probado aquí** con una imagen base local equivalente a `ubuntu:22.04` (debootstrap de jammy), porque no hay acceso a Docker Hub. En tu equipo usará `ubuntu:22.04`.
- **Sin probar aquí**: Docker 26.1.5 exactamente (sí 26.1.3), un usuario que no sea root y un escritorio real (GNOME/Wayland).

### Errores conocidos
Ninguno abierto.

---

## Actualización 0.9.1 · Terminales, desinstalación, ayuda, idiomas e interacción

### Objetivos
Cinco bloques antes de la Fase 10, sin rehacer las fases 1–9 (validadas):
1. Francés e italiano.
2. Desinstalación selectiva y componentes.
3. Ayuda integrada con enlace a GitHub.
4. Estados de interacción.
5. Apartado «Terminales» con varias sesiones simultáneas.

### Funcionalidades implementadas
- **Terminales** (D-063, D-064):
  - Varias sesiones `docker exec -it` reales e independientes.
  - Distribución 1 / 2 columnas / 1+2 / 2×2 / filas con desplazamiento, con separadores que se mueven con el ratón o el teclado.
  - Maximizar, renombrar, cerrar, volver a abrir y cerrar todas con confirmación. Las sesiones siguen abiertas al cambiar de apartado. Máximo de 12.
  - Limpieza de los procesos de cada sesión en el contenedor y aviso de contenedor detenido o eliminado.
  - Cierre ordenado de todas las sesiones al cerrar la ventana.
- **Configuración → Desinstalación y componentes** (D-065):
  - Análisis con dpkg, apt-mark, apt-cache y el historial de APT, también los archivos rotados.
  - Clasificación de cada paquete con su motivo; Docker Engine, CLI y Compose como herramientas externas protegidas; datos del usuario que no se borran.
  - Simulación de APT, resumen con el espacio que se libera y confirmación.
  - Desinstalación real con `pkexec apt-get remove --no-auto-remove`, sin `purge`.
- **Configuración → Ayuda y documentación** (D-066): 24 secciones en es/en/fr/it, índice, búsqueda, anterior y siguiente, y el botón «Visitar mi perfil de GitHub» (plugin opener limitado a esa dirección).
- **Francés e italiano** en toda la interfaz, guardados en `config.toml` (D-067).
- **Interacción** (D-068): estados de `Button`, filas con hover, descripción emergente en todos los botones con solo un icono, foco visible y movimiento reducido.

### Cambios en componentes compartidos (y por qué)
- **Motor de la terminal** (`run_terminal`, D-063): necesario para limpiar procesos y detectar el estado del contenedor. La terminal del detalle del contenedor gana la limpieza sin cambios en su interfaz. La prueba `shell_choices` se actualizó deliberadamente, porque la orden ahora imprime el PID antes de `exec`.
- **`StreamRegistry`**: lleva la cuenta de los flujos en ejecución y añade `close_all_and_wait`. El cierre de la ventana espera a que terminen.
- **`Button`**: deja de usar `pointer-events: none` cuando está desactivado (para que muestre su descripción) y añade `loading`.
- **Lista de apartados**: «Terminales» va en el grupo nuevo «Herramientas». La prueba del orden del menú se actualizó deliberadamente.
- **`localeOf`**: añade fr-FR e it-IT.

### Pruebas realizadas (entorno de desarrollo: Ubuntu 24.04, Xvfb; root con DCC_VERIFY_ALLOW_ROOT)

`scripts/verify-update-0.9.1.sh` se ejecutó completo en tres daemons:

| Daemon | Resultado automático |
|---|---|
| Docker 29.9.2 (containerd), con `--deb --compat --net` | 51 correctas, 0 fallos, 3 omitidas, 1 aviso (root) |
| Docker 26.1.3 (clásico) | 43 correctas, 0 fallos, 3 omitidas, 1 aviso |
| Docker 24.0.7 | 43 correctas, 0 fallos, 3 omitidas, 1 aviso |

| Conjunto | Resultado |
|---|---|
| Rust deterministas (workspace) | 444/444 |
| Vitest | 275/275 (antes 221) |
| Real · terminales múltiples (`live_terminals`) | 5/5 en Docker 29, 26 y 24 (y 8 ejecuciones seguidas sin fallos durante el desarrollo) |
| Real · IPC (incluye varias terminales con canales reales y cierre global) | 10/10 en los tres daemons |
| Real · regresión de las fases 1–9 (motor, contenedores, imágenes, redes, volúmenes, Compose, eventos) | Todas superadas en los tres daemons |
| Compose 2.26.1 (binario, como en Debian 13) | 6/6 en los tres daemons |
| Desinstalación real en Ubuntu 22.04 (debootstrap) con el `.deb` compatible 0.9.1 | Superada: instalación con APT, zenity comparte GTK, Docker y Compose protegidos, cancelación (126) sin cambios, desinstalación real, configuración intacta. Repetida dos veces con el paquete 0.9.0 y una con el 0.9.1 |
| Clippy `-D warnings`, `cargo fmt`, `tsc`, compilación de Vite, prototipo congelado | Correctos |
| Paquetes | `.deb` normal (Depends real) y compatible (glibc ≤ 2.35) |
| Recursos de prueba al terminar | Ninguno (verificado por el script) |

**Comprobaciones en la aplicación real** (Xvfb, xdotool y capturas):
- Dos terminales con salida independiente y `stty size` propio.
- Distribuciones con 3, 4 y 5 terminales, maximizar y restaurar, y arrastrar el separador.
- Al cerrar la ventana con `sleep 1000 &` en una sesión, en los contenedores solo queda su proceso principal.
- Desinstalación y componentes con la aplicación no instalada como paquete.
- Ayuda.
- El botón de GitHub abre `xdg-open https://github.com/SergiioAlfaro`.
- Francés y tema oscuro guardados en `config.toml`, con las sesiones conservadas al cambiar de apartado.

### Errores encontrados y corregidos durante el desarrollo
- **Distribución con 3 o más terminales**: los paneles crecían más que la ventana, porque los contenedores intermedios no eran flex. Con 5, una altura escrita como `calc(100vh-190px)` sin espacios no era válida en CSS. Lo detecté en la aplicación real, ya que jsdom no calcula diseño. Corregido, y una prueba comprueba ahora la altura de cada fila y que la última ocupa todo el ancho.
- **Marca del PID visible**: si el usuario escribía antes de que arrancara la shell, el eco llegaba primero. Ahora se busca en los primeros 4 KB.
- **Procesos «restantes»**: eran zombis del PID 1 `sleep infinity`, que no recoge a sus hijos. Las pruebas y la limpieza ignoran el estado Z.
- **Carrera detenido/eliminado**: `docker rm -f` deja el contenedor en estado `removing`, que ahora cuenta como eliminado. Si el código de salida es ≥128, se espera a que Docker actualice el estado.
- **Prueba real de desinstalación**: comprobaba el estado `rc` de dpkg, pero la aplicación no tiene archivos de configuración de dpkg, así que dpkg la olvida. La ausencia de `purge` se comprueba en la orden registrada.

### Limitaciones y observaciones
- **Textos del backend**: los resúmenes del panel de actividad y los errores con datos concretos siguen en español en todos los idiomas (D-067). Se indica en la ayuda.
- **Desinstalación en tu equipo**: la prueba automática real necesita un chroot y root, así que en tu equipo se comprueba a mano (M7–M9). La autenticación real de pkexec no se ha podido probar aquí (no hay agente de polkit); sí su orden exacta y el tratamiento de los códigos 126 y 127.
- **Arranque en Ubuntu 22.04**: en un Ubuntu 22.04 mínimo creado de nuevo con debootstrap, la ventana queda en blanco porque WebKit no inicia su proceso web. Ocurre igual con el paquete 0.9.0, que en la Fase 9 sí arrancó en otro chroot más completo que ya no existe. No lo atribuyo a la 0.9.1, pero el arranque en Ubuntu 22.04 de esta versión no queda comprobado aquí. El binario compatible 0.9.1 sí arranca y funciona en Ubuntu 24.04.
- **`npm audit`** avisa de vulnerabilidades en Vitest y tinypool. Son herramientas de pruebas: no forman parte del paquete.
- **Sin probar aquí**: GNOME/Wayland real, un usuario no root y Docker 26.1.5 exacto.

### Errores conocidos
Ninguno abierto.

---

## Actualización 0.9.2 · Idiomas, color principal, imagen desde contenedor y edición de contenedores

### Objetivos
Cuatro bloques antes de la Fase 10, sin rehacer lo validado (fases 1–9 y 0.9.1):
1. Catalán, gallego, euskera y portugués, en el orden pedido y con toda la interfaz y la ayuda traducidas.
2. Color principal morado integrado en el sistema de temas.
3. «Crear imagen a partir de este contenedor».
4. «Editar configuración» con cambios directos y recreación segura.

### Funcionalidades implementadas
- **Editar configuración** (D-070, D-071):
  - Siete pestañas; cada cambio se clasifica como directo, que exige recrear o de solo lectura.
  - Revisión con la comparación actual/nuevo/se mantiene.
  - Recreación segura: bloqueos, riesgos que se aceptan uno a uno, confirmación, copia `-dcc-backup-`, comprobación del contenedor nuevo y vuelta atrás.
  - Copias que se restauran o eliminan; MAC solo si se pide.
- **Crear imagen** (D-069):
  - Repositorio, etiqueta, autor y mensaje validados en Rust, y nombre completo.
  - Montajes excluidos.
  - Aviso de que las variables de entorno sí quedan en la imagen.
  - Pausa opcional; confirmación si el nombre existe.
  - Abre la imagen en Imágenes.
- **Color principal** (D-072):
  - Morado mediante `data-accent`, que solo cambia los tokens del acento, en claro y oscuro, al momento, también en las terminales abiertas. Se guarda.
  - `--beacon-nav` para la barra lateral.
  - Verde azulado claro ajustado a AA (4,43 → 4,61:1).
- **Idiomas** (D-073):
  - ca, gl, eu y pt-PT (unas 1900 cadenas cada uno).
  - Selector desplegable en el orden English, Español, Català, Galego, Euskara, Français, Italiano, Português, con endónimos.
  - Ayuda de 26 secciones en los ocho idiomas.
- **Panel de actividad** (D-074): distintivo APP; registra idioma, color, rename, update, recreación, restauración y commit, sin valores sensibles.
- **Teclado** (D-074): menú de acciones accesible (botón ⋯ y botón derecho) y grupos de opciones con flechas.

### Cambios en componentes compartidos (y por qué)
- **`Transport::App`** en el registro de actividad: para operaciones que no son peticiones a Docker ni procesos. El historial, el panel y el diagnóstico lo admiten.
- **`Language` y `UiConfig`**: cuatro idiomas más y el campo `accent` (por defecto `teal`). Una configuración antigua sigue cargando igual.
- **`Segmented`**: flechas para elegir opción. No cambia el orden de tabulación ni el aspecto.
- **Barra lateral**: usa `--beacon-nav`. El icono activo tenía un contraste de 1,9–2,6:1 (defecto demostrado).
- **Verde azulado claro**: `#0e858a` → `#0d8287`, para cumplir AA con texto blanco. No se aprecia a simple vista.
- **`CreateWizard`**: los editores de puertos, montajes, memoria y clave/valor se movieron sin cambios a `create/editors.tsx` para compartirlos con la edición. Sus 45 pruebas siguen pasando.
- **`Dialog`**: opción `wide`. **`DisconnectDialog`**: aviso opcional de «última red».
- **Selector de idioma**: pasa de botones a lista desplegable (ocho opciones no caben). Las pruebas que lo usaban se adaptaron a la lista, sin quitar aserciones.

### Pruebas ejecutadas
`scripts/verify-update-0.9.2.sh` completo en tres daemons. En este entorno se ejecuta como root (con `DCC_VERIFY_ALLOW_ROOT`), en Xvfb y Ubuntu 24.04.

| Daemon | Resultado |
|---|---|
| Docker 29.9.2, con `--deb --compat --net` | **56 correctas, 0 fallos**, 3 omitidas, 1 aviso (root) |
| Docker 26.1.3 | **48 correctas, 0 fallos**, 3 omitidas, 1 aviso |
| Docker 24.0.7 (API 1.43) | **48 correctas, 0 fallos**, 3 omitidas, 1 aviso |

| Conjunto | Resultado |
|---|---|
| Clippy `-D warnings`, `cargo fmt --check`, `tsc`, Vite | Correctos |
| Rust deterministas | 483/483 (eran 444) |
| Vitest | 328/328 (eran 275) |
| Edición, recreación y commit contra Docker real (`live_edit`) | 8/8 en los tres daemons |
| IPC contra Docker real | 10/10 en los tres daemons |
| Regresión real de las fases 1–9 y de la 0.9.1 | Todo superado en los tres daemons |
| Compose 2.26.1 (como en Debian 13) | 6/6 en los tres daemons |
| Paquetes | `.deb` con glibc 2.39 y Depends real; `.deb` compatible con glibc 2.34 |
| Instalación con APT del `.deb` y arranque del binario instalado | Correcto; desinstalado después |
| Recursos de prueba al terminar | Ninguno |

**Omisiones** (las mismas en los tres daemons): dos pruebas de permisos que no tienen sentido como root, y la desinstalación real con APT. Esta última no cambia en la 0.9.2 y quedó validada en la 0.9.1.

**Comprobación en la aplicación real** (Xvfb, Docker 29, con capturas):
- Morado en claro y oscuro, aplicado al momento y guardado.
- Menú ⋯.
- Crear imagen, con montajes excluidos, y abrirla en Imágenes.
- Editar: pestañas, pendientes, revisión, recreación real que conserva variable secreta, volumen, carpeta y puerto, y deja la copia.
- Activity panel e historial: todas las operaciones están, sin «secreto».
- Euskera y portugués, y la ayuda en euskera con las 26 secciones.

De esta comprobación salieron cinco correcciones (sección siguiente).

### Errores encontrados y corregidos durante el desarrollo
- **Menú de acciones recortado** (aplicación real): dentro de la tabla con desplazamiento, el menú quedaba cortado y movía la tabla. Ahora se pinta en un portal con posición fija, se abre hacia arriba si no cabe y se cierra al desplazar.
- **Copia invisible al recrear y renombrar a la vez** (aplicación real): la copia llevaba el nombre antiguo y no aparecía entre las del contenedor nuevo. Ahora lleva el nombre nuevo. La prueba real nueva falla sin la corrección y pasa con ella.
- **Cabecera de la edición tras recrear**: seguía mostrando el nombre y el ID antiguos (ahora de la copia). Ahora muestra el contenedor nuevo.
- **Texto del paso «se crea un contenedor nuevo con el nombre original»**: era falso al renombrar. Ahora muestra el nombre real, en los ocho idiomas.
- **Variables de entorno en la imagen**: al revisar la imagen creada en la aplicación real se vio que `docker commit` guarda también las variables secretas. El diálogo y la ayuda lo advierten ahora.
- **Docker 26 y 24 sin `ExposedPorts`**: los puertos no se publicaban en la prueba de vuelta atrás. Se corrigió el contenedor de prueba, sin cambiar la aserción.
- **Nombre de copia repetido en el mismo segundo**: `backup_name` avanza la marca de tiempo hasta encontrar un nombre libre.
- **MAC en Docker 26 y 24** (D-071): Docker 24 no admite fijar la MAC del endpoint. La opción se rechaza ahí y se avisa.
- **Carrera en la prueba IPC de Compose** (`live_compose_via_ipc`, existente desde la Fase 6): falló una vez en Docker 29 con `engine.busy`.
  - Causa: la prueba esperaba solo a que el registro del despliegue terminara, pero el proyecto sigue bloqueado unos instantes mientras se confirma el estado de sus contenedores.
  - La aplicación es correcta: la ventana espera al evento de fin.
  - Se reprodujo de forma determinista añadiendo un retardo temporal: la prueba original falla en la misma línea y con el mismo error, y la corregida, que espera a que la tarea termine de verdad, pasa. El retardo se retiró.
- **Disco lleno** en una pasada de regresión (Docker 26 y 24 no pudieron enlazar): era del entorno, no del código. Se liberó espacio y se repitió todo.

### Limitaciones y observaciones
- **Traducciones**: completas y comprobadas por pruebas (claves, variables, nada vacío ni igual al inglés salvo excepciones explícitas), pero sin revisión de un hablante nativo. Es recomendable, sobre todo para el euskera, antes de distribuir.
- **Textos del backend** (sin cambios, D-067): los resúmenes del panel de actividad y los errores con datos concretos siguen en español en todos los idiomas.
- **Puerto ocupado por un programa ajeno a Docker**: no se puede detectar antes de iniciar. La recreación vuelve al original automáticamente (probado con Docker real).
- **Descripción emergente nativa** (WebKitGTK): si el puntero se queda quieto sobre el botón ⋯ antes de pulsarlo, la descripción puede seguir visible sobre el menú hasta que se mueve el ratón. Es cosmético.
- **Sin probar aquí**: GNOME/Wayland real, un usuario no root, Docker 26.1.5 exacto y una revisión nativa de los idiomas.

### Errores conocidos
Ninguno abierto.

---

## Actualización 0.10.0 · Administración remota de servidores Docker por SSH

### Objetivos
Administrar el Docker de otros equipos Linux por SSH, a la vez que el local y con las mismas funciones, sin rehacer lo validado y sin rebajar la seguridad: nada de Docker en TCP sin TLS, secretos solo en el almacén del sistema, identidad del servidor siempre verificada, órdenes sin concatenar cadenas y ningún cambio automático en el servidor.

### Funcionalidades implementadas
- **Servidores** (D-078): lista con estado, versiones, sistema, última conexión y latencia; asistente de alta con siete comprobaciones; editar, credenciales, conectar/desconectar, diagnóstico y eliminar perfil sin tocar el servidor.
- **Transporte** (D-075): OpenSSH del sistema + `docker system dial-stdio`; sesión maestra por servidor y canales multiplexados; argumentos separados y entrecomillados uno a uno; sesiones huérfanas cerradas al arrancar.
- **Identidad** (D-076): huella confirmada en el primer contacto; cambio = conexión detenida con aviso; sustitución solo tras revisarla; `~/.ssh/known_hosts` intacto.
- **Credenciales** (D-077): agente, clave, clave con frase o contraseña (desaconsejada); secretos en Secret Service solo si se pide; sin almacén se avisa y se piden cada vez.
- **Paridad remota** (D-079, D-080): todas las pantallas y operaciones en el servidor elegido; Compose y archivos en el servidor, con confirmación del destino; carpetas vinculadas comprobadas en el servidor.
- **Varios servidores** (D-079): selector, vista general, cada operación ligada a su servidor, confirmaciones con servidor y dirección, fallos aislados.
- **Terminales** (D-081): de contenedores de varios servidores a la vez, etiquetadas, y shell SSH del servidor como sesión aparte; sin procesos restantes.
- **Actividad, eventos e historial**: servidor, conexión y orden equivalente en cada entrada; filtro por servidor.
- **Ayuda** (27 secciones en los ocho idiomas), INSTALL.md con los requisitos de cada servidor; el paquete recomienda `openssh-client`.

### Cambios en componentes compartidos (y por qué)
- **`AppState`**: `engine` pasa a `servers` (registro con el gestor local y los remotos). Todas las órdenes de Docker reciben el servidor; sin él, la operación podría ir a un servidor distinto del elegido.
- **IPC de la interfaz**: `api` se divide en `dockerApi(server)` (órdenes de Docker) y las propias de la aplicación; `useEngine`, las cachés de consultas y los diálogos dependen del servidor del ámbito.
- **`Dialog`**: línea opcional con el servidor de destino. **`Menu`**: `buttonClassName`.
- **Registro de flujos**: cada flujo con su servidor, para cerrar los de un servidor al desconectarlo.
- **Compose**: la lista de proyectos registrados es por servidor (antes se guardaban los remotos en la lista local, error detectado en las pruebas); `compose_projects` comprueba los archivos de todos los proyectos.
- **`cli.rs`**: `CliRequest.remote` para ejecutar el mismo programa en el servidor.
- **`EngineManager`**: constructor remoto, `same()` y `Debug`.

### Pruebas ejecutadas
Ejecutadas con `scripts/verify-update-0.10.0.sh` (registros completos de cada paso) en Ubuntu 24.04 con Docker 29.9.2, 26.1.3 y 24.0.7, Compose 5.6.1 y, aparte, Compose 2.26.1:

| Entorno | Resultado |
|---|---|
| Docker 29.9.2 + laboratorio SSH (`--deb --compat --net --ssh-lab`) | 84 correctas, 0 fallos, 4 omitidas justificadamente |
| Docker 26.1.3 | 59 correctas, 0 fallos, 5 omitidas justificadamente |
| Docker 24.0.7 | 59 correctas, 0 fallos, 5 omitidas justificadamente |
| Compose 2.26.1 (en los tres) | 6/6 |

- **Laboratorio SSH.** Tres servidores `sshd` reales: A = Docker 29.9.2, B = 26.1.3, C = 24.0.7, cada uno con su usuario.
  - `live_ssh` 12/12 (huellas, identidad cambiada, `~/.ssh/known_hosts` intacto, clave, frase y contraseña, problemas de Docker, tres servidores aislados, conexión perdida y recuperada, CLI remoto, sesiones huérfanas, Compose remoto, shell SSH, carpetas del servidor).
  - IPC con tres servidores, sin llavero y con un llavero real temporal.
  - Las suites de contenedores, imágenes, redes, volúmenes, terminales, edición y eventos, repetidas por SSH.
  - No quedan procesos ssh ni recursos de prueba en ningún servidor.
- **Deterministas.** 561 pruebas Rust y 374 de interfaz; clippy y formato limpios.
- **Revisión visual.** Paquete instalado con los tres servidores del laboratorio: asistente y huella, siete comprobaciones, vista general, borrado remoto con confirmación, panel de actividad, shell SSH, fallo de un servidor y su diagnóstico, temas, color e idiomas.
- **Omisiones (justificadas).**
  - Dos pruebas de permisos que no aplican como root.
  - Sin laboratorio, las pruebas reales por SSH (en Docker 26 y 24 solo se ejecutan con el laboratorio de la pasada de Docker 29).
  - La prueba IPC del laboratorio en 5.3, porque se ejecuta en 5.4.
  - La desinstalación real con APT, que necesita un chroot y no cambia.

### Limitaciones y observaciones
- **Servidor intermedio (`ProxyJump`).** OpenSSH comprueba su identidad con el `~/.ssh/known_hosts` y la configuración del usuario.
  - Una huella desconocida o cambiada se rechaza (comprobado), pero la aplicación no la muestra ni la confirma.
  - Hay que conectarse antes una vez con `ssh -J`.
  - No se ha probado una conexión completa a través de un intermedio: el laboratorio no permite el reenvío.
- **Docker 20.10–23.x.** Se acepta por versión de API (mínimo 1.41), pero solo se han probado 24.0.7, 26.1.3 y 29.9.2.
- **Rutas del servidor.** En un servidor remoto se escriben a mano (no hay explorador de archivos remoto). Solo se editan archivos Compose: no hay transferencia general de archivos.
- **Credenciales recordadas.** Solo en almacenes compatibles con Secret Service.
- **Mensajes técnicos.** El detalle de algunos errores del motor está en español sea cual sea el idioma de la interfaz, como en versiones anteriores.
- **Tamaño de la interfaz.** El paquete principal ocupa 1.302 kB (≈1.127 kB en la 0.9.2), sobre todo por los textos de los ocho idiomas. Se mantiene el aviso de Vite de más de 500 kB.
- **Traducciones.** Se recomienda que hablantes nativos revisen los textos nuevos en catalán, gallego, euskera, francés, italiano y portugués.
- **Pruebas por SSH en tu equipo.** El script solo las ejecuta con un laboratorio (`--ssh-lab`, que se monta solo en una máquina de pruebas). En tu equipo se validan con R1–R10.

### Errores conocidos
Ninguno conocido. Corregidos durante la verificación (cada uno con una prueba que fallaba sin la corrección):
- La copia de seguridad de una recreación podía crearse con otro nombre si se confirmaba en otro segundo. Era el fallo intermitente de `live_edit`.
- Avisos del asistente sin traducir: los códigos llevan el prefijo `server.`.
- Configuración → Conexión con Docker mostraba la conexión del servidor remoto elegido.
- Origen `ssh` sin traducir y textos y estilos menores de la lista y de la shell SSH.

---

## Fase 10 · Pruebas finales y distribución (1.0.0)

### Objetivos (especificación original, §16 y §17)
Pruebas completas, instalación limpia, generación de paquetes, documentación y verificación de todas las funcionalidades. Entrega: código fuente, dependencias documentadas, script de instalación, script de ejecución, paquete `.deb`, documentación de usuario y técnica, y pruebas automatizadas. Además, por petición del propietario: eliminar el verde como color principal.

### Funcionalidades implementadas
- **Color principal solo morado** (D-082):
  - en el sistema central de temas, sin selector;
  - migración de las preferencias guardadas (`teal` → morado, conservando el resto);
  - logotipo e iconos en morado;
  - colores semánticos sin cambios.
- **Paquetes** (D-083):
  - versión 1.0.0;
  - `copyright` DEP-5, `changelog`, documentación y `THIRD-PARTY.md` en `/usr/share/doc/docker-control-center/`;
  - icono de 256 px en la carpeta estándar;
  - paquete normal y paquete compatible.
- **`scripts/install.sh`**: instala, comprueba y desinstala. **`scripts/run.sh`**: ejecuta la aplicación.
- **Instalación limpia reproducible**: `scripts/dev/clean-install-test.sh`.
- **`scripts/verify-phase-10.sh`**: regresión completa, comprobaciones de la Fase 10 y pasos que fallan si no ejecutan pruebas (D-084).
- **Documentación**: README, INSTALL (instalador, ejecución, dependencias y desinstalación), USER_GUIDE, ayuda en los ocho idiomas, ARCHITECTURE, DECISIONS (D-082–D-084), CHANGELOG y `LICENSE`.

### Pruebas ejecutadas
@@PRUEBAS-F10@@

### Problemas detectados y resueltos
@@PROBLEMAS-F10@@

### Limitaciones
@@LIMITACIONES-F10@@

---

## Próximos pasos
1. El propietario ejecuta `bash scripts/verify-phase-10.sh --deb --net` en Debian 13 y envía `verify-phase-10-report.txt`. Si el veredicto es «parte automática superada», realiza F1–F8.
2. Con su validación, la Fase 10 queda completada y el proyecto, terminado en su versión 1.0.0.

## Decisiones pendientes del propietario
- Licencia definitiva (provisional: GPL-3.0-or-later, D-016).
- Nombre definitivo del producto antes de distribuirlo públicamente ("Docker" es marca registrada).
- Dónde se publicarán las versiones (repositorio de GitHub o dirección HTTPS): es la fuente que vendría configurada para la comprobación de actualizaciones (D-060). Hasta entonces la fuente está vacía y la aplicación no se conecta a Internet.
