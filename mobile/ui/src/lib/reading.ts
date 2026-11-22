// Cómo se lee (para todos los cómics): ajuste de la página y sentido manga. Y cómo se ven las
// historias al reproducirlas: encuadre, calidad y silencio. Se recuerda en el almacenamiento del WebView.
export type Mode = "page" | "width" | "strip";
/**
 * `auto`: en horizontal, como se pensó; en vertical, toda la pantalla (la cámara se acerca a cada
 * viñeta). `authored`: la proporción con la que se pensó la cámara (con bandas). `screen`: toda la
 * pantalla.
 */
export type StoryFit = "auto" | "authored" | "screen";
/** Resolución al reproducir: `auto` la baja si no va fluido; `saver` ahorra batería. */
export type Quality = "auto" | "high" | "saver";

export interface Reading {
  mode: Mode;
  rtl: boolean;
  /** Se guarda como `storyFraming`: el antiguo `storyFit` (sin «auto») se olvida. */
  storyFit: StoryFit;
  quality: Quality;
  /** Historias sin sonido. */
  muted: boolean;
}

const KEY = "imgx.reading";
const DEFAULTS: Reading = { mode: "page", rtl: false, storyFit: "auto", quality: "auto", muted: false };

const pick = <T extends string>(v: unknown, all: readonly T[], def: T): T => (all.includes(v as T) ? (v as T) : def);

function load(): Reading {
  try {
    const r = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return {
      mode: pick(r.mode, ["page", "width", "strip"], "page"),
      rtl: !!r.rtl,
      storyFit: pick(r.storyFraming, ["auto", "authored", "screen"], "auto"),
      quality: pick(r.quality, ["auto", "high", "saver"], "auto"),
      muted: !!r.muted,
    };
  } catch {
    return { ...DEFAULTS };
  }
}

export const reading: Reading = load();

export function saveReading(r: Partial<Reading>) {
  Object.assign(reading, r);
  const { storyFit, ...rest } = reading;
  localStorage.setItem(KEY, JSON.stringify({ ...rest, storyFraming: storyFit }));
}
