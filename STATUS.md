# Estado del proyecto

Última actualización: 2026-11-04

## Resumen

| | |
|---|---|
| **Fase actual** | 2 · Integración con Docker Engine |
| **Estado de la fase** | Implementada y probada en el entorno de desarrollo contra Docker real. **Pendiente de validación externa** en el equipo del propietario. |
| **Fases completadas (validadas)** | 1 |
| **Fases pendientes** | 2–10 |

| Fase | Estado |
|---|---|
| 1. Arquitectura y estructura inicial | **Validada** (2026-11-04, Debian 13, GNOME Wayland) |
| 2. Integración con Docker Engine (ampliada con contenedores e imágenes, D-020) | Pendiente de validación externa |
| 3. Contenedores (creación, logs, inspección, terminal) | No iniciada |
| 4. Imágenes (descarga, construcción, etiquetado, eliminación) | No iniciada |
| 5. Redes | No iniciada |
| 6. Volúmenes y bind mounts | No iniciada |
| 7. Docker Compose | No iniciada |
| 8. Actividad, eventos y monitorización | No iniciada |
| 9. Configuración y optimización | No iniciada |
| 10. Pruebas finales y distribución | No iniciada |

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

---

## Próximos pasos
1. El propietario ejecuta `bash scripts/verify-phase-2.sh` (con un usuario con acceso a Docker) y las comprobaciones M1–M15, y envía `verify-phase-2-report.txt`.
2. Corregir lo que surja y marcar la Fase 2 como validada.
3. Fase 3: creación de contenedores con asistente por pestañas, duplicado, inspección, logs en tiempo real, terminal interactiva, estadísticas básicas.

## Decisiones pendientes del propietario
- Licencia definitiva (provisional: GPL-3.0-or-later, D-016).
- Nombre definitivo del producto antes de distribuirlo públicamente ("Docker" es marca registrada).
