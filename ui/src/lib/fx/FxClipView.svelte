<script lang="ts">
  // Un efecto especial en su fila de la timeline, con bordes para cambiar inicio y duración.
  import { FX_ICON, fxInfo, fxName } from "./catalog";
  import type { Fx } from "./fx";

  interface Props {
    fx: Fx;
    /** Píxeles por segundo. */
    pps: number;
    pad: number;
    selected: boolean;
    /** Posición vertical y alto dentro de la fila (varios efectos a la vez van en subfilas). */
    top?: number;
    height?: number;
    ondown: (e: PointerEvent, part: "body" | "start" | "end") => void;
  }
  let { fx, pps, pad, selected, top = 3, height = 22, ondown }: Props = $props();
  const left = $derived(pad + fx.start * pps);
  const w = $derived(Math.max(6, fx.duration * pps));
  const sustained = $derived(fxInfo(fx.type)?.sustained ?? false);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fxclip" class:sustained class:selected class:thin={height < 16} style:left="{left}px" style:width="{w}px" style:top="{top}px" style:height="{height}px" title="{fxName(fx)} · {fx.start.toFixed(2)} s → {(fx.start + fx.duration).toFixed(2)} s"
  onpointerdown={(e) => ondown(e, "body")}>
  <span class="name">{FX_ICON[fx.type] ?? "?"} {fxName(fx)}</span>
  {#if fx.at}<span class="pin" aria-hidden="true">⌖</span>{/if}
  <span class="edge start" onpointerdown={(e) => (e.stopPropagation(), ondown(e, "start"))}></span>
  <span class="edge end" onpointerdown={(e) => (e.stopPropagation(), ondown(e, "end"))}></span>
</div>

<style>
  .fxclip {
    position: absolute; border-radius: 4px; box-sizing: border-box; overflow: hidden; cursor: grab;
    background: rgb(249 115 22 / 0.25); border: 1px solid rgb(249 115 22 / 0.7); display: flex; align-items: center; gap: 4px;
  }
  .fxclip.sustained { background: rgb(168 85 247 / 0.22); border-color: rgb(168 85 247 / 0.7); }
  .fxclip.selected { box-shadow: 0 0 0 2px #fbbf24; }
  .name { padding: 0 6px; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; pointer-events: none; }
  .fxclip.thin .name { font-size: 10px; line-height: 1; }
  .pin { font-size: 11px; color: #fbbf24; pointer-events: none; }
  .edge { position: absolute; top: 0; bottom: 0; width: 6px; cursor: ew-resize; }
  .edge.start { left: 0; }
  .edge.end { right: 0; }
  .edge:hover { background: rgb(255 255 255 / 0.25); }
</style>
