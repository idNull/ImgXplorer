<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";
  import Viewer from "./lib/viewer/Viewer.svelte";
  import Thumbs from "./lib/Thumbs.svelte";
  import ActionPanel from "./lib/action/ActionPanel.svelte";
  import PlayerBar from "./lib/action/PlayerBar.svelte";
  import { ProjectStore, type LoadedProject } from "./lib/action/project.svelte";
  import { FALLBACK, needsPanels } from "./lib/action/catalog";
  import type { PlayerInfo } from "./lib/action/player";
  import type { ViewerStatus } from "./lib/viewer/types";
  import type { PictureRef } from "./lib/viewer/picture";
  import { coreVersion, wasm, type Size } from "./lib/core";
  import { t } from "./lib/i18n";
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
  let showDiag = $state(true); // visible por defecto mientras probamos
  let showThumbs = $state(true);
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
      index = Math.min(s.start, Math.max(0, s.entries.length - 1));
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

  // Cambiar la plantilla o sus parámetros mientras se reproduce se aplica en el acto.
  const playing = $derived(!!playerInfo);
  $effect(() => {
    const json = playing ? JSON.stringify(store.data) : null;
    if (json) viewer?.playback()?.refresh();
  });

  /** Teclas durante la reproducción. Devuelve `true` si la tecla era suya. */
  function playerKey(e: KeyboardEvent): boolean {
    const p = viewer?.playback();
    if (!p) return false;
    switch (e.key) {
      case " ": p.toggle(); break;
      case "ArrowRight": e.shiftKey ? p.seek(1) : p.mark(1); break;
      case "ArrowLeft": e.shiftKey ? p.seek(-1) : p.mark(-1); break;
      case "PageDown": case "ArrowDown": p.goTo(p.index + 1); break;
      case "PageUp": case "ArrowUp": p.goTo(p.index - 1); break;
      case "Home": p.goTo(0); break;
      case "End": p.goTo(source!.entries.length - 1); break;
      case "]": p.changeSpeed(1); break;
      case "[": p.changeSpeed(-1); break;
      case "Escape": case "p": case "P": viewer.stop(); break;
      default: return false;
    }
    return true;
  }

  async function toggleFullscreen() {
    const w = getCurrentWindow();
    w.setFullscreen(!(await w.isFullscreen()));
  }

  function onKey(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
    if (playerKey(e)) return e.preventDefault();
    const zoomed = viewer?.isZoomedIn();
    const step = e.shiftKey ? 240 : 80;
    switch (e.key) {
      case "ArrowRight": zoomed ? viewer.pan(-step, 0) : go(index + 1); break;
      case "ArrowLeft": zoomed ? viewer.pan(step, 0) : go(index - 1); break;
      case "ArrowDown": zoomed ? viewer.pan(0, -step) : go(index + 1); break;
      case "ArrowUp": zoomed ? viewer.pan(0, step) : go(index - 1); break;
      case "PageDown": case " ": go(index + 1); break;
      case "PageUp": case "Backspace": go(index - 1); break;
      case "Home": go(0); break;
      case "End": go(Infinity); break;
      case "+": case "=": viewer.zoomBy(1.25); break;
      case "-": case "_": viewer.zoomBy(0.8); break;
      case "0": case "f": case "F": viewer.fit(); break;
      case "1": viewer.actualSize(); break;
      case "r": viewer.rotate(90); break;
      case "R": viewer.rotate(-90); break;
      case "F11": toggleFullscreen(); break;
      case "d": case "D": showDiag = !showDiag; break;
      case "t": case "T": showThumbs = !showThumbs; break;
      case "a": case "A": actionOpen = !actionOpen; break;
      case "p": case "P": startPlay(); break;
      case "o": case "O":
        if (!(e.ctrlKey || e.metaKey)) return;
        e.shiftKey ? pickFolder() : pickFile();
        break;
      default: return;
    }
    e.preventDefault();
  }

  function dismissCrash() {
    if (diag) diag.previousCrash = null;
    invoke("dismiss_crash_notice");
  }

  onMount(() => {
    invoke<DiagnosticsInfo>("diagnostics_info").then((d) => (diag = d));
    invoke<string | null>("initial_path").then((p) => {
      if (p) openPath(p);
    });
    const flush = () => void store.flush();
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
    <button onclick={pickFile} title="Ctrl+O">{t("openFile")}</button>
    <button onclick={pickFolder} title="Ctrl+Shift+O">{t("open")}</button>
    <span class="title">{source?.title ?? ""}</span>
    <span class="spacer"></span>
    <button onclick={() => go(index - 1)} disabled={!source || index === 0} title="{t('prev')} (PgUp)">◀</button>
    <span class="counter">{source && source.entries.length ? `${index + 1} / ${source.entries.length}` : "–"}</span>
    <button onclick={() => go(index + 1)} disabled={!source || index >= source.entries.length - 1} title="{t('next')} (PgDn)">▶</button>
    <button onclick={() => viewer.zoomBy(0.8)} title="−">−</button>
    <span class="zoom">{status?.image ? `${status.zoomPercent}%` : ""}</span>
    <button onclick={() => viewer.zoomBy(1.25)} title="+">+</button>
    <button onclick={() => viewer.fit()} title="{t('fit')} (F)">{t("fit")}</button>
    <button onclick={() => viewer.actualSize()} title="{t('actual')} (1)">1:1</button>
    <button onclick={() => viewer.rotate(90)} title="{t('rotate')} (R)">⟳</button>
    <button onclick={() => (showThumbs = !showThumbs)} class:on={showThumbs} title="{t('thumbs')} (T)">▦</button>
    <button onclick={() => (actionOpen = !actionOpen)} class:on={actionOpen} disabled={!source} title="{t('action')} (A)">🎬 {t("action")}</button>
    <button onclick={toggleFullscreen} title="{t('fullscreen')} (F11)">⛶</button>
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
    />
    {#if playerInfo && source}
      <PlayerBar info={playerInfo} count={source.entries.length} player={() => viewer.playback()} onstop={() => viewer.stop()} />
    {/if}
    {#if opening}
      <div class="hint">{t("loading")}</div>
    {:else if !source || openError}
      <div class="hint">{openError ?? t("openHint")}</div>
    {:else if !source.entries.length}
      <div class="hint">{t("noImages")}</div>
    {:else if status?.error}
      <div class="hint">{status.error}</div>
    {/if}
    {#if showDiag}
      <aside class="diag">
        <b>{t("diag")}</b> (D)
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

  {#if showThumbs && source && source.entries.length > 1}
    <Thumbs
      count={source.entries.length}
      current={index}
      url={thumbUrl}
      name={(i) => source!.entries[i].name}
      onpick={go}
    />
  {/if}
</main>

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
  .diag .name { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
