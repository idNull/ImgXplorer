<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { openFile, openFolder, fileDialog, dirOf } from "./lib/files/dialog.svelte";
  import FileDialog from "./lib/files/FileDialog.svelte";
  import ExportDialog from "./lib/export/ExportDialog.svelte";
  import type { ExportSpec } from "./lib/export/exporter";
  import { library } from "./lib/editor/libstore.svelte";
  import Viewer from "./lib/viewer/Viewer.svelte";
  import Thumbs from "./lib/Thumbs.svelte";
  import Settings from "./lib/Settings.svelte";
  import Recents from "./lib/Recents.svelte";
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
  import { t } from "./lib/i18n.svelte";
  import { log } from "./lib/log";
  import { startMcp } from "./lib/mcp/host.svelte";
  import McpBadge from "./lib/mcp/McpBadge.svelte";

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
  });
  const thumbUrl = (i: number) => `${base}t/${source!.generation}_${i}`;
  const current = $derived(source && source.entries.length ? ref(source, index) : null);
  const neighbours = $derived(
    source ? [index + 1, index - 1, index + 2].filter((i) => i >= 0 && i < source!.entries.length).map((i) => ref(source!, i)) : [],
  );

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
    ambient: (i) => ambientSpecs(store.ambients, i, indexOfEntry),
    tracks: () => trackSpecs($state.snapshot(store.tracks)),
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
    w.setFullscreen(!(await w.isFullscreen()));
  }

  function onKey(e: KeyboardEvent) {
    if (settingsOpen || fileDialog.request || exportOpen || confirmSpec) return; // los diálogos gestionan sus teclas
    const tgt = e.target;
    // En los campos de texto las teclas son para escribir. En listas, deslizadores y casillas
    // solo se quedan las teclas sueltas (flechas…): los atajos con Ctrl siguen funcionando.
    const textField = tgt instanceof HTMLTextAreaElement || (tgt instanceof HTMLInputElement && !["range", "checkbox", "radio", "color"].includes(tgt.type));
    if (textField) return;
    if ((tgt instanceof HTMLInputElement || tgt instanceof HTMLSelectElement) && !e.ctrlKey && !e.metaKey) return;
    if (recentsOpen && e.key === "Escape") {
      recentsOpen = false;
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
      case "editor": if (source) editorOpen = !editorOpen; break;
      case "exportVideo": openExport(); break;
      case "closeSource": closeSource(); break;
      case "mute": toggleMute(); break;
      case "undo": store.undo(); break;
      case "redo": store.redo(); break;
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
    {#if source}
      <button class="close-src" onclick={closeSource} disabled={opening} title="{t('closeSource')}{kt('closeSource')}">✕</button>
    {/if}
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
    <button onclick={() => (editorOpen = !editorOpen)} class:on={editorOpen} disabled={!source} title="{t('editor')}{kt('editor')}">✎ {t("editor")}</button>
    <button onclick={openExport} disabled={!source?.entries.length} title="{t('exportVideo')}{kt('exportVideo')}">🎞 {t("exportVideoShort")}</button>
    <McpBadge onstop={stopAi} />
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
    />
    {#if frame}<span class="frame-tag" title={t("frameHelp")}>{t(frame.video ? "frameTagVideo" : "frameTag", { w: frame.pw, h: frame.ph })}</span>{/if}
    </div>
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
      viewer={{ currentPose: (f) => viewer.currentPose(f), setPoseNow: (p) => viewer.setPoseNow(p) }}
      generate={editorTemplate}
      onclose={() => (editorOpen = false)}
      {index}
      entries={entryNames}
    />
    </div>
  {/if}

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

<style>
  main { display: flex; flex-direction: column; height: 100%; }
  header {
    display: flex; align-items: center; gap: 6px; padding: 6px 10px;
    background: var(--panel); border-bottom: 1px solid var(--border); user-select: none;
  }
  .title { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .close-src { padding: 0 6px; background: none; border-color: transparent; color: var(--muted); flex-shrink: 0; }
  .close-src:hover:not(:disabled) { color: var(--text); border-color: var(--border); }
  .spacer { flex: 1; }
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
  .panel-hint {
    position: absolute; left: 50%; top: 10px; transform: translateX(-50%); padding: 4px 12px; border-radius: 6px; pointer-events: none;
    background: rgb(0 0 0 / 0.7); border: 1px solid #fbbf24; color: #fde68a; font-size: 12px; white-space: nowrap;
  }
  .toast { border-color: var(--accent); color: var(--text); }
  .diag .name { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
