<script lang="ts" module>
  export interface MenuItem {
    label: string;
    key?: string;
    disabled?: boolean;
    danger?: boolean;
    hint?: boolean;
    run?: () => void;
  }
</script>

<script lang="ts">
  // Menú de la imagen (clic corto con el botón derecho): información, copiar, guardar, carpeta,
  // papelera… Cada acción tiene también su atajo, que se ve a la derecha.
  import { t } from "../i18n.svelte";

  interface Props {
    x: number;
    y: number;
    items: (MenuItem | null)[];
    onclose: () => void;
  }
  let { x, y, items, onclose }: Props = $props();

  let box: HTMLDivElement;
  let pos = $state({ x: 0, y: 0 });
  $effect(() => {
    // Que no se salga de la ventana.
    const r = box.getBoundingClientRect();
    pos = { x: Math.min(x, window.innerWidth - r.width - 6), y: Math.min(y, window.innerHeight - r.height - 6) };
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (e.preventDefault(), onclose())} />

<div class="backdrop" role="presentation" onclick={onclose} oncontextmenu={(e) => { e.preventDefault(); onclose(); }}></div>
<div class="menu" role="menu" aria-label={t("viewMenu")} bind:this={box} style:left="{pos.x}px" style:top="{pos.y}px">
  {#each items as it, i (i)}
    {#if !it}
      <hr />
    {:else if it.hint}
      <span class="hint">{it.label}</span>
    {:else}
      <button role="menuitem" class:danger={it.danger} disabled={it.disabled} onclick={() => { onclose(); it.run?.(); }}>
        <span>{it.label}</span>{#if it.key}<kbd>{it.key}</kbd>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 20; }
  .menu {
    position: fixed; z-index: 21; display: flex; flex-direction: column; min-width: 230px; padding: 4px;
    background: var(--panel); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 8px 30px rgb(0 0 0 / 0.45);
  }
  button { display: flex; justify-content: space-between; gap: 16px; text-align: left; background: transparent; border-color: transparent; padding: 5px 10px; font-size: 13px; }
  button:hover:not(:disabled) { background: var(--panel-2); }
  .danger:hover:not(:disabled) { color: #fca5a5; }
  kbd { color: var(--muted); font-size: 11.5px; font-family: inherit; }
  hr { border: none; border-top: 1px solid var(--border); margin: 4px 2px; }
  .hint { color: var(--muted); font-size: 11.5px; padding: 3px 10px; }
</style>
