# Auditoría de seguridad: Docker Control Center 1.0.0

| Campo | Valor |
|---|---|
| Versión auditada | 1.0.0 (commit `d1068d4`, árbol limpio) |
| Fecha | 12 de noviembre de 2026 |
| Tipo | Revisión de código, configuración y dependencias (sin modificar el proyecto) |
| Referencias | OWASP ASVS 5.0.0, modelo de seguridad de Tauri 2, guía de seguridad de Docker Engine, RustSec |
| Documentos relacionados | [REMEDIATION_PLAN.md](REMEDIATION_PLAN.md) · [FEATURE_ROADMAP.md](FEATURE_ROADMAP.md) · [SECURITY_CHECKLIST.md](SECURITY_CHECKLIST.md) |

Este informe no contiene contraseñas, claves, tokens ni datos personales. Los
ejemplos usan nombres ficticios.

---

## 1. Resumen ejecutivo

Docker Control Center es una aplicación de escritorio (Tauri 2, Rust y React)
que administra Docker Engine en el equipo local y en servidores por SSH. Quien
puede usar el socket de Docker tiene en la práctica privilegios de root en
ese equipo. Por eso el riesgo principal no es que alguien ataque la aplicación
desde fuera. Es que la aplicación ayude poco al usuario a ver que una operación
le da a un contenedor control sobre el equipo.

**Valoración general: buena base de seguridad, con carencias en los avisos de
riesgo de Docker y en un caso concreto de SSH.**

No se ha encontrado ninguna vulnerabilidad crítica ni ninguna vía de
ejecución remota de código. Entre los controles que funcionan bien:

- La interfaz no puede ejecutar código: CSP estricta, `freezePrototype`, no se
  inserta HTML y solo hay un permiso de apertura de URL.
- Los procesos se lanzan siempre sin shell.
- Las órdenes remotas se entrecomillan para POSIX.
- La identidad del servidor SSH se comprueba de forma estricta, con
  `known_hosts` propio.
- Los secretos se guardan en el almacén del sistema y se entregan por un socket
  efímero 0600 con comprobación del uid.
- El historial persistente no guarda cuerpos de petición.
- La desinstalación está protegida con `pkexec` y una lista de paquetes
  permitidos.
- `cargo audit` no encuentra vulnerabilidades en las 575 crates.

| Severidad | Número |
|---|---|
| Crítica | 0 |
| Alta | 1 |
| Media | 6 |
| Baja | 6 |
| Informativa | 5 |

**Los cinco problemas principales**

1. **DCC-SEC-01 (Alta).** La detección de montajes peligrosos es incompleta.
   - Montar `/var/run/docker.sock` en solo lectura no produce ningún aviso,
     aunque sigue dando control total de Docker.
   - Tampoco avisan `/etc/shadow`, `~/.ssh`, `/var/lib/docker`,
     `containerd.sock` ni las rutas sin normalizar (`//etc`, `/./etc`).
2. **DCC-SEC-02 (Media).** Con «Servidor intermedio» (ProxyJump), el `ssh` del
   salto no recibe las opciones de seguridad de la aplicación pero sí hereda
   `SSH_ASKPASS`. Con autenticación por contraseña, la contraseña del destino
   puede acabar en el servidor intermedio.
3. **DCC-SEC-05 (Media).** La heurística que oculta secretos no reconoce nombres
   habituales (`MYSQL_PWD`, `DB_PASS`, `PGPASS`, `REDIS_AUTH`, `SMTP_PASS`,
   `DATABASE_URL` con credenciales). Sus valores aparecen en el panel de
   actividad.
4. **DCC-SEC-03 / DCC-SEC-04 (Media).** No hay aviso para `cap_add` peligrosas
   (`ALL`, `SYS_ADMIN`…) ni para `network=host`. Los proyectos Compose se
   despliegan sin ningún análisis de riesgos (privilegiado, socket, `pid: host`…).
5. **DCC-SEC-06 (Media).** Las órdenes IPC de archivos Compose aceptan cualquier
   ruta absoluta. Si algún día se ejecutara código en la interfaz, permitirían
   leer y sobrescribir archivos del usuario, incluidas claves SSH sin cifrar.

**Qué corregir de inmediato:** DCC-SEC-01, DCC-SEC-02 y DCC-SEC-05. Son
correcciones localizadas, sin cambios de arquitectura (véase
[REMEDIATION_PLAN.md](REMEDIATION_PLAN.md)).

---

## 2. Arquitectura

```
┌──────────────────────── Equipo del usuario (usuario sin privilegios) ───────────────────────┐
│                                                                                              │
│  WebView (WebKitGTK)                     Proceso Rust (Tauri 2)                              │
│  React + xterm.js  ── IPC (99 órdenes) ─►  src-tauri: commands/*, state, servers, secrets    │
│  CSP estricta                              │                                                 │
│  capacidad "default"                       ├─ dcc-engine (bollard 0.21) ──► /var/run/docker.sock ──► dockerd (root)
│                                            ├─ docker CLI / compose (sin shell) ──────────────► dockerd
│                                            ├─ dcc-store: config.toml, servers.toml (0600)    │
│                                            ├─ dcc-activity: panel en memoria + historial 0600│
│                                            ├─ secret-service (D-Bus) ──► llavero del sistema │
│                                            ├─ ssh (OpenSSH, ControlMaster, sin shell local)  │
│                                            │     └─ askpass (la propia app) ◄─ socket 0600   │
│                                            ├─ pkexec apt-get remove (desinstalación)          │
│                                            └─ reqwest (comprobación de versiones, opcional)  │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
                     │ SSH (clave de host verificada con known_hosts propio)
                     ▼
            Servidor remoto: docker system dial-stdio ──► dockerd remoto
```

Funcionalidad comprobada en el código (no solo en la documentación):

- **Contenedores.** Asistente de creación (`ContainerSpec`), edición con
  recreación y copia, `commit`, registros, estadísticas, inspección y terminal
  (`exec` con TTY y marca de PID para cerrar la sesión, D-063).
- **Imágenes.** Descarga con credenciales de `~/.docker/config.json` o
  manuales, construcción (contexto en tar sin seguir enlaces y con
  `.dockerignore`), etiquetas, capas y limpieza.
- **Redes, volúmenes y directorios vinculados** (`binds.rs`, con clasificación
  de riesgo propia).
- **Compose.** Importar, crear, editar (lectura y escritura del archivo en local
  o por SSH), `up`, `down`, `build` y registros a través del CLI.
- **Servidores SSH.** Perfiles en `servers.toml`, secretos en Secret Service,
  `known_hosts` propio, ControlMaster y shell remota con PTY.
