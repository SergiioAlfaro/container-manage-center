# Guía de usuario

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
para abrir el asistente.

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
  shell; la línea de estado indica cómo terminó.
- **Estadísticas**: CPU, memoria, red, disco y procesos cada segundo, calculados
  como `docker stats`.
- **Inspección JSON**: la respuesta completa de `docker inspect`, con botón Copiar.

Logs, estadísticas y terminal son operaciones reales que aparecen en el panel de
actividad mientras están abiertas y se cierran al salir del detalle.

**Duplicar** abre el asistente con la configuración del contenedor (con nombre
`<nombre>-copia`) y avisa de los ajustes que no se trasladan.

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

## Panel de actividad

Cada operación que la aplicación ejecuta sobre Docker aparece aquí: hora,
distintivo **API** (petición a Docker Engine por el socket) o **CLI** (proceso,
como `docker compose`), tipo de operación, recurso, duración y estado. La línea
principal es el comando `docker` equivalente, que puedes copiar.

Al desplegar una entrada ves la **operación ejecutada** de verdad (por ejemplo
`POST /containers/<id>/stop?t=10` sobre `/var/run/docker.sock`), el comando
equivalente (con la aclaración de que es orientativo cuando se usó la API), el
resultado, el error de Docker y su código HTTP si falló, y las salidas de los
procesos CLI.

Para no llenar el panel, las comprobaciones automáticas de la conexión (cada
10 s) no se registran una a una; sí se registra la comprobación que detecta una
pérdida de conexión.

## Configuración

- **Apariencia**: tema (Sistema, Claro, Oscuro) e idioma (Español, English).
- **Panel de actividad**: abrirlo automáticamente ante errores y número de operaciones conservadas (50–10.000).
- **Conexión con Docker**:
  - **Automático**: busca `DOCKER_HOST`, el contexto activo, tu socket rootless y el del sistema.
  - **Socket personalizado**: una ruta absoluta concreta.
  - **Probar conexión** comprueba la configuración sin guardarla. **Guardar y conectar** la aplica. **Volver a conectar** repite la detección.
  - Conexión actual (socket, origen, versión, API, rootless) y **Docker Compose** (botón Comprobar, que ejecuta `docker compose version`). Si no está instalado lo indica y muestra cómo instalarlo; no es necesario para usar el resto de la aplicación.
- **Atajos** y **Acerca de** (versión y ruta del archivo de configuración).

Todo se guarda al momento en `~/.config/docker-control-center/config.toml`.

## Atajos de teclado

| Atajo | Acción |
|---|---|
| Ctrl+B | Contraer o expandir la barra lateral |
| Ctrl+Mayús+A | Abrir o cerrar el panel de actividad |
| Escape | Cerrar un diálogo |
