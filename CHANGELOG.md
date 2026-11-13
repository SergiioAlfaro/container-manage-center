# Cambios

## 1.3.0 — 2026-11-13 (evolución, Etapa 3: auditoría de seguridad)
- **Auditoría de seguridad** (sección nueva en «Herramientas», D-093): revisa todos los contenedores o uno, las imágenes y los proyectos Compose del servidor elegido, también remoto, **sin cambiar nada**. Comprueba:
  - los riesgos para el equipo (socket de Docker, montajes, capacidades, modo privilegiado, red, PID, IPC, opciones de seguridad, dispositivos y puertos);
  - el usuario root, el sistema de archivos modificable, la falta de límites de memoria, CPU y procesos, `no-new-privileges` y las imágenes antiguas.

  Cada hallazgo tiene nivel, explicación, por qué importa y recomendación. Se puede filtrar por nivel, tipo y texto.
- **Correcciones propuestas**: abren «Editar configuración» con los cambios ya hechos en el borrador (puerto en 127.0.0.1, carpeta en solo lectura) o en la pestaña de límites. Nada se aplica sin revisar y confirmar.
- **Vulnerabilidades con Trivy o Grype** (D-094), si están instalados y tienen su base: recuentos por gravedad y fecha de la base. La aplicación no los instala ni descarga su base. Sin ellos, o en un servidor remoto, el informe dice «No analizado», nunca «0».
- **Historial y exportación** (D-095): las auditorías se guardan por servidor (0600) y se exportan a JSON, Markdown o HTML, sin secretos y con permisos 0600.
- **Verificación**: `scripts/verify-evolution-1.3.0.sh`, con toda la regresión anterior y las pruebas de la auditoría contra Docker real, por IPC y por SSH.
- **Corrección** (terminales): al cerrar una sesión cuyo programa ignora Ctrl+C, el resumen del panel de actividad podía decir que el proceso seguía en marcha aunque la aplicación ya lo había terminado. Docker 24 tarda un momento en reflejarlo; ahora se espera hasta 2 s antes de concluirlo.

## 1.2.0 — 2026-11-13 (evolución, Etapa 2: resto del endurecimiento)
Resto de hallazgos de `docs/security/SECURITY_AUDIT.md` del plan de evolución (E2), corregidos y comprobados con pruebas.
- **Más riesgos para el equipo** (DCC-SEC-04 y DCC-SEC-08, D-088), en crear, editar y Compose: compartir `pid`/`ipc` (alto), `userns`/`cgroup` (medio) o `uts` (bajo) con el equipo; `security_opt` que desactiva seccomp, AppArmor, SELinux o `systempaths` (alto); dispositivos (`/dev/mem` crítico, discos alto); `cgroup_parent`; y puertos publicados en todas las interfaces (aviso bajo con la sugerencia de `127.0.0.1`).
- **Archivos Compose** (DCC-SEC-06 y DCC-SEC-13, D-089): el editor solo abre y guarda archivos `.yml`/`.yaml` fuera de carpetas ocultas (`~/.ssh`, `~/.config`…) y del sistema (`/etc`, `/root`…), también si son enlaces a ellas, en este equipo y en los servidores. Guardar un `compose.yaml` que es un enlace simbólico ya no lo convierte en un archivo normal.
- **Credenciales de registro en servidores remotos** (DCC-SEC-07, D-090): al descargar o construir en un servidor, las credenciales de `docker login` de este equipo solo se envían si marcas la casilla (desmarcada por omisión). Sin ella, la descarga es anónima.
- **Enlaces de las terminales** (DCC-SEC-12, D-091): un enlace de la salida se abre en el navegador del sistema solo después de confirmarlo, viendo la dirección completa, y solo si es `http://` o `https://`.
- **Superficie de la aplicación** (DCC-SEC-11, 15–18, D-091): el programa publicado no arranca como root ni con `DCC_ALLOW_ROOT`; permisos de Tauri reducidos a lo que usa la interfaz; la página de una versión solo se abre desde github.com o la fuente de actualizaciones; documentados el estilo en línea de la CSP y el requisito de shell POSIX en los servidores.
- **Cadena de suministro** (DCC-SEC-09, 10, 14, D-092): `install.sh` comprueba el paquete con `SHA256SUMS` (`scripts/release-sums.sh` lo genera); `deny.toml` versionado con excepciones justificadas; `cargo deny`, `npm audit` y ESLint en la verificación; vitest 5 (sin los avisos de `tinypool`). La firma GPG queda pendiente de la clave del propietario.
- **Verificación**: `scripts/verify-evolution-1.2.0.sh`, con toda la regresión de las fases 1–10, de la 1.1.0 y las pruebas de la Etapa 2.

