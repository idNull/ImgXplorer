// Carga de imágenes para el visor: nativas (el webview las decodifica) o RGBA servidas por Rust.
import { log } from "../log";

/** Píxeles RGBA con alfa premultiplicado, tal como los sirve `imgx://`. */
export interface RgbaPixels {
  kind: "rgba";
  data: Uint8Array;
}

export interface Picture {
  w: number;
  h: number;
  source: HTMLImageElement | RgbaPixels;
}

/** Qué cargar: la URL normal y la de respaldo forzando decodificación en Rust. */
export interface PictureRef {
  url: string;
  /** Ruta decodificada en Rust; se usa si el webview no sabe con la original. */
  fallback: string;
  /** La original ya es RGBA (formato que el webview no soporta). */
  decoded: boolean;
  /** Posición en la fuente (para sus ajustes de imagen). */
  index?: number;
}

async function fetchRgba(url: string): Promise<Picture> {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`${r.status}: ${await r.text()}`);
  const buf = await r.arrayBuffer();
  const dv = new DataView(buf);
  if (buf.byteLength < 12 || dv.getUint32(0) !== 0x52474241 /* "RGBA" */) throw new Error("respuesta RGBA inválida");
  const w = dv.getUint32(4, true);
  const h = dv.getUint32(8, true);
  return { w, h, source: { kind: "rgba", data: new Uint8Array(buf, 12, w * h * 4) } };
}

async function loadElement(url: string): Promise<Picture> {
  const el = new Image();
  el.crossOrigin = "anonymous"; // necesario para usarla como textura WebGL
  el.src = url;
  await el.decode();
  return { w: el.naturalWidth, h: el.naturalHeight, source: el };
}

async function loadPicture(ref: PictureRef): Promise<Picture> {
  if (ref.decoded) return fetchRgba(ref.url);
  try {
    return await loadElement(ref.url);
  } catch (e) {
    // p. ej. AVIF/WebP animado sin soporte en este WebKit, o JPEG CMYK: lo intenta Rust.
    log.info(`el webview no pudo decodificar ${ref.url}; usando el decodificador propio`);
    return fetchRgba(ref.fallback);
  }
}

/** Caché LRU de imágenes decodificadas, limitada por memoria estimada. */
export class PictureCache {
  private map = new Map<string, { p: Promise<Picture>; bytes: number }>();
  constructor(private budget = 768 << 20) {}

  get(ref: PictureRef): Promise<Picture> {
    const hit = this.map.get(ref.url);
    if (hit) {
      this.map.delete(ref.url);
      this.map.set(ref.url, hit);
      return hit.p;
    }
    const slot = { p: loadPicture(ref), bytes: 0 };
    this.map.set(ref.url, slot);
    slot.p.then(
      (pic) => {
        slot.bytes = pic.w * pic.h * 4;
        this.trim(ref.url);
      },
      () => this.map.delete(ref.url),
    );
    return slot.p;
  }

  private trim(keep: string) {
    let total = 0;
    for (const s of this.map.values()) total += s.bytes;
    for (const [k, s] of this.map) {
      if (total <= this.budget) break;
      if (k === keep) continue;
      total -= s.bytes;
      this.map.delete(k);
    }
  }

  clear() {
    this.map.clear();
  }
}
