<script lang="ts">
  // Una pista de sonido: nombre, volumen, panorama, silencio, solo, orden, efectos y borrar.
  import EffectsEditor from "./EffectsEditor.svelte";
  import { BUS_ICON, type Track } from "./tracks";
  import { groupName, nameOf } from "./names";
  import { solo, toggleSolo } from "./solo.svelte";
  import type { ProjectStore } from "../action/project.svelte";
  import { t } from "../i18n.svelte";

  interface Props {
    store: ProjectStore;
    track: Track;
    /** La pista se ha borrado. */
    onremoved: () => void;
  }
  let { store, track, onremoved }: Props = $props();

  const tracks = $derived(store.tracks);
  const group = $derived(tracks.filter((x) => x.bus === track.bus));
  const pos = $derived(group.findIndex((x) => x.id === track.id));
  let asking = $state(false);
  const usage = $derived(asking ? store.usage(track.id) : null);
  $effect(() => {
    void track.id;
    asking = false;
  });

  const edit = (fn: (x: Track) => void) => store.editTrack(track.id, fn);
  const num = (e: Event) => Number((e.currentTarget as HTMLInputElement).value);
  const panText = (p: number) => (Math.abs(p) < 0.005 ? t("panCenter") : `${Math.round(Math.abs(p) * 100)} ${p < 0 ? t("panLeft") : t("panRight")}`);

  function remove() {
    if (store.removeTrack(track.id)) onremoved();
  }
</script>

<h5>{BUS_ICON[track.bus]} {nameOf(tracks, track)}</h5>
<div class="meta">{t("trackGroup", { group: groupName(track.bus) })}</div>
<label class="row">
  <span>{t("clipName")}</span>
  <input type="text" value={track.name ?? ""} placeholder={nameOf(tracks, track)} disabled={!!store.readOnly}
    onkeydown={(e) => e.stopPropagation()}
    onchange={(e) => edit((x) => { const v = e.currentTarget.value.trim(); if (v) x.name = v.slice(0, 80); else delete x.name; })} />
</label>
<label class="row">
  <span>{t("volume")}</span>
  <input type="range" min="0" max="200" step="5" value={Math.round(track.volume * 100)} disabled={!!store.readOnly}
    oninput={(e) => edit((x) => (x.volume = num(e) / 100))} ondblclick={() => edit((x) => (x.volume = 1))} />
  <output>{Math.round(track.volume * 100)}%</output>
</label>
<label class="row">
  <span>{t("trackPan")}</span>
  <input type="range" min="-100" max="100" step="5" value={Math.round(track.pan * 100)} disabled={!!store.readOnly}
    oninput={(e) => edit((x) => (x.pan = num(e) / 100))} ondblclick={() => edit((x) => (x.pan = 0))} />
  <output>{panText(track.pan)}</output>
</label>
<div class="toggles">
  <button class="ms" class:on={track.mute} title={t("trackMuteHelp")} disabled={!!store.readOnly}
    onclick={() => edit((x) => { if (x.mute) delete x.mute; else x.mute = true; })}>M</button>
  <button class="ms solo" class:on={solo.ids.includes(track.id)} title={t("trackSoloHelp")} onclick={() => toggleSolo(track.id)}>S</button>
  <span class="grow"></span>
  <button disabled={pos <= 0 || !!store.readOnly} onclick={() => store.moveTrack(track.id, -1)}>▲ {t("trackUp")}</button>
  <button disabled={pos >= group.length - 1 || !!store.readOnly} onclick={() => store.moveTrack(track.id, 1)}>▼ {t("trackDown")}</button>
</div>

<h6>{t("effects")}</h6>
<EffectsEditor effects={track.effects ?? []} disabled={!!store.readOnly}
  onchange={(list) => edit((x) => { if (list.length) x.effects = list; else delete x.effects; })} />

<div class="remove">
  {#if group.length < 2}
    <p class="note">{t("trackRemoveLast")}</p>
  {:else if asking && usage}
    <p class="note">{usage.clips ? t("trackRemoveAsk", { clips: usage.clips === 1 ? t("nSound") : t("nSounds", { n: usage.clips }), images: usage.images === 1 ? t("nImage") : t("nImages", { n: usage.images }) }) : t("trackRemoveEmpty")}</p>
    <div class="toggles">
      <button class="danger" onclick={remove}>{t("trackRemove")}</button>
      <button onclick={() => (asking = false)}>{t("cancel")}</button>
    </div>
  {:else}
    <button onclick={() => (asking = true)} disabled={!!store.readOnly}>{t("trackRemove")}</button>
  {/if}
</div>

<style>
  h5 { margin: 0; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  h6 { margin: 6px 0 0; font-size: 11px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  .meta { color: var(--muted); font-size: 11px; }
  .row { display: grid; grid-template-columns: 1fr 96px 54px; align-items: center; gap: 6px; font-size: 12px; }
  .row input[type="text"] { grid-column: 2 / 4; }
  input[type="text"] {
    width: 100%; box-sizing: border-box; font: inherit; color: var(--text); background: var(--panel-2);
    border: 1px solid var(--border); border-radius: 6px; padding: 2px 6px;
  }
  input[type="range"] { width: 100%; }
  output { color: var(--muted); text-align: right; font-size: 11px; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .toggles { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; }
  .toggles button { padding: 2px 8px; font-size: 12px; }
  .grow { flex: 1; }
  .ms { width: 26px; padding: 2px 0 !important; font-weight: 600; }
  .ms.on { background: #b45309; border-color: #f59e0b; color: #fff7ed; }
  .ms.solo.on { background: #1d4ed8; border-color: #60a5fa; color: #eff6ff; }
  .remove { margin-top: 6px; display: flex; flex-direction: column; gap: 4px; }
  .remove > button { align-self: flex-start; padding: 2px 8px; font-size: 12px; }
  .note { margin: 0; color: var(--muted); font-size: 12px; line-height: 1.4; }
  .danger { background: #7f1d1d; border-color: #ef4444; color: #fecaca; }
</style>
