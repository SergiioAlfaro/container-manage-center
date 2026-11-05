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
Sin uso / Sin etiqueta** y búsqueda por etiqueta o ID. Descargar, construir,
etiquetar, inspeccionar y eliminar llegan en la Fase 4.

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
