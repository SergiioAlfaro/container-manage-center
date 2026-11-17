# Auditoría de la versión de Windows · Container Manage Center 1.9.1

Fecha: 2026-11-16. Decisión: D-115. Estado: **auditada, corregida y probada
hasta donde es posible sin Windows; pendiente de la primera validación en un
Windows real** ([VALIDACION-WINDOWS.md](VALIDACION-WINDOWS.md)).

## 1. Entorno de la auditoría

| | |
|---|---|
| Equipo | Contenedor Linux de desarrollo (Ubuntu 24.04, x86_64, 2 CPU) |
| Windows real | **No disponible.** Nada se ha ejecutado en Windows |
| Compilación para Windows | Cruzada, `x86_64-pc-windows-gnu`, MinGW-w64 (GCC 13), Rust 1.97.0 con `-Zbuild-std` (el destino de rustup no se puede descargar en este entorno) |
| Ejecución de pruebas de Windows | **Wine 9.0** (64 bits), con las pruebas unitarias compiladas para Windows. Wine implementa la API de Windows, pero no es Windows: sirve para la lógica (rutas, procesos, tuberías, credenciales), no para Docker Desktop, WebView2, notificaciones ni el instalador |
| Instalador | NSIS 3.09 generado por Tauri 2 (`makensis` de Ubuntu) |
| Docker de referencia (Linux) | 29.9.2, 26.1.3 y 24.0.7 |

## 2. Método

1. Inventario de todo el código que se comporta distinto en Windows
   (`cfg(windows)`), de las rutas, de los procesos externos y de los textos de
   la interfaz con órdenes de Linux.
2. Lectura del código de cada área (§3) buscando lo que en Windows no
   funcionaría aunque compile.
3. Ejecución de **todas** las pruebas unitarias compiladas para Windows con
   Wine **antes** de corregir: 26 fallos y la batería de la aplicación sin
   arrancar. Cada fallo se clasificó: prueba que solo tiene sentido en Linux
   (se marca `cfg(unix)` y, si procede, se añade su equivalente de Windows) o
   **fallo real de Windows** (se corrige).
4. Correcciones solo de problemas demostrados, con pruebas, y regresión
   completa de Linux después.

## 3. Resultados por área

| Área | Implementación en Windows | Resultado de la auditoría |
|---|---|---|
| Tauri, React, TypeScript, Vite | Mismo código; WebView2 en lugar de WebKitGTK; origen `http://tauri.localhost` | Sin diferencias funcionales. La interfaz decide por `isWindows()` los textos propios de Windows |
| Dependencias nativas | `zbus` y `secret-service` solo en Unix; `windows-sys`/`windows` en Windows | El programa solo depende de bibliotecas de Windows y de `WebView2Loader.dll` (comprobado con `objdump`) |
| Plugins de Tauri | `dialog`, `clipboard-manager`, `opener` (solo el perfil del autor) | Iguales en las dos plataformas |
| Sistema de archivos y rutas | `%APPDATA%` y `%LOCALAPPDATA%`, `PathBuf` | **W-01 a W-04, W-12**: las validaciones exigían rutas que empiezan por `/`; corregido con `cmc_engine::hostpath` |
| Permisos | Sin modos de Unix (`fsx`): las carpetas del perfil ya son privadas por su ACL | Correcto. La ACL de las claves SSH la comprueba OpenSSH |
| Procesos externos | Sin shell, `CREATE_NO_WINDOW`, `PATHEXT` y carpeta de Docker Desktop | **W-07**: detener no hacía nada; ahora con *job object*. **W-05**: `xdg-open` |
| Terminal | Contenedores: API de Docker (`exec` con TTY y cambio de tamaño). Servidores: `ssh -tt` por tuberías | Correcto; la del servidor no cambia de tamaño (limitación conocida). No hay terminal del propio equipo (ni en Linux) |
| Docker | Tuberías `\\.\pipe\docker_engine` y `\\.\pipe\dockerDesktopLinuxEngine`; `DOCKER_HOST` y contextos con `npipe://` | **W-06** (tubería ocupada) y **W-08** (ruta escrita tal cual). `tcp://` no se admite (se explica). Contenedores de Windows no se admiten |
| Docker Compose | `docker compose` (plugin de Docker Desktop) o `docker-compose` | **W-01** y **W-07** |
| SSH | `ssh.exe` de OpenSSH para Windows, sin ControlMaster; askpass por `127.0.0.1` con testigo de 256 bits | Correcto. **W-04** (clave con el selector). ProxyJump no disponible. Verificación de la clave del servidor sin cambios |
| Credenciales | Administrador de credenciales (`CredWriteW`…), por usuario | Correcto (prueba con Wine). Quedan al desinstalar (como el llavero en Linux) |
| Registros | Diario de actividad y de eventos en `%LOCALAPPDATA%` | **W-16**: no se podían vaciar ni rotar |
| Notificaciones | Notificaciones de Windows (WinRT) con el identificador de la aplicación | Sin probar (ni Wine ni este entorno las tienen). El instalador asigna el identificador al acceso directo |
| Instalación y desinstalación | NSIS para el usuario actual, sin administrador | **W-10**: las copias programadas quedaban huérfanas. Sin firmar |
| Actualizaciones | Solo consulta y avisa | **W-09**: instrucciones de `apt` |
| Configuración persistente | `%APPDATA%\container-manage-center\config.toml` | Correcto |
| Seguridad de montajes | Avisos de riesgo | **W-11**: no reconocía carpetas de Windows; **W-15**: `//var/run` no se normalizaba |
| Sistema | Configuración › Sistema | **W-17**: no mostraba Windows ni su versión |

