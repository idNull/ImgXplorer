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
out vec4 color;
void main() { color = texture(u_tex, v_uv) * u_alpha; }`;

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
  /** Color de fondo (RGB 0..1). */
  private bg: [number, number, number] = [0.043, 0.051, 0.063];

  /** Fondo como `#rrggbb`. */
  setBackground(hex: string) {
    const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
    if (m) this.bg = [1, 2, 3].map((i) => parseInt(m[i], 16) / 255) as [number, number, number];
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
    for (const n of ["u_off", "u_size", "u_vp", "u_m", "u_tex", "u_alpha"]) this.loc[n] = gl.getUniformLocation(prog, n);

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

  private drawSet(set: TextureSet, m: Affine, alpha: number, vp: Size): void {
    const gl = this.gl;
    gl.uniform2f(this.loc.u_vp, vp.w, vp.h);
    // mat3 por columnas: [a b 0 | c d 0 | e f 1]
    gl.uniformMatrix3fv(this.loc.u_m, false, [m[0], m[1], 0, m[2], m[3], 0, m[4], m[5], 1]);
    gl.uniform1f(this.loc.u_alpha, alpha);
    for (const t of set.tiles) {
      gl.bindTexture(gl.TEXTURE_2D, t.tex);
      gl.uniform2f(this.loc.u_off, t.x, t.y);
      gl.uniform2f(this.loc.u_size, t.w, t.h);
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
   * como peso (fundido real: se pinta aparte con su fondo y se compone encima).
   */
  draw(layers: Layer[], vp: Size, mix = false): void {
    const gl = this.gl;
    const dest = this.out?.fb ?? null;
    const size = this.size();
    gl.useProgram(this.prog);
    gl.uniform1i(this.loc.u_tex, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, dest);
    gl.viewport(0, 0, size.w, size.h);
    const visible = layers.filter((l) => l.alpha > 0 && l.set.tiles.length);
    if (mix && visible.length === 2) {
      const [a, b] = visible;
      gl.bindFramebuffer(gl.FRAMEBUFFER, this.target());
      gl.clearColor(this.bg[0], this.bg[1], this.bg[2], 1);
      gl.clear(gl.COLOR_BUFFER_BIT);
      this.drawSet(b.set, b.m, 1, vp);
      gl.bindFramebuffer(gl.FRAMEBUFFER, dest);
      gl.clear(gl.COLOR_BUFFER_BIT);
      this.drawSet(a.set, a.m, 1, vp);
      // El destino intermedio como un rectángulo de pantalla completa (invertido en Y: GL
      // guarda las texturas de abajo arriba).
      gl.uniform2f(this.loc.u_vp, vp.w, vp.h);
      gl.uniformMatrix3fv(this.loc.u_m, false, [1, 0, 0, 0, 1, 0, 0, 0, 1]);
      gl.uniform1f(this.loc.u_alpha, b.alpha);
      gl.bindTexture(gl.TEXTURE_2D, this.fbo!.tex);
      gl.uniform2f(this.loc.u_off, 0, vp.h);
      gl.uniform2f(this.loc.u_size, vp.w, -vp.h);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      return;
    }
    gl.clearColor(this.bg[0], this.bg[1], this.bg[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    for (const l of visible) this.drawSet(l.set, l.m, l.alpha, vp);
  }

  /** Tamaño en píxeles del destino de dibujo (la captura, si la hay, o el canvas). */
  private size(): Size {
    return this.out ? { w: this.out.w, h: this.out.h } : { w: this.canvas.width, h: this.canvas.height };
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
    this.gl.getExtension("WEBGL_lose_context")?.loseContext();
  }
}
