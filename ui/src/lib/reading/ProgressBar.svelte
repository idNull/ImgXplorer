<script lang="ts">
  // Barra fina de progreso abajo del visor: cuánto se lleva leído, «12 / 48» al pasar el ratón,
  // un clic salta a ese punto y los marcadores se ven como muescas. En manga se llena de derecha a
  // izquierda.
  interface Props {
    count: number;
    /** Páginas que se ven ahora (una o dos). */
    pages: number[];
    marks: number[];
    rtl: boolean;
    ongo: (index: number) => void;
  }
  let { count, pages, marks, rtl, ongo }: Props = $props();

  let bar: HTMLDivElement;
  let hover = $state<{ x: number; i: number } | null>(null);

  const last = $derived(Math.max(...pages));
  const done = $derived(count > 0 ? ((last + 1) / count) * 100 : 0);
  const pos = (i: number) => ((i + 0.5) / count) * 100;

  function at(e: PointerEvent | MouseEvent): { x: number; i: number } {
    const r = bar.getBoundingClientRect();
    const x = Math.min(r.width, Math.max(0, e.clientX - r.left));
    const f = rtl ? 1 - x / r.width : x / r.width;
    return { x, i: Math.min(count - 1, Math.max(0, Math.floor(f * count))) };
  }
</script>

<div class="progress" class:rtl bind:this={bar} role="slider" tabindex="-1" aria-valuemin={1} aria-valuemax={count} aria-valuenow={last + 1}
  onpointermove={(e) => (hover = at(e))} onpointerleave={() => (hover = null)} onclick={(e) => ongo(at(e).i)} onkeydown={() => {}}>
  <div class="track">
    <div class="fill" style:width="{done}%"></div>
    {#each marks as m (m)}<span class="mark" style:left="{rtl ? 100 - pos(m) : pos(m)}%"></span>{/each}
  </div>
  {#if hover}<span class="tip" style:left="{hover.x}px">{hover.i + 1} / {count}</span>{/if}
</div>

<style>
  .progress { position: absolute; left: 0; right: 0; bottom: 0; height: 14px; z-index: 3; cursor: pointer; display: flex; align-items: flex-end; }
  .track { position: relative; width: 100%; height: 3px; background: rgb(255 255 255 / 0.08); transition: height 0.12s; }
  .progress:hover .track { height: 7px; }
  .fill { position: absolute; top: 0; bottom: 0; left: 0; background: rgb(56 189 248 / 0.7); }
  .rtl .fill { left: auto; right: 0; }
  .mark { position: absolute; top: -3px; bottom: 0; width: 3px; margin-left: -1.5px; background: #fbbf24; border-radius: 1px; }
  .tip {
    position: absolute; bottom: 12px; transform: translateX(-50%); padding: 1px 7px; border-radius: 4px; pointer-events: none;
    background: rgb(0 0 0 / 0.8); border: 1px solid var(--border); font-size: 11.5px; white-space: nowrap; font-variant-numeric: tabular-nums;
  }
</style>
