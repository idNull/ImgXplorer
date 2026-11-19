<script lang="ts">
  // Ajustes → Voces IA: la clave de ElevenLabs (al llavero del sistema), probar la conexión, el
  // modelo de fábrica y el control del gasto.
  import { prefs } from "../prefs.svelte";
  import { t, label } from "../i18n.svelte";
  import { account, clearKey, keyStatus, setKey } from "./voices.svelte";
  import { MODELS, type AccountInfo } from "./types";

  let status = $state<{ stored: boolean; backend: "keyring" | "file" | null } | null>(null);
  let key = $state("");
  let busy = $state(false);
  let msg = $state<{ text: string; ok: boolean } | null>(null);
  let acct = $state<AccountInfo | null>(null);

  async function refresh() {
    try {
      status = await keyStatus();
    } catch (e) {
      status = { stored: false, backend: null };
      msg = { text: String(e), ok: false };
    }
  }
  refresh();

  const save = () => prefs.saveSettings();

  async function storeKey() {
    if (!key.trim()) return;
    busy = true;
    msg = null;
    try {
      const where = await setKey(key);
      key = "";
      msg = { text: where === "keyring" ? t("voicesKeySavedKeyring") : t("voicesKeySavedFile"), ok: true };
      await refresh();
    } catch (e) {
      msg = { text: String(e), ok: false };
    } finally {
      busy = false;
    }
  }

  async function removeKey() {
    busy = true;
    try {
      await clearKey();
      acct = null;
      msg = { text: t("voicesKeyRemoved"), ok: true };
      await refresh();
    } finally {
      busy = false;
    }
  }

  async function test() {
    busy = true;
    msg = null;
    acct = null;
    try {
      acct = await account();
    } catch (e) {
      msg = { text: String(e), ok: false };
    } finally {
      busy = false;
    }
  }

  const fmt = (n: number) => n.toLocaleString(prefs.settings.lang === "en" ? "en" : "es");
  const date = (s: number) => new Date(s * 1000).toLocaleDateString(prefs.settings.lang === "en" ? "en" : "es");
  const num = (e: Event) => Number((e.currentTarget as HTMLInputElement).value);
</script>

<p class="note">{t("voicesHelp")}</p>

<label class="row">
  <span>{t("voicesProvider")}</span>
  <select bind:value={prefs.settings.voicesProvider} onchange={save}>
    <option value="elevenlabs">ElevenLabs</option>
    <option value="mock">{t("voicesMock")}</option>
  </select>
</label>
{#if prefs.settings.voicesProvider === "mock"}<p class="note">{t("voicesMockHelp")}</p>{/if}

{#if prefs.settings.voicesProvider === "elevenlabs"}
  <h4>{t("voicesKey")}</h4>
  {#if status?.stored}
    <p class="state ok">✓ {status.backend === "keyring" ? t("voicesKeyInKeyring") : t("voicesKeyInFile")}</p>
  {:else if status}
    <p class="state">{t("voicesKeyMissing")}</p>
  {/if}
  <div class="keyrow">
    <input type="password" autocomplete="off" spellcheck="false" placeholder={status?.stored ? t("voicesKeyReplace") : t("voicesKeyPlaceholder")}
      bind:value={key} onkeydown={(e) => e.key === "Enter" && storeKey()} aria-label={t("voicesKey")} />
    <button class="primary" disabled={busy || !key.trim()} onclick={storeKey}>{t("voicesKeySave")}</button>
    {#if status?.stored}<button disabled={busy} onclick={removeKey}>{t("voicesKeyRemove")}</button>{/if}
  </div>
  <p class="note">{t("voicesKeyNote")}</p>
{/if}

<div class="keyrow">
  <button disabled={busy || (prefs.settings.voicesProvider === "elevenlabs" && !status?.stored)} onclick={test}>{busy ? "…" : t("voicesTest")}</button>
</div>
{#if acct}
  <p class="msg">
    ✓ {t("voicesAccount", { tier: acct.tier, used: fmt(acct.used), limit: fmt(acct.limit), left: fmt(Math.max(0, acct.limit - acct.used)) })}
    {#if acct.reset}<br />{t("voicesReset", { date: date(acct.reset) })}{/if}
    <br />{t("voicesSessionUsed", { n: fmt(acct.session) })}
  </p>
{/if}
{#if msg}<p class="msg" class:bad={!msg.ok}>{msg.text}</p>{/if}

<h4>{t("voicesDefaults")}</h4>
<label class="row">
  <span>{t("voicesModel")}</span>
  <select bind:value={prefs.settings.voicesModel} onchange={save}>
    {#each MODELS as m (m.id)}<option value={m.id}>{label(`vm.${m.id}`)}</option>{/each}
  </select>
</label>
<label class="row">
  <span title={t("voicesLimitHelp")}>{t("voicesLimit")}</span>
  <input type="number" min="0" step="500" value={prefs.settings.voicesLimit} onchange={(e) => ((prefs.settings.voicesLimit = Math.max(0, Math.round(num(e) || 0))), save())} />
</label>
<p class="note">{t("voicesLimitHelp")}</p>
<label class="row">
  <span title={t("voicesAutoHelp")}>{t("voicesAuto")}</span>
  <input type="number" min="0" step="100" value={prefs.settings.voicesAutoBudget} onchange={(e) => ((prefs.settings.voicesAutoBudget = Math.max(0, Math.round(num(e) || 0))), save())} />
</label>
<p class="note">{t("voicesAutoHelp")}</p>

<h4>{t("voicesPrivacyTitle")}</h4>
<p class="note">{t("voicesPrivacy")}</p>

<style>
  .row { display: grid; grid-template-columns: 1fr 200px; align-items: center; gap: 10px; }
  .row select, .row input {
    width: 100%; box-sizing: border-box; font: inherit; color: var(--text); background: var(--panel-2);
    border: 1px solid var(--border); border-radius: 6px; padding: 3px 6px;
  }
  h4 { margin: 12px 0 2px; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  .note { margin: 0; color: var(--muted); font-size: 12px; line-height: 1.45; }
  .state { margin: 0; color: var(--muted); font-size: 12px; }
  .state.ok { color: #86efac; }
  .keyrow { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; margin: 4px 0; }
  .keyrow input {
    flex: 1 1 220px; min-width: 0; font: inherit; color: var(--text); background: var(--panel-2);
    border: 1px solid var(--border); border-radius: 6px; padding: 3px 8px;
  }
  .primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
  .msg { margin: 2px 0; font-size: 12px; color: #86efac; line-height: 1.5; }
  .msg.bad { color: #fca5a5; }
</style>
