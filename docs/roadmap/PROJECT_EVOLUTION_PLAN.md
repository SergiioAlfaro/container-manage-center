# Plan de evolución de Docker Control Center

| | |
|---|---|
| Versión de partida | 1.0.0 (commit `d1068d4`) |
| Fecha | 12 de noviembre de 2026 |
| Estado de este documento | Etapa 0 completada; Etapa 1 preparada, **pendiente de autorización** |
| Documentos relacionados | [STATUS.md](../../STATUS.md) (fases 1–10) · [DECISIONS.md](../../DECISIONS.md) · [docs/security/](../security/) |

Este plan continúa el proyecto existente. No es una reescritura ni sustituye a
las fases 1–10, cuyo historial sigue en STATUS.md. Las nuevas entregas se llaman
**etapas de evolución (E0–E6)** para no confundirlas con las fases originales.

Hay dos documentos que este plan no duplica:

- **[FEATURE_ROADMAP.md](../security/FEATURE_ROADMAP.md)** propuso herramientas
  sueltas (H-01…H-14). Este plan adopta tres de ellas como herramientas oficiales
  y deja el resto como candidatas (§5.4).
- **[REMEDIATION_PLAN.md](../security/REMEDIATION_PLAN.md)** sigue siendo la
  referencia técnica de cada corrección (R-01…R-19). Aquí se agrupan en etapas.

---

## 1. Estado actual del proyecto

### 1.1 Fases

| Fase o actualización | Estado real comprobado |
|---|---|
| 1–9 | Validadas por el propietario en Debian 13 (STATUS.md, del 04-11 al 08-11-2026) |
| 0.9.1, 0.9.2, 0.10.0 | Validadas por el propietario (11-11 y 12-11-2026) |
| 10. Pruebas finales y distribución (1.0.0) | **Implementada y verificada automáticamente** (64 comprobaciones correctas con Docker 24, 26 y 29, el `.deb` y el `.deb` compatible). **Pendiente de la validación manual del propietario (F1–F8).** |
| Auditoría de seguridad | Hecha el 12-11-2026 (docs/security). No ha habido cambios de código desde entonces: el árbol coincide con `d1068d4` y solo se ha añadido `docs/` |

**Consecuencia:** la evolución parte de una 1.0.0 cuya validación manual está
pendiente. No hace falta esperarla para empezar la Etapa 1, porque las
correcciones son independientes de F1–F8. Si F1–F8 revelan algún error, se
corregirá como 1.0.x antes de publicar la 1.1.0.

### 1.2 Arquitectura (resumen)

```
ui/ (React 19 + TS, Vite, xterm.js, i18next ×8, TanStack Query, zustand)
   │ IPC Tauri (99 órdenes, cada orden de Docker con servidor explícito)
src-tauri/ (estado, ServerManager por servidor, secretos, flujos, actualizaciones, desinstalación)
   ├── dcc-engine   Docker (bollard 0.21 + CLI sin shell), Compose, SSH, terminales, edición, construcción
   │      └── dcc-activity   panel de actividad en memoria + historial (sin dependencias internas)
   └── dcc-store    config.toml, servers.toml, diario JSONL 0600 (sin dependencias internas)
```

Grafo sin ciclos: `dcc-activity` y `dcc-store` son hojas, `dcc-engine` depende de
`dcc-activity`, y la aplicación depende de los tres. Las nuevas piezas deben
respetar este orden (§4).

### 1.3 Servidores remotos (0.10.0)

Se ha comprobado lo siguiente en el código:

- Un `ServerManager` por servidor, con su propio `Engine`, registro de actividad
  (`with_server`) y bloqueo.
- Transporte por OpenSSH con ControlMaster y `docker system dial-stdio`.
- `known_hosts` propio, askpass por socket efímero y secretos en Secret Service.
- Historial y eventos filtrados por servidor (D-079). Cada orden IPC exige un
  servidor explícito (prueba `docker_commands_are_bound_to_an_explicit_server`).

Esta separación es la base que deben usar las tres herramientas nuevas para no
mezclar los datos de distintos servidores.

---

## 2. Funcionalidades existentes que se reutilizarán

| Necesidad de las etapas | Ya existe | Dónde | Cómo se reutiliza |
|---|---|---|---|
| Reglas de riesgo | Avisos de `privileged`, de montajes sensibles (lista exacta) y del puerto privilegiado; clasificación de riesgo de los directorios vinculados | `crates/dcc-engine/src/spec.rs`, `binds.rs` | Se unifican en `dcc-engine::risk` (E1) y las usan la creación, la edición, Compose y la auditoría (E3) |
| Ocultación de secretos | `is_secret_key`, `REDACTED`, `redacted_body`, ocultación de `buildargs` en el historial | `spec.rs`, `edit.rs`, `build/mod.rs`, `dcc-activity/history.rs`, `specModel.ts` | Se centraliza en `dcc-activity::redact` (E1), el nivel más bajo del grafo |
| Punto único de registro | `ActivityLog::begin` / `succeed` / `fail` / `cancel` (cuerpo, orden equivalente, stdout, stderr, error, resumen) | `crates/dcc-activity/src/log.rs`, `model.rs` | La ocultación se aplica ahí, antes de guardar o emitir (E1) |
| Historiales persistentes | `Journal<T>` JSONL 0600, compactación, líneas dañadas descartadas | `crates/dcc-store/src/journal.rs` | Historial de auditorías (E3), de copias (E4) y de métricas reducidas (E5) |
| Inspección de contenedores | `inspect`, `EditableConfig` (configuración completa editable) | `details.rs`, `edit.rs`, `edit_ops.rs` | Entrada de la auditoría (E3) y metadatos de las copias (E4) |
| Recreación segura con copia | Edición con copia y restauración confirmada (D-070) | `edit_ops.rs` | Acciones correctivas de la auditoría con vista previa (E3) |
| Estadísticas | Flujo `stats_open` por contenedor, cálculo de CPU y memoria igual que el CLI (cgroup v1 y v2) | `streams.rs`, `commands/containers.rs` | Muestreo de la monitorización (E5) |
| Eventos | `events_watch`, `events_query`, diario de eventos (D-057, D-058) | `events.rs` | Reinicios, OOM y paradas para las alertas (E5) |
| Tamaño de volúmenes | `GET /system/df?type=volume` | `volumes.rs` | Estimación previa de las copias (E4) |
| Contenedores auxiliares | Ejecución con progreso, `exec` con TTY | `streams.rs`, `containers.rs` | Copia y restauración de volúmenes con un contenedor efímero (E4) |
| Archivos Compose | Lectura y guardado atómico (local y remoto), `docker compose config` | `compose/file.rs`, `remote_file.rs`, `project.rs` | Análisis de riesgos (E1/E2), copia de proyectos (E4) |
| Diálogo con servidor de destino | `Dialog` + `DialogTarget` | `ui/src/shared/ui/feedback.tsx` | Confirmaciones de las tres herramientas |
| Panel de inicio | `DashboardView` | `ui/src/features/dashboard` | Lo amplía la monitorización (E5), que no lo sustituye |

