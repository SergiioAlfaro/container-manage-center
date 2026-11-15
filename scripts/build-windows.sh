#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
#
# Instalador de Windows de PRUEBA compilado desde Linux con MinGW-w64
# (1.9.0, D-114). Solo para probar en un Windows real: la compilación oficial
# es la de GitHub Actions con MSVC (.github/workflows/windows.yml).
#
# Requisitos (Debian/Ubuntu): gcc-mingw-w64-x86-64, nsis (makensis), Node.js,
# Rust estable y el código fuente de la biblioteca estándar de la MISMA
# versión de rustc (rust-src). Si `rustup` puede instalar el destino
# x86_64-pc-windows-gnu, se usa; si no (sin acceso a static.rust-lang.org), se
# compila la biblioteca estándar con -Zbuild-std desde RUST_SRC.
#
# Uso:
#   scripts/build-windows.sh                        # con rustup target
#   RUST_SRC=/ruta/rust/library scripts/build-windows.sh   # con build-std
#
# Variables: CARGO_TARGET_DIR (por defecto target/), OUT (carpeta de salida,
# por defecto target/windows-installer).
#
# AVISO LEGAL: el instalador incluye WebView2Loader.dll (Microsoft,
# BSD-3-Clause, sin código fuente publicado). Hasta que el propietario decida
# cómo cumplir la GPL v3 con ese componente (DEPENDENCY_LICENSE_AUDIT-windows.md),
# el instalador es SOLO PARA PRUEBAS y no se publica ni se redistribuye.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
TARGET=x86_64-pc-windows-gnu
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target}
OUT=${OUT:-$CARGO_TARGET_DIR/windows-installer}
VERSION=$(node -p "require('./package.json').version")

need() { command -v "$1" >/dev/null || { echo "Falta $1 ($2)" >&2; exit 1; }; }
need x86_64-w64-mingw32-gcc "apt install gcc-mingw-w64-x86-64"
need makensis "apt install nsis"
need npm "Node.js"
need cargo "Rust"

# Los avisos de terceros de Windows deben estar al día antes de empaquetarlos.
python3 scripts/third-party-licenses.py --windows --check

export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
EXTRA=()
if [[ -n "${RUST_SRC:-}" ]]; then
    [[ -f "$RUST_SRC/std/Cargo.toml" ]] || { echo "RUST_SRC no contiene std/Cargo.toml: $RUST_SRC" >&2; exit 1; }
    export RUSTC_BOOTSTRAP=1
    export __CARGO_TESTS_ONLY_SRC_ROOT="$RUST_SRC"
    EXTRA=(-- "-Zbuild-std=std,panic_abort")
    # La CLI de Tauri da el destino por no instalado si `rustup target list`
    # no lo marca «(installed)»; con build-std no hace falta. Solo para esa
    # orden, un envoltorio de rustup lo marca; todo lo demás va al rustup real.
    SHIM=$(mktemp -d)
    trap 'rm -rf "$SHIM"' EXIT
    REAL_RUSTUP=$(command -v rustup)
    cat > "$SHIM/rustup" <<EOS
#!/bin/sh
if [ "\$*" = "target list" ]; then
    "$REAL_RUSTUP" target list | sed 's/^$TARGET\$/$TARGET (installed)/'
    exit 0
fi
exec "$REAL_RUSTUP" "\$@"
EOS
    chmod +x "$SHIM/rustup"
    export PATH="$SHIM:$PATH"
elif ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
    rustup target add "$TARGET"
fi

mkdir -p "$CARGO_TARGET_DIR"
df -h "$CARGO_TARGET_DIR" | tail -1
# `tauri build` ejecuta `npm run build` (beforeBuildCommand), compila en
# modo release con el protocolo propio de Tauri y genera el instalador NSIS
# con tauri.windows.conf.json.
npx tauri build --target "$TARGET" --bundles nsis --ci "${EXTRA[@]}"

BUNDLE="$CARGO_TARGET_DIR/$TARGET/release/bundle/nsis"
mkdir -p "$OUT"
for f in "$BUNDLE"/*.exe; do
    name=$(basename "$f" .exe)
    cp "$f" "$OUT/${name}-mingw-SOLO-PRUEBAS.exe"
done
cp "$CARGO_TARGET_DIR/$TARGET/release/container-manage-center.exe" "$OUT/container-manage-center-$VERSION-mingw-SOLO-PRUEBAS-portable.exe"
cp "$CARGO_TARGET_DIR/$TARGET/release/WebView2Loader.dll" "$OUT/" 2>/dev/null || true
(cd "$OUT" && sha256sum -- *.exe *.dll 2>/dev/null > SHA256SUMS)
echo "Instalador de prueba en $OUT (no redistribuir)"
ls -la "$OUT"