- **Actividad.** Panel en memoria con la petición real y la orden equivalente
  (con secretos ocultos), eventos de Docker e historial persistente sin cuerpos.
- **Sistema.** Comprobación de versiones (solo consulta, sin descarga),
  desinstalación de componentes con `pkexec` y negativa a ejecutarse como root.

Nada indica funcionalidad anunciada que no esté implementada. La aplicación no
instala Docker ni cambia grupos del sistema, tal como indica el README.

### 2.1 Modelo de amenazas

**Activos**

- Control de Docker Engine local y remoto, que equivale a root en cada equipo.
- Secretos:
  - contraseñas y frases de paso SSH en Secret Service;
  - credenciales de registro en `~/.docker/config.json` o en sus *helpers*;
  - variables de entorno de contenedores.
- Archivos del usuario: proyectos Compose, claves SSH y configuración.
- Integridad de la identidad de los servidores (`known_hosts` propio).
- Integridad del paquete instalado.
- Disponibilidad de los contenedores en producción.

**Usuarios y privilegios**

- Un único usuario de escritorio, sin privilegios, que pertenece al grupo
  `docker` o usa Docker rootless.
- En remoto, el usuario SSH del perfil.
- Root solo se usa a través de `pkexec`, para desinstalar.

**Límites de confianza**

1. WebView ↔ proceso Rust (IPC).
2. Proceso Rust ↔ `dockerd` (socket Unix; el daemon es root).
3. Proceso Rust ↔ `ssh` ↔ servidor remoto (red).
4. Contenedores ↔ equipo (montajes, capacidades, red).
5. Salida de contenedores y servidores ↔ interfaz (registros, terminal).
6. Distribución (`.deb`, `install.sh`) ↔ sistema (apt como root).
7. Dependencias de terceros ↔ binario.

**Componentes expuestos y superficies de ataque**

- 99 órdenes IPC.
- La API de Docker, a través de las especificaciones que crea el usuario.
- Archivos Compose de terceros.
- Salida de terminal con secuencias de escape.
- Servidores SSH y saltos.
- Fuente de actualizaciones (HTTPS).
- *Credential helpers* (ejecutables del PATH).
- Paquete `.deb` y dependencias.

**Atacantes considerados**

- Una imagen o un contenedor comprometido.
- Un autor malicioso de un `compose.yaml` o de una imagen pública.
- Un atacante en la red, o un servidor intermedio malicioso.
- Un administrador del servidor remoto.
- Un proceso local de otro usuario.
- Un atacante de la cadena de suministro.
- El propio usuario cometiendo un error, con la operación correcta en el sitio
  equivocado.

**Escenarios pedidos**

| # | Escenario | Probabilidad | Impacto | Estado actual y prevención |
|---|---|---|---|---|
| 1 | Usuario malicioso que ejecuta órdenes desde la interfaz | Baja | Alto | La interfaz solo llama a órdenes con tipos fijos; no hay shell (`Command::new` con argumentos) y las órdenes remotas se entrecomillan. Quien maneja la aplicación ya controla Docker por diseño. Mejoras: DCC-SEC-06 (rutas) y DCC-SEC-17 (permisos mínimos) |
| 2 | Aplicación o contenedor comprometido que accede al socket de Docker | Media | Crítico (root) | La aplicación no expone el socket a la red (solo socket Unix). El riesgo está en los contenedores con el socket montado: **DCC-SEC-01** (no avisa del socket en solo lectura) y **DCC-SEC-04** (Compose sin análisis) |
| 3 | Acceso a archivos del equipo mediante montajes | Media | Alto | Avisos solo para coincidencias exactas: **DCC-SEC-01**. `binds.rs` tiene un criterio más amplio que hay que unificar |
| 4 | Conexión SSH suplantada | Baja | Alto | `StrictHostKeyChecking=yes` con `known_hosts` propio, huella confirmada por el usuario y `UpdateHostKeys=no`. Excepción: **DCC-SEC-02** (el salto se verifica con la configuración del usuario y puede recibir la contraseña) |
| 5 | Secreto que aparece en el panel de actividad | Alta | Medio | Ocultación por nombre y el historial no guarda cuerpos. Hueco: **DCC-SEC-05** (nombres habituales sin ocultar) |
| 6 | Operación remota ejecutada en el servidor equivocado | Baja | Alto | Cada orden lleva el id del servidor, los diálogos muestran el servidor de destino y la identidad está fijada por la clave de host. Riesgo residual: **DCC-SEC-07** (credenciales locales enviadas a cualquier servidor). Mejora: color o etiqueta por servidor en el modo de producción (FEATURE_ROADMAP) |
| 7 | Paquete manipulado | Baja | Crítico (instalación como root) | Sin firma ni sumas: **DCC-SEC-09** |
| 8 | Dependencia vulnerable | Media | Variable | 0 vulnerabilidades en las crates; avisos de npm solo en desarrollo (**DCC-SEC-10**); crates sin mantenimiento (**DCC-SEC-14**). Sin comprobación automática en la verificación: mejora P2 |

---

## 3. Alcance y limitaciones

**Dentro del alcance**

- `src-tauri/`: configuración, capacidades, órdenes IPC, estado, secretos,
  actualizaciones y desinstalación.
- `crates/dcc-engine`: Docker, SSH, Compose, terminales y credenciales de
  registro.
- `crates/dcc-store` y `crates/dcc-activity`.
- `ui/src`: puntos donde se inserta contenido, enlaces, terminal y portapapeles.
- `scripts/install.sh`, `scripts/run.sh` y la configuración del paquete `.deb`.
- `Cargo.lock` (575 paquetes) y `package-lock.json` (281 paquetes).

**Limitaciones**

- **No se han hecho pruebas de explotación** contra servidores reales ni contra
  el sistema del usuario. Los hallazgos de SSH se basan en el código y en el
  comportamiento documentado y observado de OpenSSH 9.6p1. Se ejecutó
  `ssh -v -J` contra puertos cerrados de `127.0.0.1`, sin conexión real.
- **OSV-Scanner 2.2.2** leyó los dos archivos de bloqueo, pero no pudo consultar
  `api.osv.dev` ni descargar la base sin conexión. El proxy de red del entorno de
  auditoría bloquea esos dominios. Lo cubren en parte `cargo audit`, que usa la
  base RustSec, y `npm audit`, que usa la base de avisos de GitHub.
- **Trivy no se ejecutó.** La descarga de las versiones probadas falló (404).
  Además, en marzo de 2026 se publicó una versión maliciosa de Trivy (v0.69.4,
  CVE-2026-33634). Cualquier uso futuro debe fijar versiones y verificar firmas
  (véase la sección 12).