**No existe y habrá que crearlo:**

- notificaciones de escritorio (no hay ningún plugin de notificaciones);
- planificador de tareas;
- gráficos (no hay ninguna biblioteca de gráficos);
- cifrado de archivos;
- exportación de informes.

---

## 3. Problemas detectados

### 3.1 Hallazgos de seguridad pendientes

Todos los hallazgos siguen **pendientes**. Se ha comprobado que no hay cambios de
código desde la auditoría (`git status`: solo `docs/` sin seguimiento) y que las
líneas citadas en el informe siguen siendo las mismas.

| ID | Sev. | Estado | Etapa |
|---|---|---|---|
| DCC-SEC-01 Montajes peligrosos (socket en solo lectura, subrutas, rutas sin normalizar) | Alta | Pendiente | **E1** |
| DCC-SEC-02 ProxyJump: el salto no hereda las opciones y recibe el askpass | Media | Pendiente | **E1** |
| DCC-SEC-03 Capacidades peligrosas y red `host` | Media | Pendiente | **E1** |
| DCC-SEC-05 Ocultación de secretos incompleta | Media | Pendiente | **E1** |
| DCC-SEC-04 Compose sin análisis de riesgos | Media | Pendiente | E1 (montajes y capacidades, porque lo exigen SEC-01 y SEC-03) + E2 (resto de opciones) |
| DCC-SEC-06 IPC de archivos Compose sin límite de rutas | Media | Pendiente | E2 |
| DCC-SEC-07 Credenciales locales enviadas a daemons remotos | Media | Pendiente | E2 |
| DCC-SEC-08 Puertos en todas las interfaces | Baja | Pendiente | E2 |
| DCC-SEC-09 Paquetes sin firma ni sumas | Baja | Pendiente | E2 |
| DCC-SEC-10 vitest/tinypool (solo desarrollo) | Baja | Pendiente | E2 |
| DCC-SEC-11 `DCC_ALLOW_ROOT` en el binario de publicación | Baja | Pendiente | E2 |
| DCC-SEC-12 Enlaces OSC 8 en las terminales | Baja | Pendiente | E2 |
| DCC-SEC-13 El guardado de Compose sustituye los enlaces simbólicos | Baja | Pendiente | E2 |
| DCC-SEC-14 a 18 (informativas) | Inf. | Pendientes | E2 (17, 16 y 18) y seguimiento (14 y 15) |

### 3.2 Otros problemas encontrados en la Etapa 0

| Problema | Impacto | Tratamiento |
|---|---|---|
| ESLint no está configurado (no hay configuración ni dependencia) | No hay análisis estático del TypeScript, salvo `tsc` | Propuesta para E2: añadir ESLint con reglas mínimas (`react-hooks`, `no-unsafe-*`). Es una dependencia de desarrollo y requiere autorización |
| El disco del entorno de desarrollo se llenó al 98 % con cachés de compilación | Las compilaciones fallan | Se liberaron cachés recompilables. No afecta al proyecto |
| La primera ejecución de la línea base no arrancó Docker 29 (containerd esperando el bloqueo de su base) | Fallo del entorno, no del proyecto | Se paró todo, se arrancó de nuevo y se repitió la ejecución (§8) |

---

## 4. Dependencias técnicas

### 4.1 Arquitectura compartida de las tres herramientas

Se añaden módulos, no se reorganizan los existentes:

| Componente compartido | Ubicación | Lo usan | Dependencias |
|---|---|---|---|
| `redact`: detección y ocultación de secretos (nombres y valores) | `dcc-activity::redact` | Toda la aplicación | Ninguna (hoja) |
| `risk`: reglas de riesgo de montajes, capacidades, red, puertos, seguridad y recursos, con severidad y código | `dcc-engine::risk` | Creación, edición, Compose, auditoría | `redact` |
| Identificación de servidores y conexiones | `ServerManager`, `ServerTag` (existentes) | Las tres herramientas | — |
| Registro de operaciones | `ActivityLog` (existente) | Las tres | — |
| Historiales | `dcc-store::journal` (existente) + nuevos tipos por herramienta, en archivos **separados por servidor** | Auditoría, copias, monitorización | — |
| Informes | `dcc-engine::report` (JSON, Markdown y HTML sin scripts; los secretos pasan por `redact`) | Auditoría y copias | `risk`, `redact` |
| Notificaciones | `src-tauri::notify` (complemento oficial `tauri-plugin-notification`, MIT/Apache-2.0, a confirmar) | Monitorización y copias programadas | — |
| Tareas programadas | `src-tauri::scheduler` (§5.2, E4) | Copias y, más adelante, auditorías | `dcc-store` |
| Permisos y confirmaciones | Confirmación con los códigos de riesgo aceptados, comprobada en Rust (§6.1) | Las tres | `risk` |

