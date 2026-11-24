# Publicar una versión de Container Manage Center

Guía para publicar una versión **solo desde la web de GitHub**, sin entorno de
desarrollo local (1.11.1, D-125). Todo lo que compila o empaqueta lo hace
GitHub Actions; publicar es siempre una decisión del titular.

## Qué hay en el repositorio

| Flujo (pestaña **Actions**) | Para qué sirve |
|---|---|
| `ci` | Se ejecuta solo en cada *pull request* y en cada cambio de `main`: interfaz (tipos, lint, Vitest), Rust en Linux y en Windows, licencias. Guarda los informes de pruebas en **Artifacts** |
| `linux-build` | Compila el paquete `.deb` (en Ubuntu 22.04: vale para Debian 12+ y Ubuntu 22.04+) y, si se marca, prueba contra Docker real |
| `windows-build` | Compila el instalador de Windows `.exe` (NSIS, MSVC) |
| `release-draft` | Hace lo anterior y crea una **Release en borrador** con el `.deb`, el `.exe`, el código fuente, el manual en PDF y `SHA256SUMS` |

## Antes de publicar

1. Todos los cambios están fusionados en `main` y el flujo `ci` de `main` está
   en verde.
2. La versión nueva está en `package.json`, `package-lock.json`,
   `Cargo.toml`, `Cargo.lock` y `src-tauri/tauri.conf.json`, y `CHANGELOG.md`
   tiene la sección `## <versión> — <fecha> (…)`.
3. El manual en PDF está al día en `docs/Container-Manage-Center-Manual-ES.pdf`
   (se genera con `bash scripts/build-manual.sh`, que necesita pandoc y
   XeLaTeX: lo hace quien prepare la versión).

## Crear el borrador

1. **Actions** › **release-draft** (lista de la izquierda) › **Run workflow**.
2. Rama: `main`. Versión: la exacta, por ejemplo `1.11.1`. **Run workflow**.
3. Al terminar (unos 25 minutos; la compilación de Windows es la más lenta),
   en **Releases** aparece **Container Manage Center 1.11.1** marcada como
   *Draft*. Solo la ve el titular.

Si el flujo falla en «Versión y Release existente», el mensaje dice por qué:
versión distinta en algún archivo, falta la sección del `CHANGELOG.md` o ya
existe una Release con esa etiqueta (nunca la sustituye).

## Revisar y publicar

1. Abre el borrador (**Releases** › la versión › icono del lápiz).
2. Comprueba los archivos: `container-manage-center_<versión>_amd64.deb`,
   `Container.Manage.Center_<versión>_x64-setup-msvc.exe`,
   `container-manage-center-<versión>-src.tar.gz`,
   `Container-Manage-Center-Manual-<versión>-ES.pdf` y `SHA256SUMS`.
3. Revisa las notas (vienen del `CHANGELOG.md`) y añade, si quieres, el
   estado de las pruebas en un Windows real.
4. **Set as a pre-release** marca la versión como preliminar. Ten en cuenta
   que la comprobación de actualizaciones de la aplicación **ignora las
   versiones preliminares y los borradores**: si quieres que los usuarios la
   vean anunciada, publícala como versión normal y explica en las notas que
   Windows sigue en pruebas.
5. **Publish release**. En ese momento se crea la etiqueta `v<versión>` sobre el
   commit con el que se compiló.

No sustituyas los archivos de una versión ya publicada: una corrección es una
versión nueva (por ejemplo, 1.11.2).

## Si un flujo no está en la lista

GitHub solo permite lanzar a mano los flujos que están en la rama principal.
Un flujo nuevo aparece en **Actions** cuando su *pull request* se fusiona en
`main`.

## Comprobar una descarga

En Linux, con los archivos y `SHA256SUMS` en la misma carpeta:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

En Windows (PowerShell):

```powershell
Get-FileHash .\Container.Manage.Center_1.11.1_x64-setup-msvc.exe -Algorithm SHA256
```

y compara el resultado con la línea del archivo en `SHA256SUMS`.
