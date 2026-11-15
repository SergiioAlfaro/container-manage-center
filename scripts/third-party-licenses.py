#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
"""Avisos y auditoría de licencias de los componentes de terceros (1.7.0, D-112).

Genera dos documentos a partir de `Cargo.lock` y `package-lock.json`, sin
descargar nada (`cargo metadata --offline`, el registro local de Cargo y
`node_modules`):

- docs/legal/THIRD_PARTY_NOTICES.md: los avisos de copyright y los textos de
  licencia **completos** de cada componente que se distribuye dentro del
  paquete (bibliotecas Rust enlazadas en el programa y paquetes npm incluidos
  en la interfaz, tipografías incluidas). Se copian tal cual de los archivos
  LICENSE/COPYING/NOTICE de cada componente; los textos idénticos se escriben
  una sola vez. Va en el .deb y en el código fuente.
- docs/legal/DEPENDENCY_LICENSE_AUDIT.md: el inventario completo (también lo
  que solo se usa para compilar o probar), la clasificación frente a
  GPL-3.0-or-later y las incidencias.

Con `--windows` (1.9.0, D-114) hace lo mismo para Windows x86_64
(`THIRD_PARTY_NOTICES-windows.md` y `DEPENDENCY_LICENSE_AUDIT-windows.md`) y
añade lo que solo va en el instalador de Windows: WebView2Loader.dll de
Microsoft y, en la compilación con MinGW, su runtime y el de GCC (textos en
`packaging/windows/licenses/`).

Uso (desde la raíz del proyecto):
    python3 scripts/third-party-licenses.py           # escribe los dos archivos
    python3 scripts/third-party-licenses.py --check   # falla si no están al día
    python3 scripts/third-party-licenses.py --windows [--check]
"""

import hashlib
import json
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEGAL = ROOT / "docs" / "legal"
WINDOWS = "--windows" in sys.argv
SUFFIX = "-windows" if WINDOWS else ""
NOTICES = LEGAL / f"THIRD_PARTY_NOTICES{SUFFIX}.md"
AUDIT = LEGAL / f"DEPENDENCY_LICENSE_AUDIT{SUFFIX}.md"
# CMC_LICENSE_TARGET permite generar la lista de Windows para MSVC (GitHub
# Actions); por defecto, la de la compilación con MinGW.
TARGET = os.environ.get("CMC_LICENSE_TARGET") or ("x86_64-pc-windows-gnu" if WINDOWS else "x86_64-unknown-linux-gnu")
PLATFORM = "Windows x86_64" if WINDOWS else "Linux x86_64"
WIN_LICENSES = ROOT / "packaging" / "windows" / "licenses"
# Componentes que no vienen de Cargo ni de npm y solo van en Windows.
WINDOWS_EXTRA = [
    # (nombre, versión, licencia, archivo, origen, nota)
    ("WebView2Loader.dll (Microsoft Edge WebView2 SDK)", "1.0.3650.58", "BSD-3-Clause",
     "WebView2Loader.LICENSE.txt", "estándar",
     "binario de Microsoft incluido en webview2-com-sys 0.39.1 (sin código fuente publicado)"),
    ("Runtime de MinGW-w64 (solo compilación con MinGW)", "11.0.1", "ZPL-2.1 AND MIT AND LicenseRef-public-domain",
     "mingw-w64-runtime.copyright", "sistema", "archivo copyright de Debian/Ubuntu"),
    ("libgcc de GCC (solo compilación con MinGW)", "13.2.0", "GPL-3.0-or-later WITH GCC-exception-3.1",
     "gcc-mingw-w64.copyright", "sistema", "archivo copyright de Debian/Ubuntu"),
]
# Crates del propio proyecto (nombres internos, D-110).
OWN = {"container-manage-center", "cmc-engine", "cmc-store", "cmc-activity"}
LICENSE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|COPYRIGHT|UNLICENSE)([-._].*)?$", re.I)

