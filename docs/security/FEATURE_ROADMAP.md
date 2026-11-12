# Hoja de ruta de nuevas herramientas

Propuestas de nuevas herramientas para Docker Control Center 1.0.0. **Ninguna
está implementada.** Cada una responde a un problema concreto de quien
administra Docker en local o por SSH con esta aplicación. Se descartan las que
solo añadirían botones.

Criterios:

- se reutiliza primero lo que la aplicación ya tiene (bollard, el CLI, SSH, el
  panel de actividad y el módulo de riesgos de R-01);
- las herramientas externas son opcionales: se detectan y nunca se instalan;
- toda operación queda en el panel de actividad, con la orden equivalente.

**Licencias y requisitos comprobados el 12-11-2026.** Hay que revisarlos de
nuevo antes de integrar cada herramienta.

| Herramienta externa | Licencia | Requisitos y observaciones |
|---|---|---|
| Trivy (Aqua Security) | Apache-2.0 | Binario Go y base de vulnerabilidades descargada de Internet. **Incidente de cadena de suministro en marzo de 2026:** la versión v0.69.4 era maliciosa (CVE-2026-33634). Hay que fijar la versión y verificar la firma cosign y la suma |
| Grype y Syft (Anchore) | Apache-2.0 | Binarios Go; base de vulnerabilidades descargada |
| Dockle | Apache-2.0 | Analiza la configuración de imágenes según CIS |
| Docker Bench for Security | Apache-2.0 | Script que se ejecuta en un contenedor **con privilegios elevados** y montajes del equipo (`/etc`, `/var/lib`, el socket…) |
| Hadolint | GPL-3.0 | Binario Haskell; compatible con la licencia provisional GPL-3.0-or-later |
| restic | BSD-2-Clause | Binario Go; repositorios cifrados |
| Docker Scout | Depende de una suscripción de Docker | Plan gratuito: 3 repositorios con Scout y análisis local ilimitado. La licencia del CLI no se ha podido confirmar. **No se recomienda** como dependencia |

---

## Seguridad

### H-01 · Auditoría de configuración de contenedores en ejecución ⭐

- **Qué hace.** Recorre los contenedores de un servidor y aplica las reglas de
  riesgo (R-01, R-04 y R-05) a su configuración real (`inspect`):
  - privilegiado, capacidades, red, PID o IPC del equipo, `seccomp` o
    `apparmor` desactivados;
  - socket de Docker montado, montajes sensibles;
  - puertos en todas las interfaces, usuario root, sin límites de memoria o
    CPU, `restart` sin límite;
  - imagen `:latest` sin *digest*.
- **Problema que resuelve.** Hoy los avisos solo aparecen al crear o editar. Lo
  que ya está desplegado, incluido lo que viene de Compose o del CLI, no se
  revisa nunca.
- **Integración.** Nueva vista «Seguridad» por servidor que reutiliza
  `risk.rs`. Cada aviso enlaza con «Editar configuración».
- **Dependencias.** Ninguna externa.
- **Recursos.** Un `inspect` por contenedor, con concurrencia limitada.
- **Dificultad.** Media.
- **Riesgos de seguridad.** Solo lectura. Muestra la configuración (ocultando
  los secretos con R-02).
- **Local o remoto.** Ambos.

### H-02 · Análisis de vulnerabilidades de imágenes (Trivy o Grype, opcional) ⭐

- **Qué hace.** Analiza una imagen local o del servidor y muestra las CVE por
  severidad, con la versión que las corrige.
- **Problema que resuelve.** Saber si una imagen en uso tiene vulnerabilidades
  conocidas sin salir de la aplicación.
- **Integración.**
  - Si `trivy` o `grype` están en el PATH, se ejecutan sin shell con
    `--format json`, sobre `docker-archive` o sobre el socket.
  - En remoto, solo si la herramienta está instalada en el servidor.
  - Nunca se descargan binarios desde la aplicación.
  - Se muestra la versión detectada y se avisa si coincide con versiones
    retiradas (lista de versiones bloqueadas, empezando por Trivy v0.69.4).
- **Dependencias.** Trivy o Grype (Apache-2.0), instalados por el usuario.
- **Recursos.** CPU y disco altos durante el análisis; la base de datos ocupa
  cientos de MB.
- **Dificultad.** Media.
- **Riesgos de seguridad.**
  - Ejecuta un binario de terceros: es un riesgo de cadena de suministro.
  - La descarga de la base va a Internet.
  - Mitigación: verificación de la versión y documentación de cómo verificar la
    firma.
