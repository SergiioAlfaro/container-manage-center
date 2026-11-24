## 1. Qué es Container Manage Center {#about}

Container Manage Center (**CMC**) es una aplicación de escritorio para administrar **Docker Engine** sin depender de la terminal, en este equipo y en servidores remotos por SSH. Todo lo que muestra son datos reales que pide a Docker en ese momento, y cada acción es una operación real: nada se simula.

- Habla con Docker Engine con su **API oficial**: por el socket Unix en Linux, por la tubería con nombre de Docker Desktop en Windows y por SSH con los servidores remotos.
- Ejecuta **Docker Compose** con su programa oficial, con argumentos separados y sin shell.
- Cada operación aparece en el **panel de actividad** con su comando `docker` equivalente, para que sepas exactamente qué se ha hecho.

No hace falta saber usar Docker desde la terminal: cada apartado explica lo que hace antes de hacerlo y las operaciones que borran o cambian algo piden confirmación. Si un término no te resulta familiar, consulta el **Glosario** (sección 33).

## 2. Requisitos del sistema {#requirements}

| Elemento | Requisito |
|---|---|
| Sistema (Linux) | Debian 12 o superior, o Ubuntu 22.04 o superior, de 64 bits (x86_64) |
| Sistema (Windows, en pruebas) | Windows 10 u 11 de 64 bits con Docker Desktop (sección 28) |
| Docker | Docker Engine 20.10 o posterior (API 1.41+), en modo normal o rootless |
| Docker Compose | v2 o posterior, solo para el apartado Docker Compose (`docker compose` o `docker-compose` v2). Compose v1 no se admite |
| Bibliotecas (Linux) | WebKitGTK 4.1, GTK 3 y glibc. Las instala APT con el paquete |
| Servidores remotos | Cliente SSH del sistema (`ssh`) y, en el servidor, Docker Engine 20.10 o posterior |

Docker y Docker Compose son **requisitos externos**: la aplicación no los instala ni los configura, pero te dice cómo hacerlo si faltan.

La aplicación no necesita Internet. Solo las operaciones que hace Docker Engine contra registros remotos (descargar imágenes, buscar en Docker Hub, construir con imágenes base que no estén en el equipo) necesitan que Docker tenga red.

## 3. Instalación y primera ejecución {#install}

En Debian y Ubuntu, instala el paquete con APT, que resuelve las dependencias:

```
sudo apt install ./container-manage-center_X.Y.Z_amd64.deb
```

Para Debian 12 y Ubuntu 22.04 usa el paquete compatible, que termina en `-compat-debian12-ubuntu2204.deb`. Antes de instalar puedes comprobar la descarga con el archivo `SHA256SUMS` que se publica con cada versión (`sha256sum -c SHA256SUMS`).

La aplicación aparece en el menú como **Container Manage Center**. Se ejecuta con tu usuario normal: **no la ejecutes con sudo** (se niega a arrancar como root).

**Primera ejecución.** Al abrirla, busca Docker Engine automáticamente. Si lo encuentra, el **Dashboard** muestra su estado; si no, muestra el diagnóstico con la causa y los pasos para resolverla. No hace falta configurar nada más para empezar.

**Actualizar.** Instala el paquete nuevo de la misma forma: se conservan tu configuración y tu historial.

En Windows, la instalación se explica en la sección 28.

## 4. Interfaz principal y navegación {#interface}

La ventana tiene cuatro zonas:

- **Barra lateral** (izquierda): los apartados, agrupados. Se contrae con **Ctrl+B** o con **Contraer barra lateral**, al pie.
- **Barra superior**: el título del apartado, el **selector de servidor** (en qué Docker trabajas: este equipo o un servidor remoto), el **indicador de conexión** y el botón **Actividad**.
- **Área de trabajo**: el contenido del apartado elegido.
- **Panel de actividad** (derecha): las operaciones ejecutadas. Se abre y se cierra con **Ctrl+Mayús+A** o con el botón **Actividad**.

| Grupo | Apartados |
|---|---|
| (arriba) | Dashboard, Servidores |
| Recursos | Contenedores, Gestión masiva, Imágenes, Redes, Volúmenes, Directorios vinculados |
| Orquestación | Docker Compose |
| Herramientas | Terminales, Auditoría, Copias |
| Supervisión | Monitorización, Eventos y registros |
| (al pie) | Configuración |

Este manual está en **Configuración › Ayuda y documentación**. El índice de la izquierda lleva a cada sección, **Anterior** y **Siguiente** recorren el manual en orden y el buscador muestra solo las secciones que contienen el texto que escribas.

| Atajo | Acción |
|---|---|
| Ctrl+B | Contraer o expandir la barra lateral |
| Ctrl+Mayús+A | Abrir o cerrar el panel de actividad |
| Escape | Cerrar un diálogo o un menú |
| Flechas, Inicio y Fin | Moverse por los menús y mover los separadores de Terminales |

Los colores de estado son siempre los mismos: **verde** para lo que funciona o está en marcha, **ámbar** para avisos y **rojo** para errores.

## 5. Conexión con Docker Engine {#connection}

El **indicador de conexión** de la barra superior muestra el estado:

| Aspecto | Significado |
|---|---|
| Punto verde y `Docker 29.x` | Conectado. Al pasar el ratón se ve el socket |
| `Conectando con Docker…` | Detectando o conectando |
| Rojo, `Sin conexión con Docker` | Sin conexión. Púlsalo para ver el diagnóstico |

En modo **automático**, la aplicación prueba por este orden: la variable `DOCKER_HOST` (solo `unix://`), el contexto activo de Docker, tu socket rootless y el socket del sistema `/var/run/docker.sock`. Vigila la conexión cada 10 segundos y vuelve a conectar sola cuando Docker vuelve a estar disponible.

La aplicación nunca pide sudo, no cambia permisos ni grupos y no ejecuta por ti las instrucciones del diagnóstico. Tu usuario necesita poder abrir el socket: con Docker rootless o perteneciendo al grupo `docker` (lo que equivale en la práctica a tener privilegios de administrador).

La configuración se guarda al momento en `~/.config/container-manage-center/config.toml`. Si prefieres indicar un socket concreto, consulta la sección siguiente.

## 6. Docker rootless y sockets personalizados {#rootless}

Con **Docker rootless** el daemon se ejecuta con tu usuario y el socket está en `$XDG_RUNTIME_DIR/docker.sock` (normalmente `/run/user/1000/docker.sock`). La aplicación lo detecta sola, también a través del contexto `rootless`, y lo indica con la etiqueta **rootless**. Es la opción recomendada por seguridad.

En **Configuración › Conexión con Docker** puedes:

- Elegir **Socket personalizado** y escribir una ruta absoluta.
- **Probar conexión**: comprueba la ruta sin guardarla.
- **Guardar y conectar**: aplica la configuración.
- **Volver a conectar**: repite la detección.

