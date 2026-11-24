#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
"""Parte I del manual en PDF (1.11.1): el manual de la aplicación.

Convierte ui/src/features/help/docs/es.md (lo que se ve en Configuración ›
Ayuda y documentación) en capítulos de pandoc, sin cambiar el texto: cada
sección «## N. Título {#id}» pasa a ser el capítulo N con el mismo título, así
la numeración del PDF y la de la aplicación coinciden. Las capturas se
insertan tras el primer párrafo de la sección que indica FIGURAS
(id<TAB>imagen<TAB>pie).

Uso: ayuda-a-pdf.py ES.md FIGURAS.tsv > parte1.md
"""
import re
import sys

HEAD = re.compile(r"^## (\d+)\. (.*?) \{#([a-z-]+)\}\s*$")


def main(doc_path, fig_path):
    figs = {}
    for line in open(fig_path, encoding="utf-8"):
        if not line.strip() or line.startswith("#"):
            continue
        sid, img, caption = line.rstrip("\n").split("\t")
        figs.setdefault(sid, []).append((img, caption))

    sections, cur = [], None
    for line in open(doc_path, encoding="utf-8").read().splitlines():
        m = HEAD.match(line)
        if m:
            cur = {"n": int(m.group(1)), "title": m.group(2), "id": m.group(3), "lines": []}
            sections.append(cur)
        elif cur is not None:
            cur["lines"].append(line)

    unknown = set(figs) - {s["id"] for s in sections}
    if unknown:
        sys.exit(f"Figuras para secciones que no existen: {sorted(unknown)}")

    out = ["```{=latex}", "\\part{Manual de usuario}", "```", ""]
    for i, s in enumerate(sections, 1):
        if s["n"] != i:
            sys.exit(f"Numeración rota en la sección {s['id']}: {s['n']} (se esperaba {i})")
        out.append(f"# {s['title']} {{#ayuda-{s['id']}}}")
        body = "\n".join(s["lines"]).strip("\n").split("\n\n")
        pics = [f"![{cap}](imagenes/{img}){{width=88%}}" for img, cap in figs.get(s["id"], [])]
        if pics:
            body = body[:1] + pics + body[1:]
        out.append("")
        out.append("\n\n".join(body))
        out.append("")
    sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
