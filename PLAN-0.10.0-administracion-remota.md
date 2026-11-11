# Plan técnico · Administración remota y multiservidor (versión 0.10.0)

> Estado: **aprobado el 2026-11-11**, con contraseña SSH incluida (desaconsejada) y la terminal SSH del servidor incluida ya.
> Base: 0.9.2 (commit `93532f6`). La Fase 10 no se inicia.

## 0. Qué hay hoy (inspección)

| Pieza | Situación actual | Consecuencia para lo remoto |
|---|---|---|
| Conexión | Una sola conexión con Docker (D-004): `AppState.engine: EngineManager`, que crea `Engine` con `Docker::connect_with_unix`. El monitor, el estado y el diagnóstico están en el gestor | Hay que pasar de **uno** a **un gestor por servidor** |
| IPC | 83 órdenes que obtienen el motor con `state.engine.engine()` y no reciben ningún servidor | **Todas** deben recibir un servidor explícito |
| Interfaz | Las claves de consulta (`keys.containers`, …) no incluyen el servidor | Si no se separan por servidor, las cachés se mezclarían |
| Bollard 0.21 | Tiene la función `ssh`, pero usa el crate `openssh`: no permite pasar opciones a `ssh`, acepta claves de host nuevas automáticamente (`KnownHosts::Add`) y abre una sesión maestra nueva por conexión. Ofrece `connect_with_custom_transport` | **No se usará** su `ssh`. Se usará un transporte propio con `connect_with_custom_transport` |
| Construcción de imágenes | `build/http.rs` abre el `UnixStream` del socket directamente | Necesita la misma abstracción de conexión |
| Compose | Ejecuta el CLI local con `DOCKER_HOST=unix://…` | No sirve tal cual para lo remoto: los archivos y las rutas serían los del equipo local |
| Terminales y flujos | Usan `exec` y `attach` por la API (con upgrade HTTP) | Funcionan por cualquier transporte que admita upgrade |
| Directorios vinculados | Leen permisos y existencia en el disco local | En remoto, solo los datos que da Docker |

## 1. Arquitectura de conexiones

```
AppState
 └─ servers: ServerRegistry        (nuevo; sustituye a `engine`)
     ├─ "local"  → EngineManager { endpoint: Local(socket) }   ← como hoy
     ├─ "srv-a"  → EngineManager { endpoint: Ssh(perfil A) }
     └─ "srv-b"  → EngineManager { endpoint: Ssh(perfil B) }
          cada uno con su monitor, estado, bloqueos, diagnóstico y cliente bollard
```

- **`Endpoint`** pasa a ser un enum `Local { socket }` o `Ssh { perfil }`, ampliable a otros transportes seguros. `Engine` guarda a qué servidor pertenece, con id, nombre y dirección.
- **Lógica de Docker**: se reutiliza **toda** (contenedores, imágenes, redes, volúmenes, edición, commit, flujos, eventos). Solo cambia cómo se abre la conexión, que se aísla en un único punto (`Engine::new`, más un `connect_stream()` para `build/http.rs`). No hay copias de la lógica para lo remoto.
- **Servidor local**: es el id fijo `local`, con la configuración y la detección de hoy, sin cambios de comportamiento.

## 2. Transporte SSH elegido

**El cliente OpenSSH del sistema (`ssh`) ejecuta `docker system dial-stdio` en el servidor.** Es el mismo mecanismo que usan `docker -H ssh://…` y los contextos Docker con SSH.

- **Conexión maestra**: hay una por servidor (`ControlMaster`, socket en `$XDG_RUNTIME_DIR/docker-control-center/ssh/<id>`, solo legible por el usuario).
- **Conexiones HTTP**: cada conexión de bollard es un canal multiplexado (`ssh -S <ctl> … -- docker system dial-stdio`), sin repetir la autenticación.
- **Integración con bollard**: un conector propio (`tower::Service<Uri>`) con el cliente `hyper-util`, pasado a `Docker::connect_with_custom_transport`. Las respuestas con upgrade (terminales, attach) siguen funcionando.

**Opciones fijas de `ssh`** (no configurables, porque protegen la conexión):

| Opción | Efecto |
|---|---|
| `StrictHostKeyChecking=yes` | No se conecta a un servidor con identidad desconocida o distinta |
| `UserKnownHostsFile=<archivo de la app> ~/.ssh/known_hosts` | Usa los hosts de confianza de la aplicación y los del usuario |
| `BatchMode` o `SSH_ASKPASS_REQUIRE=force` | Nunca se pide nada por terminal |
| `ConnectTimeout`, `ServerAliveInterval=15`, `ServerAliveCountMax=3` | Limitan la espera y detectan caídas |
| `ExitOnForwardFailure=yes` | Falla si no se puede establecer lo pedido |
| `ClearAllForwardings=yes`, `ForwardAgent=no`, `ForwardX11=no` | Sin reenvíos |

