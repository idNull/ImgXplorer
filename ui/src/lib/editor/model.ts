// Timeline de cámara tal como se guarda en el `.imgx` (ver crates/imgx-core/src/schema.rs) y
// operaciones de edición puras sobre ella (mutan la timeline que reciben). La evaluación (tiempo → pose) la hace siempre
// imgx-core vía WASM; aquí solo se mueven, estiran y crean clips y claves.

import type { Effect } from "../audio/effects";
import type { Fx } from "../fx/fx";

/** Copia profunda que acepta proxies de `$state` (structuredClone no puede con ellos). */
const $snapshot = <T>(v: T): T => JSON.parse(JSON.stringify(v));

export interface PoseObj {
  cx: number;
  cy: number;
  zoom: number;
  rot: number;
}

export type EasingType = "linear" | "ease-in" | "ease-out" | "ease-in-out" | "bounce" | "elastic" | "step" | "cubic-bezier";
export type Easing = { type: Exclude<EasingType, "cubic-bezier"> } | { type: "cubic-bezier"; x1: number; y1: number; x2: number; y2: number };
export const EASINGS: EasingType[] = ["ease-in-out", "linear", "ease-in", "ease-out", "bounce", "elastic", "step", "cubic-bezier"];

/**
 * Progreso con easing (0..1, el elástico se pasa de 1). Copia de `imgx-core::easing` solo para
 * dibujar las curvas en el editor: la reproducción la evalúa siempre el núcleo.
 */
export function easeAt(e: Easing, t: number): number {
  t = Math.min(1, Math.max(0, t));
  switch (e.type) {
    case "linear": return t;
    case "ease-in": return t ** 3;
    case "ease-out": return 1 - (1 - t) ** 3;
    case "ease-in-out": return t < 0.5 ? 4 * t ** 3 : 1 - (-2 * t + 2) ** 3 / 2;
    case "bounce": {
      const n = 7.5625, d = 2.75;
      if (t < 1 / d) return n * t * t;
      if (t < 2 / d) return n * (t -= 1.5 / d) * t + 0.75;
      if (t < 2.5 / d) return n * (t -= 2.25 / d) * t + 0.9375;
      return n * (t -= 2.625 / d) * t + 0.984375;
    }
    case "elastic":
      return t === 0 || t === 1 ? t : 2 ** (-10 * t) * Math.sin((t * 10 - 0.75) * ((2 * Math.PI) / 3)) + 1;
    case "step": return t >= 1 ? 1 : 0;
    case "cubic-bezier": {
      const bez = (p1: number, p2: number, s: number) => 3 * (1 - s) ** 2 * s * p1 + 3 * (1 - s) * s * s * p2 + s ** 3;
      let lo = 0, hi = 1;
      for (let i = 0; i < 30; i++) {
        const mid = (lo + hi) / 2;
        if (bez(e.x1, e.x2, mid) < t) lo = mid;
        else hi = mid;
      }
      return bez(e.y1, e.y2, (lo + hi) / 2);
    }
  }
}

/** Puntos de una polilínea con la curva en un rectángulo `w`×`h` (y hacia abajo). */
export function easePoints(e: Easing, x0: number, w: number, h: number, base: number, n = 32): string {
  const pts: string[] = [];
  for (let i = 0; i <= n; i++) {
    const u = i / n;
    pts.push(`${(x0 + u * w).toFixed(1)},${(base - easeAt(e, u) * h).toFixed(1)}`);
  }
  return pts.join(" ");
}

export interface Keyframe {
  t: number;
  pose: PoseObj;
  easing: Easing;
}
export interface Sample {
  t: number;
  pose: PoseObj;
}

export type Clip =
  | { type: "keyframes"; keys: Keyframe[] }
  | { type: "recorded"; samples: Sample[] }
  | { type: "hold"; length: number; pose?: PoseObj };

export interface ClipInstance {
  name?: string;
  start: number;
  duration: number;
  clip: { inline: Clip } | { ref: string };
  trimIn: number;
  trimOut: number;
}

export interface Timeline {
  camera: ClipInstance[];
  audio?: { voice?: AudioClip[]; sfx?: AudioClip[]; ambient?: unknown[] };
  /** Efectos especiales visuales. */
  fx?: Fx[];
}

/** Pistas de sonido de una imagen. */
export type AudioBus = "voice" | "sfx";

