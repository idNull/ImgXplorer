#!/bin/bash
# Instalador de imgxplorer para macOS: comprueba el Mac, instala lo necesario para compilar,
# compila la app en este Mac y la deja en Aplicaciones.
#
#   bash "Instalar imgxplorer.command"            instala (pregunta antes de hacer nada)
#   bash "Instalar imgxplorer.command" --check    solo comprueba
#   bash "Instalar imgxplorer.command" --no-ffmpeg  sin ffmpeg (no exportará vídeo)
#
# Se compila aquí porque una app sin firmar de Apple que llega de otro ordenador la bloquea
# Gatekeeper; compilada en el propio Mac no hay ese problema y no hace falta cuenta de desarrollador.
# Compatible con el bash 3.2 que trae macOS.
set -o pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
if [ -f "$HERE/codigo/Cargo.toml" ]; then SRC="$HERE/codigo"; else SRC=$(cd "$HERE/../.." && pwd); fi
[ -f "$SRC/Cargo.toml" ] || { echo "No encuentro el código de imgxplorer junto a este script."; exit 1; }

if [ -t 1 ]; then B=$'\033[1m' G=$'\033[32m' Y=$'\033[33m' R=$'\033[31m' D=$'\033[2m' N=$'\033[0m'; else B= G= Y= R= D= N=; fi
LOG="$HOME/Library/Logs/imgxplorer-install.log"
mkdir -p "$HOME/Library/Logs" && exec > >(tee -a "$LOG") 2>&1
ok()    { printf '  %s✔%s %s\n' "$G" "$N" "$*"; }
warn()  { printf '  %s!%s %s\n' "$Y" "$N" "$*"; }
bad()   { printf '  %s✘%s %s\n' "$R" "$N" "$*"; FATAL=$((FATAL + 1)); }
todo()  { printf '  %s+%s %s %s(se instalará)%s\n' "$Y" "$N" "$1" "$D" "$N"; PLAN="$PLAN$1, "; }
note()  { printf '    %s%s%s\n' "$D" "$*" "$N"; }
title() { printf '\n%s%s%s\n' "$B" "$*" "$N"; }
die()   { printf '\n%s%s%s\n' "$R" "$*" "$N"; printf 'Registro completo: %s\n' "$LOG"; exit 1; }
step()  { printf '\n%s▶ %s%s\n' "$B" "$*" "$N"; }
ver_ge() { [ "$(printf '%s\n%s\n' "$2" "$1" | sort -t. -k1,1n -k2,2n -k3,3n | head -n1)" = "$2" ]; }
FATAL=0 PLAN=""

MODE=install FFMPEG=1
for a in "$@"; do
  case "$a" in
    --check) MODE=check ;;
    --no-ffmpeg) FFMPEG=0 ;;
    -h|--help) sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "Opción desconocida: $a" ;;
  esac
done

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$SRC/Cargo.toml" | head -n1)
printf '%simgxplorer %s — instalador para macOS%s\n' "$B" "$VERSION" "$N"
printf '%sRegistro: %s%s\n' "$D" "$LOG" "$N"
date

# ---------------------------------------------------------------- 1. el Mac
title "1. Este Mac"
[ "$(uname -s)" = Darwin ] || die "Este instalador es para macOS. En Linux usa install.sh."

MACOS=$(sw_vers -productVersion)
if ver_ge "$MACOS" 11.0; then ok "macOS $MACOS (mínimo 11 Big Sur)"; else bad "macOS $MACOS: hace falta macOS 11 Big Sur o posterior"; fi

ARCH=$(uname -m)
if [ "$(sysctl -in sysctl.proc_translated 2>/dev/null)" = 1 ]; then
  bad "La Terminal se está ejecutando con Rosetta (como Intel)"
  note "Cierra Terminal; en Finder > Aplicaciones > Utilidades > Terminal > Obtener información, desmarca «Abrir con Rosetta»."
elif [ "$ARCH" = arm64 ]; then ok "Apple Silicon ($(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo arm64))"
elif [ "$ARCH" = x86_64 ]; then ok "Procesador Intel"
else bad "Procesador desconocido: $ARCH"
fi
if [ "$ARCH" = arm64 ]; then BREW_PREFIX=/opt/homebrew; else BREW_PREFIX=/usr/local; fi

FREE_GB=$(df -g "$HOME" | awk 'NR==2{print $4}')
if [ "${FREE_GB:-0}" -ge 8 ]; then ok "Espacio libre: ${FREE_GB} GB (hacen falta unos 8)"; else bad "Solo hay ${FREE_GB} GB libres: hacen falta unos 8 para compilar"; fi

if id -Gn | grep -qw admin; then ok "Tu usuario es administrador"; else ADMIN=0; warn "Tu usuario no es administrador: no podrá instalar Homebrew ni copiar en /Aplicaciones"; fi

if curl -sfI --max-time 10 https://github.com >/dev/null 2>&1; then ok "Conexión a Internet"; else bad "Sin conexión a Internet (hace falta para descargar las herramientas)"; fi

