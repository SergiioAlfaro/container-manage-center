# Instalación y requisitos

Hay dos paquetes de Linux de la misma versión y un instalador de Windows (en
pruebas):

| Archivo | Para |
|---|---|
| `container-manage-center_1.9.1_amd64.deb` | Debian 13 y Ubuntu 24.04 o posteriores (glibc 2.39 o posterior) |
| `container-manage-center_1.9.1_amd64-compat-debian12-ubuntu2204.deb` | También Debian 12 y Ubuntu 22.04 (glibc 2.34 o posterior) |
| `Container Manage Center_1.9.1_x64-setup.exe` | Windows 10 y 11 de 64 bits — **solo pruebas** |
| `SHA256SUMS` | Sumas para comprobar los archivos |

## Debian 13

### Requisitos

- Debian 13 de 64 bits (amd64).
- **Docker Engine 20.10 o posterior** (API 1.41) y acceso de tu usuario a su
  socket. Debian 13 trae `docker.io`; también vale el repositorio oficial de
  Docker.
- **Docker Compose v2** para el apartado Compose: `sudo apt install
  docker-compose` (Compose 2.26 en Debian 13) o `docker-compose-plugin` del
  repositorio de Docker.
- Opcional: un almacén Secret Service (GNOME Keyring o KWallet) para recordar
  contraseñas de servidores; `pkexec` para desinstalar desde la aplicación;
  Trivy o Grype para analizar vulnerabilidades.

### Instalación

Con el script, desde la carpeta de los paquetes y con tu usuario normal:

```bash
bash install.sh            # elige el paquete, comprueba SHA256SUMS e instala con apt
bash install.sh --check    # solo comprueba el sistema y qué paquete usaría
```

El script comprueba el sistema, elige el paquete según la glibc, verifica la
suma SHA-256 (si no coincide **no instala nada**), simula la instalación,
instala con `sudo apt-get install ./paquete.deb` y comprueba, sin cambiar
nada, Docker, tu acceso al socket, Compose, el cliente SSH y el almacén de
contraseñas. **Nunca** instala Docker ni te añade al grupo `docker`.

A mano: `sudo apt install ./container-manage-center_1.9.1_amd64.deb`.

### Dependencias

`apt` instala las de ejecución: `libwebkit2gtk-4.1-0`,
`libjavascriptcoregtk-4.1-0`, `libgtk-3-0`, `libsoup-3.0-0`, `libglib2.0-0`,
`libgdk-pixbuf-2.0-0`, `libcairo2`, `libdbus-1-3`, `libgcc-s1` y `libc6` (con
las versiones mínimas que declara el paquete), y recomienda `openssh-client`.

### Configuración del acceso a Docker

La aplicación necesita que **tu usuario** pueda abrir el socket de Docker:

```bash
# Opción recomendada por seguridad: Docker rootless
systemctl --user enable --now docker

# Opción habitual: grupo docker (equivale a privilegios de administrador)
sudo usermod -aG docker "$USER"   # y cerrar la sesión y volver a entrar
```

### Primera ejecución

Abre **Container Manage Center** desde el menú de aplicaciones (o
`container-manage-center` en una terminal), **sin sudo**: la aplicación se
niega a ejecutarse como root. Al arrancar busca Docker sola y el indicador de
la barra superior muestra `Docker 29.x` en verde si conecta.

### Conexión con Docker

Si no conecta, el **Dashboard** muestra el diagnóstico con la causa y los
pasos. En **Configuración › Conexión con Docker** se puede fijar un socket
concreto y **Probar conexión** sin guardar. Desde una terminal,
`cmc-diagnose` (compilado desde el código) muestra el mismo diagnóstico.

## Ubuntu

### Requisitos

- Ubuntu 24.04 (paquete normal) o 22.04 (paquete compatible), 64 bits.
- Docker Engine 20.10 o posterior: `docker.io` de Ubuntu o el repositorio
  oficial.
- Docker Compose v2: `sudo apt install docker-compose-v2` (o
  `docker-compose-plugin` del repositorio de Docker).

### Instalación y dependencias

Igual que en Debian: `bash install.sh` elige el paquete normal en 24.04 y el
compatible en 22.04. Las dependencias de ejecución las instala `apt`.