**Por qué el `ssh` del sistema y no una implementación en Rust** (russh):
- Respeta lo que ya tienes configurado: `~/.ssh/config`, `ProxyJump`, el agente, las llaves FIDO y `known_hosts`.
- No reimplementamos criptografía.
- Recibe las actualizaciones de seguridad de Debian.

**Coste**: `openssh-client` pasa a ser una dependencia (Recommends del paquete). Si falta, la sección Servidores lo explica, y lo local sigue funcionando.

**Sin shell local**: `ssh` se lanza con argumentos separados (`Command::args`), como todo lo demás.

**Orden remota**: es una constante (`docker system dial-stdio`). Si el perfil indica un socket o un contexto, se añade `--host unix:///ruta` o `--context nombre`, validados con una lista estricta de caracteres y entrecomillados POSIX. La función de entrecomillado se prueba con entradas maliciosas.

**Validación** (en Rust):
- **Host**: nombre DNS, IPv4 o IPv6.
- **Usuario**: `[a-z_][a-z0-9_.-]*`.
- **Puerto**: de 1 a 65535.
- **Nada empieza por `-`**, y se pone `--` antes del destino.
- **Opciones avanzadas**: solo de una lista cerrada (`ProxyJump` validado y tiempos de espera), nunca `-o` libres.

**Prohibido explícitamente**:
- `tcp://` sin TLS: no existe ese transporte en la aplicación y una prueba lo comprueba.
- Exponer o abrir puertos en el servidor.
- Tocar su configuración SSH.

## 3. Verificación de la identidad del servidor (host keys)

1. **Primera conexión**: `ssh-keyscan` obtiene las claves públicas del servidor y `ssh-keygen -lf` calcula su huella SHA256. La aplicación muestra el tipo y la huella, y pide confirmar comparándola con la que dé el administrador del servidor (con la orden `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` para obtenerla).
2. **Al confirmar**: la línea se añade al `known_hosts` de la aplicación (`~/.config/docker-control-center/known_hosts`, permisos 600). Nunca se modifica `~/.ssh/known_hosts`. Si el servidor ya estaba allí, se usa sin pedir nada.
3. **Conexiones siguientes**: con `StrictHostKeyChecking=yes`, si la clave cambia, `ssh` se niega. Se detecta («REMOTE HOST IDENTIFICATION HAS CHANGED» o código 255 con ese diagnóstico) y el servidor pasa a **«Identidad del servidor cambiada»**, con un aviso de posible ataque de intermediario. No se reintenta.
4. **Revisar la identidad**: es una acción explícita que muestra la huella guardada y la nueva, y exige confirmar de forma expresa para sustituirla, solo en el archivo de la aplicación. Si el cambio viene de `~/.ssh/known_hosts`, se explica cómo corregirlo (`ssh-keygen -R`), sin tocar ese archivo.

**Limitación honesta**: la primera vez es TOFU (se confía en la primera clave), igual que `ssh`. Por eso se muestra la huella para compararla.

## 4. Credenciales

**Métodos de autenticación**:

| Método | Cómo | Qué se guarda |
|---|---|---|
| Agente SSH del sistema | `SSH_AUTH_SOCK` heredado | Nada |
| Clave privada existente | `IdentityFile=<ruta elegida>` + `IdentitiesOnly=yes`. **La clave no se copia ni se lee**: solo se guarda su ruta | Ruta (en la configuración) |
| Clave con frase de paso | Igual que la anterior; la frase se entrega a `ssh` por `SSH_ASKPASS` (ver abajo) | Frase en el **keyring** (opcional; si no, se pide en cada conexión y solo vive en memoria) |
| Contraseña (aprobada, desaconsejada en la interfaz) | Mismo mecanismo `SSH_ASKPASS` (oficial de OpenSSH ≥ 8.4: Debian 12 tiene 9.2 y Ubuntu 22.04 tiene 8.9). Se desaconseja frente a claves | Contraseña en el **keyring** u otra vez cada vez |

**Cómo se entrega un secreto a `ssh`**:
- `SSH_ASKPASS` apunta al propio binario de la aplicación en modo «askpass», con `SSH_ASKPASS_REQUIRE=force`.
- Ese proceso pide el secreto a la aplicación por un socket Unix efímero (permisos 600, ruta aleatoria, válido para una sola conexión) y lo escribe en la salida que lee `ssh`.
- El secreto nunca va en argumentos, variables de entorno ni archivos.

