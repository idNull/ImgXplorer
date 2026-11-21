<script lang="ts">
  // Mesa de luz: las páginas en cuadrícula. Clic elige (Ctrl/Mayús+clic, varias) y arrastrar las
  // mueve; una línea marca dónde caerán. Funciona con el ratón y con el dedo.
  import { fileThumb, type Tile } from "./comic.svelte";
  import { t } from "../i18n.svelte";

  interface Props {
    tiles: Tile[];
    selected: string[];
    /** Miniatura de una página de la fuente abierta. */
    thumb: (index: number) => string;
    onselect: (keys: string[]) => void;
    /** Mover `keys` delante de la posición `to`. */
    onmove: (keys: string[], to: number) => void;
    /** Página que no estaba (se añade al guardar). */
    isNew?: (t: Tile) => boolean;
  }
  let { tiles, selected, thumb, onselect, onmove, isNew }: Props = $props();

  let grid: HTMLDivElement;
  let anchor: string | null = null;
  const loaded = $state<Record<string, string>>({});
  const failed = $state<Record<string, boolean>>({});

  $effect(() => {
    for (const tile of tiles) {
      if (tile.path && loaded[tile.key] === undefined) {
        loaded[tile.key] = "";
        void fileThumb(tile.path).then((u) => (u ? (loaded[tile.key] = u) : (failed[tile.key] = true)));
      }
    }
  });
  const src = (tile: Tile) => (tile.path ? loaded[tile.key] : tile.index !== undefined ? thumb(tile.index) : "");

  function click(e: MouseEvent, tile: Tile) {
    if (e.ctrlKey || e.metaKey) {
      onselect(selected.includes(tile.key) ? selected.filter((k) => k !== tile.key) : [...selected, tile.key]);
      anchor = tile.key;
    } else if (e.shiftKey && anchor) {
      const keys = tiles.map((x) => x.key);
      const [a, b] = [keys.indexOf(anchor), keys.indexOf(tile.key)].sort((x, y) => x - y);
      onselect(a >= 0 ? keys.slice(a, b + 1) : [tile.key]);
    } else {
      onselect([tile.key]);
      anchor = tile.key;
    }
  }

  // Arrastrar: empieza al moverse unos píxeles con el botón pulsado.
  let press: { key: string; x: number; y: number; id: number } | null = null;
  let dragging = $state<string[] | null>(null);
  let drop = $state<{ at: number; side: "before" | "after" } | null>(null);
  let ghost = $state<{ x: number; y: number } | null>(null);
  let scrollTimer = 0;

  function down(e: PointerEvent, tile: Tile) {
    if (e.button !== 0) return;
    press = { key: tile.key, x: e.clientX, y: e.clientY, id: e.pointerId };
  }

  function target(x: number, y: number) {
    const el = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-k]");
    if (!el) {
      // Por debajo de la última fila: al final.
      const last = grid.querySelector<HTMLElement>("[data-k]:last-of-type");
      const r = last?.getBoundingClientRect();
      return r && y > r.top ? { at: tiles.length - 1, side: "after" as const } : null;
    }
    const i = tiles.findIndex((x) => x.key === el.dataset.k);
    const r = el.getBoundingClientRect();
    return { at: i, side: x < r.left + r.width / 2 ? ("before" as const) : ("after" as const) };
  }

  function move(e: PointerEvent) {
    if (!press || e.pointerId !== press.id) return;
    if (!dragging) {
      if (Math.hypot(e.clientX - press.x, e.clientY - press.y) < 6) return;
      const keys = selected.includes(press.key) ? tiles.filter((x) => selected.includes(x.key)).map((x) => x.key) : [press.key];
      if (!selected.includes(press.key)) onselect([press.key]);
      dragging = keys;
      grid.setPointerCapture(e.pointerId);
    }
    ghost = { x: e.clientX, y: e.clientY };
    drop = target(e.clientX, e.clientY);
    // Cerca del borde, la mesa se desplaza sola.
    const r = grid.getBoundingClientRect();
    const dy = e.clientY < r.top + 40 ? -14 : e.clientY > r.bottom - 40 ? 14 : 0;
    clearInterval(scrollTimer);
    if (dy) scrollTimer = window.setInterval(() => grid.scrollBy(0, dy), 16);
  }

  // El clic que llega al soltar después de arrastrar no cambia la selección.
  let suppress = false;

  function up(e: PointerEvent) {
    clearInterval(scrollTimer);
    if (dragging) {
      suppress = true;
      setTimeout(() => (suppress = false));
    }
    if (dragging && drop) onmove(dragging, drop.side === "before" ? drop.at : drop.at + 1);
    if (dragging && grid.hasPointerCapture(e.pointerId)) grid.releasePointerCapture(e.pointerId);
    press = null;
    dragging = null;
    drop = null;
    ghost = null;
  }

  $effect(() => {
    // La seleccionada, siempre a la vista (al moverla con el teclado, al añadir…).
    const last = selected[selected.length - 1];
    if (last) grid?.querySelector(`[data-k="${last}"]`)?.scrollIntoView({ block: "nearest" });
  });
