#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
# Verificación de Container Manage Center 1.7.0 (antes Docker Control Center:
# identidad, licencia y copyright) en Debian/Ubuntu. Repite toda la regresión
# automática de las fases 1-10 y de las etapas 1-6, y comprueba lo nuevo de la
# 1.7.0 (D-110-D-112):
#   - nombre visible Container Manage Center y metadatos coherentes (nombre,
#     versión y licencia iguales en Cargo.toml, package.json, tauri.conf.json,
#     packaging/copyright y el lanzador), sin licencia «provisional»;
#   - LICENSE es el texto oficial de la GPL v3; encabezados SPDX en todo el
#     código propio; avisos de terceros completos y auditoría sin pendientes;
#   - paquete container-manage-center con Conflicts/Replaces/Provides del
#     anterior, /usr/bin/docker-control-center (0755) que abre el programa
#     nuevo, documentación legal y de migración en /usr/share/doc;
#   - desinstalación que reconoce los dos nombres; apartado y ayuda «Legal».
# De la Etapa 6 (1.6.0):
#   - la auditoría identifica los contenedores que vigila la monitorización y
#     que protegen las copias, por el mismo id de contenedor (D-108);
#   - las copias guardan el id del contenedor; las anteriores se reconocen por
#     nombre y así se indica;
#   - los enlaces solo leen y no se registran en el panel de actividad;
#   - de la auditoría se abre la monitorización de un contenedor o se prepara
#     una copia, que el usuario revisa antes de copiar.
# Además, de la Etapa 5:
#   - métricas iguales a «docker stats», historial con huecos, alertas con
#     histéresis y notificaciones, y consumo medido con 50 contenedores y 3
#     servidores.
# De la Etapa 4:
#   - formato de copias con manifiesto y SHA-256, restauración completa y
#     selectiva, retención exacta y temporizadores de usuario de systemd con
#     autorización expresa.
# Y de la Etapa 3:
#   - comprobaciones sobre contenedores peligrosos y legítimos (riesgos para el
#     equipo, root, sistema de archivos modificable, recursos sin límite,
#     seccomp, AppArmor, no-new-privileges, PID e IPC, puertos, imagen
#     antigua), imágenes y servicios de Compose;
#   - solo lectura: ninguna petición de escritura a Docker y nada cambia;
#   - vulnerabilidades con Trivy o Grype: «no analizado» sin herramienta o sin
#     base, nunca «0» sin análisis completo;
#   - historial por servidor (0600) y exportación JSON/Markdown/HTML sin secretos;
#   - correcciones propuestas en «Editar configuración» sin aplicar nada.
#
# Uso (desde la raíz del proyecto, con tu usuario normal, NO como root):
#   bash scripts/verify-evolution-1.7.0.sh --deb            # recomendado: incluye la compilación
#                                                    # de producción y el paquete .deb
#   bash scripts/verify-evolution-1.7.0.sh --deb --compat   # y el paquete compatible con
#        Debian 12+/Ubuntu 22.04+ (compilado en un contenedor de Docker con
#        Ubuntu 22.04: descarga la imagen ubuntu:22.04 y ~700 MB de paquetes)
#   --net    además consulta un servidor HTTPS real (TLS con los certificados
#            del sistema) para probar la comprobación de actualizaciones
#   --ssh-lab ARCHIVO   además ejecuta las pruebas reales por SSH contra un
#            laboratorio de servidores (lab.json de scripts/dev/remote-lab.sh;
#            también con DCC_SSH_LAB=ARCHIVO). Solo en una máquina de pruebas
#            desechable. Sin laboratorio se listan como omitidas: la
#            administración remota ya está validada (0.10.0).
#
# Requisitos: los de la Fase 1, acceso a Docker con tu usuario (grupo docker o
# Docker rootless), Docker Compose v2 o posterior y el cliente OpenSSH. El
# script NO usa sudo, NO instala nada (tampoco el paquete: eso lo haces tú con
# scripts/install.sh en las comprobaciones manuales), NO cambia permisos ni
# grupos y NO toca tu ~/.ssh ni tu llavero (la prueba del llavero usa una
# sesión D-Bus propia con un llavero temporal, si gnome-keyring está instalado).
#
# Recursos de prueba: contenedores, redes, imágenes y volúmenes con la etiqueta
# dcc.test=true o con nombre dcc-test-*; nunca toca recursos que no haya creado
# y al final comprueba que no queda ninguno.
#
# Un paso que filtra pruebas por nombre falla si no ejecuta ninguna: así un
# nombre cambiado no pasa por correcto sin probar nada.
#
# La prueba real de la desinstalación con APT (D-065) y la de instalación limpia
# (scripts/dev/clean-install-test.sh) necesitan root y sistemas de prueba: no se
# ejecutan en tu equipo (se listan como omitidas o se hacen a mano en M1).
#
# Resultado: verify-evolution-1.7.0-report.txt y los registros completos de cada paso
# en target/verify-evolution-1.7.0/. Código de salida 0 solo si no hay ningún fallo
# obligatorio. Las omisiones justificadas se listan aparte.

set -u
cd "$(dirname "$0")/.." || exit 1
ROOT=$(pwd)
REPORT="$ROOT/verify-evolution-1.7.0-report.txt"
LOGDIR="$ROOT/target/verify-evolution-1.7.0"
rm -rf "$LOGDIR"
mkdir -p "$LOGDIR"
: > "$REPORT"
BUILD_DEB=0; BUILD_COMPAT=0; NET_TESTS=0
SSH_LAB="${DCC_SSH_LAB:-}"
while [ $# -gt 0 ]; do
  case "$1" in
    --deb) BUILD_DEB=1 ;;
    --compat) BUILD_COMPAT=1 ;;
    --net) NET_TESTS=1 ;;
    --ssh-lab) shift; SSH_LAB="${1:-}"; [ -n "$SSH_LAB" ] || { echo "--ssh-lab necesita un archivo"; exit 2; } ;;
    *) echo "Opción desconocida: $1 (usa --deb, --compat, --net o --ssh-lab ARCHIVO)"; exit 2 ;;
  esac
  shift
