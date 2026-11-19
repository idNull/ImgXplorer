// Proyecto `.imgx` de la fuente abierta: lo que el usuario configura en el Modo Acción.
// Se guarda solo (con un pequeño retraso) en cuanto cambia algo; Rust lo valida antes de escribir.
// Deshacer/rehacer guarda instantáneas del proyecto entero (es pequeño): cada pausa en la
// edición, o cada gesto completo (`batch`), es un paso.
import { invoke } from "@tauri-apps/api/core";
import type { TemplateRef } from "./catalog";
import { audioOf, hasAudio, type AudioTracks, type Timeline } from "../editor/model";
import type { Fx } from "../fx/fx";
import { normalize, type Adjust } from "../adjust/adjust";
import { log } from "../log";
import { DEFAULT_LEVELS, type MixLevels } from "../audio/director";
import type { Effect } from "../audio/effects";
import { newTrackId, resolveTrack, withDefaults, type SoundBus, type Track } from "../audio/tracks";

export type TransitionType = "cut" | "fade" | "slide" | "continuous-camera";
export type Dir = "left" | "right" | "up" | "down";

export interface Transition {
  type: TransitionType;
  dir?: Dir;
  duration: number;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Item {
  entry: string;
  size: [number, number];
  template?: TemplateRef;
  timeline?: Timeline;
  transitionOut?: Transition;
  /** Viñetas dibujadas a mano (sustituyen a la detección). */
  panels?: Rect[];
  /** Ajustes de imagen propios (🎨). */
  adjust?: Adjust;
}

export interface ProjectData {
  format: "imgx";
  version: number;
  source: { kind: "folder" | "archive" | "image"; name: string; fingerprint?: string };
  defaults: { transition: Transition; holdBetween: number; template?: TemplateRef; mix?: MixLevels; adjust?: Adjust };
  clips: unknown[];
  items: Item[];
  /** Ambientes o música que abarcan varias imágenes. */
  ambient?: AmbientClip[];
  /** Pistas de sonido (si no hay, una por grupo). */
  tracks?: Track[];
}

/** Sonido de fondo de una imagen a otra (en bucle). */
export interface AmbientClip {
  id: string;
  sound: string;
  name?: string;
  /** Primera y última imagen (nombre de la entrada). */
  from: string;
  to: string;
  volume: number;
  fadeIn: number;
  fadeOut: number;
  track?: string;
  effects?: Effect[];
}

/** Lo que `open_source` devuelve sobre el proyecto guardado. */
export interface LoadedProject {
  json: string | null;
  path: string | null;
  error: string | null;
}

const SAVE_DELAY = 500;
/** Sin cambios durante este tiempo, lo editado cuenta como un paso de deshacer. */
const STEP_DELAY = 350;
const MAX_UNDO = 200;

export class ProjectStore {
  data = $state<ProjectData | null>(null);
  /** Ruta del archivo guardado o cargado. */
  path = $state<string | null>(null);
  /** El archivo existente no se pudo leer: no se guarda para no destruirlo. */
  readOnly = $state<string | null>(null);
  saveError = $state<string | null>(null);
  /** Cambios (pasos de edición) aún sin guardar en disco. Normalmente vuelve a 0 en medio
   *  segundo; solo se queda alto si falla el guardado. */
  unsaved = $state(0);
  private generation = 0;
  private timer: number | undefined;
  private pending = false;
  private undoStack: string[] = [];
  private redoStack: string[] = [];
  /** Estado tras el último paso guardado en el historial. */
  private base: string | null = null;
  private stepTimer: number | undefined;
  private batching = 0;
  /** Hay cambios aún sin cerrar como paso de deshacer. */
  private stepPending = false;
  canUndo = $state(false);
  canRedo = $state(false);

  async load(
    generation: number,
    source: { kind: "folder" | "archive"; title: string; fingerprint: string },
    loaded: LoadedProject,
  ) {
    await this.flush();
    this.generation = generation;
    this.path = loaded.path;
    this.readOnly = loaded.error;
    this.saveError = null;
    this.unsaved = 0;
    let data: ProjectData | null = null;
    if (loaded.json) {
      try {
        data = JSON.parse(loaded.json) as ProjectData;
      } catch (e) {
        log.warn("proyecto ilegible:", e);
      }
    }
    data ??= {
      format: "imgx",
      version: 1,
      source: { kind: source.kind, name: source.title, fingerprint: source.fingerprint },
      defaults: { transition: { type: "fade", duration: 0.5 }, holdBetween: 0.3 },
      clips: [],
      items: [],
    };
    // Si se movió o renombró, la huella sigue valiendo; se actualiza el nombre.
    data.source.name = source.title;
    data.source.fingerprint = source.fingerprint;
    this.data = data;
    clearTimeout(this.stepTimer);
    this.stepPending = false;
    this.batching = 0;
    this.undoStack = [];
    this.redoStack = [];
    this.base = this.json();
    this.syncFlags();
  }

