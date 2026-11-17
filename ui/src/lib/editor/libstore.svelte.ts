// Biblioteca del usuario: clips de cámara y plantillas con sus parámetros, para reutilizarlos en
// cualquier imagen. Se guarda en el directorio de datos de la app (library.json) y se comparte
// en paquetes `.imgxpack`.
import { invoke } from "@tauri-apps/api/core";
import { cleanClip, clipLength, type Clip } from "./model";
import { info, type TemplateRef } from "../action/catalog";
import { label } from "../i18n.svelte";
import { log } from "../log";
import { sounds, type SoundInfo } from "../audio/sounds.svelte";

export interface LibClip {
  id: string;
  name: string;
  clip: Clip;
  /** Clips de fábrica: no se guardan ni se pueden borrar. */
  builtin?: boolean;
}

export interface LibTemplate {
  id: string;
  name: string;
  template: TemplateRef;
}

interface LibraryFile {
  clips: LibClip[];
  templates: LibTemplate[];
}

const uid = () => `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

const P = (cx: number, cy: number, zoom: number, rot = 0) => ({ cx, cy, zoom, rot });
const move = (a: ReturnType<typeof P>, b: ReturnType<typeof P>, seconds: number): Clip => ({
  type: "keyframes",
  keys: [
    { t: 0, pose: a, easing: { type: "ease-in-out" } },
    { t: seconds, pose: b, easing: { type: "linear" } },
  ],
});

/** Movimientos básicos, en coordenadas normalizadas: valen para cualquier imagen. */
const BUILTIN: { id: string; clip: Clip }[] = [
  { id: "zoom-in", clip: move(P(0.5, 0.5, 1), P(0.5, 0.5, 2), 2) },
  { id: "zoom-out", clip: move(P(0.5, 0.5, 2), P(0.5, 0.5, 1), 2) },
  { id: "pan-right", clip: move(P(0.25, 0.5, 2), P(0.75, 0.5, 2), 3) },
  { id: "pan-left", clip: move(P(0.75, 0.5, 2), P(0.25, 0.5, 2), 3) },
  { id: "pan-down", clip: move(P(0.5, 0.25, 2), P(0.5, 0.75, 2), 3) },
  { id: "pan-up", clip: move(P(0.5, 0.75, 2), P(0.5, 0.25, 2), 3) },
  { id: "tilt", clip: move(P(0.5, 0.5, 1.2, -4), P(0.5, 0.5, 1.2, 4), 3) },
  { id: "hold", clip: { type: "hold", length: 1 } },
  { id: "full", clip: { type: "hold", length: 2, pose: P(0.5, 0.5, 1) } },
];

function cleanFile(raw: unknown): LibraryFile {
  const r = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
  const str = (v: unknown, fallback: string) => (typeof v === "string" && v.trim() ? v.trim().slice(0, 120) : fallback);
  const clips: LibClip[] = [];
  for (const c of Array.isArray(r.clips) ? r.clips : []) {
    const clip = cleanClip(c?.clip);
    if (clip) clips.push({ id: str(c.id, uid()), name: str(c.name, "Clip"), clip });
  }
  const templates: LibTemplate[] = [];
  for (const tpl of Array.isArray(r.templates) ? r.templates : []) {
    const t = tpl?.template;
    if (!t || typeof t.id !== "string" || !info(t.id)) continue;
    const params = t.params && typeof t.params === "object" && !Array.isArray(t.params) ? t.params : {};
    templates.push({ id: str(tpl.id, uid()), name: str(tpl.name, label(`tpl.${t.id}`)), template: { id: t.id, params } });
  }
  return { clips, templates };
}

class Library {
  /** Clips propios (los de fábrica van aparte, en `builtin()`). */
  clips = $state<LibClip[]>([]);
  templates = $state<LibTemplate[]>([]);
  #timer: number | undefined;

  load(text: string | null) {
    if (!text) return;
    try {
      const f = cleanFile(JSON.parse(text));
      this.clips = f.clips;
      this.templates = f.templates;
    } catch (e) {
      log.warn("library.json no es válido; se empieza vacía:", e);
    }
  }

  builtin(): LibClip[] {
    return BUILTIN.map((b) => ({ id: `builtin:${b.id}`, name: label(`lib.${b.id}`), clip: b.clip, builtin: true }));
  }

  all(): LibClip[] {
    return [...this.builtin(), ...this.clips];
  }

  find(id: string): LibClip | undefined {
    return this.all().find((c) => c.id === id);
  }

  addClip(name: string, clip: Clip): LibClip {
    const c = { id: uid(), name, clip: JSON.parse(JSON.stringify(clip)) as Clip };
    this.clips = [...this.clips, c];
    this.save();
    return c;
  }

  addTemplate(name: string, template: TemplateRef) {
    this.templates = [...this.templates, { id: uid(), name, template: JSON.parse(JSON.stringify(template)) }];
    this.save();
  }

  rename(id: string, name: string) {
    const n = name.trim();
    if (!n) return;
    for (const list of [this.clips, this.templates]) {
      const x = list.find((c) => c.id === id);
      if (x) x.name = n.slice(0, 120);
    }
    this.save();
  }

  remove(id: string) {
    this.clips = this.clips.filter((c) => c.id !== id);
    this.templates = this.templates.filter((c) => c.id !== id);
    this.save();
  }

  /** Lo que se guarda o exporta (sin los clips de fábrica). */
  private file(): LibraryFile {
    return $state.snapshot({ clips: this.clips, templates: this.templates }) as LibraryFile;
  }

  /** Exporta clips, plantillas y (si se pide) los sonidos propios. Devuelve cuántos sonidos van. */
  async exportPack(path: string, sounds = true): Promise<number> {
    return await invoke<number>("export_pack", { path, library: JSON.stringify(this.file()), sounds });
  }

  /** Añade lo que trae el paquete. Lo que ya existe (mismo id o mismo contenido) no se duplica. */
  async importPack(path: string): Promise<{ clips: number; templates: number; sounds: SoundInfo[] }> {
    const r = await invoke<{ library: string; sounds: SoundInfo[] }>("import_pack", { path });
    for (const s of r.sounds) sounds.add(s);
    const f = cleanFile(JSON.parse(r.library));
    const sig = (x: unknown) => JSON.stringify(x);
    const haveClips = new Set(this.clips.flatMap((c) => [c.id, sig(c.clip)]));
    const haveTpls = new Set(this.templates.flatMap((c) => [c.id, sig(c.template)]));
    const clips = f.clips.filter((c) => !haveClips.has(c.id) && !haveClips.has(sig(c.clip)));
    const templates = f.templates.filter((c) => !haveTpls.has(c.id) && !haveTpls.has(sig(c.template)));
    this.clips = [...this.clips, ...clips];
    this.templates = [...this.templates, ...templates];
    if (clips.length || templates.length) this.save();
    return { clips: clips.length, templates: templates.length, sounds: r.sounds };
  }

  save() {
    clearTimeout(this.#timer);
    this.#timer = window.setTimeout(() => this.flush(), 300);
  }

  flush() {
    if (this.#timer === undefined) return;
    clearTimeout(this.#timer);
    this.#timer = undefined;
    invoke("save_prefs", { name: "library", json: JSON.stringify(this.file(), null, 2) }).catch((e) =>
      log.warn("no se pudo guardar library.json:", e),
    );
  }
}

/** Duración con la que entra un clip en la timeline. */
export function defaultDuration(c: Clip): number {
  return Math.max(0.1, clipLength(c));
}

export const library = new Library();
