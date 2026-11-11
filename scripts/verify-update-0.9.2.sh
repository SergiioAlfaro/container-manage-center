#!/usr/bin/env bash
# Verificación de la actualización 0.9.2 (idiomas catalán, gallego, euskera y
# portugués; color principal morado; crear una imagen a partir de un contenedor;
# editar la configuración de un contenedor con recreación segura) en
# Debian/Ubuntu. Repite además toda la regresión automática de las fases 1-9 y
# de la actualización 0.9.1.
#
# Uso (desde la raíz del proyecto, con tu usuario normal, NO como root):
#   bash scripts/verify-update-0.9.2.sh                 # comprobaciones automáticas
#   bash scripts/verify-update-0.9.2.sh --deb           # además construye el paquete .deb
#   bash scripts/verify-update-0.9.2.sh --deb --compat  # y el paquete compatible con
#        Debian 12+/Ubuntu 22.04+ (compilado en un contenedor de Docker con
#        Ubuntu 22.04: descarga la imagen ubuntu:22.04 y ~700 MB de paquetes)
#   --net    además consulta un servidor HTTPS real (TLS con los certificados
#            del sistema) para probar la comprobación de actualizaciones
#
# Requisitos: los de la Fase 1, acceso a Docker con tu usuario (grupo docker o
# Docker rootless) y Docker Compose v2 o posterior (en Debian 13:
# sudo apt install docker-compose). El script NO usa sudo, NO instala nada y NO
# cambia permisos ni grupos.
#
# Recursos de prueba: los mismos que en la 0.9.1 (contenedores, redes, imágenes
# y volúmenes con la etiqueta dcc.test=true o con nombre dcc-test-vol-*; ver
# scripts/verify-update-0.9.1.sh). Las pruebas de edición (0.9.2) crean además
# contenedores, redes (10.213.0.0/16) e imágenes dcc-test/commit-* con la
# etiqueta dcc.test=true, y volúmenes con nombre dcc-test-*-<pid>-*. Nunca tocan
# contenedores, volúmenes ni imágenes que no hayan creado. Al terminar, pase lo
# que pase, se eliminan SOLO esos recursos y se comprueba que no queda ninguno.
#
# La prueba real de la desinstalación (D-065) necesita un sistema Debian/Ubuntu
# de prueba en un chroot y ejecutarse como root: NO se ejecuta en tu equipo (se
# lista como omitida). No cambia en la 0.9.2.
#
# Resultado: verify-update-0.9.2-report.txt. Código de salida 0 solo si no hay
# ningún fallo obligatorio. Las omisiones justificadas se listan aparte.

set -u
cd "$(dirname "$0")/.." || exit 1
ROOT=$(pwd)
REPORT="$ROOT/verify-update-0.9.2-report.txt"
LOGDIR="$ROOT/target/verify-update-0.9.2"
rm -rf "$LOGDIR"
mkdir -p "$LOGDIR"
: > "$REPORT"
BUILD_DEB=0; BUILD_COMPAT=0; NET_TESTS=0
for a in "$@"; do
  case "$a" in
    --deb) BUILD_DEB=1 ;;
    --compat) BUILD_COMPAT=1 ;;
    --net) NET_TESTS=1 ;;
    *) echo "Opción desconocida: $a (usa --deb, --compat o --net)"; exit 2 ;;
  esac
done

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
finish() {
  local rc=$?
  cleanup_test_resources
  exit $rc
}
trap finish EXIT
trap 'exit 130' INT TERM

