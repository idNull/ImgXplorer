// Abrir y guardar archivos. Por defecto, en Linux se usa el selector propio de la app: en
// algunos escritorios (Hyprland/Omarchy…) el diálogo GTK sale sin el botón de Aceptar. En
// macOS se usa el del sistema. Se puede cambiar en Ajustes.
import { open, save } from "@tauri-apps/plugin-dialog";
import { prefs } from "../prefs.svelte";

export interface Filter {
  name: string;
  extensions: string[];
}

export type PickMode = "open" | "folder" | "save";

export interface PickRequest {
  mode: PickMode;
  title?: string;
  filters?: Filter[];
  /** Nombre propuesto al guardar. */
  defaultName?: string;
  /** Carpeta en la que empezar (si no, la última visitada). */
  dir?: string;
  /** Abrir: se pueden elegir varios archivos (Ctrl/Mayús+clic, Ctrl+A). */
  multiple?: boolean;
}

class FileDialogState {
  request = $state<PickRequest | null>(null);
  #resolve: ((path: string | string[] | null) => void) | null = null;

  pick(req: PickRequest & { multiple: true }): Promise<string[] | null>;
  pick(req: PickRequest): Promise<string | null>;
  pick(req: PickRequest): Promise<string | string[] | null> {
    this.finish(null); // otra petición pendiente se cancela
    this.request = req;
    return new Promise((r) => (this.#resolve = r));
  }

  finish(path: string | string[] | null) {
    const r = this.#resolve;
    this.#resolve = null;
    this.request = null;
    r?.(path);
  }
}

export const fileDialog = new FileDialogState();

const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent);

export function useSystemDialog(): boolean {
  const m = prefs.settings.fileDialog;
  return m === "system" || (m === "auto" && isMac);
}

export function dirOf(path: string): string {
  const i = path.lastIndexOf("/");
  return i > 0 ? path.slice(0, i) : "/";
}

export function joinPath(dir: string, name: string): string {
  if (name.startsWith("/")) return name;
  return dir.endsWith("/") ? dir + name : `${dir}/${name}`;
}

/** Elige un archivo para abrir. */
export async function openFile(o: { title?: string; filters?: Filter[]; dir?: string } = {}): Promise<string | null> {
  if (useSystemDialog()) {
    const r = await open({ multiple: false, filters: o.filters, title: o.title, defaultPath: o.dir });
    return typeof r === "string" ? r : null;
  }
  return fileDialog.pick({ mode: "open", ...o });
}

/** Elige uno o varios archivos para abrir. */
export async function openFiles(o: { title?: string; filters?: Filter[]; dir?: string } = {}): Promise<string[] | null> {
  if (useSystemDialog()) {
    const r = await open({ multiple: true, filters: o.filters, title: o.title, defaultPath: o.dir });
    if (!r) return null;
    const list = Array.isArray(r) ? r : [r];
    return list.length ? list : null;
  }
  return fileDialog.pick({ mode: "open", multiple: true, ...o });
}

/** Elige una carpeta. */
export async function openFolder(o: { title?: string; dir?: string } = {}): Promise<string | null> {
  if (useSystemDialog()) {
    const r = await open({ directory: true, multiple: false, title: o.title, defaultPath: o.dir });
    return typeof r === "string" ? r : null;
  }
  return fileDialog.pick({ mode: "folder", ...o });
}

/** Elige dónde guardar. La extensión del primer filtro se añade si falta. */
export async function saveFile(o: { title?: string; filters?: Filter[]; defaultName?: string; dir?: string } = {}): Promise<string | null> {
  let path: string | null;
  if (useSystemDialog()) {
    const def = o.dir && o.defaultName ? joinPath(o.dir, o.defaultName) : o.defaultName;
    path = (await save({ filters: o.filters, title: o.title, defaultPath: def })) ?? null;
  } else {
    path = await fileDialog.pick({ mode: "save", ...o });
  }
  return path ? withExtension(path, o.filters) : null;
}

export function withExtension(path: string, filters?: Filter[]): string {
  const exts = filters?.[0]?.extensions ?? [];
  if (!exts.length || exts.includes("*")) return path;
  const lower = path.toLowerCase();
  return exts.some((e) => lower.endsWith(`.${e.toLowerCase()}`)) ? path : `${path}.${exts[0]}`;
}
