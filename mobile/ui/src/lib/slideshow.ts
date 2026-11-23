// Pase de fotos: cómo se reproducen los álbumes (fotos del carrete). Las historias y los cómics
// siguen con su propia animación; en un álbum, ▶ pasa de una foto a otra con la transición, el
// tiempo y el movimiento que se eligen aquí. Se recuerda en el almacenamiento del WebView.
//
// Accesibilidad: si el móvil pide «Reducir movimiento», lo predeterminado no se mueve (fundido y
// foto fija) y «Variada» solo mezcla transiciones sin movimiento. Se puede elegir otra cosa igual.
import { wasm, type Size } from "$imgx/core";
import type { Sequence } from "$imgx/action/sequence.svelte";
import type { Transition, TransitionType } from "$imgx/action/project.svelte";

/** Transiciones que se ofrecen. `camera`: viaje de cámara (`continuous-camera`); `mix`: variada. */
export type ShowTransition = "fade" | "dip" | "slide" | "zoom" | "camera" | "cut" | "mix";
/** `still`: la foto entera, quieta. `gentle`: acercamiento lento (Ken Burns), distinto en cada foto. */
export type ShowMotion = "still" | "gentle";
/** Duración de la transición. */
export type ShowPace = "fast" | "normal" | "slow";

export interface Show {
  transition: ShowTransition;
  /** Segundos que se ve cada foto (sin contar la transición). */
  seconds: number;
  motion: ShowMotion;
  pace: ShowPace;
  /** Al acabar, vuelve a empezar. */
  loop: boolean;
}

export const TRANSITIONS: ShowTransition[] = ["fade", "dip", "slide", "zoom", "camera", "cut", "mix"];
/** Las que mueven la imagen por la pantalla (se avisa con «Reducir movimiento»). */
export const MOVING: ReadonlySet<ShowTransition> = new Set(["slide", "zoom", "camera"]);
export const PACES: ShowPace[] = ["fast", "normal", "slow"];
export const MIN_SECONDS = 1;
export const MAX_SECONDS = 30;
const PACE_SECONDS: Record<ShowPace, number> = { fast: 0.4, normal: 0.8, slow: 1.5 };

/** El sistema pide reducir el movimiento (Accesibilidad → Movimiento en iOS; «Quitar animaciones» en Android). */
export const reducedMotion = (): boolean => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;

const KEY = "imgx.slideshow";

function defaults(): Show {
  const still = reducedMotion();
  return { transition: "fade", seconds: 4, motion: still ? "still" : "gentle", pace: "normal", loop: false };
}

const pick = <T extends string>(v: unknown, all: readonly T[], def: T): T => (all.includes(v as T) ? (v as T) : def);

function load(): Show {
  const d = defaults();
  try {
    const r = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    const s = Number(r.seconds);
    return {
      transition: pick(r.transition, TRANSITIONS, d.transition),
      seconds: Number.isFinite(s) ? clampSeconds(s) : d.seconds,
      motion: pick(r.motion, ["still", "gentle"], d.motion),
      pace: pick(r.pace, PACES, d.pace),
      loop: typeof r.loop === "boolean" ? r.loop : d.loop,
    };
  } catch {
    return d;
  }
}

export const clampSeconds = (s: number) => Math.round(Math.min(MAX_SECONDS, Math.max(MIN_SECONDS, s)));

export const show: Show = load();

export function saveShow(s: Partial<Show>) {
  Object.assign(show, s);
  localStorage.setItem(KEY, JSON.stringify(show));
}

/** Variada: un orden fijo (el pase se ve igual cada vez) que no repite la anterior. */
const MIX_ALL: ShowTransition[] = ["fade", "slide", "dip", "zoom", "camera"];
const MIX_STILL: ShowTransition[] = ["fade", "dip"];

/** La transición que hay detrás de la foto `i`. */
export function transitionAt(s: Show, i: number, reduce: boolean): ShowTransition {
  if (s.transition !== "mix") return s.transition;
  const list = reduce ? MIX_STILL : MIX_ALL;
  return list[i % list.length];
}

const ENGINE: Record<Exclude<ShowTransition, "mix">, TransitionType> = {
  fade: "fade",
  dip: "dip",
  slide: "slide",
  zoom: "zoom",
  camera: "continuous-camera",
  cut: "cut",
};

/** Ken Burns: hacia dónde se acerca cada foto, para que no todas hagan lo mismo. */
const FOCUS = ["center", "top", "right", "bottom", "left"];

/**
 * La reproducción de un álbum con el pase elegido. `rtl`: sentido manga (la siguiente foto llega
 * por la izquierda). El sonido y los efectos, si el álbum tuviera, siguen siendo los de `base`.
 */
export function showSequence(base: Sequence, s: Show, rtl: boolean, reduce: boolean): Sequence {
  const seconds = clampSeconds(s.seconds);
  return {
    ...base,
    async timeline(i: number, size: Size, aspect: number) {
      const tpl =
        s.motion === "gentle"
          ? { id: "ken-burns", params: { duration: seconds, zoom: 1.15, focus: FOCUS[i % FOCUS.length], way: i % 2 ? "out" : "in" } }
          : { id: "full", params: { duration: seconds } };
      return wasm.WasmTimeline.from_template(JSON.stringify(tpl), size.w, size.h, aspect, new Float64Array(), false);
    },
    transitionOut(i: number) {
      const kind = ENGINE[transitionAt(s, i, reduce) as Exclude<ShowTransition, "mix">];
      const duration = kind === "cut" ? 0 : PACE_SECONDS[s.pace];
      const tr: Transition = { type: kind, duration };
      // La siguiente llega por el lado hacia el que se lee.
      if (kind === "slide" || kind === "continuous-camera") tr.dir = rtl ? "left" : "right";
      return { json: JSON.stringify(tr), tpl: "", duration, cut: kind === "cut" };
    },
    holdBetween: () => 0,
  };
}
