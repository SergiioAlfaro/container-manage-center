# Estructura del proyecto

## Árbol de directorios

El repositorio es un *workspace* de Cargo (cuatro crates) y un proyecto npm
(la interfaz). Este es su árbol real, resumido a las carpetas que importan:

![Carpetas principales del repositorio y su función.](diagramas/08-directorios.pdf){width=70%}

```text
container-manage-center/
├── .github/                 Flujos de GitHub Actions y plantillas
│   ├── workflows/           ci.yml · linux-build.yml · windows-build.yml
│   ├── ISSUE_TEMPLATE/      Error, propuesta y enlace a seguridad
│   └── PULL_REQUEST_TEMPLATE.md
├── .cargo/config.toml       Opciones de Cargo del proyecto
├── Cargo.toml · Cargo.lock  Workspace de Rust (versión 1.11.1)
├── package.json · package-lock.json · vite.config.ts · tsconfig.json · eslint.config.js
├── deny.toml                Política de cargo-deny (licencias y avisos)
├── crates/
│   ├── cmc-activity/        Registro de actividad, ocultación de secretos, historial
│   ├── cmc-store/           Configuración, perfiles de servidores, diarios, migración
│   └── cmc-engine/          Motor (ver abajo) · src/bin/ · tests/
├── src-tauri/               Aplicación Tauri
│   ├── src/                 lib.rs, main.rs, state.rs, commands/, servidores, flujos…
│   ├── capabilities/        Permisos mínimos de la ventana
│   ├── icons/               Iconos (generados desde packaging/logo.svg)
│   ├── tauri.conf.json      Configuración común y paquete .deb
│   └── tauri.windows.conf.json   Instalador NSIS de Windows
├── ui/                      Interfaz
│   ├── index.html · public/
│   └── src/                 app/, features/ (14 apartados y 4 auxiliares), shared/, styles/, test/
├── scripts/                 Instalación, compilación, verificación, licencias
├── packaging/               Logotipo, lanzador .desktop, copyright DEP-5, Windows
└── docs/                    Legal, seguridad, validación, hoja de ruta, manual
```

## Código Rust

### `crates/cmc-engine` (el motor)

Es la parte más grande del proyecto (unas 87 fuentes). No depende de Tauri:
se puede usar y probar sin interfaz.

| Módulo | Función |
|---|---|
| `endpoint.rs`, `probe.rs`, `manager.rs` | Localizar Docker, sondear los candidatos, conectar, diagnosticar, latido y reconexión |
| `client.rs` | `Engine`: cliente bollard, *dialer* propio y registro de cada petición |
| `containers.rs`, `details.rs`, `spec.rs`, `spec/` | Lista, ciclo de vida, inspección y especificación validada de contenedores |
| `edit.rs`, `edit_ops.rs` | Edición de la configuración con recreación segura |
| `images.rs`, `pull.rs`, `reference.rs`, `registry_auth.rs`, `build/` | Imágenes, descargas, referencias, credenciales de registro y construcción |
| `networks.rs`, `volumes.rs`, `binds.rs`, `resources.rs` | Redes, volúmenes, carpetas vinculadas, validación de identificadores |
| `compose/` | Detección de Compose, proyectos, acciones y archivos (locales y remotos) |
| `events.rs`, `streams.rs` | Eventos de Docker; registros, estadísticas y terminales en flujo |
| `ssh/` | Perfiles, argumentos de `ssh`, sesión, askpass, identidad del servidor, terminal del servidor |
| `audit/` | Auditoría de seguridad y análisis de vulnerabilidades (Trivy, Grype) |
| `backup/` | Copias de volúmenes, bases de datos, configuración y Compose; restauración |
| `monitor/` | Muestras, historial por minuto y alertas |
| `risk.rs` | Riesgos para el equipo de montajes, capacidades, red, dispositivos… |
| `hostpath.rs` | Rutas del equipo (con letra de unidad en Windows) frente a rutas de servidor |
| `cli.rs`, `sys.rs`, `fsx.rs` | Procesos externos, datos del sistema, permisos de archivos por plataforma |
| `legacy.rs` | Nombres del producto anterior, solo para migrar datos |
| `bin/cmc-diagnose.rs` | Diagnóstico de la conexión desde la terminal |
| `bin/cmc-askpass.rs` | Programa auxiliar de askpass para pruebas |