say "Docker Control Center - verificación de la actualización 0.9.2 (versión $(grep -m1 '^version' Cargo.toml | cut -d'"' -f2))"
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
  say "VEREDICTO: ACTUALIZACIÓN 0.9.2 NO VERIFICADA (faltan requisitos)"
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
say "5.4 Desinstalación real con APT (D-065)"
if [ -n "${DCC_TEST_CHROOT:-}" ] && [ "$(id -u)" -eq 0 ]; then
  run_step "Desinstalación real en un sistema Debian/Ubuntu de prueba ($DCC_TEST_CHROOT): instalación con APT, dependencia compartida, Docker y Compose protegidos, cancelación, desinstalación real" cargo test --locked -p docker-control-center --lib uninstall::tests::live_ -- --test-threads=1 --nocapture
  grep -q "SIN EJECUTAR" "$LAST_LOG" && bad "La prueba real de desinstalación no se ejecutó"
else
  skip "Desinstalación real con APT: necesita un sistema de prueba (chroot) y root. No cambia en la 0.9.2; se ejecutó en el entorno de desarrollo y la validaste en la 0.9.1."
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
    say "             $DEB"
  fi
fi
if [ $BUILD_COMPAT -eq 1 ]; then
  if run_step "Paquete compatible con Debian 12+ y Ubuntu 22.04+ (compilado en Ubuntu 22.04)" bash scripts/build-deb-compat.sh; then
    CDEB=$(ls -1t "$ROOT"/target/compat/*.deb | head -1)
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
say ""

say "RESULTADO AUTOMÁTICO: $PASS correctas, $FAIL fallos, $SKIP omitidas justificadamente, $WARN avisos"
if [ "$FAIL" -gt 0 ]; then
  say "Fallos:"; for f in "${FAILS[@]}"; do say "  - $f"; done
fi
if [ "$SKIP" -gt 0 ]; then
  say "Omitidas (no cuentan como superadas):"; for s in "${SKIPS[@]}"; do say "  - $s"; done
fi
if [ "$FAIL" -gt 0 ]; then
  say "VEREDICTO: ACTUALIZACIÓN 0.9.2 NO SUPERADA en la parte automática. No continúes con las comprobaciones manuales; envía este informe."
else
  say "VEREDICTO: parte automática superada. Faltan las comprobaciones manuales M1–M10 para validar la actualización 0.9.2."
fi
say ""
cat <<'EOF' | tee -a "$REPORT"
8. Comprobaciones manuales en tu equipo (solo si la parte automática está superada)
   Instala el paquete (sudo apt install ./target/release/bundle/deb/*.deb) o arranca
   ./target/debug/docker-control-center. Solo se piden las comprobaciones de lo que
   cambia en la 0.9.2. Prepara en una terminal un contenedor con datos:
     docker volume create dcc-m92-datos
     mkdir -p /tmp/dcc-m92 && echo carpeta > /tmp/dcc-m92/f.txt
     docker run -d --name dcc-m92-web --label dcc.manual92=1 -p 18092:80 \
       -e MODO=prueba -e DB_PASSWORD=secreto -v dcc-m92-datos:/datos \
       -v /tmp/dcc-m92:/carpeta nginx:alpine
     docker exec dcc-m92-web sh -c 'echo volumen > /datos/v.txt; echo capa > /capa.txt'

   M1  Idiomas: Configuración → Apariencia → Idioma muestra, en este orden,
       English, Español, Català, Galego, Euskara, Français, Italiano, Português.
       Elige cada uno de los cuatro nuevos: cambia toda la interfaz (menú,
       Contenedores, Configuración, diálogos y la Ayuda), sin textos a medias ni
       caracteres extraños (ç, ã, l·l, ñ). Cierra y vuelve a abrir con uno de ellos:
       se mantiene. Vuelve a Español.
   M2  Color principal: Apariencia → Color principal → Morado. Cambia al momento
       (botones, pestaña y filas seleccionadas, foco con Tab, icono activo de la
       barra lateral, cursor de una terminal abierta) sin reiniciar. Repite en
       tema oscuro: el texto de los botones se lee bien. Cierra y abre: sigue
       morado. Vuelve a Verde azulado: es el color de siempre.
   M3  Crear imagen: en Contenedores, menú ⋯ de dcc-m92-web (o botón derecho en la
       fila) → «Crear imagen a partir de este contenedor». El diálogo indica que
       /datos y /carpeta no entran y que el contenedor está en ejecución (pausa
       marcada). Escribe repositorio «dcc-m92/web», etiqueta «v1», «Crear imagen».
       «Ver la imagen» abre su detalle en Imágenes. El contenedor sigue «En
       ejecución». En tu terminal:
         docker run --rm dcc-m92/web:v1 sh -c 'cat /capa.txt; ls /datos /carpeta'
       → «capa» y ambas carpetas vacías. Repite con el mismo nombre: avisa de que
       ya existe y no deja crear sin marcar «Usar este nombre…».
   M4  Editar (cambios directos): ⋯ → «Editar configuración». Hay 7 pestañas;
       «Avanzado» es de solo lectura. Cambia el nombre a dcc-m92-web2, la política
       de reinicio a «Siempre» y en Recursos CPU 0.5: abajo dice «Se aplican
       directamente». «Revisar cambios» → tabla Actual/Nuevo/Cómo se aplica →
       «Aplicar cambios». docker inspect dcc-m92-web2 muestra el mismo ID, sigue en
       ejecución y NanoCpus=500000000.
   M5  Editar (recreación): en Variables, la de DB_PASSWORD aparece oculta. Cambia
       MODO a «real»; en Puertos cambia 18092 por 18093. «Revisar cambios»: pide
       recrear, explica los pasos, avisa de la interrupción y de los archivos fuera
       de volúmenes (marca la casilla) y pide confirmación. «Recrear contenedor».
       Comprueba: curl -s localhost:18093 responde; docker exec dcc-m92-web2 cat
       /datos/v.txt /carpeta/f.txt → «volumen» y «carpeta»; docker ps -a muestra
       dcc-m92-web2-dcc-backup-… detenido; la imagen dcc-m92/web:v1 y el volumen
       siguen existiendo.
   M6  Puerto ocupado: (a) Arranca otro contenedor en 18094 (docker run -d --name
       dcc-m92-ocupa --label dcc.manual92=1 -p 18094:80 nginx:alpine), edita
       dcc-m92-web2 y pon 18094: la revisión lo señala como error y no deja
       aplicar. (b) Para probar la vuelta atrás, ocupa el puerto 18095 con un
       programa que no sea Docker (python3 -m http.server 18095 en otra
       terminal) y pon 18095: Docker no puede iniciar el contenedor nuevo y el
       resultado dice que se volvió al original; dcc-m92-web2 sigue en ejecución
       en 18093 con sus datos y no queda ningún contenedor de más. Cierra el
       servidor de python.
   M7  Copias: General → Copias de seguridad → «Restaurar» la copia (pide
       confirmación): la copia vuelve a llamarse dcc-m92-web2 y el actual queda
       apartado como …-dcc-replaced-…, sin borrarse. «Eliminar copia» no toca
       el volumen dcc-m92-datos.
   M8  Redes: en la pestaña Red, conectar a otra red y desconectarla se aplica al
       momento; si es la única red, avisa antes de desconectar.
   M9  Panel de actividad: aparecen «Cambiar idioma» y «Cambiar color principal»
       (distintivo APP), la imagen (docker commit), rename, update, la recreación
       con sus pasos y los errores de M6. En ninguna entrada aparece «secreto».
   M10 Teclado y temas: el menú ⋯ se abre con Intro, se recorre con flechas y se
       cierra con Escape; las pestañas de edición y el selector de color se usan
       con Tab y flechas, en tema claro y oscuro.

Cuando termines:
  docker rm -f $(docker ps -aq --filter label=dcc.manual92=1)
  docker image rm dcc-m92/web:v1; docker volume rm dcc-m92-datos; rm -rf /tmp/dcc-m92
EOF
[ "$FAIL" -eq 0 ]
