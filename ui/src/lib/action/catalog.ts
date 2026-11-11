// Catálogo de plantillas (definido en imgx-core) y utilidades para sus parámetros.
import { wasm } from "../core";

export type ParamSpec = { key: string; when?: [string, string] } & (
  | { kind: "number"; min: number; max: number; step: number; default: number }
  | { kind: "choice"; options: string[]; default: string }
  | { kind: "bool"; default: boolean }
);

export interface TemplateInfo {
  id: string;
  params: ParamSpec[];
}

export interface TemplateRef {
  id: string;
  params: Record<string, unknown>;
}

let cached: TemplateInfo[] | null = null;

export function catalog(): TemplateInfo[] {
  cached ??= JSON.parse(wasm.template_catalog()) as TemplateInfo[];
  return cached;
}

export function info(id: string): TemplateInfo | undefined {
  return catalog().find((t) => t.id === id);
}

/** Plantilla nueva con todos sus parámetros por defecto. */
export function fresh(id: string): TemplateRef {
  const params: Record<string, unknown> = {};
  for (const p of info(id)?.params ?? []) params[p.key] = p.default;
  return { id, params };
}

export function paramValue(tpl: TemplateRef, spec: ParamSpec): unknown {
  return tpl.params[spec.key] ?? spec.default;
}

/** El parámetro aplica con los valores actuales (p. ej. filas solo en modo rejilla). */
export function applies(tpl: TemplateRef, spec: ParamSpec): boolean {
  if (!spec.when) return true;
  const [key, value] = spec.when;
  const dep = info(tpl.id)?.params.find((p) => p.key === key);
  return (tpl.params[key] ?? dep?.default) === value;
}

/** La plantilla necesita las viñetas detectadas. */
export function needsPanels(tpl: TemplateRef | null): boolean {
  return tpl?.id === "comic" && (tpl.params.mode ?? "auto") === "auto";
}

/** Sin plantilla, cada imagen se muestra completa unos segundos. */
export const FALLBACK: TemplateRef = { id: "full", params: {} };
