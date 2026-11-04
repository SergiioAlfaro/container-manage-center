#!/usr/bin/env bash
# Verificación de la Fase 2 (integración con Docker Engine) en Debian/Ubuntu.
#
# Uso (desde la raíz del proyecto, con tu usuario normal, NO como root):
#   bash scripts/verify-phase-2.sh          # comprobaciones automáticas
#   bash scripts/verify-phase-2.sh --deb    # además construye el paquete .deb
#
# Requisitos: los de la Fase 1 y acceso a Docker con tu usuario (grupo docker o
# Docker rootless). El script NO usa sudo y NO cambia permisos ni grupos.
#
# Recursos de prueba: las pruebas crean contenedores con la etiqueta
# dcc.test=true y una imagen local dcc-test/sleeper:1 (etiqueta dcc.test=true),
# compilada en el momento sin descargar nada. Al terminar, pase lo que pase,
# se eliminan SOLO los recursos con esa etiqueta y se comprueba que no queda
# ninguno.
#
# Resultado: verify-phase-2-report.txt. Código de salida 0 solo si no hay
# ningún fallo obligatorio. Las omisiones justificadas se listan aparte.

set -u
cd "$(dirname "$0")/.." || exit 1
ROOT=$(pwd)
REPORT="$ROOT/verify-phase-2-report.txt"
LOGDIR="$ROOT/target/verify-phase-2"
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
run_step() {
  local name="$1"; shift
  LAST_LOG="$LOGDIR/$(echo "$name" | tr ' /()' '____').log"
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
  local c i
  c=$(docker ps -aq --filter label=dcc.test=true 2>/dev/null)
  [ -n "$c" ] && docker rm -f $c >/dev/null 2>&1
  i=$(docker image ls -q --filter label=dcc.test=true 2>/dev/null | sort -u)
  [ -n "$i" ] && docker image rm -f $i >/dev/null 2>&1
  return 0
}
finish() {
  local rc=$?
  cleanup_test_resources
  exit $rc
}
trap finish EXIT
trap 'exit 130' INT TERM

say "Docker Control Center - verificación de la Fase 2 (versión $(grep -m1 '^version' Cargo.toml | cut -d'"' -f2))"
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
  say "VEREDICTO: FASE 2 NO VERIFICADA (faltan requisitos)"
  exit 1
fi

# ------------------------------------------------------------------ 2. Docker
say "2. Docker en este equipo (solo lectura)"
if [ $HAVE_DOCKER_CLI -eq 1 ]; then
  CLIV=$(docker version --format '{{.Client.Version}}' 2>/dev/null || true)
  if [ -n "$CLIV" ] && ver_ge "$CLIV" 20.10.0; then ok "CLI de Docker $CLIV ($(command -v docker))"; else warn "Versión del CLI de Docker no reconocida o anterior a 20.10: '${CLIV:-?}'"; fi
  if SV=$(docker version --format '{{.Server.Version}} (API {{.Server.APIVersion}})' 2>/dev/null) && [ -n "$SV" ]; then
    ok "El CLI de Docker habla con Docker Engine $SV"
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
    "no instalado") skip "Docker Compose no está instalado: no es necesario para la Fase 2; sí para el apartado Compose (Fase 7). Ver INSTALL.md" ;;
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
say "5. Pruebas automatizadas"
if run_step "Pruebas Rust deterministas (sin Docker)" cargo test --workspace --locked; then
  grep -h "^test result" "$LAST_LOG" | awk '{s+=$4; f+=$6} END {printf "             %d correctas, %d fallidas (incluye 12 pruebas contra Docker real que en este paso no hacen nada; se ejecutan a continuación)\n", s, f}' | tee -a "$REPORT"
fi
if [ -n "$SOCK" ]; then
  export DCC_DOCKER_TESTS=1 DCC_DOCKER_SOCKET="$SOCK"
  if run_step "Pruebas contra Docker real (motor)" cargo test --locked -p dcc-engine --test live_docker -- --nocapture --test-threads=2; then
    T=$(grep -h "^test result" "$LAST_LOG" | awk '{print $4}')
    O=$(grep -c "omitida:" "$LAST_LOG" || true)
    say "             $T superadas, de ellas $O sin aplicar en este equipo (abajo) y $((T - O)) ejecutadas de verdad"
    while IFS= read -r l; do skip "Prueba real sin aplicar: ${l#*omitida: }"; done < <(grep -o "omitida: .*" "$LAST_LOG")
  fi
  run_step "Prueba contra Docker real (aplicación Tauri)" cargo test --locked -p docker-control-center live_app -- --nocapture
  run_step "Carrera de estado de Docker 26 (prueba determinista, D-031)" cargo test --locked -p dcc-engine --test stale_list
  unset DCC_DOCKER_TESTS DCC_DOCKER_SOCKET
