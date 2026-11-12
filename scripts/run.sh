#!/usr/bin/env bash
# Ejecuta Docker Control Center (Fase 10).
#
# Uso: bash scripts/run.sh [--build]
#   Si la aplicación está instalada (paquete .deb), abre /usr/bin/docker-control-center.
#   Si no, abre la compilación de este código fuente (target/release); con --build
#   la compila antes si no existe («npx tauri build --no-bundle»; requisitos en INSTALL.md).
#
# Con tu usuario normal: la aplicación no se ejecuta como root (acceder a Docker ya
# da privilegios equivalentes a root; no hacen falta más). Lo habitual es abrirla
# desde el menú de aplicaciones; este script es para usarla desde el código fuente
# o desde un terminal.
set -u
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
BUILD=0
for a in "$@"; do
  case "$a" in
    --build) BUILD=1 ;;
    -h|--help) sed -n '2,11p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Opción desconocida: $a (usa --build o --help)"; exit 2 ;;
  esac
done
if [ "$(id -u)" -eq 0 ]; then
  echo "No ejecutes Docker Control Center como root: ábrelo con tu usuario normal."
  exit 1
fi
if [ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
  echo "No hay sesión gráfica (DISPLAY/WAYLAND_DISPLAY): abre la aplicación desde tu escritorio."
  exit 1
fi
if [ -x /usr/bin/docker-control-center ]; then
  exec /usr/bin/docker-control-center
fi
BIN="$ROOT/target/release/docker-control-center"
if [ ! -x "$BIN" ] && [ $BUILD -eq 1 ]; then
  echo "Compilando (la primera vez tarda varios minutos)…"
  (cd "$ROOT" && npm ci && npx tauri build --no-bundle) || { echo "La compilación ha fallado: revisa los requisitos en INSTALL.md (Opción B)."; exit 1; }
fi
if [ -x "$BIN" ]; then
  exec "$BIN"
fi
echo "Docker Control Center no está instalado ni compilado."
echo "  Instalar el paquete: bash scripts/install.sh"
echo "  Compilar y abrir:   bash scripts/run.sh --build"
exit 1
