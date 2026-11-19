<script lang="ts">
  // Un ambiente: qué sonido, de qué imagen a cuál, volumen y fundidos.
  import { sounds } from "./sounds.svelte";
  import type { AmbientClip } from "../action/project.svelte";
  import { t } from "../i18n.svelte";
  import EffectsEditor from "./EffectsEditor.svelte";
  import { resolveTrack, type Track } from "./tracks";
  import { nameOf } from "./names";

  interface Props {
    clip: AmbientClip;
    /** Nombres de las entradas, en orden. */
    entries: string[];
    /** Imagen actual. */
    index: number;
    /** Pistas del proyecto (se elige una de ambiente). */
    tracks: Track[];
    edit: (fn: (a: AmbientClip) => void) => void;
    /** Borrar en todas las imágenes. */
    onremove: () => void;
    /** Quitarlo solo de la imagen actual (si no está, no sale el botón). */
    onremovehere?: () => void;
  }
  let { clip, entries, index, tracks, edit, onremove, onremovehere }: Props = $props();
  const ambTracks = $derived(tracks.filter((x) => x.bus === "ambient"));
  const trackId = $derived(resolveTrack(tracks, clip.track, "ambient").id);
  function setTrack(id: string) {
    const first = resolveTrack(tracks, undefined, "ambient").id;
    edit((a) => {
      if (id === first) delete a.track;
      else a.track = id;
    });
  }

  const from = $derived(entries.indexOf(clip.from));
  const to = $derived(entries.indexOf(clip.to));
  const num = (e: Event) => Number((e.currentTarget as HTMLInputElement).value);
  const at = (n: number) => entries[Math.min(entries.length - 1, Math.max(0, Math.round(n) - 1))];

  function setFrom(n: number) {
    edit((a) => {
      a.from = at(n);
      if (entries.indexOf(a.to) < entries.indexOf(a.from)) a.to = a.from;
    });
  }
  function setTo(n: number) {
    edit((a) => {
      a.to = at(n);
      if (entries.indexOf(a.to) < entries.indexOf(a.from)) a.from = a.to;
    });
  }
  const lo = $derived(Math.min(from, to < 0 ? from : to));
  const hi = $derived(Math.max(from, to < 0 ? from : to));
  /** Imágenes en las que suena. */
  const span = $derived(from < 0 ? 0 : hi - lo + 1);
  /** La imagen actual es una de las que lo continúan (no la primera). */
  const continued = $derived(span > 1 && index > lo && index <= hi);
  // Primero los pensados para ambiente; luego el resto.
  const options = $derived([...sounds.list].sort((a, b) => Number(b.ambient) - Number(a.ambient)));
</script>

<div class="amb">
  <label class="row">
    <span>{t("ambientSound")}</span>
    <select value={clip.sound} onchange={(e) => edit((a) => (a.sound = e.currentTarget.value))}>
      {#if !sounds.get(clip.sound)}<option value={clip.sound}>{t("soundMissing")}</option>{/if}
      {#each options as s (s.id)}<option value={s.id}>{s.ambient ? "🔁 " : ""}{sounds.name(s.id)}</option>{/each}
    </select>
  </label>
  {#if ambTracks.length > 1}
    <label class="row">
      <span>{t("track")}</span>
      <select value={trackId} onchange={(e) => setTrack(e.currentTarget.value)}>
        {#each ambTracks as tr (tr.id)}<option value={tr.id}>{nameOf(tracks, tr)}</option>{/each}
      </select>
    </label>
  {/if}
  <div class="row">
    <span>{t("ambientFrom")}</span>
    <input type="number" min="1" max={entries.length} value={from + 1} onchange={(e) => setFrom(num(e))} />
    <button class="here" title={t("ambientHere")} onclick={() => setFrom(index + 1)}>⌖</button>
  </div>
  <div class="row">
    <span>{t("ambientTo")}</span>
    <input type="number" min="1" max={entries.length} value={(to < 0 ? from : to) + 1} onchange={(e) => setTo(num(e))} />
    <button class="here" title={t("ambientHere")} onclick={() => setTo(index + 1)}>⌖</button>
  </div>
  <label class="row">
    <span>{t("volume")}</span>
    <input type="range" min="0" max="150" step="5" value={Math.round(clip.volume * 100)} oninput={(e) => edit((a) => (a.volume = num(e) / 100))} />
    <output>{Math.round(clip.volume * 100)}%</output>
  </label>
  <label class="row">
    <span>{t("fadeIn")}</span>
    <input type="number" min="0" max="30" step="0.5" value={clip.fadeIn} onchange={(e) => edit((a) => (a.fadeIn = Math.max(0, num(e) || 0)))} />
    <output>s</output>
  </label>
  <label class="row">
    <span>{t("fadeOut")}</span>
    <input type="number" min="0" max="30" step="0.5" value={clip.fadeOut} onchange={(e) => edit((a) => (a.fadeOut = Math.max(0, num(e) || 0)))} />
    <output>s</output>
  </label>
  <h6>{t("effects")}</h6>
  <EffectsEditor effects={clip.effects ?? []} onchange={(list) => edit((a) => { if (list.length) a.effects = list; else delete a.effects; })} />
  {#if span > 1}
    <p class="hint">{continued ? t("ambientContinued", { a: lo + 1, n: span }) : t("ambientStartsHere", { n: span })}</p>
  {/if}
  <div class="buttons">
    {#if continued && onremovehere}
      <button onclick={onremovehere} title={t("ambientRemoveHereHelp")}>✂ {t("ambientRemoveHere")}</button>
    {/if}
    {#if span > 1 && index >= lo && index < hi}
      <button onclick={() => setTo(index + 1)} title={t("ambientEndHereHelp")}>⏹ {t("ambientEndHere")}</button>
    {/if}
    <button class:danger={span > 1} onclick={onremove}>{span > 1 ? t("ambientRemoveAll", { n: span }) : t("remove")}</button>
  </div>
</div>

<style>
  .amb { display: flex; flex-direction: column; gap: 5px; }
  .row { display: grid; grid-template-columns: 1fr 96px 34px; align-items: center; gap: 6px; font-size: 12px; }
  .row select { grid-column: 2 / 4; }
  select, input[type="number"] {
    width: 100%; box-sizing: border-box; font: inherit; color: var(--text); background: var(--panel-2);
    border: 1px solid var(--border); border-radius: 6px; padding: 2px 6px;
  }
  input[type="range"] { width: 100%; }
  output { color: var(--muted); text-align: right; font-variant-numeric: tabular-nums; }
  .here { padding: 0 4px; }
  .buttons { display: flex; flex-wrap: wrap; gap: 4px; }
  .hint { margin: 4px 0 0; font-size: 11.5px; color: var(--muted); line-height: 1.4; }
  .danger { color: #fca5a5; }
  h6 { margin: 6px 0 0; font-size: 11px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  .buttons button { padding: 2px 8px; font-size: 12px; }
</style>
