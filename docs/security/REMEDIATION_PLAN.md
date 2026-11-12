# Plan de corrección de seguridad

Acompaña a [SECURITY_AUDIT.md](SECURITY_AUDIT.md). Cada corrección es una
actualización pequeña e independiente, con sus propias pruebas. **No se ha
aplicado ninguna**: hace falta la autorización del propietario antes de
modificar el código.

Para cada corrección se indica:

- qué cambiar;
- por qué;
- en qué archivos;
- cómo;
- los riesgos;
- cómo verificarla.

Ninguna requiere reescribir componentes.

---

## P1: Corregir primero

### R-01 · Reglas comunes de montajes peligrosos (DCC-SEC-01)

- **Qué.** Crear un módulo `crates/dcc-engine/src/risk.rs` con
  `classify_host_path(path, local: bool) -> Option<PathRisk>`.
  `spec.rs` (asistente y edición) y `binds.rs` deben usar ese mismo módulo.
- **Por qué.** Hoy el socket de Docker en solo lectura, las subrutas de `/etc`,
  `~/.ssh`, `/var/lib/docker`, otros sockets de *runtime* y las rutas sin
  normalizar no producen aviso. Además, `spec.rs` y `binds.rs` aplican
  criterios distintos.
- **Archivos.**
  - `crates/dcc-engine/src/spec.rs` (`SENSITIVE_BINDS`, validación hacia la
    línea 456);
  - `crates/dcc-engine/src/binds.rs` (`risk_of`);
  - nuevo `crates/dcc-engine/src/risk.rs`;
  - traducciones de los nuevos códigos en `ui/src/shared/i18n/locales/*.json`.
- **Cómo.**
  1. Normalización léxica: unir las barras repetidas, quitar los componentes
     `.` y rechazar `..` (ya se rechaza).
  2. Reglas:
     - el socket de Docker, containerd o podman (`*/docker.sock`,
       `*/containerd.sock`, `*/podman.sock`, `/run/user/*/docker.sock`) da
       siempre un aviso nuevo, `bind.runtimeSocket`, aunque el montaje sea
       `read_only`;
     - `/`, `/etc/**`, `/root/**`, `/boot/**`, `/proc/**`, `/sys/**`,
       `/dev/**`, `/var/lib/docker/**`, `/home/*/.ssh/**`, `/home/*/.gnupg/**`
       y `/home/*/.docker/**` dan `bind.sensitive`, y mantienen el aviso aunque
       sean de solo lectura cuando contienen credenciales (`/etc/shadow`,
       `.ssh`, `.gnupg`, `.docker`);
     - `/usr`, `/bin`, `/sbin`, `/lib*` dan `bind.system` (como hoy en
       `binds.rs`).
  3. Solo en local, `std::fs::canonicalize` para detectar enlaces simbólicos
     hacia esas rutas. Si falla, se usa la ruta normalizada.
- **Riesgos.**
  - Más avisos en configuraciones legítimas (Traefik, Portainer). El texto debe
    explicar el porqué sin bloquear.
  - No cambia lo que se envía a Docker.
- **Verificación.**
  - Pruebas unitarias con la tabla de DCC-SEC-01.
  - Una prueba de que `spec.rs` y `binds.rs` devuelven lo mismo para la misma
    ruta.
  - `bash scripts/verify-phase-10.sh` sin regresiones.

### R-02 · Ocultación de secretos ampliada y compartida (DCC-SEC-05)

- **Qué.** Ampliar `is_secret_key` y añadir `value_has_credentials`. Exportar la
  lista de partes a TypeScript para que `specModel.ts` use la misma.
- **Por qué.** `MYSQL_PWD`, `DB_PASS`, `PGPASS`, `REDIS_AUTH`, `SMTP_PASS` y las
  URL con usuario y contraseña se muestran en el panel de actividad y en la
  orden equivalente.
- **Archivos.**
  - `crates/dcc-engine/src/spec.rs:232-245`;
  - `ui/src/features/containers/specModel.ts:58`;
  - una constante exportada con `ts-rs` o con un archivo JSON generado;
  - pruebas en los dos lados.
- **Cómo.**
  - Partes del nombre que se ocultan: `PASSWORD`, `PASSWD`, `PASS`, `PWD`,
    `SECRET`, `TOKEN`, `KEY`, `CREDENTIAL`, `PRIVATE`, `AUTH`, `COOKIE`,
    `SESSION`, `DSN`.
  - Se ocultan también los sufijos `_URL` y `_URI` cuando el valor contiene
    `://usuario:clave@`.
  - Excepción: los nombres que terminan en `_FILE` (Docker secrets) no se
    ocultan, porque su valor es una ruta.
  - En valores: la expresión `^[a-z][a-z0-9+.-]*://[^/@\s:]+:[^/@\s]+@` oculta
    solo la clave (`postgres://usuario:********@host/db`).
