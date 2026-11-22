// Llamadas a Rust del visor móvil: la biblioteca (mobile/src-tauri/src/library.rs) y la fuente
// abierta (crates/imgx-host, las mismas órdenes que el escritorio).
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { LoadedProject } from "$imgx/action/project.svelte";
import type { PictureRef } from "$imgx/viewer/picture";
import { lang, t } from "./i18n";

export type Kind = "comic" | "story" | "album";

export interface Item {
  id: string;
  title: string;
  kind: Kind;
  path: string;
  pages: number;
  page: number;
  added: number;
  opened: number;
}

export interface Imported {
  items: Item[];
  failed: [string, string][];
}

/** El manifiesto de una historia `.imgxstory` (crates/imgx-story). */
export interface StoryManifest {
  title: string;
  credits?: string;
  variant: "desktop" | "mobile";
  pages: number;
  reading: Partial<{ fit: "window" | "width" | "height"; strip: boolean; rtl: boolean }>;
  /** Ancho / alto del vídeo para el que se pensó la cámara. */
  authoredAspect?: number;
}

export interface SourceInfo {
  generation: number;
  title: string;
  path: string;
  kind: "folder" | "archive";
  start: number;
  entries: { name: string; size: number; decoded: boolean }[];
  fingerprint: string;
  /** Animaciones: las de la historia o las guardadas junto al cómic. */
  project: LoadedProject;
  story?: StoryManifest;
}

/** `imgx://localhost/` (o `http://imgx.localhost/` en Android). */
export const base = convertFileSrc("", "imgx");

export const pictureRef = (s: SourceInfo, i: number): PictureRef => ({
  url: `${base}${s.generation}_${i}`,
  fallback: `${base}d/${s.generation}_${i}`,
  decoded: s.entries[i].decoded,
  index: i,
});

/** Título del álbum que se crea al elegir fotos del carrete. */
const albumTitle = () =>
  t("albumTitle", {
    date: new Date().toLocaleString(lang === "es" ? "es-ES" : "en-GB", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }),
  });

export const library = {
  list: () => invoke<Item[]>("library_list"),
  pick: (kind: "images" | "documents") => invoke<Imported>("library_pick", { kind, albumTitle: albumTitle() }),
  takeOpened: () => invoke<Imported>("library_take_opened", { albumTitle: albumTitle() }),
  importPaths: (paths: string[]) => invoke<Imported>("library_import_paths", { paths, albumTitle: albumTitle() }),
  progress: (id: string, page: number) => invoke("library_progress", { id, page }),
  remove: (id: string) => invoke("library_remove", { id }),
};

/** Portada de un elemento (la crea Rust la primera vez): URL de la imagen o `null`. */
export async function coverUrl(path: string): Promise<string | null> {
  const c = await invoke<{ key: string; version: number } | null>("cover", { path }).catch(() => null);
  return c ? `${base}c/${c.key}?v=${c.version}` : null;
}

export const openSource = (path: string) => invoke<SourceInfo>("open_source", { path });
export const closeSource = () => invoke("close_source");

/** Viñetas de la imagen i, `[x, y, w, h, …]` (en una historia ya vienen calculadas). */
export const detectPanels = (generation: number, index: number) =>
  invoke<number[][]>("detect_panels", { generation, index }).then((r) => r.flat());