## 1.1.0 — 2026-11-12 (evolución, Etapa 1: correcciones de seguridad prioritarias)
Hallazgos de `docs/security/SECURITY_AUDIT.md` corregidos y comprobados con pruebas.
- **Riesgos para el equipo** (DCC-SEC-01 y DCC-SEC-03, D-086): reglas únicas para crear, editar, desplegar con Compose y la pantalla de directorios vinculados.
  - El **socket de Docker** (o de containerd o Podman) se marca como crítico **también en solo lectura**; antes no avisaba.
  - Se detectan rutas sin normalizar (`//etc`), enlaces simbólicos (con Docker local), carpetas con credenciales (`/etc`, `/root`, `/home`, `~/.ssh`, `~/.docker`…) y del sistema.
  - Capacidades peligrosas (`ALL`, `SYS_ADMIN`, `SYS_PTRACE`, `NET_ADMIN`, `DAC_OVERRIDE`, `NET_RAW`…), modo privilegiado y red del equipo.
  - Los riesgos altos y críticos se aceptan uno a uno; el motor lo comprueba otra vez y rechaza la operación si falta alguno. Sin avisos en las configuraciones habituales legítimas.
- **Secretos** (DCC-SEC-05, D-085): ocultación centralizada en el panel de actividad, el historial, la orden equivalente, las salidas, los errores y los registros de la aplicación. Ahora también `MYSQL_PWD`, `DB_PASS`, `PGPASS`, `REDIS_AUTH`, `SMTP_PASS`, la contraseña de cualquier URL o DSN, las claves privadas PEM, `Authorization` y `--password`/`--token`. Los registros de los contenedores y las terminales no se alteran.
- **Servidor intermedio** (DCC-SEC-02, D-087): el salto tiene las mismas reglas que el servidor. Su huella se confirma y se guarda en la lista de la aplicación, solo admite clave o agente y nunca recibe la contraseña del servidor (antes, con `ssh -J`, podía recibirla). Un solo salto.
- **Verificación**: `scripts/verify-evolution-1.1.0.sh`, con toda la regresión de las fases 1–10 y las pruebas de la Etapa 1. El laboratorio SSH de desarrollo permite usar el servidor A como intermedio.

## 1.0.0 — 2026-11-12 (Fase 10: pruebas finales y distribución)
- **Color principal: solo morado** (D-082). Se elimina el verde azulado como color principal en el sistema central de temas, en los temas claro y oscuro. Configuración ya no tiene selector de color. Si tenías el verde azulado guardado, pasa a morado conservando el resto de tus preferencias. Los indicadores de estado (verde para lo que funciona, ámbar para avisos, rojo para errores) no cambian.
- **Logotipo e iconos en morado**, con la misma forma.
- **Paquete** (D-083):
  - versión 1.0.0;
  - en `/usr/share/doc/docker-control-center/`: `copyright`, `changelog`, guía, instalación y componentes de terceros con sus licencias (`THIRD-PARTY.md`);
  - el icono de 256 px se instala en la carpeta estándar.
- **`scripts/install.sh`**: instalación con el usuario normal (sudo solo para apt). Elige el paquete adecuado para el sistema (normal o compatible con Debian 12/Ubuntu 22.04) y comprueba, sin cambiar nada, Docker, Compose, SSH y el almacén de contraseñas. También comprueba (`--check`) y desinstala (`--uninstall`).
- **`scripts/run.sh`**: abre la versión instalada o la compilación del código fuente.
- **Instalación limpia** probada en Ubuntu 24.04 y 22.04 recién creados (`scripts/dev/clean-install-test.sh`).
- **`scripts/verify-phase-10.sh`**: verificación final con toda la regresión. Un paso que filtra pruebas falla si no ejecuta ninguna (D-084).
- **Documentación**: README con la descripción del producto, INSTALL con el instalador, las dependencias y la desinstalación, y `LICENSE`.
- **Versiones anteriores**: 0.9.2 y 0.10.0 validadas por el propietario.

