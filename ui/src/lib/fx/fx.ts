// Efectos especiales visuales de una imagen (temblor, destello, onda…). El cálculo de cada
// fotograma lo hace el núcleo (`imgx-core/fx.rs`) y el dibujo `GlViewer.draw(..., fx)`.
import { wasm, type Pose, type Size } from "../core";
import { FX_LEN, type FxFrame } from "../viewer/gl";
import { fxInfo, fxParam } from "./catalog";
import type { TextDraw } from "./text";

/** Valores de cada texto detrás de los del shader (`TextSprite::LEN` del núcleo). */
const SPRITE_LEN = 11;

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
  private constructor(
    private w: wasm.WasmFx,
    private list: Fx[],
  ) {}

  /** `null` si no hay efectos (así no se calcula nada en cada fotograma). */
  static of(list: readonly Fx[] | undefined): FxTrack | null {
    if (!list?.length) return null;
    const json = JSON.stringify(list);
    return new FxTrack(new wasm.WasmFx(json), JSON.parse(json));
  }

  /**
   * Efectos en `t` (segundos de la imagen) con la pose de ese instante; `null` si ninguno. Los
   * textos (onomatopeyas, subtítulos) van en `texts`.
   */
  frame(t: number, img: Size, pose: Pose, vp: Size): FxFrame | null {
    const f = this.w.frame(t, img.w, img.h, pose, vp.w, vp.h);
    if (!f.length) return null;
    if (f.length <= FX_LEN) return f;
    const texts: TextDraw[] = [];
    for (let o = FX_LEN; o + SPRITE_LEN <= f.length; o += SPRITE_LEN) {
      const fx = this.list[f[o]];
      const info = fx && fxInfo(fx.type);
      if (!info) continue;
      const p = (key: string) => {
        const spec = info.params.find((x) => x.key === key);
        return spec ? String(fxParam(fx, spec)) : "";
      };
      texts.push({
        kind: fx.type === "caption" ? "caption" : "text",
        text: p("text"),
        color: p("color"),
        outline: p("outline") || "#000000",
        style: p("style"),
        overlay: f[o + 1] > 0,
        x: f[o + 2],
        y: f[o + 3],
        ax: f[o + 4],
        ay: f[o + 5],
        line: f[o + 6],
        wrap: f[o + 7],
        scale: f[o + 8],
        rot: f[o + 9],
        alpha: f[o + 10],
      });
    }
    return Object.assign(f.subarray(0, FX_LEN), { texts });
  }

  free() {
    this.w.free();
  }
}