Reglas:

- Los datos de cada servidor van en rutas o claves que incluyen el `id` del
  servidor.
- Ninguna herramienta lee estado de otra salvo a través de identificadores
  (id de servidor y de contenedor).
- La auditoría usa `risk` y nunca la monitorización.
- La copia usa `EditableConfig` y nunca la auditoría.
- Así no hay ciclos.

### 4.2 Dependencias externas que se evaluarán (sin instalarlas todavía)

| Dependencia | Etapa | Licencia | Estado de la evaluación |
|---|---|---|---|
| Trivy / Grype (binarios opcionales del usuario) | E3 | Apache-2.0 | No se ha podido confirmar que estén en los repositorios oficiales de Debian 13 ni de Ubuntu; hay que comprobarlo en E3. Normalmente se instalan desde el proyecto (repositorio APT de Aqua o binarios). Ambos admiten una base local (`trivy --skip-db-update` con caché, `grype db import`). Trivy tuvo una versión maliciosa (v0.69.4, CVE-2026-33634). **Se detectan, nunca se instalan.** |
| `tauri-plugin-notification` | E5 | MIT/Apache-2.0 (a confirmar con `cargo deny`) | Comprobar en WebKitGTK/D-Bus de GNOME y KDE |
| Biblioteca de gráficos (p. ej. uPlot, MIT) o SVG propio | E5 | — | Decidir en E5 según el tamaño, la accesibilidad y el modo oscuro. SVG propio si basta con líneas y áreas |
| Cifrado de copias (crate `age`, MIT/Apache-2.0) | E4 | — | Evaluar frente a un archivo sin cifrar en una carpeta 0700 |

---

## 5. Etapas de desarrollo

Cada etapa:

- es una actualización independiente, con su propia versión;
- no adelanta funcionalidades de etapas posteriores;
- se entrega con el código, el `.deb`, el `.tar.gz` de fuentes, los informes,
  las limitaciones y las instrucciones de validación manual;
- no sobrescribe entregas anteriores.

| Etapa | Versión prevista | Alcance |
|---|---|---|
| E0 Revisión y preparación | — (solo documentos) | Este plan, el inventario y la línea base |
| E1 Correcciones de seguridad P1 | **1.1.0** | DCC-SEC-01, 02, 03 y 05 (+ parte de la 04) |
| E2 Endurecimiento restante | 1.2.0 | DCC-SEC-04 (resto), 06–13 y 16–18; dependencias; ESLint |
| E3 Auditoría de seguridad | 1.3.0 | Nueva sección «Auditoría de seguridad» |
| E4 Copias de seguridad y restauración | 1.4.0 | Nueva sección «Copias de seguridad» |
| E5 Monitorización avanzada | 1.5.0 | Nueva sección «Monitorización» |
| E6 Integración y validación final | 1.6.0 | Integración entre herramientas, regresión completa, documentación |

Se usa una versión menor por etapa porque cada una cambia el comportamiento
(avisos nuevos, confirmaciones, secciones). Los errores que aparezcan entre
etapas se publican como parches.

### 5.1 E1 · Correcciones de seguridad prioritarias (detalle en §9)

### 5.2 E2 · Endurecimiento restante

| Tarea | Hallazgo | Referencia |
|---|---|---|
| Compose: `pid`, `ipc`, `userns_mode`, `security_opt`, `devices`, `cgroup_parent` | 04 | R-05 |
| Rutas admitidas en el IPC de archivos Compose + rechazo de `~/.ssh`, `~/.gnupg`, `~/.docker` | 06 | R-06 |
| Guardado de enlaces simbólicos: escribir en el destino real o rechazar | 13 | R-13 |
| Aviso de puertos en todas las interfaces | 08 | R-10 |
| Confirmación antes de enviar credenciales de registro a un servidor remoto | 07 | R-07 |
| `cargo deny` + `npm audit --omit=dev` en la verificación, con `deny.toml` versionado | 14 | R-08 |
| vitest 5 (rama aparte) | 10 | R-14 |
| `SHA256SUMS` firmado y verificado por `install.sh` | 09 | R-09 |
| `DCC_ALLOW_ROOT` solo en compilaciones de prueba | 11 | R-11 |
| `linkHandler` propio en xterm | 12 | R-12 |
| Permisos mínimos de Tauri, origen de la página de la versión, `sh -c` en remoto | 17, 16, 18 | R-15, R-16, R-18 |
| ESLint (si se autoriza) | — | §3.2 |
| SECURITY_CHECKLIST.md con evidencias reales | — | — |

### 5.3 E3, E4 y E5 · Las tres herramientas

**E3 · Auditoría de seguridad** (sección nueva en el grupo «Herramientas»)

- **Alcance y entradas.**
  - Analiza todos los contenedores, en ejecución y detenidos, o uno solo, a
    partir de su `inspect`.
  - Analiza también los proyectos Compose (`compose config --format json`) y la
    configuración de las imágenes (usuario, puertos, antigüedad).
  - Opcionalmente, vulnerabilidades con Trivy o Grype si están instalados.
- **Comprobaciones.** Todas las que pide la especificación:
  - root, `privileged`, capacidades, montajes, socket;
  - red, PID e IPC del equipo, puertos;
  - sistema de archivos modificable, recursos sin límite;
  - `seccomp`, AppArmor, `no-new-privileges`;
  - imagen antigua, CVE.

  Cada comprobación lleva un código, una severidad, una explicación y una
  recomendación.
- **Vulnerabilidades.** Si la herramienta no está, o su base no está
  actualizada, se dice explícitamente («no analizado» / «base del día X»).
  **Nunca** se muestra «sin vulnerabilidades» si el análisis no terminó.
- **Historial y exportación.** Historial de auditorías por servidor (JSONL
  0600) y exportación a JSON, Markdown y HTML, siempre pasando por `redact`.