- No se ha hecho fuzzing ni análisis dinámico de la WebView. Tampoco se ha
  medido el consumo con cargas altas, porque la tarea prohíbe las pruebas de
  carga.
- No se ha revisado el código de terceros, salvo el manejador de enlaces
  OSC 8 de xterm.js 5.5.0.
- Licencias del proyecto: GPL-3.0-or-later es provisional (README).

---

## 4. Metodología

1. **Inventario.** Se revisaron las órdenes IPC registradas en
   `src-tauri/src/lib.rs`, las capacidades y la CSP, las variables de entorno
   leídas, los `Command::new` y los bloques `unsafe`.
2. **Revisión manual por clases de ataque:**
   - inyección de órdenes (local y remota);
   - recorrido de rutas;
   - XSS y navegación;
   - escritura y lectura arbitraria de archivos;
   - escalada de privilegios (`pkexec`, root);
   - secretos (almacén, askpass, panel e historial);
   - suplantación SSH;
   - denegación de servicio (límites);
   - condiciones de carrera (guardado de archivos, bloqueo de desinstalación);
   - cadena de suministro.
3. **Comprobación de hallazgos.**
   - Se repitió en Python, sin tocar el proyecto, la lógica de los avisos de
     montajes y de la ocultación de secretos para comprobar su salida.
   - Se observó en OpenSSH qué opciones recibe el `ssh` del salto.
4. **Dependencias.** Se ejecutaron `cargo audit`, `cargo deny` (con
   configuración externa al repositorio, en el directorio temporal),
   `npm audit --package-lock-only` y OSV-Scanner. Antes y después se compararon
   los hashes SHA-256 de `Cargo.lock` y `package-lock.json`: no cambiaron.
5. **Clasificación.** Cada hallazgo se clasifica con los criterios de la
   sección 9. Solo se marca como **confirmado** lo que el código muestra sin
   ambigüedad. Lo que depende de condiciones que no se han reproducido se marca
   como **potencial**.

No se ha modificado ningún archivo del proyecto. Los únicos archivos nuevos son
los de `docs/security/`.

---

## 5. Herramientas y versiones

| Herramienta | Versión | Uso | Resultado |
|---|---|---|---|
| rustc / cargo | 1.97.0 | `cargo tree --locked --offline` | Origen de las crates señaladas |
| cargo-audit | 0.23.0 (base RustSec de 12-11-2026, 1490 avisos) | `cargo audit --json` | 0 vulnerabilidades; 4 crates sin mantenimiento y 1 con código no seguro (*unsound*), todas transitivas |
| cargo-deny | 0.20.2 | `check licenses/bans/sources/advisories` con configuración externa | Licencias OK; orígenes OK; avisos: 4 sin mantenimiento; 37 duplicados (solo avisos) |
| npm | 10.9.9 (Node 22.23.4) | `npm audit --json --package-lock-only` | 3 avisos, todos en dependencias de desarrollo (vitest) |
| OSV-Scanner | 2.2.2 | `scan source --lockfile …` | Sin resultado: red bloqueada (limitación) |
| OpenSSH | 9.6p1 | `ssh -v -J` contra puertos cerrados | El salto no hereda las opciones `-o` |
| Trivy | — | — | No ejecutado (sección 3) |

Las herramientas se instalaron solo en el entorno aislado de auditoría (directorio
`/tmp`), no en el sistema del usuario ni en el proyecto.

---

## 6. Vulnerabilidades y debilidades confirmadas

Tipos de clasificación: **VC** vulnerabilidad confirmada · **VP** vulnerabilidad
potencial · **DD** debilidad de diseño · **RC** riesgo que depende de la
configuración · **MP** mejora preventiva.

### DCC-SEC-01 · Detección incompleta de montajes peligrosos (socket de Docker en solo lectura)

| Campo | Valor |
|---|---|
| Clasificación | DD (confirmada en el código) |
| Severidad | **Alta** |
| Componente | Motor: validación de la especificación de contenedor |
| Archivo | `crates/dcc-engine/src/spec.rs:247-257` (`SENSITIVE_BINDS`), `:456-465` (comprobación) |

**Descripción.** El aviso `bind.sensitive` solo aparece si se cumplen dos
condiciones. La ruta, sin la barra final, debe coincidir exactamente con una de
la lista `/`, `/etc`, `/root`, `/boot`, `/proc`, `/sys`, `/dev`,
`/var/run/docker.sock` o `/run/docker.sock`. Además, el montaje debe ser de
lectura y escritura.

**Evidencia.** Al repetir la lógica, ninguno de estos casos produce aviso:

| Origen | Solo lectura | ¿Aviso? |
|---|---|---|
| `/var/run/docker.sock` | sí | **no** |
| `/etc/shadow` | no | **no** |
| `//etc`, `/./etc` | no | **no** |
| `/home/<usuario>/.ssh` | no | **no** |
| `/var/lib/docker` | no | **no** |
| `/run/containerd/containerd.sock` | no | **no** |
| `/run/user/<uid>/docker.sock` (rootless) | no | **no** |

La pantalla de directorios vinculados (`binds.rs:84-97`, `risk_of`) clasifica
más rutas (`/usr`, `/var/lib/docker`, `*/docker.sock`). El asistente de
creación y la edición usan solo `spec.rs`, así que los dos criterios no
coinciden.

**Condiciones de explotación.** El usuario crea o edita un contenedor con uno de
esos montajes. Es frecuente al copiar configuraciones de Traefik, Portainer o
herramientas de monitorización, y entonces el usuario cree que `:ro` lo hace
seguro. Basta con que la imagen esté comprometida.

**Impacto.** Un socket de Docker montado, aunque sea en solo lectura, permite
llamar a toda la API: crear un contenedor privilegiado con `/` montado y obtener
root en el equipo o el servidor. Montar `/etc/shadow` o `~/.ssh` expone
credenciales.

**Probabilidad.** Media.

**Recomendación.**
- Mover a una función común la clasificación de riesgo de rutas.
- Normalizar la ruta de forma léxica (unir `//`, quitar `.`).
- Considerar sensible cualquier subruta de `/etc`, `/root`, `/boot`, `/proc`,
  `/sys`, `/dev`, `/var/lib/docker` y `/home/*/.ssh`.
- Detectar cualquier `*.sock` de docker, containerd o podman, y avisar aunque
  sea de solo lectura, con un texto que diga que `:ro` no limita la API.
