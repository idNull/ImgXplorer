<script lang="ts">
  // Ir a página (Ctrl+G): una cajita con el número; Intro salta y Esc cierra.
  import { onMount } from "svelte";
  import { t } from "../i18n.svelte";

  interface Props {
    count: number;
    current: number;
    ongo: (index: number) => void;
    onclose: () => void;
  }
  let { count, current, ongo, onclose }: Props = $props();

  let input: HTMLInputElement;
  // svelte-ignore state_referenced_locally
  let value = $state(String(current + 1));

  onMount(() => {
    input.focus();
    input.select();
  });

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    } else if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      const n = Math.round(Number(value));
      if (Number.isFinite(n)) ongo(Math.min(count, Math.max(1, n)) - 1);
      onclose();
    }
  }
</script>

<div class="goto" role="dialog" aria-label={t("readGoTo")}>
  <label>
    {t("readGoToLabel")}
    <input bind:this={input} type="number" min="1" max={count} bind:value onkeydown={key} onblur={onclose} />
    <span>/ {count}</span>
  </label>
</div>

<style>
  .goto {
    position: absolute; left: 50%; top: 12px; transform: translateX(-50%); z-index: 6; padding: 8px 12px;
    background: rgb(20 23 28 / 0.96); border: 1px solid var(--accent); border-radius: 8px; box-shadow: 0 8px 30px rgb(0 0 0 / 0.45);
  }
  label { display: flex; align-items: center; gap: 8px; font-size: 13px; }
  input { width: 76px; font-variant-numeric: tabular-nums; }
  span { color: var(--muted); }
</style>
