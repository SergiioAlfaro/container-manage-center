#!/usr/bin/env python3
"""Genera packaging/THIRD-PARTY.md: los componentes de terceros que se
distribuyen dentro del paquete y sus licencias (Fase 10).

- Rust: las bibliotecas que se enlazan en el binario de la aplicación
  (dependencias normales, no de compilación ni de pruebas, para Linux x86_64),
  según `cargo metadata`.
- Interfaz: los paquetes npm de producción que se incluyen en la interfaz
  compilada (fuentes incluidas), según `package-lock.json`.

Uso (desde la raíz del proyecto):
    python3 scripts/third-party-licenses.py           # escribe el archivo
    python3 scripts/third-party-licenses.py --check   # falla si no está al día

No descarga nada: `cargo metadata --offline` usa el registro local.
"""

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "packaging" / "THIRD-PARTY.md"
TARGET = "x86_64-unknown-linux-gnu"
OWN = {"docker-control-center", "dcc-engine", "dcc-store", "dcc-activity"}


def rust_crates():
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--offline", "--filter-platform", TARGET],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    pkgs = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = next(p["id"] for p in meta["packages"] if p["name"] == "docker-control-center")
    seen, stack = set(), [root]
    while stack:
        nid = stack.pop()
        if nid in seen:
            continue
        seen.add(nid)
        for dep in nodes[nid]["deps"]:
            # Solo dependencias normales (kind null): las de compilación y pruebas no se distribuyen.
            if any(k["kind"] is None for k in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    out = []
    for nid in seen:
        p = pkgs[nid]
        if p["name"] in OWN:
            continue
        out.append((p["name"], p["version"], p.get("license") or ("ver " + p["license_file"] if p.get("license_file") else "sin indicar")))
    return sorted(out)


def npm_packages():
    lock = json.loads((ROOT / "package-lock.json").read_text())
    out = []
    for key, v in lock["packages"].items():
        if not key or v.get("dev") or v.get("devOptional"):
            continue
        name = key.split("node_modules/")[-1]
        out.append((name, v.get("version", ""), v.get("license") or "sin indicar"))
    return sorted(out)


def render():
    rust, npm = rust_crates(), npm_packages()
    lines = [
        "# Componentes de terceros",
        "",
        "Docker Control Center incluye los componentes siguientes, cada uno con su licencia.",
        "Este archivo lo genera `scripts/third-party-licenses.py` a partir de `Cargo.lock` y",
        "`package-lock.json`; no se edita a mano.",
        "",
        f"## Bibliotecas Rust enlazadas en el programa ({len(rust)})",
        "",
        "| Componente | Versión | Licencia |",
        "|---|---|---|",
    ]
    lines += [f"| {n} | {v} | {l} |" for n, v, l in rust]
    lines += [
        "",
        f"## Interfaz: paquetes npm incluidos ({len(npm)})",
        "",
        "Las fuentes IBM Plex Sans y JetBrains Mono se distribuyen con la licencia SIL Open Font License 1.1.",
        "",
        "| Componente | Versión | Licencia |",
        "|---|---|---|",
    ]
    lines += [f"| {n} | {v} | {l} |" for n, v, l in npm]
    lines += [
        "",
        "Las bibliotecas del sistema (WebKitGTK, GTK, glib…) no se incluyen: las instala el",
        "sistema como dependencias del paquete.",
        "",
    ]
    return "\n".join(lines)


def main():
    text = render()
    if "--check" in sys.argv:
        if not OUT.exists() or OUT.read_text() != text:
            print(f"{OUT.relative_to(ROOT)} no está al día: ejecuta python3 scripts/third-party-licenses.py")
            return 1
        print(f"{OUT.relative_to(ROOT)} está al día")
        return 0
    OUT.write_text(text)
    print(f"Escrito {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
