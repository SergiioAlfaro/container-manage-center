#!/usr/bin/env bash
# Verificación de la actualización 0.10.0 (administración remota de servidores
# Docker por SSH) en Debian/Ubuntu. Repite además toda la regresión automática
# de las fases 1-9 y de las actualizaciones 0.9.1 y 0.9.2.
#
# Uso (desde la raíz del proyecto, con tu usuario normal, NO como root):
#   bash scripts/verify-update-0.10.0.sh                 # comprobaciones automáticas
#   bash scripts/verify-update-0.10.0.sh --deb           # además construye el paquete .deb
#   bash scripts/verify-update-0.10.0.sh --deb --compat  # y el paquete compatible con
#        Debian 12+/Ubuntu 22.04+ (compilado en un contenedor de Docker con
#        Ubuntu 22.04: descarga la imagen ubuntu:22.04 y ~700 MB de paquetes)
#   --net    además consulta un servidor HTTPS real (TLS con los certificados
#            del sistema) para probar la comprobación de actualizaciones
#   --ssh-lab ARCHIVO   además ejecuta las pruebas reales por SSH contra un
#            laboratorio de servidores (lab.json de scripts/dev/remote-lab.sh;
#            también con DCC_SSH_LAB=ARCHIVO). Ese laboratorio crea usuarios,
#            daemons de Docker y servidores SSH y SOLO se monta en una máquina
#            de pruebas desechable: no lo montes en tu equipo de trabajo. Sin
#            laboratorio, esas pruebas se listan como omitidas y la
#            administración remota se valida con las comprobaciones manuales.
#
# Requisitos: los de la Fase 1, acceso a Docker con tu usuario (grupo docker o
# Docker rootless), Docker Compose v2 o posterior (en Debian 13:
# sudo apt install docker-compose) y, para la administración remota, el
# cliente OpenSSH (paquete openssh-client). El script NO usa sudo, NO instala
# nada, NO cambia permisos ni grupos y NO toca tu ~/.ssh ni tu llavero: la
# prueba del llavero se hace en una sesión D-Bus propia con un llavero temporal
# (si gnome-keyring está instalado).
#
# Recursos de prueba: los mismos que en la 0.9.2 (contenedores, redes, imágenes
# y volúmenes con la etiqueta dcc.test=true o con nombre dcc-test-*; ver
# scripts/verify-update-0.9.2.sh). Nunca tocan recursos que no hayan creado.
# Con --ssh-lab se crean también en los servidores del laboratorio y se
# eliminan de ellos al terminar. Al final se comprueba que no queda ninguno y
# que no queda ningún proceso ssh de las pruebas.
#
# La prueba real de la desinstalación (D-065) necesita un sistema Debian/Ubuntu
# de prueba en un chroot y ejecutarse como root: NO se ejecuta en tu equipo (se
# lista como omitida). No cambia en la 0.10.0.
#
# Resultado: verify-update-0.10.0-report.txt y los registros completos de cada
# paso en target/verify-update-0.10.0/. Código de salida 0 solo si no hay
# ningún fallo obligatorio. Las omisiones justificadas se listan aparte.

