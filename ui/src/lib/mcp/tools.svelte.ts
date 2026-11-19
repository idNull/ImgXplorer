// Herramientas de la IA (servidor MCP). Cada una recibe los argumentos que manda la IA (ya
// validados aquí, nunca de fiar) y edita el proyecto con el mismo almacén que la interfaz: el
// usuario lo ve en directo y lo puede deshacer. Las descripciones para la IA están en
// `src-tauri/src/mcp/tools.json`.
import { invoke } from "@tauri-apps/api/core";
import type { AmbientClip, Dir, ProjectStore, Rect, Transition, TransitionType } from "../action/project.svelte";
import { exportVideo, ExportCancelled, type ExportSpec } from "../export/exporter";
import type { PictureRef } from "../viewer/picture";
import { FIT, wasm, type Pose } from "../core";
import { prefs, videoPixels, VIDEO_SIZES, type VideoQuality, type VideoShape, type VideoSize } from "../prefs.svelte";
import { fresh, type TemplateRef } from "../action/catalog";
import { itemDuration } from "../action/player";
import { inline, newAudioClip, toAbs, type AudioBus, type AudioClip, type AudioTracks, type EasingType, type Keyframe, type Timeline } from "../editor/model";
import { sounds } from "../audio/sounds.svelte";
import { FX, FX_TYPES, newEffect, type Effect, type FxType } from "../audio/effects";
import { trackSpecs, type SoundBus, type Track } from "../audio/tracks";
import { nameOf } from "../audio/names";
import { ambientRange, ambientSpecs, SoundSchedule, type ClipSpec } from "../audio/sync";
import { FrameRenderer, canvasOf, contactSheet, drawPanels, jpeg, mult4 } from "./frames";
import { FxTrack, MIN_FX_DURATION, fxEnd, fxId, type Fx } from "../fx/fx";
import { fxCatalog, fxInfo } from "../fx/catalog";
import { joinPath, dirOf } from "../files/dialog.svelte";

export interface McpSource {
  title: string;
  path: string;
  kind: "folder" | "archive";
  entries: { name: string }[];
}

/** Lo que las herramientas necesitan de la app. */
export interface McpContext {
  source(): McpSource | null;
  index(): number;
  store: ProjectStore;
  /** Abre una fuente; devuelve el error o null. */
  open(path: string): Promise<string | null>;
  go(i: number): void;
  refAt(i: number): PictureRef;
  /** Viñetas detectadas (planas: x, y, w, h…). */
  detectedPanels(i: number): Promise<number[]>;
  sequence: ExportSpec;
  play(i: number): void;
  stop(): void;
  playing(): boolean;
  say(msg: string): void;
}

export interface ToolResult {
  text: string;
  images?: string[];
}

type Args = Record<string, unknown>;

interface ExportJob {
  state: "running" | "done" | "error" | "cancelled";
  path: string;
  fraction: number;
  frames: number;
  mixing: boolean;
  error?: string;
  started: number;
  abort: AbortController;
}

/** Error para la IA (se le muestra tal cual). */
class ToolError extends Error {}
const fail = (msg: string): never => {
  throw new ToolError(msg);
};

const r4 = (v: number) => Math.round(v * 1e4) / 1e4;
const r2 = (v: number) => Math.round(v * 100) / 100;
const json = (v: unknown, images?: string[]): ToolResult => ({ text: JSON.stringify(v), images });

function num(a: Args, key: string, def: number, min: number, max: number): number {
  const v = a[key];
  if (v === undefined || v === null) return def;
  if (typeof v !== "number" || !Number.isFinite(v)) fail(`${key} must be a number`);
  return Math.min(max, Math.max(min, v as number));
}
const optNum = (a: Args, key: string, min: number, max: number) => (a[key] === undefined || a[key] === null ? undefined : num(a, key, 0, min, max));
function str(a: Args, key: string): string | undefined {
  const v = a[key];
  if (v === undefined || v === null) return undefined;
  if (typeof v !== "string") fail(`${key} must be a string`);
  return v as string;
}
function oneOf<T extends string>(a: Args, key: string, options: readonly T[]): T | undefined {
  const v = str(a, key);
  if (v === undefined) return undefined;
  if (!options.includes(v as T)) fail(`${key} must be one of: ${options.join(", ")}`);
  return v as T;
}
function int(a: Args, key: string, def?: number): number {
  const v = a[key];
  if (v === undefined || v === null) {
    if (def === undefined) fail(`${key} is required`);
    return def!;
  }
  if (typeof v !== "number" || !Number.isInteger(v)) fail(`${key} must be an integer`);
  return v as number;
}
function rectOf(v: unknown, what = "rect"): Rect {
  const r = v as Rect;
  if (!r || typeof r !== "object" || ![r.x, r.y, r.w, r.h].every((n) => typeof n === "number" && Number.isFinite(n))) fail(`${what} needs numbers x, y, w, h`);
  const x = Math.min(0.99, Math.max(0, r.x));
  const y = Math.min(0.99, Math.max(0, r.y));
  return { x, y, w: Math.min(1 - x, Math.max(0.005, r.w)), h: Math.min(1 - y, Math.max(0.005, r.h)) };
}

const EASINGS: EasingType[] = ["ease-in-out", "ease-in", "ease-out", "linear", "bounce", "elastic", "step"];
const DIRS: Dir[] = ["left", "right", "up", "down"];
const TRANSITIONS: TransitionType[] = ["cut", "fade", "slide", "continuous-camera"];

export class McpTools {
  private renderer: FrameRenderer | null = null;
  private rendererFor = "";
  private exportJob: ExportJob | null = null;

  constructor(private ctx: McpContext) {}

  private get store() {
    return this.ctx.store;
  }

  private src(): McpSource {
    const s = this.ctx.source();
    if (!s) fail("Nothing is open. Use open_source first.");
    if (!s!.entries.length) fail("The open source has no images.");
    return s!;
  }

  private img(a: Args, key = "image"): { i: number; entry: string } {
    const s = this.src();
    const i = int(a, key);
    if (i < 0 || i >= s.entries.length) fail(`${key} ${i} out of range (0..${s.entries.length - 1})`);
    return { i, entry: s.entries[i].name };
  }

  private writable() {
    if (!this.store.data) fail("No project loaded.");
    if (this.store.readOnly) fail(`The project file could not be read, so it is read-only: ${this.store.readOnly}`);
  }

  private frames(): FrameRenderer {
    const s = this.ctx.source();
    const key = s ? `${s.path}|${this.ctx.refAt(0).url}` : "";
    if (this.renderer && this.rendererFor !== key) this.renderer.reset();
    this.renderer ??= new FrameRenderer();
    this.rendererFor = key;
    return this.renderer;
  }

  private async size(i: number): Promise<[number, number]> {
    const it = this.store.item(this.src().entries[i].name);
    if (it?.size?.[0] && it.size[1]) return it.size;
    const p = await this.frames().picture(this.ctx.refAt(i));
    return [p.w, p.h];
  }

  private video() {
    const v = videoPixels(prefs.settings.video);
    return { width: v.w, height: v.h, aspect: v.w / v.h, fps: prefs.settings.video.fps, quality: prefs.settings.video.quality };
  }

  /** Una sola edición = un solo paso de deshacer. */
  private edit<T>(fn: () => T): T {
    this.writable();
    this.store.commit();
    this.store.batch(true);
    try {
      return fn();
    } finally {
      this.store.batch(false);
    }
  }

  /** Enseña al usuario la imagen que se está tocando. */
  private show(i: number) {
    if (!this.ctx.playing() && this.ctx.index() !== i) this.ctx.go(i);
  }

  // -------------------------------------------------------------------------------------------
  // Viñetas

  private direction(entry: string, a: Args): "ltr" | "rtl" {
    const d = oneOf(a, "direction", ["ltr", "rtl"] as const);
    if (d) return d;
    const tpl = this.store.templateFor(entry);
    return tpl?.id === "comic" && tpl.params.dir === "rtl" ? "rtl" : "ltr";
  }

