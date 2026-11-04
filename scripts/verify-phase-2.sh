#!/usr/bin/env bash
# Verificación de la Fase 2 (integración con Docker Engine) en Debian/Ubuntu.
#
# Uso (desde la raíz del proyecto, con tu usuario normal, NO como root):
#   bash scripts/verify-phase-2.sh          # comprobaciones automáticas
#   bash scripts/verify-phase-2.sh --deb    # además construye el paquete .deb
#
# Requisitos: los de la Fase 1 y acceso a Docker con tu usuario (grupo docker o
# Docker rootless). El script NO usa sudo, NO cambia permisos ni grupos y NO
# modifica tu Docker salvo para lo indicado aquí: las pruebas de integración
# crean y eliminan contenedores etiquetados dcc.test=true y una imagen local
# dcc-test/sleeper:1 (compilada en el momento, sin descargar nada de Internet).
#
# Deja un informe en verify-phase-2-report.txt para enviarlo.

set -u
cd "$(dirname "$0")/.." || exit 1
ROOT=$(pwd)
REPORT="$ROOT/verify-phase-2-report.txt"
LOGDIR="$ROOT/target/verify-phase-2"
mkdir -p "$LOGDIR"
: > "$REPORT"
BUILD_DEB=0
[ "${1:-}" = "--deb" ] && BUILD_DEB=1