else
  bad "Pruebas contra Docker real NO ejecutadas: no hay conexión con Docker (son obligatorias)"
fi
run_step "Clippy sin avisos" cargo clippy --workspace --all-targets --locked -- -D warnings
run_step "Formato Rust" cargo fmt --all -- --check
run_step "Tipos TypeScript" npx tsc --noEmit
if run_step "Pruebas de interfaz (Vitest)" npx vitest run; then
  grep -hE "Tests +[0-9]" "$LAST_LOG" | sed 's/^/             /' | tee -a "$REPORT"
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
  [ "$LC" -eq 0 ] && [ "$LI" -eq 0 ] && ok "No quedan contenedores ni imágenes de prueba (etiqueta dcc.test=true)" || bad "Quedan $LC contenedores y $LI imágenes de prueba"
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
  say "VEREDICTO: FASE 2 NO SUPERADA en la parte automática. No continúes con las comprobaciones manuales; envía este informe."
else
  say "VEREDICTO: parte automática superada. Faltan las comprobaciones manuales M1–M15 para validar la Fase 2."
fi
say ""
cat <<'EOF' | tee -a "$REPORT"
8. Comprobaciones manuales en tu equipo (solo si la parte automática está superada)
   Arranca la aplicación:  ./target/debug/docker-control-center

   Conexión y Dashboard
   [ ] M1  Arriba a la derecha aparece "Docker <versión>" con punto verde. Pasa el ratón:
           indica el socket real (p. ej. /var/run/docker.sock).
   [ ] M2  El Dashboard muestra datos que coinciden con "docker info": contenedores (en
           ejecución, en pausa, detenidos), imágenes, sistema, núcleo, CPU, memoria.
   [ ] M3  Panel de actividad (Ctrl+Mayús+A): las primeras operaciones son Comprobar
           conexión (GET /_ping), Consultar versión (GET /version) y Consultar
           información (GET /info), todas Correcto.

   Contenedores (crea uno de prueba: docker run -d --name dcc-manual -p 18080:80 nginx)
   [ ] M4  "Contenedores" lista lo mismo que "docker ps -a": estado, imagen, puertos, antigüedad.
   [ ] M5  Detener, Iniciar, Pausar, Reanudar y Reiniciar dcc-manual: el estado de la tabla
           es el correcto inmediatamente después de cada acción y coincide con
           "docker ps -a". En el panel, cada acción va seguida de "Comprobar estado del
           contenedor" (GET /containers/<id>/json).
   [ ] M6  Eliminar dcc-manual en ejecución: el diálogo exige "Detenerlo a la fuerza".
           Cancelar o Escape no elimina nada. Al confirmar desaparece de "docker ps -a".
   [ ] M7  "Imágenes" lista lo mismo que "docker image ls", con tamaño y uso.

   Fallos y recuperación
   [ ] M8  Con la app abierta:  sudo systemctl stop docker docker.socket
           En pocos segundos: "Sin conexión con Docker", ping fallido en el panel y
           diagnóstico en el Dashboard. Después:  sudo systemctl start docker
           La app se reconecta sola en menos de ~30 s.
   [ ] M9  Desconecta la red y reinicia la app: sigue funcionando con el Docker local.
   [ ] M10 Configuración > Conexión con Docker: "Probar conexión" en Automático da
           "Conexión correcta". Socket personalizado /tmp/no-existe.sock: "Probar"
           explica que no existe sin cambiar la conexión; "Guardar y conectar" deja la app
           sin conexión con diagnóstico. Vuelve a Automático y guarda: reconecta.
   [ ] M11 Cierra y abre la app: la configuración de conexión se conserva.
   [ ] M12 "Comprobar" en Docker Compose muestra tu versión, o "No instalado" con las
           instrucciones de instalación si no lo tienes. En el panel aparece como CLI
           "docker compose version" (sin opciones).

   Permisos (solo si tienes un usuario sin acceso a Docker; si no, N/A)
   [ ] M13 Con un usuario fuera del grupo docker: "Sin permiso para usar el socket de
           Docker" con root:docker, 660 y la orden usermod para ese usuario.

   Rootless (solo si usas Docker rootless; si no, N/A)
   [ ] M14 El indicador muestra "rootless" y el origen es "Docker rootless del usuario" o
           el contexto activo.

   Regresiones de la Fase 1
   [ ] M15 Tema, idioma, barra lateral, panel y atajos siguen funcionando.

Cuando termines: docker rm -f dcc-manual
EOF

[ "$FAIL" -eq 0 ]