# ---------------------------------------------------------------- 2. herramientas
title "2. Herramientas para compilar"
[ -x "$BREW_PREFIX/bin/brew" ] && eval "$("$BREW_PREFIX/bin/brew" shellenv)"
export PATH="$HOME/.cargo/bin:$PATH"

if xcode-select -p >/dev/null 2>&1; then ok "Herramientas de línea de comandos de Xcode"; NEED_CLT=0; else todo "Herramientas de línea de comandos de Xcode"; NEED_CLT=1; fi

if command -v brew >/dev/null; then ok "Homebrew $(brew --version | head -n1 | awk '{print $2}')"; NEED_BREW=0
else todo "Homebrew (gestor de paquetes)"; NEED_BREW=1; [ "${ADMIN:-1}" = 0 ] && bad "Homebrew necesita un usuario administrador"; fi

if command -v rustup >/dev/null; then ok "Rust (rustup $(rustup --version 2>/dev/null | awk '{print $2}'))"; NEED_RUST=0
else todo "Rust (rustup, en tu carpeta personal)"; NEED_RUST=1; fi

NODE_OK=0
if command -v node >/dev/null; then
  nv=$(node --version | tr -d v)
  if ver_ge "$nv" 20.19; then ok "Node.js $nv"; NODE_OK=1; else warn "Node.js $nv es antiguo (mínimo 20.19)"; fi
fi
BREW_PKGS=""
[ "$NODE_OK" = 1 ] || { todo "Node.js"; BREW_PKGS="$BREW_PKGS node"; }
if command -v pnpm >/dev/null; then ok "pnpm $(pnpm --version)"; else todo "pnpm"; BREW_PKGS="$BREW_PKGS pnpm"; fi
if command -v wasm-pack >/dev/null; then ok "wasm-pack"; else todo "wasm-pack"; BREW_PKGS="$BREW_PKGS wasm-pack"; fi
if command -v pkg-config >/dev/null; then ok "pkg-config"; else todo "pkg-config"; BREW_PKGS="$BREW_PKGS pkgconf"; fi

title "3. Bibliotecas de la app"
has_formula() { command -v brew >/dev/null && brew list --formula "$1" >/dev/null 2>&1; }
if has_formula libheif; then ok "libheif $(brew list --versions libheif | awk '{print $2}') (HEIC/AVIF)"; else todo "libheif (fotos HEIC y AVIF)"; BREW_PKGS="$BREW_PKGS libheif"; fi
if [ "$FFMPEG" = 1 ]; then
  if command -v ffmpeg >/dev/null; then ok "ffmpeg (exportar vídeo)"; else todo "ffmpeg (exportar vídeo)"; BREW_PKGS="$BREW_PKGS ffmpeg"; fi
else note "ffmpeg: no se instalará (--no-ffmpeg); la app no podrá exportar vídeo"; fi

if [ -w /Applications ]; then DEST_DIR=/Applications; else DEST_DIR="$HOME/Applications"; fi
title "4. Destino"
ok "$DEST_DIR/imgxplorer.app"
[ -d "$DEST_DIR/imgxplorer.app" ] && note "Ya hay una versión instalada: se reemplazará (tus ajustes se conservan)."

echo
[ "$FATAL" -gt 0 ] && die "Este Mac no cumple los requisitos ($FATAL problema(s) arriba). No se ha instalado nada."
if [ "$MODE" = check ]; then
  printf '%sComprobación terminada.%s ' "$G" "$N"
  if [ -n "$PLAN" ]; then echo "Al instalar se añadirían: ${PLAN%, }."; else echo "Tienes todo lo necesario."; fi
  exit 0
fi

printf '%sPlan:%s\n' "$B" "$N"
[ -n "$PLAN" ] && echo "  · Instalar: ${PLAN%, }"
echo "  · Compilar imgxplorer en este Mac (la primera vez tarda 10-20 minutos)"
echo "  · Copiarlo en $DEST_DIR"
[ "$NEED_BREW" = 1 ] && note "Homebrew pedirá la contraseña de tu Mac (la de iniciar sesión) y que pulses Intro."
printf '¿Continuar? [S/n] '
read -r r </dev/tty
case "$(printf '%s' "$r" | tr '[:upper:]' '[:lower:]')" in ""|s|si|sí|y|yes) ;; *) die "Cancelado. No se ha instalado nada." ;; esac
START=$(date +%s)

# ---------------------------------------------------------------- instalar herramientas
if [ "$NEED_CLT" = 1 ]; then
  step "Herramientas de línea de comandos de Xcode"
  xcode-select --install >/dev/null 2>&1
  echo "Se ha abierto una ventana de macOS: pulsa «Instalar», acepta y espera a que termine."
  echo "(Esperando; esto sigue solo cuando acabe…)"
  until xcode-select -p >/dev/null 2>&1; do sleep 10; done
  ok "Instaladas"
fi

if [ "$NEED_BREW" = 1 ]; then
  step "Homebrew"
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)" </dev/tty || die "No se pudo instalar Homebrew."
  eval "$("$BREW_PREFIX/bin/brew" shellenv)"
  if ! grep -qs "brew shellenv" "$HOME/.zprofile"; then
    printf '\neval "$(%s/bin/brew shellenv)"\n' "$BREW_PREFIX" >> "$HOME/.zprofile"
    note "Añadido Homebrew a ~/.zprofile para que la Terminal lo encuentre."
  fi
