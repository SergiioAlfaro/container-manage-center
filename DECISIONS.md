# Registro de decisiones técnicas

Cada decisión tiene un identificador estable. Si una decisión se revisa, se marca
como **sustituida** y se añade una nueva; no se borra.

| ID | Fecha | Estado |
|---|---|---|
| D-001 … D-017 | 2026-11-04 | Vigentes |

---

### D-001 · Tauri 2 + Rust + React/TypeScript
**Aprobada por el propietario del proyecto.** Tauri usa el WebKitGTK del sistema
(sin Chromium embebido), lo que reduce memoria y tamaño de paquete. Toda la
lógica de Docker vive en Rust; la interfaz nunca accede al socket.
Descartados: Electron (consumo), Python/Qt o GTK (concurrencia de streams y
empaquetado menos fiables).

### D-002 · Tauri 2 estable, no Tauri 3
En noviembre de 2026 Tauri 3 sigue en alfa (`3.0.0-alpha.5`). Se fija `tauri = "2"`
y `@tauri-apps/api ^2`. Revisar cuando Tauri 3 sea estable.

### D-003 · Plataformas mínimas: Debian 12 y Ubuntu 22.04
**Aprobada.** Tauri 2 requiere `libwebkit2gtk-4.1`, disponible desde esas versiones.

### D-004 · Comunicación con Docker por la Engine API (socket Unix)
**Aprobada.** Se usará `bollard` desde la Fase 2. No se ejecutan comandos de shell
para operaciones que la API permite.

### D-005 · Docker Compose mediante el CLI oficial, sin shell
**Aprobada.** Compose no tiene API en Docker Engine. Se invoca `docker compose`
con `std::process::Command`, argumentos uno a uno y sin `sh -c`. En el panel de
actividad estas operaciones se marcan como **CLI** y muestran el comando real.

### D-006 · Docker rootless desde la Fase 2
**Aprobada.** Detección automática de `$DOCKER_HOST`, `$XDG_RUNTIME_DIR/docker.sock`
y `/var/run/docker.sock`, con configuración manual de la conexión.

### D-007 · Español por defecto, inglés como alternativa
**Aprobada.** `i18next` con `locales/es.json` y `locales/en.json`. Una prueba
automatizada exige que ambos archivos tengan exactamente las mismas claves.

### D-008 · Registro de actividad como crate independiente (`dcc-activity`)
En la propuesta el registro formaba parte de `dcc-engine`. Se separa porque lo
usarán tanto el cliente de la API como el envoltorio del CLI de Compose, y así
ninguna de las dos capas depende de la otra.
Invariantes:
- Un `OperationRecord` solo se crea con `ActivityLog::begin` al iniciar una
  operación real. La interfaz no puede crear registros.
- `transport` describe la operación **ejecutada** (petición API o proceso CLI).
  `equivalent_command` es solo una traducción orientativa al CLI.
- Si el `OperationHandle` se descarta sin cerrarse (p. ej. tarea cancelada), la
  operación queda como `cancelled`; nunca se queda "en curso" para siempre.
- Las salidas capturadas se recortan a 64 KiB indicando el tamaño original.

### D-009 · Tipos compartidos generados con ts-rs
Los tipos del IPC se definen una sola vez en Rust y `cargo test` genera
`ui/src/shared/ipc/bindings/*.ts`. Evita que frontend y backend diverjan.
Los archivos generados se versionan para que el frontend compile sin Rust.

### D-010 · Configuración en TOML con escritura atómica
`$XDG_CONFIG_HOME/docker-control-center/config.toml`, permisos `0600`, escritura
en archivo temporal + `rename`. Un archivo corrupto se aparta a
`config.toml.corrupt` y se arranca con valores por defecto, avisando al usuario.
Todos los campos tienen valor por defecto para que añadir opciones no rompa
archivos antiguos. Valores fuera de rango se corrigen en el backend.

### D-011 · Credenciales nunca en el archivo de configuración
Las credenciales de registros irán al llavero del sistema (Secret Service) o a
los *credential helpers* de Docker (Fase 4/9). `config.toml` no las admite.

### D-012 · La aplicación se niega a ejecutarse como root
`geteuid() == 0` termina con un mensaje. `DCC_ALLOW_ROOT=1` lo permite solo para
entornos de prueba aislados (se usa en el entorno de desarrollo de Claude, que
corre como root dentro de un contenedor). No está documentado para usuarios finales.

### D-013 · Apartados no implementados: estado informativo sin acciones
Requisito "sin botones decorativos ni datos ficticios". Los apartados de fases
futuras muestran qué harán y en qué fase llegan, sin botones ni datos. Una prueba
automatizada comprueba que el área central de esos apartados no contiene botones.

### D-014 · Estado de interfaz con Zustand; sin router
Nueve apartados fijos no justifican un router. La sección activa es estado en
memoria. TanStack Query se incorporará en la Fase 2, cuando haya datos remotos
que cachear e invalidar.

### D-015 · Identidad visual "casco y baliza"
Fondo gris niebla, barra lateral azul casco, acento verde azulado (baliza) y
colores de señal para el estado de las operaciones. IBM Plex Sans para la
interfaz y JetBrains Mono solo para comandos y datos técnicos. Fuentes
empaquetadas localmente (`@fontsource`), sin dependencias de red. El panel de
actividad se diseña como un cuaderno de bitácora: franja de estado por
operación, distintivo API/CLI y comando como línea principal.

### D-016 · Licencia provisional GPL-3.0-or-later
Declarada en `Cargo.toml` y `package.json` como provisional. **Pendiente de
confirmación por el propietario** antes de la Fase 10. "Docker" es una marca
registrada de Docker, Inc.; antes de distribuir públicamente conviene revisar
el nombre del producto.

### D-017 · Nombre interno del paquete sin espacios
`productName` en `tauri.conf.json` es `docker-control-center`, de modo que el
paquete se llama `docker-control-center_<versión>_amd64.deb` y el lanzador
`docker-control-center.desktop` (convención freedesktop). El nombre visible
"Docker Control Center" se mantiene en el título de la ventana y en el menú
mediante la plantilla `packaging/docker-control-center.desktop`, que añade
traducción al español y palabras clave de búsqueda. Las dependencias de
ejecución (`libwebkit2gtk-4.1-0`, `libgtk-3-0`) las añade Tauri.