done
# Variables de las pruebas SSH: solo las define este script, en su paso.
unset DCC_SSH_LAB DCC_SSH_TEST DCC_SSH_TEST_KEY DCC_SSH_TEST_KNOWN_HOSTS DCC_KEYRING_TEST DCC_TEST_ASKPASS
if [ -n "$SSH_LAB" ]; then
  case "$SSH_LAB" in /*) ;; *) SSH_LAB="$ROOT/$SSH_LAB" ;; esac
  [ -r "$SSH_LAB" ] || { echo "No se puede leer el laboratorio SSH: $SSH_LAB"; exit 2; }
fi

PASS=0; FAIL=0; SKIP=0; WARN=0
FAILS=(); SKIPS=()
say()  { printf '%s\n' "$*" | tee -a "$REPORT"; }
ok()   { PASS=$((PASS+1)); say "  [OK]       $*"; }
bad()  { FAIL=$((FAIL+1)); FAILS+=("$*"); say "  [FALLO]    $*"; }
skip() { SKIP=$((SKIP+1)); SKIPS+=("$*"); say "  [OMITIDA]  $*"; }
warn() { WARN=$((WARN+1)); say "  [AVISO]    $*"; }
ver_ge() { [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" = "$2" ]; }

LAST_LOG=""
STEP=0
run_step() {
  local name="$1"; shift
  STEP=$((STEP+1))
  # Nombre corto y solo ASCII: un nombre de más de 255 bytes no se puede crear.
  LAST_LOG="$LOGDIR/$(printf '%02d' "$STEP")-$(printf '%s' "$name" | tr -c 'A-Za-z0-9._-' '_' | cut -c1-60).log"
  printf '  ...        %s\n' "$name"
  if "$@" >"$LAST_LOG" 2>&1; then ok "$name"; return 0; fi
  bad "$name (registro: $LAST_LOG)"
  grep -E "panicked|left:|right:|^error|FAIL|×" "$LAST_LOG" | head -12 | sed 's/^/             /' | tee -a "$REPORT" >/dev/null
  return 1
}

# Como run_step, pero falla si no se ha ejecutado ninguna prueba (un filtro que
# no coincide con nada, por ejemplo una prueba renombrada).
run_tests() {
  local name="$1"; shift
  run_step "$name" "$@" || return 1
  local n
  n=$(grep -h "^test result" "$LAST_LOG" | awk '{s+=$4} END {print s+0}')
  [ "$n" -eq 0 ] && n=$(grep -hoE "Tests +[0-9]+ passed" "$LAST_LOG" | grep -oE "[0-9]+" | head -1)
  if [ "${n:-0}" -gt 0 ]; then
    say "             $n pruebas ejecutadas"
    return 0
  fi
  PASS=$((PASS-1))
  bad "$name: no se ha ejecutado ninguna prueba (¿filtro o nombre cambiado?) ($LAST_LOG)"
  return 1
}

# ------------------------------------------------------------------ limpieza
HAVE_DOCKER_CLI=0
command -v docker >/dev/null && HAVE_DOCKER_CLI=1
cleanup_test_resources() {
  [ $HAVE_DOCKER_CLI -eq 1 ] || return 0
  local c i v n
  c=$(docker ps -aq --filter label=dcc.test=true 2>/dev/null)
  [ -n "$c" ] && docker rm -f $c >/dev/null 2>&1
  n=$(docker network ls -q --filter label=dcc.test=true 2>/dev/null)
  [ -n "$n" ] && docker network rm $n >/dev/null 2>&1
  i=$(docker image ls -q --filter label=dcc.test=true 2>/dev/null | sort -u)
  [ -n "$i" ] && docker image rm -f $i >/dev/null 2>&1
  v=$( (docker volume ls -q --filter name=dcc-test-vol- 2>/dev/null; docker volume ls -q --filter label=dcc.test=true 2>/dev/null; docker volume ls -q 2>/dev/null | grep -E '^dcc-test-') | sort -u)
  [ -n "$v" ] && docker volume rm -f $v >/dev/null 2>&1
  return 0
}
# Laboratorio SSH: valores del lab.json (sin jq: con Node, que ya es requisito).
lab_get() { node -e 'const l=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8"));let v=l;for(const k of process.argv[2].split("."))v=v?.[k];if(v===undefined)process.exit(1);process.stdout.write(String(v))' "$SSH_LAB" "$1"; }
lab_count() { node -e 'const l=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8"));process.stdout.write(String(l.servers.length))' "$SSH_LAB"; }
# Orden docker fija en un servidor del laboratorio, con su clave, su huella
# ya confirmada (StrictHostKeyChecking=yes) y sin tocar ~/.ssh. ssh une los
# argumentos en una línea que interpreta la shell del servidor: cada uno se
# entrecomilla aparte (comillas simples POSIX), nunca se concatenan sin más.
sq() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }
lab_docker() {
  local i="$1" a cmd="docker"; shift
  for a in "$@"; do cmd="$cmd $(sq "$a")"; done
  ssh -F /dev/null -o BatchMode=yes -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes \
      -o UserKnownHostsFile="$(lab_get knownHosts)" -o ConnectTimeout=10 \
      -i "$(lab_get key)" -p "$(lab_get servers.$i.port)" \
      -- "$(lab_get servers.$i.user)@$(lab_get servers.$i.host)" "$cmd"
}
LAB_LEFT=""
cleanup_lab_resources() {
  [ -n "$SSH_LAB" ] || return 0
  local i n c v k
  LAB_LEFT=""
  n=$(lab_count 2>/dev/null || echo 0)
  for ((i=0; i<n; i++)); do
    c=$(lab_docker $i ps -aq --filter label=dcc.test=true 2>/dev/null)
    [ -n "$c" ] && lab_docker $i rm -f $c >/dev/null 2>&1
    k=$(lab_docker $i network ls -q --filter label=dcc.test=true 2>/dev/null)
    [ -n "$k" ] && lab_docker $i network rm $k >/dev/null 2>&1
    v=$(lab_docker $i volume ls -q --filter label=dcc.test=true 2>/dev/null; lab_docker $i volume ls -q 2>/dev/null | grep -E '^dcc-test-')
    v=$(printf '%s\n' "$v" | sort -u | grep . || true)
    [ -n "$v" ] && lab_docker $i volume rm -f $v >/dev/null 2>&1
    c=$(lab_docker $i ps -aq --filter label=dcc.test=true 2>/dev/null | wc -l)
    k=$(lab_docker $i network ls -q --filter label=dcc.test=true 2>/dev/null | wc -l)
    LAB_LEFT="$LAB_LEFT $(lab_get servers.$i.name):$c/$k"
  done
  return 0
}
finish() {
  local rc=$?
  cleanup_test_resources
  cleanup_lab_resources
  exit $rc
}
trap finish EXIT
trap 'exit 130' INT TERM

say "Container Manage Center - verificación de la 1.7.0: identidad, licencia y copyright (versión $(grep -m1 '^version' Cargo.toml | cut -d'"' -f2))"
say "Fecha: $(date -Is)"
say "Proyecto: $ROOT"
say ""

# ------------------------------------------------------------------ 1. sistema
say "1. Sistema y herramientas"
if [ "$(id -u)" -eq 0 ]; then
  if [ "${DCC_VERIFY_ALLOW_ROOT:-}" = "1" ]; then
    warn "Ejecutado como root con DCC_VERIFY_ALLOW_ROOT=1 (solo entornos de desarrollo aislados)"
    export DCC_ALLOW_ROOT=1
  else
    bad "Estás ejecutando el script como root. Ejecútalo con tu usuario normal."
  fi
fi
. /etc/os-release 2>/dev/null || true
say "  Distribución: ${PRETTY_NAME:-desconocida}   Núcleo: $(uname -r)"
say "  Escritorio: ${XDG_CURRENT_DESKTOP:-desconocido}   Sesión: ${XDG_SESSION_TYPE:-desconocida}"
say "  Usuario: $(id -un) ($(id -u))   Grupos: $(id -Gn)"
case "${ID:-}" in
  debian) ver_ge "${VERSION_ID:-0}" 12 && ok "Debian ${VERSION_ID}" || bad "Debian ${VERSION_ID:-?}: se requiere 12 o superior" ;;
  ubuntu) ver_ge "${VERSION_ID:-0}" 22.04 && ok "Ubuntu ${VERSION_ID}" || bad "Ubuntu ${VERSION_ID:-?}: se requiere 22.04 o superior" ;;
  *) warn "Distribución no probada oficialmente (${ID:-?})" ;;
esac
if command -v rustc >/dev/null; then
  RV=$(rustc --version | awk '{print $2}')
  ver_ge "$RV" 1.80.0 && ok "Rust $RV" || bad "Rust $RV: se requiere 1.80 o superior"
else bad "Rust no está instalado"; fi
if command -v node >/dev/null; then
  NV=$(node --version | tr -d v)
  ver_ge "$NV" 20.0.0 && ok "Node.js $NV" || bad "Node.js $NV: se requiere 20 o superior"
else bad "Node.js no está instalado"; fi
command -v cc >/dev/null && ok "Compilador C (imagen de prueba local)" || bad "Falta un compilador C: sudo apt install build-essential"
if [ -n "$(find /usr/lib -name libc.a -print -quit 2>/dev/null)" ]; then ok "libc estática (libc6-dev)"; else bad "Falta libc estática: sudo apt install libc6-dev"; fi
command -v unshare >/dev/null && ok "unshare (util-linux)" || warn "unshare no disponible: no se podrá probar el funcionamiento sin red"
if command -v ssh >/dev/null; then
  SSHV=$(ssh -V 2>&1 | grep -oE 'OpenSSH_[0-9]+\.[0-9]+' | head -1 | cut -d_ -f2)
  if [ -n "$SSHV" ] && ver_ge "$SSHV" 7.6; then ok "Cliente OpenSSH $SSHV (administración remota)"; else bad "Cliente SSH no reconocido o anterior a OpenSSH 7.6: $(ssh -V 2>&1)"; fi
else
  bad "Falta el cliente OpenSSH (administración remota y sus pruebas): sudo apt install openssh-client"
fi
command -v ssh-keygen >/dev/null && ok "ssh-keygen (huellas de los servidores)" || bad "Falta ssh-keygen: sudo apt install openssh-client"
GLIBC=$(ldd --version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+$')
say "  glibc del sistema: ${GLIBC:-desconocida}"
if [ $BUILD_DEB -eq 1 ] || [ $BUILD_COMPAT -eq 1 ]; then
  command -v dpkg-shlibdeps >/dev/null && ok "dpkg-shlibdeps (dependencias reales del paquete)" || bad "Falta dpkg-shlibdeps: sudo apt install dpkg-dev"
  command -v objdump >/dev/null && ok "objdump (versión de glibc que exige el binario)" || bad "Falta objdump: sudo apt install binutils"
fi
say ""
if [ "$FAIL" -gt 0 ]; then
  say "Faltan requisitos. Corrígelos y vuelve a ejecutar el script."
  say "RESULTADO: $PASS correctas, $FAIL fallos, $SKIP omitidas, $WARN avisos"
  say "VEREDICTO: FASE 10 NO VERIFICADA (faltan requisitos)"
  exit 1
fi

# ------------------------------------------------------------------ 2. Docker
say "2. Docker en este equipo (solo lectura)"
if [ $HAVE_DOCKER_CLI -eq 1 ]; then
  CLIV=$(docker version --format '{{.Client.Version}}' 2>/dev/null || true)
  if [ -n "$CLIV" ] && ver_ge "$CLIV" 20.10.0; then ok "CLI de Docker $CLIV ($(command -v docker))"; else warn "Versión del CLI de Docker no reconocida o anterior a 20.10: '${CLIV:-?}'"; fi
  if SV=$(docker version --format '{{.Server.Version}} (API {{.Server.APIVersion}})' 2>/dev/null) && [ -n "$SV" ]; then
    ok "El CLI de Docker habla con Docker Engine $SV"
    DRV=$(docker info --format '{{.Driver}} {{json .DriverStatus}}' 2>/dev/null)
    case "$DRV" in
      *io.containerd.snapshotter*) say "  Almacén de imágenes: containerd (${DRV%% *})" ;;
      "") ;;
      *) say "  Almacén de imágenes: clásico (${DRV%% *})" ;;
    esac
  else
    warn "El CLI de Docker no puede hablar con el daemon: $(docker info 2>&1 | grep -iE 'permission denied|cannot connect|error' | head -1)"
  fi
else
  warn "CLI de Docker no encontrado (la app no lo necesita salvo para Compose; no se podrán limpiar recursos de prueba)"
fi
say "  DOCKER_HOST=${DOCKER_HOST:-(no definido)}   contexto: $(docker context show 2>/dev/null || echo '?')"
say ""

say "3. Dependencias y compilación de herramientas"
run_step "npm ci" npm ci --no-audit --no-fund
run_step "Herramienta de diagnóstico (dcc-diagnose)" cargo build --locked -p dcc-engine --bin dcc-diagnose
DIAG="$ROOT/target/debug/dcc-diagnose"
say ""

# ------------------------------------------------------------------ 4. conexión y Compose
say "4. Conexión real y Docker Compose (misma detección que la aplicación)"
SOCK=""
if [ -x "$DIAG" ]; then
  "$DIAG" --compose > "$LOGDIR/diagnose.txt" 2>&1
  DRC=$?
  sed 's/^/             /' "$LOGDIR/diagnose.txt" | tee -a "$REPORT" >/dev/null
  if [ $DRC -eq 0 ]; then
    ok "La aplicación conecta con Docker ($(head -1 "$LOGDIR/diagnose.txt"))"
    SOCK=$(grep -m1 "Socket:" "$LOGDIR/diagnose.txt" | awk '{print $2}')
  else
    bad "La aplicación no conecta con Docker con tu usuario (causa en el diagnóstico de arriba)"
  fi
  CLINE=$(grep -m1 "^Docker Compose:" "$LOGDIR/diagnose.txt" | sed 's/^Docker Compose: //')
  case "$CLINE" in
    disponible:*) ok "Docker Compose $CLINE" ;;
    "no instalado") bad "Docker Compose no está instalado: es necesario para las pruebas de la Fase 7, que se repiten (Debian 13: sudo apt install docker-compose; ver INSTALL.md)" ;;
    "") bad "No se obtuvo el resultado de la detección de Compose" ;;
    *) bad "Docker Compose detectado pero no utilizable: $CLINE" ;;
  esac
  "$DIAG" --socket /nonexistent/dcc-test.sock > "$LOGDIR/diagnose-missing.txt" 2>&1
  [ $? -eq 2 ] && grep -q "NoSocket" "$LOGDIR/diagnose-missing.txt" && ok "Socket inexistente diagnosticado como NoSocket" || bad "Diagnóstico de socket inexistente incorrecto ($LOGDIR/diagnose-missing.txt)"
  "$DIAG" --socket relativa.sock > "$LOGDIR/diagnose-invalid.txt" 2>&1
  grep -q "InvalidPath" "$LOGDIR/diagnose-invalid.txt" && ok "Ruta relativa rechazada (InvalidPath)" || bad "La ruta relativa no se rechazó ($LOGDIR/diagnose-invalid.txt)"
  if [ -z "$SOCK" ]; then
    skip "Funcionamiento sin red: no hay conexión con Docker que probar"
  elif ! command -v unshare >/dev/null || ! unshare -rn true 2>/dev/null; then
    skip "Funcionamiento sin red: este sistema no permite espacios de nombres de usuario sin privilegios (compruébalo manualmente, M9)"
  elif unshare -rn "$DIAG" --socket "$SOCK" > "$LOGDIR/diagnose-offline.txt" 2>&1; then
    ok "Funciona sin red (espacio de nombres de red vacío) con el Docker local"
  else
    bad "Sin red no conecta con el Docker local ($LOGDIR/diagnose-offline.txt)"
  fi
else
  bad "No se pudo compilar dcc-diagnose"
fi
say ""

# ------------------------------------------------------------------ 5. pruebas
# Pruebas contra Docker real: una línea por prueba con su resultado, para que un
# bloque fallido indique exactamente qué prueba falla y por qué.
list_tests() {
  local log="$1" n_ok n_fail
  grep -E "^test [^ ]+ \.\.\. (ok|FAILED|ignored)$" "$log" | sort -u | while read -r _ name _ res; do
    case "$res" in
      ok) say "             [ok]     $name" ;;
      FAILED) say "             [FALLA]  $name" ;;
      *) say "             [ignorada] $name" ;;
    esac
  done
  n_ok=$(grep -cE "^test [^ ]+ \.\.\. ok$" "$log" || true)
  n_fail=$(grep -cE "^test [^ ]+ \.\.\. FAILED$" "$log" || true)
  say "             $n_ok pruebas superadas, $n_fail fallidas"
}
live_block() {
  # live_block "Nombre" paquete binario [hilos]
  local name="$1" pkg="$2" bin="$3" threads="${4:-2}" rc
  if [ "$bin" = "-" ]; then
    run_step "$name" cargo test --locked -p "$pkg" live_ -- --show-output --test-threads="$threads"; rc=$?
  else
    run_step "$name" cargo test --locked -p "$pkg" --test "$bin" -- --show-output --test-threads="$threads"; rc=$?
  fi
  list_tests "$LAST_LOG"
  return $rc
}

say "5. Pruebas"
say ""
say "5.1 Infraestructura y calidad del código (no prueban funcionalidad)"
run_step "Clippy sin avisos" cargo clippy --workspace --all-targets --locked -- -D warnings
run_step "Formato Rust" cargo fmt --all -- --check
run_step "Tipos TypeScript" npx tsc --noEmit
run_step "Compilación de la interfaz (Vite)" npm run build
if node -e "require('playwright')" >/dev/null 2>&1; then
  run_step "La interfaz arranca con Object.prototype congelado (freezePrototype de Tauri)" node scripts/dev/check-frozen-prototype.cjs
else
  skip "Comprobación con prototipo congelado: requiere el paquete playwright (opcional; la cubre M1)"
fi
say ""
say "5.2 Pruebas funcionales deterministas (sin Docker)"
if run_step "Pruebas Rust deterministas (unitarias, de protocolo e IPC)" cargo test --workspace --locked; then
  grep -h "^test result" "$LAST_LOG" | awk '{s+=$4; f+=$6} END {printf "             %d correctas, %d fallidas (las pruebas contra Docker real no hacen nada en este paso; se ejecutan en 5.3)\n", s, f}' | tee -a "$REPORT"
fi
if run_step "Pruebas de interfaz (Vitest)" npx vitest run; then
  grep -hE "Tests +[0-9]" "$LAST_LOG" | sed 's/^/             /' | tee -a "$REPORT"
fi
run_tests "Carrera de estado de Docker 26 (prueba determinista, D-031)" cargo test --locked -p dcc-engine --test stale_list
run_tests "Carrera de estado de Docker 26 en proyectos Compose (prueba determinista, D-056)" cargo test --locked -p dcc-engine --test stale_compose
run_tests "Suscripción a eventos: corte de la conexión, rechazo y cancelación (prueba determinista, D-057)" cargo test --locked -p dcc-engine --test events_interrupted
run_tests "Comprobación de versiones nuevas: servidor local real, errores, límites, redirecciones y fuentes no válidas (D-060)" cargo test --locked -p docker-control-center --lib updates::
run_tests "Comandos de la Fase 9 por IPC: información del sistema, comprobación con servidor local, configuración [updates]" cargo test --locked -p docker-control-center --lib phase9_
run_tests "0.9.1 · Registro de flujos: cerrar todos y esperar a que terminen (cierre de la aplicación, D-063)" cargo test --locked -p docker-control-center --lib streams::
run_tests "0.9.1 · Terminal: marca del PID retirada de la salida, órdenes de cada shell (D-063)" cargo test --locked -p dcc-engine --lib streams::
run_tests "0.9.1 · Desinstalación: dpkg, historial de APT (también rotado), simulación, dependencias inversas, clasificación, protección de Docker y del sistema, nombres, pkexec sin purge (D-065)" cargo test --locked -p docker-control-center --lib uninstall::tests
run_tests "0.9.1 · Configuración: francés e italiano se guardan y se cargan" cargo test --locked -p dcc-store french_and_italian
run_tests "0.9.2 · Configuración: los ocho idiomas en orden se guardan y se cargan; el color guardado es el morado" cargo test --locked -p dcc-store language_order_and_accent_roundtrip
run_tests "0.9.2 · Los cambios de idioma quedan en el panel de actividad (una vez, como operación de la aplicación)" cargo test --locked -p docker-control-center --lib language_changes_are_recorded_once
run_tests "0.9.2 · Edición: lectura, clasificación directo/recrear, secretos ocultos, validación, cuerpo de docker update y de recreación, redes, MAC, nombres de copia, puertos ocupados (D-070, D-071)" cargo test --locked -p dcc-engine --lib edit
run_tests "0.9.2 · Restaurar una copia exige confirmación explícita también por IPC" cargo test --locked -p docker-control-center --lib backup_restore_requires_explicit_confirmation
run_tests "0.10.0 · Perfiles de servidor: archivo propio con permisos 0600, sin secretos, entradas no válidas o repetidas descartadas, archivo dañado apartado (D-078)" cargo test --locked -p dcc-store servers::
run_tests "0.10.0 · SSH: argumentos seguros (opciones antes del destino, -- y comillas por argumento), huellas, clasificación de errores, perfiles, askpass, sesiones huérfanas (D-075–D-077)" cargo test --locked -p dcc-engine --lib ssh::
run_tests "0.10.0 · Órdenes en el servidor: argv remoto entrecomillado y orden equivalente ssh … -- …; archivos Compose del servidor sin tocar el equipo (D-080)" cargo test --locked -p dcc-engine --lib -- cli:: compose::remote_file
run_tests "0.10.0 · Registro de servidores: alta, edición y eliminación aisladas, proyectos Compose por servidor, lastSeen no viene de la interfaz (D-078)" cargo test --locked -p docker-control-center --lib servers::tests
run_tests "0.10.0 · Secretos: caché de sesión, sin llavero se informa y nunca se guarda en archivo (D-077)" cargo test --locked -p docker-control-center --lib secrets::
run_tests "0.10.0 · Historial y eventos filtrados por servidor (D-079)" cargo test --locked -p docker-control-center --lib history_and_events_filter_by_server
run_tests "0.10.0 · IPC: toda orden de Docker exige un servidor explícito; perfiles sin tocar nada más; la contraseña de una prueba nunca aparece; la terminal SSH solo para servidores remotos conocidos" cargo test --locked -p docker-control-center --lib -- docker_commands_are_bound_to_an_explicit_server server_profiles_are_managed_without_touching_anything_else a_password_test_never_leaks_the_secret server_shell_is_only_for_known_remote_servers
# Llavero real (Secret Service) en una sesión D-Bus propia con un llavero
# temporal: nunca el llavero del usuario.
KEYRING_OK=0
if command -v dbus-run-session >/dev/null && command -v gnome-keyring-daemon >/dev/null; then
  KR_DIR=$(mktemp -d)
  cat > "$KR_DIR/run.sh" <<'KR'
#!/bin/bash
# $1 = directorio temporal; resto = orden. Llavero propio desbloqueado.
d="$1"; shift
export XDG_DATA_HOME="$d/data" XDG_RUNTIME_DIR="$d/run"; mkdir -p "$XDG_DATA_HOME" "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
exec dbus-run-session -- bash -c 'printf "dcc-verify" | gnome-keyring-daemon --unlock --components=secrets >/dev/null 2>&1; sleep 0.5; "$@"' _ "$@"
KR
  chmod 700 "$KR_DIR/run.sh"
  if run_tests "0.10.0 · Llavero real (Secret Service temporal): guardar, sustituir sin duplicar, leer y borrar una contraseña (D-077)" "$KR_DIR/run.sh" "$KR_DIR" env DCC_KEYRING_TEST=1 cargo test --locked -p docker-control-center --lib secrets::tests::real_keyring_store_load_delete -- --nocapture; then
    if grep -q "omitida" "$LAST_LOG"; then bad "La prueba del llavero no se ejecutó"; else KEYRING_OK=1; fi
  fi
else
  skip "Llavero real: faltan dbus-run-session o gnome-keyring-daemon (sudo apt install dbus gnome-keyring); lo cubre la comprobación manual R3"
fi
run_tests "1.0.0 · Color principal: una configuración con el verde («teal»), un valor desconocido o de otro tipo pasa a morado conservando el resto de preferencias, sin marcar el archivo como dañado; al guardar queda el morado (D-082)" cargo test --locked -p dcc-store saved_green_accent_migrates_to_purple_keeping_the_rest
run_tests "1.0.0 · Color principal en la interfaz: tokens del acento morados en claro y oscuro (tono 250°–290°), sin los valores del verde, contraste WCAG AA, colores semánticos sin cambios, Configuración sin selector, terminales en morado, estados de los botones (hover, active, foco, deshabilitado, cargando) con los tokens del tema (D-082)" npx vitest run ui/src/styles/accent.test.ts ui/src/app/appearance.test.tsx ui/src/shared/ui/controls.test.tsx
# El verde no está en el código de la interfaz, en el CSS compilado ni en el logotipo.
GREEN_RE='#0d8287|#0a6b6f|#25b0b3|#4cc6c8|#14a3a8|13 130 135|37 176 179|data-accent'
GREEN_SRC=$(grep -rliE "$GREEN_RE" ui/src packaging src-tauri/icons --include='*.ts' --include='*.tsx' --include='*.css' --include='*.svg' 2>/dev/null | grep -vE '\.test\.tsx?$' || true)
GREEN_DIST=$(grep -rliE "$GREEN_RE" dist 2>/dev/null || true)
if [ -z "$GREEN_SRC" ] && [ -z "$GREEN_DIST" ] && grep -qi "#7046d4" dist/assets/*.css 2>/dev/null; then
  ok "1.0.0 · Sin verde como color principal en el código, en la interfaz compilada (dist/) ni en el logotipo; el morado sí está"
else
  bad "1.0.0 · Queda el verde como color principal (o falta el morado en dist/): ${GREEN_SRC} ${GREEN_DIST}"
fi
run_step "1.0.0 · Lista de componentes de terceros y sus licencias al día con Cargo.lock y package-lock.json" python3 scripts/third-party-licenses.py --check
if command -v shellcheck >/dev/null; then
  run_step "1.0.0 · Scripts de instalación y de ejecución (shellcheck)" shellcheck -S warning scripts/install.sh scripts/run.sh scripts/dev/clean-install-test.sh
else
  run_step "1.0.0 · Scripts de instalación y de ejecución (sintaxis; shellcheck no está instalado)" bash -c 'bash -n scripts/install.sh && bash -n scripts/run.sh && bash -n scripts/dev/clean-install-test.sh'
fi
say ""
say "5.1b Etapa 1 (1.1.0): correcciones de seguridad prioritarias"
run_tests "1.1.0 · DCC-SEC-05 Secretos: nombres (MYSQL_PWD, DB_PASS, PGPASS, REDIS_AUTH, SMTP_PASS, tokens, claves) y valores (URL y DSN con contraseña, claves privadas PEM, Authorization), cuerpos JSON, órdenes con cualquier entrecomillado, salidas YAML de Compose, opciones --password/--token; sin tocar lo útil; reglas iguales en la interfaz (D-085)" cargo test --locked -p dcc-activity redact::
run_tests "1.1.0 · DCC-SEC-05 Ningún valor secreto llega al panel de actividad, a sus eventos ni al historial (cuerpo, orden equivalente, argumentos, ruta, resumen, salidas, error)" cargo test --locked -p dcc-activity secrets_never_reach_records_events_or_history
run_tests "1.1.0 · DCC-SEC-05 Los mensajes de error que recibe la interfaz no llevan secretos" cargo test --locked -p docker-control-center --lib error::tests
run_tests "1.1.0 · DCC-SEC-01/03 Reglas de riesgo: socket de Docker/containerd/podman también en solo lectura, rutas sin normalizar y enlaces simbólicos, credenciales, carpetas del sistema, capacidades (CAP_ y minúsculas), red del equipo, aceptación por clave (D-086)" cargo test --locked -p dcc-engine --lib risk::
run_tests "1.1.0 · DCC-SEC-01/03 Sin falsos positivos (plan §9.5) en contenedores y en Compose; las configuraciones peligrosas equivalentes sí exigen aceptación" cargo test --locked -p dcc-engine --lib -- legitimate_configurations_have_no_host_risks compose_services_risks_without_false_positives
run_tests "1.1.0 · DCC-SEC-01 Validación del asistente y pantalla de directorios vinculados con las mismas reglas" cargo test --locked -p dcc-engine --lib -- spec::tests::mounts_are_validated spec::tests::resources_restart_and_security_are_validated binds::tests::risks
run_tests "1.1.0 · DCC-SEC-02 Servidor intermedio: ProxyCommand con las reglas del destino (huella estricta, known_hosts de la aplicación, BatchMode, sin contraseña) y sin askpass, comprobado con la sustitución de %h/%p y un shell real; askpass solo para el usuario y el servidor de destino; un solo salto (D-087)" cargo test --locked -p dcc-engine --lib -- ssh::args ssh::askpass ssh::profile
run_tests "1.1.0 · Interfaz: secretos ocultos con las mismas reglas, riesgos para el equipo en el asistente, la edición y el despliegue de Compose (aceptación uno a uno, enviada al motor), huella del servidor intermedio" npx vitest run ui/src/shared/secrets.test.ts ui/src/features/containers/create/CreateWizard.test.tsx ui/src/features/containers/edit/edit.test.tsx ui/src/features/compose/ComposeView.test.tsx ui/src/features/servers/servers.test.tsx
say ""
say "5.1c Etapa 2 (1.2.0): resto del endurecimiento"
run_tests "1.2.0 · DCC-SEC-04/08 Riesgos: pid/ipc (alto), userns/cgroup (medio), uts (bajo); security_opt (seccomp/apparmor/systempaths unconfined, label=disable, no-new-privileges=false); dispositivos (memoria crítico, discos alto, multimedia bajo); cgroup_parent; puertos en todas las interfaces (bajo, no con red host) (D-088)" cargo test --locked -p dcc-engine --lib -- risk::tests::namespaces_security_options_devices_and_ports compose_namespaces_security_devices_and_ports spec::tests::ports_are_validated
run_tests "1.2.0 · DCC-SEC-04 Sin falsos positivos (plan §9.5) con las reglas nuevas" cargo test --locked -p dcc-engine --lib -- legitimate_configurations_have_no_host_risks compose_services_risks_without_false_positives
run_tests "1.2.0 · DCC-SEC-06/13 Rutas de archivos Compose: /etc/passwd, ~/.ssh, ~/.bashrc, carpetas ocultas y del sistema, no YAML y enlaces a ellos rechazados; discos de /media y /run/media permitidos; guardar por un enlace escribe en el destino y conserva el enlace; crear en una carpeta oculta (o enlazada a ella) rechazado (D-089)" cargo test --locked -p dcc-engine --lib compose::file::tests
run_tests "1.2.0 · DCC-SEC-06 Por IPC: leer, guardar y crear fuera de la política devuelve engine.pathNotAllowed" cargo test --locked -p docker-control-center --lib compose_files_via_ipc_create_read_save_and_conflict
run_tests "1.2.0 · DCC-SEC-07 Credenciales de registro: en un servidor remoto, las de este equipo solo con permiso explícito; las escritas a mano, siempre (D-090)" cargo test --locked -p dcc-engine --lib registry_auth::tests::local_credentials_reach_remote_servers_only_with_consent
run_tests "1.2.0 · DCC-SEC-11 Como root no arranca; DCC_ALLOW_ROOT solo vale en compilaciones de desarrollo (D-091)" cargo test --locked -p docker-control-center --lib root_tests
run_tests "1.2.0 · DCC-SEC-12/16 Enlaces: de las terminales solo http(s) sin credenciales; página de la versión solo de github.com o de la fuente de actualizaciones (D-091)" cargo test --locked -p docker-control-center --lib -- terminal_links_are_http_only only_https_links_are_opened release_pages_only_from_github_or_the_own_source
run_tests "1.2.0 · DCC-SEC-17 Permisos de Tauri mínimos: escuchar eventos sí; ventana, rutas, aplicación, webviews y emitir eventos, denegados por la ACL real" cargo test --locked -p docker-control-center --lib only_the_minimal_tauri_permissions_are_granted
run_tests "1.2.0 · Interfaz: enlaces de terminal con confirmación (Cancelar no abre nada), casilla de credenciales en servidores remotos (desmarcada por omisión), error de ruta traducido" npx vitest run ui/src/shared/ui/terminalLinks.test.tsx ui/src/features/images/ImagesView.test.tsx ui/src/app/languages.test.tsx
run_step "1.2.0 · ESLint (configuración mínima: errores reales y reglas de seguridad)" npm run lint
if cargo deny --version >/dev/null 2>&1; then
  run_step "1.2.0 · DCC-SEC-14 cargo deny check (avisos, licencias, orígenes; excepciones justificadas en deny.toml)" cargo deny check
else
  skip "cargo deny: no está instalado (cargo install --locked cargo-deny)"
fi
if npm audit --omit=dev >"$LOGDIR/npm-audit-prod.log" 2>&1; then
  ok "1.2.0 · npm audit --omit=dev: $(tail -1 "$LOGDIR/npm-audit-prod.log")"
elif grep -qiE "ENOTFOUND|ECONNREFUSED|EAI_AGAIN|network|audit endpoint" "$LOGDIR/npm-audit-prod.log"; then
  skip "npm audit: sin conexión con el registro de npm ($LOGDIR/npm-audit-prod.log)"
else
  bad "npm audit --omit=dev encuentra vulnerabilidades ($LOGDIR/npm-audit-prod.log)"
fi
if npm audit >"$LOGDIR/npm-audit-all.log" 2>&1; then
  ok "1.2.0 · DCC-SEC-10 npm audit (también desarrollo, vitest 5): $(tail -1 "$LOGDIR/npm-audit-all.log")"
elif grep -qiE "ENOTFOUND|ECONNREFUSED|EAI_AGAIN|network|audit endpoint" "$LOGDIR/npm-audit-all.log"; then
  skip "npm audit completo: sin conexión con el registro de npm"
else
  bad "npm audit (dependencias de desarrollo) encuentra vulnerabilidades ($LOGDIR/npm-audit-all.log)"
fi
say ""
say "5.1d Etapa 3 (1.3.0): auditoría de seguridad"
run_tests "1.3.0 · Comprobaciones: contenedor peligroso (todas), legítimo y endurecido (ninguna), privilegiado sin repetir, root con rootless/userns, cgroup v1, opciones de seguridad según el daemon, imágenes (root, antigüedad, fechas no fiables) y servicios de Compose (D-093)" cargo test --locked -p dcc-engine --lib audit::checks
run_tests "1.3.0 · Vulnerabilidades: versión y fecha de la base de Trivy y Grype, resultados, sin base o con fallo «no analizado»/«falló» (nunca 0), cancelación, sin red y sin pasar opciones como imagen (D-094)" cargo test --locked -p dcc-engine --lib audit::vuln
run_tests "1.3.0 · Historial e informe: sin auditorías, auditoría inexistente, servidor desconocido, secretos ocultos antes de guardar; exportación con secretos ocultos, permisos 0600, extensión y nombre seguro (D-095)" cargo test --locked -p docker-control-center --lib -- audit_history_and_get_without_docker commands::audit::tests
run_tests "1.3.0 · Interfaz: resultado con explicación y recomendación, filtros, propuesta de corrección que abre la edición sin aplicar nada, exportación por el diálogo de Rust, historial, cancelación, servidor remoto sin vulnerabilidades, textos de exportación escapados y sin secretos" npx vitest run ui/src/features/audit ui/src/features/containers/edit
say ""
say "5.1e Etapa 4 (1.4.0): copias de seguridad y restauración"
run_tests "1.4.0 · Formato: copia completa con carpetas 0700 y archivos 0600, manifiesto escrito el último; interrumpida = incompleta y nunca abierta; suma alterada, archivo que falta y de más detectados; rutas peligrosas rechazadas; retención exacta 7/4/3 y solo de la misma tarea (D-096, D-097)" cargo test --locked -p dcc-engine --lib backup::store
run_tests "1.4.0 · Volcados: detección del motor, guiones constantes sin credenciales en argumentos y probados en un sh real con herramientas simuladas (contraseña, variante _FILE, sin credenciales) (D-098)" cargo test --locked -p dcc-engine --lib backup::db
run_tests "1.4.0 · Restauración y petición: riesgos del contenedor con las mismas reglas que al crear, carpetas siempre nuevas, volumen vacío solo si el tar lo prueba, nombres de archivo, secretos quitados de la configuración (D-099)" cargo test --locked -p dcc-engine --lib -- backup::restore backup::run backup::docker
run_tests "1.4.0 · Tareas y temporizadores (systemctl simulado): sin autorización no se escribe nada, vista previa exacta, crear/consultar/desactivar/quitar, archivos ajenos intactos, la programación sobrevive a un reinicio de la aplicación, tareas validadas y 0600 (D-100)" cargo test --locked -p docker-control-center --lib -- systemd:: backups:: e4_jobs_and_timers_need_authorization_and_survive_a_restart
run_tests "1.4.0 · Interfaz: destino por el diálogo nativo, lista con estado y avisos, comprobar integridad, eliminar con confirmación, copia «deteniendo» con aviso, avance, autorización de systemd con los archivos a la vista, restauración selectiva con vista previa y palabra de confirmación" npx vitest run ui/src/features/backup
say ""
say "5.1f Etapa 5 (1.5.0): monitorización avanzada"
run_tests "1.5.0 · Muestreo: CPU con la misma fórmula que la CLI de Docker, memoria sin caché como «docker stats», velocidades de red y disco, contador que vuelve a empezar sin valor inventado, recuentos del servidor (D-102)" cargo test --locked -p dcc-engine --lib monitor::sampler
run_tests "1.5.0 · Historial: anillo limitado en entradas y en tiempo, huecos sin datos, último dato solo de muestras con conexión, resumen por minuto con medias, máximos y bytes (D-103)" cargo test --locked -p dcc-engine --lib monitor::history
run_tests "1.5.0 · Alertas: umbral con histéresis y muestras seguidas, periodo de silencio sin repetir el aviso, parada pedida frente a caída, reinicio, OOM, salida con error, desconexión, reglas validadas (D-104)" cargo test --locked -p dcc-engine --lib monitor::alerts
run_tests "1.5.0 · Servicio: configuración validada y 0600, minutos en disco (0600, carpeta 0700) con lectura filtrada y compactación por antigüedad y tamaño, pausa, arranque y parada; notificación por un bus D-Bus privado con un servidor falso, texto limitado y sin caracteres de control (D-103, D-105)" cargo test --locked -p docker-control-center --lib -- monitor:: notify::
run_tests "1.5.0 · Sin Docker: la monitorización registra huecos, no bloquea los comandos y vuelve a arrancar sola tras reiniciar la aplicación" cargo test --locked -p docker-control-center --lib e5_monitor_without_docker_shows_gaps_and_survives_a_restart
run_tests "1.5.0 · Interfaz: activar por servidor, pausa, intervalo, periodos en vivo y del disco, equipo según Docker, tabla con tiempo en marcha y reinicios, detalle, comparar hasta 4, reglas, prueba de notificación, historial; gráfico propio con huecos y trazos distintos (D-106)" npx vitest run ui/src/features/monitor
say ""
say "5.1g Etapa 6 (1.6.0): integración entre herramientas"
run_tests "1.6.0 · Enlaces: monitorización por id (un contenedor con el mismo nombre y otro id no se confunde), copias por id y por nombre en las anteriores o si se recreó, por proyecto Compose, la más reciente primero, solo copias completas, tareas del mismo servidor por nombre, proyecto o volumen (D-108)" cargo test --locked -p docker-control-center --lib links::
run_tests "1.6.0 · Interfaz: tabla de contenedores en las demás herramientas, «Ver en Monitorización» con el contenedor elegido, «Preparar una copia» que solo marca la selección (configuración, base de datos en marcha y volúmenes con nombre, por id), error visible y sin consulta si no hay contenedores" npx vitest run ui/src/features/audit ui/src/features/backup ui/src/features/monitor
say ""
say "5.1h 1.7.0: identidad Container Manage Center, licencia y copyright"
meta_coherence() {
  local v cv ok_=1
  cv=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
  for v in "$(node -p 'require("./package.json").version')" "$(node -p 'require("./src-tauri/tauri.conf.json").version')" "$(node -p 'require("./package-lock.json").version')"; do
    [ "$v" = "$cv" ] || { echo "versión distinta: $v (Cargo.toml $cv)"; ok_=0; }
  done
  [ "$(grep -m1 '^license' Cargo.toml | cut -d'"' -f2)" = "GPL-3.0-or-later" ] || { echo "licencia de Cargo.toml"; ok_=0; }
  [ "$(node -p 'require("./package.json").license')" = "GPL-3.0-or-later" ] || { echo "licencia de package.json"; ok_=0; }
  [ "$(node -p 'require("./src-tauri/tauri.conf.json").bundle.license')" = "GPL-3.0-or-later" ] || { echo "licencia de tauri.conf.json"; ok_=0; }
  grep -q '^License: GPL-3.0-or-later' packaging/copyright || { echo "licencia de packaging/copyright"; ok_=0; }
  node -e 'const c=require("./src-tauri/tauri.conf.json");const ok=c.productName==="container-manage-center"&&c.mainBinaryName==="container-manage-center"&&c.app.windows.every(w=>w.title==="Container Manage Center")&&c.identifier==="org.dockercontrolcenter.app"&&/2026 Sergio Alfaro Lorite/.test(c.bundle.copyright);process.exit(ok?0:1)' || { echo "nombre, título, identificador o copyright de tauri.conf.json"; ok_=0; }
  [ "$(node -p 'require("./package.json").name')" = "container-manage-center" ] || { echo "nombre de package.json"; ok_=0; }
  grep -q '^Name=Container Manage Center$' packaging/container-manage-center.desktop || { echo "nombre del lanzador"; ok_=0; }
  grep -q '<title>Container Manage Center</title>' ui/index.html || { echo "título de ui/index.html"; ok_=0; }
  if grep -rniE 'licen[cs][ie]a? provisional|provisional (license|licence)' Cargo.toml package.json src-tauri/tauri.conf.json packaging README.md INSTALL.md USER_GUIDE.md docs/legal/LICENSE_OVERVIEW.md docs/legal/COPYRIGHT.md 2>/dev/null; then echo "licencia declarada como provisional"; ok_=0; fi
  [ $ok_ -eq 1 ]
}
run_step "1.7.0 · Metadatos coherentes: versión, licencia GPL-3.0-or-later y nombre iguales en Cargo.toml, package.json, package-lock.json, tauri.conf.json (identificador sin cambiar), packaging/copyright, lanzador e index.html; ninguna licencia «provisional»" meta_coherence
license_official() {
  local want=3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986
  if [ -r /usr/share/common-licenses/GPL-3 ]; then cmp LICENSE /usr/share/common-licenses/GPL-3 || return 1; fi
  [ "$(sha256sum LICENSE | cut -d' ' -f1)" = "$want" ]
}
run_step "1.7.0 · LICENSE es el texto oficial de la GPL v3 (igual a /usr/share/common-licenses/GPL-3 de Debian)" license_official
run_step "1.7.0 · Encabezados SPDX (GPL-3.0-or-later, 2026 Sergio Alfaro Lorite) en todo el código propio; ninguno en código generado ni de terceros" python3 scripts/spdx-headers.py --check
if command -v cargo-deny >/dev/null; then
  run_step "1.7.0 · cargo deny: solo licencias permitidas en las dependencias de Rust" cargo deny --offline check licenses
else
  skip "1.7.0 · cargo deny no está instalado (la clasificación de licencias se comprueba con third-party-licenses.py --check)"
fi
legal_docs() {
  local f ok_=1
  for f in docs/legal/LICENSE_OVERVIEW.md docs/legal/DEPENDENCY_LICENSE_AUDIT.md docs/legal/THIRD_PARTY_NOTICES.md docs/legal/COPYRIGHT.md docs/legal/DISTRIBUTION_COMPLIANCE.md docs/MIGRATION-CMC.md CONTRIBUTING.md; do
    [ -s "$f" ] || { echo "falta $f"; ok_=0; }
  done
  grep -q '| 4. Pendientes de revisión | 0 |' docs/legal/DEPENDENCY_LICENSE_AUDIT.md || { echo "hay componentes pendientes"; ok_=0; }
  grep -q '| 5. Potencialmente incompatibles | 0 |' docs/legal/DEPENDENCY_LICENSE_AUDIT.md || { echo "hay componentes potencialmente incompatibles"; ok_=0; }
  grep -q '| 6. Licencia no determinable | 0 |' docs/legal/DEPENDENCY_LICENSE_AUDIT.md || { echo "hay componentes sin licencia determinable"; ok_=0; }
  [ ! -e packaging/THIRD-PARTY.md ] || { echo "queda packaging/THIRD-PARTY.md"; ok_=0; }
  [ $ok_ -eq 1 ]
}
run_step "1.7.0 · Documentos legales y de migración presentes; auditoría sin componentes pendientes, incompatibles ni indeterminados" legal_docs
visible_name() {
  # Ningún texto de la interfaz, mensaje ni notificación dice «Docker Control
  # Center»; la ayuda solo lo cita como nombre anterior en la sección Legal.
  local hits f
  hits=$(grep -rnE 'Docker Control Center|>Control Center<|"Control Center"' ui/src src-tauri/src crates ui/index.html packaging --include='*.json' --include='*.ts' --include='*.tsx' --include='*.rs' --include='*.html' --include='*.desktop' | grep -vE '\.test\.tsx?:|^[^:]+:[0-9]+:[[:space:]]*//' || true)
  [ -z "$hits" ] || { echo "$hits"; return 1; }
  for f in ui/src/features/help/docs/*.md; do
    if awk '/\{#legal\}/{l=1} /Docker Control Center/ && !l {found=1} END{exit !found}' "$f"; then echo "$f nombra el producto anterior fuera de la sección Legal"; return 1; fi
  done
}
run_step "1.7.0 · Nombre visible: ningún texto de la interfaz, mensaje, notificación ni lanzador dice «Docker Control Center» (la ayuda solo lo cita como nombre anterior)" visible_name
run_tests "1.7.0 · Desinstalación: reconoce los dos nombres de paquete y el cambio de nombre en el historial de APT no cuenta como desinstalación" cargo test --locked -p docker-control-center --lib uninstall::tests::the_rename_to_container_manage_center_keeps_the_history
run_tests "1.7.0 · Interfaz: Configuración › Legal (nombre, versión, licencia SPDX, copyright, declaración GPL, URL, avisos de terceros, independencia de Docker, Inc.) y sección «Legal» de la ayuda en los 8 idiomas" npx vitest run ui/src/features/settings/LegalSettings.test.tsx ui/src/features/help/HelpView.test.tsx ui/src/shared/i18n/i18n.test.ts
if [ $NET_TESTS -eq 1 ]; then
  run_tests "Consulta HTTPS real con TLS y los certificados del sistema (registry.npmjs.org)" env DCC_NET_TESTS=1 cargo test --locked -p docker-control-center --lib updates::tests::real_https -- --nocapture
fi
say ""
say "5.3 Pruebas funcionales contra Docker real (crean y eliminan recursos dcc.test)"
if [ -n "$SOCK" ]; then
  export DCC_DOCKER_TESTS=1 DCC_DOCKER_SOCKET="$SOCK"
  if live_block "Motor: conexión, listados, ciclo de vida (Fases 1-3)" dcc-engine live_docker; then
    O=$(grep -c "omitida:" "$LAST_LOG" || true)
    [ "$O" -gt 0 ] && say "             de ellas $O sin aplicar en este equipo (abajo)"
    while IFS= read -r l; do skip "Prueba real sin aplicar: ${l#*omitida: }"; done < <(grep -o "omitida: .*" "$LAST_LOG" | sort -u)
  fi
  live_block "Contenedores: asistente, inspección, duplicado, logs, estadísticas, terminal (Fase 3)" dcc-engine live_containers
  if live_block "Imágenes: descarga (progreso, credenciales, cancelación, contenido ya presente), búsqueda, inspección, capas, etiquetado, eliminación, limpieza, construcción BuildKit y clásica (Fase 4)" dcc-engine live_images; then
    if grep -q "búsqueda con Internet" "$LAST_LOG"; then
      say "             Búsqueda en Docker Hub: $(grep -o 'búsqueda con Internet: .*' "$LAST_LOG" | head -1)"
    else
      say "             Búsqueda en Docker Hub: el daemon no tiene Internet; se comprobó el error y su registro ($(grep -o 'búsqueda sin Internet en el daemon: .*' "$LAST_LOG" | head -1 | cut -c1-120))"
    fi
  fi
  if live_block "Redes: creación con IPAM, nombres repetidos, listado, inspección, conexión con alias e IP fija, desconexión, eliminación (en uso y con contenedores detenidos), limpieza, IPv6 (Fase 5)" dcc-engine live_networks; then
    say "             IPv6: $(grep -o 'ipv6: .*' "$LAST_LOG" | head -1 | cut -c7-200)"
  fi
  live_block "Volúmenes y directorios vinculados: creación (y nombre repetido), contenedores que los montan, tamaño real, inspección, eliminación en uso, limpieza con y sin --all, carpetas vinculadas con su estado y riesgos (Fase 6)" dcc-engine live_volumes
  if live_block "Docker Compose: modelo con perfiles, despliegue con construcción, parar/iniciar/reiniciar, perfiles, una acción a la vez, eliminación con confirmación, proyecto sin archivos, validación sin guardar, registros en vivo, cancelación, secretos fuera del registro, estado igual al de Docker tras cada operación (Fase 7)" dcc-engine live_compose 1; then
    if grep -q "omitida:" "$LAST_LOG"; then
      bad "Pruebas de Compose NO ejecutadas: $(grep -o 'omitida: .*' "$LAST_LOG" | head -1 | cut -c10-)"
    else
      say "             $(grep -m1 -o 'Compose: .*' "$LAST_LOG")"
    fi
  fi
  live_block "0.9.1 · Terminales múltiples: varias a la vez sin interferencias (salida, entrada, tamaño), cierre individual sin procesos restantes, programa que ignora Ctrl+C, contenedor detenido o eliminado, cerrar todas (D-063)" dcc-engine live_terminals 1
  live_block "0.9.2 · Edición y recreación segura: cambios directos (mismo contenedor), recreación que conserva datos (volumen, anónimo, carpeta) y deja copia, vuelta atrás si el nuevo no arranca, contenedor detenido y cambios de montajes, recrear y renombrar a la vez (copia listada y restaurable), recreaciones inseguras rechazadas sin tocar nada, MAC fija, imagen a partir de un contenedor sin datos de volúmenes (D-069–D-071)" dcc-engine live_edit 1
  live_block "Eventos de Docker: suscripción en tiempo real (orden, atributos, registro), consulta con filtros de fecha, tipo, acción y recurso, consultas no válidas (Fase 8)" dcc-engine live_events
  if run_step "1.1.0 · Contra Docker real por IPC: crear con el socket de Docker en solo lectura se rechaza sin aceptar (y con otra clave), se crea al aceptarlo; editar añadiendo /etc y desplegar Compose con el socket o SYS_ADMIN se rechazan en el motor; ningún valor secreto en el panel (a Docker sí llega el real)" cargo test --locked -p docker-control-center --lib live_e1_risks_and_secrets_via_ipc -- --nocapture --test-threads=1; then
    grep -q "omitida" "$LAST_LOG" && bad "La prueba IPC de la Etapa 1 no se ejecutó"
    grep -q "Compose no disponible" "$LAST_LOG" && bad "La parte de Compose de la prueba IPC de la Etapa 1 no se ejecutó"
  fi
  live_block "1.3.0 · Auditoría contra Docker real: contenedor peligroso con todas las comprobaciones, legítimo sin hallazgos, solo peticiones de lectura y contenedores sin cambios, imágenes y proyectos Compose (sin desplegar), cancelación" dcc-engine live_audit
  run_step "1.3.0 · Auditoría por IPC: encuentra el socket montado, se guarda en el historial (0600), se abre de nuevo y durante la auditoría solo hay lecturas" env DCC_DOCKER_TESTS=1 cargo test --locked -p docker-control-center --lib live_e3_audit_via_ipc_is_read_only_and_saved -- --nocapture --test-threads=1
  grep -q "omitida" "$LAST_LOG" && bad "La prueba IPC de la auditoría no se ejecutó"
  # Imágenes de bases de datos para las copias: las de prueba
  # (scripts/dev/build-db-test-images.sh) o las indicadas con DCC_TEST_*_IMAGE.
  DBI_MISSING=""
  for v in MARIADB:dcc-test/mariadb:1 POSTGRES:dcc-test/postgres:1 REDIS:dcc-test/redis:1; do
    var="DCC_TEST_${v%%:*}_IMAGE"; img="${!var:-${v#*:}}"
    docker image inspect "$img" >/dev/null 2>&1 || DBI_MISSING="$DBI_MISSING $img"
  done
  if live_block "1.4.0 · Copias contra Docker real: volumen idéntico tras restaurar (contenido, permisos y propietarios), en caliente marcado como no garantizado, deteniendo y volviendo a arrancar, auxiliar nunca arrancado ni restante, configuración sin secretos y contenedor restaurado sin arrancar, nunca sobre un volumen con datos, restauración selectiva, suma alterada rechazada, copia cancelada sin restos; MariaDB y PostgreSQL coherentes con escrituras simultáneas, Redis, Compose y configuración en carpetas nuevas; ningún secreto en el registro" dcc-engine live_backup 1; then
    if [ -n "$DBI_MISSING" ]; then
      skip "1.4.0 · Volcados reales de bases de datos: faltan las imágenes$DBI_MISSING (bash scripts/dev/build-db-test-images.sh, o DCC_TEST_MARIADB_IMAGE=mariadb:11 DCC_TEST_POSTGRES_IMAGE=postgres:17 DCC_TEST_REDIS_IMAGE=redis:7)"
    else
      grep -o "[a-z]*: [0-9]* transacciones en la copia.*" "$LAST_LOG" | sed 's/^/             /' | tee -a "$REPORT" >/dev/null
      grep -q "omitida" "$LAST_LOG" && bad "1.4.0 · Alguna prueba de copias no se ejecutó: $(grep -o 'omitida.*' "$LAST_LOG" | head -1)"
    fi
  fi
  run_step "1.4.0 · Tarea programada sin ventana (--run-backup, como el temporizador): copia, historial, retención solo de sus copias (la manual se queda), integridad, dos ejecuciones a la vez no se pisan, vista previa de restauración y borrado con confirmación por IPC" env DCC_DOCKER_TESTS=1 cargo test --locked -p docker-control-center --lib live_e4_scheduled_job_runs_headless_with_retention -- --nocapture --test-threads=1
  grep -q "omitida" "$LAST_LOG" && bad "La prueba de la tarea programada no se ejecutó"
  if live_block "1.5.0 · Monitorización contra Docker real: métricas iguales a «docker stats» (tolerancia de D-102), desconexión que falla enseguida y muestreo que sigue al volver, consumo con 50 contenedores y 3 servidores dentro del límite de D-107, alertas reales de CPU y OOM sin repetirse" dcc-engine live_monitor 1; then
    grep -o "consumo (.*" "$LAST_LOG" | sed 's/^/             /' | tee -a "$REPORT" >/dev/null
    grep -o "monitorización: cpu=.*" "$LAST_LOG" | sed 's/^/             /' | tee -a "$REPORT" >/dev/null
    grep -q "omitida: " "$LAST_LOG" && bad "1.5.0 · Alguna prueba de la monitorización no se ejecutó: $(grep -o 'omitida: .*' "$LAST_LOG" | head -1)"
  fi
  run_step "1.5.0 · Monitorización por IPC: muestras en vivo, minutos en el disco y alertas por eventos de Docker (caída con error, reinicio) sin repetir el aviso" env DCC_DOCKER_TESTS=1 cargo test --locked -p docker-control-center --lib live_e5_monitor_samples_minutes_and_event_alerts -- --nocapture --test-threads=1
  grep -q "omitida" "$LAST_LOG" && bad "La prueba IPC de la monitorización no se ejecutó"
  run_step "1.6.0 · Integración por IPC contra Docker real: la auditoría, la monitorización y una copia reconocen el mismo contenedor por su id; la copia guarda el id; la tarea que lo incluye; otro contenedor sin copia; al recrearlo, solo por nombre; los enlaces no se registran" env DCC_DOCKER_TESTS=1 cargo test --locked -p docker-control-center --lib live_e6_audit_links_monitoring_and_backups_by_container_id -- --nocapture --test-threads=1
  grep -q "omitida" "$LAST_LOG" && bad "La prueba IPC de la integración no se ejecutó"
  if live_block "Aplicación Tauri: IPC contra Docker real (incluye 0.9.1: varias terminales por IPC con canales reales y cierre global)" docker-control-center - 1; then
    while IFS= read -r l; do
      case "$l" in
        *Compose*) bad "Prueba IPC de Compose sin ejecutar: ${l#*omitida: }" ;;
        *) skip "Prueba IPC real sin aplicar: ${l#*omitida: }" ;;
      esac
    done < <(grep -o "omitida: .*" "$LAST_LOG" | sort -u)
  fi
  unset DCC_DOCKER_TESTS DCC_DOCKER_SOCKET
else
  bad "Pruebas contra Docker real NO ejecutadas: no hay conexión con Docker (son obligatorias)"
fi
say ""
say "5.4 Administración remota por SSH contra servidores reales (0.10.0)"
# Procesos ssh de sesiones que ya existían (por ejemplo, la aplicación abierta
# con servidores conectados): no cuentan como restos de las pruebas.
ssh_session_pids() { pgrep -u "$(id -u)" -f 'ControlPath=.*/c-[0-9]+-' 2>/dev/null | sort; }
SSH_BEFORE=$(ssh_session_pids)
if [ -z "$SSH_LAB" ]; then
  skip "Pruebas reales por SSH: no se indicó un laboratorio (--ssh-lab). La administración remota está validada (0.10.0); sus pruebas sin servidores se ejecutan en 5.2."
