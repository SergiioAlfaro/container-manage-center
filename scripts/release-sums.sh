#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
# Genera SHA256SUMS en la carpeta de una publicación (E2, CMC-SEC-09, D-092).
# Uso: bash scripts/release-sums.sh CARPETA
# Incluye los .deb (el compatible va en la misma carpeta, con el nombre
# …_amd64-compat-debian12-ubuntu2204.deb; también se admite compat/) y los .tar.gz, con rutas relativas
# a la carpeta, que es lo que comprueba install.sh. Después:
#   (cd CARPETA && sha256sum -c SHA256SUMS)
# La firma (gpg --detach-sign SHA256SUMS) necesita la clave del propietario y
# no se hace aquí.
set -euo pipefail
DIR=${1:?Uso: release-sums.sh CARPETA}
cd "$DIR"
shopt -s nullglob
FILES=(*.deb compat/*.deb *.tar.gz)
[ ${#FILES[@]} -gt 0 ] || { echo "No hay paquetes en $DIR" >&2; exit 1; }
sha256sum -- "${FILES[@]}" > SHA256SUMS
sha256sum -c --quiet SHA256SUMS
cat SHA256SUMS
