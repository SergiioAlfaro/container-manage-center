# Cambios

## 0.7.1 — 2026-11-07 (corrección de la Fase 7)
- Corregido: con Docker 26, justo después de detener (o iniciar) un proyecto Compose, su estado podía aparecer como "Parcial" porque Docker actualiza el listado de contenedores un instante después de responder. Tras desplegar, iniciar, detener o reiniciar, la aplicación confirma ahora el estado de cada contenedor con inspect (D-056), como ya hacía con las acciones de contenedores (D-031).
- Nuevas pruebas: carrera determinista con un Docker simulado, cálculo del estado en cada caso y comparación con `docker compose ps -a` e inspect tras cada operación individual y global, con perfiles y sin archivo.
- `verify-phase-7.sh` ejecuta la prueba determinista de la carrera como paso propio.

## 0.7.0 — 2026-11-07 (Fase 7: Docker Compose)
- Apartado **Docker Compose**: proyectos desplegados (detectados por las etiquetas de sus contenedores) y proyectos añadidos a la lista, con estado, carpeta y servicios; filtros y búsqueda (D-053).
- Abrir un proyecto existente (uno o varios archivos) y crear uno nuevo en una carpeta; el nombre lo decide Compose o el usuario.
- Detalle con los servicios del archivo junto a sus contenedores reales (imagen o construcción, puertos, perfiles, huérfanos) y acciones por servicio.
- Desplegar (servicios, perfiles, construir, política de descarga, recrear, huérfanos), iniciar, detener, reiniciar, descargar imágenes, construir y eliminar (con volúmenes e imágenes opcionales y confirmación), siempre con el comando exacto y la salida en vivo, que se puede interrumpir.
- Registros en vivo por servicio, editor del archivo con validación de Compose antes de guardar y protección frente a cambios externos (D-055), y configuración resuelta oculta hasta pedirla porque puede contener secretos (que no se guardan en el panel de actividad).
- Parar, iniciar, reiniciar y eliminar actúan sobre todos los contenedores del proyecto, también los de perfiles no activos, que Compose omitía en silencio (D-054).
- Proyectos cuyos archivos ya no existen: se pueden parar, iniciar, ver registros y eliminar.
- `verify-phase-7.sh` con pruebas reales de Compose (probadas con Compose 2.26.1 y 5.6.1) y comprobaciones manuales M1–M15.

## 0.6.0 — 2026-11-06 (Fase 6: volúmenes y directorios vinculados)
- Apartado **Volúmenes**: lista con contenedores que montan cada volumen, distintivo de anónimos, tamaños bajo demanda, filtros y búsqueda.
- Asistente de creación (local, tmpfs, NFS, carpeta u otro driver) con validación, comando y cuerpo exactos; aviso de nombre ya existente, que Docker ignoraría en silencio (D-051).
- Detalle con directorio, opciones, metadatos, tamaño, contenedores (ruta y acceso) e inspección JSON; crear un contenedor con el volumen.
- Eliminación (Docker la rechaza si lo monta cualquier contenedor) y limpieza de anónimos o de todos los sin uso, con estimación actualizada al abrir.
- Apartado **Directorios vinculados**: carpetas montadas en contenedores con su estado real en el equipo, permisos, contenedores y riesgos (socket de Docker, directorios del sistema, rutas inexistentes); vincular una carpeta con el selector del sistema abre el asistente de contenedores (D-052).
- La lista de contenedores incluye sus montajes (`mountPoints`).
- `verify-phase-6.sh` con pruebas reales de volúmenes y comprobaciones manuales M1–M15.

## 0.5.0 — 2026-11-06 (Fase 5: redes)
- Apartado **Redes**: lista con driver, subredes y contenedores (en marcha o no), filtros y búsqueda.
- Asistente de creación con subred, puerta de enlace, rango, IPv6, red interna, conectable, opciones del driver y metadatos; validación con las reglas de Docker antes de enviar, comando y cuerpo exactos.
- Conexión de contenedores con alias e IP fija, y desconexión con confirmación.
- Eliminación y limpieza con avisos de contenedores en marcha (Docker lo rechaza) y detenidos (Docker la elimina y no podrán arrancar) (D-047).
- Detalle con IPAM, opciones, contenedores con IP y MAC, e inspección JSON.
- Creación con `CheckDuplicate` para no crear redes con nombre repetido en Docker < 25 (D-048).
- Los diálogos consultan el estado actual a Docker al abrirse (D-049).
- `verify-phase-5.sh` con pruebas reales de redes y comprobaciones manuales M1–M16.
- Imágenes (corrección): no se permite limpiar imágenes mientras hay descargas en curso, porque con el almacén de containerd Docker las hace fallar (D-050). La prueba de limpieza ya no se solapa con las de descarga.

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