- **Riesgos.**
  - Ocultar de más, por ejemplo `PASSENGER_*` o `KEYBOARD_LAYOUT`. Es aceptable,
    porque solo afecta a la vista; el valor real sí se envía a Docker.
  - Revisar que la edición siga detectando cambios: compara valores reales, no
    los ocultos.
- **Verificación.**
  - Tabla de pruebas idéntica en Rust y en Vitest.
  - Prueba de que el cuerpo registrado en el panel no contiene el valor de
    `MYSQL_PWD`.
  - Prueba de igualdad entre las dos listas.

### R-03 · ProxyJump con las mismas garantías que la conexión directa (DCC-SEC-02)

- **Qué.** Sustituir `-J <salto>` por un `ProxyCommand` explícito que repita las
  opciones de seguridad. Limitar el askpass a la pregunta del destino.
- **Por qué.** El `ssh` del salto no recibe `StrictHostKeyChecking`, el
  `known_hosts` propio ni `BatchMode`, y hereda `SSH_ASKPASS`. Con autenticación
  por contraseña, la contraseña del destino puede entregarse al servidor
  intermedio.
- **Archivos.**
  - `crates/dcc-engine/src/ssh/args.rs` (`base_args` y `effective_config_args`,
    con `-J` en las líneas 189-192 y 303-305);
  - `ssh/askpass.rs` (`prompt_matches`, que recibiría el `usuario@host`
    esperado);
  - `ssh/session.rs:31` (`ASKPASS_ANSWERS`);
  - `ssh/hostkey.rs` (captura de la huella del salto);
  - las traducciones.
- **Cómo.**
  1. Para cada salto validado (`valid_proxy_jump`), construir el argumento
     `-o ProxyCommand=<orden>`. La orden lleva `ssh`, las mismas opciones
     `-o StrictHostKeyChecking=yes -o UserKnownHostsFile=… -o BatchMode=yes …`,
     `-W [%h]:%p` y `-- <salto>`, entrecomillada con `shell_quote`, porque
     OpenSSH ejecuta el `ProxyCommand` con `/bin/sh -c`.
     - **Alternativa más simple:** un solo salto, `BatchMode=yes` (clave o
       agente) y quitar las variables de askpass del entorno del salto con
       `env -u SSH_ASKPASS -u SSH_ASKPASS_REQUIRE -u DCC_ASKPASS_SOCKET ssh …`.
  2. Para los saltos encadenados, anidar el `ProxyCommand` o limitar a un salto.
     Documentar la decisión en DECISIONS.md.
  3. `prompt_matches(kind, prompt, expected)`:
     - con contraseña, exigir que la pregunta contenga `<usuario>@<host>`
       exactamente;
     - con frase de paso, exigir la ruta de la clave del perfil.
  4. `ASKPASS_ANSWERS` = 2.
  5. Flujo de alta del perfil: mostrar también la huella del servidor
     intermedio y guardarla en el `known_hosts` propio.
- **Riesgos.**
  - **Medio.** Puede romper perfiles con saltos que hoy funcionan gracias a
    `~/.ssh/config` (por ejemplo, saltos con alias o con `IdentityFile`
    propios). Mitigación: leer `ssh -G <salto>` para el usuario, el puerto y el
    nombre, y avisar si el salto necesita una clave que no está en el agente.
- **Verificación.**
  - Pruebas unitarias de los argumentos.
  - Prueba en un entorno aislado: dos contenedores `sshd` (bastión y destino)
    con contraseñas distintas.
    - El askpass no responde al bastión.
    - Con la clave del bastión ausente del `known_hosts` propio, la conexión
      falla con el aviso de huella.
    - Tras confirmarla, conecta.
  - No se usan servidores reales.

### R-04 · Avisos de capacidades peligrosas y red `host` (DCC-SEC-03)

- **Qué.** Añadir a `ContainerSpec::validate` los avisos `cap.dangerous` y
  `network.host`.
- **Por qué.** `cap_add: ALL` o `SYS_ADMIN` equivalen casi a `privileged`, que sí
  avisa. Con la red `host`, el contenedor alcanza los servicios locales.
- **Archivos.** `crates/dcc-engine/src/spec.rs:346` y `:499-512`, el módulo
  `risk.rs` de R-01 y las traducciones.
