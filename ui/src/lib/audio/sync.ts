// Lógica pura del sonido (sin Tauri), para poder probarla: qué clips manda cada imagen y cuándo
// hay que volver a sincronizar el reloj del motor de audio.
import type { AudioTracks } from "../editor/model";
import type { Effect } from "./effects";

/** Un clip tal como lo entiende el motor de audio (Rust). */
export interface ClipSpec {
  sound: string;
  bus: "voice" | "sfx" | "ambient";
  start: number;
  trimIn: number;
  duration: number;
  volume: number;
  fadeIn: number;
  fadeOut: number;
  looped?: boolean;
  /** Pista (si falta, la primera de su grupo). */
  track?: string;
  effects?: Effect[];
}

/** Ambiente activo: `id` estable para que siga sonando sin cortes al cambiar de imagen. */
export interface AmbientSpec extends ClipSpec {
  id: string;
}

/** Voz y efectos de una imagen para el motor. */
export function sceneSpecs(a: AudioTracks): ClipSpec[] {
  const out: ClipSpec[] = [];
  for (const bus of ["voice", "sfx"] as const) {
    for (const c of a[bus]) {
      out.push({
        sound: c.sound, bus, start: c.start, trimIn: c.trimIn, duration: c.duration, volume: c.volume, fadeIn: c.fadeIn, fadeOut: c.fadeOut,
        ...(c.track ? { track: c.track } : {}),
        ...(c.effects?.length ? { effects: c.effects } : {}),
      });
    }
  }
  return out;
}

export interface TransportState {
  /** Tiempo de la imagen (s). */
  t: number;
  /** Avanzan la voz y los efectos. */
  playing: boolean;
  /** Suena el ambiente (aunque la imagen esté congelada). */
  running: boolean;
  rate: number;
}

/** Diferencia de tiempo a partir de la cual se salta en vez de tolerarla. */
export const SEEK_THRESHOLD = 0.05;
/** Cada cuánto se recuerda el reloj aunque nada cambie (ms). */
export const RESYNC_MS = 500;

/**
 * Decide cuándo mandar el transporte al motor: al empezar, cuando cambia el estado o la
 * velocidad, cuando el tiempo salta (búsqueda, otra marca) y cada `RESYNC_MS` para corregir
 * la deriva entre relojes. Así no se manda una orden en cada fotograma.
 */
export class TransportSync {
  private last: { s: TransportState; at: number } | null = null;

  reset() {
    this.last = null;
  }

  /** Tiempo que debería llevar el motor ahora según lo último enviado. */
  predicted(now: number): number | null {
    const l = this.last;
    if (!l) return null;
    return l.s.t + (l.s.playing ? ((now - l.at) / 1000) * l.s.rate : 0);
  }

  check(s: TransportState, now: number, force = false): { send: boolean; seek: boolean } {
    const l = this.last;
    let res: { send: boolean; seek: boolean };
    if (!l || force) res = { send: true, seek: true };
    else {
      const jump = Math.abs(s.t - (this.predicted(now) ?? s.t)) > SEEK_THRESHOLD;
      const changed = s.playing !== l.s.playing || s.running !== l.s.running || s.rate !== l.s.rate;
      res = { send: jump || changed || now - l.at >= RESYNC_MS, seek: jump };
    }
    if (res.send) this.last = { s: { ...s }, at: now };
    return res;
  }
}

/** Lo mínimo de un ambiente para saber dónde suena (evita depender del almacén del proyecto). */
export interface AmbientLike {
  id: string;
  sound: string;
  from: string;
  to: string;
  volume: number;
  fadeIn: number;
  fadeOut: number;
  track?: string;
  effects?: Effect[];
}

/** Imágenes (índices, ambos incluidos) en que suena un ambiente; null si su imagen ya no existe. */
export function ambientRange(a: AmbientLike, indexOf: (entry: string) => number): [number, number] | null {
  const from = indexOf(a.from);
  if (from < 0) return null;
  const to = indexOf(a.to);
  const end = to < 0 ? from : to;
  return [Math.min(from, end), Math.max(from, end)];
}

/** Ambientes que suenan en la imagen `i`. */
export function ambientSpecs(list: AmbientLike[], i: number, indexOf: (entry: string) => number): AmbientSpec[] {
  const out: AmbientSpec[] = [];
  for (const a of list) {
    const r = ambientRange(a, indexOf);
    if (!r || i < r[0] || i > r[1]) continue;
    out.push({
      id: a.id, sound: a.sound, bus: "ambient", start: 0, trimIn: 0, duration: 0, volume: a.volume, fadeIn: a.fadeIn, fadeOut: a.fadeOut, looped: true,
      ...(a.track ? { track: a.track } : {}),
      ...(a.effects?.length ? { effects: a.effects } : {}),
    });
  }
  return out;
}

/**
 * Calendario de sonido del vídeo: la voz y los efectos de cada imagen en su instante, y cada
 * ambiente desde el principio de su primera imagen hasta el final de la última.
 */
export class SoundSchedule {
  clips: ClipSpec[] = [];
  private open = new Map<string, { spec: AmbientSpec; start: number }>();
  private lastEnd = 0;

  /** Empieza la imagen en el segundo `t` del vídeo. */
  image(t: number, audio: AudioTracks | undefined, ambient: AmbientSpec[]) {
    for (const c of audio ? sceneSpecs(audio) : []) this.clips.push({ ...c, start: t + c.start });
    const here = new Set(ambient.map((a) => a.id));
    for (const [id, o] of this.open) if (!here.has(id)) this.close(id, o);
    for (const a of ambient) if (!this.open.has(a.id)) this.open.set(a.id, { spec: a, start: t });
  }

  /** Acaba la imagen (antes de su transición) en el segundo `t`. */
  imageEnd(t: number) {
    this.lastEnd = t;
  }

  private close(id: string, o: { spec: AmbientSpec; start: number }) {
    this.open.delete(id);
    const duration = this.lastEnd - o.start;
    if (duration > 0.01) this.clips.push({ ...o.spec, bus: "ambient", start: o.start, trimIn: 0, duration, looped: true });
  }

  finish(): ClipSpec[] {
    for (const [id, o] of this.open) this.close(id, o);
    return this.clips;
  }
}
