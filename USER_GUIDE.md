# Guía de usuario

> Versión 1.0.0. (La nota original de la Fase 2 se conserva abajo por historia.)
>
> Versión 0.2.1 (Fase 2). Funcionan la conexión con Docker, el Dashboard, los
> contenedores (consultar, iniciar, detener, reiniciar, pausar, reanudar y
> eliminar), las imágenes (consultar) y la configuración. El resto de apartados
> indican en qué fase llegan.

## La ventana principal

1. **Barra lateral (izquierda)**: los apartados. Se contrae a iconos con el botón inferior o Ctrl+B.
2. **Área de trabajo (centro)**: el apartado seleccionado.
3. **Barra superior**: título del apartado, **indicador de conexión** y botón **Actividad**.
4. **Panel de actividad (derecha)**: se abre y cierra con el botón o Ctrl+Mayús+A.

## Indicador de conexión

| Aspecto | Significado |
|---|---|
| Punto verde y `Docker 29.x` | Conectado. Al pasar el ratón se ve el socket. `rootless` si el daemon es rootless |
| `Conectando con Docker…` | Detectando o conectando |
| Rojo, `Sin conexión con Docker` | No hay conexión. Púlsalo para ver el diagnóstico |

La aplicación vigila la conexión y reconecta sola cuando Docker vuelve a estar
disponible.

## Servidores remotos por SSH

Desde **Servidores** se administra el Docker de otros equipos Linux a los que
tienes acceso por SSH, a la vez que el de este equipo. La conexión usa el
cliente SSH del sistema (`ssh`) y ejecuta `docker system dial-stdio` en el
servidor: Docker **nunca** se expone en la red (nada de TCP 2375) y no se abre
ningún puerto.

**Requisitos del servidor** (detalle en INSTALL.md): servidor SSH en marcha,
Docker Engine 20.10 o posterior (API 1.41) con su CLI, y un usuario que pueda
usar Docker. Para Compose, `docker compose` o `docker-compose` en el servidor.
La aplicación **no instala Docker, no cambia permisos, no toca la configuración
SSH ni abre puertos**: si falta algo, te dice qué y cómo resolverlo.

### Añadir un servidor

1. **General**: nombre, dirección (nombre DNS, IPv4 o IPv6), usuario, puerto
   (22) y descripción.
2. **Autenticación**: agente SSH (recomendado), clave privada, clave con frase o
   contraseña (desaconsejada). La clave privada **nunca se copia**: solo se
   guarda su ruta.
3. **Docker**: el predeterminado del servidor, un socket concreto o un contexto
   de Docker; opciones avanzadas: tiempo de espera, salto (`ProxyJump`) y
   conectar al iniciar la aplicación.
4. **Probar conexión**: siete comprobaciones (DNS, conexión SSH, identidad del
   servidor, autenticación, Docker disponible, permisos, compatibilidad de la
   API). Cada problema indica la causa y cómo resolverlo.

La primera vez se muestra la **huella** del servidor (`SHA256:…`): compárala con
la del servidor y confírmala. Si la huella cambia más adelante, la conexión se
detiene con un aviso de posible **suplantación**; solo se acepta la nueva con
**Revisar identidad** y una confirmación expresa. La aplicación usa su propia
lista de huellas y nunca modifica tu `~/.ssh/known_hosts`.

Con un **servidor intermedio** (`ProxyJump`), la identidad del intermedio la
comprueba `ssh` con tu `~/.ssh/known_hosts` y tu configuración de SSH: si no la
conoce o ha cambiado, la conexión se rechaza (la aplicación nunca la acepta por
su cuenta). Conéctate antes una vez en un terminal con
`ssh -J usuario@intermedio usuario@servidor` para comprobarla y guardarla.

### Credenciales

La frase de la clave o la contraseña se piden al conectar. Si marcas
**Recordar**, se guardan en el almacén seguro del sistema (GNOME Keyring,
KWallet…); si no hay ninguno, la aplicación lo indica y las pide cada vez.
Nunca se guardan en archivos ni aparecen en el panel de actividad o en los
diagnósticos. **Olvidar credenciales guardadas** las borra del almacén.

### Trabajar con varios servidores

- El **selector** de la barra superior indica en qué servidor trabajas y su
  estado; al cambiar, toda la pantalla pasa a ese servidor. Los servidores
  remotos se distinguen del local.
- **Vista general**: todos los servidores con su estado, versión de Docker,
  contenedores en ejecución y detenidos, y recursos. Desde ella se abre un
  servidor.
