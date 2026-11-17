<script lang="ts">
  // Biblioteca: clips para arrastrar a la timeline y plantillas propias para aplicar.
  import { openFile, saveFile } from "../files/dialog.svelte";
  import { library, defaultDuration, type LibClip } from "./libstore.svelte";
  import type { ProjectStore } from "../action/project.svelte";
  import { label, t } from "../i18n.svelte";
  import { log } from "../log";

  interface Props {
    store: ProjectStore;
    entry: string;
    size: [number, number] | null;
    onadd: (c: LibClip) => void;
    ondrag: (c: LibClip, e: PointerEvent) => void;
    onnotice: (msg: string) => void;
    /** Ajustar el ancho de la columna a los nombres. */
    onfit: () => void;
  }
  let { store, entry, size, onadd, ondrag, onnotice, onfit }: Props = $props();

  let tab = $state<"clips" | "templates">("clips");
  let renaming = $state<string | null>(null);

  const ICON: Record<string, string> = { keyframes: "◆", recorded: "≈", hold: "⏸" };
  const FILTER = [{ name: "imgxplorer", extensions: ["imgxpack"] }];

  const current = $derived(store.templateFor(entry));

  function saveTemplate() {
    if (!current) return;
    const base = label(`tpl.${current.id}`);
    const n = library.templates.filter((x) => x.name.startsWith(base)).length;
    library.addTemplate(n ? `${base} ${n + 1}` : base, $state.snapshot(current));
    tab = "templates";
    onnotice(t("templateSaved"));
  }

  function applyTemplate(id: string, scope: "item" | "all") {
    const tpl = library.templates.find((x) => x.id === id)?.template;
    if (!tpl) return;
    const copy = $state.snapshot(tpl);
    if (scope === "all") store.setDefaultTemplate(copy);
    else if (size) {
      // La plantilla sustituye a la animación propia de la imagen (se puede deshacer).
      store.setTimeline(entry, size, null);
      store.setItemTemplate(entry, size, copy);
    }
  }

  async function exportPack() {
    const path = await saveFile({ defaultName: "imgxplorer.imgxpack", filters: FILTER, title: t("export").replace("…", "") });
    if (!path) return;
    try {
      await library.exportPack(path);
      onnotice(t("exported", { path }));
    } catch (e) {
      log.warn("exportar biblioteca:", e);
      onnotice(t("packError", { error: String(e) }));
    }
  }

  async function importPack() {
    const path = await openFile({ filters: FILTER, title: t("import").replace("…", "") });
    if (!path) return;
    try {
      const r = await library.importPack(path);
      onnotice(r.clips || r.templates ? t("imported", { clips: r.clips, templates: r.templates }) : t("importedNothing"));
    } catch (e) {
      log.warn("importar biblioteca:", e);
      onnotice(t("packError", { error: String(e) }));
    }
  }

  function finishRename(id: string, value: string) {
    library.rename(id, value);
    renaming = null;
  }
</script>

