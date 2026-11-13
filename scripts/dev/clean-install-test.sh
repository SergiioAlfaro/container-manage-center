#!/usr/bin/env bash
# Prueba de instalación limpia (Fase 10). SOLO para una máquina de pruebas
# desechable: necesita root, debootstrap y acceso a archive.ubuntu.com, y monta
# /proc, /sys, /dev y el socket de Docker dentro de sistemas Ubuntu recién
# creados. No se ejecuta en tu equipo de trabajo (verify-phase-10.sh no lo usa).
#
# Uso: sudo bash scripts/dev/clean-install-test.sh RELEASE_DIR [noble|jammy ...]
#   RELEASE_DIR contiene install.sh y los paquetes .deb (normal y compatible).
#
# Para cada versión de Ubuntu: crea el sistema mínimo con debootstrap, crea un
# usuario normal «tester» (sudo solo para apt-get), ejecuta install.sh como
# ese usuario (elige el paquete por la glibc), comprueba los archivos
# instalados, arranca la aplicación con ese usuario en el DISPLAY indicado
# (por defecto :99) y la desinstala con install.sh --uninstall. Deja el
# registro en /tmp/dcc-clean-install-<versión>.log.
set -u
REL=${1:?Uso: $0 RELEASE_DIR [noble|jammy ...]}; shift
REL=$(realpath "$REL")
[ "$(id -u)" -eq 0 ] || { echo "Necesita root (solo en una máquina de pruebas)."; exit 1; }
command -v debootstrap >/dev/null || { echo "Falta debootstrap."; exit 1; }
[ -f "$REL/install.sh" ] || { echo "Falta $REL/install.sh"; exit 1; }
RELEASES=("$@"); [ ${#RELEASES[@]} -gt 0 ] || RELEASES=(noble jammy)
DISP=${DISPLAY:-:99}
FAIL=0
pass() { echo "  [OK]    $*"; }
bad()  { echo "  [FALLO] $*"; FAIL=$((FAIL+1)); }

for r in "${RELEASES[@]}"; do
  D=$(mktemp -d "/tmp/dcc-clean-$r.XXXX"); LOG=/tmp/dcc-clean-install-$r.log
  # mktemp crea la carpeta con 0700: sería la raíz del sistema y el usuario no podría entrar.
  chmod 755 "$D"
  echo "== Ubuntu $r en $D (registro: $LOG)"
  mounts=()
  cleanup() { for m in "${mounts[@]}"; do umount "$D/$m" 2>/dev/null; done; }
  if ! debootstrap --variant=minbase "$r" "$D" http://archive.ubuntu.com/ubuntu >"$LOG" 2>&1; then bad "debootstrap $r"; continue; fi
  printf 'deb http://archive.ubuntu.com/ubuntu %s main universe\ndeb http://archive.ubuntu.com/ubuntu %s-updates main universe\ndeb http://archive.ubuntu.com/ubuntu %s-security main universe\n' "$r" "$r" "$r" >"$D/etc/apt/sources.list"
  cp /etc/resolv.conf "$D/etc/resolv.conf"
  for m in proc sys dev dev/pts; do mount --bind "/$m" "$D/$m" && mounts=("$m" "${mounts[@]}"); done
  chroot "$D" bash -c 'export DEBIAN_FRONTEND=noninteractive; apt-get update -q && apt-get install -y -q sudo && useradd -m -s /bin/bash tester && echo "tester ALL=(ALL) NOPASSWD: /usr/bin/apt-get" > /etc/sudoers.d/tester && chmod 440 /etc/sudoers.d/tester' >>"$LOG" 2>&1 || { bad "preparar $r"; cleanup; continue; }
  mkdir -p "$D/home/tester/release"; cp "$REL"/install.sh "$REL"/*.deb "$D/home/tester/release/"; [ -f "$REL/SHA256SUMS" ] && cp "$REL/SHA256SUMS" "$D/home/tester/release/"; chroot "$D" chown -R tester: /home/tester/release
  if chroot "$D" su - tester -c "bash ~/release/install.sh --yes" >>"$LOG" 2>&1; then pass "install.sh como usuario normal"; else bad "install.sh"; fi
  grep -E "Paquete elegido" "$LOG" | tail -1 | sed 's/^ */  /'
  V=$(chroot "$D" dpkg-query -W -f='${Version}' docker-control-center 2>/dev/null)
  [ -n "$V" ] && pass "Paquete instalado: $V" || bad "Paquete no instalado"
  for f in /usr/bin/docker-control-center /usr/share/applications/docker-control-center.desktop /usr/share/icons/hicolor/256x256/apps/docker-control-center.png /usr/share/doc/docker-control-center/copyright; do
    [ -e "$D$f" ] && pass "$f" || bad "falta $f"
  done
  if [ -S /run/docker.sock ]; then
    touch "$D/run/docker.sock" && mount --bind /run/docker.sock "$D/run/docker.sock" && mounts=("run/docker.sock" "${mounts[@]}")
    G=$(stat -c %g /run/docker.sock)
    chroot "$D" bash -c "getent group $G >/dev/null || groupadd -g $G dockerhost; usermod -aG \$(getent group $G | cut -d: -f1) tester"
  fi
  if [ -d /tmp/.X11-unix ]; then
    mkdir -p "$D/tmp/.X11-unix"; mount --bind /tmp/.X11-unix "$D/tmp/.X11-unix" && mounts=("tmp/.X11-unix" "${mounts[@]}")
    chroot "$D" su - tester -c "DISPLAY=$DISP timeout 20 docker-control-center" >>"$LOG" 2>&1; rc=$?
    # timeout devuelve 124 si la aplicación seguía en marcha a los 20 s.
    [ $rc -eq 124 ] && pass "La aplicación arranca con el usuario normal y sigue en marcha" || bad "La aplicación terminó (código $rc)"
  fi
  if chroot "$D" su - tester -c "bash ~/release/install.sh --uninstall --yes" >>"$LOG" 2>&1 \
     && ! chroot "$D" dpkg-query -W docker-control-center >/dev/null 2>&1 && [ ! -e "$D/usr/bin/docker-control-center" ]; then
    pass "Desinstalación completa"
  else
    bad "Desinstalación"
  fi
  cleanup
  rm -rf "$D"
done
echo "Fallos: $FAIL"
[ $FAIL -eq 0 ]
