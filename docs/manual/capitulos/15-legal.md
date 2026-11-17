# Apartado legal

**Container Manage Center (CMC)**

**Copyright © 2026 Sergio Alfaro Lorite**

**Licencia: GNU General Public License v3.0 or later (GPL-3.0-or-later)**

El año 2026 es el que muestra el historial del repositorio: el primer commit
es del 4 de noviembre de 2026 y no se ha publicado ninguna versión antes.

## La licencia

Container Manage Center es **software libre**: puedes usarlo, estudiarlo,
modificarlo y redistribuirlo según los términos de la GNU General Public
License publicada por la Free Software Foundation, versión 3 o (a tu
elección) cualquier versión posterior. La licencia garantiza esas libertades
a todos los que reciban el programa, y exige que las versiones modificadas
que se distribuyan mantengan las mismas libertades (*copyleft*).

Texto oficial: <https://www.gnu.org/licenses/gpl-3.0.html>. El texto
completo está en el archivo `LICENSE` del repositorio, en
`/usr/share/doc/container-manage-center/LICENSE` (Linux) y en
`legal\LICENSE.txt` de la carpeta de instalación (Windows).

## Declaración

```text
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
```

La traducción de esta declaración está en la ayuda de la aplicación y en
**Configuración › Legal**; la versión inglesa es la que tiene efecto.

## Permiso adicional para Windows

Para la versión de Windows, el titular añade un permiso adicional según la
sección 7 de la GPL v3, que permite distribuir el programa enlazado con el
cargador de Microsoft Edge WebView2 sin incluir el código fuente de ese
cargador (que Microsoft no publica). Lo **aprobó el 17 de noviembre de 2026**
y se aplica desde la versión 1.9.1. El texto completo (en inglés, el que tiene
efecto, con su traducción) está en `docs/legal/WEBVIEW2_EXCEPTION.md` y, en
Windows, en `legal\WEBVIEW2_EXCEPTION.md` de la carpeta de instalación.

## Redistribución

Puedes redistribuir copias exactas o modificadas, gratis o cobrando, siempre
que:

- incluyas la licencia y conserves los avisos de copyright y de terceros;
- ofrezcas el código fuente correspondiente;
- licencies tus modificaciones con la GPL-3.0-or-later e indiques que las has
  hecho.

Más detalles en `docs/legal/DISTRIBUTION_COMPLIANCE.md`.

## Avisos de terceros

CMC incluye bibliotecas de Rust y de JavaScript, las tipografías IBM Plex
Sans y JetBrains Mono y los iconos de lucide, de otros autores. **No son de
Sergio Alfaro Lorite** ni pasan a ser GPL: conservan sus licencias (MIT,
Apache-2.0, BSD, ISC, MPL-2.0, OFL-1.1, Zlib…) y sus avisos. La versión de
Windows incluye además WebView2Loader (Microsoft, BSD-3-Clause) y, en la
compilación con MinGW, partes del runtime de MinGW-w64 y libgcc.

- `docs/legal/THIRD_PARTY_NOTICES.md` (Linux) y
  `THIRD_PARTY_NOTICES-windows.md` (Windows): textos completos.
- `docs/legal/DEPENDENCY_LICENSE_AUDIT.md` y `…-windows.md`: auditoría de
  licencias.
- `docs/legal/COPYRIGHT.md`: titularidad del código propio.

## Limitación de responsabilidad

Según las secciones 15 y 16 de la GPL v3, el programa se proporciona «tal
cual», **sin garantía de ningún tipo**, en la medida en que lo permita la ley
aplicable, y ningún titular del copyright ni distribuidor será responsable de
los daños derivados de su uso o de la imposibilidad de usarlo, salvo que la
ley aplicable lo exija o se acuerde por escrito. El texto de la licencia es el
que tiene efecto.

## Marcas

Container Manage Center es un proyecto independiente: no está afiliado a
Docker, Inc. ni respaldado por ella. «Docker» es una marca de Docker, Inc. y
aquí solo se usa para nombrar la tecnología con la que trabaja la aplicación.
Windows, WebView2 y Microsoft Edge son marcas de Microsoft Corporation. Las
demás marcas pertenecen a sus titulares.
