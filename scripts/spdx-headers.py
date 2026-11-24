#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
"""Encabezados SPDX del código propio de Container Manage Center (1.7.0, D-111).

Añade al principio de cada archivo de código propio versionado:

    SPDX-License-Identifier: GPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite

con el comentario de su lenguaje, después de la línea `#!` o `<!doctype>` si
la hay. No toca:
- el código generado (tipos TypeScript de ts-rs en ui/src/shared/ipc/bindings);
- archivos sin comentarios (JSON), imágenes ni documentos;
- archivos que ya tengan un aviso SPDX o de copyright (no se sustituye nada).
No hay código de terceros copiado en el repositorio (DEPENDENCY_LICENSE_AUDIT.md),
salvo src-tauri/windows-app-manifest.xml (el manifiesto de Windows de
tauri-build, Apache-2.0 OR MIT, con su aviso; 1.11.1).

Uso (desde la raíz del proyecto):
    python3 scripts/spdx-headers.py           # añade los que falten
    python3 scripts/spdx-headers.py --check   # falla si falta alguno
"""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LICENSE = "SPDX-License-Identifier: GPL-3.0-or-later"
HOLDER = "SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite"
GENERATED = ("ui/src/shared/ipc/bindings/",)
LINE = {".rs": "//", ".ts": "//", ".tsx": "//", ".js": "//", ".sh": "#", ".py": "#"}
BLOCK = {".css": ("/*", " */"), ".html": ("<!--", "-->")}
NAMED = {"Dockerfile.ubuntu2204": "#"}


def own_files():
    out = subprocess.run(["git", "ls-files"], cwd=ROOT, check=True, capture_output=True, text=True).stdout
    for rel in out.splitlines():
        p = Path(rel)
        if rel.startswith(GENERATED):
            continue
        if p.suffix in LINE or p.suffix in BLOCK or p.name in NAMED:
            yield rel


def header_for(rel):
    p = Path(rel)
    if p.name in NAMED or p.suffix in LINE:
        c = NAMED.get(p.name) or LINE[p.suffix]
        return f"{c} {LICENSE}\n{c} {HOLDER}\n"
    a, b = BLOCK[p.suffix]
    return f"{a}\n  {LICENSE}\n  {HOLDER}\n{b}\n"


def has_notice(text):
    head = "\n".join(text.splitlines()[:12])
    return "SPDX-License-Identifier" in head


def add(rel):
    path = ROOT / rel
    text = path.read_text(encoding="utf-8")
    if has_notice(text):
        return False
    if "Copyright" in "\n".join(text.splitlines()[:12]):
        # Aviso preexistente: no se toca sin comprobar su titularidad.
        print(f"  {rel}: tiene un aviso de copyright propio; se deja como está")
        return False
    h = header_for(rel)
    first, sep, rest = text.partition("\n")
    if first.startswith("#!") or first.lower().startswith("<!doctype"):
        new = first + "\n" + h + rest if sep else first + "\n" + h
    else:
        new = h + text
    path.write_text(new, encoding="utf-8")
    return True


def main():
    files = list(own_files())
    if "--check" in sys.argv:
        missing = [f for f in files if not has_notice((ROOT / f).read_text(encoding="utf-8"))]
        for f in missing:
            print(f"Sin encabezado SPDX: {f}")
        print(f"{len(files) - len(missing)} de {len(files)} archivos de código propio con encabezado SPDX")
        return 1 if missing else 0
    n = sum(add(f) for f in files)
    print(f"Encabezado añadido a {n} archivos ({len(files)} de código propio)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
