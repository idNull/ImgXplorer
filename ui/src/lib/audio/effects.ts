// Catálogo de efectos de sonido: qué mandos tiene cada uno (con los mismos valores por defecto
// que el motor de Rust, `crates/imgx-audio/src/fx`) y sus ajustes predefinidos. Sin Tauri.

export type FxType = "reverb" | "delay" | "eq" | "filter" | "compressor" | "pitch" | "robot" | "distortion";

/** Un efecto de una pista o de un clip, tal como se guarda en el proyecto. */
export interface Effect {
  id: string;
  type: FxType;
  on: boolean;
  /** Ajuste predefinido del que salió; desaparece al tocar un mando. */
  preset?: string;
  params: Record<string, number>;
}

export interface ParamDef {
  key: string;
  min: number;
  max: number;
  step: number;
  def: number;
  /** Cómo se enseña el valor. */
  unit: "%" | "dB" | "ms" | "Hz" | "st" | ":1" | "onoff";
  /** Mando logarítmico (frecuencias, tiempos largos). */
  log?: boolean;
}

export interface FxDef {
  type: FxType;
  icon: string;
  params: ParamDef[];
  presets: { id: string; params: Record<string, number> }[];
}

const pct = (key: string, def: number): ParamDef => ({ key, min: 0, max: 1, step: 0.01, def, unit: "%" });

export const FX: Record<FxType, FxDef> = {
  reverb: {
    type: "reverb",
    icon: "⛪",
    params: [pct("size", 0.5), pct("damping", 0.5), pct("mix", 0.3), pct("width", 1), { key: "predelay", min: 0, max: 250, step: 1, def: 0, unit: "ms" }],
    presets: [
      { id: "room", params: { size: 0.35, damping: 0.6, mix: 0.2, width: 0.8, predelay: 0 } },
      { id: "hall", params: { size: 0.7, damping: 0.5, mix: 0.3, width: 1, predelay: 20 } },
      { id: "cathedral", params: { size: 0.92, damping: 0.3, mix: 0.4, width: 1, predelay: 50 } },
      { id: "cave", params: { size: 0.97, damping: 0.15, mix: 0.45, width: 1, predelay: 80 } },
      { id: "bathroom", params: { size: 0.45, damping: 0.1, mix: 0.35, width: 0.6, predelay: 0 } },
    ],
  },
  delay: {
    type: "delay",
    icon: "🔁",
    params: [
      { key: "time", min: 10, max: 2000, step: 1, def: 350, unit: "ms", log: true },
      { key: "feedback", min: 0, max: 0.95, step: 0.01, def: 0.35, unit: "%" },
      { key: "tone", min: 500, max: 20000, step: 1, def: 6000, unit: "Hz", log: true },
      pct("mix", 0.35),
      { key: "pingpong", min: 0, max: 1, step: 1, def: 0, unit: "onoff" },
    ],
    presets: [
      { id: "slapback", params: { time: 90, feedback: 0.1, tone: 8000, mix: 0.4, pingpong: 0 } },
      { id: "echo", params: { time: 350, feedback: 0.35, tone: 6000, mix: 0.35, pingpong: 0 } },
      { id: "mountain", params: { time: 600, feedback: 0.5, tone: 3500, mix: 0.45, pingpong: 0 } },
      { id: "pingpong", params: { time: 300, feedback: 0.45, tone: 7000, mix: 0.4, pingpong: 1 } },
    ],
  },
  eq: {
    type: "eq",
    icon: "🎚",
    params: ["low", "mid", "high"].map((key) => ({ key, min: -12, max: 12, step: 0.5, def: 0, unit: "dB" as const })),
    presets: [
      { id: "body", params: { low: 4, mid: 0, high: -1 } },
      { id: "clarity", params: { low: -2, mid: 2, high: 4 } },
      { id: "nobass", params: { low: -12, mid: 0, high: 0 } },
    ],
  },
  filter: {
    type: "filter",
    icon: "📞",
    params: [
      { key: "low", min: 20, max: 5000, step: 1, def: 20, unit: "Hz", log: true },
      { key: "high", min: 200, max: 20000, step: 1, def: 20000, unit: "Hz", log: true },
      pct("drive", 0),
    ],
    presets: [
      { id: "phone", params: { low: 300, high: 3400, drive: 0.25 } },
      { id: "radio", params: { low: 500, high: 4500, drive: 0.15 } },
      { id: "megaphone", params: { low: 600, high: 3000, drive: 0.6 } },
      { id: "underwater", params: { low: 20, high: 500, drive: 0 } },
      { id: "far", params: { low: 150, high: 2500, drive: 0 } },
      { id: "wall", params: { low: 20, high: 900, drive: 0 } },
    ],
  },
  compressor: {
    type: "compressor",
    icon: "🗜",
    params: [
      { key: "threshold", min: -60, max: 0, step: 1, def: -18, unit: "dB" },
      { key: "ratio", min: 1, max: 20, step: 0.5, def: 4, unit: ":1" },
      { key: "attack", min: 0.1, max: 200, step: 0.1, def: 10, unit: "ms", log: true },
      { key: "release", min: 10, max: 2000, step: 1, def: 150, unit: "ms", log: true },
      { key: "makeup", min: 0, max: 24, step: 0.5, def: 6, unit: "dB" },
    ],
    presets: [
      { id: "voice", params: { threshold: -18, ratio: 4, attack: 10, release: 150, makeup: 6 } },
      { id: "gentle", params: { threshold: -12, ratio: 2, attack: 20, release: 250, makeup: 3 } },
      { id: "strong", params: { threshold: -28, ratio: 8, attack: 5, release: 120, makeup: 12 } },
    ],
  },
  pitch: {
    type: "pitch",
    icon: "🎵",
    params: [{ key: "semitones", min: -12, max: 12, step: 1, def: -5, unit: "st" }, pct("mix", 1)],
    presets: [
      { id: "monster", params: { semitones: -9, mix: 1 } },
      { id: "deep", params: { semitones: -4, mix: 1 } },
      { id: "high", params: { semitones: 4, mix: 1 } },
      { id: "chipmunk", params: { semitones: 9, mix: 1 } },
    ],
  },
  robot: {
    type: "robot",
    icon: "🤖",
    params: [{ key: "freq", min: 10, max: 1000, step: 1, def: 50, unit: "Hz", log: true }, { ...pct("metal", 0.4), max: 0.9 }, pct("mix", 1)],
    presets: [
      { id: "robot", params: { freq: 50, metal: 0.5, mix: 1 } },
      { id: "alien", params: { freq: 180, metal: 0.2, mix: 0.8 } },
      { id: "metallic", params: { freq: 30, metal: 0.8, mix: 1 } },
    ],
  },
  distortion: {
    type: "distortion",
    icon: "⚡",
    params: [pct("drive", 0.5), { key: "tone", min: 300, max: 20000, step: 1, def: 6000, unit: "Hz", log: true }, pct("mix", 1)],
    presets: [
      { id: "soft", params: { drive: 0.2, tone: 8000, mix: 0.6 } },
      { id: "hard", params: { drive: 0.8, tone: 5000, mix: 1 } },
      { id: "brokenradio", params: { drive: 0.9, tone: 2500, mix: 1 } },
    ],
  },
};