elif [ -z "$SOCK" ]; then
  bad "Pruebas reales por SSH NO ejecutadas: no hay conexión con el Docker local"
else
  say "  Laboratorio: $SSH_LAB ($(lab_count) servidores)"
  for ((i=0; i<$(lab_count); i++)); do
    if V=$(lab_docker $i version --format '{{.Server.Version}} (API {{.Server.APIVersion}})' 2>"$LOGDIR/lab-$i.err"); then
      ok "Servidor $(lab_get servers.$i.name) ($(lab_get servers.$i.user)@$(lab_get servers.$i.host):$(lab_get servers.$i.port)): Docker $V"
    else
      bad "Servidor $(lab_get servers.$i.name) del laboratorio no responde ($LOGDIR/lab-$i.err)"
    fi
  done
  run_step "Ayudante de contraseñas de las pruebas (dcc-askpass)" cargo build --locked -p dcc-engine --bin dcc-askpass
  export DCC_DOCKER_TESTS=1 DCC_DOCKER_SOCKET="$SOCK" DCC_SSH_LAB="$SSH_LAB"
  # Secuencial: varias pruebas cuentan los procesos ssh que quedan abiertos.
  live_block "SSH: huella en el primer contacto y solo la confirmada, identidad cambiada rechazada sin reintentos, ~/.ssh/known_hosts nunca modificado, clave/frase/contraseña y sus errores, problemas de Docker en el servidor distinguidos, tres servidores aislados, conexión perdida y recuperada sin restos, CLI remoto, sesiones huérfanas, Compose en el servidor con sus archivos, terminal SSH del servidor, carpetas vinculadas del servidor (D-075–D-081)" dcc-engine live_ssh 1
  grep -q "omitida:" "$LAST_LOG" && bad "Pruebas live_ssh sin ejecutar: $(grep -m1 -o 'omitida: .*' "$LAST_LOG")"
  run_step "1.1.0 · DCC-SEC-02 ProxyJump real: huella del intermedio y después la del destino (las dos en el known_hosts de la aplicación), intermedio con otra identidad rechazado, intermedio sin clave rechazado sin leer el secreto del destino, y el registro del intermedio no recibe ninguna contraseña (tampoco con la sesión real)" cargo test --locked -p dcc-engine --test live_ssh proxy_jump_checks_both_identities_and_never_shares_the_target_secret -- --nocapture --test-threads=1
  run_step "1.2.0 · DCC-SEC-07 Servidor real: sin permiso, la configuración de Docker de este equipo ni se consulta; con permiso, sí" cargo test --locked -p dcc-engine --test live_ssh local_registry_credentials_reach_a_server_only_with_consent -- --nocapture --test-threads=1
  run_step "1.2.0 · DCC-SEC-06/13 Servidor real: /etc/passwd, ~/.ssh y un enlace a /etc rechazados; guardar por un enlace legítimo conserva el enlace" cargo test --locked -p dcc-engine --test live_ssh compose_runs_on_the_server_with_server_files -- --nocapture --test-threads=1
  run_step "1.3.0 · Auditoría de un servidor real por SSH: lee sus contenedores, encuentra lo peligroso, no analiza vulnerabilidades y no cambia nada" cargo test --locked -p dcc-engine --test live_ssh audit_of_a_remote_server_reads_its_containers_only -- --nocapture --test-threads=1
  DCC_TEST_ASKPASS="$ROOT/target/debug/dcc-askpass"
  if run_step "IPC como la interfaz: equipo local y tres servidores a la vez (clave y contraseña sin llavero), cada operación en su servidor, vista general, desconexión sin procesos y eliminación de un perfil sin tocar el servidor" env DCC_TEST_ASKPASS="$DCC_TEST_ASKPASS" cargo test --locked -p docker-control-center --lib live_three_remote_servers_through_ipc -- --nocapture --test-threads=1; then
    grep -q "omitida" "$LAST_LOG" && bad "La prueba IPC con servidores remotos no se ejecutó"
  fi
  if [ $KEYRING_OK -eq 1 ]; then
    if run_step "IPC con llavero real temporal: la contraseña se guarda en el llavero y se vuelve a conectar sin escribirla" "$KR_DIR/run.sh" "$KR_DIR" env DCC_KEYRING_TEST=1 DCC_TEST_ASKPASS="$DCC_TEST_ASKPASS" DCC_DOCKER_TESTS=1 DCC_SSH_LAB="$SSH_LAB" cargo test --locked -p docker-control-center --lib live_three_remote_servers_through_ipc -- --nocapture --test-threads=1; then
      grep -q "omitida" "$LAST_LOG" && bad "La prueba IPC con llavero no se ejecutó"
    fi
  else
    skip "IPC con llavero real: no hay llavero temporal disponible (ver 5.2)"
  fi
  # Las mismas pruebas de recursos que en local, ahora por SSH (servidor A del
  # laboratorio, que usa el mismo Docker que este equipo: así el registro de
  # peticiones sigue comprobando lo que llega a Docker).
  DCC_SSH_TEST="$(lab_get servers.0.user)@$(lab_get servers.0.host):$(lab_get servers.0.port)"
  DCC_SSH_TEST_KEY=$(lab_get key); DCC_SSH_TEST_KNOWN_HOSTS=$(lab_get knownHosts)
  export DCC_SSH_TEST DCC_SSH_TEST_KEY DCC_SSH_TEST_KNOWN_HOSTS
  say "  Pruebas de recursos por SSH en $DCC_SSH_TEST"
  live_block "Por SSH · Contenedores (Fase 3)" dcc-engine live_containers
  live_block "Por SSH · Imágenes (Fase 4)" dcc-engine live_images
  live_block "Por SSH · Redes (Fase 5)" dcc-engine live_networks
  live_block "Por SSH · Volúmenes y carpetas vinculadas (Fase 6)" dcc-engine live_volumes
  live_block "Por SSH · Terminales múltiples (0.9.1)" dcc-engine live_terminals 1
  live_block "Por SSH · Edición y recreación segura (0.9.2)" dcc-engine live_edit 1
  live_block "Por SSH · Eventos de Docker (Fase 8)" dcc-engine live_events
  live_block "Por SSH · Auditoría de seguridad (1.3.0)" dcc-engine live_audit 1
  live_block "Por SSH · Copias y restauración (1.4.0)" dcc-engine live_backup 1
  live_block "Por SSH · Monitorización (1.5.0): consumo y alertas (la comparación con la CLI y el corte simulado, solo en local)" dcc-engine live_monitor 1
  unset DCC_SSH_TEST DCC_SSH_TEST_KEY DCC_SSH_TEST_KNOWN_HOSTS DCC_DOCKER_TESTS DCC_DOCKER_SOCKET DCC_SSH_LAB
