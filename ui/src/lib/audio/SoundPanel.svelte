<script lang="ts">
  // Sección «Sonido» del Modo Acción: volumen de cada pista, bajar el ambiente cuando hay voz y
  // los ambientes que abarcan varias imágenes.
  import AmbientEditor from "./AmbientEditor.svelte";
  import { sounds } from "./sounds.svelte";
  import { ambientRange } from "./sync";
  import type { ProjectStore } from "../action/project.svelte";
  import type { MixLevels } from "./director";
  import { t } from "../i18n.svelte";
  import { nameOf } from "./names";
  import { solo, toggleSolo } from "./solo.svelte";

  interface Props {
    store: ProjectStore;
    entries: string[];
    index: number;
  }
  let { store, entries, index }: Props = $props();

  if (!sounds.loaded) sounds.load();

  let open = $state<string | null>(null);
  let pick = $state("");
  const options = $derived([...sounds.list].sort((a, b) => Number(b.ambient) - Number(a.ambient)));
  $effect(() => {
    if (!pick && options.length) pick = options[0].id;
  });

  const indexOf = (e: string) => entries.indexOf(e);
  const rows = $derived(
    store.ambients
      .map((a) => ({ a, r: ambientRange(a, indexOf) }))
      .sort((x, y) => (x.r?.[0] ?? -1) - (y.r?.[0] ?? -1)),
  );

  const LEVELS: { key: "voice" | "sfx" | "ambient"; label: () => string }[] = [
    { key: "voice", label: () => t("trackVoice") },
    { key: "sfx", label: () => t("trackSfx") },
    { key: "ambient", label: () => t("trackAmbient") },
  ];
  const setLevel = (k: keyof MixLevels, v: number | boolean) => store.setMix((m) => ((m as unknown as Record<string, number | boolean>)[k] = v));

  function add() {
    if (!pick || !entries.length) return;
    const id = store.addAmbient({ sound: pick, from: entries[index], to: entries[entries.length - 1], volume: 1, fadeIn: 1.5, fadeOut: 1.5 });
    if (id) open = id;
  }

  const range = (r: [number, number] | null) =>
    !r ? t("ambientLost") : r[0] === r[1] ? t("ambientOne", { a: r[0] + 1 }) : t("ambientRange", { a: r[0] + 1, b: r[1] + 1 });
</script>

<section>
  <h4>{t("sound")} · {t("mixer")}</h4>
  <p class="note">{t("mixerHelp")}</p>
  {#each LEVELS as l (l.key)}
    <label class="param group">
      <span>{l.label()}</span>
      <input type="range" min="0" max="150" step="5" value={Math.round(store.mix[l.key] * 100)} oninput={(e) => setLevel(l.key, Number(e.currentTarget.value) / 100)} />
      <output>{Math.round(store.mix[l.key] * 100)}%</output>
    </label>
    {@const mine = store.tracks.filter((x) => x.bus === l.key)}
    {#if mine.length > 1 || mine.some((x) => x.mute || x.volume !== 1 || x.effects?.length || solo.ids.includes(x.id))}
      {#each mine as tr (tr.id)}
        <div class="param track" class:muted={tr.mute}>
          <span class="tname" title={nameOf(store.tracks, tr)}>{nameOf(store.tracks, tr)}{tr.effects?.some((e) => e.on) ? " · fx" : ""}</span>
          <input type="range" min="0" max="200" step="5" value={Math.round(tr.volume * 100)} disabled={!!store.readOnly}
            oninput={(e) => store.editTrack(tr.id, (x) => (x.volume = Number(e.currentTarget.value) / 100))} />
          <span class="ms-row">
            <button class="ms" class:on={tr.mute} title={t("trackMuteHelp")} disabled={!!store.readOnly}
              onclick={() => store.editTrack(tr.id, (x) => { if (x.mute) delete x.mute; else x.mute = true; })}>M</button>
            <button class="ms solo" class:on={solo.ids.includes(tr.id)} title={t("trackSoloHelp")} onclick={() => toggleSolo(tr.id)}>S</button>
          </span>
        </div>
      {/each}
    {/if}
  {/each}
  <label class="check">
    <input type="checkbox" checked={store.mix.duck} onchange={(e) => setLevel("duck", e.currentTarget.checked)} />
    {t("duck")}
  </label>

  <h4 class="sub">{t("ambients")}</h4>
  {#each rows as { a, r } (a.id)}
    {@const here = !!r && index >= r[0] && index <= r[1]}
    <div class="ambient" class:here>
      <button class="head" onclick={() => (open = open === a.id ? null : a.id)}>
        <span class="arrow">{open === a.id ? "▾" : "▸"}</span>
        <span class="name">{a.name ?? sounds.name(a.sound)}</span>
        <span class="range">{range(r)}</span>
      </button>
      {#if open === a.id}
        <AmbientEditor clip={a} {entries} {index} tracks={store.tracks} edit={(fn) => store.editAmbient(a.id, fn)} onremove={() => store.removeAmbient(a.id)}
          onremovehere={() => store.removeAmbientAt(a.id, index, entries)} />
      {/if}
    </div>
  {:else}
    <p class="note">{t("ambientsEmpty")}</p>
  {/each}
  <div class="add">
    <select bind:value={pick}>
      {#each options as s (s.id)}<option value={s.id}>{s.ambient ? "🔁 " : ""}{sounds.name(s.id)}</option>{/each}
    </select>
    <button onclick={add} disabled={!pick || !!store.readOnly} title={t("ambientAddHelp")}>{t("ambientAdd")}</button>
  </div>
</section>

<style>
  section { display: flex; flex-direction: column; gap: 6px; }
  h4 { margin: 0 0 6px; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  h4.sub { margin-top: 6px; }
  .param { display: grid; grid-template-columns: 1fr 96px 40px; align-items: center; gap: 6px; }
  .param output { text-align: right; color: var(--muted); font-variant-numeric: tabular-nums; }
  .param.group span { font-weight: 600; }
  .param.track { padding-left: 12px; font-size: 12px; }
  .param.track.muted .tname { color: var(--muted); text-decoration: line-through; }
  .tname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ms-row { display: flex; gap: 2px; justify-content: flex-end; }
  .ms { width: 18px; height: 18px; padding: 0; font-size: 10px; font-weight: 700; line-height: 16px; border-radius: 4px; color: var(--muted); }
  .ms.on { background: #b45309; border-color: #f59e0b; color: #fff7ed; }
  .ms.solo.on { background: #1d4ed8; border-color: #60a5fa; color: #eff6ff; }
  .check { display: flex; align-items: center; gap: 6px; font-size: 12px; }
  .note { margin: 0; color: var(--muted); font-size: 12px; line-height: 1.4; }
  .ambient { border: 1px solid var(--border); border-radius: 6px; padding: 4px 6px; display: flex; flex-direction: column; gap: 6px; }
  .ambient.here { border-color: rgb(96 165 250 / 0.7); }
  .head { display: flex; align-items: center; gap: 6px; background: none; border: none; padding: 0; text-align: left; font-size: 12px; }
  .arrow { color: var(--muted); width: 10px; }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .range { color: var(--muted); font-size: 11px; white-space: nowrap; }
  .add { display: flex; gap: 4px; }
  .add select { flex: 1; min-width: 0; font: inherit; font-size: 12px; color: var(--text); background: var(--panel-2); border: 1px solid var(--border); border-radius: 6px; padding: 2px 4px; }
  .add button { font-size: 12px; padding: 2px 8px; white-space: nowrap; }
</style>
