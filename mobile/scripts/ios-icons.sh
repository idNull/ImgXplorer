#!/usr/bin/env bash
# Iconos de la app de iOS a partir del icono oficial: src-tauri/icons/svg/icono-ios.svg (el mismo
# dibujo que icono.svg, cuadrado y a sangre, porque iOS pone él las esquinas redondeadas).
# Rellena todos los tamaños de gen/apple/Assets.xcassets/AppIcon.appiconset, sin transparencia
# (la App Store rechaza iconos con canal alfa). Hace falta rsvg-convert (librsvg) e ImageMagick.
set -euo pipefail
cd "$(dirname "$0")/../.."
svg=src-tauri/icons/svg/icono-ios.svg
set_dir=mobile/src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
rsvg-convert -w 1024 -h 1024 "$svg" -o "$tmp/1024.png"
python3 - "$set_dir/Contents.json" <<'PY' | while read -r file px; do
import json, sys
for i in json.load(open(sys.argv[1]))["images"]:
    size = float(i["size"].split("x")[0])
    scale = float(i["scale"].rstrip("x"))
    print(i["filename"], round(size * scale))
PY
  magick "$tmp/1024.png" -resize "${px}x${px}" -background "#1F2639" -alpha remove -alpha off "PNG24:$set_dir/$file"
  echo "$file ${px}px"
done
