// Reproductor interactivo del Modo Acción.
//
// Estados:
//   loading    cargando la imagen o su timeline
//   playing    avanza el tiempo y la cámara sigue la timeline
//   paused     tiempo congelado
//   override   el usuario mueve la cámara a mano: el tiempo se congela
//   returning  tras un momento sin tocar, la cámara vuelve con suavidad a la pose interrumpida
//   transition paso a la siguiente imagen
//   ended      fin de la última imagen
import { lerpPose, wasm, type Pose, type Size } from "../core";
import { log } from "../log";
import { audioEnd, type AudioTracks } from "../editor/model";
import { sceneSpecs, type AmbientSpec } from "../audio/sync";
import type { SoundSink } from "../audio/director";
import { fxEnd, FxTrack, type Fx } from "../fx/fx";
import type { FxFrame } from "../viewer/gl";

/** Sin tocar durante este tiempo, la cámara vuelve a la animación. */
const IDLE_MS = 1000;
/** Duración de la vuelta a la pose de la animación. */
const RETURN_MS = 600;
/** Lo máximo que avanza el tiempo en un fotograma (evita saltos si la ventana estuvo oculta). */
const MAX_STEP = 0.1;

export type PlayerState = "loading" | "playing" | "paused" | "override" | "returning" | "transition" | "ended";

export const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 2];

export interface TransitionSpec {
  /** `Transition` en JSON. */
  json: string;
  /** Plantilla de la imagen saliente en JSON ("" si no tiene), para deducir la dirección. */
  tpl: string;
  duration: number;
  cut: boolean;
}

/** Lo que el reproductor necesita de la aplicación. */
export interface PlaySpec {
  start: number;
  count: number;
  timeline(i: number, size: Size, aspect: number): Promise<wasm.WasmTimeline>;
  transitionOut(i: number): TransitionSpec;
  holdBetween(): number;
  /** Voz y efectos de la imagen i (la imagen dura al menos hasta que acaban). */
  audio?(i: number): AudioTracks;
  /** Efectos especiales de la imagen i (la imagen dura al menos hasta que acaban). */
  fx?(i: number): Fx[];
  /** Ambientes que suenan en la imagen i. */
  ambient?(i: number): AmbientSpec[];
  /** Proporción para la que se pensaron los tamaños de los efectos (el móvil, en vertical). */
  fxFit?: number;
  /** La imagen actual ha cambiado (para el contador, las miniaturas…). */
  onIndex(i: number): void;
}

/** Duración de una imagen: su animación, su sonido o sus efectos, lo que dure más. */
export function itemDuration(tl: wasm.WasmTimeline, audio: AudioTracks | undefined, fx?: readonly Fx[]): number {
  return Math.max(tl.duration(), audio ? audioEnd(audio) : 0, fxEnd(fx));
}

const NO_AUDIO: AudioTracks = { voice: [], sfx: [] };

export interface PlayOptions extends PlaySpec {
  /** Carga la imagen i en la GPU; `key` la identifica al pintar. */
  load(i: number): Promise<{ key: string; size: Size }>;
  /** A dónde mandar el sonido (sin él, el reproductor va mudo). */
  sound?: SoundSink;
}

/** Qué pintar. `pose: null` = la pose la lleva el visor (control manual). */
export type Scene =
  | { kind: "single"; key: string; pose: Pose | null; fx?: FxFrame | null }
  | { kind: "transition"; a: string; b: string; f: Float64Array }
  | null;

export interface PlayerInfo {
  state: PlayerState;
  index: number;
  t: number;
  duration: number;
  speed: number;
}

interface Prepared {
  i: number;
  key: string;
  size: Size;
  tl: wasm.WasmTimeline;
  audio: AudioTracks;
  fx: FxTrack | null;
  dur: number;
  dead: boolean;
}

function release(p: Prepared | null) {
  if (p && !p.dead) {
    p.dead = true;
    p.tl.free();
    p.fx?.free();
  }
}

