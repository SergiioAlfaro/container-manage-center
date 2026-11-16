# Contribuir a Container Manage Center

## Licencia de las contribuciones

Container Manage Center se distribuye con la licencia **GPL-3.0-or-later**
(véase [LICENSE](LICENSE) y [docs/legal/LICENSE_OVERVIEW.md](docs/legal/LICENSE_OVERVIEW.md)).
Al enviar una contribución aceptas publicarla con esa misma licencia.
Conservas el copyright de lo que aportes.

Las contribuciones se aceptan también con el permiso adicional para el
cargador de WebView2 de Microsoft en Windows
([docs/legal/WEBVIEW2_EXCEPTION.md](docs/legal/WEBVIEW2_EXCEPTION.md)), para
que el proyecto pueda seguir distribuyendo la versión de Windows.

## Encabezados SPDX

Cada archivo de código propio empieza con:

```
// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
```

(con `#` en shell y Python, `/* … */` en CSS y `<!-- … -->` en HTML; después
de la línea `#!` si la hay). En un archivo nuevo, `python3 scripts/spdx-headers.py`
lo añade. Si modificas de forma sustancial un archivo, puedes añadir tu propia
línea debajo:

```
// SPDX-FileCopyrightText: 2027 Tu Nombre <correo@ejemplo.org>
```

No quites ni cambies las líneas de otros titulares. `python3
scripts/spdx-headers.py --check` comprueba que ningún archivo propio se quede
sin encabezado.

## Código y dependencias de terceros

- **No copies código de terceros** en el repositorio. Si hace falta una
  biblioteca, añádela como dependencia (Cargo o npm).
- La dependencia debe tener una licencia compatible con la GPL v3 y estar
  permitida en `deny.toml`. Después de añadirla:
  ```bash
  cargo deny check licenses
  python3 scripts/third-party-licenses.py          # regenera los avisos y la auditoría
  python3 scripts/third-party-licenses.py --check  # falla si algo queda sin clasificar
  ```
  e incluye en el mismo cambio `docs/legal/THIRD_PARTY_NOTICES.md` y
  `docs/legal/DEPENDENCY_LICENSE_AUDIT.md` regenerados.
- No pongas encabezados SPDX del proyecto en archivos de terceros ni en el
  código generado (`ui/src/shared/ipc/bindings/`).

## Nombre y marcas

El producto se llama **Container Manage Center** (CMC). Usa «Docker» solo para
nombrar la tecnología (Docker Engine, su API, Docker Compose, la CLI de
Docker), nunca como parte del nombre del producto. Los identificadores
internos usan el prefijo `cmc` (crates `cmc-*`, variables `CMC_*`, etiquetas
`cmc.*`). Los del nombre anterior solo existen en los módulos `legacy` de
`cmc-store` y `cmc-engine`, para pasar al nombre nuevo los datos de una
versión anterior (D-113); no los uses para nada más.

## Antes de enviar un cambio

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd ui && npx tsc --noEmit && npx eslint . && npx vitest run
```

Las pautas de diseño y arquitectura están en [ARCHITECTURE.md](ARCHITECTURE.md)
y las decisiones tomadas, en [DECISIONS.md](DECISIONS.md).
