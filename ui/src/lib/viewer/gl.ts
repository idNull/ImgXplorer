// Renderizador WebGL2: un quad con la imagen, transformado por la matriz de cámara.
import type { Affine, Size } from "../core";

const VS = `#version 300 es
in vec2 a_unit;              // 0..1
uniform vec2 u_img;          // tamaño de imagen (px)
uniform vec2 u_vp;           // tamaño de viewport (px CSS)
uniform mat3 u_m;            // imagen → viewport
out vec2 v_uv;
void main() {
  vec2 p = (u_m * vec3(a_unit * u_img, 1.0)).xy;
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
  private tex: WebGLTexture | null = null;
  private loc: Record<string, WebGLUniformLocation | null> = {};
  readonly maxTexture: number;
  private aniso: { ext: EXT_texture_filter_anisotropic; max: number } | null = null;

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
    for (const n of ["u_img", "u_vp", "u_m", "u_tex", "u_alpha"]) this.loc[n] = gl.getUniformLocation(prog, n);

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

  /** Sube la imagen como textura con mipmaps. Devuelve false si excede el tamaño máximo. */
  setImage(source: TexImageSource, size: Size): boolean {
    const gl = this.gl;
    if (this.tex) gl.deleteTexture(this.tex);
    this.tex = null;
    if (size.w > this.maxTexture || size.h > this.maxTexture) return false; // TODO fase 1: teselado
    const tex = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
    gl.generateMipmap(gl.TEXTURE_2D);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    if (this.aniso) gl.texParameterf(gl.TEXTURE_2D, this.aniso.ext.TEXTURE_MAX_ANISOTROPY_EXT, Math.min(8, this.aniso.max));
    this.tex = tex;
    return true;
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

  draw(m: Affine, img: Size, vp: Size): void {
    const gl = this.gl;
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(0.043, 0.051, 0.063, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    if (!this.tex) return;
    gl.useProgram(this.prog);
    gl.bindTexture(gl.TEXTURE_2D, this.tex);
    gl.uniform2f(this.loc.u_img, img.w, img.h);
    gl.uniform2f(this.loc.u_vp, vp.w, vp.h);
    // mat3 por columnas: [a b 0 | c d 0 | e f 1]
    gl.uniformMatrix3fv(this.loc.u_m, false, [m[0], m[1], 0, m[2], m[3], 0, m[4], m[5], 1]);
    gl.uniform1i(this.loc.u_tex, 0);
    gl.uniform1f(this.loc.u_alpha, 1);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }
}
