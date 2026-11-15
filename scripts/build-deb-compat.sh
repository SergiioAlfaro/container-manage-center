#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
#
# Fase 9 (D-059): genera el paquete .deb compatible con Debian 12+ y Ubuntu 22.04+
# de Container Manage Center.
#
# Un binario solo funciona con una glibc igual o más nueva que aquella con la
# que se compiló. Por eso el paquete para distribuir se compila dentro de un
# contenedor de Docker con Ubuntu 22.04 (glibc 2.35), sin tocar tu sistema:
#   1. En tu equipo: la interfaz (npm run build) y la descarga de las
#      dependencias de Rust (cargo fetch) a target/compat/cargo-home.
#   2. En el contenedor, sin red y con tu usuario: la compilación en modo
#      release (cargo build --offline --locked).
#   3. En tu equipo: el empaquetado de Tauri con ese binario.
#   4. En el contenedor: las dependencias reales del paquete con dpkg-shlibdeps
#      contra las bibliotecas de Ubuntu 22.04 (scripts/deb-set-depends.sh).
# Resultado: target/compat/container-manage-center_<versión>_amd64.deb
#
# Requisitos: Docker (tu usuario en el grupo docker), Node.js/npm y Rust
# (cargo) en tu equipo. La primera vez descarga la imagen ubuntu:22.04 y unos
# 700 MB de paquetes de compilación.
#
# Variables opcionales:
#   DCC_BUILD_BASE     imagen base (por defecto ubuntu:22.04)
#   DCC_BUILD_NETWORK  red para "docker build" (p. ej. host, si usas un proxy local)
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
BASE=${DCC_BUILD_BASE:-ubuntu:22.04}
IMAGE=dcc-build-ubuntu2204:$(sha256sum packaging/build/Dockerfile.ubuntu2204 | cut -c1-12)
OUT="$ROOT/target/compat"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
UIDGID="$(id -u):$(id -g)"

say() { printf '\n== %s\n' "$*"; }
need() { command -v "$1" >/dev/null || { echo "Falta $1: $2" >&2; exit 1; }; }
need docker "instala Docker Engine"
need npm "instala Node.js 20 o posterior"
need cargo "instala Rust (rustup)"
docker info >/dev/null 2>&1 || { echo "No hay acceso a Docker (¿tu usuario está en el grupo docker?)." >&2; exit 1; }
[ "$(id -u)" -ne 0 ] || [ "${DCC_BUILD_ALLOW_ROOT:-0}" = 1 ] || { echo "No ejecutes este script como root." >&2; exit 1; }
mkdir -p "$OUT"

say "1/5 Imagen de compilación ($BASE)"
if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  NET=()
  [ -n "${DCC_BUILD_NETWORK:-}" ] && NET=(--network "$DCC_BUILD_NETWORK")
  PROXY=()
  for v in http_proxy https_proxy HTTP_PROXY HTTPS_PROXY no_proxy NO_PROXY; do
    [ -n "${!v:-}" ] && PROXY+=(--build-arg "$v=${!v}")
  done
  docker build "${NET[@]}" "${PROXY[@]}" --build-arg "BASE=$BASE" -t "$IMAGE" -f packaging/build/Dockerfile.ubuntu2204 packaging/build
fi

say "2/5 Interfaz y dependencias de Rust (en tu equipo)"
npm ci --no-audit --no-fund
npm run build
CARGO_HOME="$OUT/cargo-home" cargo fetch --locked

say "3/5 Compilación en Ubuntu 22.04 (sin red)"
docker run --rm --network none --user "$UIDGID" \
  -e HOME=/tmp -e CARGO_HOME=/src/target/compat/cargo-home -e CARGO_TARGET_DIR=/src/target/compat/target \
  -v "$ROOT:/src" -w /src "$IMAGE" \
  cargo-1.91 build --offline --locked --release --bins --features tauri/custom-protocol -p docker-control-center
BIN="$OUT/target/release/docker-control-center"
NEED=$(objdump -T "$BIN" | grep -oE 'GLIBC_[0-9]+\.[0-9]+' | sed 's/GLIBC_//' | sort -Vu | tail -1)
echo "El binario exige glibc $NEED o posterior."
if [ "$(printf '%s\n2.35\n' "$NEED" | sort -V | tail -1)" != "2.35" ]; then
  echo "ERROR: exige una glibc más nueva que la de Ubuntu 22.04 (2.35)." >&2
  exit 1
fi

say "4/5 Paquete .deb (Tauri, con ese binario)"
# Tauri empaqueta target/release/container-manage-center (el ejecutable con su
# nombre de distribución, `mainBinaryName`; Cargo lo compila con el nombre
# interno docker-control-center) en target/release/bundle: lo que hubiera ahí
# (una compilación normal y su paquete) se guarda y se restaura al terminar,
# pase lo que pase.
APPBIN=container-manage-center
SAVE="$OUT/saved-release"
rm -rf "$SAVE"; mkdir -p "$SAVE" target/release
[ -e target/release/$APPBIN ] && mv target/release/$APPBIN "$SAVE/"
[ -d target/release/bundle/deb ] && mv target/release/bundle/deb "$SAVE/deb"
restore() {
  rm -f target/release/$APPBIN
  rm -rf target/release/bundle/deb
  [ -e "$SAVE/$APPBIN" ] && mv "$SAVE/$APPBIN" target/release/
  [ -d "$SAVE/deb" ] && mkdir -p target/release/bundle && mv "$SAVE/deb" target/release/bundle/deb
  rm -rf "$SAVE"
}
trap restore EXIT
cp "$BIN" target/release/$APPBIN
npx tauri bundle --bundles deb
DEB="target/release/bundle/deb/container-manage-center_${VERSION}_amd64.deb"
[ -f "$DEB" ] || { echo "Tauri no ha generado $DEB" >&2; exit 1; }
cp "$DEB" "$OUT/"
DEB="$OUT/container-manage-center_${VERSION}_amd64.deb"

say "5/5 Dependencias reales (dpkg-shlibdeps en Ubuntu 22.04)"
docker run --rm --network none --user "$UIDGID" -v "$ROOT:/src" -w /src "$IMAGE" \
  bash scripts/deb-set-depends.sh "target/compat/$(basename "$DEB")"
# El ejecutable empaquetado (Tauri le añade una marca del tipo de paquete, así
# que no es idéntico byte a byte) no puede exigir una glibc más nueva que 2.35.
CHECK=$(mktemp -d)
dpkg-deb -x "$DEB" "$CHECK"
[ -x "$CHECK/usr/bin/container-manage-center" ] || { echo "El paquete no contiene el ejecutable." >&2; rm -rf "$CHECK"; exit 1; }
PNEED=$(objdump -T "$CHECK/usr/bin/container-manage-center" | grep -oE 'GLIBC_[0-9]+\.[0-9]+' | sed 's/GLIBC_//' | sort -Vu | tail -1)
rm -rf "$CHECK"
if [ "$(printf '%s\n2.35\n' "$PNEED" | sort -V | tail -1)" != "2.35" ]; then
  echo "ERROR: el ejecutable empaquetado exige glibc $PNEED (más nueva que 2.35)." >&2
  exit 1
fi
echo "El ejecutable empaquetado exige glibc $PNEED o posterior."

say "Listo: $DEB"
sha256sum "$DEB"