- En local, resolver además los enlaces simbólicos con `canonicalize`, solo
  para avisar.

**Complejidad.** Baja.

**Prueba de verificación.** Pruebas unitarias de `ContainerSpec::validate` con
cada fila de la tabla: deben producir `bind.sensitive` o un nuevo código
`bind.dockerSocket`. Un `/home/<usuario>/proyecto` normal no debe producir
avisos.

### DCC-SEC-02 · ProxyJump: el salto no hereda las opciones de seguridad y recibe el askpass

| Campo | Valor |
|---|---|
| Clasificación | VP / RC (código confirmado; explotación no reproducida) |
| Severidad | **Media** |
| Componente | SSH |
| Archivos | `crates/dcc-engine/src/ssh/args.rs:189-192` (`-J`), `ssh/session.rs:83-92` (`SSH_ASKPASS`, `SSH_ASKPASS_REQUIRE=force`), `ssh/session.rs:31` (`ASKPASS_ANSWERS = 64`), `ssh/askpass.rs:30-44` (`prompt_matches`) |

**Descripción.** Con `-J`, OpenSSH ejecuta un segundo `ssh` para el servidor
intermedio. A ese proceso solo le pasa `-l`, `-p` y `-v`, no las opciones `-o`.

**Evidencia.**
- Salida observada de OpenSSH 9.6p1:
  `Setting implicit ProxyCommand from ProxyJump: ssh -l <u> -p <p> -v -W '[%h]:%p' <salto>`.
- En consecuencia, el salto no usa `StrictHostKeyChecking=yes`, el
  `known_hosts` de la aplicación, `BatchMode`, `UpdateHostKeys=no` ni
  `PreferredAuthentications`. Se aplica solo la configuración del usuario.
- El proceso hijo hereda el entorno de la conexión maestra: `SSH_ASKPASS` (la
  propia aplicación), `SSH_ASKPASS_REQUIRE=force` y `DCC_ASKPASS_SOCKET`.
- `prompt_matches(Password, …)` acepta cualquier pregunta que contenga
  «password». La pregunta de OpenSSH para el salto
  (`<usuario>@<salto>'s password:`) coincide.
- El servidor del askpass responde hasta 64 veces.

**Condiciones de explotación.** Deben darse las tres:
- el perfil usa autenticación por contraseña;
- el perfil tiene un servidor intermedio;
- el servidor intermedio pide contraseña, porque es así o porque lo suplanta
  alguien que aprovecha que la identidad del salto no se comprueba con la
  política de la aplicación.

**Impacto.** La contraseña del servidor de destino se envía a otro servidor
(divulgación de credenciales). La identidad del intermedio queda verificada con
una política más débil que la que la aplicación anuncia (D-076).

**Probabilidad.** Baja o media: la combinación contraseña + ProxyJump es
habitual en redes con bastión.

**Recomendación.** En orden de preferencia:
1. Construir el salto con
   `-o ProxyCommand=ssh <mismas opciones de seguridad> -W [%h]:%p <salto>`, con
   la lista de argumentos y la validación ya existentes, y quitar del entorno
   del proceso del salto las variables de askpass.
2. Como mínimo, hacer que el askpass solo responda a la pregunta que incluye
   exactamente `<usuario>@<host de destino>`.
3. Bajar `ASKPASS_ANSWERS` al número necesario (1 o 2).
4. Si no se hace lo anterior, rechazar la combinación contraseña + ProxyJump con
   un error explicado.

**Complejidad.** Media.

**Prueba de verificación.** En un entorno aislado de contenedores (bastión y
destino con `sshd`, usuarios y contraseñas de prueba distintos):
- el askpass no debe entregar la contraseña del destino a la pregunta del
  bastión;
- con una clave de host del bastión que no esté en el `known_hosts` de la
  aplicación, la conexión debe fallar.

Prueba unitaria: `prompt_matches` con `bastion@jump's password:` debe devolver
`false` para un perfil cuyo destino es otro servidor.

### DCC-SEC-03 · Capacidades y modo de red peligrosos sin aviso

| Campo | Valor |
|---|---|
| Clasificación | DD (confirmada) |
| Severidad | **Media** |
| Componente | Motor: especificación de contenedor |
| Archivo | `crates/dcc-engine/src/spec.rs:499-512` (`privileged` y `cap_add`), `:346` (red `host`) |

**Descripción.**
- `privileged` produce un aviso.
- `cap_add` solo comprueba el formato, así que acepta `ALL`, `SYS_ADMIN`,
  `SYS_PTRACE`, `SYS_MODULE`, `NET_ADMIN` o `DAC_READ_SEARCH` sin aviso.
- `network = "host"` solo se comprueba porque no admite puertos publicados.

**Impacto.** `cap_add: ALL` o `SYS_ADMIN` equivale casi a modo privilegiado:
monturas y salida del contenedor. Con `network=host` el contenedor ve todos los
servicios locales, incluidos los que solo escuchan en `127.0.0.1`.

**Probabilidad.** Media.

**Recomendación.** Avisos `cap.dangerous` (con la lista anterior) y
`network.host`, al mismo nivel que `privileged`, con texto en los ocho idiomas.

**Complejidad.** Baja.

**Prueba de verificación.** Pruebas unitarias de `validate` con cada capacidad y
con red `host`.

### DCC-SEC-04 · Despliegue de Compose sin análisis de riesgos

| Campo | Valor |
|---|---|
| Clasificación | DD (confirmada) |
| Severidad | **Media** |
| Componente | Compose |
| Archivos | `crates/dcc-engine/src/compose/` (no hay comprobaciones de `privileged`, `docker.sock`, `network_mode`, `pid`, `cap_add` ni `devices`; la validación es `docker compose config`) |

**Descripción.** El asistente de contenedores avisa de `privileged` y de
algunos montajes. Un proyecto Compose con esas mismas opciones se despliega sin
ningún aviso. Compose admite además opciones que el asistente no tiene:
`pid: host`, `ipc: host`, `devices`, `security_opt: [seccomp:unconfined]`,
`userns_mode: host` y `cgroup_parent`.

**Impacto.** Un `compose.yaml` descargado de Internet puede dar root en el
equipo en cuanto se pulsa «Desplegar».

**Probabilidad.** Media.

**Recomendación.**
- Antes de `up`, analizar la salida normalizada de `docker compose config --format json`,
  que ya se obtiene para validar.
- Aplicar las mismas reglas que DCC-SEC-01 y DCC-SEC-03.
- Mostrar los avisos en el diálogo de confirmación. Es informativo, no bloquea.

**Complejidad.** Media.