- En un servidor remoto funcionan el Dashboard, contenedores (incluida la
  edición), imágenes, redes, volúmenes, carpetas vinculadas, eventos, terminales
  y Compose como en local. Las rutas (archivos Compose, carpetas de los bind
  mounts) son **del servidor**: se escriben a mano y se comprueban allí; antes
  de escribir un archivo en el servidor se confirma el destino.
- Cada operación va al servidor en el que la iniciaste, y las confirmaciones
  muestran el **servidor y su dirección**. Un servidor caído no afecta a los
  demás.
- **Diagnóstico** distingue servidor inaccesible, SSH detenido, credenciales
  incorrectas, huella cambiada, Docker detenido, socket inaccesible, falta de
  permisos y versión incompatible.
- **Desconectar** cierra la sesión SSH y sus terminales. **Eliminar perfil**
  borra solo los datos de la aplicación (perfil, credenciales e identidad
  guardada). Ninguna de las dos toca contenedores, imágenes, redes, volúmenes
  ni datos del servidor.

Pertenecer al grupo `docker` del servidor equivale a tener acceso de **root**
en él: decídelo con su administrador; Docker rootless es la alternativa más
segura.

## Dashboard

Con conexión muestra datos obtenidos de Docker en ese momento: versión de Docker
Engine y de la API, socket y origen de la conexión, número de contenedores (en
ejecución, en pausa, detenidos), imágenes, datos del equipo (sistema, núcleo,
CPU, memoria), almacenamiento y ejecución (controlador, directorio de datos,
cgroups, logs) y los avisos del daemon. **Actualizar** vuelve a pedirlos.

Sin conexión muestra el **diagnóstico**: la causa concreta, los pasos para
resolverla (con las órdenes para copiar en una terminal), los sockets que se
han comprobado y su resultado, y lo que se ha detectado instalado. Los botones
**Volver a conectar** y **Configurar conexión** están en la cabecera.

## Contenedores

Lista todos los contenedores (como `docker ps -a`): estado, nombre e ID corto,
proyecto Compose si lo tiene, imagen, estado de Docker, puertos publicados y
antigüedad. Puedes filtrar por **Todos / En ejecución / Detenidos** y buscar por
nombre, imagen, ID o proyecto.

Acciones de cada fila (solo aparecen las válidas para su estado):

| Estado | Acciones |
|---|---|
| En ejecución | Detener, Reiniciar, Pausar, Eliminar |
| En pausa | Reanudar, Detener, Eliminar |
| Creado / Detenido | Iniciar, Eliminar |

Detener y reiniciar esperan 10 s a que el proceso termine antes de forzarlo
(como Docker). Tras cada acción, la aplicación pregunta a Docker el estado real
del contenedor (verás "Comprobar estado del contenedor" en el panel) y lo muestra
de inmediato. Mientras una acción está en curso, su fila muestra la operación y
el resto de la aplicación sigue funcionando. Si Docker rechaza una acción, el
mensaje de Docker aparece arriba de la tabla y en el panel de actividad.

**Eliminar** pide confirmación. Si el contenedor está en ejecución o en pausa,
hay que marcar **Detenerlo a la fuerza y eliminarlo**. Opcionalmente se
eliminan sus volúmenes anónimos; los volúmenes con nombre y las carpetas
vinculadas no se tocan. No se puede deshacer.

Pulsa el **nombre** de un contenedor para abrir su detalle, o **Crear contenedor**
para abrir el asistente. El menú **⋯** de cada fila (o el botón derecho sobre
ella) ofrece **Editar configuración** y **Crear imagen a partir de este
contenedor**; se maneja también con el teclado (flechas, Inicio, Fin, Escape).

### Crear un contenedor

El asistente tiene seis pestañas; solo la imagen es obligatoria:

| Pestaña | Contenido |
|---|---|
| General | Nombre, imagen (debe estar en tu equipo; la lista sugiere las locales), comando, entrypoint y directorio de trabajo |
| Entorno | Variables de entorno y etiquetas |
| Red | Red, nombre de host, puertos publicados (IP, puerto del equipo, puerto del contenedor, protocolo), publicar todos los expuestos y entradas de /etc/hosts |
| Almacenamiento | Volúmenes (con nombre o anónimos) y carpetas de tu equipo (bind mounts), de lectura y escritura o solo lectura |
| Recursos | Límite de CPU, límite y reserva de memoria, política de reinicio |
| Avanzado | Usuario, TTY, STDIN abierto, proceso init, sistema de archivos de solo lectura, eliminar al terminar, modo privilegiado y capacidades |