/** Un sonido colocado en la timeline de la imagen. */
export interface AudioClip {
  /** Id del sonido en la biblioteca (`builtin:<id>` para los de serie). */
  sound: string;
  name?: string;
  start: number;
  /** Segundos que se saltan del principio del sonido. */
  trimIn: number;
  duration: number;
  /** 1 = volumen original. */
  volume: number;
  fadeIn: number;
  fadeOut: number;
  /** Pista del proyecto (si falta, la primera de su grupo). */
  track?: string;
  /** Efectos del propio clip. */
  effects?: Effect[];
}

export interface AudioTracks {
  voice: AudioClip[];
  sfx: AudioClip[];
}

export const AUDIO_BUSES: AudioBus[] = ["voice", "sfx"];

/** Las pistas de sonido de una timeline (vacías si no tiene). */
export function audioOf(tl: Timeline | null | undefined): AudioTracks {
  return { voice: tl?.audio?.voice ?? [], sfx: tl?.audio?.sfx ?? [] };
}

export const hasAudio = (tl: Timeline | null | undefined) => !!(tl?.audio?.voice?.length || tl?.audio?.sfx?.length);

export const audioClipEnd = (c: AudioClip) => c.start + c.duration;

/** Final del último sonido de la imagen (0 si no hay): la imagen dura al menos esto. */
export function audioEnd(a: AudioTracks): number {
  return [...a.voice, ...a.sfx].reduce((m, c) => Math.max(m, audioClipEnd(c)), 0);
}

/** Clip nuevo con el sonido entero. */
export function newAudioClip(sound: { id: string; seconds: number }, start: number): AudioClip {
  return { sound: sound.id, start: Math.max(0, start), trimIn: 0, duration: Math.max(MIN_DURATION, sound.seconds), volume: 1, fadeIn: 0, fadeOut: 0 };
}

/**
 * Arrastra el borde de un clip de sonido. El inicio recorta el principio del sonido (el final no
 * se mueve); el final lo acorta o alarga, sin pasar del final del sonido.
 */
export function dragAudioEdge(c: AudioClip, edge: "start" | "end", t: number, soundSeconds: number) {
  const len = Number.isFinite(soundSeconds) && soundSeconds > 0 ? soundSeconds : Infinity;
  if (edge === "start") {
    const endT = audioClipEnd(c);
    // Ni antes del principio del sonido ni más allá de dejar un clip mínimo.
    const s = Math.min(endT - MIN_DURATION, Math.max(t, c.start - c.trimIn, 0));
    c.trimIn = Math.max(0, c.trimIn + (s - c.start));
    c.duration = endT - s;
    c.start = s;
  } else {
    c.duration = Math.max(MIN_DURATION, Math.min(t - c.start, len - c.trimIn));
  }
  c.fadeIn = Math.min(c.fadeIn, c.duration);
  c.fadeOut = Math.min(c.fadeOut, c.duration - c.fadeIn);
}

/** Duración mínima de un clip en la timeline (s). */
export const MIN_DURATION = 0.1;

export const poseObj = (p: ArrayLike<number>): PoseObj => ({ cx: p[0], cy: p[1], zoom: p[2], rot: p[3] ?? 0 });
export const poseArr = (p: PoseObj) => new Float64Array([p.cx, p.cy, p.zoom, p.rot]);

export function inline(inst: ClipInstance): Clip | null {
  return "inline" in inst.clip ? inst.clip.inline : null;
}

export function clipLength(c: Clip): number {
  switch (c.type) {
    case "keyframes":
      return c.keys.at(-1)?.t ?? 0;
    case "recorded":
      return c.samples.at(-1)?.t ?? 0;
    case "hold":
      return c.length;
  }
}

export const end = (i: ClipInstance) => i.start + i.duration;

export function timelineDuration(tl: Timeline): number {
  return tl.camera.reduce((m, i) => Math.max(m, end(i)), 0);
}

/** Parte del clip que se usa (duración natural menos recortes). */
function span(i: ClipInstance): number {
  const c = inline(i);
  return c ? Math.max(0, clipLength(c) - i.trimIn - i.trimOut) : i.duration;
}

/** Segundos de clip por segundo de timeline (1 = velocidad original). */
export function speedOf(i: ClipInstance): number {
  const s = span(i);
  return i.duration > 0 && s > 0 ? s / i.duration : 1;
}

