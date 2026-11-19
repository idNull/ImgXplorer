// Sacar una imagen de la app (copiar, guardar como, recortar): se dibuja fuera de pantalla con el
// mismo motor WebGL que el visor, así sale igual que se ve (con los ajustes de imagen si se pide).
import { invoke } from "@tauri-apps/api/core";
import { FrameRenderer, mult4 } from "../mcp/frames";
import type { PictureRef } from "../viewer/picture";
import type { Box } from "../reading/boxes.svelte";

/** Lado mayor de lo que se saca: por encima se reduce (lo que admiten la GPU y el lienzo 2D). */
export const MAX_SIDE = 8192;

let renderer: FrameRenderer | null = null;

/**
 * La imagen (o la zona `box`, normalizada) a su tamaño real, como lienzo 2D. `adj`: los ajustes de
 * imagen para el shader, o `null` para el original.
 */
export async function renderPage(ref: PictureRef, adj: ArrayLike<number> | null, box: Box | null): Promise<HTMLCanvasElement> {
  renderer ??= new FrameRenderer();
  const pic = await renderer.picture(ref);
  const [bx, by, bw, bh] = box ?? [0, 0, 1, 1];
  const rw = Math.max(1, bw * pic.w);
  const rh = Math.max(1, bh * pic.h);
  const k = Math.min(1, MAX_SIDE / Math.max(rw, rh));
  const ow = Math.max(1, Math.round(rw * k));
  const oh = Math.max(1, Math.round(rh * k));
  // La captura va en múltiplos de 4: se dibuja un poco más grande, a la misma escala, y se recorta.
  const w = mult4(ow + 3);
  const h = mult4(oh + 3);
  const fit = Math.min(w / pic.w, h / pic.h);
  const pose = new Float64Array([bx + bw / 2, by + bh / 2, k / fit, 0]);
  const img = await renderer.render(ref, pose, w, h, "#ffffff", undefined, adj);
  const full = document.createElement("canvas");
  full.width = w;
  full.height = h;
  full.getContext("2d")!.putImageData(img, 0, 0);
  const out = document.createElement("canvas");
  out.width = ow;
  out.height = oh;
  out.getContext("2d")!.drawImage(full, Math.round((w - ow) / 2), Math.round((h - oh) / 2), ow, oh, 0, 0, ow, oh);
  return out;
}

export async function encode(c: HTMLCanvasElement, type: "image/png" | "image/jpeg", quality = 0.92): Promise<Uint8Array> {
  const blob = await new Promise<Blob | null>((res) => c.toBlob(res, type, quality));
  if (!blob) throw new Error("no se pudo codificar la imagen");
  return new Uint8Array(await blob.arrayBuffer());
}

export async function copyToClipboard(c: HTMLCanvasElement): Promise<void> {
  await invoke("copy_image", await encode(c, "image/png"));
}

export async function saveTo(path: string, c: HTMLCanvasElement, type: "image/png" | "image/jpeg"): Promise<void> {
  await invoke("save_image", await encode(c, type), { headers: { path: encodeURIComponent(path) } });
}
