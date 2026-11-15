#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
#
# Instalación de Container Manage Center (CMC) en Debian, Ubuntu y derivadas.
#
# Uso (con tu usuario normal; pide la contraseña de sudo solo para apt):
#   (Si tienes una versión anterior a la 1.8.0, apt la sustituye y después se
#   pasan tus datos al nombre nuevo sin perder nada: container-manage-center --migrate.)
#   bash install.sh                      # busca el paquete .deb y lo instala
#   bash install.sh --deb RUTA.deb       # instala ese paquete
#   bash install.sh --check              # solo comprueba el sistema y elige el paquete (no instala)
#   bash install.sh --uninstall          # desinstala la aplicación (conserva tus datos)
#   Añade --yes para no tener que confirmar en apt. --no-verify omite la
#   comprobación de SHA256SUMS (no recomendado).
#
# Qué hace:
#   1. Comprueba el sistema: Debian/Ubuntu o derivada, amd64, apt y la versión de glibc.
#   2. Elige el paquete adecuado entre los que encuentra: junto a este script,
#      en dist/ y en las carpetas de compilación (target/release/bundle/deb y
#      target/compat). El paquete normal exige la glibc de Debian 13 / Ubuntu 24.04;
#      el compatible, la de Debian 12 / Ubuntu 22.04. Se usa el más reciente que
#      tu sistema puede ejecutar.
#   2b. Si junto al paquete hay un archivo SHA256SUMS, comprueba que el paquete
#      coincide con su suma; si no coincide o no aparece, no instala nada.
#   3. Lo instala con «sudo apt-get install ./paquete.deb»: apt resuelve las
#      dependencias (WebKitGTK, GTK…) y comprueba que el paquete es para tu sistema.
#   4. Comprueba, sin cambiar nada, lo que la aplicación necesita para trabajar:
#      Docker, tu acceso al socket, Docker Compose, el cliente SSH y un almacén
#      de contraseñas. Si falta algo, explica cómo resolverlo.
#
# Lo que NO hace nunca: instalar Docker, añadirte al grupo docker, cambiar
# permisos del socket, tocar la configuración de SSH ni ejecutar la aplicación
# como root. Todas las órdenes van con sus argumentos separados, sin construir
# líneas de shell con lo que escribes.
set -u

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PKG=container-manage-center
# Paquete de las versiones anteriores a la 1.7.0, con el nombre anterior del
# producto: el nuevo lo sustituye (Conflicts/Replaces). Solo se usa para
# reconocerlo (D-113).
LEGACY=docker-control-center
DEB_ARG=""; MODE=install; ASSUME_YES=0; VERIFY=1

usage() { sed -n '5,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }
while [ $# -gt 0 ]; do
  case "$1" in
    --deb) shift; DEB_ARG="${1:-}"; [ -n "$DEB_ARG" ] || { echo "--deb necesita la ruta de un paquete"; exit 2; } ;;
    --check) MODE=check ;;
    --uninstall) MODE=uninstall ;;
    --yes|-y) ASSUME_YES=1 ;;
    --no-verify) VERIFY=0 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Opción desconocida: $1"; usage; exit 2 ;;
  esac
  shift
done

if [ -t 1 ]; then B=$'\e[1m'; G=$'\e[32m'; Y=$'\e[33m'; R=$'\e[31m'; N=$'\e[0m'; else B=""; G=""; Y=""; R=""; N=""; fi
ok()   { printf '  %s✔%s %s\n' "$G" "$N" "$*"; }
warn() { printf '  %s!%s %s\n' "$Y" "$N" "$*"; }
fail() { printf '  %s✘%s %s\n' "$R" "$N" "$*"; }
info() { printf '    %s\n' "$*"; }
title() { printf '\n%s%s%s\n' "$B" "$*" "$N"; }
die()  { fail "$*"; exit 1; }

# Órdenes con privilegios: solo apt-get, con sudo si no eres root.
as_root() {
  if [ "$(id -u)" -eq 0 ]; then "$@"
  elif command -v sudo >/dev/null; then sudo -- "$@"
  else die "Hace falta sudo para ejecutar: $*  (o ejecútalo como root)"
  fi
}

title "Container Manage Center (CMC) · instalación"

# ---------------------------------------------------------------- 1. Sistema
title "1. Sistema"
[ "$(uname -s)" = Linux ] || die "Este instalador es para Linux."
OS_ID=""; OS_LIKE=""; OS_NAME="Linux"
if [ -r /etc/os-release ]; then
  # shellcheck disable=SC1091
  OS_ID=$(. /etc/os-release; echo "${ID:-}"); OS_LIKE=$(. /etc/os-release; echo "${ID_LIKE:-}"); OS_NAME=$(. /etc/os-release; echo "${PRETTY_NAME:-Linux}")
