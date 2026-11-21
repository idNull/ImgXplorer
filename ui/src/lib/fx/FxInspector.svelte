<script lang="ts">
  // Propiedades de un efecto especial: ajuste predefinido, intensidad, tiempo, mandos y punto
  // de la imagen donde ocurre.
  import { FX_ICON, formatFxParam, fxInfo, fxParam, type FxParam } from "./catalog";
  import { MIN_FX_DURATION, type Fx } from "./fx";
  import { label, t } from "../i18n.svelte";

  interface Props {
    fx: Fx;
    edit: (fn: (f: Fx) => void) => void;
    /** Esperando el clic en la imagen para el punto. */
    picking: boolean;
    onpick: () => void;
    onseek: (t: number) => void;
    onremove: () => void;
    onduplicate: () => void;
  }
  let { fx, edit, picking, onpick, onseek, onremove, onduplicate }: Props = $props();

  const info = $derived(fxInfo(fx.type));
  const num = (e: Event) => Number((e.currentTarget as HTMLInputElement).value);
  const fmt = (v: number) => Number(v.toFixed(2));

  function setParam(p: FxParam, v: number | string) {
    edit((f) => {
      f.params = { ...(f.params ?? {}), [p.key]: v };
      delete f.preset;
    });
  }

  // Lo que se está escribiendo en los textos, aún sin aplicar (por efecto y mando).
  let drafts = $state<Record<string, string>>({});
  const draftKey = (p: FxParam) => `${fx.id}:${p.key}`;
  const pending = (p: FxParam) => {
    const d = drafts[draftKey(p)];
    return d !== undefined && d !== fxParam(fx, p);
  };
  $effect(() => {
    // Otro efecto seleccionado: lo escrito en el anterior ya se aplicó al salir del cuadro.
    void fx.id;
    drafts = {};
  });

  function applyText(p: FxParam) {
    const d = drafts[draftKey(p)];
    if (d !== undefined && d !== fxParam(fx, p)) setParam(p, d);
    delete drafts[draftKey(p)];
  }
  /** Aplica todo lo escrito y sin aplicar (antes de cambiar de ajuste o de estilo). */
  function applyTexts() {
    for (const p of info?.params ?? []) if (p.kind === "text") applyText(p);
  }

  function setPreset(id: string) {
    const p = info?.presets.find((x) => x.id === id);
    if (!p) return;
    applyTexts();
    // Los textos escritos se quedan, salvo que el ajuste traiga el suyo (onomatopeyas).
    const keep = Object.fromEntries(
      (info?.params ?? []).filter((q) => q.kind === "text" && !p.params.some(([k]) => k === q.key) && fx.params?.[q.key] !== undefined).map((q) => [q.key, fx.params![q.key]]),
    );
    edit((f) => {
      f.preset = p.id;
      f.params = { ...Object.fromEntries(p.params), ...keep };
      if (!Object.keys(f.params).length) delete f.params;
    });
  }

  /** Un mando de lista. En el texto, el estilo (subtítulo, cartel, narración) trae su tamaño y color. */
  function setChoice(p: FxParam, v: string) {
    applyTexts();
    if (fx.type === "caption" && p.key === "style" && info?.presets.some((x) => x.id === v)) setPreset(v);
    else setParam(p, v);
  }

  function textKey(e: KeyboardEvent, p: FxParam) {
    const area = e.currentTarget instanceof HTMLTextAreaElement;
    if (e.key === "Enter" && (!area || e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      applyText(p);
    } else if (e.key === "Escape" && drafts[draftKey(p)] !== undefined) {
      e.stopPropagation();
      delete drafts[draftKey(p)];
    }
  }

  /** Posición del deslizador (0..1000) para un valor, y al revés. */
  const toSlider = (p: Extract<FxParam, { kind: "number" }>, v: number) => Math.round(((v - p.min) / (p.max - p.min)) * 1000);
  const fromSlider = (p: Extract<FxParam, { kind: "number" }>, s: number) => {
    const v = p.min + (s / 1000) * (p.max - p.min);
    return Number((Math.round(v / p.step) * p.step).toFixed(4));
  };
</script>

<h5>{FX_ICON[fx.type] ?? "?"} {label(`vfx.${fx.type}`)}</h5>
{#if !info}
  <p class="note">{t("fxUnknown", { type: fx.type })}</p>
{:else}
  <p class="note">{label(`vfxh.${fx.type}`)}</p>
  <label class="row">
    <span>{t("fxPreset")}</span>
    <select value={fx.preset ?? ""} onchange={(e) => setPreset(e.currentTarget.value)}>
      {#if !fx.preset}<option value="">{t("fxCustom")}</option>{/if}
      {#each info.presets as p (p.id)}<option value={p.id}>{label(`vfxp.${fx.type}.${p.id}`)}</option>{/each}
    </select>
  </label>
  <label class="param">
    <span>{t("fxIntensity")}</span>
    <input type="range" min="0" max="300" step="5" value={Math.round((fx.intensity ?? 1) * 100)}
      oninput={(e) => edit((f) => (f.intensity = num(e) / 100))} ondblclick={() => edit((f) => delete f.intensity)} />
    <output>{Math.round((fx.intensity ?? 1) * 100)} %</output>
  </label>
  <div class="times">
    <label>
      <span>{t("start")}</span>
      <input type="number" min="0" step="0.05" value={fmt(fx.start)} onchange={(e) => { const v = num(e); if (Number.isFinite(v)) edit((f) => (f.start = Math.max(0, v))); }} />
    </label>
    <label>
      <span>{t("duration")}</span>
      <input type="number" min={MIN_FX_DURATION} step="0.05" value={fmt(fx.duration)}
        onchange={(e) => { const v = num(e); if (Number.isFinite(v)) edit((f) => (f.duration = Math.max(MIN_FX_DURATION, v))); }} />
    </label>
    <button class="go" title={t("fxGoTo")} onclick={() => onseek(fx.start)}>⏵</button>
  </div>
  {#each info.params as p (p.key)}
    {@const v = fxParam(fx, p)}
    {#if p.kind === "text"}
      <label class="text">
        <span>{label(`vfxk.${p.key}`)}</span>
        {#if fx.type === "caption"}
          <textarea rows="3" maxlength={p.max} value={drafts[draftKey(p)] ?? v} placeholder={t("fxTextHint")}
            oninput={(e) => (drafts[draftKey(p)] = e.currentTarget.value)} onchange={() => applyText(p)} onkeydown={(e) => textKey(e, p)}></textarea>
        {:else}
          <input type="text" maxlength={p.max} value={drafts[draftKey(p)] ?? v}
            oninput={(e) => (drafts[draftKey(p)] = e.currentTarget.value)} onchange={() => applyText(p)} onkeydown={(e) => textKey(e, p)} />
        {/if}
      </label>
      <div class="apply">
        <span class="hint">{pending(p) ? t("fxTextPending") : fx.type === "caption" ? t("fxTextApplyHint") : ""}</span>
        <button class:primary={pending(p)} disabled={!pending(p)} onclick={() => applyText(p)} title={t("fxTextApplyHint")}>✓ {t("fxTextApply")}</button>
      </div>
    {:else}
    <label class="param">
      <span>{label(`vfxk.${p.key}`)}</span>
      {#if p.kind === "number"}
        <input type="range" min="0" max="1000" step="1" value={toSlider(p, v as number)}
          oninput={(e) => setParam(p, fromSlider(p, num(e)))} ondblclick={() => setParam(p, p.default)} />
        <output>{formatFxParam(p, v)}</output>
      {:else if p.kind === "choice"}
        <select class="wide" value={v} onchange={(e) => setChoice(p, e.currentTarget.value)}>
          {#each p.options as o (o)}<option value={o}>{label(`vfxo.${o}`)}</option>{/each}
        </select>
      {:else}
        <input class="wide color" type="color" value={v} onchange={(e) => setParam(p, e.currentTarget.value)} />
      {/if}
    </label>
    {/if}
  {/each}
  {#if fx.type !== "caption"}
  <div class="point">
    <span>{t("fxPoint")}</span>
    <span class="where">{fx.at ? `${Math.round(fx.at.x * 100)} %, ${Math.round(fx.at.y * 100)} %` : t("fxPointCenter")}</span>
    <button class:on={picking} onclick={onpick} title={t("fxPickHelp")}>⌖ {picking ? t("fxPicking") : t("fxPick")}</button>
    {#if fx.at}<button title={t("fxPointClear")} onclick={() => edit((f) => delete f.at)}>✕</button>{/if}
  </div>
  {/if}
{/if}
<div class="buttons">
  <button onclick={onduplicate}>{t("duplicate")}</button>
  <button onclick={onremove}>{t("remove")}</button>
</div>

<style>
  h5 { margin: 0; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .note { color: var(--muted); font-size: 11px; margin: 0; line-height: 1.4; }
  .row { display: grid; grid-template-columns: 1fr 140px; align-items: center; gap: 6px; font-size: 12px; }
  select, input[type="number"] {
    width: 100%; box-sizing: border-box; font: inherit; font-size: 12px; color: var(--text); background: var(--panel-2);
    border: 1px solid var(--border); border-radius: 6px; padding: 2px 4px; min-width: 0;
  }
  .param { display: grid; grid-template-columns: minmax(0, 1fr) 100px 54px; align-items: center; gap: 6px; font-size: 11px; }
  .param span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .param input[type="range"] { width: 100%; }
  .param output { text-align: right; color: var(--muted); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .param .wide { grid-column: 2 / 4; }
  .param .color { height: 22px; padding: 0; border: 1px solid var(--border); border-radius: 4px; background: none; }
  .times { display: grid; grid-template-columns: 1fr 1fr auto; gap: 6px; align-items: end; font-size: 11px; }
  .times label { display: flex; flex-direction: column; gap: 2px; color: var(--muted); }
  .go { padding: 2px 8px; }
  .point { display: flex; align-items: center; gap: 6px; font-size: 12px; flex-wrap: wrap; }
  .where { flex: 1; color: var(--muted); font-variant-numeric: tabular-nums; }
  .point button { padding: 2px 8px; }
  .point button.on { border-color: #fbbf24; color: #fbbf24; }
  .buttons { display: flex; gap: 6px; flex-wrap: wrap; }
  .text { display: flex; flex-direction: column; gap: 3px; font-size: 11px; }
  .apply { display: flex; align-items: center; justify-content: flex-end; gap: 8px; margin: -2px 0 4px; }
  .apply .hint { flex: 1; font-size: 11px; color: var(--muted); }
  .apply button { font-size: 12px; padding: 2px 10px; }
  .apply button.primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
  .text input, .text textarea {
    width: 100%; box-sizing: border-box; font: inherit; font-size: 13px; color: var(--text); background: var(--panel-2);
    border: 1px solid var(--border); border-radius: 6px; padding: 4px 6px; resize: vertical;
  }
</style>