## 0.10.0 — 2026-11-11 (actualización previa a la Fase 10: administración remota)
- **Servidores remotos por SSH** (nuevo apartado **Servidores**, D-075–D-081):
  - Lista de servidores con nombre, dirección, usuario, puerto, método, estado (Conectado, Desconectado, Conectando, Error de autenticación, Error de conexión, Docker no disponible), versión de Docker y de la API, sistema, última conexión y latencia. El equipo local sigue como siempre («Este equipo»).
  - Asistente **Añadir servidor**: datos generales, autenticación (agente SSH, clave privada, clave con frase, contraseña desaconsejada), Docker (predeterminado, socket o contexto) y opciones avanzadas (tiempo de espera, salto `ProxyJump`, conexión al iniciar). La clave privada nunca se copia: se guarda su ruta.
  - **Probar conexión** con siete comprobaciones (DNS, conexión SSH, identidad, autenticación, Docker disponible, permisos, compatibilidad de la API) y cómo resolver cada problema. Nunca instala Docker ni cambia nada en el servidor.
  - Editar, cambiar credenciales, conectar/desconectar, diagnóstico por servidor y **Eliminar perfil**, que no borra nada del servidor.
- **Conexión segura** (D-075, D-076):
  - Cliente OpenSSH del sistema con `docker system dial-stdio`: Docker nunca se expone en TCP. Una sesión maestra por servidor y canales multiplexados.
  - Argumentos separados, opciones de seguridad siempre antes del destino y `--`; los argumentos remotos se entrecomillan uno a uno.
  - Huella del servidor confirmada en el primer contacto; un cambio detiene la conexión con aviso de posible suplantación y solo se acepta tras revisarla. Lista de huellas propia (0600); `~/.ssh/known_hosts` no se modifica.
  - Al arrancar se cierran las sesiones que dejó una instancia anterior.
- **Credenciales** (D-077): en el almacén seguro del sistema (Secret Service: GNOME Keyring, KWallet…) solo si se pide; sin almacén se avisa y se piden cada vez. Nunca en archivos, en el registro ni en los diagnósticos. Los perfiles van en `servers.toml`, separados de `config.toml` y de los secretos.
- **Paridad remota** (D-078–D-080): Dashboard, contenedores, imágenes, redes, volúmenes, carpetas vinculadas, edición de contenedores, eventos y Compose en el servidor elegido. Compose se ejecuta en el servidor con los archivos del servidor; las rutas se escriben y se comprueban allí, y antes de escribir un archivo se confirma el destino.
- **Varios servidores** (D-078, D-079):
  - Selector de servidor en la barra superior y **Vista general** de todos los servidores.
  - Cada operación va ligada a su servidor; un servidor caído no afecta a los demás.
  - Las confirmaciones muestran servidor y dirección.
- **Terminales** (D-081): terminales de contenedores de varios servidores a la vez, etiquetadas con servidor y contenedor, y **Shell SSH del servidor** como tipo de sesión aparte. Al desconectar o cerrar no quedan procesos.
- **Panel de actividad, eventos e historial**: servidor y método de conexión en cada entrada, orden equivalente (`docker --host ssh://…` o `ssh … -- …`), y filtro por servidor.
- **Ayuda**: nueva sección «Servidores remotos por SSH» en los ocho idiomas (27 secciones).
- **Paquete**: recomienda `openssh-client`.
- **Corrección (edición de contenedores, 0.9.2)**: la copia de seguridad que se crea al recrear es la que mostró la revisión, aunque el usuario confirme en otro segundo (antes podía crearse con un nombre un segundo posterior).

