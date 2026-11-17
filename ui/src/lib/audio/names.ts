// Nombres visibles de grupos y pistas (con el idioma actual).
import { t } from "../i18n.svelte";
import { trackName, type SoundBus, type Track } from "./tracks";

export const groupName = (bus: SoundBus) => t(bus === "voice" ? "trackVoice" : bus === "sfx" ? "trackSfx" : "trackAmbient");

export const nameOf = (tracks: Track[], tr: Track) => trackName(tracks, tr, groupName);
