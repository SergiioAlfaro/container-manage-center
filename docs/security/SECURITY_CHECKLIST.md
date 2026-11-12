# Lista de comprobación de seguridad

Se usa antes de cada versión y al revisar cada cambio. La marca indica el
estado comprobado en la auditoría de la versión 1.0.0 (12-11-2026):

- ✅ cumple;
- ⚠️ cumple en parte (ver hallazgo);
- ❌ no cumple;
- ➖ no aplica.

Referencias: OWASP ASVS 5.0.0, modelo de seguridad de Tauri 2, guía de
seguridad de Docker Engine y RustSec.

## Tauri y la interfaz

- ✅ CSP sin `unsafe-eval` ni orígenes remotos en `script-src`. `connect-src`
  limitado a IPC.
- ⚠️ `style-src` sin `unsafe-inline` (DCC-SEC-15).
- ✅ `freezePrototype: true` y `withGlobalTauri: false`.
- ⚠️ Capacidades mínimas: `core:default` es más amplia de lo necesario
  (DCC-SEC-17).
- ✅ `opener` limitado a URL concretas. Ninguna capacidad de `shell` ni `fs`.
- ✅ No hay `dangerouslySetInnerHTML`, `innerHTML`, `eval`, `new Function` ni
  `href` dinámico. El Markdown se convierte en elementos de React.
- ✅ La interfaz no navega a orígenes remotos.
- ⚠️ Enlaces de la salida de terminal (OSC 8) con un manejador propio
  (DCC-SEC-12).
- ✅ La salida de la terminal se trata como datos (base64 → xterm) y nunca se
  ejecuta.

## Órdenes IPC y el proceso Rust

- ✅ Cada orden valida sus argumentos en Rust (tipos, longitudes, nombres,
  referencias de imagen).
- ⚠️ Las órdenes de archivos limitan las rutas a lo necesario (DCC-SEC-06).
- ✅ Las operaciones destructivas requieren confirmación (`confirmed`) y
  bloqueo cuando procede.
- ✅ Los procesos se lanzan con `Command::new` y una lista de argumentos, nunca
  con `sh -c` y entrada del usuario.
- ✅ Los bloques `unsafe` se limitan a llamadas a libc (`getuid`, PTY, `kill`,
  `ioctl`) con argumentos controlados.
- ⚠️ La negativa a ejecutarse como root no se puede desactivar en el binario de
  publicación (DCC-SEC-11).
- ✅ Hay límites de tamaño: archivos Compose de 1 MiB, respuestas de
  actualizaciones de 1 MiB, 5000 eventos, 10 000 líneas de registro, tamaño de
  terminal acotado.
- ⚠️ Límite de sesiones de terminal y flujos simultáneos (mejora P3).

## Docker Engine

- ✅ Solo socket Unix: no hay `tcp://` sin TLS.
- ⚠️ Aviso de montajes sensibles con rutas normalizadas, subrutas y sockets de
  *runtime*, aunque sean de solo lectura (**DCC-SEC-01**).
- ✅ Aviso de `privileged`.
- ⚠️ Aviso de capacidades peligrosas y de `network=host` (DCC-SEC-03).
- ⚠️ Aviso de puertos en todas las interfaces (DCC-SEC-08).
- ⚠️ Análisis de riesgos de Compose antes de desplegar (DCC-SEC-04).
- ✅ La recreación segura conserva una copia y se puede deshacer (D-070).
- ✅ El contexto de construcción no sigue enlaces y respeta `.dockerignore`.
- ⚠️ Credenciales de registro: no se envían a servidores remotos sin
  confirmación (DCC-SEC-07).

## SSH y administración remota

- ✅ `StrictHostKeyChecking=yes` con `known_hosts` propio (0600) y huella
  confirmada por el usuario.
- ✅ `UpdateHostKeys=no`, `ForwardAgent=no`, `ForwardX11=no`,
  `ClearAllForwardings=yes` y `PermitLocalCommand=no` en la línea de órdenes.
- ✅ `--` antes del host. El usuario, el host, el puerto y el salto se validan.
- ✅ Órdenes remotas entrecomilladas para POSIX, con una prueba de ida y vuelta
  por `sh`.
- ⚠️ Requisito de shell POSIX documentado (DCC-SEC-18).
- ⚠️ El servidor intermedio (ProxyJump) recibe las mismas opciones de seguridad
  y no recibe el askpass (**DCC-SEC-02**).
- ✅ Askpass:
  - socket 0600 en un directorio 0700;
  - comprobación de `peer_cred`;
  - nunca responde a preguntas de sí o no;
  - el secreto nunca va en argumentos ni en el entorno.
- ⚠️ El askpass responde solo a la pregunta del destino exacto, con un número
  mínimo de respuestas (DCC-SEC-02).
- ✅ Los diálogos muestran el servidor de destino de la operación.

## Secretos y datos sensibles

- ✅ Contraseñas y frases de paso en Secret Service, nunca en `servers.toml`.
- ✅ Configuración, perfiles e historial en archivos 0600 dentro de
  directorios 0700, con escritura atómica.
- ✅ El historial persistente no guarda cuerpos, órdenes equivalentes ni
  salidas. Los `buildargs` se ocultan.
- ⚠️ Ocultación de secretos en el panel y en la orden equivalente
  (`*_PWD`, `*_PASS`, `*_AUTH`, URL con credenciales) (**DCC-SEC-05**).
- ✅ Las credenciales de registro manuales solo viven en memoria. La salida de
  los *credential helpers* nunca se registra.
- ✅ Los documentos y los informes no contienen secretos.

## Dependencias y cadena de suministro

- ✅ `cargo audit`: 0 vulnerabilidades.
- ⚠️ Crates transitivas sin mantenimiento (DCC-SEC-14), a revisar.
- ✅ `cargo deny`: licencias y orígenes correctos (solo crates.io).
- ⚠️ `npm audit --omit=dev`: 0. `npm audit` completo: 3 avisos de vitest
  (DCC-SEC-10).
- ❌ Comprobación automática de dependencias en la verificación (R-08).
- ✅ Archivos de bloqueo versionados y usados con `--locked`.
- ⚠️ Las herramientas externas que se integren tendrán la versión fijada y la
  firma verificada (Trivy v0.69.4, CVE-2026-33634).

## Empaquetado y distribución

- ✅ El `.deb` contiene la licencia, el copyright y las licencias de terceros.
- ✅ `install.sh` usa `sudo` solo para `apt-get` y no ejecuta la aplicación como
  root.
- ❌ `SHA256SUMS` firmado y verificado por `install.sh` (DCC-SEC-09).
- ✅ La aplicación no instala Docker ni cambia grupos ni permisos del sistema.
- ✅ La documentación avisa de que el grupo `docker` equivale a root y
  recomienda rootless.

## Proceso para cada cambio

- [ ] Pruebas unitarias o IPC de la validación nueva.
- [ ] `bash scripts/verify-phase-10.sh --deb` (o su sucesor) sin regresiones,
      con Docker 24, 26 y 29.
- [ ] `cargo audit`, `cargo deny check` y `npm audit --omit=dev` sin hallazgos
      nuevos.
- [ ] Sin secretos en registros, panel, historial, capturas ni documentación.
- [ ] Traducciones en los ocho idiomas para cada aviso nuevo.
- [ ] CHANGELOG.md y DECISIONS.md actualizados. Esta lista revisada.