  /** Viñetas en orden de lectura y de dónde salen. */
  private async panels(i: number, dir: "ltr" | "rtl"): Promise<{ list: Rect[]; source: "manual" | "detected" | "none" }> {
    const entry = this.src().entries[i].name;
    const own = this.store.item(entry)?.panels;
    let flat: number[];
    let manual = false;
    if (own?.length) {
      flat = own.flatMap((r) => [r.x, r.y, r.w, r.h]);
      manual = true;
    } else {
      flat = await this.ctx.detectedPanels(i);
      // Con menos de dos no hay viñetas (la plantilla pondría una rejilla inventada).
      if (flat.length < 8) return { list: [], source: "none" };
    }
    const tpl = JSON.stringify({ id: "comic", params: { mode: "auto", dir } });
    const ordered = wasm.comic_panels(tpl, 1, 1, 1, new Float64Array(flat), manual);
    const list: Rect[] = [];
    for (let k = 0; k + 3 < ordered.length; k += 4) list.push({ x: ordered[k], y: ordered[k + 1], w: ordered[k + 2], h: ordered[k + 3] });
    return { list, source: manual ? "manual" : "detected" };
  }

  private panelInfo(list: Rect[]) {
    return list.map((r, k) => ({ n: k + 1, x: r4(r.x), y: r4(r.y), w: r4(r.w), h: r4(r.h) }));
  }

  // -------------------------------------------------------------------------------------------
  // Herramientas

  async call(tool: string, a: Args): Promise<ToolResult> {
    const fn = (this as unknown as Record<string, (a: Args) => Promise<ToolResult>>)[`t_${tool}`];
    if (typeof fn !== "function") fail(`unknown tool ${tool}`);
    return fn.call(this, a ?? {});
  }

  async t_app_status(): Promise<ToolResult> {
    const s = this.ctx.source();
    const i = this.ctx.index();
    return json({
      open: !!s,
      source: s ? { title: s.title, path: s.path, kind: s.kind, images: s.entries.length } : null,
      project: s ? { file: this.store.path, readOnly: this.store.readOnly, canUndo: this.store.canUndo } : null,
      current: s?.entries[i] ? { image: i, entry: s.entries[i].name } : null,
      output: this.video(),
      playing: this.ctx.playing(),
      exporting: this.exportJob?.state === "running",
    });
  }

  async t_open_source(a: Args): Promise<ToolResult> {
    const path = str(a, "path") ?? fail("path is required");
    const err = await this.ctx.open(path);
    if (err) fail(err);
    return this.t_app_status();
  }

  async t_list_images(a: Args): Promise<ToolResult> {
    const s = this.src();
    const from = Math.max(0, int(a, "from", 0));
    const count = Math.min(500, Math.max(1, int(a, "count", 100)));
    const def = this.store.data?.defaults.template;
    const images = s.entries.slice(from, from + count).map((e, k) => {
      const it = this.store.item(e.name);
      const audio = this.store.audioFor(e.name);
      const animation = it?.template ? `template:${it.template.id}` : it?.timeline?.camera?.length ? "keyframes" : def ? `default-template:${def.id}` : "none";
      return {
        image: from + k,
        entry: e.name,
        animation,
        voices: audio.voice.length || undefined,
        sfx: audio.sfx.length || undefined,
        panels: it?.panels ? `manual:${it.panels.length}` : undefined,
        transition: it?.transitionOut?.type,
      };
    });
    return json({ total: s.entries.length, from, images });
  }

  async t_go_to(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    this.ctx.go(i);
    return json({ image: i, entry });
  }

  async t_view_image(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    const [w, h] = await this.size(i);
    const max = num(a, "maxSize", 1400, 256, 2000);
    const k = Math.min(1, max / Math.max(w, h));
    const ow = mult4(w * k);
    const oh = mult4(h * k);
    const dir = this.direction(entry, a);
    const p = await this.panels(i, dir);
    const c = canvasOf(await this.frames().render(this.ctx.refAt(i), FIT, ow, oh, "#ffffff"));
    if (a.panels !== false && p.list.length) drawPanels(c, p.list);
    return json({ image: i, entry, size: [w, h], direction: dir, panelSource: p.source, panels: this.panelInfo(p.list) }, [jpeg(c)]);
  }

  async t_view_panel(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    const [w, h] = await this.size(i);
    let r: Rect;
    if (a.rect) r = rectOf(a.rect);
    else if (a.panel !== undefined) {
      const n = int(a, "panel");
      const p = await this.panels(i, this.direction(entry, a));
      if (n < 1 || n > p.list.length) fail(p.list.length ? `panel must be 1..${p.list.length}` : "this image has no panels: use rect");
      r = p.list[n - 1];
    } else return fail("give panel or rect");
    // Un poco de aire para no cortar los bordes.
    const pad = 0.01;
    const x0 = Math.max(0, r.x - pad), y0 = Math.max(0, r.y - pad);
    const x1 = Math.min(1, r.x + r.w + pad), y1 = Math.min(1, r.y + r.h + pad);
    const pw = (x1 - x0) * w, ph = (y1 - y0) * h;
    const max = num(a, "maxSize", 1400, 256, 2000);
    // Hasta el doble de su resolución (para leer letra pequeña), sin pasar del máximo.
    const k = Math.min(max / Math.max(pw, ph), 2);
    const ow = mult4(pw * k), oh = mult4(ph * k);
    const fit = Math.min(ow / w, oh / h);
    const zoom = ow / pw / fit;
    const pose: Pose = new Float64Array([(x0 + x1) / 2, (y0 + y1) / 2, zoom, 0]);
    const c = canvasOf(await this.frames().render(this.ctx.refAt(i), pose, ow, oh, "#ffffff"));
    return json({ image: i, entry, rect: { x: r4(r.x), y: r4(r.y), w: r4(r.w), h: r4(r.h) }, pixels: [Math.round(r.w * w), Math.round(r.h * h)] }, [jpeg(c, 0.9)]);
  }

  async t_get_panels(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    const dir = this.direction(entry, a);
    const p = await this.panels(i, dir);
    return json({ image: i, entry, direction: dir, panelSource: p.source, panels: this.panelInfo(p.list) });
  }

  async t_set_panels(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    const size = await this.size(i);
    if (a.reset === true) this.edit(() => this.store.setPanels(entry, size, null));
    else {
      if (!Array.isArray(a.panels) || !a.panels.length) fail("panels must be a non-empty array (or reset: true)");
      const rects = (a.panels as unknown[]).slice(0, 60).map((p, k) => rectOf(p, `panels[${k}]`));
      this.edit(() => this.store.setPanels(entry, size, rects));
    }
    this.show(i);
    return this.t_get_panels({ image: i, direction: a.direction });
  }

  async t_set_output(a: Args): Promise<ToolResult> {
    const v = prefs.settings.video;
    const preset = oneOf(a, "preset", Object.keys(VIDEO_SIZES) as Exclude<VideoSize, "custom">[]);
    const shape = oneOf(a, "shape", ["landscape", "portrait", "square"] as VideoShape[]);
    if (preset) v.size = preset;
    if (shape) v.shape = shape;
    if (a.width !== undefined || a.height !== undefined) {
      v.size = "custom";
      v.width = mult4(num(a, "width", v.width, 16, 8192));
      v.height = mult4(num(a, "height", v.height, 16, 8192));
    }
    if (a.fps !== undefined) {
      const f = num(a, "fps", 30, 1, 120);
      v.fps = f < 27 ? 24 : f < 45 ? 30 : 60;
    }
    const q = oneOf(a, "quality", ["high", "normal", "light"] as VideoQuality[]);
    if (q) v.quality = q;
    prefs.saveSettings();
    return json({ output: this.video(), note: "Shots planned before this change keep their old framing; plan them again if the shape changed." });
  }

