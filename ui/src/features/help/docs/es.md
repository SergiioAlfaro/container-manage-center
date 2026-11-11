## 1. Qué es Docker Control Center {#about}

Docker Control Center es una aplicación de escritorio para administrar **Docker Engine** en Linux sin depender de la terminal. Todo lo que muestra son datos reales que pide a Docker en ese momento, y cada acción es una operación real: nada se simula.

- Habla con Docker Engine por su **socket Unix**, con la API oficial.
- Ejecuta **Docker Compose** con su programa oficial, con argumentos separados y sin shell.
- Cada operación aparece en el **panel de actividad** con su comando `docker` equivalente, para que sepas exactamente qué se ha hecho.

La ventana tiene una **barra lateral** con los apartados (se contrae con Ctrl+B), el **área de trabajo**, una **barra superior** con el indicador de conexión y el botón **Actividad**, y el **panel de actividad** a la derecha (Ctrl+Mayús+A).

## 2. Requisitos del sistema y dependencias {#requirements}

- **Sistema**: Debian 12 o superior, o Ubuntu 22.04 o superior, de 64 bits (x86_64).
- **Docker Engine 20.10 o posterior** (API 1.41+), en modo normal o rootless.
- **Docker Compose v2 o posterior**, solo para el apartado Docker Compose (como plugin `docker compose` o binario `docker-compose` v2). Compose v1 no se admite.
- **Dependencias de ejecución**: WebKitGTK 4.1, GTK 3, glibc y bibliotecas relacionadas. Las instala APT con el paquete.

La aplicación no necesita Internet. Solo las operaciones que hace Docker Engine contra registros remotos (descargar imágenes, buscar en Docker Hub, construir con imágenes base que no estén en el equipo) necesitan que Docker tenga red.

## 3. Instalación y primera ejecución {#install}

Instala el paquete con APT, que resuelve las dependencias:

```
sudo apt install ./docker-control-center_X.Y.Z_amd64.deb
```

La aplicación aparece en el menú como **Docker Control Center**. Se ejecuta con tu usuario normal: **no la ejecutes con sudo** (se niega a arrancar como root).

Al abrirla, busca Docker Engine automáticamente. Si lo encuentra, el **Dashboard** muestra su estado; si no, muestra el diagnóstico con la causa y los pasos para resolverla.

Para actualizar, instala el paquete nuevo de la misma forma: se conservan tu configuración y tu historial.

## 4. Configuración inicial {#first-setup}

No hace falta configurar nada para empezar: la conexión con Docker es automática. En **Configuración** puedes elegir:

- El **tema** (Sistema, Claro u Oscuro), el **color principal** (verde azulado o morado) y el **idioma**.
- Si el panel de actividad se abre solo cuando una operación falla y cuántas operaciones conserva.
- Si se guarda el historial de operaciones y eventos entre sesiones.
- Un socket concreto de Docker, si no quieres la detección automática.

Todo se guarda al momento en `~/.config/docker-control-center/config.toml`.

## 5. Conexión con Docker Engine {#connection}

El **indicador de conexión** de la barra superior muestra el estado:

| Aspecto | Significado |
|---|---|
| Punto verde y `Docker 29.x` | Conectado. Al pasar el ratón se ve el socket |
| `Conectando con Docker…` | Detectando o conectando |
| Rojo, `Sin conexión con Docker` | Sin conexión. Púlsalo para ver el diagnóstico |

En modo **automático**, la aplicación prueba por este orden: la variable `DOCKER_HOST` (solo `unix://`), el contexto activo de Docker, tu socket rootless y el socket del sistema `/var/run/docker.sock`. Vigila la conexión cada 10 segundos y vuelve a conectar sola cuando Docker vuelve a estar disponible.

La aplicación nunca pide sudo, no cambia permisos ni grupos y no ejecuta por ti las instrucciones del diagnóstico. Tu usuario necesita poder abrir el socket: con Docker rootless o perteneciendo al grupo `docker` (lo que equivale en la práctica a tener privilegios de administrador).

## 6. Docker rootless y conexiones mediante socket {#rootless}

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

## 8. Dashboard y diagnóstico {#dashboard}

