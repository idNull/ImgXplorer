<script lang="ts">
  // Propiedades de lo seleccionado en la timeline: un clip (tiempo, duración, velocidad,
  // recortes) o una clave (instante, easing, pose).
  import {
    EASINGS, MIN_DURATION, easePoints, inline, keyTimes, moveKey, poseObj, setSpeed, setTrim, speedOf,
    type ClipInstance, type Easing, type EasingType, type PoseObj, type Timeline,
  } from "./model";
  import { label, t } from "../i18n.svelte";
  import type { Pose } from "../core";

  interface Props {
    tl: Timeline;
    clip: number;
    key?: number;
    /** La timeline sale de una plantilla (al editar se convierte en clips propios). */
    fromTemplate: boolean;
    edit: (fn: (tl: Timeline) => void) => void;
    currentPose: () => Pose;
    onseek: (t: number) => void;
    onremove: () => void;
    onduplicate: () => void;
    onsave: () => void;
  }
  let { tl, clip, key, fromTemplate, edit, currentPose, onseek, onremove, onduplicate, onsave }: Props = $props();

  const inst = $derived<ClipInstance | undefined>(tl.camera[clip]);
  const c = $derived(inst ? inline(inst) : null);
  const kf = $derived(c?.type === "keyframes" && key !== undefined ? c.keys[key] : undefined);
  const keyAt = $derived(inst && key !== undefined ? keyTimes(inst)[key] : undefined);

  const fmt = (v: number, d = 2) => Number(v.toFixed(d));
  const num = (e: Event) => Number((e.currentTarget as HTMLInputElement).value);
  const finite = (v: number) => Number.isFinite(v);

  function onInst(fn: (i: ClipInstance) => void) {
    edit((tl) => {
      const i = tl.camera[clip];
      if (i) fn(i);
    });
  }

  function onKey(fn: (k: { t: number; pose: PoseObj; easing: Easing }) => void) {
    edit((tl) => {
      const cc = tl.camera[clip] && inline(tl.camera[clip]);
      if (cc?.type === "keyframes" && key !== undefined && cc.keys[key]) fn(cc.keys[key]);
    });
  }

  /** Edita la clave `ki` del clip (su curva rige el tramo que va de ella a la siguiente). */
  function onKeyAt(ki: number, fn: (k: { easing: Easing }) => void) {
    edit((tl) => {
      const cc = tl.camera[clip] && inline(tl.camera[clip]);
      if (cc?.type === "keyframes" && cc.keys[ki]) fn(cc.keys[ki]);
    });
  }

  function setEasing(ki: number, type: EasingType) {
    onKeyAt(ki, (k) => {
      k.easing = type === "cubic-bezier" ? { type, x1: 0.42, y1: 0, x2: 0.58, y2: 1 } : { type };
    });
  }

  function setBezier(ki: number, field: "x1" | "y1" | "x2" | "y2", v: number) {
    if (!finite(v)) return;
    onKeyAt(ki, (k) => {
      if (k.easing.type !== "cubic-bezier") return;
      // x debe quedar en 0..1 para que la curva sea una función del tiempo.
      k.easing[field] = field.startsWith("x") ? Math.min(1, Math.max(0, v)) : Math.min(3, Math.max(-2, v));
    });
  }

  function setPoseField(field: keyof PoseObj, v: number) {
    if (!finite(v)) return;
    onKey((k) => {
      k.pose[field] = field === "zoom" ? Math.min(64, Math.max(0.05, v)) : v;
    });
  }

  const TYPE: Record<string, () => string> = {
    keyframes: () => t("clipKeyframes"),
    recorded: () => t("clipRecorded"),
    hold: () => t("clipHold"),
  };
</script>