  private json(): string {
    return JSON.stringify($state.snapshot(this.data));
  }

  private syncFlags() {
    this.canUndo = this.undoStack.length > 0 || this.stepPending;
    this.canRedo = this.redoStack.length > 0;
  }

  /** Cierra el paso de deshacer en curso (si hubo cambios). */
  commit() {
    clearTimeout(this.stepTimer);
    this.stepPending = false;
    if (!this.data) return;
    const now = this.json();
    if (this.base !== null && now !== this.base) {
      this.undoStack.push(this.base);
      if (this.undoStack.length > MAX_UNDO) this.undoStack.shift();
      this.redoStack = [];
    }
    this.base = now;
    this.syncFlags();
  }

  /** Agrupa todos los cambios de un gesto (arrastre…) en un solo paso. */
  batch(active: boolean) {
    this.batching = Math.max(0, this.batching + (active ? 1 : -1));
    if (!this.batching) this.commit();
  }

  undo(): boolean {
    this.commit();
    const prev = this.undoStack.pop();
    if (prev === undefined || this.base === null) return false;
    this.redoStack.push(this.base);
    this.restore(prev);
    return true;
  }

  redo(): boolean {
    this.commit();
    const next = this.redoStack.pop();
    if (next === undefined || this.base === null) return false;
    this.undoStack.push(this.base);
    this.restore(next);
    return true;
  }

  private restore(json: string) {
    this.data = JSON.parse(json) as ProjectData;
    this.base = json;
    this.syncFlags();
    this.unsaved++;
    this.save();
  }

  item(entry: string): Item | undefined {
    return this.data?.items.find((i) => i.entry === entry);
  }

  /** Plantilla propia de la imagen o, si no tiene, la de todas. */
  templateFor(entry: string): TemplateRef | null {
    const it = this.item(entry);
    if (it?.template) return it.template;
    // Una timeline propia (editor, fase 3) manda sobre la plantilla general.
    if (it?.timeline?.camera?.length) return null;
    return this.data?.defaults.template ?? null;
  }

  transitionFor(entry: string): Transition {
    return this.item(entry)?.transitionOut ?? this.data?.defaults.transition ?? { type: "fade", duration: 0.5 };
  }

  setDefaultTemplate(tpl: TemplateRef | null) {
    if (!this.data) return;
    if (tpl) this.data.defaults.template = tpl;
    else delete this.data.defaults.template;
    this.changed();
  }

  /** Plantilla solo para esta imagen (`null` la quita y vuelve a la de todas). */
  setItemTemplate(entry: string, size: [number, number], tpl: TemplateRef | null) {
    if (!this.data) return;
    let it = this.item(entry);
    if (!tpl) {
      if (it) delete it.template;
      this.prune();
    } else if (it) {
      it.template = tpl;
      it.size = size;
    } else {
      this.data.items.push({ entry, size, template: tpl });
    }
    this.changed();
  }

  /** Entrada de la imagen, creándola si no existe. */
  private ensure(entry: string, size: [number, number]): Item {
    let it = this.item(entry);
    if (!it) {
      this.data!.items.push({ entry, size });
      it = this.data!.items.at(-1)!;
    }
    it.size = size;
    return it;
  }

  /**
   * Timeline propia de la imagen (editor). Sustituye a cualquier plantilla. Vacía o `null`, la
   * imagen vuelve a usar la plantilla de todas.
   */
  setTimeline(entry: string, size: [number, number], tl: Timeline | null) {
    if (!this.data) return;
    // El sonido y los efectos de la imagen no dependen de la cámara: se conservan aunque esta cambie.
    const audio = hasAudio(this.item(entry)?.timeline) ? this.item(entry)!.timeline!.audio : undefined;
    const fx = this.item(entry)?.timeline?.fx?.length ? this.item(entry)!.timeline!.fx : undefined;
    if (tl?.camera.length) {
      const it = this.ensure(entry, size);
      if (!hasAudio(tl)) {
        if (audio) tl.audio = audio;
        else delete tl.audio;
      }
      if (!tl.fx?.length) {
        if (fx) tl.fx = fx;
        else delete tl.fx;
      }
      it.timeline = tl;
      delete it.template;
    } else {
      const it = this.item(entry);
      if (it && (audio || fx)) it.timeline = { camera: [], ...(audio ? { audio } : {}), ...(fx ? { fx } : {}) };
      else if (it) delete it.timeline;
      this.prune();
    }
    this.changed();
  }

