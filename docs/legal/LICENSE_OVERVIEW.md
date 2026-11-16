# Licencia de Container Manage Center

| | |
|---|---|
| Programa | **Container Manage Center** (CMC) |
| Licencia | **GNU General Public License, versión 3 o posterior** |
| Identificador SPDX | `GPL-3.0-or-later` |
| Titular del copyright del código propio | **Sergio Alfaro Lorite** (véase [COPYRIGHT.md](COPYRIGHT.md)) |
| Aviso | `Copyright © 2026 Sergio Alfaro Lorite` |
| Texto de la licencia | [`LICENSE`](../../LICENSE) (texto oficial, idéntico a `/usr/share/common-licenses/GPL-3` de Debian) · https://www.gnu.org/licenses/gpl-3.0.html |
| Desde | 1.7.0 (D-111). Antes era la misma licencia, pero declarada como provisional (D-016) |

## Declaración

    Container Manage Center
    Copyright © 2026 Sergio Alfaro Lorite

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

La misma declaración, traducida, está en la ayuda de la aplicación (sección
«Legal») y en **Configuración › Legal**. La versión inglesa es la que vale.

### Permiso adicional para Windows (borrador)

Para la versión de Windows, el titular añade un **permiso adicional** según la
sección 7 de la GPL v3: permite distribuir el programa enlazado con el
cargador de Microsoft Edge WebView2 (binario de Microsoft, BSD-3-Clause, sin
código fuente publicado) sin incluir el código fuente de ese cargador. Texto
completo y alcance en [WEBVIEW2_EXCEPTION.md](WEBVIEW2_EXCEPTION.md).
**Pendiente de la revisión del titular**: hasta entonces, los instaladores de
Windows son solo para pruebas.

## Qué cubre la licencia

- **Cubre** el código propio del proyecto: los crates `cmc-activity`,
  `cmc-store`, `cmc-engine` y `src-tauri`; la interfaz (`ui/`); los scripts de
  `scripts/` y `packaging/`; la documentación; el logotipo y los iconos de la
  aplicación (`src-tauri/icons/`, `ui/public/`). Cada archivo de código lleva
  el encabezado:

      SPDX-License-Identifier: GPL-3.0-or-later
      SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite

  `python3 scripts/spdx-headers.py --check` comprueba que no falte ninguno.
  No llevan encabezado los archivos que no admiten comentarios (JSON) ni los
  generados (`ui/src/shared/ipc/bindings/`, producidos por ts-rs a partir de
  los tipos Rust del proyecto); están cubiertos por la misma licencia.
- **No cubre** las dependencias de terceros (crates de Rust, paquetes npm,
  tipografías). Conservan sus licencias y sus titulares: MIT, Apache-2.0,
  ISC, BSD, Zlib, Unicode-3.0, MPL-2.0, OFL-1.1 y otras compatibles. Están
  auditadas en [DEPENDENCY_LICENSE_AUDIT.md](DEPENDENCY_LICENSE_AUDIT.md) y
  sus textos completos están en [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
  No se ha modificado ningún archivo de terceros ni se les ha atribuido la GPL.
- **No cubre** los programas que la aplicación usa sin incluirlos: Docker
  Engine, Docker Compose, OpenSSH, systemd, Trivy o Grype, ni las bibliotecas
  del sistema (WebKitGTK, GTK, glibc) que instala apt.

## Por qué el conjunto puede distribuirse con GPL-3.0-or-later

El programa distribuido (un binario con la interfaz incrustada) combina el
código propio con dependencias cuyas licencias son compatibles con la GPL v3:

- Las permisivas (MIT, ISC, BSD, Zlib, Unicode-3.0, 0BSD, Unlicense…) solo
  exigen conservar su aviso y su licencia.
- **Apache-2.0** es compatible con la GPL v3 (no con la v2). Ninguna
  dependencia trae archivo `NOTICE`.
- **MPL-2.0** (5 crates) permite la combinación porque ningún archivo lleva el
  aviso «Incompatible With Secondary Licenses»; sus archivos siguen bajo MPL y
  su código está disponible.
- **OFL-1.1** (2 tipografías) permite incluirlas con el programa si se
  conservan la licencia y los avisos y no se venden por separado.
- `ring` no contiene código bajo la licencia OpenSSL antigua.

Detalle y comprobaciones: [DEPENDENCY_LICENSE_AUDIT.md](DEPENDENCY_LICENSE_AUDIT.md).
Obligaciones al distribuir: [DISTRIBUTION_COMPLIANCE.md](DISTRIBUTION_COMPLIANCE.md).

Este análisis es técnico y no constituye asesoramiento jurídico.

## Marcas

Container Manage Center es un proyecto independiente: no está afiliado a
Docker, Inc. ni respaldado por ella. «Docker» es una marca de Docker, Inc.; el
proyecto solo la usa para nombrar la tecnología con la que trabaja (Docker
Engine, su API, Docker Compose y la CLI de Docker). La licencia GPL no concede
derechos sobre marcas de terceros.
