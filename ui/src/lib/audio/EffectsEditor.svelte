<script lang="ts">
  // Cadena de efectos de audio de una pista o de un clip: cada efecto con su interruptor, su
  // ajuste predefinido y sus mandos; se pueden reordenar y quitar.
  import { FX, FX_TYPES, formatParam, fromSlider, newEffect, paramValue, toSlider, type Effect, type FxType } from "./effects";
  import { label, t } from "../i18n.svelte";

  interface Props {
    effects: Effect[];
    /** Recibe la cadena entera ya cambiada. */
    onchange: (list: Effect[]) => void;
    disabled?: boolean;
  }
  let { effects, onchange, disabled = false }: Props = $props();

  /** Efectos plegados (por id): por defecto se ven abiertos. */
  let folded = $state<string[]>([]);

  const copy = (): Effect[] => effects.map((e) => ({ ...e, params: { ...e.params } }));
  function change(i: number, fn: (e: Effect) => void) {
    const list = copy();
    fn(list[i]);
    onchange(list);
  }
  function setParam(i: number, key: string, v: number) {
    change(i, (e) => {
      e.params[key] = v;
      delete e.preset;
    });
  }
  function setPreset(i: number, id: string) {
    change(i, (e) => {
      const p = FX[e.type].presets.find((x) => x.id === id);
      if (!p) return;
      e.preset = p.id;
      e.params = { ...p.params };
    });
  }
  function move(i: number, d: -1 | 1) {
    const list = copy();
    const j = i + d;
    if (j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
    onchange(list);
  }
  function add(type: FxType) {
    onchange([...copy(), newEffect(type)]);
  }
  const fold = (id: string) => (folded = folded.includes(id) ? folded.filter((x) => x !== id) : [...folded, id]);
</script>

<div class="fx">
  {#each effects as e, i (e.id)}
    {@const def = FX[e.type]}
    {@const open = !folded.includes(e.id)}
    <div class="card" class:off={!e.on}>
      <div class="head">
        <input type="checkbox" checked={e.on} {disabled} title={t("fxOnHelp")} onchange={(ev) => change(i, (x) => (x.on = ev.currentTarget.checked))} />
        <button class="name" onclick={() => fold(e.id)}>{open ? "▾" : "▸"} {def?.icon} {label(`fx.${e.type}`)}</button>
        <button class="mini" disabled={disabled || i === 0} title={t("trackUp")} onclick={() => move(i, -1)}>▲</button>
        <button class="mini" disabled={disabled || i === effects.length - 1} title={t("trackDown")} onclick={() => move(i, 1)}>▼</button>
        <button class="mini x" {disabled} title={t("remove")} onclick={() => onchange(copy().filter((_, k) => k !== i))}>✕</button>
      </div>
      {#if open && def}
        <select class="preset" value={e.preset ?? ""} {disabled} onchange={(ev) => setPreset(i, ev.currentTarget.value)}>
          {#if !e.preset}<option value="">{t("fxCustom")}</option>{/if}
          {#each def.presets as p (p.id)}<option value={p.id}>{label(`fxp.${e.type}.${p.id}`)}</option>{/each}
        </select>
        {#each def.params as p (p.key)}
          {@const v = paramValue(e, p)}
          <label class="param">
            <span>{label(`fxk.${e.type}.${p.key}`)}</span>
            {#if p.unit === "onoff"}
              <input type="checkbox" checked={v >= 0.5} {disabled} onchange={(ev) => setParam(i, p.key, ev.currentTarget.checked ? 1 : 0)} />
            {:else}
              <input type="range" min="0" max="1000" step="1" value={Math.round(toSlider(p, v) * 1000)} {disabled}
                oninput={(ev) => setParam(i, p.key, fromSlider(p, Number(ev.currentTarget.value) / 1000))}
                ondblclick={() => setParam(i, p.key, p.def)} />
            {/if}
            <output>{formatParam(p, v)}</output>
          </label>
        {/each}
      {/if}
    </div>
  {:else}
    <p class="none">{t("effectsNone")}</p>
  {/each}
  <select class="add" value="" {disabled} onchange={(ev) => { const v = ev.currentTarget.value as FxType; ev.currentTarget.value = ""; if (v) add(v); }}>
    <option value="">{t("fxAdd")}</option>
    {#each FX_TYPES as type (type)}<option value={type}>{FX[type].icon} {label(`fx.${type}`)}</option>{/each}
  </select>
</div>

<style>
  .fx { display: flex; flex-direction: column; gap: 6px; }
  .card { border: 1px solid var(--border); border-radius: 6px; padding: 4px 6px; display: flex; flex-direction: column; gap: 4px; }
  .card.off { opacity: 0.55; }
  .head { display: flex; align-items: center; gap: 4px; }
  .name { flex: 1; min-width: 0; text-align: left; background: none; border: none; padding: 0; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mini { padding: 0 4px; font-size: 10px; line-height: 16px; }
  .x { background: none; border-color: transparent; color: var(--muted); }
  .x:hover:not(:disabled) { color: #fca5a5; }
  select {
    font: inherit; font-size: 12px; color: var(--text); background: var(--panel-2); border: 1px solid var(--border);
    border-radius: 6px; padding: 2px 4px; min-width: 0;
  }
  .param { display: grid; grid-template-columns: minmax(0, 1fr) 90px 54px; align-items: center; gap: 6px; font-size: 11px; }
  .param span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .param input[type="range"] { width: 100%; }
  .param output { text-align: right; color: var(--muted); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .none { margin: 0; color: var(--muted); font-size: 12px; }
</style>
