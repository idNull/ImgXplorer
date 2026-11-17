// Exportar a vídeo. Recorre las imágenes como el reproductor (animación, pausa entre imágenes y
// transición), pinta cada fotograma con el mismo motor WebGL de la vista previa en un canvas
// aparte, lo convierte a YUV en la GPU y lo envía a Rust, que se lo pasa a ffmpeg.
import { invoke } from "@tauri-apps/api/core";
import { GlViewer, type TextureSet } from "../viewer/gl";
import { PictureCache, type PictureRef } from "../viewer/picture";
import { viewMatrix, wasm, type Size } from "../core";
import type { PlaySpec } from "../action/player";

export interface VideoOptions {
  path: string;
  width: number;
  height: number;
  fps: number;
  /** Como el CRF de x264: 18 alta, 23 normal, 28 ligera. */
  crf: number;
  ffmpeg: string;
  background: string;
  /** Imágenes a exportar (índices incluidos). */
  from: number;
  to: number;
}

export interface ExportProgress {
  /** Fotogramas enviados. */
  frames: number;
  /** Imagen que se está exportando. */
  index: number;
  /** 0..1 aproximado (por imágenes). */
  fraction: number;
}

export type ExportSpec = Pick<PlaySpec, "timeline" | "transitionOut" | "holdBetween">;

interface Prepared {
  i: number;
  size: Size;
  set: TextureSet;
  tl: wasm.WasmTimeline;
  dur: number;
}

export class ExportCancelled extends Error {}

/** Duración en segundos del vídeo que saldría con estas duraciones de imágenes y transiciones. */
export function videoSeconds(durations: number[], transitions: number[], hold: number): number {
  return durations.reduce((s, d) => s + d + hold, 0) + transitions.reduce((s, d) => s + d, 0);
}

export async function exportVideo(
  o: VideoOptions,
  spec: ExportSpec,
  refAt: (i: number) => PictureRef,
  onprogress: (p: ExportProgress) => void,
  signal: AbortSignal,
): Promise<string> {
  const vp: Size = { w: o.width, h: o.height };
  const aspect = o.width / o.height;
  const canvas = document.createElement("canvas");
  canvas.width = 16;
  canvas.height = 16;
  const gl = new GlViewer(canvas);
  gl.setBackground(o.background);
  gl.capture(vp);
  const cache = new PictureCache(512 << 20);
  const live = new Set<Prepared>();

  async function prepare(i: number): Promise<Prepared> {
    const pic = await cache.get(refAt(i));
    const size = { w: pic.w, h: pic.h };
    const set = gl.createSet(pic);
    try {
      const tl = await spec.timeline(i, size, aspect);
      const p = { i, size, set, tl, dur: tl.duration() };
      live.add(p);
      return p;
    } catch (e) {
      gl.deleteSet(set);
      throw e;
    }
  }
  function release(p: Prepared) {
    if (!live.delete(p)) return;
    gl.deleteSet(p.set);
    p.tl.free();
  }

  // Dos búferes: mientras uno viaja a Rust se pinta en el otro.
  const bytes = (o.width * o.height * 3) / 2;
  const bufs = [new Uint8Array(bytes), new Uint8Array(bytes)];
  let pending: Promise<unknown> = Promise.resolve();
  let frames = 0;
  let index = o.from;
  let fraction = 0;
  let lastReport = 0;

  async function emit() {
    if (signal.aborted) throw new ExportCancelled();
    const buf = bufs[frames % 2];
    gl.readYuv(buf);
    await pending;
    pending = invoke("export_frame", buf);
    frames++;
    const now = performance.now();
    if (now - lastReport > 100) {
      lastReport = now;
      onprogress({ frames, index, fraction });
      // Ceder el hilo para que la interfaz pinte el progreso y atienda al botón de cancelar.
      await new Promise((r) => setTimeout(r, 0));
    }
  }

  await invoke("export_start", {
    options: { path: o.path, width: o.width, height: o.height, fps: o.fps, crf: o.crf, ffmpeg: o.ffmpeg || null },
  });
  try {
    const count = o.to - o.from + 1;
    const hold = spec.holdBetween();
    let cur = await prepare(o.from);
    for (let i = o.from; i <= o.to; i++) {
      index = i;
      const last = i === o.to;
      const next = last ? null : prepare(i + 1);
      next?.catch(() => {}); // el error se recoge al esperarla
      // La imagen: animación y pausa (en la última, hasta el final exacto).
      const n = Math.max(1, Math.round((cur.dur + hold) * o.fps)) + (last ? 1 : 0);
      for (let k = 0; k < n; k++) {
        fraction = (i - o.from + k / n) / count;
        const pose = cur.tl.evaluate(Math.min(k / o.fps, cur.dur));
        gl.draw([{ set: cur.set, m: viewMatrix(pose, cur.size, vp), alpha: 1 }], vp);
        await emit();
      }
      if (!next) break;
      const nx = await next;
      // Transición hacia la siguiente.
      const tr = spec.transitionOut(i);
      if (!tr.cut && tr.duration >= 0.01) {
        const from = cur.tl.evaluate(cur.dur);
        const to = nx.tl.evaluate(0);
        const m = Math.max(1, Math.round(tr.duration * o.fps));
        for (let j = 0; j < m; j++) {
          const f = wasm.transition_frame(tr.json, tr.tpl, j / m, cur.size.w, cur.size.h, from, nx.size.w, nx.size.h, to, vp.w, vp.h);
          gl.draw(
            [
              { set: cur.set, m: f.subarray(0, 6), alpha: f[6] },
              { set: nx.set, m: f.subarray(7, 13), alpha: f[13] },
            ],
            vp,
            f[14] > 0,
          );
          await emit();
        }
      }
      release(cur);
      cur = nx;
    }
    await pending;
    onprogress({ frames, index: o.to, fraction: 1 });
    return await invoke<string>("export_finish");
  } catch (e) {
    await pending.catch(() => {});
    await invoke("export_cancel").catch(() => {});
    throw e;
  } finally {
    for (const p of [...live]) release(p);
    cache.clear();
    gl.dispose();
  }
}
