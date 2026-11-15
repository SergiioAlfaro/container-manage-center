#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
# Prepara un sistema Ubuntu 22.04 real (chroot creado con
#   debootstrap --variant=minbase jammy DIR http://archive.ubuntu.com/ubuntu)
# para la prueba real de «Desinstalación y componentes» (0.9.1):
#  - Docker Engine (docker.io) y Compose (docker-compose-v2) instalados ANTES,
#    para comprobar que nunca se ofrecen para quitar;
#  - libglib2.0-0 instalada antes como dependencia automática (componente
#    que ya existía);
#  - el .deb de la aplicación copiado en /root/dcc.deb.
# Uso (como root, en una máquina de pruebas): prepare-uninstall-chroot.sh DIR paquete.deb
set -euo pipefail
dir=${1:?directorio del chroot}
deb=${2:?ruta del .deb}
[[ $(id -u) == 0 ]] || { echo "Hay que ejecutarlo como root (chroot)." >&2; exit 1; }
[[ -x $dir/usr/bin/apt-get ]] || { echo "$dir no es un sistema Debian/Ubuntu." >&2; exit 1; }

cat > "$dir/etc/apt/sources.list" <<'SRC'
deb http://archive.ubuntu.com/ubuntu jammy main universe
deb http://archive.ubuntu.com/ubuntu jammy-updates main universe
deb http://security.ubuntu.com/ubuntu jammy-security main universe
SRC
# Sin servicios dentro del chroot.
printf '#!/bin/sh\nexit 101\n' > "$dir/usr/sbin/policy-rc.d"
chmod 755 "$dir/usr/sbin/policy-rc.d"
for m in proc sys dev; do
  mountpoint -q "$dir/$m" || mount --bind "/$m" "$dir/$m"
done
export DEBIAN_FRONTEND=noninteractive
chroot "$dir" apt-get update -q
chroot "$dir" apt-get install -y -q --no-install-recommends docker.io docker-compose-v2 libglib2.0-bin
cp "$deb" "$dir/root/dcc.deb"
echo "Listo: DCC_TEST_CHROOT=$dir DCC_TEST_DEB=/root/dcc.deb"