**Keyring**: API Secret Service por D-Bus (GNOME Keyring o KWallet), con el crate `keyring` o `secret-service` sobre `zbus`, que ya está en el árbol de dependencias.
- Clave `docker-control-center/<id del servidor>`.
- Se borra al eliminar el perfil o con «Olvidar credenciales».
- **Si no hay keyring**: se avisa, no se guarda nada y se pide el secreto en cada conexión (solo en memoria). Nunca se cae en texto plano.

**Separación entre perfil y secretos**:
- El perfil (`config.toml`, sección `[[servers]]`) solo lleva datos no sensibles.
- Los secretos solo están en el keyring.
- El panel de actividad, el historial, el diagnóstico y el informe del sistema no muestran secretos. Hay pruebas que buscan los valores en todos ellos.

## 5. Modelo de datos

```toml
[[servers]]
id = "srv-7f3c…"            # generado; nunca cambia
name = "Producción A"
host = "docker-a.ejemplo.com"
port = 22
user = "deploy"
description = ""
auth = { method = "agent" }                 # | { method="key", path="~/.ssh/id_ed25519" }
                                            # | { method="keyPassphrase", path=… } | { method="password" }
docker = { target = "default" }             # | { target="socket", path="/run/user/1000/docker.sock" } | { target="context", name="…" }
options = { connectTimeout = 10, proxyJump = "" }
lastConnected = "2026-11-12T10:00:00Z"      # informativo
autoConnect = false
```

`local` no aparece en la lista: sigue configurado con `[docker]`, como ahora. Una configuración antigua, sin `servers`, carga igual.

## 6. Conexiones simultáneas

- **Ciclo de vida independiente** por servidor: desconectado → conectando → conectado, o error. Los errores posibles son: servidor inaccesible, SSH detenido, autenticación, identidad cambiada, Docker detenido, socket inaccesible, sin permisos y versión incompatible.
- **Monitor y reconexión**: cada servidor tiene su monitor y su reconexión controlada, con espera creciente y un límite. No se reconecta solo si el error es de identidad o de autenticación.
- **Tiempos de espera** en cada petición. Un servidor colgado no bloquea a otros, ni a lo local: cada gestor tiene su tarea y sus bloqueos.
- **Desconectar** cierra la conexión maestra y los flujos de ese servidor. No toca nada del servidor.
- **Cerrar la aplicación** cierra todas las conexiones, sin dejar procesos `ssh` huérfanos (prueba incluida).
- **Límite** razonable de servidores conectados a la vez (por ejemplo, 16), con aviso.

## 7. Integración con contenedores, imágenes, redes y volúmenes

- **Servidor explícito en cada orden IPC**: todas reciben `server: ServerId` como argumento **obligatorio**. Así, cada operación queda ligada a un servidor (requisito 7.3) y nunca depende de un estado global en el backend. Si el servidor no existe o no está conectado, devuelve un error claro.
- **Interfaz**:
  - Hay un «servidor activo» en el estado de la aplicación.
  - Todas las claves de consulta llevan el id del servidor delante, así que cambiar de servidor no mezcla cachés.
  - Cada mutación captura el servidor **en el momento de pedirla**, no al ejecutarla.
- **Lo que cambia en remoto**:
  - **Asistente de creación**: las imágenes, redes y volúmenes que propone son los del servidor. Las rutas de carpetas se escriben como **rutas del servidor**, sin selector de archivos local y con un aviso.
  - **Directorios vinculados**: muestra lo que informa Docker, sin comprobar permisos en el disco local.
  - **Construir imagen**: el contexto se lee del equipo local y se envía al Docker remoto, que es como funciona `docker build -H ssh://…`. Se indica claramente «se construye en el servidor X».
  - **Descargar imágenes**: lo hace el Docker remoto. Las credenciales «de Docker» son las del equipo local, enviadas como hoy; se indica.
- **Confirmaciones destructivas**: añaden un bloque con el **servidor (nombre y dirección)**, el recurso y la acción. Ninguna se elimina ni se duplica.

## 8. Compose remoto

- **Detección**: por las etiquetas de los contenedores del servidor (`com.docker.compose.project`, `working_dir`, `config_files`), como hoy pero con la API remota.
- **Ejecución en el servidor**, porque las rutas y los archivos son suyos:
  - Se lanza `ssh … -- docker compose --project-directory <dir> -f <archivo> … <acción>`, con cada argumento validado y entrecomillado POSIX.
  - La salida llega en directo como ahora, y se puede cancelar (cierra el canal; Compose recibe SIGHUP).
  - Requisito en el servidor: Compose v2. Se detecta con `docker compose version`.
