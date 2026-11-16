# Integración con Docker

## Docker Engine API

CMC trabaja con la **API HTTP de Docker Engine**, la misma que usa la CLI de
Docker. Exige la versión **1.41 o posterior** (Docker Engine 20.10); con una
versión anterior la conexión se rechaza con el diagnóstico «API demasiado
antigua». La regresión automática se ejecuta con Docker **29.9.2** (API 1.56),
**26.1.3** (API 1.45) y **24.0.7** (API 1.43).

![Caminos de comunicación entre el motor y Docker.](diagramas/03-docker.pdf){width=95%}

La mayoría de las operaciones pasan por **bollard 0.21**. Las que necesitan un
control fino de la conexión (construcción de imágenes con su progreso,
suscripción a eventos, tamaños de volúmenes, creación de redes con su cuerpo
exacto) usan un *dialer* propio que abre una conexión nueva por el mismo
transporte (`client.rs`, `Dialer`).

Cada petición queda en el panel de actividad con su **ruta real**, por
ejemplo `POST /containers/<id>/stop?t=10` sobre `/var/run/docker.sock`, y la
orden equivalente (`docker stop --time 10 <nombre>`), que es orientativa: la
aplicación no la ejecuta.

## Docker CLI

La CLI de Docker solo se usa para lo que **no tiene API**:

- **Docker Compose** (`docker compose …` o `docker-compose …`), siempre con el
  programa y los argumentos por separado, sin shell;
- en los servidores remotos, `docker system dial-stdio` (por SSH), que abre
  un canal con el Docker del servidor;
- las credenciales de los registros, con el *credential helper* configurado en
  `~/.docker/config.json` (`docker-credential-…`).

## Docker Compose

`compose/mod.rs` detecta Compose: primero el plugin (`docker compose
version`), después el binario `docker-compose` (versión 2 o posterior). En
Windows también se busca en la carpeta de Docker Desktop. Si no hay Compose,
el apartado explica cómo instalarlo y el resto de la aplicación funciona.

Los proyectos se reconocen por las **etiquetas** que Compose pone en los
contenedores (`com.docker.compose.project`, `…working_dir`,
`…config_files`), así que aparecen también los desplegados desde la terminal.
Las acciones (`up`, `down`, `start`, `stop`, `restart`, `pull`, `build`,
`logs`) se ejecutan como procesos con salida en vivo; `docker compose config
--format json` da el modelo del proyecto para la vista de servicios y para
detectar riesgos antes de desplegar.

## Conexión mediante sockets

| Sistema | Transporte | Ruta |
|---|---|---|
| Linux, Docker del sistema | Socket Unix | `/var/run/docker.sock` |
| Linux, Docker rootless | Socket Unix | `$XDG_RUNTIME_DIR/docker.sock` (o `/run/user/<uid>/docker.sock`) |
| Windows, Docker Desktop | Tubería con nombre | `\\.\pipe\docker_engine` y `\\.\pipe\dockerDesktopLinuxEngine` |
| Servidor remoto | Canal SSH | `ssh … docker system dial-stdio` |

`DOCKER_HOST=tcp://…` **no se admite** (Docker sin TLS por TCP da control del
equipo a cualquiera que llegue al puerto): la aplicación lo detecta y explica
cómo usar el socket local o un servidor por SSH.

## Detección del daemon

![Flujo de conexión con el Docker local.](diagramas/04-conexion-local.pdf){width=45%}

En modo **automático**, `endpoint.rs` calcula los candidatos en el mismo
orden que la CLI de Docker, sin duplicados:

1. `DOCKER_HOST`, si está definido;
2. el **contexto activo** (`DOCKER_CONTEXT` o `currentContext` de
   `~/.docker/config.json`; el host del contexto se lee de
   `contexts/meta/<sha256(nombre)>/meta.json`);
3. en Linux, el socket **rootless** del usuario (si no es root);
4. el socket del sistema; en Windows, las dos tuberías de Docker Desktop.

En modo **personalizado** solo se prueba la ruta indicada (en Windows también
se admite la ruta de la tubería escrita tal cual).

`probe.rs` sondea cada candidato **sin enviar nada todavía**: si existe, si es
un socket, si hay permiso y si alguien escucha. Después `manager.rs` conecta
con el primero que responde y comprueba que es Docker (`ping` y versión de la
API).

## Contextos y `DOCKER_HOST`

- Un contexto con host `unix://…` o `npipe://…` se usa como candidato.
- Un contexto con host `ssh://…` o `tcp://…` no se usa para el equipo local:
  los servidores remotos se configuran en el apartado **Servidores**, con su
  identidad verificada y sus credenciales en el llavero.
