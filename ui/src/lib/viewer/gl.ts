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

const BG = [0.043, 0.051, 0.063] as const;

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

  constructor(private canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl2", { alpha: false, antialias: false, premultipliedAlpha: true });
    if (!gl) throw new Error("WebGL2 no disponible");
    this.gl = gl;
    this.maxTexture = gl.getParameter(gl.MAX_TEXTURE_SIZE);
    const ext = gl.getExtension("EXT_texture_filter_anisotropic");
    if (ext) this.aniso = { ext, max: gl.getParameter(ext.MAX_TEXTURE_MAX_ANISOTROPY_EXT) };

    const prog = gl.createProgram()!;
    gl.attachShader(prog, compile(gl, gl.VERTEX_SHADER, VS));
    gl.attachShader(prog, compile(gl, gl.FRAGMENT_SHADER, FS));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog) ?? "link");
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
    const { clientWidth: w, clientHeight: h } = this.canvas;
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
    const { width: w, height: h } = this.canvas;
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
    gl.useProgram(this.prog);
    gl.uniform1i(this.loc.u_tex, 0);
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    const visible = layers.filter((l) => l.alpha > 0 && l.set.tiles.length);
    if (mix && visible.length === 2) {
      const [a, b] = visible;
      gl.bindFramebuffer(gl.FRAMEBUFFER, this.target());
      gl.clearColor(BG[0], BG[1], BG[2], 1);
      gl.clear(gl.COLOR_BUFFER_BIT);
      this.drawSet(b.set, b.m, 1, vp);
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
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
    gl.clearColor(BG[0], BG[1], BG[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    for (const l of visible) this.drawSet(l.set, l.m, l.alpha, vp);
  }
}