### `crates/cmc-store` y `crates/cmc-activity`

- **cmc-store**: `config.rs` (estructura de `config.toml` con valores por
  defecto y tolerancia a archivos corruptos), `servers.rs` (perfiles),
  `journal.rs` (diarios JSONL con límite), `migrate.rs` y `legacy.rs` (paso
  de los datos del nombre anterior), `fsx.rs` (permisos por plataforma).
- **cmc-activity**: `log.rs` (registro en memoria con difusión a la ventana),
  `model.rs` (operación: tipo, transporte API o CLI, petición, resultado),
  `redact.rs` (ocultación de secretos), `quote.rs` (orden equivalente con
  comillas correctas), `history.rs` (historial persistente sin cuerpos ni
  salidas).

### `src-tauri` (la aplicación)

| Archivo o carpeta | Función |
|---|---|
| `main.rs` | Punto de entrada: modos sin ventana (askpass, migración, desinstalación en Windows, copia programada) y arranque |
| `lib.rs` | Construcción de la aplicación Tauri, registro de las 130 órdenes, negativa a ejecutarse como root o como administrador |
| `state.rs` | `AppState`: actividad, configuración, servidores, flujos, diarios, monitorización |
| `commands/` | Órdenes IPC por área: `app`, `config`, `engine`, `resources`, `containers`, `images`, `networks`, `volumes`, `compose`, `events`, `servers`, `audit`, `backup`, `monitor`, `links`, `components`, `activity` |
| `servers.rs`, `secrets.rs` | Servidores locales y remotos; llavero (Secret Service o Administrador de credenciales) |
| `streams.rs`, `events.rs` | Registro de flujos abiertos; puentes de eventos hacia la ventana |
| `journal.rs`, `monitor.rs`, `backups.rs`, `links.rs` | Diarios, monitorización, tareas de copia, enlaces entre herramientas |
| `systemd.rs`, `schtasks.rs` | Copias programadas: temporizadores de usuario de systemd (Linux) o Programador de tareas (Windows) |
| `notify.rs` | Notificaciones de escritorio (D-Bus o WinRT) |
| `updates.rs` | Comprobación de versiones nuevas (solo consulta) |
| `uninstall.rs` | Desinstalación selectiva con APT (Linux) |
| `migration.rs` | Migración de datos y tareas del nombre anterior |
| `ipc_tests.rs` | Pruebas de las órdenes con el runtime simulado de Tauri |

## Código de la interfaz

```text
ui/src/
├── main.tsx              Arranque: i18n, tema, React Query, App
├── app/                  App, barra lateral, apartados, servidor elegido, ErrorBoundary
├── features/             Un apartado por carpeta (ver el capítulo Frontend)
├── shared/
│   ├── ipc/              client.ts (todas las órdenes) y bindings/ (tipos generados)
│   ├── i18n/             index.ts y locales/{en,es,ca,gl,eu,fr,it,pt}.json
│   ├── ui/               Controles, diálogos, menú, gráficas, riesgos, terminal
│   ├── format.ts         Números y fechas con Intl
│   ├── platform.ts       Detección de Windows para los textos propios
│   └── query.ts          Configuración de TanStack Query
├── styles/app.css        Tokens de color (morado), tema claro y oscuro
└── test/                 Dobles de la API, datos de prueba, render y simulación de xterm
```

## Configuración de compilación

