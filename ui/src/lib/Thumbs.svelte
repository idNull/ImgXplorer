<script lang="ts">
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
  }
  let { count, current, url, name, onpick, size = "m" }: Props = $props();
  const HEIGHT = { s: 84, m: 112, l: 156 };

  let strip: HTMLDivElement;
  const failed = $state<Record<number, boolean>>({});

  $effect(() => {
    const el = strip?.querySelector<HTMLElement>(`[data-i="${current}"]`);
    el?.scrollIntoView({ block: "nearest", inline: "center", behavior: "smooth" });
  });

  function wheel(e: WheelEvent) {
    // Rueda vertical → desplazamiento horizontal.
    if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      strip.scrollLeft += e.deltaY;
      e.preventDefault();
    }
  }
</script>

<div class="strip" bind:this={strip} onwheel={wheel} style:--h="{HEIGHT[size]}px">
  {#each { length: count } as _, i (i)}
    <button class="thumb" class:current={i === current} data-i={i} title={name(i)} onclick={() => onpick(i)}>
      {#if failed[i]}
        <span class="broken">?</span>
      {:else}
        <img src={url(i)} alt="" loading="lazy" decoding="async" draggable="false" onerror={() => (failed[i] = true)} />
      {/if}
      <span class="n">{i + 1}</span>
    </button>
  {/each}
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
  img { max-width: 100%; max-height: 100%; object-fit: contain; }
  .n {
    position: absolute; right: 2px; bottom: 2px; padding: 0 4px; border-radius: 3px;
    font-size: 10px; background: rgb(0 0 0 / 0.65); color: var(--muted);
  }
  .broken { color: var(--muted); }
</style>