- Al elegir una imagen local, los campos muestran como referencia su comando,
  entrypoint, directorio y usuario, y en Red puedes **publicar los puertos que
  expone**.
- **Comando** y **Entrypoint** se escriben como en una terminal
  (`nginx -g 'daemon off;'`) y se dividen en argumentos, que ves debajo. No se
  ejecuta ninguna shell: `$HOME` o `*` llegan tal cual.
- El panel **Equivalente** muestra en todo momento el `docker run` (o
  `docker create`) equivalente y el cuerpo que se enviará a la API. La
  aplicación no ejecuta ese comando: usa la API, y la operación queda en el
  panel de actividad.
- **Revisión** lista los errores (en rojo, impiden crear) y las advertencias (en
  ámbar: puertos privilegiados, carpetas delicadas montadas con escritura, modo
  privilegiado…). Cada pestaña muestra cuántos errores tiene; pulsa un problema
  para ir a su campo.
- Los valores de variables como `PASSWORD`, `TOKEN` o `SECRET` se ocultan en
  pantalla y en el panel de actividad (`********`), pero Docker los recibe
  completos y cualquiera con acceso a Docker puede leerlos.
- **Crear** crea el contenedor sin iniciarlo; **Crear e iniciar** también lo
  inicia. Si se crea pero no puede iniciarse (p. ej. un puerto ocupado), el
  detalle muestra el error de Docker y el contenedor queda creado.

### Detalle de un contenedor

Arriba, las acciones válidas para su estado, **Duplicar** y **Eliminar**.

- **Resumen**: estado, fechas, código de salida, proceso, puertos, variables
  (los valores sensibles se ocultan hasta pulsar *Mostrar*), política de
  reinicio, límites, redes, montajes y etiquetas.
- **Logs**: salida en directo (stdout normal, stderr en rojo). Puedes elegir
  cuántas líneas anteriores cargar, filtrar, ver las marcas de tiempo, seguir el
  final, copiar lo visible y dejar de seguir. Se conservan las últimas 10 000 líneas.
- **Terminal**: pulsa **Abrir terminal** para iniciar una shell (`bash` si existe,
  si no `sh`, o la que elijas) con `docker exec -it`. **Cerrar sesión** termina la
  shell y los procesos que dejó dentro del contenedor; la línea de estado indica
  cómo terminó. Para varias terminales a la vez, usa el apartado **Terminales**.
- **Estadísticas**: CPU, memoria, red, disco y procesos cada segundo, calculados
  como `docker stats`.
- **Inspección JSON**: la respuesta completa de `docker inspect`, con botón Copiar.

Logs, estadísticas y terminal son operaciones reales que aparecen en el panel de
actividad mientras están abiertas y se cierran al salir del detalle.

**Duplicar** abre el asistente con la configuración del contenedor (con nombre
`<nombre>-copia`) y avisa de los ajustes que no se trasladan.

### Editar la configuración

**Editar configuración** (menú ⋯ o cabecera del detalle) organiza la
configuración en pestañas: General, Red, Almacenamiento, Variables, Puertos,
Recursos y Avanzado. Cada campo indica cómo se aplica su cambio:

| Cambio | Cómo se aplica |
|---|---|
| Nombre, política de reinicio, CPU, memoria y memoria reservada | Directamente (`docker rename`, `docker update`), sin reiniciar |
| Conectar o desconectar redes | Directamente y al momento |
| Variables, puertos, volúmenes y carpetas, descripción, quitar un límite | Hay que **recrear** el contenedor |
| Imagen, comando, usuario y el resto de «Avanzado» | Solo lectura (Docker no permite cambiarlos): usa **Duplicar** |

Las pestañas con cambios pendientes llevan un punto, y la barra inferior resume
los cambios. **Revisar cambios** compara la configuración actual con la nueva
(qué cambia, qué se mantiene y qué exige recrear); no se aplica nada hasta que
confirmas.

**Recreación segura.** Si hace falta recrear, la revisión explica los pasos y:
- no deja continuar si no es seguro (contenedor de Compose o Swarm, creado con
  `--rm`, en pausa o reiniciándose, imagen que ya no existe);
- pide aceptar uno a uno los riesgos (archivos escritos fuera de los volúmenes,
  que no pasan al contenedor nuevo; montajes que quitas; contenedores que usan
  la red de este…) y una confirmación final;
