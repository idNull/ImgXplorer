<script lang="ts">
  // Menú de lectura (📖): cómo se ajusta la página, una o dos páginas, tira vertical, sentido
  // manga, la barra de progreso y los marcadores. Se recuerda por cómic.
  import { t } from "../i18n.svelte";
  import { prefs } from "../prefs.svelte";
  import { FITS, type Reading } from "./reading.svelte";

  interface Props {
    rd: Reading;
    onchange: (patch: Partial<Reading>) => void;
    /** Páginas marcadas: posición (0..) y nombre. */
    marks: { index: number; name: string }[];
    ongo: (index: number) => void;
    ongoto: () => void;
    /** «(tecla)» de cada acción para los títulos. */
    kt: (a: "fitWidth" | "fitHeight" | "fit" | "double" | "strip" | "loupe" | "goTo" | "bookmark") => string;
  }
  let { rd, onchange, marks, ongo, ongoto, kt }: Props = $props();

  const FIT_KEY = { window: "fit", width: "fitWidth", height: "fitHeight" } as const;

  function toggleBar() {
    prefs.settings.progressBar = !prefs.settings.progressBar;
    prefs.saveSettings();
  }
</script>

<div class="reading" role="menu" aria-label={t("readTitle")}>
  <div class="row">
    <span class="lbl">{t("readFit")}</span>
    <div class="seg">
      {#each FITS as f (f)}
        <button class:on={rd.fit === f} onclick={() => onchange({ fit: f })} title="{t(`readFitHelp_${f}`)}{kt(FIT_KEY[f])}">{t(`readFit_${f}`)}</button>
      {/each}
    </div>
  </div>
  <label class="check" title={t("readCropHelp")}>
    <input type="checkbox" checked={rd.crop} onchange={(e) => onchange({ crop: e.currentTarget.checked })} /> ✂ {t("readCrop")}
  </label>

  <div class="row">
    <span class="lbl">{t("readPages")}</span>
    <div class="seg">
      <button class:on={!rd.double && !rd.strip} onclick={() => onchange({ double: false, strip: false })}>{t("readSingle")}</button>
      <button class:on={rd.double && !rd.strip} onclick={() => onchange({ double: true, strip: false })} title="{t('readDoubleHelp')}{kt('double')}">{t("readDouble")}</button>
      <button class:on={rd.strip} onclick={() => onchange({ strip: true })} title="{t('readStripHelp')}{kt('strip')}">{t("readStrip")}</button>
    </div>
  </div>
  {#if rd.double && !rd.strip}
    <label class="check sub">
      <input type="checkbox" checked={rd.cover} onchange={(e) => onchange({ cover: e.currentTarget.checked })} /> {t("readCover")}
    </label>
  {/if}

  <div class="row">
    <span class="lbl">{t("readDir")}</span>
    <div class="seg">
      <button class:on={!rd.rtl} onclick={() => onchange({ rtl: false })} title={t("readLtrHelp")}>{t("readLtr")} →</button>
      <button class:on={rd.rtl} onclick={() => onchange({ rtl: true })} title={t("readRtlHelp")}>← {t("readRtl")}</button>
    </div>
  </div>

  <label class="check">
    <input type="checkbox" checked={prefs.settings.progressBar} onchange={toggleBar} /> {t("readBar")}
  </label>

  <div class="marks">
    <span class="lbl">🔖 {t("readMarks")}</span>
    {#if marks.length}
      <div class="chips">
        {#each marks as m (m.name)}
          <button onclick={() => ongo(m.index)} title={m.name}>{m.index + 1}</button>
        {/each}
      </div>
    {:else}
      <span class="muted">{t("readNoMarks", { key: kt("bookmark").trim() })}</span>
    {/if}
  </div>

  <footer>
    <button onclick={ongoto}>{t("readGoTo")}{kt("goTo")}</button>
    <span class="muted">🔍 {t("readLoupeHint", { key: kt("loupe").trim() })}</span>
  </footer>
</div>

<style>
  .reading { display: flex; flex-direction: column; gap: 8px; padding: 4px 4px 2px; font-size: 13px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  .lbl { color: var(--muted); font-size: 12px; white-space: nowrap; }
  .seg { display: flex; }
  .seg button { border-radius: 0; font-size: 12px; padding: 3px 8px; white-space: nowrap; }
  .seg button:first-child { border-radius: 6px 0 0 6px; }
  .seg button:last-child { border-radius: 0 6px 6px 0; }
  .seg button + button { border-left: none; }
  .seg button.on { background: var(--accent); color: #04121b; border-color: var(--accent); }
  .check { display: flex; align-items: center; gap: 6px; font-size: 12.5px; cursor: pointer; }
  .check.sub { padding-left: 14px; margin-top: -4px; }
  .marks { display: flex; flex-direction: column; gap: 4px; border-top: 1px solid var(--border); padding-top: 8px; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; max-height: 84px; overflow-y: auto; }
  .chips button { padding: 1px 8px; font-size: 12px; font-variant-numeric: tabular-nums; }
  .muted { color: var(--muted); font-size: 11.5px; }
  footer { display: flex; align-items: center; justify-content: space-between; gap: 8px; border-top: 1px solid var(--border); padding-top: 8px; }
  footer button { padding: 2px 8px; font-size: 12px; white-space: nowrap; }
</style>