fi
[ -n "${KR_DIR:-}" ] && rm -rf "$KR_DIR"
# Ninguna prueba deja procesos ssh (sesiones maestras o canales) abiertos.
LEFT_SSH=$(comm -13 <(printf '%s\n' "$SSH_BEFORE" | grep .) <(ssh_session_pids))
if [ -z "$LEFT_SSH" ]; then ok "No queda ningún proceso ssh de las pruebas"
else bad "Quedan procesos ssh de las pruebas: $(for p in $LEFT_SSH; do ps -o pid=,args= -p "$p"; done | head -3)"; fi
say ""
say "5.5 Desinstalación real con APT (D-065)"
if [ -n "${DCC_TEST_CHROOT:-}" ] && [ "$(id -u)" -eq 0 ]; then
  run_step "Desinstalación real en un sistema Debian/Ubuntu de prueba ($DCC_TEST_CHROOT): instalación con APT, dependencia compartida, Docker y Compose protegidos, cancelación, desinstalación real" cargo test --locked -p docker-control-center --lib uninstall::tests::live_ -- --test-threads=1 --nocapture
  grep -q "SIN EJECUTAR" "$LAST_LOG" && bad "La prueba real de desinstalación no se ejecutó"
else
  skip "Desinstalación real con APT: necesita un sistema de prueba (chroot) y root. No cambia en la 1.0.0; se ejecutó en el entorno de desarrollo y la validaste en la 0.9.1."