Las conexiones `tcp://` no se admiten. Los servidores remotos se administran por SSH desde **Servidores** (sección siguiente).

## 7. Servidores remotos por SSH {#remote-servers}

Desde **Servidores** puedes administrar el Docker de otros equipos Linux a los que tengas acceso por **SSH**, a la vez que el de este equipo. La conexión usa el cliente SSH del sistema (`ssh`) y ejecuta `docker system dial-stdio` en el servidor: Docker **nunca** se expone por la red (nada de TCP 2375) y no se abre ningún puerto.

**Requisitos del servidor**: servidor SSH en marcha, Docker Engine 20.10 o posterior (API 1.41) y un usuario que pueda usar Docker. Para Compose, `docker compose` o `docker-compose` en el servidor. La aplicación **no instala Docker, no cambia permisos, no toca la configuración SSH ni abre puertos**: si falta algo, te dice qué y cómo resolverlo.

**Añadir servidor** (asistente): datos generales (nombre, host, usuario, puerto 22), autenticación, Docker (predeterminado, un socket concreto o un contexto) y **Probar conexión**, que comprueba siete pasos: DNS, conexión SSH, identidad del servidor, autenticación, Docker disponible, permisos y compatibilidad de la API.

- **Autenticación**: agente SSH (recomendado), clave privada, clave con frase de paso o contraseña (desaconsejada). La clave privada **nunca se copia**: solo se guarda su ruta.
- **Credenciales**: la frase de paso o la contraseña se piden al conectar. Si eliges **recordar**, se guardan en el **almacén seguro del sistema** (GNOME Keyring, KWallet…); si no hay uno, se piden cada vez y **nunca** se guardan en texto plano. Puedes borrarlas al editar el servidor.
- **Identidad del servidor**: la primera vez se muestra la **huella** (`SHA256:…`) para que la compruebes y la confirmes. Si cambia, la conexión se detiene con un aviso de posible **ataque de intermediario**; solo se sustituye tras revisarla. Si la clave anterior está en tu `~/.ssh/known_hosts`, la aplicación no modifica ese archivo y te da la orden para hacerlo tú.

**Varios servidores**: el **selector** de la barra superior indica en qué servidor trabajas, con su estado, y distingue los remotos. Al cambiar de servidor, toda la pantalla pasa a ese servidor. **Vista general** muestra todos a la vez (estado, versión, contenedores y recursos). Cada servidor es independiente: uno caído no afecta a los demás, y cada operación va al servidor en el que la iniciaste. Las confirmaciones indican **servidor y dirección**, y el panel de actividad muestra el servidor de cada operación y permite filtrar por él.

**En un servidor remoto** funcionan contenedores, imágenes, redes, volúmenes, terminales y Compose como en local. Las rutas (archivos Compose, carpetas de los bind mounts) son **del servidor**: se escriben a mano y se comprueban allí; antes de escribir un archivo en el servidor se confirma el destino. En **Terminales** también puedes abrir la **shell SSH del servidor**, un tipo de sesión aparte.

**Desconectar** cierra la sesión SSH y sus terminales; **eliminar el perfil** borra solo los datos de la aplicación (perfil, credenciales e identidad guardada). Ninguna de las dos cosas toca los contenedores, imágenes, redes, volúmenes ni datos del servidor.

Pertenecer al grupo `docker` del servidor equivale a tener acceso de **root** en él: decídelo con su administrador; Docker rootless es la alternativa más segura.

En un servidor remoto también funcionan la **Auditoría**, las **Copias** y la **Monitorización** (secciones 23 a 25).

## 8. Dashboard y diagnóstico {#dashboard}

Con conexión, el **Dashboard** muestra datos obtenidos en ese momento: versión de Docker Engine y de la API, socket y origen de la conexión, contenedores (en ejecución, en pausa, detenidos), imágenes, datos del equipo (sistema, núcleo, CPU, memoria), almacenamiento y ejecución (controlador, directorio de datos, cgroups, logs) y los avisos del daemon. **Actualizar** vuelve a pedirlos.

Sin conexión muestra el **diagnóstico**: la causa concreta, los pasos para resolverla (con las órdenes para copiar en una terminal), los sockets comprobados con su resultado y lo que se ha detectado instalado. Los botones **Volver a conectar** y **Configurar conexión** están en la cabecera.

Las causas más habituales de un fallo de conexión y su solución están en la sección 30.

## 9. Gestión de contenedores {#containers}

El apartado **Contenedores** lista todos los contenedores, como `docker ps -a`: estado, nombre e ID corto, proyecto Compose, imagen, estado de Docker, puertos publicados y antigüedad. Puedes filtrar por **Todos / En ejecución / Detenidos** y buscar por nombre, imagen, ID o proyecto. El menú **⋯** de cada fila (o el botón derecho) ofrece **Editar configuración** y **Crear imagen a partir de este contenedor**.

| Estado | Acciones disponibles |
|---|---|
| En ejecución | Detener, Reiniciar, Pausar, Eliminar |
| En pausa | Reanudar, Detener, Eliminar |
| Creado o detenido | Iniciar, Eliminar |

Detener y reiniciar esperan 10 segundos a que el proceso termine antes de forzarlo, como Docker. Tras cada acción se consulta el estado real del contenedor. Si Docker rechaza una acción, su mensaje aparece encima de la tabla y en el panel de actividad. Las listas se actualizan solas cuando algo cambia en Docker, también desde la terminal.

Para aplicar una misma acción a muchos contenedores a la vez, usa **Gestión masiva** (sección 13).

## 10. Crear, duplicar, inspeccionar y eliminar contenedores {#container-lifecycle}

**Crear contenedor** abre un asistente con seis pestañas (solo la imagen es obligatoria): General, Entorno, Red, Almacenamiento, Recursos y Avanzado.

- **Comando** y **Entrypoint** se escriben como en una terminal y se dividen en argumentos, que ves debajo. No se ejecuta ninguna shell.
- El panel **Equivalente** muestra el `docker run` equivalente y el cuerpo exacto que se envía a la API.
- **Revisión** marca los errores (impiden crear) y las advertencias (puertos privilegiados, carpetas delicadas con escritura, modo privilegiado…).
- **Crear** crea el contenedor sin iniciarlo; **Crear e iniciar** también lo inicia.

Al pulsar el nombre de un contenedor se abre su **detalle**: Resumen, Logs, Terminal, Estadísticas e Inspección JSON (la respuesta completa de `docker inspect`, con botón Copiar).

**Duplicar** abre el asistente con la configuración del contenedor y el nombre `<nombre>-copia`, y avisa de los ajustes que no se trasladan.

**Eliminar** pide confirmación. Un contenedor en ejecución o en pausa solo se elimina si marcas **Detenerlo a la fuerza y eliminarlo**. Opcionalmente se eliminan sus volúmenes anónimos; los volúmenes con nombre y las carpetas vinculadas no se tocan. No se puede deshacer.