PASS=0; FAIL=0; WARN=0
say()  { printf '%s\n' "$*" | tee -a "$REPORT"; }
ok()   { PASS=$((PASS+1)); say "  [OK]    $*"; }
bad()  { FAIL=$((FAIL+1)); say "  [FALLO] $*"; }
warn() { WARN=$((WARN+1)); say "  [AVISO] $*"; }
ver_ge() { [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" = "$2" ]; }

run_step() {
  local name="$1"; shift
  local log
  log="$LOGDIR/$(echo "$name" | tr ' /()' '____').log"
  printf '  ... %s\n' "$name"
  if "$@" >"$log" 2>&1; then ok "$name"; else bad "$name (ver $log)"; tail -30 "$log" | sed 's/^/        /' | tee -a "$REPORT" >/dev/null; fi
  LAST_LOG="$log"
}

say "Docker Control Center - verificación de la Fase 2"
say "Fecha: $(date -Is)"
say "Proyecto: $ROOT"
say ""

# ------------------------------------------------------------------ 1. sistema
say "1. Sistema y herramientas"
if [ "$(id -u)" -eq 0 ]; then
  bad "Estás ejecutando el script como root. Ejecútalo con tu usuario normal."
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
command -v rustc >/dev/null && ok "Rust $(rustc --version | awk '{print $2}')" || bad "Rust no está instalado"
command -v node >/dev/null && ok "Node.js $(node --version)" || bad "Node.js no está instalado"
command -v cc >/dev/null && ok "Compilador C (para la imagen de prueba local)" || bad "Falta un compilador C: sudo apt install build-essential"
[ -f /usr/lib/x86_64-linux-gnu/libc.a ] || [ -n "$(find /usr/lib -name libc.a 2>/dev/null | head -1)" ] && ok "libc estática (libc6-dev)" || bad "Falta libc estática: sudo apt install libc6-dev"
say ""
if [ "$FAIL" -gt 0 ]; then
  say "Faltan requisitos. Corrígelos y vuelve a ejecutar el script."
  say "RESULTADO: $PASS correctas, $FAIL fallos, $WARN avisos"
  exit 1
fi

# ------------------------------------------------------------------ 2. Docker
say "2. Docker en este equipo (solo lectura)"
if command -v docker >/dev/null; then
  ok "CLI de Docker: $(command -v docker)"
  if DV=$(docker version --format '{{.Server.Version}} (API {{.Server.APIVersion}})' 2>/dev/null) && [ -n "$DV" ]; then
    ok "Docker Engine $DV"
  else
    warn "El CLI de Docker no puede hablar con el daemon: $(docker info 2>&1 | grep -iE 'permission denied|cannot connect|error' | head -1)"
  fi
  CV=$(docker compose version --short 2>&1) && ok "Docker Compose $CV" || warn "Docker Compose no disponible: $CV"
else
  warn "CLI de Docker no encontrado (la app no lo necesita salvo para Compose)"
fi
say "  DOCKER_HOST=${DOCKER_HOST:-(no definido)}   contexto: $(docker context show 2>/dev/null || echo '?')"
say ""

say "3. Dependencias y compilación"
run_step "npm ci" npm ci --no-audit --no-fund
run_step "Herramienta de diagnóstico (dcc-diagnose)" cargo build --locked -p dcc-engine --bin dcc-diagnose
DIAG="$ROOT/target/debug/dcc-diagnose"
say ""

# ------------------------------------------------------------------ 4. conexión real
say "4. Conexión real con la detección de la aplicación"
if [ -x "$DIAG" ]; then
  "$DIAG" > "$LOGDIR/diagnose.txt" 2>&1
  DRC=$?
  sed 's/^/        /' "$LOGDIR/diagnose.txt" | tee -a "$REPORT" >/dev/null
  if [ $DRC -eq 0 ]; then
    ok "La aplicación conecta con Docker ($(head -1 "$LOGDIR/diagnose.txt"))"
    SOCK=$(grep -m1 "Socket:" "$LOGDIR/diagnose.txt" | awk '{print $2}')
  else
    bad "La aplicación no conecta con Docker con tu usuario. Revisa el diagnóstico anterior."
    SOCK=""
  fi
  "$DIAG" --socket /nonexistent/dcc-test.sock > "$LOGDIR/diagnose-missing.txt" 2>&1
  [ $? -eq 2 ] && grep -q "NoSocket" "$LOGDIR/diagnose-missing.txt" && ok "Socket inexistente diagnosticado como NoSocket" || bad "Diagnóstico de socket inexistente incorrecto (ver $LOGDIR/diagnose-missing.txt)"
  "$DIAG" --socket relativa.sock > "$LOGDIR/diagnose-invalid.txt" 2>&1
  grep -q "InvalidPath" "$LOGDIR/diagnose-invalid.txt" && ok "Ruta relativa rechazada (InvalidPath)" || bad "La ruta relativa no se rechazó"
  if [ -z "$SOCK" ]; then
    warn "Prueba sin red omitida: no hay conexión con Docker"
  elif command -v unshare >/dev/null && unshare -rn true 2>/dev/null; then
    if unshare -rn "$DIAG" --socket "$SOCK" > "$LOGDIR/diagnose-offline.txt" 2>&1; then
      ok "Funciona sin red (espacio de nombres de red vacío) con el Docker local"
    else
      bad "Sin red no conecta con el Docker local (ver $LOGDIR/diagnose-offline.txt)"
    fi
  else
    warn "unshare -rn no disponible: prueba sin red pendiente (hazla manualmente, M9)"
  fi
else
  bad "No se pudo compilar dcc-diagnose"
  SOCK=""
fi
say ""

# ------------------------------------------------------------------ 5. pruebas automáticas
say "5. Pruebas automatizadas"
run_step "Pruebas Rust deterministas (sin Docker)" cargo test --workspace --locked
grep -h "^test result" "$LAST_LOG" | awk '{s+=$4; f+=$6} END {printf "        %d correctas, %d fallidas (incluye 12 pruebas contra Docker real que en este paso se omiten; se ejecutan a continuación)\n", s, f}' | tee -a "$REPORT"
if [ -n "$SOCK" ]; then
  export DCC_DOCKER_TESTS=1 DCC_DOCKER_SOCKET="$SOCK"
  run_step "Pruebas contra Docker real (motor)" cargo test --locked -p dcc-engine --test live_docker -- --nocapture --test-threads=2
  grep -h "^test result" "$LAST_LOG" | sed 's/^/        /' | tee -a "$REPORT"
  OMIT=$(grep -c "omitida" "$LAST_LOG" || true)
  say "        Omitidas (no aplican en este equipo): $OMIT"
  grep "omitida" "$LAST_LOG" | sed 's/^/          /' | tee -a "$REPORT" >/dev/null
  run_step "Prueba contra Docker real (aplicación Tauri)" cargo test --locked -p docker-control-center live_app -- --nocapture
  unset DCC_DOCKER_TESTS DCC_DOCKER_SOCKET
  LEFT=$(docker ps -a --filter label=dcc.test=true -q 2>/dev/null | wc -l)
  [ "$LEFT" -eq 0 ] && ok "Las pruebas no dejaron contenedores de prueba" || warn "Quedan $LEFT contenedores con la etiqueta dcc.test=true"
else
  bad "Pruebas contra Docker real NO ejecutadas: no hay conexión con Docker"
fi
run_step "Clippy sin avisos" cargo clippy --workspace --all-targets --locked -- -D warnings
run_step "Formato Rust" cargo fmt --all -- --check
run_step "Tipos TypeScript" npx tsc --noEmit
run_step "Pruebas de interfaz (Vitest)" npx vitest run
grep -hE "Tests +[0-9]" "$LAST_LOG" | sed 's/^/        /' | tee -a "$REPORT"
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
  if kill -0 "$APP" 2>/dev/null; then ok "La aplicación sigue en ejecución 8 s después de arrancar"; else bad "La aplicación terminó al arrancar (ver $LOGDIR/arranque.log)"; fi
  kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null
  rm -rf "$TMPCFG"
else
  warn "Sin sesión gráfica: arranque no comprobado"
fi
if [ $BUILD_DEB -eq 1 ]; then
  run_step "Paquete .deb" npx tauri build --bundles deb
  ls -1 "$ROOT"/target/release/bundle/deb/*.deb 2>/dev/null | sed 's/^/        /' | tee -a "$REPORT"
fi
say ""

say "RESULTADO AUTOMÁTICO: $PASS correctas, $FAIL fallos, $WARN avisos"
say ""
cat <<'EOF' | tee -a "$REPORT"
7. Comprobaciones manuales en tu equipo (anota OK o describe el problema)
   Arranca la aplicación:  ./target/debug/docker-control-center

   Conexión y Dashboard
   [ ] M1  Arriba a la derecha aparece "Docker <versión>" con punto verde. Pasa el ratón:
           indica el socket real (p. ej. /var/run/docker.sock).
   [ ] M2  El Dashboard muestra la versión de Docker, el socket, el origen y datos que
           coinciden con "docker info": nº de contenedores (en ejecución, en pausa,
           detenidos), imágenes, sistema, núcleo, CPU, memoria, almacenamiento.
   [ ] M3  Abre el panel de actividad (Ctrl+Mayús+A): las primeras operaciones son
           Comprobar conexión (GET /_ping), Consultar versión (GET /version, docker
           version) y Consultar información (GET /info, docker info), todas Correcto.

   Contenedores (crea uno de prueba en una terminal:
   docker run -d --name dcc-manual -p 18080:80 nginx   o con cualquier imagen local)
   [ ] M4  "Contenedores" lista los mismos contenedores que "docker ps -a", con estado,
           imagen, puertos y antigüedad.
   [ ] M5  Detener, Iniciar, Pausar, Reanudar y Reiniciar dcc-manual: el estado cambia en
           la tabla y en "docker ps -a". Cada acción aparece en el panel con su comando
           equivalente (p. ej. docker stop --time 10 <id>).
   [ ] M6  Eliminar dcc-manual en ejecución: el diálogo exige marcar "Detenerlo a la fuerza".
           Cancelar o Escape no elimina nada. Al confirmar desaparece de "docker ps -a".
   [ ] M7  "Imágenes" lista las mismas imágenes que "docker image ls", con tamaño y uso.

   Fallos y recuperación
   [ ] M8  Con la app abierta:  sudo systemctl stop docker docker.socket
           En pocos segundos el indicador pasa a "Sin conexión con Docker", el panel de
           actividad registra un ping fallido y el Dashboard explica la causa con pasos.
           Después:  sudo systemctl start docker   La app se reconecta sola en menos de
           ~30 s y vuelve a mostrar los datos.
   [ ] M9  Desconecta la red (Wi-Fi/cable) y reinicia la app: sigue conectando con el
           Docker local y funciona igual.
   [ ] M10 Configuración > Conexión con Docker: "Probar conexión" en Automático da
           "Conexión correcta". Cambia a "Socket personalizado" con /tmp/no-existe.sock:
           "Probar conexión" explica que no existe y no cambia la conexión actual.
           "Guardar y conectar" con esa ruta deja la app sin conexión y el Dashboard
           muestra el diagnóstico. Vuelve a Automático y guarda: reconecta.
   [ ] M11 Cierra y abre la app: la configuración de conexión elegida se conserva.
   [ ] M12 "Comprobar" en Docker Compose muestra tu versión de Compose y la operación
           aparece en el panel como CLI con "docker compose version --format json".

   Permisos (solo si tienes un usuario sin acceso a Docker; si no, márcalo como N/A)
   [ ] M13 Con un usuario que no está en el grupo docker, la app muestra "Sin permiso para
           usar el socket de Docker" con root:docker, modo 660 y la orden usermod para
           ese usuario. La app no pide sudo ni cambia nada.

   Rootless (solo si usas Docker rootless; si no, N/A)
   [ ] M14 Con Docker rootless activo, el indicador muestra "rootless" y el origen es
           "Docker rootless del usuario" o el contexto activo.

   Regresiones de la Fase 1
   [ ] M15 Tema, idioma, barra lateral, panel y atajos siguen funcionando como en la Fase 1.

Envíame verify-phase-2-report.txt y el resultado de M1–M15. Las capturas ayudan.
Cuando termines puedes borrar el contenedor de prueba:  docker rm -f dcc-manual
EOF

[ "$FAIL" -eq 0 ]
