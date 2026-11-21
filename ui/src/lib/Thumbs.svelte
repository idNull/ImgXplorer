<script lang="ts">
  import { t } from "./i18n.svelte";
  // Tira de miniaturas. Carga perezosa (solo las visibles) y la actual siempre a la vista.
  interface Props {
    count: number;
    current: number;
    /** URL de la miniatura i. */
    url: (i: number) => string;
    name: (i: number) => string;
    onpick: (i: number) => void;
    /** Tamaño de la tira: pequeña, mediana o grande. */
    size?: "s" | "m" | "l";
    /** Páginas marcadas (🔖). */
    marks?: Set<number>;
    /** Páginas que se ven a la vez (doble página): también se resaltan. */
    shown?: number[];
    /** Arrastrar una miniatura a otro sitio: mover la página `from` delante de la posición `to`. */
    onmove?: (from: number, to: number) => void;
    /** Botón ▦ al final: abrir la mesa de luz. */
    onorganize?: () => void;
  }
  let { count, current, url, name, onpick, size = "m", marks, shown = [], onmove, onorganize }: Props = $props();
  const HEIGHT = { s: 84, m: 112, l: 156 };

  let strip: HTMLDivElement;
  const failed = $state<Record<number, boolean>>({});

  $effect(() => {
    const el = strip?.querySelector<HTMLElement>(`[data-i="${current}"]`);
    el?.scrollIntoView({ block: "nearest", inline: "center", behavior: "smooth" });
  });

  // Arrastrar una miniatura: empieza al moverse unos píxeles con el botón pulsado.
  let press: { i: number; x: number; id: number } | null = null;
  let dragging = $state<number | null>(null);
  let drop = $state<{ at: number; side: "before" | "after" } | null>(null);
  let suppress = false;
  let scrollTimer = 0;

  function down(e: PointerEvent, i: number) {
    if (e.button === 0 && onmove) press = { i, x: e.clientX, id: e.pointerId };
  }
  function move(e: PointerEvent) {
    if (!press || e.pointerId !== press.id) return;
    if (dragging === null) {
      if (Math.abs(e.clientX - press.x) < 8) return;
      dragging = press.i;
      strip.setPointerCapture(e.pointerId);
    }
    const el = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("[data-i]");
    if (el) {
      const r = el.getBoundingClientRect();
      drop = { at: Number(el.dataset.i), side: e.clientX < r.left + r.width / 2 ? "before" : "after" };
    }
    // Cerca de los bordes, la tira se desplaza sola.
    const r = strip.getBoundingClientRect();
    const dx = e.clientX < r.left + 40 ? -16 : e.clientX > r.right - 40 ? 16 : 0;
    clearInterval(scrollTimer);
    if (dx) scrollTimer = window.setInterval(() => (strip.scrollLeft += dx), 16);
  }
  function up(e: PointerEvent) {
    clearInterval(scrollTimer);
    if (dragging !== null) {
      suppress = true;
      setTimeout(() => (suppress = false));
      if (drop) {
        const to = drop.side === "before" ? drop.at : drop.at + 1;
        if (to !== dragging && to !== dragging + 1) onmove?.(dragging, to);
      }
      if (strip.hasPointerCapture(e.pointerId)) strip.releasePointerCapture(e.pointerId);
    }
    press = null;
    dragging = null;
    drop = null;
  }

  function wheel(e: WheelEvent) {
    // Rueda vertical → desplazamiento horizontal.
    if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      strip.scrollLeft += e.deltaY;
      e.preventDefault();
    }
  }
</script>

<div class="strip" bind:this={strip} onwheel={wheel} style:--h="{HEIGHT[size]}px" class:dragging={dragging !== null} role="group" aria-label={t("thumbs")}
  onpointermove={move} onpointerup={up} onpointercancel={up}>
  {#each { length: count } as _, i (i)}
    <button class="thumb" class:current={i === current || shown.includes(i)} class:moving={dragging === i}
      class:ins-before={drop?.at === i && drop.side === "before"} class:ins-after={drop?.at === i && drop.side === "after"}
      data-i={i} title={name(i)} onpointerdown={(e) => down(e, i)} onclick={() => !suppress && onpick(i)}>
      {#if failed[i]}
        <span class="broken">?</span>
      {:else}
        <img src={url(i)} alt="" loading="lazy" decoding="async" draggable="false" onerror={() => (failed[i] = true)} />
      {/if}
      <span class="n">{i + 1}</span>
      {#if marks?.has(i)}<span class="mark">🔖</span>{/if}
    </button>
  {/each}
  {#if onorganize}
    <button class="organize" data-tour="organize" onclick={onorganize} title={t("comicStrip")}>▦</button>
  {/if}
</div>

<style>
  .strip {
    display: flex; gap: 6px; padding: 6px 8px; overflow-x: auto; overflow-y: hidden;
    background: var(--panel); border-top: 1px solid var(--border); height: var(--h); flex: none;
    scrollbar-width: thin;
  }
  .thumb {
    position: relative; flex: none; width: calc((var(--h) - 12px) * 0.72); height: 100%; padding: 0; overflow: hidden;
    display: grid; place-items: center; background: var(--bg); border: 2px solid transparent; border-radius: 4px;
  }
  .thumb.current { border-color: var(--accent); }
  .strip.dragging { cursor: grabbing; }
  .thumb.moving { opacity: 0.4; }
  .thumb.ins-before { box-shadow: inset 4px 0 0 var(--accent); }
  .thumb.ins-after { box-shadow: inset -4px 0 0 var(--accent); }
  .organize { flex: none; align-self: center; position: sticky; right: 0; font-size: 18px; padding: 6px 9px; background: var(--panel-2); }
  img { max-width: 100%; max-height: 100%; object-fit: contain; }
  .n {
    position: absolute; right: 2px; bottom: 2px; padding: 0 4px; border-radius: 3px;
    font-size: 10px; background: rgb(0 0 0 / 0.65); color: var(--muted);
  }
  .broken { color: var(--muted); }
  .mark { position: absolute; left: 2px; top: 0; font-size: 12px; line-height: 1; filter: drop-shadow(0 1px 1px rgb(0 0 0 / 0.7)); }
</style>
