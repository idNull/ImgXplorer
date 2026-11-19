<script lang="ts">
  import { onMount } from "svelte";
  import { GlViewer, TextureError, type FxFrame, type Layer, type TextureSet } from "./gl";
  import { attachInput } from "./input";
  import type { FxTrack } from "../fx/fx";
  import { HANDLES, cursorAt, handlePoint, press, type PanelEdit } from "./panelEdit";
  import { PictureCache, type PictureRef } from "./picture";
  import type { ViewerStatus } from "./types";
  import { Player, type PlayerInfo, type PlayOptions, type Scene } from "../action/player";
  import { FIT, actualSizeZoom, lerpPose, panBy, rotated, viewMatrix, zoomAt, type Pose, type Size } from "../core";
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
  }

  let {
    item, prefetch = [], refAt, panels = null, onstatus, onplayer, background = "#0b0d10", wheelSpeed = 1, onmanual, editPanels = null,
  }: Props = $props();

  interface Slot {
    key: string;
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
    if (shown) {
      // % respecto a píxeles físicos (100% = 1:1 en pantalla)
      status.zoomPercent = Math.round((pose[2] / actualSizeZoom(shown.size, vp)) * 100);
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
          const slot = { key: ref.url, size: { w: pic.w, h: pic.h }, set: gl.createSet(pic) };
          slots.set(ref.url, slot);
          trimSlots(slot.key);
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
    const keep = new Set([shown?.key, fresh, ...(player?.keys() ?? [])]);
    for (const [k, s] of slots) {
      if (slots.size <= MAX_SLOTS) break;
      if (keep.has(k)) continue;
      gl.deleteSet(s.set);
      slots.delete(k);
    }
  }

  function show(slot: Slot, resetPose: boolean) {
    const changed = shown !== slot;
    shown = touchSlot(slot);
    if (resetPose) {
      pose = new Float64Array([0.5, 0.5, 1, 0]);
      anim = null;
    }
    if (changed) {
      trimSlots();
      report({ image: slot.size, error: null, tiles: slot.set.tiles.length });
    }
    dirty = true;
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
    if (!manual()) return;
    setPose(new Float64Array([0.5, 0.5, 1, pose[3]]), 180);
    status.lastInput = "teclado";
  }
  export function actualSize() {
    if (!shown || !manual()) return;
    const p = new Float64Array(pose);
    p[2] = actualSizeZoom(shown.size, vp);
    setPose(p, 180);
  }
  export function zoomBy(f: number) {
    if (!shown || !manual()) return;
    setPose(zoomAt(anim ? anim.to : pose, f, vp.w / 2, vp.h / 2, shown.size, vp), 140);
    status.lastInput = "teclado";
  }
  export function pan(dx: number, dy: number) {
    if (!shown || !manual()) return;
    setPose(panBy(anim ? anim.to : pose, dx, dy, shown.size, vp), 120);
    status.lastInput = "teclado";
  }
  export function rotate(deg: number) {
    if (!manual()) return;
    setPose(rotated(anim ? anim.to : pose, deg), 220);
  }
  export function isZoomedIn(): boolean {
    return pose[2] > 1.01;
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
    if (player) return; // durante la reproducción manda el reproductor
    if (!ref) {
      shown = null;
      report({ image: null });
      dirty = true;
      return;
    }
    if (shown?.key === ref.url) return;
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
      { set: a.set, m: f.subarray(0, 6), alpha: f[6] },
      { set: b.set, m: f.subarray(7, 13), alpha: f[13] },
    ];
    gl.draw(layers, vp, f[14] > 0);
    clearOverlay();
  }

  function drawShown(fx: FxFrame | null = player || !fxNow || !shown ? null : fxNow.track.frame(fxNow.t, shown.size, pose, vp)) {
    gl.draw(shown ? [{ set: shown.set, m: viewMatrix(pose, shown.size, vp), alpha: 1 }] : [], vp, false, fx);
    drawOverlay();
  }

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
    if (!shown || player) return;
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

  onMount(() => {
    gl = new GlViewer(canvas);
    report({ maxTexture: gl.maxTexture });

    const detach = attachInput(canvas, {
      zoom(f, x, y, srcName) {
        if (!shown) return;
        if (!Number.isFinite(f) || f <= 0 || !Number.isFinite(x) || !Number.isFinite(y)) {
          log.warn(`zoom ignorado: factor=${f} en (${x}, ${y}) desde ${srcName}`);
          return;
        }
        if (!manual()) return;
        anim = null;
        pose = zoomAt(pose, f, x, y, shown.size, vp);
        dirty = true;
        status.lastInput = srcName;
      },
      pan(dx, dy, srcName) {
        if (!shown || !manual()) return;
        anim = null;
        pose = panBy(pose, dx, dy, shown.size, vp);
        dirty = true;
        status.lastInput = srcName;
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
        if (!shown) return;
        if (pose[2] > 1.01) fit();
        else if (manual()) setPose(zoomAt(pose, actualSizeZoom(shown.size, vp) / pose[2], x, y, shown.size, vp), 200);
      },
    });

    mounted = true;
    const ro = new ResizeObserver(() => (dirty = true));
    ro.observe(canvas);

    let raf = 0;
    let frames = 0;
    let fpsT0 = performance.now();
    const frame = (now: number) => {
      raf = requestAnimationFrame(frame);
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
        drawShown();
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

<div class="viewer" class:picking={!!picking} onpointerdowncapture={onPickDown}>
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
