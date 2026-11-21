<script lang="ts">
  // Pestaña «📦 Historia» de Exportar: guarda el cómic con su animación, sus efectos y sus sonidos en
  // un solo archivo `.imgxstory` para compartirlo. No necesita FFmpeg.
  import { onDestroy, untrack } from "svelte";
  import { errorText } from "../errors";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { saveFile, dirOf, joinPath } from "../files/dialog.svelte";
  import { sounds } from "../audio/sounds.svelte";
  import { t } from "../i18n.svelte";
  import { log } from "../log";
  import type { Reading } from "../reading/reading.svelte";

  interface Props {
    generation: number;
    /** Nombre de la fuente (para el título y el archivo propuestos). */
    title: string;
    /** Ruta de la fuente (para no proponer el mismo archivo). */
    sourcePath: string;
    /** Carpeta donde proponer la historia. */
    dir: string;
    count: number;
    index: number;
    /** El proyecto tal como está ahora (JSON). */
    project: () => string;
    /** Proporción del vídeo para la que se pensó la cámara. */
    aspect: number;
    reading: Reading;
    lang: string;
    onclose: () => void;
    /** Hay una exportación en marcha (para que el diálogo no se cierre). */
    running?: boolean;
  }
  let { generation, title, sourcePath, dir, count, index, project, aspect, reading, lang, onclose, running = $bindable(false) }: Props = $props();

  type Variant = "desktop" | "mobile";
  interface Written {
    path: string;
    pages: number;
    sounds: number;
    converted: number;
    bytes: number;
    missing: string[];
  }

  const stem = (s: string) => s.replace(/\.(cbz|cbr|cb7|zip|7z|rar|imgxstory)$/i, "") || "historia";
  let range = $state<"current" | "all" | "range">(untrack(() => (count > 1 ? "all" : "current")));
  let from = $state(1);
  let to = $state(untrack(() => count));
  let variant = $state<Variant>("desktop");
  let name = $state(untrack(() => stem(title)));
  let credits = $state("");
  let keepTakes = $state(false);
  let path = $state("");
  let pathTouched = false;
  let exists = $state(false);
  let phase = $state<"setup" | "running" | "done" | "error">("setup");
  let progress = $state({ done: 0, total: 1 });
  let result = $state<Written | null>(null);
  /** Error tal como llega (se traduce al enseñarlo). */
  let error = $state<unknown>("");
  let estimate = $state<{ pages: number; sounds: number } | null>(null);

  const span = $derived.by(() => {
    if (range === "current") return { a: index, b: index };
    if (range === "all") return { a: 0, b: count - 1 };
    const a = Math.min(Math.max(1, Math.round(from)), count) - 1;
    const b = Math.min(Math.max(a + 1, Math.round(to)), count) - 1;
    return { a, b };
  });

  /** Lo que usa el proyecto: tomas anteriores de las voces y sonidos hechos con IA. */
  const usage = $derived.by(() => {
    let takes = false;
    let generated = false;
    try {
      const p = JSON.parse(project()) as { items?: { lines?: { takes?: string[] }[]; timeline?: { audio?: { voice?: { sound: string }[]; sfx?: { sound: string }[] } } }[]; ambient?: { sound: string }[] };
      const used = new Set<string>();
      for (const it of p.items ?? []) {
        for (const l of it.lines ?? []) if ((l.takes?.length ?? 0) > 1) takes = true;
        for (const c of [...(it.timeline?.audio?.voice ?? []), ...(it.timeline?.audio?.sfx ?? [])]) used.add(c.sound);
      }
      for (const a of p.ambient ?? []) used.add(a.sound);
      generated = [...used].some((id) => sounds.get(id)?.kind === "generated");
    } catch (e) {
      log.warn("historia: proyecto ilegible", e);
    }
    return { takes, generated };
  });

  // Archivo propuesto: junto a la fuente, con su nombre (y «-movil» en la variante para móvil).
  $effect(() => {
    const suffix = (range === "current" && count > 1 ? `-${String(index + 1).padStart(String(count).length, "0")}` : "") + (variant === "mobile" ? "-movil" : "");
    let p = joinPath(dir, `${stem(title)}${suffix}.imgxstory`);
    if (p === sourcePath) p = p.replace(/\.imgxstory$/, "-2.imgxstory");
    if (!pathTouched) path = p;
  });

  $effect(() => {
    const p = path;
    if (!p) return void (exists = false);
    invoke<boolean[]>("paths_exist", { paths: [p] }).then(([e]) => {
      if (p === path) exists = e;
    });
  });

  $effect(() => {
    const { a, b } = span;
    invoke<{ pages: number; sounds: number }>("story_estimate", { generation, from: a, to: b, project: untrack(project) })
      .then((e) => (estimate = e))
      .catch((e) => log.warn("historia: estimar", e));
  });

  const mb = (n: number) => (n >= 1 << 30 ? `${(n / (1 << 30)).toFixed(1)} GB` : `${Math.max(0.1, n / (1 << 20)).toFixed(1)} MB`);

  async function pickPath() {
    const p = await saveFile({ title: t("storyExport"), dir: dirOf(path), defaultName: path.slice(path.lastIndexOf("/") + 1), filters: [{ name: "imgxstory", extensions: ["imgxstory"] }] });
    if (!p) return;
    path = /\.imgxstory$/i.test(p) ? p : `${p}.imgxstory`;
    pathTouched = true;
  }

  let unlisten: (() => void) | null = null;
  onDestroy(() => {
    unlisten?.();
    if (running) invoke("story_cancel").catch(() => {});
  });

  export async function start() {
    if (phase !== "setup" || !path.trim()) return;
    phase = "running";
    running = true;
    progress = { done: 0, total: 1 };
    unlisten = await listen<{ done: number; total: number }>("story-progress", (e) => (progress = e.payload));
    try {
      result = await invoke<Written>("story_export", {
        generation,
        opts: {
          path,
          from: span.a,
          to: span.b,
          variant,
          title: name.trim(),
          credits: credits.trim(),
          keepTakes,
          reading: { fit: reading.fit, crop: reading.crop, double: reading.double, cover: reading.cover, strip: reading.strip, rtl: reading.rtl },
          aspect,
          lang,
          project: project(),
        },
      });
      phase = "done";
    } catch (e) {
      if (String(e) === "cancelado") phase = "setup";
      else {
        error = e;
        phase = "error";
      }
    } finally {
      unlisten?.();
      unlisten = null;
      running = false;
    }
  }

  export function cancel() {
    invoke("story_cancel").catch((e) => log.warn("historia: cancelar", e));
  }

  const reveal = () => result && invoke("show_in_folder", { path: result.path }).catch((e) => log.warn("abrir carpeta:", e));
