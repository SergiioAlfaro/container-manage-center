# Cambios

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
