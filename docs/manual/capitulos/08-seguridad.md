# Seguridad

> **Aviso.** Container Manage Center **no está certificado ni auditado por
> terceros**. Este capítulo describe las medidas implementadas y sus límites.
> Administrar Docker da, por naturaleza, un control muy amplio sobre el equipo:
> quien puede usar el socket de Docker puede obtener privilegios de
> administrador.

## Modelo de amenazas

| Amenaza | Medida principal |
|---|---|
| Un contenedor obtiene control del equipo por un montaje o una opción peligrosa | Detección de riesgos antes de crear, editar o desplegar; aceptación uno a uno |
| Secretos (contraseñas, tokens) en pantalla, registros o archivos | Ocultación en el registro de actividad; llavero del sistema; `zeroize` |
| Suplantación de un servidor SSH | Verificación estricta de la huella; confirmación expresa |
| Una interfaz comprometida usa las órdenes IPC para leer o escribir archivos | Validación en Rust; reglas de rutas de Compose; permisos mínimos de Tauri |
| Ejecución de órdenes no previstas | Procesos sin shell; argumentos por separado; órdenes de diagnóstico solo para copiar |
| Operaciones destructivas por error | Confirmación en la interfaz **y** en el backend |
| Exposición de Docker en la red | Nunca TCP: solo socket, tubería o `dial-stdio` por SSH |
| Paquete alterado | `SHA256SUMS` verificado por `install.sh` |

## Gestión de credenciales y llavero

- Las contraseñas de SSH y las frases de las claves se piden al conectar. Con
  **Recordar**, se guardan en el **almacén seguro del sistema**: Secret Service
  (GNOME Keyring, KWallet…) en Linux, con los atributos
  `application=container-manage-center`, servidor y tipo; el **Administrador
  de credenciales** en Windows, con el destino
  `container-manage-center/<servidor>/<tipo>`.
- Si no hay almacén disponible, se avisa y se piden cada vez (caché de sesión
  en memoria).
- Las claves privadas **nunca se copian**: solo se guarda su ruta.
- Las credenciales de los registros de imágenes se leen de la configuración de
  Docker (`docker login` o su *credential helper*) solo para la descarga; las
  que se escriben a mano no se guardan.
- En un servidor remoto, las credenciales de registro del equipo solo se
  envían si se marca expresamente en cada descarga.

## Conexiones SSH y verificación de la clave del servidor

- `StrictHostKeyChecking=yes` con la lista de huellas **de la aplicación**
  (`known_hosts` propio) y la del usuario, que nunca se modifica.
- Primera conexión: huella `SHA256:…` mostrada y confirmada por el usuario.
- Huella cambiada: la conexión se detiene con aviso de posible suplantación;
  solo se acepta la nueva con **Revisar identidad** y confirmación.
- `ProxyJump`: el servidor intermedio se verifica con las mismas reglas y solo
  admite clave o agente; la contraseña del servidor nunca se envía al
  intermedio.
- El askpass solo responde a la pregunta esperada, una vez por autenticación,
  por un socket privado (Linux) o por `127.0.0.1` con un testigo aleatorio de
  256 bits (Windows).

## Protección frente a ejecución insegura

- Los procesos externos se lanzan **sin shell**, con la ruta del programa y
  los argumentos por separado. Los valores (`$HOME`, `*`, `;`) llegan
  literalmente.
- Las órdenes remotas se entrecomillan con reglas POSIX (`shell_quote`); por
  eso el usuario remoto necesita una shell POSIX.
- Comando y Entrypoint del asistente se dividen con reglas de comillas POSIX
  en la interfaz, **sin ejecutar ninguna shell**.
- Las órdenes que propone el diagnóstico son texto para copiar: la aplicación
  nunca las ejecuta.
- La negativa a ejecutarse como **root** en Linux, o **como administrador**
  (proceso elevado) en Windows, solo se puede desactivar en compilaciones de
  desarrollo.

## Gestión de secretos

- **Ocultación** (`cmc-activity/redact.rs`) antes de guardar o mostrar: valores
  de variables cuyo nombre contiene `PASSWORD`, `SECRET`, `TOKEN`, `KEY`,
  `CREDENTIAL`, `PRIVATE`… o termina en `PASS`, `PWD`, `AUTH`, `DSN`,
  `COOKIE`; contraseñas en URL (`postgres://app:********@db`); claves
  privadas PEM; argumentos de construcción.
- El **historial** persistente no guarda cuerpos, órdenes equivalentes ni
  salidas.
