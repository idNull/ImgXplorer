<script lang="ts">
  // Resumen del pedido: la lista exacta de lo que se va a pedir al proveedor, con caracteres, coste
  // aproximado y lo que sale gratis (ya generado). Se puede quitar o retocar cada fragmento. Nada
  // se envía sin pulsar «Generar»; una IA tampoco puede saltárselo.
  import { prefs } from "../prefs.svelte";
  import { t } from "../i18n.svelte";
  import { order, rowCost, type OrderRow } from "./voices.svelte";

  const rows = $derived(order.rows ?? []);
  const active = $derived(rows.filter((r) => r.on));
  const paid = $derived(active.filter((r) => !r.cached));
  const chars = $derived(paid.reduce((n, r) => n + rowCost(r).chars, 0));
  const usd = $derived(paid.reduce((n, r) => n + rowCost(r).usd, 0));
  const real = $derived(prefs.settings.voicesProvider !== "mock");
  const needConsent = $derived(real && !prefs.settings.voicesConsent);
  let consent = $state(false);
  const limit = $derived(prefs.settings.voicesLimit);
  const over = $derived(limit > 0 && order.session + chars > limit);

  function textOf(r: OrderRow): string {
    const it = r.item;
    return it.kind === "speech" || it.kind === "sfx" ? it.text : it.kind === "music" ? it.prompt : it.description;
  }
  function setText(r: OrderRow, v: string) {
    const it = r.item;
    if (it.kind === "music") it.prompt = v;
    else if (it.kind === "design") it.description = v;
    else it.text = v;
    r.cached = false;
  }
  const ICON = { speech: "🗣", sfx: "💥", music: "🎵", design: "🎙" };
  const icon = (r: OrderRow) => ICON[r.item.kind];
  const money = (v: number) => (v < 0.01 && v > 0 ? "< 0,01 $" : `${v.toFixed(2).replace(".", prefs.settings.lang === "en" ? "." : ",")} $`);

  function accept() {
    if (needConsent) {
      if (!consent) return;
      prefs.settings.voicesConsent = true;
      prefs.saveSettings();
    }
    order.finish(true);
  }

  function key(e: KeyboardEvent) {
    if (!order.rows) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopImmediatePropagation();
      order.finish(false);
    }
  }
</script>

<svelte:window onkeydowncapture={key} />

{#if order.rows}
  <div class="backdrop" role="presentation">
    <div class="dialog" role="dialog" aria-modal="true" aria-label={t("voicesOrderTitle")}>
      <header>
        <b>🗣 {t("voicesOrderTitle")}</b>
        {#if order.fromAi}<span class="ai">🤖 {t("voicesOrderFromAi")}</span>{/if}
      </header>
      <p class="note">{real ? t("voicesOrderHelp") : t("voicesOrderMock")}</p>
      <ul>
        {#each rows as r, i (i)}
          {@const c = rowCost(r)}
          <li class:off={!r.on}>
            <input type="checkbox" bind:checked={r.on} aria-label={t("voicesOrderInclude")} />
            <span class="who"><span class="dot" style:background={r.color ?? "var(--muted)"}></span>{icon(r)} {r.who}</span>
            <textarea rows="2" value={textOf(r)} disabled={!r.on} oninput={(e) => setText(r, e.currentTarget.value)}></textarea>
            <span class="cost">
              {#if r.cached}<span class="free" title={t("voicesCachedHelp")}>{t("voicesCached")}</span>
              {:else if r.item.kind === "speech"}{c.chars} {t("voicesChars")}{#if real}<br /><small>≈ {money(c.usd)}</small>{/if}
              {:else if r.item.kind === "design"}<small>{t("voicesDesignCost")}</small>
              {:else if real}<small>≈ {money(c.usd)}</small>{/if}
            </span>
          </li>
        {/each}
      </ul>
      <footer>
        <p class="sum">
          {t("voicesOrderSum", { n: active.length, chars, free: active.length - paid.length })}
          {#if real}· ≈ {money(usd)}{/if}
          {#if limit > 0}<br /><span class:bad={over}>{t("voicesOrderSession", { used: order.session, limit })}{over ? ` · ${t("voicesOverLimit")}` : ""}</span>{/if}
        </p>
        {#if needConsent}
          <label class="consent"><input type="checkbox" bind:checked={consent} /> {t("voicesConsent")}</label>
        {/if}
        <div class="buttons">
          <button onclick={() => order.finish(false)}>{t("cancel")}</button>
          <button class="primary" disabled={!active.length || over || (needConsent && !consent)} onclick={accept}>
            {rows.every((r) => r.item.kind === "design") ? t("voicesDesign") : paid.length ? t("voicesGenerate", { n: active.length }) : t("voicesUseCached")}
          </button>
        </div>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; z-index: 75; background: rgb(0 0 0 / 0.55); display: grid; place-items: center; }
  .dialog {
    width: min(720px, calc(100vw - 32px)); max-height: calc(100vh - 48px); display: flex; flex-direction: column;
    background: var(--panel); border: 1px solid var(--border); border-radius: 10px; box-shadow: var(--shadow); font-size: 13px;
  }
  header { display: flex; align-items: center; gap: 10px; padding: 10px 14px 4px; }
  header b { flex: 1; font-size: 14px; }
  .ai { font-size: 12px; color: #fbbf24; }
  .note { margin: 0 14px 6px; color: var(--muted); font-size: 12px; line-height: 1.45; }
  ul { list-style: none; margin: 0; padding: 0 14px; overflow-y: auto; flex: 1; }
  li { display: grid; grid-template-columns: auto 130px 1fr 92px; gap: 8px; align-items: start; padding: 6px 0; border-bottom: 1px solid rgb(42 47 56 / 0.6); }
  li.off { opacity: 0.5; }
  li input[type="checkbox"] { margin-top: 4px; }
  .who { display: flex; align-items: center; gap: 5px; font-weight: 600; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding-top: 2px; }
  .dot { width: 9px; height: 9px; border-radius: 50%; flex: none; }
  textarea {
    width: 100%; box-sizing: border-box; resize: vertical; font: inherit; color: var(--text); background: var(--panel-2);
    border: 1px solid var(--border); border-radius: 6px; padding: 3px 6px; line-height: 1.4;
  }
  .cost { text-align: right; font-variant-numeric: tabular-nums; font-size: 12px; color: var(--muted); padding-top: 2px; }
  .free { color: #86efac; font-weight: 600; }
  footer { padding: 8px 14px 12px; border-top: 1px solid var(--border); display: flex; flex-direction: column; gap: 6px; }
  .sum { margin: 0; font-size: 12.5px; }
  .bad { color: #fca5a5; }
  .consent { display: flex; gap: 6px; align-items: flex-start; font-size: 12px; color: #fde68a; line-height: 1.4; }
  .buttons { display: flex; justify-content: flex-end; gap: 8px; }
  .primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
</style>