- detiene el contenedor si está en ejecución, le cambia el nombre a
  `<nombre>-dcc-backup-<fecha>` (la copia), crea el nuevo con **toda** la
  configuración del original más tus cambios, conecta sus redes y comprueba que
  funciona;
- si algo falla, elimina el nuevo (sin sus volúmenes), devuelve el nombre al
  original y lo vuelve a iniciar.

Nunca se borran volúmenes, carpetas ni imágenes. La copia queda detenida en
**General › Copias de seguridad**, desde donde puedes **Restaurar** (el actual
se aparta, no se borra) o **Eliminar** cuando ya no la necesites.

### Crear una imagen a partir de un contenedor

Equivale a `docker commit`: guarda el estado actual del sistema de archivos del
contenedor como una imagen nueva. Indica repositorio y etiqueta (vacía =
`latest`) y, si quieres, autor y mensaje; se muestra el nombre completo antes de
crear. **No incluye los volúmenes ni las carpetas montadas** (el diálogo los
enumera). Sí guarda la configuración del contenedor, incluidas sus variables de
entorno (también las secretas): no la publiques en un registro si contiene
secretos. Si el contenedor está en ejecución, por defecto se pausa unos
segundos mientras se crea la imagen; nunca se detiene. Si ya existe una imagen
con ese nombre, hay que aceptarlo expresamente (la anterior se queda sin
etiqueta, no se borra). **Ver la imagen** abre su detalle en Imágenes.

## Imágenes

Lista las imágenes locales (como `docker image ls`): etiquetas, ID corto,
tamaño, antigüedad y qué contenedores las usan. Filtros **Todas / En uso /
Sin uso / Sin etiqueta** y búsqueda por etiqueta o ID. En cada fila: **Crear
contenedor** (abre el asistente con la imagen puesta), **Etiquetar** y
**Eliminar**. Pulsa el nombre para abrir su detalle.

### Descargar

**Descargar imagen** pide la referencia (`nginx:1.27`, `usuario/app`,
`registro.ejemplo.com:5000/app:v2`; sin etiqueta se usa `latest`) y la comprueba
con las mismas reglas que Docker. Credenciales:

| Opción | Qué hace |
|---|---|
| Las de Docker | Como `docker pull`: usa lo guardado con `docker login` (o su *credential helper*) para ese registro, si lo hay |
| Ninguna | Descarga anónima |
| Usuario y contraseña | Solo para esta descarga: no se guardan ni aparecen en el panel de actividad |

El panel **Descargas** muestra el progreso total y por capa; puedes cambiar de
apartado mientras tanto. **Cancelar** detiene la descarga en Docker.

### Buscar en Docker Hub

Busca como `docker search`, con la opción de ver solo imágenes oficiales. La
búsqueda la hace Docker Engine, que necesita acceso a Internet; si no lo tiene,
se muestra el error de Docker. **Descargar** en un resultado abre el diálogo de
descarga con el nombre puesto.

### Detalle de una imagen

Pestañas **Resumen** (ID, etiquetas, digests, fecha, tamaño, plataforma,
comando, entrypoint, variables —con los valores sensibles ocultos—, puertos,
volúmenes y metadatos LABEL), **Capas** (como `docker image history`),
**Contenedores** que la usan e **Inspección JSON**. Acciones: crear contenedor,
etiquetar, descargar de nuevo (para actualizarla desde su registro) y eliminar.

### Etiquetar, eliminar y limpiar

- **Etiquetar** añade un nombre (`repositorio:etiqueta`) a la imagen.
- **Eliminar**: si la imagen tiene varias etiquetas puedes quitar solo una; la
  imagen completa exige forzar. Si la usan contenedores, se listan antes de
  confirmar; Docker no elimina imágenes de contenedores en ejecución.
- **Limpiar** elimina las imágenes sin etiqueta que no usa ningún contenedor o,
  con confirmación adicional, todas las que no usa ningún contenedor.
  No se puede limpiar mientras hay descargas en curso: con el almacén de
  imágenes de containerd, Docker las haría fallar.

### Construir

**Construir imagen** abre el asistente:

1. **Contexto**: la carpeta que se envía a Docker (respetando `.dockerignore`) y
   el Dockerfile dentro de ella. A la derecha verás cuántos archivos y cuánto
   ocupan, cuántos excluye `.dockerignore` y si alguno no se puede leer.
2. **Nombres de la imagen** (`--tag`), **argumentos** (`--build-arg`; los
   valores de claves como `TOKEN` se ocultan, pero recuerda que quedan en el
   historial de la imagen) y **metadatos** (`--label`).
