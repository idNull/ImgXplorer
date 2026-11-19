<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { GlViewer, TextureError, then, type FxFrame, type Layer, type Lens, type TextureSet } from "./gl";
  import { attachInput } from "./input";
  import type { FxTrack } from "../fx/fx";
  import { HANDLES, cursorAt, handlePoint, press, type PanelEdit } from "./panelEdit";
  import { PictureCache, type PictureRef } from "./picture";
  import type { ViewerStatus } from "./types";
  import { Player, type PlayerInfo, type PlayOptions, type Scene } from "../action/player";
  import { FIT, actualSizeZoom, lerpPose, panBy, rotated, viewMatrix, zoomAt, type Affine, type Pose, type Size } from "../core";
  import { WHOLE, framePose, unionBox, type Framing, type ReadLayout } from "../reading/layout";
  import type { Box } from "../reading/boxes.svelte";
  import { Strip } from "../reading/strip";
  import { t } from "../i18n.svelte";
  import { log } from "../log";

  interface Props {
    /** Imagen actual (fuera de la reproducción). */
    item: PictureRef | null;
    /** Imágenes a precargar (vecinas). */
    prefetch?: PictureRef[];
    /** Referencia de la imagen i (la usa el reproductor). */
    refAt?: (i: number) => PictureRef;
    /** Viñetas a dibujar sobre la imagen: `[x, y, w, h, …]` normalizadas, en orden de lectura. */
    panels?: Float64Array | null;
    onstatus?: (s: ViewerStatus) => void;
    /** Estado del reproductor (`null` al parar). */
    onplayer?: (p: PlayerInfo | null) => void;
    /** Color de fondo (#rrggbb). */
    background?: string;
    /** Multiplicador de la velocidad de zoom con la rueda. */
    wheelSpeed?: number;
    /** El usuario ha movido la cámara a mano (fuera de la reproducción). */
    onmanual?: () => void;
    /** Edición de viñetas: el botón izquierdo dibuja, mueve y redimensiona en vez de desplazar. */
    editPanels?: PanelEdit | null;
    /** Ajustes de imagen (🎨) de la imagen i, listos para el shader. Cambiar la función repinta. */
    adjust?: ((index: number) => ArrayLike<number> | null) | null;
    /** Cómo se lee (📖): doble página o tira vertical. Sin él, una página como siempre. */
    layout?: ReadLayout | null;
    /** Encuadre de lectura (ventana, ancho, alto, ✂ contenido). Sin él, la imagen entera. */
    framing?: Framing | null;
    /** Ya se sabe el tamaño de la imagen i (la doble página lo necesita para emparejar). */
    onsize?: (index: number, size: Size) => void;
    /** En la tira vertical, la página del centro de la vista ha cambiado. */
    onindex?: (index: number) => void;
    /** Mayús + arrastrar ha elegido una zona de la imagen (normalizada); `null` al quitarla. */
    onselect?: (box: Box | null) => void;
    /** Clic corto con el botón derecho (mantenerlo es la lupa): menú de la imagen. */
    oncontext?: (x: number, y: number) => void;
  }

  let {
    item, prefetch = [], refAt, panels = null, onstatus, onplayer, background = "#0b0d10", wheelSpeed = 1, onmanual, editPanels = null, adjust = null,
    layout = null, framing = null, onsize, onindex, onselect, oncontext,
  }: Props = $props();

  interface Slot {
    key: string;
    /** Posición en la fuente (−1 si no se sabe). */
    index: number;
    size: Size;
    set: TextureSet;
  }

  let canvas: HTMLCanvasElement;
  let overlay: HTMLCanvasElement;
  let gl: GlViewer;
  let pose: Pose = FIT;
  let vp: Size = { w: 1, h: 1 };
  let dirty = true;
  let anim: { from: Pose; to: Pose; t0: number; ms: number } | null = null;
  let status: ViewerStatus = { zoomPercent: 100, image: null, lastInput: null, fps: 0, maxTexture: 0, error: null, tiles: 0, loadMs: 0 };

  // Imágenes decodificadas en memoria (actual + vecinas), limitada por tamaño.
  const cache = new PictureCache();
  // Imágenes subidas a la GPU, de la menos a la más reciente. Se conservan unas pocas: la
  // mostrada, las que usa el reproductor y la recién subida (aunque aún no se muestre).
  const MAX_SLOTS = 4;
  const slots = new Map<string, Slot>();
  const uploading = new Map<string, Promise<Slot>>();
  let shown: Slot | null = null;
  /** Doble página: las dos imágenes como una sola de `size` (cada una con su matriz dentro). */
  let spread: { key: string; size: Size; parts: { slot: Slot; m: Affine }[] } | null = null;
  /** Tira vertical (webtoon). */
  let strip: Strip | null = null;
  let stripId = "";
  /** Última página actual que se avisó desde la tira. */
  let stripAt = -1;
  /** Imágenes que usa la tira ahora (no se liberan). */
  let stripKeys = new Set<string>();
  /** La cámara sigue como la dejó el encuadre (sin tocar a mano): se puede reencuadrar sola. */
  let touched = false;
  /** La próxima imagen se encuadra por el final (al volver atrás leyendo). */
  let arriveEnd = false;
  let framedEnd = false;
  /** Lupa: 1 = botón derecho, 2 = tecla; y el ratón sobre el visor. */
  let lensBits = 0;
  let mouse: { x: number; y: number } | null = null;
  let player: Player | null = null;
  let playerInfoAt = 0;
  let lastPlayerState = "";
  let mounted = $state(false);

  $effect(() => {
    const bg = background;
    if (!mounted) return;
    gl.setBackground(bg);
    dirty = true;
  });

  function report(patch: Partial<ViewerStatus>) {
    status = { ...status, ...patch };
    if (strip) {
      const c = stripSlot(strip.center(vp));
      if (c) status.zoomPercent = Math.round((strip.colW(vp) / c.size.w) * (window.devicePixelRatio || 1) * 100);
    } else if (shown) {
      // % respecto a píxeles físicos (100% = 1:1 en pantalla)
      status.zoomPercent = Math.round((pose[2] / actualSizeZoom(viewSize()!, vp)) * 100);
    }
    onstatus?.(status);
  }

  function ensureSlot(ref: PictureRef): Promise<Slot> {
    const have = slots.get(ref.url);
    if (have?.set.tiles.length) return Promise.resolve(touchSlot(have));
    let p = uploading.get(ref.url);
    if (!p) {
      p = cache
        .get(ref)
        .then((pic) => {
          const slot = { key: ref.url, index: ref.index ?? -1, size: { w: pic.w, h: pic.h }, set: gl.createSet(pic) };
          slots.set(ref.url, slot);
          trimSlots(slot.key);
          if (slot.index >= 0) {
            strip?.setSize(slot.index, slot.size);
            onsize?.(slot.index, slot.size);
          }
          return slot;
        })
        .finally(() => uploading.delete(ref.url));
      uploading.set(ref.url, p);
    }
    return p;
  }

  /** Marca la imagen como usada recientemente. */
  function touchSlot(s: Slot): Slot {
    slots.delete(s.key);
    slots.set(s.key, s);
    return s;
  }

  /** Libera de la GPU las imágenes más antiguas que no se muestran ni usa el reproductor. */
  function trimSlots(fresh?: string) {
    const keep = new Set([shown?.key, fresh, ...(player?.keys() ?? []), ...(spread?.parts.map((p) => p.slot.key) ?? []), ...stripKeys]);
    for (const [k, s] of slots) {
      if (slots.size <= MAX_SLOTS) break;
      if (keep.has(k)) continue;
      gl.deleteSet(s.set);
      slots.delete(k);
    }
  }

  function show(slot: Slot, resetPose: boolean, sp: typeof spread = null) {
    const changed = shown !== slot || spread?.key !== sp?.key;
    if (changed && sel) {
      sel = null;
      onselect?.(null);
    }
    spread = sp;
    shown = touchSlot(slot);
    if (resetPose) {
      vp = gl.resize();
      framedEnd = arriveEnd;
      arriveEnd = false;
      pose = framed(0, framedEnd);
      anim = null;
      touched = false;
    }
    if (changed) {
      trimSlots();
      const size = viewSize()!;
      report({ image: size, error: null, tiles: sp ? sp.parts.reduce((n, p) => n + p.slot.set.tiles.length, 0) : slot.set.tiles.length });
    }
    dirty = true;
  }

  /** Dos imágenes juntas, a la misma altura. */
  function showSpread(key: string, a: Slot, b: Slot) {
    const h = Math.max(a.size.h, b.size.h);
    const ka = h / a.size.h;
    const kb = h / b.size.h;
    const wa = a.size.w * ka;
    touchSlot(b);
    show(a, true, {
      key,
      size: { w: wa + b.size.w * kb, h },
      parts: [
        { slot: a, m: new Float64Array([ka, 0, 0, ka, 0, 0]) },
        { slot: b, m: new Float64Array([kb, 0, 0, kb, wa, 0]) },
      ],
    });
  }

  /** Tamaño de lo que se ve: la imagen o la doble página. */
  function viewSize(): Size | null {
    return spread?.size ?? shown?.size ?? null;
  }

  /** Recuadro con contenido de lo que se ve (✂), normalizado; `null` sin recorte. */
  function boxNow(): Box | null {
    const f = framing?.box;
    if (!f || !shown) return null;
    if (!spread) return shown.index >= 0 ? f(shown.index) : null;
    const { size } = spread;
    const mapped = spread.parts.map(({ slot, m }) => {
      const b = (slot.index >= 0 ? f(slot.index) : null) ?? WHOLE;
      const k = m[0];
      return [(m[4] + b[0] * slot.size.w * k) / size.w, (m[5] + b[1] * slot.size.h * k) / size.h, (b[2] * slot.size.w * k) / size.w, (b[3] * slot.size.h * k) / size.h] as Box;
    });
    const u = unionBox(mapped)!;
    return u[2] >= 0.999 && u[3] >= 0.999 ? null : u;
  }

  /** Pose del encuadre de lectura (sin él, la imagen entera, como siempre). */
  function framed(rot: number, end: boolean): Pose {
    const size = viewSize();
    if (!framing || !size) return new Float64Array([0.5, 0.5, 1, rot]);
    return framePose(size, vp, rot, framing.fit, boxNow(), framing.rtl, end);
  }

  function setPose(p: Pose, animate = 0) {
    if (animate > 0) {
      anim = { from: pose, to: p, t0: performance.now(), ms: animate };
    } else {
      anim = null;
      pose = p;
    }
    dirty = true;
  }

  /** Antes de un movimiento manual: durante la reproducción, el reproductor cede el control. */
  function manual(): boolean {
    if (!player) {
      touched = true;
      onmanual?.();
      return true;
    }
    const s = player.touch();
    if (!s) return false;
    const slot = slots.get(s.key);
    if (slot && slot !== shown) show(slot, false);
    if (s.pose) pose = s.pose;
    return true;
  }

  // API pública (bind:this desde App)
  export function fit() {
    if (strip && !player) {
      strip.reset(vp);
      dirty = true;
      return report({});
    }
    if (!manual()) return;
    setPose(player ? new Float64Array([0.5, 0.5, 1, pose[3]]) : framed(pose[3], false), 180);
    touched = false;
    status.lastInput = "teclado";
  }
  export function actualSize() {
    if (strip && !player) {
      const c = stripSlot(strip.center(vp));
      if (c) strip.zoomAt(c.size.w / (window.devicePixelRatio || 1) / strip.colW(vp), vp.w / 2, vp.h / 2, vp);
      dirty = true;
      return report({});
    }
    if (!shown || !manual()) return;
    const p = new Float64Array(pose);
    p[2] = actualSizeZoom(viewSize()!, vp);
    setPose(p, 180);
  }
  export function zoomBy(f: number) {
    if (strip && !player) {
      strip.zoomAt(f, vp.w / 2, vp.h / 2, vp);
      dirty = true;
      return report({});
    }
    if (!shown || !manual()) return;
    setPose(zoomAt(anim ? anim.to : pose, f, vp.w / 2, vp.h / 2, viewSize()!, vp), 140);
    status.lastInput = "teclado";
  }
  export function pan(dx: number, dy: number) {
    if (strip && !player) {
      strip.scroll(-dy, vp, true);
      strip.pan(dx, vp);
      dirty = true;
      return;
    }
    if (!shown || !manual()) return;
    setPose(panBy(anim ? anim.to : pose, dx, dy, viewSize()!, vp), 120);
    status.lastInput = "teclado";
  }
  export function rotate(deg: number) {
    if (strip && !player) return;
    if (!manual()) return;
    setPose(rotated(anim ? anim.to : pose, deg), 220);
  }
  export function isZoomedIn(): boolean {
    return !strip && pose[2] > 1.01;
  }

  /**
   * Lectura: desplaza hacia abajo/derecha (`dir` 1) o arriba/izquierda (−1) lo que quede de página
   * fuera de la vista en ese eje, como mucho `max` px. `false` si ya se ve ese borde (toca pasar
   * de página). `max` 0 usa casi una pantalla.
   */
  export function scrollToward(axis: "x" | "y", dir: 1 | -1, max = 0): boolean {
    if (player) return false;
    const limit = max || (axis === "y" ? vp.h : vp.w) * 0.85;
    if (strip) {
      if (axis === "x") return false;
      if (dir < 0 ? strip.atStart() : strip.atEnd(vp)) return false;
      strip.scroll(dir * limit, vp, true);
      dirty = true;
      return true;
    }
    const size = viewSize();
    if (!size) return false;
    const from = anim ? anim.to : pose;
    const m = viewMatrix(from, size, vp);
    const [bx, by, bw, bh] = boxNow() ?? WHOLE;
    const xs: number[] = [];
    const ys: number[] = [];
    for (const [u, v] of [[bx, by], [bx + bw, by], [bx, by + bh], [bx + bw, by + bh]]) {
      xs.push(m[0] * u * size.w + m[2] * v * size.h + m[4]);
      ys.push(m[1] * u * size.w + m[3] * v * size.h + m[5]);
    }
    const lo = Math.min(...(axis === "x" ? xs : ys));
    const hi = Math.max(...(axis === "x" ? xs : ys));
    const over = dir > 0 ? hi - (axis === "x" ? vp.w : vp.h) : -lo;
    if (over <= 1 || !manual()) return false;
    const d = Math.min(over, limit) * dir;
    setPose(panBy(from, axis === "x" ? -d : 0, axis === "y" ? -d : 0, size, vp), 200);
    return true;
  }

  /** La próxima imagen que se muestre se encuadra por el final (volviendo atrás al leer). */
  /** Zona de la imagen que se ve ahora (normalizada), en una sola página; `null` si no aplica. */
  export function visibleBox(): Box | null {
    if (!shown || spread || strip || player) return null;
    const { w, h } = shown.size;
    const [a, b, c, d, ex, fy] = viewMatrix(pose, shown.size, vp);
    const det = a * d - b * c;
    const xs: number[] = [];
    const ys: number[] = [];
    for (const [px, py] of [[0, 0], [vp.w, 0], [0, vp.h], [vp.w, vp.h]]) {
      const x = px - ex;
      const y = py - fy;
      xs.push((d * x - c * y) / det / w);
      ys.push((-b * x + a * y) / det / h);
    }
    const x0 = Math.max(0, Math.min(...xs));
    const y0 = Math.max(0, Math.min(...ys));
    const x1 = Math.min(1, Math.max(...xs));
    const y1 = Math.min(1, Math.max(...ys));
    return x1 - x0 > 1e-3 && y1 - y0 > 1e-3 ? [x0, y0, x1 - x0, y1 - y0] : null;
  }

  /** Zona elegida con Mayús + arrastrar (coordenadas normalizadas de la imagen actual). */
  let sel: { x0: number; y0: number; x1: number; y1: number; dragging: boolean } | null = null;

  export function clearSelection() {
    if (!sel) return;
    sel = null;
    dirty = true;
    onselect?.(null);
  }

  function selBox(): Box | null {
    if (!sel) return null;
    const x0 = Math.max(0, Math.min(sel.x0, sel.x1));
    const y0 = Math.max(0, Math.min(sel.y0, sel.y1));
    const x1 = Math.min(1, Math.max(sel.x0, sel.x1));
    const y1 = Math.min(1, Math.max(sel.y0, sel.y1));
    return x1 - x0 > 0.005 && y1 - y0 > 0.005 ? [x0, y0, x1 - x0, y1 - y0] : null;
  }

  /** Punto de la vista → punto normalizado de la imagen actual (sin límites). */
  function toImage(clientX: number, clientY: number): [number, number] {
    const r = canvas.getBoundingClientRect();
    const [a, b, c, d, ex, fy] = viewMatrix(pose, shown!.size, vp);
    const x = clientX - r.left - ex;
    const y = clientY - r.top - fy;
    const det = a * d - b * c;
    return [(d * x - c * y) / det / shown!.size.w, (-b * x + a * y) / det / shown!.size.h];
  }

  export function arriveAtEnd() {
    arriveEnd = true;
  }

  /** Lupa con la tecla (mientras se mantiene pulsada). */
  export function setLens(on: boolean) {
    lensBits = on ? lensBits | 2 : lensBits & ~2;
    dirty = true;
  }

  const LENS_ZOOM = 2.5;
  function lensNow(): Lens | null {
    if (!lensBits || !mouse || player || editPanels || picking) return null;
    const r = Math.min(220, Math.max(90, Math.min(vp.w, vp.h) * 0.2));
    return { x: mouse.x, y: mouse.y, r, zoom: LENS_ZOOM };
  }

  /**
   * Pose de la cámara. `final`: al final de la animación en curso (para guardar una clave);
   * si no, la que se ve en este fotograma (para grabar el movimiento).
   */
  export function currentPose(final = true): Pose {
    return new Float64Array(final && anim ? anim.to : pose);
  }

  /** Efectos especiales que se ven fuera de la reproducción (el editor, en el cabezal). */
  let fxNow: { track: FxTrack; t: number } | null = null;

  /**
   * Efectos a mostrar con la pose actual en el segundo `t` de la imagen (el editor los manda al
   * mover el cabezal); `null` los quita.
   */
  export function setFxNow(track: FxTrack | null, t = 0) {
    if (player || (!track && !fxNow)) return;
    fxNow = track ? { track, t } : null;
    dirty = true;
  }

  /** Esperando un clic en la imagen («Elegir punto» del editor). */
  let picking = $state<((p: { x: number; y: number } | null) => void) | null>(null);

  /** El siguiente clic en el visor devuelve ese punto de la imagen (0..1); `null` si se cancela. */
  export function pickPoint(): Promise<{ x: number; y: number } | null> {
    picking?.(null);
    return new Promise((resolve) => {
      picking = (p) => {
        picking = null;
        resolve(p);
      };
    });
  }

  export function cancelPick() {
    picking?.(null);
  }

  function onPickDown(e: PointerEvent) {
    if (!picking && e.shiftKey && e.button === 0 && shown && !spread && !strip && !player && !editPanels) {
      // Mayús + arrastrar: elegir una zona para copiarla o guardarla.
      e.stopPropagation();
      e.preventDefault();
      const [x, y] = toImage(e.clientX, e.clientY);
      sel = { x0: x, y0: y, x1: x, y1: y, dragging: true };
      capture(e);
      dirty = true;
      return;
    }
    if (!picking) return;
    e.stopPropagation();
    e.preventDefault();
    if (!shown || e.button !== 0) return picking(null);
    const r = canvas.getBoundingClientRect();
    const [a, b, c, d, ex, fy] = viewMatrix(pose, shown.size, vp);
    const x = e.clientX - r.left - ex;
    const y = e.clientY - r.top - fy;
    const det = a * d - b * c;
    const ix = (d * x - c * y) / det / shown.size.w;
    const iy = (-b * x + a * y) / det / shown.size.h;
    picking({ x: Math.min(1, Math.max(0, ix)), y: Math.min(1, Math.max(0, iy)) });
  }

  /** Pone la pose sin animación (el editor la lleva al mover el cabezal). */
  export function setPoseNow(p: Pose) {
    if (player) return;
    anim = null;
    pose = new Float64Array(p);
    dirty = true;
    report({});
  }

  /** Ancho / alto del área de la imagen. */
  export function aspect(): number {
    return vp.w / Math.max(1, vp.h);
  }

  /** Empieza a reproducir el Modo Acción desde `spec.start`. */
  export function play(spec: Omit<PlayOptions, "load">) {
    stop();
    if (!refAt) return;
    spread = null;
    const at = refAt;
    player = new Player(
      {
        ...spec,
        load: async (i) => {
          const slot = await ensureSlot(at(i));
          return { key: slot.key, size: slot.size };
        },
      },
      vp.w / Math.max(1, vp.h),
    );
    lastPlayerState = "";
    dirty = true;
  }

  export function stop() {
    if (!player) return;
    player.stop();
    player = null;
    anim = null;
    trimSlots();
    onplayer?.(null);
    dirty = true;
  }

  export function playback(): Player | null {
    return player;
  }

  $effect(() => {
    const ref = item;
    const lay = layout;
    if (player) return; // durante la reproducción manda el reproductor
    if (lay?.kind === "strip") return useStrip(lay);
    if (strip) {
      strip = null;
      stripKeys = new Set();
      shown = null;
    }
    if (!ref) {
      shown = null;
      spread = null;
      report({ image: null });
      dirty = true;
      return;
    }
    if (lay?.kind === "double" && lay.pages.length === 2) {
      const key = lay.pages.map((p) => p.url).join("|");
      if (spread?.key === key) return;
      let cancelled = false;
      Promise.all(lay.pages.map(ensureSlot))
        .then(([a, b]) => {
          if (!cancelled && !player) showSpread(key, a, b);
        })
        .catch((e) => {
          if (cancelled) return;
          // Si una de las dos no se puede cargar, se ve la que pidió el usuario sola.
          log.warn("doble página:", e);
          ensureSlot(ref).then((slot) => !cancelled && !player && show(slot, true), () => {});
        });
      return () => {
        cancelled = true;
      };
    }
    if (!spread && shown?.key === ref.url) return;
    let cancelled = false;
    const t0 = performance.now();
    ensureSlot(ref)
      .then((slot) => {
        if (cancelled || player) return;
        show(slot, true);
        report({ loadMs: Math.round(performance.now() - t0) });
      })
      .catch((e) => {
        if (cancelled) return;
        shown = null;
        dirty = true;
        if (e instanceof TextureError) {
          log.error(`no se pudo subir ${ref.url} a la GPU:`, e.message);
          report({ image: null, error: t("tooLarge", { max: gl.maxTexture }) });
        } else {
          log.warn(`no se pudo cargar ${ref.url}:`, e);
          report({ image: null, error: t("loadError") });
        }
      });
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    for (const r of prefetch) cache.get(r).catch(() => {});
  });

  $effect(() => {
    void panels;
    void editPanels?.rects;
    void editPanels?.selected;
    dirty = true;
  });

  /** Pinta lo que pide el reproductor. */
  function drawScene(scene: Scene) {
    if (!scene) return drawShown();
    if (scene.kind === "single") {
      const slot = slots.get(scene.key);
      if (!slot) return drawShown();
      if (slot !== shown) show(slot, false);
      if (scene.pose) {
        pose = scene.pose;
        anim = null;
      }
      return drawShown(scene.fx ?? null);
    }
    const a = slots.get(scene.a);
    const b = slots.get(scene.b);
    if (!a || !b) return drawShown();
    const f = scene.f;
    const layers: Layer[] = [
      { set: a.set, m: f.subarray(0, 6), alpha: f[6], adj: adjOf(a) },
      { set: b.set, m: f.subarray(7, 13), alpha: f[13], adj: adjOf(b) },
    ];
    gl.draw(layers, vp, f[14] > 0);
    clearOverlay();
  }

  function drawShown(fx: FxFrame | null = player || !fxNow || !shown ? null : fxNow.track.frame(fxNow.t, shown.size, pose, vp)) {
    let layers: Layer[] = [];
    if (spread) {
      const v = viewMatrix(pose, spread.size, vp);
      layers = spread.parts.map((p) => ({ set: p.slot.set, m: then(p.m, v), alpha: 1, adj: adjOf(p.slot) }));
    } else if (shown) {
      layers = [{ set: shown.set, m: viewMatrix(pose, shown.size, vp), alpha: 1, adj: adjOf(shown) }];
    }
    gl.draw(layers, vp, false, fx, lensNow());
    drawOverlay();
  }

  // ── Tira vertical ──────────────────────────────────────────────────────────

  function useStrip(lay: Extract<ReadLayout, { kind: "strip" }>) {
    if (!strip || stripId !== lay.id || strip.count !== lay.count) {
      strip = new Strip(lay.count, lay.at);
      strip.full = framing?.fit === "width";
      stripId = lay.id;
      stripAt = lay.at;
      for (const sl of slots.values()) if (sl.index >= 0) strip.setSize(sl.index, sl.size);
      strip.jump(lay.at, vp);
      shown = null;
      spread = null;
      anim = null;
      dirty = true;
      return;
    }
    if (lay.at !== stripAt) {
      // Se ha ido a otra página desde fuera (miniaturas, Inicio, Ctrl+G…).
      stripAt = lay.at;
      strip.jump(lay.at, vp);
      dirty = true;
    }
  }

  function stripSlot(i: number): Slot | null {
    const r = refAt?.(i);
    const s = r ? slots.get(r.url) : undefined;
    return s?.set.tiles.length ? s : null;
  }

  function drawStrip() {
    const s = strip!;
    const col = s.column(vp);
    const layers: Layer[] = [];
    const keys = new Set<string>();
    for (const p of s.visible(vp, 2)) {
      const r = refAt?.(p.index);
      if (!r) continue;
      keys.add(r.url);
      const slot = slots.get(r.url);
      if (!slot?.set.tiles.length) {
        ensureSlot(r).then(() => (dirty = true), (e) => log.warn(`tira: no se pudo cargar ${r.url}:`, e));
        continue;
      }
      if (p.y >= vp.h || p.y + p.h <= 0) continue;
      const k = col.w / slot.size.w;
      layers.push({ set: slot.set, m: new Float64Array([k, 0, 0, k, col.x, p.y]), alpha: 1, adj: adjOf(slot) });
    }
    stripKeys = keys;
    gl.draw(layers, vp, false, null, lensNow());
    drawOverlay();
    const c = s.center(vp);
    if (c !== stripAt) {
      stripAt = c;
      onindex?.(c);
    }
    const cs = stripSlot(c);
    if (cs && status.image !== cs.size) report({ image: cs.size, error: null, tiles: cs.set.tiles.length });
  }

  const adjOf = (s: Slot) => (adjust && s.index >= 0 ? adjust(s.index) : null);

  $effect(() => {
    void adjust;
    dirty = true;
  });

  // Cambia el encuadre de lectura (o llega el recuadro con contenido): si la cámara sigue como la
  // dejó el encuadre, se reencuadra.
  $effect(() => {
    const f = framing;
    if (!mounted) return;
    untrack(() => {
      if (strip) {
        const full = f?.fit === "width";
        if (strip.full !== full) {
          strip.full = full;
          strip.reset(vp);
          dirty = true;
        }
        return;
      }
      if (!touched && shown && !player && !picking) setPose(framed(pose[3], framedEnd), 160);
    });
  });

  function clearOverlay() {
    overlay.getContext("2d")!.clearRect(0, 0, overlay.width, overlay.height);
  }

  /** Viñetas numeradas sobre la imagen (fuera de la reproducción). */
  function drawOverlay(): void {
    const dpr = window.devicePixelRatio || 1;
    const pw = Math.round(vp.w * dpr);
    const ph = Math.round(vp.h * dpr);
    if (overlay.width !== pw || overlay.height !== ph) {
      overlay.width = pw;
      overlay.height = ph;
    }
    const ctx = overlay.getContext("2d")!;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, pw, ph);
    const lens = lensNow();
    if (lens) {
      ctx.lineWidth = 2 * dpr;
      ctx.strokeStyle = "rgb(255 255 255 / 0.85)";
      ctx.beginPath();
      ctx.arc(lens.x * dpr, lens.y * dpr, lens.r * dpr, 0, Math.PI * 2);
      ctx.stroke();
      ctx.lineWidth = 1 * dpr;
      ctx.strokeStyle = "rgb(0 0 0 / 0.6)";
      ctx.beginPath();
      ctx.arc(lens.x * dpr, lens.y * dpr, (lens.r + 1.5) * dpr, 0, Math.PI * 2);
      ctx.stroke();
    }
    if (!shown || player || spread) return;
    const sb = selBox();
    if (sb && sel) {
      const m = viewMatrix(pose, shown.size, vp);
      const { w, h } = shown.size;
      const pt = (x: number, y: number) => [(m[0] * x * w + m[2] * y * h + m[4]) * dpr, (m[1] * x * w + m[3] * y * h + m[5]) * dpr];
      const c = [pt(sb[0], sb[1]), pt(sb[0] + sb[2], sb[1]), pt(sb[0] + sb[2], sb[1] + sb[3]), pt(sb[0], sb[1] + sb[3])];
      // Fuera de la zona, oscurecido; la zona, con borde discontinuo.
      ctx.fillStyle = "rgb(0 0 0 / 0.45)";
      ctx.beginPath();
      ctx.rect(0, 0, pw, ph);
      c.forEach(([x, y], k) => (k ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
      ctx.closePath();
      ctx.fill("evenodd");
      ctx.setLineDash([6 * dpr, 4 * dpr]);
      ctx.lineWidth = 1.5 * dpr;
      ctx.strokeStyle = "#fbbf24";
      ctx.beginPath();
      c.forEach(([x, y], k) => (k ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
      ctx.closePath();
      ctx.stroke();
      ctx.setLineDash([]);
    }
    if (editPanels) return drawEditing(ctx, dpr);
    if (!panels?.length) return;
    const m = viewMatrix(pose, shown.size, vp);
    const { w, h } = shown.size;
    const pt = (x: number, y: number) => [(m[0] * x * w + m[2] * y * h + m[4]) * dpr, (m[1] * x * w + m[3] * y * h + m[5]) * dpr];
    for (let i = 0; i + 3 < panels.length; i += 4) {
      drawRect(ctx, dpr, pt, panels.subarray(i, i + 4), i / 4 + 1, false);
    }
  }

  type Pt = (x: number, y: number) => number[];

  function drawRect(ctx: CanvasRenderingContext2D, dpr: number, pt: Pt, r: ArrayLike<number>, n: number, selected: boolean) {
    const [x, y, rw, rh] = [r[0], r[1], r[2], r[3]];
    const color = selected ? "251 191 36" : "56 189 248";
    const corners = [pt(x, y), pt(x + rw, y), pt(x + rw, y + rh), pt(x, y + rh)];
    ctx.lineWidth = 2 * dpr;
    ctx.strokeStyle = `rgb(${color} / 0.95)`;
    ctx.fillStyle = `rgb(${color} / ${selected ? 0.14 : 0.08})`;
    ctx.beginPath();
    corners.forEach(([cx, cy], k) => (k ? ctx.lineTo(cx, cy) : ctx.moveTo(cx, cy)));
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
    if (n <= 0) return;
    const [cx, cy] = pt(x + rw / 2, y + rh / 2);
    ctx.font = `600 ${14 * dpr}px system-ui, sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillStyle = "rgb(11 13 16 / 0.85)";
    ctx.beginPath();
    ctx.arc(cx, cy, 14 * dpr, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = `rgb(${color})`;
    ctx.fillText(String(n), cx, cy);
  }

  /** Viñetas editables: número de lectura (de `panels`) y tiradores en la seleccionada. */
  function drawEditing(ctx: CanvasRenderingContext2D, dpr: number) {
    const e = editPanels!;
    const m = viewMatrix(pose, shown!.size, vp);
    const { w, h } = shown!.size;
    const pt: Pt = (x, y) => [(m[0] * x * w + m[2] * y * h + m[4]) * dpr, (m[1] * x * w + m[3] * y * h + m[5]) * dpr];
    const order = (r: number[]) => {
      if (!panels) return 0;
      for (let k = 0; k + 3 < panels.length; k += 4) {
        if (panels[k] === r[0] && panels[k + 1] === r[1] && panels[k + 2] === r[2] && panels[k + 3] === r[3]) return k / 4 + 1;
      }
      return 0;
    };
    for (let i = 0; i * 4 < e.rects.length; i++) {
      const r = e.rects.slice(i * 4, i * 4 + 4);
      drawRect(ctx, dpr, pt, r, order(r), i === e.selected);
    }
    if (e.selected < 0 || e.selected * 4 >= e.rects.length) return;
    const r = e.rects.slice(e.selected * 4, e.selected * 4 + 4);
    ctx.fillStyle = "#fbbf24";
    ctx.strokeStyle = "#0b0d10";
    ctx.lineWidth = 1.5 * dpr;
    for (const hd of HANDLES) {
      const [hx, hy] = pt(...handlePoint(r, hd));
      ctx.beginPath();
      ctx.rect(hx - 4.5 * dpr, hy - 4.5 * dpr, 9 * dpr, 9 * dpr);
      ctx.fill();
      ctx.stroke();
    }
  }

  /** Seguir recibiendo el puntero aunque salga del visor (si el navegador lo permite). */
  function capture(e: PointerEvent) {
    try {
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    } catch {
      // Sin captura: el arrastre termina al salir del visor.
    }
  }

  // Botón derecho: mantenerlo pulsado es la lupa; un clic corto abre el menú de la imagen.
  let rightTimer = 0;
  let rightDown = false;
  function onLensDown(e: PointerEvent) {
    if (e.button !== 2 || player || editPanels) return;
    onMouse(e);
    rightDown = true;
    capture(e);
    clearTimeout(rightTimer);
    rightTimer = window.setTimeout(() => {
      rightTimer = 0;
      if (!rightDown) return;
      lensBits |= 1;
      dirty = true;
    }, 180);
  }
  function onLensUp(e: PointerEvent) {
    if (sel?.dragging && e.button === 0) {
      sel.dragging = false;
      const b = selBox();
      if (!b) sel = null;
      dirty = true;
      onselect?.(b);
      return;
    }
    if (!rightDown || (e.type === "pointerup" && e.button !== 2)) return;
    rightDown = false;
    if (rightTimer) {
      clearTimeout(rightTimer);
      rightTimer = 0;
      if (e.type === "pointerup") oncontext?.(e.clientX, e.clientY);
      return;
    }
    lensBits &= ~1;
    dirty = true;
  }
  function onMouse(e: PointerEvent) {
    const r = canvas.getBoundingClientRect();
    mouse = { x: e.clientX - r.left, y: e.clientY - r.top };
    if (sel?.dragging && shown) {
      [sel.x1, sel.y1] = toImage(e.clientX, e.clientY);
      dirty = true;
    }
    if (lensBits) dirty = true;
  }

  onMount(() => {
    gl = new GlViewer(canvas);
    report({ maxTexture: gl.maxTexture });

    const detach = attachInput(canvas, {
      zoom(f, x, y, srcName) {
        if (!Number.isFinite(f) || f <= 0 || !Number.isFinite(x) || !Number.isFinite(y)) {
          log.warn(`zoom ignorado: factor=${f} en (${x}, ${y}) desde ${srcName}`);
          return;
        }
        if (strip && !player) {
          strip.zoomAt(f, x, y, vp);
          dirty = true;
          status.lastInput = srcName;
          return report({});
        }
        if (!shown) return;
        if (!manual()) return;
        anim = null;
        pose = zoomAt(pose, f, x, y, viewSize()!, vp);
        dirty = true;
        status.lastInput = srcName;
      },
      pan(dx, dy, srcName) {
        if (strip && !player) {
          strip.scroll(-dy, vp);
          strip.pan(dx, vp);
          dirty = true;
          status.lastInput = srcName;
          return;
        }
        if (!shown || !manual()) return;
        anim = null;
        pose = panBy(pose, dx, dy, viewSize()!, vp);
        dirty = true;
        status.lastInput = srcName;
      },
      scroll(dx, dy, srcName) {
        // En la tira vertical, la rueda desplaza (Ctrl + rueda hace zoom).
        if (!strip || player) return false;
        if (srcName === "rueda") strip.scroll(dy * 1.4, vp, true);
        else {
          strip.scroll(dy, vp);
          strip.pan(-dx, vp);
        }
        dirty = true;
        status.lastInput = srcName;
        return true;
      },
      interaction(active) {
        // Mientras se mantiene el arrastre o el pellizco, la animación no recupera el control.
        player?.hold(active);
      },
      wheelSpeed: () => wheelSpeed,
      press(x, y) {
        if (!editPanels || !shown || player) return null;
        anim = null;
        const edit = editPanels;
        return press(edit, () => ({ m: viewMatrix(pose, shown!.size, vp), img: shown!.size }), x, y);
      },
      hover(x, y) {
        if (!editPanels || !shown || player) return null;
        return cursorAt(editPanels, viewMatrix(pose, shown.size, vp), shown.size, x, y);
      },
      doubleClick(x, y) {
        if (strip && !player) {
          if (Math.abs(strip.zoom - 1) > 0.01) strip.reset(vp);
          else strip.zoomAt(2, x, y, vp);
          dirty = true;
          return report({});
        }
        if (!shown) return;
        const size = viewSize()!;
        if (pose[2] > 1.01) fit();
        else if (manual()) setPose(zoomAt(pose, actualSizeZoom(size, vp) / pose[2], x, y, size, vp), 200);
      },
    });

    mounted = true;
    let resized = false;
    const ro = new ResizeObserver(() => {
      dirty = true;
      resized = true;
    });
    ro.observe(canvas);

    let raf = 0;
    let frames = 0;
    let fpsT0 = performance.now();
    let last = performance.now();
    const frame = (now: number) => {
      raf = requestAnimationFrame(frame);
      const dt = Math.min(100, now - last);
      last = now;
      if (strip && !player && strip.step(dt, vp)) dirty = true;
      if (anim) {
        const u = Math.min(1, (now - anim.t0) / anim.ms);
        const e = 1 - (1 - u) ** 3; // ease-out
        pose = lerpPose(anim.from, anim.to, e);
        if (u >= 1) anim = null;
        dirty = true;
      }
      if (player) {
        vp = gl.resize();
        drawScene(player.frame(now, pose, vp));
        const info = player.info();
        if (info.state !== lastPlayerState || now - playerInfoAt > 100) {
          lastPlayerState = info.state;
          playerInfoAt = now;
          onplayer?.(info);
          if (shown) report({});
        }
      } else {
        if (!dirty) return;
        dirty = false;
        vp = gl.resize();
        if (resized) {
          resized = false;
          // Al cambiar el tamaño de la ventana, el encuadre de lectura se rehace si no se ha tocado.
          if (strip) strip.scroll(0, vp);
          else if (framing && shown && !touched && !anim) pose = framed(pose[3], framedEnd);
        }
        if (strip) drawStrip();
        else drawShown();
      }
      frames++;
      if (now - fpsT0 > 500) {
        report({ fps: Math.round((frames * 1000) / (now - fpsT0)) });
        frames = 0;
        fpsT0 = now;
      }
    };
    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      detach();
      player?.stop();
    };
  });
</script>

<div class="viewer" class:picking={!!picking} onpointerdowncapture={onPickDown} onpointerdown={onLensDown} onpointerup={onLensUp}
  onpointercancel={onLensUp} onpointermove={onMouse} onpointerleave={() => { mouse = null; if (lensBits) dirty = true; }}
  oncontextmenu={(e) => e.preventDefault()} role="presentation">
  <canvas bind:this={canvas}></canvas>
  <canvas class="overlay" bind:this={overlay}></canvas>
</div>

<style>
  .viewer {
    position: relative;
    width: 100%;
    height: 100%;
  }
  .viewer.picking, .viewer.picking canvas { cursor: crosshair; }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
    touch-action: none;
    cursor: grab;
  }
  .overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
</style>
