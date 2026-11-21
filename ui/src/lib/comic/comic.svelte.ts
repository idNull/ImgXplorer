// Cómics propios: lo que comparten el diálogo de crear y la mesa de luz.
import { invoke } from "@tauri-apps/api/core";
import { prefs } from "../prefs.svelte";

/** Una página en la mesa: de la fuente abierta (`index`) o una imagen de fuera (`path`). */
export interface Tile {
  key: string;
  name: string;
  index?: number;
  path?: string;
}

/** Cómo se guarda el orden: las de la fuente por su nombre o índice, las nuevas por su ruta. */
export type ArrangePage = { entry: string } | { path: string };

let seq = 0;
export const newKey = () => `k${++seq}`;

export const baseName = (p: string) => p.slice(Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\")) + 1);

/** Sin el número que pone imgxplorer delante (`0003 - x.jpg` → `x.jpg`). */
export const stripNumber = (n: string) => n.match(/^\d+ - (.+)$/)?.[1] ?? n;

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
export const byName = (a: Tile, b: Tile) => collator.compare(stripNumber(baseName(a.name)), stripNumber(baseName(b.name)));

export const fromFiles = (paths: string[]): Tile[] => paths.map((path) => ({ key: newKey(), name: baseName(path), path }));

/** Mueve las páginas `keys` (en su orden) para que queden delante de la posición `to` de `tiles`. */
export function moveKeys(tiles: Tile[], keys: string[], to: number): Tile[] {
  const set = new Set(keys);
  const moving = tiles.filter((t) => set.has(t.key));
  const before = tiles.slice(0, to).filter((t) => !set.has(t.key)).length;
  const rest = tiles.filter((t) => !set.has(t.key));
  rest.splice(before, 0, ...moving);
  return rest;
}

// Miniaturas de imágenes de fuera (las de la fuente abierta las sirve el protocolo `imgx`).
const thumbs = new Map<string, Promise<string>>();
export function fileThumb(path: string): Promise<string> {
  let p = thumbs.get(path);
  if (!p) {
    p = invoke<string>("thumb_file", { path }).catch(() => "");
    thumbs.set(path, p);
    if (thumbs.size > 400) thumbs.delete(thumbs.keys().next().value!);
  }
  return p;
}

/** Carpeta donde se crean los cómics: la de Ajustes o `Imágenes/imgxplorer`. */
export async function comicsDir(): Promise<string> {
  if (prefs.settings.comicsDir) return prefs.settings.comicsDir;
  return (await invoke<string | null>("comic_default_dir").catch(() => null)) ?? "";
}
