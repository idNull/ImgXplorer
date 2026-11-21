<script lang="ts">
  // Estantería de la pantalla de inicio: los mismos recientes, con portada y progreso. La lista de
  // siempre (Recents) sigue a un clic con «☰ Lista» y en el menú 🕘.
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { prefs, type Recent } from "./prefs.svelte";
  import { t } from "./i18n.svelte";
  import { log } from "./log";

  interface Props {
    onopen: (path: string) => void;
    /** «📦 Compartir historia…»: abrirlo y exportarlo como `.imgxstory`. */
    onshare?: (path: string) => void;
  }
  let { onopen, onshare }: Props = $props();

  type Sort = "recent" | "name" | "progress";
  let sort = $state<Sort>("recent");
  let query = $state("");
  let missing = $state<Record<string, boolean>>({});
  let menu = $state<{ x: number; y: number; r: Recent } | null>(null);

  const base = convertFileSrc("", "imgx");
  /** URL de la portada de cada ruta (`null`: no se pudo hacer). */
  let covers = $state<Record<string, string | null>>({});

  // Qué rutas siguen existiendo.
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

  // Portadas: se piden una a una (Rust las crea si faltan, en segundo plano), primero la de arriba.
  $effect(() => {
    const paths = prefs.recents.map((r) => r.path).filter((p) => !(p in covers));
    let cancelled = false;
    (async () => {
      for (const path of paths) {
        if (cancelled) return;
        try {
          const c = await invoke<{ key: string; version: number } | null>("cover", { path });
          if (!cancelled) covers[path] = c ? `${base}c/${c.key}?v=${c.version}` : null;
        } catch (e) {
          log.warn("portada:", path, e);
          if (!cancelled) covers[path] = null;
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  });

  const progress = (r: Recent) => (r.count > 1 ? (r.index + 1) / r.count : 0);
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = prefs.recents.filter((r) => !q || r.title.toLowerCase().includes(q) || r.path.toLowerCase().includes(q));
    if (sort === "name") return [...list].sort((a, b) => a.title.localeCompare(b.title, undefined, { numeric: true, sensitivity: "base" }));
    if (sort === "progress") return [...list].sort((a, b) => progress(b) - progress(a));
    return list;
  });
  /** «Continuar leyendo»: el último abierto, si sigue existiendo y no se está buscando. */
  const hero = $derived(!query.trim() && sort === "recent" && prefs.recents[0] && !missing[prefs.recents[0].path] ? prefs.recents[0] : null);
  const grid = $derived(hero ? shown.filter((r) => r !== hero) : shown);

  const ago = (ms: number) => {
    const f = new Intl.RelativeTimeFormat(document.documentElement.lang || undefined, { numeric: "auto" });
    const s = (ms - Date.now()) / 1000;
    for (const [u, n] of [["year", 31536000], ["month", 2592000], ["day", 86400], ["hour", 3600], ["minute", 60]] as const) {
      if (Math.abs(s) >= n) return f.format(Math.round(s / n), u);
    }
    return f.format(0, "minute");
  };

  function forget(r: Recent) {
    menu = null;
    prefs.forget(r.path);
    delete covers[r.path];
    invoke("forget_cover", { path: r.path }).catch(() => {});
  }

  function folder(r: Recent) {
    menu = null;
    invoke("show_in_folder", { path: r.path }).catch((e) => log.warn("abrir carpeta:", e));
  }

  function context(e: MouseEvent, r: Recent) {
    e.preventDefault();
    menu = { x: e.clientX, y: e.clientY, r };
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && menu && (menu = null)} />

<div class="shelf">
  {#if prefs.recents.length > 12}
    <div class="tools">
      <input type="search" placeholder={t("shelfSearch")} bind:value={query} />
      <select bind:value={sort} title={t("shelfSort")}>
        <option value="recent">{t("shelfSort_recent")}</option>
        <option value="name">{t("shelfSort_name")}</option>
        <option value="progress">{t("shelfSort_progress")}</option>
      </select>
    </div>
  {/if}

  {#if hero}
    <button class="hero" onclick={() => onopen(hero.path)} oncontextmenu={(e) => context(e, hero)} title={hero.path}>
      <span class="cover big">
        {#if covers[hero.path]}<img src={covers[hero.path]} alt="" draggable="false" />{:else}<span class="ph">{hero.kind === "archive" ? "📚" : "📁"}</span>{/if}
      </span>
      <span class="info">
        <span class="kicker">{t("shelfContinue")}</span>
        <span class="htitle">{hero.title}</span>
        {#if hero.count > 1}
          <span class="meta">{t("pageOf", { i: hero.index + 1, n: hero.count })} · {Math.round(progress(hero) * 100)} %</span>
          <span class="bar"><span style:width="{progress(hero) * 100}%"></span></span>
        {/if}
        <span class="meta">{ago(hero.at)}</span>
        <span class="go">▶ {t("shelfResume")}</span>
      </span>
    </button>
  {/if}

  {#if grid.length}
    <div class="grid">
      {#each grid as r (r.path)}
        <div class="card" class:missing={missing[r.path]}>
          <button class="open" onclick={() => onopen(r.path)} oncontextmenu={(e) => context(e, r)} disabled={missing[r.path]}
            title={missing[r.path] ? t("missing") : r.path}>
            <span class="cover">
              {#if covers[r.path] && !missing[r.path]}<img src={covers[r.path]} alt="" loading="lazy" draggable="false" />
              {:else}<span class="ph">{r.kind === "archive" ? "📚" : "📁"}</span>{/if}
              {#if missing[r.path]}<span class="gone">{t("missing")}</span>{/if}
            </span>
            <span class="ctitle">{r.title}</span>
            {#if r.count > 1}<span class="bar"><span style:width="{progress(r) * 100}%"></span></span>{/if}
            <span class="meta">{r.count > 1 ? `${r.index + 1}/${r.count} · ` : ""}{ago(r.at)}</span>
          </button>
          <button class="forget" onclick={() => forget(r)} title={t("forget")} aria-label={t("forget")}>✕</button>
        </div>
      {/each}
    </div>
  {:else if query.trim()}
    <p class="empty">{t("aboutNoMatch")}</p>
  {/if}
</div>

{#if menu}
  <div class="backdrop" role="presentation" onclick={() => (menu = null)} oncontextmenu={(e) => { e.preventDefault(); menu = null; }}></div>
  <div class="ctx" role="menu" style:left="{menu.x}px" style:top="{menu.y}px">
    {#if !missing[menu.r.path]}
      <button onclick={() => { const p = menu!.r.path; menu = null; onopen(p); }}>{t("shelfOpen")}</button>
      <button onclick={() => folder(menu!.r)}>{t("shelfFolder")}</button>
      {#if onshare}<button onclick={() => { const p = menu!.r.path; menu = null; onshare(p); }}>📦 {t("storyExport")}…</button>{/if}
    {/if}
    <button class="danger" onclick={() => forget(menu!.r)}>{t("shelfForget")}</button>
  </div>
{/if}

<style>
  .shelf { display: flex; flex-direction: column; gap: 14px; width: 100%; }
  .tools { display: flex; gap: 8px; }
  .tools input { flex: 1; }
  .hero {
    display: flex; gap: 18px; align-items: stretch; padding: 12px; text-align: left; border-radius: 12px;
    background: var(--panel); border: 1px solid var(--border); color: var(--text);
  }
  .hero:hover { border-color: var(--accent); }
  .info { display: flex; flex-direction: column; justify-content: center; gap: 6px; min-width: 0; flex: 1; }
  .kicker { color: var(--accent); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; font-weight: 600; }
  .htitle { font-size: 20px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .go { margin-top: 6px; align-self: flex-start; padding: 4px 12px; border-radius: 6px; background: var(--accent); color: #04121b; font-weight: 600; font-size: 13px; }
  .cover {
    position: relative; display: grid; place-items: center; overflow: hidden; border-radius: 6px; background: var(--bg);
    aspect-ratio: 0.7; width: 100%;
  }
  .cover.big { width: 130px; flex: none; }
  .cover img { width: 100%; height: 100%; object-fit: cover; }
  .ph { font-size: 34px; opacity: 0.5; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 12px; }
  .card { position: relative; min-width: 0; }
  .open {
    display: flex; flex-direction: column; gap: 5px; width: 100%; padding: 6px; text-align: left;
    background: transparent; border-color: transparent; border-radius: 8px; color: var(--text);
  }
  .open:hover:not(:disabled) { background: var(--panel-2); }
  .ctitle { font-size: 12.5px; line-height: 1.25; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; }
  .meta { color: var(--muted); font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .bar { display: block; height: 3px; border-radius: 2px; background: rgb(255 255 255 / 0.1); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent); }
  .forget {
    position: absolute; top: 10px; right: 10px; padding: 0 6px; font-size: 11px; opacity: 0;
    background: rgb(0 0 0 / 0.65); border-color: transparent; color: var(--muted);
  }
  .card:hover .forget, .forget:focus-visible { opacity: 1; }
  .forget:hover { color: #fca5a5; }
  .missing .cover { filter: grayscale(1); opacity: 0.5; }
  .missing .ctitle { text-decoration: line-through; color: var(--muted); }
  .gone { position: absolute; bottom: 6px; left: 6px; right: 6px; text-align: center; font-size: 11px; padding: 2px; border-radius: 4px; background: rgb(0 0 0 / 0.7); color: #fca5a5; }
  .empty { color: var(--muted); }
  .backdrop { position: fixed; inset: 0; z-index: 20; }
  .ctx {
    position: fixed; z-index: 21; display: flex; flex-direction: column; min-width: 180px; padding: 4px;
    background: var(--panel); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 8px 30px rgb(0 0 0 / 0.45);
  }
  .ctx button { text-align: left; background: transparent; border-color: transparent; padding: 5px 10px; font-size: 13px; }
  .ctx button:hover { background: var(--panel-2); }
  .ctx .danger:hover { color: #fca5a5; }
</style>