const easeInOut = (u: number) => (u < 0.5 ? 4 * u * u * u : 1 - (-2 * u + 2) ** 3 / 2);

export class Player {
  state: PlayerState = "loading";
  speed = 1;
  private cur: Prepared | null = null;
  private next: Prepared | null = null;
  private nextLoading: number | null = null;
  /** Imagen siguiente que no se pudo preparar (no se reintenta en cada fotograma). */
  private nextFailed: number | null = null;
  private t = 0;
  private trans: (TransitionSpec & { u: number; from: Pose }) | null = null;
  private lastNow = 0;
  private lastTouch = 0;
  private ret: { from: Pose; t0: number } | null = null;
  /** La pose la lleva el usuario (en pausa o en override). */
  private manual = false;
  /** Cada salto invalida las cargas en curso. */
  private token = 0;
  private stopped = false;
  /** Arrastre o pellizco en curso. */
  private held = false;

  constructor(
    private o: PlayOptions,
    private aspect: number,
  ) {
    this.goTo(o.start, "playing");
  }

  get index(): number {
    return this.cur?.i ?? this.o.start;
  }

  /** Claves de las imágenes que el reproductor tiene en uso (no se deben liberar). */
  keys(): string[] {
    return [this.cur?.key, this.next?.key].filter((k): k is string => !!k);
  }

  info(): PlayerInfo {
    return { state: this.state, index: this.index, t: this.t, duration: this.cur?.dur ?? 0, speed: this.speed };
  }

  private async prepare(i: number): Promise<Prepared> {
    const { key, size } = await this.o.load(i);
    const tl = await this.o.timeline(i, size, this.aspect);
    const audio = this.o.audio?.(i) ?? NO_AUDIO;
    const list = this.o.fx?.(i);
    return { i, key, size, tl, audio, fx: FxTrack.of(list, this.o.fxFit), dur: itemDuration(tl, audio, list), dead: false };
  }

  /** Va a la imagen `i`, desde el principio, sin transición. */
  async goTo(i: number, want?: "playing" | "paused") {
    if (i < 0 || i >= this.o.count) return;
    const tok = ++this.token;
    want ??= this.state === "paused" ? "paused" : "playing";
    this.state = "loading";
    this.trans = null;
    this.ret = null;
    this.manual = false;
    try {
      const p = this.next?.i === i ? this.next : await this.prepare(i);
      if (tok !== this.token || this.stopped) {
        if (p !== this.next) release(p);
        return;
      }
      if (p !== this.next) release(this.next);
      this.next = null;
      release(this.cur);
      this.cur = p;
      this.t = 0;
      this.state = want;
      this.o.onIndex(i);
      this.soundFor(p);
      this.prefetchNext();
    } catch (e) {
      log.warn(`reproductor: no se pudo preparar la imagen ${i + 1}:`, e);
      if (tok === this.token) this.state = this.cur ? "paused" : "ended";
    }
  }

  private prefetchNext() {
    const c = this.cur;
    if (!c) return;
    const n = c.i + 1;
    if (n >= this.o.count || this.next?.i === n || this.nextLoading === n || this.nextFailed === n) return;
    release(this.next);
    this.next = null;
    this.nextLoading = n;
    const tok = this.token;
    this.prepare(n)
      .then((p) => {
        if (tok !== this.token || this.stopped || this.cur?.i !== n - 1) release(p);
        else this.next = p;
      })
      .catch((e) => {
        log.warn(`reproductor: no se pudo precargar la imagen ${n + 1}:`, e);
        if (tok === this.token) this.nextFailed = n;
      })
      .finally(() => {
        if (this.nextLoading === n) this.nextLoading = null;
      });
  }

