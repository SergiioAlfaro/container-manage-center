# Instalación

Plataformas soportadas: **Debian 12 o superior** y **Ubuntu 22.04 o superior** (x86_64).
Docker Engine compatible: **20.10 o posterior** (API 1.41+), en modo normal o rootless.

## Opción A: paquete .deb (recomendada)

Hay dos paquetes de la misma versión:

| Paquete | Para |
|---|---|
| `docker-control-center_1.2.0_amd64.deb` | Debian 13 y Ubuntu 24.04 o posteriores (glibc 2.39+) |
| `docker-control-center_1.2.0_amd64-compat-debian12-ubuntu2204.deb` | También Debian 12 y Ubuntu 22.04 (glibc 2.34+) |

### Con el script de instalación

```bash
bash scripts/install.sh            # o: bash install.sh, junto a los paquetes
```

Ejecútalo con tu usuario normal; pide la contraseña de administrador solo para
`apt-get`. El script:

1. Comprueba el sistema (Debian, Ubuntu o derivada; amd64; versión de glibc).
2. Busca los paquetes junto a él, en `dist/` y en las carpetas de compilación
   (`target/release/bundle/deb`, `target/compat`) y elige el más reciente que
   tu sistema puede ejecutar, según la glibc que declara cada paquete.
3. Si junto al paquete (o junto al script) hay un archivo `SHA256SUMS`,
   comprueba que la suma SHA-256 del paquete coincide (1.2.0). Si no coincide o
   el paquete no aparece, **no instala nada**. Sin `SHA256SUMS`, avisa de que no
   puede comprobarlo.
4. Simula la instalación con apt y luego instala con
   `sudo apt-get install ./paquete.deb`, que resuelve las dependencias.
5. Comprueba, **sin cambiar nada**, lo que la aplicación necesita para trabajar
   (Docker y tu acceso al socket, Docker Compose, el cliente SSH, un almacén de
   contraseñas) y explica cómo resolver lo que falte.

Opciones: `--check` (solo comprueba y elige el paquete), `--deb RUTA` (instala
ese paquete), `--yes` (sin confirmar en apt), `--uninstall` y `--no-verify`
(omite la comprobación de `SHA256SUMS`; no recomendado). El script **nunca**
instala Docker, te añade al grupo `docker`, cambia permisos del socket ni toca
la configuración de SSH.

### Comprobar la integridad a mano

```bash
sha256sum -c SHA256SUMS      # en la carpeta de la publicación
```

`SHA256SUMS` detecta un paquete dañado o cambiado. Todavía **no está firmado**
(la firma GPG necesita la clave del autor; decisión pendiente): alguien que
pudiera cambiar a la vez el paquete y `SHA256SUMS` no se detectaría. Descarga
los dos del mismo sitio de confianza. Quien publica los paquetes genera el
archivo con `bash scripts/release-sums.sh CARPETA`.

### A mano

```bash
sudo apt install ./docker-control-center_1.2.0_amd64.deb
```

`apt` instala las dependencias de ejecución (WebKitGTK 4.1, GTK 3, glibc…).
El paquete declara las versiones mínimas que necesita su binario
(`dpkg-deb -f paquete.deb Depends`). Si tu sistema es demasiado antiguo para ese
paquete, apt lo rechaza al instalar en vez de instalar algo que no arranca.

### Ejecutar

La aplicación aparece en el menú como **Docker Control Center**; también se abre
con `docker-control-center` o, desde el código fuente, con `bash scripts/run.sh`
(abre la versión instalada o la compilación de `target/release`; `--build` la
compila). Se ejecuta con tu usuario normal: **no la ejecutes con sudo** (se
niega a arrancar como root; en el programa publicado no hay forma de
forzarlo, D-091).

### Desinstalar

Con `bash scripts/install.sh --uninstall`, desde **Configuración › Desinstalación
y componentes** (muestra qué se puede quitar, la simulación de APT y la orden
exacta; pide la contraseña de administrador solo para `apt-get remove`), o a
mano con `sudo apt remove docker-control-center`. En todos los casos se
conservan tu configuración y tus servidores (`~/.config/docker-control-center/`),
el historial (`~/.local/state/docker-control-center/`) y las contraseñas
recordadas (en el llavero del sistema); bórralos a mano si no los quieres.
Docker, sus datos y Docker Compose no se tocan.

