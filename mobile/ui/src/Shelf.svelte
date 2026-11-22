<script lang="ts">
  // Estantería: lo que hay en la biblioteca del móvil, con portada y progreso. Desde aquí se eligen
  // fotos del carrete o archivos (cómics, historias) y se abre cada uno. Mantener pulsado, quitar.
  import logo from "../../../src-tauri/icons/svg/icono.svg?url";
  import { coverUrl, library, type Item } from "./lib/api";
  import { t } from "./lib/i18n";

  let { items, busy, onopen, onimport, oninfo, onchanged }: {
    items: Item[];
    busy: boolean;
    onopen: (item: Item) => void;
    onimport: (kind: "images" | "documents") => void;
    oninfo: () => void;
    onchanged: () => void;
  } = $props();

  // Portadas: se piden a Rust según aparecen (la primera vez la crea a partir de la primera página).
  let covers = $state<Record<string, string | null>>({});
  $effect(() => {
    for (const it of items) {
      if (it.path in covers) continue;
      covers[it.path] = null;
      coverUrl(it.path).then((u) => (covers[it.path] = u));
    }
  });

  const resume = $derived(items.find((i) => i.opened > 0 && i.page < i.pages - 1) ?? null);

  // Mantener pulsado (medio segundo) para quitar. Si el dedo se mueve, es desplazar la lista.
  let asking = $state<Item | null>(null);
  let hold: { timer: number; x: number; y: number; fired: boolean } | null = null;
  function down(e: PointerEvent, it: Item) {
    const h = { x: e.clientX, y: e.clientY, fired: false, timer: 0 };
    h.timer = window.setTimeout(() => {
      h.fired = true;
      navigator.vibrate?.(20);
      asking = it;
    }, 500);
    hold = h;
  }
  function move(e: PointerEvent) {
    if (hold && Math.hypot(e.clientX - hold.x, e.clientY - hold.y) > 12) cancelHold();
  }
  function cancelHold() {
    if (hold) clearTimeout(hold.timer);
  }
  function click(it: Item) {
    if (hold?.fired) return;
    onopen(it);
  }
  async function remove() {
    const it = asking;
    asking = null;
    if (!it) return;
    await library.remove(it.id);
    onchanged();
  }

  const count = (it: Item) => (it.kind === "album" ? t("photosN", { n: it.pages }) : t("pagesN", { n: it.pages }));
  const progress = (it: Item) => (it.pages > 1 ? (it.page + 1) / it.pages : 0);
</script>

