<script lang="ts">
  // Aviso en la cabecera mientras una IA maneja la app: qué ha hecho y un botón para detenerla.
  import { mcp } from "./host.svelte";
  import { t, label } from "../i18n.svelte";

  interface Props {
    onstop: () => void;
  }
  let { onstop }: Props = $props();
  let open = $state(false);

  const shown = $derived(mcp.connections > 0 || !!mcp.busy);
  const toolName = (tool: string) => {
    const l = label(`mcp.${tool}`);
    return l === `mcp.${tool}` ? tool : l;
  };
  const ago = (at: number) => {
    const s = Math.round((Date.now() - at) / 1000);
    return s < 60 ? `${s} s` : `${Math.round(s / 60)} min`;
  };
</script>

{#if shown}
  <div class="wrap">
    <button class="badge" class:busy={!!mcp.busy} onclick={() => (open = !open)} title={t("aiConnectedHelp")}>
      🤖 {t("aiConnected")}
    </button>
    {#if open}
      <div class="backdrop" role="presentation" onclick={() => (open = false)}></div>
      <div class="pop">
        <b>{t("aiConnectedTitle")}</b>
        {#if mcp.busy}<p class="now">⏳ {toolName(mcp.busy)}</p>{/if}
        {#if mcp.actions.length}
          <ul>
            {#each mcp.actions.slice(0, 12) as a (a.at + a.tool)}
              <li class:bad={!a.ok}><span>{a.ok ? "✓" : "✗"} {toolName(a.tool)}</span><span class="when">{ago(a.at)}</span></li>
            {/each}
          </ul>
        {:else}
          <p class="note">{t("aiNoActions")}</p>
        {/if}
        <p class="note">{t("aiUndoHint")}</p>
        <button class="stop" onclick={() => ((open = false), onstop())}>■ {t("aiStop")}</button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .wrap { position: relative; }
  .badge { border-color: #a78bfa; color: #ddd6fe; white-space: nowrap; }
  .badge.busy { animation: pulse 1.2s ease-in-out infinite; }
  @keyframes pulse { 50% { background: rgb(167 139 250 / 0.25); } }
  .backdrop { position: fixed; inset: 0; z-index: 9; }
  .pop {
    position: absolute; right: 0; top: calc(100% + 6px); z-index: 10; width: 300px; padding: 10px;
    display: flex; flex-direction: column; gap: 6px; font-size: 12px;
    background: var(--panel); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 8px 30px rgb(0 0 0 / 0.45);
  }
  ul { list-style: none; margin: 0; padding: 0; max-height: 240px; overflow-y: auto; }
  li { display: flex; justify-content: space-between; gap: 8px; padding: 2px 0; }
  li.bad { color: #fca5a5; }
  .when, .note { color: var(--muted); }
  .note, .now { margin: 0; line-height: 1.4; }
  .stop { align-self: flex-start; background: #7f1d1d; border-color: #ef4444; color: #fecaca; }
</style>