export const FX_TYPES = Object.keys(FX) as FxType[];

const newId = () => `fx-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;

/** Efecto nuevo con su primer ajuste predefinido. */
export function newEffect(type: FxType): Effect {
  const p = FX[type].presets[0];
  return { id: newId(), type, on: true, preset: p.id, params: { ...p.params } };
}

/** Copia de una cadena de efectos con ids nuevos (al duplicar un clip). */
export const cloneEffects = (list: Effect[] | undefined): Effect[] | undefined =>
  list?.length ? list.map((e) => ({ ...e, id: newId(), params: { ...e.params } })) : undefined;

/** Valor de un mando (el de por defecto si no está). */
export function paramValue(e: Effect, p: ParamDef): number {
  const v = e.params[p.key];
  return typeof v === "number" && Number.isFinite(v) ? Math.min(p.max, Math.max(p.min, v)) : p.def;
}

/** Posición 0–1 del deslizador para un valor (logarítmica en los mandos `log`). */
export function toSlider(p: ParamDef, v: number): number {
  if (p.log) return Math.log(v / p.min) / Math.log(p.max / p.min);
  return (v - p.min) / (p.max - p.min);
}

export function fromSlider(p: ParamDef, x: number): number {
  const v = p.log ? p.min * Math.pow(p.max / p.min, x) : p.min + x * (p.max - p.min);
  const r = Math.round(v / p.step) * p.step;
  return Math.min(p.max, Math.max(p.min, Number(r.toFixed(4))));
}

/** El valor como se enseña. */
export function formatParam(p: ParamDef, v: number): string {
  switch (p.unit) {
    case "%":
      return `${Math.round(v * 100)}%`;
    case "dB":
      return `${v > 0 ? "+" : ""}${v.toFixed(Number.isInteger(v) ? 0 : 1)} dB`;
    case "ms":
      return v < 10 ? `${v.toFixed(1)} ms` : `${Math.round(v)} ms`;
    case "Hz":
      return v >= 1000 ? `${(v / 1000).toFixed(v >= 10000 ? 0 : 1)} kHz` : `${Math.round(v)} Hz`;
    case "st":
      return `${v > 0 ? "+" : ""}${v}`;
    case ":1":
      return `${v}:1`;
    case "onoff":
      return v >= 0.5 ? "✓" : "–";
  }
}

/** Hay algún efecto encendido. */
export const hasEffects = (list: Effect[] | undefined) => !!list?.some((e) => e.on);