{#snippet easing(ki: number, title: string)}
  {@const k = c?.type === "keyframes" ? c.keys[ki] : undefined}
  {#if k}
    <div class="ease">
      <div class="ease-head">
        <span>{title}</span>
        <svg viewBox="-2 -8 44 34" width="44" height="34" aria-hidden="true">
          <rect x="0" y="0" width="40" height="20" class="frame" />
          <polyline points={easePoints(k.easing, 0, 40, 20, 20)} />
        </svg>
      </div>
      <select value={k.easing.type} onchange={(e) => setEasing(ki, e.currentTarget.value as EasingType)}>
        {#each EASINGS as e (e)}<option value={e}>{label(`ease.${e}`)}</option>{/each}
      </select>
      {#if k.easing.type === "cubic-bezier"}
        {@const b = k.easing}
        <div class="bezier">
          {#each ["x1", "y1", "x2", "y2"] as const as f (f)}
            <label><span>{f}</span><input type="number" step="0.05" value={b[f]} onchange={(e) => setBezier(ki, f, num(e))} /></label>
          {/each}
        </div>
      {/if}
      <p class="note">{label(`ease.${k.easing.type}.help`)}</p>
    </div>
  {/if}
{/snippet}

{#if inst && c}
  {#if fromTemplate}<p class="note">{t("templateReadOnly")}</p>{/if}
  {#if kf && keyAt !== undefined}
    <h5>{t("keyframe")} {key! + 1} / {c.type === "keyframes" ? c.keys.length : 0}</h5>
    <label class="row">
      <span>{t("instant")}</span>
      <input type="number" step="0.05" min="0" value={fmt(keyAt)}
        onchange={(e) => { const v = num(e); if (finite(v)) { edit((tl) => moveKey(tl, { clip, key: key! }, v)); onseek(v); } }} />
    </label>
    {#if c.type === "keyframes"}
      {#if key! > 0}{@render easing(key! - 1, t("easingIn", { n: key! }))}{/if}
      {#if key! < c.keys.length - 1}{@render easing(key!, t("easingOut", { n: key! + 2 }))}{/if}
      {#if c.keys.length < 2}<p class="note">{t("easingSingle")}</p>{/if}
    {/if}
    <div class="pose">
      {#each [["cx", 0.01], ["cy", 0.01], ["zoom", 0.05], ["rot", 1]] as const as [f, step] (f)}
        <label><span>{t(`pose_${f}`)}</span><input type="number" {step} value={fmt(kf.pose[f], 3)} onchange={(e) => setPoseField(f, num(e))} /></label>
      {/each}
    </div>
    <div class="buttons">
      <button onclick={() => onKey((k) => (k.pose = poseObj(currentPose())))}>{t("useView")}</button>
      <button onclick={onremove}>{t("removeKeyframe")}</button>
    </div>
  {:else}
    <h5>{inst.name ?? TYPE[c.type]()}</h5>
    <label class="row">
      <span>{t("clipName")}</span>
      <input type="text" value={inst.name ?? ""} placeholder={TYPE[c.type]()}
        onchange={(e) => onInst((i) => { const v = e.currentTarget.value.trim(); if (v) i.name = v.slice(0, 120); else delete i.name; })} />
    </label>
    <div class="meta">
      {TYPE[c.type]()} ·
      {#if c.type === "keyframes"}{t("nKeys", { n: c.keys.length })}{:else if c.type === "recorded"}{t("nSamples", { n: c.samples.length })}{:else}{c.pose ? t("holdFixed") : t("holdPrev")}{/if}
    </div>
    <label class="row">
      <span>{t("start")}</span>
      <input type="number" step="0.1" min="0" value={fmt(inst.start)} onchange={(e) => { const v = num(e); if (finite(v)) onInst((i) => (i.start = Math.max(0, v))); }} />
    </label>
    <label class="row">
      <span>{t("duration")}</span>
      <input type="number" step="0.1" min={MIN_DURATION} value={fmt(inst.duration)}
        onchange={(e) => { const v = num(e); if (finite(v)) onInst((i) => (i.duration = Math.max(MIN_DURATION, v))); }} />
    </label>
    {#if c.type !== "hold"}
      <label class="row">
        <span>{t("speed")}</span>
        <input type="number" step="5" min="5" value={Math.round(speedOf(inst) * 100)}
          onchange={(e) => { const v = num(e); if (finite(v) && v > 0) onInst((i) => setSpeed(i, v / 100)); }} />
        <span class="unit">%</span>
      </label>
      <label class="row">
        <span>{t("trimIn")}</span>
        <input type="number" step="0.1" min="0" value={fmt(inst.trimIn)} onchange={(e) => { const v = num(e); if (finite(v)) onInst((i) => setTrim(i, "in", v)); }} />
      </label>
      <label class="row">
        <span>{t("trimOut")}</span>
        <input type="number" step="0.1" min="0" value={fmt(inst.trimOut)} onchange={(e) => { const v = num(e); if (finite(v)) onInst((i) => setTrim(i, "out", v)); }} />
      </label>
    {:else}
      <div class="buttons">
        <button onclick={() => onInst((i) => { const h = inline(i); if (h?.type === "hold") h.pose = poseObj(currentPose()); })}>{t("useView")}</button>
        {#if c.pose}<button onclick={() => onInst((i) => { const h = inline(i); if (h?.type === "hold") delete h.pose; })}>{t("holdPrev")}</button>{/if}
      </div>
    {/if}
    <div class="buttons">
      <button onclick={onsave}>{t("saveToLibrary")}</button>
      <button onclick={onduplicate}>{t("duplicate")}</button>
      <button onclick={onremove}>{t("remove")}</button>
    </div>
  {/if}
{/if}

<style>
  h5 { margin: 0; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row { display: grid; grid-template-columns: 1fr 86px 14px; align-items: center; gap: 6px; }
  .row > :nth-child(2):last-child { grid-column: 2 / 4; }
  input[type="number"], input[type="text"], select {
    width: 100%; box-sizing: border-box; font: inherit; color: var(--text); background: var(--panel-2);
    border: 1px solid var(--border); border-radius: 6px; padding: 2px 6px;
  }
  .row input[type="text"] { grid-column: 2 / 4; }
  .unit { color: var(--muted); }
  .meta, .note { color: var(--muted); font-size: 11px; margin: 0; line-height: 1.4; }
  .bezier, .pose { display: grid; grid-template-columns: 1fr 1fr; gap: 4px 8px; }
  .bezier label, .pose label { display: grid; grid-template-columns: 34px 1fr; align-items: center; gap: 4px; color: var(--muted); }
  .ease { display: flex; flex-direction: column; gap: 4px; padding: 6px; border: 1px solid var(--border); border-radius: 6px; }
  .ease-head { display: flex; align-items: center; justify-content: space-between; gap: 6px; font-size: 12px; }
  .ease svg { flex: none; overflow: visible; }
  .ease svg .frame { fill: none; stroke: var(--border); stroke-dasharray: 2 2; }
  .ease svg polyline { fill: none; stroke: var(--accent); stroke-width: 1.5; }
  .buttons { display: flex; flex-wrap: wrap; gap: 4px; }
  .buttons button { padding: 2px 8px; font-size: 12px; }
</style>