</script>

{#if phase === "setup"}
  <div class="content">
    <p class="note">{t("storyHelp")}</p>
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
      <span>{t("storyVariant")}</span>
      <div class="col">
        <div class="seg">
          <button class:on={variant === "desktop"} onclick={() => (variant = "desktop")}>🖥 {t("storyDesktop")}</button>
          <button class:on={variant === "mobile"} onclick={() => (variant = "mobile")}>📱 {t("storyMobile")}</button>
        </div>
        <p class="note">{variant === "desktop" ? t("storyDesktopHelp") : t("storyMobileHelp")}</p>
        {#if estimate}
          <p class="note">
            {variant === "desktop" ? t("storySize", { size: mb(estimate.pages + estimate.sounds) }) : t("storySizeMobile", { size: mb(estimate.pages + estimate.sounds) })}
          </p>
        {/if}
      </div>
    </div>

    <div class="row">
      <span>{t("storyTitle")}</span>
      <input class="wide" bind:value={name} maxlength="200" placeholder={stem(title)} />
    </div>
    <div class="row">
      <span>{t("storyCredits")}</span>
      <textarea rows="2" maxlength="2000" bind:value={credits} placeholder={t("storyCreditsHint")}></textarea>
    </div>

    <div class="row">
      <span>{t("videoFile")}</span>
      <div class="inline grow">
        <input class="path" bind:value={path} spellcheck="false" oninput={() => (pathTouched = true)} />
        <button onclick={pickPath}>{t("browse")}</button>
      </div>
    </div>
    {#if usage.takes}
      <label class="check"><input type="checkbox" bind:checked={keepTakes} /> {t("storyKeepTakes")}</label>
    {/if}
    {#if exists}<p class="warn">{t("videoExists")}</p>{/if}
    <p class="note">⚖ {t("storyRights")}</p>
    {#if usage.generated}<p class="note">🗣 {t("storyElevenLabs")}</p>{/if}
  </div>
  <footer>
    <button onclick={onclose}>{t("cancel")}</button>
    <button class="primary" onclick={start} disabled={!path.trim()}>📦 {t("storyStart")}</button>
  </footer>
{:else if phase === "running"}
  <div class="content">
    <div class="bar"><div style:width="{Math.round((progress.done / Math.max(1, progress.total)) * 100)}%"></div></div>
    <p>{t("storyProgress", { done: progress.done, total: progress.total })}</p>
    <p class="note">{t("storyWait")}</p>
  </div>
  <footer>
    <button onclick={cancel}>{t("cancel")}</button>
  </footer>
{:else if phase === "done" && result}
  <div class="content">
    <p>✓ {t("storyDone", { pages: result.pages, sounds: result.sounds, size: mb(result.bytes) })}</p>
    {#if result.converted}<p class="note">{t("storyConverted", { n: result.converted })}</p>{/if}
    {#if result.missing.length}<p class="warn">{t("storyMissing", { n: result.missing.length })}</p>{/if}
    <code class="result">{result.path}</code>
    <p class="note">{t("storyShare")}</p>
  </div>
  <footer>
    <button onclick={reveal}>📂 {t("infoReveal")}</button>
    <button class="primary" onclick={onclose}>{t("close")}</button>
  </footer>
{:else}
  <div class="content">
    <p class="err">✗ {t("storyFailed")}</p>
    <pre>{errorText(error)}</pre>
  </div>
  <footer>
    <button onclick={() => (phase = "setup")}>{t("back")}</button>
    <button onclick={onclose}>{t("close")}</button>
  </footer>
{/if}

<style>
  .content { padding: 12px 14px; display: flex; flex-direction: column; gap: 10px; overflow-y: auto; }
  .row { display: grid; grid-template-columns: 140px 1fr; gap: 10px; align-items: start; }
  .row > span { color: var(--muted); padding-top: 3px; }
  .row > * { min-width: 0; }
  @media (max-width: 600px) {
    .row { grid-template-columns: 1fr; gap: 4px; }
  }
  .col { display: flex; flex-direction: column; gap: 4px; }
  .col label { display: flex; align-items: center; gap: 6px; }
  .check { display: flex; align-items: center; gap: 6px; }
  .inline { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
  .grow input.path { flex: 1; min-width: 0; }
  input[type="number"] { width: 70px; }
  input:not([type]), input[type="number"], textarea {
    font: inherit; color: var(--text); background: var(--bg); border: 1px solid var(--border); border-radius: 6px; padding: 2px 6px;
  }
  textarea { resize: vertical; min-height: 2.6em; }
  .seg { display: inline-flex; }
  .seg button { border-radius: 0; padding: 2px 10px; }
  .seg button:first-child { border-radius: 6px 0 0 6px; }
  .seg button:last-child { border-radius: 0 6px 6px 0; }
  .seg button + button { border-left: none; }
  .seg button.on { background: var(--accent); color: #04121b; border-color: var(--accent); }
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