- **Local o remoto.** Ambos, si la herramienta existe en el destino.

### H-03 · Análisis de Dockerfile antes de construir (reglas propias y Hadolint opcional)

- **Qué hace.** Avisa de:
  - `USER` ausente (se ejecuta como root);
  - `ADD` con URL;
  - secretos en `ENV` o `ARG`;
  - `curl | sh`;
  - imagen base sin etiqueta fija.
- **Problema que resuelve.** Imágenes inseguras desde el origen. La aplicación
  ya lee el Dockerfile para las imágenes base (`build/mod.rs`).
- **Integración.**
  - Reglas propias en `dcc-engine`, sin dependencias.
  - Si `hadolint` está instalado, se añaden sus resultados (`-f json`).
- **Dependencias.** Ninguna, o Hadolint (GPL-3.0), opcional.
- **Recursos.** Mínimos.
- **Dificultad.** Baja.
- **Riesgos de seguridad.** Mínimos. Hadolint se ejecuta sin shell sobre el
  archivo.
- **Local o remoto.** Local. En remoto, el Dockerfile se lee por SSH.

### H-04 · Comprobación CIS del equipo con Docker Bench for Security (guiada)

- **Qué hace.** Ejecuta la comprobación CIS del daemon y del equipo y muestra
  los resultados.
- **Problema que resuelve.** Configuración insegura del propio Docker Engine:
  `live-restore`, `userns`, registros, TLS.
- **Integración.**
  - Asistente que **muestra la orden completa** (contenedor con `--net host`,
    `--pid host`, `--userns host`, `--cap-add audit_control` y montajes del
    equipo) y pide una confirmación explícita.
  - Imagen fijada por *digest*.
- **Dependencias.** La imagen `docker/docker-bench-security` (Apache-2.0).
- **Recursos.** Bajos.
- **Dificultad.** Media.
- **Riesgos de seguridad.** **Altos:** ejecuta un contenedor con acceso amplio al
  equipo. Por eso es opcional, guiado y nunca automático.
- **Local o remoto.** Ambos.

---

## Monitorización

### H-05 · Historial de recursos y alertas de estado ⭐

- **Qué hace.** Guarda en memoria, o en disco de forma opcional y con límite,
  la CPU, la memoria, la red y el estado `health` de cada contenedor. Avisa (con
  notificación de escritorio) si un contenedor pasa a `unhealthy`, entra en
  bucle de reinicios o lo mata el OOM (eventos `oom` y `die` de Docker).
- **Problema que resuelve.** Hoy las estadísticas son en directo y los eventos
  se ven si se mira el panel. No hay aviso de un servicio caído.
- **Integración.**
  - Reutiliza el flujo de eventos (`events.rs`) y `stats`.
  - Las reglas de alerta son sencillas y se configuran en ajustes.
- **Dependencias.** Ninguna.
- **Recursos.** Una conexión de eventos por servidor; estadísticas solo de los
  contenedores marcados.
- **Dificultad.** Media.
- **Riesgos de seguridad.** Bajos. Las notificaciones no deben incluir datos
  sensibles.
- **Local o remoto.** Ambos.

### H-06 · Uso de disco global y previsión

- **Qué hace.** Vista de `system df` completa, con imágenes, contenedores,
  volúmenes y caché de construcción. Muestra lo que se puede recuperar y una
  previsión de limpieza.
- **Problema que resuelve.** Discos llenos por caché de construcción y capas
  huérfanas. Hoy `system/df` solo se usa para el tamaño de los volúmenes.
- **Integración.** Panel de inicio. Las acciones de limpieza ya existentes se
  muestran con una vista previa.
- **Dependencias.** Ninguna.
- **Recursos.** `system/df` es costoso en servidores grandes, así que solo se
  ejecuta cuando el usuario lo pide.
- **Dificultad.** Baja.
- **Riesgos de seguridad.** Ninguno.
- **Local o remoto.** Ambos.

---

## Copias de seguridad

### H-07 · Copia y restauración de volúmenes ⭐

- **Qué hace.** Exporta un volumen a un `.tar.gz` local o del servidor y lo
  restaura en un volumen nuevo o vacío. Puede detener temporalmente los
  contenedores que lo usan.
- **Problema que resuelve.** Hoy no hay forma de hacer copia de los datos de una
  base de datos en un volumen sin recurrir a órdenes manuales.