fi

if [ -n "$BREW_PKGS" ]; then
  step "Instalando con Homebrew:$BREW_PKGS"
  # shellcheck disable=SC2086
  brew install $BREW_PKGS || die "Homebrew no pudo instalar algo (mira el mensaje de arriba)."
fi

if [ "$NEED_RUST" = 1 ]; then
  step "Rust"
  # Si hay un Rust de Homebrew, rustup se instala igualmente y queda delante en el PATH.
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | RUSTUP_INIT_SKIP_PATH_CHECK=yes sh -s -- -y --profile minimal --default-toolchain stable \
    || die "No se pudo instalar Rust."
fi
step "Preparando Rust"
export RUSTUP_TOOLCHAIN=stable
rustup toolchain install stable --profile minimal >/dev/null 2>&1 || die "rustup no pudo instalar la versión estable de Rust."
rustup target add wasm32-unknown-unknown --toolchain stable || die "No se pudo añadir el destino WebAssembly."
ok "$(rustc --version)"

# ---------------------------------------------------------------- compilar
step "Compilando imgxplorer (paciencia: la primera vez tarda bastante)"
cd "$SRC" || die "No puedo entrar en $SRC"
CI=true pnpm install --frozen-lockfile || die "pnpm install falló."
pnpm tauri build --bundles app || die "La compilación falló. Envía el registro: $LOG"
BUILT="$SRC/target/release/bundle/macos/imgxplorer.app"
[ -d "$BUILT" ] || die "No encuentro la app compilada en $BUILT"

# ---------------------------------------------------------------- instalar la app
step "Copiando en $DEST_DIR"
mkdir -p "$DEST_DIR"
DEST="$DEST_DIR/imgxplorer.app"
pkill -x imgxplorer 2>/dev/null
rm -rf "$DEST"
ditto "$BUILT" "$DEST" || die "No se pudo copiar en $DEST_DIR"
xattr -dr com.apple.quarantine "$DEST" 2>/dev/null
# Firma local (sin cuenta de Apple): basta para que macOS la deje abrir en este Mac.
codesign --force --deep --sign - "$DEST" >/dev/null 2>&1 || warn "No se pudo firmar localmente (normalmente no importa)"

step "Comprobando la app"
if out=$("$DEST/Contents/MacOS/imgxplorer" --self-check 2>&1); then
  ok "$(printf '%s\n' "$out" | head -n1) arranca"
  heif=$(printf '%s\n' "$out" | sed -n 's/^libheif=//p')
  if [ "$(printf '%s\n' "$out" | sed -n 's/^heic=//p')" = si ]; then ok "libheif $heif: abre HEIC"; else warn "libheif $heif sin HEVC: no abrirá HEIC"; fi
else
  printf '%s\n' "$out"
  die "La app no arranca. Envía el registro: $LOG"
fi
if [ "$FFMPEG" = 1 ] && command -v ffmpeg >/dev/null; then
  if grep -qE 'libx264|h264_videotoolbox' <<< "$(ffmpeg -hide_banner -encoders 2>/dev/null)"; then ok "ffmpeg listo para exportar vídeo"; else warn "ffmpeg sin H.264"; fi
fi

# IA (opcional, solo si dices que sí).
AIS=""
command -v claude >/dev/null && AIS="$AIS Claude Code,"
command -v codex >/dev/null && AIS="$AIS Codex,"
command -v gemini >/dev/null && AIS="$AIS Gemini CLI,"
if [ -n "$AIS" ]; then
  echo
  echo "Tienes estas IAs:${AIS%,}. imgxplorer puede conectarse con ellas para que monten vídeos por ti."
  printf '¿Conectar imgxplorer con ellas? [s/N] '
  read -r r </dev/tty
  case "$(printf '%s' "$r" | tr '[:upper:]' '[:lower:]')" in
    s|si|sí|y|yes) "$DEST/Contents/MacOS/imgxplorer" --mcp-install all | sed 's/^/  /'; note "Activa «Permitir el control por IA» en Ajustes → IA." ;;
    *) note "Podrás hacerlo cuando quieras en Ajustes → IA." ;;
  esac
fi

MIN=$(( ($(date +%s) - START + 59) / 60 ))
echo
printf '%s¡Listo!%s imgxplorer está en %s (%s min).\n' "$G" "$N" "$DEST_DIR" "$MIN"
note "Búscalo en Launchpad o en Aplicaciones. Para quitarlo, arrástralo a la Papelera"
note "(si lo conectaste con una IA, quítalo antes en Ajustes → IA)."
note "La carpeta del código ya no hace falta para usarlo (ocupa unos GB por la compilación)."
note "La app usa la libheif de Homebrew: no la desinstales con «brew uninstall libheif»."
printf '¿Abrir imgxplorer ahora? [S/n] '
read -r r </dev/tty
case "$(printf '%s' "$r" | tr '[:upper:]' '[:lower:]')" in ""|s|si|sí|y|yes) open "$DEST" ;; esac
exit 0
