// Pistas de sonido del proyecto (las mismas en todas las imágenes). Lógica pura, sin Tauri.
import type { Effect } from "./effects";

export type SoundBus = "voice" | "sfx" | "ambient";
export const SOUND_BUSES: SoundBus[] = ["voice", "sfx", "ambient"];

export interface Track {
  id: string;
  bus: SoundBus;
  /** Sin nombre: el del grupo («Voz», «Efectos», «Ambiente»), con su número si hay varias. */
  name?: string;
  volume: number;
  /** −1 izquierda … 1 derecha. */
  pan: number;
  mute?: boolean;
  effects?: Effect[];
}

/** La pista de un grupo en los proyectos sin pistas (su id es el del grupo). */
export const defaultTrack = (bus: SoundBus): Track => ({ id: bus, bus, volume: 1, pan: 0 });

/** Pistas en orden (voces, efectos, ambientes), con al menos una por grupo. */
export function withDefaults(list: Track[] | undefined): Track[] {
  const all = list ?? [];
  return SOUND_BUSES.flatMap((bus) => {
    const mine = all.filter((t) => t.bus === bus);
    return mine.length ? mine : [defaultTrack(bus)];
  });
}

/** La pista de un clip: la suya si existe y es de su grupo; si no, la primera del grupo. */
export function resolveTrack(tracks: Track[], id: string | undefined, bus: SoundBus): Track {
  const list = withDefaults(tracks);
  return list.find((t) => t.id === id && t.bus === bus) ?? list.find((t) => t.bus === bus)!;
}

/** Lo que el motor necesita de cada pista. */
export interface TrackSpec {
  id: string;
  bus: SoundBus;
  volume: number;
  pan: number;
  mute: boolean;
  effects: Effect[];
}

export function trackSpecs(tracks: Track[] | undefined): TrackSpec[] {
  return withDefaults(tracks).map((t) => ({ id: t.id, bus: t.bus, volume: t.volume, pan: t.pan, mute: !!t.mute, effects: t.effects ?? [] }));
}

/** Id nuevo de pista. */
export const newTrackId = () => `trk-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;

/** Nombre visible de una pista: el suyo o el del grupo, numerado si hay varias. */
export function trackName(tracks: Track[], tr: Track, group: (bus: SoundBus) => string): string {
  if (tr.name) return tr.name;
  const same = tracks.filter((x) => x.bus === tr.bus);
  const k = same.findIndex((x) => x.id === tr.id);
  return same.length > 1 ? `${group(tr.bus)} ${k + 1}` : group(tr.bus);
}

export const BUS_ICON: Record<SoundBus, string> = { voice: "🎙", sfx: "💥", ambient: "🌧" };