  /** La proporción del viewport cambió: las plantillas se regeneran (encuadres distintos). */
  private reflow(aspect: number) {
    this.aspect = aspect;
    for (const p of [this.cur, this.next]) {
      if (!p) continue;
      this.o.timeline(p.i, p.size, aspect).then(
        (tl) => {
          if (p.dead) return tl.free();
          p.tl.free();
          p.tl = tl;
          p.audio = this.o.audio?.(p.i) ?? NO_AUDIO;
          const list = this.o.fx?.(p.i);
          p.fx?.free();
          p.fx = FxTrack.of(list, this.o.fxFit);
          p.dur = itemDuration(tl, p.audio, list);
          if (p === this.cur) this.soundFor(p);
        },
        (e) => log.warn("reproductor: no se pudo regenerar la timeline:", e),
      );
    }
  }

  /** La configuración cambió (plantilla, parámetros…): se regeneran las timelines. */
  refresh() {
    this.reflow(this.aspect);
  }

  private poseAt(p: Prepared, t: number): Pose {
    return p.tl.evaluate(Math.min(Math.max(t, 0), p.dur));
  }

  private startTransition() {
    const c = this.cur!;
    const spec = this.o.transitionOut(c.i);
    if (spec.cut || spec.duration < 0.01) return this.finishTransition();
    this.trans = { ...spec, u: 0, from: this.poseAt(c, c.dur) };
    this.state = "transition";
  }

  private finishTransition() {
    const n = this.next;
    this.trans = null;
    if (!n) return;
    release(this.cur);
    this.cur = n;
    this.next = null;
    this.t = 0;
    this.state = "playing";
    this.o.onIndex(n.i);
    this.soundFor(n);
    this.prefetchNext();
  }

  /** Sonido de la imagen que pasa a ser la actual. */
  private soundFor(p: Prepared) {
    const snd = this.o.sound;
    if (!snd || this.stopped) return;
    snd.scene(this, sceneSpecs(p.audio));
    snd.ambient(this, this.o.ambient?.(p.i) ?? []);
  }

  /** Avanza el reloj y devuelve qué pintar. `viewerPose` es la pose que muestra el visor. */
  frame(now: number, viewerPose: Pose, vp: Size): Scene {
    const scene = this.step(now, viewerPose, vp);
    const snd = this.o.sound;
    if (snd && !this.stopped) {
      const st = this.state;
      snd.transport(this, {
        t: this.t,
        playing: st === "playing",
        // El ambiente sigue mientras se mueve la cámara a mano o se pasa de imagen.
        running: st === "playing" || st === "override" || st === "returning" || st === "transition" || st === "loading",
        rate: this.speed,
      });
    }
    return scene;
  }

  private step(now: number, viewerPose: Pose, vp: Size): Scene {
    const dt = this.lastNow ? Math.min(MAX_STEP, (now - this.lastNow) / 1000) * this.speed : 0;
    this.lastNow = now;
    const aspect = vp.w / Math.max(1, vp.h);
    if (Math.abs(aspect - this.aspect) / this.aspect > 0.005) this.reflow(aspect);
    const c = this.cur;
    if (!c) return null;
    const single = (pose: Pose | null): Scene => ({ kind: "single", key: c.key, pose, fx: pose && c.fx?.frame(this.t, c.size, pose, vp) });

    switch (this.state) {
      case "loading":
        return single(null);
      case "playing": {
        this.t += dt;
        if (this.t >= c.dur + this.o.holdBetween()) {
          if (c.i + 1 >= this.o.count || this.nextFailed === c.i + 1) {
            this.state = "ended";
            this.t = c.dur;
          } else if (this.next) {
            this.startTransition();
            return this.step(now, viewerPose, vp);
          } else {
            this.prefetchNext(); // la siguiente aún carga: se espera en la última pose
          }
        }
        return single(this.poseAt(c, this.t));
      }
      case "transition": {
        const tr = this.trans!;
        const n = this.next;
        if (!n) {
          this.finishTransition();
          return single(this.poseAt(c, this.t));
        }
        tr.u += dt / tr.duration;
        if (tr.u >= 1) {
          this.finishTransition();
          const pose = this.poseAt(n, 0);
          return { kind: "single", key: n.key, pose, fx: n.fx?.frame(0, n.size, pose, vp) };
        }
        const f = wasm.transition_frame(tr.json, tr.tpl, tr.u, c.size.w, c.size.h, tr.from, n.size.w, n.size.h, this.poseAt(n, 0), vp.w, vp.h);
        return { kind: "transition", a: c.key, b: n.key, f };
      }
      case "override":
        if (!this.held && now - this.lastTouch > IDLE_MS) {
          this.state = "returning";
          this.ret = null;
        }
        return single(null);
      case "returning": {
        this.ret ??= { from: new Float64Array(viewerPose), t0: now };
        this.manual = false;
        const k = Math.min(1, (now - this.ret.t0) / RETURN_MS);
        const target = this.poseAt(c, this.t);
        if (k >= 1) {
          this.state = "playing";
          this.ret = null;
          return single(target);
        }
        return single(lerpPose(this.ret.from, target, easeInOut(k)));
      }
      case "paused":
      case "ended":
        return single(this.manual ? null : this.poseAt(c, this.t));
    }
  }

