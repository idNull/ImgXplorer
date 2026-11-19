// Voces con IA: reparto, guion y lo que se pide al proveedor (ver `src-tauri/src/voices`).

/** Voz de un proveedor. */
export interface VoiceRef {
  provider: string;
  id: string;
  name?: string;
}

/** Un personaje del reparto. */
export interface CastMember {
  id: string;
  name: string;
  color?: string;
  /** Aspecto, edad, carácter y forma de hablar: es el «prompt» de su voz. */
  description?: string;
  /** ISO 639-1 (`es`, `en`…). */
  language?: string;
  voice?: VoiceRef;
  /** stability, similarity, style (0..1) y speed (0,7..1,2). */
  settings?: Record<string, number>;
}

export type LineKind = "dialogue" | "thought" | "narration" | "crowd";
export type LineStatus = "draft" | "generated" | "stale";

/** Una frase del guion de una imagen. */
export interface Line {
  id: string;
  cast?: string;
  text: string;
  kind: LineKind;
  /** Segundo de la imagen en que empieza (si falta, detrás de la anterior). */
  start?: number;
  status?: LineStatus;
  /** Sonidos generados; el último es el que suena. */
  takes?: string[];
}

export const LINE_KINDS: LineKind[] = ["dialogue", "thought", "narration", "crowd"];

/** Lo que se pide al proveedor (igual que `GenItem` en Rust). */
export type GenItem =
  | { kind: "speech"; voice: string; model: string; text: string; language?: string; settings?: Record<string, number>; seed?: number }
  | { kind: "sfx"; text: string; seconds?: number; loop?: boolean }
  | { kind: "music"; prompt: string; seconds: number; instrumental?: boolean };

export interface VoiceInfo {
  id: string;
  name: string;
  category?: string | null;
  description?: string | null;
  labels: Record<string, string>;
  previewUrl?: string | null;
}

export interface AccountInfo {
  tier: string;
  used: number;
  limit: number;
  reset?: number | null;
  /** Caracteres de voz enviados en esta sesión. */
  session: number;
}

export interface Estimate {
  items: { chars: number; usd: number; cached: boolean }[];
  chars: number;
  usd: number;
  cached: number;
  session: number;
}

/** Modelos de voz que se ofrecen (el primero, el de fábrica). */
export const MODELS = [
  { id: "eleven_v3", tags: true },
  { id: "eleven_v4", tags: true },
  { id: "eleven_multilingual_v2", tags: false },
  { id: "eleven_flash_v2_5", tags: false },
] as const;

/** Colores para los personajes, en orden. */
export const CAST_COLORS = ["#f472b6", "#60a5fa", "#fbbf24", "#34d399", "#a78bfa", "#fb923c", "#f87171", "#2dd4bf"];

/** Etiquetas de interpretación (en inglés, que es como mejor las entiende el modelo). */
export const TAGS = ["shouting", "whispers", "laughs", "crying", "angry", "excited", "sighs", "scared", "sarcastic", "out of breath"] as const;

let seq = 0;
export const newId = (prefix: string) => `${prefix}${Date.now().toString(36)}${(seq++).toString(36)}`;