**Prueba de verificación.** Proyectos de prueba con cada opción: el plan de
despliegue debe listar el aviso correspondiente.

### DCC-SEC-05 · Ocultación de secretos incompleta en el panel de actividad

| Campo | Valor |
|---|---|
| Clasificación | DD (confirmada) |
| Severidad | **Media** |
| Componente | Actividad, interfaz |
| Archivos | `crates/dcc-engine/src/spec.rs:232-245` (`is_secret_key`), `ui/src/features/containers/specModel.ts:58` (`SECRET`). Se usa en `edit.rs` (`redacted_body`), `spec.rs` (`redacted()`), `build/mod.rs`, `containers.rs` e `images.rs` |

**Descripción.** Solo se ocultan las variables cuyo nombre contiene `PASSWORD`,
`PASSWD`, `SECRET`, `TOKEN`, `KEY`, `CREDENTIAL` o `PRIVATE`.

**Evidencia.** Al repetir la lógica, no se ocultan `MYSQL_PWD`, `DB_PASS`,
`PGPASS`, `REDIS_AUTH`, `SMTP_PASS` ni `DATABASE_URL`. Además, `MONKEY` se
oculta sin ser un secreto (falso positivo inocuo).

**Alcance real.** El historial persistente (`crates/dcc-activity/src/history.rs`)
no guarda cuerpos ni órdenes equivalentes. Los argumentos de construcción se
ocultan en la ruta de la API. La exposición se limita al panel de actividad en
memoria y a la interfaz: cuerpo de la petición, orden `docker run -e …`
equivalente y vista del asistente. Ahí puede verse o copiarse sin querer en
una captura de pantalla o al compartir la orden.

**Impacto.** Divulgación de credenciales de bases de datos o servicios
(escenario 5 del modelo de amenazas).

**Probabilidad.** Alta: esos nombres son los de las imágenes oficiales de MySQL,
MariaDB y PostgreSQL.

**Recomendación.**
- Ampliar la regla en un único lugar, compartido con la interfaz mediante una
  lista exportada:
  - nombres con las partes `PASS`, `PWD`, `AUTH`, `APIKEY`, `DSN` o `COOKIE`;
  - sufijos `_URL` y `_URI`.
- Ocultar además cualquier valor con credenciales en una URL (`esquema://usuario:clave@`),
  sea cual sea el nombre de la variable.
- Tratar `_FILE` (Docker secrets) como no secreto, porque es una ruta.

**Complejidad.** Baja.

**Prueba de verificación.** Prueba unitaria con la tabla anterior en Rust y en
TypeScript. Una prueba de igualdad entre las dos listas evita que se desajusten.

### DCC-SEC-06 · Órdenes IPC de archivos Compose sin límite de rutas

| Campo | Valor |
|---|---|
| Clasificación | DD (defensa en profundidad; no explotable sin ejecución de código en la WebView) |
| Severidad | **Media** |
| Componente | IPC / Compose |
| Archivos | `src-tauri/src/commands/compose.rs:336-386` (`compose_file_read`, `compose_file_save`, `compose_file_create`), `crates/dcc-engine/src/compose/file.rs:51-56` (`absolute`), `:62-84`, `:86-125`; equivalente remoto en `compose/remote_file.rs` |

**Descripción.**
- `compose_file_read` devuelve el contenido de cualquier archivo absoluto de
  hasta 1 MiB que sea UTF-8 y que el usuario pueda leer.
- `compose_file_save` reemplaza cualquier archivo existente en el que el usuario
  pueda escribir. Solo exige la huella, que se obtiene leyéndolo antes.
- `compose_file_create` crea `compose.yaml` en cualquier carpeta.
- En servidores remotos hacen lo mismo con el usuario SSH.

**Condiciones de explotación.** Hace falta ejecutar código en la WebView. Esta
auditoría no ha encontrado ninguna forma de hacerlo:
- no se inserta HTML;
- la CSP no admite `script-src` externo ni `unsafe-eval`;
- `withGlobalTauri` es `false`;
- no se navega a orígenes remotos.

**Impacto si se combina con un XSS futuro:**
- lectura de claves SSH privadas sin cifrar, de `~/.docker/config.json` con
  `auths` y de tokens en `~/.config`;
- persistencia mediante escritura en `~/.bashrc` o en `~/.config/autostart`.

**Probabilidad.** Baja.

**Recomendación.**
- **Lectura y guardado:** aceptar solo
  - rutas cuyo nombre sea un nombre Compose admitido (`compose.yaml`,
    `compose.yml`, `docker-compose.y(a)ml`, `*.override.y(a)ml`), o
  - rutas registradas en un proyecto conocido (`config.compose.projects`), o
  - rutas elegidas por el usuario en el diálogo nativo durante la sesión
    (lista blanca en el estado de Rust).
- **Rutas que se rechazan siempre:** las que estén bajo `~/.ssh`, `~/.gnupg` o
  `~/.docker`, y las que, una vez resueltas, apunten a ellas.

**Complejidad.** Media.

**Prueba de verificación.** Pruebas IPC (`ipc_tests.rs`):
- `compose_file_read("/home/<u>/.ssh/id_ed25519")` y
  `compose_file_read("/etc/passwd")` deben fallar con `invalidInput`;
- un `compose.yaml` de un proyecto debe seguir funcionando.

### DCC-SEC-07 · Las credenciales de registro locales se envían a los daemons remotos

| Campo | Valor |
|---|---|
| Clasificación | RC |
| Severidad | **Media** |
| Componente | Imágenes, SSH |
| Archivos | `crates/dcc-engine/src/registry_auth.rs:40-48` (`PullAuth::DockerConfig` por defecto), `:201-216`; `crates/dcc-engine/src/pull.rs:214` |

**Descripción.** Al descargar una imagen en un servidor remoto, la aplicación
resuelve las credenciales con el `~/.docker/config.json` **local**, y sus
*credential helpers*. Después las envía en `X-Registry-Auth` al daemon
**remoto**. Es lo mismo que hace `docker -H ssh://…`, pero la interfaz no lo
indica.

**Impacto.** Quien sea root en el servidor remoto, o haya comprometido ese
`dockerd`, puede capturar las credenciales de registro del usuario.

**Probabilidad.** Baja o media.

**Recomendación.**
- En servidores remotos, usar `Anonymous` por defecto, o pedir confirmación la
  primera vez para cada servidor y registro.
- Indicar en el diálogo de descarga «Se enviarán tus credenciales de
  `<registro>` al servidor `<nombre>`».

**Complejidad.** Baja.