- **Modo.** Solo lectura. Las correcciones abren «Editar configuración» con los
  cambios propuestos y una vista previa (recreación segura D-070). Nada se
  aplica sin confirmación.

**E4 · Copias de seguridad y restauración** (sección nueva)

- **Qué se copia.**
  - Volúmenes, con un contenedor auxiliar efímero de imagen fijada por
    *digest*, el volumen en solo lectura y un `tar` con hash SHA-256.
  - La configuración de los contenedores (`EditableConfig` sin secretos).
  - Los proyectos Compose (archivos, más `.env` solo con aceptación expresa).
  - La configuración de la aplicación, sin secretos.
- **Tipos de copia.** Manuales, completas o selectivas.
- **Consistencia** (se investigará y documentará en E4):
  - MySQL y MariaDB: `mariadb-dump`/`mysqldump --single-transaction` ejecutado
    **dentro** del contenedor, con las credenciales que el contenedor ya tiene,
    sin sacarlas de él;
  - PostgreSQL: `pg_dump`/`pg_dumpall`;
  - Redis: `BGSAVE` y copia de `dump.rdb`;
  - MongoDB: `mongodump`;
  - resto: pausa o parada controlada con confirmación, o copia «en caliente»
    marcada como **no garantizada**.
- **Destino.** Carpetas locales elegidas por el usuario. La arquitectura queda
  preparada para destinos remotos (trait `BackupTarget`).
- **Seguridad de los archivos.** Directorio 0700 y archivos 0600, manifiesto con
  hashes, verificación de integridad, cifrado opcional (§4.2) y ninguna
  credencial en texto plano.
- **Retención.** Política configurable, 7 diarias, 4 semanales y 3 mensuales
  por defecto, y nunca se borra fuera de ella.
- **Restauración.** Completa o selectiva, con:
  - lista de lo que se recupera y de lo que se sobrescribe;
  - aviso de pérdida de datos;
  - confirmación;
  - registro en la actividad.

  Se ofrece restauración de prueba en un volumen nuevo.
- **Copias programadas.** Decisión pendiente (§10, pregunta 2):
  - **(a)** planificador interno, que solo funciona con la aplicación abierta,
    guarda la planificación en `config.toml` y al arrancar ejecuta o avisa de
    las copias perdidas;
  - **(b)** temporizador de usuario de systemd, que funciona sin la aplicación
    pero crea archivos en `~/.config/systemd/user`, y por tanto exige
    autorización expresa.

  En cualquier caso se documenta cómo se guarda, se cancela y se recupera.

**E5 · Monitorización** (sección nueva; el panel de inicio se mantiene)

- **Métricas por contenedor.**
  - CPU, memoria y su límite, red de entrada y salida, PIDs, estado, tiempo de
    actividad y reinicios: todo sale de `stats` e `inspect`.
  - E/S de disco solo donde Docker la da (`blkio`, limitada con cgroup v2).
- **Métricas por servidor.**
  - Estado del motor, recuentos, conexión.
  - «Equipo» solo con fuentes fiables (`info` de Docker: número de CPU y
    memoria total), sin presentarlo como monitorización del sistema operativo.
- **Muestreo e históricos.**
  - Muestreo configurable, entre 2 y 60 s.
  - Históricos en anillo en memoria por servidor, con reducción a resolución de
    1 min para el disco, limitados en tamaño y tiempo.
  - Se puede pausar. Las desconexiones aparecen como huecos, sin inventar datos.
- **Alertas.** Umbrales configurables, histéresis y periodo de silencio para
  no repetirlas, y notificaciones de escritorio.
- **Agentes.** No se instala ningún agente en los servidores.

### 5.4 Herramientas candidatas que no se adoptan por ahora

H-10 (modo de producción por servidor), H-09, H-11, H-12, H-13 y H-14 de
FEATURE_ROADMAP.md siguen como candidatas. **H-10 está muy relacionada con E1**
(evitar operaciones en el servidor equivocado), pero no forma parte de la hoja
de ruta oficial. Se propone tratarla en E6 si el propietario lo aprueba.

---

## 6. Criterios de aceptación

### 6.1 Comunes a todas las etapas

- La regresión completa (`verify-phase-10.sh` o su sucesor) no tiene fallos con
  Docker 24, 26 y 29 y con el laboratorio SSH.
- `cargo fmt --check`, `cargo clippy -D warnings`, `tsc --noEmit`, Vitest y el
  `.deb` se superan, y el `.deb` se instala y arranca en el entorno disponible.
- Cada aviso o texto nuevo está en los 8 idiomas: la prueba de claves
  compartidas de i18n falla si falta alguno.
- No hay secretos en el panel, el historial, los registros, los informes ni los
  archivos de prueba. Hay pruebas que lo comprueban.
- **Confirmación de operaciones peligrosas comprobada en Rust:**
  - la orden IPC (`container_create`, `container_edit_apply`, `compose_run_open`
    con `up`) recibe la lista de códigos de riesgo que el usuario aceptó;
  - el motor recalcula los riesgos y se niega si hay alguno no aceptado;
  - así, la interfaz no puede saltarse el aviso.
- Ningún cambio de comportamiento sin una entrada en DECISIONS.md y en el
  CHANGELOG.
- Cumple las 15 restricciones generales de la especificación (sin root, sin TCP
  sin TLS, sin desactivar la verificación de claves SSH, sin texto plano, sin
  shell concatenada, sin tocar servidores remotos ni el sistema sin
  autorización, sin dependencias del sistema instaladas automáticamente, sin
  telemetría…).

### 6.2 Por etapa

