// Catálogo de efectos especiales (sale del núcleo, `fx_catalog`): tipos con sus mandos y ajustes
// predefinidos, y situaciones listas para usar.
import { wasm } from "../core";
import { label } from "../i18n.svelte";
import type { Fx } from "./fx";

export type FxParam =
  | { key: string; kind: "number"; min: number; max: number; step: number; default: number; unit: string }
  | { key: string; kind: "choice"; options: string[]; default: string }
  | { key: string; kind: "color"; default: string };

export interface FxPreset {
  id: string;
  duration: number;
  params: [string, number | string][];
}

export interface FxInfo {
  type: string;
  sustained: boolean;
  duration: number;
  params: FxParam[];
  presets: FxPreset[];
}

export interface FxSituation {
  id: string;
  sound?: string;
  parts: { type: string; preset: string; offset: number; duration: number; intensity: number }[];
}

let cache: { effects: FxInfo[]; situations: FxSituation[] } | null = null;
export function fxCatalog(): { effects: FxInfo[]; situations: FxSituation[] } {
  cache ??= JSON.parse(wasm.fx_catalog());
  return cache!;
}

export const fxInfo = (type: string) => fxCatalog().effects.find((e) => e.type === type);

export const FX_ICON: Record<string, string> = {
  shake: "〰",
  "zoom-punch": "🔍",
  flash: "⚡",
  shockwave: "◎",
  chroma: "🌈",
  "radial-blur": "💫",
  "focus-lines": "✳",
  "speed-lines": "≡",
  impact: "◐",
  color: "🎨",
  vignette: "◯",
  wave: "🌊",
};

export const SITUATION_ICON: Record<string, string> = {
  explosion: "💥",
  punch: "👊",
  shot: "💢",
  lightning: "🌩",
  earthquake: "🌍",
  shout: "📢",
  speed: "💨",
  surprise: "❗",
  fear: "😱",
  flashback: "🕰",
  dream: "💭",
  power: "✨",
  damage: "💔",
};

/** Nombre visible de un efecto (el de su ajuste predefinido si lo tiene). */
export function fxName(f: Fx): string {
  const base = label(`vfx.${f.type}`);
  return f.preset ? `${base} · ${label(`vfxp.${f.type}.${f.preset}`)}` : base;
}

/** Valor de un mando (el de serie si falta o no vale). */
export function fxParam(f: Fx, p: FxParam): number | string {
  const v = f.params?.[p.key];
  if (p.kind === "number") return typeof v === "number" && Number.isFinite(v) ? Math.min(p.max, Math.max(p.min, v)) : p.default;
  if (p.kind === "choice") return typeof v === "string" && p.options.includes(v) ? v : p.default;
  return typeof v === "string" && /^#[0-9a-f]{6}$/i.test(v) ? v : p.default;
}

export function formatFxParam(p: FxParam, v: number | string): string {
  if (p.kind !== "number" || typeof v !== "number") return "";
  const d = p.step >= 1 ? 0 : p.step >= 0.1 ? 1 : 2;
  return `${v.toFixed(d)}${p.unit ? (p.unit === "°" || p.unit === "%" ? p.unit : ` ${p.unit}`) : ""}`;
}