fi
say ""

# ------------------------------------------------------------------ 6. app
say "6. Compilación y arranque de la aplicación"
run_step "Compilación de escritorio (debug)" npx tauri build --debug --no-bundle
BIN="$ROOT/target/debug/container-manage-center"
[ -x "$BIN" ] || BIN="$ROOT/target/debug/docker-control-center"
if [ -x "$BIN" ] && [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
  TMPCFG=$(mktemp -d)
  DCC_CONFIG="$TMPCFG/config.toml" DCC_LOG=info "$BIN" >"$LOGDIR/arranque.log" 2>&1 &
  APP=$!
  sleep 8
  if kill -0 "$APP" 2>/dev/null; then ok "La aplicación sigue en ejecución 8 s después de arrancar"; else bad "La aplicación terminó al arrancar ($LOGDIR/arranque.log)"; fi
  kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null
  rm -rf "$TMPCFG"
else
  skip "Arranque de la aplicación: no hay sesión gráfica"
fi
# El paquete debe declarar la glibc (y bibliotecas) que necesita de verdad (D-059).
deb_glibc_need() {
  local w need
  w=$(mktemp -d)
  dpkg-deb -x "$1" "$w"
  need=$(objdump -T "$w/usr/bin/container-manage-center" | grep -oE 'GLIBC_[0-9]+\.[0-9]+' | sed 's/GLIBC_//' | sort -Vu | tail -1)
  rm -rf "$w"
  printf '%s' "$need"
}
deb_libc6_dep() { dpkg-deb -f "$1" Depends | grep -oE 'libc6 \(>= [0-9.]+\)' | grep -oE '[0-9]+\.[0-9]+'; }
check_deb_depends() {
  local need dep
  need=$(deb_glibc_need "$1"); dep=$(deb_libc6_dep "$1")
  say "             $(basename "$1"): el binario exige glibc $need; Depends: $(dpkg-deb -f "$1" Depends)"
  [ -n "$dep" ] && [ -n "$need" ] && ver_ge "$dep" "$need"
}
# Metadatos y contenido de un paquete (Fase 10).
check_package() {
  local deb="$1" v files f missing="" w
  v=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
  if [ "$(dpkg-deb -f "$deb" Version)" = "$v" ]; then ok "$(basename "$deb"): versión $v"; else bad "$(basename "$deb"): versión $(dpkg-deb -f "$deb" Version), se esperaba $v"; fi
  files=$(dpkg-deb -c "$deb" | awk '{print $6}')
  local D=./usr/share/doc/container-manage-center
  for f in ./usr/bin/container-manage-center ./usr/bin/docker-control-center ./usr/share/applications/container-manage-center.desktop \
           ./usr/share/icons/hicolor/32x32/apps/container-manage-center.png ./usr/share/icons/hicolor/128x128/apps/container-manage-center.png \
           ./usr/share/icons/hicolor/256x256/apps/container-manage-center.png ./usr/share/icons/hicolor/512x512/apps/container-manage-center.png \
           $D/copyright $D/changelog.gz $D/LICENSE $D/THIRD_PARTY_NOTICES.md $D/LICENSE_OVERVIEW.md $D/COPYRIGHT.md \
           $D/DISTRIBUTION_COMPLIANCE.md $D/MIGRATION.md $D/INSTALL.md $D/USER_GUIDE.md $D/README.md; do
    printf '%s\n' "$files" | grep -qxF "$f" || missing="$missing $f"
  done
  if [ -z "$missing" ]; then
    ok "$(basename "$deb"): programa, ruta anterior, lanzador, iconos de 32 a 512 px en carpetas estándar, copyright, GPL, avisos de terceros, documentos legales, migración, changelog y documentación"
  else
    bad "$(basename "$deb"): faltan$missing"
  fi
  if printf '%s\n' "$files" | grep -q '@2'; then bad "$(basename "$deb"): iconos en carpetas no estándar (@2)"; fi
  # 1.7.0 (D-110): nombre, sustitución del paquete anterior y ruta antigua.
  if [ "$(dpkg-deb -f "$deb" Package)" = "container-manage-center" ]; then ok "$(basename "$deb"): Package: container-manage-center"; else bad "$(basename "$deb"): Package: $(dpkg-deb -f "$deb" Package)"; fi
  local rel
  for rel in Conflicts Replaces Provides; do
    if dpkg-deb -f "$deb" "$rel" | tr ',' '\n' | sed 's/ //g' | grep -qx 'docker-control-center'; then ok "$(basename "$deb"): $rel: docker-control-center"; else bad "$(basename "$deb"): falta $rel: docker-control-center ($(dpkg-deb -f "$deb" "$rel"))"; fi
  done
  if printf '%s\n' "$files" | grep -qE 'applications/docker-control-center\.desktop|share/doc/docker-control-center/|THIRD-PARTY\.md'; then bad "$(basename "$deb"): quedan archivos con el nombre anterior (lanzador, documentación o THIRD-PARTY.md)"; else ok "$(basename "$deb"): sin lanzador ni documentación con el nombre anterior"; fi
  if dpkg-deb -c "$deb" | grep -E ' \./usr/bin/docker-control-center$' | grep -q '^-rwxr-xr-x root/root'; then ok "$(basename "$deb"): /usr/bin/docker-control-center es 0755 root:root"; else bad "$(basename "$deb"): /usr/bin/docker-control-center no es 0755 root:root: $(dpkg-deb -c "$deb" | grep ' ./usr/bin/docker-control-center$')"; fi
  w=$(mktemp -d); dpkg-deb -x "$deb" "$w"
  if grep -qx 'exec /usr/bin/container-manage-center "\$@"' "$w/usr/bin/docker-control-center" && sh -n "$w/usr/bin/docker-control-center"; then ok "$(basename "$deb"): la ruta anterior abre /usr/bin/container-manage-center con los mismos argumentos"; else bad "$(basename "$deb"): /usr/bin/docker-control-center no abre el programa nuevo"; fi
  if grep -q '^Name=Container Manage Center$' "$w/usr/share/applications/container-manage-center.desktop" && grep -q '^Exec=container-manage-center' "$w/usr/share/applications/container-manage-center.desktop"; then ok "$(basename "$deb"): el lanzador se llama «Container Manage Center» y abre container-manage-center"; else bad "$(basename "$deb"): nombre o Exec del lanzador incorrectos"; fi
  if cmp -s "$w/usr/share/doc/container-manage-center/LICENSE" LICENSE && cmp -s "$w/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md" docs/legal/THIRD_PARTY_NOTICES.md \
     && grep -q '^License: GPL-3.0-or-later' "$w/usr/share/doc/container-manage-center/copyright" && grep -q 'Copyright: 2026 Sergio Alfaro Lorite' "$w/usr/share/doc/container-manage-center/copyright"; then
    ok "$(basename "$deb"): LICENSE, avisos de terceros y copyright (GPL-3.0-or-later, 2026 Sergio Alfaro Lorite) iguales a los del código"
  else
    bad "$(basename "$deb"): la licencia, los avisos o el copyright del paquete no coinciden con los del código"
  fi
  rm -rf "$w"
  if command -v desktop-file-validate >/dev/null; then
    w=$(mktemp -d); dpkg-deb -x "$deb" "$w"
    if desktop-file-validate "$w/usr/share/applications/container-manage-center.desktop" >"$LOGDIR/desktop-$(basename "$deb").log" 2>&1; then
      ok "$(basename "$deb"): lanzador .desktop válido (desktop-file-validate)"
    else
      bad "$(basename "$deb"): lanzador .desktop no válido ($LOGDIR/desktop-$(basename "$deb").log)"
    fi
    rm -rf "$w"
  fi
}
if [ $BUILD_DEB -eq 1 ]; then
  if run_step "Paquete .deb" npx tauri build --bundles deb; then
    DEB=$(ls -1t "$ROOT"/target/release/bundle/deb/*.deb | head -1)
    run_step "Dependencias reales del paquete (dpkg-shlibdeps)" bash scripts/deb-set-depends.sh "$DEB"
    if check_deb_depends "$DEB"; then ok "El paquete declara la versión de libc6 que necesita su binario"; else bad "El paquete no declara la versión de libc6 que necesita su binario"; fi
    if dpkg-deb -f "$DEB" Recommends | grep -qw openssh-client; then ok "El paquete recomienda openssh-client (administración remota)"; else bad "El paquete no recomienda openssh-client"; fi
    check_package "$DEB"
    say "             $DEB"
    # DCC-SEC-11: el binario de publicación no arranca como root ni con DCC_ALLOW_ROOT.
    REL_BIN="$ROOT/target/release/container-manage-center"
    [ -x "$REL_BIN" ] || REL_BIN="$ROOT/target/release/docker-control-center"
    if [ "$(id -u)" -eq 0 ] && [ -x "$REL_BIN" ]; then
      env DCC_ALLOW_ROOT=1 timeout 20 "$REL_BIN" >"$LOGDIR/release-root.log" 2>&1; rc=$?
      if [ $rc -eq 1 ] && grep -q "no debe ejecutarse como root" "$LOGDIR/release-root.log"; then
        ok "1.2.0 · DCC-SEC-11 El binario de publicación se niega a arrancar como root aunque se defina DCC_ALLOW_ROOT=1"
      else
        bad "1.2.0 · DCC-SEC-11 El binario de publicación arrancó como root con DCC_ALLOW_ROOT=1 (código $rc, $LOGDIR/release-root.log)"
      fi
    else
      skip "1.2.0 · DCC-SEC-11 con el binario de publicación: solo se puede probar como root (se prueba la regla en root_tests)"
    fi
    # DCC-SEC-09: install.sh comprueba SHA256SUMS (correcto, alterado, sin entrada).
    SUMDIR=$(mktemp -d "$LOGDIR/sums.XXXX")
    cp "$DEB" scripts/install.sh "$SUMDIR/"
    if bash scripts/release-sums.sh "$SUMDIR" >"$LOGDIR/sums.log" 2>&1 \
      && bash "$SUMDIR/install.sh" --check >"$LOGDIR/sums-ok.log" 2>&1 && grep -q "Suma SHA-256 correcta" "$LOGDIR/sums-ok.log"; then
      sed -i 's/^./0/' "$SUMDIR/SHA256SUMS"
      if ! bash "$SUMDIR/install.sh" --check >"$LOGDIR/sums-bad.log" 2>&1 && grep -q "no coincide" "$LOGDIR/sums-bad.log"; then
        printf 'otra-cosa\n' >"$SUMDIR/SHA256SUMS"
        if ! bash "$SUMDIR/install.sh" --check >"$LOGDIR/sums-missing.log" 2>&1 && grep -q "no aparece" "$LOGDIR/sums-missing.log"; then
          ok "1.2.0 · DCC-SEC-09 install.sh comprueba SHA256SUMS: suma correcta aceptada; alterada o sin entrada, no instala"
        else
          bad "1.2.0 · DCC-SEC-09 install.sh no rechaza un paquete que no está en SHA256SUMS ($LOGDIR/sums-missing.log)"
        fi
      else
        bad "1.2.0 · DCC-SEC-09 install.sh no rechaza una suma alterada ($LOGDIR/sums-bad.log)"
      fi
    else
      bad "1.2.0 · DCC-SEC-09 install.sh no acepta un SHA256SUMS correcto ($LOGDIR/sums-ok.log)"
    fi
    rm -rf "$SUMDIR"
  fi
fi
if [ $BUILD_COMPAT -eq 1 ]; then
  if run_step "Paquete compatible con Debian 12+ y Ubuntu 22.04+ (compilado en Ubuntu 22.04)" bash scripts/build-deb-compat.sh; then
    CDEB=$(ls -1t "$ROOT"/target/compat/*.deb | head -1)
    if dpkg-deb -f "$CDEB" Recommends | grep -qw openssh-client; then ok "El paquete compatible recomienda openssh-client"; else bad "El paquete compatible no recomienda openssh-client"; fi
    check_package "$CDEB"
    if check_deb_depends "$CDEB" && ver_ge 2.35 "$(deb_libc6_dep "$CDEB")"; then
      ok "El paquete compatible exige como mucho glibc 2.35 (Ubuntu 22.04; Debian 12 tiene 2.36)"
    else
      bad "El paquete compatible exige una glibc más nueva que la de Ubuntu 22.04"
    fi
    say "             $CDEB"
  fi
fi
say ""

if [ $BUILD_DEB -eq 1 ] || [ $BUILD_COMPAT -eq 1 ]; then
  # Sin instalar nada: comprueba el sistema y elige el paquete adecuado para esta glibc.
  if run_step "1.0.0 · Script de instalación en modo comprobación (elige el paquete para este sistema; no instala nada)" bash scripts/install.sh --check; then
    grep -E "Paquete elegido" "$LAST_LOG" | sed 's/^ */             /' | tee -a "$REPORT"
  fi
  say ""
fi
# ------------------------------------------------------------------ 7. recursos
say "7. Recursos de prueba"
cleanup_test_resources
if [ $HAVE_DOCKER_CLI -eq 1 ] && [ -n "$SOCK" ]; then
  LC=$(docker ps -aq --filter label=dcc.test=true 2>/dev/null | wc -l)
  LI=$(docker image ls -q --filter label=dcc.test=true 2>/dev/null | wc -l)
  LV=$( (docker volume ls -q --filter name=dcc-test-vol- 2>/dev/null; docker volume ls -q --filter label=dcc.test=true 2>/dev/null; docker volume ls -q 2>/dev/null | grep -E '^dcc-test-') | sort -u | wc -l)
  LN=$(docker network ls -q --filter label=dcc.test=true 2>/dev/null | wc -l)
  [ "$LC" -eq 0 ] && [ "$LI" -eq 0 ] && [ "$LV" -eq 0 ] && [ "$LN" -eq 0 ] && ok "No quedan contenedores, imágenes, volúmenes ni redes de prueba" || bad "Quedan $LC contenedores, $LI imágenes, $LV volúmenes y $LN redes de prueba"
else
  skip "Comprobación de recursos de prueba: sin acceso a Docker desde el CLI"
fi
if [ -n "$SSH_LAB" ]; then
  cleanup_lab_resources
  if printf '%s' "$LAB_LEFT" | grep -qE ':[1-9]|:[0-9]+/[1-9]'; then
    bad "Quedan recursos de prueba en el laboratorio (contenedores/redes):$LAB_LEFT"
  else
    ok "No quedan contenedores ni redes de prueba en los servidores del laboratorio ($LAB_LEFT )"
  fi
fi
say ""

say "RESULTADO AUTOMÁTICO: $PASS correctas, $FAIL fallos, $SKIP omitidas justificadamente, $WARN avisos"
if [ "$FAIL" -gt 0 ]; then
  say "Fallos:"; for f in "${FAILS[@]}"; do say "  - $f"; done
fi
if [ "$SKIP" -gt 0 ]; then
  say "Omitidas (no cuentan como superadas):"; for s in "${SKIPS[@]}"; do say "  - $s"; done
fi
if [ "$FAIL" -gt 0 ]; then
  say "VEREDICTO: 1.7.0 NO SUPERADA en la parte automática. No continúes con las comprobaciones manuales; envía este informe."
else
  say "VEREDICTO: parte automática superada. Falta la validación manual de la 1.7.0 (docs/validation/VALIDACION-1.7.0.md)."
fi
say ""
cat <<'EOF' | tee -a "$REPORT"
8. Validación manual de la 1.7.0 (solo si la parte automática está superada)
   Las fases anteriores ya están validadas: no se repiten. Solo lo nuevo, en
   Debian 13, con la 1.6.0 instalada y en uso antes de empezar:
       docs/validation/VALIDACION-1.7.0.md
   Apartados: identidad, paquete, instalación, actualización desde la 1.6.0,
   migración, persistencia, Legal y distribución. Tiene una hoja de resultados
   al final; envíala con este informe.
EOF
[ "$FAIL" -eq 0 ]
