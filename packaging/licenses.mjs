// Genera la lista de licencias de terceros (pnpm third-party):
//   - ui/src/lib/about/licenses.json  → la vista «Licencias de terceros» del «Acerca de».
//   - THIRD-PARTY.md                  → el mismo contenido en texto, que va dentro de los paquetes.
//
// Rust: cargo-about (cargo install cargo-about --locked --features cli), configurado en about.toml.
// JavaScript: la opción build.license de Vite (solo lo que acaba dentro de la interfaz).
// A mano: packaging/licenses-extra.json (UnRAR, libheif, FFmpeg…).
//
// `node packaging/licenses.mjs --check` solo comprueba que la lista corresponde a los Cargo.lock y
// pnpm-lock.yaml actuales (lo usa dist.sh). La misma comprobación está en src-tauri/tests/licenses.rs.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const OUT_JSON = join(root, "ui/src/lib/about/licenses.json");
const OUT_MD = join(root, "THIRD-PARTY.md");
const OWN = /^imgx/; // crates y paquetes propios: no son de terceros

/** Huella de las dependencias de terceros (FNV-1a de 64 bits). Igual que en src-tauri/tests/licenses.rs. */
export function lockHash() {
  const cargo = readFileSync(join(root, "Cargo.lock"), "utf8").replace(/\r/g, "");
  const pkgs = [...cargo.matchAll(/\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"/g)]
    .filter((m) => !OWN.test(m[1]))
    .map((m) => `${m[1]} ${m[2]}`)
    .sort();
  const pnpm = readFileSync(join(root, "pnpm-lock.yaml"), "utf8").replace(/\r/g, "");
  const input = new TextEncoder().encode(pkgs.join("\n") + "\n--\n" + pnpm);
  let h = 0xcbf29ce484222325n;
  for (const b of input) h = BigInt.asUintN(64, (h ^ BigInt(b)) * 0x100000001b3n);
  return h.toString(16).padStart(16, "0");
}

if (process.argv.includes("--check")) {
  const saved = existsSync(OUT_JSON) ? JSON.parse(readFileSync(OUT_JSON, "utf8")).lockHash : null;
  if (saved !== lockHash()) {
    console.error("La lista de licencias de terceros no corresponde a las dependencias actuales: ejecuta «pnpm third-party».");
    process.exit(1);
  }
  process.exit(0);
}

const cleanAuthor = (a) => a.replace(/\s*<[^>]*>/g, "").replace(/\s*\([^)]*\)$/, "").trim();
const cleanUrl = (u) =>
  u ? String(u).replace(/^git\+/, "").replace(/^git:\/\//, "https://").replace(/\.git$/, "").replace(/^github:/, "https://github.com/") : null;

// Textos sin repetir: misma licencia con distinto espaciado cuenta como el mismo texto.
const texts = [];
const textIndex = new Map();
function addText(t) {
  const text = t.replace(/\r/g, "").trim();
  const key = text.replace(/\s+/g, " ");
  if (!textIndex.has(key)) {
    textIndex.set(key, texts.length);
    texts.push(text);
  }
  return textIndex.get(key);
}

// ── Rust ──
function rust() {
  let json;
  try {
    json = execFileSync("cargo", ["about", "generate", "--format", "json", "--locked", "--fail"], {
      cwd: root,
      maxBuffer: 256 << 20,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "inherit"],
    });
  } catch (e) {
    console.error("\nNo se pudo ejecutar cargo-about. Instálalo con:\n  cargo install cargo-about --locked --features cli\n");
    throw e;
  }
  const about = JSON.parse(json);
  const byId = new Map();
  for (const lic of about.licenses) {
    const idx = addText(lic.text);
    for (const { crate: c } of lic.used_by) {
      if (OWN.test(c.name)) continue;
      const id = `${c.name}@${c.version}`;
      let p = byId.get(id);
      if (!p) {
        p = {
          name: c.name,
          version: c.version,
          authors: (c.authors ?? []).map(cleanAuthor).filter(Boolean),
          license: c.license ?? lic.id,
          url: cleanUrl(c.repository) ?? cleanUrl(c.homepage) ?? `https://crates.io/crates/${c.name}`,
          from: "rust",
          texts: [],
        };
        byId.set(id, p);
      }
      if (!p.texts.includes(idx)) p.texts.push(idx);
    }
  }
  return [...byId.values()];
}

// ── JavaScript (solo lo que Vite mete en la interfaz) ──
async function js() {
  const { build } = await import("vite");
  const out = mkdtempSync(join(tmpdir(), "imgx-licenses-"));
  try {
    await build({
      configFile: join(root, "ui/vite.config.ts"),
      logLevel: "warn",
      build: { outDir: out, emptyOutDir: true, license: { fileName: "licenses.json" } },
    });
    const list = JSON.parse(readFileSync(join(out, "licenses.json"), "utf8"));
    return list
      .filter((l) => !OWN.test(l.name))
      .map((l) => {
        let pkg = {};
        try {
          pkg = JSON.parse(readFileSync(join(root, "node_modules", l.name, "package.json"), "utf8"));
        } catch {
          /* sin package.json legible: nos quedamos con lo que da Vite */
        }
        const author = typeof pkg.author === "string" ? pkg.author : pkg.author?.name;
        return {
          name: l.name,
          version: l.version,
          authors: author ? [cleanAuthor(author)] : [],
          license: l.identifier ?? pkg.license ?? "?",
          url: cleanUrl(typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url) ?? cleanUrl(pkg.homepage) ?? `https://www.npmjs.com/package/${l.name}`,
          from: "js",
          texts: l.text ? [addText(l.text)] : [],
        };
      });
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}

// ── A mano: avisos que las herramientas no conocen ──
function extra() {
  const list = JSON.parse(readFileSync(join(root, "packaging/licenses-extra.json"), "utf8"));
  return list.map((e) => {
    let text = null;
    if (e.text === "unrar") {
      // El texto oficial viene dentro del crate unrar_sys.
      const meta = JSON.parse(execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], { cwd: root, maxBuffer: 256 << 20, encoding: "utf8" }));
      const sys = meta.packages.find((p) => p.name === "unrar_sys");
      if (!sys) throw new Error("unrar_sys no está entre las dependencias");
      text = readFileSync(join(dirname(sys.manifest_path), "vendor/unrar/license.txt"), "latin1");
    } else if (e.text) {
      text = readFileSync(join(root, e.text), "utf8");
    }
    return { name: e.name, authors: e.authors, license: e.license, url: e.url, note: e.note, texts: text ? [addText(text)] : [] };
  });
}

