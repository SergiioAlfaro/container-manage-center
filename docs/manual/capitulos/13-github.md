# Estructura de GitHub y contribución

## Repositorio

El repositorio es
**`https://github.com/SergiioAlfaro/container-manage-center`** (privado; rama
principal `main`). Se publicó por primera vez el 24 de noviembre de 2026 con
el historial completo hasta la 1.11.0; desde entonces cada cambio entra por
una rama y un *pull request*. Su estructura es la del capítulo
[Estructura del proyecto](#estructura-del-proyecto); para GitHub se añaden:

| Archivo o carpeta | Función |
|---|---|
| `README.md` | Presentación, capturas, estado real de cada plataforma, instalación y enlaces |
| `LICENSE` | Texto completo de la GPL v3 |
| `CHANGELOG.md` | Cambios de cada versión |
| `CONTRIBUTING.md` | Licencia de las contribuciones, encabezados SPDX, dependencias, pruebas |
| `SECURITY.md` | Versiones con soporte y aviso privado de vulnerabilidades |
| `CODE_OF_CONDUCT.md` | Contributor Covenant 2.1 |
| `.gitignore` | Compilaciones, dependencias, informes, paquetes, secretos y archivos personales |
| `.gitattributes` | Finales de línea (LF; CRLF para los archivos de Windows), binarios y archivos generados |
| `.github/ISSUE_TEMPLATE/` | Formularios de error y de propuesta; enlace al aviso privado de seguridad |
| `.github/PULL_REQUEST_TEMPLATE.md` | Lista de comprobación de cada cambio |
| `.github/workflows/` | `ci.yml`, `linux-build.yml`, `windows-build.yml`, `release-draft.yml` |

## Cómo clonar el proyecto

```bash
git clone https://github.com/SergiioAlfaro/container-manage-center.git
cd container-manage-center
```

## Cómo preparar el entorno

Ver [Desarrollo y compilación](#desarrollo-y-compilación): dependencias del
sistema, Rust, Node.js y `npm ci`.

## Cómo crear cambios

1. Crea una rama desde `main` con un nombre descriptivo
   (`corrige-ruta-compose-windows`).
2. Haz el cambio con sus pruebas. Si toca la interfaz, añade los textos en los
   **ocho idiomas**.
3. Los archivos nuevos de código llevan el encabezado SPDX:

   ```rust
   // SPDX-License-Identifier: GPL-3.0-or-later
   // SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
   ```

4. Si añades una dependencia, comprueba que su licencia es compatible y
   regenera los avisos (`third-party-licenses.py` y `--windows`).

## Cómo ejecutar las pruebas

```bash
npm run typecheck && npm run lint && npm test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/verify-evolution-1.11.0.sh --deb     # completa, con Docker
```

## Cómo preparar una contribución

- Un cambio por *pull request*, con su motivo.
- Rellena la plantilla: qué cambia, cómo se ha probado (y si toca Windows,
  qué se ha probado en un Windows real).
- No incluyas secretos, rutas personales ni datos reales (usa valores de
  ejemplo).
- Al enviar aceptas publicar tu contribución con la GPL-3.0-or-later y con el
  permiso adicional para WebView2.

## Pull requests

Cada *pull request* pasa la integración continua (`ci.yml`):

| Trabajo | Qué comprueba |
|---|---|
| Interfaz | `npm ci`, tipos, ESLint y Vitest |
| Rust en Linux | rustfmt, Clippy `-D warnings` y `cargo test` (sin Docker real) |
| Rust para Windows | Clippy y `cargo test` en un runner de Windows |
| Licencias | Encabezados SPDX y avisos de terceros al día (Linux y Windows) |

Cada trabajo guarda su informe de pruebas como artefacto de la ejecución
(`informe-interfaz`, con el XML JUnit de Vitest; `informe-rust-linux`;
`informe-rust-windows`).

Las acciones de GitHub están **fijadas por commit** (no por etiqueta), los
flujos tienen permisos de **solo lectura** y no usan secretos. Las pruebas que
necesitan Docker real y los paquetes van en `linux-build.yml` y
`windows-build.yml`, que se lanzan a mano y guardan los resultados como
artefactos de la ejecución, **sin publicar** ninguna versión.

## Flujos de trabajo y cómo lanzarlos

| Flujo | Cuándo se ejecuta | Qué produce |
|---|---|---|
| `ci.yml` | En cada *push* a `main`, en cada *pull request* y a mano | Informes de pruebas (interfaz, Rust en Linux y en Windows) y comprobación de licencias |
| `linux-build.yml` | A mano | `.deb` compilado en Ubuntu 22.04 (sirve para Debian 12+ y Ubuntu 22.04+) con `SHA256SUMS`; opcionalmente, pruebas contra el Docker del runner |
| `windows-build.yml` | A mano | Instalador NSIS (`…_x64-setup-msvc.exe`), avisos de terceros de Windows y `SHA256SUMS` |
| `release-draft.yml` | A mano, con la versión exacta | Llama a los dos anteriores y crea una **Release en borrador** con el `.deb`, el `.exe`, el código fuente, el manual en PDF y `SHA256SUMS` |

Para lanzar uno desde la web: pestaña **Actions** del repositorio › el flujo
en la lista de la izquierda › **Run workflow** › rama (y, en
`release-draft.yml`, la versión) › **Run workflow**. Los archivos quedan en el
resumen de la ejecución, en **Artifacts**, durante 14 días.

GitHub solo permite lanzar a mano los flujos que están en la rama principal:
`release-draft.yml` se puede usar una vez fusionado en `main`.

`release-draft.yml` nunca publica nada por sí solo: comprueba que la versión
coincide con `package.json`, `Cargo.toml` y `CHANGELOG.md`, se detiene si ya
existe una Release con esa etiqueta y crea un **borrador**, que solo ve el
titular. La etiqueta `v<versión>` se crea cuando el titular revisa el
borrador y pulsa **Publish release**.

## Issues

- **Error**: formulario con versión, sistema, Docker, dónde está Docker, pasos,
  resultado esperado y registro de actividad.
- **Propuesta**: problema, propuesta, implicaciones de seguridad y
  plataformas.
- **Seguridad**: nunca en un issue público; aviso privado de GitHub
  (`SECURITY.md`).

## Convenciones del proyecto

- **Idioma**: código, comentarios, documentación y mensajes en castellano;
  identificadores en inglés; interfaz en ocho idiomas.
- **Decisiones**: cada decisión de diseño o de producto se anota en
  `DECISIONS.md` (D-001 a D-125) y se cita en el código.
- **Versionado**: SemVer (`MAYOR.MENOR.PARCHE`); la versión está en
  `Cargo.toml`, `package.json` y `tauri.conf.json`.
- **Mensajes de commit**: en castellano, con la versión y el resumen
  (`1.11.1: …`).
- **Seguridad primero**: ninguna operación destructiva sin confirmación en el
  backend; ninguna orden con shell; ningún secreto en registros.
- **Sin telemetría** ni servicios externos: la aplicación solo se conecta a
  Docker, a los servidores configurados y, si se configura, a la fuente de
  actualizaciones.