- **Integración.**
  - Contenedor auxiliar efímero con una imagen mínima fijada por *digest*
    (por ejemplo `busybox`), con el volumen en solo lectura y un destino montado.
  - Todo queda en el panel de actividad con la orden `docker run` equivalente.
  - La restauración nunca sobrescribe un volumen con datos sin confirmación.
- **Dependencias.** Ninguna externa. restic (BSD-2-Clause) puede ser un destino
  opcional futuro.
- **Recursos.** Disco y E/S proporcionales al volumen.
- **Dificultad.** Media.
- **Riesgos de seguridad.**
  - El archivo contiene datos sensibles: se escribe con permisos 0600.
  - Se usa una ruta de destino validada.
  - Se avisa si el destino está en una carpeta compartida.
- **Local o remoto.** Ambos. En remoto, la copia queda en el servidor, o se
  transfiere por el canal SSH si se pide.

### H-08 · Exportar e importar la configuración de la aplicación (sin secretos)

- **Qué hace.** Genera un archivo con los perfiles de servidores, los proyectos
  Compose y las preferencias, sin contraseñas ni frases de paso, y lo importa en
  otro equipo con la verificación de huellas.
- **Problema que resuelve.** Migrar a otro equipo o recuperarse de una
  reinstalación.
- **Integración.** Ajustes. Reutiliza `dcc-store`.
- **Dependencias.** Ninguna.
- **Recursos.** Mínimos.
- **Dificultad.** Baja.
- **Riesgos de seguridad.**
  - Una importación con huellas manipuladas sería un riesgo. Por eso las
    huellas importadas se muestran y se confirman, y no se añaden en silencio
    al `known_hosts` propio.
- **Local o remoto.** Local.

---

## Administración

### H-09 · Actualizar la imagen de un contenedor con vista previa y vuelta atrás ⭐

- **Qué hace.**
  - Comprueba si la etiqueta de un contenedor apunta a un *digest* nuevo en el
    registro (consulta del manifiesto sin descargar).
  - Descarga la imagen y recrea el contenedor con la misma configuración.
  - Conserva el contenedor anterior renombrado hasta que el usuario confirme.
- **Problema que resuelve.** Actualizar servicios sin perder configuración. Se
  apoya en la recreación segura que ya existe (D-070, edición con copia).
- **Integración.** Botón en el detalle del contenedor y vista «Actualizaciones
  disponibles». No hay actualización automática.
- **Dependencias.** Ninguna.
- **Recursos.** La descarga de la imagen.
- **Dificultad.** Media.
- **Riesgos de seguridad.**
  - Una imagen nueva puede estar comprometida. Se muestra el *digest* y se
    sugiere H-02 antes de recrear.
  - En remoto, las credenciales siguen R-07.
- **Local o remoto.** Ambos.

### H-10 · Modo de producción por servidor ⭐

- **Qué hace.** Marca un servidor como «producción». Las operaciones
  destructivas (eliminar, detener, `down`, limpiar o recrear) piden escribir el
  nombre del servidor, y la interfaz muestra un color y una etiqueta fijos.
- **Problema que resuelve.** Ejecutar una operación en el servidor equivocado
  (escenario 6 del modelo de amenazas).
- **Integración.** Campo en `servers.toml` y comprobación en `Dialog` /
  `DialogTarget`, que ya muestra el servidor. Una comprobación en Rust impide
  que una orden destructiva llegue sin el campo de confirmación.
- **Dependencias.** Ninguna.
- **Recursos.** Ninguno.
- **Dificultad.** Baja.
- **Riesgos de seguridad.** Reduce el riesgo.
- **Local o remoto.** Ambos (también el equipo local).

---

## Desarrollo

### H-11 · Generar un `compose.yaml` a partir de contenedores existentes

- **Qué hace.** Convierte la configuración de uno o varios contenedores en un
  `compose.yaml` editable, ocultando los secretos y con una referencia a un
  `.env`.
- **Problema que resuelve.** Pasar contenedores creados a mano a un proyecto
  reproducible.
- **Integración.** Reutiliza `EditableConfig` y el editor de Compose.
- **Dependencias.** Ninguna.
- **Recursos.** Mínimos.
- **Dificultad.** Media.
- **Riesgos de seguridad.** Los valores secretos se sustituyen por `${VAR}`. El
  `.env` se crea con permisos 0600 solo si se pide.
- **Local o remoto.** Ambos.

### H-12 · Comparar dos imágenes o dos versiones de una imagen

- **Qué hace.** Compara capas, variables, puertos, usuario, punto de entrada y
  tamaño entre dos imágenes.
- **Problema que resuelve.** Entender qué cambia al actualizar (complemento de
  H-09).
