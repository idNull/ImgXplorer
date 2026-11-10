<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";
  import Viewer from "./lib/viewer/Viewer.svelte";
  import Thumbs from "./lib/Thumbs.svelte";
  import type { ViewerStatus } from "./lib/viewer/types";
  import type { PictureRef } from "./lib/viewer/picture";
  import { coreVersion } from "./lib/core";
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
    try {
      const s = await invoke<SourceInfo>("open_source", { path });
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
    index = Math.max(0, Math.min(source.entries.length - 1, i));
  }

  async function toggleFullscreen() {
    const w = getCurrentWindow();
    w.setFullscreen(!(await w.isFullscreen()));
  }

  function onKey(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement) return;
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
    const un = getCurrentWebview().onDragDropEvent((ev) => {
      if (ev.payload.type === "drop" && ev.payload.paths.length) openPath(ev.payload.paths[0]);
    });
    return () => {
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

  <section class="stage">
    <Viewer bind:this={viewer} item={current} prefetch={neighbours} onstatus={(s) => (status = s)} />
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
        {#if source?.entries[index]}<div class="name">{source.entries[index].name}</div>{/if}
      </aside>
    {/if}
  </section>

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
  .stage { position: relative; flex: 1; min-height: 0; }
  .hint {
    position: absolute; inset: 0; display: grid; place-items: center; pointer-events: none;
    color: var(--muted); font-size: 16px;
  }
  .diag {
    position: absolute; right: 10px; bottom: 10px; padding: 8px 10px; border-radius: 8px;
    background: rgb(0 0 0 / 0.7); border: 1px solid var(--border); font-size: 12px; line-height: 1.6;
    pointer-events: none; max-width: 360px;
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
