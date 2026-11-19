<script lang="ts" module>
  export type SaveFormat = "original" | "png" | "jpeg";
  export type SaveZone = "page" | "visible" | "selection";
  export interface SaveOptions {
    format: SaveFormat;
    zone: SaveZone;
    adjust: boolean;
  }
</script>

<script lang="ts">
  // Guardar imagen como… (Ctrl+S): el archivo original tal cual, o PNG / JPEG de la página entera,
  // de lo que se ve o de la zona elegida, con o sin los ajustes de imagen.
  import { onMount } from "svelte";
  import { t } from "../i18n.svelte";

  interface Props {
    /** Formato del original (para el texto del botón). */
    original: string;
    canVisible: boolean;
    canSelection: boolean;
    adjusted: boolean;
    zone: SaveZone;
    onsave: (o: SaveOptions) => void;
    oncancel: () => void;
  }
  let { original, canVisible, canSelection, adjusted, zone: initial, onsave, oncancel }: Props = $props();

  // svelte-ignore state_referenced_locally
  let zone = $state<SaveZone>(initial);
  // svelte-ignore state_referenced_locally
  let format = $state<SaveFormat>(initial === "page" && !adjusted ? "original" : "png");
  // svelte-ignore state_referenced_locally
  let adjust = $state(adjusted);
  let box: HTMLDivElement;

  // El original solo tiene sentido con la página entera y sin ajustes.
  const originalOk = $derived(zone === "page" && !(adjusted && adjust));
  $effect(() => {
    if (!originalOk && format === "original") format = "png";
  });

  onMount(() => box.focus());

  function key(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      oncancel();
    } else if (e.key === "Enter" && !(e.target instanceof HTMLButtonElement)) {
      e.preventDefault();
      onsave({ format, zone, adjust: adjusted && adjust });
    }
  }
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && oncancel()}>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("saveImgTitle")} tabindex="-1" bind:this={box} onkeydown={key}>
    <header><b>💾 {t("saveImgTitle")}</b></header>
    <div class="content">
      <div class="row">
        <span class="lbl">{t("saveImgZone")}</span>
        <div class="seg">
          <button class:on={zone === "page"} onclick={() => (zone = "page")}>{t("saveImgPage")}</button>
          <button class:on={zone === "visible"} disabled={!canVisible} onclick={() => (zone = "visible")} title={canVisible ? "" : t("saveImgVisibleNo")}>{t("saveImgVisible")}</button>
          <button class:on={zone === "selection"} disabled={!canSelection} onclick={() => (zone = "selection")} title={canSelection ? "" : t("saveImgSelectionNo")}>{t("saveImgSelection")}</button>
        </div>
      </div>
      <div class="row">
        <span class="lbl">{t("saveImgFormat")}</span>
        <div class="seg">
          <button class:on={format === "original"} disabled={!originalOk} onclick={() => (format = "original")} title={t("saveImgOriginalHelp")}>{t("saveImgOriginal", { f: original })}</button>
          <button class:on={format === "png"} onclick={() => (format = "png")}>PNG</button>
          <button class:on={format === "jpeg"} onclick={() => (format = "jpeg")}>JPEG</button>
        </div>
      </div>
      {#if adjusted}
        <label class="check"><input type="checkbox" bind:checked={adjust} /> {t("saveImgAdjust")}</label>
      {/if}
      <p class="note">{format === "original" ? t("saveImgOriginalHelp") : t("saveImgRenderHelp")}</p>
    </div>
    <footer>
      <button onclick={oncancel}>{t("cancel")}</button>
      <button class="primary" onclick={() => onsave({ format, zone, adjust: adjusted && adjust })}>{t("saveImgGo")}</button>
    </footer>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 60; background: rgb(0 0 0 / 0.55); display: grid; place-items: center; }
  .dialog {
    width: min(500px, calc(100vw - 40px)); display: flex; flex-direction: column; outline: none;
    background: var(--panel); border: 1px solid var(--border); border-radius: 10px; box-shadow: 0 12px 40px rgb(0 0 0 / 0.5); font-size: 13px;
  }
  header { padding: 8px 12px; border-bottom: 1px solid var(--border); }
  .content { padding: 12px 14px; display: flex; flex-direction: column; gap: 10px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .lbl { color: var(--muted); font-size: 12px; }
  .seg { display: flex; }
  .seg button { border-radius: 0; font-size: 12px; padding: 3px 9px; white-space: nowrap; }
  .seg button:first-child { border-radius: 6px 0 0 6px; }
  .seg button:last-child { border-radius: 0 6px 6px 0; }
  .seg button + button { border-left: none; }
  .seg button.on { background: var(--accent); color: #04121b; border-color: var(--accent); }
  .check { display: flex; align-items: center; gap: 6px; cursor: pointer; }
  .note { margin: 0; color: var(--muted); font-size: 11.5px; line-height: 1.4; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
  .primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
</style>