  /**
   * El usuario va a mover la cámara. Devuelve la imagen (y la pose, si cambió por terminar una
   * transición) sobre la que aplicar el movimiento.
   */
  touch(): { key: string; pose: Pose | null } | null {
    let pose: Pose | null = null;
    if (this.state === "transition") {
      this.finishTransition();
      if (this.cur) pose = this.poseAt(this.cur, 0);
    }
    if (!this.cur || this.state === "loading") return null;
    this.lastTouch = performance.now();
    this.manual = true;
    this.ret = null;
    if (this.state === "playing" || this.state === "returning") this.state = "override";
    return { key: this.cur.key, pose };
  }

  /** Empieza o termina un gesto (el botón sigue pulsado aunque no se mueva). */
  hold(active: boolean) {
    this.held = active;
    if (!active) this.lastTouch = performance.now();
  }

  toggle() {
    switch (this.state) {
      case "transition":
        this.finishTransition();
        this.state = "paused";
        break;
      case "playing":
      case "override":
      case "returning":
        this.state = "paused";
        this.ret = null;
        break;
      case "paused":
        this.state = this.manual ? "returning" : "playing";
        break;
      case "ended":
        this.t = 0;
        this.state = this.manual ? "returning" : "playing";
        break;
    }
  }

  private afterSeek() {
    this.manual = false;
    this.ret = null;
    if (this.state === "override" || this.state === "returning") this.state = "playing";
    if (this.state === "ended") this.state = "paused";
  }

  /** Salta a la marca (inicio/fin de clip o keyframe) siguiente o anterior. */
  mark(dir: 1 | -1) {
    if (this.state === "transition") this.finishTransition();
    const c = this.cur;
    if (!c || this.state === "loading") return;
    const marks = Array.from(c.tl.marks());
    if (dir > 0) {
      const m = marks.find((x) => x > this.t + 0.05);
      if (m === undefined) return void this.goTo(c.i + 1);
      this.t = m;
    } else {
      const prev = marks.filter((x) => x < this.t - 0.3);
      if (prev.length) this.t = prev[prev.length - 1];
      else if (this.t > 0.3 || c.i === 0) this.t = 0;
      else return void this.goTo(c.i - 1);
    }
    this.afterSeek();
  }

  seek(seconds: number) {
    if (this.state === "transition") this.finishTransition();
    const c = this.cur;
    if (!c || this.state === "loading") return;
    this.t = Math.min(Math.max(this.t + seconds, 0), c.dur);
    this.afterSeek();
  }

  changeSpeed(dir: 1 | -1) {
    const i = SPEEDS.indexOf(this.speed);
    this.speed = SPEEDS[Math.min(SPEEDS.length - 1, Math.max(0, (i < 0 ? 2 : i) + dir))];
  }

  stop() {
    this.stopped = true;
    this.o.sound?.release(this);
    this.token++;
    release(this.cur);
    release(this.next);
    this.cur = this.next = null;
  }
}