Los logs, las estadísticas y la terminal del detalle se explican en la sección 21.

## 11. Editar la configuración de un contenedor {#container-edit}

**Editar configuración** está en el menú **⋯** de cada contenedor (también con el botón derecho sobre la fila) y en la cabecera del detalle. La configuración se organiza en pestañas: **General**, **Red**, **Almacenamiento**, **Variables**, **Puertos**, **Recursos** y **Avanzado**. Cada cambio indica cómo se aplica:

| Cambio | Cómo se aplica |
|---|---|
| Nombre | Directo (`docker rename`) |
| Política de reinicio, CPU, memoria y memoria reservada | Directo (`docker update`), sin reiniciar el contenedor |
| Conectar o desconectar redes | Directo y al momento (`docker network connect` / `disconnect`) |
| Variables de entorno, puertos, volúmenes y carpetas, descripción | Requiere **recrear** el contenedor |
| Quitar un límite de CPU o memoria | Requiere recrear (`docker update` no sabe quitarlo) |
| Imagen, comando, usuario, directorio de trabajo y demás opciones de **Avanzado** | Solo lectura: Docker no permite cambiarlas. Usa **Duplicar** |

- **Red**: muestra las redes, IP, MAC y alias. Docker no permite cambiar los alias ni la IP de una conexión existente: hay que desconectar y volver a conectar. Si es la única red del contenedor, se avisa antes de desconectarla.
- **Variables**: los valores que parecen secretos se ocultan en pantalla y nunca se guardan en el panel de actividad.
- **Descripción**: se guarda en la etiqueta `cmc.description` del contenedor.

**Revisar cambios** compara la configuración actual con la nueva: qué cambia (antes y después, elemento a elemento), qué se mantiene y qué requiere recrear. Nada se aplica hasta que lo confirmas.

**Recreación segura.** Cuando un cambio exige recrear, el asistente:

1. Comprueba la imagen, los conflictos de nombre y de puertos con otros contenedores y los motivos que impiden recrear con seguridad: contenedores de Compose o Swarm, creados con `--rm`, en pausa o reiniciándose. En esos casos no deja continuar.
2. Muestra los riesgos que debes aceptar uno a uno: archivos escritos fuera de los volúmenes (no pasan al contenedor nuevo), montajes que dejan de estar montados, contenedores que usan la red de este…
3. Al confirmar: detiene el contenedor si está en marcha, le cambia el nombre a `<nombre>-cmc-backup-<fecha>` (copia de seguridad), crea el nuevo con **toda** la configuración original más tus cambios, conecta sus redes y, si estaba en marcha, lo inicia y comprueba que sigue funcionando unos segundos.
4. Si algo falla, elimina el contenedor nuevo (sin tocar sus volúmenes), devuelve el nombre al original y lo vuelve a iniciar.

**Nunca se eliminan volúmenes, carpetas ni imágenes.** Los volúmenes anónimos se vuelven a montar con su contenido. La copia queda detenida en **General › Copias de seguridad**, desde donde puedes **Restaurarla** (el contenedor actual se aparta, no se elimina) o **Eliminarla** cuando ya no la necesites. Todas las operaciones quedan en el panel de actividad, sin valores sensibles.

## 12. Crear una imagen a partir de un contenedor {#container-commit}

**Crear imagen a partir de este contenedor** (menú **⋯** de la lista y cabecera del detalle) equivale a `docker commit`: guarda el estado actual del sistema de archivos del contenedor como una imagen nueva.

- Indica el **repositorio** y la **etiqueta** (vacía = `latest`); opcionalmente, **autor** y **mensaje**. Se ve el nombre completo antes de crear y se valida.
- **No incluye los volúmenes ni las carpetas montadas**: solo la capa de escritura del contenedor. El diálogo lista los montajes que quedan fuera.
- La imagen **sí guarda la configuración del contenedor**, incluidas sus variables de entorno (también las que contienen contraseñas o tokens): no la publiques en un registro si contiene secretos.
- Si el contenedor está en ejecución, por defecto se **pausa** unos segundos mientras se crea la imagen, para que sea coherente; puedes desmarcarlo. Nunca se detiene.
- Si ya existe una imagen con ese nombre, se avisa y hay que aceptarlo expresamente: la anterior no se elimina, se queda sin etiqueta.
- El contenedor original no se modifica. Al terminar, **Ver la imagen** abre su detalle en Imágenes.

## 13. Gestión masiva {#bulk}

**Gestión masiva** (en «Recursos») aplica una misma operación o un mismo comando a muchos contenedores a la vez, también de varios servidores. Se reúnen los contenedores de este equipo y de los servidores remotos **conectados**; los demás se indican en un aviso. Está dividida en apartados: **Resumen**, **Contenedores**, **Operaciones**, **Configuración masiva**, **Comandos masivos** e **Historial**.

- **Resumen**: total, en ejecución, detenidos, en pausa y con error. Pulsar un contador filtra por ese estado. **Actualizar** y la actualización automática (de 5 s a 5 min, se puede desactivar) vuelven a pedir los datos.
- **Contenedores**: busca por nombre o ID; filtra por estado, imagen, servidor y etiqueta (`clave` o `clave=valor`); ordena pulsando el título de una columna. Se selecciona con la casilla de cada fila, la de la página o **todo lo filtrado**.
- **Operaciones**: iniciar, detener, reiniciar, pausar, reanudar y eliminar. Lo que no se puede aplicar en su estado se **omite** y se explica.
- **Configuración masiva**: muestra la política de reinicio de la selección y permite cambiarla en todos (en caliente, con `docker update`; no en los creados con `--rm`). Las **etiquetas** solo se muestran: Docker no permite cambiarlas en un contenedor que ya existe (habría que recrearlo). Para un contenedor concreto, usa **Duplicar**.
- **Historial**: las operaciones y los comandos masivos anteriores, con lo que se hizo en cada contenedor.

**Antes de aplicar** se muestra una confirmación con la operación, los servidores, los contenedores afectados y los omitidos, las consecuencias y cuántas operaciones van a la vez (de 1 a 16). **Eliminar** pide escribir el número de contenedores y **nunca elimina volúmenes ni imágenes**.

**Resultados**: cada contenedor tiene el suyo: correcto (Docker lo confirmó), fallido (con el error), omitido, cancelado o **desconocido** (la conexión se cortó antes de la confirmación; actualiza para ver su estado). Si uno falla, los demás siguen; nada se deshace ni se reintenta solo. **Cancelar lo pendiente** deja de enviar órdenes. Cada orden se registra en el panel de actividad con el distintivo «Masiva», y **Ver en actividad** muestra solo esa operación.

**Comandos masivos** ejecuta un mismo comando dentro de varios contenedores en ejecución con Docker Exec, sin pasar por ninguna shell de este equipo:

1. Elige el modo: **Ejecutable con argumentos** (un argumento por línea, enviado tal cual) o **Secuencia con un intérprete** (varias líneas para `/bin/sh`, `bash`, `python3`…). También puedes cargar un ejemplo o un comando frecuente guardado.
2. Si hace falta, indica el usuario, el directorio de trabajo, el tiempo máximo (de 1 a 3600 s) y cuántas ejecuciones van a la vez.
3. Revisa la **vista previa**: el `docker exec` equivalente, los argumentos exactos que recibe Docker y el nivel de riesgo (Informativo, Modificación, Administrativo o Potencialmente destructivo). La clasificación es orientativa: no sustituye a revisar el comando.
4. Confirma. Los comandos de riesgo administrativo o destructivo piden escribir el número de contenedores.

Cada contenedor da su estado, código de salida, duración, `stdout` y `stderr` por separado (copiables y exportables a Markdown o JSON con los secretos ocultos). **Cancelar pendientes** no lanza el comando en los que aún no empezaron; **Interrumpir activas**, además, deja de esperar a los que están en marcha. Docker no permite detener un proceso ya lanzado: con el tiempo agotado o al interrumpir, el proceso puede seguir dentro del contenedor, y CMC **nunca** detiene ni reinicia el contenedor para pararlo. Los comandos frecuentes se pueden guardar, **nunca con secretos**.

## 14. Imágenes {#images}

El apartado **Imágenes** lista las imágenes locales, como `docker image ls`: etiquetas, ID corto, tamaño, antigüedad y qué contenedores las usan. Filtros **Todas / En uso / Sin uso / Sin etiqueta** y búsqueda por etiqueta o ID.

- **Detalle**: Resumen (con los valores sensibles de las variables ocultos), Capas (como `docker image history`), Contenedores que la usan e Inspección JSON.
- **Etiquetar** añade un nombre `repositorio:etiqueta`.
- **Eliminar**: si la imagen tiene varias etiquetas puedes quitar solo una; la imagen completa exige forzar. Docker no elimina imágenes de contenedores en ejecución.
- **Limpiar** elimina las imágenes sin etiqueta que no usa ningún contenedor o, con confirmación adicional, todas las que no se usan.

## 15. Descargar y construir imágenes {#pull-build}

**Descargar imagen** pide la referencia (`nginx:1.27`, `usuario/app`, `registro.ejemplo.com:5000/app:v2`; sin etiqueta se usa `latest`). Credenciales:

| Opción | Qué hace |
|---|---|
| Las de Docker | Usa lo guardado con `docker login` (o su *credential helper*) |
| Ninguna | Descarga anónima |
| Usuario y contraseña | Solo para esta descarga: no se guardan ni aparecen en el panel de actividad |

El panel **Descargas** muestra el progreso total y por capa; puedes cambiar de apartado mientras tanto y **Cancelar**. **Buscar en Docker Hub** busca como `docker search` (lo hace Docker Engine, que necesita Internet).

**Construir imagen** abre un asistente: contexto (la carpeta que se envía, respetando `.dockerignore`) y Dockerfile, nombres (`--tag`), argumentos (`--build-arg`), metadatos (`--label`), etapa final, sin caché, buscar imágenes base nuevas y constructor (**BuildKit** o **clásico**). Se ven los pasos con su estado y la salida como `docker build --progress=plain`; se puede cancelar.

## 16. Redes {#networks}

El apartado **Redes** lista las redes, como `docker network ls`, con driver, subredes y contenedores. Las redes **bridge**, **host** y **none** son predefinidas y no se pueden eliminar.

- **Crear red**: nombre, driver (`bridge`, `macvlan`, `ipvlan`, `overlay` u otro), red interna, direcciones IPv4 (subred, puerta de enlace y rango) e IPv6, opciones del driver y metadatos. Se ven el comando equivalente y la petición exacta, y los errores se marcan antes de enviar nada a Docker.
- **Conectar** un contenedor (en marcha o detenido), con alias e IP fija opcionales. **Desconectar** pide confirmación.
- **Eliminar** y **Limpiar** avisan si hay contenedores detenidos que tienen configurada la red y que no podrán arrancar después.

## 17. Volúmenes {#volumes}

El apartado **Volúmenes** lista los volúmenes, como `docker volume ls`, con su driver y los contenedores que los montan. Los anónimos se marcan como tales. **Calcular tamaños** pide a Docker el espacio de cada uno (como `docker system df -v`); no se hace automáticamente porque puede tardar.

- **Crear volumen**: nombre (opcional) y tipo: local, tmpfs, NFS, una carpeta del equipo u otro driver.
- **Detalle**: directorio, opciones, metadatos, tamaño, contenedores con su ruta y modo, e inspección JSON. **Crear contenedor con él** lo monta en `/data`.
- **Eliminar**: Docker no elimina un volumen que monte cualquier contenedor, aunque esté detenido.
- **Limpiar**: elimina los volúmenes anónimos sin uso o, con confirmación, todos los volúmenes sin uso.

**Los datos de un volumen eliminado no se pueden recuperar.** Si los necesitas, haz antes una copia (sección 24).

## 18. Directorios vinculados {#binds}

Este apartado muestra las carpetas y archivos de tu equipo montados dentro de contenedores, agrupados por ruta: tipo, permisos y propietario, si existen, qué contenedores los montan, dónde y con qué acceso. Se señalan los que requieren atención: el **socket de Docker** (da control de Docker al contenedor), la raíz o directorios del sistema y las rutas que ya no existen.

Docker no permite añadir una carpeta a un contenedor que ya existe. **Vincular una carpeta…** abre el selector de carpetas y después el asistente de contenedores con esa carpeta montada en `/mnt/<nombre>`.

## 19. Docker Compose: proyectos {#compose}

El apartado **Docker Compose** necesita Compose v2 o posterior; si no está instalado, explica cómo instalarlo. Todas las operaciones ejecutan el Docker Compose oficial y aparecen en el panel de actividad con el comando exacto.

La lista muestra los proyectos con contenedores en Docker (también los desplegados desde la terminal) y los que añades, con su estado (en marcha, parcial, detenido o sin desplegar), carpeta y servicios.

- **Abrir proyecto…**: elige su `compose.yaml` (y los archivos adicionales en el orden en que se combinan).
- **Nuevo proyecto**: elige una carpeta, escribe el contenido (se parte de una plantilla) y **Crear proyecto**: se valida con Docker Compose antes de crear `compose.yaml`.
- **Quitar de la lista** solo la quita de la lista; no toca Docker ni los archivos.

## 20. Docker Compose: servicios, despliegue y perfiles {#compose-projects}

En el detalle de un proyecto:

- **Servicios**: cada servicio con su imagen, estado, contenedores, puertos y **perfiles**. Cada fila permite desplegar, iniciar, detener, reiniciar o ver los registros de ese servicio.
- **Desplegar** (`docker compose up --detach`): todos los servicios o los elegidos, **perfiles** que activar, construir antes, política de descarga de imágenes, volver a crear contenedores y eliminar huérfanos. El diálogo muestra el comando exacto.
- **Iniciar, Detener, Reiniciar**, **Descargar imágenes** y **Construir**.
- **Eliminar** (`docker compose down`): contenedores y redes del proyecto; opcionalmente sus volúmenes con nombre (sus datos se pierden) y sus imágenes. Hay que confirmarlo.
- **Salida** de la operación en curso (se puede **Interrumpir**) y **Registros** de los servicios en directo.
- **Archivo**: edita los archivos; **Validar** los comprueba con Docker Compose sin guardar, y **Guardar** valida antes y no sobrescribe si otro programa los cambió.
- **Configuración**: la configuración final según Docker Compose. Puede contener secretos, así que está oculta hasta que la pides y no se guarda en el panel de actividad.

## 21. Registros, estadísticas y terminal de un contenedor {#logs-stats-terminal}

En el detalle de un contenedor:

- **Logs**: salida en directo (stderr en rojo). Puedes elegir cuántas líneas anteriores cargar, filtrar, ver las marcas de tiempo, seguir el final, copiar lo visible y dejar de seguir. Se conservan las últimas 10 000 líneas.
- **Estadísticas**: CPU, memoria, red, disco y procesos cada segundo, calculados como `docker stats`.
- **Terminal**: **Abrir terminal** inicia una shell con `docker exec -it` (Automática usa `bash` si existe y si no `sh`; también puedes elegir una). **Cerrar sesión** termina la shell y los procesos que dejó dentro del contenedor.

Son operaciones reales que aparecen en el panel de actividad mientras están abiertas y se cierran al salir del detalle. Para trabajar con varias terminales a la vez, usa el apartado **Terminales**.

Para ver la evolución del uso en el tiempo y recibir alertas, usa **Monitorización** (sección 25).

## 22. Terminales simultáneas {#terminals}

El apartado **Terminales** permite abrir y ver a la vez varias terminales reales (`docker exec -it`) en contenedores en ejecución, en el mismo contenedor o en otros.

- **Nueva terminal**: elige un contenedor en ejecución, la shell (Automática, bash o sh) y, si quieres, un nombre para la sesión.
- Cada panel muestra el **contenedor**, el **nombre de la sesión** y un **indicador de conexión** (Conectando, Conectada, Finalizada o Error), con botones para **renombrar**, **maximizar** y **cerrar**. Doble clic en el nombre también lo renombra.
- **Distribución**: una terminal ocupa todo el espacio; dos van en dos columnas; tres, una a la izquierda y dos a la derecha; cuatro, en cuadrícula 2×2; con más de cuatro, en filas de dos con desplazamiento vertical.
- Los **separadores** se arrastran con el ratón. Con el foco en un separador, las flechas lo mueven, Inicio y Fin lo llevan a los extremos y el doble clic reparte el espacio a partes iguales.
- Cada sesión es independiente: su propia shell, su entrada y salida, su tamaño y su estado. Lo que haces en una no afecta a las demás.
- Las sesiones **siguen abiertas al cambiar de apartado**. **Cerrar todas** pide confirmación. Se admiten hasta 12 sesiones a la vez.
- Al **cerrar** una sesión se termina su shell y los procesos que dejó en segundo plano dentro del contenedor. Si el contenedor se **detiene** o se **elimina**, el panel lo indica; una sesión terminada se puede **volver a abrir** si el contenedor sigue existiendo.
- Al **cerrar la aplicación** se terminan todas las sesiones.

El panel de actividad registra la apertura, el cierre y los errores de cada sesión, pero no lo que se escribe ni la salida de la terminal.

## 23. Auditoría de seguridad {#audit}

**Herramientas › Auditoría** revisa la seguridad del servidor elegido (este equipo o uno remoto) y **no cambia nada**: solo lee la configuración.

1. Elige **qué auditar**: todos los contenedores (en ejecución y detenidos) o uno; también las imágenes y los proyectos Compose (se lee su configuración con `docker compose config`, sin desplegar).
2. Pulsa **Auditar**. Se ve el avance y se puede cancelar.
3. Revisa los **hallazgos**, por nivel (crítico, alto, medio, bajo) y agrupados por contenedor, servicio o imagen. Cada uno explica qué ocurre, **por qué importa** y qué se **recomienda**. Se pueden filtrar por nivel, por tipo y por texto.

Se comprueban los riesgos para el equipo (socket de Docker montado, carpetas sensibles, capacidades, modo privilegiado, red del equipo, PID e IPC compartidos, seccomp, AppArmor o SELinux desactivados, dispositivos y puertos abiertos a todas las interfaces) y el endurecimiento (proceso como root, sistema de archivos modificable, sin límites de memoria, CPU o procesos, sin `no-new-privileges` e imágenes de más de un año). Con Docker rootless o `userns-remap`, el root del contenedor se marca como bajo.

- **Proponer correcciones** (contenedores; el botón indica cuántas): abre **Editar configuración** con los cambios ya puestos en el borrador. Revísalos y aplícalos como cualquier edición (sección 11); si los descartas, no cambia nada.
- **Vulnerabilidades**: si tienes **Trivy** o **Grype** instalados con su base descargada, marca la casilla y cada imagen muestra cuántas vulnerabilidades conocidas tiene, por gravedad. La aplicación no instala estas herramientas. Sin ellas, o en un servidor remoto, el informe dice «No analizado», nunca «0».
- **Exportar** a JSON, Markdown o HTML: el archivo queda solo para tu usuario y los secretos se ocultan.
- **Auditorías anteriores**: las 200 últimas de cada servidor, para abrirlas de nuevo.
- **Contenedores en las demás herramientas**: para cada contenedor auditado, su peor hallazgo, si la monitorización lo vigila, su copia más reciente y las tareas de copias que lo incluyen, con accesos a **Ver en Monitorización** y **Preparar una copia**.

## 24. Copias de seguridad {#backup}

**Herramientas › Copias** hace copias de seguridad del servidor elegido (este equipo o uno remoto) y las restaura.

1. **Carpeta de destino**: **Elegir carpeta** abre el diálogo del sistema. Dentro se crea `container-manage-center-copias/<servidor>/<copia>/`, solo para tu usuario. No se pueden usar carpetas del sistema; sí un disco externo o una carpeta de red ya montada.
2. **Nueva copia**: marca lo que quieras copiar.
3. **Hacer copia ahora** muestra el avance y se puede cancelar. Una copia cancelada o interrumpida nunca se da por buena.

| Elemento | Cómo se copia |
|---|---|
| Volúmenes | Por la API de Docker, con un contenedor auxiliar que nunca se arranca. Si un contenedor en marcha lo usa, elige **en caliente** (copia no garantizada) o **deteniendo los contenedores** (se paran mientras se copia y vuelven a arrancar) |
| Bases de datos | MariaDB, MySQL, PostgreSQL, Redis o MongoDB en marcha: el volcado lo hace la herramienta del propio motor dentro del contenedor, con la contraseña de sus variables. Es coherente aunque haya escrituras |
| Configuración de contenedores | Para volver a crearlos, sin las variables con secretos |
| Proyectos Compose | Solo en este equipo: sus archivos. Los `.env` solo si marcas «Incluir los archivos .env» |
| Configuración de la aplicación | Preferencias, servidores y tareas (las contraseñas están en el llavero y no se copian) |