  async t_plan_shots(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    const size = await this.size(i);
    if (!Array.isArray(a.shots)) fail("shots must be an array");
    const raw = (a.shots as Args[]).slice(0, 80);
    const dir = this.direction(entry, a);
    const needPanels = raw.some((s) => s && s.panel !== undefined && !s.rect);
    const p = needPanels ? await this.panels(i, dir) : { list: [] as Rect[], source: "none" as const };
    const targets: string[] = [];
    const shots = raw.map((s, k) => {
      if (!s || typeof s !== "object") fail(`shots[${k}] must be an object`);
      let rect: Rect | null = null;
      if (s.rect) {
        rect = rectOf(s.rect, `shots[${k}].rect`);
        targets.push("rect");
      } else if (s.panel !== undefined && s.full !== true) {
        const n = int(s, "panel");
        if (n < 1 || n > p.list.length) fail(p.list.length ? `shots[${k}].panel must be 1..${p.list.length}` : "this image has no panels: use rect or full");
        rect = p.list[n - 1];
        targets.push(`panel ${n}`);
      } else targets.push("full page");
      const easing = oneOf(s, "easing", EASINGS) ?? "ease-in-out";
      return {
        rect,
        hold: num(s, "hold", 2, 0, 120),
        move: num(s, "move", 0.8, 0, 20),
        easing: { type: easing },
        margin: num(s, "margin", 0.04, 0, 0.5),
        zoom: num(s, "zoom", 1, 0.2, 16),
        rot: num(s, "rot", 0, -360, 360),
        push: num(s, "push", 1, 0.5, 2),
        maxZoom: num(s, "maxZoom", 6, 1, 64),
      };
    });
    const plan = {
      shots,
      start: oneOf(a, "start", ["overview", "direct"] as const) ?? "direct",
      end: oneOf(a, "end", ["overview", "direct"] as const) ?? "direct",
      overviewHold: num(a, "overviewHold", 1, 0, 120),
    };
    const out = JSON.parse(wasm.plan_shots(JSON.stringify(plan), size[0], size[1], this.video().aspect)) as {
      timeline: Timeline;
      shots: { arrive: number; hold: number; end: number }[];
      duration: number;
    };
    this.edit(() => this.store.setTimeline(entry, size, out.timeline));
    this.show(i);
    const audio = this.store.audioFor(entry);
    const warnings: string[] = [];
    out.shots.forEach((t, k) => {
      if (t.end - t.hold < 1.2 && shots[k].hold > 0) warnings.push(`shot ${k + 1} holds only ${r2(t.end - t.hold)} s`);
    });
    const end = Math.max(out.duration, ...[...audio.voice, ...audio.sfx].map((c) => c.start + c.duration));
    if (end > out.duration + 0.05) warnings.push(`existing sounds last until ${r2(end)} s, longer than the shots (${r2(out.duration)} s): the image will last until then`);
    return json({
      image: i,
      entry,
      duration: r2(out.duration),
      shots: out.shots.map((t, k) => ({ n: k + 1, target: targets[k], arrive: r2(t.arrive), hold: r2(t.hold), end: r2(t.end), note: (raw[k] as Args).note })),
      warnings,
    });
  }

  /** Timeline con la que se reproduce la imagen (la propia o la de su plantilla). */
  private async playTimeline(i: number) {
    const s = await this.size(i);
    return { tl: await this.ctx.sequence.timeline(i, { w: s[0], h: s[1] }, this.video().aspect), size: s };
  }

  private clipInfo(c: AudioClip, bus: AudioBus, index: number, tracks: Track[]) {
    const tr = tracks.find((t) => t.id === c.track && t.bus === bus) ?? tracks.find((t) => t.bus === bus);
    return {
      bus,
      index,
      sound: c.sound,
      soundName: sounds.get(c.sound) ? sounds.name(c.sound) : "(missing from library)",
      name: c.name,
      start: r2(c.start),
      end: r2(c.start + c.duration),
      duration: r2(c.duration),
      trimIn: c.trimIn ? r2(c.trimIn) : undefined,
      volume: c.volume !== 1 ? r2(c.volume) : undefined,
      fadeIn: c.fadeIn || undefined,
      fadeOut: c.fadeOut || undefined,
      track: tr?.id,
      effects: c.effects?.map((e) => e.preset ?? e.type),
    };
  }

  async t_get_timeline(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    if (!sounds.loaded) await sounds.load();
    const it = this.store.item(entry);
    const tpl = this.store.templateFor(entry);
    const { tl } = await this.playTimeline(i);
    const audio = this.store.audioFor(entry);
    const tracks = this.store.tracks;
    let camera: unknown;
    if (!tpl && it?.timeline?.camera?.length) {
      camera = it.timeline.camera.map((inst) => {
        const c = inline(inst);
        const keys = c?.type === "keyframes" ? c.keys.map((k) => ({ t: r2(toAbs(inst, k.t)), cx: r4(k.pose.cx), cy: r4(k.pose.cy), zoom: r4(k.pose.zoom), rot: k.pose.rot || undefined, easing: k.easing.type })) : undefined;
        return { name: inst.name, start: r2(inst.start), duration: r2(inst.duration), type: c?.type ?? "library-clip", keys };
      });
    } else {
      camera = { template: tpl ?? { id: "full", params: {} }, generated: JSON.parse(tl.to_json()) };
    }
    const ambients = this.store.ambients
      .filter((x) => {
        const r = ambientRange(x, (e) => this.src().entries.findIndex((y) => y.name === e));
        return r && i >= r[0] && i <= r[1];
      })
      .map((x) => ({ id: x.id, sound: x.sound, name: x.name }));
    const fxl = this.store.fxFor(entry);
    const res = {
      image: i,
      entry,
      duration: r2(itemDuration(tl, audio, fxl)),
      cameraDuration: r2(tl.duration()),
      camera,
      transitionOut: this.store.transitionFor(entry),
      holdBetween: this.store.data?.defaults.holdBetween ?? 0.3,
      voice: audio.voice.map((c, k) => this.clipInfo(c, "voice", k, tracks)),
      sfx: audio.sfx.map((c, k) => this.clipInfo(c, "sfx", k, tracks)),
      fx: fxl.length ? fxl.map((f, k) => this.fxInfoOut(f, k)) : undefined,
      ambients,
    };
    tl.free();
    return json(res);
  }

  async t_set_keyframes(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    const size = await this.size(i);
    if (!Array.isArray(a.keys) || !a.keys.length) fail("keys must be a non-empty array");
    const keys: Keyframe[] = (a.keys as Args[]).slice(0, 500).map((k, n) => ({
      t: num(k, "t", 0, 0, 3600),
      pose: { cx: num(k, "cx", 0.5, -1, 2), cy: num(k, "cy", 0.5, -1, 2), zoom: num(k, "zoom", 1, 0.05, 64), rot: num(k, "rot", 0, -3600, 3600) },
      easing: { type: oneOf(k, "easing", EASINGS) ?? "ease-in-out" } as Keyframe["easing"],
      _n: n,
    })).sort((x, y) => x.t - y.t);
    for (const k of keys) delete (k as Partial<{ _n: number }>)._n;
    const t0 = keys[0].t;
    for (const k of keys) k.t -= t0; // la primera clave abre la imagen
    const dur = Math.max(0.1, keys.at(-1)!.t);
    const tl: Timeline = { camera: [{ name: "Keyframes", start: 0, duration: dur, clip: { inline: { type: "keyframes", keys } }, trimIn: 0, trimOut: 0 }] };
    this.edit(() => this.store.setTimeline(entry, size, tl));
    this.show(i);
    return json({ image: i, entry, keys: keys.length, duration: r2(dur), note: t0 > 0 ? `times shifted by -${r2(t0)} s so the first key is at 0` : undefined });
  }

  async t_apply_template(a: Args): Promise<ToolResult> {
    this.src();
    const id = oneOf(a, "template", ["comic", "scroll", "bounce", "ken-burns", "full", "none"] as const) ?? fail("template is required");
    let tpl: TemplateRef | null = null;
    if (id !== "none") {
      tpl = fresh(id);
      if (a.params && typeof a.params === "object") tpl.params = { ...tpl.params, ...(a.params as Record<string, unknown>) };
    }
    if (!Array.isArray(a.images)) {
      this.edit(() => this.store.setDefaultTemplate(tpl));
      return json({ default: tpl, note: "Images with their own animation keep it." });
    }
    const list = (a.images as unknown[]).map((v) => this.img({ image: v }));
    const sizes = await Promise.all(list.map((x) => this.size(x.i)));
    this.edit(() => list.forEach((x, k) => this.store.setItemTemplate(x.entry, sizes[k], tpl)));
    if (list[0]) this.show(list[0].i);
    return json({ images: list.map((x) => x.i), template: tpl });
  }

