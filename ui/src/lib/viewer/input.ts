// Traduce ratón, touchpad y gestos nativos a intenciones de alto nivel (zoom / pan).
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type InputSource = "rueda" | "ctrl+rueda" | "touchpad-scroll" | "pellizco-gtk" | "pellizco-mac" | "arrastre";

export interface InputHandlers {
  /** factor multiplicativo de zoom alrededor de (x, y) en px CSS relativos al elemento. */
  zoom(factor: number, x: number, y: number, src: InputSource): void;
  pan(dx: number, dy: number, src: InputSource): void;
  /** Inicio/fin de una interacción manual (el reproductor la usa para "override"). */
  interaction?(active: boolean): void;
  doubleClick?(x: number, y: number): void;
  /** Multiplicador de la velocidad de zoom con la rueda del ratón (ajustes). */
  wheelSpeed?(): number;
  /**
   * Botón izquierdo pulsado en (x, y): si devuelve un arrastre, se encarga él (edición de
   * viñetas) en lugar de desplazar la imagen. El botón central siempre desplaza.
   */
  press?(x: number, y: number): { move(x: number, y: number): void; up(): void } | null;
  /** Cursor sobre (x, y) cuando no se arrastra (`null`: el normal). */
  hover?(x: number, y: number): string | null;
}

/** Heurística: la rueda de un ratón da saltos grandes y enteros; el touchpad, deltas finos. */
function looksLikeMouseWheel(e: WheelEvent): boolean {
  if (e.deltaMode !== WheelEvent.DOM_DELTA_PIXEL) return true;
  return e.deltaX === 0 && Number.isInteger(e.deltaY) && Math.abs(e.deltaY) >= 50;
}

export function attachInput(el: HTMLElement, h: InputHandlers): () => void {
  const local = (cx: number, cy: number) => {
    const r = el.getBoundingClientRect();
    return [cx - r.left, cy - r.top] as const;
  };
  let interactionTimer: number | undefined;
  const pulse = () => {
    h.interaction?.(true);
    clearTimeout(interactionTimer);
    interactionTimer = window.setTimeout(() => h.interaction?.(false), 150);
  };

  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    const [x, y] = local(e.clientX, e.clientY);
    const dy = e.deltaMode === WheelEvent.DOM_DELTA_LINE ? e.deltaY * 40 : e.deltaY;
    pulse();
    if (e.ctrlKey) {
      h.zoom(Math.exp(-dy * 0.01), x, y, "ctrl+rueda"); // pellizco en Chromium/algunos WebKit
    } else if (looksLikeMouseWheel(e)) {
      h.zoom(Math.exp(-dy * 0.0025 * (h.wheelSpeed?.() ?? 1)), x, y, "rueda");
    } else {
      h.pan(-e.deltaX, -e.deltaY, "touchpad-scroll");
    }
  };

  let drag: { id: number; x: number; y: number; own: { move(x: number, y: number): void; up(): void } | null } | null = null;
  const onDown = (e: PointerEvent) => {
    if (e.button !== 0 && e.button !== 1) return;
    const own = e.button === 0 ? (h.press?.(...local(e.clientX, e.clientY)) ?? null) : null;
    drag = { id: e.pointerId, x: e.clientX, y: e.clientY, own };
    el.setPointerCapture(e.pointerId);
    if (own) return;
    el.style.cursor = "grabbing";
    h.interaction?.(true);
  };
  const onMove = (e: PointerEvent) => {
    if (!drag) {
      if (h.hover) el.style.cursor = h.hover(...local(e.clientX, e.clientY)) ?? "";
      return;
    }
    if (e.pointerId !== drag.id) return;
    if (drag.own) return drag.own.move(...local(e.clientX, e.clientY));
    h.pan(e.clientX - drag.x, e.clientY - drag.y, "arrastre");
    drag.x = e.clientX;
    drag.y = e.clientY;
  };
  const onUp = (e: PointerEvent) => {
    if (!drag || e.pointerId !== drag.id) return;
    const own = drag.own;
    drag = null;
    if (own) return own.up();
    el.style.cursor = "";
    h.interaction?.(false);
  };
  const onDbl = (e: MouseEvent) => {
    const [x, y] = local(e.clientX, e.clientY);
    h.doubleClick?.(x, y);
  };

  // macOS / WKWebView: eventos de gesto propietarios de WebKit.
  let macScale = 1;
  const onGestureStart = (e: any) => {
    e.preventDefault();
    macScale = 1;
    h.interaction?.(true);
  };
  const onGestureChange = (e: any) => {
    e.preventDefault();
    const [x, y] = local(e.clientX, e.clientY);
    h.zoom(e.scale / macScale, x, y, "pellizco-mac");
    macScale = e.scale;
  };
  const onGestureEnd = (e: any) => {
    e.preventDefault();
    h.interaction?.(false);
  };

  el.addEventListener("wheel", onWheel, { passive: false });
  el.addEventListener("pointerdown", onDown);
  el.addEventListener("pointermove", onMove);
  el.addEventListener("pointerup", onUp);
  el.addEventListener("pointercancel", onUp);
  el.addEventListener("dblclick", onDbl);
  el.addEventListener("gesturestart", onGestureStart);
  el.addEventListener("gesturechange", onGestureChange);
  el.addEventListener("gestureend", onGestureEnd);

  // Linux / WebKitGTK: pellizco reenviado desde Rust (ver src-tauri/src/gestures.rs).
  let gtkScale = 1;
  let unlisten: UnlistenFn | undefined;
  let disposed = false;
  listen<{ phase: string; scale: number; x: number; y: number }>("pinch", ({ payload: p }) => {
    // Las coordenadas vienen relativas al webview; el elemento puede no estar en (0,0).
    const r = el.getBoundingClientRect();
    const [x, y] = [p.x - r.left, p.y - r.top];
    if (p.phase === "begin") {
      gtkScale = 1;
      h.interaction?.(true);
    } else if (p.phase === "update") {
      h.zoom(p.scale / gtkScale, x, y, "pellizco-gtk");
      gtkScale = p.scale;
    } else {
      h.interaction?.(false);
    }
  }).then((u) => (disposed ? u() : (unlisten = u)));

  return () => {
    disposed = true;
    unlisten?.();
    el.removeEventListener("wheel", onWheel);
    el.removeEventListener("pointerdown", onDown);
    el.removeEventListener("pointermove", onMove);
    el.removeEventListener("pointerup", onUp);
    el.removeEventListener("pointercancel", onUp);
    el.removeEventListener("dblclick", onDbl);
    el.removeEventListener("gesturestart", onGestureStart);
    el.removeEventListener("gesturechange", onGestureChange);
    el.removeEventListener("gestureend", onGestureEnd);
  };
}