Si al abrir la ventana aparece «La interfaz no pudo arrancar» con un mensaje
técnico, copia ese mensaje al informar del problema.

### Dependencias

| Tipo | Paquetes |
|---|---|
| Obligatorias (las instala apt) | `libwebkit2gtk-4.1-0`, `libjavascriptcoregtk-4.1-0`, `libgtk-3-0`, `libsoup-3.0-0`, `libglib2.0-0`, `libgdk-pixbuf-2.0-0`, `libcairo2`, `libdbus-1-3`, `libgcc-s1`, `libc6` (versiones mínimas en el `Depends` del paquete) |
| Recomendada (la instala apt) | `openssh-client`: servidores remotos |
| Para trabajar | Docker Engine 20.10+ con acceso para tu usuario; Docker Compose v2 (`docker compose` o `docker-compose`) para el apartado Compose |
| Opcional | Un almacén Secret Service (GNOME Keyring, KWallet) para recordar contraseñas de servidores; `pkexec` (polkit) para desinstalar desde la aplicación |

Los componentes de terceros que van dentro del programa y sus licencias están en
[packaging/THIRD-PARTY.md](packaging/THIRD-PARTY.md) (instalado en
`/usr/share/doc/docker-control-center/`).

## Opción B: compilar desde el código fuente

1. Paquetes de sistema: `bash scripts/install-build-deps.sh`
   (o `sudo apt install build-essential curl wget file pkg-config libssl-dev libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev`).
2. Rust 1.80+: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
3. Node.js 20+: con nvm (`nvm install 22`) o el repositorio de NodeSource; las versiones de Debian 12 y Ubuntu 22.04 son demasiado antiguas.
4. Compilar y ejecutar:
   ```bash
   npm ci
   npx tauri build --debug --no-bundle && ./target/debug/docker-control-center
   npx tauri build --no-bundle && bash scripts/run.sh   # compilación de producción
   npx tauri build --bundles deb   # paquete: target/release/bundle/deb/
   bash scripts/deb-set-depends.sh target/release/bundle/deb/*.deb   # dependencias reales
   ```
   Un paquete compilado así solo sirve para sistemas con una glibc igual o más
   nueva que la del equipo donde se compiló.

## Opción C: paquete para distribuir (Debian 12+ y Ubuntu 22.04+)

```bash
bash scripts/build-deb-compat.sh   # resultado: target/compat/docker-control-center_<versión>_amd64.deb
```

Compila dentro de un contenedor de Docker con Ubuntu 22.04 (la glibc más
antigua soportada) y con tu usuario, sin red durante la compilación y sin sudo.
El paquete resultante se instala y funciona en Debian 12, Debian 13, Ubuntu 22.04
y Ubuntu 24.04. Necesita Docker, Node.js y Rust en el equipo. La primera vez
descarga la imagen `ubuntu:22.04` y unos 700 MB de paquetes de compilación.

## Acceso a Docker

La aplicación habla con Docker Engine por su **socket Unix** y necesita que tu
usuario pueda abrirlo. Nunca pide sudo, no cambia permisos ni grupos, y no
ejecuta las instrucciones de diagnóstico por ti.

### Opción 1: Docker rootless (recomendada por seguridad)
El daemon se ejecuta con tu usuario y el socket está en
`$XDG_RUNTIME_DIR/docker.sock` (normalmente `/run/user/1000/docker.sock`).
Guía oficial: <https://docs.docker.com/engine/security/rootless/>.
La aplicación lo detecta sola (también a través del contexto `rootless` que
crea el instalador) y lo indica con la etiqueta **rootless**.

```bash
systemctl --user enable --now docker
```

### Opción 2: grupo `docker`
```bash
sudo usermod -aG docker "$USER"
# Cierra la sesión y vuelve a entrar.
```

> Pertenecer al grupo `docker` equivale en la práctica a tener privilegios de
> administrador en el equipo. Valóralo antes de hacerlo.

### Orden de detección (modo automático)
1. Variable `DOCKER_HOST` (solo `unix://…`; `tcp://` y `ssh://` no se admiten en esta versión).
2. Contexto activo de Docker (`docker context use …`).
3. Socket rootless de tu usuario.
4. Socket del sistema `/var/run/docker.sock`.

