// Letras de los efectos de texto: onomatopeyas («¡BOOM!») con la fuente Bangers y subtítulos,
// carteles y cartelas de narración. Se pintan una vez en un lienzo 2D (que la GPU usa como
// textura) y el núcleo dice dónde va cada una en cada fotograma.
import { log } from "../log";

/** Qué hay que pintar (también es la clave de la caché de texturas). */
export interface TextLook {
  kind: "text" | "caption";
  text: string;
  color: string;
  outline: string;
  /** Solo subtítulos: `subtitle`, `title` o `narration`. */
  style: string;
}

/** Un texto de un fotograma: cómo se ve y dónde va (en px de la vista). */
export interface TextDraw extends TextLook {
  overlay: boolean;
  x: number;
  y: number;
  ax: number;
  ay: number;
  /** Alto de una línea y ancho máximo (px de la vista; 0 sin partir). */
  line: number;
  wrap: number;
  scale: number;
  rot: number;
  alpha: number;
}

export const COMIC_FONT = '"Bangers", "Impact", "Arial Black", sans-serif';

/** Cambia cuando termina de cargar una fuente: las letras ya pintadas se repintan con ella. */
export let fontEpoch = 0;
document.fonts?.addEventListener?.("loadingdone", () => fontEpoch++);
const CAPTION_FONT = 'system-ui, "Segoe UI", "Helvetica Neue", Arial, sans-serif';

/** Espera a que la fuente de cómic esté lista (si no, la primera textura saldría con otra). */
export async function loadTextFonts(): Promise<void> {
  try {
    // Se pide la cara de la fuente directamente (no la lista con alternativas).
    const faces = [...document.fonts].filter((f) => f.family.replace(/"/g, "") === "Bangers");
    await Promise.race([Promise.all(faces.map((f) => f.load())), new Promise((r) => setTimeout(r, 2000))]);
    log.info(`fuente de cómic: ${faces.length} caras, ${faces.map((f) => f.status).join(", ")}; lista: ${document.fonts.check('64px "Bangers"', "BOOM")}`);
  } catch (e) {
    // Sin la fuente se usa la siguiente de la lista.
    log.warn("fuente de cómic:", e);
  }
}

/** Parte el texto en líneas que caben en `max` px (respetando los saltos de línea escritos). */
function wrapLines(g: CanvasRenderingContext2D, text: string, max: number): string[] {
  const out: string[] = [];
  for (const para of text.split(/\r?\n/)) {
    if (!max) {
      out.push(para);
      continue;
    }
    let line = "";
    for (const word of para.split(/\s+/).filter(Boolean)) {
      const next = line ? `${line} ${word}` : word;
      if (line && g.measureText(next).width > max) {
        out.push(line);
        line = word;
      } else {
        line = next;
      }
    }
    out.push(line);
  }
  return out;
}

/**
 * Lienzo con las letras. `px`: alto de una línea en píxeles del destino; `wrap`: ancho máximo en
 * píxeles del destino (0: sin partir).
 */
export function paintText(look: TextLook, px: number, wrap: number): HTMLCanvasElement {
  const c = document.createElement("canvas");
  const g = c.getContext("2d")!;
  const comic = look.kind === "text";
  const style = comic ? "comic" : look.style;
  const font = comic ? `${px}px ${COMIC_FONT}` : `${style === "title" ? 800 : 600} ${px}px ${CAPTION_FONT}`;
  g.font = font;
  const lines = wrapLines(g, look.text.trim(), comic ? 0 : wrap);
  const lh = px * (comic ? 1.05 : 1.25);
  const widest = Math.max(1, ...lines.map((l) => g.measureText(l).width));
  const stroke = comic ? px * 0.16 : style === "narration" ? 0 : px * 0.16;
  const pad = style === "narration" ? px * 0.55 : stroke + px * 0.12;
  c.width = Math.ceil(widest + pad * 2);
  c.height = Math.ceil(lh * lines.length + pad * 2);
  g.font = font;
  g.textBaseline = "middle";
  g.lineJoin = "round";
  g.miterLimit = 2;
  if (style === "narration") {
    // Cartela de cómic: recuadro amarillo con borde de tinta.
    g.fillStyle = "#fde68a";
    g.strokeStyle = "#111111";
    g.lineWidth = Math.max(2, px * 0.08);
    g.fillRect(g.lineWidth / 2, g.lineWidth / 2, c.width - g.lineWidth, c.height - g.lineWidth);
    g.strokeRect(g.lineWidth / 2, g.lineWidth / 2, c.width - g.lineWidth, c.height - g.lineWidth);
  }
  const align = style === "narration" ? "left" : "center";
  g.textAlign = align;
  const x = align === "left" ? pad : c.width / 2;
  lines.forEach((l, i) => {
    const y = pad + lh * (i + 0.5);
    if (stroke > 0) {
      g.strokeStyle = comic ? look.outline : "rgb(0 0 0 / 0.9)";
      g.lineWidth = stroke * 2;
      if (comic) {
        // Sombra dura desplazada, como en las viñetas.
        g.save();
        g.translate(px * 0.05, px * 0.07);
        g.strokeText(l, x, y);
        g.restore();
      }
      g.strokeText(l, x, y);
    }
    g.fillStyle = look.color;
    g.fillText(l, x, y);
  });
  return c;
}

/** Matriz textura → vista de un texto de `w`×`h` px pintado a `k` px por px de la vista. */
export function textMatrix(d: TextDraw, w: number, h: number, k: number): Float64Array {
  const s = d.scale / k;
  const r = (d.rot * Math.PI) / 180;
  const a = s * Math.cos(r);
  const b = s * Math.sin(r);
  const c = -b;
  const dd = a;
  const px = d.ax * w;
  const py = d.ay * h;
  return new Float64Array([a, b, c, dd, d.x - (a * px + c * py), d.y - (b * px + dd * py)]);
}