- **Integración.** Reutiliza la vista de capas e `inspect`.
- **Dependencias.** Ninguna.
- **Recursos.** Bajos.
- **Dificultad.** Baja.
- **Riesgos de seguridad.** Ninguno.
- **Local o remoto.** Ambos.

---

## Diagnóstico

### H-13 · Informe de diagnóstico con datos ocultos ⭐

- **Qué hace.** Genera un archivo con la versión de la aplicación y de Docker,
  el modo (rootless o normal), los contextos, el estado de los servidores (sin
  host ni usuario si se elige «anónimo»), los últimos errores del panel y la
  salida de `dcc-diagnose`.
- **Problema que resuelve.** Pedir ayuda o informar de errores sin filtrar
  secretos ni datos personales.
- **Integración.** Ayuda → «Crear informe de diagnóstico», con vista previa del
  contenido antes de guardar.
- **Dependencias.** Ninguna.
- **Recursos.** Mínimos.
- **Dificultad.** Baja.
- **Riesgos de seguridad.** Riesgo de filtrar datos. Mitigación: R-02, sustituir
  las rutas personales por `~` y la vista previa obligatoria.
- **Local o remoto.** Ambos.

### H-14 · Prueba de conectividad entre contenedores

- **Qué hace.** Comprueba desde la red de un contenedor si otro servicio
  responde (DNS y puerto TCP), con un contenedor auxiliar efímero en la misma
  red.
- **Problema que resuelve.** El típico «el servicio A no ve a B» en Compose.
- **Integración.** Detalle de red y de contenedor. Imagen auxiliar mínima
  fijada por *digest*. Queda en el panel de actividad.
- **Dependencias.** Ninguna externa (`busybox` o similar).
- **Recursos.** Un contenedor efímero.
- **Dificultad.** Baja.
- **Riesgos de seguridad.** Bajos: sin privilegios ni montajes, y se elimina al
  terminar.
- **Local o remoto.** Ambos.

---

## Las 10 más interesantes (⭐ y orden de valor)

1. **H-01** Auditoría de configuración de contenedores en ejecución.
2. **H-10** Modo de producción por servidor.
3. **H-07** Copia y restauración de volúmenes.
4. **H-05** Historial de recursos y alertas de estado.
5. **H-02** Análisis de vulnerabilidades de imágenes (Trivy o Grype, opcional).
6. **H-09** Actualizar la imagen de un contenedor con vuelta atrás.
7. **H-13** Informe de diagnóstico con datos ocultos.
8. **H-03** Análisis de Dockerfile antes de construir.
9. **H-11** Generar `compose.yaml` a partir de contenedores.
10. **H-06** Uso de disco global y previsión.

## Prioridades y orden de implementación

| Orden | Herramienta | Prioridad | Dificultad | Impacto | Riesgo de regresión | Depende de |
|---|---|---|---|---|---|---|
| 1 | H-10 Modo de producción | P1 | Baja | Alto | Bajo | — |
| 2 | H-01 Auditoría de contenedores | P2 | Media | Alto | Bajo | R-01, R-02, R-04 |
| 3 | H-13 Informe de diagnóstico | P2 | Baja | Medio | Nulo | R-02 |
| 4 | H-07 Copias de volúmenes | P2 | Media | Alto | Bajo | — |
| 5 | H-05 Alertas de estado | P3 | Media | Alto | Bajo | — |
| 6 | H-03 Análisis de Dockerfile | P3 | Baja | Medio | Nulo | — |
| 7 | H-09 Actualizar imagen | P3 | Media | Alto | Medio | R-07 |
| 8 | H-02 Vulnerabilidades de imágenes | P3 | Media | Alto | Bajo | — |
| 9 | H-06 Uso de disco | P4 | Baja | Medio | Nulo | — |
| 10 | H-11 Compose desde contenedores | P4 | Media | Medio | Nulo | R-02 |
| 11 | H-08 Exportar configuración | P4 | Baja | Medio | Bajo | — |
| 12 | H-12 Comparar imágenes | P4 | Baja | Bajo | Nulo | — |
| 13 | H-14 Conectividad | P4 | Baja | Medio | Nulo | — |
| 14 | H-04 Docker Bench | P4 | Media | Medio | Bajo | — |

Cada herramienta es una actualización independiente. Lleva sus propias pruebas,
la documentación en USER_GUIDE.md, traducciones a los ocho idiomas y una
entrada en DECISIONS.md cuando se integre algo externo, indicando su licencia y
su versión mínima.