3. **Opciones**: etapa final, sin caché, buscar imágenes base nuevas y
   constructor (**BuildKit**, el de `docker build`, o el **clásico**).

Durante la construcción se ven los pasos con su estado y duración y la salida
como `docker build --progress=plain`. Puedes cancelarla o cambiar de apartado
(vuelves con **Ver progreso**). Al terminar: **Ver imagen**, **Crear
contenedor** o **Volver al formulario** con los mismos datos.

## Redes

Lista las redes de Docker (como `docker network ls`): nombre, ID corto, driver,
subredes y los contenedores que tienen configurada cada red (en marcha o no).
Las redes **bridge**, **host** y **none** las crea Docker («Predefinida») y no se
pueden eliminar. Filtros **Todas / En uso / Sin uso / Creadas por el usuario** y
búsqueda por nombre, ID, subred o contenedor. Pulsa el nombre para abrir su
detalle.

### Crear una red

**Crear red** abre el asistente:

- **Nombre** y **driver**: `bridge` (red privada en este equipo, la habitual),
  `macvlan` o `ipvlan` (salen por una interfaz física: indícala en la opción
  `parent`), `overlay` (requiere Docker Swarm) u otro driver de un plugin.
- **Red interna** (sin salida al exterior) y **conectable manualmente** (para
  overlay).
- **Direcciones IPv4**: subred (`172.30.0.0/24`), puerta de enlace y rango
  asignable. Son opcionales: sin subred, Docker elige una libre. Para poder dar
  a un contenedor una **IP fija** en la red, hay que indicar la subred aquí.
- **IPv6**: requiere que el sistema y Docker lo admitan.
- **Opciones del driver** (`--opt`) y **metadatos** (`--label`).

A la derecha se ven el comando `docker network create` equivalente, la petición
y su cuerpo exactos, y la revisión: los errores (subred mal escrita, puerta de
enlace fuera de la subred…) se marcan antes de enviar nada a Docker.

### Conectar y desconectar contenedores

**Conectar** (en la fila o en el detalle) elige un contenedor, en marcha o
detenido. Los que Docker no aceptaría aparecen desactivados con el motivo (ya
conectado, contenedor en modo host o sin red). Opcionalmente:

- **Alias**: nombres adicionales con los que los demás contenedores de la red lo
  encuentran (no disponibles en la red predefinida bridge).
- **IPv4/IPv6 fija** dentro de la subred de la red.

En la pestaña **Contenedores** del detalle se ve la IP y la MAC de cada
contenedor en marcha; los detenidos reciben la dirección al arrancar.
**Desconectar** pide confirmación: un contenedor en marcha pierde al instante la
conexión por esa red.

### Eliminar y limpiar

- **Eliminar**: Docker no elimina una red con contenedores en marcha conectados
  (el diálogo lo indica y no deja continuar). Si solo la tienen configurada
  contenedores detenidos, Docker **sí** la elimina y esos contenedores no podrán
  arrancar hasta que los desconectes o vuelvas a crear la red: el diálogo los
  lista y pide confirmarlo expresamente.
- **Limpiar** elimina las redes creadas por el usuario sin contenedores en
  marcha (como `docker network prune`), también las que solo usan contenedores
  detenidos, con el mismo aviso.

Los diálogos consultan a Docker el estado actual al abrirse, así que tienen en
cuenta los cambios hechos desde la terminal u otros programas.

### Detalle de una red

Pestañas **Resumen** (driver, ámbito, fecha, interna, IPv4/IPv6, subredes,
puertas de enlace, rangos, opciones del driver y metadatos), **Contenedores** e
**Inspección JSON** (como `docker network inspect`).

## Volúmenes

Lista los volúmenes (como `docker volume ls`) con su driver y los contenedores
que los montan, en marcha o no. Los volúmenes **anónimos** (los que Docker crea
sin nombre) se marcan como tales. **Calcular tamaños** pide a Docker el espacio
que ocupa cada uno (como `docker system df -v`); con volúmenes grandes puede
tardar, por eso no se hace automáticamente.

- **Crear volumen**: nombre (opcional: sin nombre se crea uno anónimo) y tipo:
  local (lo habitual), tmpfs (en memoria), NFS, una carpeta del equipo u otro
  driver. Las opciones se pueden editar y se ven el comando y la petición
  exactos. Si ya existe un volumen con ese nombre, se avisa: Docker devolvería
  el existente sin aplicar los cambios.
