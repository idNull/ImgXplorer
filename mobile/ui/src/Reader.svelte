<script lang="ts">
  // Lector táctil: el visor WebGL del escritorio (Viewer.svelte, con teselas, tira vertical y
  // encuadre) a pantalla completa. Tocar a los lados o deslizar pasa de página; tocar en el centro
  // enseña u oculta los controles; pellizcar hace zoom y el doble toque amplía.
  import { onDestroy, onMount } from "svelte";
  import Viewer from "$imgx/viewer/Viewer.svelte";
  import type { Framing, ReadLayout } from "$imgx/reading/layout";
  import { closeSource, library, openSource, pictureRef, type Item, type SourceInfo } from "./lib/api";
  import { t } from "./lib/i18n";
  import { reading, saveReading, type Mode } from "./lib/reading";

  let { item, onclose }: { item: Item; onclose: () => void } = $props();

  let source = $state<SourceInfo | null>(null);
  let error = $state<string | null>(null);
  let index = $state(0);
  let bars = $state(true);
  let viewer = $state<Viewer>();
  let mode = $state<Mode>(reading.mode);
  let rtl = $state(reading.rtl);

  const count = $derived(source?.entries.length ?? 0);
  const current = $derived(source && count ? pictureRef(source, index) : null);
  const neighbours = $derived(
    source ? [index + 1, index - 1, index + 2].filter((i) => i >= 0 && i < count).map((i) => pictureRef(source!, i)) : [],
  );
  const layout = $derived<ReadLayout | null>(
    source && mode === "strip" ? { kind: "strip", id: String(source.generation), count, at: index } : null,
  );
  const framing = $derived<Framing>({ fit: mode === "width" ? "width" : "window", rtl, box: null });

  let hideTimer = 0;
  function showBars(ms = 2500) {
    bars = true;
    clearTimeout(hideTimer);
    hideTimer = window.setTimeout(() => (bars = false), ms);
  }

  onMount(async () => {
    try {
      const s = await openSource(item.path);
      index = Math.min(item.page, Math.max(0, s.entries.length - 1));
      source = s;
      showBars();
    } catch (e) {
      error = String(e);
    }
  });
  onDestroy(() => {
    clearTimeout(hideTimer);
    clearTimeout(saveTimer);
    if (source) library.progress(item.id, index);
    closeSource();
  });

  // Progreso: se guarda poco después de cambiar de página.
  let saveTimer = 0;
  $effect(() => {
    const i = index;
    if (!source) return;
    clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => library.progress(item.id, i), 600);
  });

  function go(i: number) {
    if (!count) return;
    index = Math.max(0, Math.min(count - 1, i));
  }
  /** Página siguiente en el sentido de lectura (`dir` 1) o la anterior (−1). */
  const step = (dir: 1 | -1) => go(index + dir);

  function tap(x: number) {
    const w = window.innerWidth;
    if (mode !== "strip" && x < w * 0.3) return step(rtl ? 1 : -1);
    if (mode !== "strip" && x > w * 0.7) return step(rtl ? -1 : 1);
    if (bars) {
      bars = false;
      clearTimeout(hideTimer);
    } else showBars(4000);
  }

  function swipe(dx: number, dy: number): boolean {
    if (mode === "strip") return false;
    // Con zoom (pellizco) el dedo mueve la imagen; ajustada al ancho, el desplazamiento es vertical.
    if (mode === "page" && viewer?.isZoomedIn()) return false;
    if (Math.abs(dx) < 50 || Math.abs(dx) < Math.abs(dy) * 1.5) return false;
    const forward = dx < 0 !== rtl;
    step(forward ? 1 : -1);
    return true;
  }

  const MODES: Mode[] = ["page", "width", "strip"];
  const modeLabel = { page: () => `📄 ${t("modePage")}`, width: () => `↔ ${t("modeWidth")}`, strip: () => `📜 ${t("modeStrip")}` };
  function nextMode() {
    mode = MODES[(MODES.indexOf(mode) + 1) % MODES.length];
    saveReading({ mode, rtl });
    showBars();
  }
  function toggleRtl() {
    rtl = !rtl;
    saveReading({ mode, rtl });
    showBars();
  }
</script>

<div class="reader">
  {#if error}
    <div class="error">
      <p>{t("openError", { err: error })}</p>
      <button onclick={onclose}>{t("close")}</button>
    </div>
  {:else if source}
    <Viewer
      bind:this={viewer}
      item={current}
      prefetch={neighbours}
      refAt={(i) => pictureRef(source!, i)}
      background="#000000"
      {layout}
      {framing}
      onindex={(i) => (index = i)}
      ontap={(x) => tap(x)}
      onswipe={(dx, dy) => swipe(dx, dy)}
    />
  {/if}

  {#if bars && source}
    <div class="top">
      <button class="back" onclick={onclose} aria-label={t("back")}>←</button>
      <span class="title">{item.title}</span>
      <span class="num">{index + 1} / {count}</span>
    </div>
    <div class="bottom">
      {#if count > 1}
        <input
          type="range"
          min="0"
          max={count - 1}
          value={index}
          dir={rtl ? "rtl" : "ltr"}
          oninput={(e) => {
            go(+(e.currentTarget as HTMLInputElement).value);
            showBars(4000);
          }}
        />
      {/if}
      <div class="row">
        <button onclick={nextMode} title={t("modeHint")}>{modeLabel[mode]()}</button>
        <button class:on={rtl} onclick={toggleRtl} title={t("mangaHint")}>⇄ {t("manga")}</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .reader { position: fixed; inset: 0; background: #000; }
  .top, .bottom {
    position: absolute; left: 0; right: 0; z-index: 2;
    background: rgb(15 18 23 / 0.88); backdrop-filter: blur(8px);
  }
  .top {
    top: 0; display: flex; align-items: center; gap: 10px;
    padding: calc(env(safe-area-inset-top) + 6px) 12px 8px;
  }
  .back { background: none; border: 0; color: var(--text); font-size: 24px; padding: 4px 10px 4px 2px; }
  .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }
  .num { color: var(--muted); font-variant-numeric: tabular-nums; }
  .bottom { bottom: 0; padding: 10px 16px calc(env(safe-area-inset-bottom) + 10px); }
  input[type="range"] { width: 100%; accent-color: var(--accent); }
  .row { display: flex; gap: 10px; justify-content: center; margin-top: 6px; }
  .row button {
    border: 0; border-radius: 999px; padding: 9px 16px; font: inherit; font-size: 15px;
    background: #2a313b; color: var(--text);
  }
  .row button.on { background: var(--accent); color: white; }
  .error { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 14px; padding: 24px; text-align: center; }
  .error button { border: 0; border-radius: 12px; padding: 12px 22px; background: var(--panel); color: var(--text); font: inherit; }
</style>