| Etapa | Criterios de aceptación propios |
|---|---|
| E1 | Las tablas de casos de DCC-SEC-01, 03 y 05 están cubiertas por pruebas en Rust y TS. Hay 0 falsos positivos en un conjunto de configuraciones legítimas (§9.5). ProxyJump se prueba en el laboratorio sin enviar la contraseña al salto. La creación, la edición y el despliegue de Compose con riesgos sin aceptar se rechazan por IPC |
| E2 | Las verificaciones de R-05…R-19 tienen sus pruebas. `npm audit --omit=dev` y `cargo deny` dan 0 errores. SECURITY_CHECKLIST.md tiene evidencias |
| E3 | Cada comprobación se ejecuta sobre contenedores de prueba peligrosos y legítimos. Nunca se modifica nada sin confirmación (prueba IPC). Sin herramienta de CVE, el informe dice «no analizado». La exportación no contiene secretos |
| E4 | Copia correcta, interrumpida (sin archivo parcial dado por bueno), integridad (un hash alterado se detecta), restauración completa y selectiva, datos de prueba de MariaDB/PostgreSQL consistentes, permisos 0600 y retención exacta. La planificación sobrevive a un reinicio de la aplicación |
| E5 | Métricas iguales a `docker stats` (tolerancia documentada). Desconexión y reconexión sin bloquear la interfaz. Alertas con histéresis sin repetición. Consumo de la aplicación medido con 50 contenedores y 3 servidores, dentro del límite fijado en E5 |
| E6 | La auditoría identifica los contenedores que monitoriza la monitorización y que respaldan las copias (mismo id). Regresión completa y validación manual guiada |

---

## 7. Riesgos

| Riesgo | Prob. | Impacto | Mitigación |
|---|---|---|---|
| Demasiados avisos (fatiga) en configuraciones legítimas (Traefik, Portainer, monitorización) | Media | Medio | Conjunto de pruebas de casos legítimos; avisos agrupados con explicación; las confirmaciones se recuerdan por contenedor o proyecto |
| La confirmación comprobada en Rust rompe los flujos que hoy crean contenedores sin pasar por el asistente (duplicar, restaurar copia de edición) | Media | Medio | Esos flujos se revisan en E1. Las copias y duplicados heredan la aceptación del original solo si la configuración es idéntica |
| ProxyJump: el cambio rompe perfiles que hoy funcionan gracias a `~/.ssh/config` | Media | Alto | Leer `ssh -G <salto>`; pruebas en el laboratorio; mensaje de error que explica qué falta |
| Ocultar de más dificulta el diagnóstico | Media | Bajo | Se conserva el nombre de la variable, el esquema, el host y la longitud; solo se oculta el valor secreto |
| Copias inconsistentes de bases de datos | Alta si se hace mal | Alto | Volcado específico por motor; la copia en caliente se marca como no garantizada |
| Restaurar sobre datos reales | Baja | Crítico | Vista previa, confirmación escribiendo el nombre y restauración en un volumen nuevo por defecto |
| Coste de la monitorización en servidores remotos (flujos `stats` por SSH) | Media | Medio | Muestreo limitado, solo los contenedores visibles o marcados, pausa y un único flujo por contenedor compartido entre vistas |
| Herramientas externas de CVE comprometidas o desactualizadas | Baja | Alto | Detectar, no instalar; mostrar la versión y la fecha de la base; lista de versiones retiradas |
| Disco del entorno de desarrollo | Media | Bajo | Limpiar cachés entre etapas |
| La validación F1–F8 de la 1.0.0 revela errores | Baja | Medio | Parche 1.0.x antes de publicar la 1.1.0 |

---

## 8. Línea base (Etapa 0)

Ejecutada el 12-11-2026 (19:37–19:55) en el entorno de desarrollo: Ubuntu 24.04
en contenedor, WebKitGTK 2.52 y Xvfb. Se usó
`scripts/verify-phase-10.sh --deb --net --ssh-lab <lab.json>` con Docker
29.9.2 (API 1.56) y el laboratorio SSH de tres servidores (Docker 29, 26 y 24).
**No se modificó el repositorio:** el informe está en `.gitignore` y, al
terminar, `git status` muestra solo `docs/`.

| Comprobación | Resultado |
|---|---|
| **Veredicto** | **90 correctas, 0 fallos, 4 omitidas justificadamente, 1 aviso** («parte automática superada») |
| `cargo fmt --check` | Correcto |
| `cargo clippy --workspace --all-targets -D warnings` | Correcto (0 avisos) |
| `cargo test --workspace` (deterministas) | 562 superadas, 0 fallidas |
| Vitest | 41 archivos, 376 pruebas superadas |
| `tsc --noEmit` | Correcto |
| Compilación de Vite (producción) | Correcta |
| Pruebas reales con Docker 29 (contenedores, imágenes, redes, volúmenes, Compose, eventos, edición, terminales) | Correctas |
| Pruebas reales por SSH (tres servidores; contenedores, imágenes, redes, volúmenes, terminales, edición, eventos, huellas; sin procesos `ssh` huérfanos) | Correctas |
| Compilación de escritorio (debug) y paquete `.deb` 1.0.0 (contenido, `dpkg-shlibdeps`, libc6, `.desktop` válido, `install.sh --check`, shellcheck) | Correctos |
| Consulta HTTPS real (comprobación de versiones) | Correcta |
| Recursos de prueba restantes (`dcc.test`) | 0 contenedores, 0 volúmenes, 0 redes, 0 imágenes |
| ESLint | **No configurado** (no hay nada que ejecutar) |

**Omitidas, con su justificación:**

- desinstalación real con APT (necesita chroot y root);
- una prueba IPC con llavero real que necesita la variable `DCC_SSH_LAB`, no
  definida en esta ejecución;
- dos pruebas de permisos que no tienen sentido como root.

**Aviso:** el entorno de desarrollo se ejecuta como root con
`DCC_VERIFY_ALLOW_ROOT=1`.

**Fuera de esta línea base, ya hechos en la Fase 10:**

- el paquete compatible Debian 12+/Ubuntu 22.04+ (`--compat`);
- la prueba de instalación limpia en chroot;
- la regresión con Docker 24 y 26 como daemon local.

No se repitieron, para no llenar el disco. Se repetirán en la verificación de
la Etapa 1.

