#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
#
# Prueba de instalación limpia y de actualización (Fase 10; 1.8.0, D-113).
# SOLO para una máquina de pruebas desechable: necesita root, debootstrap y
# acceso a archive.ubuntu.com, y monta /proc, /sys, /dev y el socket de Docker
# dentro de sistemas Ubuntu recién creados. No se ejecuta en tu equipo de
# trabajo.
#
# Uso: sudo bash scripts/dev/clean-install-test.sh RELEASE_DIR [noble|jammy ...]
#   RELEASE_DIR contiene install.sh y los paquetes .deb (normal y compatible).
#   UPGRADE_FROM=DIR (opcional): carpeta de una publicación anterior a la
#   1.8.0 (con su install.sh y sus .deb). Entonces, antes de instalar la nueva,
#   se instala la anterior, se crean datos de usuario en las rutas del nombre
#   anterior y, tras actualizar, se comprueba que están en las rutas nuevas
#   con el mismo contenido y que no queda nada con el nombre anterior.
#
# Para cada versión de Ubuntu: crea el sistema mínimo con debootstrap, crea un
# usuario normal «tester» (sudo solo para apt-get), ejecuta install.sh como
# ese usuario (elige el paquete por la glibc), comprueba los archivos
# instalados, arranca la aplicación con ese usuario en el DISPLAY indicado
# (por defecto :99) y la desinstala con install.sh --uninstall, comprobando
# que los datos del usuario se conservan. Deja el registro en
# /tmp/cmc-clean-install-<versión>.log.
set -u
REL=${1:?Uso: $0 RELEASE_DIR [noble|jammy ...]}; shift
REL=$(realpath "$REL")
OLD=${UPGRADE_FROM:-}
[ -n "$OLD" ] && OLD=$(realpath "$OLD")
[ "$(id -u)" -eq 0 ] || { echo "Necesita root (solo en una máquina de pruebas)."; exit 1; }
command -v debootstrap >/dev/null || { echo "Falta debootstrap."; exit 1; }
[ -f "$REL/install.sh" ] || { echo "Falta $REL/install.sh"; exit 1; }
[ -z "$OLD" ] || [ -f "$OLD/install.sh" ] || { echo "Falta $OLD/install.sh"; exit 1; }
RELEASES=("$@"); [ ${#RELEASES[@]} -gt 0 ] || RELEASES=(noble jammy)
DISP=${DISPLAY:-:99}
FAIL=0
PKG=container-manage-center
# Nombre anterior (hasta la 1.7.0): rutas y paquete de los datos de prueba.
OLDN=docker-control-center
pass() { echo "  [OK]    $*"; }
bad()  { echo "  [FALLO] $*"; FAIL=$((FAIL+1)); }

# Datos de usuario que no se pueden perder: rutas con el nombre anterior y
# dónde deben quedar (relativas a /home/tester).
OLDDATA=(".config/$OLDN/servers.toml" ".config/$OLDN/known_hosts" ".config/$OLDN/copias.json"
         ".config/$OLDN/monitor.json" ".local/state/$OLDN/actividad.jsonl"
         "Copias/$OLDN-copias/local/20261110T030000-0a1b/manifest.json")
NEWDATA=(".config/$PKG/servers.toml" ".config/$PKG/known_hosts" ".config/$PKG/copias.json"
         ".config/$PKG/monitor.json" ".local/state/$PKG/actividad.jsonl"
         "Copias/$PKG-copias/local/20261110T030000-0a1b/manifest.json")
OLDUNIT=.config/systemd/user/dcc-backup-job-0a1b2c3d.service

for r in "${RELEASES[@]}"; do
  D=$(mktemp -d "/tmp/cmc-clean-$r.XXXX"); LOG=/tmp/cmc-clean-install-$r.log
  # mktemp crea la carpeta con 0700: sería la raíz del sistema y el usuario no podría entrar.
  chmod 755 "$D"
  echo "== Ubuntu $r en $D (registro: $LOG)${OLD:+ · actualización desde $(basename "$OLD")}"
  mounts=()
  cleanup() { for m in "${mounts[@]}"; do umount "$D/$m" 2>/dev/null; done; }
  as_tester() { chroot "$D" su - tester -c "$1" >>"$LOG" 2>&1; }
  if ! debootstrap --variant=minbase "$r" "$D" http://archive.ubuntu.com/ubuntu >"$LOG" 2>&1; then bad "debootstrap $r"; continue; fi
  printf 'deb http://archive.ubuntu.com/ubuntu %s main universe\ndeb http://archive.ubuntu.com/ubuntu %s-updates main universe\ndeb http://archive.ubuntu.com/ubuntu %s-security main universe\n' "$r" "$r" "$r" >"$D/etc/apt/sources.list"
  cp /etc/resolv.conf "$D/etc/resolv.conf"
  for m in proc sys dev dev/pts; do mount --bind "/$m" "$D/$m" && mounts=("$m" "${mounts[@]}"); done
  chroot "$D" bash -c 'export DEBIAN_FRONTEND=noninteractive; apt-get update -q && apt-get install -y -q sudo && useradd -m -s /bin/bash tester && echo "tester ALL=(ALL) NOPASSWD: /usr/bin/apt-get" > /etc/sudoers.d/tester && chmod 440 /etc/sudoers.d/tester' >>"$LOG" 2>&1 || { bad "preparar $r"; cleanup; continue; }
  if [ -S /run/docker.sock ]; then
    touch "$D/run/docker.sock" && mount --bind /run/docker.sock "$D/run/docker.sock" && mounts=("run/docker.sock" "${mounts[@]}")
    G=$(stat -c %g /run/docker.sock)
    chroot "$D" bash -c "getent group $G >/dev/null || groupadd -g $G dockerhost; usermod -aG \$(getent group $G | cut -d: -f1) tester"
  fi
  X=0
  if [ -d /tmp/.X11-unix ]; then
    mkdir -p "$D/tmp/.X11-unix"; mount --bind /tmp/.X11-unix "$D/tmp/.X11-unix" && mounts=("tmp/.X11-unix" "${mounts[@]}") && X=1
  fi

  # ----------------------------------------------- Versión anterior y datos
  if [ -n "$OLD" ]; then
    mkdir -p "$D/home/tester/anterior"; cp "$OLD"/install.sh "$OLD"/*.deb "$D/home/tester/anterior/"; [ -f "$OLD/SHA256SUMS" ] && cp "$OLD/SHA256SUMS" "$D/home/tester/anterior/"; chroot "$D" chown -R tester: /home/tester/anterior
    if as_tester "bash ~/anterior/install.sh --yes"; then
      pass "Instalada la versión anterior ($(chroot "$D" dpkg-query -W -f='${Package} ${Version} ' $OLDN $PKG 2>/dev/null))"
    else
      bad "instalar la versión anterior"
    fi
    as_tester "mkdir -p ~/.config/$OLDN ~/.local/state/$OLDN ~/.config/systemd/user ~/Copias/$OLDN-copias/local/20261110T030000-0a1b
      printf '[[servers]]\nid = \"srv-prueba\"\nname = \"Servidor de prueba\"\n' > ~/.config/$OLDN/servers.toml
      printf 'srv.example ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPrueba\n' > ~/.config/$OLDN/known_hosts
      printf '{\"version\":1,\"destination\":\"/home/tester/Copias\",\"jobs\":[]}\n' > ~/.config/$OLDN/copias.json
      printf '{\"intervalSecs\":5,\"servers\":[\"local\"]}\n' > ~/.config/$OLDN/monitor.json
      printf '{\"op\":\"prueba\"}\n' > ~/.local/state/$OLDN/actividad.jsonl
      printf '# dcc-managed-backup-job: job-0a1b2c3d\n[Service]\nExecStart=/usr/bin/$OLDN --run-backup job-0a1b2c3d\n' > ~/$OLDUNIT
      printf '{\"format\":1,\"id\":\"20261110T030000-0a1b\"}\n' > ~/Copias/$OLDN-copias/local/20261110T030000-0a1b/manifest.json"
    (cd "$D/home/tester" && sha256sum "${OLDDATA[@]}" | cut -d' ' -f1) > "$D/tmp/datos.sha256" 2>>"$LOG" || bad "crear los datos de prueba"
  fi

  # -------------------------------------------------------- Versión nueva
  mkdir -p "$D/home/tester/release"; cp "$REL"/install.sh "$REL"/*.deb "$D/home/tester/release/"; [ -f "$REL/SHA256SUMS" ] && cp "$REL/SHA256SUMS" "$D/home/tester/release/"; chroot "$D" chown -R tester: /home/tester/release
  if as_tester "bash ~/release/install.sh --yes"; then pass "install.sh como usuario normal"; else bad "install.sh"; fi
  grep -E "Paquete elegido|apt quitará|Datos de versiones anteriores" "$LOG" | tail -3 | sed 's/^ */  /'
  V=$(chroot "$D" dpkg-query -W -f='${Version}' $PKG 2>/dev/null)
  [ -n "$V" ] && pass "Paquete instalado: $PKG $V" || bad "Paquete no instalado"
  if chroot "$D" dpkg-query -W -f='${Status}' $OLDN 2>/dev/null | grep -q "install ok installed"; then
    bad "Sigue instalado el paquete $OLDN (instalación duplicada)"
  else
    pass "Sin instalación duplicada: $OLDN no está instalado"
  fi
  for f in /usr/bin/$PKG /usr/share/applications/$PKG.desktop /usr/share/icons/hicolor/256x256/apps/$PKG.png \
           /usr/share/doc/$PKG/copyright /usr/share/doc/$PKG/LICENSE /usr/share/doc/$PKG/THIRD_PARTY_NOTICES.md /usr/share/doc/$PKG/MIGRATION.md; do
    [ -e "$D$f" ] && pass "$f" || bad "falta $f"
  done
  left=""
  for f in /usr/bin/$OLDN /usr/share/applications/$OLDN.desktop /usr/share/doc/$OLDN; do
    [ -e "$D$f" ] && left="$left $f"
  done
  [ -z "$left" ] && pass "Nada instalado con el nombre anterior" || bad "Quedan archivos con el nombre anterior:$left"
  grep -q "^Name=Container Manage Center$" "$D/usr/share/applications/$PKG.desktop" && pass "El lanzador se llama «Container Manage Center»" || bad "Nombre del lanzador"
  if [ -n "$OLD" ]; then
    # install.sh ya ha ejecutado «container-manage-center --migrate» con el usuario.
    if (cd "$D/home/tester" && sha256sum "${NEWDATA[@]}" 2>>"$LOG" | cut -d' ' -f1 | diff -q - "$D/tmp/datos.sha256") >>"$LOG" 2>&1; then
      pass "Datos del usuario en las rutas nuevas, con el mismo contenido (${#NEWDATA[@]} archivos: servidores, huellas SSH, copias, monitorización, historial)"
    else
      bad "Datos del usuario cambiados o perdidos al pasarlos al nombre nuevo"
    fi
    left=""
    for f in .config/$OLDN .local/state/$OLDN Copias/$OLDN-copias $OLDUNIT; do
      [ -e "$D/home/tester/$f" ] && left="$left ~/$f"
    done
    [ -z "$left" ] && pass "No queda ninguna carpeta ni unidad de systemd con el nombre anterior" || bad "Quedan con el nombre anterior:$left"
    # Repetir la migración no hace nada.
    as_tester "/usr/bin/$PKG --migrate" && pass "«$PKG --migrate» repetido: nada que hacer" || bad "«$PKG --migrate» repetido"
  fi
  if [ $X -eq 1 ]; then
    as_tester "DISPLAY=$DISP timeout 20 $PKG"; rc=$?
    # timeout devuelve 124 si la aplicación seguía en marcha a los 20 s.
    [ $rc -eq 124 ] && pass "La aplicación arranca con el usuario normal y sigue en marcha" || bad "La aplicación terminó (código $rc)"
    if [ -n "$OLD" ]; then
      grep -q "srv-prueba" "$D/home/tester/.config/$PKG/servers.toml" && pass "Tras abrirla, los servidores siguen ahí" || bad "Servidores perdidos tras abrir la aplicación"
    fi
  fi
  # Como root no arranca (CMC-SEC-11).
  out=$(chroot "$D" env CMC_ALLOW_ROOT=1 timeout 20 /usr/bin/$PKG 2>&1); rc=$?
  if [ $rc -eq 1 ] && printf '%s' "$out" | grep -q "no debe ejecutarse como root"; then pass "No arranca como root"; else bad "Como root (código $rc): $out"; fi
  # Datos del usuario que existen antes de desinstalar. La aplicación solo crea
  # ~/.config/<paquete> cuando se guarda algo; en el primer arranque crea
  # ~/.local/state/<paquete>. Se comprueba que lo que había sigue ahí.
  before=()
  for f in .config/$PKG .local/state/$PKG; do
    [ -e "$D/home/tester/$f" ] && before+=("$f")
  done
  if as_tester "bash ~/release/install.sh --uninstall --yes" \
     && ! chroot "$D" dpkg-query -W -f='${Status}' $PKG 2>/dev/null | grep -q "install ok installed" \
     && [ ! -e "$D/usr/bin/$PKG" ] && [ ! -e "$D/usr/share/applications/$PKG.desktop" ]; then
    pass "Desinstalación completa"
  else
    bad "Desinstalación"
  fi
  if [ -n "$OLD" ]; then
    (cd "$D/home/tester" && ls "${NEWDATA[@]}") >>"$LOG" 2>&1 && pass "Tras desinstalar, los datos del usuario se conservan" || bad "Datos del usuario borrados al desinstalar"
  else
    lost=""
    for f in "${before[@]}"; do [ -e "$D/home/tester/$f" ] || lost="$lost ~/$f"; done
    if [ ${#before[@]} -eq 0 ]; then
      bad "La aplicación no creó datos del usuario: no se puede comprobar que se conservan"
    elif [ -z "$lost" ]; then
      pass "Tras desinstalar, los datos del usuario se conservan:$(printf ' ~/%s' "${before[@]}")"
    else
      bad "Datos del usuario borrados al desinstalar:$lost"
    fi
  fi
  cleanup
  rm -rf "$D"
done
echo "Fallos: $FAIL"
[ $FAIL -eq 0 ]
