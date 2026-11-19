// Tira vertical (webtoon): las páginas una debajo de otra, del mismo ancho, con desplazamiento
// continuo. La posición se guarda respecto a una página «ancla» (la de arriba de la vista), así
// que cuando se conoce el alto de una página que aún no se había cargado la vista no salta.
import type { Size } from "../core";
import { stripWidth } from "./layout";

const MIN_ZOOM = 0.25;
const MAX_ZOOM = 6;
/** Separación entre páginas (px de pantalla). */
const GAP = 0;

export interface StripPage {
  index: number;
  /** Arriba de la página en la vista (px). */
  y: number;
  h: number;
}

export class Strip {
  /** Alto / ancho de cada página conocida. */
  private aspects = new Map<number, number>();
  /** Proporción supuesta de las que aún no se conocen (la de la primera conocida). */
  private guess = 1.5;
  zoom = 1;
  /** Página de arriba y px desde su borde superior hasta el de la vista. */
  private a = 0;
  private o = 0;
  /** Desplazamiento horizontal (px), con zoom mayor que la ventana. */
  private x = 0;
  /** Desplazamiento pendiente con animación (px). */
  private pending = 0;
  /** Columna a todo el ancho de la ventana (ajuste «al ancho»). */
  full = false;

  readonly count: number;

  constructor(count: number, at = 0) {
    this.count = count;
    this.a = Math.max(0, Math.min(count - 1, at));
  }

  colW(vp: Size): number {
    return stripWidth(vp, this.full) * this.zoom;
  }

  private h(i: number, vp: Size): number {
    return this.colW(vp) * (this.aspects.get(i) ?? this.guess) + GAP;
  }

  /** Se conoce el tamaño de una página. Devuelve si cambia algo. */
  setSize(i: number, s: Size): boolean {
    const r = s.h / Math.max(1, s.w);
    if (this.aspects.get(i) === r) return false;
    if (!this.aspects.size) this.guess = r;
    this.aspects.set(i, r);
    return true;
  }

  /** Va al principio de una página (o al final de la tira con `end`). */
  jump(i: number, vp: Size, end = false) {
    this.pending = 0;
    this.a = Math.max(0, Math.min(this.count - 1, i));
    this.o = end ? this.h(this.a, vp) : 0;
    this.clamp(vp);
  }

  /** Desplaza `dy` px (positivo: hacia abajo de la tira). Con `smooth`, poco a poco. */
  scroll(dy: number, vp: Size, smooth = false) {
    if (smooth) {
      this.pending += dy;
      return;
    }
    this.o += dy;
    this.clamp(vp);
  }

  /** Lo que queda de la animación de desplazamiento; devuelve si sigue moviéndose. */
  step(dt: number, vp: Size): boolean {
    if (Math.abs(this.pending) < 0.5) {
      this.pending = 0;
      return false;
    }
    const k = 1 - Math.exp(-dt / 70);
    const d = this.pending * k;
    this.pending -= d;
    const before = this.position();
    this.o += d;
    this.clamp(vp);
    // Tope de la tira: no seguir empujando.
    if (this.position() === before) this.pending = 0;
    return true;
  }

  private position(): string {
    return `${this.a}:${this.o.toFixed(2)}`;
  }

  pan(dx: number, vp: Size) {
    this.x += dx;
    this.clamp(vp);
  }

  /** Zoom manteniendo fijo el punto (px, py) de la vista. */
  zoomAt(f: number, px: number, py: number, vp: Size) {
    const z = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, this.zoom * f));
    const r = z / this.zoom;
    if (r === 1) return;
    this.zoom = z;
    this.o = (this.o + py) * r - py;
    // El centro de la columna está en vp.w / 2 + x.
    this.x = px - vp.w / 2 - (px - vp.w / 2 - this.x) * r;
    this.clamp(vp);
  }

  reset(vp: Size) {
    this.zoom = 1;
    this.x = 0;
    this.clamp(vp);
  }

  private clamp(vp: Size) {
    // El ancla es la página que toca el borde de arriba de la vista.
    while (this.o < 0 && this.a > 0) {
      this.a--;
      this.o += this.h(this.a, vp);
    }
    while (this.a < this.count - 1 && this.o >= this.h(this.a, vp)) {
      this.o -= this.h(this.a, vp);
      this.a++;
    }
    // Final de la tira: la última página no sube más allá del borde de abajo.
    let rest = -this.o;
    for (let i = this.a; i < this.count; i++) {
      rest += this.h(i, vp);
      if (rest >= vp.h) break;
    }
    if (rest < vp.h) {
      this.o -= vp.h - rest;
      while (this.o < 0 && this.a > 0) {
        this.a--;
        this.o += this.h(this.a, vp);
      }
    }
    if (this.a === 0 && this.o < 0) this.o = 0;
    const over = Math.max(0, (this.colW(vp) - vp.w) / 2);
    this.x = Math.min(over, Math.max(-over, this.x));
  }

  /** Páginas que se ven, de arriba abajo, y `extra` más por debajo (para precargarlas). */
  visible(vp: Size, extra = 0): StripPage[] {
    const out: StripPage[] = [];
    let y = -this.o;
    let after = 0;
    for (let i = this.a; i < this.count; i++) {
      const h = this.h(i, vp);
      if (y >= vp.h && ++after > extra) break;
      out.push({ index: i, y, h: h - GAP });
      y += h;
    }
    return out;
  }

  /** La página en el centro de la vista (la que cuenta como «actual»). */
  center(vp: Size): number {
    for (const p of this.visible(vp)) if (p.y + p.h >= vp.h / 2) return p.index;
    return Math.min(this.count - 1, this.a);
  }

  /** Columna en la vista: x de su borde izquierdo y ancho. */
  column(vp: Size): { x: number; w: number } {
    const w = this.colW(vp);
    return { x: (vp.w - w) / 2 + this.x, w };
  }

  atStart(): boolean {
    return this.a === 0 && this.o <= 0.5 && this.pending <= 0;
  }

  /** Se ve el final de la última página. */
  atEnd(vp: Size): boolean {
    if (this.pending > 0) return false;
    let rest = -this.o;
    for (let i = this.a; i < this.count; i++) {
      rest += this.h(i, vp);
      if (rest > vp.h + 0.5) return false;
    }
    return true;
  }
}