- **Detalle**: directorio en el equipo, opciones, metadatos, tamaño, contenedores
  con la ruta y el modo de acceso, e inspección JSON. **Crear contenedor con
  él** abre el asistente de contenedores con el volumen montado en `/data`.
- **Eliminar**: Docker no elimina un volumen que monte cualquier contenedor, ni
  siquiera uno detenido; el diálogo lo indica y no deja continuar.
- **Limpiar**: elimina los volúmenes anónimos sin uso o, con confirmación,
  todos los volúmenes sin uso (también los que tienen nombre). Con Docker
  anterior a la versión 23 la limpieza siempre elimina también los que tienen
  nombre; el diálogo lo avisa.

## Directorios vinculados

Carpetas (y archivos) de este equipo montados dentro de contenedores, agrupados
por ruta: tipo, permisos y propietario, si existen, qué contenedores los montan,
dónde y con qué acceso. Se señalan los que requieren atención: el **socket de
Docker** (da control de Docker al contenedor), la raíz o directorios del sistema,
y las rutas que ya no existen.

Docker no permite añadir una carpeta a un contenedor que ya existe. **Vincular
una carpeta…** abre el selector de carpetas del sistema y después el asistente
de contenedores con esa carpeta montada en `/mnt/<nombre>`; **Nuevo contenedor
con ella** hace lo mismo con una carpeta de la lista.

## Docker Compose

Necesita Docker Compose v2 o posterior (ver INSTALL.md); si no está instalado,
el apartado explica cómo instalarlo. Todas las operaciones ejecutan el Docker
Compose oficial y aparecen en el panel de actividad con el comando exacto.

La lista muestra los proyectos que tienen contenedores en Docker (también los
desplegados desde la terminal, que se detectan solos) y los que añades a la
lista, con su estado (en marcha, parcial, detenido o sin desplegar), carpeta y
servicios.

- **Abrir proyecto…**: elige su `compose.yaml` (si usa varios archivos, añádelos
  en el orden en que se combinan). El nombre lo decide Docker Compose (el `name:`
  del archivo o el nombre de la carpeta) salvo que escribas otro.
- **Nuevo proyecto**: elige una carpeta sin archivo Compose, escribe el contenido
  (se parte de una plantilla) y **Crear proyecto**: se valida con Docker Compose,
  se crea `compose.yaml` y se añade a la lista. Si escribes un nombre, se añade
  al archivo para que la terminal use el mismo.
- **Servicios**: cada servicio del archivo con su imagen (o si se construye),
  estado, contenedores, puertos y perfiles, más los contenedores de servicios
  que ya no están en el archivo. Cada fila permite desplegar, iniciar, detener,
  reiniciar o ver los registros de ese servicio.
- **Desplegar** (`docker compose up --detach`): todos los servicios o los
  elegidos, perfiles, construir antes, descargar imágenes (si faltan, siempre o
  nunca), volver a crear los contenedores y eliminar huérfanos. El diálogo
  muestra el comando exacto.
- **Iniciar, Detener, Reiniciar** actúan sobre todos los contenedores del
  proyecto, también los de servicios con perfil. **Descargar imágenes** y
  **Construir** preparan las imágenes sin desplegar.
- **Eliminar** (`docker compose down`) detiene y elimina los contenedores y las
  redes del proyecto; opcionalmente también sus volúmenes con nombre (sus datos
  se pierden) y sus imágenes. Hay que confirmarlo.
- **Salida**: la salida de la operación en curso o de la última, en directo.
  **Interrumpir** detiene Docker Compose; lo que ya hubiera hecho se mantiene.
  Solo puede haber una operación a la vez en cada proyecto.
- **Registros**: los de todos los servicios o los elegidos, en directo, con las
  últimas 100, 500 o 2000 líneas y marcas de tiempo opcionales.
- **Archivo**: edita los archivos del proyecto. **Validar** comprueba el
  contenido con Docker Compose sin guardarlo; **Guardar** valida antes y, si
  Compose lo rechaza, solo guarda si insistes. Si otro programa ha cambiado el
  archivo mientras lo editabas, no se sobrescribe.
- **Configuración**: la configuración final tal como la entiende Docker Compose.
  Puede contener valores secretos de las variables (por ejemplo del `.env`), por
  eso está oculta hasta que la pides y no se guarda en el panel de actividad.
- **Quitar de la lista** solo la quita de la lista (no toca Docker ni los
  archivos); **Añadir a la lista** conserva un proyecto desplegado desde la
  terminal aunque se eliminen sus contenedores.

