#!/usr/bin/env bash
# Verificación de la Fase 1 de Docker Control Center en Debian/Ubuntu.
#
# Uso (desde la raíz del proyecto, con tu usuario normal, NO como root):
#   bash scripts/verify-phase-1.sh
#
# El script no instala nada ni usa sudo. Si falta algo, te dice qué instalar.
# Al terminar deja un informe en verify-phase-1-report.txt para enviarlo.

set -u
cd "$(dirname "$0")/.." || exit 1
ROOT=$(pwd)
REPORT="$ROOT/verify-phase-1-report.txt"
LOGDIR="$ROOT/target/verify-phase-1"
mkdir -p "$LOGDIR"
: > "$REPORT"

PASS=0; FAIL=0; WARN=0
say()  { printf '%s\n' "$*" | tee -a "$REPORT"; }
ok()   { PASS=$((PASS+1)); say "  [OK]    $*"; }
bad()  { FAIL=$((FAIL+1)); say "  [FALLO] $*"; }
warn() { WARN=$((WARN+1)); say "  [AVISO] $*"; }

ver_ge() { [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" = "$2" ]; }

say "Docker Control Center - verificación de la Fase 1"
say "Fecha: $(date -Is)"
say "Proyecto: $ROOT"
say ""

# ---------------------------------------------------------------- sistema
say "1. Sistema"
if [ "$(id -u)" -eq 0 ]; then
  bad "Estás ejecutando el script como root. Ejecútalo con tu usuario normal."
fi
. /etc/os-release 2>/dev/null || true
say "  Distribución: ${PRETTY_NAME:-desconocida}"
case "${ID:-}" in
  debian) ver_ge "${VERSION_ID:-0}" 12 && ok "Debian ${VERSION_ID} (mínimo 12)" || bad "Debian ${VERSION_ID:-?}: se requiere 12 o superior" ;;
  ubuntu) ver_ge "${VERSION_ID:-0}" 22.04 && ok "Ubuntu ${VERSION_ID} (mínimo 22.04)" || bad "Ubuntu ${VERSION_ID:-?}: se requiere 22.04 o superior" ;;
  *) if echo "${ID_LIKE:-}" | grep -qE 'debian|ubuntu'; then warn "Derivada de Debian/Ubuntu (${ID:-?}): no es una plataforma probada oficialmente"; else bad "Distribución no soportada (${ID:-?})"; fi ;;
esac
say "  Arquitectura: $(uname -m)   Núcleo: $(uname -r)"
say "  Escritorio: ${XDG_CURRENT_DESKTOP:-desconocido}   Sesión: ${XDG_SESSION_TYPE:-desconocida}"
say ""

# ---------------------------------------------------------------- paquetes
say "2. Paquetes de sistema necesarios para compilar"
PKGS="build-essential curl wget file pkg-config libssl-dev libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev"
MISSING=""
for p in $PKGS; do
  if dpkg-query -W -f='${Status}' "$p" 2>/dev/null | grep -q "install ok installed"; then ok "$p"; else MISSING="$MISSING $p"; bad "$p no está instalado"; fi
done
if [ -n "$MISSING" ]; then
  say ""
  say "  Instálalos con:"
  say "    sudo apt update && sudo apt install -y$MISSING"
fi
WK=$(pkg-config --modversion webkit2gtk-4.1 2>/dev/null || true)
[ -n "$WK" ] && say "  WebKitGTK 4.1: $WK"
say ""

# ---------------------------------------------------------------- toolchains
say "3. Herramientas de desarrollo"
if command -v rustc >/dev/null; then
  RV=$(rustc --version | awk '{print $2}')
  ver_ge "$RV" 1.80.0 && ok "Rust $RV (mínimo 1.80)" || bad "Rust $RV: se requiere 1.80 o superior (rustup update)"
else
  bad "Rust no está instalado. Instálalo con: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
fi
if command -v node >/dev/null; then
  NV=$(node --version | tr -d v)
  ver_ge "$NV" 20.0.0 && ok "Node.js $NV (mínimo 20)" || bad "Node.js $NV: se requiere 20 o superior (ver INSTALL.md)"
else
  bad "Node.js no está instalado (ver INSTALL.md)"
fi
command -v npm >/dev/null && ok "npm $(npm --version)" || bad "npm no está instalado"
say ""

if [ "$FAIL" -gt 0 ]; then
  say "Faltan requisitos. Corrígelos y vuelve a ejecutar el script."
  say "RESULTADO: $PASS correctas, $FAIL fallos, $WARN avisos"
  exit 1
fi