# Licencias permisivas compatibles con GPL-3.0-or-later (obligan a conservar
# el aviso de copyright y la licencia).
PERMISSIVE = {
    "MIT", "MIT-0", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause",
    "ISC", "Zlib", "0BSD", "Unlicense", "CC0-1.0", "BSL-1.0", "Unicode-3.0", "BlueOak-1.0.0",
    # Runtime de MinGW-w64 (solo en Windows con MinGW).
    "ZPL-2.1", "LicenseRef-public-domain",
}
# Compatibles con condiciones concretas (ver CONDITIONS).
CONDITIONAL = {"MPL-2.0", "OFL-1.1", "LGPL-2.1-or-later", "GPL-3.0-or-later WITH GCC-exception-3.1"}
CONDITIONS = {
    "MPL-2.0": "copyleft por archivo: se conservan sus avisos y su código fuente está disponible "
    "(crates.io y el .tar.gz con Cargo.lock). Ningún archivo lleva el aviso «Incompatible With "
    "Secondary Licenses», así que la cláusula 3.3 permite combinarlo con GPL-3.0-or-later.",
    "OFL-1.1": "tipografías: se distribuyen con el programa conservando su licencia y sus avisos; "
    "no se venden por separado ni se modifican (no se usa su nombre reservado en versiones modificadas).",
    "LGPL-2.1-or-later": "biblioteca: se conserva su aviso y se puede sustituir.",
    "GPL-3.0-or-later WITH GCC-exception-3.1": "libgcc (solo compilación con MinGW): la excepción de "
    "bibliotecas de ejecución de GCC permite distribuir el programa compilado con cualquier licencia; "
    "se conserva su aviso.",
}


def audit_class(eco, origin, lic):
    """Clase de auditoría: la de la licencia, salvo los binarios de Windows sin
    código fuente publicado, que quedan pendientes (WebView2Loader.dll)."""
    c, conds = classify(lic)
    if eco == "Windows" and origin == "estándar":
        return "pendiente", conds
    return c, conds


def norm_expr(expr):
    """Normaliza las formas antiguas («MIT/Apache-2.0») a SPDX («MIT OR Apache-2.0»)."""
    e = (expr or "").strip()
    e = re.sub(r"\s*/\s*", " OR ", e)
    return e


def classify(expr):
    """Clasificación frente a GPL-3.0-or-later de una expresión SPDX sencilla.

    OR: basta una opción compatible (se elige esa). AND: todas deben serlo.
    Devuelve (clase, licencias que la condicionan)."""
    e = norm_expr(expr)
    if not e:
        return "indeterminada", []
    e = e.replace("(", " ").replace(")", " ")

    def term(t):
        t = t.strip()
        if t in PERMISSIVE:
            return "compatible", []
        if t in CONDITIONAL:
            return "condicionada", [t]
        return "pendiente", [t]

    def any_of(opts):
        res = [all_of(o.split(" AND ")) for o in opts]
        for want in ("compatible", "condicionada", "pendiente"):
            for r in res:
                if r[0] == want:
                    return r
        return res[0]

    def all_of(parts):
        rs = [term(p) for p in parts]
        order = ["pendiente", "condicionada", "compatible"]
        worst = min(rs, key=lambda r: order.index(r[0]))
        conds = sorted({c for r in rs for c in r[1]})
        return worst[0], conds

    return any_of(re.split(r"\s+OR\s+", e))


def cargo_meta():
    return json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--offline", "--filter-platform", TARGET],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )


def rust_sets(meta):
    """(enlazadas en el programa, solo compilación, solo pruebas) para la plataforma elegida."""
    pkgs = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    ws = set(meta["workspace_members"])
    root = next(p["id"] for p in meta["packages"] if p["name"] == "container-manage-center")

    def reach(starts, follow):
        seen, stack = set(), list(starts)
        while stack:
            nid = stack.pop()
            for dep in nodes[nid]["deps"]:
                kinds = {k["kind"] for k in dep["dep_kinds"]}
                if follow(nid, kinds) and dep["pkg"] not in seen:
                    seen.add(dep["pkg"])
                    stack.append(dep["pkg"])
        return seen

    # Solo dependencias normales desde la aplicación: lo que se enlaza.
    linked = reach([root], lambda _n, k: None in k) - ws
    everything = reach(ws, lambda _n, _k: True) - ws
    # Todo lo alcanzable sin las dependencias de pruebas del propio proyecto.
    no_dev = reach(ws, lambda n, k: not (n in ws and k == {"dev"})) - ws
    build = no_dev - linked
    dev = everything - no_dev
    return pkgs, linked, build, dev