| Archivo | Función |
|---|---|
| `Cargo.toml` (raíz) | Workspace, versión común, dependencias compartidas |
| `src-tauri/Cargo.toml` | Dependencias de la aplicación por plataforma (`zbus` y `secret-service` en Unix; `windows-sys` y `windows` en Windows) |
| `src-tauri/build.rs` | `tauri-build`: incluye la configuración, los permisos y los iconos |
| `src-tauri/tauri.conf.json` | Identificador `org.containermanagecenter.app`, ventana, paquete `.deb` y su documentación |
| `src-tauri/tauri.windows.conf.json` | Sustituciones para Windows: nombre visible, NSIS, idiomas, recursos legales, gancho del desinstalador |
| `package.json` | Scripts (`build`, `typecheck`, `lint`, `test`, `bindings`) y dependencias de la interfaz |
| `vite.config.ts` · `tsconfig.json` · `eslint.config.js` | Compilación, comprobación de tipos estricta y reglas de lint |
| `deny.toml` | `cargo deny`: licencias permitidas y avisos de seguridad de dependencias |

## Scripts

| Script | Función |
|---|---|
| `scripts/install.sh` | Instalar, comprobar o desinstalar el paquete adecuado para el sistema |
| `scripts/install-build-deps.sh` | Dependencias de compilación en Debian/Ubuntu |
| `scripts/run.sh` | Ejecutar la versión instalada o compilar y ejecutar |
| `scripts/deb-set-depends.sh` | Dependencias reales del `.deb` con `dpkg-shlibdeps` |
| `scripts/build-deb-compat.sh` | Paquete compatible compilado en un contenedor de Ubuntu 22.04 |
| `scripts/build-windows.sh` | Instalador de Windows de prueba con MinGW-w64 |
| `scripts/third-party-licenses.py` | Avisos de terceros y auditoría de licencias (Linux y `--windows`) |
| `scripts/spdx-headers.py` | Encabezados SPDX del código propio |
| `scripts/release-sums.sh` | `SHA256SUMS` de una entrega |
| `scripts/verify-evolution-1.11.0.sh` | Verificación completa de la versión |
| `scripts/build-manual.sh` | Generación de este manual |
| `scripts/dev/` | Laboratorio SSH, imágenes de bases de datos de prueba, instalación limpia, prototipo |

## Pruebas

- **Unitarias de Rust**: módulos `#[cfg(test)]` junto al código.
- **Integración contra Docker real**: `crates/cmc-engine/tests/live_*.rs`
  (solo se ejecutan con `CMC_DOCKER_TESTS=1`).
- **Órdenes IPC**: `src-tauri/src/ipc_tests.rs` con el runtime simulado de Tauri.
- **Interfaz**: archivos `*.test.ts(x)` junto a cada componente, con Vitest y
  Testing Library.

## Recursos

- `packaging/logo.svg` (y `logo-1024.png`): logotipo, del que se generan los
  iconos de `src-tauri/icons/`.
- `packaging/container-manage-center.desktop`: lanzador de Linux.
- `packaging/copyright`: copyright en formato DEP-5 para el paquete.
- `packaging/windows/`: textos de licencia de WebView2Loader y MinGW, y el
  gancho del desinstalador NSIS.
- `ui/src/assets/logo.svg`: logotipo en la interfaz.

## Documentación

| Documento | Contenido |
|---|---|
| `README.md`, `INSTALL.md`, `USER_GUIDE.md` | Presentación, instalación y guía de uso |
| `ARCHITECTURE.md`, `DECISIONS.md`, `STATUS.md`, `CHANGELOG.md` | Arquitectura, decisiones (D-001 a D-116), estado y cambios |
| `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md` | Contribución, seguridad y conducta |
| `docs/legal/` | Licencia, copyright, avisos de terceros, permiso adicional de WebView2 |
| `docs/security/` | Auditoría de seguridad, plan de corrección y lista de comprobación |
| `docs/validation/` | Auditoría de Windows y guías de validación manual |
| `docs/manual/` | Fuente de este manual (Markdown, diagramas, plantilla) |

## Archivos de distribución

Los paquetes no se guardan en el repositorio (están en `.gitignore`): se
generan con los scripts y se publican como artefactos. Ver
[Distribución y versiones](#distribución-y-versiones).