# ---------------------------------------------------------------- pruebas
run_step() {
  local name="$1"; shift
  local log
  log="$LOGDIR/$(echo "$name" | tr ' /()' '____').log"
  printf '  ... %s\n' "$name"
  if "$@" >"$log" 2>&1; then ok "$name"; else bad "$name (ver $log)"; tail -25 "$log" | sed 's/^/        /' | tee -a "$REPORT" >/dev/null; fi
}

say "4. Dependencias del proyecto"
run_step "npm ci" npm ci --no-audit --no-fund
say ""

say "5. Pruebas automatizadas"
run_step "Pruebas Rust (unitarias e integración IPC)" cargo test --workspace --locked
grep -h "^test result" "$LOGDIR"/Pruebas_Rust*.log 2>/dev/null | sed 's/^/        /' | tee -a "$REPORT"
run_step "Clippy sin avisos" cargo clippy --workspace --all-targets --locked -- -D warnings
run_step "Comprobación de tipos TypeScript" npx tsc --noEmit
run_step "Pruebas de interfaz (Vitest)" npx vitest run
grep -hE "Tests +[0-9]" "$LOGDIR"/Pruebas_de_interfaz*.log 2>/dev/null | sed 's/^/        /' | tee -a "$REPORT"
say ""

say "6. Compilación de la aplicación"
run_step "Compilación de escritorio (debug)" npx tauri build --debug --no-bundle
BIN="$ROOT/target/debug/docker-control-center"
[ -x "$BIN" ] && ok "Binario generado: $BIN" || bad "No se generó el binario"
say ""

say "7. Arranque"
if [ -x "$BIN" ] && [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
  TMPCFG=$(mktemp -d)
  DCC_CONFIG="$TMPCFG/config.toml" "$BIN" >"$LOGDIR/arranque.log" 2>&1 &
  APP=$!
  sleep 6
  if kill -0 "$APP" 2>/dev/null; then
    ok "La aplicación sigue en ejecución 6 s después de arrancar"
    kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null
  else
    bad "La aplicación terminó al arrancar (ver $LOGDIR/arranque.log)"
    tail -20 "$LOGDIR/arranque.log" | sed 's/^/        /' | tee -a "$REPORT" >/dev/null
  fi
  rm -rf "$TMPCFG"
else
  warn "No hay sesión gráfica; omitida la prueba de arranque"
fi
say ""

say "RESULTADO AUTOMÁTICO: $PASS correctas, $FAIL fallos, $WARN avisos"
say ""
cat <<'EOF' | tee -a "$REPORT"
8. Comprobaciones manuales (anota OK o describe el problema)
   Arranca la aplicación:  ./target/debug/docker-control-center

   [ ] M1  La ventana se abre con barra lateral, área central y botón "Actividad".
   [ ] M2  La barra lateral muestra, en este orden: Dashboard, Contenedores, Imágenes,
           Redes, Volúmenes, Directorios vinculados, Docker Compose, Eventos y registros,
           Configuración.
   [ ] M3  Al pulsar cada apartado cambia el título superior y se indica la fase en que
           llegará. No aparece ningún dato inventado ni botones de acción.
   [ ] M4  "Contraer barra lateral" la reduce a iconos; vuelve a expandirse. Ctrl+B hace lo mismo.
   [ ] M5  El botón "Actividad" abre y cierra el panel derecho sin cambiar de apartado.
           Ctrl+Mayús+A hace lo mismo. El panel indica "Aún no hay operaciones".
   [ ] M6  En Configuración, el tema Claro / Oscuro / Sistema cambia los colores al instante.
           Con "Sistema", la app sigue el tema de tu escritorio.
   [ ] M7  Cambiar a English traduce toda la interfaz; volver a Español la restaura.
   [ ] M8  Cierra y vuelve a abrir la app: se mantienen tema, idioma, barra lateral y panel.
   [ ] M9  Tras cambiar cualquier preferencia existe ~/.config/docker-control-center/config.toml
           con permisos -rw------- (600):  ls -l ~/.config/docker-control-center/
   [ ] M10 Ejecutar "sudo ./target/debug/docker-control-center" muestra que no debe
           ejecutarse como root y no abre la ventana.
   [ ] M11 La interfaz se ve nítida (sin texto borroso ni cortado) y es fluida al
           redimensionar la ventana hasta su tamaño mínimo.

   Opcional (paquete .deb, si te lo he entregado):
   [ ] D1  sudo apt install ./docker-control-center_0.1.0_amd64.deb
   [ ] D2  La aplicación aparece en el menú de aplicaciones y arranca desde él.
   [ ] D3  sudo apt remove docker-control-center la desinstala limpiamente.

Envíame el archivo verify-phase-1-report.txt y el resultado de las comprobaciones
manuales. Si algo falla, una captura de pantalla ayuda mucho.
EOF

[ "$FAIL" -eq 0 ]