**Prueba de verificación.** Prueba del motor con un perfil remoto simulado:
`resolve` no debe devolver credenciales sin la confirmación.

### DCC-SEC-08 · Puertos publicados en todas las interfaces sin aviso

| Campo | Valor |
|---|---|
| Clasificación | MP |
| Severidad | **Baja** |
| Archivo | `crates/dcc-engine/src/spec.rs:372` (`host_ip` vacío se acepta sin aviso) |

**Descripción.** Si no se indica IP, Docker publica en `0.0.0.0` y `::`, y sus
reglas de iptables saltan UFW y firewalld en su configuración habitual.

**Impacto.** Un servicio de desarrollo queda accesible desde la red.

**Recomendación.** Mostrar un aviso informativo
(«accesible desde otros equipos; usa 127.0.0.1 si es solo para ti»).

**Complejidad.** Baja.

**Prueba de verificación.** Prueba unitaria de `validate`.

### DCC-SEC-09 · Paquetes sin firma ni sumas de verificación

| Campo | Valor |
|---|---|
| Clasificación | RC (distribución) |
| Severidad | **Baja** |
| Archivos | `scripts/install.sh` (instala `./*.deb` sin verificar), `src-tauri/tauri.conf.json` (`bundle` sin firma) |

**Descripción.** No se generan `SHA256SUMS` ni firma GPG o Sigstore. El
instalador confía en el `.deb` que tenga al lado.

**Impacto.** Un paquete manipulado en la distribución (escenario 7) se instala
como root mediante `apt`.

**Recomendación.**
- Generar `SHA256SUMS` y firmarlo, con GPG o con `cosign sign-blob`.
- Hacer que `install.sh` lo verifique cuando esté presente, con la opción
  `--no-verify` explícita.

**Complejidad.** Baja.

**Prueba de verificación.** `install.sh` debe negarse a instalar un `.deb` cuya
suma no coincida.

### DCC-SEC-10 · Dependencias de desarrollo con avisos (vitest y tinypool)

| Campo | Valor |
|---|---|
| Clasificación | VC en las dependencias / sin impacto en el producto |
| Severidad | **Baja** |
| Archivo | `package-lock.json`: `vitest` 3.2.7, `@vitest/mocker`, `tinypool` 1.1.1 |

**Evidencia.** `npm audit`:
- GHSA-82fw-gwwq-j7x9 (moderada, CWE-22);
- GHSA-qxg6-89mg-rqv8 (moderada, CWE-94/306);
- GHSA-5gmw-xhrv-c9v3 y GHSA-85c8-ppgw-ccpr (críticas en npm, CWE-1321/94).

La corrección es vitest 5.1.0, un cambio de versión mayor.

**Impacto real.** Son solo de desarrollo. No van en el `.deb`, porque `dist/` se
construye con Vite sin vitest. Los avisos de tinypool requieren que el atacante
controle las opciones de los *workers*, y los de mocker un servidor de pruebas
expuesto. El riesgo se limita a la máquina de desarrollo durante `vitest`.

**Recomendación.** Actualizar vitest en una tarea aparte con revisión de las
pruebas. No se ha actualizado nada en esta auditoría.

**Complejidad.** Media, por el cambio de versión mayor.

**Prueba de verificación.** `npm audit --omit=dev` debe dar 0; tras actualizar,
`npm audit` debe dar 0.

### DCC-SEC-11 · `DCC_ALLOW_ROOT` desactiva la negativa a ejecutarse como root en el binario distribuido

| Campo | Valor |
|---|---|
| Clasificación | MP |
| Severidad | **Baja** |
| Archivo | `src-tauri/src/lib.rs:220-226` |

**Descripción.** La variable de entorno pensada para pruebas aisladas también
funciona en el binario de producción.

**Impacto.** Con `sudo -E` o un lanzador mal configurado, la aplicación (WebView
incluida) se ejecuta como root.

**Recomendación.** Compilarla solo con `cfg(debug_assertions)` o con una
*feature* de pruebas. Otra opción es exigir además que `DCC_CONFIG` apunte a un
directorio temporal.

**Complejidad.** Baja.

**Prueba de verificación.** Con el binario de publicación ejecutado como root y
`DCC_ALLOW_ROOT=1`, la aplicación debe terminar con el mensaje de negativa.

### DCC-SEC-12 · Enlaces OSC 8 en las terminales con el manejador por defecto de xterm.js

| Campo | Valor |
|---|---|
| Clasificación | VP (pendiente de prueba dinámica) |
| Severidad | **Baja** |
| Archivos | `ui/src/features/terminals/sessions.ts:180`, `ui/src/features/containers/detail/TerminalPanel.tsx:42` (`new Terminal` sin `linkHandler`) |

**Descripción.** xterm.js 5.5.0 convierte en enlaces las secuencias OSC 8 de la
salida, solo `http` y `https`. Al pulsarlas, el manejador por defecto muestra
`confirm()` y llama a `window.open()`. La salida viene de contenedores y
servidores, que no son de confianza.

En Tauri 2 y WebKitGTK, lo previsible es que `window.open` sin manejador de
ventanas nuevas no haga nada. La nueva ventana no tendría capacidades IPC,
porque la capacidad está limitada a la ventana `main`. No se ha comprobado
dinámicamente.

**Impacto.** Engaño o *phishing*, que requiere que el usuario pulse y acepte.
Como mucho, una ventana con contenido remoto y sin IPC.

**Recomendación.** Definir `linkHandler` para que abra los enlaces con el
navegador del sistema mediante la orden validada (DCC-SEC-16), mostrando la URL
completa. También puede desactivarse con `linkHandler: null`.

**Complejidad.** Baja.

**Prueba de verificación.** Ejecutar
`printf '\e]8;;https://example.com\e\\enlace\e]8;;\e\\'` en una terminal:
- sin confirmación no debe ocurrir nada;
- con ella debe abrirse el navegador del sistema, nunca la WebView.

### DCC-SEC-13 · Guardar un archivo Compose que es un enlace simbólico lo sustituye por un archivo normal

| Campo | Valor |
|---|---|
| Clasificación | DD (integridad) |
| Severidad | **Baja** |
| Archivo | `crates/dcc-engine/src/compose/file.rs:86-125` |

**Descripción.**
- `read` sigue el enlace.
- `save` crea el temporal en la carpeta del enlace y hace `rename` sobre la ruta
  del enlace. Con eso sustituye el enlace por un archivo normal.
- El archivo de destino no cambia.

**Impacto.** El archivo real queda sin actualizar sin que el usuario lo note
(integridad o disponibilidad, no confidencialidad).