Si los archivos de un proyecto ya no existen, se puede detener, iniciar,
reiniciar, ver sus registros y eliminar (Docker Compose usa las etiquetas de los
contenedores); para desplegarlo hacen falta los archivos.

## Terminales

Varias terminales reales (`docker exec -it`) a la vez, en el mismo contenedor o
en otros.

- **Nueva terminal**: elige un contenedor en ejecución, la shell (Automática,
  bash o sh) y, opcionalmente, un nombre para la sesión.
- Cada panel muestra el contenedor, el nombre de la sesión y su estado
  (Conectando, Conectada, Finalizada o Error), con botones para renombrar
  (también con doble clic en el nombre), maximizar y cerrar.
- Distribución: una terminal ocupa todo; dos, en columnas; tres, una a la
  izquierda y dos a la derecha; cuatro, en 2×2; con más, filas de dos con
  desplazamiento. Los separadores se arrastran o, con el foco, se mueven con las
  flechas (Inicio/Fin a los extremos, doble clic para repartir a partes iguales).
- Las sesiones siguen abiertas al cambiar de apartado. Hasta 12 a la vez.
  **Cerrar todas** pide confirmación.
- Cerrar una sesión termina su shell y los procesos que dejó dentro del
  contenedor. Si el contenedor se detiene o se elimina, el panel lo indica; una
  sesión terminada se puede volver a abrir si el contenedor sigue existiendo.
- Al cerrar la aplicación se terminan todas las sesiones.
- Con servidores remotos, cada panel indica el servidor y el contenedor, y se
  pueden tener terminales de varios servidores a la vez. **Shell SSH del
  servidor** abre una sesión en el propio servidor (no en un contenedor). Al
  desconectar un servidor se cierran sus terminales sin dejar procesos.
- El panel de actividad registra la apertura, el cierre y los errores, nunca lo
  que se escribe ni la salida.

## Eventos y registros

Docker anuncia cada cambio (un contenedor que se crea, arranca o se detiene,
una imagen descargada, una red conectada…) como un **evento**, venga de esta
aplicación, de la terminal o de cualquier otra herramienta. La aplicación los
recibe mientras está conectada y **actualiza al instante** las listas y el
panel afectados: no hace falta pulsar Actualizar.

- **En directo**: los eventos de esta sesión, los más recientes arriba. Filtra
  por tipo (contenedor, imagen, volumen, red…) o busca por nombre, ID, acción o
  imagen. **Pausar** congela la lista (los eventos se siguen recibiendo y se
  muestran al reanudar). Despliega un evento para ver todos sus datos y abre el
  recurso con su enlace.
- **Guardados**: los eventos recibidos mientras la aplicación estaba abierta,
  también de sesiones anteriores (se conservan los 20 000 más recientes), con
  filtros de fecha, acciones (`start, die`), tipo y texto.
- **Consultar a Docker**: pide a Docker los eventos de un periodo
  (`docker events --since … --until …`), con filtros de tipo, acciones y
  recurso. Incluye lo que pasó con la aplicación cerrada, pero **Docker solo
  conserva los últimos 256 eventos** y los pierde al reiniciarse. Se muestra el
  comando equivalente.
- **Historial de operaciones**: lo que la aplicación ha hecho en Docker, también
  en sesiones anteriores (las 10 000 más recientes), con filtros de fecha,
  categoría, estado y texto. Las de sesiones anteriores llevan la marca
  "Sesión anterior".

El historial guarda **qué** se hizo, **cuándo** y **con qué resultado**, pero
nunca el cuerpo de las peticiones, el comando equivalente ni las salidas, porque
pueden contener contraseñas (variables de entorno, argumentos de construcción).
Esos detalles solo están en el panel de actividad mientras la aplicación está
abierta. Los archivos están en `~/.local/state/docker-control-center/`, solo
legibles por tu usuario. Puedes borrar lo guardado desde cada pestaña o dejar de
guardar con la opción **Guardar historial** de Configuración.

## Panel de actividad

Cada operación que la aplicación ejecuta sobre Docker aparece aquí: hora,
distintivo **API** (petición a Docker Engine por el socket), **CLI** (proceso,
como `docker compose`) o **APP** (operación de la propia aplicación: cambio de
idioma, o el resumen de una recreación cuyos pasos aparecen aparte),
tipo de operación, recurso, duración y estado. La línea
principal es el comando `docker` equivalente, que puedes copiar.

Al desplegar una entrada ves la **operación ejecutada** de verdad (por ejemplo
`POST /containers/<id>/stop?t=10` sobre `/var/run/docker.sock`), el comando
equivalente (con la aclaración de que es orientativo cuando se usó la API), el
resultado, el error de Docker y su código HTTP si falló, y las salidas de los
procesos CLI.