/** Tiempo local del clip → tiempo absoluto. */
export function toAbs(i: ClipInstance, local: number): number {
  return i.start + (local - i.trimIn) / speedOf(i);
}

/** Tiempo absoluto → tiempo local del clip (sin limitar). */
export function toLocal(i: ClipInstance, t: number): number {
  return i.trimIn + (t - i.start) * speedOf(i);
}

/** Tiempos absolutos de las claves de un clip de keyframes (vacío en otros tipos). */
export function keyTimes(i: ClipInstance): number[] {
  const c = inline(i);
  if (c?.type !== "keyframes") return [];
  if (span(i) <= 0) return c.keys.map(() => i.start);
  return c.keys.map((k) => toAbs(i, k.t));
}

/** Tras cambiar claves o muestras, ajusta la duración para que la velocidad no cambie. */
function keepSpeed(i: ClipInstance, speed: number) {
  const s = span(i);
  i.duration = s > 0 ? Math.max(MIN_DURATION, s / speed) : 0;
}

export function newInstance(clip: Clip, start: number, name?: string, duration?: number): ClipInstance {
  const len = clipLength(clip);
  return { ...(name ? { name } : {}), start: Math.max(0, start), duration: duration ?? len, clip: { inline: clip }, trimIn: 0, trimOut: 0 };
}

/** Clip de keyframes activo en `t` (el que empieza más tarde, como en el evaluador). */
function keyframesAt(tl: Timeline, t: number, eps: number): number {
  let best = -1;
  tl.camera.forEach((i, idx) => {
    if (inline(i)?.type !== "keyframes") return;
    if (t >= i.start - eps && t <= end(i) + eps && (best < 0 || i.start >= tl.camera[best].start)) best = idx;
  });
  return best;
}

export interface KeyRef {
  clip: number;
  key: number;
}

/**
 * Añade (o sustituye, si ya hay una muy cerca) una clave con `pose` en el instante `t`.
 * - Dentro de un clip de keyframes: se inserta en él.
 * - Justo después del último clip de keyframes: lo alarga.
 * - Si no: crea un clip nuevo que empieza en `t`.
 */
export function addKeyframe(tl: Timeline, t: number, pose: PoseObj, easing: Easing = { type: "ease-in-out" }, eps = 0.04): KeyRef {
  let idx = keyframesAt(tl, t, eps);
  if (idx < 0) {
    // ¿Alargar el clip de keyframes que acaba más tarde antes de t, si nada lo tapa?
    let last = -1;
    tl.camera.forEach((i, k) => {
      if (end(i) <= t + eps && (last < 0 || end(i) > end(tl.camera[last]))) last = k;
    });
    const covered = tl.camera.some((i) => i.start > (last >= 0 ? tl.camera[last].start : -1) && i.start <= t);
    if (last >= 0 && !covered && inline(tl.camera[last])?.type === "keyframes" && tl.camera[last].trimOut === 0) idx = last;
  }
  if (idx < 0) {
    tl.camera.push(newInstance({ type: "keyframes", keys: [{ t: 0, pose, easing }] }, t));
    return { clip: tl.camera.length - 1, key: 0 };
  }
  const inst = tl.camera[idx];
  const clip = inline(inst) as Extract<Clip, { type: "keyframes" }>;
  // Con una sola clave el clip dura 0 y `speedOf` da 1: el tiempo local es el absoluto.
  const speed = speedOf(inst);
  let local = toLocal(inst, t);
  const near = clip.keys.findIndex((k) => Math.abs(k.t - local) / speed <= eps);
  if (near >= 0) {
    clip.keys[near].pose = pose;
    return { clip: idx, key: near };
  }
  if (local < 0) {
    // Antes del principio del clip: se corre todo para que el clip empiece en t.
    for (const k of clip.keys) k.t -= local;
    inst.start += local / speed;
    local = 0;
  }
  const at = clip.keys.findIndex((k) => k.t > local);
  const pos = at < 0 ? clip.keys.length : at;
  // La clave nueva hereda el easing del tramo en el que cae.
  const inherited = pos > 0 ? clip.keys[pos - 1].easing : easing;
  clip.keys.splice(pos, 0, { t: local, pose, easing: $snapshot(inherited) });
  keepSpeed(inst, speed);
  return { clip: idx, key: pos };
}

