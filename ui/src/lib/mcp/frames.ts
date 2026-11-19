// Dibujo fuera de pantalla para la IA: páginas, viñetas y fotogramas del vídeo con el mismo
// motor WebGL que la vista previa y la exportación (así lo que revisa es lo que saldrá).
import { GlViewer, type TextureSet } from "../viewer/gl";
import { PictureCache, type Picture, type PictureRef } from "../viewer/picture";
import { viewMatrix, type Pose, type Size } from "../core";
import type { FxTrack } from "../fx/fx";

/** Múltiplo de 4 (lo pide la captura). */
export const mult4 = (n: number) => Math.max(16, Math.round(n / 4) * 4);

export class FrameRenderer {
  private canvas = document.createElement("canvas");
  private gl: GlViewer;
  private cache = new PictureCache(320 << 20);
  /** Texturas recientes (subir una página grande cuesta). */
  private sets = new Map<string, TextureSet>();

  constructor() {
    this.canvas.width = 16;
    this.canvas.height = 16;
    this.gl = new GlViewer(this.canvas);
  }

  picture(ref: PictureRef): Promise<Picture> {
    return this.cache.get(ref);
  }

  private async set(ref: PictureRef): Promise<{ set: TextureSet; pic: Picture }> {
    const pic = await this.picture(ref);
    let set = this.sets.get(ref.url);
    if (set) {
      this.sets.delete(ref.url);
    } else {
      set = this.gl.createSet(pic);
      while (this.sets.size >= 3) {
        const [k, old] = this.sets.entries().next().value!;
        this.gl.deleteSet(old);
        this.sets.delete(k);
      }
    }
    this.sets.set(ref.url, set);
    return { set, pic };
  }

  /**
   * La imagen con la pose `pose` en un fotograma de `w`×`h` (múltiplos de 4). `fx`: efectos
   * especiales de la imagen y el segundo en que se dibuja.
   */
  async render(ref: PictureRef, pose: Pose, w: number, h: number, background: string, fx?: { track: FxTrack | null; t: number }): Promise<ImageData> {
    const { set, pic } = await this.set(ref);
    const vp: Size = { w, h };
    const size = { w: pic.w, h: pic.h };
    this.gl.setBackground(background);
    this.gl.capture(vp, false);
    const frame = fx?.track?.frame(fx.t, size, pose, vp) ?? null;
    this.gl.draw([{ set, m: viewMatrix(pose, size, vp), alpha: 1 }], vp, false, frame);
    return this.gl.readRgba();
  }

  /** Olvida lo cargado (al cambiar de fuente). */
  reset() {
    for (const s of this.sets.values()) this.gl.deleteSet(s);
    this.sets.clear();
    this.cache.clear();
  }
}

/** Lienzo 2D con unos píxeles dentro. */
export function canvasOf(img: ImageData): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = img.width;
  c.height = img.height;
  c.getContext("2d")!.putImageData(img, 0, 0);
  return c;
}

/** JPEG en `data:` (lo que viaja a Rust para guardarlo y dárselo a la IA). */
export const jpeg = (c: HTMLCanvasElement, quality = 0.85) => c.toDataURL("image/jpeg", quality);

/** Etiqueta legible sobre cualquier fondo. */
export function tag(g: CanvasRenderingContext2D, text: string, x: number, y: number, size: number, color = "#fde047") {
  g.font = `bold ${size}px sans-serif`;
  const w = g.measureText(text).width;
  g.fillStyle = "rgba(0,0,0,0.75)";
  g.fillRect(x, y, w + size * 0.6, size * 1.35);
  g.fillStyle = color;
  g.textBaseline = "top";
  g.fillText(text, x + size * 0.3, y + size * 0.18);
}

export interface PanelBox {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Dibuja las viñetas numeradas encima de la página. */
export function drawPanels(c: HTMLCanvasElement, panels: PanelBox[]) {
  const g = c.getContext("2d")!;
  const lw = Math.max(2, Math.round(Math.min(c.width, c.height) / 300));
  const size = Math.max(14, Math.round(Math.min(c.width, c.height) / 28));
  panels.forEach((p, k) => {
    const [x, y, w, h] = [p.x * c.width, p.y * c.height, p.w * c.width, p.h * c.height];
    g.lineWidth = lw * 2.5;
    g.strokeStyle = "rgba(0,0,0,0.8)";
    g.strokeRect(x, y, w, h);
    g.lineWidth = lw;
    g.strokeStyle = "#facc15";
    g.strokeRect(x, y, w, h);
    tag(g, String(k + 1), x + lw * 2, y + lw * 2, size);
  });
}

/** Hoja de miniaturas: los fotogramas en rejilla, cada uno con su rótulo. */
export function contactSheet(frames: { img: ImageData; label: string }[]): HTMLCanvasElement {
  const n = frames.length;
  const cols = n <= 3 ? n : n <= 4 ? 2 : n <= 9 ? 3 : 4;
  const rows = Math.ceil(n / cols);
  const fw = frames[0].img.width;
  const fh = frames[0].img.height;
  const gap = 6;
  const c = document.createElement("canvas");
  c.width = cols * fw + (cols + 1) * gap;
  c.height = rows * fh + (rows + 1) * gap;
  const g = c.getContext("2d")!;
  g.fillStyle = "#222";
  g.fillRect(0, 0, c.width, c.height);
  const size = Math.max(12, Math.round(fw / 22));
  frames.forEach((f, k) => {
    const x = gap + (k % cols) * (fw + gap);
    const y = gap + Math.floor(k / cols) * (fh + gap);
    g.putImageData(f.img, x, y);
    tag(g, f.label, x + 4, y + 4, size);
  });
  return c;
}