Puedes fijar un socket concreto en **Configuración › Conexión con Docker ›
Socket personalizado**. "Probar conexión" comprueba una ruta sin guardarla.

## Docker Compose

La aplicación funciona sin Docker Compose, pero el apartado **Docker Compose**
lo necesita (si falta, el apartado explica cómo instalarlo) y
`scripts/verify-phase-7.sh` lo exige. Se admite **Compose v2 o posterior**, como plugin
(`docker compose`) o como binario independiente (`docker-compose` v2). Compose
v1 (`docker-compose 1.x`) no se admite.

| Sistema | Instalación |
|---|---|
| Debian 13 | `sudo apt install docker-compose` (Compose 2.26) |
| Debian 12 | El paquete `docker-compose` es la v1 (no admitida). Usa el repositorio oficial de Docker: `sudo apt install docker-compose-plugin` |
| Ubuntu 22.04 o posterior | `sudo apt install docker-compose-v2` |
| Repositorio oficial de Docker (cualquiera) | `sudo apt install docker-compose-plugin` |

Para comprobarlo: **Configuración › Conexión con Docker › Docker Compose ›
Comprobar**, o `docker compose version` en una terminal. Si el plugin no está
instalado, `docker compose version --short` responde `unknown flag: --short`:
no es que la versión no admita la opción, es que el CLI de Docker no encuentra
el plugin.

## Servidores remotos (0.10.0)

Desde **Servidores** se administran servidores Docker remotos por **SSH**, a la vez que el equipo local. Docker no se expone por la red: la aplicación usa el cliente SSH del sistema y `docker system dial-stdio` en el servidor.

### En este equipo
- Cliente OpenSSH (`ssh` y `ssh-keygen`): paquete `openssh-client`, que el `.deb` recomienda.
- Para recordar frases de paso o contraseñas, un almacén seguro de secretos (GNOME Keyring, KWallet u otro compatible con Secret Service). Sin él, las credenciales se piden en cada conexión y no se guardan nunca en texto plano.

### Requisitos mínimos de cada servidor
| Requisito | Detalle |
|---|---|
| Servidor SSH | En marcha y accesible desde este equipo (puerto 22 u otro). |
| Docker Engine | 20.10 o posterior (API 1.41). |
| Cliente `docker` | En el `PATH` del usuario SSH (`docker system dial-stdio`). |
| Permisos | El usuario SSH puede usar el socket de Docker (grupo `docker`, Docker rootless o un socket propio indicado en el perfil). |
| Docker Compose | Opcional: `docker compose` (plugin) o `docker-compose` v2 en el servidor. |
| Utilidades | `stat`, `base64`, `find`, `dd`, `mv`, `chmod`, `readlink` (coreutils y findutils de GNU), para los archivos Compose y las carpetas vinculadas. |
| Shell de inicio | POSIX: `bash`, `sh`, `dash` o `zsh` (1.2.0, documentado). Las órdenes se entrecomillan para POSIX; `fish`, `csh` y `tcsh` no están soportados. |
| Servidor intermedio (opcional) | Un solo salto (`usuario@host:puerto`). Su huella se confirma en la aplicación como la del servidor (1.1.0). Solo admite clave SSH o agente SSH, no contraseña. Debe permitir el reenvío hacia el servidor (`AllowTcpForwarding`). |

Pertenecer al grupo `docker` del servidor equivale a tener acceso de **root** en él. Docker rootless, con su socket indicado en el perfil, es la alternativa más segura.

### Lo que la aplicación no hace en un servidor
No instala Docker, no cambia permisos ni grupos, no modifica la configuración SSH, no abre puertos y no modifica `~/.ssh/known_hosts`. La identidad de cada servidor se confirma con su huella la primera vez y se guarda en `~/.config/docker-control-center/known_hosts` (0600).

### Archivos
- `~/.config/docker-control-center/servers.toml`: perfiles (sin secretos, 0600).
- `~/.config/docker-control-center/known_hosts`: identidades confirmadas.
- `$XDG_RUNTIME_DIR/docker-control-center/ssh/`: sockets de control temporales (0700).

## Diagnóstico de problemas