- El diagnóstico indica de dónde salió cada candidato (`DOCKER_HOST`,
  contexto, rootless, sistema, personalizado).

## Conexiones remotas por SSH

![Conexión con un servidor remoto por SSH.](diagramas/05-conexion-ssh.pdf){width=95%}

1. **Perfil**: dirección, puerto, usuario, autenticación (agente, clave,
   clave con frase o contraseña), Docker del servidor (predeterminado, socket o
   contexto) y opciones (tiempo de espera, `ProxyJump`, conectar al iniciar).
2. **Identidad del servidor** (`ssh/hostkey.rs`): la primera vez se muestra la
   huella `SHA256:…` y se guarda en el `known_hosts` **de la aplicación** solo
   si el usuario la confirma; si cambia, la conexión se detiene. Nunca se
   modifica `~/.ssh/known_hosts` del usuario.
3. **Secretos** (`ssh/askpass.rs`): `ssh` se lanza con `SSH_ASKPASS` apuntando
   al propio ejecutable de la aplicación, que pide el secreto al proceso
   principal por un socket privado (Linux) o por `127.0.0.1` con un testigo de
   256 bits (Windows) y solo responde a la pregunta esperada.
4. **Sesión**: en Linux, una sesión maestra de OpenSSH (`ControlMaster`) y un
   canal por conexión; en Windows (sin `ControlMaster`), un `ssh` directo por
   canal con las mismas opciones.
5. **Docker remoto**: bollard habla HTTP por canales `docker system
   dial-stdio`. Docker **nunca** se expone en la red.
6. **Compose remoto**: `ssh … -- docker compose …` con los archivos del
   servidor.

La prueba de conexión hace **siete comprobaciones**: DNS, conexión SSH,
identidad del servidor, autenticación, Docker disponible, permisos y
compatibilidad de la API.

## Reconexión

- **Latido**: cada 10 s se comprueba la conexión (sin registrar cada
  comprobación en el panel de actividad).
- Si falla, el estado pasa a **desconectado** con diagnóstico, se registra la
  comprobación que detectó la pérdida y se reintenta con espera creciente de
  **2 s a 30 s**.
- Los problemas que necesitan al usuario (identidad cambiada, credenciales) no
  se reintentan solos.
- Al volver la conexión se invalidan todas las consultas de la interfaz y la
  suscripción a eventos se reanuda.

## Diagnóstico

Sin conexión, el **Dashboard** muestra la causa más relevante con pasos
concretos y las órdenes para copiar (la aplicación **nunca** las ejecuta):

| Problema | Ejemplo de solución propuesta (Linux) | En Windows |
|---|---|---|
| Docker no instalado | `sudo apt install docker.io` o el repositorio oficial | Instalar Docker Desktop |
| Daemon detenido | `sudo systemctl enable --now docker` | Abrir Docker Desktop |
| Sin permiso en el socket | `sudo usermod -aG docker <usuario>` (con el aviso de que equivale a root) o Docker rootless | `net localgroup docker-users "<usuario>" /add` |
| `DOCKER_HOST` no admitido | `unset DOCKER_HOST; docker context use default` | `Remove-Item Env:DOCKER_HOST; docker context use default` |
| Tiempo agotado | `sudo systemctl restart docker` | Reiniciar Docker Desktop |
| API antigua | Actualizar Docker Engine a 20.10 o posterior | Actualizar Docker Desktop |

El diagnóstico también lista los candidatos probados con su resultado y lo que
se ha detectado instalado. Desde la terminal, `cmc-diagnose` (en
`crates/cmc-engine/src/bin/`) muestra lo mismo.

## Diferencias entre Linux y Windows

| Aspecto | Linux | Windows |
|---|---|---|
| Docker local | Docker Engine (sistema o rootless) | Docker Desktop (motor de Linux) |
| Transporte | Socket Unix | Tubería con nombre (con reintento si está ocupada) |
| Rutas del equipo | `/…` | `C:\…`, `\\servidor\recurso\…` |
| Rutas de contenedores y servidores | `/…` | `/…` (también son Linux) |
| SSH | OpenSSH con `ControlMaster` | OpenSSH de Windows, un `ssh` por canal; sin `ProxyJump` |
| Askpass | Socket Unix privado | `127.0.0.1` con testigo de 256 bits |
| Detener procesos | Grupo de procesos (SIGTERM, SIGKILL) | *Job object* (`TerminateJobObject`) |
| Contenedores de Windows | — | No admitidos |