**Recomendación.**
- Resolver con `canonicalize` antes de guardar y escribir junto al destino real.
- Si se prefiere no seguir enlaces, rechazar el guardado con un mensaje.

**Complejidad.** Baja.

**Prueba de verificación.** Prueba unitaria con `compose.yaml` → `real.yaml`:
tras guardar, el enlace debe seguir existiendo y `real.yaml` debe tener el
contenido nuevo.

---

## 7. Riesgos potenciales y observaciones informativas

| ID | Título | Clasificación | Severidad | Evidencia y recomendación |
|---|---|---|---|---|
| DCC-SEC-14 | Crates transitivas sin mantenimiento o con código no seguro | RC (cadena de suministro) | Informativa | `zbus_names` 4.3.5 (RUSTSEC-2026-0371), `zvariant` 5.15.1 (RUSTSEC-2026-0370) y `zvariant_derive` (RUSTSEC-2026-0372) llegan por `secret-service` 5.2.0 y `tauri-plugin-opener` 2.7.2. `proc-macro-error` 1.0.4 (RUSTSEC-2024-0370) llega por `glib-macros` (gtk3-rs, Tauri/wry). `glib` 0.18.5 tiene código no seguro (RUSTSEC-2024-0429) en iteradores de `VariantStrIter` que la aplicación no usa directamente. Ninguno es una vulnerabilidad explotable. Hay que seguir las versiones de Tauri y `secret-service` y añadir `cargo deny` a la verificación |
| DCC-SEC-15 | CSP con `style-src 'unsafe-inline'` | MP | Informativa | `tauri.conf.json`. Permite inyectar CSS si existiera inyección de HTML, que hoy no existe. Valorar *nonces* o hashes de Tauri cuando sea posible sin romper Tailwind |
| DCC-SEC-16 | `open_release_page` acepta cualquier URL `https://` | MP | Informativa | `src-tauri/src/commands/app.rs:112`. La URL procede de la fuente de actualizaciones configurada por el usuario. Recomendación: aceptar solo el origen de la fuente configurada y `github.com` |
| DCC-SEC-17 | Capacidad `core:default` más amplia de lo necesario | MP | Informativa | `src-tauri/capabilities/default.json`. Incluye los permisos por defecto de ventana, webview, menú, bandeja, etc. Recomendación: sustituirla por la lista mínima (`core:event:default`, `core:window:allow-…` usados) para reducir lo que puede hacer un XSS hipotético |
| DCC-SEC-18 | Entrecomillado remoto válido para shells POSIX | MP | Informativa | `ssh/args.rs:20-41`. El comentario indica sh, bash, dash y zsh. Con un shell de inicio no POSIX en el servidor (fish, csh), el comportamiento no está probado. Recomendación: ejecutar las órdenes remotas como `sh -c '<orden>'`, con la orden entrecomillada, o documentar el requisito |

**Controles que funcionan correctamente.** Se comprobaron y se mantienen como
referencia en la lista de comprobación:

- No hay `dangerouslySetInnerHTML`, `innerHTML`, `eval`, `new Function`,
  `window.open` propio ni `href` dinámico. La ayuda en Markdown crea elementos de
  React.
- Todos los `Command::new` usan una lista de argumentos sin shell. Los
  *credential helpers* se validan por nombre y el servidor se pasa por stdin.
- SSH:
  - `StrictHostKeyChecking=yes`, `UpdateHostKeys=no`, `ForwardAgent=no`,
    `ClearAllForwardings=yes` y `PermitLocalCommand=no` se pasan en la línea de
    órdenes (prevalecen sobre `~/.ssh/config`);
  - `--` antes del host;
  - el usuario, el host y el salto se validan.
- Askpass: socket en un directorio 0700, socket 0600, comprobación de
  `peer_cred`, y nunca responde a preguntas de sí o no.
- Comprobación de versiones:
  - solo HTTPS, o `http://` a la propia máquina sin proxy;
  - 1 MiB y 15 s como máximo;
  - redirecciones restringidas;
  - nunca descarga ni instala.
- Desinstalación: confirmación, bloqueo, repetición de la simulación de APT,
  lista de paquetes permitidos, paquetes protegidos y `pkexec` con ruta
  absoluta.
- Configuración, perfiles e historial en archivos 0600 dentro de directorios
  0700, con escritura atómica.
- Docker solo por socket Unix: no hay transporte `tcp://` sin TLS.
- El contexto de construcción no sigue enlaces simbólicos y respeta
  `.dockerignore`.
- Terminal: tamaño acotado, cierre de toda la sesión con el PID anunciado y
  salida enviada como base64 a xterm, que nunca la ejecuta como código.
- Los diálogos de confirmación muestran el servidor de destino
  (`DialogTarget`, 0.10.0).

---

## 8. Evidencias

| Evidencia | Dónde |
|---|---|
| Listas de avisos de montajes y ocultación de secretos (reproducción de la lógica) | Sección 6, DCC-SEC-01 y DCC-SEC-05 |
| Opciones que OpenSSH pasa al salto | `ssh -v -J` → `Setting implicit ProxyCommand from ProxyJump: ssh -l … -p … -v -W '[%h]:%p' …` |
| `cargo audit` | 0 vulnerabilidades; avisos: RUSTSEC-2024-0370, RUSTSEC-2026-0370/0371/0372, RUSTSEC-2024-0429 |
| `cargo deny` | `licenses ok`, `sources ok`, `bans` con 37 avisos de duplicados (sin errores), `advisories` con los 4 sin mantenimiento |
| `npm audit` | 3 paquetes (vitest, @vitest/mocker, tinypool), todos de desarrollo |
| Integridad de los archivos de bloqueo | `sha256sum -c`: `Cargo.lock: OK`, `package-lock.json: OK` |
| Árbol del repositorio | `git status` limpio antes de crear `docs/security/` |

Las salidas completas de las herramientas no se incluyen en el repositorio,
porque contienen rutas del entorno de auditoría.

---

## 9. Clasificación de la severidad

| Severidad | Criterio |
|---|---|
| Crítica | Explotable a distancia o sin intervención del usuario, con control total del equipo o divulgación masiva de secretos |
| Alta | Lleva a root en el equipo o servidor, o a divulgar credenciales, con una acción habitual del usuario que la aplicación debería advertir |
| Media | Impacto alto pero con condiciones (configuración concreta, otra vulnerabilidad previa), o divulgación limitada de secretos |
| Baja | Impacto limitado, o requiere varias condiciones improbables |
| Informativa | Mejora de la postura de seguridad sin vía de ataque demostrada |

