// Ajustes de la app y lista de recientes. Se leen al arrancar y se guardan solos (con un
// pequeño retardo) en el directorio de configuración, vía Rust.
import { invoke } from "@tauri-apps/api/core";
import { setLang, type LangSetting } from "./i18n.svelte";
import { cleanOverrides, type KeyOverrides } from "./keys";
import { log } from "./log";

export type ThumbSize = "s" | "m" | "l";

export interface Settings {
  lang: LangSetting;
  /** Color de fondo del visor (#rrggbb). */
  background: string;
  showThumbs: boolean;
  thumbSize: ThumbSize;
  showDiag: boolean;
  /** Factor de cada paso de zoom con teclado y botones. */
  zoomStep: number;
  /** Multiplicador de la velocidad de zoom con la rueda del ratón. */
  wheelSpeed: number;
  /** Al volver a abrir algo, ir a la imagen en la que se dejó. */
  resume: boolean;
  keys: KeyOverrides;
}

export interface Recent {
  path: string;
  title: string;
  kind: "folder" | "archive";
  /** Imagen en la que se dejó (por nombre; el índice por si el nombre ya no está). */
  entry: string;
  index: number;
  count: number;
  /** ms desde la época. */
  at: number;
}

export const BACKGROUNDS = ["#0b0d10", "#000000", "#2b2f36", "#808080", "#f2f2f2"] as const;
export const DEFAULT_SETTINGS: Settings = {
  lang: "auto",
  background: BACKGROUNDS[0],
  showThumbs: true,
  thumbSize: "m",
  showDiag: false,
  zoomStep: 1.25,
  wheelSpeed: 1,
  resume: true,
  keys: {},
};
export const MAX_RECENTS = 20;

function clean(raw: unknown): Settings {
  const s: Settings = { ...DEFAULT_SETTINGS, keys: {} };
  if (!raw || typeof raw !== "object") return s;
  const r = raw as Record<string, unknown>;
  if (r.lang === "es" || r.lang === "en" || r.lang === "auto") s.lang = r.lang;
  if (typeof r.background === "string" && /^#[0-9a-f]{6}$/i.test(r.background)) s.background = r.background.toLowerCase();
  if (typeof r.showThumbs === "boolean") s.showThumbs = r.showThumbs;
  if (r.thumbSize === "s" || r.thumbSize === "m" || r.thumbSize === "l") s.thumbSize = r.thumbSize;
  if (typeof r.showDiag === "boolean") s.showDiag = r.showDiag;
  if (typeof r.zoomStep === "number" && Number.isFinite(r.zoomStep)) s.zoomStep = Math.min(2, Math.max(1.05, r.zoomStep));
  if (typeof r.wheelSpeed === "number" && Number.isFinite(r.wheelSpeed)) s.wheelSpeed = Math.min(4, Math.max(0.25, r.wheelSpeed));
  if (typeof r.resume === "boolean") s.resume = r.resume;
  s.keys = cleanOverrides(r.keys);
  return s;
}

function cleanRecents(raw: unknown): Recent[] {
  if (!Array.isArray(raw)) return [];
  return raw
    .filter((r): r is Recent => !!r && typeof r.path === "string" && typeof r.title === "string")
    .map((r): Recent => ({
      path: r.path,
      title: r.title,
      kind: r.kind === "archive" ? "archive" : "folder",
      entry: typeof r.entry === "string" ? r.entry : "",
      index: Number.isInteger(r.index) && r.index >= 0 ? r.index : 0,
      count: Number.isInteger(r.count) && r.count >= 0 ? r.count : 0,
      at: typeof r.at === "number" ? r.at : 0,
    }))
    .slice(0, MAX_RECENTS);
}

class Prefs {
  settings = $state<Settings>({ ...DEFAULT_SETTINGS, keys: {} });
  recents = $state<Recent[]>([]);
  loaded = $state(false);

  #timers: Partial<Record<"settings" | "recents", number>> = {};

  async load() {
    try {
      const p = await invoke<{ settings: string | null; recents: string | null }>("load_prefs");
      this.settings = clean(parse(p.settings, "settings.json"));
      this.recents = cleanRecents(parse(p.recents, "recents.json"));
    } catch (e) {
      log.warn("no se pudieron leer los ajustes:", e);
    }
    setLang(this.settings.lang);
    this.loaded = true;
  }

  /** Llamar tras cambiar `settings`. */
  saveSettings() {
    setLang(this.settings.lang);
    this.#save("settings", () => this.settings);
  }

  /** Apunta la fuente abierta (y la imagen actual) al principio de la lista. */
  remember(r: Omit<Recent, "at">) {
    const prev = this.recents[0];
    const same = prev?.path === r.path && prev.entry === r.entry && prev.index === r.index && prev.count === r.count;
    if (same && Date.now() - prev.at < 60_000) return;
    this.recents = [{ ...r, at: Date.now() }, ...this.recents.filter((x) => x.path !== r.path)].slice(0, MAX_RECENTS);
    this.#save("recents", () => this.recents);
  }

  find(path: string): Recent | undefined {
    return this.recents.find((r) => r.path === path);
  }

  forget(path: string) {
    this.recents = this.recents.filter((r) => r.path !== path);
    this.#save("recents", () => this.recents);
  }

  clearRecents() {
    this.recents = [];
    this.#save("recents", () => this.recents);
  }

  /** Escribe ya lo pendiente (al cerrar). */
  flush() {
    for (const name of ["settings", "recents"] as const) {
      if (this.#timers[name] === undefined) continue;
      clearTimeout(this.#timers[name]);
      this.#timers[name] = undefined;
      this.#write(name, name === "settings" ? this.settings : this.recents);
    }
  }

  #save(name: "settings" | "recents", value: () => unknown) {
    clearTimeout(this.#timers[name]);
    this.#timers[name] = window.setTimeout(() => {
      this.#timers[name] = undefined;
      this.#write(name, value());
    }, 400);
  }

  #write(name: "settings" | "recents", value: unknown) {
    const json = JSON.stringify($state.snapshot(value), null, 2);
    invoke("save_prefs", { name, json }).catch((e) => log.warn(`no se pudo guardar ${name}.json:`, e));
  }
}

function parse(text: string | null, what: string): unknown {
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch (e) {
    log.warn(`${what} no es válido, se usan los valores por defecto:`, e);
    return null;
  }
}

export const prefs = new Prefs();
