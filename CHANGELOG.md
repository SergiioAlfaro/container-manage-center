# Cambios

## 0.2.1 — 2026-11-04 (corrección de la Fase 2)
- Corregido: tras detener (u otra acción), la tabla de contenedores podía mostrar el estado anterior porque Docker actualiza el listado un instante después de responder. Ahora cada acción confirma el estado con `GET /containers/{id}/json` (D-031).
- Corregido: la detección de Docker Compose usaba `--format json`, que falla con «unknown flag» cuando el plugin no está instalado. Ahora distingue Compose instalado (plugin o binario v2), no instalado, plugin inutilizable, Compose v1 no admitido y respuestas no reconocidas, e indica cómo instalarlo (D-032).
- `verify-phase-2.sh`: omisiones justificadas separadas de los fallos, veredicto explícito, código de salida fiable, limpieza garantizada de los recursos de prueba y comprobación de versiones de herramientas (D-033).
- `dcc-diagnose --compose`.

## 0.2.0 — 2026-11-04 (Fase 2)
- Conexión con Docker Engine, diagnóstico, Dashboard, contenedores, imágenes, sección Docker en Configuración.

## 0.1.0 — 2026-11-04 (Fase 1)
- Estructura, ventana principal, navegación, panel de actividad, configuración.