  /**
   * Transición de salida de una imagen (`entry`) o, con `null`, la de todas. Para una imagen,
   * `tr = null` le quita la suya y vuelve a usar la general.
   */
  setTransition(entry: string | null, tr: Transition | null, size?: [number, number]) {
    if (!this.data || this.readOnly) return;
    if (entry === null) {
      if (tr) this.data.defaults.transition = tr;
    } else if (tr) {
      const it = this.item(entry) ?? (size ? this.ensure(entry, size) : undefined);
      if (!it) return;
      it.transitionOut = tr;
    } else {
      const it = this.item(entry);
      if (it) delete it.transitionOut;
      this.prune();
    }
    this.changed();
  }

  /** Pausa al final de cada imagen, antes de su transición (s). */
  setHoldBetween(seconds: number) {
    if (!this.data || this.readOnly) return;
    this.data.defaults.holdBetween = Math.min(10, Math.max(0, seconds));
    this.changed();
  }

  /** Ambientes del proyecto. */
  get ambients(): AmbientClip[] {
    return this.data?.ambient ?? [];
  }

  addAmbient(a: Omit<AmbientClip, "id">): string | null {
    if (!this.data || this.readOnly) return null;
    const id = `amb-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
    // Por el proxy: `x ??= []` devuelve el array sin envolver y lo añadido no se vería.
    this.data.ambient ??= [];
    this.data.ambient.push({ id, ...a });
    this.changed();
    return id;
  }

  editAmbient(id: string, fn: (a: AmbientClip) => void) {
    const a = this.data?.ambient?.find((x) => x.id === id);
    if (!a || this.readOnly) return;
    fn(a);
    this.changed();
  }

  removeAmbient(id: string) {
    if (!this.data?.ambient || this.readOnly) return;
    this.data.ambient = this.data.ambient.filter((x) => x.id !== id);
    if (!this.data.ambient.length) delete this.data.ambient;
    this.changed();
  }

  /**
   * Borra un ambiente desde la imagen `index` (`entries`: nombres en orden). En la imagen donde
   * empieza se borra entero; en una de las siguientes solo deja de sonar en esa: el tramo se
   * acorta o se parte en dos (antes y después). Es un solo paso de deshacer.
   */
  removeAmbientAt(id: string, index: number, entries: string[]): "all" | "here" | null {
    const a = this.data?.ambient?.find((x) => x.id === id);
    if (!a || this.readOnly) return null;
    const from = entries.indexOf(a.from);
    const to = entries.indexOf(a.to) < 0 ? from : entries.indexOf(a.to);
    const [lo, hi] = [Math.min(from, to), Math.max(from, to)];
    if (from < 0 || index <= lo || index > hi) {
      this.removeAmbient(id);
      return "all";
    }
    const rest = index < hi ? { ...$state.snapshot(a), from: entries[index + 1], to: entries[hi] } : null;
    a.from = entries[lo];
    a.to = entries[index - 1];
    if (rest) {
      const copy: AmbientClip = { ...rest, id: `amb-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}` };
      this.data!.ambient!.splice(this.data!.ambient!.indexOf(a) + 1, 0, copy);
    }
    this.changed();
    return "here";
  }

  /** Mezcla del proyecto (la de fábrica si no se ha tocado). */
  get mix(): MixLevels {
    return this.data?.defaults.mix ?? DEFAULT_LEVELS;
  }

  setMix(fn: (m: MixLevels) => void) {
    if (!this.data || this.readOnly) return;
    this.data.defaults.mix ??= { ...DEFAULT_LEVELS };
    fn(this.data.defaults.mix);
    this.changed();
  }

  /** Pistas de sonido, en orden y con al menos una por grupo. */
  get tracks(): Track[] {
    return withDefaults(this.data?.tracks);
  }

  /** La lista de pistas del proyecto, ya escrita (a través del proxy). */
  private ownTracks(): Track[] | null {
    if (!this.data || this.readOnly) return null;
    if (!this.data.tracks?.length) this.data.tracks = withDefaults(undefined);
    return this.data.tracks;
  }

  /** Pista nueva al final de su grupo. Devuelve su id. */
  addTrack(bus: SoundBus): string | null {
    const list = this.ownTracks();
    if (!list) return null;
    const id = newTrackId();
    let last = -1;
    list.forEach((t, i) => t.bus === bus && (last = i));
    const next = [...list];
    next.splice(last + 1, 0, { id, bus, volume: 1, pan: 0 });
    this.data!.tracks = withDefaults(next);
    this.changed();
    return id;
  }

  editTrack(id: string, fn: (t: Track) => void) {
    const t = this.ownTracks()?.find((x) => x.id === id);
    if (!t) return;
    fn(t);
    this.changed();
  }

  /** Sube (−1) o baja (1) una pista dentro de su grupo. */
  moveTrack(id: string, dir: -1 | 1) {
    const list = this.ownTracks();
    if (!list) return;
    const i = list.findIndex((t) => t.id === id);
    if (i < 0) return;
    let j = i + dir;
    while (j >= 0 && j < list.length && list[j].bus !== list[i].bus) j += dir;
    if (j < 0 || j >= list.length) return;
    const next = [...list];
    [next[i], next[j]] = [next[j], next[i]];
    this.data!.tracks = next;
    this.changed();
  }

  /** Clips (y en cuántas imágenes) que suenan en una pista. */
  usage(id: string): { clips: number; images: number } {
    const tracks = this.tracks;
    const track = tracks.find((t) => t.id === id);
    if (!track || !this.data) return { clips: 0, images: 0 };
    const mine = (c: { track?: string }, bus: SoundBus) => resolveTrack(tracks, c.track, bus).id === id;
    let clips = 0;
    let images = 0;
    for (const it of this.data.items) {
      const a = audioOf(it.timeline);
      const n = a.voice.filter((c) => mine(c, "voice")).length + a.sfx.filter((c) => mine(c, "sfx")).length;
      clips += n;
      if (n) images++;
    }
    for (const a of this.data.ambient ?? []) if (mine(a, "ambient")) clips++;
    return { clips, images };
  }

  /** Borra una pista y todo lo que suena en ella. Cada grupo conserva al menos una. */
  removeTrack(id: string): boolean {
    if (!this.data || this.readOnly) return false;
    const tracks = this.tracks;
    const track = tracks.find((t) => t.id === id);
    if (!track || tracks.filter((t) => t.bus === track.bus).length < 2) return false;
    const mine = (c: { track?: string }, bus: SoundBus) => resolveTrack(tracks, c.track, bus).id === id;
    // Los clips sin pista van a la primera de su grupo: si se borra otra, siguen donde estaban.
    for (const it of this.data.items) {
      const a = it.timeline?.audio;
      if (!a) continue;
      for (const bus of ["voice", "sfx"] as const) {
        const list = a[bus];
        if (list) a[bus] = list.filter((c) => !mine(c, bus));
      }
      if (!hasAudio(it.timeline)) {
        delete it.timeline!.audio;
        if (!it.timeline!.camera.length && !it.timeline!.fx?.length) delete it.timeline;
      }
    }
    if (this.data.ambient) {
      this.data.ambient = this.data.ambient.filter((a) => !mine(a, "ambient"));
      if (!this.data.ambient.length) delete this.data.ambient;
    }
    this.data.tracks = tracks.filter((t) => t.id !== id);
    this.prune();
    this.changed();
    return true;
  }

  /** Voz y efectos de la imagen. */
  audioFor(entry: string): AudioTracks {
    return audioOf(this.item(entry)?.timeline);
  }

  /** Cambia el sonido de la imagen sin tocar su cámara ni su plantilla. */
  editAudio(entry: string, size: [number, number], fn: (a: AudioTracks) => void) {
    if (!this.data || this.readOnly) return;
    const it = this.ensure(entry, size);
    // Siempre a través del proxy (ver `addAmbient`).
    it.timeline ??= { camera: [] };
    const tl = it.timeline;
    tl.audio ??= {};
    const a = tl.audio;
    a.voice ??= [];
    a.sfx ??= [];
    fn(a as AudioTracks);
    if (!hasAudio(tl)) {
      delete tl.audio;
      if (!tl.camera.length && !tl.fx?.length) delete it.timeline;
      this.prune();
    }
    this.changed();
  }

  /** Efectos especiales de la imagen. */
  fxFor(entry: string): Fx[] {
    return this.item(entry)?.timeline?.fx ?? [];
  }

  /** Cambia los efectos de la imagen sin tocar su cámara, su plantilla ni su sonido. */
  editFx(entry: string, size: [number, number], fn: (list: Fx[]) => void) {
    if (!this.data || this.readOnly) return;
    const it = this.ensure(entry, size);
    it.timeline ??= { camera: [] };
    const tl = it.timeline;
    tl.fx ??= [];
    fn(tl.fx);
    if (!tl.fx.length) {
      delete tl.fx;
      if (!tl.camera.length && !hasAudio(tl)) delete it.timeline;
      this.prune();
    }
    this.changed();
  }

  /** Ajustes de imagen que se le aplican: los suyos o, si no tiene, los de todas. */
  adjustFor(entry: string): Adjust | null {
    return normalize(this.item(entry)?.adjust) ?? normalize(this.data?.defaults.adjust);
  }

  /** La imagen tiene ajustes propios (distintos de los de todas). */
  hasOwnAdjust(entry: string): boolean {
    return !!normalize(this.item(entry)?.adjust);
  }

  /** Ajustes de una imagen o, con `entry = null`, de todas. `null` o neutro los quita. */
  setAdjust(entry: string | null, adj: Adjust | null, size?: [number, number]) {
    if (!this.data || this.readOnly) return;
    const n = normalize(adj);
    if (entry === null) {
      if (n) this.data.defaults.adjust = n;
      else delete this.data.defaults.adjust;
    } else if (n && size) {
      this.ensure(entry, size).adjust = n;
    } else {
      const it = this.item(entry);
      if (it) delete it.adjust;
      this.prune();
    }
    this.changed();
  }

  /** Viñetas dibujadas a mano; `null` vuelve a la detección automática. */
  setPanels(entry: string, size: [number, number], panels: Rect[] | null) {
    if (!this.data) return;
    if (panels) this.ensure(entry, size).panels = panels;
    else {
      const it = this.item(entry);
      if (it) delete it.panels;
      this.prune();
    }
    this.changed();
  }

  /** Quita las entradas que ya no aportan nada. */
  private prune() {
    if (!this.data) return;
    this.data.items = this.data.items.filter((i) => i.template || i.transitionOut || i.timeline?.camera?.length || hasAudio(i.timeline) || i.timeline?.fx?.length || i.panels || i.adjust);
  }

  changed() {
    if (this.readOnly || !this.data) return;
    if (!this.stepPending) this.unsaved++;
    this.stepPending = true;
    clearTimeout(this.stepTimer);
    if (!this.batching) this.stepTimer = window.setTimeout(() => this.commit(), STEP_DELAY);
    this.canUndo = true;
    this.save();
  }

  private save() {
    if (this.readOnly || !this.data) return;
    this.pending = true;
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.flush(), SAVE_DELAY);
  }

  /** Guarda ya lo pendiente. Devuelve si todo quedó guardado. */
  async flush(): Promise<boolean> {
    clearTimeout(this.timer);
    if (!this.data || this.readOnly) return true;
    // Tras un fallo se reintenta aunque no haya cambios nuevos.
    if (!this.pending && !this.unsaved) return true;
    this.pending = false;
    const n = this.unsaved;
    try {
      this.path = await invoke<string>("save_project", {
        generation: this.generation,
        json: JSON.stringify($state.snapshot(this.data)),
      });
      this.saveError = null;
      this.unsaved = Math.max(0, this.unsaved - n); // lo cambiado mientras se guardaba sigue pendiente
      return !this.pending;
    } catch (e) {
      this.saveError = String(e);
      log.warn("no se pudo guardar el proyecto:", e);
      return false;
    }
  }

  /** Renuncia a guardar lo pendiente (el usuario lo ha decidido tras un fallo). */
  discard() {
    clearTimeout(this.timer);
    this.pending = false;
    this.unsaved = 0;
  }

  /** Olvida la fuente cerrada (llamar después de `flush`). */
  unload() {
    clearTimeout(this.timer);
    clearTimeout(this.stepTimer);
    this.data = null;
    this.path = null;
    this.readOnly = null;
    this.saveError = null;
    this.pending = false;
    this.unsaved = 0;
    this.stepPending = false;
    this.batching = 0;
    this.undoStack = [];
    this.redoStack = [];
    this.base = null;
    this.syncFlags();
  }
}
