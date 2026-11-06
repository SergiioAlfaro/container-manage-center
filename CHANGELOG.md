# Cambios

## 0.4.1 — 2026-11-06 (corrección de la Fase 4)
- Corregidas las pruebas de descarga que fallaban con el almacén de imágenes clásico (overlay2, Debian 13): el registro de prueba servía una imagen que el daemon ya tenía, así que Docker no descargaba ninguna capa. Ahora cada prueba descarga una imagen nueva, con aserciones más estrictas: progreso parcial real, una sola petición de la capa, conexión cortada al cancelar, nueva descarga completa tras cancelar, un registro por descarga (D-045).
- Nueva prueba del caso "contenido ya presente" y nueva prueba IPC contra Docker real de descarga completa y cancelada.
- El resumen de una descarga sin capas dice que Docker ya tenía el contenido, en lugar de "0 capas descargadas".
- Las imágenes sin fecha de creación muestran "—" en lugar de "hace 2027 años" (D-046).
- Las pruebas ya no dejan una imagen sin etiqueta al crear la imagen de prueba en paralelo.
- `verify-phase-4.sh`: infraestructura, pruebas deterministas y pruebas contra Docker real por separado, con el resultado de cada prueba real y el almacén de imágenes del daemon.

## 0.4.0 — 2026-11-06 (Fase 4)
- Descarga de imágenes con progreso por capa, cancelación y credenciales de Docker, anónimas o solo para esa descarga (D-042).
- Búsqueda en Docker Hub, detalle de imagen con capas, contenedores e inspección, etiquetado, eliminación con aviso de uso y limpieza (D-044).
- Asistente de construcción con BuildKit o el constructor clásico, `.dockerignore`, resumen del contexto, pasos y salida en directo (D-041).
- Pruebas reales de descarga con un registro local (D-043).
- `scripts/verify-phase-4.sh`.

## 0.3.0 — 2026-11-05 (Fase 3)
- Asistente de creación de contenedores por pestañas con validación en vivo, comando `docker run` equivalente y cuerpo de la API (D-034, D-035).
- Detalle de contenedor: resumen, logs en directo, terminal interactiva, estadísticas e inspección JSON (D-036, D-037).
- Duplicar contenedores con aviso de los ajustes que no se trasladan (D-038).
- Valores sensibles ocultos en pantalla y en el panel de actividad (D-039).
- La interfaz muestra el error si falla antes de arrancar; xterm.js 5.5 por compatibilidad con `freezePrototype` (D-040).
- `scripts/verify-phase-3.sh`.

## 0.2.1 — 2026-11-04 (corrección de la Fase 2)
- Corregido: tras detener (u otra acción), la tabla de contenedores podía mostrar el estado anterior porque Docker actualiza el listado un instante después de responder. Ahora cada acción confirma el estado con `GET /containers/{id}/json` (D-031).
- Corregido: la detección de Docker Compose usaba `--format json`, que falla con «unknown flag» cuando el plugin no está instalado. Ahora distingue Compose instalado (plugin o binario v2), no instalado, plugin inutilizable, Compose v1 no admitido y respuestas no reconocidas, e indica cómo instalarlo (D-032).
- `verify-phase-2.sh`: omisiones justificadas separadas de los fallos, veredicto explícito, código de salida fiable, limpieza garantizada de los recursos de prueba y comprobación de versiones de herramientas (D-033).
- `dcc-diagnose --compose`.

## 0.2.0 — 2026-11-04 (Fase 2)
- Conexión con Docker Engine, diagnóstico, Dashboard, contenedores, imágenes, sección Docker en Configuración.

## 0.1.0 — 2026-11-04 (Fase 1)
- Estructura, ventana principal, navegación, panel de actividad, configuración.