## 4. Incidencias encontradas y corregidas

| ID | Gravedad | Problema | Cómo se encontró | Corrección | Prueba |
|---|---|---|---|---|---|
| W-01 | Alta | Compose con archivos del equipo: abrir, crear, levantar | Lectura de código | `hostpath::is_local_absolute`; reglas de rutas de Windows (`check_windows_compose_path`) | `compose_files_via_ipc_*` (Wine), `create_refuses_*` |
| W-02 | Alta | Carpetas vinculadas `C:\…` rechazadas o tomadas por volúmenes | Lectura de código | `bind_source_ok`, `split_bind` con letra de unidad | `mounts_are_validated`, `splits_binds_with_drive_letters` |
| W-03 | Alta | Construir una imagen desde una carpeta | Lectura de código + Wine | `is_local_absolute` en `BuildSpec::validate` | `build::tests::validation` (Wine) |
| W-04 | Alta | Clave SSH elegida con el selector | Lectura de código | `valid_local_path` acepta rutas del equipo | `ssh::profile` (Wine) |
| W-05 | Alta | «Abrir en el navegador» siempre fallaba | Lectura de código | `ShellExecuteW` solo con `http`/`https` validados | Compilación y Clippy (no se puede ejecutar sin escritorio) |
| W-06 | Media | Conexiones propias con la tubería ocupada | Lectura de código | Reintento de 2 s (`ERROR_PIPE_BUSY`) | Compilación y Clippy |
| W-07 | Alta | Detener Compose: no se detenía nada | Lectura de código + Wine | *Job object* con `KILL_ON_JOB_CLOSE` y `TerminateJobObject` | `windows_cancelling_stops_the_process_tree` (Wine) |
| W-08 | Baja | `\\.\pipe\…` escrito tal cual en la conexión personalizada | Lectura de código | `windows_pipe` | `windows_named_pipes`, `windows_candidates_*` (Wine) |
| W-09 | Media | Textos de `apt`, `systemctl`, `getent`, `chmod` en Windows | Lectura de código | Textos e instrucciones de PowerShell (8 idiomas) | Vitest (`diagnosis`, `remote`) |
| W-10 | Media | Copias programadas huérfanas tras desinstalar | Lectura de código | `--remove-scheduled-tasks` desde el gancho del desinstalador (no al actualizar) | Ejecutable con Wine (sale con 0) |
| W-11 | Media | Avisos de montajes peligrosos sin carpetas de Windows | Lectura de código | `classify_windows` | `windows_mount_risks` |
| W-12 | Alta | Destino de las copias `C:\…` rechazado | Pruebas con Wine | `check_windows_destination` | IPC de copias (Wine) |
| W-13 | Alta | «Comprobar» una copia daba todos los archivos por «de más» | Pruebas con Wine | Rutas relativas con `/` | `altered_missing_and_extra_files_are_detected` (Wine) |
| W-14 | Media | Restaurar en una carpeta nueva `C:\…` | Lectura de código | `check_local_compose_path` | Unitarias de restauración (Wine) |
| W-15 | Baja | `//var/run/docker.sock` sin clasificar en Windows | Pruebas con Wine | Normalización léxica (idéntica en Linux) | `docker_socket_is_critical_even_read_only` (Wine) |
| W-16 | Alta | Vaciar o rotar el registro de actividad y el diario de eventos: «Acceso denegado» | Pruebas con Wine | Cerrar el archivo antes de sustituirlo | `journal::tests` (Wine) |
| W-17 | Baja | Sistema sin el nombre ni la versión de Windows | Pruebas con Wine | Registro de Windows (`CurrentVersion`) | `windows_names_from_the_registry` |
| W-18 | Media | La aplicación arranca como administrador (en Linux rechaza root) | Lectura de código | Comprobar `TokenElevation` al arrancar y negarse (aviso en un cuadro de mensaje; sin ventana, solo código 1); `CMC_ALLOW_ROOT` solo en desarrollo. Corregida el 2026-11-17 con la aprobación del titular | `root_tests`, `elevation_can_be_queried` (Wine) |

