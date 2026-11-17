<script lang="ts">
  // Controles flotantes durante la reproducción.
  import type { Player, PlayerInfo } from "./player";
  import { t } from "../i18n.svelte";
  import { prefs } from "../prefs.svelte";
  import { display, keysFor, type PlayerAction } from "../keys";

  const k = (a: PlayerAction) => {
    const first = keysFor(prefs.settings.keys, "player", a)[0];
    return first ? ` (${display(first)})` : "";
  };

  interface Props {
    info: PlayerInfo;
    count: number;
    player: () => Player | null;
    onstop: () => void;
  }
  let { info, count, player, onstop }: Props = $props();

  const progress = $derived(info.duration > 0 ? Math.min(1, info.t / info.duration) : 0);
  const chip = $derived(
    {
      override: t("stManual"),
      returning: t("stReturning"),
      ended: t("stEnded"),
      loading: t("stLoading"),
      paused: t("stPaused"),
      playing: "",
      transition: "",
    }[info.state],
  );
  const running = $derived(!["paused", "ended"].includes(info.state));
</script>

<div class="bar">
  <button onclick={() => player()?.goTo(info.index - 1)} disabled={info.index === 0} title="{t('prev')}{k('pagePrev')}">⏮</button>
  <button onclick={() => player()?.toggle()} title="{running ? t('pause') : t('play')}{k('toggle')}">{running ? "⏸" : "▶"}</button>
  <button onclick={() => player()?.goTo(info.index + 1)} disabled={info.index >= count - 1} title="{t('next')}{k('pageNext')}">⏭</button>
  <div class="track"><div class="fill" style:width="{progress * 100}%"></div></div>
  <span class="time">{info.t.toFixed(1)} / {info.duration.toFixed(1)} s</span>
  <button class="speed" onclick={() => player()?.changeSpeed(1)} title="{t('kSpeed')}{k('faster')}">{info.speed}×</button>
  {#if chip}<span class="chip" class:manual={info.state === "override" || info.state === "returning"}>{chip}</span>{/if}
  <button onclick={onstop} title="{t('stop')}{k('stop')}">■</button>
</div>

<style>
  .bar {
    position: absolute; left: 50%; bottom: 14px; transform: translateX(-50%);
    display: flex; align-items: center; gap: 6px; padding: 6px 10px; border-radius: 10px;
    background: rgb(11 13 16 / 0.82); border: 1px solid var(--border); backdrop-filter: blur(6px);
    min-width: 440px; user-select: none;
  }
  .track { flex: 1; height: 4px; border-radius: 2px; background: var(--border); overflow: hidden; min-width: 100px; }
  .fill { height: 100%; background: var(--accent); }
  .time { font-variant-numeric: tabular-nums; color: var(--muted); font-size: 12px; min-width: 84px; text-align: right; }
  .speed { min-width: 48px; }
  .chip { font-size: 12px; padding: 1px 8px; border-radius: 10px; background: var(--panel-2); color: var(--muted); }
  .chip.manual { color: #04121b; background: #fbbf24; }
</style>