La probabilidad se estima como baja, media o alta según lo habitual que es la
configuración necesaria en usuarios de Docker en Debian y Ubuntu.

---

## 10. Soluciones (resumen)

El detalle de cada corrección (archivos, cómo, riesgos y pruebas) está en
[REMEDIATION_PLAN.md](REMEDIATION_PLAN.md). Ninguna requiere reescribir
componentes:

| Prioridad | Hallazgos | Tipo de cambio |
|---|---|---|
| P1 | 01, 02, 05, 03 | Funciones de validación y SSH localizadas, con pruebas unitarias |
| P2 | 04, 06, 07 | Análisis de Compose antes de `up`; lista de rutas admitidas; política de credenciales remotas |
| P3 | 08, 09, 11, 12, 13, 10 | Avisos, firma de la distribución, ajustes de compilación y de xterm, actualización de vitest |
| P4 | 14–18 | Endurecimiento de la configuración de Tauri y seguimiento de dependencias |

---

## 11. Mejoras técnicas recomendadas

| Mejora | Beneficio | Dificultad | Prioridad |
|---|---|---|---|
| **Módulo único de reglas de riesgo** (`dcc-engine::risk`), usado por el asistente, la edición, los directorios vinculados, Compose y una futura auditoría de contenedores | Coherencia: hoy `spec.rs` y `binds.rs` dan respuestas distintas | Baja | P1 |
| **Lista única de reglas de secretos** exportada a TypeScript con `ts-rs` | Evita que la interfaz y el motor se desajusten | Baja | P1 |
| `cargo deny` y `npm audit --omit=dev` en `verify-phase-*.sh` (con configuración `deny.toml` versionada) | Detecta pronto las dependencias vulnerables o sin mantenimiento | Baja | P2 |
| Pruebas de propiedades (*proptest*) para `shell_quote` y `remote_command` (la ida y vuelta por `sh` ya existe para casos fijos) | Mayor confianza en la frontera de inyección remota | Baja | P3 |
| Permisos mínimos de Tauri (sustituir `core:default`) | Menos superficie si apareciera un XSS | Baja | P3 |
| Límite de sesiones de terminal y flujos simultáneos por servidor (hoy solo los acota el canal de 256 mensajes) | Evita agotar memoria o descriptores por error de la interfaz | Baja | P3 |
| Mensajes de error sin rutas absolutas del usuario en el panel (normalizar con `~`) | Menos datos personales en capturas | Baja | P4 |
| Rendimiento: el aviso de montajes sensibles con `canonicalize` solo en local y fuera del hilo de la interfaz | Sin coste perceptible | Baja | P2 (con DCC-SEC-01) |
| Experiencia: mostrar los avisos de seguridad agrupados («Este contenedor puede controlar el equipo») en lugar de códigos sueltos | Más comprensible para el usuario | Media | P2 |

---

## 12. Nuevas herramientas

El análisis completo, con integración, dependencias, licencias y riesgos, está
en [FEATURE_ROADMAP.md](FEATURE_ROADMAP.md). Solo se proponen herramientas que
responden a un problema real de quien administra Docker con esta aplicación.

Licencias y condiciones comprobadas: Trivy (Apache-2.0), Grype y Syft
(Apache-2.0), Dockle (Apache-2.0), Docker Bench for Security (Apache-2.0),
Hadolint (GPL-3.0) y restic (BSD-2-Clause). Docker Scout depende de una
suscripción de Docker: el plan gratuito permite 3 repositorios con Scout y
análisis local ilimitado; la licencia del CLI no se ha podido confirmar. **Aviso
sobre Trivy:** en marzo de 2026 se distribuyó una versión maliciosa (v0.69.4,
CVE-2026-33634). Cualquier integración debe exigir una versión fijada y
verificar su firma y su suma.

---

## 13. Hoja de ruta

| Orden | Actualización pequeña e independiente | Prioridad | Dificultad | Impacto | Riesgo de regresión | Depende de |
|---|---|---|---|---|---|---|
| 1 | Reglas de montajes peligrosos (DCC-SEC-01) | P1 | Baja | Alto | Bajo | — |
| 2 | Ocultación de secretos ampliada (DCC-SEC-05) | P1 | Baja | Medio | Bajo | — |
| 3 | ProxyJump seguro (DCC-SEC-02) | P1 | Media | Medio | Medio (conexiones SSH) | — |
| 4 | Avisos de capacidades y red `host` (DCC-SEC-03) | P1 | Baja | Medio | Bajo | 1 |
| 5 | Análisis de riesgos de Compose (DCC-SEC-04) | P2 | Media | Alto | Bajo | 1, 4 |
| 6 | Rutas admitidas en IPC de Compose (DCC-SEC-06) | P2 | Media | Medio | Medio (importar proyectos) | — |
| 7 | Credenciales en servidores remotos (DCC-SEC-07) | P2 | Baja | Medio | Bajo | — |
| 8 | `cargo deny` y `npm audit` en la verificación | P2 | Baja | Medio | Nulo | — |
| 9 | Sumas y firma de la distribución (DCC-SEC-09) | P3 | Baja | Medio | Bajo | — |
| 10 | Ajustes menores (DCC-SEC-08, 11, 12, 13) | P3 | Baja | Bajo | Bajo | — |
| 11 | vitest 5 (DCC-SEC-10) | P3 | Media | Bajo | Medio (pruebas) | — |
| 12 | Permisos mínimos de Tauri, CSP, origen de la página de versión (14–18) | P4 | Baja | Bajo | Bajo | — |
| 13 | Nuevas herramientas (FEATURE_ROADMAP.md) | P2–P4 | Variable | Variable | Variable | 1, 4, 5 |

---

## 14. Conclusión

El diseño de seguridad de Docker Control Center es sólido en lo que más suele
fallar en aplicaciones de este tipo:

- no hay shell ni HTML inyectable;
- la identidad SSH se comprueba de forma estricta;
- los secretos están en el almacén del sistema;
- se usan privilegios mínimos.

No hay vulnerabilidades críticas ni dependencias de ejecución vulnerables.

Lo que falta es sobre todo **avisar mejor de lo que es peligroso en Docker**.
El socket en solo lectura, las capacidades, la red `host` y Compose son, hoy,
el camino más corto de un contenedor comprometido a root en el equipo. A eso se
suma un caso concreto de SSH (ProxyJump con contraseña) y una regla de secretos
demasiado estrecha. Todas las correcciones son localizadas y se pueden publicar
como actualizaciones pequeñas e independientes, en el orden de la sección 13.

Ningún cambio de código se ha aplicado. Se espera la revisión y autorización del
propietario antes de empezar.
