// Cómo se lee (para todos los cómics): ajuste de la página y sentido manga. Y cómo se ven las
// historias al reproducirlas: encuadre y calidad. Se recuerda en el almacenamiento del WebView.
export type Mode = "page" | "width" | "strip";
/** `authored`: la proporción con la que se pensó la cámara (con bandas). `screen`: toda la pantalla. */
export type StoryFit = "authored" | "screen";
/** Resolución al reproducir: `auto` la baja si no va fluido; `saver` ahorra batería. */
export type Quality = "auto" | "high" | "saver";

export interface Reading {
  mode: Mode;
  rtl: boolean;
  storyFit: StoryFit;
  quality: Quality;
}

const KEY = "imgx.reading";
const DEFAULTS: Reading = { mode: "page", rtl: false, storyFit: "authored", quality: "auto" };

const pick = <T extends string>(v: unknown, all: readonly T[], def: T): T => (all.includes(v as T) ? (v as T) : def);

function load(): Reading {
  try {
    const r = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return {
      mode: pick(r.mode, ["page", "width", "strip"], "page"),
      rtl: !!r.rtl,
      storyFit: pick(r.storyFit, ["authored", "screen"], "authored"),
      quality: pick(r.quality, ["auto", "high", "saver"], "auto"),
    };
  } catch {
    return { ...DEFAULTS };
  }
}

export const reading: Reading = load();

export function saveReading(r: Partial<Reading>) {
  Object.assign(reading, r);
  localStorage.setItem(KEY, JSON.stringify(reading));
}