</script>

<div class="grid" bind:this={grid} role="listbox" aria-multiselectable="true" aria-label={t("comicPages")} tabindex="-1"
  onpointermove={move} onpointerup={up} onpointercancel={up} class:dragging={!!dragging}>
  {#each tiles as tile, i (tile.key)}
    <button
      class="tile"
      class:sel={selected.includes(tile.key)}
      class:moving={dragging?.includes(tile.key)}
      class:ins-before={drop?.at === i && drop.side === "before"}
      class:ins-after={drop?.at === i && drop.side === "after"}
      data-k={tile.key}
      role="option"
      aria-selected={selected.includes(tile.key)}
      title={tile.name}
      onpointerdown={(e) => down(e, tile)}
      onclick={(e) => !suppress && click(e, tile)}
    >
      <span class="img">
        {#if failed[tile.key]}
          <span class="broken">?</span>
        {:else if src(tile)}
          <img src={src(tile)} alt="" loading="lazy" decoding="async" draggable="false" onerror={() => (failed[tile.key] = true)} />
        {/if}
      </span>
      <span class="n">{i + 1}</span>
      {#if isNew?.(tile)}<span class="new">{t("comicNew")}</span>{/if}
      <span class="name">{tile.name}</span>
    </button>
  {/each}
</div>
{#if dragging && ghost}
  <div class="ghost" style:left="{ghost.x + 12}px" style:top="{ghost.y + 12}px">{dragging.length === 1 ? "⇄ 1" : `⇄ ${dragging.length}`}</div>
{/if}

<style>
  .grid {
    flex: 1; min-height: 0; overflow-y: auto; display: grid; grid-template-columns: repeat(auto-fill, minmax(132px, 1fr));
    gap: 14px; padding: 12px; align-content: start; outline: none; touch-action: none;
  }
  .grid.dragging { cursor: grabbing; }
  .tile {
    position: relative; display: flex; flex-direction: column; gap: 4px; padding: 6px; border-radius: 8px; background: var(--panel-2);
    border: 2px solid transparent; cursor: grab; text-align: left; min-width: 0;
  }
  .tile:hover { border-color: var(--border); }
  .tile.sel { border-color: var(--accent); background: rgb(56 189 248 / 0.12); }
  .tile.moving { opacity: 0.4; }
  .tile.ins-before::before, .tile.ins-after::after {
    content: ""; position: absolute; top: 4px; bottom: 4px; width: 4px; border-radius: 2px; background: var(--accent);
  }
  .tile.ins-before::before { left: -10px; }
  .tile.ins-after::after { right: -10px; }
  .img { aspect-ratio: 2 / 3; display: flex; align-items: center; justify-content: center; background: #0006; border-radius: 4px; overflow: hidden; }
  .img img { max-width: 100%; max-height: 100%; object-fit: contain; pointer-events: none; }
  .broken { color: var(--muted); font-size: 20px; }
  .n {
    position: absolute; left: 10px; top: 10px; padding: 0 6px; border-radius: 4px; background: rgb(0 0 0 / 0.7); font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .new {
    position: absolute; right: 10px; top: 10px; padding: 0 6px; border-radius: 4px; background: var(--accent); color: #04121b;
    font-size: 11px; font-weight: 600;
  }
  .name { font-size: 11px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ghost {
    position: fixed; z-index: 100; pointer-events: none; padding: 2px 8px; border-radius: 6px; background: var(--accent); color: #04121b;
    font-size: 12px; font-weight: 600;
  }
</style>