Con conexión, el **Dashboard** muestra datos obtenidos en ese momento: versión de Docker Engine y de la API, socket y origen de la conexión, contenedores (en ejecución, en pausa, detenidos), imágenes, datos del equipo (sistema, núcleo, CPU, memoria), almacenamiento y ejecución (controlador, directorio de datos, cgroups, logs) y los avisos del daemon. **Actualizar** vuelve a pedirlos.

Sin conexión muestra el **diagnóstico**: la causa concreta, los pasos para resolverla (con las órdenes para copiar en una terminal), los sockets comprobados con su resultado y lo que se ha detectado instalado. Los botones **Volver a conectar** y **Configurar conexión** están en la cabecera.

## 9. Gestión de contenedores {#containers}

El apartado **Contenedores** lista todos los contenedores, como `docker ps -a`: estado, nombre e ID corto, proyecto Compose, imagen, estado de Docker, puertos publicados y antigüedad. Puedes filtrar por **Todos / En ejecución / Detenidos** y buscar por nombre, imagen, ID o proyecto. El menú **⋯** de cada fila (o el botón derecho) ofrece **Editar configuración** y **Crear imagen a partir de este contenedor**.

| Estado | Acciones disponibles |
|---|---|
| En ejecución | Detener, Reiniciar, Pausar, Eliminar |
| En pausa | Reanudar, Detener, Eliminar |
| Creado o detenido | Iniciar, Eliminar |

Detener y reiniciar esperan 10 segundos a que el proceso termine antes de forzarlo, como Docker. Tras cada acción se consulta el estado real del contenedor. Si Docker rechaza una acción, su mensaje aparece encima de la tabla y en el panel de actividad. Las listas se actualizan solas cuando algo cambia en Docker, también desde la terminal.

## 10. Creación, duplicación, inspección y eliminación de contenedores {#container-lifecycle}

**Crear contenedor** abre un asistente con seis pestañas (solo la imagen es obligatoria): General, Entorno, Red, Almacenamiento, Recursos y Avanzado.

- **Comando** y **Entrypoint** se escriben como en una terminal y se dividen en argumentos, que ves debajo. No se ejecuta ninguna shell.
- El panel **Equivalente** muestra el `docker run` equivalente y el cuerpo exacto que se envía a la API.
- **Revisión** marca los errores (impiden crear) y las advertencias (puertos privilegiados, carpetas delicadas con escritura, modo privilegiado…).
- **Crear** crea el contenedor sin iniciarlo; **Crear e iniciar** también lo inicia.

Al pulsar el nombre de un contenedor se abre su **detalle**: Resumen, Logs, Terminal, Estadísticas e Inspección JSON (la respuesta completa de `docker inspect`, con botón Copiar).

**Duplicar** abre el asistente con la configuración del contenedor y el nombre `<nombre>-copia`, y avisa de los ajustes que no se trasladan.

**Eliminar** pide confirmación. Un contenedor en ejecución o en pausa solo se elimina si marcas **Detenerlo a la fuerza y eliminarlo**. Opcionalmente se eliminan sus volúmenes anónimos; los volúmenes con nombre y las carpetas vinculadas no se tocan. No se puede deshacer.

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
- **Descripción**: se guarda en la etiqueta `dcc.description` del contenedor.

**Revisar cambios** compara la configuración actual con la nueva: qué cambia (antes y después, elemento a elemento), qué se mantiene y qué requiere recrear. Nada se aplica hasta que lo confirmas.

**Recreación segura.** Cuando un cambio exige recrear, el asistente:

1. Comprueba la imagen, los conflictos de nombre y de puertos con otros contenedores y los motivos que impiden recrear con seguridad: contenedores de Compose o Swarm, creados con `--rm`, en pausa o reiniciándose. En esos casos no deja continuar.
2. Muestra los riesgos que debes aceptar uno a uno: archivos escritos fuera de los volúmenes (no pasan al contenedor nuevo), montajes que dejan de estar montados, contenedores que usan la red de este…
3. Al confirmar: detiene el contenedor si está en marcha, le cambia el nombre a `<nombre>-dcc-backup-<fecha>` (copia de seguridad), crea el nuevo con **toda** la configuración original más tus cambios, conecta sus redes y, si estaba en marcha, lo inicia y comprueba que sigue funcionando unos segundos.
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

## 13. Logs, estadísticas y terminales {#logs-stats-terminal}

En el detalle de un contenedor:

