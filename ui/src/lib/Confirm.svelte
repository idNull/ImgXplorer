<script lang="ts" module>
  export interface ConfirmButton {
    label: string;
    value: string;
    /** Botón principal (el de Intro). */
    primary?: boolean;
    /** Acción que pierde algo: se pinta en naranja. */
    danger?: boolean;
  }
  export interface ConfirmSpec {
    title: string;
    message: string;
    detail?: string;
    buttons: ConfirmButton[];
    /** Lo que devuelve Escape o pulsar fuera. */
    cancel: string;
  }
</script>

<script lang="ts">
  let { spec, onanswer }: { spec: ConfirmSpec; onanswer: (value: string) => void } = $props();

  let box: HTMLDivElement;
  $effect(() => {
    // El foco en el botón principal: Intro lo acepta, Tab recorre los demás.
    box?.querySelector<HTMLButtonElement>("button.primary")?.focus();
  });

  function onkey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      onanswer(spec.cancel);
    }
  }
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onanswer(spec.cancel)}>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="dialog" role="alertdialog" aria-modal="true" aria-label={spec.title} tabindex="-1" bind:this={box} onkeydown={onkey}>
    <header><b>⚠ {spec.title}</b></header>
    <div class="content">
      <p>{spec.message}</p>
      {#if spec.detail}<code>{spec.detail}</code>{/if}
    </div>
    <footer>
      {#each spec.buttons as b (b.value)}
        <button class:primary={b.primary} class:danger={b.danger} onclick={() => onanswer(b.value)}>{b.label}</button>
      {/each}
    </footer>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 60; background: rgb(0 0 0 / 0.55); display: grid; place-items: center; }
  .dialog {
    width: min(480px, calc(100vw - 40px)); display: flex; flex-direction: column; outline: none;
    background: var(--panel); border: 1px solid var(--border); border-radius: 10px; box-shadow: 0 12px 40px rgb(0 0 0 / 0.5); font-size: 13px;
  }
  header { padding: 8px 12px; border-bottom: 1px solid var(--border); color: #fbbf24; }
  .content { padding: 12px 14px; display: flex; flex-direction: column; gap: 8px; }
  p { margin: 0; line-height: 1.45; }
  code { font-size: 12px; color: var(--muted); overflow-wrap: anywhere; }
  footer { display: flex; justify-content: flex-end; flex-wrap: wrap; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
  .primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
  .danger { background: #b45309; border-color: #f59e0b; color: #fff; }
</style>
