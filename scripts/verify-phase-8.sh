#!/usr/bin/env bash
# Verificación de la Fase 8 (actividad, eventos y monitorización) en Debian/Ubuntu.
#
# Uso (desde la raíz del proyecto, con tu usuario normal, NO como root):
#   bash scripts/verify-phase-8.sh          # comprobaciones automáticas
#   bash scripts/verify-phase-8.sh --deb    # además construye el paquete .deb
#
# Requisitos: los de la Fase 1, acceso a Docker con tu usuario (grupo docker o
# Docker rootless) y Docker Compose v2 o posterior (en Debian 13:
# sudo apt install docker-compose). El script NO usa sudo, NO instala nada y NO
# cambia permisos ni grupos.
#
# Recursos de prueba: las pruebas crean contenedores con la etiqueta
# dcc.test=true, volúmenes llamados dcc-test-vol-* y una imagen local
# dcc-test/sleeper:2 (etiqueta dcc.test=true), compilada en el momento sin
# descargar nada. Las pruebas de descarga levantan un registro en 127.0.0.1 con
# imágenes generadas en el momento (también con dcc.test=true). Las pruebas de
# redes crean redes dcc-test-* con la etiqueta dcc.test=true y subredes libres en
# 10.213.0.0/16. Las de volúmenes crean volúmenes dcc-test-* (y anónimos) con la
# etiqueta dcc.test=true, una imagen dcc-test/volumen-datos-* y carpetas
# temporales. Las de eventos crean y eliminan un volumen y un contenedor
# dcc-test-ev-* y dcc-test-ipc8-*. Las de Compose despliegan proyectos dcc-test-c7-* y dcc-test-ipc7-*
# desde carpetas temporales: sus contenedores, redes, volúmenes e imágenes
# construidas llevan la etiqueta dcc.test=true. Al terminar, pase lo que pase, se
# eliminan SOLO esos recursos y se comprueba que no queda ninguno.
#
# Resultado: verify-phase-8-report.txt. Código de salida 0 solo si no hay
# ningún fallo obligatorio. Las omisiones justificadas se listan aparte.

set -u
cd "$(dirname "$0")/.." || exit 1
ROOT=$(pwd)
REPORT="$ROOT/verify-phase-8-report.txt"
LOGDIR="$ROOT/target/verify-phase-8"
rm -rf "$LOGDIR"
mkdir -p "$LOGDIR"
: > "$REPORT"
BUILD_DEB=0
[ "${1:-}" = "--deb" ] && BUILD_DEB=1

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
  v=$( (docker volume ls -q --filter name=dcc-test-vol- 2>/dev/null; docker volume ls -q --filter label=dcc.test=true 2>/dev/null) | sort -u)
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

