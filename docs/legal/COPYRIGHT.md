# Copyright de Container Manage Center

    Copyright © 2026 Sergio Alfaro Lorite
    SPDX-License-Identifier: GPL-3.0-or-later

## Titular

**Sergio Alfaro Lorite** es el titular de los derechos de autor del **código y
los materiales propios** de Container Manage Center (antes *Docker Control
Center*):

- el código de `crates/`, `src-tauri/` y `ui/src/`, salvo los tipos
  generados de `ui/src/shared/ipc/bindings/` (que ts-rs produce a partir de los
  tipos Rust del proyecto y siguen su misma titularidad y licencia);
- los scripts de `scripts/` y los archivos de `packaging/`;
- la documentación (`*.md`, `docs/`, la ayuda de la aplicación) y las
  traducciones de la interfaz;
- el logotipo (`packaging/logo.svg`) y los iconos derivados de él
  (`src-tauri/icons/`).

## Año

**2026**. Es el año que muestra el historial del repositorio: el primer commit
es del 4 de noviembre de 2026 (`0d1a194`, «Fase 1») y todo el desarrollo hasta
la 1.7.0 es de noviembre de 2026. No se ha publicado ninguna versión antes de
ese año. En versiones de años posteriores, el aviso se ampliará con el año de
cada modificación (por ejemplo, «2026-2027») cuando la haya.

## Cómo se ha desarrollado

El historial de Git registra que el código se ha escrito **con la ayuda de un
asistente de inteligencia artificial (Claude, de Anthropic), bajo la dirección
de Sergio Alfaro Lorite**, que ha definido los requisitos, ha tomado las
decisiones de diseño y de producto (DECISIONS.md) y ha validado el resultado.
Los commits figuran con el autor «Claude» (o con el nombre anterior del
proyecto) y la línea
`Co-Authored-By: Claude …`. No hay en el historial contribuciones de otras
personas.

Este documento no afirma que Sergio Alfaro Lorite sea el único autor de todo
lo que se distribuye: solo es titular del código y los materiales propios
indicados arriba. La protección por derechos de autor de las partes generadas
con ayuda de herramientas de IA depende de la legislación de cada país; este
documento no hace ninguna afirmación jurídica sobre ello.

## Lo que **no** es de Sergio Alfaro Lorite

- **Dependencias de terceros** (crates de Rust, paquetes npm, tipografías IBM
  Plex Sans y JetBrains Mono, iconos de lucide/Feather): pertenecen a sus
  autores y conservan sus licencias y avisos. Están en
  [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), con los titulares que
  declara cada componente, sin cambios.
- **Programas externos** que la aplicación usa sin incluirlos (Docker Engine,
  Docker Compose, OpenSSH, systemd, Trivy, Grype) y las bibliotecas del
  sistema.
- **El texto de la GPL** (`LICENSE`): Copyright © 2007 Free Software
  Foundation, Inc.
- **La marca «Docker»**, de Docker, Inc. Container Manage Center es un
  proyecto independiente, no afiliado a Docker, Inc. ni respaldado por ella.

## Contribuciones futuras

Quien contribuya conserva el copyright de su aportación y la publica con la
misma licencia (`GPL-3.0-or-later`); puede añadir su propia línea
`SPDX-FileCopyrightText` en los archivos que modifique. Véase
[CONTRIBUTING.md](../../CONTRIBUTING.md).