## 0.9.2 — 2026-11-11 (actualización previa a la Fase 10)
- **Editar configuración** de un contenedor (menú ⋯ de la lista, botón derecho y detalle; D-070):
  - Pestañas General, Red, Almacenamiento, Variables, Puertos, Recursos y Avanzado.
  - Cada cambio indica si se aplica directamente (nombre, reinicio, CPU, memoria, redes) o exige recrear (variables, puertos, montajes, descripción, quitar un límite). Lo que Docker no permite cambiar se muestra como solo lectura.
  - Revisión con la comparación actual/nuevo, lo que se mantiene y lo que exige recrear.
  - **Recreación segura**: bloqueos y riesgos que se aceptan uno a uno, confirmación, copia `-dcc-backup-` del original, comprobación de que el nuevo funciona y vuelta atrás automática si falla. Nunca se borran volúmenes, carpetas ni imágenes.
  - Copias de seguridad: restaurar (con confirmación) o eliminar.
  - Dirección MAC: se conserva solo si se pide o si es conocida (D-071).
- **Crear imagen a partir de este contenedor** (`docker commit`, D-069):
  - Repositorio, etiqueta, autor y mensaje, con validación y vista previa del nombre completo.
  - Lista de volúmenes y carpetas que no entran en la imagen.
  - Pausa opcional si el contenedor está en ejecución, sin detenerlo nunca.
  - Aviso y confirmación expresa si el nombre ya existe.
  - «Ver la imagen» la abre en Imágenes.
- **Color principal morado** en Configuración → Apariencia, junto al verde azulado de siempre (D-072):
  - Se aplica al momento, también a las terminales abiertas, en tema claro y oscuro, y se guarda.
  - La barra lateral tiene un acento propio con contraste suficiente.
  - El verde azulado claro se ajustó de forma imperceptible para cumplir AA con texto blanco (4,43 → 4,61:1).
- **Catalán, gallego, euskera y portugués** (pt-PT) en toda la interfaz y en la ayuda; el selector es una lista en el orden pedido (D-073).
- **Ayuda**: 26 secciones en los ocho idiomas, con «Editar la configuración de un contenedor» y «Crear una imagen a partir de un contenedor».
- **Panel de actividad**:
  - Distintivo APP para las operaciones propias de la aplicación: cambios de idioma y de color, y resumen de recreaciones y restauraciones.
  - Nuevas operaciones: `container.rename`, `container.update`, `container.recreate`, `container.restoreBackup` y `container.commit`.
  - Nunca registra los valores de variables sensibles.
- **Teclado**: menú de acciones accesible (flechas, Inicio, Fin, Escape) y grupos de opciones que se manejan con flechas (D-074).

## 0.9.1 — 2026-11-08 (actualización previa a la Fase 10)
- **Terminales** (nuevo apartado, D-064):
  - Varias terminales reales (`docker exec -it`) a la vez, en el mismo contenedor o en otros.
  - Distribución adaptable: 1, 2 columnas, 3, 2×2 y filas con desplazamiento a partir de 5.
  - Separadores que se arrastran o se mueven con el teclado.
  - Cada panel muestra el contenedor, un nombre de sesión que se puede cambiar y el estado de conexión, y se puede maximizar, cerrar y volver a abrir.
  - «Cerrar todas» pide confirmación.
  - Las sesiones siguen abiertas al cambiar de apartado.
- **Limpieza de las sesiones de terminal** (D-063):
  - Al cerrar una sesión se terminan también los procesos que dejó dentro del contenedor.
  - Se indica si el contenedor se detuvo o se eliminó.
  - Al cerrar la aplicación se cierran todas las sesiones y se espera a que terminen.
  - También se aplica a la terminal del detalle del contenedor.
- **Configuración → Desinstalación y componentes** (D-065):
  - Muestra el paquete de la aplicación, las dependencias que se pueden quitar y las que se conservan (compartidas, explícitas, previas, de origen incierto o del sistema, con el motivo).
  - Docker Engine, CLI y Compose se muestran como herramientas externas que se conservan siempre. Los datos del usuario no se tocan.
  - Simulación de APT, resumen con el espacio que se libera y confirmación.
  - La desinstalación es real: `pkexec apt-get remove --no-auto-remove`, sin `purge`.
- **Configuración → Ayuda y documentación** (D-066): documento de 24 secciones en los cuatro idiomas, con índice, búsqueda y navegación, y el botón «Visitar mi perfil de GitHub» (plugin opener de Tauri, limitado a esa dirección).
- **Francés e italiano** en toda la interfaz (D-067).
- **Estados de interacción** (D-068):
  - Estados de encima, pulsado, ocupado y desactivado coherentes; los botones desactivados siguen mostrando su descripción.
  - Las filas de las tablas se resaltan al pasar el ratón.
  - Todos los botones con solo un icono tienen nombre accesible y descripción.
