# Desarrollo y compilación

## Herramientas necesarias

| Herramienta | Versión | Para qué |
|---|---|---|
| Rust (rustup) | 1.80 o posterior (probado con 1.97) | Crates y aplicación |
| Node.js y npm | 20 o posterior (probado con 22) | Interfaz, Vite, Vitest, CLI de Tauri |
| Paquetes de sistema (Debian/Ubuntu) | `build-essential`, `pkg-config`, `libssl-dev`, `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `librsvg2-dev`, `libayatana-appindicator3-dev` | Compilar Tauri en Linux |
| Docker Engine y Compose v2 | 20.10+ | Pruebas de integración y uso |
| OpenSSH | — | Servidores remotos y sus pruebas |
| Opcionales | `shellcheck`, `cargo-deny`, MinGW-w64 y NSIS, Wine, pandoc y XeLaTeX, Mermaid CLI | Lint de scripts, licencias, Windows, manual |

## Preparar el entorno

```bash
git clone https://github.com/SergiioAlfaro/container-manage-center.git
cd container-manage-center
bash scripts/install-build-deps.sh     # paquetes de sistema (pide sudo para apt)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # si falta Rust
npm ci                                 # dependencias exactas de package-lock.json
```

## Estructura de trabajo

- La interfaz se desarrolla en `ui/src/`; Vite sirve `ui/` en modo desarrollo.
- El motor en `crates/cmc-engine/` se prueba sin interfaz.
- Las órdenes nuevas se añaden en `src-tauri/src/commands/`, se registran en
  `lib.rs` y se exponen en `ui/src/shared/ipc/client.ts`.
- Tras cambiar un tipo compartido, `npm run bindings` regenera los tipos de
  TypeScript.

## Compilación

```bash
npx tauri dev                                   # desarrollo con recarga de la interfaz
npx tauri build --debug --no-bundle             # binario de depuración
./target/debug/container-manage-center
npx tauri build --no-bundle && bash scripts/run.sh   # compilación de producción
```

![Proceso de compilación y empaquetado.](diagramas/09-compilacion.pdf){width=85%}

## Pruebas

Comandos ejecutados y superados en la 1.9.1:

```bash
npm run typecheck        # TypeScript estricto
npm run lint             # ESLint (13 avisos de react-hooks, sin errores)
npx vitest run           # 444 pruebas de la interfaz
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace   # unitarias e IPC (las de Docker real se omiten)
CMC_DOCKER_TESTS=1 cargo test -p cmc-engine --test live_docker   # contra Docker real
```

La verificación completa (regresión, paquetes, licencias, migración, Windows):

```bash
bash scripts/verify-evolution-1.9.1.sh --deb
```

## Lint y Clippy

- **ESLint** (`eslint.config.js`): configuración mínima con reglas de
  seguridad y de *hooks* de React.
- **Clippy** con `-D warnings` para todo el workspace y todos los objetivos
  (incluidas las pruebas), en Linux y para Windows.
- **rustfmt** con la configuración por defecto.
- **shellcheck** para los scripts.
- **cargo deny** (`deny.toml`) para licencias y avisos de dependencias.
- `python3 scripts/spdx-headers.py --check` para los encabezados SPDX.

## Generación de paquetes

```bash
npx tauri build --bundles deb                            # paquete .deb
bash scripts/deb-set-depends.sh target/release/bundle/deb/*.deb   # dependencias reales
bash scripts/build-deb-compat.sh                         # paquete compatible (Ubuntu 22.04 en Docker)
bash scripts/release-sums.sh CARPETA                     # SHA256SUMS
python3 scripts/third-party-licenses.py                  # avisos de terceros (Linux)
python3 scripts/third-party-licenses.py --windows        # avisos de terceros (Windows)
```

## Compilación para Linux

El paquete compilado en un equipo solo sirve para sistemas con una glibc igual
o más nueva. Para distribuir a Debian 12 y Ubuntu 22.04, el paquete
compatible se compila en un contenedor de Ubuntu 22.04 (glibc 2.35), sin red
durante la compilación y sin sudo (`scripts/build-deb-compat.sh`).

## Compilación para Windows

**Desde Linux con MinGW-w64** (instalador de prueba; comprobado en la 1.9.1):

```bash
sudo apt install gcc-mingw-w64-x86-64 nsis
bash scripts/build-windows.sh                  # con el destino de rustup
RUST_SRC=/ruta/rust/library bash scripts/build-windows.sh   # sin él, con -Zbuild-std
```

El resultado (`target/windows-installer/`) incluye el instalador, el programa
suelto, `WebView2Loader.dll` y `SHA256SUMS`.

**En Windows con MSVC** (sin ejecutar todavía):

```powershell
npm ci
npx tauri build --bundles nsis
```

**En GitHub Actions**: `windows-build.yml` (se lanza a mano).

**Pruebas de Windows desde Linux** con Wine (comprobado en la 1.9.1; 613
pruebas superadas):

```bash
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine64 \
cargo test --target x86_64-pc-windows-gnu --workspace --lib --bins
```

`verify-evolution-1.9.1.sh` lo hace solo si encuentra MinGW-w64, Wine y el
destino de Windows (o `RUST_SRC`).

## Regenerar este manual

```bash
bash scripts/build-manual.sh       # docs/manual → target/manual/Container-Manage-Center-Manual-1.9.1.pdf
```

Necesita pandoc, XeLaTeX (con `fontspec`, `polyglossia`, `titlesec`,
`fancyhdr`, `tcolorbox`), las fuentes Inter y DejaVu Sans Mono y, para los
diagramas, Mermaid CLI (`mmdc`).
