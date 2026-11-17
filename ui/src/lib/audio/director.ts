// Director del sonido: el reproductor y el editor le cuentan qué imagen suena y en qué instante
// está; él se lo traslada al motor de audio de Rust sin inundarlo de órdenes.
//
// Solo un «dueño» a la vez (el reproductor o el editor): si el editor se cierra justo cuando
// empieza el reproductor, su orden de silencio no apaga lo que ya suena.
import { invoke } from "@tauri-apps/api/core";
import { log } from "../log";
import { TransportSync, type AmbientSpec, type ClipSpec, type TransportState } from "./sync";
import type { TrackSpec } from "./tracks";

export interface MixLevels {
  voice: number;
  sfx: number;
  ambient: number;
  duck: boolean;
}

export const DEFAULT_LEVELS: MixLevels = { voice: 1, sfx: 1, ambient: 0.7, duck: true };

/** Lo que el reproductor necesita del sonido (el exportador no lo usa). */
export interface SoundSink {
  scene(owner: object, clips: ClipSpec[]): void;
  ambient(owner: object, list: AmbientSpec[]): void;
  transport(owner: object, s: TransportState): void;
  release(owner: object): void;
}

const warn = (what: string) => (e: unknown) => log.warn(`audio (${what}):`, e);

class Director implements SoundSink {
  private owner: object | null = null;
  private sceneKey = "";
  private ambientKey = "";
  private levelsKey = "";
  private sync = new TransportSync();
  /** Ya ha sonado algo: hasta entonces no se abre el dispositivo de sonido. */
  private active = false;

  /** `owner` pasa a mandar sobre el sonido. */
  private claim(owner: object) {
    if (this.owner === owner) return;
    this.owner = owner;
    this.sceneKey = "";
    this.ambientKey = "";
    this.sync.reset();
  }

  scene(owner: object, clips: ClipSpec[]) {
    this.claim(owner);
    const key = JSON.stringify(clips);
    if (key === this.sceneKey) return;
    this.sceneKey = key;
    if (!clips.length && !this.active) return;
    this.wake();
    invoke("audio_scene", { clips }).catch(warn("escena"));
  }

  ambient(owner: object, list: AmbientSpec[]) {
    this.claim(owner);
    const key = JSON.stringify(list);
    if (key === this.ambientKey) return;
    this.ambientKey = key;
    if (!list.length && !this.active) return;
    this.wake();
    invoke("audio_ambient", { list }).catch(warn("ambiente"));
  }

  /** Reloj de la imagen; se manda solo cuando hace falta. `seek` fuerza un salto. */
  transport(owner: object, s: TransportState, seek = false) {
    if (this.owner !== owner || !this.active) return;
    const r = this.sync.check(s, performance.now(), seek);
    if (r.send) invoke("audio_transport", { ...s, seek: r.seek }).catch(warn("transporte"));
  }

  private wanted: { levels: MixLevels; master: number; mute: boolean } = { levels: DEFAULT_LEVELS, master: 1, mute: false };
  /** Silencio mientras se graba la voz (que el micrófono no recoja los altavoces). */
  private recording = false;

  setRecording(on: boolean) {
    this.recording = on;
    const w = this.wanted;
    this.levels(w.levels, w.master, w.mute);
  }

  levels(levels: MixLevels, master: number, wantMute: boolean) {
    this.wanted = { levels, master, mute: wantMute };
    const mute = wantMute || this.recording;
    const key = JSON.stringify([levels, master, mute]);
    if (key === this.levelsKey) return;
    this.levelsKey = key;
    this.pendingLevels = { levels, master, mute };
    if (this.active) this.flushLevels();
  }

  private pendingLevels: { levels: MixLevels; master: number; mute: boolean } | null = null;
  private flushLevels() {
    const l = this.pendingLevels;
    if (!l) return;
    this.pendingLevels = null;
    invoke("audio_levels", l).catch(warn("volumen"));
  }

  private tracksKey = "";
  private pendingTracks: { tracks: TrackSpec[]; solo: string[] } | null = null;

  /** Pistas del proyecto (volumen, panorama, silencio, efectos) y las que están en solo. */
  tracks(tracks: TrackSpec[], solo: string[]) {
    const key = JSON.stringify([tracks, solo]);
    if (key === this.tracksKey) return;
    this.tracksKey = key;
    this.pendingTracks = { tracks, solo };
    if (this.active) this.flushTracks();
  }

  private flushTracks() {
    const p = this.pendingTracks;
    if (!p) return;
    this.pendingTracks = null;
    invoke("audio_tracks", p).catch(warn("pistas"));
  }

  /** Primera vez que suena algo: los volúmenes y las pistas pendientes van antes que el sonido. */
  private wake() {
    if (this.active) return;
    this.active = true;
    this.flushLevels();
    this.flushTracks();
  }

  /** El dueño deja de sonar (cierra el editor, para el reproductor…). */
  release(owner: object) {
    if (this.owner !== owner) return;
    this.owner = null;
    this.sceneKey = "";
    this.ambientKey = "";
    this.sync.reset();
    if (this.active) invoke("audio_silence").catch(warn("silencio"));
  }
}

export const director = new Director();
