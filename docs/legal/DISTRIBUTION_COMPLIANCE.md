# Cumplimiento al distribuir Container Manage Center

Qué hay que entregar con cada versión para cumplir la GPL-3.0-or-later y las
licencias de las dependencias, y cómo se comprueba. Análisis técnico, no
asesoramiento jurídico.

## 1. Lo que se publica en cada versión

| Archivo | Contenido |
|---|---|
| `container-manage-center_X.Y.Z_amd64.deb` | Paquete para Debian 13 / Ubuntu 24.04 o posterior |
| `container-manage-center_X.Y.Z_amd64-compat-debian12-ubuntu2204.deb` | Paquete compatible (glibc 2.35) |
| `container-manage-center-X.Y.Z-src.tar.gz` | **Código fuente correspondiente** (GPL v3 §6): el árbol completo del commit de la versión |
| `install.sh` | Instalador opcional |
| `SHA256SUMS` | Sumas de todo lo anterior |

El binario y el código fuente se ofrecen **en el mismo sitio y a la vez**
(GPL v3 §6 d). Si algún día se publicara solo el binario, habría que incluir
una oferta escrita del código fuente válida durante tres años (§6 b).

## 2. Lo que contiene el paquete .deb

En `/usr/share/doc/container-manage-center/`:

| Archivo | Para qué |
|---|---|
| `copyright` | Aviso en formato DEP-5: titular, licencia, componentes de terceros |
| `LICENSE` | Texto completo de la GPL v3 |
| `LICENSE_OVERVIEW.md` | Qué cubre la licencia y por qué el conjunto es compatible |
| `COPYRIGHT.md` | Titularidad y año |
| `THIRD_PARTY_NOTICES.md` | **Licencias y avisos completos de todos los componentes de terceros incluidos** |
| `DISTRIBUTION_COMPLIANCE.md` | Este documento |
| `MIGRATION.md` | Actualizar desde una versión anterior |
| `README.md`, `INSTALL.md`, `USER_GUIDE.md` | Documentación |

Además, la aplicación muestra la licencia, el copyright y la ruta de los
avisos en **Configuración › Legal** y en la sección «Legal» de la ayuda
(GPL v3 §0 y §5 d: «avisos legales apropiados» en la interfaz interactiva).

## 3. Obligaciones de las dependencias

| Licencia | Obligación | Cómo se cumple |
|---|---|---|
| MIT, ISC, BSD, Zlib, Unicode-3.0, 0BSD, Unlicense… | Conservar aviso de copyright y licencia | Textos completos en `THIRD_PARTY_NOTICES.md` |
| Apache-2.0 | Conservar licencia y avisos; incluir `NOTICE` si existe | Texto completo; ninguna dependencia trae `NOTICE` (comprobado por el script) |
| MPL-2.0 (5 crates) | Conservar avisos; código fuente de esos archivos disponible | Avisos y texto MPL en `THIRD_PARTY_NOTICES.md`; el código está en crates.io (versiones fijadas en `Cargo.lock`, incluido en el .tar.gz) |
| OFL-1.1 (IBM Plex Sans, JetBrains Mono) | Distribuir con su licencia; no venderlas por separado; no usar el nombre reservado si se modifican | Texto OFL en `THIRD_PARTY_NOTICES.md`; se incluyen sin modificar dentro del programa |

Los componentes que solo se usan para compilar o probar (20 crates y 336
paquetes npm) no se distribuyen y no generan obligaciones en el paquete.

## 4. Lo que el .tar.gz de código fuente **no** debe contener

Claves privadas, contraseñas, tokens, credenciales, archivos personales,
configuraciones locales ni secretos de desarrollo. El .tar.gz se genera con
`git archive` del commit de la versión, de modo que solo contiene archivos
versionados; antes de publicarlo se comprueba:

```bash
git archive --format=tar.gz --prefix=container-manage-center-X.Y.Z/ \
    -o container-manage-center-X.Y.Z-src.tar.gz vX.Y.Z   # o el hash del commit
# Igual al commit:
diff <(tar -tzf container-manage-center-X.Y.Z-src.tar.gz | sed 's#^[^/]*/##' | grep -v '/$' | sort) \
     <(git ls-tree -r --name-only HEAD | sort)
# Sin secretos (debe salir vacío):
tar -tzf container-manage-center-X.Y.Z-src.tar.gz | grep -Ei '\.(pem|key|p12|pfx|env)$|id_(rsa|ed25519|ecdsa)|known_hosts|\.netrc|credentials'
```

El repositorio no contiene claves de prueba: los servidores SSH de prueba
generan las suyas al vuelo, fuera del árbol del proyecto.

## 5. Antes de publicar una versión

1. `python3 scripts/third-party-licenses.py --check`: los avisos y la
   auditoría corresponden a `Cargo.lock` y `package-lock.json`; ninguna
   licencia nueva queda sin clasificar. Si aparece un componente de las
   clases 4, 5 o 6, **no se publica** hasta resolverlo.
2. `cargo deny check licenses`: solo licencias de la lista permitida
   (`deny.toml`).
3. `python3 scripts/spdx-headers.py --check`: todo el código propio lleva su
   encabezado SPDX.
4. `scripts/verify-evolution-X.Y.Z.sh --deb`: comprueba, entre otras cosas,
   que el paquete contiene los archivos del apartado 2 y que los metadatos
   (nombre, versión, licencia) coinciden en todos los manifiestos.
5. Se genera el .tar.gz y se hacen las comprobaciones del apartado 4.
6. `scripts/release-sums.sh` genera `SHA256SUMS`.

## 5 bis. Windows (1.9.0, D-114): no se distribuye todavía

El instalador de Windows incluye **WebView2Loader** de Microsoft (como
`WebView2Loader.dll` en la compilación con MinGW y dentro del programa en la
de MSVC). Su licencia es BSD-3-Clause, compatible con la GPL, pero Microsoft
**no publica su código fuente**, y la GPL v3 pide ofrecer el código fuente
correspondiente de lo que se distribuye. No está claro que la excepción de
bibliotecas del sistema lo cubra.

Por eso el titular del copyright concede un **permiso adicional** según la
sección 7 de la GPL v3 ([WEBVIEW2_EXCEPTION.md](WEBVIEW2_EXCEPTION.md),
aprobado el 2026-11-17): se puede distribuir el programa enlazado con el
cargador de WebView2 sin incluir el código fuente de ese cargador. El resto
del código fuente correspondiente se ofrece completo, como siempre.

- Quien redistribuya CMC para Windows debe conservar ese permiso junto a la
  licencia (el instalador lo lleva en `legal\WEBVIEW2_EXCEPTION.md`); quien
  distribuya una versión modificada puede quitarlo de su versión.
- La versión de Windows sigue **en pruebas** hasta su validación en un
  Windows real; eso no es una restricción legal.
- El flujo de GitHub Actions solo se lanza a mano y guarda el instalador como
  artefacto de la ejecución, sin crear versiones; publicarlas es una decisión
  del titular.

Cuando se distribuya, además de lo de los apartados anteriores: el instalador
lleva `LICENSE.txt`, `THIRD_PARTY_NOTICES.md` (la de Windows, generada con
`scripts/third-party-licenses.py --windows`) y los textos de WebView2Loader y
de MinGW-w64/libgcc en la carpeta `legal\` de la instalación.

## 6. Historial

Hasta la 1.6.0, el paquete incluía
`THIRD-PARTY.md`, que solo era una **tabla** de componentes y licencias, sin
los textos de licencia ni los avisos de copyright. Eso no cumplía del todo la
obligación de conservar los avisos (MIT, Apache-2.0, BSD…). Desde la 1.7.0 el
paquete incluye `THIRD_PARTY_NOTICES.md` con los textos completos; la 1.7.0 y
posteriores sustituyen a las versiones anteriores.
