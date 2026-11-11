// Proyecto `.imgx` de la fuente abierta: lo que el usuario configura en el Modo Acción.
// Se guarda solo (con un pequeño retraso) en cuanto cambia algo; Rust lo valida antes de escribir.
import { invoke } from "@tauri-apps/api/core";
import type { TemplateRef } from "./catalog";
import { log } from "../log";

export type TransitionType = "cut" | "fade" | "slide" | "continuous-camera";
export type Dir = "left" | "right" | "up" | "down";

export interface Transition {
  type: TransitionType;
  dir?: Dir;
  duration: number;
}

export interface Item {
  entry: string;
  size: [number, number];
  template?: TemplateRef;
  timeline?: { camera: unknown[]; audio?: unknown };
  transitionOut?: Transition;
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

export class ProjectStore {
  data = $state<ProjectData | null>(null);
  /** Ruta del archivo guardado o cargado. */
  path = $state<string | null>(null);
  /** El archivo existente no se pudo leer: no se guarda para no destruirlo. */
  readOnly = $state<string | null>(null);
  saveError = $state<string | null>(null);
  private generation = 0;
  private timer: number | undefined;
  private pending = false;

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

  /** Quita las entradas que ya no aportan nada. */
  private prune() {
    if (!this.data) return;
    this.data.items = this.data.items.filter((i) => i.template || i.transitionOut || i.timeline?.camera?.length);
  }

  changed() {
    if (this.readOnly || !this.data) return;
    this.pending = true;
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.flush(), SAVE_DELAY);
  }

  /** Guarda ya lo pendiente. */
  async flush() {
    clearTimeout(this.timer);
    if (!this.pending || !this.data || this.readOnly) return;
    this.pending = false;
    try {
      this.path = await invoke<string>("save_project", {
        generation: this.generation,
        json: JSON.stringify($state.snapshot(this.data)),
      });
      this.saveError = null;
    } catch (e) {
      this.saveError = String(e);
      log.warn("no se pudo guardar el proyecto:", e);
    }
  }
}