/** Mueve una clave al instante absoluto `t`, sin pasar de sus vecinas. */
export function moveKey(tl: Timeline, ref: KeyRef, t: number) {
  const inst = tl.camera[ref.clip];
  const clip = inline(inst);
  if (clip?.type !== "keyframes") return;
  const speed = speedOf(inst);
  const keys = clip.keys;
  const gap = 0.02 * speed;
  const lo = ref.key > 0 ? keys[ref.key - 1].t + gap : -Infinity;
  const hi = ref.key < keys.length - 1 ? keys[ref.key + 1].t - gap : Infinity;
  let local = Math.min(hi, Math.max(lo, toLocal(inst, t)));
  if (ref.key === 0) {
    // La primera clave marca el inicio del clip: se mueve el clip y se recolocan las demás.
    local = Math.min(local, hi);
    const shift = local - keys[0].t;
    const newStart = Math.max(0, inst.start + shift / speed);
    const realShift = (newStart - inst.start) * speed;
    for (let k = 1; k < keys.length; k++) keys[k].t -= realShift;
    inst.start = newStart;
    keepSpeed(inst, speed);
    return;
  }
  keys[ref.key].t = Math.max(0, local);
  keepSpeed(inst, speed);
}

export function deleteKey(tl: Timeline, ref: KeyRef) {
  const inst = tl.camera[ref.clip];
  const clip = inline(inst);
  if (clip?.type !== "keyframes") return;
  if (clip.keys.length <= 1) {
    tl.camera.splice(ref.clip, 1);
    return;
  }
  const speed = speedOf(inst);
  if (ref.key === 0) {
    const shift = clip.keys[1].t;
    clip.keys.splice(0, 1);
    for (const k of clip.keys) k.t -= shift;
    inst.start += shift / speed;
  } else {
    clip.keys.splice(ref.key, 1);
  }
  keepSpeed(inst, speed);
}

/** Mueve un clip para que empiece en `start`. */
export function moveClip(tl: Timeline, idx: number, start: number) {
  tl.camera[idx].start = Math.max(0, start);
}

/**
 * Arrastre de un borde del clip hasta el instante `t`.
 * - `stretch` (por defecto): cambia la duración, la animación va más deprisa o más despacio.
 * - recorte: quita (o devuelve) parte del principio o del final, a la misma velocidad.
 */
export function dragEdge(tl: Timeline, idx: number, edge: "start" | "end", t: number, mode: "stretch" | "trim") {
  const i = tl.camera[idx];
  const c = inline(i);
  const fixedEnd = end(i);
  if (mode === "stretch" || !c || c.type === "hold") {
    if (edge === "end") i.duration = Math.max(MIN_DURATION, t - i.start);
    else {
      i.start = Math.max(0, Math.min(t, fixedEnd - MIN_DURATION));
      i.duration = fixedEnd - i.start;
    }
    if (c?.type === "hold" && mode === "trim") c.length = i.duration;
    return;
  }
  const speed = speedOf(i);
  const len = clipLength(c);
  if (edge === "end") {
    const trimOut = Math.min(len - i.trimIn - MIN_DURATION * speed, Math.max(0, len - toLocal(i, t)));
    i.trimOut = trimOut;
  } else {
    const trimIn = Math.min(len - i.trimOut - MIN_DURATION * speed, Math.max(0, toLocal(i, t)));
    i.trimIn = trimIn;
    i.start = fixedEnd - (len - i.trimIn - i.trimOut) / speed;
  }
  keepSpeed(i, speed);
  if (edge === "end") i.duration = Math.max(MIN_DURATION, (len - i.trimIn - i.trimOut) / speed);
}

/** Cambia un recorte (en segundos del clip) sin cambiar la velocidad. */
export function setTrim(i: ClipInstance, which: "in" | "out", v: number) {
  const c = inline(i);
  if (!c || c.type === "hold") return;
  const speed = speedOf(i);
  const len = clipLength(c);
  const other = which === "in" ? i.trimOut : i.trimIn;
  const value = Math.min(Math.max(0, v), Math.max(0, len - other - MIN_DURATION * speed));
  if (which === "in") i.trimIn = value;
  else i.trimOut = value;
  keepSpeed(i, speed);
}

