#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
#
# Prueba de instalación limpia y de actualización (Fase 10; 1.7.0, D-110).
# SOLO para una máquina de pruebas desechable: necesita root, debootstrap y
# acceso a archive.ubuntu.com, y monta /proc, /sys, /dev y el socket de Docker
# dentro de sistemas Ubuntu recién creados. No se ejecuta en tu equipo de
# trabajo.
#
# Uso: sudo bash scripts/dev/clean-install-test.sh RELEASE_DIR [noble|jammy ...]
#   RELEASE_DIR contiene install.sh y los paquetes .deb (normal y compatible).
#   UPGRADE_FROM=DIR (opcional): carpeta de una publicación anterior
#   (docker-control-center 1.6.0, con su install.sh y sus .deb). Entonces,
#   antes de instalar la nueva, se instala la anterior, se crean datos de
#   usuario con ella y, tras actualizar, se comprueba que nada se pierde.
#
# Para cada versión de Ubuntu: crea el sistema mínimo con debootstrap, crea un
# usuario normal «tester» (sudo solo para apt-get), ejecuta install.sh como
# ese usuario (elige el paquete por la glibc), comprueba los archivos
# instalados, arranca la aplicación con ese usuario en el DISPLAY indicado
# (por defecto :99) y la desinstala con install.sh --uninstall, comprobando
# que los datos del usuario se conservan. Deja el registro en
# /tmp/dcc-clean-install-<versión>.log.
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
LEGACY=docker-control-center
pass() { echo "  [OK]    $*"; }
bad()  { echo "  [FALLO] $*"; FAIL=$((FAIL+1)); }

# Datos de usuario que no se pueden perder (rutas relativas a /home/tester).
DATA=(.config/docker-control-center/servers.toml
      .config/docker-control-center/known_hosts
      .config/docker-control-center/copias.json
      .config/docker-control-center/monitor.json
      .local/state/docker-control-center/actividad.jsonl
      .config/systemd/user/dcc-backup-job-0a1b2c3d.service
      Copias/docker-control-center-copias/local/20261110T030000-0a1b/manifest.json)