- **Copias**: las del servidor, con su estado (Completa, Incompleta), elementos, tamaño y avisos. **Comprobar** recalcula la suma SHA-256 de cada archivo y dice si alguno está alterado, falta o sobra.
- **Restaurar**: elige qué elementos y adónde, y pulsa **Vista previa**, que comprueba la integridad y dice qué se hará con cada uno. Un volumen se restaura en uno **nuevo** o vacío, nunca sobre uno con datos. Un contenedor se crea nuevo, con otro nombre y sin arrancar. Los archivos se escriben en una carpeta nueva.
- **Restaurar una base de datos sustituye sus datos actuales**: hay que escribir «restaurar» para confirmarlo.
- **Tareas programadas**: **Guardar como tarea** guarda lo elegido con un nombre, una frecuencia (diaria, semanal o mensual) y una retención (por defecto, 7 diarias, 4 semanales y 3 mensuales). La retención solo borra copias antiguas **de esa tarea**. **Programar** crea un temporizador de usuario de systemd (en Windows, una tarea del Programador de tareas) que funciona aunque la aplicación esté cerrada; antes te muestra lo que va a crear y solo lo crea si lo autorizas. No hace falta ser administrador. Los temporizadores de usuario de Linux solo se ejecutan con tu sesión iniciada, salvo que actives tú `loginctl enable-linger`.
- **Historial**: cada copia, manual o programada, con su resultado y las copias antiguas que borró la retención.

Las copias **no están cifradas**: contienen tus datos tal cual. Si las guardas fuera del equipo, usa un disco cifrado.

## 25. Monitorización y alertas {#monitoring}

**Supervisión › Monitorización** muestra el uso de los contenedores del servidor elegido a lo largo del tiempo. No instala nada en el servidor: pregunta a Docker lo mismo que `docker stats`, también por SSH.

- **Activar**: marca «Monitorizar este servidor». Se activa en cada servidor por separado y sigue activa al volver a abrir la aplicación. El **intervalo** (de 2 a 60 s) es común a todos; **Pausar** detiene todas las muestras.
- **Resumen**: CPU y memoria del equipo según Docker, contenedores en marcha, en pausa y parados, y el uso total en gráficos. La tabla tiene, por contenedor, CPU, memoria y su límite, red, disco, procesos, tiempo en marcha y reinicios. Pulsa un nombre para ver sus gráficos. La CPU está en % de una CPU, como `docker stats`: un contenedor que usa dos CPU enteras marca 200 %.
- **Periodo**: 5, 15 y 30 minutos vienen de la memoria (desde que abriste la aplicación); 1, 6 y 24 horas, del resumen por minuto que se guarda en tu equipo.
- **Comparar**: hasta 4 contenedores en marcha y una métrica en el mismo gráfico.
- **Sin conexión**: si el servidor deja de responder, los gráficos muestran un tramo rayado. No se inventan datos.

**Alertas**: CPU alta y memoria alta (con umbral de aviso y de resolución, para no avisar a cada pequeña subida), el contenedor se cae y Docker lo vuelve a arrancar, falta de memoria (OOM), termina con error sin que nadie lo pare y se pierde la conexión con el servidor. Parar tú un contenedor no es un error. Tras un aviso, la misma alerta no se repite durante los minutos de silencio (15 por defecto). Las **notificaciones de escritorio** no salen del equipo; **Probar notificación** envía una de prueba. Todas las alertas quedan en el **Historial de alertas**.

La monitorización no apunta cada muestra en el panel de actividad: solo cuándo empieza y cuándo termina en cada servidor.

## 26. Eventos, actividad e historial de operaciones {#activity}

Cada operación aparece en el **panel de actividad**: hora, distintivo **API** (petición a Docker Engine), **CLI** (proceso, como `docker compose` o `apt-get`) o **APP** (operación de la propia aplicación, como un cambio de idioma, o el resumen de una recreación cuyos pasos aparecen aparte), tipo, recurso, servidor, duración y estado. La línea principal es el comando equivalente, que puedes copiar. Al desplegar una entrada ves la operación ejecutada de verdad, el resultado, el error y su código HTTP si falló, y las salidas de los procesos. Los valores secretos se ocultan con `********`.

El apartado **Eventos y registros** tiene cuatro pestañas:

- **En directo**: los eventos de Docker de esta sesión (un contenedor que arranca, una imagen descargada…), vengan de esta aplicación o de cualquier otra herramienta. Se pueden filtrar y **Pausar**.
- **Guardados**: los eventos recibidos con la aplicación abierta, también de sesiones anteriores (los 20 000 más recientes).
- **Consultar a Docker**: los eventos de un periodo (`docker events --since … --until …`). Incluye lo que pasó con la aplicación cerrada, pero Docker solo conserva los últimos 256.
- **Historial de operaciones**: lo que la aplicación ha hecho, también en sesiones anteriores (las 10 000 más recientes).

El historial guarda qué se hizo, cuándo y con qué resultado, pero nunca el cuerpo de las peticiones, el comando ni las salidas, porque pueden contener contraseñas. Está en `~/.local/state/container-manage-center/`, solo legible por tu usuario. Puedes borrarlo desde cada pestaña o dejar de guardarlo en Configuración.

## 27. Configuración de la aplicación {#settings}

- **Apariencia**: tema (Sistema, Claro, Oscuro) e idioma, por este orden: **English**, **Español** (predeterminado), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** y **Português**. Cada idioma aparece con su propio nombre. El idioma se guarda y se mantiene al volver a abrir la aplicación. El color principal es el **morado** y no se puede cambiar.
- **Panel de actividad**: abrirlo solo cuando una operación falla, operaciones conservadas (50–10 000) y **Guardar el historial entre sesiones**.
- **Conexión con Docker** (secciones 5 y 6) y comprobación de **Docker Compose**.
- **Actualizaciones**: dónde se publican las versiones y **Comprobar ahora**. La aplicación nunca descarga ni instala nada.
- **Sistema**: versiones de la aplicación, del paquete, de WebKitGTK y glibc, del sistema y de Docker; **Copiar** genera un informe sin datos sensibles.
- **Desinstalación y componentes** (sección 29), **Ayuda y documentación** (este manual) y **Legal** (sección 34).
- **Restablecer las preferencias**, tras confirmarlo. No cambia la conexión con Docker.

Todo se guarda al momento en `~/.config/container-manage-center/config.toml`.

Los mensajes que genera el propio Docker (por ejemplo, un error de Docker Engine) y la salida de los programas se muestran tal como llegan, sin traducir. Los resúmenes técnicos que el motor de la aplicación escribe en el panel de actividad y los mensajes de error con datos concretos se generan en español en todos los idiomas.

