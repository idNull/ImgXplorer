<script lang="ts">
  // Un clip de sonido en su pista: forma de onda del trozo que suena, fundidos y bordes para
  // recortar.
  import { sounds } from "../audio/sounds.svelte";
  import { audioClipEnd, type AudioBus, type AudioClip } from "./model";
  import { t } from "../i18n.svelte";

  interface Props {
    clip: AudioClip;
    bus: AudioBus;
    /** Píxeles por segundo. */
    pps: number;
    pad: number;
    height: number;
    selected: boolean;
    ondown: (e: PointerEvent, part: "body" | "start" | "end") => void;
  }
  let { clip, bus, pps, pad, height, selected, ondown }: Props = $props();

  const info = $derived(sounds.get(clip.sound));
  const missing = $derived(sounds.loaded && !info);
  const name = $derived(clip.name ?? (info ? sounds.name(clip.sound) : t("soundMissing")));
  const left = $derived(pad + clip.start * pps);
  const w = $derived(Math.max(6, clip.duration * pps));

  let peaks = $state<Float32Array | null>(null);
  $effect(() => {
    const s = info;
    if (!s) return void (peaks = null);
    let alive = true;
    // Unos 40 picos por segundo, con límites para sonidos muy cortos o muy largos.
    sounds.peaks(s.id, Math.min(6000, Math.max(64, Math.round(s.seconds * 40)))).then(
      (p) => alive && (peaks = p),
      () => alive && (peaks = null),
    );
    return () => {
      alive = false;
    };
  });

  /** Forma de onda simétrica del tramo que suena (de `trimIn` a `trimIn + duración`). */
  const wave = $derived.by(() => {
    const p = peaks;
    const s = info;
    if (!p || !s || s.seconds <= 0) return "";
    const mid = height / 2;
    const amp = (height / 2 - 3) * Math.min(1.4, Math.max(0.15, clip.volume));
    const cols = Math.max(2, Math.min(800, Math.round(w / 2)));
    const top: string[] = [];
    const bottom: string[] = [];
    for (let k = 0; k <= cols; k++) {
      const x = (k / cols) * w;
      const sec = clip.trimIn + x / pps;
      const v = sec >= s.seconds ? 0 : p[Math.min(p.length - 1, Math.floor((sec / s.seconds) * p.length))] ?? 0;
      const h = Math.max(0.5, v * amp);
      top.push(`${x.toFixed(1)},${(mid - h).toFixed(1)}`);
      bottom.push(`${x.toFixed(1)},${(mid + h).toFixed(1)}`);
    }
    return [...top, ...bottom.reverse()].join(" ");
  });

  const fmt = (s: number) => `${s.toFixed(2)} s`;
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="aclip {bus}"
  class:selected
  class:missing
  style:left="{left}px"
  style:width="{w}px"
  style:height="{height}px"
  title="{name} · {fmt(clip.start)} → {fmt(audioClipEnd(clip))}{missing ? ` · ${t('soundMissingHelp')}` : ''}"
  onpointerdown={(e) => ondown(e, "body")}
>
  <svg width={w} {height} aria-hidden="true">
    {#if wave}<polygon class="wave" points={wave} />{/if}
    {#if clip.fadeIn > 0}<polygon class="fade" points="0,0 {Math.min(w, clip.fadeIn * pps)},0 0,{height}" />{/if}
    {#if clip.fadeOut > 0}<polygon class="fade" points="{w},0 {Math.max(0, w - clip.fadeOut * pps)},0 {w},{height}" />{/if}
  </svg>
  <span class="name">{missing ? "⚠ " : ""}{name}</span>
  <span class="edge start" onpointerdown={(e) => ondown(e, "start")}></span>
  <span class="edge end" onpointerdown={(e) => ondown(e, "end")}></span>
</div>

<style>
  .aclip {
    position: absolute; top: 3px; box-sizing: border-box; border-radius: 5px; cursor: grab; user-select: none; overflow: hidden;
    background: rgb(52 211 153 / 0.14); border: 1px solid rgb(52 211 153 / 0.6);
  }
  .aclip.sfx { background: rgb(251 146 60 / 0.14); border-color: rgb(251 146 60 / 0.6); }
  .aclip.selected { border-color: #fbbf24; box-shadow: 0 0 0 1px #fbbf24; }
  .aclip.missing { background: rgb(239 68 68 / 0.15); border-color: #ef4444; border-style: dashed; }
  svg { position: absolute; left: -1px; top: -1px; pointer-events: none; }
  .wave { fill: rgb(110 231 183 / 0.45); }
  .sfx .wave { fill: rgb(253 186 116 / 0.45); }
  .fade { fill: rgb(0 0 0 / 0.35); }
  .name { position: absolute; left: 6px; right: 6px; top: 1px; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; pointer-events: none; text-shadow: 0 1px 2px #000; }
  .edge { position: absolute; top: 0; bottom: 0; width: 7px; cursor: ew-resize; }
  .edge.start { left: -1px; }
  .edge.end { right: -1px; }
</style>
