#!/usr/bin/env bash
# Instalador de imgxplorer para Linux.
#
#   ./install.sh               comprueba el sistema, instala lo que falte e instala la app para ti
#   ./install.sh --check       solo comprueba; no instala ni cambia nada
#   ./install.sh --system      instala para todos los usuarios (en /usr/local, pide contraseña)
#   ./install.sh --no-ffmpeg   no instala ffmpeg (la app funciona, pero no exporta vídeo)
#   ./install.sh --yes         no pregunta (acepta el plan)
#   ./install.sh --uninstall   desinstala la app (no quita las bibliotecas del sistema)
#
# Antes de tocar nada comprueba la arquitectura, la distribución y su versión, la glibc y que el
# gestor de paquetes tenga lo necesario. Después de instalar las dependencias ejecuta la propia app
# en modo comprobación: si arranca es que encuentra todas sus bibliotecas.
[ -n "${BASH_VERSION:-}" ] || exec bash "$0" "$@"
set -uo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
APP=imgxplorer
BIN_SRC="$HERE/bin/$APP"

# ---------------------------------------------------------------- salida
if [ -t 1 ]; then B=$'\e[1m' G=$'\e[32m' Y=$'\e[33m' R=$'\e[31m' D=$'\e[2m' N=$'\e[0m'; else B= G= Y= R= D= N=; fi
LOG="${XDG_CACHE_HOME:-$HOME/.cache}/imgxplorer-install.log"
mkdir -p "$(dirname "$LOG")" 2>/dev/null && exec > >(tee -a "$LOG") 2>&1
ok()    { printf '  %s✔%s %s\n' "$G" "$N" "$*"; }
warn()  { printf '  %s!%s %s\n' "$Y" "$N" "$*"; WARNINGS=$((WARNINGS + 1)); }
bad()   { printf '  %s✘%s %s\n' "$R" "$N" "$*"; FATAL=$((FATAL + 1)); }
note()  { printf '    %s%s%s\n' "$D" "$*" "$N"; }
title() { printf '\n%s%s%s\n' "$B" "$*" "$N"; }
die()   { printf '\n%s%s%s\n' "$R" "$*" "$N"; exit 1; }
FATAL=0 WARNINGS=0

# $1 >= $2 (versiones tipo 2.39 o 1.17.6)
ver_ge() { [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -n1)" = "$2" ]; }

ask() { # ask "pregunta" -> 0 si sí
  [ "$YES" = 1 ] && return 0
  local r; read -r -p "$1 [S/n] " r </dev/tty || return 1
  case "${r,,}" in ""|s|si|sí|y|yes) return 0 ;; *) return 1 ;; esac
}

# ---------------------------------------------------------------- argumentos
MODE=install SCOPE=user YES=0 FFMPEG=1
for a in "$@"; do
  case "$a" in
    --check) MODE=check ;;
    --uninstall) MODE=uninstall ;;
    --system) SCOPE=system ;;
    --yes|-y) YES=1 ;;
    --no-ffmpeg) FFMPEG=0 ;;
    -h|--help) sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "Opción desconocida: $a (usa --help)" ;;
  esac
done
[ "$(id -u)" = 0 ] && SCOPE=system

if [ "$SCOPE" = system ]; then PREFIX=/usr/local; else PREFIX="$HOME/.local"; fi
MANIFEST="$PREFIX/share/$APP/installed-files"

# sudo solo cuando hace falta
SUDO=
if [ "$(id -u)" != 0 ]; then command -v sudo >/dev/null && SUDO=sudo; fi
as_root() { if [ "$(id -u)" = 0 ]; then "$@"; elif [ -n "$SUDO" ]; then sudo "$@"; else die "Hace falta sudo para: $*"; fi; }
in_prefix() { if [ "$SCOPE" = system ]; then as_root "$@"; else "$@"; fi; }

