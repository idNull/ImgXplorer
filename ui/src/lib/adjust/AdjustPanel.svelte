<script lang="ts">
  // Panel flotante de ajustes de imagen (🎨): un clic para lo habitual y el resto plegado.
  // Nada toca el archivo: se guarda en el proyecto (esta página o todo el cómic).
  import type { ProjectStore } from "../action/project.svelte";
  import { prefs } from "../prefs.svelte";
  import { t } from "../i18n.svelte";
  import { PRESETS, SLIDERS, normalize, presetOf, type Adjust, type AdjustKey } from "./adjust";

  interface Props {
    store: ProjectStore;
    entry: string | null;
    size: [number, number] | null;
    /** «Limpiar escaneo» está esperando el análisis de la página. */
    analyzing: boolean;
    onclose: () => void;
  }
  let { store, entry, size, analyzing, onclose }: Props = $props();

  // Por defecto, todo el cómic (lo normal al leer); si la página tiene los suyos, esa página.
  let scope = $state<"page" | "all">("all");
  let more = $state(false);
  let lastEntry: string | null = null;
  $effect(() => {
    if (entry !== lastEntry) {
      lastEntry = entry;
      scope = entry && store.hasOwnAdjust(entry) ? "page" : "all";
    }
  });

  const readOnly = $derived(!!store.readOnly);
  const own = $derived(entry ? store.hasOwnAdjust(entry) : false);
  /** Lo que se está editando en el ámbito elegido. */
  const current = $derived<Adjust>(
    (scope === "page" && entry ? (normalize(store.item(entry)?.adjust) ?? store.adjustFor(entry)) : normalize(store.data?.defaults.adjust)) ?? {},
  );
  const active = $derived(presetOf(current));

  function set(adj: Adjust | null) {
    if (readOnly) return;
    if (scope === "page") {
      if (entry && size) store.setAdjust(entry, adj, size);
    } else {
      store.setAdjust(null, adj);
    }
  }

  function setValue(key: AdjustKey, v: number) {
    set({ ...current, [key]: v });
  }

  /** Un ajuste de un clic: cambia el estilo pero respeta los deslizadores que no toca. */
  function preset(p: Adjust) {
    if (!Object.keys(p).length) return set(null);
    set({ ...current, tone: p.tone ?? "none", clean: p.clean ?? current.clean });
  }

  function toggleNight() {
    prefs.settings.night = !prefs.settings.night;
    prefs.saveSettings();
  }
  function nightStyle(style: "warm" | "invert") {
    prefs.settings.nightStyle = style;
    prefs.saveSettings();
  }

  const pct = (v: number) => `${v > 0 ? "+" : ""}${Math.round(v * 100)}`;
</script>

