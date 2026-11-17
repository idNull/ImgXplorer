// Biblioteca de sonidos: los importados y grabados (copias en el directorio de datos de la app) y
// los efectos de serie, que fabrica la propia app.
import { invoke } from "@tauri-apps/api/core";
import { label } from "../i18n.svelte";
import { log } from "../log";

export type SoundKind = "imported" | "recorded" | "builtin";

export interface SoundInfo {
  id: string;
  /** Vacío en los de serie (se traduce con `snd.<id>`). */
  name: string;
  kind: SoundKind;
  seconds: number;
  /** Pensado para repetirse en bucle (ambientes de serie). */
  ambient: boolean;
}

/** Extensiones de audio que se pueden importar. */
export const AUDIO_EXTENSIONS = ["mp3", "wav", "wave", "flac", "ogg", "oga", "m4a", "aac"];

export const isAudioPath = (p: string) => AUDIO_EXTENSIONS.includes(p.split(".").pop()?.toLowerCase() ?? "");

class SoundStore {
  list = $state<SoundInfo[]>([]);
  loaded = $state(false);
  /** Sonido que se está escuchando en la biblioteca. */
  previewing = $state<string | null>(null);
  #previewTimer = 0;
  #peaks = new Map<string, Promise<Float32Array>>();

  async load() {
    try {
      this.list = await invoke<SoundInfo[]>("sound_list");
      this.loaded = true;
    } catch (e) {
      log.warn("sonidos:", e);
    }
  }

  get(id: string): SoundInfo | undefined {
    return this.list.find((s) => s.id === id);
  }

  /** Nombre para mostrar (los de serie, traducido). */
  name(id: string): string {
    const s = this.get(id);
    if (!s) return "?";
    return s.kind === "builtin" ? label(`snd.${id.replace(/^builtin:/, "")}`) : s.name;
  }

  async import(paths: string[]): Promise<{ added: SoundInfo[]; errors: string[] }> {
    const r = await invoke<{ added: SoundInfo[]; errors: string[] }>("sound_import", { paths });
    for (const s of r.added) if (!this.get(s.id)) this.list.push(s);
    return r;
  }

  /** Añade una grabación recién guardada. */
  add(s: SoundInfo) {
    if (!this.get(s.id)) this.list.push(s);
  }

  async rename(id: string, name: string) {
    const s = this.get(id);
    const clean = name.trim();
    if (!s || s.kind === "builtin" || !clean || clean === s.name) return;
    await invoke("sound_rename", { id, name: clean });
    s.name = clean.slice(0, 120);
  }

  async remove(id: string) {
    await invoke("sound_delete", { id });
    this.list = this.list.filter((s) => s.id !== id);
    this.#peaks.delete(id);
  }

  /** Forma de onda: `buckets` picos (0..1). Se pide una vez por sonido y tamaño. */
  peaks(id: string, buckets = 400): Promise<Float32Array> {
    const key = `${id}@${buckets}`;
    let p = this.#peaks.get(key);
    if (!p) {
      p = invoke<number[]>("sound_peaks", { id, buckets }).then((v) => new Float32Array(v));
      p.catch(() => this.#peaks.delete(key));
      this.#peaks.set(key, p);
    }
    return p;
  }

  /** Escucha un sonido (o lo para si ya sonaba). */
  async preview(id: string | null) {
    clearTimeout(this.#previewTimer);
    const stop = id === null || this.previewing === id;
    this.previewing = stop ? null : id;
    try {
      await invoke("sound_preview", { id: stop ? null : id });
    } catch (e) {
      log.warn("escuchar sonido:", e);
      this.previewing = null;
      return;
    }
    const s = id && this.get(id);
    if (!stop && s) this.#previewTimer = window.setTimeout(() => (this.previewing = null), s.seconds * 1000 + 100);
  }
}

export const sounds = new SoundStore();