**Herramientas de dependencias** (auditoría del mismo día, árbol idéntico):
`cargo audit` 0 vulnerabilidades; `cargo deny` sin errores de licencias ni de
orígenes; `npm audit` 3 avisos, solo en desarrollo.

---

## 9. Plan de implementación de la Etapa 1 (1.1.0)

**No se ha empezado.** Requiere autorización.

### 9.1 Orden

1. **E1-1 · `dcc-activity::redact`** (DCC-SEC-05). No depende de nada.
2. **E1-2 · `dcc-engine::risk`** (DCC-SEC-01 y 03), que usa `redact` para los
   detalles.
3. **E1-3 · Integración de `risk`:** creación, edición y Compose (parte de la 04),
   más la confirmación comprobada en Rust.
4. **E1-4 · ProxyJump** (DCC-SEC-02). Es independiente y puede ir en paralelo con
   E1-2 y E1-3.
5. **E1-5 · Interfaz:** avisos agrupados, diálogo de confirmación y textos en 8
   idiomas.
6. **E1-6 · Verificación:** `scripts/verify-evolution-1.1.0.sh`, que reutiliza la
   regresión completa, documentación y entregables.

### 9.2 E1-1 · Ocultación de secretos centralizada (DCC-SEC-05)

- **Nuevo** `crates/dcc-activity/src/redact.rs`:
  - `is_secret_name(&str)`:
    - partes `PASSWORD`, `PASSWD`, `PASS`, `PWD`, `SECRET`, `TOKEN`, `KEY`,
      `CREDENTIAL`, `PRIVATE`, `AUTH`, `COOKIE`, `SESSION`, `DSN`;
    - excepción `*_FILE`;
    - lista de excepciones de falsos positivos con pruebas.
  - `redact_value(&str)`: URL con usuario y contraseña (solo se oculta la
    contraseña), bloques PEM `PRIVATE KEY`, `Authorization: Bearer|Basic`.
  - `redact_kv` para `NOMBRE=valor` y `redact_args` para órdenes:
    - `-e NOMBRE=valor`, `--env`, `--build-arg`;
    - `-p<clave>` de clientes de bases de datos;
    - `--password[=]`, `--token[=]`.
  - `redact_text(&str)`: aplica todo lo anterior a texto libre (stdout, stderr,
    errores).
- **`ActivityLog`** (`log.rs`) aplica `redact_*` a `request_body`,
  `equivalent_command`, `summary`, `stdout`, `stderr` y `error` **antes** de
  guardar y emitir. Así se cubren el panel, el historial y la interfaz desde un
  único punto. Las ocultaciones por nombre que ya existen
  (`spec.rs` `is_secret_key`, `edit.rs`, `build`) pasan a llamar a `redact`.
- **Registros de la aplicación** (`tracing`, 16 puntos): revisar que ninguno
  imprima valores. Se añade una prueba con un `subscriber` de captura.
- **Interfaz:** la lista de nombres se exporta con `ts-rs` y `specModel.ts` la
  usa. Una prueba de igualdad Rust↔TS evita que se desajusten.
- **Pruebas** (sin secretos reales, valores ficticios marcados):
  - tabla de nombres (`MYSQL_PWD`, `DB_PASS`, `PGPASS`, `REDIS_AUTH`,
    `SMTP_PASS`, `DATABASE_URL`, `GITHUB_TOKEN`, `AWS_SECRET_ACCESS_KEY`,
    `SSH_PRIVATE_KEY`, `API_KEY`…) y de valores;
  - **prueba de extremo a extremo:** se crea un contenedor de prueba con esas
    variables, se recorren el panel, el historial en disco, la orden
    equivalente y el error forzado, y se busca cada valor ficticio: debe haber 0
    apariciones;
  - legibilidad: el nombre de la variable, el host de la URL y el código de
    error siguen visibles.
- **Pendiente de decidir:** la salida de los **registros de los contenedores**
  y de las **terminales** son datos del usuario. Se propone **no** alterarlos
  (§10, pregunta 1).

### 9.3 E1-2 · Reglas de riesgo centralizadas (DCC-SEC-01 y 03)

- **Nuevo** `crates/dcc-engine/src/risk.rs`, con
  `Risk { code, severity (critical|high|medium|low), subject, params }`. Los
  textos se traducen en la interfaz.
- **Montajes.**
  - Normalización léxica.
  - Clases:
    - socket de *runtime* (`docker.sock`, `containerd.sock`, `podman.sock`,
      rootless), **crítico aunque sea `:ro`**;
    - raíz y sistema (`/`, `/etc/**`, `/boot`, `/proc`, `/sys`, `/dev`,
      `/var/lib/docker/**`), alto;
    - credenciales (`/etc/shadow`, `/etc/sudoers*`, `~/.ssh`, `~/.gnupg`,
      `~/.docker`, `/root`), alto aunque sea `:ro`;
    - binarios del sistema (`/usr`, `/bin`, `/lib*`), medio si es de lectura y
      escritura.
  - En local, `canonicalize` para seguir los enlaces, solo para avisar.
  - `binds.rs::risk_of` pasa a usar `risk`.
- **Capacidades.**
  - `ALL` y `SYS_ADMIN`: crítico.
  - `SYS_PTRACE`, `SYS_MODULE`, `SYS_RAWIO`, `DAC_READ_SEARCH`,
    `DAC_OVERRIDE`, `NET_ADMIN`, `BPF`, `PERFMON`, `MAC_ADMIN`, `SYS_BOOT`,
    `SYS_TIME`: alto.
  - `NET_RAW` añadida explícitamente: medio.
  - Se normalizan el prefijo `CAP_` y las mayúsculas.
  - `cap_drop: ALL` se reconoce como buena práctica (sin aviso).
  - Las capacidades del usuario nunca se modifican.
- **Otros.** `privileged` (crítico, ya existe como aviso) y red `host` (alto).
- **Pruebas.** Tabla de casos peligrosos y **conjunto de configuraciones
  legítimas sin avisos** (§9.5).