- **Logs**: salida en directo (stderr en rojo). Puedes elegir cuántas líneas anteriores cargar, filtrar, ver las marcas de tiempo, seguir el final, copiar lo visible y dejar de seguir. Se conservan las últimas 10 000 líneas.
- **Estadísticas**: CPU, memoria, red, disco y procesos cada segundo, calculados como `docker stats`.
- **Terminal**: **Abrir terminal** inicia una shell con `docker exec -it` (Automática usa `bash` si existe y si no `sh`; también puedes elegir una). **Cerrar sesión** termina la shell y los procesos que dejó dentro del contenedor.

Son operaciones reales que aparecen en el panel de actividad mientras están abiertas y se cierran al salir del detalle. Para trabajar con varias terminales a la vez, usa el apartado **Terminales**.

## 14. Gestión de imágenes {#images}

El apartado **Imágenes** lista las imágenes locales, como `docker image ls`: etiquetas, ID corto, tamaño, antigüedad y qué contenedores las usan. Filtros **Todas / En uso / Sin uso / Sin etiqueta** y búsqueda por etiqueta o ID.

- **Detalle**: Resumen (con los valores sensibles de las variables ocultos), Capas (como `docker image history`), Contenedores que la usan e Inspección JSON.
- **Etiquetar** añade un nombre `repositorio:etiqueta`.
- **Eliminar**: si la imagen tiene varias etiquetas puedes quitar solo una; la imagen completa exige forzar. Docker no elimina imágenes de contenedores en ejecución.
- **Limpiar** elimina las imágenes sin etiqueta que no usa ningún contenedor o, con confirmación adicional, todas las que no se usan.

## 15. Descarga y construcción de imágenes {#pull-build}

**Descargar imagen** pide la referencia (`nginx:1.27`, `usuario/app`, `registro.ejemplo.com:5000/app:v2`; sin etiqueta se usa `latest`). Credenciales:

| Opción | Qué hace |
|---|---|
| Las de Docker | Usa lo guardado con `docker login` (o su *credential helper*) |
| Ninguna | Descarga anónima |
| Usuario y contraseña | Solo para esta descarga: no se guardan ni aparecen en el panel de actividad |

El panel **Descargas** muestra el progreso total y por capa; puedes cambiar de apartado mientras tanto y **Cancelar**. **Buscar en Docker Hub** busca como `docker search` (lo hace Docker Engine, que necesita Internet).

**Construir imagen** abre un asistente: contexto (la carpeta que se envía, respetando `.dockerignore`) y Dockerfile, nombres (`--tag`), argumentos (`--build-arg`), metadatos (`--label`), etapa final, sin caché, buscar imágenes base nuevas y constructor (**BuildKit** o **clásico**). Se ven los pasos con su estado y la salida como `docker build --progress=plain`; se puede cancelar.

## 16. Redes Docker {#networks}

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

## 18. Directorios vinculados (Bind Mounts) {#binds}

Este apartado muestra las carpetas y archivos de tu equipo montados dentro de contenedores, agrupados por ruta: tipo, permisos y propietario, si existen, qué contenedores los montan, dónde y con qué acceso. Se señalan los que requieren atención: el **socket de Docker** (da control de Docker al contenedor), la raíz o directorios del sistema y las rutas que ya no existen.

Docker no permite añadir una carpeta a un contenedor que ya existe. **Vincular una carpeta…** abre el selector de carpetas y después el asistente de contenedores con esa carpeta montada en `/mnt/<nombre>`.

## 19. Docker Compose {#compose}

El apartado **Docker Compose** necesita Compose v2 o posterior; si no está instalado, explica cómo instalarlo. Todas las operaciones ejecutan el Docker Compose oficial y aparecen en el panel de actividad con el comando exacto.

La lista muestra los proyectos con contenedores en Docker (también los desplegados desde la terminal) y los que añades, con su estado (en marcha, parcial, detenido o sin desplegar), carpeta y servicios.

- **Abrir proyecto…**: elige su `compose.yaml` (y los archivos adicionales en el orden en que se combinan).
- **Nuevo proyecto**: elige una carpeta, escribe el contenido (se parte de una plantilla) y **Crear proyecto**: se valida con Docker Compose antes de crear `compose.yaml`.
- **Quitar de la lista** solo la quita de la lista; no toca Docker ni los archivos.

## 20. Gestión de proyectos y perfiles Compose {#compose-projects}

