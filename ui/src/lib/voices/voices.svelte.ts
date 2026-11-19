// Voces con IA en la interfaz: pedir al proveedor (siempre a través de Rust), enseñar el resumen
// del pedido para aprobarlo y colocar lo generado en la línea de tiempo como clips normales.
import { invoke } from "@tauri-apps/api/core";
import { prefs } from "../prefs.svelte";
import { t } from "../i18n.svelte";
import { log } from "../log";
import { sounds, type SoundInfo } from "../audio/sounds.svelte";
import { newEffect } from "../audio/effects";
import { audioClipEnd, newAudioClip, type AudioClip } from "../editor/model";
import type { ProjectStore } from "../action/project.svelte";
import type { AccountInfo, CastMember, Estimate, GenItem, Line, VoiceInfo } from "./types";

export const provider = () => prefs.settings.voicesProvider;

// ------------------------------------------------------------------ llamadas

export const keyStatus = () => invoke<{ stored: boolean; backend: "keyring" | "file" | null }>("voices_key_status");
export const setKey = (key: string) => invoke<"keyring" | "file">("voices_key_set", { key });
export const clearKey = () => invoke("voices_key_clear");
export const account = () => invoke<AccountInfo>("voices_account", { provider: provider() });
export const listVoices = () => invoke<VoiceInfo[]>("voices_list", { provider: provider() });
export const design = (description: string, text?: string) =>
  invoke<{ generatedId: string; seconds: number }[]>("voices_design", { provider: provider(), description, text: text || null });
export const playPreview = (generatedId: string | null) => invoke("voices_preview", { generatedId });
export const saveDesign = (name: string, description: string, generatedId: string) =>
  invoke<VoiceInfo>("voices_save_design", { provider: provider(), name, description, generatedId });
// Siempre copias simples: lo que llega del resumen del pedido son objetos reactivos.
export const estimate = (items: GenItem[]) => invoke<Estimate>("voices_estimate", { provider: provider(), items: $state.snapshot(items) });

/** Genera un fragmento (o lo saca de la caché) y lo añade a la biblioteca de sonidos. */
export async function generate(item: GenItem, name: string): Promise<{ sound: SoundInfo; cached: boolean; chars: number }> {
  const r = await invoke<{ sound: SoundInfo; cached: boolean; chars: number }>("voices_generate", {
    provider: provider(),
    item: $state.snapshot(item),
    name,
    limit: prefs.settings.voicesLimit,
  });
  sounds.add(r.sound);
  return r;
}

/** Dólares por 1000 caracteres (precios de la API; igual que en Rust). */
export const usdPer1k = (model: string) => (/flash|turbo|conversational/.test(model) ? 0.04 : 0.08);

/** Texto sin las etiquetas `[…]` (para nombres y vistas previas). */
export const spoken = (text: string) => text.replace(/\[[^\]]*\]/g, "").replace(/\s+/g, " ").trim();

// ------------------------------------------------------------------ resumen del pedido

/** Crear una voz (Voice Design): solo se aprueba aquí, no pasa por `voices_generate`. */
export interface DesignItem {
  kind: "design";
  description: string;
}

export interface OrderRow {
  /** Quién o qué («Ana», «💥 Efecto», «🎵 Música»). */
  who: string;
  color?: string;
  item: GenItem | DesignItem;
  /** Ya generado: no cuesta nada. */
  cached: boolean;
  on: boolean;
  /** Para colocarlo después (frase del guion y su imagen). */
  line?: string;
  ctx?: LineContext;
}

/** Caracteres y coste de una fila. */
export function rowCost(r: OrderRow): { chars: number; usd: number } {
  const it = r.item;
  if (it.kind === "speech") {
    const chars = [...it.text].length;
    return { chars, usd: (chars / 1000) * usdPer1k(it.model) };
  }
  if (it.kind === "design") return { chars: 0, usd: 0 };
  if (it.kind === "sfx") return { chars: 0, usd: ((it.seconds ?? 5) / 60) * 0.12 };
  return { chars: 0, usd: (it.seconds / 60) * 0.15 };
}

