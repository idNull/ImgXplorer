#!/usr/bin/env python3
"""Genera los iconos de la app (src-tauri/icons) a partir de los tres SVG de src-tauri/icons/svg.

  icono.svg             completo: 128 px o más (y portadas, README, web)
  icono-intermedio.svg  48 y 64 px: sin rayos, ventanas ni personaje
  icono-pequeno.svg     16, 24 y 32 px: cinco viñetas y la cámara

En el Mac todos los tamaños llevan el margen de su rejilla (el cuadrado redondeado ocupa 824 de
1024), para que en el Dock y en el Finder midan lo mismo que los demás iconos.

Uso: python3 packaging/iconos.py   (necesita rsvg-convert e ImageMagick)
"""
import os, struct, subprocess, tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ICONS = os.path.join(ROOT, "src-tauri", "icons")
SVG = os.path.join(ICONS, "svg")


def source(px):
    if px <= 32:
        return os.path.join(SVG, "icono-pequeno.svg")
    if px <= 64:
        return os.path.join(SVG, "icono-intermedio.svg")
    return os.path.join(SVG, "icono.svg")


def render(px, out, mac=False):
    """PNG de `px` píxeles con la versión que toca a ese tamaño."""
    src = source(px)
    if mac and src != os.path.join(SVG, "icono.svg"):
        # Las versiones pequeñas llenan el lienzo: en el Mac van con el margen de la rejilla.
        inner = max(1, round(px * 824 / 1024))
        subprocess.run(["rsvg-convert", "-w", str(inner), "-h", str(inner), src, "-o", out], check=True)
        subprocess.run(["magick", out, "-background", "none", "-gravity", "center", "-extent", f"{px}x{px}", out], check=True)
    else:
        subprocess.run(["rsvg-convert", "-w", str(px), "-h", str(px), src, "-o", out], check=True)


def icns(path, tmp):
    # Formato ICNS: cabecera «icns» + bloques (tipo, longitud, PNG).
    kinds = [("icp4", 16), ("ic11", 32), ("icp5", 32), ("ic12", 64), ("ic07", 128), ("ic13", 256),
             ("ic08", 256), ("ic14", 512), ("ic09", 512), ("ic10", 1024)]
    blocks = b""
    for kind, px in kinds:
        f = os.path.join(tmp, f"mac-{px}.png")
        if not os.path.exists(f):
            render(px, f, mac=True)
        data = open(f, "rb").read()
        blocks += kind.encode() + struct.pack(">I", len(data) + 8) + data
    open(path, "wb").write(b"icns" + struct.pack(">I", len(blocks) + 8) + blocks)


def main():
    with tempfile.TemporaryDirectory() as tmp:
        for name, px in [("32x32.png", 32), ("64x64.png", 64), ("128x128.png", 128), ("128x128@2x.png", 256), ("icon.png", 512)]:
            render(px, os.path.join(ICONS, name))
        ico = []
        for px in (16, 24, 32, 48, 64, 256):
            f = os.path.join(tmp, f"ico-{px}.png")
            render(px, f)
            ico.append(f)
        subprocess.run(["magick", *ico, os.path.join(ICONS, "icon.ico")], check=True)
        icns(os.path.join(ICONS, "icon.icns"), tmp)
    print("iconos generados en", ICONS)


if __name__ == "__main__":
    main()
