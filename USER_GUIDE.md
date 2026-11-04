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