# Que el escritorio vea los iconos nuevos. Si otra app dejó una caché (icon-theme.cache) en la carpeta,
# GTK se fía de ella y no ve los iconos añadidos después: se rehace. Cambiar la fecha de la carpeta
# también hace que GTK descarte una caché vieja.
refresh_icons() {
  local d="$1/share/icons/hicolor"
  [ -d "$d" ] || return 0
  in_prefix touch "$d" 2>/dev/null
  if [ -f "$d/icon-theme.cache" ] || [ -f "$d/index.theme" ]; then
    command -v gtk-update-icon-cache >/dev/null && in_prefix gtk-update-icon-cache -qtf "$d" 2>/dev/null
  fi
  return 0
}

# ---------------------------------------------------------------- desinstalar
if [ "$MODE" = uninstall ]; then
  title "Desinstalar imgxplorer"
  found=0
  for p in "$HOME/.local" /usr/local; do
    m="$p/share/$APP/installed-files"
    [ -f "$m" ] || continue
    found=1
    if [ "$p" = /usr/local ]; then SCOPE=system; else SCOPE=user; fi
    # Quitarlo también de las IAs a las que se conectó.
    [ -x "$p/bin/$APP" ] && "$p/bin/$APP" --mcp-uninstall all 2>/dev/null | sed 's/^/  /'
    while IFS= read -r f; do [ -n "$f" ] && in_prefix rm -f "$f" && note "borrado $f"; done < "$m"
    in_prefix rm -rf "$p/share/$APP"
    command -v update-desktop-database >/dev/null && in_prefix update-desktop-database -q "$p/share/applications" 2>/dev/null
    refresh_icons "$p"
    ok "Quitado de $p"
  done
  [ "$found" = 1 ] || die "No encuentro ninguna instalación hecha con este instalador."
  note "Tus ajustes y proyectos siguen en ~/.config/com.imgxplorer.app y ~/.local/share/com.imgxplorer.app."
  note "Las bibliotecas del sistema (WebKitGTK, libheif, ffmpeg) no se tocan."
  exit 0
fi

[ -x "$BIN_SRC" ] || die "No encuentro $BIN_SRC. Ejecuta este script desde la carpeta que descomprimiste."
# shellcheck source=/dev/null
. "$HERE/REQUISITOS"   # VERSION, ARCH, GLIBC_MIN, LIBHEIF_MIN

printf '%simgxplorer %s — instalador para Linux%s\n' "$B" "$VERSION" "$N"
printf '%sRegistro: %s%s\n' "$D" "$LOG" "$N"

# ---------------------------------------------------------------- 1. sistema
title "1. Sistema"
if [ "$(uname -s)" != Linux ]; then die "Este instalador es para Linux. En macOS usa «Instalar imgxplorer.command»."; fi
m=$(uname -m)
if [ "$m" = "$ARCH" ]; then ok "Procesador $m"; else bad "Procesador $m: este paquete es para $ARCH"; fi

ID= ID_LIKE= VERSION_ID= PRETTY_NAME= UBUNTU_CODENAME=
# shellcheck source=/dev/null
[ -r /etc/os-release ] && . /etc/os-release
DISTRO="${PRETTY_NAME:-desconocida}"

FAMILY=
for i in $ID $ID_LIKE; do
  case "$i" in
    arch) FAMILY=arch ;;
    debian|ubuntu) FAMILY=debian ;;
    fedora|rhel) FAMILY=fedora ;;
    suse|opensuse*) FAMILY=suse ;;
  esac
  [ -n "$FAMILY" ] && break
done

# Versión mínima por distribución (las que conocemos). El resto se juzga por glibc y paquetes.
min=
case "$ID" in
  ubuntu|pop|neon) min=24.04 ;;
  debian) min=13 ;;
  fedora) min=40 ;;
  opensuse-leap) min=16.0 ;;
  arch|archarm|cachyos|endeavouros|manjaro|garuda|artix|omarchy|opensuse-tumbleweed|opensuse-slowroll) min=rolling ;;
