<script module lang="ts">
  import type { Pose } from "../core";
  import type { FxTrack } from "../fx/fx";

  /** Lo que el editor necesita del visor. */
  export interface ViewerApi {
    currentPose(final?: boolean): Pose;
    setPoseNow(p: Pose): void;
    /** Efectos especiales a mostrar en el segundo `t` (fuera de la reproducción). */
    setFxNow(track: FxTrack | null, t?: number): void;
    /** El siguiente clic en el visor: punto de la imagen (0..1) o `null` si se cancela. */
    pickPoint(): Promise<{ x: number; y: number } | null>;
    cancelPick(): void;
  }
</script>

<script lang="ts">
  // Editor de timeline de la imagen actual: pista de cámara con clips y claves, cabezal,
  // captura de la vista (K), grabación del movimiento (G), biblioteca e inspector.
  import { onDestroy, untrack } from "svelte";
  import { errorText } from "../errors";
  import { invoke } from "@tauri-apps/api/core";
  import { wasm } from "../core";
  import type { ProjectStore } from "../action/project.svelte";
  import { label, t } from "../i18n.svelte";
  import { EDITOR_HEIGHT, INSPECTOR_WIDTH, LIB_WIDTH, prefs, videoPixels, type FrameMode } from "../prefs.svelte";
  import { display, keysFor, type EditorAction, type ViewerAction } from "../keys";
  import {
    addKeyframe, audioClipEnd, audioEnd, bakeInstance, deleteKey, dragAudioEdge, dragEdge, easePoints, end, inline, keyTimes, moveClip,
    moveKey, newAudioClip, newInstance, nextName, poseObj, snap, snapPoints, timelineDuration,
    type AudioBus, type AudioClip, type AudioTracks, type ClipInstance, type Sample, type Timeline,
  } from "./model";
  import AudioClipView from "./AudioClipView.svelte";
  import AudioInspector from "./AudioInspector.svelte";
  import { sounds, type SoundInfo } from "../audio/sounds.svelte";
  import { director } from "../audio/director";
  import { ambientRange, ambientSpecs, sceneSpecs } from "../audio/sync";
  import AmbientEditor from "../audio/AmbientEditor.svelte";
  import TrackInspector from "../audio/TrackInspector.svelte";
  import { BUS_ICON, resolveTrack, type SoundBus, type Track } from "../audio/tracks";
  import { nameOf } from "../audio/names";
  import { solo, toggleSolo } from "../audio/solo.svelte";
  import { cloneEffects, hasEffects } from "../audio/effects";
  import { defaultDuration, library, type LibClip } from "./libstore.svelte";
  import Inspector from "./Inspector.svelte";
  import FxClipView from "../fx/FxClipView.svelte";
  import FxInspector from "../fx/FxInspector.svelte";
  import FxPicker from "../fx/FxPicker.svelte";
  import { FxTrack as FxEval, MIN_FX_DURATION, fxEnd, fxId, type Fx } from "../fx/fx";
  import Library from "./Library.svelte";
  import VoicesPanel from "../voices/VoicesPanel.svelte";
  import { log } from "../log";

  interface Props {
    store: ProjectStore;
    entry: string;
    size: [number, number] | null;
    viewer: ViewerApi;
    /** Timeline que genera la plantilla de esta imagen (`null` si no usa plantilla). */
    generate: () => Promise<wasm.WasmTimeline | null>;
    onclose: () => void;
    /** Imagen actual y nombres de todas (los ambientes van de una imagen a otra). */
    index: number;
    entries: string[];
  }
  let { store, entry, size, viewer, generate, onclose, index, entries }: Props = $props();

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
  /** Zona de pistas (se desplaza en los dos ejes) y su contenido (el segundo 0 está en PAD). */
  let scroller: HTMLDivElement;
  let content: HTMLDivElement;
  /** Fila de cada pista de sonido, por id. */
  const laneEls: Record<string, HTMLDivElement> = {};
  /** Ambiente seleccionado (excluye `sel` y `asel`). */
  let ambsel = $state<string | null>(null);
  /** Pista seleccionada (su inspector) y la última usada (donde van los sonidos nuevos). */
  let tsel = $state<string | null>(null);
  let lastTrack = $state<string | null>(null);
  let renamingTrack = $state<string | null>(null);
  /** Pista en la que se está grabando la voz. */
  let voiceTrack = $state<string | null>(null);
  /** Grabando voz con el micrófono: desde qué instante, nivel (0..1) y segundos grabados. */
  let voiceOn = $state(false);
  let voiceStart = 0;
  let voiceLevel = $state(0);
  let voiceSecs = $state(0);
  let voiceTok = 0;
  /** Arrastre desde la biblioteca: un clip de cámara o un sonido (y a qué pista va). */
  let libDrag = $state<{ clip?: LibClip; sound?: SoundInfo; x: number; y: number; t: number | null; track: Track | null } | null>(null);
  /** Panel 🗣 Voces IA en la columna del inspector (se cierra al seleccionar algo). */
  let voicesOpen = $state(false);
  $effect(() => {
    if (sel || asel || ambsel || tsel || fsel !== null) untrack(() => (voicesOpen = false));
  });
  function openVoices() {
    sel = null;
    asel = null;
    ambsel = null;
    tsel = null;
    fsel = null;
    voicesOpen = true;
  }
  /** Color del personaje de cada frase de esta imagen (para los clips generados). */
  const genColor = $derived.by(() => {
    const m = new Map<string, string>();
    for (const l of store.linesFor(entry)) {
      const c = store.cast.find((x) => x.id === l.cast);
      if (c?.color) m.set(l.id, c.color);
    }
    return m;
  });
  function removeLineClip(lineId: string) {
    if (!size) return;
    editAudio((a) => {
      a.voice = a.voice.filter((c) => c.gen !== lineId);
      a.sfx = a.sfx.filter((c) => c.gen !== lineId);
    });
  }

  /** Clip de sonido seleccionado (excluye `sel`). */
  let asel = $state<{ bus: AudioBus; idx: number } | null>(null);
  /** Efecto especial seleccionado (posición en la lista de la imagen). */
  let fsel = $state<number | null>(null);
  /** Menú de efectos abierto: en qué segundo se añadirá. */
  let picker = $state<{ at: number } | null>(null);
  /** Las situaciones añaden también su sonido. */
  let fxSound = $state(true);
  /** Con la onomatopeya de la situación («¡BOOM!»…). */
  let fxText = $state(true);
  /** Esperando el clic en la imagen para el punto del efecto seleccionado. */
  let picking = $state(false);

  const item = $derived(store.item(entry));
  const own = $derived<Timeline | null>(item?.timeline?.camera?.length ? item.timeline : null);
  const tpl = $derived(store.templateFor(entry));
  const view = $derived<Timeline>(own ?? generated ?? { camera: [] });
  const fromTemplate = $derived(!own && !!generated);
  const audio = $derived<AudioTracks>(store.audioFor(entry));
  const fxList = $derived<Fx[]>(store.fxFor(entry));
  /** Subfila de cada efecto (los que coinciden en el tiempo van uno debajo de otro) y cuántas hay. */
  const fxRows = $derived.by(() => {
    const ends: number[] = [];
    const row = fxList
      .map((f, i) => ({ f, i }))
      .sort((a, b) => a.f.start - b.f.start || a.i - b.i)
      .reduce((m, { f, i }) => {
        let r = ends.findIndex((e) => e <= f.start + 1e-6);
        if (r < 0) r = ends.push(0) - 1;
        ends[r] = f.start + f.duration;
        m[i] = r;
        return m;
      }, [] as number[]);
    return { row, count: Math.max(1, ends.length) };
  });
  /** Alto de cada subfila de efectos. */
  const FX_ROW = $derived(fxRows.count > 1 ? Math.max(9, Math.min(22, Math.floor(56 / fxRows.count))) : 22);
  const tracks = $derived(store.tracks);
  const trackOf = (c: { track?: string }, bus: SoundBus) => resolveTrack(tracks, c.track, bus).id;
  /** Voz y efectos de cada pista (con su posición en la lista de su grupo). */
  const laneClips = $derived.by(() => {
    const m = new Map<string, { c: AudioClip; bus: AudioBus; i: number }[]>();
    for (const bus of ["voice", "sfx"] as const) {
      audio[bus].forEach((c, i) => {
        const id = trackOf(c, bus);
        if (!m.has(id)) m.set(id, []);
        m.get(id)!.push({ c, bus, i });
      });
    }
    return m;
  });
  /** Pista donde va un sonido nuevo del grupo `bus`: la última usada si es de ese grupo. */
  function targetTrack(bus: SoundBus): string {
    const last = tracks.find((x) => x.id === lastTrack && x.bus === bus);
    return last?.id ?? resolveTrack(tracks, undefined, bus).id;
  }
  /** `track` del clip: vacío si es la primera del grupo (como en los proyectos sin pistas). */
  const trackField = (id: string, bus: SoundBus) => (id === resolveTrack(tracks, undefined, bus).id ? undefined : id);
  const indexOf = (e: string) => entries.indexOf(e);
  /** Ambientes que suenan en esta imagen. */
  const ambHere = $derived(
    store.ambients
      .map((a) => ({ a, r: ambientRange(a, indexOf) }))
      .filter((x): x is { a: typeof x.a; r: [number, number] } => !!x.r && index >= x.r[0] && index <= x.r[1]),
  );
  const ambLane = (id: string) => ambHere.filter((x) => trackOf(x.a, "ambient") === id);
  // La imagen dura lo que su animación, su sonido o sus efectos, lo que sea más largo.
  const duration = $derived(Math.max(timelineDuration(view), audioEnd(audio), fxEnd(fxList)));
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

  // Efectos especiales de lo que se ve; se rehacen en cada cambio.
  let fxTrack: FxEval | null = null;
  $effect(() => {
    const tr = FxEval.of($state.snapshot(fxList));
    untrack(() => {
      fxTrack?.free();
      fxTrack = tr;
      viewer.setFxNow(tr, playhead);
    });
  });

  // Otra imagen: se empieza de cero.
  $effect(() => {
    void entry;
    untrack(() => {
      void stopVoice(false);
      stopRecording(false);
      pause();
      playhead = 0;
      sel = null;
      asel = null;
      ambsel = null;
      tsel = null;
      fsel = null;
      picker = null;
      if (picking) viewer.cancelPick();
      free = true;
    });
  });

  // Sonido: el editor manda el de esta imagen al motor mientras está abierto.
  const owner = {};
  if (!sounds.loaded) sounds.load();
  $effect(() => {
    const specs = sceneSpecs(audio);
    const amb = ambientSpecs(store.ambients, index, indexOf);
    untrack(() => {
      director.scene(owner, specs);
      director.ambient(owner, amb);
      syncSound(true);
    });
  });
  function syncSound(seek = false) {
    const on = playing || recording;
    director.transport(owner, { t: playhead, playing: on, running: on, rate: 1 }, seek);
  }

  onDestroy(() => {
    void stopVoice(false);
    director.release(owner);
    stopRecording(false);
    pause();
    evalTl?.free();
    evalTl = null;
    viewer.setFxNow(null);
    if (picking) viewer.cancelPick();
    fxTrack?.free();
    fxTrack = null;
    window.removeEventListener("pointermove", onDragMove);
    window.removeEventListener("pointerup", onDragUp);
  });

  function say(msg: string) {
    notice = msg;
  }

  function showAt(t: number) {
    if (evalTl) {
      viewer.setPoseNow(evalTl.evaluate(t));
      free = false;
    }
    viewer.setFxNow(fxTrack, t);
  }

  function seek(t: number) {
    playhead = Math.max(0, t);
    showAt(playhead);
    syncSound(true);
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
    if (duration <= 0 || recording) return;
    if (playhead >= duration - 0.01) playhead = 0;
    playing = true;
    last = performance.now();
    showAt(playhead);
    syncSound(true);
    raf = requestAnimationFrame(tick);
  }
  function tick(now: number) {
    if (!playing) return;
    playhead = Math.min(duration, playhead + (now - last) / 1000);
    last = now;
    showAt(playhead);
    if (playhead >= duration) {
      playing = false;
      return syncSound();
    }
    syncSound();
    raf = requestAnimationFrame(tick);
  }
  function pause() {
    const was = playing;
    playing = false;
    cancelAnimationFrame(raf);
    if (was) syncSound();
  }

  /** El usuario ha movido la cámara a mano. */
  /** Termina las grabaciones en curso y las conserva (al cerrar la fuente). */
  export async function finishRecording() {
    if (recording) stopRecording(true);
    if (voiceOn) await stopVoice(true);
  }

  // --- grabar voz -----------------------------------------------------------------------
  /** Graba del micrófono mientras la animación avanza desde el cabezal. */
  async function startVoice() {
    if (!size || store.readOnly || voiceOn || recording) return;
    pause();
    try {
      await invoke("record_start", { device: prefs.settings.sound.input || null });
    } catch (e) {
      log.warn("micrófono:", e);
      return say(t("micError", { error: errorText(e) }));
    }
    const tok = ++voiceTok;
    voiceOn = true;
    voiceTrack = targetTrack("voice");
    voiceStart = playhead;
    voiceLevel = 0;
    voiceSecs = 0;
    director.setRecording(true);
    say(t("voiceRecHelp", { key: ek("recordVoice"), cancel: ek("cancel") }));
    const t0 = performance.now();
    let lastPoll = 0;
    const step = (now: number) => {
      if (tok !== voiceTok) return;
      playhead = voiceStart + (now - t0) / 1000;
      showAt(playhead);
      if (now - lastPoll > 100) {
        lastPoll = now;
        invoke<{ level: number; seconds: number } | null>("record_level").then((r) => {
          if (r && tok === voiceTok) {
            voiceLevel = Math.min(1, r.level);
            voiceSecs = r.seconds;
          }
        });
      }
      requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  async function stopVoice(keep: boolean) {
    if (!voiceOn) return;
    voiceOn = false;
    voiceTok++;
    director.setRecording(false);
    notice = null;
    const start = voiceStart;
    const name = nextName(t("voiceName"), sounds.list.filter((s) => s.kind === "recorded").map((s) => s.name));
    try {
      const info = await invoke<SoundInfo | null>("record_stop", { keep, name });
      if (info) {
        sounds.add(info);
        addSound(info, "voice", start, voiceTrack ?? undefined);
        say(t("voiceRecorded", { s: info.seconds.toFixed(1) }));
      } else {
        seek(start);
      }
    } catch (e) {
      log.warn("grabar voz:", e);
      say(t("micError", { error: errorText(e) }));
      seek(start);
    }
  }

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
    syncSound(true); // el sonido acompaña a la grabación, para moverse al ritmo de la voz
    const step = (now: number) => {
      if (!rec) return;
      const el = (now - rec.t0) / 1000;
      rec.flat.push(el, ...viewer.currentPose(false));
      playhead = rec.start + el;
      syncSound();
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
    syncSound();
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

  // --- sonido de la imagen -----------------------------------------------------------------
  function editAudio(fn: (a: AudioTracks) => void): boolean {
    if (!size || store.readOnly) {
      if (store.readOnly) say(t("projectReadOnly", { error: store.readOnly }));
      return false;
    }
    store.editAudio(entry, size, fn);
    return true;
  }

  function addSound(s: SoundInfo, bus: AudioBus, at: number, track = targetTrack(bus)) {
    let idx = -1;
    editAudio((a) => {
      const c = newAudioClip(s, at);
      const tr = trackField(track, bus);
      if (tr) c.track = tr;
      a[bus].push(c);
      idx = a[bus].length - 1;
    });
    if (idx >= 0) {
      asel = { bus, idx };
      sel = null;
      ambsel = null;
      tsel = null;
      fsel = null;
      fsel = null;
      lastTrack = track;
    }
  }

  /** «+» de la biblioteca: a la última pista usada; si no hay, voz o efectos según el sonido. */
  function addFromSounds(s: SoundInfo) {
    const last = tracks.find((x) => x.id === lastTrack);
    if (last?.bus === "ambient") return addAmbientHere(s, last.id);
    const bus = last ? (last.bus as AudioBus) : busFor(s);
    addSound(s, bus, playhead, last?.id ?? targetTrack(bus));
  }

  /** Un sonido soltado en la pista de ambiente: suena de fondo en esta imagen. */
  function addAmbientHere(s: SoundInfo, track = targetTrack("ambient")) {
    if (!entries[index]) return;
    const tr = trackField(track, "ambient");
    const id = store.addAmbient({ sound: s.id, from: entries[index], to: entries[index], volume: 1, fadeIn: 1, fadeOut: 1, ...(tr ? { track: tr } : {}) });
    if (id) {
      ambsel = id;
      sel = null;
      asel = null;
      tsel = null;
      fsel = null;
      lastTrack = track;
      say(t("ambientAddedHere"));
    }
  }

  /** Pista por defecto: las grabaciones a la voz, el resto a efectos. */
  const busFor = (s: SoundInfo): AudioBus => (s.kind === "recorded" ? "voice" : "sfx");

  let removedAt = -Infinity;

  /**
   * Borra el ambiente en la imagen actual: si empieza aquí, entero; si viene de una imagen
   * anterior, solo deja de sonar en esta (se acorta o se parte en dos).
   */
  function removeAmbientHere(id: string) {
    ambsel = null;
    const done = store.removeAmbientAt(id, index, entries);
    if (done === "here") say(t("ambientRemovedHere", { n: index + 1 }));
    else if (done === "all") say(t("ambientRemovedAll"));
  }

  function removeAudio() {
    const s = asel;
    if (!s) return;
    asel = null;
    editAudio((a) => void a[s.bus].splice(s.idx, 1));
  }

  function duplicateAudio() {
    const s = asel;
    if (!s) return;
    let idx = -1;
    editAudio((a) => {
      const src = a[s.bus][s.idx];
      if (!src) return;
      const copy: AudioClip = { ...$state.snapshot(src), start: audioClipEnd(src) };
      const fx = cloneEffects(copy.effects);
      if (fx) copy.effects = fx;
      a[s.bus].push(copy);
      idx = a[s.bus].length - 1;
    });
    if (idx >= 0) asel = { bus: s.bus, idx };
  }

  /** Pasa el clip seleccionado a otra pista (también de voz a efectos o al revés). */
  function setClipTrack(id: string) {
    const s = asel;
    const tr = tracks.find((x) => x.id === id);
    if (!s || !tr || tr.bus === "ambient") return;
    const to = tr.bus as AudioBus;
    let idx = s.idx;
    editAudio((a) => {
      const c = a[s.bus][s.idx];
      if (!c) return;
      const copy: AudioClip = $state.snapshot(c);
      const field = trackField(id, to);
      if (field) copy.track = field;
      else delete copy.track;
      if (to === s.bus) a[s.bus][s.idx] = copy;
      else {
        a[s.bus].splice(s.idx, 1);
        a[to].push(copy);
        idx = a[to].length - 1;
      }
    });
    asel = { bus: to, idx };
    lastTrack = id;
  }

  // --- efectos especiales ----------------------------------------------------------------
  function editFx(fn: (list: Fx[]) => void): boolean {
    if (!size || store.readOnly) {
      if (store.readOnly) say(t("projectReadOnly", { error: store.readOnly }));
      return false;
    }
    store.editFx(entry, size, fn);
    return true;
  }

  function selectFx(i: number) {
    fsel = i;
    sel = null;
    asel = null;
    ambsel = null;
    tsel = null;
  }

  /** Añade un efecto suelto o una situación (con su sonido, si se pide) en el segundo `at`. */
  function addFx(p: { situation: string } | { type: string }, at: number) {
    picker = null;
    let added: Fx[] = [];
    let sound: string | null = null;
    try {
      if ("situation" in p) {
        const r = JSON.parse(wasm.fx_situation(p.situation, fxId(), at, NaN, NaN, fxText)) as { fx: Fx[]; sound?: string | null };
        added = r.fx;
        sound = r.sound ?? null;
      } else {
        added = [JSON.parse(wasm.fx_new(fxId(), p.type, "", at)) as Fx];
      }
    } catch (e) {
      log.warn("editor: no se pudo crear el efecto:", e);
      return;
    }
    if (!added.length) return;
    const first = fxList.length;
    store.batch(true);
    try {
      if (!editFx((list) => list.push(...added))) return;
      const info = sound && fxSound ? sounds.get(sound) : null;
      if (info) {
        editAudio((a) => {
          const c = newAudioClip(info, at);
          const tr = trackField(targetTrack("sfx"), "sfx");
          if (tr) c.track = tr;
          a.sfx.push(c);
        });
      }
    } finally {
      store.batch(false);
    }
    selectFx(first);
    seek(at);
  }

  function removeFx() {
    const i = fsel;
    if (i === null) return;
    fsel = null;
    if (picking) viewer.cancelPick();
    editFx((list) => void list.splice(i, 1));
  }

  function duplicateFx() {
    const i = fsel;
    if (i === null || !fxList[i]) return;
    const src = $state.snapshot(fxList[i]) as Fx;
    const n = fxList.length;
    editFx((list) => list.push({ ...src, id: fxId(), start: src.start + src.duration }));
    selectFx(n);
  }

  /** «Elegir punto»: el siguiente clic en la imagen marca dónde ocurre el efecto seleccionado. */
  async function pickFxPoint() {
    if (picking) return viewer.cancelPick();
    const i = fsel;
    if (i === null) return;
    const id = fxList[i]?.id;
    pause();
    picking = true;
    say(t("fxPickNotice"));
    const p = await viewer.pickPoint();
    picking = false;
    notice = null;
    if (!p || !id) return;
    editFx((list) => {
      const f = list.find((x) => x.id === id);
      if (f) f.at = { x: Number(p.x.toFixed(4)), y: Number(p.y.toFixed(4)) };
    });
  }

  function onFxDown(e: PointerEvent, idx: number, part: "body" | "start" | "end") {
    selectFx(idx);
    const f = fxList[idx];
    if (!f) return;
    if (part === "body") beginDrag({ kind: "fmove", idx, offset: timeAt(e.clientX) - f.start }, e);
    else beginDrag({ kind: "fedge", idx, edge: part }, e);
  }

  /** Fila de pista que hay bajo el puntero. */
  function laneAt(e: PointerEvent): Track | null {
    return tracks.find((x) => inside(laneEls[x.id], e)) ?? null;
  }

  function selectTrack(id: string) {
    tsel = id;
    lastTrack = id;
    sel = null;
    asel = null;
    ambsel = null;
    fsel = null;
  }

  function addTrack(bus: SoundBus) {
    if (store.readOnly) return say(t("projectReadOnly", { error: store.readOnly }));
    const id = store.addTrack(bus);
    if (id) selectTrack(id);
  }

  function renameTrack(id: string, value: string) {
    renamingTrack = null;
    store.editTrack(id, (x) => {
      const v = value.trim();
      if (v) x.name = v.slice(0, 80);
      else delete x.name;
    });
  }

  /** Bordes de los demás clips de sonido, para el ajuste magnético. */
  function audioPoints(bus: AudioBus | null, idx: number): number[] {
    const pts: number[] = [];
    for (const b of ["voice", "sfx"] as const) audio[b].forEach((c, i) => (b !== bus || i !== idx) && pts.push(c.start, audioClipEnd(c)));
    return pts;
  }

  function onAudioDown(e: PointerEvent, bus: AudioBus, idx: number, part: "body" | "start" | "end") {
    asel = { bus, idx };
    sel = null;
    ambsel = null;
    tsel = null;
    fsel = null;
    const c = audio[bus][idx];
    if (!c) return;
    lastTrack = trackOf(c, bus);
    if (part === "body") beginDrag({ kind: "amove", bus, idx, offset: timeAt(e.clientX) - c.start }, e);
    else beginDrag({ kind: "aedge", bus, idx, edge: part }, e);
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
        if (voiceOn) void stopVoice(true);
        else if (recording) stopRecording(true);
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
      case "recordVoice":
        if (voiceOn) void stopVoice(true);
        else void startVoice();
        return true;
      case "remove":
        if (fsel !== null) removeFx();
        else if (asel) removeAudio();
        else if (ambsel) removeAmbientHere(ambsel);
        else if (sel) removeSelection();
        // Justo después de borrar, otra pulsación no hace nada: con `Retroceso` (que fuera del
        // editor es «imagen anterior») un doble toque no te saca de la imagen.
        else return performance.now() - removedAt < 1200;
        removedAt = performance.now();
        return true;
      case "markPrev":
        mark(-1);
        return true;
      case "markNext":
        mark(1);
        return true;
      case "cancel":
        if (voiceOn) void stopVoice(false);
        else if (recording) stopRecording(false);
        else if (picking) viewer.cancelPick();
        else if (picker) picker = null;
        else if (playing) pause();
        else if (sel || asel || ambsel || tsel || fsel !== null) (sel = null), (asel = null), (ambsel = null), (tsel = null), (fsel = null);
        else return false;
        return true;
    }
  }

  // --- arrastres en la pista -------------------------------------------------------------
  type Drag =
    | { kind: "scrub" }
    | { kind: "move"; idx: number; offset: number }
    | { kind: "edge"; idx: number; edge: "start" | "end" }
    | { kind: "key"; idx: number; key: number }
    | { kind: "amove"; bus: AudioBus; idx: number; offset: number }
    | { kind: "aedge"; bus: AudioBus; idx: number; edge: "start" | "end" }
    | { kind: "fmove"; idx: number; offset: number }
    | { kind: "fedge"; idx: number; edge: "start" | "end" };
  let drag: Drag | null = null;
  let downAt = { x: 0, y: 0 };
  let moved = $state(false);

  function timeAt(clientX: number): number {
    return (clientX - content.getBoundingClientRect().left - PAD) / pps;
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
    if (d.kind === "fmove" || d.kind === "fedge") {
      const pts = [...snapPoints(view, playhead, -1), ...audioPoints(null, -1), ...fxList.flatMap((f, i) => (i === d.idx ? [] : [f.start, f.start + f.duration]))];
      editFx((list) => {
        const f = list[d.idx];
        if (!f) return;
        if (d.kind === "fmove") {
          const s0 = Math.max(0, t - d.offset);
          const sa = snap(s0, pts, thr);
          const sb = snap(s0 + f.duration, pts, thr) - f.duration;
          const da = sa !== s0 ? Math.abs(sa - s0) : Infinity;
          const db = sb !== s0 ? Math.abs(sb - s0) : Infinity;
          f.start = Math.max(0, da === Infinity && db === Infinity ? s0 : da <= db ? sa : sb);
        } else if (d.edge === "start") {
          const endT = f.start + f.duration;
          const s1 = Math.min(endT - MIN_FX_DURATION, Math.max(0, snap(t, pts, thr)));
          f.start = s1;
          f.duration = endT - s1;
        } else {
          f.duration = Math.max(MIN_FX_DURATION, snap(t, pts, thr) - f.start);
        }
      });
      return;
    }
    if (d.kind === "amove" || d.kind === "aedge") {
      const pts = [...snapPoints(view, playhead, -1), ...audioPoints(d.bus, d.idx)];
      editAudio((a) => {
        const c = a[d.bus][d.idx];
        if (!c) return;
        if (d.kind === "amove") {
          // Arrastrado a otra fila del mismo grupo: cambia de pista.
          const over = laneAt(e);
          if (over && over.bus === d.bus && over.id !== trackOf(c, d.bus)) {
            const field = trackField(over.id, d.bus);
            if (field) c.track = field;
            else delete c.track;
            lastTrack = over.id;
          }
          // Igual que la cámara: se pega por el principio o por el final.
          const s0 = Math.max(0, t - d.offset);
          const sa = snap(s0, pts, thr);
          const sb = snap(s0 + c.duration, pts, thr) - c.duration;
          const da = sa !== s0 ? Math.abs(sa - s0) : Infinity;
          const db = sb !== s0 ? Math.abs(sb - s0) : Infinity;
          c.start = Math.max(0, da === Infinity && db === Infinity ? s0 : da <= db ? sa : sb);
        } else {
          dragAudioEdge(c, d.edge, snap(t, pts, thr), sounds.get(c.sound)?.seconds ?? Infinity);
        }
      });
      return;
    }
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
    asel = null;
    ambsel = null;
    tsel = null;
    fsel = null;
    const inst = view.camera[idx];
    if (part === "body") beginDrag({ kind: "move", idx, offset: timeAt(e.clientX) - inst.start }, e);
    else beginDrag({ kind: "edge", idx, edge: part }, e);
  }

  function onKeyDown(e: PointerEvent, idx: number, key: number) {
    sel = { clip: idx, key };
    asel = null;
    ambsel = null;
    tsel = null;
    fsel = null;
    beginDrag({ kind: "key", idx, key }, e);
    seek(keyTimes(view.camera[idx])[key]);
  }

  function onLaneDown(e: PointerEvent) {
    sel = null;
    asel = null;
    ambsel = null;
    tsel = null;
    fsel = null;
    beginDrag({ kind: "scrub" }, e);
  }

  /** Rueda: Ctrl acerca o aleja; sobre los nombres de las pistas o con Mayús sube y baja; si no,
   *  avanza en el tiempo. */
  function onWheel(e: WheelEvent) {
    if (e.ctrlKey) {
      e.preventDefault();
      const t = timeAt(e.clientX);
      const x = e.clientX - content.getBoundingClientRect().left;
      pps = Math.min(600, Math.max(8, pps * Math.exp(-e.deltaY * 0.002)));
      scroller.scrollLeft += t * pps + PAD - x;
    } else if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      e.preventDefault();
      const vertical = e.shiftKey || (e.target as HTMLElement | null)?.closest?.(".labels");
      if (vertical) scroller.scrollTop += e.deltaY;
      else scroller.scrollLeft += e.deltaY;
    }
  }

  function zoomTimeline(f: number) {
    pps = Math.min(600, Math.max(8, pps * f));
  }

  // Arrastrar desde la biblioteca (con eventos de puntero: el arrastre HTML no es fiable en
  // todos los webviews).
  function startLibDrag(c: LibClip, e: PointerEvent) {
    beginLibDrag({ clip: c }, e);
  }
  function startSoundDrag(s: SoundInfo, e: PointerEvent) {
    beginLibDrag({ sound: s }, e);
  }
  function beginLibDrag(what: { clip?: LibClip; sound?: SoundInfo }, e: PointerEvent) {
    if (e.button !== 0) return;
    libDrag = { ...what, x: e.clientX, y: e.clientY, t: null, track: null };
    downAt = { x: e.clientX, y: e.clientY };
    moved = false;
    window.addEventListener("pointermove", onDragMove);
    window.addEventListener("pointerup", onDragUp);
  }
  const inside = (el: HTMLElement | undefined, e: PointerEvent) => {
    if (!el) return false;
    const r = el.getBoundingClientRect();
    return e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
  };
  function onLibMove(e: PointerEvent) {
    if (!libDrag) return;
    if (!moved && Math.hypot(e.clientX - downAt.x, e.clientY - downAt.y) < 4) return;
    moved = true;
    // Un sonido va a la pista que haya debajo; un clip, a la de cámara.
    const track = libDrag.sound ? laneAt(e) : null;
    const over = libDrag.sound ? !!track : inside(scroller, e);
    const pts = libDrag.sound ? [...snapPoints(view, playhead, -1), ...audioPoints(null, -1)] : snapPoints(view, playhead, -1);
    const t = over ? snap(Math.max(0, timeAt(e.clientX)), pts, SNAP_PX / pps) : null;
    libDrag = { ...libDrag, x: e.clientX, y: e.clientY, t, track };
  }
  function onLibUp() {
    const d = libDrag;
    libDrag = null;
    if (!d || !moved || d.t === null) return;
    if (d.clip) addFromLibrary(d.clip, d.t);
    else if (d.sound && d.track?.bus === "ambient") addAmbientHere(d.sound, d.track.id);
    else if (d.sound && d.track) addSound(d.sound, d.track.bus as AudioBus, d.t, d.track.id);
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

  /** Altura del editor arrastrando su borde superior. */
  function startHeight(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    const y0 = e.clientY;
    const h0 = prefs.settings.editorHeight;
    const max = Math.min(EDITOR_HEIGHT.max, Math.round(window.innerHeight * 0.85));
    const move = (ev: PointerEvent) => {
      prefs.settings.editorHeight = Math.round(Math.min(max, Math.max(EDITOR_HEIGHT.min, h0 + (y0 - ev.clientY))));
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

  const videoPx = $derived(videoPixels(prefs.settings.video));
  function setFrame(m: FrameMode) {
    prefs.settings.editorFrame = m;
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

<section class="editor" style:height="{prefs.settings.editorHeight}px">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="vsplit" title={t("resizeHelp")} onpointerdown={startHeight}
    ondblclick={() => ((prefs.settings.editorHeight = EDITOR_HEIGHT.def), prefs.saveSettings())}></div>
  <header>
    <b>{t("editor")}</b>
    <button onclick={() => (playing ? pause() : play())} disabled={duration <= 0 || recording} title={tip(playing ? t("pause") : t("play"), ek("toggle"))}>
      {playing ? "⏸" : "▶"}
    </button>
    <button onclick={() => (pause(), seek(0))} title={t("toStart")}>⏮</button>
    <span class="time">{fmtTime(playhead)} / {fmtTime(duration)}</span>
    <span class="sep"></span>
    <button onclick={keyframe} disabled={recording || !size} title={tip(t("addKeyframe"), ek("keyframe"))}>◆ {t("keyframeShort")}</button>
    <button onclick={() => (pause(), (picker = { at: playhead }))} disabled={recording || !size} title={t("fxAddHelp")}>{t("fxAddHere")}</button>
    <button class="rec" class:on={recording} onclick={() => (recording ? stopRecording(true) : startRecording())} disabled={!size}
      title={tip(recording ? t("stopRecording") : t("record"), ek("record"))}>● {recording ? t("stopRecording") : t("record")}</button>
    <button class="rec" class:on={voiceOn} onclick={() => (voiceOn ? stopVoice(true) : startVoice())} disabled={!size || recording}
      title={tip(voiceOn ? t("stopRecording") : t("recordVoiceHelp"), ek("recordVoice"))}>🎙 {voiceOn ? `${voiceSecs.toFixed(1)} s` : t("recordVoice")}</button>
    <span class="sep"></span>
    <button onclick={() => store.undo()} disabled={!store.canUndo} title={tip(t("undo"), vk("undo"))}>↶</button>
    <button onclick={() => store.redo()} disabled={!store.canRedo} title={tip(t("redo"), vk("redo"))}>↷</button>
    <span class="sep"></span>
    <button onclick={() => zoomTimeline(1 / 1.4)} title={t("zoomOutTl")}>−</button>
    <button onclick={() => zoomTimeline(1.4)} title={t("zoomInTl")}>+</button>
    <span class="sep"></span>
    <label class="frame-mode" title={t("frameHelp")}>
      ▭
      <select value={prefs.settings.editorFrame} onchange={(e) => setFrame(e.currentTarget.value as FrameMode)}>
        <option value="playback">{t("framePlayback")}</option>
        <option value="video">{t("frameVideo", { w: videoPx.w, h: videoPx.h })}</option>
        <option value="off">{t("frameOff")}</option>
      </select>
    </label>
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
      <Library {store} {entry} {size} onadd={(c) => addFromLibrary(c)} ondrag={startLibDrag} onnotice={say} onfit={fitLibrary}
        onsounddrag={startSoundDrag} onsoundadd={addFromSounds} />
    </aside>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="splitter" title={t("resizeHelp")} onpointerdown={(e) => startResize("lib", e)} ondblclick={fitLibrary}></div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="tracks" bind:this={scroller} onwheel={onWheel}>
      <div class="grid">
        <div class="labels">
          <div class="ruler-label"></div>
          <div class="lane-label">🎥 {t("trackCamera")}</div>
          <div class="lane-label fxl" title={t("fxLaneHelp")} style:height="{fxRows.count * (FX_ROW + 1) + 5}px">
            <span class="tname">✨ {t("trackFx")}</span>
            <button class="ms add" title={t("fxAddHelp")} disabled={!size || !!store.readOnly} onclick={() => (pause(), (picker = { at: playhead }))}>+</button>
          </div>
          {#each tracks as tr, k (tr.id)}
            {@const last = tracks[k + 1]?.bus !== tr.bus}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              class="lane-label track {tr.bus}"
              class:selected={tsel === tr.id}
              class:muted={tr.mute}
              class:last
              title={t("trackHelp")}
              onclick={() => selectTrack(tr.id)}
            >
              <span class="ticon">{BUS_ICON[tr.bus]}</span>
              {#if renamingTrack === tr.id}
                <!-- svelte-ignore a11y_autofocus -->
                <input class="tname-input" value={tr.name ?? ""} placeholder={nameOf(tracks, tr)} autofocus
                  onkeydown={(e) => { e.stopPropagation(); if (e.key === "Enter") renameTrack(tr.id, e.currentTarget.value); if (e.key === "Escape") renamingTrack = null; }}
                  onblur={(e) => renamingTrack === tr.id && renameTrack(tr.id, e.currentTarget.value)} />
              {:else}
                <span class="tname" ondblclick={() => !store.readOnly && (renamingTrack = tr.id)}>{nameOf(tracks, tr)}</span>
              {/if}
              {#if voiceOn && voiceTrack === tr.id}
                <span class="meter" title={t("micLevel")}><span style:width="{Math.round(Math.sqrt(voiceLevel) * 100)}%" class:hot={voiceLevel > 0.95}></span></span>
              {:else if hasEffects(tr.effects)}
                <span class="fxb" title={t("effects")}>fx</span>
              {/if}
              <button class="ms" class:on={tr.mute} title={t("trackMuteHelp")} disabled={!!store.readOnly}
                onclick={(e) => { e.stopPropagation(); store.editTrack(tr.id, (x) => { if (x.mute) delete x.mute; else x.mute = true; }); }}>M</button>
              <button class="ms solo" class:on={solo.ids.includes(tr.id)} title={t("trackSoloHelp")}
                onclick={(e) => { e.stopPropagation(); toggleSolo(tr.id); }}>S</button>
              {#if tr.bus === "voice" && tracks.find((x) => x.bus === "voice")?.id === tr.id}
                <button class="ms voices" class:on={voicesOpen} title={t("voicesOpenHelp")} onclick={(e) => { e.stopPropagation(); if (voicesOpen) voicesOpen = false; else openVoices(); }}>🗣</button>
              {/if}
              {#if last}
                <button class="ms add" title={t("trackAdd")} disabled={!!store.readOnly} onclick={(e) => { e.stopPropagation(); addTrack(tr.bus); }}>+</button>
              {/if}
            </div>
          {/each}
        </div>
        <div class="content" bind:this={content} style:width="{width}px">
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
            {#if libDrag?.clip && libDrag.t != null}
              <div class="drop" style:left="{PAD + libDrag.t * pps}px" style:width="{Math.max(8, defaultDuration(libDrag.clip.clip) * pps)}px"></div>
            {/if}
          </div>
          <div class="lane fx" onpointerdown={onLaneDown} style:height="{fxRows.count * (FX_ROW + 1) + 5}px">
            {#each fxList as f, i (f.id)}
              <FxClipView fx={f} {pps} pad={PAD} selected={fsel === i} top={3 + (fxRows.row[i] ?? 0) * (FX_ROW + 1)} height={FX_ROW}
                ondown={(e, part) => onFxDown(e, i, part)} />
            {/each}
          </div>
          {#each tracks as tr (tr.id)}
            {#if tr.bus === "ambient"}
              {@const here = ambLane(tr.id)}
              <div class="lane amb" class:muted={tr.mute} bind:this={laneEls[tr.id]} onpointerdown={onLaneDown} title={here.length ? "" : t("ambientLaneHelp")}>
                {#each here as { a, r }, k (a.id)}
                  {@const h = Math.max(8, Math.floor(22 / here.length))}
                  <button
                    class="ambbar"
                    class:selected={ambsel === a.id}
                    class:first={r[0] === index}
                    class:last={r[1] === index}
                    style:top="{2 + k * h}px"
                    style:height="{h - 1}px"
                    style:left="{PAD}px"
                    style:width="{Math.max(24, duration * pps)}px"
                    onpointerdown={(e) => { e.stopPropagation(); ambsel = a.id; sel = null; asel = null; tsel = null; fsel = null; lastTrack = tr.id; }}
                  >
                    🔁 {a.name ?? sounds.name(a.sound)} · {r[0] === r[1] ? t("ambientOne", { a: r[0] + 1 }) : t("ambientRange", { a: r[0] + 1, b: r[1] + 1 })}
                  </button>
                {/each}
                {#if libDrag?.sound && libDrag.track?.id === tr.id}<div class="drop ambdrop" style:left="{PAD}px" style:width="{Math.max(24, duration * pps)}px"></div>{/if}
              </div>
            {:else}
              <div class="lane audio" class:muted={tr.mute} bind:this={laneEls[tr.id]} onpointerdown={onLaneDown}>
                {#each laneClips.get(tr.id) ?? [] as { c, bus, i } (bus + i)}
                  <AudioClipView clip={c} {bus} {pps} pad={PAD} height={28} selected={asel?.bus === bus && asel.idx === i} color={c.gen ? genColor.get(c.gen) : undefined} ondown={(e, part) => onAudioDown(e, bus, i, part)} />
                {/each}
                {#if libDrag?.sound && libDrag.track?.id === tr.id && libDrag.t != null}
                  <div class="drop audio" style:left="{PAD + libDrag.t * pps}px" style:width="{Math.max(8, libDrag.sound.seconds * pps)}px"></div>
                {/if}
              </div>
            {/if}
          {/each}
          <div class="playhead" class:rec={recording || voiceOn} style:left="{PAD + playhead * pps}px"></div>
        </div>
      </div>
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="splitter" title={t("resizeHelp")} onpointerdown={(e) => startResize("inspector", e)}
      ondblclick={() => ((prefs.settings.inspectorWidth = INSPECTOR_WIDTH.def), prefs.saveSettings())}></div>
    <aside class="inspector" style:width="{prefs.settings.inspectorWidth}px">
      {#if voicesOpen}
        <VoicesPanel {store} {entry} {size} {playhead} onseek={seek} onnotice={say} onclose={() => (voicesOpen = false)} onremoveclip={removeLineClip} />
      {:else if fsel !== null && fxList[fsel]}
        {@const fi = fsel}
        <FxInspector
          fx={fxList[fi]}
          edit={(fn) => editFx((list) => { const f = list[fi]; if (f) fn(f); })}
          {picking}
          onpick={pickFxPoint}
          onseek={seek}
          onremove={removeFx}
          onduplicate={duplicateFx}
        />
      {:else if tsel && tracks.find((x) => x.id === tsel)}
        <TrackInspector {store} track={tracks.find((x) => x.id === tsel)!} onremoved={() => (tsel = null)} />
      {:else if ambsel && store.ambients.find((x) => x.id === ambsel)}
        {@const am = store.ambients.find((x) => x.id === ambsel)!}
        <h5 class="amb-title">🔁 {am.name ?? sounds.name(am.sound)}</h5>
        <AmbientEditor clip={am} {entries} {index} {tracks} edit={(fn) => store.editAmbient(am.id, fn)} onremove={() => { store.removeAmbient(am.id); ambsel = null; }}
          onremovehere={() => removeAmbientHere(am.id)} />
      {:else if asel && audio[asel.bus][asel.idx]}
        {@const a = asel}
        <AudioInspector
          clip={audio[a.bus][a.idx]}
          bus={a.bus}
          edit={(fn) => editAudio((tr) => { const c = tr[a.bus][a.idx]; if (c) fn(c); })}
          {tracks}
          track={trackOf(audio[a.bus][a.idx], a.bus)}
          onsettrack={setClipTrack}
          onseek={seek}
          onremove={removeAudio}
          onduplicate={duplicateAudio}
        />
      {:else if sel && view.camera[sel.clip]}
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
          onfx={(at) => (pause(), seek(at), (picker = { at }))}
        />
      {:else}
        <p class="note">{fromTemplate ? t("editorTemplate", { name: tpl ? label(`tpl.${tpl.id}`) : "" }) : t("editorHelp")}</p>
        <p class="note">{t("editorDragHelp")}</p>
        <button class="voices-open" onclick={openVoices} title={t("voicesOpenHelp")}>🗣 {t("voicesTitle")}</button>
      {/if}
    </aside>
  </div>
</section>

{#if picker}
  <FxPicker at={picker.at} withSound={fxSound} onsound={(on) => (fxSound = on)} withText={fxText} ontext={(on) => (fxText = on)} onpick={(p) => addFx(p, picker!.at)} onclose={() => (picker = null)} />
{/if}

{#if libDrag && moved}
  <div class="ghost" style:left="{libDrag.x + 10}px" style:top="{libDrag.y + 10}px">
    {#if libDrag.clip}{ICON[libDrag.clip.clip.type]} {libDrag.clip.name}{:else if libDrag.sound}♪ {sounds.name(libDrag.sound.id)}{/if}
  </div>
{/if}

<style>
  .editor { position: relative; display: flex; flex-direction: column; max-height: 85vh; flex: none; background: var(--panel); border-top: 1px solid var(--border); font-size: 13px; }
  .vsplit { position: absolute; left: 0; right: 0; top: -3px; height: 6px; cursor: row-resize; z-index: 8; }
  .vsplit:hover { background: rgb(56 189 248 / 0.35); }
  header { display: flex; align-items: center; gap: 6px; padding: 4px 10px; border-bottom: 1px solid var(--border); min-width: 0; }
  header button { padding: 2px 8px; }
  .frame-mode { display: flex; align-items: center; gap: 4px; color: var(--muted); }
  .frame-mode select { font: inherit; font-size: 12px; color: var(--text); background: var(--panel-2); border: 1px solid var(--border); border-radius: 6px; padding: 1px 4px; }
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
  /* Pistas: se desplazan en los dos ejes; la regla y los nombres se quedan fijos. */
  .tracks { flex: 1; min-width: 0; min-height: 0; overflow: auto; position: relative; }
  .grid { display: flex; min-height: 100%; width: max-content; }
  .labels { width: 168px; flex: none; border-right: 1px solid var(--border); position: sticky; left: 0; z-index: 6; background: var(--panel); }
  .ruler-label { height: 22px; border-bottom: 1px solid var(--border); position: sticky; top: 0; z-index: 7; background: var(--panel); }
  .lane-label { height: 44px; display: flex; align-items: center; padding: 0 8px; font-size: 12px; border-bottom: 1px solid rgb(42 47 56 / 0.6); }
  .lane-label.off { height: 26px; color: var(--muted); opacity: 0.6; }
  .lane-label.track { height: 34px; gap: 4px; padding: 0 4px 0 8px; cursor: pointer; user-select: none; }
  .lane-label.track.ambient { height: 26px; }
  .lane-label.track.last { border-bottom-color: var(--border); }
  .lane-label.track:hover { background: rgb(255 255 255 / 0.03); }
  .lane-label.track.selected { background: rgb(251 191 36 / 0.12); box-shadow: inset 2px 0 0 #fbbf24; }
  .lane-label.track.muted .tname { color: var(--muted); text-decoration: line-through; }
  .ticon { flex: none; width: 16px; text-align: center; }
  .tname { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tname-input { flex: 1; min-width: 0; font: inherit; font-size: 12px; color: var(--text); background: var(--bg); border: 1px solid var(--accent); border-radius: 4px; padding: 0 4px; }
  .fxb { flex: none; font-size: 9px; font-weight: 700; padding: 0 3px; border-radius: 3px; color: #04121b; background: #a78bfa; }
  .ms { flex: none; width: 18px; height: 18px; padding: 0; font-size: 10px; font-weight: 700; line-height: 16px; border-radius: 4px; color: var(--muted); }
  .ms.on { background: #b45309; border-color: #f59e0b; color: #fff7ed; }
  .ms.solo.on { background: #1d4ed8; border-color: #60a5fa; color: #eff6ff; }
  .ms.add { color: var(--accent); }
  .meter { flex: 1; height: 6px; margin-left: 6px; border-radius: 3px; background: var(--border); overflow: hidden; }
  .meter span { display: block; height: 100%; background: #34d399; transition: width 80ms linear; }
  .meter span.hot { background: #ef4444; }
  .content { position: relative; min-height: 100%; flex: none; }
  .ruler { height: 22px; position: sticky; top: 0; z-index: 4; background: var(--panel); border-bottom: 1px solid var(--border); cursor: ew-resize; user-select: none; }
  .tick { position: absolute; bottom: 0; height: 5px; border-left: 1px solid var(--border); font-size: 10px; color: var(--muted); padding-left: 3px; line-height: 1; }
  .tick.major { height: 14px; }
  .lane { position: relative; height: 44px; border-bottom: 1px solid rgb(42 47 56 / 0.6); }
  .lane.audio { height: 34px; }
  .lane.fx { height: 28px; }
  .lane-label.fxl { height: 28px; gap: 4px; padding: 0 4px 0 8px; }
  .lane-label.fxl .tname { flex: 1; }
  .lane.amb { height: 26px; }
  .lane.muted { opacity: 0.45; }
  .ambbar {
    position: absolute; box-sizing: border-box; padding: 0 8px; font-size: 10px; text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    border-radius: 4px; background: rgb(96 165 250 / 0.16); border: 1px solid rgb(96 165 250 / 0.55); color: var(--text); cursor: pointer;
  }
  .ambbar.first { border-left-width: 4px; }
  .ambbar.last { border-right-width: 4px; }
  .ambbar.selected { border-color: #fbbf24; box-shadow: 0 0 0 1px #fbbf24; }
  .ambdrop { top: 2px; height: 22px; }
  .amb-title { margin: 0; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .drop.audio { top: 3px; height: 28px; }
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
  .playhead { position: absolute; top: 0; bottom: 0; width: 0; border-left: 2px solid #f87171; pointer-events: none; z-index: 5; }
  .playhead.rec { border-left-color: #ef4444; box-shadow: 0 0 6px #ef4444; }
  .ghost {
    position: fixed; z-index: 30; pointer-events: none; padding: 2px 8px; font-size: 12px; border-radius: 6px;
    background: var(--panel-2); border: 1px solid var(--accent);
  }
  .voices-open { margin-top: 8px; padding: 2px 10px; font-size: 12px; }
  .ms.voices.on { border-color: var(--accent); color: var(--accent); }
</style>
