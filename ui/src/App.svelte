<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { openFile, openFolder, saveFile, fileDialog, dirOf } from "./lib/files/dialog.svelte";
  import FileDialog from "./lib/files/FileDialog.svelte";
  import ExportDialog from "./lib/export/ExportDialog.svelte";
  import type { ExportSpec } from "./lib/export/exporter";
  import { library } from "./lib/editor/libstore.svelte";
  import Viewer from "./lib/viewer/Viewer.svelte";
  import Thumbs from "./lib/Thumbs.svelte";
  import Settings from "./lib/Settings.svelte";
  import Recents from "./lib/Recents.svelte";
  import Shelf from "./lib/Shelf.svelte";
  import Confirm, { type ConfirmSpec } from "./lib/Confirm.svelte";
  import { sounds, isAudioPath } from "./lib/audio/sounds.svelte";
  import { director } from "./lib/audio/director";
  import { ambientSpecs } from "./lib/audio/sync";
  import { trackSpecs } from "./lib/audio/tracks";
  import { clearSolo, solo } from "./lib/audio/solo.svelte";
  import { prefs, videoPixels } from "./lib/prefs.svelte";
  import { comboOf, display, keymap, keysFor, type ViewerAction } from "./lib/keys";
  import Editor from "./lib/editor/Editor.svelte";
  import { removeRect, type PanelEdit } from "./lib/viewer/panelEdit";
  import ActionPanel from "./lib/action/ActionPanel.svelte";
  import PlayerBar from "./lib/action/PlayerBar.svelte";
  import { ProjectStore, type LoadedProject } from "./lib/action/project.svelte";
  import { FALLBACK, needsPanels, type TemplateRef } from "./lib/action/catalog";
  import type { PlayerInfo } from "./lib/action/player";
  import type { ViewerStatus } from "./lib/viewer/types";
  import type { PictureRef } from "./lib/viewer/picture";
  import { coreVersion, wasm, type Size } from "./lib/core";
  import { label, t } from "./lib/i18n.svelte";
  import { log } from "./lib/log";
  import { startMcp } from "./lib/mcp/host.svelte";
  import McpBadge from "./lib/mcp/McpBadge.svelte";
  import AdjustPanel from "./lib/adjust/AdjustPanel.svelte";
  import { adjustUniforms, normalize } from "./lib/adjust/adjust";
  import { levels } from "./lib/adjust/levels.svelte";
  import ReadingMenu from "./lib/reading/ReadingMenu.svelte";
  import ProgressBar from "./lib/reading/ProgressBar.svelte";
  import GoToPage from "./lib/reading/GoToPage.svelte";
  import { reading, type Reading } from "./lib/reading/reading.svelte";
  import { boxes } from "./lib/reading/boxes.svelte";
  import { spreadOf, spreadStep, type Framing, type ReadLayout } from "./lib/reading/layout";
  import type { Box } from "./lib/reading/boxes.svelte";
  import InfoPanel from "./lib/utils/InfoPanel.svelte";
  import ViewMenu, { type MenuItem } from "./lib/utils/ViewMenu.svelte";
  import SaveImage, { type SaveOptions, type SaveZone } from "./lib/utils/SaveImage.svelte";
  import { copyToClipboard, renderPage, saveTo } from "./lib/utils/imageOut";
  import KeysHelp from "./lib/help/KeysHelp.svelte";
  import Tour from "./lib/help/Tour.svelte";
  import { tour } from "./lib/help/tour.svelte";

  interface DiagnosticsInfo {
    logDir: string | null;
    previousCrash: { pid: number; started: number; reason: string } | null;
    os: string;
    sessionType: string | null;
    webkit: string | null;
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
  let editorOpen = $state(false);
  let editor = $state<Editor>();
  /** Editando las viñetas de la imagen actual sobre el visor. */
  let panelEdit = $state(false);
  let panelSel = $state(-1);
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
    index: i,
  });
  const thumbUrl = (i: number) => `${base}t/${source!.generation}_${i}`;
  const current = $derived(source && source.entries.length ? ref(source, index) : null);

  // Preguntas al usuario (cambios sin guardar…).
  let confirmSpec = $state<ConfirmSpec | null>(null);
  let confirmAnswer: ((v: string) => void) | null = null;
  function askUser(spec: ConfirmSpec): Promise<string> {
    confirmAnswer?.(spec.cancel);
    return new Promise((res) => {
      confirmSpec = spec;
      confirmAnswer = res;
    });
  }
  function answer(v: string) {
    const f = confirmAnswer;
    confirmSpec = null;
    confirmAnswer = null;
    f?.(v);
  }

  /**
   * Antes de dejar la fuente abierta (cerrarla, abrir otra o salir): termina la grabación en
   * curso y guarda. Las animaciones se guardan solas al momento, así que solo pregunta si el
   * guardado falla. Devuelve si se puede seguir.
   */
  async function readyToLeave(kind: "close" | "open" | "quit"): Promise<boolean> {
    if (!source) return true;
    await editor?.finishRecording();
    if (panelEdit) finishPanelEdit();
    store.commit();
    for (;;) {
      if (await store.flush()) return true;
      const n = Math.max(1, store.unsaved);
      const v = await askUser({
        title: t("unsavedTitle"),
        message: t(n === 1 ? "unsavedOne" : "unsavedMany", { n, title: source.title }),
        detail: store.saveError ?? undefined,
        buttons: [
          { label: t("cancel"), value: "cancel" },
          { label: t(`unsavedDiscard.${kind}`), value: "discard", danger: true },
          { label: t("unsavedRetry"), value: "retry", primary: true },
        ],
        cancel: "cancel",
      });
      if (v === "cancel") return false;
      if (v === "discard") {
        store.discard();
        return true;
      }
    }
  }

  /** Cierra lo que se está viendo y vuelve a la pantalla de inicio (recientes). */
  async function closeSource() {
    if (!source || opening) return;
    if (!(await readyToLeave("close"))) return;
    viewer?.stop();
    actionOpen = false;
    editorOpen = false;
    exportOpen = false;
    recentsOpen = false;
    panelEdit = false;
    panelSel = -1;
    prefs.flush();
    source = null;
    index = 0;
    status = null;
    openError = null;
    overlayPanels = null;
    detectedCount = null;
    panelCache.clear();
    sizes.clear();
    readingOpen = false;
    gotoOpen = false;
    store.unload();
    clearSolo();
    invoke("close_source").catch((e) => log.warn("cerrar fuente:", e));
    getCurrentWindow().setTitle("imgxplorer").catch((e) => log.warn("título:", e));
  }

  /** Abre una fuente. Devuelve el error (o null si se abrió). */
  async function openPath(path: string): Promise<string | null> {
    if (opening) return t("loading");
    if (!(await readyToLeave("open"))) return "cancelled by the user";
    opening = true;
    viewer?.stop();
    try {
      const s = await invoke<SourceInfo>("open_source", { path });
      panelCache.clear();
      clearSolo();
      await store.load(s.generation, { kind: s.kind, title: s.title, fingerprint: s.fingerprint }, s.project);
      levels.reset(s.generation);
      boxes.reset(s.generation);
      sizes.clear();
      sizeRev++;
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
    return openError;
  }

  // Mensaje breve sobre el visor (p. ej. al soltar un paquete .imgxpack).
  let toast = $state<string | null>(null);
  let toastTimer = 0;
  function say(msg: string) {
    toast = msg;
    clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => (toast = null), 4000);
  }

  /** Archivos de audio soltados sobre la ventana: se copian a la biblioteca de sonidos. */
  async function importSounds(paths: string[]) {
    say(t("loading"));
    try {
      if (!sounds.loaded) await sounds.load();
      const r = await sounds.import(paths);
      say(r.errors.length ? t("soundImportError", { error: r.errors.join(" · ") }) : t("soundsImported", { n: r.added.length }));
    } catch (e) {
      log.warn("importar sonidos:", e);
      say(t("soundImportError", { error: String(e) }));
    }
  }

  /** Paquetes soltados sobre la ventana: se añaden a la biblioteca. */
  async function importPacks(paths: string[]) {
    let clips = 0;
    let templates = 0;
    let snd = 0;
    if (!sounds.loaded) await sounds.load();
    for (const p of paths) {
      try {
        const r = await library.importPack(p);
        clips += r.clips;
        templates += r.templates;
        snd += r.sounds.length;
      } catch (e) {
        log.warn("importar paquete:", p, e);
        return say(t("packError", { error: String(e) }));
      }
    }
    say(clips || templates || snd ? t("imported", { clips, templates, sounds: snd }) : t("importedNothing"));
  }

  async function pickFolder() {
    const dir = await openFolder({ dir: source ? dirOf(source.path) : undefined });
    if (dir) openPath(dir);
  }

  async function pickFile() {
    const file = await openFile({
      dir: source ? (source.kind === "folder" ? source.path : dirOf(source.path)) : undefined,
      filters: [
        { name: t("filterAll"), extensions: [...ARCHIVES, ...IMAGES] },
        { name: t("filterArchives"), extensions: ARCHIVES },
        { name: t("filterImages"), extensions: IMAGES },
      ],
    });
    if (file) openPath(file);
  }

  // ── Utilidades (ℹ️): información, copiar, guardar, recortar, carpeta y papelera ──
  let infoOpen = $state(false);
  let saveZone = $state<SaveZone | null>(null);
  let viewMenu = $state<{ x: number; y: number } | null>(null);
  /** Ayuda de atajos (?) y pestaña con la que se abren los ajustes. */
  let helpOpen = $state(false);
  let settingsTab = $state<"general" | "keys">("general");
  /** Zona elegida con Mayús + arrastrar. */
  let selection = $state<Box | null>(null);
  /** Lo que se ve, apuntado al abrir «Guardar como» (antes de que el diálogo cambie nada). */
  let visibleAtSave = $state<Box | null>(null);

  async function outCanvas(zone: SaveZone, withAdjust: boolean): Promise<HTMLCanvasElement> {
    const s = source!;
    const i = index;
    const adj = withAdjust ? await sequence.adjust!(i) : null;
    const box = zone === "selection" ? selection : zone === "visible" ? visibleAtSave : null;
    return renderPage(ref(s, i), adj, box);
  }

  async function copyImage() {
    if (!source?.entries.length) return;
    say(t("copying"));
    try {
      await copyToClipboard(await outCanvas(selection ? "selection" : "page", true));
      say(t(selection ? "copiedZone" : "copiedImage"));
    } catch (e) {
      log.warn("copiar imagen:", e);
      say(t("copyError", { error: String(e) }));
    }
  }

  function openSave() {
    if (!source?.entries.length) return;
    visibleAtSave = viewer.visibleBox();
    saveZone = selection ? "selection" : "page";
  }

  async function doSave(o: SaveOptions) {
    saveZone = null;
    const s = source;
    if (!s) return;
    const i = index;
    const name = s.entries[i].name;
    const file = name.slice(name.lastIndexOf("/") + 1);
    const stem = file.replace(/\.[^.]+$/, "");
    const ext = o.format === "original" ? (/\.([^.]+)$/.exec(file)?.[1] ?? "png") : o.format === "png" ? "png" : "jpg";
    const suffix = o.zone === "page" ? "" : o.zone === "visible" ? t("saveImgSuffixVisible") : t("saveImgSuffixCrop");
    const path = await saveFile({
      title: t("saveImgTitle"),
      dir: s.kind === "folder" ? s.path : dirOf(s.path),
      defaultName: `${stem}${suffix}.${ext}`,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (!path) return;
    say(t("saving"));
    try {
      if (o.format === "original") await invoke("save_original", { generation: s.generation, index: i, path });
      else await saveTo(path, await outCanvas(o.zone, o.adjust), o.format === "png" ? "image/png" : "image/jpeg");
      say(t("savedImage", { path }));
    } catch (e) {
      log.warn("guardar imagen:", e);
      say(t("saveImgError", { error: String(e) }));
    }
  }

  function reveal() {
    if (source?.entries.length) invoke("reveal_entry", { generation: source.generation, index }).catch((e) => say(String(e)));
  }

  /** Mover a la papelera del sistema (solo en carpetas), preguntando antes. */
  async function trashCurrent() {
    const s = source;
    const name = curEntry;
    if (!s || name === null) return;
    if (s.kind !== "folder") return say(t("trashArchive"));
    const v = await askUser({
      title: t("trashTitle"),
      message: t("trashMessage", { name }),
      detail: t("trashDetail"),
      buttons: [
        { label: t("cancel"), value: "cancel" },
        { label: t("trashGo"), value: "go", danger: true },
      ],
      cancel: "cancel",
    });
    if (v !== "go") return;
    try {
      await invoke("trash_entry", { generation: s.generation, index });
      // La lista de la carpeta ha cambiado: se vuelve a abrir en la misma posición.
      await openPath(s.path);
      say(t("trashed", { name }));
    } catch (e) {
      log.warn("papelera:", e);
      say(t("trashError", { error: String(e) }));
    }
  }

  const viewMenuItems = $derived.by((): (MenuItem | null)[] => {
    const k = (a: ViewerAction) => kt(a).trim().replace(/^\(|\)$/g, "");
    const folder = source?.kind === "folder";
    return [
      { label: `ℹ️ ${t("infoTitle")}`, key: k("info"), run: () => (infoOpen = !infoOpen) },
      { label: `📋 ${t(selection ? "menuCopyZone" : "menuCopy")}`, key: k("copyImage"), run: copyImage },
      { label: `💾 ${t("saveImgTitle")}`, key: k("saveImage"), run: openSave },
      { label: `✂ ${t("menuCropHint")}`, hint: true },
      null,
      { label: `🔖 ${t(markSet.has(index) ? "menuUnmark" : "menuMark")}`, key: k("bookmark"), run: toggleMark },
      { label: `↪ ${t("readGoTo")}`, key: k("goTo"), run: () => (gotoOpen = true) },
      null,
      { label: `📂 ${t("infoReveal")}`, key: k("reveal"), run: reveal },
      { label: `🗑 ${t("trashGo")}`, key: k("trash"), danger: true, disabled: !folder, run: trashCurrent },
      null,
      { label: `⌨️ ${t("menuHelp")}`, key: k("help"), run: () => (helpOpen = true) },
    ];
  });

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

  /** Viñetas para la plantilla: las dibujadas a mano o, si hace falta, las detectadas. */
  async function panelsInput(i: number, tpl: TemplateRef | null): Promise<{ panels: number[]; manual: boolean }> {
    const own = store.item(source!.entries[i].name)?.panels;
    if (own?.length) return { panels: own.flatMap((r) => [r.x, r.y, r.w, r.h]), manual: true };
    return { panels: needsPanels(tpl) ? await panelsFor(i) : [], manual: false };
  }

  async function templateTimeline(i: number, tpl: TemplateRef, size: Size, aspect: number): Promise<wasm.WasmTimeline> {
    const { panels, manual } = await panelsInput(i, tpl);
    return wasm.WasmTimeline.from_template(JSON.stringify($state.snapshot(tpl)), size.w, size.h, aspect, new Float64Array(panels), manual);
  }

  async function timelineFor(i: number, size: Size, aspect: number): Promise<wasm.WasmTimeline> {
    const entry = source!.entries[i].name;
    const it = store.item(entry);
    const tpl = store.templateFor(entry);
    if (!tpl && it?.timeline?.camera?.length) return new wasm.WasmTimeline(JSON.stringify($state.snapshot(it.timeline)));
    return templateTimeline(i, tpl ?? FALLBACK, size, aspect);
  }

  /** Para el editor: la timeline de la plantilla de la imagen actual (null si no usa). */
  async function editorTemplate(): Promise<wasm.WasmTimeline | null> {
    const s = source;
    const img = status?.image;
    if (!s || !img) return null;
    const i = index;
    const tpl = store.templateFor(s.entries[i].name);
    return tpl ? templateTimeline(i, tpl, img, viewer.aspect()) : null;
  }

  /** Cómo se recorre la fuente al reproducir (y al exportar a vídeo). */
  const sequence: ExportSpec = {
    timeline: timelineFor,
    transitionOut: (i) => {
      const entry = source!.entries[i].name;
      const tr = $state.snapshot(store.transitionFor(entry));
      const tpl = store.templateFor(entry);
      return { json: JSON.stringify(tr), tpl: tpl ? JSON.stringify($state.snapshot(tpl)) : "", duration: tr.duration, cut: tr.type === "cut" };
    },
    holdBetween: () => store.data?.defaults.holdBetween ?? 0.3,
    audio: (i) => store.audioFor(source!.entries[i].name),
    fx: (i) => $state.snapshot(store.fxFor(source!.entries[i].name)),
    ambient: (i) => ambientSpecs(store.ambients, i, indexOfEntry),
    tracks: () => trackSpecs($state.snapshot(store.tracks)),
    // Ajustes de imagen (🎨), sin el modo noche (es solo para leer).
    adjust: async (i) => {
      const adj = store.adjustFor(source!.entries[i].name);
      if (!adj) return null;
      return adjustUniforms(adj, adj.clean ? await levels.wait(i) : null, false);
    },
    adjusted: () => !!store.data && (!!normalize(store.data.defaults.adjust) || store.data.items.some((it) => normalize(it.adjust))),
  };

  /** Nombres de las entradas, en orden, y su posición (para los ambientes). */
  const entryNames = $derived(source ? source.entries.map((e) => e.name) : []);
  const entryIndex = $derived(new Map(entryNames.map((n, i) => [n, i])));
  const indexOfEntry = (name: string) => entryIndex.get(name) ?? -1;

  // Volumen de cada pista (del proyecto) y general y silencio (de los ajustes).
  $effect(() => {
    director.levels($state.snapshot(store.mix), prefs.settings.sound.master, prefs.settings.sound.mute);
  });
  // Pistas (con sus efectos) y las que están en solo (solo existentes).
  $effect(() => {
    const tracks = trackSpecs($state.snapshot(store.tracks));
    director.tracks(tracks, solo.ids.filter((id) => tracks.some((t) => t.id === id)));
  });

  function toggleMute() {
    prefs.settings.sound.mute = !prefs.settings.sound.mute;
    prefs.saveSettings();
    say(prefs.settings.sound.mute ? t("muted") : t("unmuted"));
  }

  function startPlay() {
    if (!source || !source.entries.length) return;
    viewer.play({ ...sequence, start: index, count: source.entries.length, onIndex: (i) => (index = i), sound: director });
  }

  let exportOpen = $state(false);
  function openExport() {
    if (!source?.entries.length) return;
    if (playerInfo) viewer.stop();
    exportOpen = true;
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
    const own = entry ? store.item(entry)?.panels : undefined;
    // Lee todos los parámetros y las viñetas propias: se recalcula al cambiarlos.
    const json = tpl?.id === "comic" ? JSON.stringify(tpl) : null;
    void JSON.stringify(own);
    if (!json && !panelEdit) {
      overlayPanels = null;
      return;
    }
    let cancelled = false;
    const auto = needsPanels(tpl) && !own?.length;
    if (auto) detectedCount = null;
    panelsInput(i, tpl).then(({ panels, manual }) => {
      if (cancelled) return;
      if (auto) detectedCount = panels.length / 4;
      overlayPanels = json ? wasm.comic_panels(json, 1, 1, 1, new Float64Array(panels), manual) : new Float64Array(panels);
    });
    return () => {
      cancelled = true;
    };
  });

  const curEntry = $derived(source?.entries[index]?.name ?? null);
  // El estado del visor cambia en cada fotograma de una animación: el tamaño se deriva de
  // números para que `curSize` solo cambie (y avise a quien lo usa) si cambia de verdad.
  const imgW = $derived(status?.image?.w ?? 0);
  const imgH = $derived(status?.image?.h ?? 0);
  const curSize = $derived<[number, number] | null>(imgW && imgH ? [imgW, imgH] : null);
  // Ajustes de imagen (🎨) de cada imagen para el visor. Se rehace (y el visor repinta) al cambiar
  // los ajustes del proyecto, al llegar los niveles de «limpiar escaneo» o con el modo noche.
  let adjustOpen = $state(false);
  /** Mantener pulsada la tecla de comparar enseña el original. */
  let comparing = $state(false);
  const adjustAt = $derived.by(() => {
    const s = source;
    const data = store.data;
    if (!s || !data) return null;
    const night = prefs.settings.night;
    void levels.rev;
    const own = new Map(data.items.filter((i) => i.adjust).map((i) => [i.entry, normalize($state.snapshot(i.adjust))]));
    const all = normalize($state.snapshot(data.defaults.adjust));
    if (!own.size && !all && !night) return null;
    return (i: number) => {
      const name = s.entries[i]?.name;
      const adj = (name !== undefined ? own.get(name) : null) ?? all;
      return adjustUniforms(adj, adj?.clean ? levels.get(i) : null, night);
    };
  });
  const curAdjust = $derived(curEntry ? store.adjustFor(curEntry) : null);
  /** «Limpiar escaneo» activo en esta página pero aún sin su análisis. */
  const analyzing = $derived.by(() => {
    void levels.rev;
    return !!curAdjust?.clean && !!source && levels.get(index) === null;
  });

  const manualPanels = $derived(curEntry ? (store.item(curEntry)?.panels ?? null) : null);

  function setManualPanels(flat: number[]) {
    if (!curEntry || !curSize) return;
    const rects = [];
    for (let k = 0; k + 3 < flat.length; k += 4) rects.push({ x: flat[k], y: flat[k + 1], w: flat[k + 2], h: flat[k + 3] });
    store.setPanels(curEntry, curSize, rects);
  }

  function togglePanelEdit() {
    if (panelEdit) return finishPanelEdit();
    if (!curEntry || !curSize) return;
    // Se parte de lo que se ve (detectadas o rejilla) para corregirlo.
    if (!manualPanels) setManualPanels(overlayPanels ? Array.from(overlayPanels) : []);
    panelSel = -1;
    panelEdit = true;
  }

  /** Termina de editar viñetas. Si no quedó ninguna, vuelve la detección automática. */
  function finishPanelEdit(entry = curEntry) {
    panelEdit = false;
    panelSel = -1;
    const it = entry ? store.item(entry) : undefined;
    if (it?.panels && !it.panels.length) store.setPanels(it.entry, it.size, null);
  }

  const editPanels = $derived<PanelEdit | null>(
    panelEdit && manualPanels
      ? {
          rects: manualPanels.flatMap((r) => [r.x, r.y, r.w, r.h]),
          selected: panelSel,
          select: (i) => (panelSel = i),
          change: setManualPanels,
          gesture: (active) => store.batch(active),
        }
      : null,
  );

  // Al cambiar de imagen, cerrar el panel de Acción o reproducir, se termina de editar viñetas.
  let lastEntry: string | null = null;
  $effect(() => {
    const e = curEntry;
    const stop = !actionOpen || !!playerInfo;
    untrack(() => {
      if (panelEdit && (stop || (lastEntry !== null && e !== lastEntry))) finishPanelEdit(lastEntry);
      lastEntry = e;
    });
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
  const editorKeys = $derived(keymap(prefs.settings.keys, "editor"));
  const editorShown = $derived(editorOpen && !!source?.entries.length && !playerInfo);

  // ── Lectura (📖): modo de cada cómic, doble página, tira vertical, marcadores ──
  let readingOpen = $state(false);
  let gotoOpen = $state(false);
  const rd = $derived(reading.of(source?.path));
  function setReading(patch: Partial<Reading>) {
    if (source) reading.set(source.path, patch);
  }
  /** Tamaño de las imágenes ya cargadas (para saber cuáles son apaisadas en doble página). */
  const sizes = new Map<number, Size>();
  let sizeRev = $state(0);
  function onSize(i: number, sz: Size) {
    const had = sizes.get(i);
    sizes.set(i, sz);
    if (!had || had.w > had.h !== sz.w > sz.h) sizeRev++;
  }
  const wide = (i: number) => {
    const sz = sizes.get(i);
    return !!sz && sz.w > sz.h;
  };
  /** Los modos de lectura se aplican al leer: no en el editor, el Modo Acción ni la reproducción. */
  const readingActive = $derived(!!source?.entries.length && !editorShown && !actionOpen && !panelEdit && !playerInfo);
  const stripOn = $derived(readingActive && rd.strip);
  const doubleOn = $derived(readingActive && rd.double && !rd.strip);
  const count = $derived(source?.entries.length ?? 0);
  /** Páginas que se ven ahora (dos en doble página). */
  const spreadNow = $derived.by(() => {
    void sizeRev;
    return doubleOn ? spreadOf(index, count, rd.cover, wide) : [index];
  });
  const readLayout = $derived.by<ReadLayout | null>(() => {
    const s = source;
    if (!s || !s.entries.length) return null;
    if (stripOn) return { kind: "strip", id: String(s.generation), count: s.entries.length, at: index };
    if (doubleOn && spreadNow.length === 2) {
      const pages = spreadNow.map((i) => ref(s, i));
      return { kind: "double", pages: rd.rtl ? pages.reverse() : pages };
    }
    return null;
  });
  const framing = $derived.by<Framing | null>(() => {
    if (!readingActive) return null;
    const crop = rd.crop;
    if (crop) void boxes.rev;
    return { fit: rd.fit, rtl: rd.rtl, box: crop ? (i: number) => boxes.get(i) : null };
  });
  // Con ✂, el recuadro de las vecinas se pide antes de llegar a ellas (así no hay salto).
  $effect(() => {
    if (!rd.crop || !readingActive) return;
    for (const n of neighbours) if (n.index !== undefined) boxes.get(n.index);
  });
  const markSet = $derived.by(() => {
    const s = source;
    if (!s) return new Set<number>();
    const names = new Set(rd.marks);
    return new Set(s.entries.flatMap((e, i) => (names.has(e.name) ? [i] : [])));
  });
  const markList = $derived([...markSet].sort((a, b) => a - b).map((i) => ({ index: i, name: source!.entries[i].name })));

  /** Página siguiente o anterior (en doble página, de dos en dos). */
  function pageStep(dir: 1 | -1) {
    if (!source) return;
    if (doubleOn) return go(spreadStep(index, dir, count, rd.cover, wide));
    go(index + dir);
  }
  /** Volver atrás leyendo: la página anterior se encuadra por el final (al ancho, por abajo). */
  function back() {
    if (!source || spreadNow[0] <= 0) return;
    if (readingActive && !rd.strip) viewer.arriveAtEnd();
    pageStep(-1);
  }
  /** En los ajustes al ancho y al alto (y la tira), avanzar lee la página entera antes de pasarla. */
  const scrollMode = () => readingActive && (rd.strip || rd.fit !== "window");
  function primaryScroll(dir: 1 | -1): boolean {
    if (rd.fit === "height" && !rd.strip) return viewer.scrollToward("x", rd.rtl ? (-dir as 1 | -1) : dir);
    return viewer.scrollToward("y", dir);
  }
  async function changeReading(patch: Partial<Reading>, msg?: string) {
    setReading(patch);
    if (msg) say(msg);
    await tick();
    viewer?.fit();
  }
  function toggleMark() {
    if (!source || !curEntry) return;
    const on = reading.toggleMark(source.path, curEntry);
    say(t(on ? "readMarked" : "readUnmarked", { n: index + 1 }));
  }

  // Si las páginas son muy altas (más de 2,5 veces su ancho), se ofrece leerlo como webtoon.
  const webtoonAsk = $derived(
    !!source && readingActive && !rd.strip && !rd.asked && imgW > 0 && imgH > imgW * 2.5 && source.entries.length > 1,
  );

  const neighbours = $derived(
    source
      ? (doubleOn ? [index + 2, index + 3, index - 1, index - 2, index + 1] : [index + 1, index - 1, index + 2])
          .filter((i) => i >= 0 && i < source!.entries.length)
          .map((i) => ref(source!, i))
      : [],
  );

  // Marco de reproducción: con el editor abierto el visor es más bajo que al reproducir (el
  // editor se oculta), así que el encuadre cambiaría. El visor se estrecha a la proporción que
  // tendrá al reproducir y lo que se ve dentro es exactamente lo que se verá.
  let stageW = $state(0);
  let stageH = $state(0);
  let editorH = $state(0);
  // Con el marco "vídeo", la proporción es la del vídeo que se exportará.
  const frame = $derived.by(() => {
    const mode = prefs.settings.editorFrame;
    if (!editorShown || mode === "off" || !stageW || !stageH || !editorH) return null;
    const video = mode === "video" ? videoPixels(prefs.settings.video) : null;
    const pw = video?.w ?? stageW;
    const ph = video?.h ?? stageH + editorH;
    const aspect = pw / ph;
    let w = stageW;
    let h = Math.round(stageW / aspect);
    if (h > stageH) {
      h = stageH;
      w = Math.max(1, Math.round(stageH * aspect));
    }
    return { x: Math.round((stageW - w) / 2), y: Math.round((stageH - h) / 2), w, h, pw, ph, video: !!video };
  });
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
    await w.setFullscreen(!(await w.isFullscreen()));
    await syncFullscreen();
  }

  // Pantalla completa limpia: la cabecera y las miniaturas flotan sobre la imagen y se esconden
  // a los 2 s sin mover el ratón cerca del borde (Ajustes → «Ocultar las barras…» lo desactiva).
  let fullscreen = $state(false);
  /** Menú «⋯» de la cabecera adaptable (abajo). */
  let moreMenu = $state<{ x: number; y: number } | null>(null);
  async function syncFullscreen() {
    const was = fullscreen;
    fullscreen = await getCurrentWindow().isFullscreen().catch(() => false);
    if (fullscreen && !was) {
      wakeBars();
      if (prefs.settings.hideBars) say(t("fullscreenHint"));
    }
  }
  const clean = $derived(fullscreen && prefs.settings.hideBars && !!source?.entries.length && !editorShown);
  let barsAwake = $state(true);
  let pointerIdle = $state(false);
  let overBars = false;
  let barsTimer = 0;
  let idleTimer = 0;
  let thumbsH = $state(0);
  const barsHidden = $derived(clean && !barsAwake && !recentsOpen && !readingOpen && !moreMenu);
  function wakeBars() {
    barsAwake = true;
    clearTimeout(barsTimer);
    barsTimer = window.setTimeout(() => {
      if (!overBars) barsAwake = false;
    }, 2000);
  }
  function onPointerMove(e: PointerEvent) {
    if (!clean) return;
    pointerIdle = false;
    clearTimeout(idleTimer);
    idleTimer = window.setTimeout(() => (pointerIdle = true), 2000);
    const edge = e.clientY < 56 || e.clientY > window.innerHeight - Math.max(40, thumbsH + 16);
    if (edge || overBars) wakeBars();
  }
  $effect(() => {
    if (!clean) {
      barsAwake = true;
      pointerIdle = false;
    } else wakeBars();
  });

  // Cabecera adaptable: si la ventana es tan estrecha que los botones no caben, los que van al
  // final de esta lista pasan (en este orden) al menú «⋯». Con un tamaño normal se ven todos.
  type Fold = "export" | "editor" | "action" | "fullscreen" | "thumbs" | "rotate" | "actual" | "fit" | "zoom" | "open";
  const FOLDS: Fold[] = ["export", "editor", "action", "fullscreen", "thumbs", "rotate", "actual", "fit", "zoom", "open"];
  let folded = $state(0);
  let headerEl: HTMLElement | undefined = $state();
  let moreBtn: HTMLButtonElement | undefined = $state();
  const shows = (f: Fold) => FOLDS.indexOf(f) >= folded;
  function headerOverflows(): boolean {
    const h = headerEl!;
    const cs = getComputedStyle(h);
    // El hueco y el título se estiran o encogen solos: no cuentan.
    const kids = ([...h.children] as HTMLElement[]).filter((k) => !k.classList.contains("spacer") && !k.classList.contains("title"));
    const gap = parseFloat(cs.columnGap) || 0;
    const used = kids.reduce((a, k) => a + k.offsetWidth, 0) + gap * (kids.length + 1) + parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
    return used > h.clientWidth + 0.5;
  }
  let fitting = false;
  async function fitHeader() {
    if (!headerEl || fitting) return;
    fitting = true;
    try {
      folded = 0;
      await tick();
      while (folded < FOLDS.length && headerOverflows()) {
        folded++;
        await tick();
      }
    } finally {
      fitting = false;
    }
  }
  $effect(() => {
    // Cambia lo que ocupa la cabecera: título, idioma, botón de la IA…
    void [source?.title, t("open"), prefs.settings.aiControl, clean];
    void tick().then(fitHeader);
  });
  $effect(() => {
    if (!headerEl) return;
    const ro = new ResizeObserver(() => void fitHeader());
    ro.observe(headerEl);
    return () => ro.disconnect();
  });
  function openMore() {
    const r = moreBtn!.getBoundingClientRect();
    moreMenu = { x: r.left, y: r.bottom + 4 };
  }
  const moreItems = $derived.by((): MenuItem[] => {
    const k = (a: ViewerAction) => kt(a).trim().replace(/^\(|\)$/g, "");
    const all: [Fold, MenuItem[]][] = [
      ["open", [
        { label: `📄 ${t("openFile")}`, key: k("openFile"), run: pickFile },
        { label: `📁 ${t("open")}`, key: k("openFolder"), run: pickFolder },
      ]],
      ["zoom", [
        { label: `＋ ${label("kv.zoomIn")}`, key: k("zoomIn"), disabled: !source, run: zoomIn },
        { label: `－ ${label("kv.zoomOut")}`, key: k("zoomOut"), disabled: !source, run: zoomOut },
      ]],
      ["fit", [{ label: `⤢ ${t("fit")}`, key: k("fit"), disabled: !source, run: () => viewer.fit() }]],
      ["actual", [{ label: `1:1 ${t("actual")}`, key: k("actual"), disabled: !source, run: () => viewer.actualSize() }]],
      ["rotate", [{ label: `⟳ ${t("rotate")}`, key: k("rotateCw"), disabled: !source, run: () => viewer.rotate(90) }]],
      ["thumbs", [{ label: `▦ ${t("thumbs")}${prefs.settings.showThumbs ? " ✓" : ""}`, key: k("thumbs"), run: toggleThumbs }]],
      ["fullscreen", [{ label: `⛶ ${t("fullscreen")}${fullscreen ? " ✓" : ""}`, key: k("fullscreen"), run: toggleFullscreen }]],
      ["action", [{ label: `🎬 ${t("action")}${actionOpen ? " ✓" : ""}`, key: k("action"), disabled: !source, run: () => (actionOpen = !actionOpen) }]],
      ["editor", [{ label: `✎ ${t("editor")}${editorOpen ? " ✓" : ""}`, key: k("editor"), disabled: !source, run: () => (editorOpen = !editorOpen) }]],
      ["export", [{ label: `🎞 ${t("exportVideo")}`, key: k("exportVideo"), disabled: !source?.entries.length, run: openExport }]],
    ];
    // En el orden de la cabecera (de izquierda a derecha).
    return all.filter(([f]) => !shows(f)).flatMap(([, items]) => items);
  });

  // Primer uso guiado: sale sola una vez, cuando ya se saben los ajustes.
  $effect(() => {
    if (prefs.loaded && !prefs.settings.tourDone && tour.step < 0) untrack(() => tour.start());
  });
  $effect(() => {
    if (tour.step >= 0) untrack(() => {
      settingsOpen = false;
      helpOpen = false;
    });
  });

  function onKey(e: KeyboardEvent) {
    if (settingsOpen || fileDialog.request || exportOpen || confirmSpec || saveZone || helpOpen || tour.step >= 0) return; // los diálogos gestionan sus teclas
    if ((viewMenu || moreMenu) && e.key === "Escape") return; // lo cierra el propio menú
    const tgt = e.target;
    // En los campos de texto las teclas son para escribir. En listas, deslizadores y casillas
    // solo se quedan las teclas sueltas (flechas…): los atajos con Ctrl siguen funcionando.
    const textField = tgt instanceof HTMLTextAreaElement || (tgt instanceof HTMLInputElement && !["range", "checkbox", "radio", "color"].includes(tgt.type));
    if (textField) return;
    // Borrar en el editor funciona aunque acabes de mover un deslizador o elegir en una lista del
    // inspector (ahí Supr y Retroceso no hacen nada más).
    const editorRemove = editorShown && editorKeys.get(comboOf(e) ?? "") === "remove";
    if ((tgt instanceof HTMLInputElement || tgt instanceof HTMLSelectElement) && !e.ctrlKey && !e.metaKey && !editorRemove) return;
    if (recentsOpen && e.key === "Escape") {
      recentsOpen = false;
      return e.preventDefault();
    }
    if (adjustOpen && e.key === "Escape" && !editorShown) {
      adjustOpen = false;
      return e.preventDefault();
    }
    const combo = comboOf(e);
    if (!combo) return;
    if (playerKey(combo)) return e.preventDefault();
    const ea = editorShown ? editorKeys.get(combo) : undefined;
    if (ea && editor?.handle(ea)) return e.preventDefault();
    if (panelEdit && (e.key === "Delete" || e.key === "Backspace") && panelSel >= 0 && editPanels) {
      setManualPanels(removeRect(editPanels.rects, panelSel));
      panelSel = -1;
      return e.preventDefault();
    }
    if (panelEdit && e.key === "Escape") {
      finishPanelEdit();
      return e.preventDefault();
    }
    if (selection && e.key === "Escape") {
      viewer?.clearSelection();
      return e.preventDefault();
    }
    if (infoOpen && e.key === "Escape" && !editorShown) {
      infoOpen = false;
      return e.preventDefault();
    }
    if (gotoOpen && e.key === "Escape") {
      gotoOpen = false;
      return e.preventDefault();
    }
    if (readingOpen && e.key === "Escape") {
      readingOpen = false;
      return e.preventDefault();
    }
    const action = viewerKeys.get(combo);
    if (!action && e.key === "Escape" && fullscreen) {
      void getCurrentWindow().setFullscreen(false).then(syncFullscreen);
      return e.preventDefault();
    }
    if (!action) return;
    const zoomed = viewer?.isZoomedIn();
    const step = e.shiftKey ? 240 : 80;
    const scroll = scrollMode();
    // En manga, la página siguiente está a la izquierda.
    const toRight = () => (readingActive && rd.rtl ? back() : pageStep(1));
    const toLeft = () => (readingActive && rd.rtl ? pageStep(1) : back());
    switch (action) {
      case "right": scroll ? viewer.scrollToward("x", 1, step) || toRight() : zoomed ? viewer.pan(-step, 0) : toRight(); break;
      case "left": scroll ? viewer.scrollToward("x", -1, step) || toLeft() : zoomed ? viewer.pan(step, 0) : toLeft(); break;
      case "down": scroll ? viewer.scrollToward("y", 1, step) || pageStep(1) : zoomed ? viewer.pan(0, -step) : pageStep(1); break;
      case "up": scroll ? viewer.scrollToward("y", -1, step) || back() : zoomed ? viewer.pan(0, step) : pageStep(-1); break;
      case "next": scroll ? primaryScroll(1) || pageStep(1) : pageStep(1); break;
      case "prev": scroll ? primaryScroll(-1) || back() : pageStep(-1); break;
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
      case "settings": settingsTab = "general"; settingsOpen = true; break;
      case "editor": if (source) editorOpen = !editorOpen; break;
      case "exportVideo": openExport(); break;
      case "closeSource": closeSource(); break;
      case "mute": toggleMute(); break;
      case "undo": store.undo(); break;
      case "redo": store.redo(); break;
      case "adjust": if (source) adjustOpen = !adjustOpen; break;
      case "night": prefs.settings.night = !prefs.settings.night; prefs.saveSettings(); break;
      case "compare": comparing = true; break;
      case "reading": if (source) readingOpen = !readingOpen; break;
      case "fitWidth": if (source) changeReading({ fit: rd.fit === "width" ? "window" : "width" }, t(rd.fit === "width" ? "readFit_window" : "readFit_width")); break;
      case "fitHeight": if (source) changeReading({ fit: rd.fit === "height" ? "window" : "height" }, t(rd.fit === "height" ? "readFit_window" : "readFit_height")); break;
      case "double": if (source) changeReading({ double: !rd.double, strip: false }, t(rd.double && !rd.strip ? "readSingle" : "readDouble")); break;
      case "strip": if (source) changeReading({ strip: !rd.strip }, t(rd.strip ? "readStripOff" : "readStrip")); break;
      case "loupe": lensKey = true; viewer?.setLens(true); break;
      case "goTo": if (source?.entries.length) gotoOpen = true; break;
      case "bookmark": toggleMark(); break;
      case "info": if (source?.entries.length) infoOpen = !infoOpen; break;
      case "copyImage": copyImage(); break;
      case "saveImage": openSave(); break;
      case "reveal": reveal(); break;
      // Con el editor abierto, Supr es para borrar lo seleccionado en él (nunca la imagen).
      case "trash": if (source?.entries.length && !editorShown && !panelEdit) trashCurrent(); break;
      case "help": helpOpen = true; break;
    }
    e.preventDefault();
  }

  /** Soltar la tecla de comparar vuelve a los ajustes. */
  let lensKey = false;
  function onKeyUp(e: KeyboardEvent) {
    if (!comparing && !lensKey) return;
    const combo = comboOf(e);
    const a = combo ? viewerKeys.get(combo) : undefined;
    if (a === "compare") comparing = false;
    if (a === "loupe" || (lensKey && combo && combo.toLowerCase() === "z")) {
      lensKey = false;
      viewer?.setLens(false);
    }
  }
  function onBlur() {
    comparing = false;
    if (lensKey) {
      lensKey = false;
      viewer?.setLens(false);
    }
  }

  function openRecent(path: string) {
    recentsOpen = false;
    openPath(path);
  }

  function dismissCrash() {
    if (diag) diag.previousCrash = null;
    invoke("dismiss_crash_notice");
  }

  // Control por IA: Rust no atiende a la IA hasta saber si el usuario lo permite.
  $effect(() => {
    if (prefs.loaded) invoke("mcp_enable", { enabled: prefs.settings.aiControl }).catch((e) => log.warn("control por IA:", e));
  });

  function stopAi() {
    prefs.settings.aiControl = false;
    prefs.saveSettings();
    say(t("aiStopped"));
  }

  onMount(() => {
    invoke<DiagnosticsInfo>("diagnostics_info").then((d) => (diag = d));
    const stopMcp = startMcp({
      source: () => source,
      index: () => index,
      store,
      open: openPath,
      go,
      refAt: (i) => ref(source!, i),
      detectedPanels: panelsFor,
      sequence,
      play: (i) => {
        index = i;
        startPlay();
      },
      stop: () => viewer?.stop(),
      playing: () => !!playerInfo,
      say,
    });
    // Primero los ajustes y recientes: abrir la ruta inicial necesita saber dónde se quedó.
    prefs.load().then(() => invoke<string | null>("initial_path")).then((p) => {
      if (p) openPath(p);
    });
    const flush = () => {
      void store.flush();
      prefs.flush();
    };
    window.addEventListener("beforeunload", flush);
    // Cerrar la ventana: guardar de verdad (esperando) y avisar si no se pudo.
    const unClose = getCurrentWindow().onCloseRequested(async (e) => {
      if (!(await readyToLeave("quit"))) return e.preventDefault();
      prefs.flush();
    });
    // macOS: archivo abierto desde Finder con la app ya en marcha.
    void syncFullscreen();
    const unResize = getCurrentWindow().onResized(() => void syncFullscreen());
    const unOpen = listen<string>("open-path", (e) => {
      void invoke("initial_path"); // ya atendido: que no se vuelva a abrir al recargar
      openPath(e.payload);
    });
    const un = getCurrentWebview().onDragDropEvent((ev) => {
      if (ev.payload.type !== "drop" || !ev.payload.paths.length) return;
      const packs = ev.payload.paths.filter((p) => p.toLowerCase().endsWith(".imgxpack"));
      const audio = ev.payload.paths.filter(isAudioPath);
      if (packs.length) importPacks(packs);
      else if (audio.length) importSounds(audio);
      else openPath(ev.payload.paths[0]);
    });
    return () => {
      stopMcp();
      window.removeEventListener("beforeunload", flush);
      un.then((f) => f());
      unOpen.then((f) => f());
      unClose.then((f) => f());
      unResize.then((f) => f());
    };
  });
</script>

<svelte:window onkeydown={onKey} onkeyup={onKeyUp} onblur={onBlur} />

<main class:clean class:bars-off={barsHidden} class:idle={clean && pointerIdle && barsHidden} onpointermove={onPointerMove}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header bind:this={headerEl} onpointerenter={() => (overBars = true)} onpointerleave={() => { overBars = false; if (clean) wakeBars(); }}>
    {#if shows("open")}
      <button data-tour="open" onclick={pickFile} title="{t('openFile')}{kt('openFile')}">{t("openFile")}</button>
      <button data-tour="open" onclick={pickFolder} title="{t('open')}{kt('openFolder')}">{t("open")}</button>
    {/if}
    <div class="menu">
      <button data-tour="open" onclick={() => (recentsOpen = !recentsOpen)} class:on={recentsOpen} title="{t('recents')}{kt('recents')}">🕘 ▾</button>
      {#if recentsOpen}
        <div class="backdrop" role="presentation" onclick={() => (recentsOpen = false)}></div>
        <div class="popup">
          <Recents onopen={openRecent} current={source?.path} />
        </div>
      {/if}
    </div>
    <span class="title">{source?.title ?? ""}</span>
    {#if source}
      <button class="close-src" onclick={closeSource} disabled={opening} title="{t('closeSource')}{kt('closeSource')}">✕</button>
    {/if}
    <span class="spacer"></span>
    <button onclick={() => pageStep(-1)} disabled={!source || spreadNow[0] === 0} title="{t('prev')}{kt('prev')}">◀</button>
    <span class="counter">{source && source.entries.length ? `${spreadNow.map((i) => i + 1).join("–")} / ${source.entries.length}` : "–"}</span>
    <button onclick={() => pageStep(1)} disabled={!source || spreadNow[spreadNow.length - 1] >= source.entries.length - 1} title="{t('next')}{kt('next')}">▶</button>
    {#if shows("zoom")}
      <button onclick={zoomOut} title="{label('kv.zoomOut')}{kt('zoomOut')}">−</button>
      <span class="zoom">{status?.image ? `${status.zoomPercent}%` : ""}</span>
      <button onclick={zoomIn} title="{label('kv.zoomIn')}{kt('zoomIn')}">+</button>
    {/if}
    {#if shows("fit")}<button onclick={() => viewer.fit()} title="{t('fit')}{kt('fit')}">{t("fit")}</button>{/if}
    {#if shows("actual")}<button onclick={() => viewer.actualSize()} title="{t('actual')}{kt('actual')}">1:1</button>{/if}
    {#if shows("rotate")}<button onclick={() => viewer.rotate(90)} title="{t('rotate')}{kt('rotateCw')}">⟳</button>{/if}
    <button class="adj-btn" data-tour="adjust" onclick={() => (adjustOpen = !adjustOpen)} class:on={adjustOpen} class:dot={!!curAdjust || prefs.settings.night}
      disabled={!source} title="{t('adjButton')}{kt('adjust')}">🎨</button>
    <div class="menu">
      <button class="adj-btn read-btn" data-tour="reading" onclick={() => (readingOpen = !readingOpen)} class:on={readingOpen}
        class:dot={rd.fit !== "window" || rd.crop || rd.double || rd.strip || rd.rtl} disabled={!source} title="{t('readButton')}{kt('reading')}">📖</button>
      {#if readingOpen && source}
        <div class="backdrop" role="presentation" onclick={() => (readingOpen = false)}></div>
        <div class="popup reading-popup">
          <ReadingMenu {rd} marks={markList} ongo={(i) => { readingOpen = false; go(i); }} ongoto={() => { readingOpen = false; gotoOpen = true; }}
            onchange={(p) => changeReading(p)} kt={(a) => kt(a)} />
        </div>
      {/if}
    </div>
    {#if shows("thumbs")}<button onclick={toggleThumbs} class:on={prefs.settings.showThumbs} title="{t('thumbs')}{kt('thumbs')}">▦</button>{/if}
    {#if shows("action")}
      <button data-tour="action" onclick={() => (actionOpen = !actionOpen)} class:on={actionOpen} disabled={!source} title="{t('action')}{kt('action')}">🎬 {t("action")}</button>
    {/if}
    {#if shows("editor")}
      <button data-tour="editor" onclick={() => (editorOpen = !editorOpen)} class:on={editorOpen} disabled={!source} title="{t('editor')}{kt('editor')}">✎ {t("editor")}</button>
    {/if}
    {#if shows("export")}
      <button data-tour="export" onclick={openExport} disabled={!source?.entries.length} title="{t('exportVideo')}{kt('exportVideo')}">🎞 {t("exportVideoShort")}</button>
    {/if}
    <McpBadge onstop={stopAi} />
    {#if shows("fullscreen")}<button onclick={toggleFullscreen} class:on={fullscreen} title="{t('fullscreen')}{kt('fullscreen')}">⛶</button>{/if}
    {#if folded > 0}
      <button bind:this={moreBtn} data-tour="more" class:on={!!moreMenu} onclick={openMore} title={t("moreButtons")} aria-haspopup="menu">⋯</button>
    {/if}
    <button data-tour="settings" onclick={() => { settingsTab = "general"; settingsOpen = true; }} title="{t('settings')}{kt('settings')}">⚙</button>
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
  <section class="stage" class:framed={!!frame} bind:clientWidth={stageW} bind:clientHeight={stageH}>
    <div class="frame" class:framed={!!frame} style:left={frame ? `${frame.x}px` : null} style:top={frame ? `${frame.y}px` : null}
      style:width={frame ? `${frame.w}px` : null} style:height={frame ? `${frame.h}px` : null}>
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
      onmanual={() => editor?.notifyManual()}
      {editPanels}
      adjust={comparing ? null : adjustAt}
      layout={readLayout}
      {framing}
      onsize={onSize}
      onindex={(i) => (index = i)}
      onselect={(b) => (selection = b)}
      oncontext={(x, y) => source?.entries.length && (viewMenu = { x, y })}
    />
    {#if frame}<span class="frame-tag" title={t("frameHelp")}>{t(frame.video ? "frameTagVideo" : "frameTag", { w: frame.pw, h: frame.ph })}</span>{/if}
    </div>
    {#if adjustOpen && source}
      <AdjustPanel {store} entry={curEntry} size={curSize} {analyzing} onclose={() => (adjustOpen = false)} />
    {/if}
    {#if infoOpen && source?.entries[index]}
      <InfoPanel generation={source.generation} {index} count={source.entries.length} name={source.entries[index].name}
        bytes={source.entries[index].size} pixels={status?.image ?? null} source={source.path} onreveal={reveal} onclose={() => (infoOpen = false)} />
    {/if}
    {#if selection}
      <div class="sel-bar" role="toolbar">
        <span>✂ {t("selTitle")}</span>
        <button onclick={copyImage}>📋 {t("menuCopy")}</button>
        <button onclick={openSave}>💾 {t("selSave")}</button>
        <button onclick={() => viewer.clearSelection()} title={t("close")}>✕</button>
      </div>
    {/if}
    {#if gotoOpen && source}
      <GoToPage count={source.entries.length} current={index} ongo={go} onclose={() => (gotoOpen = false)} />
    {/if}
    {#if webtoonAsk}
      <div class="ask" role="alert">
        <span>{t("readWebtoonAsk")}</span>
        <button class="primary" onclick={() => changeReading({ strip: true, asked: true })}>{t("readWebtoonYes")}</button>
        <button onclick={() => setReading({ asked: true })}>{t("readWebtoonNo")}</button>
      </div>
    {/if}
    {#if prefs.settings.progressBar && source && source.entries.length > 1 && !playerInfo}
      <ProgressBar count={source.entries.length} pages={spreadNow} marks={[...markSet]} rtl={rd.rtl} ongo={go} />
    {/if}
    {#if panelEdit}
      <div class="panel-hint">{t("panelEditHint")}</div>
    {:else if toast}
      <div class="panel-hint toast">{toast}</div>
    {/if}
    {#if playerInfo && source}
      <PlayerBar info={playerInfo} count={source.entries.length} player={() => viewer.playback()} onstop={() => viewer.stop()} onmute={toggleMute} />
    {/if}
    {#if opening}
      <div class="hint">{t("loading")}</div>
    {:else if openError}
      <div class="hint">{openError}</div>
    {:else if !source}
      <div class="start" class:shelf-on={prefs.recents.length && prefs.settings.startView === "covers"}>
        <p>{t("openHint")}</p>
        <div class="start-open">
          <button onclick={pickFile}>{t("openFile")}</button>
          <button onclick={pickFolder}>{t("open")}</button>
        </div>
        {#if prefs.recents.length}
          <div class="start-head">
            <h4>{t("recents")}</h4>
            <div class="seg" role="radiogroup" aria-label={t("shelfView")}>
              <button class:on={prefs.settings.startView === "covers"} onclick={() => { prefs.settings.startView = "covers"; prefs.saveSettings(); }}>▦ {t("shelfCovers")}</button>
              <button class:on={prefs.settings.startView === "list"} onclick={() => { prefs.settings.startView = "list"; prefs.saveSettings(); }}>☰ {t("shelfList")}</button>
            </div>
          </div>
          {#if prefs.settings.startView === "covers"}
            <div class="start-shelf"><Shelf onopen={openRecent} /></div>
          {:else}
            <div class="start-list"><Recents onopen={openRecent} /></div>
          {/if}
        {/if}
        <button class="start-keys" onclick={() => (helpOpen = true)}>⌨️ {t("startKeysHint")}</button>
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
      entry={curEntry}
      size={curSize}
      detected={detectedCount}
      playing={!!playerInfo}
      onplay={togglePlay}
      onclose={() => (actionOpen = false)}
      {panelEdit}
      manualPanels={manualPanels?.length ?? null}
      onpaneledit={togglePanelEdit}
      onresetpanels={() => { finishPanelEdit(); if (curEntry && curSize) store.setPanels(curEntry, curSize, null); }}
      oneditor={() => (editorOpen = true)}
      entries={entryNames}
      {index}
    />
  {/if}
  </div>

  {#if editorShown && curEntry}
    <div class="editor-wrap" bind:offsetHeight={editorH}>
    <Editor
      bind:this={editor}
      {store}
      entry={curEntry}
      size={curSize}
      viewer={{ currentPose: (f) => viewer.currentPose(f), setPoseNow: (p) => viewer.setPoseNow(p), setFxNow: (tr, t) => viewer.setFxNow(tr, t), pickPoint: () => viewer.pickPoint(), cancelPick: () => viewer.cancelPick() }}
      generate={editorTemplate}
      onclose={() => (editorOpen = false)}
      {index}
      entries={entryNames}
    />
    </div>
  {/if}

  {#if prefs.settings.showThumbs && source && source.entries.length > 1}
    <div class="thumbs-wrap" bind:offsetHeight={thumbsH} role="presentation"
      onpointerenter={() => (overBars = true)} onpointerleave={() => { overBars = false; if (clean) wakeBars(); }}>
    <Thumbs
      count={source.entries.length}
      current={index}
      url={thumbUrl}
      name={(i) => source!.entries[i].name}
      onpick={go}
      size={prefs.settings.thumbSize}
      marks={markSet}
      shown={spreadNow}
    />
    </div>
  {/if}
</main>

{#if settingsOpen}
  <Settings initialTab={settingsTab} onclose={() => (settingsOpen = false)} />
{/if}

{#if exportOpen && source}
  <ExportDialog
    title={source.title}
    dir={source.kind === "folder" ? source.path : dirOf(source.path)}
    count={source.entries.length}
    {index}
    refAt={(i) => ref(source!, i)}
    spec={sequence}
    os={diag?.os ?? ""}
    mix={store.mix}
    onclose={() => (exportOpen = false)}
  />
{/if}

{#if fileDialog.request}
  {#key fileDialog.request}<FileDialog req={fileDialog.request} />{/key}
{/if}

{#if confirmSpec}<Confirm spec={confirmSpec} onanswer={answer} />{/if}

{#if viewMenu}<ViewMenu x={viewMenu.x} y={viewMenu.y} items={viewMenuItems} onclose={() => (viewMenu = null)} />{/if}
{#if moreMenu}<ViewMenu x={moreMenu.x} y={moreMenu.y} items={moreItems} onclose={() => (moreMenu = null)} />{/if}

{#if helpOpen}
  <KeysHelp onclose={() => (helpOpen = false)} onedit={() => { helpOpen = false; settingsTab = "keys"; settingsOpen = true; }} />
{/if}

<Tour />

{#if saveZone && source?.entries[index]}
  <SaveImage original={(/\.([^.]+)$/.exec(source.entries[index].name)?.[1] ?? "").toUpperCase()} canVisible={!!visibleAtSave}
    canSelection={!!selection} adjusted={!!curAdjust} zone={saveZone} onsave={doSave} oncancel={() => (saveZone = null)} />
{/if}

<style>
  main { position: relative; display: flex; flex-direction: column; height: 100%; }
  /* Pantalla completa limpia: las barras flotan sobre la imagen y se esconden solas. */
  main.clean header, main.clean .thumbs-wrap {
    position: absolute; left: 0; right: 0; z-index: 30; transition: transform 0.22s ease, opacity 0.22s ease;
  }
  main.clean header { top: 0; box-shadow: 0 4px 18px rgb(0 0 0 / 0.4); }
  main.clean .thumbs-wrap { bottom: 0; }
  main.bars-off header { transform: translateY(-100%); opacity: 0; pointer-events: none; }
  main.bars-off .thumbs-wrap { transform: translateY(100%); opacity: 0; pointer-events: none; }
  main.idle, main.idle :global(*) { cursor: none !important; }
  .thumbs-wrap { flex: none; }
  .start-keys { margin-top: 10px; background: none; border-color: transparent; color: var(--muted); font-size: 12px; }
  .start-keys:hover { color: var(--text); }
  header {
    display: flex; align-items: center; gap: 6px; padding: 6px 10px;
    background: var(--panel); border-bottom: 1px solid var(--border); user-select: none;
  }
  /* Los botones no se encogen ni parten su texto: si no caben, pasan al menú «⋯». */
  header > :global(*) { flex-shrink: 0; }
  header :global(button) { white-space: nowrap; }
  .title { flex-shrink: 1; min-width: 0; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .close-src { padding: 0 6px; background: none; border-color: transparent; color: var(--muted); flex-shrink: 0; }
  .close-src:hover:not(:disabled) { color: var(--text); border-color: var(--border); }
  .spacer { flex: 1 1 0; }
  .counter, .zoom { min-width: 64px; text-align: center; font-variant-numeric: tabular-nums; color: var(--muted); }
  .body { flex: 1; min-height: 0; display: flex; }
  .stage { position: relative; flex: 1; min-width: 0; }
  .stage.framed { background: repeating-linear-gradient(135deg, #07080a 0 10px, #0d0f13 10px 20px); }
  .frame { position: absolute; inset: 0; }
  .frame.framed { right: auto; bottom: auto; box-shadow: 0 0 0 1px rgb(251 191 36 / 0.7); }
  .frame-tag {
    position: absolute; left: 6px; bottom: 6px; padding: 1px 6px; border-radius: 4px; font-size: 11px; pointer-events: none;
    background: rgb(0 0 0 / 0.6); color: #fde68a;
  }
  .editor-wrap { flex: none; }
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
  /* Un punto en 🎨 cuando la imagen se ve retocada (para no olvidarlo). */
  .adj-btn { position: relative; }
  .adj-btn.dot::after {
    content: ""; position: absolute; top: 3px; right: 3px; width: 6px; height: 6px; border-radius: 50%; background: #fbbf24;
  }
  .read-btn.dot::after { background: var(--accent); }
  .reading-popup { width: 330px; left: auto; right: 0; }
  .ask {
    position: absolute; left: 50%; top: 10px; transform: translateX(-50%); z-index: 4; display: flex; align-items: center; gap: 8px;
    padding: 6px 10px; border-radius: 8px; background: var(--float); border: 1px solid var(--accent); font-size: 13px;
    box-shadow: var(--shadow);
  }
  .ask button { padding: 2px 10px; font-size: 12px; }
  .sel-bar {
    position: absolute; left: 50%; bottom: 22px; transform: translateX(-50%); z-index: 4; display: flex; align-items: center; gap: 6px;
    padding: 5px 8px; border-radius: 8px; background: var(--float); border: 1px solid #fbbf24; font-size: 12.5px;
    box-shadow: var(--shadow);
  }
  .sel-bar button { padding: 2px 9px; font-size: 12px; }
  .ask .primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
  .menu { position: relative; }
  .menu .backdrop { position: fixed; inset: 0; z-index: 9; }
  .popup {
    position: absolute; top: calc(100% + 6px); left: 0; z-index: 10; width: 440px; max-height: 60vh; overflow-y: auto;
    padding: 6px; background: var(--panel); border: 1px solid var(--border); border-radius: 8px; box-shadow: var(--shadow);
  }
  .start {
    position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 8px; padding: 20px; color: var(--muted); font-size: 16px;
  }
  .start p { margin: 0 0 12px; }
  .start h4 { margin: 0; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; }
  .start-list { width: min(560px, 100%); max-height: 50vh; overflow-y: auto; font-size: 14px; color: var(--text); }
  .start.shelf-on { justify-content: flex-start; overflow-y: auto; padding-top: 28px; }
  .start-open { display: flex; gap: 8px; margin: -4px 0 8px; }
  .start-open button { font-size: 13px; }
  .start-head { width: min(560px, 100%); display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  .shelf-on .start-head, .start-shelf { width: min(900px, 100%); }
  .start-shelf { color: var(--text); font-size: 14px; padding-bottom: 20px; }
  .start-head .seg { display: flex; }
  .start-head .seg button { border-radius: 0; font-size: 12px; padding: 3px 9px; }
  .start-head .seg button:first-child { border-radius: 6px 0 0 6px; }
  .start-head .seg button:last-child { border-radius: 0 6px 6px 0; border-left: none; }
  .start-head .seg button.on { background: var(--accent); color: #04121b; border-color: var(--accent); }
  .panel-hint {
    position: absolute; left: 50%; top: 10px; transform: translateX(-50%); padding: 4px 12px; border-radius: 6px; pointer-events: none;
    background: rgb(0 0 0 / 0.7); border: 1px solid #fbbf24; color: #fde68a; font-size: 12px; white-space: nowrap;
  }
  .toast { border-color: var(--accent); color: var(--text); }
  .diag .name { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
