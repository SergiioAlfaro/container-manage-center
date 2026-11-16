#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
#
# Genera el manual técnico y de usuario en PDF a partir de docs/manual/
# (1.9.1): capítulos en Markdown, diagramas Mermaid y plantilla de XeLaTeX.
#
# Requisitos: pandoc 3, XeLaTeX (fontspec, polyglossia, titlesec, fancyhdr,
# tocloft, tcolorbox, fvextra, lastpage), las fuentes Inter y DejaVu Sans Mono,
# rsvg-convert (logotipo) y, para volver a dibujar los diagramas, Mermaid CLI
# (mmdc). Si falta mmdc, se usan los PDF de los diagramas ya generados.
#
# Uso: bash scripts/build-manual.sh [SALIDA.pdf]
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
SRC=${CMC_MANUAL_SRC:-$ROOT/docs/manual}
VERSION=$(node -p "require('$ROOT/package.json').version" 2>/dev/null || grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)
OUT=${1:-$ROOT/target/manual/Container-Manage-Center-Manual-$VERSION.pdf}
mkdir -p "$(dirname "$OUT")"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

need() { command -v "$1" >/dev/null || { echo "Falta $1" >&2; exit 1; }; }
need pandoc
need xelatex

# Diagramas: .mmd → .pdf vectorial (solo si cambió el origen).
if command -v mmdc >/dev/null; then
  for f in "$SRC"/diagramas/*.mmd; do
    pdf="${f%.mmd}.pdf"
    if [ ! -f "$pdf" ] || [ "$f" -nt "$pdf" ]; then
      mmdc -q -i "$f" -o "$pdf" -c "$SRC/diagramas/mermaid-config.json" --pdfFit -b white
    fi
  done
fi

# Logotipo de la portada.
if [ ! -f "$SRC/imagenes/logo.pdf" ] || [ "$ROOT/packaging/logo.svg" -nt "$SRC/imagenes/logo.pdf" ]; then
  need rsvg-convert
  rsvg-convert -f pdf -o "$SRC/imagenes/logo.pdf" "$ROOT/packaging/logo.svg"
fi

# Fecha de generación (en castellano) y versión en la portada.
MESES=(enero febrero marzo abril mayo junio julio agosto septiembre octubre noviembre diciembre)
read -r Y M D <<<"$(date +'%Y %-m %-d')"
FECHA_ES="$D de ${MESES[$((M - 1))]} de $Y"
sed -e "s/Versión 1\.9\.1/Versión $VERSION/g" "$SRC/plantilla/estilo.tex" > "$WORK/estilo.tex"
{
  printf '\\newcommand{\\fechageneracion}{%s}\n' "$FECHA_ES"
  sed -e "s/Versión 1\.9\.1/Versión $VERSION/g" "$SRC/plantilla/portada.tex"
} > "$WORK/portada.tex"

cd "$SRC"
pandoc capitulos/*.md \
  --from markdown+pipe_tables+implicit_figures+link_attributes \
  --pdf-engine=xelatex \
  --top-level-division=chapter \
  --number-sections \
  --highlight-style=tango \
  --lua-filter="$SRC/plantilla/codigo-partible.lua" \
  --resource-path="$SRC" \
  -V documentclass=report \
  -V papersize=a4 \
  -V geometry:margin=2.2cm \
  -V geometry:headheight=14pt \
  -V lang=es \
  -V colorlinks=true \
  -H "$WORK/estilo.tex" \
  -B "$WORK/portada.tex" \
  -o "$OUT"

echo "Manual: $OUT"
