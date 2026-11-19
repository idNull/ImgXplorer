// Renderizador WebGL2: la imagen como uno o varios quads (teselas), transformados por la
// matriz de cámara. Se tesela cuando la imagen supera MAX_TEXTURE_SIZE de la GPU.
import type { Affine, Size } from "../core";
import type { Picture } from "./picture";

/** Lado de tesela cuando hace falta teselar (más pequeña que el máximo: menos memoria desperdiciada). */
const TILE = 4096;

interface Tile {
  tex: WebGLTexture;
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Una imagen subida a la GPU (una o varias teselas). */
export interface TextureSet {
  tiles: Tile[];
}

/** Una imagen a pintar con su matriz imagen→viewport y opacidad. */
export interface Layer {
  set: TextureSet;
  m: Affine;
  alpha: number;
  /** Ajustes de imagen (🎨) ya convertidos para el shader (`adjustUniforms`); sin ellos, la imagen tal cual. */
  adj?: ArrayLike<number> | null;
}

/**
 * Ajustes de imagen para el shader de capas, 16 valores: `[activo, negro(3), blanco(3), limpiar,
 * brillo, contraste, saturación, exposición, temperatura, nitidez, tono (0 nada, 1 gris, 2 sepia),
 * noche]`.
 */
export const ADJ_LEN = 16;
const NO_ADJ = new Float32Array(ADJ_LEN);

/**
 * Efectos especiales de un instante, tal como los calcula el núcleo (`WasmFx.frame`): 50 valores
 * `[post, cam(6), flash(4), shock(5), chroma(3), blur(3), lines(11), impact(2), color(7),
 * vignette(5), wave(3)]`, o vacío si no hay ninguno activo.
 */
export type FxFrame = ArrayLike<number>;

/** Lupa (🔍): círculo de radio `r` (px CSS) en (x, y) que enseña la escena ampliada `zoom` veces. */
export interface Lens {
  x: number;
  y: number;
  r: number;
  zoom: number;
}
export const FX_LEN = 50;

/** Compone dos afines: primero `m`, luego `s` (mismo orden que el núcleo). */
export function then(m: Affine, s: ArrayLike<number>): Affine {
  return new Float64Array([
    s[0] * m[0] + s[2] * m[1],
    s[1] * m[0] + s[3] * m[1],
    s[0] * m[2] + s[2] * m[3],
    s[1] * m[2] + s[3] * m[3],
    s[0] * m[4] + s[2] * m[5] + s[4],
    s[1] * m[4] + s[3] * m[5] + s[5],
  ]) as Affine;
}

/** La imagen no cabe en la memoria de la GPU. */
export class TextureError extends Error {}


const VS = `#version 300 es
in vec2 a_unit;              // 0..1
uniform vec2 u_off;          // origen de la tesela en la imagen (px)
uniform vec2 u_size;         // tamaño de la tesela (px)
uniform vec2 u_vp;           // tamaño de viewport (px CSS)
uniform mat3 u_m;            // imagen → viewport
out vec2 v_uv;
void main() {
  vec2 p = (u_m * vec3(u_off + a_unit * u_size, 1.0)).xy;
  vec2 clip = p / u_vp * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
  v_uv = a_unit;
}`;

const FS = `#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_tex;
uniform float u_alpha;
uniform vec2 u_texel;        // 1 / tamaño de la tesela
uniform float u_adj[16];     // ajustes de imagen (ADJ_LEN)
uniform vec3 u_clip;         // lupa: solo dentro del círculo (x, y, radio en px del destino)
out vec4 color;
float luma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }
void main() {
  if (u_clip.z > 0.0 && distance(gl_FragCoord.xy, u_clip.xy) > u_clip.z) discard;
  vec4 t = texture(u_tex, v_uv);
  if (u_adj[0] > 0.5 && t.a > 0.0) {
    vec3 c = t.rgb / t.a;
    // Nitidez: realce respecto a los vecinos a la escala en que se ve (no a la del archivo).
    if (u_adj[13] > 0.0) {
      vec2 d = max(fwidth(v_uv), u_texel);
      vec3 b = (texture(u_tex, v_uv + vec2(d.x, 0.0)).rgb + texture(u_tex, v_uv - vec2(d.x, 0.0)).rgb
              + texture(u_tex, v_uv + vec2(0.0, d.y)).rgb + texture(u_tex, v_uv - vec2(0.0, d.y)).rgb) * 0.25;
      c += (c - b / max(t.a, 1e-3)) * u_adj[13] * 1.5;
    }
    // Limpiar escaneo: papel → blanco y tinta → negro, canal a canal.
    if (u_adj[7] > 0.0) {
      vec3 bl = vec3(u_adj[1], u_adj[2], u_adj[3]);
      vec3 wh = vec3(u_adj[4], u_adj[5], u_adj[6]);
      c = mix(c, clamp((c - bl) / max(wh - bl, vec3(0.05)), 0.0, 1.0), u_adj[7]);
    }
    if (u_adj[11] != 0.0) c = pow(max(c, 0.0), vec3(exp2(-u_adj[11] * 1.2)));
    c += u_adj[8] * 0.35;
    c = (c - 0.5) * (1.0 + u_adj[9]) + 0.5;
    c = mix(vec3(luma(c)), c, 1.0 + u_adj[10]);
    c *= vec3(1.0 + 0.12 * u_adj[12], 1.0 + 0.02 * u_adj[12], 1.0 - 0.12 * u_adj[12]);
    c = clamp(c, 0.0, 1.0);
    if (u_adj[14] > 1.5) {
      c = min(vec3(dot(c, vec3(0.393, 0.769, 0.189)), dot(c, vec3(0.349, 0.686, 0.168)), dot(c, vec3(0.272, 0.534, 0.131))), 1.0);
    } else if (u_adj[14] > 0.5) {
      c = vec3(luma(c));
    }
    // Noche: luminosidad invertida conservando el color, sin negro ni blanco puros.
    if (u_adj[15] > 0.5) {
      float l = luma(c);
      c = 0.09 + clamp(c + (1.0 - 2.0 * l), 0.0, 1.0) * 0.8;
    }
    t = vec4(clamp(c, 0.0, 1.0) * t.a, t.a);
  }
  color = t * u_alpha;
}`;

// Conversión a YUV 4:2:0 (I420, BT.709, rango limitado) para exportar vídeo. La salida es una
// textura RGBA de (W/4)×(1.5·H) cuyos bytes, leídos fila a fila, son exactamente los planos Y, U
// y V seguidos: cada texel lleva 4 bytes consecutivos del archivo.
const YUV_VS = `#version 300 es
void main() {
  vec2 p = vec2(gl_VertexID == 1 ? 3.0 : -1.0, gl_VertexID == 2 ? 3.0 : -1.0);
  gl_Position = vec4(p, 0.0, 1.0);
}`;

const YUV_FS = `#version 300 es
precision highp float;
precision highp int;
uniform sampler2D u_src;
uniform int u_w;
uniform int u_h;
out vec4 color;
// Píxel (x, y) de la imagen con y = 0 arriba (la textura guarda las filas de abajo arriba).
vec3 px(int x, int y) { return texelFetch(u_src, ivec2(x, u_h - 1 - y), 0).rgb; }
float luma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }
float byteAt(int i) {
  int ysize = u_w * u_h;
  int cw = u_w / 2;
  if (i < ysize) {
    return (16.0 + 219.0 * luma(px(i % u_w, i / u_w))) / 255.0;
  }
  int j = i - ysize;
  int csize = cw * (u_h / 2);
  bool v = j >= csize;
  if (v) j -= csize;
  int cx = (j % cw) * 2;
  int cy = (j / cw) * 2;
  vec3 c = (px(cx, cy) + px(cx + 1, cy) + px(cx, cy + 1) + px(cx + 1, cy + 1)) * 0.25;
  float l = luma(c);
  float d = v ? (c.r - l) / 1.5748 : (c.b - l) / 1.8556;
  return (128.0 + 224.0 * d) / 255.0;
}
void main() {
  int base = (int(gl_FragCoord.y) * (u_w / 4) + int(gl_FragCoord.x)) * 4;
  color = vec4(byteAt(base), byteAt(base + 1), byteAt(base + 2), byteAt(base + 3));
}`;

// Pasada de efectos especiales sobre la escena ya dibujada. Trabaja en píxeles CSS con y hacia
// abajo (como el núcleo). `u_f` son los 50 valores de `FxFrame`.
const FX_FS = `#version 300 es
precision highp float;
uniform sampler2D u_src;
uniform vec2 u_res;
uniform vec2 u_vp;
uniform float u_f[50];
out vec4 color;
const float TAU = 6.2831853;
float hash(float x) { return fract(sin(x * 12.9898 + 78.233) * 43758.5453); }
vec3 samp(vec2 p) { return texture(u_src, vec2(p.x / u_vp.x, 1.0 - p.y / u_vp.y)).rgb; }
vec3 chroma(vec2 q, float a) {
  if (a <= 0.0) return samp(q);
  vec2 d = (q - u_vp * 0.5) / (length(u_vp) * 0.5);
  return vec3(samp(q + d * a).r, samp(q).g, samp(q - d * a).b);
}
float luma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }
void main() {
  vec2 p = vec2(gl_FragCoord.x, u_res.y - gl_FragCoord.y) / u_res * u_vp;
  // Ondulación
  float wa = u_f[47];
  if (wa > 0.0) {
    p.x += wa * sin(p.y / u_vp.y * u_f[48] * TAU + u_f[49]);
    p.y += wa * 0.5 * sin(p.x / u_vp.x * u_f[48] * 0.7 * TAU + u_f[49] * 1.3);
  }
  // Onda expansiva: un anillo que empuja la imagen hacia fuera.
  float ss = u_f[15];
  if (ss != 0.0) {
    vec2 d = p - vec2(u_f[11], u_f[12]);
    float r = length(d);
    float x = (r - u_f[13]) / max(u_f[14], 1.0);
    if (abs(x) < 1.0) p -= d / max(r, 1e-3) * sin(x * 3.14159) * (1.0 - x * x) * ss;
  }
  // Glitch: franjas horizontales desplazadas.
  float g = u_f[17];
  if (g > 0.0) {
    float row = floor(p.y / (u_vp.y / 24.0));
    if (hash(row + u_f[18]) < g * 0.5) p.x += (hash(row * 1.7 + u_f[18]) - 0.5) * g * u_vp.x * 0.08;
  }
  // Desenfoque radial y aberración cromática.
  vec3 c;
  float ba = u_f[21];
  if (ba > 0.0) {
    vec2 bc = vec2(u_f[19], u_f[20]);
    vec3 acc = vec3(0.0);
    for (int i = 0; i < 12; i++) acc += chroma(bc + (p - bc) * (1.0 - ba * 0.25 * float(i) / 11.0), u_f[16]);
    c = acc / 12.0;
  } else {
    c = chroma(p, u_f[16]);
  }
  // Líneas de concentración (1) o de velocidad (2).
  float kind = u_f[22];
  if (kind > 0.0 && u_f[29] > 0.0) {
    float seed = u_f[30];
    float mask = 0.0;
    if (kind < 1.5) {
      vec2 d = p - vec2(u_f[23], u_f[24]);
      float r = length(d);
      float cell = (atan(d.y, d.x) / TAU + 0.5) * floor(u_f[25]);
      float id = floor(cell);
      float w = u_f[26] * (0.25 + 0.75 * hash(id + seed)) * 0.7;
      float inner = u_f[27] * (0.75 + 0.6 * hash(id * 3.1 + seed));
      float taper = smoothstep(inner, inner + u_vp.y * 0.5, r);
      float hw = w * taper;
      float line = 1.0 - smoothstep(hw * 0.6, hw + 0.02, abs(fract(cell) - 0.5));
      mask = hash(id * 7.3 + seed) < 0.2 ? 0.0 : line * smoothstep(inner, inner * 1.1 + 2.0, r);
    } else {
      float an = u_f[31];
      vec2 q = vec2(cos(an) * p.x + sin(an) * p.y, -sin(an) * p.x + cos(an) * p.y);
      float rows = q.y / u_vp.y * u_f[25];
      float id = floor(rows);
      float hw = u_f[26] * (0.06 + 0.44 * hash(id + seed)) * 0.5;
      float line = 1.0 - smoothstep(hw * 0.6, hw + 0.02, abs(fract(rows) - 0.5));
      // Trazos sueltos de largo variable que corren a lo largo de la línea.
      float seg = fract(q.x / u_vp.x * (0.4 + 0.8 * hash(id * 2.1 + seed)) + u_f[32] * (0.5 + hash(id * 4.7 + seed)) + hash(id * 9.1 + seed));
      float len = 0.25 + 0.4 * hash(id * 5.3 + seed);
      mask = hash(id * 7.3 + seed) < 0.55 ? 0.0 : line * smoothstep(0.0, 0.08, seg) * (1.0 - smoothstep(len - 0.08, len, seg));
    }
    c = mix(c, vec3(u_f[28]), clamp(mask, 0.0, 1.0) * u_f[29]);
  }
  // Fotograma de impacto: tinta en blanco y negro y negativo.
  if (u_f[34] > 0.0) c = mix(c, vec3(smoothstep(0.42, 0.58, luma(c))), u_f[34]);
  if (u_f[33] > 0.0) c = mix(c, 1.0 - c, u_f[33]);
  // Color: sepia, gris, tinte y contraste.
  if (u_f[35] > 0.0) {
    vec3 sep = vec3(dot(c, vec3(0.393, 0.769, 0.189)), dot(c, vec3(0.349, 0.686, 0.168)), dot(c, vec3(0.272, 0.534, 0.131)));
    c = mix(c, min(sep, 1.0), u_f[35]);
  }
  if (u_f[36] > 0.0) c = mix(c, vec3(luma(c)), u_f[36]);
  if (u_f[40] > 0.0) {
    // Colorear conservando la luminosidad.
    vec3 tc = vec3(u_f[37], u_f[38], u_f[39]);
    c = mix(c, luma(c) * tc / max(luma(tc), 0.1), u_f[40]);
  }
  if (u_f[41] != 0.0) c = (c - 0.5) * (1.0 + u_f[41] * 1.5) + 0.5;
  // Viñeta
  if (u_f[42] > 0.0) {
    float v = length((p / u_vp - 0.5) * vec2(u_vp.x / u_vp.y, 1.0));
    float m = smoothstep(u_f[43] * 0.45, u_f[43] * 0.45 + 0.5, v) * u_f[42];
    c = mix(c, vec3(u_f[44], u_f[45], u_f[46]), m);
  }
  // Destello
  c = mix(c, vec3(u_f[7], u_f[8], u_f[9]), u_f[10]);
  color = vec4(clamp(c, 0.0, 1.0), 1.0);
}`;

function program(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const prog = gl.createProgram()!;
  gl.attachShader(prog, compile(gl, gl.VERTEX_SHADER, vs));
  gl.attachShader(prog, compile(gl, gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog) ?? "link");
  return prog;
}

interface Target {
  fb: WebGLFramebuffer;
  tex: WebGLTexture;
  w: number;
  h: number;
}

function compile(gl: WebGL2RenderingContext, type: number, src: string): WebGLShader {
  const s = gl.createShader(type)!;
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? "shader");
  return s;
}