Si no hay conexión, el **Dashboard** muestra la causa, los sockets comprobados y
los pasos para resolverlo. También puedes diagnosticar desde una terminal con el
mismo código que usa la aplicación (compilando desde el código fuente):

```bash
cargo run -q -p dcc-engine --bin dcc-diagnose            # detección automática
cargo run -q -p dcc-engine --bin dcc-diagnose -- --socket /run/user/1000/docker.sock
cargo run -q -p dcc-engine --bin dcc-diagnose -- --json  # salida completa
cargo run -q -p dcc-engine --bin dcc-diagnose -- --compose  # incluye Docker Compose
```

| Causa mostrada | Significado | Qué hacer |
|---|---|---|
| Docker no parece estar instalado | No hay socket, ni `docker`, ni `dockerd` | `sudo apt install docker.io` o la guía oficial de Docker |
| No se encuentra Docker Engine | Hay instalación pero ningún socket | `sudo systemctl enable --now docker` (rootless: `systemctl --user enable --now docker`) |
| Docker Engine está detenido | El socket existe pero nadie responde | Igual que arriba; revisa `systemctl status docker` |
| Sin permiso para usar el socket | Tu usuario no puede abrirlo | Grupo `docker` y volver a iniciar sesión, o Docker rootless |
| …ya perteneces al grupo… | La sesión actual no tiene el grupo activo | Cerrar sesión y volver a entrar |
| Conexión de Docker no compatible | `DOCKER_HOST`/contexto con `tcp://` o `ssh://` | `unset DOCKER_HOST; docker context use default` o socket personalizado. Para un Docker remoto por SSH, añádelo en **Servidores** (0.10.0) |
| La ruta no es un socket / no responde como Docker | La ruta configurada apunta a otra cosa | Corregir la ruta en Configuración |
| Docker no responde a tiempo | Sin respuesta en 5 s | `sudo systemctl restart docker` |
| Versión de Docker demasiado antigua | API < 1.41 | Actualizar a Docker 20.10 o posterior |

La aplicación reintenta la conexión sola cada pocos segundos (de 2 s a 30 s) y
vigila la conexión activa cada 10 s.

## Variables de entorno

| Variable | Uso |
|---|---|
| `DCC_CONFIG` | Ruta alternativa del archivo de configuración |
| `DCC_STATE_DIR` | Carpeta alternativa del historial y del diario de eventos |
| `HTTPS_PROXY`, `NO_PROXY` | Proxy para la comprobación de actualizaciones (si se activa) |
| `DCC_LOG` | Nivel de registro del backend (`error`, `warn`, `info`, `debug`, `trace`) |
| `DOCKER_HOST`, `DOCKER_CONTEXT` | Se respetan en la detección automática |
| `DOCKER_CONFIG` | Carpeta de la configuración de Docker: contexto y credenciales de registro (`config.json`) |

## Funcionamiento sin Internet

La aplicación no necesita Internet: las fuentes y recursos van empaquetados, la
política de seguridad de la ventana bloquea cualquier conexión externa y Docker
se usa a través del socket local (y, para los servidores remotos que añadas, por
SSH hasta cada servidor). La única conexión propia de la aplicación es la
comprobación de versiones nuevas (Configuración → Actualizaciones), que está
desactivada mientras no indiques una fuente y la uses. Solo las operaciones que **Docker Engine** hace
contra registros remotos necesitan que Docker tenga red: descargar imágenes,
buscar en Docker Hub y construir con imágenes base que no estén en el equipo.

## Credenciales de registros privados

La aplicación no guarda credenciales de registros. Con la opción "Las de Docker" usa las que
guardó `docker login` (en `~/.docker/config.json` o en el *credential helper*
configurado allí, como `docker-credential-secretservice` o
`docker-credential-pass`). Si el helper no está instalado, la descarga sigue sin
credenciales y se avisa. También puedes escribir usuario y contraseña para una
sola descarga.

En un **servidor remoto** (1.2.0), las credenciales de este equipo solo se envían
al Docker del servidor si marcas la casilla **Enviar al servidor … las
credenciales de registro de este equipo** en esa descarga o construcción; sin
marcarla, el servidor descarga de forma anónima. Para registros privados en un
servidor, lo más seguro es hacer `docker login` en el propio servidor (Docker
Compose en el servidor usa siempre esa configuración).
