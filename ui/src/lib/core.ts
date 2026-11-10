// Envoltorio tipado de imgx-core (WASM). Toda la matemática de cámara vive en Rust:
// así la vista previa y el render MP4 producen exactamente lo mismo.
import init, * as wasm from "./wasm/pkg/imgx.js";

/** Pose de cámara: [cx, cy, zoom, rot] (coordenadas normalizadas de imagen). */
export type Pose = Float64Array;
/** Matriz afín imagen→viewport [a, b, c, d, e, f]. */
export type Affine = Float64Array;

export interface Size {
  w: number;
  h: number;
}

export const FIT: Pose = new Float64Array([0.5, 0.5, 1, 0]);

export async function initCore(): Promise<void> {
  await init();
}

export const coreVersion = () => wasm.core_version();

export function viewMatrix(p: Pose, img: Size, vp: Size): Affine {
  return wasm.view_matrix(p, img.w, img.h, vp.w, vp.h);
}

export function zoomAt(p: Pose, factor: number, ax: number, ay: number, img: Size, vp: Size): Pose {
  return wasm.zoom_at(p, factor, ax, ay, img.w, img.h, vp.w, vp.h);
}

export function panBy(p: Pose, dx: number, dy: number, img: Size, vp: Size): Pose {
  return wasm.pan_by(p, dx, dy, img.w, img.h, vp.w, vp.h);
}

/** Zoom con el que 1 píxel de imagen = 1 píxel físico de pantalla. */
export function actualSizeZoom(img: Size, vp: Size): number {
  const fit = Math.min(vp.w / img.w, vp.h / img.h);
  return 1 / (fit * window.devicePixelRatio);
}

export function rotated(p: Pose, deg: number): Pose {
  const r = new Float64Array(p);
  // Sin normalizar a 0..360: así la animación 270°→360° no da la vuelta hacia atrás.
  r[3] = p[3] + deg;
  return r;
}

export { wasm };

export function lerpPose(a: Pose, b: Pose, t: number): Pose {
  return wasm.lerp_pose(a, b, t);
}
