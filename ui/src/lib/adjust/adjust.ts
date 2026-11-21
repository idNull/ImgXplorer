// Ajustes de imagen (🎨): lo que se guarda en el proyecto y cómo se convierte para el shader.
// Mismo esquema que crates/imgx-core/src/adjust.rs: todo es opcional y 0 es neutro.
import { ADJ_LEN } from "../viewer/gl";

export type Tone = "none" | "gray" | "sepia";

export interface Adjust {
  /** «Limpiar escaneo» 0..1 */
  clean?: number;
  brightness?: number;
  contrast?: number;
  saturation?: number;
  /** Tonos medios −1..1 */
  exposure?: number;
  temperature?: number;
  /** 0..1 */
  sharpen?: number;
  tone?: Tone;
}

export type AdjustKey = Exclude<keyof Adjust, "tone">;

/** Color del papel y de la tinta (lo calcula Rust: `image_levels`). */
export interface Levels {
  black: [number, number, number];
  white: [number, number, number];
}

export const SLIDERS: { key: AdjustKey; min: number; max: number }[] = [
  { key: "clean", min: 0, max: 1 },
  { key: "brightness", min: -1, max: 1 },
  { key: "contrast", min: -1, max: 1 },
  { key: "saturation", min: -1, max: 1 },
  { key: "exposure", min: -1, max: 1 },
  { key: "temperature", min: -1, max: 1 },
  { key: "sharpen", min: 0, max: 1 },
];

/** Ajustes de un clic. «Noche» va aparte: es de lectura y se suma a cualquiera de estos. */
export const PRESETS: { id: "original" | "clean" | "gray" | "sepia"; icon: string; adj: Adjust }[] = [
  { id: "original", icon: "🖼", adj: {} },
  { id: "clean", icon: "✨", adj: { clean: 1 } },
  { id: "gray", icon: "🔳", adj: { tone: "gray" } },
  { id: "sepia", icon: "🟫", adj: { tone: "sepia" } },
];

/** Quita lo neutro (0, tono «none»); `null` si no queda nada. */
export function normalize(a: Adjust | null | undefined): Adjust | null {
  if (!a) return null;
  const out: Adjust = {};
  for (const { key, min, max } of SLIDERS) {
    const v = a[key];
    if (typeof v === "number" && Number.isFinite(v) && v !== 0) out[key] = Math.min(max, Math.max(min, v));
  }
  if (a.tone && a.tone !== "none") out.tone = a.tone;
  return Object.keys(out).length ? out : null;
}

export const isNeutral = (a: Adjust | null | undefined) => normalize(a) === null;

/** El ajuste coincide con uno de un clic (para marcar su botón). */
export function presetOf(a: Adjust | null | undefined): string | null {
  const n = JSON.stringify(normalize(a) ?? {});
  return PRESETS.find((p) => JSON.stringify(normalize(p.adj) ?? {}) === n)?.id ?? null;
}

/**
 * Los 16 valores del shader de capas (`Layer.adj`), o `null` si no hay nada que hacer. Sin los
 * niveles de la página, «limpiar escaneo» se queda en espera (no se aplica a medias).
 */
/** Modo noche de lectura: apagado, o su estilo e intensidad (0..1, solo cuenta en «warm»). */
export type Night = null | { style: "warm" | "invert"; level: number };

export function adjustUniforms(a: Adjust | null | undefined, levels: Levels | null, night: Night): Float32Array | null {
  const n = normalize(a);
  if (!n && !night) return null;
  const u = new Float32Array(ADJ_LEN);
  u[0] = 1;
  const lv = levels ?? { black: [0, 0, 0], white: [1, 1, 1] };
  u.set(lv.black, 1);
  u.set(lv.white, 4);
  u[7] = levels ? (n?.clean ?? 0) : 0;
  u[8] = n?.brightness ?? 0;
  u[9] = n?.contrast ?? 0;
  u[10] = n?.saturation ?? 0;
  u[11] = n?.exposure ?? 0;
  u[12] = n?.temperature ?? 0;
  u[13] = n?.sharpen ?? 0;
  u[14] = n?.tone === "gray" ? 1 : n?.tone === "sepia" ? 2 : 0;
  u[15] = !night ? 0 : night.style === "invert" ? 1 : 2 + Math.min(1, Math.max(0, night.level));
  return u;
}
