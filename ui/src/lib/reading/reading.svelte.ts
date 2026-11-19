// Modo de lectura (📖) y marcadores de cada cómic. Se recuerdan por ruta en `reading.json`, aparte
// de los recientes (que solo guardan 20): así un cómic conserva sus marcadores aunque salga de la
// lista. Nada de esto toca el cómic ni su proyecto `.imgx`.
import { invoke } from "@tauri-apps/api/core";
import { log } from "../log";

/** A qué se ajusta la página: la ventana entera (como siempre), el ancho o el alto. */
export type FitMode = "window" | "width" | "height";
export const FITS: FitMode[] = ["window", "width", "height"];

export interface Reading {
  fit: FitMode;
  /** ✂ Ajustar al contenido: el encuadre ignora los márgenes en blanco. */
  crop: boolean;
  /** Doble página. */
  double: boolean;
  /** En doble página, la primera (la portada) va sola. */
  cover: boolean;
  /** Tira vertical continua (webtoon). */
  strip: boolean;
  /** Sentido manga: de derecha a izquierda. */
  rtl: boolean;
  /** Páginas marcadas (por nombre). */
  marks: string[];
  /** Ya se preguntó si leerlo como webtoon. */
  asked: boolean;
}

export const DEFAULT_READING: Reading = { fit: "window", crop: false, double: false, cover: true, strip: false, rtl: false, marks: [], asked: false };

/** Cómics que se recuerdan (los más antiguos se olvidan). */
const MAX = 400;

type Stored = Reading & { at: number };

function clean(raw: unknown): Record<string, Stored> {
  const out: Record<string, Stored> = {};
  if (!raw || typeof raw !== "object") return out;
  for (const [path, v] of Object.entries(raw as Record<string, unknown>)) {
    if (!v || typeof v !== "object") continue;
    const r = v as Record<string, unknown>;
    out[path] = {
      fit: FITS.includes(r.fit as FitMode) ? (r.fit as FitMode) : "window",
      crop: r.crop === true,
      double: r.double === true,
      cover: r.cover !== false,
      strip: r.strip === true,
      rtl: r.rtl === true,
      marks: Array.isArray(r.marks) ? r.marks.filter((m): m is string => typeof m === "string").slice(0, 1000) : [],
      asked: r.asked === true,
      at: typeof r.at === "number" ? r.at : 0,
    };
  }
  return out;
}

class ReadingPrefs {
  #all = $state<Record<string, Stored>>({});
  #timer: number | undefined;

  load(text: string | null) {
    try {
      this.#all = clean(text ? JSON.parse(text) : null);
    } catch (e) {
      log.warn("reading.json no es válido, se empieza de cero:", e);
      this.#all = {};
    }
  }

  /** El modo de lectura de un cómic (el de siempre si no tiene). */
  of(path: string | null | undefined): Reading {
    return (path && this.#all[path]) || DEFAULT_READING;
  }

  /** ¿Se ha guardado algo de este cómic? */
  has(path: string): boolean {
    return path in this.#all;
  }

  set(path: string, patch: Partial<Reading>) {
    const prev = this.#all[path] ?? { ...DEFAULT_READING, marks: [] };
    this.#all[path] = { ...prev, ...patch, at: Date.now() };
    const paths = Object.keys(this.#all);
    if (paths.length > MAX) {
      paths.sort((a, b) => this.#all[a].at - this.#all[b].at);
      for (const p of paths.slice(0, paths.length - MAX)) delete this.#all[p];
    }
    this.#save();
  }

  /** Marca o desmarca una página. Devuelve si queda marcada. */
  toggleMark(path: string, entry: string): boolean {
    const marks = this.of(path).marks;
    const on = !marks.includes(entry);
    this.set(path, { marks: on ? [...marks, entry] : marks.filter((m) => m !== entry) });
    return on;
  }

  #save() {
    clearTimeout(this.#timer);
    this.#timer = window.setTimeout(() => this.flush(), 400);
  }

  flush() {
    if (this.#timer === undefined) return;
    clearTimeout(this.#timer);
    this.#timer = undefined;
    const json = JSON.stringify($state.snapshot(this.#all));
    invoke("save_prefs", { name: "reading", json }).catch((e) => log.warn("no se pudo guardar reading.json:", e));
  }
}

export const reading = new ReadingPrefs();
