// Ajustes de la app y lista de recientes. Se leen al arrancar y se guardan solos (con un
// pequeño retardo) en el directorio de configuración, vía Rust.
import { invoke } from "@tauri-apps/api/core";
import { setLang, type LangSetting } from "./i18n.svelte";
import { cleanOverrides, type KeyOverrides } from "./keys";
import { log } from "./log";
import { library } from "./editor/libstore.svelte";

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
  /** Selector de archivos: el propio de la app, el del sistema o según el sistema operativo. */
  fileDialog: FileDialogMode;
  /** Última carpeta visitada en el selector propio. */
  lastDir: string;
  /** Marco del editor: la proporción de la reproducción, la del vídeo o ninguno. */
  editorFrame: FrameMode;
  /** Anchos (px) de las columnas de biblioteca e inspector del editor. */
  libWidth: number;
  inspectorWidth: number;
  /** Altura del editor (px). */
  editorHeight: number;
  /** Opciones de la última exportación a vídeo. */
  video: VideoPrefs;
  /** Sonido: volumen general (0..1,5), silencio y micrófono para grabar voz ("" = el del sistema). */
  sound: { master: number; mute: boolean; input: string };
  /** Una IA puede manejar la app (servidor MCP). */
  aiControl: boolean;
  keys: KeyOverrides;
}

export type FrameMode = "playback" | "video" | "off";

export type VideoSize = "720p" | "1080p" | "1440p" | "4k" | "custom";
export type VideoShape = "landscape" | "portrait" | "square";
export type VideoQuality = "high" | "normal" | "light";
export interface VideoPrefs {
  size: VideoSize;
  shape: VideoShape;
  /** Tamaño a medida (px). */
  width: number;
  height: number;
  fps: 24 | 30 | 60;
  quality: VideoQuality;
  /** Ruta de ffmpeg (vacía: buscarlo). */
  ffmpeg: string;
}

export const VIDEO_SIZES: Record<Exclude<VideoSize, "custom">, number> = { "720p": 720, "1080p": 1080, "1440p": 1440, "4k": 2160 };
export const QUALITY_CRF: Record<VideoQuality, number> = { high: 18, normal: 23, light: 28 };
export const DEFAULT_VIDEO: VideoPrefs = { size: "1080p", shape: "landscape", width: 1920, height: 1080, fps: 30, quality: "normal", ffmpeg: "" };

const mult4 = (v: number) => Math.max(16, Math.min(8192, Math.round(v / 4) * 4));

/** Tamaño en píxeles del vídeo (múltiplos de 4). */
export function videoPixels(v: VideoPrefs): { w: number; h: number } {
  if (v.size === "custom") return { w: mult4(v.width), h: mult4(v.height) };
  const short = VIDEO_SIZES[v.size];
  const long = Math.round((short * 16) / 9);
  if (v.shape === "square") return { w: short, h: short };
  return v.shape === "portrait" ? { w: mult4(short), h: mult4(long) } : { w: mult4(long), h: mult4(short) };
}

function cleanVideo(raw: unknown): VideoPrefs {
  const v: VideoPrefs = { ...DEFAULT_VIDEO };
  if (!raw || typeof raw !== "object") return v;
  const r = raw as Record<string, unknown>;
  if (r.size === "custom" || (typeof r.size === "string" && r.size in VIDEO_SIZES)) v.size = r.size as VideoSize;
  if (r.shape === "landscape" || r.shape === "portrait" || r.shape === "square") v.shape = r.shape;
  if (typeof r.width === "number" && Number.isFinite(r.width)) v.width = mult4(r.width);
  if (typeof r.height === "number" && Number.isFinite(r.height)) v.height = mult4(r.height);
  if (r.fps === 24 || r.fps === 30 || r.fps === 60) v.fps = r.fps;
  if (r.quality === "high" || r.quality === "normal" || r.quality === "light") v.quality = r.quality;
  if (typeof r.ffmpeg === "string") v.ffmpeg = r.ffmpeg;
  return v;
}

export type FileDialogMode = "auto" | "app" | "system";
export const LIB_WIDTH = { min: 160, max: 640, def: 220 } as const;
export const INSPECTOR_WIDTH = { min: 200, max: 520, def: 250 } as const;
export const EDITOR_HEIGHT = { min: 170, max: 1200, def: 260 } as const;

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
  fileDialog: "auto",
  lastDir: "",
  editorFrame: "playback",
  libWidth: LIB_WIDTH.def,
  inspectorWidth: INSPECTOR_WIDTH.def,
  editorHeight: EDITOR_HEIGHT.def,
  video: DEFAULT_VIDEO,
  sound: { master: 1, mute: false, input: "" },
  aiControl: false,
  keys: {},
};
export const MAX_RECENTS = 20;

function clean(raw: unknown): Settings {
  const s: Settings = { ...DEFAULT_SETTINGS, video: { ...DEFAULT_VIDEO }, sound: { ...DEFAULT_SETTINGS.sound }, keys: {} };
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
  if (r.fileDialog === "auto" || r.fileDialog === "app" || r.fileDialog === "system") s.fileDialog = r.fileDialog;
  if (typeof r.lastDir === "string") s.lastDir = r.lastDir;
  if (r.editorFrame === "playback" || r.editorFrame === "video" || r.editorFrame === "off") s.editorFrame = r.editorFrame;
  else if (r.editorFrame === false) s.editorFrame = "off";
  s.video = cleanVideo(r.video);
  const width = (v: unknown, w: { min: number; max: number; def: number }) =>
    typeof v === "number" && Number.isFinite(v) ? Math.round(Math.min(w.max, Math.max(w.min, v))) : w.def;
  s.libWidth = width(r.libWidth, LIB_WIDTH);
  s.inspectorWidth = width(r.inspectorWidth, INSPECTOR_WIDTH);
  s.editorHeight = width(r.editorHeight, EDITOR_HEIGHT);
  const snd = (r.sound && typeof r.sound === "object" ? r.sound : {}) as Record<string, unknown>;
  if (typeof snd.master === "number" && Number.isFinite(snd.master)) s.sound.master = Math.min(1.5, Math.max(0, snd.master));
  if (typeof snd.mute === "boolean") s.sound.mute = snd.mute;
  if (typeof snd.input === "string") s.sound.input = snd.input.slice(0, 300);
  if (typeof r.aiControl === "boolean") s.aiControl = r.aiControl;
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
  settings = $state<Settings>({ ...DEFAULT_SETTINGS, video: { ...DEFAULT_VIDEO }, sound: { ...DEFAULT_SETTINGS.sound }, keys: {} });
  recents = $state<Recent[]>([]);
  loaded = $state(false);

  #timers: Partial<Record<"settings" | "recents", number>> = {};

  async load() {
    try {
      const p = await invoke<{ settings: string | null; recents: string | null; library: string | null }>("load_prefs");
      this.settings = clean(parse(p.settings, "settings.json"));
      this.recents = cleanRecents(parse(p.recents, "recents.json"));
      library.load(p.library);
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
    library.flush();
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