export class GlViewer {
  private gl: WebGL2RenderingContext;
  private prog: WebGLProgram;
  private loc: Record<string, WebGLUniformLocation | null> = {};
  readonly maxTexture: number;
  private aniso: { ext: EXT_texture_filter_anisotropic; max: number } | null = null;
  /** Destino intermedio para los fundidos (se crea al primer uso). */
  private fbo: { fb: WebGLFramebuffer; tex: WebGLTexture; w: number; h: number } | null = null;
  /** Escena antes de la pasada de efectos, y su programa (se crean al primer uso). */
  private scene: Target | null = null;
  private fxProg: { prog: WebGLProgram; loc: Record<string, WebGLUniformLocation | null> } | null = null;
  /** Color de fondo (RGB 0..1). */
  private bg: [number, number, number] = [0.043, 0.051, 0.063];

  /** Textura de 1×1 del color de fondo (el fondo de la lupa). */
  private bgTex: WebGLTexture | null = null;

  /** Fondo como `#rrggbb`. */
  setBackground(hex: string) {
    const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
    if (m) this.bg = [1, 2, 3].map((i) => parseInt(m[i], 16) / 255) as [number, number, number];
    if (this.bgTex) this.gl.deleteTexture(this.bgTex);
    this.bgTex = null;
  }