- **Cómo.**
  - Lista de capacidades peligrosas: `ALL`, `SYS_ADMIN`, `SYS_PTRACE`,
    `SYS_MODULE`, `SYS_RAWIO`, `SYS_BOOT`, `NET_ADMIN`, `DAC_READ_SEARCH`,
    `DAC_OVERRIDE`, `BPF`, `PERFMON`, `SYS_TIME`, `MAC_ADMIN`.
  - Normalizar el prefijo `CAP_`.
  - Severidad `Warning`, como `privileged`.
- **Riesgos.** Bajos: solo son avisos.
- **Verificación.** Pruebas unitarias con cada capacidad (`CAP_SYS_ADMIN` y
  `sys_admin` incluidas) y con la red `host`.

---

## P2: Corregir después

### R-05 · Análisis de riesgos antes de desplegar Compose (DCC-SEC-04)

- **Qué.** Antes de `up`, analizar el modelo normalizado de Compose y mostrar
  los avisos en el diálogo de confirmación.
- **Archivos.**
  - `crates/dcc-engine/src/compose/ops.rs` (plan de despliegue);
  - `compose/project.rs`;
  - `risk.rs` de R-01 y R-04;
  - `ui/src/features/compose/*` (diálogo).
- **Cómo.**
  - Ejecutar `docker compose config --format json`, que ya se usa para validar,
    y deserializar solo estos campos:
    - `privileged`, `cap_add`, `network_mode`, `pid`, `ipc`, `userns_mode`;
    - `security_opt` (`seccomp=unconfined`, `apparmor=unconfined`,
      `label=disable`, `no-new-privileges=false`);
    - `devices`, `volumes` de tipo `bind`, y `ports` sin `host_ip`.
  - Aplicar las mismas reglas que R-01 y R-04, y sumar los avisos por servicio.
- **Riesgos.**
  - El formato JSON de Compose cambia entre versiones. Hay que ser tolerante
    con los campos desconocidos.
  - Las versiones antiguas de `docker-compose` v1 no tienen `--format json`: en
    ese caso, analizar el YAML con `serde_yaml`, solo para avisar.
- **Verificación.** Proyectos de prueba con cada opción. La prueba con Docker 24,
  26 y 29 (`switch.sh`) debe mostrar los avisos sin cambiar el despliegue.

### R-06 · Rutas admitidas en las órdenes de archivos Compose (DCC-SEC-06)

- **Qué.** `compose_file_read`, `compose_file_save` y `compose_file_create` deben
  aceptar solo rutas admitidas.
- **Archivos.**
  - `src-tauri/src/commands/compose.rs:336-402`;
  - `crates/dcc-engine/src/compose/file.rs`;
  - `compose/remote_file.rs`;
  - `src-tauri/src/state.rs` (lista de rutas autorizadas en la sesión).
- **Cómo.** Se admite una ruta si cumple una de estas condiciones:
  - su nombre es un nombre Compose (`compose.y(a)ml`, `docker-compose.y(a)ml`,
    `*.override.y(a)ml`, o `*.y(a)ml` dentro de la carpeta de un proyecto
    registrado);
  - pertenece a un proyecto de `config.compose.projects`;
  - el usuario la eligió en el diálogo nativo (`dialog:allow-open`). Para
    esto, la interfaz llama a una orden `compose_authorize_path` que Rust
    registra; no basta con que la interfaz lo afirme.

  En local, se rechaza siempre una ruta que, una vez resuelta, esté bajo
  `~/.ssh`, `~/.gnupg`, `~/.docker` o `~/.config/docker-control-center`.
- **Riesgos.**
  - **Medio.** Proyectos con archivos de nombre no estándar
    (`produccion.yml`). Mitigación: se autorizan al importarlos con el diálogo.
- **Verificación.** Pruebas IPC:
  - leer `/etc/passwd` → `invalidInput`;
  - leer `~/.ssh/id_ed25519` → `invalidInput`;
  - el archivo de un proyecto importado → OK.
  - Pruebas de interfaz del editor sin cambios.

### R-07 · Credenciales de registro en servidores remotos (DCC-SEC-07)

- **Qué.** En los perfiles remotos, no enviar por defecto las credenciales
  locales.
- **Archivos.**
  - `crates/dcc-engine/src/registry_auth.rs`;
  - `pull.rs:214`;
  - `build/mod.rs:412`;
  - `ui/src/features/images/*` (diálogo de descarga).
- **Cómo.**
  - `Engine` ya sabe si es remoto. Para un daemon remoto, `PullAuth::DockerConfig`
    pide confirmación a la interfaz la primera vez para cada servidor y
    registro, y la recuerda en `servers.toml` sin secretos.
  - El diálogo muestra el registro y el servidor.
