<script lang="ts">
  import { onMount } from "svelte";
  import { GlViewer } from "./gl";
  import { attachInput } from "./input";
  import { PictureCache, type PictureRef } from "./picture";
  import type { ViewerStatus } from "./types";
  import { FIT, actualSizeZoom, lerpPose, panBy, rotated, viewMatrix, zoomAt, type Pose, type Size } from "../core";
  import { t } from "../i18n";
  import { log } from "../log";

  interface Props {
    /** Imagen actual. */
    item: PictureRef | null;
    /** Imágenes a precargar (vecinas). */
    prefetch?: PictureRef[];
    onstatus?: (s: ViewerStatus) => void;
  }

  let { item, prefetch = [], onstatus }: Props = $props();

  let canvas: HTMLCanvasElement;
  let gl: GlViewer;
  let pose: Pose = FIT;
  let img: Size | null = null;
  let vp: Size = { w: 1, h: 1 };
  let dirty = true;
  let anim: { from: Pose; to: Pose; t0: number; ms: number } | null = null;
  let status: ViewerStatus = { zoomPercent: 100, image: null, lastInput: null, fps: 0, maxTexture: 0, error: null, tiles: 0, loadMs: 0 };

  // Imágenes decodificadas (actual + vecinas), limitada por memoria.
  const cache = new PictureCache();

  function report(patch: Partial<ViewerStatus>) {
    status = { ...status, ...patch };
    if (img) {
      // % respecto a píxeles físicos (100% = 1:1 en pantalla)
      status.zoomPercent = Math.round((pose[2] / actualSizeZoom(img, vp)) * 100);
    }
    onstatus?.(status);
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

  // API pública (bind:this desde App)
  export function fit() {
    setPose(new Float64Array([0.5, 0.5, 1, pose[3]]), 180);
    status.lastInput = "teclado";
  }
  export function actualSize() {
    if (!img) return;
    const p = new Float64Array(pose);
    p[2] = actualSizeZoom(img, vp);
    setPose(p, 180);
  }
  export function zoomBy(f: number) {
    if (!img) return;
    setPose(zoomAt(anim ? anim.to : pose, f, vp.w / 2, vp.h / 2, img, vp), 140);
    status.lastInput = "teclado";
  }
  export function pan(dx: number, dy: number) {
    if (!img) return;
    setPose(panBy(anim ? anim.to : pose, dx, dy, img, vp), 120);
    status.lastInput = "teclado";
  }
  export function rotate(deg: number) {
    setPose(rotated(anim ? anim.to : pose, deg), 220);
  }
  export function isZoomedIn(): boolean {
    return pose[2] > 1.01;
  }

  $effect(() => {
    const ref = item;
    if (!ref) {
      img = null;
      gl?.clear();
      dirty = true;
      return;
    }
    let cancelled = false;
    const t0 = performance.now();
    cache
      .get(ref)
      .then((pic) => {
        if (cancelled) return;
        const size = { w: pic.w, h: pic.h };
        try {
          gl.setImage(pic);
        } catch (e) {
          log.error(`no se pudo subir ${ref.url} (${pic.w}×${pic.h}) a la GPU:`, e);
          img = null;
          report({ image: size, error: t("tooLarge", { max: gl.maxTexture }) });
          return;
        }
        img = size;
        pose = new Float64Array([0.5, 0.5, 1, 0]);
        anim = null;
        report({ image: size, error: null, tiles: gl.tileCount, loadMs: Math.round(performance.now() - t0) });
        dirty = true;
      })
      .catch((e) => {
        log.warn(`no se pudo cargar ${ref.url}:`, e);
        if (!cancelled) {
          img = null;
          gl.clear();
          dirty = true;
          report({ error: t("loadError") });
        }
      });
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    for (const r of prefetch) cache.get(r).catch(() => {});
  });

  onMount(() => {
    gl = new GlViewer(canvas);
    report({ maxTexture: gl.maxTexture });

    const detach = attachInput(canvas, {
      zoom(f, x, y, srcName) {
        if (!img) return;
        if (!Number.isFinite(f) || f <= 0 || !Number.isFinite(x) || !Number.isFinite(y)) {
          log.warn(`zoom ignorado: factor=${f} en (${x}, ${y}) desde ${srcName}`);
          return;
        }
        anim = null;
        pose = zoomAt(pose, f, x, y, img, vp);
        dirty = true;
        status.lastInput = srcName;
      },
      pan(dx, dy, srcName) {
        if (!img) return;
        anim = null;
        pose = panBy(pose, dx, dy, img, vp);
        dirty = true;
        status.lastInput = srcName;
      },
      doubleClick(x, y) {
        if (!img) return;
        if (pose[2] > 1.01) fit();
        else setPose(zoomAt(pose, actualSizeZoom(img, vp) / pose[2], x, y, img, vp), 200);
      },
    });

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
      if (!dirty) return;
      dirty = false;
      vp = gl.resize();
      gl.draw(img ? viewMatrix(pose, img, vp) : new Float64Array(6), vp);
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
    };
  });
</script>

<canvas bind:this={canvas}></canvas>

<style>
  canvas {
    display: block;
    width: 100%;
    height: 100%;
    touch-action: none;
    cursor: grab;
  }
</style>