### 9.4 E1-3 · Integración y confirmación

- **`ContainerSpec::validate`** devuelve los riesgos a través de `risk`. Se
  mantienen los códigos existentes (`privileged`, `bind.sensitive`) para no
  romper la interfaz, y se añaden los nuevos.
- **Edición** (`edit.rs::validate`): solo se avisa de los riesgos que **añade**
  el cambio. Los que ya tenía el contenedor se muestran como informativos.
- **Compose:**
  - `compose_preview` obtiene `compose config --format json` (con YAML como
    alternativa para Compose v1) y aplica `risk` a `volumes`, `cap_add` y
    `privileged` por servicio, más `network_mode: host`.
  - El resto de opciones de la 04 se dejan para E2.
- **Confirmación en Rust.**
  - `container_create`, `container_edit_apply` y `compose_run_open(up)` reciben
    `accepted_risks: Vec<String>`.
  - El motor recalcula los riesgos y rechaza la orden con `risk.unaccepted` si
    falta alguno de severidad alta o crítica.
  - Las rutas que hoy crean contenedores sin el asistente (duplicar, restaurar
    la copia de edición) se revisan una a una.
- **Pruebas IPC.** Crear con el socket montado sin aceptar → error. Con la
  aceptación → se crea (en el daemon de pruebas, etiqueta `dcc.test`).

### 9.5 Conjunto de configuraciones legítimas (falsos positivos)

Estas configuraciones no deben producir avisos de severidad alta ni crítica:

- `nginx` con `./html:/usr/share/nginx/html:ro`;
- PostgreSQL con un volumen con nombre;
- aplicación con `/home/<u>/proyecto:/app`;
- `cap_drop: [ALL]` + `cap_add: [NET_BIND_SERVICE]`;
- `tmpfs`;
- puertos en `127.0.0.1`;
- Compose con redes propias.

Traefik y Portainer con el socket **deben** avisar: es lo correcto.

### 9.6 E1-4 · ProxyJump (DCC-SEC-02)

- **Argumentos.**
  - Sustituir `-J` por `-o ProxyCommand=…`, con un `ssh` que repite las
    opciones de seguridad (`StrictHostKeyChecking=yes`, `known_hosts` propio,
    `UpdateHostKeys=no`, `ForwardAgent=no`, `ClearAllForwardings=yes`,
    `PermitLocalCommand=no`, `BatchMode=yes`), con `-W [%h]:%p -- <salto>`.
    Cada elemento va entrecomillado con `shell_quote`, porque OpenSSH ejecuta el
    `ProxyCommand` con `sh -c`.
- **Entorno del proceso del salto.** Se le quitan `SSH_ASKPASS`,
  `SSH_ASKPASS_REQUIRE` y `DCC_ASKPASS_SOCKET` (`env -u …`). El salto **no
  recibe nunca** el secreto del destino.
- **Autenticación del salto.**
  - Se hace con agente o clave y `BatchMode=yes`.
  - Si el salto exige contraseña, se muestra un error claro
    (`ssh.jumpNeedsKey`).
  - Se documenta en USER_GUIDE.
  - Contraseña también para el salto: **decisión pendiente** (§10, pregunta 3).
- **Askpass.** `prompt_matches` exige `<usuario>@<host de destino>` (o la ruta
  de la clave para la frase de paso), y `ASKPASS_ANSWERS` pasa a 2.
- **Huella del salto.** El alta del perfil muestra y guarda en el `known_hosts`
  propio la huella del salto, con el mismo flujo que la del destino.
- **Saltos encadenados.** Un `ProxyCommand` anidado por salto, o un límite a un
  salto si el anidamiento resulta frágil (se decide con pruebas y se documenta).
- **Pruebas.**
  - Unitarias de los argumentos y del entorno.
  - **Laboratorio** (`scripts/dev/remote-lab.sh`, contenedores `sshd` aislados):
    - bastión que pide contraseña: el askpass no responde y aparece el error
      `jumpNeedsKey`;
    - bastión con clave desconocida: falla la huella;
    - bastión con clave conocida: conecta;
    - la conexión directa no cambia.

### 9.7 Archivos previstos (E1)

| Nuevos | Modificados |
|---|---|
| `crates/dcc-activity/src/redact.rs` | `crates/dcc-activity/src/log.rs`, `lib.rs` |
| `crates/dcc-engine/src/risk.rs` | `crates/dcc-engine/src/spec.rs`, `binds.rs`, `edit.rs`, `edit_ops.rs`, `compose/ops.rs`, `compose/project.rs`, `build/mod.rs` |
| `scripts/verify-evolution-1.1.0.sh` | `crates/dcc-engine/src/ssh/args.rs`, `askpass.rs`, `session.rs`, `hostkey.rs`, `connect.rs` |
| Pruebas: `redact`, `risk`, IPC, laboratorio SSH | `src-tauri/src/commands/containers.rs`, `compose.rs`, `ipc_tests.rs` |
| — | `ui/src/features/containers/specModel.ts`, asistente, edición, Compose, `ui/src/shared/i18n/locales/*.json` (8) |
| — | DECISIONS.md (D-085…), CHANGELOG.md (1.1.0), STATUS.md (sección «Evolución»), USER_GUIDE.md, SECURITY_CHECKLIST.md |

Las versiones se actualizan a 1.1.0 en `Cargo.toml`, `package.json` y
`tauri.conf.json`.

### 9.8 Validación de la Etapa 1

- **Automática:** regresión completa con Docker 24, 26 y 29 + laboratorio SSH +
  `.deb` y paquete compatible, más los pasos nuevos de E1.
- **Manual (Debian 13), solo en las áreas afectadas.** Unos 8 pasos:
  - crear un contenedor con el socket `:ro` (aviso y confirmación);
  - crear con `cap_add SYS_ADMIN`;
  - crear un contenedor legítimo (sin avisos);
  - editar añadiendo un montaje sensible;
  - desplegar un Compose con el socket;
  - comprobar que una variable `MYSQL_PWD` no aparece en el panel ni en la
    orden;
  - perfil con salto con clave (conecta);
  - perfil con salto que pide contraseña (error claro).