- **Panel de actividad**: registra las nuevas operaciones:
  - `container.exec.cleanup`.
  - `packages.query`, `packages.simulate` y `packages.remove`.

## 0.9.0 — 2026-11-08 (Fase 9: configuración y optimización)
- **Compatibilidad con Debian 12 y Ubuntu 22.04** (D-059):
  - El paquete declara sus dependencias reales con `dpkg-shlibdeps`, incluida la versión mínima de `libc6`. Antes, un paquete compilado en un sistema más nuevo se instalaba en Debian 12 o Ubuntu 22.04 y no arrancaba («GLIBC_2.39 not found»).
  - `scripts/build-deb-compat.sh` compila el paquete para distribuir en un contenedor de Docker con Ubuntu 22.04: exige glibc 2.34 como mucho y se ha comprobado que funciona en un Ubuntu 22.04 real.
- **Configuración → Actualizaciones** (D-060): comprueba si hay una versión nueva en un repositorio de GitHub o en un JSON por HTTPS y avisa al iniciar si se activa. Nunca descarga ni instala nada, y por defecto no se conecta a Internet.
- **Configuración → Sistema**: versiones de la aplicación, el paquete, WebKitGTK, glibc, el sistema, Docker y Compose, con un informe para copiar.
- **Restablecer las preferencias** de apariencia, panel de actividad y actualizaciones, con confirmación.
- **Rendimiento** (D-061):
  - Durante una ráfaga de cambios hechos desde fuera (100 contenedores arrancados desde la terminal con 200 en la lista), la interfaz consume 7 veces menos CPU (de 23,0 s a 3,1 s) y hace un tercio de las peticiones.
  - Solo se repintan las filas que cambian y se recarga como mucho una vez por segundo durante las ráfagas.
  - El JavaScript inicial pasa de 1044 KB a 500 KB: los apartados y la terminal se cargan al abrirlos y se precargan tras el arranque.
- **Corregido**: un evento desplegado en «En directo» se cerraba al llegar otro, porque se volvían a crear todas las filas con cada evento.
- **Errores en el idioma de la interfaz** (los de texto fijo, como «Not connected to Docker Engine.») y **un único formato de fecha y hora** de 24 horas en el panel de actividad, los eventos, el historial y los logs (D-062).
- `verify-phase-9.sh` con `--deb` (dependencias reales del paquete), `--compat` (paquete compatible) y `--net` (HTTPS real), y comprobaciones manuales M1–M15.

## 0.8.0 — 2026-11-07 (Fase 8: actividad, eventos y monitorización)
- Apartado **Eventos y registros** con cuatro pestañas:
  - **En directo**: los eventos de Docker según ocurren (también los causados desde la terminal u otras herramientas), con filtros por tipo, búsqueda, pausa y detalle de cada evento; abrir el recurso afectado.
  - **Guardados**: los eventos recibidos con la aplicación abierta, también de sesiones anteriores (últimos 20 000), con filtros de fecha, acción, tipo y texto.
  - **Consultar a Docker**: `docker events --since … --until …` con filtros validados antes de llamar a Docker y el comando equivalente; incluye lo ocurrido con la aplicación cerrada, dentro de los 256 eventos que Docker conserva (D-057).
  - **Historial de operaciones**: las operaciones de la aplicación, también de sesiones anteriores (últimas 10 000), con filtros por fecha, categoría, estado y texto (D-058).
- **Actualización en tiempo real** de toda la aplicación: un cambio en Docker (desde la aplicación o desde fuera) recarga al instante las listas y el panel afectados, sin pulsar Actualizar.
- Historial y diario guardados en `~/.local/state/docker-control-center/` (carpeta 0700, archivos 0600), **sin secretos**: no se guardan cuerpos de peticiones, comandos equivalentes ni salidas, y se ocultan los valores de `buildargs`. Opción **Guardar historial** en Configuración.
- La suscripción a los eventos aparece en el panel de actividad al abrirse y, si se interrumpe, como operación fallida; se reintenta sola mientras haya conexión.
- `verify-phase-8.sh` con pruebas reales de eventos y comprobaciones manuales M1–M15.