En Linux todas estas correcciones conservan el comportamiento anterior; lo
comprueba la regresión completa (§6).

## 5. Matriz de pruebas

Estados: PASS (superada), FAIL (fallida), BLOCKED (no se ha podido ejecutar),
NOT TESTED (no realizada), PARTIAL (validada solo en parte). Ninguna prueba se
ha marcado PASS sin ejecutarse.

### 5.1 Ejecutadas en este entorno

| ID | Funcionalidad | Prueba | Resultado esperado | Resultado obtenido | Plataforma | Estado | Observaciones |
|---|---|---|---|---|---|---|---|
| AUT-01 | Compilación | `tauri build` para `x86_64-pc-windows-gnu` | Instalador y programa | `…_1.9.1_x64-setup.exe` (5,1 MiB) | Linux → Windows (MinGW) | PASS | MSVC (GitHub Actions) sin ejecutar |
| AUT-02 | Calidad | `cargo clippy --all-targets -D warnings` para Windows | Sin avisos | 0 avisos | Linux → Windows | PASS | |
| AUT-03 | Lógica de Windows | `cargo test --lib --bins` compilado para Windows | Todas superadas | 614 superadas, 0 fallidas (con la prueba de W-18) | Wine 9.0 | PASS | Antes de corregir: 26 fallos y la batería de la aplicación sin arrancar |
| AUT-04 | Rutas | `hostpath`, Compose, carpetas vinculadas, copias, restauración | Rutas `C:\…` y `\\servidor\…` válidas; sin `..`; carpetas del sistema y `AppData` rechazadas | Correcto | Wine 9.0 | PASS | |
| AUT-05 | Docker | Candidatos y `DOCKER_HOST`/`npipe://`; `tcp://` no admitido | Tuberías de Docker Desktop en orden | Correcto | Wine 9.0 | PASS | |
| AUT-06 | Docker | Sonda de una tubería inexistente | «Falta Docker» | Correcto | Wine 9.0 | PASS | La conexión real con Docker Desktop: WIN-DOC-01 |
| AUT-07 | Procesos | Detener una orden con hijos (`cmd` → `ping`) | Se detiene en segundos | Correcto | Wine 9.0 | PASS | Con `docker-compose.exe` real: WIN-DOC-11 |
| AUT-08 | Procesos | Códigos de salida, salida estándar y de errores, tiempo máximo | Registrados | Correcto | Wine 9.0 | PASS | |
| AUT-09 | Credenciales | Guardar, leer, comprobar y borrar en el Administrador de credenciales | Ida y vuelta | Correcto | Wine 9.0 | PARTIAL | La implementación de Wine, no la de Windows: WIN-SSH-03 |
| AUT-10 | Tareas programadas | XML de la tarea (marca, `StartWhenAvailable`, escapado) | Correcto | Correcto | Wine 9.0 | PARTIAL | Sin el Programador de tareas real: WIN-BAK-04 a 06 |
| AUT-11 | Registros | Vaciar y rotar los diarios | Sin error | Correcto (antes: acceso denegado) | Wine 9.0 | PASS | |
| AUT-12 | Copias | Comprobar la integridad de una copia | Solo lo alterado o de más | Correcto (antes: todo «de más») | Wine 9.0 | PASS | |
| AUT-13 | IPC | Batería de órdenes de la interfaz con Tauri simulado | 161 superadas | 161 superadas | Wine 9.0 | PASS | Sin Docker ni ventana |
| AUT-14 | Seguridad | Riesgo de montar carpetas de Windows | Crítico/alto/medio según la carpeta | Correcto | Wine 9.0 | PASS | |
| AUT-15 | Interfaz | Textos de Windows (diagnóstico, SSH, actualizaciones, sistema) | Sin órdenes de Linux; las del servidor iguales | 444 pruebas de Vitest superadas | Linux (jsdom) | PASS | |
| AUT-16 | Programa | Modos sin ventana: `--remove-scheduled-tasks` y `--run-backup` | Salen sin ventana con 0 y 1 | 0 y 1 («no existe la tarea») | Wine 9.0 | PASS | |
| AUT-17 | Programa | Bibliotecas de las que depende el `.exe` | Solo de Windows y `WebView2Loader.dll` | Correcto | `objdump` | PASS | |
| AUT-18 | Instalador | Contenido del script NSIS generado | Usuario actual, 5 idiomas, licencia, `legal\`, gancho de desinstalación, identificador del acceso directo | Correcto | Inspección | PARTIAL | |
| AUT-19 | Instalador | Instalación silenciosa con Wine | Instala | No arranca: el instalador NSIS es de 32 bits y solo hay Wine de 64 bits | Wine 9.0 | BLOCKED | WIN-INS-01 a 03 |
| AUT-20 | Ventana | Abrir la aplicación | Ventana | No probado: Wine no tiene WebView2 Runtime | — | BLOCKED | WIN-INS-04 |
| AUT-21 | Linux | Regresión completa (Docker 29, 26, 24) | Sin fallos | 172 / 130 / 130 correctas, 0 fallos | Ubuntu 24.04 | PASS | Detalle en §6 |
| AUT-22 | Privilegios (W-18) | Regla de arranque (`root_tests`) y consulta de `TokenElevation` (`elevation_can_be_queried`) | Con privilegios no arranca salvo en desarrollo con `CMC_ALLOW_ROOT`; la consulta funciona | Correcto | Wine 9.0 y Linux | PARTIAL | El cuadro de mensaje y el rechazo con «Ejecutar como administrador» real: WIN-INS-06 |

### 5.2 Pendientes en Windows real

Todas las de [VALIDACION-WINDOWS.md](VALIDACION-WINDOWS.md) están **NOT TESTED**:

| ID | Funcionalidad | Estado |
|---|---|---|
| WIN-INS-01 a 06 | Instalación, ventana, sistema, rechazo como administrador | NOT TESTED |
| WIN-DOC-01 a 17 | Docker Desktop: contenedores, navegador, terminal, carpetas vinculadas, riesgos, edición, imágenes, redes, volúmenes, construcción, Compose, detener, reglas de rutas, diagnóstico, reconexión, tubería personalizada, componentes | NOT TESTED |
| WIN-SSH-01 a 08 | SSH: clave, contraseña y Administrador de credenciales, terminal, diagnóstico, ProxyJump, cambio de clave del servidor | NOT TESTED |
| WIN-BAK-01 a 06, WIN-MON-01, WIN-ACT-01 | Copias, restauración, Programador de tareas, notificaciones, registros | NOT TESTED |
| WIN-LEG-01 a 02, WIN-UPD-01 a 02, WIN-UNI-01 a 03 | Ayuda, legal, actualización sobre una instalación anterior, desinstalación | NOT TESTED |

## 6. Regresión de Linux

`verify-evolution-1.9.1.sh` ejecutado el 2026-11-16, después de todas las
correcciones de Windows, en Ubuntu 24.04 (entorno de pruebas, como root con
`CMC_VERIFY_ALLOW_ROOT=1`):

| Docker | Opciones | Correctas | Fallos | Omitidas | Avisos | Compose 2.26.1 (`live_compose`) |
|---|---|---|---|---|---|---|
| 29.9.2 (API 1.56) | `--deb --net --ssh-lab` | 172 | 0 | 5 | 1 | 6/6 |
| 26.1.3 (API 1.45) | — | 130 | 0 | 6 | 1 | 6/6 |
| 24.0.7 (API 1.43) | — | 130 | 0 | 6 | 1 | 6/6 |

- **Aviso** (los tres): ejecución como root en el entorno de pruebas.
- **Omitidas** (justificadas por el propio script): la comprobación de Windows
  dentro del script (en esta ejecución no se le indicó la fuente de Rust; las
  pruebas de Windows se ejecutaron aparte con Wine, AUT-03); la desinstalación
  real con APT (necesita un chroot); dos pruebas de permisos que root no puede
  hacer; la IPC real por SSH sin laboratorio y, en Docker 26 y 24, las pruebas
  por SSH (el laboratorio solo se monta con Docker 29).
- Las versiones de Linux no cambian de comportamiento: los cambios de la 1.9.1
  que tocan código común (normalización léxica de rutas en `risk.rs`, rutas
  relativas con `/` en `store.rs`, cierre del diario antes de sustituirlo en
  `journal.rs`, `split_bind`) quedan cubiertos por estas ejecuciones.
- Paquete `container-manage-center_1.9.1_amd64.deb` generado y comprobado en la
  ejecución con Docker 29.

## 7. Compatibilidad

| | Estado |
|---|---|
| Versiones de Windows **probadas** | Ninguna |
| Compatibles **según el análisis** | Windows 10 y 11 de 64 bits (x86_64). Para Docker en el propio equipo, lo que pide Docker Desktop: Windows 10 22H2 (19045) o Windows 11 23H2 (22631) o posterior, Pro, Enterprise o Education, con WSL 2 (2.1.5 o posterior) o Hyper-V. Sin Docker Desktop, solo servidores remotos por SSH (OpenSSH de Windows 10 1809 o posterior) |
| No admitido | Windows de 32 bits o ARM64 (no se compila), contenedores de Windows, Docker Engine dentro de WSL sin Docker Desktop, `DOCKER_HOST=tcp://`, ProxyJump |
| WebView2 Runtime | Incluido en Windows 11; en Windows 10, el instalador lo descarga si falta |
| PowerShell | No lo necesita la aplicación. Las instrucciones de diagnóstico son órdenes de PowerShell 5.1 (incluido en Windows 10 y 11) |