say "Docker Control Center - verificación de la Fase 8 (versión $(grep -m1 '^version' Cargo.toml | cut -d'"' -f2))"
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
say ""
if [ "$FAIL" -gt 0 ]; then
  say "Faltan requisitos. Corrígelos y vuelve a ejecutar el script."
  say "RESULTADO: $PASS correctas, $FAIL fallos, $SKIP omitidas, $WARN avisos"
  say "VEREDICTO: FASE 8 NO VERIFICADA (faltan requisitos)"
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
  live_block "Eventos de Docker: suscripción en tiempo real (orden, atributos, registro), consulta con filtros de fecha, tipo, acción y recurso, consultas no válidas (Fase 8)" dcc-engine live_events
  if live_block "Aplicación Tauri: IPC contra Docker real" docker-control-center - 1; then
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
if [ $BUILD_DEB -eq 1 ]; then
  run_step "Paquete .deb" npx tauri build --bundles deb && ls -1 "$ROOT"/target/release/bundle/deb/*.deb 2>/dev/null | sed 's/^/             /' | tee -a "$REPORT"
fi
say ""

# ------------------------------------------------------------------ 7. recursos
say "7. Recursos de prueba"
cleanup_test_resources
if [ $HAVE_DOCKER_CLI -eq 1 ] && [ -n "$SOCK" ]; then
  LC=$(docker ps -aq --filter label=dcc.test=true 2>/dev/null | wc -l)
  LI=$(docker image ls -q --filter label=dcc.test=true 2>/dev/null | wc -l)
  LV=$( (docker volume ls -q --filter name=dcc-test-vol- 2>/dev/null; docker volume ls -q --filter label=dcc.test=true 2>/dev/null) | sort -u | wc -l)
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
  say "VEREDICTO: FASE 8 NO SUPERADA en la parte automática. No continúes con las comprobaciones manuales; envía este informe."
else
  say "VEREDICTO: parte automática superada. Faltan las comprobaciones manuales M1–M15 para validar la Fase 8."
fi
say ""
cat <<'EOF' | tee -a "$REPORT"
8. Comprobaciones manuales en tu equipo (solo si la parte automática está superada)
   Instala el paquete (sudo apt install ./target/release/bundle/deb/*.deb) o arranca
   ./target/debug/docker-control-center. Usa una terminal al lado de la aplicación.
   El historial y los eventos se guardan en ~/.local/state/docker-control-center/.

   Eventos en directo y actualización automática
   [ ] M1  Eventos y registros › En directo. En la terminal:
             docker run -d --name dcc-manual-ev alpine:3.20 sleep 600
           En menos de un segundo aparecen (los más recientes arriba) los eventos
           pull (si no tenías la imagen), create y start de dcc-manual-ev; al pulsar
           su nombre se abre el detalle del contenedor; la flecha muestra los atributos.
   [ ] M2  Con Contenedores abierto: docker stop dcc-manual-ev → su estado cambia solo,
           sin pulsar Actualizar. Igual en Volúmenes (docker volume create dcc-manual-vol),
           Redes (docker network create dcc-manual-net), Imágenes
           (docker tag alpine:3.20 dcc-manual:1) y en los contadores del Dashboard.
   [ ] M3  Con Docker Compose abierto: cd ~ && mkdir -p dcc-manual-ev8 && cd dcc-manual-ev8 &&
           printf 'services:\n  app:\n    image: alpine:3.20\n    command: ["sleep","600"]\n' > compose.yaml &&
           docker compose up -d → el proyecto dcc-manual-ev8 aparece y pasa a "En marcha"
           sin actualizar.
   [ ] M4  En directo: el filtro Volumen deja solo los de volúmenes; la búsqueda filtra
           por nombre o acción. Pausar y crear algo (docker volume create dcc-manual-v2):
           la lista no cambia y el botón dice "Reanudar (N nuevos)"; al reanudar aparecen.
   [ ] M5  Panel de actividad: hay una operación "Suscripción a los eventos de Docker"
           correcta (GET /events, docker events) por conexión; no queda "en curso" para
           siempre (el contador del botón Actividad vuelve a 0).
   [ ] M6  (Opcional, requiere sudo) sudo systemctl restart docker: la aplicación muestra
           la desconexión, se reconecta sola y vuelve a recibir eventos (M1 funciona de
           nuevo). En el panel aparece la suscripción interrumpida (error) y la nueva.

   Eventos guardados y consulta a Docker
   [ ] M7  Cierra y vuelve a abrir la aplicación. Guardados: siguen los eventos de antes.
           Filtra por Desde/Hasta (última hora), tipo Contenedor y acciones "start, die":
           solo quedan esos.
   [ ] M8  Consultar a Docker: tipo Contenedor, recurso dcc-manual-ev, Consultar → sus
           eventos y el comando "docker events --since … --until … --filter …"; ese comando
           en la terminal devuelve los mismos eventos.

   Historial de operaciones
   [ ] M9  Inicia y detén dcc-manual-ev desde la aplicación. Historial de operaciones:
           aparecen arriba con su estado y duración; al desplegar una se ve la petición
           ejecutada (POST /containers/…/stop). Tras reiniciar la aplicación siguen ahí
           con "Sesión anterior".
   [ ] M10 Crea en la aplicación un contenedor alpine:3.20 con la variable de entorno
           CLAVE=secreto-manual-8. El panel de actividad de la sesión la muestra en el
           cuerpo de la petición, pero
             grep -c secreto-manual-8 ~/.local/state/docker-control-center/*.jsonl
           da 0 en los dos archivos.
   [ ] M11 ls -la ~/.local/state/docker-control-center/ → la carpeta es drwx------ y los
           archivos historial.jsonl y eventos.jsonl son -rw-------.
   [ ] M12 Configuración › Actividad › desactiva "Guardar el historial entre sesiones":
           las operaciones nuevas no se añaden al historial ni los eventos a Guardados
           (ambas pestañas lo avisan). Vuelve a activarla.
   [ ] M13 "Borrar historial" y "Borrar eventos guardados" piden confirmación y vacían
           los archivos; el panel de actividad de la sesión no cambia.

   Otros y regresiones
   [ ] M14 En English y con tema oscuro, Eventos y registros aparece en inglés y legible.
   [ ] M15 Contenedores, Imágenes, Redes, Volúmenes, Directorios vinculados y Docker
           Compose siguen funcionando. Con Docker detenido, En directo indica
           "Sin conexión con Docker" y Guardados e Historial se siguen pudiendo consultar.

Cuando termines:
  docker rm -f dcc-manual-ev; docker volume rm dcc-manual-vol dcc-manual-v2
  docker network rm dcc-manual-net; docker rmi dcc-manual:1
  (cd ~/dcc-manual-ev8 && docker compose down); rm -rf ~/dcc-manual-ev8
EOF

[ "$FAIL" -eq 0 ]