En el detalle de un proyecto:

- **Servicios**: cada servicio con su imagen, estado, contenedores, puertos y **perfiles**. Cada fila permite desplegar, iniciar, detener, reiniciar o ver los registros de ese servicio.
- **Desplegar** (`docker compose up --detach`): todos los servicios o los elegidos, **perfiles** que activar, construir antes, política de descarga de imágenes, volver a crear contenedores y eliminar huérfanos. El diálogo muestra el comando exacto.
- **Iniciar, Detener, Reiniciar**, **Descargar imágenes** y **Construir**.
- **Eliminar** (`docker compose down`): contenedores y redes del proyecto; opcionalmente sus volúmenes con nombre (sus datos se pierden) y sus imágenes. Hay que confirmarlo.
- **Salida** de la operación en curso (se puede **Interrumpir**) y **Registros** de los servicios en directo.
- **Archivo**: edita los archivos; **Validar** los comprueba con Docker Compose sin guardar, y **Guardar** valida antes y no sobrescribe si otro programa los cambió.
- **Configuración**: la configuración final según Docker Compose. Puede contener secretos, así que está oculta hasta que la pides y no se guarda en el panel de actividad.

## 21. Terminales simultáneas {#terminals}

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

## 22. Panel de actividad y registro de operaciones {#activity}

Cada operación aparece en el **panel de actividad**: hora, distintivo **API** (petición a Docker Engine por el socket), **CLI** (proceso, como `docker compose` o `apt-get`) o **APP** (operación de la propia aplicación, como un cambio de idioma o de color, o el resumen de una recreación cuyos pasos aparecen aparte), tipo, recurso, duración y estado. La línea principal es el comando equivalente, que puedes copiar. Al desplegar una entrada ves la operación ejecutada de verdad, el resultado, el error y su código HTTP si falló, y las salidas de los procesos.

El apartado **Eventos y registros** muestra los eventos de Docker en directo, los guardados, la consulta a Docker de un periodo (`docker events --since … --until …`; Docker solo conserva los últimos 256) y el **historial de operaciones** de esta y otras sesiones.

El historial guarda qué se hizo, cuándo y con qué resultado, pero nunca el cuerpo de las peticiones, el comando ni las salidas, porque pueden contener contraseñas. Está en `~/.local/state/docker-control-center/`, solo legible por tu usuario.

## 23. Configuración, temas e idiomas {#settings}

- **Apariencia**: tema (Sistema, Claro, Oscuro), **color principal** (verde azulado o **morado**; se aplica al momento a botones, selección, indicadores y navegación) e idioma, por este orden: **English**, **Español** (predeterminado), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** y **Português**. Cada idioma aparece con su propio nombre. El idioma y el color se guardan, se mantienen al volver a abrir la aplicación y sus cambios quedan en el panel de actividad.
- **Panel de actividad**: abrirlo ante errores, operaciones conservadas (50–10 000) y **Guardar el historial entre sesiones**.
- **Conexión con Docker** y comprobación de **Docker Compose**.
- **Actualizaciones**: dónde se publican las versiones y **Comprobar ahora**. La aplicación nunca descarga ni instala nada.
- **Sistema**: versiones de la aplicación, del paquete, de WebKitGTK y glibc, del sistema y de Docker; **Copiar** genera un informe sin datos sensibles.
- **Desinstalación y componentes** y **Ayuda y documentación**.
- **Restablecer las preferencias**, tras confirmarlo. No cambia la conexión con Docker.

| Atajo | Acción |
|---|---|
| Ctrl+B | Contraer o expandir la barra lateral |
| Ctrl+Mayús+A | Abrir o cerrar el panel de actividad |
| Escape | Cerrar un diálogo |

Los mensajes que genera el propio Docker (por ejemplo, un error de Docker Engine) y la salida de los programas se muestran tal como llegan, sin traducir. Los resúmenes técnicos que el motor de la aplicación escribe en el panel de actividad (por ejemplo, cómo terminó una terminal) y los mensajes de error con datos concretos se generan en español en todos los idiomas.

## 24. Desinstalación y gestión de dependencias {#uninstall}

**Configuración › Desinstalación y componentes** muestra qué instaló la aplicación y permite quitarlo sin usar la terminal. **Analizar** consulta dpkg y APT sin cambiar nada:

- **Paquete de la aplicación** (`docker-control-center`), con su espacio.
- **Dependencias que se pueden quitar**: solo las que APT instaló automáticamente **junto con la aplicación** (según su historial) y que ningún otro paquete necesita.
- **Se conservan**, con el motivo: dependencias compartidas (con los paquetes que las usan), instaladas a mano, que ya existían antes, de origen incierto (si el historial de APT no muestra cómo se instaló la aplicación) y componentes del sistema.
- **Herramientas externas**: Docker Engine, Docker CLI y Docker Compose. **Se conservan siempre.**
- **Datos que no se borran**: tu configuración, tu historial y los datos de la ventana. Tampoco se toca nada de Docker: imágenes, contenedores, redes, volúmenes y proyectos Compose.
- La **simulación de APT** completa de la desinstalación.

Puedes elegir **Solo Docker Control Center** o **con las dependencias que se pueden quitar**, o marcar los paquetes uno a uno; el resumen indica cuánto espacio se recupera. **Revisar y desinstalar…** vuelve a comprobar la selección y simula exactamente la orden: si APT fuera a quitar algo que no has elegido, no se puede continuar. El diálogo muestra la orden que se va a ejecutar:

```
pkexec /usr/bin/apt-get remove -y --no-auto-remove docker-control-center …
```

Al confirmar, el sistema pide tu contraseña de administrador (pkexec) **solo para esa orden**; la aplicación sigue sin privilegios. Si cancelas la autenticación no se quita nada. No se usa `purge`. Todo queda en el panel de actividad. Si se quita la propia aplicación, ciérrala al terminar.

Si `pkexec` no está instalado, la aplicación no puede pedir permisos: el diálogo muestra la orden equivalente con `sudo` para que la ejecutes tú en una terminal.

## 25. Seguridad y tratamiento de credenciales {#security}

- La aplicación **no se ejecuta como root**, nunca pide sudo para funcionar y no cambia permisos del socket ni grupos.
- Los comandos externos se ejecutan **sin shell** y con argumentos separados: lo que escribes nunca se interpreta como órdenes.
- **No guarda credenciales.** Las de un registro se usan solo para una descarga y no aparecen en el panel de actividad. Con «Las de Docker» se usan las de `docker login`.
- Los valores de variables como `PASSWORD`, `TOKEN` o `SECRET` se ocultan en pantalla y en el panel (`********`), aunque Docker los recibe completos.
- El historial persistente nunca guarda cuerpos de peticiones, comandos ni salidas.
- Las operaciones destructivas piden confirmación.
- La ventana bloquea conexiones externas. La única conexión propia es la comprobación de versiones, desactivada mientras no indiques una fuente. El botón de GitHub de la Ayuda solo puede abrir esa dirección, en tu navegador.
- Montar el socket de Docker en un contenedor o pertenecer al grupo `docker` da control total del equipo: la aplicación lo señala.

## 26. Solución de problemas frecuentes {#troubleshooting}

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

## 27. Preguntas frecuentes {#faq}

**¿La aplicación ejecuta comandos `docker`?** No: usa la API de Docker Engine. El comando que ves en el panel de actividad es el equivalente, para que puedas reproducirlo. Docker Compose sí se ejecuta con su programa oficial.

**¿Necesito ser administrador?** No. Solo necesitas acceso al socket de Docker. Únicamente la desinstalación de paquetes pide permisos, y solo para esa orden.

**¿Funciona sin Internet?** Sí. Solo las operaciones de Docker contra registros remotos necesitan red.

**¿Qué pasa con mis contenedores si cierro la aplicación?** Siguen funcionando: los gestiona Docker. Al cerrar se terminan las terminales abiertas y se dejan de seguir logs y estadísticas.

**¿Puedo perder datos al editar un contenedor?** No se elimina ningún volumen, carpeta ni imagen, y el original se conserva como copia. Solo los archivos escritos fuera de los volúmenes se quedan en la copia, y la revisión te lo avisa antes.

**¿Se pierden mis datos al desinstalar?** No. Se conservan tu configuración, tu historial y todo lo de Docker.

**¿Dónde se guardan la configuración y el historial?** En `~/.config/docker-control-center/` y `~/.local/state/docker-control-center/`.

**¿Cómo cambio el idioma?** En Configuración › Apariencia › Idioma.
