<script lang="ts">
  // Menú para añadir efectos: situaciones listas (varios efectos y su sonido) o un efecto suelto.
  import { FX_ICON, SITUATION_ICON, fxCatalog } from "./catalog";
  import { label, t } from "../i18n.svelte";

  interface Props {
    /** Segundo donde se añadirá (se muestra en el título). */
    at: number;
    /** Con el sonido de la situación. */
    withSound: boolean;
    onsound: (on: boolean) => void;
    /** Con la onomatopeya de la situación («¡BOOM!»). */
    withText: boolean;
    ontext: (on: boolean) => void;
    onpick: (p: { situation: string } | { type: string }) => void;
    onclose: () => void;
  }
  let { at, withSound, onsound, withText, ontext, onpick, onclose }: Props = $props();
  const cat = fxCatalog();
</script>

<svelte:window onkeydowncapture={(e) => e.key === "Escape" && (e.stopPropagation(), onclose())} />
<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="backdrop" onclick={onclose}></div>
<div class="picker" role="dialog" aria-label={t("fxAddTitle")}>
  <div class="head">
    <b>✨ {t("fxAddTitle")}</b>
    <span class="at">{t("fxAt", { t: at.toFixed(2) })}</span>
    <button class="x" onclick={onclose} title={t("close")}>✕</button>
  </div>
  <h6>{t("fxSituations")}</h6>
  <div class="grid">
    {#each cat.situations as s (s.id)}
      <button class="opt" title={label(`vfxsh.${s.id}`)} onclick={() => onpick({ situation: s.id })}>
        <span class="icon">{SITUATION_ICON[s.id] ?? "✨"}</span>{label(`vfxs.${s.id}`)}{#if s.text || s.sound}<span class="snd">{s.text ? "💬" : ""}{s.sound ? "♪" : ""}</span>{/if}
      </button>
    {/each}
  </div>
  <div class="opts">
    <label class="sound"><input type="checkbox" checked={withSound} onchange={(e) => onsound(e.currentTarget.checked)} /> ♪ {t("fxWithSound")}</label>
    <label class="sound"><input type="checkbox" checked={withText} onchange={(e) => ontext(e.currentTarget.checked)} /> 💬 {t("fxWithText")}</label>
  </div>
  <h6>{t("fxSingle")}</h6>
  <div class="grid">
    {#each cat.effects as e (e.type)}
      <button class="opt" title={label(`vfxh.${e.type}`)} onclick={() => onpick({ type: e.type })}><span class="icon">{FX_ICON[e.type] ?? "?"}</span>{label(`vfx.${e.type}`)}</button>
    {/each}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 30; }
  .picker {
    position: fixed; left: 50%; bottom: 20px; transform: translateX(-50%); z-index: 31; width: min(700px, calc(100vw - 30px));
    max-height: 70vh; overflow-y: auto; background: var(--panel); border: 1px solid var(--border); border-radius: 10px;
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.5); padding: 10px 12px; display: flex; flex-direction: column; gap: 6px; font-size: 12px;
  }
  .head { display: flex; align-items: center; gap: 8px; }
  .head b { font-size: 13px; }
  .at { flex: 1; color: var(--muted); }
  .x { padding: 0 6px; background: none; border-color: transparent; color: var(--muted); }
  h6 { margin: 4px 0 0; font-size: 11px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); font-weight: 600; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 4px; }
  .opt { display: flex; align-items: center; gap: 6px; text-align: left; padding: 5px 8px; font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .icon { width: 18px; text-align: center; flex: none; }
  .snd { margin-left: auto; color: var(--muted); font-size: 11px; }
  .sound { display: flex; align-items: center; gap: 6px; color: var(--muted); }
  .opts { display: flex; gap: 16px; flex-wrap: wrap; }
</style>
