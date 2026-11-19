<script lang="ts">
  // «Licencias de terceros» del «Acerca de»: quién hizo cada biblioteca que usa imgxplorer, con su
  // licencia. Los avisos especiales (UnRAR, libheif, FFmpeg…) van primero.
  import { invoke } from "@tauri-apps/api/core";
  import { currentLang, t } from "../i18n.svelte";
  import { licenseGroup, type Licensed, type Licenses } from "./licenses";

  interface Props {
    data: Licenses;
    onback: () => void;
  }
  let { data, onback }: Props = $props();

  let q = $state("");
  /** Entradas con el texto desplegado («nombre@versión»). */
  let open = $state<Record<string, boolean>>({});

  const key = (p: Licensed) => `${p.name}@${p.version ?? ""}`;
  const matches = (p: Licensed, needle: string) =>
    !needle ||
    p.name.toLowerCase().includes(needle) ||
    p.license.toLowerCase().includes(needle) ||
    p.authors.some((a) => a.toLowerCase().includes(needle));

  const needle = $derived(q.trim().toLowerCase());
  const special = $derived(data.special.filter((p) => matches(p, needle)));
  const groups = $derived.by(() => {
    const by = new Map<string, Licensed[]>();
    for (const p of data.packages) {
      if (!matches(p, needle)) continue;
      const g = licenseGroup(p.license);
      by.set(g, [...(by.get(g) ?? []), p]);
    }
    return [...by].sort((a, b) => b[1].length - a[1].length || a[0].localeCompare(b[0]));
  });
  const empty = $derived(!special.length && !groups.length);

  function openSite(url: string) {
    invoke("open_url", { url }).catch(() => {});
  }
</script>

{#snippet entry(p: Licensed)}
  {@const k = key(p)}
  <li>
    <div class="line">
      <button class="name" disabled={!p.texts.length} title={p.texts.length ? undefined : t("aboutNoText")} onclick={() => (open[k] = !open[k])}>
        <span class="caret">{p.texts.length ? (open[k] ? "▾" : "▸") : "·"}</span>
        <b>{p.name}</b>
        {#if p.version}<span class="ver">{p.version}</span>{/if}
      </button>
      <span class="who">{p.authors.join(", ")}</span>
      <span class="lic">{p.license}</span>
      <button class="site" title={t("aboutOpenSite")} onclick={() => openSite(p.url)}>↗</button>
    </div>
    {#if p.note}<p class="note">{p.note[currentLang()]}</p>{/if}
    {#if open[k]}
      {#each p.texts as i (i)}<pre>{data.texts[i]}</pre>{/each}
    {/if}
  </li>
{/snippet}

<div class="third">
  <div class="top">
    <button class="small" onclick={onback}>{t("aboutBack")}</button>
    <input type="search" placeholder={t("aboutSearch")} bind:value={q} />
  </div>
  <p class="thanks">{t("aboutThanks")}</p>

  {#if special.length}
    <h4>{t("aboutSpecial")}</h4>
    <ul>{#each special as p (key(p))}{@render entry(p)}{/each}</ul>
  {/if}

  {#each groups as [lic, list] (lic)}
    <details open={!!needle}>
      <summary><b>{lic}</b> <span class="count">{t("aboutLibs", { n: list.length })}</span></summary>
      <ul>{#each list as p (key(p))}{@render entry(p)}{/each}</ul>
    </details>
  {/each}

  {#if empty}<p class="muted">{t("aboutNoMatch", { q })}</p>{/if}
</div>

<style>
  .third { display: flex; flex-direction: column; gap: 8px; }
  .top { display: flex; gap: 8px; align-items: center; }
  .top input {
    flex: 1; min-width: 0; font: inherit; font-size: 12px; color: var(--text); background: var(--panel-2);
    border: 1px solid var(--border); border-radius: 6px; padding: 4px 8px;
  }
  .small { padding: 2px 8px; font-size: 12px; }
  .thanks { margin: 0; color: var(--muted); font-size: 12px; line-height: 1.4; }
  h4 { margin: 4px 0 0; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  details { border: 1px solid var(--border); border-radius: 8px; background: var(--panel-2); }
  summary { cursor: pointer; padding: 6px 10px; font-size: 12px; }
  .count { color: var(--muted); margin-left: 6px; }
  ul { list-style: none; margin: 0; padding: 0 6px 6px; display: flex; flex-direction: column; gap: 1px; }
  li { padding: 2px 4px; border-radius: 4px; }
  li:hover { background: rgb(255 255 255 / 0.03); }
  .line { display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr) auto auto; gap: 8px; align-items: center; font-size: 12px; }
  .name {
    all: unset; cursor: pointer; display: flex; gap: 5px; align-items: baseline; min-width: 0;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .name:disabled { cursor: default; }
  .name:focus-visible { outline: 1px solid var(--accent); }
  .caret { color: var(--muted); width: 10px; flex: none; }
  .ver, .who, .lic { color: var(--muted); }
  .ver { font-size: 11px; font-variant-numeric: tabular-nums; }
  .who { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .lic { font-size: 11px; white-space: nowrap; }
  .site { padding: 0 6px; font-size: 12px; line-height: 18px; }
  .note { margin: 2px 0 2px 15px; color: var(--muted); font-size: 11px; line-height: 1.4; }
  pre {
    margin: 4px 0 6px 15px; padding: 8px; max-height: 240px; overflow: auto; white-space: pre-wrap;
    font-size: 11px; line-height: 1.35; background: var(--bg); border: 1px solid var(--border); border-radius: 6px;
  }
  .muted { color: var(--muted); font-size: 12px; }
</style>
