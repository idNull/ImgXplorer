#!/usr/bin/env bash
# Prepara en dist/ los paquetes para repartir (pnpm dist, pnpm dist:linux, pnpm dist:mac):
#
#   linux  imgxplorer-<v>-linux-x86_64.tar.gz  binario compilado en Ubuntu 24.04 (con Docker, para
#          que funcione en distribuciones de 2024 en adelante) + instalador que pone las dependencias
#   mac    imgxplorer-<v>-mac.tar.gz           código + instalador que compila la app en el Mac
#
# Los dos toman los archivos del proyecto tal como están ahora (también lo no confirmado en git).
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
DIST="$ROOT/dist"
IMAGE=imgxplorer-build:ubuntu24.04
mkdir -p "$DIST"

# Archivos del proyecto sin lo que ignora git (target, node_modules, dist…).
sources() { git ls-files -co --exclude-standard -z; }
pack_sources() { sources | tar --null --ignore-failed-read -T - -cf - "$@"; }

# La lista de licencias de terceros (Acerca de y THIRD-PARTY.md) tiene que estar al día.
check_licenses() {
  node packaging/licenses.mjs --check || exit 1
}

linux() {
  check_licenses
  command -v docker >/dev/null || { echo "Hace falta Docker para compilar el binario compatible."; exit 1; }
  echo "== Interfaz y núcleo WASM"
  pnpm wasm >/dev/null && pnpm build >/dev/null
  echo "== Imagen de compilación ($IMAGE)"
  docker build -q -t "$IMAGE" -f packaging/linux/Dockerfile packaging/linux >/dev/null
  echo "== Compilando en Ubuntu 24.04 (la primera vez tarda unos minutos)"
  local out; out=$(mktemp -d)
  # El código entra por la entrada estándar: así el contenedor no escribe nada en el proyecto.
  pack_sources ui/dist | docker run --rm -i \
      -v imgxplorer-cargo:/opt/cargo/registry -v imgxplorer-target:/target -v "$out":/out \
      "$IMAGE" bash -euc "
        mkdir /work && tar -xf - -C /work && cd /work
        CARGO_TARGET_DIR=/target cargo build --release --locked -p imgxplorer --features tauri/custom-protocol
        cp /target/release/imgxplorer /out/ && chown $(id -u):$(id -g) /out/imgxplorer"

  local name="imgxplorer-$VERSION-linux-x86_64" stage="$DIST/imgxplorer-$VERSION-linux-x86_64"
  rm -rf "$stage"
  install -Dm755 "$out/imgxplorer" "$stage/bin/imgxplorer"
  rm -rf "$out"
  install -Dm755 packaging/linux/install.sh "$stage/install.sh"
  install -Dm644 packaging/linux/LEEME.txt "$stage/LEEME.txt"
  install -Dm755 packaging/linux/diagnostico.sh "$stage/diagnostico.sh"
  install -Dm644 LICENSE "$stage/LICENSE"
  install -Dm644 THIRD-PARTY.md "$stage/THIRD-PARTY.md"
  install -Dm644 packaging/linux/imgxplorer.desktop "$stage/share/applications/imgxplorer.desktop"
  for s in 32x32 64x64 128x128; do
    install -Dm644 "src-tauri/icons/$s.png" "$stage/share/icons/hicolor/$s/apps/imgxplorer.png"
  done
  install -Dm644 "src-tauri/icons/128x128@2x.png" "$stage/share/icons/hicolor/256x256/apps/imgxplorer.png"
  install -Dm644 src-tauri/icons/icon.png "$stage/share/icons/hicolor/512x512/apps/imgxplorer.png"

  # Requisitos medidos sobre el propio binario.
  local glibc
  glibc=$(objdump -T "$stage/bin/imgxplorer" | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -n1)
  cat > "$stage/REQUISITOS" <<EOF
# Lo lee install.sh. Calculado por packaging/dist.sh.
VERSION=$VERSION
ARCH=x86_64
GLIBC_MIN=$glibc
LIBHEIF_MIN=1.17
EOF
  tar -C "$DIST" -czf "$DIST/$name.tar.gz" "$name"
  echo "   $DIST/$name.tar.gz ($(du -h "$DIST/$name.tar.gz" | cut -f1); necesita glibc ≥ $glibc)"
}

mac() {
  local name="imgxplorer-$VERSION-mac" stage="$DIST/imgxplorer-$VERSION-mac"
  echo "== Paquete para macOS (código + instalador)"
  check_licenses
  # El disco del Mac no distingue mayúsculas: «Settings.svelte» y «settings.svelte.ts» se
  # confunden al importar «./settings.svelte». Linux sí las distingue, así que aquí no se nota.
  local clash
  clash=$(sources | tr '\0' '\n' | sed -E 's/\.(ts|js)$//' | tr '[:upper:]' '[:lower:]' | sort | uniq -d)
  if [ -n "$clash" ]; then
    echo "Estos archivos solo se distinguen por mayúsculas y en macOS se confunden:"
    echo "$clash" | sed 's/^/   /'
    exit 1
  fi
  rm -rf "$stage"
  mkdir -p "$stage/codigo"
  pack_sources | tar -xf - -C "$stage/codigo"
  install -m755 packaging/macos/install.sh "$stage/Instalar imgxplorer.command"
  install -m644 packaging/macos/LEEME.txt "$stage/LEEME.txt"
  install -m644 LICENSE "$stage/LICENSE"
  install -m644 THIRD-PARTY.md "$stage/THIRD-PARTY.md"
  tar -C "$DIST" -czf "$DIST/$name.tar.gz" "$name"
  echo "   $DIST/$name.tar.gz ($(du -h "$DIST/$name.tar.gz" | cut -f1))"
}

case "${1:-all}" in
  linux) linux ;;
  mac) mac ;;
  all) linux; mac ;;
  *) echo "Uso: packaging/dist.sh [linux|mac|all]"; exit 1 ;;
esac