### Configuración y primera ejecución

Igual que en Debian (acceso al socket, primera ejecución sin sudo, diagnóstico).

### Desinstalación en Linux

`bash install.sh --uninstall`, **Configuración › Desinstalación y
componentes** o `sudo apt remove container-manage-center`. Se conservan la
configuración (`~/.config/container-manage-center/`), el historial
(`~/.local/state/container-manage-center/`) y las contraseñas del llavero.
Docker y sus datos no se tocan.

## Windows

> **En pruebas.** La versión de Windows **no se ha ejecutado todavía en un
> Windows real** (guía de la primera prueba:
> `docs/validation/VALIDACION-WINDOWS.md`). El cargador de WebView2 que lleva
> el instalador está cubierto por el permiso adicional de la GPL v3 del
> titular (`docs/legal/WEBVIEW2_EXCEPTION.md`).

### Requisitos

| | Requisito |
|---|---|
| Sistema | Windows 10 u 11 de 64 bits (x86_64) |
| Docker local | **Docker Desktop** con el motor de Linux. Según la documentación de Docker: Windows 10 22H2 (19045) o Windows 11 23H2 (22631) o posterior, ediciones Pro, Enterprise o Education, 8 GB de RAM y virtualización activada |
| Servidores remotos | Cliente OpenSSH de Windows (`ssh.exe`, Windows 10 1809 o posterior) |
| Navegador interno | WebView2 Runtime (incluido en Windows 11; el instalador lo descarga si falta) |

### Docker Desktop y WSL 2

Docker Desktop ejecuta Docker en una máquina virtual Linux, con el *backend*
de **WSL 2** (WSL 2.1.5 o posterior) o de **Hyper-V**. CMC habla con él por
sus tuberías con nombre (`\\.\pipe\docker_engine` y
`\\.\pipe\dockerDesktopLinuxEngine`), con cualquiera de los dos *backends*.
No se admiten: los contenedores de Windows, un Docker Engine instalado dentro
de WSL sin Docker Desktop, ni `DOCKER_HOST=tcp://…`.

### Instalación

1. Comprueba el instalador:
   `Get-FileHash .\<instalador>.exe -Algorithm SHA256` y compáralo con
   `SHA256SUMS`.
2. Ejecútalo. Como **no está firmado**, SmartScreen avisará: «Más
   información › Ejecutar de todas formas».
3. Se instala para tu usuario, sin permisos de administrador, en
   `%LOCALAPPDATA%\Container Manage Center`, con acceso en el menú Inicio.

### Primera ejecución y conexión

Abre Docker Desktop y espera a que el motor esté en marcha; después abre
**Container Manage Center** con tu usuario normal: como administrador
(«Ejecutar como administrador») no arranca, avisa y se cierra, igual que en
Linux con root. Si no conecta, el diagnóstico muestra la causa
con instrucciones de Windows (abrir Docker Desktop, añadir tu usuario al
grupo `docker-users`, quitar `DOCKER_HOST`…).

### Solución de problemas

| Síntoma | Qué hacer |
|---|---|
| «Docker Desktop no está en marcha» | Abrir Docker Desktop y esperar al motor; pulsar **Reintentar** |
| «Sin permiso para usar Docker Desktop» | `net localgroup docker-users "<usuario>" /add` como administrador y volver a iniciar sesión |
| «Host no admitido» | Quitar `DOCKER_HOST` del entorno (también de las variables de entorno de Windows) |
| Falta `ssh` | Configuración › Sistema › Características opcionales › «Cliente OpenSSH» |
| Clave SSH rechazada por permisos | `icacls "<ruta>" /inheritance:r /grant:r "$($env:USERNAME):R"` |
| SmartScreen bloquea el instalador | «Más información › Ejecutar de todas formas» (no está firmado) |

### Desinstalación en Windows

Configuración de Windows › Aplicaciones › «Container Manage Center» ›
Desinstalar. El desinstalador quita las copias programadas propias del
Programador de tareas (no al actualizar). Se conservan los datos
(`%APPDATA%\container-manage-center`, `%LOCALAPPDATA%\container-manage-center`),
las copias y las credenciales guardadas.