  /** Captura para exportar: se pinta en `out` y se convierte a YUV en `yuv`. */
  private out: Target | null = null;
  private yuv: (Target & { prog: WebGLProgram; loc: Record<string, WebGLUniformLocation | null> }) | null = null;

  constructor(private canvas: HTMLCanvasElement | OffscreenCanvas) {
    const gl = canvas.getContext("webgl2", { alpha: false, antialias: false, premultipliedAlpha: true }) as WebGL2RenderingContext | null;
    if (!gl) throw new Error("WebGL2 no disponible");
    this.gl = gl;
    this.maxTexture = gl.getParameter(gl.MAX_TEXTURE_SIZE);
    const ext = gl.getExtension("EXT_texture_filter_anisotropic");
    if (ext) this.aniso = { ext, max: gl.getParameter(ext.MAX_TEXTURE_MAX_ANISOTROPY_EXT) };

    const prog = program(gl, VS, FS);
    this.prog = prog;
    for (const n of ["u_off", "u_size", "u_vp", "u_m", "u_tex", "u_alpha", "u_texel", "u_adj", "u_clip"]) this.loc[n] = gl.getUniformLocation(prog, n);

    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0, 0, 1, 0, 0, 1, 1, 1]), gl.STATIC_DRAW);
    const a = gl.getAttribLocation(prog, "a_unit");
    gl.enableVertexAttribArray(a);
    gl.vertexAttribPointer(a, 2, gl.FLOAT, false, 0, 0);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  }

  /** Sube la imagen como textura(s) con mipmaps; tesela si supera el máximo de la GPU. */
  createSet(pic: Picture): TextureSet {
    const { w, h } = pic;
    const step = w <= this.maxTexture && h <= this.maxTexture ? Math.max(w, h) : Math.min(TILE, this.maxTexture);
    const set: TextureSet = { tiles: [] };
    try {
      for (let y = 0; y < h; y += step) {
        for (let x = 0; x < w; x += step) {
          const tw = Math.min(step, w - x);
          const th = Math.min(step, h - y);
          set.tiles.push({ tex: this.upload(pic, x, y, tw, th), x, y, w: tw, h: th });
        }
      }
      const err = this.gl.getError();
      if (err === this.gl.OUT_OF_MEMORY) throw new TextureError("GL_OUT_OF_MEMORY");
    } catch (e) {
      this.deleteSet(set);
      throw e instanceof TextureError ? e : new TextureError(String(e));
    }
    return set;
  }

  deleteSet(set: TextureSet): void {
    for (const t of set.tiles) this.gl.deleteTexture(t.tex);
    set.tiles = [];
  }

  private upload(pic: Picture, x: number, y: number, w: number, h: number): WebGLTexture {
    const gl = this.gl;
    const tex = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, tex);
    const src = pic.source;
    if (src instanceof HTMLImageElement) {
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
      if (w === pic.w && h === pic.h) {
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, src);
      } else {
        // Recorte vía canvas 2D: más fiable en WebKit que UNPACK_SKIP_* con elementos DOM.
        const c = document.createElement("canvas");
        c.width = w;
        c.height = h;
        c.getContext("2d")!.drawImage(src, x, y, w, h, 0, 0, w, h);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, c);
      }
    } else {
      // RGBA de Rust: ya viene premultiplicado.
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
      gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
      gl.pixelStorei(gl.UNPACK_ROW_LENGTH, pic.w);
      gl.pixelStorei(gl.UNPACK_SKIP_PIXELS, x);
      gl.pixelStorei(gl.UNPACK_SKIP_ROWS, y);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, src.data);
      gl.pixelStorei(gl.UNPACK_ROW_LENGTH, 0);
      gl.pixelStorei(gl.UNPACK_SKIP_PIXELS, 0);
      gl.pixelStorei(gl.UNPACK_SKIP_ROWS, 0);
    }
    gl.generateMipmap(gl.TEXTURE_2D);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    if (this.aniso) gl.texParameterf(gl.TEXTURE_2D, this.aniso.ext.TEXTURE_MAX_ANISOTROPY_EXT, Math.min(8, this.aniso.max));
    return tex;
  }

  /** Ajusta el canvas al tamaño CSS × devicePixelRatio. Devuelve el viewport en px CSS. */
  resize(): Size {
    const dpr = window.devicePixelRatio || 1;
    const { clientWidth: w, clientHeight: h } = this.canvas as HTMLCanvasElement;
    const pw = Math.max(1, Math.round(w * dpr));
    const ph = Math.max(1, Math.round(h * dpr));
    if (this.canvas.width !== pw || this.canvas.height !== ph) {
      this.canvas.width = pw;
      this.canvas.height = ph;
    }
    return { w, h };
  }

  private drawSet(set: TextureSet, m: Affine, alpha: number, vp: Size, adj: ArrayLike<number> | null = null): void {
    const gl = this.gl;
    gl.uniform2f(this.loc.u_vp, vp.w, vp.h);
    // mat3 por columnas: [a b 0 | c d 0 | e f 1]
    gl.uniformMatrix3fv(this.loc.u_m, false, [m[0], m[1], 0, m[2], m[3], 0, m[4], m[5], 1]);
    gl.uniform1f(this.loc.u_alpha, alpha);
    gl.uniform1fv(this.loc.u_adj, adj && adj.length >= ADJ_LEN && adj[0] > 0 ? Float32Array.from(adj as ArrayLike<number>).subarray(0, ADJ_LEN) : NO_ADJ);
    for (const t of set.tiles) {
      gl.bindTexture(gl.TEXTURE_2D, t.tex);
      gl.uniform2f(this.loc.u_off, t.x, t.y);
      gl.uniform2f(this.loc.u_size, t.w, t.h);
      gl.uniform2f(this.loc.u_texel, 1 / Math.abs(t.w), 1 / Math.abs(t.h));
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    }
  }

  private target(): WebGLFramebuffer {
    const gl = this.gl;
    const { w, h } = this.size();
    if (this.fbo && (this.fbo.w !== w || this.fbo.h !== h)) {
      gl.deleteFramebuffer(this.fbo.fb);
      gl.deleteTexture(this.fbo.tex);
      this.fbo = null;
    }
    if (!this.fbo) {
      const tex = gl.createTexture()!;
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      const fb = gl.createFramebuffer()!;
      gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
      this.fbo = { fb, tex, w, h };
    }
    return this.fbo.fb;
  }

  /**
   * Pinta las capas en orden. Con `mix`, la segunda se mezcla con la primera usando su alfa
   * como peso (fundido real: se pinta aparte con su fondo y se compone encima). `fx`: efectos
   * especiales de ese instante (temblor, destello…); sin ellos el camino es el de siempre.
   */
  draw(layers: Layer[], vp: Size, mix = false, fx: FxFrame | null = null, lens: Lens | null = null): void {
    const on = !!fx && fx.length >= FX_LEN;
    if (on) {
      // Temblor y golpe de zoom: un movimiento de pantalla después de la cámara.
      const cam = Array.prototype.slice.call(fx, 1, 7) as number[];
      const identity = cam[0] === 1 && cam[1] === 0 && cam[2] === 0 && cam[3] === 1 && cam[4] === 0 && cam[5] === 0;
      if (!identity) layers = layers.map((l) => ({ ...l, m: then(l.m, cam) }));
    }
    const post = on && fx![0] > 0;
    const final = this.out?.fb ?? null;
    this.drawLayers(layers, vp, mix, post ? this.sceneTarget().fb : final);
    if (post) this.applyFx(fx!, vp, final);
    else if (lens && !mix) this.drawLens(layers, vp, lens);
  }

  /** La lupa: el fondo y las capas otra vez, ampliadas alrededor del punto y recortadas al círculo. */
  private drawLens(layers: Layer[], vp: Size, lens: Lens): void {
    const gl = this.gl;
    const size = this.size();
    const k = size.w / Math.max(1, vp.w);
    gl.uniform3f(this.loc.u_clip, lens.x * k, size.h - lens.y * k, lens.r * k);
    if (!this.bgTex) {
      this.bgTex = gl.createTexture()!;
      gl.bindTexture(gl.TEXTURE_2D, this.bgTex);
      gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([...this.bg.map((c) => Math.round(c * 255)), 255]));
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    }
    const r = lens.r;
    gl.uniform2f(this.loc.u_vp, vp.w, vp.h);
    gl.uniformMatrix3fv(this.loc.u_m, false, [2 * r, 0, 0, 0, 2 * r, 0, lens.x - r, lens.y - r, 1]);
    gl.uniform1f(this.loc.u_alpha, 1);
    gl.uniform1fv(this.loc.u_adj, NO_ADJ);
    gl.bindTexture(gl.TEXTURE_2D, this.bgTex);
    gl.uniform2f(this.loc.u_off, 0, 0);
    gl.uniform2f(this.loc.u_size, 1, 1);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    const z = lens.zoom;
    const around = [z, 0, 0, z, lens.x * (1 - z), lens.y * (1 - z)];
    for (const l of layers) if (l.alpha > 0 && l.set.tiles.length) this.drawSet(l.set, then(l.m, around), l.alpha, vp, l.adj);
    gl.uniform3f(this.loc.u_clip, 0, 0, 0);
  }

  private drawLayers(layers: Layer[], vp: Size, mix: boolean, dest: WebGLFramebuffer | null): void {
    const gl = this.gl;
    const size = this.size();
    gl.useProgram(this.prog);
    gl.uniform1i(this.loc.u_tex, 0);
    gl.uniform3f(this.loc.u_clip, 0, 0, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, dest);
    gl.viewport(0, 0, size.w, size.h);
    const visible = layers.filter((l) => l.alpha > 0 && l.set.tiles.length);
    if (mix && visible.length === 2) {
      const [a, b] = visible;
      gl.bindFramebuffer(gl.FRAMEBUFFER, this.target());
      gl.clearColor(this.bg[0], this.bg[1], this.bg[2], 1);
      gl.clear(gl.COLOR_BUFFER_BIT);
      this.drawSet(b.set, b.m, 1, vp, b.adj);
      gl.bindFramebuffer(gl.FRAMEBUFFER, dest);
      gl.clear(gl.COLOR_BUFFER_BIT);
      this.drawSet(a.set, a.m, 1, vp, a.adj);
      // El destino intermedio como un rectángulo de pantalla completa (invertido en Y: GL
      // guarda las texturas de abajo arriba).
      gl.uniform2f(this.loc.u_vp, vp.w, vp.h);
      gl.uniformMatrix3fv(this.loc.u_m, false, [1, 0, 0, 0, 1, 0, 0, 0, 1]);
      gl.uniform1f(this.loc.u_alpha, b.alpha);
      gl.uniform1fv(this.loc.u_adj, NO_ADJ);
      gl.bindTexture(gl.TEXTURE_2D, this.fbo!.tex);
      gl.uniform2f(this.loc.u_off, 0, vp.h);
      gl.uniform2f(this.loc.u_size, vp.w, -vp.h);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      return;
    }
    gl.clearColor(this.bg[0], this.bg[1], this.bg[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    for (const l of visible) this.drawSet(l.set, l.m, l.alpha, vp, l.adj);
  }

  /** Tamaño en píxeles del destino de dibujo (la captura, si la hay, o el canvas). */
  private size(): Size {
    return this.out ? { w: this.out.w, h: this.out.h } : { w: this.canvas.width, h: this.canvas.height };
  }

  /** Destino donde se pinta la escena cuando luego hay pasada de efectos (del tamaño de salida). */
  private sceneTarget(): Target {
    const { w, h } = this.size();
    if (this.scene && (this.scene.w !== w || this.scene.h !== h)) {
      this.dropTarget(this.scene);
      this.scene = null;
    }
    this.scene ??= this.makeTarget(w, h, this.gl.LINEAR);
    return this.scene;
  }

  private applyFx(fx: FxFrame, vp: Size, dest: WebGLFramebuffer | null): void {
    const gl = this.gl;
    if (!this.fxProg) {
      const prog = program(gl, YUV_VS, FX_FS);
      const loc: Record<string, WebGLUniformLocation | null> = {};
      for (const n of ["u_src", "u_res", "u_vp", "u_f"]) loc[n] = gl.getUniformLocation(prog, n);
      this.fxProg = { prog, loc };
    }
    const { prog, loc } = this.fxProg;
    const size = this.size();
    gl.bindFramebuffer(gl.FRAMEBUFFER, dest);
    gl.viewport(0, 0, size.w, size.h);
    gl.disable(gl.BLEND);
    gl.useProgram(prog);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.scene!.tex);
    gl.uniform1i(loc.u_src, 0);
    gl.uniform2f(loc.u_res, size.w, size.h);
    gl.uniform2f(loc.u_vp, vp.w, vp.h);
    gl.uniform1fv(loc.u_f, Float32Array.from({ length: FX_LEN }, (_, i) => fx[i]));
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.enable(gl.BLEND);
  }

  private makeTarget(w: number, h: number, filter: number): Target {
    const gl = this.gl;
    const tex = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    const fb = gl.createFramebuffer()!;
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
    if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) throw new TextureError(`destino ${w}×${h} no disponible`);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return { fb, tex, w, h };
  }

  private dropTarget(t: Target | null) {
    if (!t) return;
    this.gl.deleteFramebuffer(t.fb);
    this.gl.deleteTexture(t.tex);
  }

  /**
   * A partir de ahora se pinta en un destino de `w`×`h` píxeles (múltiplos de 4) para leerlo
   * con `readYuv`. `null` vuelve a pintar en el canvas.
   */
  capture(size: Size | null, yuv = true) {
    this.dropTarget(this.out);
    this.out = null;
    if (this.yuv) {
      this.dropTarget(this.yuv);
      this.gl.deleteProgram(this.yuv.prog);
      this.yuv = null;
    }
    if (!size) return;
    if (size.w % 4 || size.h % 4) throw new Error(`tamaño de vídeo no válido: ${size.w}×${size.h}`);
    this.out = this.makeTarget(size.w, size.h, this.gl.NEAREST);
    if (!yuv) return;
    const prog = program(this.gl, YUV_VS, YUV_FS);
    const loc: Record<string, WebGLUniformLocation | null> = {};
    for (const n of ["u_src", "u_w", "u_h"]) loc[n] = this.gl.getUniformLocation(prog, n);
    this.yuv = { ...this.makeTarget(size.w / 4, (size.h * 3) / 2, this.gl.NEAREST), prog, loc };
  }

  /** Bytes de un fotograma YUV 4:2:0 (W·H·1.5). */
  readYuv(buf: Uint8Array): void {
    const gl = this.gl;
    const out = this.out;
    const y = this.yuv;
    if (!out || !y) throw new Error("no hay captura activa");
    gl.bindFramebuffer(gl.FRAMEBUFFER, y.fb);
    gl.viewport(0, 0, y.w, y.h);
    gl.disable(gl.BLEND);
    gl.useProgram(y.prog);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, out.tex);
    gl.uniform1i(y.loc.u_src, 0);
    gl.uniform1i(y.loc.u_w, out.w);
    gl.uniform1i(y.loc.u_h, out.h);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.readPixels(0, 0, y.w, y.h, gl.RGBA, gl.UNSIGNED_BYTE, buf);
    gl.enable(gl.BLEND);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  }

  /** Píxeles RGBA de la captura (filas de arriba abajo), para guardarla como imagen. */
  readRgba(): ImageData {
    const gl = this.gl;
    const out = this.out;
    if (!out) throw new Error("no hay captura activa");
    const raw = new Uint8ClampedArray(out.w * out.h * 4);
    gl.bindFramebuffer(gl.FRAMEBUFFER, out.fb);
    gl.readPixels(0, 0, out.w, out.h, gl.RGBA, gl.UNSIGNED_BYTE, raw);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    // GL lee de abajo arriba.
    const row = out.w * 4;
    const flipped = new Uint8ClampedArray(raw.length);
    for (let y = 0; y < out.h; y++) flipped.set(raw.subarray((out.h - 1 - y) * row, (out.h - y) * row), y * row);
    return new ImageData(flipped, out.w, out.h);
  }

  /** Libera la GPU (contextos de exportación). */
  dispose() {
    this.capture(null);
    this.dropTarget(this.fbo);
    this.fbo = null;
    this.dropTarget(this.scene);
    this.scene = null;
    if (this.bgTex) this.gl.deleteTexture(this.bgTex);
    this.bgTex = null;
    if (this.fxProg) this.gl.deleteProgram(this.fxProg.prog);
    this.fxProg = null;
    this.gl.getExtension("WEBGL_lose_context")?.loseContext();
  }
}