- **Archivos**:
  - **Leer**: se descargan por el canal SSH (`cat -- <ruta>`) para verlos y editarlos.
  - **Guardar**: se pide confirmación mostrando el **servidor y la ruta de destino**. Se comprueba que no cambiaron desde que se leyeron (hash) y se escribe a un temporal en el mismo directorio y luego `mv`. Nunca se sobrescribe sin confirmar.
- **Rutas**: nunca se asume que un archivo local existe en el servidor. «Abrir proyecto» en remoto pide la ruta en el servidor.

## 9. Terminales remotas

- **Mismo mecanismo de hoy** (`exec` interactivo por la API) por el transporte SSH. Las 12 sesiones simultáneas pueden ser de servidores distintos.
- **Identificación**: cada panel muestra el servidor (nombre y distintivo local/remoto) y el contenedor.
- **Desconexión SSH**: la sesión termina con el estado «Conexión con el servidor perdida», sin afectar a las demás. Se puede reabrir al reconectar.
- **Cierre**: los recursos se liberan como hoy (probado).
- **Terminal SSH del propio servidor** (aprobada para esta versión): es un **tipo de sesión independiente** («Servidor»), distinto en icono, etiqueta y color de una terminal de contenedor.
  - Abre una shell del usuario en el host con `ssh -tt` por la misma conexión maestra, sin pedir otra vez la autenticación.
  - Para que el tamaño llegue al servidor, `ssh` se lanza sobre un **PTY local** (`openpty`); al redimensionar xterm se ajusta el PTY y `ssh` lo reenvía.
  - Antes de abrirla se recuerda que es una shell en el servidor, con los permisos de ese usuario.
  - Las mismas garantías de cierre: al cerrarla termina el proceso `ssh` de esa sesión y se registra la apertura y el cierre, nunca lo que se escribe.

## 10. Panel de actividad

- **Servidor en cada registro** (`server: { id, name, address, kind: local|ssh }`).
- **Transporte**: «API · SSH», «CLI · SSH» o «API · local».
- **Orden equivalente**: con `--host ssh://usuario@host:puerto` (o `docker --context` si se usa contexto).
- **Filtros**: se añade el filtro **Servidor** a los de tipo, operación, estado y fecha.
- **Historial persistente**: guarda el id y el nombre del servidor.
- **Sin secretos**: no se registra ninguno; una prueba busca los valores sembrados.

## 11. Interfaz

- **Navegación**: nueva sección **Servidores** en la barra lateral, con la lista, el asistente (*Añadir servidor*), los detalles y diagnóstico, la vista general multiservidor y la gestión de identidad y credenciales.
- **Selector de servidor**: está en la barra superior, junto al indicador de conexión, y muestra el nombre y el estado. Se maneja con el teclado.
- **Indicador permanente**: una franja o distintivo de «Remoto · nombre» en la cabecera de cada sección cuando no es local, en el tema y el color actuales.
- **Ventanas**: no se abren nuevas; todo va en el mismo patrón de páginas y diálogos.
- **Idiomas**: ocho, sin excepción, con la ayuda ampliada (sección nueva «Administración remota» y actualización de las afectadas).

## 12. Pruebas previstas

**Entorno de desarrollo** (no tu equipo):
- `openssh-server` y `openssh-client` instalados.
- Tres `sshd` en 127.0.0.1 (puertos 2201, 2202 y 2203), cada uno con su clave de host y un usuario no root distinto.
- Cada usuario ve un Docker Engine distinto (29, 26 y 24 a la vez, con sockets, raíces de datos y redes separadas). Si no fuera posible tenerlos simultáneos, el aislamiento se probaría con dos y se diría.
- Además, el Docker local.

**Pruebas deterministas** (sin red):
- Validación y entrecomillado (con entradas maliciosas).
- Construcción de los argumentos de `ssh`.
- Lectura de `known_hosts` y huellas.
- Clasificación de errores de `ssh` a partir de salidas reales.
- Migración de la configuración.
- Separación de la caché por servidor en la interfaz.
- Confirmaciones con el servidor.
- Filtros de actividad.

**Pruebas reales** (`DCC_DOCKER_TESTS=1` + `DCC_SSH_TESTS=1`):
- **Conexión**:
  - Primera conexión con huella y confirmación.
  - Rechazo sin confirmar.
  - Clave de host cambiada: se rechaza sin reintentos.
  - Autenticación con clave, con clave y frase de paso por askpass, y con agente.
  - Contraseña incorrecta.
  - Docker parado.
  - Usuario sin permisos sobre el socket.
  - API antigua.
  - Desconectar y reconectar.
  - Cambiar credenciales.