esac
cur="$VERSION_ID"
if [ -z "$min" ] && [ -n "$UBUNTU_CODENAME" ]; then # derivadas de Ubuntu (Mint, Zorin, elementary…)
  case "$UBUNTU_CODENAME" in
    focal) cur=20.04 ;; jammy) cur=22.04 ;; noble) cur=24.04 ;; oracular) cur=24.10 ;;
    plucky) cur=25.04 ;; questing) cur=25.10 ;; resolute) cur=26.04 ;; *) cur= ;;
  esac
  [ -n "$cur" ] && min=24.04 && DISTRO="$DISTRO (base Ubuntu $cur)"
fi
if [ "$min" = rolling ]; then
  ok "$DISTRO (se actualiza continuamente)"
elif [ -n "$min" ] && [ -n "$cur" ]; then
  if ver_ge "$cur" "$min"; then ok "$DISTRO"; else bad "$DISTRO: hace falta la versión $min o posterior"; fi
elif [ -n "$FAMILY" ]; then
  ok "$DISTRO"; note "No conozco esta distribución en concreto; decidirán la glibc y los paquetes."
else
  bad "Distribución no reconocida ($DISTRO)"
  note "Necesitas WebKitGTK 4.1, GTK 3 y libheif ≥ $LIBHEIF_MIN; el AppImage puede servirte."
fi

glibc_version() { # getconf no siempre está; ldd y la propia libc dicen lo mismo
  local v
  v=$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{print $2}')
  [ -n "$v" ] || v=$(ldd --version 2>/dev/null | head -n1 | grep -o '[0-9]\+\.[0-9]\+$')
  for l in /lib64/libc.so.6 /lib/x86_64-linux-gnu/libc.so.6 /usr/lib/libc.so.6; do
    [ -n "$v" ] && break
    [ -x "$l" ] && v=$("$l" 2>/dev/null | head -n1 | grep -o 'version [0-9.]*' | grep -o '[0-9][0-9.]*[0-9]')
  done
  printf '%s' "$v"
}
glibc=$(glibc_version)
if [ -z "$glibc" ]; then
  bad "No hay glibc (¿musl?): este binario no puede funcionar aquí"
elif ver_ge "$glibc" "$GLIBC_MIN"; then
  ok "glibc $glibc (mínimo $GLIBC_MIN)"
else
  bad "glibc $glibc: hace falta $GLIBC_MIN o posterior (la distribución es demasiado antigua)"
fi

if [ -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]; then ok "Sesión gráfica ${XDG_SESSION_TYPE:-}"; else warn "No hay sesión gráfica abierta: se instala, pero ábrela desde el escritorio"; fi

NVIDIA=0
[ -r /proc/driver/nvidia/version ] && NVIDIA=1

# ---------------------------------------------------------------- 2. paquetes
title "2. Bibliotecas"
PM=
case "$FAMILY" in
  arch)   command -v pacman  >/dev/null && PM=pacman ;;
  debian) command -v apt-get >/dev/null && PM=apt ;;
  fedora) command -v dnf     >/dev/null && PM=dnf ;;
  suse)   command -v zypper  >/dev/null && PM=zypper ;;
esac

# Cada entrada: alternativas separadas por «|» (la primera que exista en el repositorio).
case "$PM" in
  pacman) REQ=(webkit2gtk-4.1 gtk3 libheif alsa-lib); OPT=(ffmpeg) ;;
  apt)    REQ=(libwebkit2gtk-4.1-0 "libgtk-3-0t64|libgtk-3-0" "libheif1|libheif1t64" libheif-plugin-libde265 "libheif-plugin-dav1d|libheif-plugin-aomdec" "libasound2t64|libasound2"); OPT=(ffmpeg) ;;
  dnf)    REQ=(webkit2gtk4.1 gtk3 libheif alsa-lib); OPT=("ffmpeg|ffmpeg-free") ;;
  zypper) REQ=(libwebkit2gtk-4_1-0 libgtk-3-0 libheif1 libasound2); OPT=("ffmpeg-7|ffmpeg-6|ffmpeg") ;;
  *)      REQ=(); OPT=() ;;