  async t_set_transition(a: Args): Promise<ToolResult> {
    const type = oneOf(a, "type", [...TRANSITIONS, "default"] as const) ?? fail("type is required");
    const dir = oneOf(a, "dir", DIRS);
    const tr: Transition | null = type === "default" ? null : { type, duration: num(a, "duration", 0.5, 0, 5), ...(dir ? { dir } : {}) };
    if (a.image === undefined || a.image === null) {
      if (!tr) fail("type 'default' needs an image");
      this.edit(() => this.store.setTransition(null, tr));
      return json({ default: tr });
    }
    const { i, entry } = this.img(a);
    const size = await this.size(i);
    this.edit(() => this.store.setTransition(entry, tr, size));
    return json({ image: i, transition: this.store.transitionFor(entry) });
  }

  async t_set_hold_between(a: Args): Promise<ToolResult> {
    const s = num(a, "seconds", 0.3, 0, 10);
    this.edit(() => this.store.setHoldBetween(s));
    return json({ holdBetween: s });
  }

  // -------------------------------------------------------------------------------------------
  // Sonido

  private async soundList() {
    if (!sounds.loaded) await sounds.load();
    return sounds.list;
  }

  private sound(id: string) {
    const s = sounds.get(id);
    if (!s) fail(`unknown sound "${id}": use list_sounds or import_sounds`);
    return s!;
  }

  async t_list_sounds(a: Args): Promise<ToolResult> {
    const q = str(a, "query")?.toLowerCase();
    const kind = oneOf(a, "kind", ["builtin", "imported", "recorded"] as const);
    const list = (await this.soundList())
      .filter((s) => !kind || s.kind === kind)
      .map((s) => ({ id: s.id, name: sounds.name(s.id), kind: s.kind, seconds: r2(s.seconds), loops: s.ambient || undefined }))
      .filter((s) => !q || s.name.toLowerCase().includes(q) || s.id.toLowerCase().includes(q));
    return json({ sounds: list });
  }

  async t_find_audio_files(a: Args): Promise<ToolResult> {
    const folder = str(a, "folder") ?? fail("folder is required");
    const files = await invoke<unknown[]>("mcp_find_audio", { folder, recursive: a.recursive !== false, limit: 500 });
    return json({ folder, files });
  }

  async t_import_sounds(a: Args): Promise<ToolResult> {
    if (!Array.isArray(a.paths) || !a.paths.every((p) => typeof p === "string")) fail("paths must be an array of strings");
    await this.soundList();
    const r = await sounds.import((a.paths as string[]).slice(0, 200));
    return json({ added: r.added.map((s) => ({ id: s.id, name: s.name, seconds: r2(s.seconds) })), errors: r.errors });
  }

  private trackInfo(t: Track, all: Track[]) {
    return { id: t.id, bus: t.bus, name: nameOf(all, t), volume: r2(t.volume), pan: r2(t.pan), mute: t.mute || undefined, effects: t.effects?.map((e) => ({ type: e.type, preset: e.preset, on: e.on, params: e.params })) };
  }

  async t_list_tracks(): Promise<ToolResult> {
    const all = this.store.tracks;
    return json({ tracks: all.map((t) => this.trackInfo(t, all)), mix: this.store.mix });
  }

  /** Efectos que manda la IA → efectos del proyecto (validados contra el catálogo). */
  private effects(v: unknown): Effect[] {
    if (!Array.isArray(v)) fail("effects must be an array");
    return (v as Args[]).slice(0, 12).map((x, k) => {
      const type = oneOf(x, "type", FX_TYPES) ?? fail(`effects[${k}].type is required`);
      const def = FX[type as FxType];
      const e = newEffect(type as FxType);
      const preset = str(x, "preset");
      if (preset) {
        const p = def.presets.find((p) => p.id === preset) ?? fail(`unknown preset ${preset} for ${type}: ${def.presets.map((p) => p.id).join(", ")}`);
        e.preset = p.id;
        e.params = { ...p.params };
      }
      if (x.params && typeof x.params === "object") {
        for (const [key, val] of Object.entries(x.params as Record<string, unknown>)) {
          const pd = def.params.find((p) => p.key === key) ?? fail(`${type} has no parameter ${key}: ${def.params.map((p) => p.key).join(", ")}`);
          if (typeof val !== "number" && typeof val !== "boolean") fail(`${type}.${key} must be a number`);
          e.params[key] = Math.min(pd.max, Math.max(pd.min, Number(val)));
        }
        if (Object.keys(x.params as object).length) delete e.preset;
      }
      e.on = x.on !== false;
      return e;
    });
  }

  private track(id: string | undefined, bus: SoundBus): string | undefined {
    if (id === undefined) return undefined;
    const t = this.store.tracks.find((t) => t.id === id);
    if (!t) fail(`unknown track ${id}: use list_tracks`);
    if (t!.bus !== bus) fail(`track ${id} is a ${t!.bus} track, not ${bus}`);
    return id;
  }

  async t_add_track(a: Args): Promise<ToolResult> {
    const bus = oneOf(a, "bus", ["voice", "sfx", "ambient"] as const) ?? fail("bus is required");
    let id: string | null = null;
    this.edit(() => {
      id = this.store.addTrack(bus);
      if (!id) return;
      this.store.editTrack(id, (t) => {
        const name = str(a, "name")?.trim();
        if (name) t.name = name.slice(0, 80);
        t.volume = num(a, "volume", 1, 0, 2);
        t.pan = num(a, "pan", 0, -1, 1);
      });
    });
    if (!id) fail("could not add the track");
    const all = this.store.tracks;
    return json({ track: this.trackInfo(all.find((t) => t.id === id)!, all) });
  }

  async t_edit_track(a: Args): Promise<ToolResult> {
    const id = str(a, "track") ?? fail("track is required");
    const t = this.store.tracks.find((t) => t.id === id) ?? fail(`unknown track ${id}: use list_tracks`);
    const effects = a.effects !== undefined ? this.effects(a.effects) : undefined;
    this.edit(() =>
      this.store.editTrack(t.id, (x) => {
        const name = str(a, "name");
        if (name !== undefined) {
          if (name.trim()) x.name = name.trim().slice(0, 80);
          else delete x.name;
        }
        if (a.volume !== undefined) x.volume = num(a, "volume", 1, 0, 2);
        if (a.pan !== undefined) x.pan = num(a, "pan", 0, -1, 1);
        if (a.mute !== undefined) {
          if (a.mute === true) x.mute = true;
          else delete x.mute;
        }
        if (effects) {
          if (effects.length) x.effects = effects;
          else delete x.effects;
        }
      }),
    );
    const all = this.store.tracks;
    return json({ track: this.trackInfo(all.find((x) => x.id === id)!, all) });
  }

  /** Aplica a un clip los campos que manda la IA. */
  private applyClip(c: AudioClip, a: Args, bus: AudioBus, seconds: number) {
    if (a.start !== undefined) c.start = num(a, "start", 0, 0, 3600);
    if (a.trimIn !== undefined) c.trimIn = num(a, "trimIn", 0, 0, Math.max(0, seconds - 0.1));
    const rest = Math.max(0.1, seconds - c.trimIn);
    if (a.duration !== undefined) c.duration = num(a, "duration", rest, 0.1, rest);
    else c.duration = Math.min(c.duration, rest);
    if (a.volume !== undefined) c.volume = num(a, "volume", 1, 0, 2);
    if (a.fadeIn !== undefined) c.fadeIn = num(a, "fadeIn", 0, 0, c.duration);
    if (a.fadeOut !== undefined) c.fadeOut = num(a, "fadeOut", 0, 0, c.duration);
    c.fadeIn = Math.min(c.fadeIn, c.duration);
    c.fadeOut = Math.min(c.fadeOut, c.duration - c.fadeIn);
    const name = str(a, "name");
    if (name !== undefined) {
      if (name.trim()) c.name = name.trim().slice(0, 80);
      else delete c.name;
    }
    if (a.track !== undefined) {
      const id = this.track(str(a, "track"), bus);
      if (id) c.track = id;
    }
    if (a.effects !== undefined) {
      const fx = this.effects(a.effects);
      if (fx.length) c.effects = fx;
      else delete c.effects;
    }
  }

