// Efectos especiales visuales de una imagen (temblor, destello, onda…). El cálculo de cada
// fotograma lo hace el núcleo (`imgx-core/fx.rs`) y el dibujo `GlViewer.draw(..., fx)`.
import { wasm, type Pose, type Size } from "../core";
import type { FxFrame } from "../viewer/gl";

/** Un efecto en la timeline de una imagen (`timeline.fx`). */
export interface Fx {
  id: string;
  type: string;
  /** Segundo de la imagen en que empieza. */
  start: number;
  duration: number;
  intensity?: number;
  preset?: string;
  params?: Record<string, number | string>;
  /** Punto de la imagen (0..1) donde ocurre; sin él, el centro de la pantalla. */
  at?: { x: number; y: number };
}

export const MIN_FX_DURATION = 0.05;

/** Final del último efecto (0 si no hay). */
export function fxEnd(list: readonly Fx[] | undefined): number {
  return (list ?? []).reduce((m, f) => Math.max(m, f.start + f.duration), 0);
}

let seq = 0;
/** Id nuevo para un efecto (único en la sesión y entre sesiones). */
export function fxId(): string {
  return `fx${Date.now().toString(36)}${(seq++).toString(36)}`;
}

/** Efectos de una imagen listos para calcular fotogramas. Hay que llamar a `free()`. */
export class FxTrack {
  private constructor(private w: wasm.WasmFx) {}

  /** `null` si no hay efectos (así no se calcula nada en cada fotograma). */
  static of(list: readonly Fx[] | undefined): FxTrack | null {
    if (!list?.length) return null;
    return new FxTrack(new wasm.WasmFx(JSON.stringify(list)));
  }

  /** Efectos en `t` (segundos de la imagen) con la pose de ese instante; `null` si ninguno. */
  frame(t: number, img: Size, pose: Pose, vp: Size): FxFrame | null {
    const f = this.w.frame(t, img.w, img.h, pose, vp.w, vp.h);
    return f.length ? f : null;
  }

  free() {
    this.w.free();
  }
}