## 28. Windows {#windows}

La versión para Windows está **en pruebas**: el instalador se genera, pero todavía no se ha validado en un Windows real. Es un instalador `.exe` para tu usuario, sin permisos de administrador, y no está firmado, así que Windows puede mostrar un aviso de SmartScreen al abrirlo.

En Windows, Container Manage Center funciona igual que en Linux, con estas diferencias:

- **Docker local**: se conecta con **Docker Desktop** por su tubería con nombre (`\\.\pipe\docker_engine`). Docker Desktop tiene que estar en marcha, y tu usuario, en el grupo local `docker-users` (lo añade el instalador de Docker Desktop).
- **Servidores remotos por SSH**: usan el cliente OpenSSH de Windows (`ssh.exe`, incluido en Windows 10 y 11). Como no admite conexiones multiplexadas, cada operación abre su propia conexión: la primera vez se comprueba la identidad del servidor igual que en Linux, y la contraseña o la frase de paso guardada se usa sola. Los **servidores intermedios** (ProxyJump) todavía no están disponibles, y en la **terminal del servidor** el cambio de tamaño de la ventana no llega al servidor.
- **Contraseñas recordadas**: en el **Administrador de credenciales** de Windows (credenciales de Windows), con el nombre `container-manage-center/…`.
- **Copias programadas**: con el **Programador de tareas**, en la carpeta «Container Manage Center», con tu usuario y sin privilegios de administrador. Se ejecutan con tu sesión iniciada; si el equipo estaba apagado a esa hora, la copia se hace en cuanto se pueda.
- **Notificaciones**: las del sistema (centro de notificaciones).
- **Datos**: configuración en `%APPDATA%\container-manage-center` e historial en `%LOCALAPPDATA%\container-manage-center`.
- **Desinstalar**: desde Configuración de Windows › Aplicaciones › Aplicaciones instaladas. Tus datos y Docker Desktop no se tocan. El apartado «Desinstalación y componentes» solo existe en Debian y Ubuntu.

## 29. Desinstalación y componentes {#uninstall}

**Configuración › Desinstalación y componentes** muestra qué instaló la aplicación y permite quitarlo sin usar la terminal. **Analizar** consulta dpkg y APT sin cambiar nada:

- **Paquete de la aplicación** (`container-manage-center`), con su espacio.
- **Dependencias que se pueden quitar**: solo las que APT instaló automáticamente **junto con la aplicación** (según su historial) y que ningún otro paquete necesita.
- **Se conservan**, con el motivo: dependencias compartidas (con los paquetes que las usan), instaladas a mano, que ya existían antes, de origen incierto (si el historial de APT no muestra cómo se instaló la aplicación) y componentes del sistema.
- **Herramientas externas**: Docker Engine, Docker CLI y Docker Compose. **Se conservan siempre.**
- **Datos que no se borran**: tu configuración, tu historial y los datos de la ventana. Tampoco se toca nada de Docker: imágenes, contenedores, redes, volúmenes y proyectos Compose.
- La **simulación de APT** completa de la desinstalación.

Puedes elegir **Solo Container Manage Center** o **con las dependencias que se pueden quitar**, o marcar los paquetes uno a uno; el resumen indica cuánto espacio se recupera. **Revisar y desinstalar…** vuelve a comprobar la selección y simula exactamente la orden: si APT fuera a quitar algo que no has elegido, no se puede continuar. El diálogo muestra la orden que se va a ejecutar:

```
pkexec /usr/bin/apt-get remove -y --no-auto-remove container-manage-center …
```

Al confirmar, el sistema pide tu contraseña de administrador (pkexec) **solo para esa orden**; la aplicación sigue sin privilegios. Si cancelas la autenticación no se quita nada. No se usa `purge`. Todo queda en el panel de actividad. Si se quita la propia aplicación, ciérrala al terminar.

Si `pkexec` no está instalado, la aplicación no puede pedir permisos: el diálogo muestra la orden equivalente con `sudo` para que la ejecutes tú en una terminal.

## 30. Solución de problemas {#troubleshooting}

| Causa mostrada | Qué hacer |
|---|---|
| Docker no parece estar instalado | `sudo apt install docker.io` o la guía oficial de Docker |
| No se encuentra Docker Engine / está detenido | `sudo systemctl enable --now docker` (rootless: `systemctl --user enable --now docker`) |
| Sin permiso para usar el socket | Docker rootless, o el grupo `docker` y volver a iniciar sesión |
| Ya perteneces al grupo, pero no funciona | Cierra la sesión y vuelve a entrar |
| Conexión de Docker no compatible | `tcp://` o `ssh://` en la conexión local: usa un socket local (`docker context use default`); para un servidor remoto, añádelo en **Servidores** |
| Docker no responde a tiempo | `sudo systemctl restart docker` |
| Versión de Docker demasiado antigua | Actualiza a Docker 20.10 o posterior |

- **Docker Compose no aparece**: instálalo (Debian 13: `sudo apt install docker-compose`; Ubuntu: `sudo apt install docker-compose-v2`; repositorio de Docker: `docker-compose-plugin`) y pulsa **Comprobar** en Configuración.
- **Una descarga o búsqueda falla**: Docker Engine necesita acceso a Internet; el mensaje de error es el de Docker.
- **Una terminal no se abre**: el contenedor debe estar en ejecución y tener una shell (`sh` o `bash`).
- **No se permite recrear un contenedor**: el motivo aparece en la revisión (contenedor de Compose, creado con `--rm`, en pausa, imagen que ya no existe…).
- **La desinstalación no ofrece una dependencia**: la usa otro programa, ya estaba instalada o no se puede saber con seguridad; el motivo aparece junto a ella.
- **Para informar de un problema**: Configuración › Sistema › **Copiar** genera un informe sin datos sensibles.

- **Un servidor remoto no conecta**: **Probar conexión** indica en cuál de los siete pasos falla (sección 7).
- **Un comando masivo falla en un contenedor**: el contenedor debe estar en ejecución y tener el programa o el intérprete indicados; el error de Docker aparece en su resultado.

## 31. Seguridad y buenas prácticas {#security}

- La aplicación **no se ejecuta como root**, nunca pide sudo para funcionar y no cambia permisos del socket ni grupos.
- Los comandos externos se ejecutan **sin shell** y con argumentos separados: lo que escribes nunca se interpreta como órdenes.
- **No guarda credenciales.** Las de un registro se usan solo para una descarga y no aparecen en el panel de actividad. Con «Las de Docker» se usan las de `docker login`.
- Los valores de variables como `PASSWORD`, `TOKEN` o `SECRET` se ocultan en pantalla y en el panel (`********`), aunque Docker los recibe completos.
- El historial persistente nunca guarda cuerpos de peticiones, comandos ni salidas.
- Las operaciones destructivas piden confirmación.
- La ventana bloquea conexiones externas. La única conexión propia es la comprobación de versiones, desactivada mientras no indiques una fuente. El botón de GitHub de la Ayuda solo puede abrir esa dirección, en tu navegador.
- Montar el socket de Docker en un contenedor o pertenecer al grupo `docker` da control total del equipo: la aplicación lo señala.