Para no llenar el panel, las comprobaciones automáticas de la conexión (cada
10 s) no se registran una a una; sí se registra la comprobación que detecta una
pérdida de conexión. La suscripción a los eventos de Docker aparece una vez,
cuando Docker la acepta, y otra como fallida si se interrumpe (se reintenta sola
mientras haya conexión).

## Configuración

- **Apariencia**: tema (Sistema, Claro, Oscuro) e idioma, en este orden: English, Español (por defecto), Català, Galego, Euskara, Français, Italiano y Português. El idioma se guarda y sus cambios quedan en el panel de actividad. El color principal es el **morado**: desde la versión 1.0.0 no se elige (el verde azulado anterior ya no existe; si lo tenías, se cambia a morado conservando el resto de tu configuración). Los indicadores de estado no cambian: verde para lo que funciona o está en marcha, ámbar para avisos y rojo para errores.
- **Panel de actividad**: abrirlo automáticamente ante errores, número de operaciones conservadas (50–10.000) y **Guardar historial** (operaciones y eventos de Docker entre sesiones, sin secretos).
- **Conexión con Docker**:
  - **Automático**: busca `DOCKER_HOST`, el contexto activo, tu socket rootless y el del sistema.
  - **Socket personalizado**: una ruta absoluta concreta.
  - **Probar conexión** comprueba la configuración sin guardarla. **Guardar y conectar** la aplica. **Volver a conectar** repite la detección.
  - Conexión actual (socket, origen, versión, API, rootless) y **Docker Compose** (botón Comprobar, que ejecuta `docker compose version`). Si no está instalado lo indica y muestra cómo instalarlo; no es necesario para usar el resto de la aplicación.
- **Actualizaciones**: versión instalada (y si es el paquete .deb) y dónde se
  publican las versiones (un repositorio de GitHub, `propietario/repositorio`,
  o la dirección `https://` de un archivo JSON). **Comprobar ahora** consulta esa
  fuente y dice si hay una versión nueva, con sus novedades y un enlace a su
  página. **Comprobar al iniciar** lo hace cada vez que abres la aplicación y, si
  hay una versión nueva, la avisa arriba. La aplicación **nunca descarga ni
  instala nada**: para actualizar, instala el paquete nuevo con
  `sudo apt install ./docker-control-center_X.Y.Z_amd64.deb` (se conservan tu
  configuración y tu historial). Sin fuente no se conecta a Internet.
- **Sistema**: versiones de la aplicación, del paquete, de WebKitGTK y de glibc,
  el sistema operativo y el escritorio, y los datos de Docker y Compose.
  **Copiar** genera un informe en texto para comunicar un problema (sin
  proyectos, variables ni credenciales).
- **Desinstalación y componentes**: **Analizar componentes** consulta dpkg y APT
  (sin privilegios ni cambios) y muestra el paquete de la aplicación, las
  dependencias que se pueden quitar (las que APT instaló automáticamente junto
  con la aplicación y nadie más usa) y las que se conservan, con el motivo
  (compartidas, instaladas de forma explícita, ya existentes, de origen incierto o
  del sistema). Docker Engine, Docker CLI y Docker Compose se conservan siempre;
  tus datos y todo lo de Docker, también. Elige **Solo Docker Control Center** o
  también las dependencias, revisa la simulación de APT y la orden exacta y
  confirma: el sistema pide la contraseña de administrador (pkexec) solo para
  `apt-get remove`. Sin pkexec, se muestra la orden para ejecutarla a mano.
- **Ayuda y documentación**: guía de todas las funciones con índice y búsqueda, y
  el botón **Visitar mi perfil de GitHub**.
- **Restablecer las preferencias**: tema, idioma, panel de actividad, historial y
  actualizaciones vuelven a sus valores iniciales, tras confirmarlo. No cambia la
  conexión con Docker ni los proyectos Compose.
- **Atajos** de teclado.

Las fechas y horas de los registros (panel de actividad, eventos, historial y
logs) usan siempre el mismo formato de 24 horas.

Todo se guarda al momento en `~/.config/docker-control-center/config.toml`.

## Atajos de teclado

| Atajo | Acción |
|---|---|
| Ctrl+B | Contraer o expandir la barra lateral |
| Ctrl+Mayús+A | Abrir o cerrar el panel de actividad |
| Escape | Cerrar un diálogo |
