<script lang="ts">
  // Lista de recientes: en la pantalla de inicio y como menú desplegable de la cabecera.
  import { invoke } from "@tauri-apps/api/core";
  import { prefs } from "./prefs.svelte";
  import { t } from "./i18n.svelte";

  interface Props {
    onopen: (path: string) => void;
    /** Ruta de la fuente abierta (se marca). */
    current?: string | null;
  }
  let { onopen, current = null }: Props = $props();

  let missing = $state<Record<string, boolean>>({});

  // Se comprueba qué rutas siguen existiendo cada vez que cambia la lista.
  $effect(() => {
    const paths = prefs.recents.map((r) => r.path);
    if (!paths.length) return;
    let cancelled = false;
    invoke<boolean[]>("paths_exist", { paths }).then((ok) => {
      if (!cancelled) missing = Object.fromEntries(paths.map((p, i) => [p, !ok[i]]));
    });
    return () => {
      cancelled = true;
    };
  });

  const ago = (ms: number) => {
    const f = new Intl.RelativeTimeFormat(document.documentElement.lang || undefined, { numeric: "auto" });
    const s = (ms - Date.now()) / 1000;
    for (const [u, n] of [["year", 31536000], ["month", 2592000], ["day", 86400], ["hour", 3600], ["minute", 60]] as const) {
      if (Math.abs(s) >= n) return f.format(Math.round(s / n), u);
    }
    return f.format(0, "minute");
  };
</script>

<ul class="recents">
  {#each prefs.recents as r (r.path)}
    <li class:current={r.path === current} class:missing={missing[r.path]}>
      <button class="open" onclick={() => onopen(r.path)} disabled={missing[r.path]} title={missing[r.path] ? t("missing") : r.path}>
        <span class="icon">{r.kind === "archive" ? "📚" : "📁"}</span>
        <span class="text">
          <span class="title">{r.title}</span>
          <span class="path">{r.path}</span>
        </span>
        <span class="meta">
          {#if r.count > 1}<span>{t("pageOf", { i: r.index + 1, n: r.count })}</span>{/if}
          <span>{missing[r.path] ? t("missing") : ago(r.at)}</span>
        </span>
      </button>
      <button class="forget" onclick={() => { prefs.forget(r.path); invoke("forget_cover", { path: r.path }).catch(() => {}); }} title={t("forget")} aria-label={t("forget")}>✕</button>
    </li>
  {:else}
    <li class="empty">{t("noRecents")}</li>
  {/each}
</ul>

<style>
  .recents { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  li { display: flex; align-items: stretch; gap: 2px; }
  .open {
    flex: 1; min-width: 0; display: flex; align-items: center; gap: 10px; text-align: left;
    background: transparent; border-color: transparent; padding: 6px 8px;
  }
  .open:hover:not(:disabled) { background: var(--panel-2); }
  li.current .open { border-color: var(--accent); }
  .icon { font-size: 18px; }
  .text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .title, .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .path { color: var(--muted); font-size: 11px; }
  .meta { display: flex; flex-direction: column; align-items: flex-end; color: var(--muted); font-size: 11px; white-space: nowrap; }
  .forget { background: transparent; border-color: transparent; color: var(--muted); padding: 0 8px; }
  .forget:hover { color: #fca5a5; }
  li.missing .title { text-decoration: line-through; color: var(--muted); }
  .empty { color: var(--muted); padding: 8px; }
</style>
