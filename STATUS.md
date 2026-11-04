# Estado del proyecto

Última actualización: 2026-11-04

## Resumen

| | |
|---|---|
| **Fase actual** | 2 · Integración con Docker Engine |
| **Estado de la fase** | En desarrollo |
| **Fases completadas (validadas)** | 1 |
| **Fases pendientes** | 2–10 |

| Fase | Estado |
|---|---|
| 1. Arquitectura y estructura inicial | **Validada** (2026-11-04, Debian 13, GNOME Wayland) |
| 2. Integración con Docker Engine | En desarrollo |
| 3. Contenedores | No iniciada |
| 4. Imágenes | No iniciada |
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
README, STATUS, DECISIONS (D-001 a D-016), ARCHITECTURE, USER_GUIDE, INSTALL.

---

## Próximos pasos
1. El propietario ejecuta `bash scripts/verify-phase-1.sh` y las comprobaciones manuales, y envía `verify-phase-1-report.txt`.
2. Corregir lo que surja y marcar la fase 1 como validada.
3. Fase 2: crate `dcc-engine` con `bollard`; detección del socket (rootful, rootless, `DOCKER_HOST`), comprobación de permisos con instrucciones, información del sistema y versiones de Engine y Compose, errores de conexión y reconexión, y las primeras operaciones reales en el panel de actividad. Pruebas de integración contra el daemon Docker real del entorno de desarrollo.

## Decisiones pendientes del propietario
- Licencia definitiva (provisional: GPL-3.0-or-later, D-016).
- Nombre definitivo del producto antes de distribuirlo públicamente ("Docker" es marca registrada).
