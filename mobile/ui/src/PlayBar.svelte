<script lang="ts">
  // Controles de la reproducción en el móvil: anterior, pausa, siguiente, el avance de la imagen
  // actual, la velocidad y parar. Debajo, el silencio, el pase de fotos (en un álbum), el encuadre
  // (si la historia lo trae) y la calidad.
  import { SPEEDS, type Player, type PlayerInfo } from "$imgx/action/player";
  import { t } from "./lib/i18n";
  import type { Quality, StoryFit } from "./lib/reading";

  let { info, count, player, fit, quality, muted, onstop, onfit, onquality, onreplay, onmute, show = null, onshow }: {
    info: PlayerInfo;
    count: number;
    player: () => Player | null;
    /** `null`: la historia no dice para qué proporción se pensó (no hay nada que elegir). */
    fit: StoryFit | null;
    quality: Quality;
    muted: boolean;
    onstop: () => void;
    onfit: () => void;
    onquality: () => void;
    onreplay: () => void;
    onmute: () => void;
    /** Resumen del pase de fotos («Fundido · 4 s») en un álbum; `null` en historias y cómics. */
    show?: string | null;
    onshow?: () => void;
  } = $props();

  const progress = $derived(info.duration > 0 ? Math.min(1, info.t / info.duration) : 0);
  const running = $derived(!["paused", "ended"].includes(info.state));
  const chip = $derived(
    info.state === "loading" ? t("stLoading") : info.state === "override" || info.state === "returning" ? t("stManual") : "",
  );
  /** Toque: más rápido; pasado 2×, vuelve a la más lenta. */
  function cycleSpeed() {
    const p = player();
    if (!p) return;
    if (info.speed < SPEEDS[SPEEDS.length - 1]) return p.changeSpeed(1);
    for (let i = 1; i < SPEEDS.length; i++) p.changeSpeed(-1);
  }
  const fitLabel = { auto: () => `🔄 ${t("fitAuto")}`, authored: () => `🎬 ${t("fitAuthored")}`, screen: () => `📱 ${t("fitScreen")}` };
  const qualityLabel = { auto: () => `⚙ ${t("qualityAuto")}`, high: () => `✦ ${t("qualityHigh")}`, saver: () => `🔋 ${t("qualitySaver")}` };
</script>

<div class="bar">
  <div class="row main">
    <button onclick={() => player()?.goTo(info.index - 1)} disabled={info.index === 0} aria-label={t("prev")}>⏮&#xFE0E;</button>
    {#if info.state === "ended" && info.index >= count - 1}
      <button class="big" onclick={onreplay} aria-label={t("replay")}>↻</button>
    {:else}
      <button class="big" onclick={() => player()?.toggle()} aria-label={running ? t("pause") : t("play")}>{running ? "⏸\uFE0E" : "▶\uFE0E"}</button>
    {/if}
    <button onclick={() => player()?.goTo(info.index + 1)} disabled={info.index >= count - 1} aria-label={t("next")}>⏭&#xFE0E;</button>
    <div class="track"><div class="fill" style:width="{progress * 100}%"></div></div>
    <button class="speed" onclick={cycleSpeed} aria-label={t("speed")}>{info.speed}×</button>
    <button onclick={onstop} aria-label={t("stop")}>⏹&#xFE0E;</button>
  </div>
  <div class="row extra">
    {#if chip}<span class="chip">{chip}</span>{/if}
    {#if info.state === "ended" && info.index >= count - 1}<span class="chip end">{t("ended")}</span>{/if}
    <button class="pill" class:off={muted} onclick={onmute} aria-label={muted ? t("unmute") : t("mute")}>{muted ? "🔇" : "🔊"}</button>
    {#if show}
      <button class="pill" onclick={onshow} title={t("showHint")} aria-haspopup="dialog">✨ {show}</button>
    {/if}
    {#if fit}
      <button class="pill" onclick={onfit} title={t("fitHint")}>{fitLabel[fit]()}</button>
    {/if}
    <button class="pill" onclick={onquality} title={t("qualityHint")}>{qualityLabel[quality]()}</button>
  </div>
</div>

<style>
  .bar { display: flex; flex-direction: column; gap: 8px; }
  .row { display: flex; align-items: center; gap: 6px; }
  .extra { justify-content: center; flex-wrap: wrap; }
  .main button {
    border: 0; background: none; color: var(--text); font-size: 20px; padding: 6px 8px; min-width: 40px;
  }
  .main button:disabled { opacity: 0.35; }
  .main .big { font-size: 26px; }
  .main .speed { font-size: 14px; font-variant-numeric: tabular-nums; min-width: 44px; }
  .track { flex: 1; height: 4px; background: #2a313b; border-radius: 2px; overflow: hidden; }
  .fill { height: 100%; background: var(--accent); }
  .pill {
    border: 0; border-radius: 999px; padding: 8px 14px; font: inherit; font-size: 14px;
    background: #2a313b; color: var(--text);
  }
  .pill.off { background: #4a2a2e; }
  .chip { font-size: 13px; color: var(--muted); padding: 4px 8px; }
  .chip.end { color: var(--accent); font-weight: 600; }
</style>
