#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
# Instala los paquetes de sistema necesarios para COMPILAR Container Manage Center
# en Debian 12+ / Ubuntu 22.04+. Pide sudo solo para apt.
# Rust y Node.js se instalan aparte (ver INSTALL.md), sin sudo.
set -euo pipefail

if [ "$(id -u)" -eq 0 ]; then
  echo "Ejecuta este script con tu usuario normal; pedirá sudo cuando lo necesite." >&2
  exit 1
fi

PKGS=(build-essential curl wget file pkg-config libssl-dev
      libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev)

echo "Se instalarán: ${PKGS[*]}"
sudo apt-get update
sudo apt-get install -y "${PKGS[@]}"

command -v rustc >/dev/null || echo "Falta Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
command -v node  >/dev/null || echo "Falta Node.js 20 o superior: consulta INSTALL.md"
echo "Listo."