---

## 10. Decisiones que necesitan al propietario antes o durante la Etapa 1

1. **Registros de contenedores y terminales.** ¿Se dejan sin alterar (propuesta),
   ocultando secretos solo en lo que produce la aplicación? La alternativa es
   ocultar también los patrones evidentes (URL con contraseña, PEM) en la vista
   de registros, con un interruptor «mostrar sin ocultar».
2. **Copias programadas (E4).** ¿Planificador interno, que solo funciona con la
   aplicación abierta (propuesta), o temporizador de usuario de systemd, que
   escribe en `~/.config/systemd/user`?
3. **ProxyJump con contraseña en el salto.** ¿Se rechaza y se pide clave o
   agente para el salto (propuesta, la opción más segura), o se admite una
   segunda contraseña propia del salto, guardada en el llavero y con su propio
   askpass?
4. **ESLint (E2).** ¿Se añade como dependencia de desarrollo?
5. **H-10 «modo de producción».** ¿Se incorpora en E6?

---

## 11. Estrategia de validación

- **Por etapa:**
  - regresión completa (las fases 1–10 no se vuelven a validar a mano);
  - pruebas nuevas de la etapa;
  - `.deb` instalado y arrancado en el entorno disponible;
  - validación manual **solo** de las áreas afectadas, con una lista corta y
    numerada.
- **Entornos:** daemons Docker 24, 26 y 29 aislados, laboratorio SSH de tres
  servidores en contenedores, Xvfb para la compilación de escritorio y el
  paquete compatible Debian 12+/Ubuntu 22.04+. **Nunca** servidores reales ni
  datos reales.
- **Informe de cada etapa:** cada punto aparece como implementado, probado
  automáticamente, validado manualmente, pendiente o no verificable en este
  entorno. No se da por correcta ninguna prueba que no se haya ejecutado.

---

## 12. Inventario de tareas

Estados: ☐ pendiente · ◐ en curso · ☑ hecha.

### E0
- ☑ Inspección del repositorio, la documentación y las fases.
- ☑ Estado real de los hallazgos.
- ☑ Componentes reutilizables.
- ☑ Línea base (§8).
- ☑ Este plan.

### E1 (1.1.0)
- ☐ E1-1 `redact` + integración en `ActivityLog` + exportación a TS + pruebas de extremo a extremo.
- ☐ E1-2 `risk`: montajes, capacidades, red `host` + casos peligrosos y legítimos.
- ☐ E1-3 Integración en creación, edición y Compose + `accepted_risks` comprobado en Rust + pruebas IPC.
- ☐ E1-4 ProxyJump seguro + huella del salto + askpass estricto + pruebas de laboratorio.
- ☐ E1-5 Interfaz y 8 idiomas.
- ☐ E1-6 Verificación, documentación, `.deb`, `.tar.gz` e informes.

### E2 (1.2.0)
- ☐ Resto de Compose (R-05).
- ☐ Rutas admitidas (R-06).
- ☐ Enlaces simbólicos (R-13).
- ☐ Puertos (R-10).
- ☐ Credenciales remotas (R-07).
- ☐ `deny.toml` y `npm audit` (R-08).
- ☐ vitest 5 (R-14).
- ☐ `SHA256SUMS` firmado (R-09).
- ☐ `DCC_ALLOW_ROOT` (R-11).
- ☐ OSC 8 (R-12).
- ☐ Permisos de Tauri, origen de la página de la versión y `sh -c` remoto (R-15, R-16, R-18).
- ☐ ESLint (si se autoriza).
- ☐ SECURITY_CHECKLIST.md con evidencias.

### E3 (1.3.0)
- ☐ Modelo de hallazgo y motor de auditoría sobre `risk`.
- ☐ Comprobaciones de `inspect`: seccomp, AppArmor, `no-new-privileges`, usuario, recursos, `readonly_rootfs`, PID e IPC.
- ☐ Compose e imágenes.
- ☐ Detección de Trivy y Grype, versión y fecha de la base, estados «no analizado».
- ☐ Historial por servidor.
- ☐ Exportación a JSON, Markdown y HTML.
- ☐ Interfaz: resumen, filtros, detalle, recomendaciones y acción correctiva con vista previa.
- ☐ Pruebas con contenedores peligrosos y legítimos.

### E4 (1.4.0)
- ☐ Formato de copia (manifiesto, hashes, permisos).
- ☐ Trait `BackupTarget` (local).
- ☐ Copia de volúmenes con un contenedor auxiliar fijado por *digest*.
- ☐ Volcados por motor (MySQL, MariaDB, PostgreSQL, Redis, MongoDB) y parada controlada.
- ☐ Configuración y Compose sin secretos.
- ☐ Cifrado (según la evaluación).
- ☐ Verificación de integridad.
- ☐ Retención.
- ☐ Restauración completa, selectiva y de prueba, con vista previa.
- ☐ Planificación (según la decisión 2).
- ☐ Historial.
- ☐ Pruebas de copia interrumpida, alterada, de base de datos y selectiva.

### E5 (1.5.0)
- ☐ Muestreador por servidor (`stats` compartido, frecuencia y pausa).
- ☐ Históricos en anillo y reducción a disco con límite.
- ☐ Métricas de servidor.
- ☐ Gráficos (decisión de biblioteca).
- ☐ Comparación de contenedores.
- ☐ Alertas con histéresis y notificaciones.
- ☐ Desconexión y reconexión.
- ☐ Medición del consumo de la aplicación.

### E6 (1.6.0)
- ☐ Enlaces entre herramientas: de la auditoría a la monitorización y a las copias por id.
- ☐ H-10 (si se aprueba).
- ☐ Regresión completa.
- ☐ Validación manual guiada.
- ☐ Documentación final.
