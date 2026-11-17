// Edición de viñetas sobre la imagen: arrastrar para dibujar una nueva, arrastrar una viñeta para
// moverla y sus tiradores para cambiarla de tamaño. Todo en coordenadas normalizadas de imagen,
// así funciona igual con la imagen girada o con zoom.
import type { Affine, Size } from "../core";

export interface PanelEdit {
  /** `[x, y, w, h, …]` normalizadas, en el orden en que se guardan. */
  rects: number[];
  /** Viñeta seleccionada (-1: ninguna). */
  selected: number;
  select(i: number): void;
  change(rects: number[]): void;
  /** Inicio/fin de un arrastre (para agruparlo en un paso de deshacer). */
  gesture(active: boolean): void;
}

export interface Drag {
  move(x: number, y: number): void;
  up(): void;
}

const HANDLE_PX = 9;
const MIN = 0.01;

/** Punto de pantalla (px CSS) → punto normalizado de la imagen. */
export function toImage(m: Affine, img: Size, x: number, y: number): [number, number] {
  const [a, b, c, d, e, f] = m;
  const det = a * d - b * c;
  const px = x - e;
  const py = y - f;
  return [(d * px - c * py) / det / img.w, (-b * px + a * py) / det / img.h];
}

export function toScreen(m: Affine, img: Size, x: number, y: number): [number, number] {
  return [m[0] * x * img.w + m[2] * y * img.h + m[4], m[1] * x * img.w + m[3] * y * img.h + m[5]];
}

/** Tiradores: esquinas y centros de los lados, como [sx, sy] (-1 izquierda/arriba, 1 derecha/abajo). */
export const HANDLES: [number, number][] = [
  [-1, -1], [0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0],
];

export function handlePoint(r: ArrayLike<number>, [sx, sy]: [number, number]): [number, number] {
  return [r[0] + ((sx + 1) / 2) * r[2], r[1] + ((sy + 1) / 2) * r[3]];
}

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

function rectOf(rects: number[], i: number): [number, number, number, number] {
  return [rects[i * 4], rects[i * 4 + 1], rects[i * 4 + 2], rects[i * 4 + 3]];
}

function withRect(rects: number[], i: number, r: [number, number, number, number]): number[] {
  const out = rects.slice();
  out.splice(i * 4, 4, ...r);
  return out;
}

/** Cursor según lo que hay bajo el puntero. */
export function cursorAt(edit: PanelEdit, m: Affine, img: Size, x: number, y: number): string {
  if (handleAt(edit, m, img, x, y)) return "crosshair";
  return rectAt(edit.rects, ...toImage(m, img, x, y)) >= 0 ? "move" : "copy";
}

function handleAt(edit: PanelEdit, m: Affine, img: Size, x: number, y: number): [number, number] | null {
  if (edit.selected < 0 || edit.selected * 4 >= edit.rects.length) return null;
  const r = rectOf(edit.rects, edit.selected);
  for (const h of HANDLES) {
    const [hx, hy] = toScreen(m, img, ...handlePoint(r, h));
    if (Math.hypot(hx - x, hy - y) <= HANDLE_PX) return h;
  }
  return null;
}

/** La viñeta más pequeña que contiene el punto (así se puede elegir una dentro de otra). */
function rectAt(rects: number[], u: number, v: number): number {
  let best = -1;
  let area = Infinity;
  for (let i = 0; i * 4 < rects.length; i++) {
    const [x, y, w, h] = rectOf(rects, i);
    if (u >= x && u <= x + w && v >= y && v <= y + h && w * h < area) {
      best = i;
      area = w * h;
    }
  }
  return best;
}

/** Empieza un arrastre en (x, y) px. `view()` da la transformación vigente en cada momento. */
export function press(edit: PanelEdit, view: () => { m: Affine; img: Size }, x: number, y: number): Drag {
  const { m, img } = view();
  const start = toImage(m, img, x, y);
  const at = (px: number, py: number) => {
    const v = view();
    return toImage(v.m, v.img, px, py);
  };
  const handle = handleAt(edit, m, img, x, y);
  let rects = edit.rects;
  let moved = false;
  edit.gesture(true);

  if (handle) {
    const i = edit.selected;
    const [ox, oy, ow, oh] = rectOf(rects, i);
    return {
      move(px, py) {
        const [u, v] = at(px, py);
        let [x1, y1, x2, y2] = [ox, oy, ox + ow, oy + oh];
        if (handle[0] < 0) x1 = Math.min(clamp01(u), x2 - MIN);
        if (handle[0] > 0) x2 = Math.max(clamp01(u), x1 + MIN);
        if (handle[1] < 0) y1 = Math.min(clamp01(v), y2 - MIN);
        if (handle[1] > 0) y2 = Math.max(clamp01(v), y1 + MIN);
        rects = withRect(rects, i, [x1, y1, x2 - x1, y2 - y1]);
        edit.change(rects);
      },
      up: () => edit.gesture(false),
    };
  }

  const hit = rectAt(rects, ...start);
  if (hit >= 0) {
    edit.select(hit);
    const [ox, oy, ow, oh] = rectOf(rects, hit);
    return {
      move(px, py) {
        const [u, v] = at(px, py);
        const nx = Math.min(1 - ow, Math.max(0, ox + u - start[0]));
        const ny = Math.min(1 - oh, Math.max(0, oy + v - start[1]));
        rects = withRect(rects, hit, [nx, ny, ow, oh]);
        edit.change(rects);
      },
      up: () => edit.gesture(false),
    };
  }

  // En vacío: dibujar una viñeta nueva.
  const idx = rects.length / 4;
  const [sx, sy] = [x, y];
  return {
    move(px, py) {
      if (!moved && Math.hypot(px - sx, py - sy) < 6) return;
      const [u, v] = at(px, py);
      const [a, b] = [clamp01(start[0]), clamp01(start[1])];
      const [c, d] = [clamp01(u), clamp01(v)];
      const r: [number, number, number, number] = [Math.min(a, c), Math.min(b, d), Math.max(MIN, Math.abs(c - a)), Math.max(MIN, Math.abs(d - b))];
      rects = moved ? withRect(rects, idx, r) : [...rects, ...r];
      if (!moved) edit.select(idx);
      moved = true;
      edit.change(rects);
    },
    up() {
      if (!moved) edit.select(-1);
      edit.gesture(false);
    },
  };
}

/** Quita la viñeta `i`. */
export function removeRect(rects: number[], i: number): number[] {
  const out = rects.slice();
  out.splice(i * 4, 4);
  return out;
}
