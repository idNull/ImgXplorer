<script lang="ts">
  // Crear un cómic con imágenes sueltas y la mesa de luz para ordenar sus páginas.
  //   create  imágenes de fuera → un cómic nuevo
  //   arrange el cómic propio abierto: se ordena en su sitio
  //   copy    otra fuente (CBZ, carpeta…): se crea un cómic nuevo con este orden; la fuente no cambia
  import { invoke } from "@tauri-apps/api/core";
  import { errorText } from "../errors";
  import { onMount, tick, untrack } from "svelte";
  import PageTable from "./PageTable.svelte";
  import { byName, comicsDir, fromFiles, moveKeys, newKey, type ArrangePage, type Tile } from "./comic.svelte";
  import { openFiles, openFolder, fileDialog } from "../files/dialog.svelte";
  import { IMAGES } from "../files/formats";
  import { t } from "../i18n.svelte";

  export type Mode = "create" | "arrange" | "copy";
  interface Props {
    mode: Mode;
    /** Fuente abierta (arrange y copy). */
    generation?: number;
    entries?: { name: string }[];
    title?: string;
    thumb?: (index: number) => string;
    /** Página en la que se está: empieza elegida. */
    current?: number;
    /** Imágenes con las que empezar (create). */
    files?: string[];
    /** La animación de la fuente abierta (copy): va con el cómic nuevo. */
    project?: () => string;
    /** Guardar el orden del cómic abierto (lo hace la app: guarda, ordena y vuelve a abrirlo). */
    onarrange?: (pages: ArrangePage[], removed: string[]) => Promise<boolean>;
    /** Abrir el cómic recién creado. */
    onopen: (path: string) => void;
    onclose: () => void;
  }
  let { mode, generation, entries = [], title = "", thumb = () => "", current = 0, files = [], project, onarrange, onopen, onclose }: Props = $props();

  // Lo de partida: el diálogo trabaja con su copia (las props no cambian mientras está abierto).
  const start = untrack(() => ({ mode, title, current, initial: mode === "create" ? fromFiles(files) : entries.map((e, index): Tile => ({ key: newKey(), name: e.name, index })) }));
  let tiles = $state<Tile[]>(start.initial);
  let selected = $state<string[]>(start.mode === "create" || !start.initial[start.current] ? [] : [start.initial[start.current].key]);
  let removed = $state<Tile[]>([]);
  let past = $state<{ tiles: Tile[]; removed: Tile[] }[]>([]);
  let future = $state<{ tiles: Tile[]; removed: Tile[] }[]>([]);
  let name = $state(start.mode === "copy" ? start.title : "");
  let dir = $state("");
  let busy = $state(false);
  // El error se traduce al enseñarlo (si cambia el idioma, cambia también).
  let error = $state<(() => string) | null>(null);
  let leaving = $state(false);
  let nameEl = $state<HTMLInputElement>();

  const changed = $derived(past.length > 0);
  const added = $derived(tiles.filter((x) => x.path).length);
  const heading = $derived(mode === "create" ? t("comicCreate") : t("comicOrganizeTitle", { title }));
  const primary = $derived(mode === "create" ? t("comicCreateGo") : mode === "arrange" ? t("comicSave") : t("comicCopyGo"));

  onMount(async () => {
    dir = await comicsDir();
    if (mode === "create") {
      await tick();
      nameEl?.focus();
    }
  });

  function commit(next: Tile[], nextRemoved = removed) {
    past = [...past.slice(-99), { tiles, removed }];
    future = [];
    tiles = next;
    removed = nextRemoved;
    error = null;
  }
  function undo() {
    const p = past.at(-1);
    if (!p) return;
    future = [...future, { tiles, removed }];
    past = past.slice(0, -1);
    ({ tiles, removed } = p);
  }
  function redo() {
    const f = future.at(-1);
    if (!f) return;
    past = [...past, { tiles, removed }];
    future = future.slice(0, -1);
    ({ tiles, removed } = f);
  }

  /** Añade imágenes de fuera delante o detrás de lo elegido (o al principio / al final). */
  export function addFiles(paths: string[], where: "before" | "after" = "after") {
    const list = fromFiles(paths.filter((p) => IMAGES.includes(p.slice(p.lastIndexOf(".") + 1).toLowerCase())));
    if (!list.length) return;
    const idx = tiles.map((x, i) => (selected.includes(x.key) ? i : -1)).filter((i) => i >= 0);
    const at = where === "before" ? (idx.length ? Math.min(...idx) : 0) : idx.length ? Math.max(...idx) + 1 : tiles.length;
    const next = [...tiles];
    next.splice(at, 0, ...list);
    commit(next);
    selected = list.map((x) => x.key);
  }

  async function pickAndAdd(where: "before" | "after") {
    const paths = await openFiles({ title: t("comicAddTitle"), filters: [{ name: t("filterImages"), extensions: IMAGES }] });
    if (paths) addFiles(paths, where);
  }

  function removeSelected() {
    if (!selected.length) return;
    const gone = tiles.filter((x) => selected.includes(x.key));
    const i = tiles.findIndex((x) => selected.includes(x.key));
    const next = tiles.filter((x) => !selected.includes(x.key));
    // Las de la fuente se recuerdan (al guardar van a `.quitadas/`); las nuevas solo se olvidan.
    commit(next, [...removed, ...gone.filter((x) => x.index !== undefined)]);
    selected = next[Math.min(i, next.length - 1)] ? [next[Math.min(i, next.length - 1)].key] : [];
  }

  function sortByName() {
    commit([...tiles].sort(byName));
  }

  function nudge(d: number) {
    if (!selected.length) return;
    const idx = tiles.map((x, i) => (selected.includes(x.key) ? i : -1)).filter((i) => i >= 0);
    const to = d < 0 ? Math.max(0, Math.min(...idx) - 1) : Math.min(tiles.length, Math.max(...idx) + 2);
    commit(moveKeys(tiles, selected, to));
  }

  async function changeDir() {
    const d = await openFolder({ title: t("comicFolder"), dir: dir || undefined });
    if (d) dir = d;
  }

  async function finish() {
    if (busy) return;
    error = null;
    if (!tiles.length) return (error = () => t("comicEmpty"));
    if (mode !== "arrange" && !name.trim()) {
      error = () => t("comicNameMissing");
      nameEl?.focus();
      return;
    }
    busy = true;
    try {
      if (mode === "create") {
        const path = await invoke<string>("comic_create", { title: name.trim(), dir: dir || null, files: tiles.map((x) => x.path!) });
        onclose();
        onopen(path);
      } else if (mode === "copy") {
        const pages = tiles.map((x) => (x.index !== undefined ? { index: x.index } : { path: x.path! }));
        const path = await invoke<string>("comic_from_source", { generation, title: name.trim(), dir: dir || null, pages, project: project?.() ?? null });
        onclose();
        onopen(path);
      } else {
        const pages: ArrangePage[] = tiles.map((x) => (x.index !== undefined ? { entry: entries[x.index].name } : { path: x.path! }));
        if (await onarrange?.(pages, removed.map((x) => entries[x.index!].name))) onclose();
      }
    } catch (e) {
      error = () => errorText(e);
    } finally {
      busy = false;
    }
  }

  function close() {
    if (busy) return;
    if (changed && !leaving) return (leaving = true);
    onclose();
  }

  function onKey(e: KeyboardEvent) {
    if (fileDialog.request) return; // el selector de archivos, encima, tiene sus teclas
    e.stopImmediatePropagation();
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
    const ctrl = e.ctrlKey || e.metaKey;
    if (e.key === "Escape") {
      e.preventDefault();
      return leaving ? (leaving = false) : close();
    }
    if (typing) {
      if (e.key === "Enter") {
        e.preventDefault();
        void finish();
      }
      return;
    }
    const k = e.key.toLowerCase();
    if (ctrl && k === "z" && !e.shiftKey) undo();
    else if (ctrl && (k === "y" || (k === "z" && e.shiftKey))) redo();
    else if (ctrl && k === "a") selected = tiles.map((x) => x.key);
    else if (e.key === "Delete" || e.key === "Backspace") removeSelected();
    else if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowUp")) nudge(-1);
    else if (e.altKey && (e.key === "ArrowRight" || e.key === "ArrowDown")) nudge(1);
    else if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      const i = tiles.findIndex((x) => x.key === selected.at(-1));
      const j = Math.max(0, Math.min(tiles.length - 1, i + (e.key === "ArrowLeft" ? -1 : 1)));
      if (tiles[j]) selected = [tiles[j].key];
    } else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={heading}>
    <header>
      <b>{mode === "create" ? "➕" : "▦"} {heading}</b>
      <span class="count">{t("comicCount", { n: tiles.length })}{added && mode !== "create" ? ` · ${t("comicAdded", { n: added })}` : ""}{removed.length ? ` · ${t("comicRemoved", { n: removed.length })}` : ""}</span>
      <button class="close" onclick={close} title="{t('close')} (Esc)">✕</button>
    </header>
    <div class="tools" role="toolbar">
      <button onclick={() => pickAndAdd("before")} title={t("comicAddBeforeHelp")}>➕ {t("comicAddBefore")}</button>
      <button onclick={() => pickAndAdd("after")} title={t("comicAddAfterHelp")}>➕ {t("comicAddAfter")}</button>
      <button onclick={sortByName} disabled={tiles.length < 2} title={t("comicSortHelp")}>⇅ {t("comicSort")}</button>
      <button onclick={removeSelected} disabled={!selected.length} title="{t('comicRemoveHelp')} (Supr)">🗑 {t("comicRemove")}</button>
      <span class="sep"></span>
      <button onclick={undo} disabled={!past.length} title="{t('undo')} (Ctrl+Z)">↶</button>
      <button onclick={redo} disabled={!future.length} title="{t('redo')} (Ctrl+Y)">↷</button>
    </div>
    {#if tiles.length}
      <PageTable {tiles} {selected} {thumb} onselect={(k) => (selected = k)} onmove={(keys, to) => commit(moveKeys(tiles, keys, to))}
        isNew={mode === "create" ? undefined : (x) => !!x.path} />
    {:else}
      <div class="empty">
        <p class="big">🖼️</p>
        <p>{t("comicEmptyHint")}</p>
        <button class="primary" onclick={() => pickAndAdd("after")}>➕ {t("comicChoose")}</button>
      </div>
    {/if}
    <p class="hint">{t("comicTableHint")}</p>
    {#if mode === "copy"}<p class="note">{t("comicCopyNote")}</p>{/if}
    {#if mode !== "arrange"}
      <div class="fields">
        <label>
          <span>{t("comicName")}</span>
          <input bind:this={nameEl} bind:value={name} maxlength="120" placeholder={t("comicNamePh")} oninput={() => (error = null)} />
        </label>
        <label class="where">
          <span>{t("comicFolder")}</span>
          <input value={dir} readonly title={dir} />
          <button onclick={changeDir}>{t("comicChange")}</button>
        </label>
      </div>
    {/if}
    <footer>
      {#if error}<span class="err">{error()}</span>{/if}
      {#if leaving}
        <span class="warn">{t("comicDiscard")}</span>
        <button onclick={() => (leaving = false)}>{t("comicKeep")}</button>
        <button class="danger" onclick={onclose}>{t("comicDiscardGo")}</button>
      {:else}
        <span class="spacer"></span>
        <button onclick={close}>{t("cancel")}</button>
        <button class="primary" onclick={finish} disabled={busy}>{busy ? t("comicWorking") : primary}</button>
      {/if}
    </footer>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 30; display: flex; align-items: center; justify-content: center; background: rgb(0 0 0 / 0.55); }
  .dialog {
    width: min(1100px, 94vw); height: min(820px, 90vh); display: flex; flex-direction: column; background: var(--panel);
    border: 1px solid var(--border); border-radius: 12px; box-shadow: var(--shadow); overflow: hidden;
  }
  header { display: flex; align-items: center; gap: 12px; padding: 10px 12px; border-bottom: 1px solid var(--border); }
  header b { font-size: 15px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count { flex: 1; color: var(--muted); font-size: 12px; white-space: nowrap; }
  .close { background: none; border-color: transparent; }
  .tools { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 8px 12px; border-bottom: 1px solid var(--border); }
  .tools button { font-size: 12.5px; }
  .sep { width: 1px; height: 20px; background: var(--border); margin: 0 4px; }
  .empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; color: var(--muted); border: 2px dashed var(--border); margin: 14px; border-radius: 12px; }
  .empty .big { font-size: 40px; margin: 0; }
  .hint, .note { margin: 0; padding: 4px 12px; font-size: 11.5px; color: var(--muted); }
  .note { color: #fbbf24; }
  .fields { display: flex; flex-wrap: wrap; gap: 10px 18px; padding: 8px 12px; border-top: 1px solid var(--border); }
  .fields label { display: flex; align-items: center; gap: 8px; font-size: 13px; }
  .fields input { min-width: 220px; }
  .fields .where { flex: 1; }
  .fields .where input { flex: 1; color: var(--muted); }
  footer { display: flex; align-items: center; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
  .spacer { flex: 1; }
  .err { flex: 1; color: #fca5a5; font-size: 12.5px; }
  .warn { flex: 1; color: #fbbf24; font-size: 12.5px; }
  .primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
  .danger { border-color: #ef4444; color: #fecaca; }
</style>