/** Cambia la velocidad (1 = la original) estirando o encogiendo el clip. */
export function setSpeed(i: ClipInstance, speed: number) {
  if (speed > 0) keepSpeed(i, speed);
}

/** Copia un clip de la timeline (para la biblioteca o para duplicarlo): sin recortes. */
export function bakeInstance(i: ClipInstance): Clip | null {
  const c = inline(i);
  if (!c) return null;
  const copy = $snapshot(c);
  const speed = speedOf(i);
  if (copy.type === "hold") return { ...copy, length: i.duration };
  const lo = i.trimIn;
  const hi = clipLength(c) - i.trimOut;
  const scale = (t: number) => (t - lo) / speed;
  if (copy.type === "keyframes") copy.keys = copy.keys.filter((k) => k.t >= lo - 1e-9 && k.t <= hi + 1e-9).map((k) => ({ ...k, t: scale(k.t) }));
  else copy.samples = copy.samples.filter((s) => s.t >= lo - 1e-9 && s.t <= hi + 1e-9).map((s) => ({ ...s, t: scale(s.t) }));
  return copy;
}

/** Puntos a los que se pegan los arrastres: 0, el cabezal, bordes de clip y claves. */
export function snapPoints(tl: Timeline, playhead: number, exclude: number): number[] {
  const pts = [0, playhead];
  tl.camera.forEach((i, k) => {
    if (k === exclude) return;
    pts.push(i.start, end(i), ...keyTimes(i));
  });
  return pts;
}

export function snap(t: number, points: number[], threshold: number): number {
  let best = t;
  let d = threshold;
  for (const p of points) {
    if (Math.abs(p - t) < d) {
      d = Math.abs(p - t);
      best = p;
    }
  }
  return best;
}

/** Nombre libre del estilo "Grabación 3". */
export function nextName(base: string, used: (string | undefined)[]): string {
  const taken = new Set(used);
  for (let n = 1; ; n++) if (!taken.has(`${base} ${n}`)) return `${base} ${n}`;
}

/** Valida lo que viene de fuera (biblioteca importada): un clip con la forma correcta. */
export function cleanClip(raw: unknown): Clip | null {
  if (!raw || typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  const num = (v: unknown) => typeof v === "number" && Number.isFinite(v);
  const pose = (v: unknown): PoseObj | null => {
    const p = v as Record<string, unknown> | null;
    if (!p || !num(p.cx) || !num(p.cy) || !num(p.zoom) || (p.zoom as number) <= 0) return null;
    return { cx: p.cx as number, cy: p.cy as number, zoom: p.zoom as number, rot: num(p.rot) ? (p.rot as number) : 0 };
  };
  const easing = (v: unknown): Easing => {
    const e = v as Record<string, unknown> | null;
    if (e?.type === "cubic-bezier" && num(e.x1) && num(e.y1) && num(e.x2) && num(e.y2))
      return { type: "cubic-bezier", x1: e.x1 as number, y1: e.y1 as number, x2: e.x2 as number, y2: e.y2 as number };
    const t = EASINGS.find((x) => x === e?.type && x !== "cubic-bezier");
    return { type: (t ?? "ease-in-out") as Exclude<EasingType, "cubic-bezier"> };
  };
  const timed = <T>(list: unknown, map: (o: Record<string, unknown>, p: PoseObj) => T): T[] | null => {
    if (!Array.isArray(list)) return null;
    const out: T[] = [];
    let last = -Infinity;
    for (const o of list) {
      const p = pose(o?.pose);
      if (!p || !num(o.t) || o.t < last) return null;
      last = o.t;
      out.push(map(o, p));
    }
    return out;
  };
  if (r.type === "keyframes") {
    const keys = timed(r.keys, (o, p) => ({ t: o.t as number, pose: p, easing: easing(o.easing) }));
    return keys?.length ? { type: "keyframes", keys } : null;
  }
  if (r.type === "recorded") {
    const samples = timed(r.samples, (o, p) => ({ t: o.t as number, pose: p }));
    return samples?.length ? { type: "recorded", samples } : null;
  }
  if (r.type === "hold" && num(r.length) && (r.length as number) > 0) {
    const p = r.pose === undefined ? undefined : pose(r.pose);
    if (p === null) return null;
    return p ? { type: "hold", length: r.length as number, pose: p } : { type: "hold", length: r.length as number };
  }
  return null;
}