  async t_add_audio(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    await this.soundList();
    const s = this.sound(str(a, "sound") ?? fail("sound is required"));
    const bus = oneOf(a, "bus", ["voice", "sfx"] as const) ?? fail("bus is required");
    const size = await this.size(i);
    const c = newAudioClip(s, num(a, "start", 0, 0, 3600));
    this.applyClip(c, a, bus, s.seconds);
    let index = -1;
    this.edit(() =>
      this.store.editAudio(entry, size, (t) => {
        t[bus].push(c);
        index = t[bus].length - 1;
      }),
    );
    this.show(i);
    return json({ image: i, clip: this.clipInfo(c, bus, index, this.store.tracks) });
  }

  private clipAt(a: Args): { i: number; entry: string; bus: AudioBus; index: number; clip: AudioClip } {
    const { i, entry } = this.img(a);
    const bus = oneOf(a, "bus", ["voice", "sfx"] as const) ?? fail("bus is required");
    const index = int(a, "index");
    const clip = this.store.audioFor(entry)[bus][index] ?? fail(`no ${bus} clip ${index} in image ${i}: use get_timeline`);
    return { i, entry, bus, index, clip };
  }

  async t_edit_audio(a: Args): Promise<ToolResult> {
    const { i, entry, bus, index } = this.clipAt(a);
    await this.soundList();
    const size = await this.size(i);
    let after: AudioClip | null = null;
    // Se valida sobre una copia: si algo falla no queda nada a medias.
    const copy = JSON.parse(JSON.stringify(this.store.audioFor(entry)[bus][index])) as AudioClip;
    this.applyClip(copy, a, bus, sounds.get(copy.sound)?.seconds ?? copy.trimIn + copy.duration);
    this.edit(() =>
      this.store.editAudio(entry, size, (t: AudioTracks) => {
        t[bus][index] = copy;
        after = t[bus][index];
      }),
    );
    this.show(i);
    return json({ image: i, clip: this.clipInfo(after!, bus, index, this.store.tracks) });
  }

  async t_remove_audio(a: Args): Promise<ToolResult> {
    const { i, entry, bus, index } = this.clipAt(a);
    const size = await this.size(i);
    this.edit(() => this.store.editAudio(entry, size, (t) => void t[bus].splice(index, 1)));
    return json({ image: i, removed: { bus, index }, note: "Indexes of later clips in this bus moved down by one." });
  }

  private ambientInfo(x: AmbientClip) {
    const idx = (e: string) => this.src().entries.findIndex((y) => y.name === e);
    return { id: x.id, sound: x.sound, soundName: sounds.get(x.sound) ? sounds.name(x.sound) : "(missing)", name: x.name, from: idx(x.from), to: idx(x.to), volume: r2(x.volume), fadeIn: x.fadeIn, fadeOut: x.fadeOut, track: x.track, effects: x.effects?.map((e) => e.preset ?? e.type) };
  }

  private applyAmbient(x: AmbientClip, a: Args) {
    const s = this.src();
    if (a.sound !== undefined) x.sound = this.sound(str(a, "sound")!).id;
    if (a.from !== undefined) x.from = this.img({ image: a.from }).entry;
    if (a.to !== undefined) x.to = this.img({ image: a.to }).entry;
    const fi = s.entries.findIndex((e) => e.name === x.from);
    const ti = s.entries.findIndex((e) => e.name === x.to);
    if (fi > ti) [x.from, x.to] = [x.to, x.from];
    if (a.volume !== undefined) x.volume = num(a, "volume", 0.8, 0, 2);
    if (a.fadeIn !== undefined) x.fadeIn = num(a, "fadeIn", 1, 0, 30);
    if (a.fadeOut !== undefined) x.fadeOut = num(a, "fadeOut", 1.5, 0, 30);
    const name = str(a, "name");
    if (name !== undefined) {
      if (name.trim()) x.name = name.trim().slice(0, 80);
      else delete x.name;
    }
    if (a.track !== undefined) {
      const id = this.track(str(a, "track"), "ambient");
      if (id) x.track = id;
    }
    if (a.effects !== undefined) {
      const fx = this.effects(a.effects);
      if (fx.length) x.effects = fx;
      else delete x.effects;
    }
  }

  async t_add_ambient(a: Args): Promise<ToolResult> {
    await this.soundList();
    this.writable();
    const x: AmbientClip = { id: "", sound: "", from: this.img(a, "from").entry, to: this.img(a, "to").entry, volume: 0.8, fadeIn: 1, fadeOut: 1.5 };
    this.applyAmbient(x, { ...a, from: undefined, to: undefined, sound: a.sound ?? fail("sound is required") });
    const { id: _, ...rest } = x;
    let id: string | null = null;
    this.edit(() => (id = this.store.addAmbient(rest)));
    if (!id) fail("could not add the ambience");
    return json({ ambient: this.ambientInfo(this.store.ambients.find((y) => y.id === id)!) });
  }

  async t_edit_ambient(a: Args): Promise<ToolResult> {
    await this.soundList();
    const id = str(a, "id") ?? fail("id is required");
    const cur = this.store.ambients.find((y) => y.id === id) ?? fail(`unknown ambience ${id}: use summary`);
    const copy = JSON.parse(JSON.stringify(cur)) as AmbientClip;
    this.applyAmbient(copy, a);
    this.edit(() => this.store.editAmbient(id, (y) => Object.assign(y, copy)));
    return json({ ambient: this.ambientInfo(this.store.ambients.find((y) => y.id === id)!) });
  }

  async t_remove_ambient(a: Args): Promise<ToolResult> {
    const id = str(a, "id") ?? fail("id is required");
    if (!this.store.ambients.some((y) => y.id === id)) fail(`unknown ambience ${id}`);
    this.edit(() => this.store.removeAmbient(id));
    return json({ removed: id });
  }

  async t_set_mix(a: Args): Promise<ToolResult> {
    this.edit(() =>
      this.store.setMix((m) => {
        if (a.voice !== undefined) m.voice = num(a, "voice", 1, 0, 2);
        if (a.sfx !== undefined) m.sfx = num(a, "sfx", 1, 0, 2);
        if (a.ambient !== undefined) m.ambient = num(a, "ambient", 0.7, 0, 2);
        if (typeof a.duck === "boolean") m.duck = a.duck;
      }),
    );
    return json({ mix: this.store.mix });
  }

  // -------------------------------------------------------------------------------------------
  // Efectos especiales visuales

  async t_fx_catalog(): Promise<ToolResult> {
    const cat = fxCatalog();
    return json({
      effects: cat.effects.map((e) => ({
        type: e.type,
        kind: e.sustained ? "sustained (fades in/out)" : "impact (hits and decays)",
        duration: e.duration,
        params: e.params,
        presets: e.presets.map((p) => ({ id: p.id, duration: p.duration, params: Object.fromEntries(p.params) })),
      })),
      situations: cat.situations,
      notes: [
        "Percent units are of the video height. Times in seconds from the start of the image.",
        "at: normalized image point (0..1) where the effect happens; it follows the image when the camera moves. Without it, the center of the screen.",
        "Flashes and impact frames blink at most 3 times per second (photosensitivity).",
      ],
    });
  }

  private fxInfoOut(f: Fx, k: number) {
    return { id: f.id, index: k, type: f.type, preset: f.preset, start: r2(f.start), duration: r2(f.duration), intensity: f.intensity, params: f.params, at: f.at };
  }

  private fxById(entry: string, id: string): { list: Fx[]; k: number } {
    const list = this.store.fxFor(entry);
    const k = list.findIndex((f) => f.id === id);
    if (k < 0) fail(`no effect "${id}" in this image: use get_timeline`);
    return { list, k };
  }