- La **configuración resuelta** de Compose (que puede tener secretos del
  `.env`) está oculta hasta que se pide y no se registra.
- Las copias de configuración de contenedores se hacen **sin** las variables
  secretas.
- Los secretos en memoria se borran al soltarlos (`zeroize`).
- Límite: Docker recibe los secretos completos y cualquiera con acceso a
  Docker puede leerlos (`docker inspect`); los registros de los contenedores y
  la salida de las terminales se muestran tal cual.

## Confirmaciones de operaciones destructivas

Eliminar contenedores, imágenes, redes y volúmenes, limpiar, `compose down`,
desconectar una red, restaurar una base de datos (hay que escribir
«restaurar») y quitar componentes del sistema piden confirmación en la
interfaz, y el backend exige `confirmed = true`. Los diálogos consultan el
estado actual a Docker al abrirse para no confirmar con datos antiguos.

## Validación de entradas

Todo lo que llega por IPC se valida en Rust: identificadores de recursos,
referencias de imagen (reglas de Docker), nombres de contenedores, redes,
volúmenes, servicios y perfiles (no pueden empezar por `-`), subredes y
direcciones, rutas, perfiles SSH (usuario, host, puerto, rutas del servidor
con caracteres seguros) y URL de actualizaciones (solo HTTPS, salvo la propia
máquina).

## Permisos

- **Tauri**: sin `core:default`; solo escuchar eventos, el diálogo de abrir,
  el portapapeles y un enlace concreto. Política de seguridad de contenido
  (CSP) estricta: solo recursos propios.
- **Archivos**: en Linux, carpetas 0700 y archivos 0600 (configuración,
  diarios, copias, exportaciones de auditoría). En Windows, la ACL del perfil
  del usuario.
- **Sistema**: la aplicación no cambia grupos, permisos ni configuración del
  sistema; la desinstalación selectiva (Linux) solo eleva `apt-get remove` con
  `pkexec`, tras simularla y confirmarla.

## Montajes sensibles y configuraciones peligrosas

`risk.rs` calcula los **riesgos para el equipo** de una especificación, de una
edición o de un proyecto Compose:

| Nivel | Ejemplos |
|---|---|
| Crítico | Socket de Docker, containerd o Podman montado (también en solo lectura); raíz `/` (o `C:\` en Windows); modo privilegiado; `/dev/mem` |
| Alto | Carpetas con credenciales (`/etc`, `/root`, `/home`, `~/.ssh`, `~/.docker`; `C:\Users`, `AppData`); capacidades `ALL`, `SYS_ADMIN`…; red, PID o IPC del equipo; seccomp/AppArmor/SELinux desactivados; discos |
| Medio | Carpetas del sistema en solo lectura; programas del sistema con escritura; `userns`, `cgroup`, `uts` del equipo |
| Bajo | Puerto publicado en todas las interfaces |

Los altos y críticos hay que aceptarlos uno a uno; la aceptación se comprueba
otra vez al ejecutar.

## Auditoría y avisos de seguridad

El apartado **Auditoría** revisa contenedores, imágenes y proyectos sin
cambiar nada (ver [Funcionalidades](#auditoría-de-seguridad)). Además, el
proyecto mantiene `docs/security/` con la auditoría de seguridad de la 1.0.0
(13 hallazgos confirmados), el plan de corrección y una lista de comprobación
que se revisa en cada versión.

## Estado de las protecciones

| Estado | Protección |
|---|---|
| **Implementada** | Validación en Rust; confirmación en el backend; riesgos con aceptación; ocultación de secretos; llavero; verificación estricta de la clave SSH; procesos sin shell; CSP y permisos mínimos de Tauri; negativa a root (Linux); reglas de rutas de Compose (Linux y Windows); `SHA256SUMS` verificado por `install.sh` |
| **Parcial** | Ocultación de secretos: depende del nombre de la variable o del formato del valor (un secreto en una variable con nombre neutro no se reconoce); detección de riesgos: conoce rutas y opciones concretas, no todas las posibles |
| **Limitación conocida** | Copias sin cifrar; paquetes e instalador sin firmar; `style-src 'unsafe-inline'` en la CSP (estilos en línea de React); dependencias transitivas sin mantenimiento señaladas por `cargo deny` (sin vulnerabilidad conocida); en Windows no se comprueba si se ejecuta como administrador |
| **Pendiente** | Firma GPG de `SHA256SUMS` (decisión del titular, CMC-SEC-09); validación en Windows real; aprobación del permiso adicional de WebView2 |
