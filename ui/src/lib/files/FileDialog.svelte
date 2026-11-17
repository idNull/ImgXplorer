<script lang="ts">
  // Selector de archivos propio: abrir archivo, abrir carpeta y guardar. Teclado: flechas para
  // elegir, Intro para entrar o aceptar, Retroceso para subir, Esc para cancelar.
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { fileDialog, dirOf, joinPath, withExtension, type PickRequest } from "./dialog.svelte";
  import { prefs } from "../prefs.svelte";
  import { label, t } from "../i18n.svelte";

  interface Entry {
    name: string;
    dir: boolean;
    size: number;
    modified: number | null;
  }
  interface Listing {
    path: string;
    parent: string | null;
    entries: Entry[];
    truncated: boolean;
  }
  interface Place {
    key: string;
    name: string;
    path: string;
  }

  interface Props {
    req: PickRequest;
  }
  let { req }: Props = $props();

  let listing = $state<Listing | null>(null);
  let places = $state<Place[]>([]);
  let error = $state<string | null>(null);
  let hidden = $state(false);
  let filterIdx = $state(0);
  let selected = $state<string | null>(null);
  let name = $state("");
  let pathText = $state("");
  let confirm = $state<string | null>(null);
  let listEl: HTMLDivElement;
  let nameEl = $state<HTMLInputElement>();

  const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
  const filters = $derived(req.filters ?? []);
  const exts = $derived(filters[filterIdx]?.extensions.map((e) => e.toLowerCase()) ?? []);

  const shown = $derived.by(() => {
    if (!listing) return [];
    const ok = (e: Entry) => {
      if (e.dir) return true;
      if (req.mode === "folder") return false;
      if (!exts.length || exts.includes("*")) return true;
      const n = e.name.toLowerCase();
      return exts.some((x) => n.endsWith(`.${x}`));
    };
    return listing.entries.filter(ok).sort((a, b) => (a.dir !== b.dir ? (a.dir ? -1 : 1) : collator.compare(a.name, b.name)));
  });

  const accept = $derived(req.mode === "save" ? t("fdSave") : req.mode === "folder" ? t("fdChooseFolder") : t("fdOpen"));
  const title = $derived(req.title ?? (req.mode === "save" ? t("fdTitleSave") : req.mode === "folder" ? t("fdTitleFolder") : t("fdTitleOpen")));
  const canAccept = $derived.by(() => {
    if (!listing) return false;
    if (req.mode === "save") return !!name.trim();
    if (req.mode === "folder") return true;
    return !!selected && !shown.find((e) => e.name === selected)?.dir;
  });

  async function go(path: string, select: string | null = null) {
    try {
      const l = await invoke<Listing>("list_dir", { path, hidden });
      listing = l;
      pathText = l.path;
      error = null;
      confirm = null;
      selected = select;
      await tick();
      listEl?.scrollTo({ top: 0 });
      if (select) reveal();
    } catch (e) {
      error = String(e);
      pathText = listing?.path ?? path;
    }
  }

  const up = () => {
    if (!listing?.parent) return;
    const from = listing.path.slice(listing.path.lastIndexOf("/") + 1);
    go(listing.parent, from);
  };

  function done(path: string | null) {
    if (path && listing) {
      prefs.settings.lastDir = req.mode === "folder" ? dirOf(path) : listing.path;
      prefs.saveSettings();
    }
    fileDialog.finish(path);
  }

  async function submit() {
    if (!listing) return;
    const dir = listing.path;
    const entry = selected ? shown.find((e) => e.name === selected) : undefined;
    if (req.mode === "open") {
      if (!entry) return;
      if (entry.dir) return go(joinPath(dir, entry.name));
      return done(joinPath(dir, entry.name));
    }
    if (req.mode === "folder") return done(entry?.dir ? joinPath(dir, entry.name) : dir);
    // Guardar
    const n = name.trim();
    if (!n) return;
    const full = withExtension(joinPath(dir, n), filters.length ? [filters[filterIdx]] : undefined);
    const known = listing.entries.find((e) => joinPath(dir, e.name) === full);
    if (known?.dir) return go(full);
    if (!known) {
      const [exists] = await invoke<boolean[]>("paths_exist", { paths: [full] });
      if (!exists) return done(full);
    }
    if (confirm === full) return done(full);
    confirm = full;
  }

  function activate(e: Entry) {
    if (e.dir) return go(joinPath(listing!.path, e.name));
    selected = e.name;
    if (req.mode === "save") name = e.name;
    submit();
  }

  function pick(e: Entry) {
    selected = e.name;
    confirm = null;
    if (req.mode === "save" && !e.dir) name = e.name;
  }

  async function reveal() {
    await tick();
    listEl?.querySelector(".row.sel")?.scrollIntoView({ block: "nearest" });
  }

  function move(d: number) {
    if (!shown.length) return;
    const i = shown.findIndex((e) => e.name === selected);
    const j = i < 0 ? (d > 0 ? 0 : shown.length - 1) : Math.max(0, Math.min(shown.length - 1, i + d));
    pick(shown[j]);
    reveal();
  }

  // Escribir letras salta a la primera entrada que empieza así.
  let typed = "";
  let typedAt = 0;
  function typeAhead(ch: string) {
    const now = performance.now();
    typed = now - typedAt > 800 ? ch : typed + ch;
    typedAt = now;
    const hit = shown.find((e) => e.name.toLowerCase().startsWith(typed.toLowerCase()));
    if (hit) {
      pick(hit);
      reveal();
    }
  }

  function onKey(e: KeyboardEvent) {
    // El diálogo es modal: ninguna tecla llega al resto de la app.
    e.stopImmediatePropagation();
    const tgt = e.target as HTMLElement;
    const inInput = tgt instanceof HTMLInputElement || tgt instanceof HTMLSelectElement;
    if (e.key === "Escape") {
      e.preventDefault();
      return done(null);
    }
    if (inInput) {
      if (e.key === "Enter" && tgt.classList.contains("path")) {
        e.preventDefault();
        go(pathText.trim() || "/");
      } else if (e.key === "Enter" && tgt.classList.contains("name")) {
        e.preventDefault();
        submit();
      } else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && tgt.classList.contains("name")) {
        e.preventDefault();
        move(e.key === "ArrowDown" ? 1 : -1);
      }
      return;
    }
    const entry = selected ? shown.find((x) => x.name === selected) : undefined;
    switch (e.key) {
      case "ArrowDown": move(1); break;
      case "ArrowUp": if (e.altKey) up(); else move(-1); break;
      case "PageDown": move(10); break;
      case "PageUp": move(-10); break;
      case "Home": move(-1e9); break;
      case "End": move(1e9); break;
      case "Backspace": up(); break;
      case "Enter":
        if (entry?.dir && !(e.ctrlKey && req.mode === "folder")) go(joinPath(listing!.path, entry.name));
        else submit();
        break;
      default:
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) typeAhead(e.key);
        else return;
    }
    e.preventDefault();
  }

  const fmtSize = (n: number) =>
    n < 1024 ? `${n} B` : n < 1024 ** 2 ? `${(n / 1024).toFixed(0)} KB` : n < 1024 ** 3 ? `${(n / 1024 ** 2).toFixed(1)} MB` : `${(n / 1024 ** 3).toFixed(1)} GB`;
  const fmtDate = (s: number | null) => (s ? new Date(s * 1000).toLocaleDateString(undefined, { year: "numeric", month: "2-digit", day: "2-digit" }) : "");
  const placeName = (p: Place) => (p.key ? label(`place.${p.key}`) : p.name);

  function toggleHidden() {
    hidden = !hidden;
    if (listing) go(listing.path, selected);
  }

  onMount(async () => {
    name = req.defaultName ?? "";
    try {
      places = await invoke<Place[]>("places");
    } catch {
      places = [];
    }
    const home = places.find((p) => p.key === "home")?.path ?? "/";
    const start = req.dir || prefs.settings.lastDir || home;
    await go(start);
    if (!listing) await go(home);
    if (req.mode === "save") {
      await tick();
      nameEl?.focus();
      // Seleccionar el nombre sin la extensión, como hacen los diálogos del sistema.
      const dot = name.lastIndexOf(".");
      nameEl?.setSelectionRange(0, dot > 0 ? dot : name.length);
    } else listEl?.focus();
  });
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && done(null)}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={title}>
    <header>
      <b>{title}</b>
      <button class="close" onclick={() => done(null)} title="{t('fdCancel')} (Esc)">✕</button>
    </header>
    <div class="bar">
      <button onclick={up} disabled={!listing?.parent} title="{t('fdUp')} (⌫)">↑</button>
      <input class="path" bind:value={pathText} spellcheck="false" title={t("fdPathHelp")} />
      <button class:on={hidden} onclick={toggleHidden} title={t("fdHiddenHelp")}>{t("fdHidden")}</button>
    </div>
    <div class="main">
      <nav>
        {#each places as p (p.path)}
          <button class:on={listing?.path === p.path} onclick={() => go(p.path)} title={p.path}>{placeName(p)}</button>
        {/each}
      </nav>
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div class="list" bind:this={listEl} tabindex="0" role="listbox" aria-label={title}>
        {#if error}<div class="msg err">{error}</div>{/if}
        {#if listing?.truncated}<div class="msg">{t("fdTruncated")}</div>{/if}
        {#each shown as e (e.name)}
          <div
            class="row"
            class:sel={selected === e.name}
            class:dir={e.dir}
            role="option"
            aria-selected={selected === e.name}
            tabindex="-1"
            onclick={() => pick(e)}
            ondblclick={() => activate(e)}
            onkeydown={() => {}}
          >
            <span class="icon">{e.dir ? "📁" : "📄"}</span>
            <span class="n" title={e.name}>{e.name}</span>
            <span class="s">{e.dir ? "" : fmtSize(e.size)}</span>
            <span class="d">{fmtDate(e.modified)}</span>
          </div>
        {:else}
          {#if listing && !error}<div class="msg">{t("fdEmpty")}</div>{/if}
        {/each}
      </div>
    </div>
    <footer>
      {#if req.mode === "save"}
        <label class="name-row">
          <span>{t("fdName")}</span>
          <input class="name" bind:this={nameEl} bind:value={name} spellcheck="false" oninput={() => (confirm = null)} />
        </label>
      {/if}
      {#if filters.length > 1}
        <select bind:value={filterIdx} onchange={() => (confirm = null)}>
          {#each filters as f, i (i)}<option value={i}>{f.name} ({f.extensions.map((x) => `*.${x}`).join(" ")})</option>{/each}
        </select>
      {:else if filters.length === 1}
        <span class="filter">{filters[0].name} ({filters[0].extensions.map((x) => `*.${x}`).join(" ")})</span>
      {/if}
      <span class="spacer"></span>
      {#if confirm}<span class="warn">{t("fdExists", { name: confirm.slice(confirm.lastIndexOf("/") + 1) })}</span>{/if}
      <button onclick={() => done(null)}>{t("fdCancel")}</button>
      <button class="primary" class:danger={!!confirm} onclick={submit} disabled={!canAccept}>{confirm ? t("fdReplace") : accept}</button>
    </footer>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; background: rgb(0 0 0 / 0.55); display: grid; place-items: center; }
  .dialog {
    width: min(860px, calc(100vw - 40px)); height: min(560px, calc(100vh - 40px)); display: flex; flex-direction: column;
    background: var(--panel); border: 1px solid var(--border); border-radius: 10px; box-shadow: 0 12px 40px rgb(0 0 0 / 0.5); font-size: 13px;
  }
  header { display: flex; align-items: center; padding: 8px 12px; border-bottom: 1px solid var(--border); }
  header b { flex: 1; }
  .close { background: none; border-color: transparent; }
  .bar { display: flex; gap: 6px; padding: 8px 12px; }
  .bar button { padding: 2px 10px; }
  .bar button.on { border-color: var(--accent); color: var(--accent); }
  .path { flex: 1; min-width: 0; font: inherit; color: var(--text); background: var(--bg); border: 1px solid var(--border); border-radius: 6px; padding: 3px 8px; }
  .path:focus { border-color: var(--accent); outline: none; }
  .main { flex: 1; min-height: 0; display: flex; border-top: 1px solid var(--border); border-bottom: 1px solid var(--border); }
  nav { width: 170px; flex: none; overflow-y: auto; padding: 6px; display: flex; flex-direction: column; gap: 2px; border-right: 1px solid var(--border); }
  nav button { text-align: left; background: transparent; border-color: transparent; padding: 3px 8px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  nav button:hover { background: var(--panel-2); }
  nav button.on { color: var(--accent); }
  .list { flex: 1; min-width: 0; overflow-y: auto; padding: 4px 0; outline: none; }
  .row { display: flex; align-items: center; gap: 8px; padding: 3px 12px; cursor: default; user-select: none; }
  .row:hover { background: var(--panel-2); }
  .row.sel { background: rgb(56 189 248 / 0.22); }
  .list:focus .row.sel { box-shadow: inset 0 0 0 1px var(--accent); }
  .icon { width: 18px; text-align: center; }
  .n { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .s, .d { color: var(--muted); font-size: 12px; font-variant-numeric: tabular-nums; flex: none; }
  .s { width: 70px; text-align: right; }
  .d { width: 90px; text-align: right; }
  .msg { padding: 10px 14px; color: var(--muted); }
  .msg.err { color: #fca5a5; }
  footer { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; padding: 10px 12px; }
  .name-row { display: flex; align-items: center; gap: 8px; flex: 1 1 300px; }
  .name { flex: 1; min-width: 0; font: inherit; color: var(--text); background: var(--bg); border: 1px solid var(--border); border-radius: 6px; padding: 3px 8px; }
  .name:focus { border-color: var(--accent); outline: none; }
  .filter { color: var(--muted); font-size: 12px; }
  .spacer { flex: 1; }
  .warn { color: #fbbf24; font-size: 12px; }
  .primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
  .primary.danger { background: #b45309; border-color: #f59e0b; color: #fff; }
  .primary:disabled { opacity: 0.5; }
</style>