## 8. Seguridad en Windows

- **Credenciales**: Administrador de credenciales del usuario (tipo genérico,
  persistencia local); nunca en texto plano en archivos. Los secretos se borran
  de la memoria al terminar (`zeroize`).
- **SSH**: se conserva la verificación estricta de la clave del servidor
  (`StrictHostKeyChecking=yes` con el `known_hosts` de la aplicación y el del
  usuario). El askpass escucha solo en `127.0.0.1`, comprueba el origen y un
  testigo aleatorio de 256 bits, y solo responde a la pregunta esperada.
  Riesgo bajo conocido: otro proceso del mismo equipo podría ocupar una
  conexión del askpass mientras dura la autenticación (no obtiene el secreto).
- **Procesos**: sin shell; los argumentos se pasan uno a uno. Las órdenes de
  diagnóstico de la interfaz son texto para copiar: la aplicación no las
  ejecuta.
- **Navegador**: `ShellExecuteW` solo con direcciones `http`/`https` ya
  validadas.
- **Rutas**: las reglas de Compose y de copias tienen su equivalente en
  Windows (carpetas ocultas, `AppData`, carpetas del sistema).
- **Montajes**: avisos de riesgo también para carpetas de Windows.
- **Instalador**: para el usuario actual, sin administrador; **sin firmar**
  (SmartScreen avisa y no hay forma de comprobar su origen salvo por las
  sumas SHA-256). Desinstalar no borra datos, copias ni credenciales guardadas.