- **Mismas operaciones contra el servidor remoto**: se reutilizan las pruebas reales existentes ejecutándolas contra un `Engine` SSH, para demostrar que no hay lógica duplicada. Cubren crear, iniciar, detener, logs, inspección, terminal, editar y recrear, commit, descargar, red, volumen y Compose remoto.
- **Multiservidor**:
  - Un recurso creado en A no existe en B ni en el local.
  - Terminales de A y B abiertas a la vez.
  - Desconectar A no afecta a B.
  - Actividad etiquetada por servidor.
  - Una orden con el servidor equivocado falla y no actúa sobre otro.
- **Seguridad**:
  - Ningún secreto en la actividad, el historial, el diagnóstico ni los registros.
  - Nunca se usa `tcp://`.
  - No quedan procesos `ssh` huérfanos.
  - El archivo de hosts tiene permisos 600.

**Regresión** completa en Docker 29, 26 y 24, paquetes `.deb` normal y compatible, instalación y arranque real. Las comprobaciones manuales solo cubren lo remoto.

## 13. Riesgos técnicos y medidas

| Riesgo | Medida |
|---|---|
| Operación en el servidor equivocado | Servidor obligatorio en cada orden IPC; la mutación captura el servidor al pedirla; las confirmaciones lo muestran; pruebas de aislamiento |
| Inyección por argumentos de `ssh` o en la orden remota | Sin shell local; validación estricta; `--` antes del destino; orden remota constante o argumentos entrecomillados POSIX probados |
| Aceptar una identidad falsa | `StrictHostKeyChecking=yes`; confirmación con huella; identidad cambiada bloquea y avisa |
| Fuga de secretos | Keyring; askpass por socket efímero; nunca en argumentos ni entorno; pruebas de búsqueda de secretos |
| Servidor colgado que bloquea la aplicación | Tiempos de espera, `ServerAlive`, tareas independientes por servidor |
| Latencia por SSH | Conexión maestra multiplexada, reutilización de conexiones de hyper y consultas como hoy |
| Sin keyring en el sistema | Aviso y petición en cada conexión, sin texto plano |
| `docker` no instalado en el servidor o sin permisos | Diagnóstico con instrucciones; no se instala ni se cambian permisos |
| Pertenecer al grupo `docker` del servidor equivale a root | Aviso en el asistente y en la ayuda |
| Tamaño del cambio (83 órdenes, todas las vistas) | Por etapas con pruebas en cada una; lo local se prueba en cada etapa con la regresión completa |

## 14. Etapas de implementación

1. **Transporte y conexión** (backend): `Endpoint` enum, conector SSH para bollard y para `build/http.rs`, `known_hosts`, askpass y keyring, clasificación de errores, prueba de conexión en 7 pasos. Pruebas reales con `sshd`.
2. **Varios servidores**: `ServerRegistry`, perfiles en la configuración, servidor en las 83 órdenes, servidor en la actividad, aislamiento. La regresión local debe seguir intacta.
3. **Interfaz**: sección Servidores, asistente, prueba de conexión, identidad, selector, indicador remoto, vista general, diagnóstico y confirmaciones con servidor.
4. **Compose remoto, terminales y ajustes**: carpetas, directorios vinculados y construcción.
5. **Idiomas, ayuda y documentación**: ocho idiomas, ayuda, decisiones D-075 y siguientes, guía, script `verify-update-0.10.0.sh`.
6. **Verificación y entrega**: regresión en tres daemons, paquetes, instalación e informe.

**Versión**: **0.10.0**, porque es una funcionalidad grande que cambia la arquitectura de conexión. La 1.0 queda para la Fase 10.

## 15. Requisitos de cada servidor remoto

- **Servidor SSH** accesible: puerto abierto en el cortafuegos de la red (la aplicación no lo abre).
- **Docker Engine 20.10 o posterior** (API 1.41+) **con el CLI `docker`** instalado. `docker system dial-stdio` lo incluye desde la 18.09.
- **Usuario con acceso al socket de Docker**: grupo `docker` (equivale a root en ese servidor) o Docker rootless (se indica su socket en el perfil).
- **Para Compose remoto**: Docker Compose v2 en el servidor.
- **Autenticación**: clave pública del usuario en `~/.ssh/authorized_keys` del servidor (recomendado), o contraseña si el servidor la permite.
