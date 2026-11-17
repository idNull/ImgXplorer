<script lang="ts">
  // Propiedades de un clip de sonido: cuándo empieza, cuánto suena, recorte, volumen y fundidos.
  import { sounds } from "../audio/sounds.svelte";
  import { MIN_DURATION, audioClipEnd, dragAudioEdge, type AudioBus, type AudioClip } from "./model";
  import { t } from "../i18n.svelte";
  import EffectsEditor from "../audio/EffectsEditor.svelte";
  import { BUS_ICON, type Track } from "../audio/tracks";
  import { nameOf } from "../audio/names";

  interface Props {
    clip: AudioClip;
    bus: AudioBus;
    /** Aplica un cambio al clip (se guarda y entra en deshacer). */
    edit: (fn: (c: AudioClip) => void) => void;
    /** Pistas del proyecto y la de este clip. */
    tracks: Track[];
    track: string;
    onsettrack: (id: string) => void;
    onseek: (t: number) => void;
    onremove: () => void;
    onduplicate: () => void;
  }
  let { clip, bus, edit, tracks, track, onsettrack, onseek, onremove, onduplicate }: Props = $props();

  const info = $derived(sounds.get(clip.sound));
  const len = $derived(info?.seconds ?? Infinity);
  const fmt = (v: number, d = 2) => Number(v.toFixed(d));
  const num = (e: Event) => Number((e.currentTarget as HTMLInputElement).value);
  const ok = (v: number) => Number.isFinite(v);

  function setStart(v: number) {
    edit((c) => (c.start = Math.max(0, v)));
  }
  function setDuration(v: number) {
    edit((c) => dragAudioEdge(c, "end", c.start + v, len));
  }
  function setTrimIn(v: number) {
    // Recortar el principio sin mover el clip: el final del sonido no cambia.
    edit((c) => {
      const trim = Math.min(Math.max(0, v), (Number.isFinite(len) ? len : c.trimIn + c.duration) - MIN_DURATION);
      const endInSound = c.trimIn + c.duration;
      c.trimIn = trim;
      c.duration = Math.max(MIN_DURATION, endInSound - trim);
    });
  }
  function setFade(which: "fadeIn" | "fadeOut", v: number) {
    edit((c) => {
      const other = which === "fadeIn" ? c.fadeOut : c.fadeIn;
      c[which] = Math.min(Math.max(0, v), Math.max(0, c.duration - other));
    });
  }
</script>

<h5>{clip.name ?? (info ? sounds.name(clip.sound) : t("soundMissing"))}</h5>
{#if !info && sounds.loaded}<p class="warn">{t("soundMissingHelp")}</p>{/if}
<label class="row">
  <span>{t("clipName")}</span>
  <input type="text" value={clip.name ?? ""} placeholder={info ? sounds.name(clip.sound) : ""}
    onchange={(e) => edit((c) => { const v = e.currentTarget.value.trim(); if (v) c.name = v.slice(0, 120); else delete c.name; })} />
</label>
<div class="meta">{bus === "voice" ? t("trackVoice") : t("trackSfx")} · {info ? t("soundLength", { s: fmt(info.seconds, 1) }) : ""}</div>
<label class="row">
  <span>{t("track")}</span>
  <select value={track} onchange={(e) => onsettrack(e.currentTarget.value)}>
    {#each tracks.filter((x) => x.bus !== "ambient") as tr (tr.id)}<option value={tr.id}>{BUS_ICON[tr.bus]} {nameOf(tracks, tr)}</option>{/each}
  </select>
</label>
<label class="row">
  <span>{t("start")}</span>
  <input type="number" step="0.1" min="0" value={fmt(clip.start)} onchange={(e) => ok(num(e)) && setStart(num(e))} />
</label>
<label class="row">
  <span>{t("duration")}</span>
  <input type="number" step="0.1" min={MIN_DURATION} value={fmt(clip.duration)} onchange={(e) => ok(num(e)) && setDuration(num(e))} />
</label>
<label class="row">
  <span>{t("trimIn")}</span>
  <input type="number" step="0.1" min="0" value={fmt(clip.trimIn)} onchange={(e) => ok(num(e)) && setTrimIn(num(e))} />
</label>
<label class="row">
  <span>{t("volume")}</span>
  <input type="range" min="0" max="200" step="5" value={Math.round(clip.volume * 100)} oninput={(e) => edit((c) => (c.volume = num(e) / 100))} />
  <span class="unit">{Math.round(clip.volume * 100)}%</span>
</label>
<label class="row">
  <span>{t("fadeIn")}</span>
  <input type="number" step="0.1" min="0" value={fmt(clip.fadeIn)} onchange={(e) => ok(num(e)) && setFade("fadeIn", num(e))} />
  <span class="unit">s</span>
</label>
<label class="row">
  <span>{t("fadeOut")}</span>
  <input type="number" step="0.1" min="0" value={fmt(clip.fadeOut)} onchange={(e) => ok(num(e)) && setFade("fadeOut", num(e))} />
  <span class="unit">s</span>
</label>
<div class="buttons">
  <button onclick={() => onseek(clip.start)} title={t("goToStartHelp")}>⇤ {fmt(clip.start, 1)} s</button>
  <button onclick={() => onseek(audioClipEnd(clip))}>{fmt(audioClipEnd(clip), 1)} s ⇥</button>
  <button onclick={onduplicate}>{t("duplicate")}</button>
  <button onclick={onremove}>{t("remove")}</button>
</div>

<h6>{t("effects")}</h6>
<p class="meta">{t("fxClipNote")}</p>
<EffectsEditor effects={clip.effects ?? []} onchange={(list) => edit((c) => { if (list.length) c.effects = list; else delete c.effects; })} />

<style>
  h5 { margin: 0; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row { display: grid; grid-template-columns: 1fr 86px 34px; align-items: center; gap: 6px; }
  .row > :nth-child(2):last-child { grid-column: 2 / 4; }
  input[type="number"], input[type="text"] {
    width: 100%; box-sizing: border-box; font: inherit; color: var(--text); background: var(--panel-2);
    border: 1px solid var(--border); border-radius: 6px; padding: 2px 6px;
  }
  .row input[type="text"], .row select { grid-column: 2 / 4; }
  select { width: 100%; min-width: 0; font: inherit; font-size: 12px; color: var(--text); background: var(--panel-2); border: 1px solid var(--border); border-radius: 6px; padding: 2px 4px; }
  h6 { margin: 6px 0 0; font-size: 11px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  input[type="range"] { width: 100%; }
  .unit { color: var(--muted); font-size: 12px; }
  .meta { color: var(--muted); font-size: 11px; margin: 0; }
  .warn { color: #fca5a5; font-size: 11px; margin: 0; }
  .buttons { display: flex; flex-wrap: wrap; gap: 4px; }
  .buttons button { padding: 2px 8px; font-size: 12px; }
</style>
