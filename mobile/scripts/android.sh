#!/usr/bin/env bash
# Lanza `tauri android …` con Java, el SDK y el NDK de Android localizados, sin tener que tocar la
# configuración de la shell. Si ya tienes JAVA_HOME, ANDROID_HOME o NDK_HOME, se respetan.
#   bash scripts/android.sh init | dev | build --apk …
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ -z "${JAVA_HOME:-}" ]]; then
  for j in /usr/lib/jvm/java-17-openjdk /usr/lib/jvm/java-21-openjdk /usr/lib/jvm/default; do
    [[ -x "$j/bin/java" ]] && { export JAVA_HOME="$j"; break; }
  done
fi
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
if [[ -z "${NDK_HOME:-}" && -d "$ANDROID_HOME/ndk" ]]; then
  # El NDK más nuevo que haya instalado.
  NDK_HOME="$(find "$ANDROID_HOME/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n1)"
  export NDK_HOME
fi
# Rust con rustup, instalado solo en ~/.rustup (sus órdenes en ~/.rustup/cargo/bin, fuera del PATH
# normal): aquí se pone delante, así que solo lo usa la compilación para Android. Fuera de este
# script, `cargo` y `rustc` siguen siendo los del sistema. La caché de crates (~/.cargo) es la misma.
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
[[ -x "$RUSTUP_HOME/cargo/bin/rustup" ]] && export PATH="$RUSTUP_HOME/cargo/bin:$PATH"
export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"

missing=()
[[ -n "${JAVA_HOME:-}" ]] || missing+=("Java 17 (sudo pacman -S jdk17-openjdk)")
[[ -x "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" ]] || missing+=("SDK de Android en $ANDROID_HOME")
[[ -n "${NDK_HOME:-}" ]] || missing+=("NDK de Android (sdkmanager \"ndk;…\")")
command -v rustup >/dev/null || missing+=("rustup en ~/.rustup (ver «Preparar el ordenador»)")
if ((${#missing[@]})); then
  printf 'Falta para compilar para Android:\n' >&2
  printf '  - %s\n' "${missing[@]}" >&2
  printf 'Los pasos están en mobile/README_Android.md («Preparar el ordenador»).\n' >&2
  exit 1
fi

# APK de depuración manejable: sin información de depuración de Rust (pasaba de 900 MB y tardaba en
# instalarse por USB), pero con los nombres de las funciones para que los fallos se entiendan.
export CARGO_PROFILE_DEV_STRIP="${CARGO_PROFILE_DEV_STRIP:-debuginfo}"

# Este proyecto, no el del escritorio (el repositorio tiene dos tauri.conf.json).
export TAURI_APP_PATH=src-tauri TAURI_FRONTEND_PATH=.
exec ../node_modules/.bin/tauri android "$@"
