#!/usr/bin/env bash
# Instalación de Docker Control Center en Debian, Ubuntu y derivadas (Fase 10).
#
# Uso (con tu usuario normal; pide la contraseña de sudo solo para apt):
#   bash install.sh                      # busca el paquete .deb y lo instala
#   bash install.sh --deb RUTA.deb       # instala ese paquete
#   bash install.sh --check              # solo comprueba el sistema y elige el paquete (no instala)
#   bash install.sh --uninstall          # desinstala la aplicación (conserva tus datos)
#   Añade --yes para no tener que confirmar en apt.
#
# Qué hace:
#   1. Comprueba el sistema: Debian/Ubuntu o derivada, amd64, apt y la versión de glibc.
#   2. Elige el paquete adecuado entre los que encuentra: junto a este script,
#      en dist/ y en las carpetas de compilación (target/release/bundle/deb y
#      target/compat). El paquete normal exige la glibc de Debian 13 / Ubuntu 24.04;
#      el compatible, la de Debian 12 / Ubuntu 22.04. Se usa el más reciente que
#      tu sistema puede ejecutar.
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
PKG=docker-control-center
DEB_ARG=""; MODE=install; ASSUME_YES=0

usage() { sed -n '2,9p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }
while [ $# -gt 0 ]; do
  case "$1" in
    --deb) shift; DEB_ARG="${1:-}"; [ -n "$DEB_ARG" ] || { echo "--deb necesita la ruta de un paquete"; exit 2; } ;;
    --check) MODE=check ;;
    --uninstall) MODE=uninstall ;;
    --yes|-y) ASSUME_YES=1 ;;
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

title "Docker Control Center · instalación"

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
  if ! dpkg-query -W -f='${Status}' "$PKG" 2>/dev/null | grep -q "install ok installed"; then
    ok "Docker Control Center no está instalado."; exit 0
  fi
  YES=(); [ $ASSUME_YES -eq 1 ] && YES=(-y)
  as_root apt-get remove "${YES[@]}" "$PKG" || die "apt no ha podido desinstalar el paquete."
  ok "Desinstalado. No se ha tocado Docker ni ningún contenedor, imagen, red o volumen."
  info "Tus datos se conservan; bórralos a mano si ya no los quieres:"
  info "  ~/.config/docker-control-center  (configuración, servidores e identidades de SSH)"
  info "  ~/.local/share/docker-control-center  (historial)"
  info "  Las contraseñas recordadas están en el llavero del sistema («Docker Control Center»)."
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
  [ "$(dpkg-deb -f "$f" Package 2>/dev/null)" = "$PKG" ] || { warn "$(basename "$f"): no es un paquete de Docker Control Center; se ignora."; continue; }
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

INSTALLED=$(dpkg-query -W -f='${Status} ${Version}' "$PKG" 2>/dev/null | sed -n 's/^install ok installed //p')
[ -n "$INSTALLED" ] && info "Versión instalada ahora: $INSTALLED"

# Simulación de apt (sin privilegios): dependencias que se instalarían.
SIM=$(apt-get install -s "$BEST" 2>&1); SIM_RC=$?
if [ $SIM_RC -ne 0 ]; then
  fail "apt no puede instalar el paquete en este sistema:"
  printf '%s\n' "$SIM" | grep -E "^ |Depends|E:" | head -15 | sed 's/^/    /'
  info "Si faltan paquetes, comprueba que tienes activados los repositorios principales y ejecuta «sudo apt update»."
  exit 1
fi
NEW=$(printf '%s\n' "$SIM" | sed -n 's/^Inst \([^ ]*\).*/\1/p' | grep -vx "$PKG" | tr '\n' ' ')
[ -n "$NEW" ] && info "apt instalará también: $NEW" || info "No hace falta instalar otras dependencias."

# ---------------------------------------------------------- 3. Instalar
if [ "$MODE" = install ]; then
  title "3. Instalación"
  YES=(); [ $ASSUME_YES -eq 1 ] && YES=(-y)
  info "Se ejecutará: sudo apt-get install ${YES[*]} $BEST"
  as_root apt-get install "${YES[@]}" "$BEST" || die "apt no ha terminado la instalación."
  STATUS=$(dpkg-query -W -f='${Status} ${Version}' "$PKG" 2>/dev/null)
  case "$STATUS" in
    "install ok installed $BEST_VER") ok "Docker Control Center $BEST_VER instalado" ;;
    *) die "El paquete no ha quedado instalado ($STATUS)." ;;
  esac
  [ -x /usr/bin/docker-control-center ] && ok "Programa: /usr/bin/docker-control-center" || fail "Falta /usr/bin/docker-control-center"
  [ -f /usr/share/applications/docker-control-center.desktop ] && ok "Lanzador en el menú de aplicaciones" || fail "Falta el lanzador del menú"
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
  info "Abre «Docker Control Center» desde el menú de aplicaciones o ejecuta: docker-control-center"
  info "Desinstalar: bash $0 --uninstall  (o sudo apt remove $PKG)"
fi
exit 0
