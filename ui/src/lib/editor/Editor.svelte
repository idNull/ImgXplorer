<script module lang="ts">
  import type { Pose } from "../core";

  /** Lo que el editor necesita del visor. */
  export interface ViewerApi {
    currentPose(final?: boolean): Pose;
    setPoseNow(p: Pose): void;
  }
</script>

<script lang="ts">
  // Editor de timeline de la imagen actual: pista de cámara con clips y claves, cabezal,
  // captura de la vista (K), grabación del movimiento (G), biblioteca e inspector.
  import { onDestroy, untrack } from "svelte";
  import { wasm } from "../core";
  import type { ProjectStore } from "../action/project.svelte";
  import { label, t } from "../i18n.svelte";
  import { INSPECTOR_WIDTH, LIB_WIDTH, prefs } from "../settings.svelte";
  import { display, keysFor, type EditorAction, type ViewerAction } from "../keys";
  import {
    addKeyframe, bakeInstance, deleteKey, dragEdge, easePoints, end, inline, keyTimes, moveClip, moveKey, newInstance, nextName,
    poseObj, snap, snapPoints, timelineDuration, type ClipInstance, type Sample, type Timeline,
  } from "./model";
  import { defaultDuration, library, type LibClip } from "./library.svelte";
  import Inspector from "./Inspector.svelte";
  import Library from "./Library.svelte";
  import { log } from "../log";

  interface Props {
    store: ProjectStore;
    entry: string;
    size: [number, number] | null;
    viewer: ViewerApi;
    /** Timeline que genera la plantilla de esta imagen (`null` si no usa plantilla). */
    generate: () => Promise<wasm.WasmTimeline | null>;
    onclose: () => void;
  }
  let { store, entry, size, viewer, generate, onclose }: Props = $props();

  /** Margen a la izquierda del segundo 0 (px). */
  const PAD = 12;
  const SNAP_PX = 8;
  /** Lo que avanza el cabezal tras añadir una clave al final. */
  const STEP = 1;

  type Sel = { clip: number; key?: number } | null;

  let playhead = $state(0);
  let playing = $state(false);
  let recording = $state(false);
  /** La vista no sigue la timeline (el usuario ha movido la cámara). */
  let free = $state(true);
  let sel = $state<Sel>(null);
  let pps = $state(80);
  let notice = $state<string | null>(null);
  let generated = $state<Timeline | null>(null);
  let lanes: HTMLDivElement;
  let libDrag = $state<{ clip: LibClip; x: number; y: number; t: number | null } | null>(null);

  const item = $derived(store.item(entry));
  const own = $derived<Timeline | null>(item?.timeline?.camera?.length ? item.timeline : null);
  const tpl = $derived(store.templateFor(entry));
  const view = $derived<Timeline>(own ?? generated ?? { camera: [] });
  const fromTemplate = $derived(!own && !!generated);
  const duration = $derived(timelineDuration(view));
  const width = $derived(Math.max(duration + 4, 12) * pps + PAD * 2);

  const ek = (a: EditorAction) => keysFor(prefs.settings.keys, "editor", a).slice(0, 1).map(display).join("");
  const vk = (a: ViewerAction) => keysFor(prefs.settings.keys, "viewer", a).slice(0, 1).map(display).join("");
  const tip = (text: string, k: string) => (k ? `${text} (${k})` : text);

  // Timeline de la plantilla, para verla (y convertirla en clips al editar).
  $effect(() => {
    const json = own || !tpl ? null : JSON.stringify(tpl);
    void size?.[0];
    void size?.[1];
    void entry;
    if (!json) {
      generated = null;
      return;
    }
    let cancelled = false;
    // `generate` lee estado de la aplicación (p. ej. el del visor, que cambia con cada
    // movimiento de cámara): fuera del seguimiento, o esto se volvería a ejecutar sin parar.
    untrack(generate)
      .then((w) => {
        if (!w) return void (generated = null);
        const tl = JSON.parse(w.to_json()) as Timeline;
        w.free();
        // Igual que lo que ya hay: no se toca (evita rehacer el evaluador y mover la cámara).
        // Se compara en el formato de JS (Rust escribe `1.0` donde JS escribe `1`).
        if (!cancelled && JSON.stringify(tl) !== untrack(() => JSON.stringify(generated))) generated = tl;
      })
      .catch((e) => log.warn("editor: no se pudo generar la plantilla:", e));
    return () => {
      cancelled = true;
    };
  });

  // Evaluador WASM de lo que se ve; se rehace en cada cambio.
  let evalTl: wasm.WasmTimeline | null = null;
  $effect(() => {
    const json = JSON.stringify(view);
    const w = view.camera.length ? new wasm.WasmTimeline(json) : null;
    untrack(() => {
      evalTl?.free();
      evalTl = w;
      if (!free && !recording) showAt(playhead);
    });
  });

  // Otra imagen: se empieza de cero.
  $effect(() => {
    void entry;
    untrack(() => {
      stopRecording(false);
      pause();
      playhead = 0;
      sel = null;
      free = true;
    });
  });

  onDestroy(() => {
    stopRecording(false);
    pause();
    evalTl?.free();
    evalTl = null;
    window.removeEventListener("pointermove", onDragMove);
    window.removeEventListener("pointerup", onDragUp);
  });

  function say(msg: string) {
    notice = msg;
  }

  function showAt(t: number) {
    if (!evalTl) return;
    viewer.setPoseNow(evalTl.evaluate(t));
    free = false;
  }

  function seek(t: number) {
    playhead = Math.max(0, t);
    showAt(playhead);
  }

  /**
   * Aplica un cambio a la timeline propia. Si la imagen usaba una plantilla, primero se
   * convierte en clips editables (se avisa; se puede deshacer).
   */
  function edit(fn: (tl: Timeline) => void): boolean {
    if (!size || store.readOnly) {
      if (store.readOnly) say(t("projectReadOnly", { error: store.readOnly }));
      return false;
    }
    if (own) {
      fn(own);
      if (own.camera.length) store.changed();
      else store.setTimeline(entry, size, null);
      return true;
    }
    const base: Timeline = generated ? JSON.parse(JSON.stringify(generated)) : { camera: [] };
    if (generated && tpl) {
      for (const c of base.camera) c.name ??= label(`tpl.${tpl.id}`);
      say(t("baked"));
    }
    fn(base);
    store.setTimeline(entry, size, base);
    return true;
  }

  // --- reproducción de la vista previa ---------------------------------------------------
  let raf = 0;
  let last = 0;
  function play() {
    if (!view.camera.length || recording) return;
    if (playhead >= duration - 0.01) playhead = 0;
    playing = true;
    last = performance.now();
    showAt(playhead);
    raf = requestAnimationFrame(tick);
  }
  function tick(now: number) {
    if (!playing) return;
    playhead = Math.min(duration, playhead + (now - last) / 1000);
    last = now;
    showAt(playhead);
    if (playhead >= duration) return void (playing = false);
    raf = requestAnimationFrame(tick);
  }
  function pause() {
    playing = false;
    cancelAnimationFrame(raf);
  }

  /** El usuario ha movido la cámara a mano. */
  export function notifyManual() {
    free = true;
    if (playing) pause();
  }

  // --- claves y grabación ----------------------------------------------------------------
  function keyframe() {
    if (recording) return;
    pause();
    const atEnd = playhead >= duration - 0.01;
    const pose = poseObj(viewer.currentPose());
    let ref: { clip: number; key: number } | null = null;
    if (!edit((tl) => (ref = addKeyframe(tl, playhead, pose)))) return;
    sel = ref;
    free = false;
    if (atEnd) playhead += STEP;
  }

  let rec: { start: number; t0: number; flat: number[]; raf: number } | null = null;
  function startRecording() {
    if (!size || store.readOnly) return;
    pause();
    rec = { start: playhead, t0: performance.now(), flat: [], raf: 0 };
    recording = true;
    free = true;
    say(t("recordingHelp", { key: ek("record"), cancel: ek("cancel") }));
    const step = (now: number) => {
      if (!rec) return;
      const el = (now - rec.t0) / 1000;
      rec.flat.push(el, ...viewer.currentPose(false));
      playhead = rec.start + el;
      rec.raf = requestAnimationFrame(step);
    };
    rec.raf = requestAnimationFrame(step);
  }

  function stopRecording(keep: boolean) {
    const r = rec;
    if (!r) return;
    rec = null;
    recording = false;
    cancelAnimationFrame(r.raf);
    notice = null;
    if (!keep) {
      playhead = r.start;
      return;
    }
    if (r.flat.length < 10) return say(t("recordingShort"));
    const out = wasm.simplify_recording(new Float64Array(r.flat), 0);
    const samples: Sample[] = [];
    for (let i = 0; i + 4 < out.length; i += 5) samples.push({ t: out[i], pose: poseObj(out.subarray(i + 1, i + 5)) });
    const last = samples.at(-1)!.t;
    if (last < 0.05) return say(t("recordingShort"));
    let idx = -1;
    edit((tl) => {
      const name = nextName(t("recName"), tl.camera.map((c) => c.name));
      tl.camera.push(newInstance({ type: "recorded", samples }, r.start, name));
      idx = tl.camera.length - 1;
    });
    if (idx >= 0) sel = { clip: idx };
    say(t("recorded", { n: samples.length, s: last.toFixed(1) }));
  }

  function removeSelection() {
    const s = sel;
    if (!s) return;
    sel = null;
    edit((tl) => {
      if (s.key !== undefined) deleteKey(tl, { clip: s.clip, key: s.key });
      else tl.camera.splice(s.clip, 1);
    });
  }

  function duplicate() {
    const s = sel;
    if (!s) return;
    let idx = -1;
    edit((tl) => {
      const src = tl.camera[s.clip];
      const copy: ClipInstance = JSON.parse(JSON.stringify(src));
      copy.start = end(src);
      tl.camera.push(copy);
      idx = tl.camera.length - 1;
    });
    if (idx >= 0) sel = { clip: idx };
  }

  function saveToLibrary() {
    const inst = sel ? view.camera[sel.clip] : undefined;
    const clip = inst && bakeInstance($state.snapshot(inst) as ClipInstance);
    if (!clip) return;
    const name = inst.name ?? nextName(t("clipDefault"), library.clips.map((c) => c.name));
    library.addClip(name, clip);
    say(t("savedToLibrary", { name }));
  }

  function addFromLibrary(c: LibClip, at?: number) {
    let idx = -1;
    edit((tl) => {
      tl.camera.push(newInstance(JSON.parse(JSON.stringify(c.clip)), at ?? timelineDuration(tl), c.name, defaultDuration(c.clip)));
      idx = tl.camera.length - 1;
    });
    if (idx >= 0) sel = { clip: idx };
  }

  function mark(dir: 1 | -1) {
    if (!evalTl) return;
    const marks = Array.from(evalTl.marks());
    const m = dir > 0 ? marks.find((x) => x > playhead + 0.01) : marks.filter((x) => x < playhead - 0.01).at(-1);
    pause();
    seek(m ?? (dir > 0 ? duration : 0));
  }

  /** Teclas del grupo "editor". Devuelve `true` si la ha usado. */
  export function handle(action: EditorAction): boolean {
    switch (action) {
      case "toggle":
        if (recording) stopRecording(true);
        else if (playing) pause();
        else play();
        return true;
      case "keyframe":
        keyframe();
        return true;
      case "record":
        if (recording) stopRecording(true);
        else startRecording();
        return true;
      case "remove":
        if (!sel) return false;
        removeSelection();
        return true;
      case "markPrev":
        mark(-1);
        return true;
      case "markNext":
        mark(1);
        return true;
      case "cancel":
        if (recording) stopRecording(false);
        else if (playing) pause();
        else if (sel) sel = null;
        else return false;
        return true;
    }
  }

  // --- arrastres en la pista -------------------------------------------------------------
  type Drag =
    | { kind: "scrub" }
    | { kind: "move"; idx: number; offset: number }
    | { kind: "edge"; idx: number; edge: "start" | "end" }
    | { kind: "key"; idx: number; key: number };
  let drag: Drag | null = null;
  let downAt = { x: 0, y: 0 };
  let moved = $state(false);

  function timeAt(clientX: number): number {
    const r = lanes.getBoundingClientRect();
    return (clientX - r.left + lanes.scrollLeft - PAD) / pps;
  }

  function beginDrag(d: Drag, e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    pause();
    drag = d;
    downAt = { x: e.clientX, y: e.clientY };
    moved = false;
    if (d.kind !== "scrub") store.batch(true);
    window.addEventListener("pointermove", onDragMove);
    window.addEventListener("pointerup", onDragUp);
    if (d.kind === "scrub") seek(timeAt(e.clientX));
  }

  function onDragMove(e: PointerEvent) {
    if (libDrag) return onLibMove(e);
    const d = drag;
    if (!d) return;
    if (!moved && Math.hypot(e.clientX - downAt.x, e.clientY - downAt.y) < 3) return;
    moved = true;
    const t = timeAt(e.clientX);
    const thr = SNAP_PX / pps;
    if (d.kind === "scrub") return seek(t);
    edit((tl) => {
      const pts = snapPoints(tl, playhead, d.idx);
      const inst = tl.camera[d.idx];
      if (!inst) return;
      if (d.kind === "move") {
        // Se pega por el principio o por el final, lo que quede más cerca.
        const s = t - d.offset;
        const a = snap(s, pts, thr);
        const b = snap(s + inst.duration, pts, thr) - inst.duration;
        const da = a !== s ? Math.abs(a - s) : Infinity;
        const db = b !== s ? Math.abs(b - s) : Infinity;
        moveClip(tl, d.idx, da === Infinity && db === Infinity ? s : da <= db ? a : b);
      } else if (d.kind === "edge") {
        dragEdge(tl, d.idx, d.edge, snap(t, pts, thr), e.shiftKey ? "trim" : "stretch");
      } else {
        // El cabezal sigue a la clave: no sirve como punto de pegado.
        moveKey(tl, { clip: d.idx, key: d.key }, snap(t, pts.filter((p) => p !== playhead), thr));
      }
    });
    if (d.kind === "key") {
      const inst = own?.camera[d.idx];
      if (inst) seek(keyTimes(inst)[d.key] ?? playhead);
    }
  }

  function onDragUp() {
    window.removeEventListener("pointermove", onDragMove);
    window.removeEventListener("pointerup", onDragUp);
    if (libDrag) return onLibUp();
    if (drag && drag.kind !== "scrub") store.batch(false);
    drag = null;
  }

  function onClipDown(e: PointerEvent, idx: number, part: "body" | "start" | "end") {
    sel = { clip: idx };
    const inst = view.camera[idx];
    if (part === "body") beginDrag({ kind: "move", idx, offset: timeAt(e.clientX) - inst.start }, e);
    else beginDrag({ kind: "edge", idx, edge: part }, e);
  }

  function onKeyDown(e: PointerEvent, idx: number, key: number) {
    sel = { clip: idx, key };
    beginDrag({ kind: "key", idx, key }, e);
    seek(keyTimes(view.camera[idx])[key]);
  }

  function onLaneDown(e: PointerEvent) {
    sel = null;
    beginDrag({ kind: "scrub" }, e);
  }

  function onWheel(e: WheelEvent) {
    if (e.ctrlKey) {
      e.preventDefault();
      const t = timeAt(e.clientX);
      const r = lanes.getBoundingClientRect();
      pps = Math.min(600, Math.max(8, pps * Math.exp(-e.deltaY * 0.002)));
      lanes.scrollLeft = t * pps + PAD - (e.clientX - r.left);
    } else if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      e.preventDefault();
      lanes.scrollLeft += e.deltaY;
    }
  }

  function zoomTimeline(f: number) {
    pps = Math.min(600, Math.max(8, pps * f));
  }

  // Arrastrar desde la biblioteca (con eventos de puntero: el arrastre HTML no es fiable en
  // todos los webviews).
  function startLibDrag(c: LibClip, e: PointerEvent) {
    if (e.button !== 0) return;
    libDrag = { clip: c, x: e.clientX, y: e.clientY, t: null };
    downAt = { x: e.clientX, y: e.clientY };
    moved = false;
    window.addEventListener("pointermove", onDragMove);
    window.addEventListener("pointerup", onDragUp);
  }
  function onLibMove(e: PointerEvent) {
    if (!libDrag) return;
    if (!moved && Math.hypot(e.clientX - downAt.x, e.clientY - downAt.y) < 4) return;
    moved = true;
    const r = lanes.getBoundingClientRect();
    const over = e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
    const t = over ? snap(Math.max(0, timeAt(e.clientX)), snapPoints(view, playhead, -1), SNAP_PX / pps) : null;
    libDrag = { ...libDrag, x: e.clientX, y: e.clientY, t };
  }
  function onLibUp() {
    const d = libDrag;
    libDrag = null;
    if (d && moved && d.t !== null) addFromLibrary(d.clip, d.t);
  }

  // Columnas redimensionables (biblioteca e inspector); el ancho se guarda en los ajustes.
  let libEl: HTMLElement;
  function startResize(which: "lib" | "inspector", e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    const x0 = e.clientX;
    const lim = which === "lib" ? LIB_WIDTH : INSPECTOR_WIDTH;
    const w0 = which === "lib" ? prefs.settings.libWidth : prefs.settings.inspectorWidth;
    const move = (ev: PointerEvent) => {
      const d = ev.clientX - x0;
      const w = Math.round(Math.min(lim.max, Math.max(lim.min, w0 + (which === "lib" ? d : -d))));
      if (which === "lib") prefs.settings.libWidth = w;
      else prefs.settings.inspectorWidth = w;
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      prefs.saveSettings();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  /** Ensancha la biblioteca lo justo para leer el nombre más largo (o vuelve al ancho normal). */
  function fitLibrary() {
    const names = [...libEl.querySelectorAll<HTMLElement>(".name")];
    const need = Math.max(0, ...names.map((n) => n.scrollWidth - n.clientWidth));
    prefs.settings.libWidth = need > 1
      ? Math.min(LIB_WIDTH.max, prefs.settings.libWidth + need + 6)
      : LIB_WIDTH.def;
    prefs.saveSettings();
  }

  function toggleFrame() {
    prefs.settings.editorFrame = !prefs.settings.editorFrame;
    prefs.saveSettings();
  }

  const fmtTime = (s: number) => `${s.toFixed(2)} s`;
  const ticks = $derived.by(() => {
    const step = pps >= 160 ? 0.5 : pps >= 50 ? 1 : pps >= 20 ? 2 : 5;
    const out: number[] = [];
    for (let s = 0; s * pps <= width; s += step) out.push(Number(s.toFixed(2)));
    return out;
  });

  const ICON: Record<string, string> = { keyframes: "◆", recorded: "≈", hold: "⏸" };
  const TYPE: Record<string, () => string> = {
    keyframes: () => t("clipKeyframes"),
    recorded: () => t("clipRecorded"),
    hold: () => t("clipHold"),
  };
</script>

<section class="editor">
  <header>
    <b>{t("editor")}</b>
    <button onclick={() => (playing ? pause() : play())} disabled={!view.camera.length || recording} title={tip(playing ? t("pause") : t("play"), ek("toggle"))}>
      {playing ? "⏸" : "▶"}
    </button>
    <button onclick={() => (pause(), seek(0))} title={t("toStart")}>⏮</button>
    <span class="time">{fmtTime(playhead)} / {fmtTime(duration)}</span>
    <span class="sep"></span>
    <button onclick={keyframe} disabled={recording || !size} title={tip(t("addKeyframe"), ek("keyframe"))}>◆ {t("keyframeShort")}</button>
    <button class="rec" class:on={recording} onclick={() => (recording ? stopRecording(true) : startRecording())} disabled={!size}
      title={tip(recording ? t("stopRecording") : t("record"), ek("record"))}>● {recording ? t("stopRecording") : t("record")}</button>
    <span class="sep"></span>
    <button onclick={() => store.undo()} disabled={!store.canUndo} title={tip(t("undo"), vk("undo"))}>↶</button>
    <button onclick={() => store.redo()} disabled={!store.canRedo} title={tip(t("redo"), vk("redo"))}>↷</button>
    <span class="sep"></span>
    <button onclick={() => zoomTimeline(1 / 1.4)} title={t("zoomOutTl")}>−</button>
    <button onclick={() => zoomTimeline(1.4)} title={t("zoomInTl")}>+</button>
    <span class="sep"></span>
    <button class:on={prefs.settings.editorFrame} onclick={toggleFrame} title={t("frameHelp")}>▭ {t("frameToggle")}</button>
    {#if fromTemplate}
      <button onclick={() => edit(() => {})} title={t("bakeHelp")}>{t("bake")}</button>
    {:else if own}
      <button onclick={() => size && (store.setTimeline(entry, size, null), (sel = null))} title={t("clearOwnHelp")}>{t("clearOwn")}</button>
    {/if}
    <span class="status">
      {#if notice}{notice}{:else if free && view.camera.length}{t("freeView", { key: ek("keyframe") })}{/if}
    </span>
    <button class="close" onclick={onclose} title={tip(t("close"), vk("editor"))}>✕</button>
  </header>

  <div class="body">
    <aside class="library" bind:this={libEl} style:width="{prefs.settings.libWidth}px">
      <Library {store} {entry} {size} onadd={(c) => addFromLibrary(c)} ondrag={startLibDrag} onnotice={say} onfit={fitLibrary} />
    </aside>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="splitter" title={t("resizeHelp")} onpointerdown={(e) => startResize("lib", e)} ondblclick={fitLibrary}></div>

    <div class="tracks">
      <div class="labels">
        <div class="ruler-label"></div>
        <div class="lane-label">🎥 {t("trackCamera")}</div>
        <div class="lane-label off" title={t("soon")}>🎙 {t("trackVoice")}</div>
        <div class="lane-label off" title={t("soon")}>💥 {t("trackSfx")}</div>
        <div class="lane-label off" title={t("soon")}>🌧 {t("trackAmbient")}</div>
      </div>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="lanes" bind:this={lanes} onwheel={onWheel}>
        <div class="content" style:width="{width}px">
          <div class="ruler" onpointerdown={(e) => beginDrag({ kind: "scrub" }, e)}>
            {#each ticks as s (s)}
              <span class="tick" class:major={Number.isInteger(s)} style:left="{PAD + s * pps}px">{Number.isInteger(s) ? `${s}s` : ""}</span>
            {/each}
          </div>
          <div class="lane camera" class:template={fromTemplate} onpointerdown={onLaneDown}>
            {#each view.camera as inst, i (i)}
              {@const c = inline(inst)}
              {@const left = PAD + inst.start * pps}
              {@const w = Math.max(8, inst.duration * pps)}
              <div
                class="clip {c?.type ?? 'ref'}"
                class:selected={sel?.clip === i && sel.key === undefined}
                style:left="{left}px"
                style:width="{w}px"
                title="{inst.name ?? (c ? TYPE[c.type]() : '')} · {fmtTime(inst.start)} → {fmtTime(end(inst))}"
                onpointerdown={(e) => onClipDown(e, i, "body")}
              >
                {#if c?.type === "keyframes" && c.keys.length > 1}
                  {@const kts = keyTimes(inst)}
                  {@const sk = sel?.clip === i ? sel.key : undefined}
                  <svg class="curves" width={w} height="34" aria-hidden="true">
                    {#each c.keys.slice(0, -1) as k, j (j)}
                      {@const x0 = (kts[j] - inst.start) * pps}
                      {@const sw = (kts[j + 1] - kts[j]) * pps}
                      {#if sk !== undefined && (j === sk - 1 || j === sk)}
                        <rect class="seg" class:in={j === sk - 1} x={x0} y="0" width={Math.max(0, sw)} height="34" />
                      {/if}
                      {#if sw > 6}<polyline points={easePoints(k.easing, x0, sw, 16, 27, Math.min(48, Math.max(8, Math.round(sw / 3))))} />{/if}
                    {/each}
                  </svg>
                {/if}
                <span class="clip-name">{c ? ICON[c.type] : "?"} {inst.name ?? (c ? TYPE[c.type]() : "")}</span>
                <span class="edge start" onpointerdown={(e) => onClipDown(e, i, "start")}></span>
                <span class="edge end" onpointerdown={(e) => onClipDown(e, i, "end")}></span>
                {#each keyTimes(inst) as kt, k (k)}
                  <span
                    class="key"
                    class:selected={sel?.clip === i && sel.key === k}
                    style:left="{(kt - inst.start) * pps}px"
                    title="{t('keyframe')} {k + 1} · {fmtTime(kt)}"
                    onpointerdown={(e) => onKeyDown(e, i, k)}
                  ></span>
                {/each}
              </div>
            {/each}
            {#if !view.camera.length}
              <span class="empty">{t("editorEmpty", { k: ek("keyframe"), g: ek("record") })}</span>
            {/if}
            {#if libDrag?.t != null}
              <div class="drop" style:left="{PAD + libDrag.t * pps}px" style:width="{Math.max(8, defaultDuration(libDrag.clip.clip) * pps)}px"></div>
            {/if}
          </div>
          <div class="lane off"></div>
          <div class="lane off"></div>
          <div class="lane off"></div>
          <div class="playhead" class:rec={recording} style:left="{PAD + playhead * pps}px"></div>
        </div>
      </div>
    </div>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="splitter" title={t("resizeHelp")} onpointerdown={(e) => startResize("inspector", e)}
      ondblclick={() => ((prefs.settings.inspectorWidth = INSPECTOR_WIDTH.def), prefs.saveSettings())}></div>
    <aside class="inspector" style:width="{prefs.settings.inspectorWidth}px">
      {#if sel && view.camera[sel.clip]}
        <Inspector
          tl={view}
          clip={sel.clip}
          key={sel.key}
          {fromTemplate}
          {edit}
          currentPose={() => viewer.currentPose()}
          onseek={seek}
          onremove={removeSelection}
          onduplicate={duplicate}
          onsave={saveToLibrary}
        />
      {:else}
        <p class="note">{fromTemplate ? t("editorTemplate", { name: tpl ? label(`tpl.${tpl.id}`) : "" }) : t("editorHelp")}</p>
        <p class="note">{t("editorDragHelp")}</p>
      {/if}
    </aside>
  </div>
</section>

{#if libDrag && moved}
  <div class="ghost" style:left="{libDrag.x + 10}px" style:top="{libDrag.y + 10}px">{ICON[libDrag.clip.clip.type]} {libDrag.clip.name}</div>
{/if}

<style>
  .editor { display: flex; flex-direction: column; height: 250px; flex: none; background: var(--panel); border-top: 1px solid var(--border); font-size: 13px; }
  header { display: flex; align-items: center; gap: 6px; padding: 4px 10px; border-bottom: 1px solid var(--border); min-width: 0; }
  header button { padding: 2px 8px; }
  header button.on { border-color: var(--accent); color: var(--accent); }
  .time { font-variant-numeric: tabular-nums; color: var(--muted); min-width: 120px; }
  .sep { width: 1px; height: 18px; background: var(--border); }
  .rec.on { background: #7f1d1d; border-color: #ef4444; color: #fecaca; }
  .rec { color: #fca5a5; }
  .status { flex: 1; min-width: 0; color: #fbbf24; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .body { flex: 1; min-height: 0; display: flex; }
  .library { flex: none; padding: 6px 8px; min-height: 0; box-sizing: border-box; }
  .splitter { flex: none; width: 5px; margin: 0 -2px; z-index: 2; cursor: col-resize; position: relative; }
  .splitter::after { content: ""; position: absolute; top: 0; bottom: 0; left: 2px; width: 1px; background: var(--border); }
  .splitter:hover::after { background: var(--accent); width: 2px; left: 1.5px; }
  .inspector { flex: none; box-sizing: border-box; padding: 6px 10px; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; }
  .note { margin: 0; color: var(--muted); font-size: 12px; line-height: 1.45; }
  .tracks { flex: 1; min-width: 0; display: flex; }
  .labels { width: 100px; flex: none; border-right: 1px solid var(--border); }
  .ruler-label { height: 22px; border-bottom: 1px solid var(--border); }
  .lane-label { height: 44px; display: flex; align-items: center; padding: 0 8px; font-size: 12px; border-bottom: 1px solid rgb(42 47 56 / 0.6); }
  .lane-label.off { height: 26px; color: var(--muted); opacity: 0.6; }
  .lanes { flex: 1; min-width: 0; overflow-x: auto; overflow-y: hidden; position: relative; }
  .content { position: relative; min-height: 100%; }
  .ruler { height: 22px; position: relative; border-bottom: 1px solid var(--border); cursor: ew-resize; user-select: none; }
  .tick { position: absolute; bottom: 0; height: 5px; border-left: 1px solid var(--border); font-size: 10px; color: var(--muted); padding-left: 3px; line-height: 1; }
  .tick.major { height: 14px; }
  .lane { position: relative; height: 44px; border-bottom: 1px solid rgb(42 47 56 / 0.6); }
  .lane.off { height: 26px; background: repeating-linear-gradient(135deg, transparent 0 8px, rgb(255 255 255 / 0.02) 8px 16px); }
  .clip {
    position: absolute; top: 5px; height: 34px; box-sizing: border-box; border-radius: 6px; cursor: grab; user-select: none;
    background: rgb(56 189 248 / 0.18); border: 1px solid rgb(56 189 248 / 0.6); overflow: visible;
  }
  .clip.recorded { background: rgb(167 139 250 / 0.2); border-color: rgb(167 139 250 / 0.7); }
  .clip.hold { background: rgb(148 163 184 / 0.15); border-color: rgb(148 163 184 / 0.5); }
  .lane.template .clip { border-style: dashed; opacity: 0.75; }
  .clip.selected { border-color: #fbbf24; box-shadow: 0 0 0 1px #fbbf24; }
  .clip-name { position: absolute; left: 8px; right: 8px; top: 2px; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; pointer-events: none; }
  .curves { position: absolute; left: -1px; top: -1px; pointer-events: none; }
  .curves polyline { fill: none; stroke: rgb(186 230 253 / 0.45); stroke-width: 1.2; }
  .curves .seg { fill: rgb(251 191 36 / 0.16); }
  .curves .seg.in { fill: rgb(251 191 36 / 0.08); }
  .edge { position: absolute; top: 0; bottom: 0; width: 7px; cursor: ew-resize; }
  .edge.start { left: -2px; }
  .edge.end { right: -2px; }
  .key {
    position: absolute; bottom: 4px; width: 10px; height: 10px; margin-left: -5px; transform: rotate(45deg); cursor: pointer;
    background: var(--accent); border: 1px solid #0b0d10;
  }
  .key.selected { background: #fbbf24; }
  .empty { position: absolute; left: 16px; top: 13px; color: var(--muted); font-size: 12px; pointer-events: none; white-space: nowrap; }
  .drop { position: absolute; top: 5px; height: 34px; border: 2px dashed var(--accent); border-radius: 6px; pointer-events: none; }
  .playhead { position: absolute; top: 0; bottom: 0; width: 0; border-left: 2px solid #f87171; pointer-events: none; }
  .playhead.rec { border-left-color: #ef4444; box-shadow: 0 0 6px #ef4444; }
  .ghost {
    position: fixed; z-index: 30; pointer-events: none; padding: 2px 8px; font-size: 12px; border-radius: 6px;
    background: var(--panel-2); border: 1px solid var(--accent);
  }
</style>
