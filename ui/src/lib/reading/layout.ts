// Cuentas de los modos de lectura (📖), sin estado: encuadre según el ajuste elegido y qué
// páginas van juntas en doble página.
import type { Pose, Size } from "../core";
import type { FitMode } from "./reading.svelte";
import type { Box } from "./boxes.svelte";
import type { PictureRef } from "../viewer/picture";

const MIN_ZOOM = 0.05;
const MAX_ZOOM = 64;
export const WHOLE: Box = [0, 0, 1, 1];

/** Giro normalizado a 0, 90, 180 o 270. */
function quarter(rot: number): number {
  return ((Math.round(rot / 90) % 4) + 4) % 4;
}

/**
 * Pose que encuadra `box` (zona normalizada de la imagen, o la imagen entera) según el ajuste: a
 * la ventana (entera, como siempre), al ancho o al alto. Si la zona no cabe, se empieza por arriba
 * (al ancho) o por el lado por el que se lee (al alto); `end` empieza por el final (al volver a
 * la página anterior).
 */
export function framePose(img: Size, vp: Size, rot: number, fit: FitMode, box: Box | null, rtl: boolean, end = false): Pose {
  const [bx, by, bw, bh] = box ?? WHOLE;
  const q = quarter(rot);
  const rw = bw * img.w;
  const rh = bh * img.h;
  const [sw, sh] = q % 2 ? [rh, rw] : [rw, rh];
  const s = fit === "width" ? vp.w / sw : fit === "height" ? vp.h / sh : Math.min(vp.w / sw, vp.h / sh);
  const base = Math.min(vp.w / img.w, vp.h / img.h);
  const zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, s / base));
  let cx = bx + bw / 2;
  let cy = by + bh / 2;
  // Solo sin girar: con la página girada se encuadra el centro.
  if (q === 0) {
    const scale = base * zoom;
    const halfH = vp.h / 2 / scale / img.h;
    const halfW = vp.w / 2 / scale / img.w;
    if (fit === "width" && bh > halfH * 2) cy = end ? by + bh - halfH : by + halfH;
    if (fit === "height" && bw > halfW * 2) {
      const fromRight = rtl !== end;
      cx = fromRight ? bx + bw - halfW : bx + halfW;
    }
  }
  return new Float64Array([cx, cy, zoom, rot]);
}

/** Unión de recuadros (`null` si no hay ninguno). */
export function unionBox(list: (Box | null)[]): Box | null {
  const b = list.filter((x): x is Box => !!x);
  if (!b.length) return null;
  const x0 = Math.min(...b.map((r) => r[0]));
  const y0 = Math.min(...b.map((r) => r[1]));
  const x1 = Math.max(...b.map((r) => r[0] + r[2]));
  const y1 = Math.max(...b.map((r) => r[1] + r[3]));
  return [x0, y0, x1 - x0, y1 - y0];
}

/**
 * Páginas que se ven juntas en doble página: `[i]` o `[i, i + 1]` (en orden de lectura), las que
 * contienen la página `at`. Van solas la portada (si se pide), las páginas apaisadas (ya son una
 * doble) y la última si queda suelta; tras una apaisada se vuelve a emparejar desde la siguiente.
 */
export function spreadOf(at: number, count: number, cover: boolean, wide: (i: number) => boolean): number[] {
  let s = 0;
  while (s < count) {
    const single = (cover && s === 0) || wide(s) || s + 1 >= count || wide(s + 1);
    const span = single ? 1 : 2;
    if (at < s + span) return single ? [s] : [s, s + 1];
    s += span;
  }
  return [Math.max(0, count - 1)];
}

/** Primera página del grupo anterior y del siguiente al que contiene `at`. */
export function spreadStep(at: number, dir: 1 | -1, count: number, cover: boolean, wide: (i: number) => boolean): number {
  const cur = spreadOf(at, count, cover, wide);
  if (dir > 0) return Math.min(count - 1, cur[cur.length - 1] + 1);
  return cur[0] <= 0 ? 0 : spreadOf(cur[0] - 1, count, cover, wide)[0];
}

/** Lado de la columna de la tira vertical a zoom 1: el ancho de la ventana, o algo menos si es apaisada. */
export function stripWidth(vp: Size, full: boolean): number {
  return full ? vp.w : Math.min(vp.w, Math.max(560, vp.h * 0.9));
}

/** Cómo se colocan las páginas en el visor fuera de la reproducción y del editor. */
export type ReadLayout =
  /** Dos páginas juntas, ya en el orden en que se ven de izquierda a derecha. */
  | { kind: "double"; pages: PictureRef[] }
  /** Tira vertical continua de toda la fuente; `at` es la página actual. */
  | { kind: "strip"; id: string; count: number; at: number };

/** Encuadre de lectura: a qué se ajusta y, con ✂, el recuadro con contenido de cada página. */
export interface Framing {
  fit: FitMode;
  rtl: boolean;
  box: ((index: number) => Box | null) | null;
}