esac

installed() {
  case "$PM" in
    pacman) pacman -Q "$1" >/dev/null 2>&1 ;;
    apt)    [ "$(dpkg-query -W -f='${Status}' "$1" 2>/dev/null)" = "install ok installed" ] ;;
    dnf|zypper) rpm -q --whatprovides "$1" >/dev/null 2>&1 ;;
  esac
}
available() {
  case "$PM" in
    pacman) LC_ALL=C pacman -Si "$1" >/dev/null 2>&1 ;;
    apt)    grep -q 'Candidate: [^(]' <<< "$(LC_ALL=C apt-cache policy "$1" 2>/dev/null)" ;;
    dnf)    dnf -q list --available "$1" >/dev/null 2>&1 ;;
    zypper) zypper -q --non-interactive search -x "$1" >/dev/null 2>&1 ;;
  esac
}
repo_version() { # versión del repositorio (solo donde es rápido saberla)
  case "$PM" in
    pacman) LC_ALL=C pacman -Si "$1" 2>/dev/null | awk -F': *' '/^Version/{print $2; exit}' ;;
    apt)    LC_ALL=C apt-cache policy "$1" 2>/dev/null | awk '/Candidate:/{print $2; exit}' ;;
  esac | sed 's/^[0-9]*://; s/[-+~].*//'
}

TO_INSTALL=()
resolve() { # resolve "a|b" opcional -> añade a TO_INSTALL si falta
  local alts="$1" optional="$2" p
  IFS='|' read -r -a cand <<< "$alts"
  for p in "${cand[@]}"; do installed "$p" && { ok "$p"; return 0; }; done
  for p in "${cand[@]}"; do
    if available "$p"; then TO_INSTALL+=("$p"); printf '  %s+%s %s %s(se instalará)%s\n' "$Y" "$N" "$p" "$D" "$N"; return 0; fi
  done
  if [ "$optional" = 1 ]; then warn "${cand[0]} no está en tus repositorios"; else bad "${cand[0]} no está en tus repositorios"; fi
  return 1
}

if [ -z "$PM" ]; then
  bad "No encuentro un gestor de paquetes conocido (pacman, apt, dnf o zypper)"