<div class="adjust" role="dialog" aria-label={t("adjTitle")}>
  <header>
    <b>🎨 {t("adjTitle")}</b>
    <button class="x" onclick={onclose} title={t("close")}>✕</button>
  </header>

  <div class="seg" role="radiogroup">
    <button class:on={scope === "page"} disabled={!entry} onclick={() => (scope = "page")}>{t("adjScopePage")}</button>
    <button class:on={scope === "all"} onclick={() => (scope = "all")}>{t("adjScopeAll")}</button>
  </div>
  {#if scope === "all" && own}
    <p class="note">{t("adjOwnNote")} <button class="link" onclick={() => entry && store.setAdjust(entry, null)}>{t("adjUseAll")}</button></p>
  {/if}

  <div class="presets">
    {#each PRESETS as p (p.id)}
      <button class:on={active === p.id} disabled={readOnly} onclick={() => preset(p.adj)} title={t(`adjHelp_${p.id}`)}>
        <span class="ico">{p.icon}</span>{t(`adjPreset_${p.id}`)}
      </button>
    {/each}
    <button class="night" class:on={prefs.settings.night} onclick={toggleNight} title={t("adjNightHelp")}>
      <span class="ico">🌙</span>{t("adjNight")}
    </button>
  </div>
  {#if prefs.settings.night}
    <div class="night-opts">
      <div class="seg" role="radiogroup" aria-label={t("adjNight")}>
        <button class:on={prefs.settings.nightStyle === "warm"} onclick={() => nightStyle("warm")} title={t("adjNightWarmHelp")}>{t("adjNightWarm")}</button>
        <button class:on={prefs.settings.nightStyle === "invert"} onclick={() => nightStyle("invert")} title={t("adjNightInvertHelp")}>{t("adjNightInvert")}</button>
      </div>
      {#if prefs.settings.nightStyle === "warm"}
        <label class="param">
          <span>{t("adjNightLevel")}</span>
          <input type="range" min="0.1" max="1" step="0.05" value={prefs.settings.nightLevel}
            oninput={(e) => (prefs.settings.nightLevel = Number(e.currentTarget.value))} onchange={() => prefs.saveSettings()} />
          <output>{Math.round(prefs.settings.nightLevel * 100)}</output>
        </label>
      {:else}
        <p class="note">{t("adjNightInvertNote")}</p>
      {/if}
    </div>
  {/if}
  {#if analyzing}<p class="note">{t("adjAnalyzing")}</p>{/if}

  <button class="more" onclick={() => (more = !more)} aria-expanded={more}>{more ? "▾" : "▸"} {t("adjMore")}</button>
  {#if more}
    <div class="sliders">
      {#each SLIDERS as s (s.key)}
        {@const v = current[s.key] ?? 0}
        <label class="param" title={t("adjDblReset")}>
          <span>{t(`adjk_${s.key}`)}</span>
          <input type="range" min={s.min} max={s.max} step="0.01" value={v} disabled={readOnly}
            onpointerdown={() => store.batch(true)} onpointerup={() => store.batch(false)} onpointercancel={() => store.batch(false)}
            oninput={(e) => setValue(s.key, Number(e.currentTarget.value))}
            ondblclick={() => setValue(s.key, 0)} />
          <output>{pct(v)}</output>
        </label>
      {/each}
    </div>
  {/if}

  <footer>
    <button onclick={() => set(null)} disabled={readOnly || !Object.keys(current).length}>{t("adjReset")}</button>
    <span class="hint">{t("adjCompare")}</span>
  </footer>
</div>

<style>
  .adjust {
    position: absolute; top: 10px; right: 10px; z-index: 5; width: 290px; max-height: calc(100% - 20px); overflow-y: auto;
    display: flex; flex-direction: column; gap: 8px; padding: 10px 12px; font-size: 13px;
    background: var(--float); border: 1px solid var(--border); border-radius: 10px;
    box-shadow: var(--shadow); backdrop-filter: blur(6px);
  }
  header { display: flex; align-items: center; justify-content: space-between; }
  .x { padding: 1px 7px; }
  .seg { display: flex; }
  .seg button { flex: 1; border-radius: 0; font-size: 12px; padding: 3px 6px; }
  .seg button:first-child { border-radius: 6px 0 0 6px; }
  .seg button:last-child { border-radius: 0 6px 6px 0; border-left: none; }
  .seg button.on { background: var(--accent); color: #04121b; border-color: var(--accent); }
  .presets { display: grid; grid-template-columns: repeat(5, 1fr); gap: 4px; }
  .presets button {
    display: flex; flex-direction: column; align-items: center; gap: 2px; padding: 5px 2px; font-size: 10.5px; line-height: 1.15;
    min-width: 0; overflow: hidden; text-align: center;
  }
  .presets .ico { font-size: 16px; }
  .presets button.on { border-color: var(--accent); background: rgb(56 189 248 / 0.12); }
  .presets .night.on { border-color: #fbbf24; background: rgb(251 191 36 / 0.12); }
  .night-opts { display: flex; flex-direction: column; gap: 6px; padding: 6px 8px; border-radius: 8px; background: rgb(251 191 36 / 0.07); border: 1px solid rgb(251 191 36 / 0.25); }
  .night-opts .seg button.on { background: #fbbf24; border-color: #fbbf24; }
  .more { align-self: flex-start; padding: 2px 6px; font-size: 12px; background: none; border-color: transparent; color: var(--muted); }
  .more:hover { color: var(--text); }
  .sliders { display: flex; flex-direction: column; gap: 4px; }
  .param { display: grid; grid-template-columns: minmax(0, 1fr) 120px 34px; align-items: center; gap: 6px; font-size: 12px; }
  .param span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .param input { width: 100%; }
  .param output { text-align: right; color: var(--muted); font-variant-numeric: tabular-nums; font-size: 11px; }
  .note { margin: 0; color: var(--muted); font-size: 11.5px; line-height: 1.4; }
  .link { padding: 0; border: none; background: none; color: var(--accent); font-size: inherit; text-decoration: underline; cursor: pointer; }
  footer { display: flex; align-items: center; justify-content: space-between; gap: 8px; border-top: 1px solid var(--border); padding-top: 8px; }
  footer button { padding: 2px 8px; font-size: 12px; }
  .hint { color: var(--muted); font-size: 11px; text-align: right; }
</style>