- **Privilegios**: la aplicación no arranca como administrador (W-18), igual
  que en Linux no arranca como root.
- **Legal**: el cargador de WebView2 de Microsoft se distribuye en binario,
  cubierto por el permiso adicional de la GPL v3 que el titular aprobó el
  2026-11-17 (`docs/legal/WEBVIEW2_EXCEPTION.md`).

## 9. Limitaciones conocidas

1. Sin validar en Windows real (§5.2).
2. Sin ProxyJump; la terminal del servidor no cambia de tamaño.
3. Instalador sin firmar.
4. Al desinstalar quedan los datos, las copias y las credenciales guardadas
   (a propósito, como en Linux).
5. La aplicación no revisa la ACL de los archivos de clave SSH: lo hace el
   propio OpenSSH para Windows, que rechaza las claves demasiado abiertas (la
   aplicación lo explica con la orden `icacls` para corregirlo).
6. Las pruebas de integración con Docker real solo existen para Linux.
7. W-18 (corregida el 2026-11-17): la aplicación se niega a arrancar como
   administrador. Una cuenta de administrador con el control de cuentas de
   usuario (UAC) desactivado ejecuta todo elevado, así que en ese equipo la
   aplicación no arrancará: es lo que se decidió, igual que en Linux con root.
   Comprobarlo en Windows real: WIN-INS-06.