else
  ok "Gestor de paquetes: $PM"
  # Sin lista de paquetes (sistema recién instalado) apt no sabe qué hay en el repositorio.
  if [ "$PM" = apt ] && ! ls /var/lib/apt/lists/*Packages* >/dev/null 2>&1; then
    if [ "$MODE" = install ]; then
      note "apt no tiene la lista de paquetes: la descargo (apt-get update; puede pedir contraseña)"
      as_root apt-get update -q >/dev/null || bad "apt-get update falló"
    else
      warn "apt no tiene la lista de paquetes; ejecuta «sudo apt-get update» y vuelve a comprobar"
    fi
  fi
  for r in "${REQ[@]}"; do resolve "$r" 0; done

  # libheif: el binario necesita al menos LIBHEIF_MIN.
  hv=$(repo_version "$( [ "$PM" = apt ] && echo libheif1 || echo libheif )")
  if [ -n "$hv" ]; then
    if ver_ge "$hv" "$LIBHEIF_MIN"; then ok "libheif $hv en el repositorio (mínimo $LIBHEIF_MIN)"; else bad "libheif $hv en el repositorio: hace falta $LIBHEIF_MIN"; fi
  fi

  if [ "$FFMPEG" = 1 ]; then
    if command -v ffmpeg >/dev/null; then
      if grep -qE 'libx264|libopenh264' <<< "$(ffmpeg -hide_banner -encoders 2>/dev/null)"; then ok "ffmpeg (exportar vídeo)"; else warn "ffmpeg está, pero sin codificador H.264: no podrá exportar vídeo"; fi
    else
      resolve "${OPT[0]}" 1 || note "Sin ffmpeg la app funciona, pero no exporta vídeo."
      [ "$PM" = dnf ] && note "Para el ffmpeg completo, activa RPM Fusion (rpmfusion.org)."
    fi
  else
    note "ffmpeg: no se instalará (--no-ffmpeg)"
  fi
fi

# ---------------------------------------------------------------- 3. destino
title "3. Dónde se instala"
if [ "$SCOPE" = system ]; then ok "Para todos los usuarios: $PREFIX"; else ok "Solo para ti: $PREFIX (sin contraseña)"; fi
avail=$(df -Pk "$(dirname "$PREFIX")" 2>/dev/null | awk 'NR==2{print int($4/1024)}')
if [ -n "$avail" ] && [ "$avail" -lt 200 ]; then bad "Solo quedan $avail MB libres"; fi
other=$(command -v $APP 2>/dev/null || true)
if [ -n "$other" ] && [ "$other" != "$PREFIX/bin/$APP" ]; then
  warn "Ya hay otro imgxplorer en $other (de un .deb, .rpm o AUR): conviene quitarlo para no tener dos"
fi
[ "$NVIDIA" = 1 ] && note "Tarjeta Nvidia: la app desactiva sola el renderizado DMA-BUF de WebKit (evita ventanas en blanco y cierres)."

# ---------------------------------------------------------------- resumen
echo
if [ "$FATAL" -gt 0 ]; then
  die "El sistema no cumple los requisitos ($FATAL problema(s) arriba). No se ha instalado nada."
fi

if [ "$MODE" = check ]; then
  if [ ${#TO_INSTALL[@]} -eq 0 ]; then
    if out=$("$BIN_SRC" --self-check 2>&1); then ok "La app arranca en este sistema"; else bad "La app no arranca:"; note "$out"; fi
  fi
  printf '%sComprobación terminada.%s ' "$G" "$N"
  if [ ${#TO_INSTALL[@]} -gt 0 ]; then echo "Al instalar se añadirían: ${TO_INSTALL[*]}"; else echo "No falta ninguna biblioteca."; fi
  exit 0
fi

printf '%sPlan:%s\n' "$B" "$N"
[ ${#TO_INSTALL[@]} -gt 0 ] && echo "  · Instalar con $PM: ${TO_INSTALL[*]} (pedirá tu contraseña)"
echo "  · Copiar imgxplorer en $PREFIX/bin y añadirlo al menú de aplicaciones"
ask "¿Continuar?" || die "Cancelado. No se ha instalado nada."

# ---------------------------------------------------------------- instalar
if [ ${#TO_INSTALL[@]} -gt 0 ]; then
  title "Instalando bibliotecas"
  case "$PM" in
    pacman) as_root pacman -S --needed --noconfirm "${TO_INSTALL[@]}" \
              || die "pacman falló. Si dice «404» o «no se pudo recuperar», actualiza antes con: sudo pacman -Syu" ;;
    apt)    as_root apt-get update -q && as_root apt-get install -y "${TO_INSTALL[@]}" || die "apt falló (mira el mensaje de arriba)." ;;
    dnf)    as_root dnf install -y "${TO_INSTALL[@]}" || die "dnf falló (mira el mensaje de arriba)." ;;
    zypper) as_root zypper --non-interactive install "${TO_INSTALL[@]}" || die "zypper falló (mira el mensaje de arriba)." ;;
  esac
fi

title "Comprobando que la app arranca"
if ! out=$("$BIN_SRC" --self-check 2>&1); then
  printf '%s\n' "$out"
  missing=$(ldd "$BIN_SRC" 2>/dev/null | awk '/not found/{print $1}')
  [ -n "$missing" ] && note "Faltan: $missing"
  die "La app no arranca en este sistema; no se ha copiado nada. Envía el registro ($LOG)."
fi
get() { printf '%s\n' "$out" | sed -n "s/^$1=//p"; }
ok "$(printf '%s\n' "$out" | head -n1), WebKitGTK $(get webkitgtk)"
heif=$(get libheif)
if ver_ge "$heif" "$LIBHEIF_MIN"; then ok "libheif $heif"; else warn "libheif $heif es antigua (mínimo $LIBHEIF_MIN): HEIC/AVIF pueden fallar"; fi
if [ "$(get heic)" = si ]; then ok "Abre HEIC (fotos de iPhone)"; else
  warn "libheif no tiene decodificador HEVC: no abrirá HEIC"
  case "$PM" in dnf) note "En Fedora: activa RPM Fusion e instala libheif-freeworld" ;;
                apt) note "Instala libheif-plugin-libde265" ;;
                zypper) note "En openSUSE: añade el repositorio Packman e instala libheif con HEVC" ;; esac
fi
[ "$(get avif)" = si ] || note "libheif sin AV1: las miniaturas AVIF las hará WebKit"

title "Copiando la app"
FILES=()
put() { # put origen destino modo
  in_prefix install -Dm"$3" "$1" "$2" && FILES+=("$2")
}
put "$BIN_SRC" "$PREFIX/bin/$APP" 755
for f in "$HERE"/share/icons/hicolor/*/apps/$APP.png; do
  size=$(basename "$(dirname "$(dirname "$f")")")
  put "$f" "$PREFIX/share/icons/hicolor/$size/apps/$APP.png" 644