set -u
cd "$(dirname "$0")/.." || exit 1
ROOT=$(pwd)
REPORT="$ROOT/verify-update-0.10.0-report.txt"
LOGDIR="$ROOT/target/verify-update-0.10.0"
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
# ya confirmada (StrictHostKeyChecking=yes) y sin tocar ~/.ssh. Ningún
# argumento procede de una entrada del usuario.
lab_docker() {
  local i="$1"; shift
  ssh -F /dev/null -o BatchMode=yes -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes \
      -o UserKnownHostsFile="$(lab_get knownHosts)" -o ConnectTimeout=10 \
      -i "$(lab_get key)" -p "$(lab_get servers.$i.port)" \
      -- "$(lab_get servers.$i.user)@$(lab_get servers.$i.host)" docker "$@"
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

say "Docker Control Center - verificación de la actualización 0.10.0 (versión $(grep -m1 '^version' Cargo.toml | cut -d'"' -f2))"
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
  say "VEREDICTO: ACTUALIZACIÓN 0.10.0 NO VERIFICADA (faltan requisitos)"
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
run_step "Carrera de estado de Docker 26 (prueba determinista, D-031)" cargo test --locked -p dcc-engine --test stale_list
run_step "Carrera de estado de Docker 26 en proyectos Compose (prueba determinista, D-056)" cargo test --locked -p dcc-engine --test stale_compose
run_step "Suscripción a eventos: corte de la conexión, rechazo y cancelación (prueba determinista, D-057)" cargo test --locked -p dcc-engine --test events_interrupted
run_step "Comprobación de versiones nuevas: servidor local real, errores, límites, redirecciones y fuentes no válidas (D-060)" cargo test --locked -p docker-control-center --lib updates::
run_step "Comandos de la Fase 9 por IPC: información del sistema, comprobación con servidor local, configuración [updates]" cargo test --locked -p docker-control-center --lib phase9_
run_step "0.9.1 · Registro de flujos: cerrar todos y esperar a que terminen (cierre de la aplicación, D-063)" cargo test --locked -p docker-control-center --lib streams::
run_step "0.9.1 · Terminal: marca del PID retirada de la salida, órdenes de cada shell (D-063)" cargo test --locked -p dcc-engine --lib streams::
run_step "0.9.1 · Desinstalación: dpkg, historial de APT (también rotado), simulación, dependencias inversas, clasificación, protección de Docker y del sistema, nombres, pkexec sin purge (D-065)" cargo test --locked -p docker-control-center --lib uninstall::tests
run_step "0.9.1 · Configuración: francés e italiano se guardan y se cargan" cargo test --locked -p dcc-store french_and_italian
run_step "0.9.2 · Configuración: los ocho idiomas en orden y el color principal se guardan y se cargan" cargo test --locked -p dcc-store language_order_and_accent_roundtrip
run_step "0.9.2 · Cambios de idioma y de color quedan en el panel de actividad (una vez, como operación de la aplicación)" cargo test --locked -p docker-control-center --lib language_and_accent_changes_are_recorded_once
run_step "0.9.2 · Edición: lectura, clasificación directo/recrear, secretos ocultos, validación, cuerpo de docker update y de recreación, redes, MAC, nombres de copia, puertos ocupados (D-070, D-071)" cargo test --locked -p dcc-engine --lib edit
run_step "0.9.2 · Restaurar una copia exige confirmación explícita también por IPC" cargo test --locked -p docker-control-center --lib backup_restore_requires_explicit_confirmation
run_step "0.10.0 · Perfiles de servidor: archivo propio con permisos 0600, sin secretos, entradas no válidas o repetidas descartadas, archivo dañado apartado (D-078)" cargo test --locked -p dcc-store servers::
run_step "0.10.0 · SSH: argumentos seguros (opciones antes del destino, -- y comillas por argumento), huellas, clasificación de errores, perfiles, askpass, sesiones huérfanas (D-075–D-077)" cargo test --locked -p dcc-engine --lib ssh::
run_step "0.10.0 · Órdenes en el servidor: argv remoto entrecomillado y orden equivalente ssh … -- …; archivos Compose del servidor sin tocar el equipo (D-080)" cargo test --locked -p dcc-engine --lib -- cli:: compose::remote_file
run_step "0.10.0 · Registro de servidores: alta, edición y eliminación aisladas, proyectos Compose por servidor, lastSeen no viene de la interfaz (D-078)" cargo test --locked -p docker-control-center --lib servers::tests
run_step "0.10.0 · Secretos: caché de sesión, sin llavero se informa y nunca se guarda en archivo (D-077)" cargo test --locked -p docker-control-center --lib secrets::
run_step "0.10.0 · Historial y eventos filtrados por servidor (D-079)" cargo test --locked -p docker-control-center --lib history_and_events_filter_by_server
run_step "0.10.0 · IPC: toda orden de Docker exige un servidor explícito; perfiles sin tocar nada más; la contraseña de una prueba nunca aparece; la terminal SSH solo para servidores remotos conocidos" cargo test --locked -p docker-control-center --lib -- docker_commands_are_bound_to_an_explicit_server server_profiles_are_managed_without_touching_anything_else a_password_test_never_leaks_the_secret server_shell_is_only_for_known_remote_servers
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
  if run_step "0.10.0 · Llavero real (Secret Service temporal): guardar, sustituir sin duplicar, leer y borrar una contraseña (D-077)" "$KR_DIR/run.sh" "$KR_DIR" env DCC_KEYRING_TEST=1 cargo test --locked -p docker-control-center --lib secrets::tests::real_keyring_store_load_delete -- --nocapture; then
    if grep -q "omitida" "$LAST_LOG"; then bad "La prueba del llavero no se ejecutó"; else KEYRING_OK=1; fi
  fi
else
  skip "Llavero real: faltan dbus-run-session o gnome-keyring-daemon (sudo apt install dbus gnome-keyring); lo cubre la comprobación manual R3"
fi
if [ $NET_TESTS -eq 1 ]; then
  run_step "Consulta HTTPS real con TLS y los certificados del sistema (registry.npmjs.org)" env DCC_NET_TESTS=1 cargo test --locked -p docker-control-center --lib updates::tests::real_https -- --nocapture
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
  skip "Pruebas reales por SSH: no se indicó un laboratorio (--ssh-lab). La administración remota se valida con las comprobaciones manuales R1–R10."
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
  skip "Desinstalación real con APT: necesita un sistema de prueba (chroot) y root. No cambia en la 0.10.0; se ejecutó en el entorno de desarrollo y la validaste en la 0.9.1."
fi
say ""

# ------------------------------------------------------------------ 6. app
say "6. Compilación y arranque de la aplicación"
run_step "Compilación de escritorio (debug)" npx tauri build --debug --no-bundle
BIN="$ROOT/target/debug/docker-control-center"
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
  need=$(objdump -T "$w/usr/bin/docker-control-center" | grep -oE 'GLIBC_[0-9]+\.[0-9]+' | sed 's/GLIBC_//' | sort -Vu | tail -1)
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
if [ $BUILD_DEB -eq 1 ]; then
  if run_step "Paquete .deb" npx tauri build --bundles deb; then
    DEB=$(ls -1t "$ROOT"/target/release/bundle/deb/*.deb | head -1)
    run_step "Dependencias reales del paquete (dpkg-shlibdeps)" bash scripts/deb-set-depends.sh "$DEB"
    if check_deb_depends "$DEB"; then ok "El paquete declara la versión de libc6 que necesita su binario"; else bad "El paquete no declara la versión de libc6 que necesita su binario"; fi
    if dpkg-deb -f "$DEB" Recommends | grep -qw openssh-client; then ok "El paquete recomienda openssh-client (administración remota)"; else bad "El paquete no recomienda openssh-client"; fi
    say "             $DEB"
  fi
fi
if [ $BUILD_COMPAT -eq 1 ]; then
  if run_step "Paquete compatible con Debian 12+ y Ubuntu 22.04+ (compilado en Ubuntu 22.04)" bash scripts/build-deb-compat.sh; then
    CDEB=$(ls -1t "$ROOT"/target/compat/*.deb | head -1)
    if dpkg-deb -f "$CDEB" Recommends | grep -qw openssh-client; then ok "El paquete compatible recomienda openssh-client"; else bad "El paquete compatible no recomienda openssh-client"; fi
    if check_deb_depends "$CDEB" && ver_ge 2.35 "$(deb_libc6_dep "$CDEB")"; then
      ok "El paquete compatible exige como mucho glibc 2.35 (Ubuntu 22.04; Debian 12 tiene 2.36)"
    else
      bad "El paquete compatible exige una glibc más nueva que la de Ubuntu 22.04"
    fi
    say "             $CDEB"
  fi
fi
say ""

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
  say "VEREDICTO: ACTUALIZACIÓN 0.10.0 NO SUPERADA en la parte automática. No continúes con las comprobaciones manuales; envía este informe."
else
  say "VEREDICTO: parte automática superada. Faltan las comprobaciones manuales R1–R10 para validar la actualización 0.10.0."
fi
say ""
cat <<'EOF' | tee -a "$REPORT"
8. Comprobaciones manuales en tu equipo (solo si la parte automática está superada)
   Instala el paquete (sudo apt install ./target/release/bundle/deb/*.deb) o arranca
   ./target/debug/docker-control-center. Solo se piden las comprobaciones de lo que
   cambia en la 0.10.0 (administración remota).

   Necesitas un servidor con Docker al que entres por SSH con tu clave (otro
   equipo, una máquina virtual o un servidor de pruebas). Requisitos mínimos del
   servidor (detalle en INSTALL.md): servidor OpenSSH, Docker Engine 20.10 o
   posterior con su CLI (`docker system dial-stdio`), un usuario con acceso a
   Docker (grupo docker o Docker rootless) y, para Compose, Docker Compose v2.
   La aplicación NO instala nada en el servidor ni cambia su configuración.
   Antes de empezar, en tu terminal (sustituye usuario@servidor):
     ssh usuario@servidor docker version      # funciona sin pedir nada raro
     ssh-keygen -lf <(ssh-keyscan -t ed25519 servidor 2>/dev/null)   # anota la huella
   y en el servidor, para R5:
     docker run -d --name dcc-r010-web --label dcc.manual010=1 nginx:alpine

   R1  Alta: Servidores → «Añadir servidor». Nombre, dirección, usuario, puerto
       22, autenticación «Clave privada» con la ruta de tu clave (no se copia: se
       guarda solo la ruta). «Probar conexión»: muestra la huella del servidor;
       compárala con la anotada y confírmala. Las siete comprobaciones (DNS,
       conexión SSH, identidad, autenticación, Docker, permisos, compatibilidad
       de la API) quedan en verde. Guarda.
   R2  Conexión y selector: «Conectar». El servidor pasa a «Conectado» con la
       versión de Docker, el sistema y la latencia. En el selector de servidor de
       la parte superior elige el servidor: Panel, Contenedores, Imágenes, Redes,
       Volúmenes y Compose muestran los recursos del servidor (dcc-r010-web
       aparece) y un distintivo indica que es remoto. Vuelve a «Este equipo»:
       dcc-r010-web no aparece.
   R3  Credenciales: edita el servidor y cambia a «Contraseña» (desaconsejada)
       o a una clave con frase. Al conectar se pide el secreto; si marcas
       «Recordar» se guarda en el llavero del sistema (Contraseñas y claves /
       Seahorse lo muestra como «Docker Control Center — nombre (…)») y nunca en
       ~/.config/docker-control-center/servers.toml (compruébalo con grep).
       «Olvidar credenciales» lo quita del llavero. Si tu sesión no tiene
       llavero, la aplicación lo dice y pide el secreto cada vez.
   R4  Huella cambiada: en Servidores → (servidor) → «Revisar identidad» se ve la
       huella guardada. (Opcional, en un servidor de pruebas) regenera su clave
       de host: al conectar aparece el aviso de posible suplantación, no se
       conecta, y solo se acepta la nueva huella con «Revisar identidad» y
       confirmación expresa.
   R5  Operación destructiva: en el servidor, detén y elimina dcc-r010-web desde
       la aplicación. Cada confirmación muestra el nombre y la dirección del
       servidor, el recurso y la acción. En el panel de actividad la entrada
       indica el servidor, «SSH» y la orden equivalente (docker --host ssh://…).
   R6  Terminales: abre una terminal de un contenedor del servidor y otra de un
       contenedor local, y «Shell SSH del servidor». Las tres funcionan a la
       vez y cada pestaña indica servidor y contenedor. Ciérralas: en el
       servidor no queda ningún proceso (ps -ef | grep -c "docker exec").
   R7  Compose: en el servidor crea /tmp/dcc-r010/compose.yaml (un servicio
       nginx:alpine con la etiqueta dcc.manual010=1). En Compose del servidor
       «Añadir proyecto» y escribe esa ruta (no se abre el selector de archivos
       de tu equipo). Editar y guardar el archivo muestra antes la ruta de
       destino en el servidor. Despliega, mira los registros y elimínalo.
   R8  Desconexión y fallos: corta la red del servidor (o detén su sshd) con la
       aplicación conectada: el servidor pasa a «Error de conexión» sin afectar a
       «Este equipo» ni a otros servidores; al volver, «Conectar» lo recupera.
       «Diagnóstico» distingue el problema (servidor inaccesible, SSH detenido,
       credenciales, huella, Docker detenido, permisos) y explica cómo
       resolverlo sin hacer cambios en el servidor.
   R9  Eliminar el perfil: Servidores → (servidor) → «Eliminar perfil». Pide
       confirmación e indica que no se borra nada del servidor. Después, en el
       servidor, docker ps -a, docker volume ls y docker image ls siguen igual.
   R10 Interfaz: la vista general de servidores, el selector, los diálogos y la
       ayuda (sección «Servidores remotos por SSH») se ven bien en tema claro y
       oscuro, con color verde azulado y morado, y en al menos dos idiomas.

Cuando termines (en el servidor):
  docker rm -f $(docker ps -aq --filter label=dcc.manual010=1); rm -rf /tmp/dcc-r010
EOF
[ "$FAIL" -eq 0 ]