## 0.7.1 — 2026-11-07 (corrección de la Fase 7)
- Corregido: con Docker 26, justo después de detener (o iniciar) un proyecto Compose, su estado podía aparecer como "Parcial" porque Docker actualiza el listado de contenedores un instante después de responder. Tras desplegar, iniciar, detener o reiniciar, la aplicación confirma ahora el estado de cada contenedor con inspect (D-056), como ya hacía con las acciones de contenedores (D-031).
- Nuevas pruebas: carrera determinista con un Docker simulado, cálculo del estado en cada caso y comparación con `docker compose ps -a` e inspect tras cada operación individual y global, con perfiles y sin archivo.
- `verify-phase-7.sh` ejecuta la prueba determinista de la carrera como paso propio.

## 0.7.0 — 2026-11-07 (Fase 7: Docker Compose)
- Apartado **Docker Compose**: proyectos desplegados (detectados por las etiquetas de sus contenedores) y proyectos añadidos a la lista, con estado, carpeta y servicios; filtros y búsqueda (D-053).
- Abrir un proyecto existente (uno o varios archivos) y crear uno nuevo en una carpeta; el nombre lo decide Compose o el usuario.
- Detalle con los servicios del archivo junto a sus contenedores reales (imagen o construcción, puertos, perfiles, huérfanos) y acciones por servicio.
- Desplegar (servicios, perfiles, construir, política de descarga, recrear, huérfanos), iniciar, detener, reiniciar, descargar imágenes, construir y eliminar (con volúmenes e imágenes opcionales y confirmación), siempre con el comando exacto y la salida en vivo, que se puede interrumpir.
- Registros en vivo por servicio, editor del archivo con validación de Compose antes de guardar y protección frente a cambios externos (D-055), y configuración resuelta oculta hasta pedirla porque puede contener secretos (que no se guardan en el panel de actividad).
- Parar, iniciar, reiniciar y eliminar actúan sobre todos los contenedores del proyecto, también los de perfiles no activos, que Compose omitía en silencio (D-054).
- Proyectos cuyos archivos ya no existen: se pueden parar, iniciar, ver registros y eliminar.
- `verify-phase-7.sh` con pruebas reales de Compose (probadas con Compose 2.26.1 y 5.6.1) y comprobaciones manuales M1–M15.

## 0.6.0 — 2026-11-06 (Fase 6: volúmenes y directorios vinculados)
- Apartado **Volúmenes**: lista con contenedores que montan cada volumen, distintivo de anónimos, tamaños bajo demanda, filtros y búsqueda.
- Asistente de creación (local, tmpfs, NFS, carpeta u otro driver) con validación, comando y cuerpo exactos; aviso de nombre ya existente, que Docker ignoraría en silencio (D-051).
- Detalle con directorio, opciones, metadatos, tamaño, contenedores (ruta y acceso) e inspección JSON; crear un contenedor con el volumen.
- Eliminación (Docker la rechaza si lo monta cualquier contenedor) y limpieza de anónimos o de todos los sin uso, con estimación actualizada al abrir.
- Apartado **Directorios vinculados**: carpetas montadas en contenedores con su estado real en el equipo, permisos, contenedores y riesgos (socket de Docker, directorios del sistema, rutas inexistentes); vincular una carpeta con el selector del sistema abre el asistente de contenedores (D-052).
- La lista de contenedores incluye sus montajes (`mountPoints`).
- `verify-phase-6.sh` con pruebas reales de volúmenes y comprobaciones manuales M1–M15.

