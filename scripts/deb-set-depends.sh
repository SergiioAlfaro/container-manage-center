#!/usr/bin/env bash
# Fase 9 (D-059): escribe en un paquete .deb sus dependencias reales.
#
# Tauri genera el paquete con "Depends: libwebkit2gtk-4.1-0, libgtk-3-0", sin
# versión de libc6. Un paquete compilado en un sistema con una glibc más nueva
# se instalaba en Debian 12 o Ubuntu 22.04 y no arrancaba ("GLIBC_2.39 not
# found"). Este script calcula las dependencias con dpkg-shlibdeps (la
# herramienta estándar de Debian) a partir del propio binario, contra las
# bibliotecas del sistema en el que se ejecuta, y las escribe en el paquete:
# así apt se niega a instalarlo donde no puede funcionar.
#
# Uso: scripts/deb-set-depends.sh PAQUETE.deb
# Requiere dpkg-dev (dpkg-shlibdeps), que instala build-essential.
set -euo pipefail

DEB=${1:?Uso: $0 PAQUETE.deb}
[ -f "$DEB" ] || { echo "No existe $DEB" >&2; exit 1; }
command -v dpkg-shlibdeps >/dev/null || { echo "Falta dpkg-shlibdeps (paquete dpkg-dev)." >&2; exit 1; }
DEB=$(realpath "$DEB")
W=$(mktemp -d)
trap 'rm -rf "$W"' EXIT

dpkg-deb -R "$DEB" "$W/pkg"
BIN="$W/pkg/usr/bin/docker-control-center"
[ -x "$BIN" ] || { echo "El paquete no contiene usr/bin/docker-control-center" >&2; exit 1; }

mkdir -p "$W/debian"
printf 'Source: docker-control-center\n\nPackage: docker-control-center\nArchitecture: any\n' > "$W/debian/control"
DEPS=$(cd "$W" && dpkg-shlibdeps -O "pkg/usr/bin/docker-control-center" 2>"$W/shlibdeps.err" | sed -n 's/^shlibs:Depends=//p')
if [ -z "$DEPS" ]; then
  echo "dpkg-shlibdeps no ha devuelto dependencias:" >&2
  cat "$W/shlibdeps.err" >&2
  exit 1
fi
case "$DEPS" in
  *libc6*) ;;
  *) echo "Las dependencias calculadas no incluyen libc6: $DEPS" >&2; exit 1 ;;
esac

sed -i "s|^Depends: .*|Depends: $DEPS|" "$W/pkg/DEBIAN/control"
grep -q "^Depends: $DEPS\$" "$W/pkg/DEBIAN/control" || { echo "No se pudo escribir Depends" >&2; exit 1; }
# gzip, como lo genera Tauri: lo lee cualquier dpkg de Debian 12+ y Ubuntu 22.04+.
dpkg-deb --root-owner-group -Zgzip -b "$W/pkg" "$W/out.deb" >/dev/null
mv "$W/out.deb" "$DEB"

# glibc más nueva que exige el binario, para comprobar que Depends la cubre.
NEED=$(objdump -T "$BIN" 2>/dev/null | grep -oE 'GLIBC_[0-9]+\.[0-9]+' | sed 's/GLIBC_//' | sort -Vu | tail -1 || true)
echo "Paquete:   $DEB"
echo "Depends:   $(dpkg-deb -f "$DEB" Depends)"
[ -n "$NEED" ] && echo "El binario exige glibc $NEED o posterior."
