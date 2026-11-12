<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";
  import Viewer from "./lib/viewer/Viewer.svelte";
  import Thumbs from "./lib/Thumbs.svelte";
  import Settings from "./lib/Settings.svelte";
  import Recents from "./lib/Recents.svelte";
  import { prefs } from "./lib/settings.svelte";
  import { comboOf, display, keymap, keysFor, type ViewerAction } from "./lib/keys";
  import ActionPanel from "./lib/action/ActionPanel.svelte";
  import PlayerBar from "./lib/action/PlayerBar.svelte";
  import { ProjectStore, type LoadedProject } from "./lib/action/project.svelte";
  import { FALLBACK, needsPanels } from "./lib/action/catalog";
  import type { PlayerInfo } from "./lib/action/player";
  import type { ViewerStatus } from "./lib/viewer/types";
  import type { PictureRef } from "./lib/viewer/picture";
  import { coreVersion, wasm, type Size } from "./lib/core";
  import { t } from "./lib/i18n.svelte";
  import { log } from "./lib/log";

  interface DiagnosticsInfo {
    logDir: string | null;
    previousCrash: { pid: number; started: number; reason: string } | null;
    os: string;
    sessionType: string | null;
  }

  interface SourceInfo {
    generation: number;
    title: string;
    path: string;
    kind: "folder" | "archive";
    start: number;
    picked: boolean;
    entries: { name: string; size: number; decoded: boolean }[];
    fingerprint: string;
    project: LoadedProject;
  }

  const ARCHIVES = ["zip", "cbz", "7z", "cb7", "rar", "cbr"];
  const IMAGES = ["jpg", "jpeg", "jfif", "png", "apng", "gif", "webp", "bmp", "avif", "svg", "svgz", "heic", "heif", "hif", "jxl", "psd",
    "tif", "tiff", "tga", "ico", "qoi", "exr", "hdr", "pnm", "pbm", "pgm", "ppm", "pam", "dds",
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "raf", "pef", "srw", "x3f", "3fr", "iiq", "mos", "erf", "kdc", "dcr", "mrw", "rwl", "raw"];

  let source = $state<SourceInfo | null>(null);
  let index = $state(0);
  let status = $state<ViewerStatus | null>(null);
  let settingsOpen = $state(false);
  let recentsOpen = $state(false);
  let opening = $state(false);
  let openError = $state<string | null>(null);
  let viewer: Viewer;
  let diag = $state<DiagnosticsInfo | null>(null);
  let actionOpen = $state(false);
  let playerInfo = $state<PlayerInfo | null>(null);
  let overlayPanels = $state<Float64Array | null>(null);
  let detectedCount = $state<number | null>(null);
  const store = new ProjectStore();
  // Viñetas detectadas por imagen (la detección es en Rust; se pide una vez por imagen).
  const panelCache = new Map<number, Promise<number[]>>();

  // `imgx://localhost/` (o `http://imgx.localhost/` según plataforma). Las rutas se añaden sin
  // codificar: convertFileSrc codificaría las barras de `t/…` y `d/…`.
  const base = convertFileSrc("", "imgx");
  const ref = (s: SourceInfo, i: number): PictureRef => ({
    url: `${base}${s.generation}_${i}`,
    fallback: `${base}d/${s.generation}_${i}`,
    decoded: s.entries[i].decoded,
  });
  const thumbUrl = (i: number) => `${base}t/${source!.generation}_${i}`;
  const current = $derived(source && source.entries.length ? ref(source, index) : null);
  const neighbours = $derived(
    source ? [index + 1, index - 1, index + 2].filter((i) => i >= 0 && i < source!.entries.length).map((i) => ref(source!, i)) : [],
  );

  async function openPath(path: string) {
    opening = true;
    viewer?.stop();
    try {
      await store.flush(); // lo pendiente de la fuente anterior, antes de cambiar
      const s = await invoke<SourceInfo>("open_source", { path });
      panelCache.clear();
      await store.load(s.generation, { kind: s.kind, title: s.title, fingerprint: s.fingerprint }, s.project);
      source = s;
      // Volver a la imagen en la que se dejó (salvo que se haya abierto una imagen concreta).
      const rec = prefs.find(s.path);
      let start = s.start;
      if (rec && !s.picked && prefs.settings.resume) {
        const byName = s.entries.findIndex((e) => e.name === rec.entry);
        start = byName >= 0 ? byName : rec.index;
      }
      index = Math.max(0, Math.min(start, s.entries.length - 1));
      openError = null;
      getCurrentWindow().setTitle(`${s.title} — imgxplorer`).catch((e) => log.warn("título:", e));
    } catch (e) {
      openError = String(e);
      log.warn("abrir falló:", path, e);
    } finally {
      opening = false;
    }
  }

  async function pickFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") openPath(dir);
  }

  async function pickFile() {
    const file = await open({
      multiple: false,
      filters: [
        { name: t("filterAll"), extensions: [...ARCHIVES, ...IMAGES] },
        { name: t("filterArchives"), extensions: ARCHIVES },
        { name: t("filterImages"), extensions: IMAGES },
      ],
    });
    if (typeof file === "string") openPath(file);
  }

  function go(i: number) {
    if (!source || !source.entries.length) return;
    const to = Math.max(0, Math.min(source.entries.length - 1, i));
    const p = viewer?.playback();
    if (p) p.goTo(to);
    else index = to;
  }

  function panelsFor(i: number): Promise<number[]> {
    const s = source!;
    let p = panelCache.get(i);
    if (!p) {
      p = invoke<number[][]>("detect_panels", { generation: s.generation, index: i })
        .then((r) => r.flat())
        .catch((e) => {
          log.warn(`detección de viñetas falló en ${s.entries[i]?.name}:`, e);
          return [];
        });
      panelCache.set(i, p);
    }
    return p;
  }

  async function timelineFor(i: number, size: Size, aspect: number): Promise<wasm.WasmTimeline> {
    const entry = source!.entries[i].name;
    const it = store.item(entry);
    const tpl = store.templateFor(entry);
    if (!tpl && it?.timeline?.camera?.length) return new wasm.WasmTimeline(JSON.stringify($state.snapshot(it.timeline)));
    const use = tpl ?? FALLBACK;
    const panels = needsPanels(use) ? await panelsFor(i) : [];
    return wasm.WasmTimeline.from_template(JSON.stringify($state.snapshot(use)), size.w, size.h, aspect, new Float64Array(panels));
  }

  function startPlay() {
    if (!source || !source.entries.length) return;
    viewer.play({
      start: index,
      count: source.entries.length,
      timeline: timelineFor,
      transitionOut: (i) => {
        const entry = source!.entries[i].name;
        const tr = $state.snapshot(store.transitionFor(entry));
        const tpl = store.templateFor(entry);
        return { json: JSON.stringify(tr), tpl: tpl ? JSON.stringify($state.snapshot(tpl)) : "", duration: tr.duration, cut: tr.type === "cut" };
      },
      holdBetween: () => store.data?.defaults.holdBetween ?? 0.3,
      onIndex: (i) => (index = i),
    });
  }

  function togglePlay() {
    if (playerInfo) viewer.stop();
    else startPlay();
  }

  // Viñetas sobre la imagen actual mientras se configura la plantilla de cómic.
  $effect(() => {
    const s = source;
    const i = index;
    const entry = s?.entries[i]?.name;
    const tpl = actionOpen && entry ? store.templateFor(entry) : null;
    const json = tpl?.id === "comic" ? JSON.stringify(tpl) : null; // lee todos los parámetros: se recalcula al cambiarlos
    if (!json) {
      overlayPanels = null;
      return;
    }
    let cancelled = false;
    const auto = needsPanels(tpl);
    if (auto) detectedCount = null;
    (auto ? panelsFor(i) : Promise.resolve([])).then((det) => {
      if (cancelled) return;
      if (auto) detectedCount = det.length / 4;
      overlayPanels = wasm.comic_panels(json, 1, 1, 1, new Float64Array(det));
    });
    return () => {
      cancelled = true;
    };
  });

  // Recientes: la fuente abierta y la imagen en la que está.
  $effect(() => {
    const s = source;
    const i = index;
    if (!s || !s.entries.length || !prefs.loaded) return;
    const r = { path: s.path, title: s.title, kind: s.kind, entry: s.entries[i]?.name ?? "", index: i, count: s.entries.length };
    untrack(() => prefs.remember(r));
  });

  // Cambiar la plantilla o sus parámetros mientras se reproduce se aplica en el acto.
  const playing = $derived(!!playerInfo);
  $effect(() => {
    const json = playing ? JSON.stringify(store.data) : null;
    if (json) viewer?.playback()?.refresh();
  });

  const viewerKeys = $derived(keymap(prefs.settings.keys, "viewer"));
  const playerKeys = $derived(keymap(prefs.settings.keys, "player"));
  /** Primera tecla de una acción del visor, para los títulos de los botones. */
  const kt = (a: ViewerAction) => {
    const k = keysFor(prefs.settings.keys, "viewer", a)[0];
    return k ? ` (${display(k)})` : "";
  };

  /** Teclas durante la reproducción. Devuelve `true` si la tecla era suya. */
  function playerKey(combo: string): boolean {
    const p = viewer?.playback();
    if (!p) return false;
    const action = playerKeys.get(combo);
    switch (action) {
      case "toggle": p.toggle(); break;
      case "markNext": p.mark(1); break;
      case "markPrev": p.mark(-1); break;
      case "seekFwd": p.seek(1); break;
      case "seekBack": p.seek(-1); break;
      case "pageNext": p.goTo(p.index + 1); break;
      case "pagePrev": p.goTo(p.index - 1); break;
      case "first": p.goTo(0); break;
      case "last": p.goTo(source!.entries.length - 1); break;
      case "faster": p.changeSpeed(1); break;
      case "slower": p.changeSpeed(-1); break;
      case "stop": viewer.stop(); break;
      default: return false;
    }
    return true;
  }

  const zoomIn = () => viewer.zoomBy(prefs.settings.zoomStep);
  const zoomOut = () => viewer.zoomBy(1 / prefs.settings.zoomStep);
  function toggleThumbs() {
    prefs.settings.showThumbs = !prefs.settings.showThumbs;
    prefs.saveSettings();
  }

  async function toggleFullscreen() {
    const w = getCurrentWindow();
    w.setFullscreen(!(await w.isFullscreen()));
  }

  function onKey(e: KeyboardEvent) {
    if (settingsOpen) return; // la ventana de ajustes gestiona sus teclas
    const tgt = e.target;
    if (tgt instanceof HTMLInputElement || tgt instanceof HTMLSelectElement || tgt instanceof HTMLTextAreaElement) return;
    if (recentsOpen && e.key === "Escape") {
      recentsOpen = false;
      return e.preventDefault();
    }
    const combo = comboOf(e);
    if (!combo) return;
    if (playerKey(combo)) return e.preventDefault();
    const action = viewerKeys.get(combo);
    if (!action) return;
    const zoomed = viewer?.isZoomedIn();
    const step = e.shiftKey ? 240 : 80;
    switch (action) {
      case "right": zoomed ? viewer.pan(-step, 0) : go(index + 1); break;
      case "left": zoomed ? viewer.pan(step, 0) : go(index - 1); break;
      case "down": zoomed ? viewer.pan(0, -step) : go(index + 1); break;
      case "up": zoomed ? viewer.pan(0, step) : go(index - 1); break;
      case "next": go(index + 1); break;
      case "prev": go(index - 1); break;
      case "first": go(0); break;
      case "last": go(Infinity); break;
      case "zoomIn": zoomIn(); break;
      case "zoomOut": zoomOut(); break;
      case "fit": viewer.fit(); break;
      case "actual": viewer.actualSize(); break;
      case "rotateCw": viewer.rotate(90); break;
      case "rotateCcw": viewer.rotate(-90); break;
      case "fullscreen": toggleFullscreen(); break;
      case "diag": prefs.settings.showDiag = !prefs.settings.showDiag; prefs.saveSettings(); break;
      case "thumbs": toggleThumbs(); break;
      case "action": actionOpen = !actionOpen; break;
      case "play": startPlay(); break;
      case "openFile": pickFile(); break;
      case "openFolder": pickFolder(); break;
      case "recents": recentsOpen = !recentsOpen; break;
      case "settings": settingsOpen = true; break;
    }
    e.preventDefault();
  }

  function openRecent(path: string) {
    recentsOpen = false;
    openPath(path);
  }

  function dismissCrash() {
    if (diag) diag.previousCrash = null;
    invoke("dismiss_crash_notice");
  }

  onMount(() => {
    invoke<DiagnosticsInfo>("diagnostics_info").then((d) => (diag = d));
    // Primero los ajustes y recientes: abrir la ruta inicial necesita saber dónde se quedó.
    prefs.load().then(() => invoke<string | null>("initial_path")).then((p) => {
      if (p) openPath(p);
    });
    const flush = () => {
      void store.flush();
      prefs.flush();
    };
    window.addEventListener("beforeunload", flush);
    const un = getCurrentWebview().onDragDropEvent((ev) => {
      if (ev.payload.type === "drop" && ev.payload.paths.length) openPath(ev.payload.paths[0]);
    });
    return () => {
      window.removeEventListener("beforeunload", flush);
      un.then((f) => f());
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<main>
  <header>
    <button onclick={pickFile} title="{t('openFile')}{kt('openFile')}">{t("openFile")}</button>
    <button onclick={pickFolder} title="{t('open')}{kt('openFolder')}">{t("open")}</button>
    <div class="menu">
      <button onclick={() => (recentsOpen = !recentsOpen)} class:on={recentsOpen} title="{t('recents')}{kt('recents')}">🕘 ▾</button>
      {#if recentsOpen}
        <div class="backdrop" role="presentation" onclick={() => (recentsOpen = false)}></div>
        <div class="popup">
          <Recents onopen={openRecent} current={source?.path} />
        </div>
      {/if}
    </div>
    <span class="title">{source?.title ?? ""}</span>
    <span class="spacer"></span>
    <button onclick={() => go(index - 1)} disabled={!source || index === 0} title="{t('prev')}{kt('prev')}">◀</button>
    <span class="counter">{source && source.entries.length ? `${index + 1} / ${source.entries.length}` : "–"}</span>
    <button onclick={() => go(index + 1)} disabled={!source || index >= source.entries.length - 1} title="{t('next')}{kt('next')}">▶</button>
    <button onclick={zoomOut} title={kt("zoomOut").trim()}>−</button>
    <span class="zoom">{status?.image ? `${status.zoomPercent}%` : ""}</span>
    <button onclick={zoomIn} title={kt("zoomIn").trim()}>+</button>
    <button onclick={() => viewer.fit()} title="{t('fit')}{kt('fit')}">{t("fit")}</button>
    <button onclick={() => viewer.actualSize()} title="{t('actual')}{kt('actual')}">1:1</button>
    <button onclick={() => viewer.rotate(90)} title="{t('rotate')}{kt('rotateCw')}">⟳</button>
    <button onclick={toggleThumbs} class:on={prefs.settings.showThumbs} title="{t('thumbs')}{kt('thumbs')}">▦</button>
    <button onclick={() => (actionOpen = !actionOpen)} class:on={actionOpen} disabled={!source} title="{t('action')}{kt('action')}">🎬 {t("action")}</button>
    <button onclick={toggleFullscreen} title="{t('fullscreen')}{kt('fullscreen')}">⛶</button>
    <button onclick={() => (settingsOpen = true)} title="{t('settings')}{kt('settings')}">⚙</button>
  </header>

  {#if diag?.previousCrash}
    <div class="crash" role="alert">
      <span>
        ⚠ {t("crashNotice", { reason: diag.previousCrash.reason })}
        {#if diag.logDir}<code>{diag.logDir}/imgxplorer.log</code>{/if}
        {#if diag.os === "linux"} · <code>coredumpctl info {diag.previousCrash.pid}</code>{/if}
      </span>
      <button onclick={dismissCrash}>{t("dismiss")}</button>
    </div>
  {/if}

  <div class="body">
  <section class="stage">
    <Viewer
      bind:this={viewer}
      item={current}
      prefetch={neighbours}
      refAt={(i) => ref(source!, i)}
      panels={overlayPanels}
      onstatus={(s) => (status = s)}
      onplayer={(p) => (playerInfo = p)}
      background={prefs.settings.background}
      wheelSpeed={prefs.settings.wheelSpeed}
    />
    {#if playerInfo && source}
      <PlayerBar info={playerInfo} count={source.entries.length} player={() => viewer.playback()} onstop={() => viewer.stop()} />
    {/if}
    {#if opening}
      <div class="hint">{t("loading")}</div>
    {:else if openError}
      <div class="hint">{openError}</div>
    {:else if !source}
      <div class="start">
        <p>{t("openHint")}</p>
        {#if prefs.recents.length}
          <h4>{t("recents")}</h4>
          <div class="start-list"><Recents onopen={openRecent} /></div>
        {/if}
      </div>
    {:else if !source.entries.length}
      <div class="hint">{t("noImages")}</div>
    {:else if status?.error}
      <div class="hint">{status.error}</div>
    {/if}
    {#if prefs.settings.showDiag}
      <aside class="diag">
        <b>{t("diag")}</b>{kt("diag")}
        <div>{t("lastInput")}: <code>{status?.lastInput ?? "–"}</code></div>
        <div>FPS: {status?.fps ?? 0} · max tex: {status?.maxTexture ?? 0}</div>
        <div>img: {status?.image ? `${status.image.w}×${status.image.h}` : "–"} · dpr {window.devicePixelRatio}</div>
        <div>
          {source?.entries[index]?.decoded ? "decodificada en Rust" : "nativa (webview)"} · {status?.loadMs ?? 0} ms
          {#if (status?.tiles ?? 0) > 1}· {status?.tiles} teselas{/if}
        </div>
        <div>core {coreVersion()} · {diag?.os ?? ""} {diag?.sessionType ?? ""}</div>
        {#if diag?.logDir}<div class="name">log: {diag.logDir}</div>{/if}
        {#if playerInfo}<div>player: {playerInfo.state} · t {playerInfo.t.toFixed(2)} / {playerInfo.duration.toFixed(2)}</div>{/if}
        {#if source?.entries[index]}<div class="name">{source.entries[index].name}</div>{/if}
      </aside>
    {/if}
  </section>
  {#if actionOpen && source}
    <ActionPanel
      {store}
      entry={source.entries[index]?.name ?? null}
      size={status?.image ? [status.image.w, status.image.h] : null}
      detected={detectedCount}
      playing={!!playerInfo}
      onplay={togglePlay}
      onclose={() => (actionOpen = false)}
    />
  {/if}
  </div>

  {#if prefs.settings.showThumbs && source && source.entries.length > 1}
    <Thumbs
      count={source.entries.length}
      current={index}
      url={thumbUrl}
      name={(i) => source!.entries[i].name}
      onpick={go}
      size={prefs.settings.thumbSize}
    />
  {/if}
</main>

{#if settingsOpen}
  <Settings onclose={() => (settingsOpen = false)} />
{/if}

<style>
  main { display: flex; flex-direction: column; height: 100%; }
  header {
    display: flex; align-items: center; gap: 6px; padding: 6px 10px;
    background: var(--panel); border-bottom: 1px solid var(--border); user-select: none;
  }
  .title { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .spacer { flex: 1; }
  .counter, .zoom { min-width: 64px; text-align: center; font-variant-numeric: tabular-nums; color: var(--muted); }
  .body { flex: 1; min-height: 0; display: flex; }
  .stage { position: relative; flex: 1; min-width: 0; }
  .hint {
    position: absolute; inset: 0; display: grid; place-items: center; pointer-events: none;
    color: var(--muted); font-size: 16px;
  }
  .diag {
    position: absolute; right: 10px; bottom: 10px; padding: 8px 10px; border-radius: 8px;
    background: rgb(0 0 0 / 0.7); border: 1px solid var(--border); font-size: 12px; line-height: 1.6;
    pointer-events: none; max-width: 360px; top: 10px; bottom: auto;
  }
  .diag code { color: var(--accent); }
  .crash {
    display: flex; align-items: center; gap: 12px; padding: 8px 12px;
    background: #3b1d1d; border-bottom: 1px solid #7f1d1d; color: #fecaca; font-size: 13px;
  }
  .crash span { flex: 1; }
  .crash code { color: #fff; user-select: all; }
  header button.on { border-color: var(--accent); }
  .menu { position: relative; }
  .menu .backdrop { position: fixed; inset: 0; z-index: 9; }
  .popup {
    position: absolute; top: calc(100% + 6px); left: 0; z-index: 10; width: 440px; max-height: 60vh; overflow-y: auto;
    padding: 6px; background: var(--panel); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 8px 30px rgb(0 0 0 / 0.45);
  }
  .start {
    position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 8px; padding: 20px; color: var(--muted); font-size: 16px;
  }
  .start p { margin: 0 0 12px; }
  .start h4 { margin: 0; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; }
  .start-list { width: min(560px, 100%); max-height: 50vh; overflow-y: auto; font-size: 14px; color: var(--text); }
  .diag .name { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