def license_files(directory):
    d = Path(directory)
    if not d.is_dir():
        return []
    out = []
    for f in sorted(d.iterdir(), key=lambda p: p.name.lower()):
        if f.is_file() and LICENSE_FILE.match(f.name) and f.suffix.lower() != ".spdx":
            try:
                out.append((f.name, f.read_text(encoding="utf-8", errors="replace").strip()))
            except OSError:
                pass
    return out


def registry_dir(name, version):
    for base in sorted((Path.home() / ".cargo" / "registry" / "src").glob("*")):
        d = base / f"{name}-{version}"
        if d.is_dir():
            return d
    return None


def standard_texts():
    """Textos estándar para los componentes que no traen archivo de licencia,
    tomados de componentes del propio registro que sí los traen."""
    def first(*cands):
        for crate, fname in cands:
            for base in sorted((Path.home() / ".cargo" / "registry" / "src").glob("*")):
                for d in sorted(base.glob(f"{crate}-*")):
                    f = d / fname
                    if f.is_file():
                        return f.read_text(encoding="utf-8", errors="replace").strip()
        return None

    return {
        "Apache-2.0": first(("serde", "LICENSE-APACHE"), ("tokio", "LICENSE-APACHE")),
        "MPL-2.0": first(("cssparser", "LICENSE")),
        # clipboard-win (Windows) no trae archivo; error-code, del mismo autor, sí.
        "BSL-1.0": first(("error-code", "LICENSE")),
    }