done
exec_line="$PREFIX/bin/$APP %f"
tmp=$(mktemp)
sed "s|^Exec=.*|Exec=$exec_line|" "$HERE/share/applications/$APP.desktop" > "$tmp"
put "$tmp" "$PREFIX/share/applications/$APP.desktop" 644
rm -f "$tmp"
printf '%s\n' "${FILES[@]}" > "${tmp}.list"
in_prefix install -Dm644 "${tmp}.list" "$MANIFEST"; rm -f "${tmp}.list"
in_prefix install -Dm755 "$0" "$PREFIX/share/$APP/install.sh"
# Licencia y avisos de terceros (se borran con --uninstall junto con share/$APP).
for f in LICENSE THIRD-PARTY.md; do
  if [ -f "$HERE/$f" ]; then in_prefix install -Dm644 "$HERE/$f" "$PREFIX/share/$APP/$f"; fi
done
command -v update-desktop-database >/dev/null && in_prefix update-desktop-database -q "$PREFIX/share/applications" 2>/dev/null
refresh_icons "$PREFIX"
ok "Instalado en $PREFIX"

# ---------------------------------------------------------------- IA (opcional, solo si dices que sí)
AIS=""
command -v claude >/dev/null && AIS="$AIS Claude Code,"
command -v codex >/dev/null && AIS="$AIS Codex,"
command -v gemini >/dev/null && AIS="$AIS Gemini CLI,"
if [ -n "$AIS" ] && [ "$YES" != 1 ]; then
  echo
  echo "Tienes estas IAs:${AIS%,}. imgxplorer puede conectarse con ellas para que monten vídeos por ti."
  read -r -p "¿Conectar imgxplorer con ellas? [s/N] " r </dev/tty || r=n
  case "${r,,}" in
    s|si|sí|y|yes) "$PREFIX/bin/$APP" --mcp-install all | sed 's/^/  /'; note "Activa «Permitir el control por IA» en Ajustes → IA." ;;
    *) note "Podrás hacerlo cuando quieras en Ajustes → IA." ;;
  esac
fi

echo
printf '%s¡Listo!%s Ábrelo desde el menú de aplicaciones («imgxplorer»)' "$G" "$N"
case ":$PATH:" in *":$PREFIX/bin:"*) echo " o escribiendo: imgxplorer" ;; *) echo "."; note "Desde la terminal: $PREFIX/bin/$APP (esa carpeta no está en tu PATH)" ;; esac
note "Para desinstalar: $PREFIX/share/$APP/install.sh --uninstall"
[ "$WARNINGS" -gt 0 ] && note "Hubo $WARNINGS aviso(s) arriba; la app funciona, pero revísalos."
exit 0
