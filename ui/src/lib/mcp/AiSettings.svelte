<script lang="ts">
  // Ajustes → IA: permitir que una IA maneje la app y conectarla con Claude Code, Codex o Gemini.
  import { invoke } from "@tauri-apps/api/core";
  import { errorText } from "../errors";
  import { prefs } from "../prefs.svelte";
  import { t } from "../i18n.svelte";
  import { log } from "../log";

  type ClientId = "claude" | "codex" | "gemini";
  interface ClientStatus {
    id: ClientId;
    name: string;
    /** El programa de esa IA está instalado. */
    installed: boolean;
    /** imgxplorer está conectado y con qué ruta. */
    connected: boolean;
    stale: boolean;
    /** Comando para conectarlo a mano. */
    command: string;
    detail: string | null;
  }

  let clients = $state<ClientStatus[] | null>(null);
  let busy = $state<string | null>(null);
  let message = $state<{ id: ClientId; text: string; ok: boolean } | null>(null);
  let copied = $state<string | null>(null);

  async function refresh() {
    try {
      clients = await invoke<ClientStatus[]>("mcp_clients");
    } catch (e) {
      log.warn("IAs:", e);
      clients = [];
    }
  }
  refresh();

  function setEnabled(v: boolean) {
    prefs.settings.aiControl = v;
    prefs.saveSettings();
  }

  async function act(c: ClientStatus, action: "connect" | "disconnect" | "test") {
    busy = `${c.id}:${action}`;
    message = null;
    try {
      const text = await invoke<string>("mcp_client_action", { client: c.id, action });
      message = { id: c.id, text, ok: true };
      if (action === "connect" && !prefs.settings.aiControl) setEnabled(true);
    } catch (e) {
      message = { id: c.id, text: errorText(e), ok: false };
    } finally {
      busy = null;
      refresh();
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = text;
      setTimeout(() => copied === text && (copied = null), 1500);
    } catch (e) {
      log.warn("copiar:", e);
    }
  }

  const example = $derived(t("aiExamplePrompt"));
</script>

<p class="note">{t("aiHelp")}</p>
<label class="row">
  <span>{t("aiAllow")}</span>
  <input type="checkbox" checked={prefs.settings.aiControl} onchange={(e) => setEnabled(e.currentTarget.checked)} />
</label>
<p class="note">{t("aiAllowHelp")}</p>

<h4>{t("aiClients")}</h4>
{#if !clients}
  <p class="note">{t("loading")}</p>
{:else}
  {#each clients as c (c.id)}
    <div class="client">
      <div class="head">
        <b>{c.name}</b>
        <span class="state" class:ok={c.connected && !c.stale} class:warn={c.stale}>
          {#if !c.installed}{t("aiNotInstalled")}{:else if c.stale}{t("aiStale")}{:else if c.connected}✓ {t("aiLinked")}{:else}{t("aiNotLinked")}{/if}
        </span>
        <span class="grow"></span>
        {#if c.installed}
          {#if c.connected}
            {#if c.stale}<button disabled={!!busy} onclick={() => act(c, "connect")}>{t("aiUpdate")}</button>{/if}
            <button disabled={!!busy} onclick={() => act(c, "test")}>{busy === `${c.id}:test` ? "…" : t("aiTest")}</button>
            <button disabled={!!busy} onclick={() => act(c, "disconnect")}>{t("aiUnlink")}</button>
          {:else}
            <button class="primary" disabled={!!busy} onclick={() => act(c, "connect")}>{busy === `${c.id}:connect` ? "…" : t("aiLink")}</button>
          {/if}
        {/if}
      </div>
      {#if c.detail}<p class="note">{c.detail}</p>{/if}
      {#if message?.id === c.id}<p class="msg" class:bad={!message.ok}>{message.text}</p>{/if}
      {#if !c.installed || !c.connected}
        <div class="cmd">
          <code>{c.command}</code>
          <button class="small" onclick={() => copy(c.command)}>{copied === c.command ? "✓" : t("copy")}</button>
        </div>
      {/if}
    </div>
  {/each}
{/if}

<h4>{t("aiExample")}</h4>
<div class="cmd">
  <code class="wrap">{example}</code>
  <button class="small" onclick={() => copy(example)}>{copied === example ? "✓" : t("copy")}</button>
</div>
<p class="note">{t("aiPrivacy")}</p>

<style>
  .row { display: grid; grid-template-columns: 1fr 180px 56px; align-items: center; gap: 10px; }
  .row > :nth-child(2):last-child { grid-column: 2 / 4; justify-self: start; }
  h4 { margin: 10px 0 2px; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  .note { margin: 0; color: var(--muted); font-size: 12px; line-height: 1.4; }
  .client { display: flex; flex-direction: column; gap: 4px; padding: 8px 10px; border: 1px solid var(--border); border-radius: 8px; }
  .head { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .head button { padding: 2px 10px; font-size: 12px; }
  .grow { flex: 1; }
  .state { color: var(--muted); font-size: 12px; }
  .state.ok { color: #86efac; }
  .state.warn { color: #fbbf24; }
  .primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
  .cmd { display: flex; align-items: flex-start; gap: 6px; }
  code { flex: 1; min-width: 0; font-size: 11px; padding: 4px 6px; background: var(--bg); border-radius: 4px; overflow-x: auto; white-space: nowrap; user-select: all; }
  code.wrap { white-space: normal; line-height: 1.4; }
  .small { padding: 1px 7px; font-size: 12px; flex: none; }
  .msg { margin: 0; font-size: 12px; color: #86efac; white-space: pre-wrap; }
  .msg.bad { color: #fca5a5; }
</style>