  private fxParams(type: string, v: unknown): Record<string, number | string> | undefined {
    if (v === undefined || v === null) return undefined;
    if (typeof v !== "object" || Array.isArray(v)) fail("params must be an object");
    const info = fxInfo(type)!;
    const out: Record<string, number | string> = {};
    for (const [k, x] of Object.entries(v as Record<string, unknown>)) {
      const p = info.params.find((q) => q.key === k);
      if (!p) fail(`${type} has no parameter "${k}" (fx_catalog lists them)`);
      if (x === null) continue;
      if (p!.kind === "number") {
        if (typeof x !== "number" || !Number.isFinite(x)) fail(`${k} must be a number`);
        out[k] = Math.min(p!.max, Math.max(p!.min, x as number));
      } else if (p!.kind === "choice") {
        if (typeof x !== "string" || !p!.options.includes(x)) fail(`${k} must be one of: ${p!.options.join(", ")}`);
        out[k] = x as string;
      } else {
        if (typeof x !== "string" || !/^#[0-9a-f]{6}$/i.test(x)) fail(`${k} must be a color like #ff2020`);
        out[k] = (x as string).toLowerCase();
      }
    }
    return out;
  }

  private pointOf(v: unknown): { x: number; y: number } {
    const p = v as { x: number; y: number };
    if (!p || typeof p !== "object" || ![p.x, p.y].every((n) => typeof n === "number" && Number.isFinite(n))) fail("at needs numbers x and y (0..1)");
    return { x: r4(Math.min(1, Math.max(0, p.x))), y: r4(Math.min(1, Math.max(0, p.y))) };
  }

  /** Segundo de la imagen donde empieza: `start` o la llegada del plano `shot` más `offset`. */
  private async fxStart(i: number, a: Args): Promise<number> {
    if (a.shot === undefined || a.shot === null) return num(a, "start", 0, 0, 3600);
    const shot = int(a, "shot");
    const { tl } = await this.playTimeline(i);
    try {
      let holds = this.holds(tl);
      // Como en plan_shots: la vista de la página entera del principio no cuenta como plano.
      const first = holds[0] && Float64Array.from(tl.evaluate(holds[0][0]));
      if (first && holds[0][0] < 0.01 && Math.abs(first[0] - 0.5) < 0.01 && Math.abs(first[1] - 0.5) < 0.01 && Math.abs(first[2] - 1) < 0.02) holds = holds.slice(1);
      if (shot < 1 || shot > holds.length) fail(`shot ${shot} out of range: this image has ${holds.length} shots (1..${holds.length})`);
      return Math.max(0, holds[shot - 1][0] + num(a, "offset", 0, -60, 60));
    } finally {
      tl.free();
    }
  }

  async t_add_fx(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    const situation = str(a, "situation");
    const type = str(a, "type");
    if (!situation === !type) fail("give either situation or type");
    const start = await this.fxStart(i, a);
    const size = await this.size(i);
    const at = a.at === undefined || a.at === null ? undefined : this.pointOf(a.at);
    let added: Fx[];
    let sound: string | null = null;
    if (situation) {
      const sit = fxCatalog().situations.find((x) => x.id === situation);
      if (!sit) fail(`unknown situation "${situation}": ${fxCatalog().situations.map((x) => x.id).join(", ")}`);
      const r = JSON.parse(wasm.fx_situation(situation, fxId(), start, at?.x ?? NaN, at?.y ?? NaN)) as { fx: Fx[]; sound?: string | null };
      added = r.fx;
      if (a.intensity !== undefined) for (const f of added) f.intensity = r2((f.intensity ?? 1) * num(a, "intensity", 1, 0, 3));
      if (a.sound !== false) sound = r.sound ?? null;
    } else {
      const info = fxInfo(type!);
      if (!info) fail(`unknown effect type "${type}": ${fxCatalog().effects.map((x) => x.type).join(", ")}`);
      const preset = str(a, "preset");
      if (preset && !info!.presets.some((p) => p.id === preset)) fail(`${type} has no preset "${preset}": ${info!.presets.map((p) => p.id).join(", ")}`);
      const f = JSON.parse(wasm.fx_new(fxId(), type!, preset ?? "", start)) as Fx;
      const d = optNum(a, "duration", MIN_FX_DURATION, 120);
      if (d !== undefined) f.duration = d;
      if (a.intensity !== undefined) f.intensity = num(a, "intensity", 1, 0, 3);
      const params = this.fxParams(type!, a.params);
      if (params && Object.keys(params).length) {
        f.params = { ...(f.params ?? {}), ...params };
        delete f.preset;
      }
      if (at) f.at = at;
      added = [f];
    }
    let clip: AudioClip | null = null;
    if (sound) {
      await this.soundList();
      const s = sounds.get(sound);
      if (s) clip = newAudioClip(s, start);
    }
    let first = 0;
    this.edit(() => {
      this.store.editFx(entry, size, (list) => {
        first = list.length;
        list.push(...added);
      });
      if (clip) this.store.editAudio(entry, size, (t) => void t.sfx.push(clip!));
    });
    this.show(i);
    const list = this.store.fxFor(entry);
    return json({
      image: i,
      added: added.map((_, k) => this.fxInfoOut(list[first + k], first + k)),
      sound: clip ? { sound: clip.sound, bus: "sfx", index: this.store.audioFor(entry).sfx.length - 1, start: r2(clip.start) } : undefined,
      warnings: this.fxWarnings(i, entry),
    });
  }

  async t_edit_fx(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    const id = str(a, "id") ?? fail("id is required");
    const { list, k } = this.fxById(entry, id!);
    const copy = JSON.parse(JSON.stringify(list[k])) as Fx;
    const info = fxInfo(copy.type) ?? fail(`effect ${id} has an unknown type`);
    if (a.start !== undefined) copy.start = num(a, "start", 0, 0, 3600);
    if (a.duration !== undefined) copy.duration = num(a, "duration", 1, MIN_FX_DURATION, 120);
    if (a.intensity !== undefined) copy.intensity = num(a, "intensity", 1, 0, 3);
    const preset = str(a, "preset");
    if (preset) {
      const p = info!.presets.find((x) => x.id === preset) ?? fail(`${copy.type} has no preset "${preset}"`);
      copy.preset = p!.id;
      copy.params = Object.fromEntries(p!.params);
    }
    if (a.params !== undefined && a.params !== null) {
      const raw = a.params as Record<string, unknown>;
      const set = this.fxParams(copy.type, raw) ?? {};
      const merged: Record<string, number | string> = { ...(copy.params ?? {}), ...set };
      for (const [key, v] of Object.entries(raw)) if (v === null) delete merged[key];
      copy.params = merged;
      delete copy.preset;
    }
    if (copy.params && !Object.keys(copy.params).length) delete copy.params;
    if (a.resetPoint === true) delete copy.at;
    else if (a.at !== undefined && a.at !== null) copy.at = this.pointOf(a.at);
    const size = await this.size(i);
    this.edit(() =>
      this.store.editFx(entry, size, (l) => {
        const n = l.findIndex((f) => f.id === id);
        if (n >= 0) l[n] = copy;
      }),
    );
    this.show(i);
    return json({ image: i, fx: this.fxInfoOut(copy, k), warnings: this.fxWarnings(i, entry) });
  }

  async t_remove_fx(a: Args): Promise<ToolResult> {
    const { i, entry } = this.img(a);
    const id = str(a, "id") ?? fail("id is required");
    this.fxById(entry, id!);
    const size = await this.size(i);
    this.edit(() => this.store.editFx(entry, size, (l) => void l.splice(l.findIndex((f) => f.id === id), 1)));
    return json({ image: i, removed: id });
  }

  /** Avisos de los efectos de una imagen (destellos seguidos, efectos que alargan la imagen…). */
  private fxWarnings(i: number, entry: string): string[] | undefined {
    const list = this.store.fxFor(entry);
    if (!list.length) return undefined;
    void i;
    const audio = this.store.audioFor(entry);
    const it = this.store.item(entry);
    const cam = it?.timeline?.camera?.length && !this.store.templateFor(entry) ? Math.max(...it.timeline.camera.map((c) => c.start + c.duration)) : 0;
    const end = Math.max(cam, audioEndOf(audio));
    const w = JSON.parse(wasm.fx_warnings(JSON.stringify($state.snapshot(list)), end > 0 ? end : fxEnd(list))) as string[];
    return w.length ? w : undefined;
  }

  async t_effects_catalog(): Promise<ToolResult> {
    return json({
      effects: FX_TYPES.map((type) => ({
        type,
        params: FX[type].params.map((p) => ({ key: p.key, min: p.min, max: p.max, default: p.def, unit: p.unit === "%" ? "0..1" : p.unit })),
        presets: FX[type].presets,
      })),
      note: "Track effects apply to everything on the track; clip effects only to that clip (before the track's).",
    });
  }

  // -------------------------------------------------------------------------------------------
  // Revisar

  /** Tramos quietos de una timeline (para un fotograma por plano). */
  private holds(tl: wasm.WasmTimeline): [number, number][] {
    const marks = Array.from(tl.marks()).sort((x, y) => x - y);
    const out: [number, number][] = [];
    const same = (p: Float64Array, q: Float64Array) => p.every((v, k) => Math.abs(v - q[k]) < 1e-6);
    for (let k = 0; k + 1 < marks.length; k++) {
      const [t0, t1] = [marks[k], marks[k + 1]];
      if (t1 - t0 < 0.05) continue;
      const a = Float64Array.from(tl.evaluate(t0));
      const b = Float64Array.from(tl.evaluate(t1));
      const mid = Float64Array.from(tl.evaluate((t0 + t1) / 2));
      // Quieto (o acercándose despacio: mismo centro, poco cambio de zoom).
      const slow = Math.abs(a[0] - b[0]) < 1e-4 && Math.abs(a[1] - b[1]) < 1e-4 && Math.abs(Math.log(b[2] / a[2])) < 0.2;
      if (same(a, b) || (slow && Math.abs(mid[0] - a[0]) < 1e-4)) out.push([t0, t1]);
    }
    return out;
  }

  async t_summary(a: Args): Promise<ToolResult> {
    const s = this.src();
    await this.soundList();
    const from = Math.max(0, int(a, "from", 0));
    const count = Math.min(200, Math.max(1, int(a, "count", 40)));
    const to = Math.min(s.entries.length, from + count);
    const hold = this.store.data?.defaults.holdBetween ?? 0.3;
    const tracks = this.store.tracks;
    const images = [];
    let total = 0;
    for (let i = from; i < to; i++) {
      const entry = s.entries[i].name;
      const it = this.store.item(entry);
      const { tl } = await this.playTimeline(i);
      const audio = this.store.audioFor(entry);
      const fxl = this.store.fxFor(entry);
      const dur = itemDuration(tl, audio, fxl);
      const tr = this.store.transitionFor(entry);
      const last = i === s.entries.length - 1;
      total += dur + hold + (last || tr.type === "cut" ? 0 : tr.duration);
      const holds = this.holds(tl);
      const warnings: string[] = [];
      const tpl = this.store.templateFor(entry);
      if (!tpl && !it?.timeline?.camera?.length) warnings.push("no animation: the whole image shows for 3 s");
      holds.forEach(([h0, h1], k) => {
        if (h1 - h0 < 1.2) warnings.push(`shot ${k + 1} (at ${r2(h0)} s) holds only ${r2(h1 - h0)} s`);
      });
      if (audioEndOf(audio) > tl.duration() + 0.3) warnings.push(`sound lasts until ${r2(audioEndOf(audio))} s but the camera stops at ${r2(tl.duration())} s`);
      for (const bus of ["voice", "sfx"] as const) {
        audio[bus].forEach((c, k) => {
          const snd = sounds.get(c.sound);
          if (!snd) warnings.push(`${bus} clip ${k} uses a sound missing from the library`);
          else if (c.trimIn + c.duration > snd.seconds + 0.05) warnings.push(`${bus} clip ${k} is longer than its sound`);
        });
      }
      for (const [x, y] of overlaps(audio.voice, tracks)) warnings.push(`voice clips ${x} and ${y} overlap on the same track`);
      warnings.push(...(this.fxWarnings(i, entry) ?? []));
      images.push({
        image: i,
        entry,
        animation: tpl ? `template:${tpl.id}` : it?.timeline?.camera?.length ? "keyframes" : "none",
        duration: r2(dur),
        shots: holds.length,
        voices: audio.voice.length || undefined,
        sfx: audio.sfx.length || undefined,
        fx: fxl.length ? fxl.map((f) => `${f.preset ?? f.type}@${r2(f.start)}s`) : undefined,
        transition: last ? undefined : `${tr.type} ${tr.duration}s`,
        warnings: warnings.length ? warnings : undefined,
      });
      tl.free();
    }
    return json({
      images,
      range: { from, to: to - 1, seconds: r2(total), all: from === 0 && to === s.entries.length },
      ambients: this.store.ambients.map((x) => this.ambientInfo(x)),
      tracks: tracks.map((t) => this.trackInfo(t, tracks)),
      mix: this.store.mix,
      holdBetween: hold,
      output: this.video(),
    });
  }

  async t_render_frames(a: Args): Promise<ToolResult> {
    const { i } = this.img(a);
    const { tl } = await this.playTimeline(i);
    try {
      const entry = this.src().entries[i].name;
      const audio = this.store.audioFor(entry);
      const fxl = $state.snapshot(this.store.fxFor(entry)) as Fx[];
      const dur = itemDuration(tl, audio, fxl);
      let times: number[];
      if (Array.isArray(a.times) && a.times.length) times = (a.times as unknown[]).filter((v): v is number => typeof v === "number" && Number.isFinite(v)).map((v) => Math.min(dur, Math.max(0, v)));
      else if (a.count !== undefined) {
        const n = Math.min(16, Math.max(1, int(a, "count")));
        times = Array.from({ length: n }, (_, k) => ((k + 0.5) / n) * dur);
      } else {
        times = this.holds(tl).map(([h0, h1]) => (h0 + h1) / 2);
        if (!times.length) times = [0, dur / 2, dur].map((t) => Math.min(dur, t));
        // El momento fuerte de cada efecto (poco después de empezar o a mitad si es sostenido).
        for (const f of fxl) times.push(f.start + (fxInfo(f.type)?.sustained ? f.duration / 2 : Math.min(0.08, f.duration / 3)));
        times = [...new Set(times.map(r2))].sort((x, y) => x - y);
      }
      times = times.slice(0, 16);
      const v = this.video();
      const fw = mult4(num(a, "width", 400, 128, 960));
      const fh = mult4(fw / v.aspect);
      const frames = [];
      const track = FxTrack.of(fxl);
      try {
        for (const [k, t] of times.entries()) {
          const pose = Float64Array.from(tl.evaluate(Math.min(t, tl.duration())));
          frames.push({ img: await this.frames().render(this.ctx.refAt(i), pose, fw, fh, prefs.settings.background, { track, t }), label: `${k + 1} · ${t.toFixed(2)} s` });
        }
      } finally {
        track?.free();
      }
      const sheet = contactSheet(frames);
      return json({ image: i, duration: r2(dur), output: `${v.width}x${v.height}`, frames: times.map((t, k) => ({ n: k + 1, t: r2(t) })) }, [jpeg(sheet)]);
    } finally {
      tl.free();
    }
  }

  async t_sound_report(a: Args): Promise<ToolResult> {
    const s = this.src();
    await this.soundList();
    const from = Math.max(0, Math.min(s.entries.length - 1, int(a, "from", 0)));
    const to = Math.max(from, Math.min(s.entries.length - 1, int(a, "to", s.entries.length - 1)));
    const hold = this.ctx.sequence.holdBetween();
    const schedule = new SoundSchedule();
    const starts: { image: number; start: number; end: number }[] = [];
    const notes: string[] = [];
    const tracks = this.store.tracks;
    let t = 0;
    for (let i = from; i <= to; i++) {
      const entry = s.entries[i].name;
      const { tl } = await this.playTimeline(i);
      const audio = this.ctx.sequence.audio?.(i);
      const dur = itemDuration(tl, audio, this.ctx.sequence.fx?.(i));
      tl.free();
      schedule.image(t, audio, this.ctx.sequence.ambient?.(i) ?? []);
      starts.push({ image: i, start: r2(t), end: r2(t + dur + hold) });
      t += dur + hold;
      schedule.imageEnd(t);
      const a2 = this.store.audioFor(entry);
      for (const [x, y] of overlaps(a2.voice, tracks)) notes.push(`image ${i}: voice clips ${x} and ${y} overlap on the same track`);
      if (i < to) {
        const tr = this.ctx.sequence.transitionOut(i);
        if (!tr.cut) t += tr.duration;
      }
    }
    const clips: ClipSpec[] = schedule.finish();
    if (!clips.length) return json({ range: { from, to }, seconds: r2(t), note: "No sounds in this range." });
    const audio = { clips, tracks: this.ctx.sequence.tracks?.() ?? trackSpecs(JSON.parse(JSON.stringify(tracks))), levels: JSON.parse(JSON.stringify(this.store.mix)), seconds: t };
    const report = await invoke<Record<string, unknown>>("mcp_sound_report", { audio });
    return json({ range: { from, to }, images: starts, ...report, notes });
  }

  async t_play(a: Args): Promise<ToolResult> {
    this.src();
    if (a.stop === true) {
      this.ctx.stop();
      return json({ playing: false });
    }
    const i = a.image === undefined ? this.ctx.index() : this.img(a).i;
    if (this.ctx.playing()) this.ctx.stop();
    this.ctx.go(i);
    // El visor tiene que haber cargado la imagen antes de reproducir.
    await new Promise((r) => setTimeout(r, 300));
    this.ctx.play(i);
    return json({ playing: true, from: i, note: "Playing in the app for the user. You cannot hear it: ask the user what they think." });
  }

  // -------------------------------------------------------------------------------------------
  // Exportar

  async t_export_video(a: Args): Promise<ToolResult> {
    const s = this.src();
    if (this.exportJob?.state === "running") fail("an export is already running: use export_status or cancel_export");
    const last = s.entries.length - 1;
    const from = Math.max(0, Math.min(last, int(a, "from", 0)));
    const to = Math.max(from, Math.min(last, int(a, "to", last)));
    const dir = s.kind === "folder" ? s.path : dirOf(s.path);
    const stem = s.title.replace(/\.[^./]+$/, "") || "video";
    let path = str(a, "path") ?? joinPath(dir, `${stem}.mp4`);
    if (!/\.mp4$/i.test(path)) path += ".mp4";
    const [exists] = await invoke<boolean[]>("paths_exist", { paths: [path] });
    if (exists && a.overwrite !== true) fail(`${path} already exists: pass overwrite: true or another path`);
    const ff = await invoke<{ encoder: string; path: string }>("ffmpeg_info", { path: prefs.settings.video.ffmpeg || null }).catch((e) => fail(`ffmpeg: ${e}`));
    if (!ff.encoder) fail(`${ff.path} has no H.264 encoder`);
    const v = this.video();
    if (this.ctx.playing()) this.ctx.stop();
    const abort = new AbortController();
    const job: ExportJob = { state: "running", path, fraction: 0, frames: 0, mixing: false, started: Date.now(), abort };
    this.exportJob = job;
    const crf = { high: 18, normal: 23, light: 28 }[v.quality];
    this.ctx.say("🤖 Exportando vídeo…");
    exportVideo(
      { path, width: v.width, height: v.height, fps: v.fps, crf, ffmpeg: prefs.settings.video.ffmpeg, background: prefs.settings.background, from, to, sound: a.sound === false ? null : JSON.parse(JSON.stringify(this.store.mix)) },
      this.ctx.sequence,
      this.ctx.refAt,
      (p) => {
        job.fraction = p.fraction;
        job.frames = p.frames;
        job.mixing = !!p.mixing;
      },
      abort.signal,
    ).then(
      (out) => {
        job.state = "done";
        job.path = out;
        job.fraction = 1;
        this.ctx.say("🤖 Vídeo exportado");
      },
      (e) => {
        job.state = e instanceof ExportCancelled ? "cancelled" : "error";
        job.error = String(e);
      },
    );
    return json({ started: true, path, from, to, output: v, note: "Poll export_status every few seconds." });
  }

  async t_export_status(): Promise<ToolResult> {
    const j = this.exportJob;
    if (!j) return json({ state: "idle" });
    return json({ state: j.state, path: j.path, fraction: r2(j.fraction), frames: j.frames, mixingSound: j.mixing || undefined, seconds: Math.round((Date.now() - j.started) / 1000), error: j.error });
  }

  async t_cancel_export(): Promise<ToolResult> {
    const j = this.exportJob;
    if (j?.state !== "running") return json({ state: j?.state ?? "idle" });
    j.abort.abort();
    return json({ cancelling: true });
  }

  async t_inspect_video(a: Args): Promise<ToolResult> {
    const path = str(a, "path") ?? fail("path is required");
    const width = num(a, "width", 480, 64, 1920);
    const ffmpeg = prefs.settings.video.ffmpeg || null;
    let times = Array.isArray(a.times) ? (a.times as unknown[]).filter((v): v is number => typeof v === "number" && v >= 0) : [];
    type Probe = { probe: { format?: { duration?: string; size?: string }; streams?: Record<string, unknown>[] }; audio: unknown; frames: { t: number; image: string | null }[] };
    let r = await invoke<Probe>("mcp_inspect_video", { path, times, width, ffmpeg });
    const duration = Number(r.probe?.format?.duration ?? 0);
    if (!times.length && duration > 0) {
      const n = Math.min(16, Math.max(1, int(a, "count", 6)));
      times = Array.from({ length: n }, (_, k) => r2(((k + 0.5) / n) * duration));
      r = await invoke<Probe>("mcp_inspect_video", { path, times, width, ffmpeg });
    }
    const streams = (r.probe?.streams ?? []).map((st) => {
      const { codec_type, codec_name, width: w, height: h, r_frame_rate, duration: d, channels, sample_rate } = st as Record<string, unknown>;
      return { type: codec_type, codec: codec_name, width: w, height: h, fps: r_frame_rate, duration: d, channels, sampleRate: sample_rate };
    });
    const images = r.frames.filter((f) => f.image).map((f) => `data:image/jpeg;base64,${f.image}`);
    return json(
      {
        path,
        duration: r2(duration),
        megabytes: r2(Number(r.probe?.format?.size ?? 0) / 1e6),
        streams,
        audioLevel: r.audio,
        frames: r.frames.map((f, k) => ({ n: k + 1, t: f.t, ok: !!f.image })),
        note: "Images are in the same order as frames.",
      },
      images,
    );
  }

  async t_undo(a: Args): Promise<ToolResult> {
    let n = 0;
    const steps = Math.min(50, Math.max(1, int(a, "steps", 1)));
    while (n < steps && this.store.undo()) n++;
    return json({ undone: n, canUndo: this.store.canUndo });
  }

  async t_redo(a: Args): Promise<ToolResult> {
    let n = 0;
    const steps = Math.min(50, Math.max(1, int(a, "steps", 1)));
    while (n < steps && this.store.redo()) n++;
    return json({ redone: n, canRedo: this.store.canRedo });
  }
}

/** Final del último sonido de la imagen. */
function audioEndOf(a: AudioTracks): number {
  return [...a.voice, ...a.sfx].reduce((m, c) => Math.max(m, c.start + c.duration), 0);
}

/** Pares de clips que se pisan en la misma pista. */
export function overlaps(list: AudioClip[], tracks: Track[]): [number, number][] {
  const out: [number, number][] = [];
  const first = tracks.find((t) => t.bus === "voice")?.id;
  const trackOf = (c: AudioClip) => (c.track && tracks.some((t) => t.id === c.track) ? c.track : first);
  for (let x = 0; x < list.length; x++) {
    for (let y = x + 1; y < list.length; y++) {
      const [p, q] = [list[x], list[y]];
      if (trackOf(p) !== trackOf(q)) continue;
      if (p.start < q.start + q.duration - 0.05 && q.start < p.start + p.duration - 0.05) out.push([x, y]);
    }
  }
  return out;
}

export { ToolError };