// La interfaz importa licenses.json: la primera vez tiene que existir para poder compilarla.
if (!existsSync(OUT_JSON)) {
  mkdirSync(dirname(OUT_JSON), { recursive: true });
  writeFileSync(OUT_JSON, '{"lockHash":"","special":[],"packages":[],"texts":[]}\n');
}

const special = extra();
const packages = [...rust(), ...(await js())].sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
const data = { lockHash: lockHash(), special, packages, texts };
writeFileSync(OUT_JSON, JSON.stringify(data) + "\n");

// ── THIRD-PARTY.md ──
const md = [
  "# Licencias de terceros · Third-party licenses",
  "",
  "imgxplorer (MIT, © 2026 Pedro López del Castillo) usa el trabajo de estas personas y proyectos.",
  "imgxplorer (MIT) is built on the work of these people and projects.",
  "",
  "Generado con `pnpm third-party`; no editar a mano.",
  "",
  "## Avisos especiales · Special notices",
  "",
];
for (const s of special) {
  md.push(`### ${s.name}`, "", `- ${s.license} · ${s.url}`, `- ${s.authors.join(", ")}`, `- ${s.note.es}`, `- ${s.note.en}`, "");
  for (const i of s.texts) md.push("```text", texts[i], "```", "");
}
md.push("## Bibliotecas · Libraries", "");
const users = texts.map(() => []);
for (const p of packages) for (const i of p.texts) users[i].push(`${p.name} ${p.version}`);
const done = new Set(special.flatMap((s) => s.texts));
texts.forEach((t, i) => {
  if (done.has(i) || !users[i].length) return;
  md.push(`### ${users[i].join(", ")}`, "", "```text", t, "```", "");
});
md.push("## Índice · Index", "", "| Biblioteca | Versión | Licencia | Origen |", "|---|---|---|---|");
for (const p of packages) md.push(`| [${p.name}](${p.url}) | ${p.version} | ${p.license} | ${p.from === "rust" ? "Rust" : "JavaScript"} |`);
writeFileSync(OUT_MD, md.join("\n") + "\n");

console.log(`${packages.length} bibliotecas, ${special.length} avisos especiales, ${texts.length} textos de licencia.`);
