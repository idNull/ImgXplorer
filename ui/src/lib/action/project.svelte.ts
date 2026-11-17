// Proyecto `.imgx` de la fuente abierta: lo que el usuario configura en el Modo Acción.
// Se guarda solo (con un pequeño retraso) en cuanto cambia algo; Rust lo valida antes de escribir.
// Deshacer/rehacer guarda instantáneas del proyecto entero (es pequeño): cada pausa en la
// edición, o cada gesto completo (`batch`), es un paso.
import { invoke } from "@tauri-apps/api/core";
import type { TemplateRef } from "./catalog";
import type { Timeline } from "../editor/model";
import { log } from "../log";

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
}

export interface ProjectData {
  format: "imgx";
  version: number;
  source: { kind: "folder" | "archive" | "image"; name: string; fingerprint?: string };
  defaults: { transition: Transition; holdBetween: number; template?: TemplateRef };
  clips: unknown[];
  items: Item[];
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
    if (tl?.camera.length) {
      const it = this.ensure(entry, size);
      it.timeline = tl;
      delete it.template;
    } else {
      const it = this.item(entry);
      if (it) delete it.timeline;
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
    this.data.items = this.data.items.filter((i) => i.template || i.transitionOut || i.timeline?.camera?.length || i.panels);
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