<div class="lib">
  <div class="seg">
    <button class:on={tab === "clips"} onclick={() => (tab = "clips")}>{t("libClips")}</button>
    <button class:on={tab === "templates"} onclick={() => (tab = "templates")}>{t("libTemplates")}</button>
    <button class="fit" onclick={onfit} title={t("libFitHelp")}>↔</button>
  </div>

  <ul>
    {#if tab === "clips"}
      {#each library.all() as c (c.id)}
        <li class:builtin={c.builtin}>
          <button class="item" title={t("libDragHelp")} onpointerdown={(e) => ondrag(c, e)} ondblclick={() => !c.builtin && (renaming = c.id)}>
            <span class="icon">{ICON[c.clip.type]}</span>
            {#if renaming === c.id}
              <!-- svelte-ignore a11y_autofocus -->
              <input value={c.name} autofocus onpointerdown={(e) => e.stopPropagation()}
                onkeydown={(e) => { if (e.key === "Enter") finishRename(c.id, e.currentTarget.value); if (e.key === "Escape") renaming = null; e.stopPropagation(); }}
                onblur={(e) => finishRename(c.id, e.currentTarget.value)} />
            {:else}
              <span class="name">{c.name}</span>
            {/if}
            <span class="dur">{defaultDuration(c.clip).toFixed(1)} s</span>
          </button>
          <button class="small" title={t("libAdd")} onclick={() => onadd(c)}>+</button>
          {#if !c.builtin}<button class="small x" title={t("forget")} onclick={() => library.remove(c.id)}>✕</button>{/if}
        </li>
      {/each}
    {:else}
      {#each library.templates as tp (tp.id)}
        <li>
          <span class="item static" role="button" tabindex="-1" ondblclick={() => (renaming = tp.id)}>
            {#if renaming === tp.id}
              <!-- svelte-ignore a11y_autofocus -->
              <input value={tp.name} autofocus
                onkeydown={(e) => { if (e.key === "Enter") finishRename(tp.id, e.currentTarget.value); if (e.key === "Escape") renaming = null; e.stopPropagation(); }}
                onblur={(e) => finishRename(tp.id, e.currentTarget.value)} />
            {:else}
              <span class="name" title={label(`tpl.${tp.template.id}`)}>{tp.name}</span>
            {/if}
          </span>
          <button class="small" title={t("applyItem")} onclick={() => applyTemplate(tp.id, "item")} disabled={!size}>{t("applyItemShort")}</button>
          <button class="small" title={t("applyAll")} onclick={() => applyTemplate(tp.id, "all")}>{t("applyAllShort")}</button>
          <button class="small x" title={t("forget")} onclick={() => library.remove(tp.id)}>✕</button>
        </li>
      {:else}
        <li class="empty">{t("noTemplatesSaved")}</li>
      {/each}
    {/if}
  </ul>

  <div class="foot">
    {#if tab === "templates"}
      <button onclick={saveTemplate} disabled={!current} title={current ? label(`tpl.${current.id}`) : t("noTemplateHere")}>{t("saveTemplate")}</button>
    {/if}
    <button onclick={exportPack} disabled={!library.clips.length && !library.templates.length} title={t("exportHelp")}>{t("export")}</button>
    <button onclick={importPack}>{t("import")}</button>
  </div>
</div>

<style>
  .lib { display: flex; flex-direction: column; gap: 6px; min-height: 0; height: 100%; }
  .seg { display: flex; }
  .seg button { flex: 1; border-radius: 0; padding: 2px 6px; font-size: 12px; }
  .seg button:first-child { border-radius: 6px 0 0 6px; }
  .seg button:nth-child(2) { border-radius: 0 6px 6px 0; border-left: none; }
  .seg .fit { flex: none; margin-left: 4px; border-radius: 6px; padding: 2px 6px; }
  .seg button.on { background: var(--accent); color: #04121b; border-color: var(--accent); }
  ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 1px; }
  li { display: flex; align-items: center; gap: 2px; }
  .item {
    flex: 1; min-width: 0; display: flex; align-items: center; gap: 6px; padding: 2px 6px; text-align: left;
    background: transparent; border-color: transparent; cursor: grab; font-size: 12px;
  }
  .item.static { cursor: default; }
  .item:hover { background: var(--panel-2); }
  .icon { color: var(--accent); width: 12px; text-align: center; }
  li.builtin .icon { color: var(--muted); }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item input { flex: 1; min-width: 0; font: inherit; color: var(--text); background: var(--bg); border: 1px solid var(--accent); border-radius: 4px; padding: 0 4px; }
  .dur { color: var(--muted); font-size: 11px; font-variant-numeric: tabular-nums; }
  .small { padding: 0 6px; font-size: 12px; }
  .x { background: none; border-color: transparent; color: var(--muted); }
  .x:hover { color: #fca5a5; }
  .empty { color: var(--muted); font-size: 12px; padding: 4px; }
  .foot { display: flex; flex-wrap: wrap; gap: 4px; }
  .foot button { padding: 2px 8px; font-size: 12px; }
</style>
