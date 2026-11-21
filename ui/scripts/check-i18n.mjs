// Comprueba que los dos idiomas están completos: las mismas claves, los mismos {huecos} y nada sin
// traducir. Se ejecuta con `pnpm check` (y suelto: `node ui/scripts/check-i18n.mjs`).
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const file = fileURLToPath(new URL("../src/lib/i18n.svelte.ts", import.meta.url));
const src = readFileSync(file, "utf8");

/** El objeto literal que sigue a `start` (hasta su llave de cierre), evaluado. */
function literal(start) {
  const i = src.indexOf(start);
  if (i < 0) throw new Error(`no se encontró «${start}»`);
  const open = src.indexOf("{", i + start.length - 1);
  let depth = 0;
  let quote = null;
  for (let j = open; j < src.length; j++) {
    const c = src[j];
    if (quote) {
      if (c === "\\") j++;
      else if (c === quote) quote = null;
    } else if (c === '"' || c === "'" || c === "`") quote = c;
    else if (c === "{") depth++;
    else if (c === "}" && --depth === 0) return new Function(`return ${src.slice(open, j + 1)}`)();
  }
  throw new Error(`«${start}» sin cerrar`);
}

// Textos que se escriben igual en los dos idiomas a propósito: símbolos y números, y estas claves
// (palabras que son iguales en español e inglés). Una clave nueva igual en los dos idiomas tiene
// que añadirse aquí a conciencia.
const SYMBOLS = /^[\s\p{P}\p{S}\d]*$/u;
const SAME = new Set([
  "general", "editor", "soundFilter", "clipDefault", "libClips", "adjPreset_original", "saveImgOriginal", "mPinch",
  "vfx.color", "vfxp.focus-lines.shock", "vfxp.color.flashback", "vfxk.color", "vfxo.x", "vfxo.y", "vfxs.flashback",
  "fx.reverb", "fx.robot", "fxp.delay.pingpong", "fxp.filter.radio", "fxp.robot.robot", "fxp.robot.alien",
  "fxk.delay.pingpong", "fxk.robot.metal", "tpl.ken-burns", "ease.cubic-bezier",
  "pose_cx", "pose_cy", "pose_zoom", "packError", "qualityNormal", "aboutApp", "adjPreset_sepia", "readRtl", "exif_iso", "exif_flash",
  "vfxp.chroma.glitch", "vfxk.glitch", "vfxo.sepia", "p.zoom",
]);

let errors = 0;
const fail = (msg) => {
  errors++;
  console.error(`✗ ${msg}`);
};
const holes = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(",");

for (const [name, obj] of [["dict", literal("const dict = {")], ["labels", literal("const labels: Record<\"es\" | \"en\", Record<string, string>> = {")]]) {
  const { es, en } = obj;
  for (const k of Object.keys(es)) if (!(k in en)) fail(`${name}: falta «${k}» en inglés`);
  for (const k of Object.keys(en)) if (!(k in es)) fail(`${name}: falta «${k}» en español`);
  for (const k of Object.keys(es)) {
    if (!(k in en)) continue;
    if (holes(es[k]) !== holes(en[k])) fail(`${name}: «${k}» tiene huecos distintos: {${holes(es[k])}} / {${holes(en[k])}}`);
    if (es[k] === en[k] && !SYMBOLS.test(es[k]) && !SAME.has(k)) fail(`${name}: «${k}» parece sin traducir: ${JSON.stringify(en[k])}`);
  }
  console.log(`${name}: ${Object.keys(es).length} textos en español y ${Object.keys(en).length} en inglés`);
}
// Cada herramienta de la IA tiene su etiqueta (la que se ve en el aviso «🤖 …»).
const tools = JSON.parse(readFileSync(fileURLToPath(new URL("../../src-tauri/src/mcp/tools.json", import.meta.url)), "utf8"));
const labels = literal("const labels: Record<\"es\" | \"en\", Record<string, string>> = {");
for (const { name } of tools) for (const l of ["es", "en"]) if (!labels[l][`mcp.${name}`]) fail(`labels: falta «mcp.${name}» (${l})`);
console.log(`herramientas de la IA: ${tools.length} con su etiqueta`);

if (errors) {
  console.error(`${errors} problemas en los idiomas`);
  process.exit(1);
}