## 0.5.0 — 2026-11-06 (Fase 5: redes)
- Apartado **Redes**: lista con driver, subredes y contenedores (en marcha o no), filtros y búsqueda.
- Asistente de creación con subred, puerta de enlace, rango, IPv6, red interna, conectable, opciones del driver y metadatos; validación con las reglas de Docker antes de enviar, comando y cuerpo exactos.
- Conexión de contenedores con alias e IP fija, y desconexión con confirmación.
- Eliminación y limpieza con avisos de contenedores en marcha (Docker lo rechaza) y detenidos (Docker la elimina y no podrán arrancar) (D-047).
- Detalle con IPAM, opciones, contenedores con IP y MAC, e inspección JSON.
- Creación con `CheckDuplicate` para no crear redes con nombre repetido en Docker < 25 (D-048).
- Los diálogos consultan el estado actual a Docker al abrirse (D-049).
- `verify-phase-5.sh` con pruebas reales de redes y comprobaciones manuales M1–M16.
- Imágenes (corrección): no se permite limpiar imágenes mientras hay descargas en curso, porque con el almacén de containerd Docker las hace fallar (D-050). La prueba de limpieza ya no se solapa con las de descarga.

## 0.4.1 — 2026-11-06 (corrección de la Fase 4)
- Corregidas las pruebas de descarga que fallaban con el almacén de imágenes clásico (overlay2, Debian 13): el registro de prueba servía una imagen que el daemon ya tenía, así que Docker no descargaba ninguna capa. Ahora cada prueba descarga una imagen nueva, con aserciones más estrictas: progreso parcial real, una sola petición de la capa, conexión cortada al cancelar, nueva descarga completa tras cancelar, un registro por descarga (D-045).
- Nueva prueba del caso "contenido ya presente" y nueva prueba IPC contra Docker real de descarga completa y cancelada.
- El resumen de una descarga sin capas dice que Docker ya tenía el contenido, en lugar de "0 capas descargadas".
- Las imágenes sin fecha de creación muestran "—" en lugar de "hace 2027 años" (D-046).
- Las pruebas ya no dejan una imagen sin etiqueta al crear la imagen de prueba en paralelo.
- `verify-phase-4.sh`: infraestructura, pruebas deterministas y pruebas contra Docker real por separado, con el resultado de cada prueba real y el almacén de imágenes del daemon.

## 0.4.0 — 2026-11-06 (Fase 4)
- Descarga de imágenes con progreso por capa, cancelación y credenciales de Docker, anónimas o solo para esa descarga (D-042).
- Búsqueda en Docker Hub, detalle de imagen con capas, contenedores e inspección, etiquetado, eliminación con aviso de uso y limpieza (D-044).
- Asistente de construcción con BuildKit o el constructor clásico, `.dockerignore`, resumen del contexto, pasos y salida en directo (D-041).
- Pruebas reales de descarga con un registro local (D-043).
- `scripts/verify-phase-4.sh`.

## 0.3.0 — 2026-11-05 (Fase 3)
- Asistente de creación de contenedores por pestañas con validación en vivo, comando `docker run` equivalente y cuerpo de la API (D-034, D-035).
- Detalle de contenedor: resumen, logs en directo, terminal interactiva, estadísticas e inspección JSON (D-036, D-037).
- Duplicar contenedores con aviso de los ajustes que no se trasladan (D-038).
- Valores sensibles ocultos en pantalla y en el panel de actividad (D-039).
- La interfaz muestra el error si falla antes de arrancar; xterm.js 5.5 por compatibilidad con `freezePrototype` (D-040).
- `scripts/verify-phase-3.sh`.

## 0.2.1 — 2026-11-04 (corrección de la Fase 2)
- Corregido: tras detener (u otra acción), la tabla de contenedores podía mostrar el estado anterior porque Docker actualiza el listado un instante después de responder. Ahora cada acción confirma el estado con `GET /containers/{id}/json` (D-031).
- Corregido: la detección de Docker Compose usaba `--format json`, que falla con «unknown flag» cuando el plugin no está instalado. Ahora distingue Compose instalado (plugin o binario v2), no instalado, plugin inutilizable, Compose v1 no admitido y respuestas no reconocidas, e indica cómo instalarlo (D-032).
- `verify-phase-2.sh`: omisiones justificadas separadas de los fallos, veredicto explícito, código de salida fiable, limpieza garantizada de los recursos de prueba y comprobación de versiones de herramientas (D-033).
- `dcc-diagnose --compose`.

## 0.2.0 — 2026-11-04 (Fase 2)
- Conexión con Docker Engine, diagnóstico, Dashboard, contenedores, imágenes, sección Docker en Configuración.

## 0.1.0 — 2026-11-04 (Fase 1)
- Estructura, ventana principal, navegación, panel de actividad, configuración.