MIT_TEMPLATE = """Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""

BSD3_TEMPLATE = """Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE."""


def fallback_texts(expr, authors, std):
    """Textos para un componente sin archivo de licencia: el estándar de su
    licencia declarada (la primera opción que se pueda reproducir), con el
    aviso de copyright tomado de sus autores declarados."""
    who = ", ".join(authors) if authors else "los autores del componente"
    opts = [o.strip() for o in re.split(r"\s+OR\s+", norm_expr(expr).replace("(", "").replace(")", ""))]
    for o in opts:
        if o == "MIT":
            return [("MIT (texto estándar; el componente no incluye archivo de licencia)", f"Copyright (c) {who}\n\n{MIT_TEMPLATE}")]
        if o == "BSD-3-Clause":
            return [("BSD-3-Clause (texto estándar; el componente no incluye archivo de licencia)", f"Copyright (c) {who}\n\n{BSD3_TEMPLATE}")]
        if o in std and std[o]:
            return [(f"{o} (texto estándar; el componente no incluye archivo de licencia; autores: {who})", std[o])]
    return []


def npm_sets():
    lock = json.loads((ROOT / "package-lock.json").read_text())
    prod, dev = [], []
    for key, v in lock["packages"].items():
        if not key:
            continue
        name = key.split("node_modules/")[-1]
        lic = v.get("license")
        lic = lic if isinstance(lic, str) else (json.dumps(lic) if lic else "")
        item = (name, v.get("version", ""), lic, ROOT / key)
        (dev if v.get("dev") or v.get("devOptional") else prod).append(item)
    return sorted(prod), sorted(dev)


def collect():
    meta = cargo_meta()
    pkgs, linked, build, dev = rust_sets(meta)
    std = standard_texts()
    shipped = []  # (ecosistema, nombre, versión, licencia, [(archivo, texto)], origen del texto)
    for nid in sorted(linked, key=lambda i: (pkgs[i]["name"], pkgs[i]["version"])):
        p = pkgs[nid]
        if p["name"] in OWN:
            continue
        files = license_files(Path(p["manifest_path"]).parent)
        origin = "paquete"
        if not [f for f in files if not f[0].upper().startswith("NOTICE")]:
            files = files + fallback_texts(p.get("license"), p.get("authors") or [], std)
            origin = "estándar"
        shipped.append(("Rust", p["name"], p["version"], p.get("license") or "", files, origin))
    prod, npm_dev = npm_sets()
    tauri_api = license_files(ROOT / "node_modules" / "@tauri-apps" / "api")
    for name, version, lic, d in prod:
        files = license_files(d)
        origin = "paquete"
        if not files and name.startswith("@tauri-apps/"):
            # Los complementos de Tauri solo traen LICENSE.spdx: mismo proyecto
            # y mismos titulares que @tauri-apps/api.
            files = [(f"{n} (de @tauri-apps/api: mismo proyecto y titulares)", t) for n, t in tauri_api]
            origin = "proyecto"
        shipped.append(("npm", name, version, lic, files, origin))
    if WINDOWS:
        for name, version, lic, fname, origin, _note in WINDOWS_EXTRA:
            text = (WIN_LICENSES / fname).read_text(encoding="utf-8", errors="replace").strip()
            label = "BSD-3-Clause (texto estándar con el titular de Microsoft)" if origin == "estándar" else fname
            shipped.append(("Windows", name, version, lic, [(label, text)], origin))
    rust_build = [(pkgs[i]["name"], pkgs[i]["version"], pkgs[i].get("license") or "") for i in build if pkgs[i]["name"] not in OWN]
    rust_dev = [(pkgs[i]["name"], pkgs[i]["version"], pkgs[i].get("license") or "") for i in dev if pkgs[i]["name"] not in OWN]
    total_lock = len([p for p in meta["packages"] if p["name"] not in OWN])
    mpl_flags = {}
    for eco, name, version, lic, _f, _o in shipped:
        if eco == "Rust" and "MPL" in lic:
            d = registry_dir(name, version)
            hits = 0
            if d:
                # El aviso de la MPL (Exhibit B) en un archivo de código; el
                # propio texto de la licencia lo menciona y no cuenta.
                notice = re.compile(r'is\s+"?Incompatible\s+With\s+Secondary\s+Licenses"?\s*,?\s+as\s+defined', re.I)
                for f in d.rglob("*.rs"):
                    try:
                        if notice.search(f.read_text(errors="ignore")):
                            hits += 1
                    except OSError:
                        pass
            mpl_flags[f"{name} {version}"] = hits
    return shipped, sorted(rust_build), sorted(rust_dev), [(n, v, l) for n, v, l, _ in npm_dev], total_lock, mpl_flags


def render_notices(shipped):
    texts = {}  # hash → (título, texto)
    uses = defaultdict(list)
    comp = []
    for eco, name, version, lic, files, origin in shipped:
        refs = []
        for fname, text in files:
            h = hashlib.sha256(text.encode()).hexdigest()[:12]
            if h not in texts:
                texts[h] = (fname, text)
            uses[h].append(f"{name} {version}")
            refs.append(h)
        comp.append((eco, name, version, lic, refs, origin))
    lines = [
        "# Avisos de terceros · Container Manage Center",
        "",
        "Container Manage Center (CMC) incluye los componentes de terceros siguientes. Cada uno",
        "conserva **su propia licencia y sus propios avisos de copyright**; la licencia",
        "GPL-3.0-or-later de CMC se aplica al código propio del proyecto y no cambia la de",
        "estos componentes.",
        "",
        "Este archivo lo genera `scripts/third-party-licenses.py` a partir de `Cargo.lock` y",
        "`package-lock.json`, copiando tal cual los archivos de licencia y aviso de cada",
        "componente (los textos idénticos aparecen una sola vez). No se edita a mano.",
        "",
    ]
    if WINDOWS:
        lines += [
            "Esta es la lista del **instalador de Windows**. Las bibliotecas de Windows (WebView2",
            "Runtime de Microsoft Edge, kernel32, user32, ucrt…) forman parte del sistema y no se",
            "incluyen. WebView2Loader.dll sí se incluye: es un binario de Microsoft (BSD-3-Clause).",
            "El runtime de MinGW-w64 y libgcc solo se incluyen en la compilación con MinGW; la",
            "compilación con MSVC usa el runtime de Visual C++ del sistema.",
            "",
        ]
    else:
        lines += [
            "Las bibliotecas del sistema (WebKitGTK, GTK, glib, libc…) no se incluyen en el paquete:",
            "las instala el sistema como dependencias y conservan sus propios avisos en",
            "`/usr/share/doc/` de cada paquete de Debian.",
            "",
        ]
    lines += [
        f"## Componentes incluidos ({len(comp)})",
        "",
        "| Componente | Versión | Ecosistema | Licencia | Textos |",
        "|---|---|---|---|---|",
    ]
    for eco, name, version, lic, refs, origin in comp:
        mark = {"paquete": "", "estándar": " (estándar)", "sistema": " (del sistema de compilación)"}.get(origin, " (del proyecto)")
        lines.append(f"| {name} | {version} | {eco} | {lic or 'sin indicar'} | {', '.join('T-' + r for r in refs) or '—'}{mark} |")
    lines += ["", "## Textos de licencia y avisos", ""]
    for h, (fname, text) in texts.items():
        users = uses[h]
        shown = ", ".join(users[:12]) + (f" y {len(users) - 12} más" if len(users) > 12 else "")
        lines += [f"### T-{h} · {fname}", "", f"Usado por: {shown}.", "", "```text", text.replace("```", "ʼʼʼ"), "```", ""]
    return "\n".join(lines)


def render_audit(shipped, rust_build, rust_dev, npm_dev, total_lock, mpl_flags):
    cls = Counter()
    by_class = defaultdict(list)
    lic_count = Counter()
    for eco, name, version, lic, files, origin in shipped:
        c, conds = audit_class(eco, origin, lic)
        cls[c] += 1
        by_class[c].append((eco, name, version, lic, conds, origin))
        lic_count[norm_expr(lic) or "sin indicar"] += 1
    if any(mpl_flags.values()):
        # No debería ocurrir: el aviso impide combinarlo con la GPL.
        cls["pendiente"] += sum(1 for v in mpl_flags.values() if v)
    rust_n = sum(1 for s in shipped if s[0] == "Rust")
    win_n = sum(1 for s in shipped if s[0] == "Windows")
    npm_n = len(shipped) - rust_n - win_n
    std_n = [s for s in shipped if s[5] != "paquete"]
    L = [
        f"# Auditoría de licencias de dependencias · Container Manage Center {json.loads((ROOT / 'package.json').read_text())['version']}"
        + (" · Windows" if WINDOWS else ""),
        "",
        "Generada por `scripts/third-party-licenses.py` a partir de `Cargo.lock` y",
        "`package-lock.json` (sin conexión). La licencia del proyecto es **GPL-3.0-or-later**.",
        "Las conclusiones sobre compatibilidad son técnicas, no asesoramiento jurídico.",
        "",
        "## 1. Alcance",
        "",
        "| Ecosistema | Analizados | Se distribuyen en el paquete |",
        "|---|---|---|",
        f"| Rust (`Cargo.lock`, {PLATFORM}) | {total_lock} crates de terceros | **{rust_n}** enlazadas en el programa; {len(rust_build)} solo de compilación y {len(rust_dev)} solo de pruebas no se distribuyen |",
        f"| npm (`package-lock.json`) | {npm_n + len(npm_dev)} paquetes | **{npm_n}** de producción incluidos en la interfaz (con las tipografías); {len(npm_dev)} de desarrollo no se distribuyen |",
        (
            f"| Windows | componentes binarios del instalador | **{win_n}**: WebView2Loader.dll (Microsoft) y, en la compilación con MinGW, su runtime y libgcc. El WebView2 Runtime, el runtime de Visual C++ y las bibliotecas de Windows son del sistema y no se incluyen |"
            if WINDOWS
            else "| Sistema (Debian) | dependencias del paquete | Ninguna: `libwebkit2gtk-4.1-0`, `libgtk-3-0` y `libc6` las instala apt y se enlazan dinámicamente; `openssh-client` es recomendada |"
        ),
        "| Recursos | logotipo e iconos de la aplicación, iconos de la interfaz, tipografías | Logotipo e iconos de la aplicación: propios del proyecto. Iconos de la interfaz: lucide-react (ISC, con partes de Feather, MIT). Tipografías: IBM Plex Sans y JetBrains Mono (OFL-1.1) |",
        (
            "| Herramientas externas | Docker Desktop o Docker Engine, Docker Compose, OpenSSH de Windows, Trivy/Grype (opcionales), Programador de tareas | No se incluyen: se usan las del sistema o del servidor |"
            if WINDOWS
            else "| Herramientas externas | Docker Engine, Docker Compose, OpenSSH, Trivy/Grype (opcionales), systemd | No se incluyen: se usan las del sistema o del servidor |"
        ),
        "",
        "## 2. Resultado (componentes que se distribuyen)",
        "",
        "| Clase | Componentes | Significado |",
        "|---|---|---|",
        f"| 1. Compatibles | {cls['compatible']} | Licencias permisivas compatibles con GPL-3.0-or-later. Obligan a conservar su aviso y su licencia: están completos en `{NOTICES.name}` |",
        f"| 2. Compatibles bajo condiciones | {cls['condicionada']} | {'MPL-2.0, OFL-1.1 y la excepción de GCC' if WINDOWS else 'MPL-2.0 y OFL-1.1'} (condiciones abajo). Apache-2.0 sola es compatible con GPL v3 (no lo sería con GPL v2) y se cuenta en la clase 1 |",
        f"| 3. Requieren avisos o materiales adicionales | {len(std_n)} (también contados en {'otra clase' if WINDOWS else 'la clase 1 o 2'}) | Sin archivo de licencia propio en su paquete: se reproduce el texto estándar de su licencia declarada con sus autores declarados. Ningún componente trae archivo NOTICE de Apache |",
        f"| 4. Pendientes de revisión | {cls['pendiente']} | "
        + ("WebView2Loader.dll: licencia compatible, pero es un binario sin código fuente publicado (ver «Decisión pendiente») |" if WINDOWS else "— |"),
        "| 5. Potencialmente incompatibles | 0 | — |",
        f"| 6. Licencia no determinable | {cls['indeterminada']} | — |",
        "",
        "### Condiciones",
        "",
    ]
    for lic, text in CONDITIONS.items():
        users = [f"{n} {v}" for _e, n, v, _l, conds, _o in by_class["condicionada"] if lic in conds]
        if users:
            L += [f"- **{lic}** ({', '.join(users)}): {text}"]
    L += [
        "",
        "### Comprobaciones hechas a mano",
        "",
        "- **MPL-2.0**: archivos de código (`*.rs`) con el aviso «Incompatible With Secondary Licenses» (Exhibit B): "
        + ", ".join(f"{k}: {v}" for k, v in sorted(mpl_flags.items()))
        + ".",
        "- **ring 0.17.14** (Apache-2.0 AND ISC): sus archivos `LICENSE`, `LICENSE-BoringSSL` y "
        "`LICENSE-other-bits` solo contienen Apache-2.0, ISC y MIT (once_cell); no queda código bajo "
        "la licencia OpenSSL/SSLeay, que sería incompatible con la GPL.",
        "- **lucide-react** (ISC): su `LICENSE` reconoce partes de Feather (MIT, Cole Bemis); el aviso se conserva.",
        "- **Complementos de Tauri** (`@tauri-apps/plugin-*`): solo traen `LICENSE.spdx` (MIT OR Apache-2.0); "
        "se reproducen los textos de `@tauri-apps/api`, del mismo proyecto y titulares.",
        "- **Bibliotecas del sistema**: no se redistribuyen; se enlazan dinámicamente "
        + ("(Windows, WebView2 Runtime)." if WINDOWS else "desde Debian."),
        "",
    ]
    if WINDOWS:
        L += [
            "### Decisión pendiente del propietario: WebView2Loader.dll",
            "",
            "- Tauri usa WebView2 en Windows a través de `webview2-com-sys`, que trae `WebView2Loader.dll` "
            "(MinGW: se instala junto al programa) y `WebView2LoaderStatic.lib` (MSVC: queda dentro del programa). "
            "Son binarios de Microsoft bajo BSD-3-Clause, licencia compatible con la GPL, **pero Microsoft no "
            "publica su código fuente** (Eclipse IP Lab, incidencia 14072).",
            "- La GPL v3 exige ofrecer el «código fuente correspondiente» del programa distribuido. El cargador "
            "no es una «biblioteca del sistema» en el sentido de la sección 1 de la GPL v3 en todos los casos "
            "(se distribuye con el programa), así que no está claro que la excepción de bibliotecas del sistema lo cubra.",
            "- Opciones: (a) que el titular del copyright de CMC añada un **permiso adicional** (GPL v3, sección 7) "
            "que permita enlazar con el cargador de WebView2 de Microsoft, como hacen otros proyectos GPL con "
            "componentes no libres; (b) consultar a un asesor jurídico; (c) no distribuir la versión de Windows.",
            "- **Hasta que se decida, los instaladores de Windows son solo para pruebas y no se publican ni se "
            "redistribuyen.**",
            "- El texto de la licencia es el BSD-3-Clause estándar con el titular «Microsoft Corporation», como "
            "indica el paquete NuGet Microsoft.Web.WebView2; el `LICENSE.txt` original del paquete no se ha podido "
            "descargar en este entorno (nuget.org bloqueado). La compilación de GitHub Actions lo descarga y lo "
            "compara.",
            "- **Runtime de MinGW-w64 y libgcc**: partes enlazadas estáticamente al compilar con MinGW. libgcc "
            "tiene la excepción de bibliotecas de ejecución de GCC 3.1, que permite distribuir el programa con "
            "cualquier licencia; el runtime de MinGW-w64 es ZPL-2.1 y MIT, con partes en dominio público.",
            "",
        ]
    L += [
        "### Componentes de la clase 3 (texto estándar)",
        "",
    ]
    L += [f"- {name} {version} ({lic})" for _e, name, version, lic, _f, _o in std_n] or ["- Ninguno."]
    L += [
        "",
        "## 3. Licencias de los componentes distribuidos",
        "",
        "| Expresión de licencia | Componentes |",
        "|---|---|",
    ]
    L += [f"| {k} | {v} |" for k, v in sorted(lic_count.items(), key=lambda x: (-x[1], x[0]))]
    L += [
        "",
        "## 4. Herramientas y límites",
        "",
        f"- `cargo metadata --offline` (grafo de dependencias para {PLATFORM}) y `cargo deny check licenses` "
        "(política de `deny.toml`, que forma parte de la verificación).",
        "- Lectura de los archivos `LICENSE*`, `COPYING*` y `NOTICE*` de cada componente en el registro "
        "local de Cargo y en `node_modules`.",
        "- No se ha usado `cargo-about` ni herramientas de SBOM (no instaladas; decisión del propietario).",
        "- La licencia de cada crate es la que declara su autor en crates.io; no se ha revisado archivo por "
        "archivo salvo en las comprobaciones de arriba.",
        "- Las conclusiones son técnicas; no sustituyen a un asesoramiento jurídico.",
        "",
        "## 5. Inventario: se distribuyen",
        "",
        "| Componente | Versión | Ecosistema | Licencia | Clase |",
        "|---|---|---|---|---|",
    ]
    for eco, name, version, lic, _f, origin in shipped:
        c, conds = audit_class(eco, origin, lic)
        tag = {"compatible": "1", "condicionada": "2", "pendiente": "4", "indeterminada": "6"}[c]
        if origin != "paquete":
            tag += " · 3"
        L.append(f"| {name} | {version} | {eco} | {lic or 'sin indicar'} | {tag}{' (' + ', '.join(conds) + ')' if conds else ''} |")
    L += ["", "## 6. Inventario: no se distribuyen (compilación, pruebas y desarrollo)", "", "| Componente | Versión | Uso | Licencia |", "|---|---|---|---|"]
    L += [f"| {n} | {v} | Rust · compilación | {l} |" for n, v, l in rust_build]
    L += [f"| {n} | {v} | Rust · pruebas | {l} |" for n, v, l in rust_dev]
    L += [f"| {n} | {v} | npm · desarrollo | {l or 'sin indicar'} |" for n, v, l in npm_dev]
    L.append("")
    return "\n".join(L)


def main():
    shipped, rust_build, rust_dev, npm_dev, total_lock, mpl = collect()
    missing = [f"{n} {v}" for _e, n, v, _l, files, _o in shipped if not files]
    if missing:
        print("Componentes distribuidos sin ningún texto de licencia:", ", ".join(missing))
        return 1
    out = {NOTICES: render_notices(shipped), AUDIT: render_audit(shipped, rust_build, rust_dev, npm_dev, total_lock, mpl)}
    if "--check" in sys.argv:
        bad = [p for p, t in out.items() if not p.exists() or p.read_text() != t]
        for p in bad:
            print(f"{p.relative_to(ROOT)} no está al día: ejecuta python3 scripts/third-party-licenses.py")
        if not bad:
            print("Avisos de terceros y auditoría de licencias al día")
        return 1 if bad else 0
    LEGAL.mkdir(parents=True, exist_ok=True)
    for p, t in out.items():
        p.write_text(t)
        print(f"Escrito {p.relative_to(ROOT)} ({len(t) // 1024} KiB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