- **Riesgos.** Una pregunta más en el primer uso remoto.
- **Verificación.** Prueba del motor: sin la confirmación, una descarga remota se
  hace de forma anónima. Con ella, se envían las credenciales.

### R-08 · Comprobación de dependencias en la verificación

- **Qué.**
  - Añadir `deny.toml` versionado, con licencias permitidas, `sources` limitado
    a crates.io y los avisos de crates sin mantenimiento con excepciones
    justificadas para las crates transitivas de DCC-SEC-14.
  - Añadir los pasos `cargo deny check` y `npm audit --omit=dev` a
    `scripts/verify-phase-10.sh`, o a su sucesor.
- **Riesgos.** Nulos en tiempo de ejecución. Puede fallar la verificación cuando
  aparezcan avisos nuevos, que es lo que se busca.
- **Verificación.** El paso debe dar «correcta» con el estado actual y las
  excepciones documentadas.

---

## P3: Mejoras de distribución y ajustes menores

| ID | Hallazgo | Cambio | Archivos | Riesgo | Verificación |
|---|---|---|---|---|---|
| R-09 | DCC-SEC-09 | Generar `SHA256SUMS` al empaquetar y firmarlo (GPG o `cosign sign-blob`). `install.sh` lo verifica si está presente; `--no-verify` es explícito | `scripts/install.sh`, script de empaquetado | Bajo | Un `.deb` alterado debe ser rechazado |
| R-10 | DCC-SEC-08 | Aviso `port.allInterfaces` si `host_ip` está vacío | `spec.rs:372`, traducciones | Bajo | Prueba unitaria |
| R-11 | DCC-SEC-11 | `DCC_ALLOW_ROOT` solo con `cfg(debug_assertions)` o con una *feature* `test-root` | `src-tauri/src/lib.rs:220-226`, `scripts/dev/clean-install-test.sh` (si la usa) | Bajo: revisar las pruebas en chroot | Binario de publicación como root → negativa |
| R-12 | DCC-SEC-12 | `linkHandler` propio en xterm que abre con el navegador del sistema (orden validada) tras mostrar la URL completa | `ui/src/features/terminals/sessions.ts:180`, `ui/src/features/containers/detail/TerminalPanel.tsx:42` | Bajo | Prueba Vitest con una secuencia OSC 8; prueba manual |
| R-13 | DCC-SEC-13 | `save` resuelve el enlace con `canonicalize` y escribe junto al destino real | `crates/dcc-engine/src/compose/file.rs:86-125`, `remote_file.rs` (`readlink -f`) | Bajo | Prueba unitaria con un enlace |
| R-14 | DCC-SEC-10 | Actualizar vitest a 5.x en una rama aparte y adaptar las pruebas | `package.json`, `package-lock.json`, `vite.config.ts` | Medio (pruebas) | `npm audit` = 0; las mismas pruebas superadas |

## P4: Endurecimiento

| ID | Hallazgo | Cambio | Archivos |
|---|---|---|---|
| R-15 | DCC-SEC-17 | Sustituir `core:default` por los permisos usados | `src-tauri/capabilities/default.json` |
| R-16 | DCC-SEC-16 | `open_release_page` solo acepta el origen de la fuente configurada y `https://github.com/` | `src-tauri/src/commands/app.rs:112` |
| R-17 | DCC-SEC-15 | Valorar quitar `style-src 'unsafe-inline'` (estilos en línea de React → clases) | `tauri.conf.json`, componentes con `style=` |
| R-18 | DCC-SEC-18 | Ejecutar las órdenes remotas como `sh -c '<orden entrecomillada>'`, o documentar que se necesita un shell POSIX | `crates/dcc-engine/src/ssh/args.rs:36-41` |
| R-19 | DCC-SEC-14 | Seguir las versiones de `secret-service`, `tauri-plugin-opener` y Tauri, que eliminen las dependencias sin mantenimiento. Sin acción inmediata | — |

---

## Orden y dependencias

```
R-01 ─┬─► R-04 ─► R-05
      └─► (avisos agrupados en la interfaz, FEATURE_ROADMAP)
R-02 (independiente)
R-03 (independiente; necesita el entorno de pruebas con dos sshd)
R-06, R-07, R-08 (independientes)
R-09 … R-19 (independientes)
```

Cada actualización debe incluir:

- pruebas unitarias o IPC nuevas;
- la verificación completa sin regresiones con Docker 24, 26 y 29;
- una entrada en CHANGELOG.md y, si cambia el comportamiento, en DECISIONS.md;
- una actualización de [SECURITY_CHECKLIST.md](SECURITY_CHECKLIST.md).
