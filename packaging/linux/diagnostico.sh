#!/usr/bin/env bash
# Recoge información para diagnosticar problemas de imgxplorer en Linux (ventana, icono, cierres de WebKit).
# No cambia nada. Deja el resultado en ~/imgxplorer-diagnostico.txt para enviarlo.
#
#   bash diagnostico.sh          solo recoge información
#   bash diagnostico.sh --probar además abre la app tres veces con distintos ajustes de WebKit (cierra cada ventana)
OUT="$HOME/imgxplorer-diagnostico.txt"
BIN=$(command -v imgxplorer || echo "$HOME/.local/bin/imgxplorer")
sec() { printf '\n== %s\n' "$*"; }
{
  sec "Sistema"
  date
  grep -E '^(PRETTY_NAME|VERSION_ID)=' /etc/os-release
  uname -r
  echo "sesión: ${XDG_SESSION_TYPE:-?}  escritorio: ${XDG_CURRENT_DESKTOP:-?}  WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-}  DISPLAY=${DISPLAY:-}"
  echo "virtualización: $(systemd-detect-virt 2>/dev/null)"

  sec "Gráfica"
  lspci -nnk 2>/dev/null | grep -A3 -Ei 'vga|3d|display'
  [ -r /proc/driver/nvidia/version ] && head -n1 /proc/driver/nvidia/version
  command -v glxinfo >/dev/null && glxinfo -B 2>/dev/null | grep -E 'renderer|version string'
  for m in /sys/class/drm/card*-*/modes; do mode=$(head -n1 "$m" 2>/dev/null); [ -n "$mode" ] && echo "$(basename "$(dirname "$m")"): $mode"; done
  command -v gnome-randr >/dev/null && gnome-randr 2>/dev/null | head -20

  sec "WebKitGTK"
  dpkg -l 'libwebkit2gtk-4.1-0' 'libjavascriptcoregtk-4.1-0' 2>/dev/null | awk '/^ii/{print $2, $3}'

  sec "Instalación de imgxplorer"
  echo "ejecutable: $BIN"
  ls -l "$BIN" 2>&1
  [ -x "$BIN" ] && "$BIN" --self-check 2>&1 | head -n 8
  for d in "$HOME/.local/share" /usr/local/share /usr/share; do
    for f in "$d"/applications/*imgx*.desktop; do [ -f "$f" ] && { echo "--- $f"; grep -E '^(Name|Exec|Icon|StartupWMClass)=' "$f"; }; done
    ls "$d"/icons/hicolor/*/apps/imgxplorer.png 2>/dev/null
    [ -f "$d/icons/hicolor/icon-theme.cache" ] && ls -l "$d/icons/hicolor/icon-theme.cache"
  done
  command -v desktop-file-validate >/dev/null && for f in "$HOME"/.local/share/applications/imgxplorer.desktop /usr/local/share/applications/imgxplorer.desktop; do [ -f "$f" ] && desktop-file-validate "$f"; done

  sec "Cierres registrados"
  ls -l /var/crash 2>/dev/null | grep -Ei 'webkit|imgx'
  journalctl -k -b --no-pager 2>/dev/null | grep -Ei 'trap|WebKit|VBlank|imgxplorer' | tail -n 20
  journalctl --user -b --no-pager 2>/dev/null | grep -Ei 'imgxplorer|WebKitWebProcess|VBlank' | tail -n 40
  command -v coredumpctl >/dev/null && coredumpctl list --no-pager 2>/dev/null | grep -Ei 'webkit|imgx' | tail -n 5
} > "$OUT" 2>&1

if [ "${1:-}" = --probar ] && [ -x "$BIN" ]; then
  # La app ya desactiva DMA-BUF sola con Nvidia; con =0 se prueba sin ese ajuste.
  for envs in "WEBKIT_DISABLE_DMABUF_RENDERER=0" "WEBKIT_DISABLE_DMABUF_RENDERER=1" "WEBKIT_DMABUF_RENDERER_FORCE_SHM=1"; do
    echo
    echo "Abriendo imgxplorer con $envs. Usa la app unos segundos, mira si sale el error y ciérrala."
    { sec "Prueba: $envs"; env $envs RUST_LOG=info timeout 300 "$BIN" 2>&1 | tail -n 40; echo "salida: $?"; } >> "$OUT"
    read -r -p "¿Salió el mensaje de error esta vez? [s/n] " r </dev/tty
    echo "¿error?: $r" >> "$OUT"
  done
fi
echo "Listo. Envía este archivo: $OUT"