/** Pedido pendiente de aprobar: el diálogo (`OrderDialog`) lo enseña. */
class Order {
  rows = $state<OrderRow[] | null>(null);
  /** Lo pide una IA (MCP): el diálogo lo dice. */
  fromAi = $state(false);
  session = $state(0);
  #resolve: ((rows: OrderRow[] | null) => void) | null = null;

  ask(rows: OrderRow[], fromAi: boolean, session: number): Promise<OrderRow[] | null> {
    this.#resolve?.(null);
    this.rows = rows;
    this.fromAi = fromAi;
    this.session = session;
    return new Promise((r) => (this.#resolve = r));
  }

  finish(ok: boolean) {
    const rows = this.rows;
    this.rows = null;
    const r = this.#resolve;
    this.#resolve = null;
    r?.(ok && rows ? rows.filter((x) => x.on) : null);
  }
}
export const order = new Order();

/** Lo que una IA lleva gastado sin preguntar en esta sesión (caracteres). */
let aiSpent = 0;

/**
 * Pide aprobación (si hace falta) y genera. Una IA no gasta sin el usuario: se le enseña el pedido,
 * salvo que quepa en el presupuesto aprobado en los ajustes o que todo salga de la caché.
 */
export async function approveAndGenerate(
  rows: OrderRow[],
  fromAi: boolean,
  onprogress?: (done: number, total: number) => void,
): Promise<{ results: { row: OrderRow; sound?: SoundInfo; cached?: boolean; error?: string }[] } | null> {
  if (!rows.length) return { results: [] };
  const est = await estimate(rows.map((r) => r.item as GenItem));
  rows.forEach((r, i) => (r.cached = est.items[i]?.cached ?? false));
  const newChars = rows.filter((r) => !r.cached).reduce((n, r) => n + rowCost(r).chars, 0);
  const onlyVoice = rows.every((r) => r.cached || r.item.kind === "speech");
  const budget = prefs.settings.voicesAutoBudget;
  const free = rows.every((r) => r.cached) || (fromAi && onlyVoice && budget > 0 && aiSpent + newChars <= budget && prefs.settings.voicesConsent);
  const accepted = free ? rows : await order.ask(rows, fromAi, est.session);
  if (!accepted) return null;
  if (fromAi && free) aiSpent += newChars;
  const results: { row: OrderRow; sound?: SoundInfo; cached?: boolean; error?: string }[] = [];
  let done = 0;
  for (const row of accepted) {
    onprogress?.(done, accepted.length);
    try {
      const name = nameFor(row);
      const r = await generate(row.item as GenItem, name);
      results.push({ row, sound: r.sound, cached: r.cached });
    } catch (e) {
      log.warn("voces: no se pudo generar:", e);
      results.push({ row, error: String(e) });
      // Sin clave, sin conexión o sin créditos: no tiene sentido seguir pidiendo.
      if (/clave|key|401|402|límite|conectar/i.test(String(e))) break;
    }
    done++;
  }
  onprogress?.(done, accepted.length);
  return { results };
}

function nameFor(row: OrderRow): string {
  const it = row.item;
  const text = it.kind === "speech" ? spoken(it.text) : it.kind === "music" ? it.prompt : it.kind === "sfx" ? it.text : it.description;
  const short = text.length > 40 ? `${text.slice(0, 40)}…` : text;
  return `${row.who}: ${short}`;
}

// ------------------------------------------------------------------ guion → clips

export interface LineContext {
  store: ProjectStore;
  entry: string;
  size: [number, number];
}

/** Lo que se pide para una frase (o el motivo por el que no se puede). */
export function lineItem(line: Line, cast: CastMember[]): { item: GenItem; who: string; color?: string } | { error: string } {
  if (!line.text.trim()) return { error: t("voicesLineEmpty") };
  const c = cast.find((x) => x.id === line.cast);
  if (line.kind === "crowd") return { item: { kind: "sfx", text: spoken(line.text) || line.text }, who: c?.name ?? t("voicesCrowd"), color: c?.color };
  if (!c) return { error: t("voicesLineNoCast", { text: spoken(line.text).slice(0, 30) }) };
  if (!c.voice) return { error: t("voicesCastNoVoice", { name: c.name }) };
  const item: GenItem = { kind: "speech", voice: c.voice.id, model: prefs.settings.voicesModel, text: line.text.trim() };
  if (c.language) item.language = c.language;
  if (c.settings && Object.keys(c.settings).length) item.settings = { ...c.settings };
  return { item, who: c.name, color: c.color };
}

/** Filas del pedido para frases del guion de una imagen. */
export function lineRows(lines: Line[], cast: CastMember[]): { rows: OrderRow[]; errors: string[] } {
  const rows: OrderRow[] = [];
  const errors: string[] = [];
  for (const l of lines) {
    const r = lineItem(l, cast);
    if ("error" in r) errors.push(r.error);
    else rows.push({ who: r.who, color: r.color, item: r.item, cached: false, on: true, line: l.id });
  }
  return { rows, errors };
}

/** Separación entre frases seguidas (s). */
const GAP = 0.25;

/** Segundo en que empieza una frase sin hora fija: detrás de la última voz generada (o a 0,3 s). */
function nextStart(clips: AudioClip[], lines: Line[], line: Line): number {
  const idx = lines.findIndex((l) => l.id === line.id);
  const before = new Set(lines.slice(0, Math.max(0, idx)).map((l) => l.id));
  const ends = clips.filter((c) => c.gen && before.has(c.gen)).map(audioClipEnd);
  return ends.length ? Math.max(...ends) + GAP : 0.3;
}

/**
 * Coloca lo generado para una frase: si ya tenía clip (otra toma), cambia el sonido sin mover el
 * clip ni quitarle sus efectos; si no, uno nuevo en la pista de voz (multitudes, en efectos).
 */
export function placeLine(ctx: LineContext, line: Line, sound: SoundInfo, who: string) {
  const { store, entry, size } = ctx;
  const lines = store.linesFor(entry);
  let start = 0;
  /** Frases que se han corrido (por una toma más larga) y su nuevo inicio. */
  const moved = new Map<string, number>();
  store.editAudio(entry, size, (a) => {
    const bus = line.kind === "crowd" ? "sfx" : "voice";
    const all = [...a.voice, ...a.sfx];
    const old = all.find((c) => c.gen === line.id);
    const name = `${who}: ${spoken(line.text).slice(0, 40)}`;
    if (old) {
      old.sound = sound.id;
      old.trimIn = 0;
      old.duration = Math.max(0.05, sound.seconds);
      old.name = name;
      start = old.start;
      // Si la toma nueva pisa a las voces generadas que vienen detrás, todas ellas se corren lo
      // justo para empezar al acabar esta (los sonidos puestos a mano no se tocan).
      const after = a[bus].filter((c) => c !== old && c.gen && c.start > old.start + 0.01).sort((x, y) => x.start - y.start);
      const shift = after.length ? audioClipEnd(old) + GAP - after[0].start : 0;
      if (shift > 0.01) {
        for (const c of after) {
          c.start = Number((c.start + shift).toFixed(3));
          moved.set(c.gen!, c.start);
        }
      }
      return;
    }
    start = line.start ?? nextStart(all, lines, line);
    const c = newAudioClip(sound, start);
    c.gen = line.id;
    c.name = name;
    if (line.kind === "thought") {
      // Pensamiento: un poco de sala, como dentro de la cabeza.
      const e = newEffect("reverb");
      e.params = { ...e.params, size: 0.4, mix: 0.35 };
      delete e.preset;
      c.effects = [e];
    }
    if (line.kind === "crowd") c.volume = 0.6;
    a[bus].push(c);
  });
  store.editLines(entry, size, (list) => {
    for (const [id, t] of moved) {
      const m = list.find((x) => x.id === id);
      if (m) m.start = Number(t.toFixed(2));
    }
    const l = list.find((x) => x.id === line.id);
    if (!l) return;
    l.takes = [...(l.takes ?? []).filter((s) => s !== sound.id), sound.id];
    l.status = "generated";
    l.start ??= Number(start.toFixed(2));
  });
}

/**
 * Genera frases de una o varias imágenes con un solo pedido y las coloca (`ids` null: las que no
 * están al día). Un solo paso de deshacer. `null` si se canceló el pedido.
 */
export async function generateMany(
  list: { ctx: LineContext; ids: string[] | null }[],
  fromAi: boolean,
  onprogress?: (done: number, total: number) => void,
): Promise<{ done: number; cached: number; errors: string[] } | null> {
  const rows: OrderRow[] = [];
  const errors: string[] = [];
  const cast = list[0]?.ctx.store.cast ?? [];
  for (const { ctx, ids } of list) {
    const lines = ctx.store.linesFor(ctx.entry).filter((l) => (ids ? ids.includes(l.id) : l.status !== "generated") && l.text.trim());
    const r = lineRows(lines, cast);
    errors.push(...r.errors);
    rows.push(...r.rows.map((x) => ({ ...x, ctx })));
  }
  if (!rows.length) return { done: 0, cached: 0, errors };
  const r = await approveAndGenerate(rows, fromAi, onprogress);
  if (!r) return null;
  let done = 0;
  let cached = 0;
  const store = list[0].ctx.store;
  store.batch(true);
  try {
    for (const res of r.results) {
      const ctx = res.row.ctx;
      if (res.error || !res.sound || !ctx) {
        errors.push(res.error ?? "?");
        continue;
      }
      const line = ctx.store.linesFor(ctx.entry).find((l) => l.id === res.row.line);
      if (!line) continue;
      // El texto pudo cambiarse en el resumen del pedido: se guarda en el guion.
      const item = res.row.item;
      const text = item.kind === "speech" ? item.text : line.text;
      if (text !== line.text) {
        ctx.store.editLines(ctx.entry, ctx.size, (l) => {
          const x = l.find((y) => y.id === line.id);
          if (x) x.text = text;
        });
      }
      placeLine(ctx, { ...line, text }, res.sound, res.row.who);
      done++;
      if (res.cached) cached++;
    }
  } finally {
    store.batch(false);
  }
  return { done, cached, errors };
}

export const generateLines = (ctx: LineContext, ids: string[] | null, fromAi: boolean, onprogress?: (done: number, total: number) => void) =>
  generateMany([{ ctx, ids }], fromAi, onprogress);

/**
 * Crear una voz para un personaje: aprobación (la descripción se envía al proveedor), muestras y
 * guardar la elegida (`pick`, desde 1). Devuelve la voz o `null` si se canceló.
 */
export async function designFor(c: CastMember, fromAi: boolean, pick = 1): Promise<VoiceInfo | null> {
  const ok = await order.ask([{ who: c.name, color: c.color, item: { kind: "design", description: c.description ?? "" }, cached: false, on: true }], fromAi, 0);
  const row = ok?.[0];
  if (!row || row.item.kind !== "design") return null;
  const previews = await design(row.item.description);
  const p = previews[Math.min(previews.length, Math.max(1, pick)) - 1];
  return saveDesign(c.name, row.item.description, p.generatedId);
}

/** Al cambiar una frase ya generada, queda desactualizada (su clip sigue sonando). */
export function markStale(l: Line) {
  if (l.status === "generated") l.status = "stale";
}