for r in "${RELEASES[@]}"; do
  D=$(mktemp -d "/tmp/dcc-clean-$r.XXXX"); LOG=/tmp/dcc-clean-install-$r.log
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
    if as_tester "bash ~/anterior/install.sh --yes"; then pass "Instalada la versión anterior ($(chroot "$D" dpkg-query -W -f='${Package} ${Version}' $LEGACY 2>/dev/null))"; else bad "instalar la versión anterior"; fi
    # La versión anterior crea su configuración al arrancar.
    [ $X -eq 1 ] && as_tester "DISPLAY=$DISP timeout 15 $LEGACY"
    as_tester 'mkdir -p ~/.config/docker-control-center ~/.local/state/docker-control-center ~/.config/systemd/user ~/Copias/docker-control-center-copias/local/20261110T030000-0a1b
      printf "[[servers]]\nid = \"srv-prueba\"\nname = \"Servidor de prueba\"\n" > ~/.config/docker-control-center/servers.toml
      printf "srv.example ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPrueba\n" > ~/.config/docker-control-center/known_hosts
      printf "{\"version\":1,\"destination\":\"/home/tester/Copias\",\"jobs\":[]}\n" > ~/.config/docker-control-center/copias.json
      printf "{\"intervalSecs\":5,\"servers\":[\"local\"]}\n" > ~/.config/docker-control-center/monitor.json
      printf "{\"op\":\"prueba\"}\n" > ~/.local/state/docker-control-center/actividad.jsonl
      printf "# dcc-managed-backup-job: job-0a1b2c3d\n[Service]\nExecStart=/usr/bin/docker-control-center --run-backup job-0a1b2c3d\n" > ~/.config/systemd/user/dcc-backup-job-0a1b2c3d.service
      printf "{\"format\":1,\"id\":\"20261110T030000-0a1b\"}\n" > ~/Copias/docker-control-center-copias/local/20261110T030000-0a1b/manifest.json'
    (cd "$D/home/tester" && sha256sum "${DATA[@]}") > "$D/tmp/datos.sha256" 2>>"$LOG" || bad "crear los datos de prueba"
  fi

  # -------------------------------------------------------- Versión nueva
  mkdir -p "$D/home/tester/release"; cp "$REL"/install.sh "$REL"/*.deb "$D/home/tester/release/"; [ -f "$REL/SHA256SUMS" ] && cp "$REL/SHA256SUMS" "$D/home/tester/release/"; chroot "$D" chown -R tester: /home/tester/release
  if as_tester "bash ~/release/install.sh --yes"; then pass "install.sh como usuario normal"; else bad "install.sh"; fi
  grep -E "Paquete elegido|apt quitará" "$LOG" | tail -2 | sed 's/^ */  /'
  V=$(chroot "$D" dpkg-query -W -f='${Version}' $PKG 2>/dev/null)
  [ -n "$V" ] && pass "Paquete instalado: $PKG $V" || bad "Paquete no instalado"
  if chroot "$D" dpkg-query -W -f='${Status}' $LEGACY 2>/dev/null | grep -q "install ok installed"; then
    bad "Sigue instalado el paquete anterior $LEGACY (instalación duplicada)"
  else
    pass "Sin instalación duplicada: $LEGACY no está instalado"
  fi
  for f in /usr/bin/$PKG /usr/bin/$LEGACY /usr/share/applications/$PKG.desktop /usr/share/icons/hicolor/256x256/apps/$PKG.png \
           /usr/share/doc/$PKG/copyright /usr/share/doc/$PKG/LICENSE /usr/share/doc/$PKG/THIRD_PARTY_NOTICES.md /usr/share/doc/$PKG/MIGRATION.md; do
    [ -e "$D$f" ] && pass "$f" || bad "falta $f"
  done
  [ -x "$D/usr/bin/$LEGACY" ] && pass "/usr/bin/$LEGACY es ejecutable (copias programadas anteriores)" || bad "/usr/bin/$LEGACY no es ejecutable"
  n=0; for f in "$D/usr/share/applications/$PKG.desktop" "$D/usr/share/applications/$LEGACY.desktop"; do [ -e "$f" ] && n=$((n+1)); done
  [ "$n" -eq 1 ] && pass "Un solo lanzador en el menú" || bad "$n lanzadores en el menú"
  grep -q "^Name=Container Manage Center$" "$D/usr/share/applications/$PKG.desktop" && pass "El lanzador se llama «Container Manage Center»" || bad "Nombre del lanzador"
  # El nombre anterior llama al programa nuevo (lo usan las unidades de systemd ya creadas).
  out=$(chroot "$D" su - tester -c "$LEGACY --run-backup job-no-existe" 2>&1)
  case "$out" in *"Container Manage Center"*) pass "«$LEGACY --run-backup» ejecuta Container Manage Center" ;; *) bad "«$LEGACY --run-backup»: $out" ;; esac
  if [ -n "$OLD" ]; then
    if (cd "$D/home/tester" && sha256sum -c --quiet "$D/tmp/datos.sha256") >>"$LOG" 2>&1; then
      pass "Datos del usuario intactos tras actualizar (${#DATA[@]} archivos: servidores, huellas SSH, copias, tareas, historial)"
    else
      bad "Datos del usuario cambiados o perdidos al actualizar"
    fi
  fi
  if [ $X -eq 1 ]; then
    as_tester "DISPLAY=$DISP timeout 20 $PKG"; rc=$?
    # timeout devuelve 124 si la aplicación seguía en marcha a los 20 s.
    [ $rc -eq 124 ] && pass "La aplicación arranca con el usuario normal y sigue en marcha" || bad "La aplicación terminó (código $rc)"
    if [ -n "$OLD" ]; then
      grep -q "srv-prueba" "$D/home/tester/.config/docker-control-center/servers.toml" && pass "Tras abrirla, los servidores siguen ahí" || bad "Servidores perdidos tras abrir la aplicación"
    fi
  fi
  # Como root no arranca, tampoco por la ruta anterior (DCC-SEC-11).
  out=$(chroot "$D" env DCC_ALLOW_ROOT=1 timeout 20 /usr/bin/$LEGACY 2>&1); rc=$?
  if [ $rc -eq 1 ] && printf '%s' "$out" | grep -q "no debe ejecutarse como root"; then pass "Por la ruta anterior tampoco arranca como root"; else bad "Como root por /usr/bin/$LEGACY (código $rc): $out"; fi
  if [ -n "$OLD" ]; then
    # Vuelta atrás: apt cambia la versión nueva por la anterior, que encuentra los datos.
    # El paquete anterior que sirve para la glibc de este sistema (el compatible en 22.04).
    G=$(chroot "$D" ldd --version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+$')
    OLDDEB=$(ls "$OLD"/"${LEGACY}"_*_amd64.deb | head -1)
    if [ "$(printf '%s\n2.39\n' "$G" | sort -V | head -1)" != "2.39" ]; then OLDDEB=$(ls "$OLD"/"${LEGACY}"_*_amd64-compat-*.deb | head -1); fi
    if as_tester "cd ~/anterior && sudo apt-get install -y -q --allow-downgrades ./$(basename "$OLDDEB")" \
       && chroot "$D" dpkg-query -W -f='${Status}' $LEGACY 2>/dev/null | grep -q "install ok installed" \
       && ! chroot "$D" dpkg-query -W -f='${Status}' $PKG 2>/dev/null | grep -q "install ok installed"; then
      pass "Vuelta atrás a $(chroot "$D" dpkg-query -W -f='${Package} ${Version}' $LEGACY): apt quita $PKG y pone la anterior"
    else
      bad "Vuelta atrás a la versión anterior"
    fi
    if grep -q "srv-prueba" "$D/home/tester/.config/docker-control-center/servers.toml" && [ -f "$D/home/tester/.config/systemd/user/dcc-backup-job-0a1b2c3d.service" ]; then
      pass "Tras volver atrás, los datos siguen donde estaban"
    else
      bad "Datos perdidos al volver atrás"
    fi
    if as_tester "bash ~/release/install.sh --yes" && chroot "$D" dpkg-query -W -f='${Status}' $PKG 2>/dev/null | grep -q "install ok installed"; then
      pass "De nuevo a $PKG $V"
    else
      bad "Reinstalar $PKG tras volver atrás"
    fi
  fi
  if as_tester "bash ~/release/install.sh --uninstall --yes" \
     && ! chroot "$D" dpkg-query -W -f='${Status}' $PKG 2>/dev/null | grep -q "install ok installed" \
     && [ ! -e "$D/usr/bin/$PKG" ] && [ ! -e "$D/usr/bin/$LEGACY" ] && [ ! -e "$D/usr/share/applications/$PKG.desktop" ]; then
    pass "Desinstalación completa"
  else
    bad "Desinstalación"
  fi
  if [ -n "$OLD" ]; then
    (cd "$D/home/tester" && ls "${DATA[@]}") >>"$LOG" 2>&1 && pass "Tras desinstalar, los datos del usuario se conservan" || bad "Datos del usuario borrados al desinstalar"
  else
    [ -d "$D/home/tester/.config/docker-control-center" ] && pass "Tras desinstalar, la configuración del usuario se conserva" || bad "Configuración borrada al desinstalar"
  fi
  cleanup
  rm -rf "$D"
done
echo "Fallos: $FAIL"
[ $FAIL -eq 0 ]