<main>
  <header>
    <img class="logo" src={logo} alt="" />
    <h1>imgxplorer</h1>
    <button class="icon" onclick={oninfo} aria-label={t("info")} title={t("info")}>ⓘ</button>
  </header>

  <div class="actions">
    <button class="big" disabled={busy} onclick={() => onimport("images")} title={t("photosHint")}>
      <span class="emoji">📷</span>{t("photos")}
    </button>
    <button class="big" disabled={busy} onclick={() => onimport("documents")} title={t("filesHint")}>
      <span class="emoji">📂</span>{t("files")}
    </button>
  </div>

  {#if busy}
    <p class="busy">{t("importing")}</p>
  {/if}

  {#if items.length === 0 && !busy}
    <section class="empty">
      <h2>{t("emptyTitle")}</h2>
      <p>{t("emptyText")}</p>
    </section>
  {/if}

  {#if resume}
    <button class="resume" onclick={() => onopen(resume)}>
      {#if covers[resume.path]}<img src={covers[resume.path]} alt="" />{:else}<span class="ph"></span>{/if}
      <span class="text">
        <small>{t("continue")}</small>
        <strong>{resume.title}</strong>
        <span>{t("pageOf", { page: resume.page + 1, pages: resume.pages })}</span>
        <span class="bar"><span style:width="{progress(resume) * 100}%"></span></span>
      </span>
    </button>
  {/if}

  {#if items.length}
    <ul class="grid">
      {#each items as it (it.id)}
        <li>
          <button
            class="card"
            onpointerdown={(e) => down(e, it)}
            onpointermove={move}
            onpointerup={cancelHold}
            onpointercancel={cancelHold}
            onclick={() => click(it)}
            oncontextmenu={(e) => e.preventDefault()}
          >
            <span class="cover">
              {#if covers[it.path]}<img src={covers[it.path]} alt="" loading="lazy" />{/if}
              {#if it.kind !== "comic"}<span class="badge">{it.kind === "album" ? "📷" : "🎬"}</span>{/if}
            </span>
            <span class="title">{it.title}</span>
            <span class="meta">{it.opened ? t("pageOf", { page: it.page + 1, pages: it.pages }) : count(it)}</span>
            {#if it.opened}<span class="bar"><span style:width="{progress(it) * 100}%"></span></span>{/if}
          </button>
        </li>
      {/each}
    </ul>
    <p class="hint">{t("holdHint")}</p>
  {/if}
</main>

{#if asking}
  <div class="sheet-bg" role="presentation" onclick={() => (asking = null)}></div>
  <div class="sheet" role="dialog" aria-modal="true">
    <h3>{t("removeAsk", { title: asking.title })}</h3>
    <p>{t("removeText")}</p>
    <div class="row">
      <button onclick={() => (asking = null)}>{t("cancel")}</button>
      <button class="danger" onclick={remove}>{t("remove")}</button>
    </div>
  </div>
{/if}

<style>
  main { height: 100%; overflow-y: auto; padding: 12px 14px 24px; }
  header { display: flex; align-items: center; gap: 10px; padding: 4px 2px 10px; }
  .logo { width: 36px; height: 36px; }
  h1 { flex: 1; margin: 0; font-size: 22px; }
  .icon { background: none; border: 0; color: var(--muted); font-size: 22px; padding: 6px 8px; }
  .actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .big {
    display: flex; align-items: center; justify-content: center; gap: 8px;
    padding: 16px 10px; border: 0; border-radius: 14px;
    background: var(--panel); color: var(--text); font: inherit; font-size: 17px; font-weight: 600;
  }
  .big:active { background: #2a313b; }
  .big:disabled { opacity: 0.5; }
  .emoji { font-size: 22px; }
  .busy { color: var(--accent); text-align: center; margin: 14px 0 0; }
  .empty { margin: 40px 10px; text-align: center; color: var(--muted); }
  .empty h2 { color: var(--text); font-size: 18px; }
  .resume {
    display: flex; gap: 12px; width: 100%; margin-top: 16px; padding: 10px; text-align: left;
    background: linear-gradient(135deg, #1f3354, #1e232b); border: 0; border-radius: 14px; color: var(--text); font: inherit;
  }
  .resume img, .resume .ph { width: 64px; height: 92px; object-fit: cover; border-radius: 8px; background: #2a313b; flex: none; }
  .resume .text { display: flex; flex-direction: column; gap: 3px; min-width: 0; flex: 1; justify-content: center; }
  .resume small { color: var(--accent); font-weight: 600; }
  .resume strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .resume span { color: var(--muted); font-size: 14px; }
  .grid { list-style: none; margin: 18px 0 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(104px, 1fr)); gap: 14px 10px; }
  .card {
    display: flex; flex-direction: column; gap: 4px; width: 100%; padding: 0; text-align: left;
    background: none; border: 0; color: var(--text); font: inherit;
    -webkit-touch-callout: none;
  }
  .cover { position: relative; display: block; aspect-ratio: 2 / 3; border-radius: 8px; overflow: hidden; background: var(--panel); }
  .cover img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .badge { position: absolute; right: 4px; bottom: 4px; font-size: 14px; background: rgb(0 0 0 / 0.55); border-radius: 6px; padding: 1px 4px; }
  .title { font-size: 13px; line-height: 1.25; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .meta { font-size: 12px; color: var(--muted); }
  .bar { display: block; height: 3px; background: #2a313b; border-radius: 2px; overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent); }
  .resume .bar { margin-top: 4px; }
  .hint { color: var(--muted); font-size: 12px; text-align: center; margin-top: 18px; }
  .sheet-bg { position: fixed; inset: 0; background: rgb(0 0 0 / 0.5); }
  .sheet {
    position: fixed; left: 0; right: 0; bottom: 0; padding: 18px 18px calc(18px + env(safe-area-inset-bottom));
    background: var(--panel); border-radius: 18px 18px 0 0;
  }
  .sheet h3 { margin: 0 0 6px; font-size: 17px; }
  .sheet p { margin: 0 0 16px; color: var(--muted); }
  .sheet .row { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .sheet button { padding: 13px; border: 0; border-radius: 12px; font: inherit; font-weight: 600; background: #2a313b; color: var(--text); }
  .sheet .danger { background: var(--bad); color: white; }
</style>
