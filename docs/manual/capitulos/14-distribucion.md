# Distribución y versiones

![Del código fuente a la publicación.](diagramas/10-distribucion.pdf){width=92%}

## Versionado

CMC sigue el **versionado semántico**: `MAYOR.MENOR.PARCHE`. La versión se
mantiene a la vez en `Cargo.toml` (workspace), `package.json`,
`package-lock.json` y `src-tauri/tauri.conf.json`, y aparece en
**Configuración › Sistema**. La historia de versiones está en
`CHANGELOG.md`; las más recientes:

| Versión | Contenido |
|---|---|
| 1.7.0 | Identidad Container Manage Center, licencia GPL-3.0-or-later, avisos de terceros |
| 1.8.0 | Fin del nombre anterior, con migración de los datos |
| 1.9.0 | Versión de Windows en pruebas |
| 1.9.1 | Auditoría y correcciones de Windows, pruebas con Wine, preparación para GitHub |

## Paquetes Debian

- `container-manage-center_<versión>_amd64.deb`: compilado en Ubuntu 24.04
  (glibc 2.39). Dependencias reales calculadas con `dpkg-shlibdeps`.
- `…_amd64-compat-debian12-ubuntu2204.deb`: compilado en un contenedor de
  Ubuntu 22.04; sirve para Debian 12, Debian 13, Ubuntu 22.04 y 24.04.
- Contenido: `/usr/bin/container-manage-center`, el lanzador `.desktop`, los
  iconos y, en `/usr/share/doc/container-manage-center/`, la licencia, el
  copyright, los avisos de terceros y la guía de migración.
- `Conflicts`/`Replaces` con el paquete del nombre anterior para que `apt` lo
  sustituya.

## Instaladores de Windows

- Instalador **NSIS** (`…_x64-setup.exe`) para el usuario actual, sin
  administrador, en inglés, español, francés, italiano o portugués.
- Incluye en `legal\` la licencia, los avisos de terceros de Windows, el
  permiso adicional de WebView2 y los textos de licencia de WebView2Loader y
  de MinGW-w64/libgcc.
- Descarga el WebView2 Runtime si falta.
- **No está firmado** (sin certificado de firma de código).
- **Solo para pruebas** hasta la validación en Windows real y la aprobación del
  permiso adicional.

## Código fuente

Cada entrega incluye `container-manage-center-<versión>-src.tar.gz`, generado
con `git archive` del commit de la versión (sin `.git`, sin compilaciones, sin
secretos), y `SHA256SUMS` con las sumas de todo.

## Dependencias

- **Rust**: `Cargo.lock` fija las versiones; `cargo deny` comprueba licencias
  y avisos.
- **npm**: `package-lock.json`; `npm audit` en la verificación.
- **Avisos**: `docs/legal/THIRD_PARTY_NOTICES.md` (Linux) y
  `THIRD_PARTY_NOTICES-windows.md` (Windows), con los textos completos de las
  licencias de todo lo que se distribuye; auditorías en
  `DEPENDENCY_LICENSE_AUDIT*.md`. Ninguna licencia incompatible ni
  indeterminada.

## Actualizaciones

La aplicación **solo consulta y avisa**: con una fuente configurada (un
repositorio de GitHub o un JSON por HTTPS), comprueba si hay una versión nueva
al pulsar **Comprobar ahora** o al iniciar, y enlaza a su página. **Nunca
descarga ni instala nada**:

- Linux: `sudo apt install ./container-manage-center_X.Y.Z_amd64.deb`
  (se conservan la configuración y el historial);
- Windows: ejecutar el instalador nuevo, que sustituye al anterior.

## Distribución

La publicación es una decisión del titular. Antes de publicar:

1. `verify-evolution-1.9.1.sh --deb` sin fallos.
2. Paquetes, instalador (si se publica), código fuente y `SHA256SUMS` en la
   misma publicación.
3. Para Windows, aprobar el permiso adicional de WebView2 y validar en un
   Windows real.

## Requisitos de la GPL al distribuir

Quien distribuya CMC (también modificado) debe, según la GPL v3:

- acompañar los binarios del **código fuente correspondiente** o de una
  oferta para obtenerlo (el `…-src.tar.gz` de la misma versión cumple esto);
- conservar los avisos de copyright, la licencia y los avisos de terceros;
- distribuir las modificaciones con la misma licencia, indicando que se han
  modificado;
- no añadir restricciones a los derechos de quien recibe el programa.

Para la versión de Windows, el permiso adicional de la sección 7 (en
borrador) exime de incluir el código fuente del cargador de WebView2 de
Microsoft. El detalle está en `docs/legal/DISTRIBUTION_COMPLIANCE.md`.