fi
case " $OS_ID $OS_LIKE " in
  *" debian "*|*" ubuntu "*) ok "$OS_NAME" ;;
  *) warn "$OS_NAME no es Debian, Ubuntu ni una derivada: el paquete .deb puede no funcionar." ;;
esac
command -v dpkg >/dev/null && command -v apt-get >/dev/null || die "Faltan dpkg y apt-get: este instalador necesita un sistema basado en Debian."
ARCH=$(dpkg --print-architecture)
[ "$ARCH" = amd64 ] || die "Arquitectura $ARCH: los paquetes son para amd64 (64 bits x86)."
ok "Arquitectura amd64"
GLIBC=$(dpkg-query -W -f='${Version}' libc6 2>/dev/null | sed 's/-.*//')
[ -n "$GLIBC" ] || die "No se ha podido leer la versión de libc6."
ok "glibc $GLIBC"
if [ "$(id -u)" -eq 0 ]; then
  warn "Estás ejecutando el instalador como root. Instalar está bien, pero usa la aplicación con tu usuario normal."
fi

# ------------------------------------------------------------- Desinstalar
if [ "$MODE" = uninstall ]; then
  title "Desinstalar"
  REMOVE=()
  dpkg-query -W -f='${Status}' "$PKG" 2>/dev/null | grep -q "install ok installed" && REMOVE+=("$PKG")
  if [ ${#REMOVE[@]} -eq 0 ]; then
    ok "Container Manage Center no está instalado."; exit 0
  fi
  YES=(); [ $ASSUME_YES -eq 1 ] && YES=(-y)
  as_root apt-get remove "${YES[@]}" "${REMOVE[@]}" || die "apt no ha podido desinstalar el paquete."
  ok "Desinstalado (${REMOVE[*]}). No se ha tocado Docker ni ningún contenedor, imagen, red o volumen."
  info "Tus datos se conservan; bórralos a mano si ya no los quieres:"
  info "  ~/.config/container-manage-center  (configuración, servidores e identidades de SSH)"
  info "  ~/.local/state/container-manage-center  (historial)"
  info "  Las contraseñas recordadas están en el llavero del sistema («Container Manage Center»)."
  info "  Las copias de seguridad, en la carpeta container-manage-center-copias de su destino."
  exit 0
fi

# ------------------------------------------------------- 2. Elegir paquete
title "2. Paquete"
# Versión de glibc que exige un paquete (de su Depends: libc6 (>= X)).
needs_glibc() { dpkg-deb -f "$1" Depends 2>/dev/null | grep -o 'libc6 (>= [^)]*)' | sed 's/libc6 (>= //; s/)//' | head -1; }
CANDIDATES=()
if [ -n "$DEB_ARG" ]; then
  [ -f "$DEB_ARG" ] || die "No existe $DEB_ARG"
  CANDIDATES=("$(realpath "$DEB_ARG")")
else
  for d in "$SCRIPT_DIR" "$SCRIPT_DIR/dist" "$SCRIPT_DIR/../dist" "$SCRIPT_DIR/../target/release/bundle/deb" "$SCRIPT_DIR/../target/compat" "$PWD"; do
    [ -d "$d" ] || continue
    for f in "$d"/"$PKG"_*_amd64*.deb; do [ -f "$f" ] && CANDIDATES+=("$(realpath "$f")"); done
  done
fi
# Sin duplicados.
mapfile -t CANDIDATES < <(printf '%s\n' "${CANDIDATES[@]}" | awk 'NF && !seen[$0]++')
[ ${#CANDIDATES[@]} -gt 0 ] || die "No se ha encontrado ningún paquete ${PKG}_*_amd64.deb. Indica uno con --deb RUTA."
BEST=""; BEST_NEED=""; BEST_VER=""
for f in "${CANDIDATES[@]}"; do
  [ "$(dpkg-deb -f "$f" Package 2>/dev/null)" = "$PKG" ] || { warn "$(basename "$f"): no es un paquete de Container Manage Center; se ignora."; continue; }
  [ "$(dpkg-deb -f "$f" Architecture)" = amd64 ] || { warn "$(basename "$f"): no es para amd64; se ignora."; continue; }
  need=$(needs_glibc "$f"); ver=$(dpkg-deb -f "$f" Version)
  if [ -z "$need" ]; then
    warn "$(basename "$f") $ver no declara la glibc que necesita; se ignora."
    continue
  fi
  if dpkg --compare-versions "$GLIBC" ge "$need"; then
    info "$f — versión $ver, exige glibc $need: válido"
    # El más reciente por versión; a igual versión, el compilado para la glibc más nueva.
    if [ -z "$BEST" ] || dpkg --compare-versions "$ver" gt "$BEST_VER" || { [ "$ver" = "$BEST_VER" ] && dpkg --compare-versions "$need" gt "$BEST_NEED"; }; then
      BEST=$f; BEST_NEED=$need; BEST_VER=$ver
    fi
  else
    info "$f — versión $ver, exige glibc $need: tu sistema tiene $GLIBC, no sirve"
  fi
done
if [ -z "$BEST" ]; then
  fail "Ningún paquete encontrado funciona con glibc $GLIBC."
  info "Para Debian 12 o Ubuntu 22.04 usa el paquete compatible (scripts/build-deb-compat.sh lo genera)."
  exit 1
fi
ok "Paquete elegido: $(basename "$BEST") (versión $BEST_VER)"

# Integridad (E2, CMC-SEC-09, D-092): suma SHA-256 del paquete según el
# SHA256SUMS de la publicación (junto a este script o junto al paquete).
# Las sumas detectan un paquete dañado o cambiado; la firma GPG de SHA256SUMS
# queda pendiente de la clave del propietario (ver INSTALL.md).
SUMS=""
for s in "$SCRIPT_DIR/SHA256SUMS" "$(dirname "$BEST")/SHA256SUMS"; do
  [ -f "$s" ] && { SUMS=$s; break; }
done
if [ $VERIFY -eq 0 ]; then
  warn "Comprobación de SHA256SUMS omitida (--no-verify)."
elif [ -z "$SUMS" ]; then
  warn "No hay SHA256SUMS junto al paquete: no se puede comprobar su integridad."
  info "Si lo has descargado, comprueba su suma con la que publica el autor: sha256sum $(basename "$BEST")"
else
  REL=$(realpath --relative-to="$(dirname "$SUMS")" "$BEST")
  EXPECTED=$(awk -v f="$REL" '$2 == f || $2 == "*" f { print $1; exit }' "$SUMS")
  [ -n "$EXPECTED" ] || die "$REL no aparece en $SUMS: no se instala (usa --no-verify solo si sabes de dónde viene el paquete)."
  ACTUAL=$(sha256sum -- "$BEST" | cut -d' ' -f1)
  if [ "$ACTUAL" = "$EXPECTED" ]; then
    ok "Suma SHA-256 correcta ($SUMS)"
  else
    die "La suma SHA-256 de $REL no coincide con $SUMS: el paquete está dañado o se ha modificado. No se instala."
  fi
fi

INSTALLED=$(dpkg-query -W -f='${Status} ${Version}' "$PKG" 2>/dev/null | sed -n 's/^install ok installed //p')
[ -n "$INSTALLED" ] && info "Versión instalada ahora: $INSTALLED"
LEGACY_VER=$(dpkg-query -W -f='${Status} ${Version}' "$LEGACY" 2>/dev/null | sed -n 's/^install ok installed //p')
if [ -n "$LEGACY_VER" ]; then
  info "Tienes instalada una versión anterior ($LEGACY_VER, paquete $LEGACY). apt la sustituirá y después"
  info "se pasarán tus datos al nombre nuevo: configuración, servidores, historial y tareas programadas."
  info "Las contraseñas del llavero y las carpetas de copias se pasan al abrir la aplicación o usarlas."
fi

# Simulación de apt (sin privilegios): dependencias que se instalarían.
SIM=$(apt-get install -s "$BEST" 2>&1); SIM_RC=$?
if [ $SIM_RC -ne 0 ]; then
  fail "apt no puede instalar el paquete en este sistema:"
  printf '%s\n' "$SIM" | grep -E "^ |Depends|E:" | head -15 | sed 's/^/    /'
  info "Si faltan paquetes, comprueba que tienes activados los repositorios principales y ejecuta «sudo apt update»."
  exit 1
fi
NEW=$(printf '%s\n' "$SIM" | sed -n 's/^Inst \([^ ]*\).*/\1/p' | grep -vx "$PKG" | tr '\n' ' ')
GONE=$(printf '%s\n' "$SIM" | sed -n 's/^Remv \([^ ]*\).*/\1/p' | tr '\n' ' ')
if [ -n "$GONE" ] && [ "$(echo $GONE)" != "$LEGACY" ]; then
  fail "apt quitaría además estos paquetes: $GONE"
  die "No se instala: revisa la simulación con «apt-get install -s $BEST»."
fi
[ -n "$GONE" ] && info "apt quitará el paquete anterior: $LEGACY (sustituido por $PKG)."
[ -n "$NEW" ] && info "apt instalará también: $NEW" || info "No hace falta instalar otras dependencias."

# ---------------------------------------------------------- 3. Instalar
if [ "$MODE" = install ]; then
  title "3. Instalación"
  YES=(); [ $ASSUME_YES -eq 1 ] && YES=(-y)
  info "Se ejecutará: sudo apt-get install ${YES[*]} $BEST"
  as_root apt-get install "${YES[@]}" "$BEST" || die "apt no ha terminado la instalación."
  STATUS=$(dpkg-query -W -f='${Status} ${Version}' "$PKG" 2>/dev/null)
  case "$STATUS" in
    "install ok installed $BEST_VER") ok "Container Manage Center $BEST_VER instalado" ;;
    *) die "El paquete no ha quedado instalado ($STATUS)." ;;
  esac
  [ -x /usr/bin/container-manage-center ] && ok "Programa: /usr/bin/container-manage-center" || fail "Falta /usr/bin/container-manage-center"
  [ -f /usr/share/applications/container-manage-center.desktop ] && ok "Lanzador en el menú de aplicaciones" || fail "Falta el lanzador del menú"
  if dpkg-query -W -f='${Status}' "$LEGACY" 2>/dev/null | grep -q "install ok installed"; then
    fail "El paquete anterior $LEGACY sigue instalado."
  fi
  # Datos de una versión anterior: se pasan ya, con tu usuario (D-113). Así
  # las tareas de copia programadas siguen funcionando aunque no abras la
  # aplicación. Si no había nada que pasar, no hace nada.
  if /usr/bin/container-manage-center --migrate; then
    ok "Datos de versiones anteriores: al día"
  else
    warn "No se pudieron pasar todos los datos de una versión anterior; se conservan sin cambios y se volverá a intentar al abrir la aplicación."
  fi
else
  title "3. Instalación"
  info "Modo --check: no se instala nada. Para instalar: bash $0 --deb $BEST"
fi

# ------------------------------------------- 4. Requisitos para trabajar
title "4. Lo que la aplicación necesita (solo se comprueba; no se cambia nada)"
if command -v docker >/dev/null; then
  ok "Cliente de Docker: $(docker --version 2>/dev/null)"
else
  warn "No está el cliente de Docker (docker). La aplicación habla con Docker Engine por su socket, pero el apartado Docker Compose necesita el CLI (docker compose)."
  info "Instala Docker Engine siguiendo https://docs.docker.com/engine/install/ (paquetes docker-ce) o el paquete docker.io de tu distribución."
fi
SOCK=""
if [ -n "${DOCKER_HOST:-}" ] && [ "${DOCKER_HOST#unix://}" != "$DOCKER_HOST" ]; then SOCK=${DOCKER_HOST#unix://}
elif [ -S "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/docker.sock" ]; then SOCK="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/docker.sock"
elif [ -S /var/run/docker.sock ]; then SOCK=/var/run/docker.sock
fi
if [ -z "$SOCK" ]; then
  warn "No se encuentra el socket de Docker (¿está Docker Engine instalado y en marcha?)."
  info "Con systemd: sudo systemctl enable --now docker"
elif [ -r "$SOCK" ] && [ -w "$SOCK" ]; then
  ok "Tienes acceso a Docker: $SOCK"
else
  warn "Docker está en $SOCK pero tu usuario no tiene acceso."
  info "Opción recomendada por seguridad: Docker rootless (https://docs.docker.com/engine/security/rootless/)."
  info "Opción habitual: sudo usermod -aG docker $USER  y vuelve a iniciar sesión."
  info "Atención: pertenecer al grupo docker equivale a tener acceso de root. El instalador no lo hace por ti."
fi
if command -v docker >/dev/null && docker compose version >/dev/null 2>&1; then
  ok "Docker Compose: $(docker compose version --short 2>/dev/null)"
elif command -v docker-compose >/dev/null; then
  ok "Docker Compose: $(docker-compose version --short 2>/dev/null) (docker-compose)"
else
  warn "No está Docker Compose v2: el apartado Docker Compose no funcionará (el resto sí)."
  info "Debian: sudo apt install docker-compose  ·  Docker oficial: sudo apt install docker-compose-plugin"
fi
if command -v ssh >/dev/null; then
  ok "Cliente SSH: $(ssh -V 2>&1) (servidores remotos)"
else
  warn "No está el cliente OpenSSH: no podrás añadir servidores remotos. sudo apt install openssh-client"
fi
if command -v busctl >/dev/null && busctl --user list 2>/dev/null | grep -q "org.freedesktop.secrets"; then
  ok "Almacén seguro de contraseñas disponible (Secret Service)"
else
  info "No se detecta un almacén de contraseñas (Secret Service) en esta sesión: las contraseñas de los servidores se pedirán cada vez. GNOME y KDE lo incluyen."
fi

title "Listo"
if [ "$MODE" = install ]; then
  info "Abre «Container Manage Center» desde el menú de aplicaciones o ejecuta: container-manage-center"
  info "Desinstalar: bash $0 --uninstall  (o sudo apt remove $PKG)"
fi
exit 0