**Buenas prácticas**: usa Docker rootless cuando puedas; revisa la **Auditoría** de vez en cuando; haz **copias** antes de eliminar volúmenes o actualizar bases de datos; no publiques puertos en todas las interfaces si solo los usas en este equipo; no guardes contraseñas en imágenes ni en los comandos frecuentes. En la gestión masiva, eliminar y los comandos de riesgo administrativo o destructivo piden escribir el número de contenedores.

## 32. Preguntas frecuentes {#faq}

**¿La aplicación ejecuta comandos `docker`?** No: usa la API de Docker Engine. El comando que ves en el panel de actividad es el equivalente, para que puedas reproducirlo. Docker Compose sí se ejecuta con su programa oficial.

**¿Necesito ser administrador?** No. Solo necesitas acceso al socket de Docker. Únicamente la desinstalación de paquetes pide permisos, y solo para esa orden.

**¿Funciona sin Internet?** Sí. Solo las operaciones de Docker contra registros remotos necesitan red.

**¿Qué pasa con mis contenedores si cierro la aplicación?** Siguen funcionando: los gestiona Docker. Al cerrar se terminan las terminales abiertas y se dejan de seguir logs y estadísticas.

**¿Puedo perder datos al editar un contenedor?** No se elimina ningún volumen, carpeta ni imagen, y el original se conserva como copia. Solo los archivos escritos fuera de los volúmenes se quedan en la copia, y la revisión te lo avisa antes.

**¿Se pierden mis datos al desinstalar?** No. Se conservan tu configuración, tu historial y todo lo de Docker.

**¿Dónde se guardan la configuración y el historial?** En `~/.config/container-manage-center/` y `~/.local/state/container-manage-center/`.

**¿Cómo cambio el idioma?** En Configuración › Apariencia › Idioma.

**¿Siguen funcionando las copias programadas con la aplicación cerrada?** Sí: las ejecuta el sistema (systemd o el Programador de tareas de Windows), con tu sesión iniciada.

## 33. Glosario {#glossary}

| Término | Significado |
|---|---|
| Contenedor | Un programa en ejecución aislado del resto del equipo, creado a partir de una imagen |
| Imagen | Plantilla de solo lectura con el sistema de archivos y la configuración para crear contenedores |
| Etiqueta de imagen (*tag*) | Nombre de una versión de una imagen, como `nginx:1.27` |
| Etiqueta de contenedor (*label*) | Dato `clave=valor` guardado en un contenedor, como `cmc.description` |
| Registro | Servidor del que se descargan imágenes, como Docker Hub |
| Volumen | Almacenamiento que gestiona Docker y que se conserva aunque se elimine el contenedor |
| Directorio vinculado (*bind mount*) | Carpeta o archivo del equipo montado dentro de un contenedor |
| Red | Red virtual que comunica contenedores entre sí y con el exterior |
| Puerto publicado | Puerto del equipo que lleva a un puerto del contenedor |
| Política de reinicio | Qué hace Docker cuando un contenedor termina o el equipo arranca |
| Docker Engine | El servicio que ejecuta los contenedores; CMC habla con él |
| API de Docker | Interfaz con la que se controla Docker Engine; la usan la CLI de Docker y CMC |
| Socket Unix | Archivo especial para comunicarse con Docker en el mismo equipo (`/var/run/docker.sock`) |
| Rootless | Docker ejecutado con un usuario normal, sin privilegios de administrador |
| Contexto de Docker | Configuración con nombre de a qué Docker se conecta la CLI (`docker context`) |
| `DOCKER_HOST` | Variable de entorno que indica a qué Docker conectarse |
| Docker Compose | Herramienta para definir y ejecutar aplicaciones de varios contenedores con un archivo YAML |
| Servicio y perfil (Compose) | Un servicio es cada parte de un proyecto Compose; un perfil agrupa servicios opcionales |
| Docker Exec | Ejecutar un programa dentro de un contenedor que ya está en marcha |
| `stdout` y `stderr` | Salida normal y salida de errores de un programa |
| Huella (*fingerprint*) | Resumen de la clave de un servidor SSH (`SHA256:…`) para comprobar su identidad |
| Almacén seguro (llavero) | Lugar del sistema donde se guardan contraseñas cifradas (GNOME Keyring, KWallet, Administrador de credenciales) |
| Docker Desktop | Aplicación de Docker para Windows y macOS |
| Tubería con nombre | Mecanismo de Windows con el que se conecta a Docker Desktop (`\\.\pipe\docker_engine`) |

## 34. Información legal y licencia {#legal}

**Nombre:** Container Manage Center (abreviado **CMC**).

**Licencia:** GNU General Public License, versión 3 o posterior (identificador SPDX `GPL-3.0-or-later`).

**Copyright © 2026 Sergio Alfaro Lorite.**

Este programa es software libre: puede redistribuirlo y/o modificarlo bajo los términos de la Licencia Pública General de GNU publicada por la Free Software Foundation, ya sea la versión 3 de la Licencia o (a su elección) cualquier versión posterior. Se distribuye con la esperanza de que sea útil, pero SIN NINGUNA GARANTÍA; ni siquiera la garantía implícita de COMERCIABILIDAD o IDONEIDAD PARA UN PROPÓSITO PARTICULAR. Consulte la Licencia Pública General de GNU para más detalles.

Texto completo de la licencia: https://www.gnu.org/licenses/gpl-3.0.html (también en `/usr/share/doc/container-manage-center/LICENSE`).

**Componentes de terceros.** La aplicación incluye bibliotecas y tipografías de terceros que conservan sus propias licencias (MIT, Apache-2.0, MPL-2.0, OFL-1.1 y otras) y los avisos de copyright de sus autores; no pasan a ser GPL ni de Sergio Alfaro Lorite. Sus textos completos están en `/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md`, y el aviso de copyright del paquete en `/usr/share/doc/container-manage-center/copyright`.

**Código fuente.** El código fuente correspondiente se publica junto con cada versión del paquete (`container-manage-center-X.Y.Z-src.tar.gz`).

**Marcas.** Container Manage Center es un proyecto independiente: no está afiliado a Docker, Inc. ni ha sido respaldado por ella. «Docker» es una marca de Docker, Inc. y aquí solo se usa para nombrar la tecnología con la que trabaja la aplicación (Docker Engine, su API, Docker Compose y la CLI de Docker).

**Windows.** El instalador de Windows incluye el cargador de WebView2 de Microsoft, cubierto por un permiso adicional de la GPL v3 §7.

Los mismos datos están en **Configuración › Legal**.
