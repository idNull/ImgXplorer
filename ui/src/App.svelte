<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";
  import Viewer from "./lib/viewer/Viewer.svelte";
  import type { ViewerStatus } from "./lib/viewer/types";
  import { coreVersion } from "./lib/core";
  import { t } from "./lib/i18n";
  import { log } from "./lib/log";

  interface DiagnosticsInfo {
    logDir: string | null;
    previousCrash: { pid: number; started: number } | null;
    os: string;
    sessionType: string | null;
  }

  interface SourceInfo {
    generation: number;
    title: string;
    entries: { name: string; size: number }[];
  }

  let source = $state<SourceInfo | null>(null);
  let index = $state(0);
  let status = $state<ViewerStatus | null>(null);
  let showDiag = $state(true); // fase 0: visible por defecto para las pruebas de gestos
  let openError = $state<string | null>(null);
  let viewer: Viewer;
  let diag = $state<DiagnosticsInfo | null>(null);

  const url = (i: number) => (source ? convertFileSrc(`${source.generation}_${i}`, "imgx") : null);
  const current = $derived(source && source.entries.length ? url(index) : null);
  const neighbours = $derived(
    source ? [index + 1, index - 1, index + 2].filter((i) => i >= 0 && i < source!.entries.length).map((i) => url(i)!) : [],
  );

  async function openPath(path: string) {
    try {
      source = await invoke<SourceInfo>("open_source", { path });
      index = 0;
      openError = null;
      getCurrentWindow().setTitle(`${source.title} — imgxplorer`).catch((e) => log.warn("título:", e));
    } catch (e) {
      openError = String(e);
      log.warn("abrir falló:", path, e);
    }
  }

  async function pickFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") openPath(dir);
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
      case "o": if (e.ctrlKey || e.metaKey) pickFolder(); else return; break;
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
    <button onclick={pickFolder}>{t("open")}</button>
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
    <button onclick={toggleFullscreen} title="{t('fullscreen')} (F11)">⛶</button>
  </header>

  {#if diag?.previousCrash}
    <div class="crash" role="alert">
      <span>
        ⚠ {t("crashNotice")}
        {#if diag.logDir}<code>{diag.logDir}/imgxplorer.log</code>{/if}
        {#if diag.os === "linux"} · <code>coredumpctl info {diag.previousCrash.pid}</code>{/if}
      </span>
      <button onclick={dismissCrash}>{t("dismiss")}</button>
    </div>
  {/if}

  <section class="stage">
    <Viewer bind:this={viewer} src={current} prefetch={neighbours} onstatus={(s) => (status = s)} />
    {#if !source}
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
        <div>core {coreVersion()} · {diag?.os ?? ""} {diag?.sessionType ?? ""}</div>
        {#if diag?.logDir}<div class="name">log: {diag.logDir}</div>{/if}
        {#if source?.entries[index]}<div class="name">{source.entries[index].name}</div>{/if}
      </aside>
    {/if}
  </section>
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
  .diag .name { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
