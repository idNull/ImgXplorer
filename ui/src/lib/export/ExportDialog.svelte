<script lang="ts">
  // Ventana "Exportar vídeo": qué imágenes, tamaño, fotogramas por segundo, calidad y archivo.
  // Durante la exportación muestra el progreso y permite cancelar.
  import { onMount, untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { exportVideo, ExportCancelled, type ExportProgress, type ExportSpec } from "./exporter";
  import { saveFile, openFile, dirOf, joinPath } from "../files/dialog.svelte";
  import { prefs, videoPixels, QUALITY_CRF, VIDEO_SIZES, type VideoSize } from "../prefs.svelte";
  import type { PictureRef } from "../viewer/picture";
  import { t } from "../i18n.svelte";
  import type { MixLevels } from "../audio/director";
  import { log } from "../log";

  interface Props {
    /** Nombre de la fuente (para el archivo propuesto). */
    title: string;
    /** Carpeta donde proponer el vídeo. */
    dir: string;
    count: number;
    index: number;
    refAt: (i: number) => PictureRef;
    spec: ExportSpec;
    os: string;
    /** Mezcla del proyecto (volumen de voz, efectos y ambiente). */
    mix: MixLevels;
    onclose: () => void;
  }
  let { title, dir, count, index, refAt, spec, os, mix, onclose }: Props = $props();

  interface Ffmpeg {
    path: string;
    version: string;
    encoder: string;
  }

  let ff = $state<Ffmpeg | null>(null);
  let ffError = $state<string | null>(null);
  let checking = $state(true);
  let range = $state<"current" | "all" | "range">(untrack(() => (count > 1 ? "all" : "current")));
  let from = $state(1);
  let to = $state(untrack(() => count));
  let path = $state("");
  let pathTouched = false;
  let exists = $state(false);
  let phase = $state<"setup" | "running" | "done" | "error">("setup");
  let progress = $state<ExportProgress>({ frames: 0, index: 0, fraction: 0 });
  let startedAt = 0;
  let now = $state(0);
  let result = $state("");
  let abort: AbortController | null = null;

  const px = $derived(videoPixels(prefs.settings.video));
  const span = $derived.by(() => {
    if (range === "current") return { a: index, b: index };
    if (range === "all") return { a: 0, b: count - 1 };
    const a = Math.min(Math.max(1, Math.round(from)), count) - 1;
    const b = Math.min(Math.max(a + 1, Math.round(to)), count) - 1;
    return { a, b };
  });
  const ready = $derived(phase === "setup" && !!ff?.encoder && !!path.trim());
  /** Alguna imagen del tramo tiene sonido. */
  const hasSound = $derived.by(() => {
    for (let i = span.a; i <= span.b; i++) {
      const a = spec.audio?.(i);
      if (a?.voice.length || a?.sfx.length || spec.ambient?.(i).length) return true;
    }
    return false;
  });
  let withSound = $state(true);

  const save = () => prefs.saveSettings();
  const stem = (s: string) => s.replace(/\.[^./]+$/, "") || "video";

  // Archivo propuesto: junto a la fuente, con su nombre (y la página si es solo una).
  $effect(() => {
    const suffix = range === "current" && count > 1 ? `-${String(index + 1).padStart(String(count).length, "0")}` : "";
    if (!pathTouched) path = joinPath(dir, `${stem(title)}${suffix}.mp4`);
  });

  $effect(() => {
    const p = path;
    if (!p) return void (exists = false);
    invoke<boolean[]>("paths_exist", { paths: [p] }).then(([e]) => {
      if (p === path) exists = e;
    });
  });

  async function check() {
    checking = true;
    try {
      ff = await invoke<Ffmpeg>("ffmpeg_info", { path: prefs.settings.video.ffmpeg || null });
      ffError = ff.encoder ? null : t("ffmpegNoH264");
    } catch (e) {
      ff = null;
      ffError = String(e);
    } finally {
      checking = false;
    }
  }

  async function pickFfmpeg() {
    const p = await openFile({ title: "ffmpeg", dir: "/usr/bin" });
    if (!p) return;
    prefs.settings.video.ffmpeg = p;
    save();
    check();
  }

  async function pickPath() {
    const p = await saveFile({ title: t("exportVideo"), dir: dirOf(path), defaultName: path.slice(path.lastIndexOf("/") + 1), filters: [{ name: "MP4", extensions: ["mp4"] }] });
    if (!p) return;
    path = p;
    pathTouched = true;
  }

  async function start() {
    if (!ready) return;
    abort = new AbortController();
    phase = "running";
    progress = { frames: 0, index: span.a, fraction: 0 };
    startedAt = performance.now();
    now = startedAt;
    const tick = setInterval(() => (now = performance.now()), 500);
    try {
      result = await exportVideo(
        {
          path,
          width: px.w,
          height: px.h,
          fps: prefs.settings.video.fps,
          crf: QUALITY_CRF[prefs.settings.video.quality],
          ffmpeg: prefs.settings.video.ffmpeg,
          background: prefs.settings.background,
          from: span.a,
          to: span.b,
          sound: hasSound && withSound ? $state.snapshot(mix) : null,
        },
        spec,
        refAt,
        (p) => (progress = p),
        abort.signal,
      );
      phase = "done";
    } catch (e) {
      if (e instanceof ExportCancelled) {
        phase = "setup";
      } else {
        log.warn("exportar vídeo:", e);
        result = String(e);
        phase = "error";
      }
    } finally {
      clearInterval(tick);
      abort = null;
    }
  }

  function close() {
    if (phase === "running") return abort?.abort();
    onclose();
  }

  const clock = (ms: number) => {
    const s = Math.max(0, Math.round(ms / 1000));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  };
  const elapsed = $derived(now - startedAt);
  const remaining = $derived(progress.fraction > 0.02 ? (elapsed / progress.fraction) * (1 - progress.fraction) : null);
  const videoTime = $derived(progress.frames / prefs.settings.video.fps);

  function onKey(e: KeyboardEvent) {
    e.stopImmediatePropagation();
    const tgt = e.target as HTMLElement;
    if (e.key === "Escape") {
      e.preventDefault();
      if (phase !== "running") onclose();
    } else if (e.key === "Enter" && phase === "setup" && !(tgt instanceof HTMLSelectElement)) {
      e.preventDefault();
      start();
    }
  }

  const SIZES: VideoSize[] = [...(Object.keys(VIDEO_SIZES) as VideoSize[]), "custom"];
  const install = $derived(os === "macos" ? "brew install ffmpeg" : "sudo pacman -S ffmpeg   ·   sudo apt install ffmpeg   ·   sudo dnf install ffmpeg");

  onMount(() => {
    check();
    return () => abort?.abort();
  });
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && phase !== "running" && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("exportVideo")}>
    <header>
      <b>🎞 {t("exportVideo")}</b>
      <button class="close" onclick={close} title={phase === "running" ? t("cancel") : `${t("close")} (Esc)`}>✕</button>
    </header>

    {#if phase === "setup"}
      <div class="content">
        <div class="ff" class:bad={!checking && !ff?.encoder}>
          {#if checking}
            {t("ffmpegChecking")}
          {:else if ff?.encoder}
            ✓ {ff.version} · {ff.encoder}
          {:else}
            <p>✗ {ffError ?? t("ffmpegMissing")}</p>
            <p>{t("ffmpegInstall")} <code>{install}</code></p>
          {/if}
          <div class="ff-row">
            <input placeholder={t("ffmpegAuto")} value={prefs.settings.video.ffmpeg} spellcheck="false"
              onchange={(e) => { prefs.settings.video.ffmpeg = e.currentTarget.value.trim(); save(); check(); }} />
            <button onclick={pickFfmpeg}>{t("browse")}</button>
            <button onclick={check}>{t("ffmpegCheck")}</button>
          </div>
        </div>

        <div class="row">
          <span>{t("exportWhat")}</span>
          <div class="col">
            <label><input type="radio" bind:group={range} value="current" /> {t("exportCurrent", { n: index + 1 })}</label>
            {#if count > 1}
              <label><input type="radio" bind:group={range} value="all" /> {t("exportAll", { n: count })}</label>
              <label class="inline">
                <input type="radio" bind:group={range} value="range" /> {t("exportFrom")}
                <input type="number" min="1" max={count} bind:value={from} onfocus={() => (range = "range")} />
                {t("exportTo")}
                <input type="number" min="1" max={count} bind:value={to} onfocus={() => (range = "range")} />
              </label>
            {/if}
          </div>
        </div>

        <div class="row">
          <span>{t("videoSize")}</span>
          <div class="inline">
            <select value={prefs.settings.video.size} onchange={(e) => { prefs.settings.video.size = e.currentTarget.value as VideoSize; save(); }}>
              {#each SIZES as s (s)}<option value={s}>{s === "custom" ? t("videoCustom") : s === "4k" ? "4K" : s}</option>{/each}
            </select>
            {#if prefs.settings.video.size === "custom"}
              <input type="number" min="16" max="8192" step="4" value={prefs.settings.video.width}
                onchange={(e) => { prefs.settings.video.width = Number(e.currentTarget.value) || 1920; save(); }} />
              ×
              <input type="number" min="16" max="8192" step="4" value={prefs.settings.video.height}
                onchange={(e) => { prefs.settings.video.height = Number(e.currentTarget.value) || 1080; save(); }} />
            {:else}
              <div class="seg">
                {#each [["landscape", "shapeLandscape"], ["portrait", "shapePortrait"], ["square", "shapeSquare"]] as const as [s, k] (s)}
                  <button class:on={prefs.settings.video.shape === s} onclick={() => { prefs.settings.video.shape = s; save(); }}>{t(k)}</button>
                {/each}
              </div>
            {/if}
            <span class="muted">{px.w}×{px.h}</span>
          </div>
        </div>

        <div class="row">
          <span>{t("videoFps")}</span>
          <div class="seg">
            {#each [24, 30, 60] as const as f (f)}
              <button class:on={prefs.settings.video.fps === f} onclick={() => { prefs.settings.video.fps = f; save(); }}>{f}</button>
            {/each}
          </div>
        </div>

        <div class="row">
          <span>{t("videoQuality")}</span>
          <div class="seg">
            {#each [["high", "qualityHigh"], ["normal", "qualityNormal"], ["light", "qualityLight"]] as const as [q, k] (q)}
              <button class:on={prefs.settings.video.quality === q} onclick={() => { prefs.settings.video.quality = q; save(); }}>{t(k)}</button>
            {/each}
          </div>
        </div>

        <div class="row">
          <span>{t("videoFile")}</span>
          <div class="inline grow">
            <input class="path" bind:value={path} spellcheck="false" oninput={() => (pathTouched = true)} />
            <button onclick={pickPath}>{t("browse")}</button>
          </div>
        </div>
        {#if hasSound}
          <label class="sound"><input type="checkbox" bind:checked={withSound} /> {t("exportSound")}</label>
        {/if}
        {#if exists}<p class="warn">{t("videoExists")}</p>{/if}
        <p class="note">{t("exportNote")}</p>
      </div>
      <footer>
        <button onclick={onclose}>{t("cancel")}</button>
        <button class="primary" onclick={start} disabled={!ready}>{t("exportStart")}</button>
      </footer>
    {:else if phase === "running"}
      <div class="content">
        <div class="bar"><div style:width="{Math.round(progress.fraction * 100)}%"></div></div>
        <p>
          {t("exportProgress", { i: progress.index - span.a + 1, n: span.b - span.a + 1, pct: Math.round(progress.fraction * 100) })}
        </p>
        <p class="muted">
          {t("exportStats", { frames: progress.frames, video: clock(videoTime * 1000), elapsed: clock(elapsed) })}
          {#if remaining !== null} · {t("exportRemaining", { t: clock(remaining) })}{/if}
        </p>
        <p class="note">{progress.mixing ? t("exportMixing") : t("exportWait")}</p>
      </div>
      <footer>
        <button onclick={() => abort?.abort()}>{t("cancel")}</button>
      </footer>
    {:else if phase === "done"}
      <div class="content">
        <p>✓ {t("exportDone", { time: clock(elapsed) })}</p>
        <code class="result">{result}</code>
      </div>
      <footer>
        <button class="primary" onclick={onclose}>{t("close")}</button>
      </footer>
    {:else}
      <div class="content">
        <p class="err">✗ {t("exportFailed")}</p>
        <pre>{result}</pre>
      </div>
      <footer>
        <button onclick={() => (phase = "setup")}>{t("back")}</button>
        <button onclick={onclose}>{t("close")}</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 45; background: rgb(0 0 0 / 0.55); display: grid; place-items: center; }
  .dialog {
    width: min(620px, calc(100vw - 40px)); max-height: calc(100vh - 40px); display: flex; flex-direction: column;
    background: var(--panel); border: 1px solid var(--border); border-radius: 10px; box-shadow: 0 12px 40px rgb(0 0 0 / 0.5); font-size: 13px;
  }
  header { display: flex; align-items: center; padding: 8px 12px; border-bottom: 1px solid var(--border); }
  header b { flex: 1; }
  .close { background: none; border-color: transparent; }
  .content { padding: 12px 14px; display: flex; flex-direction: column; gap: 10px; overflow-y: auto; }
  .row { display: grid; grid-template-columns: 140px 1fr; gap: 10px; align-items: start; }
  .row > span { color: var(--muted); padding-top: 3px; }
  .row > * { min-width: 0; }
  .col label { display: flex; align-items: center; gap: 6px; }
  @media (max-width: 600px) {
    .row { grid-template-columns: 1fr; gap: 4px; }
  }
  .col { display: flex; flex-direction: column; gap: 4px; }
  .sound { display: flex; align-items: center; gap: 6px; }
  .inline { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
  .grow input.path { flex: 1; min-width: 0; }
  input[type="number"] { width: 70px; }
  input:not([type]), input[type="number"], select {
    font: inherit; color: var(--text); background: var(--bg); border: 1px solid var(--border); border-radius: 6px; padding: 2px 6px;
  }
  .seg { display: inline-flex; }
  .seg button { border-radius: 0; padding: 2px 10px; }
  .seg button:first-child { border-radius: 6px 0 0 6px; }
  .seg button:last-child { border-radius: 0 6px 6px 0; }
  .seg button + button { border-left: none; }
  .seg button.on { background: var(--accent); color: #04121b; border-color: var(--accent); }
  .ff { padding: 8px 10px; border: 1px solid var(--border); border-radius: 8px; display: flex; flex-direction: column; gap: 6px; color: #86efac; }
  .ff.bad { color: #fca5a5; border-color: #7f1d1d; }
  .ff p { margin: 0; }
  .ff code { color: var(--text); user-select: all; }
  .ff-row { display: flex; gap: 6px; }
  .ff-row input { flex: 1; min-width: 80px; }
  .muted { color: var(--muted); }
  .note { margin: 0; color: var(--muted); font-size: 12px; line-height: 1.45; }
  .warn { margin: 0; color: #fbbf24; font-size: 12px; }
  .err { color: #fca5a5; margin: 0; }
  pre { margin: 0; white-space: pre-wrap; font-size: 12px; color: var(--muted); max-height: 200px; overflow: auto; }
  .result { user-select: all; word-break: break-all; }
  p { margin: 0; }
  .bar { height: 10px; border-radius: 5px; background: var(--panel-2); overflow: hidden; }
  .bar div { height: 100%; background: var(--accent); transition: width 0.2s; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
  .primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
  .primary:disabled { opacity: 0.5; }
</style>
