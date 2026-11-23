<script lang="ts">
  // Lector táctil: el visor WebGL del escritorio (Viewer.svelte, con teselas, tira vertical y
  // encuadre) a pantalla completa. Tocar a los lados o deslizar pasa de página; tocar en el centro
  // enseña u oculta los controles; pellizcar hace zoom y el doble toque amplía.
  //
  // Reproducir (▶) usa el reproductor del Modo Acción del escritorio: la cámara, las transiciones y
  // los efectos de una historia `.imgxstory` (o, en un cómic, la cámara automática), con su sonido
  // por el motor de Rust (el mismo director que en el escritorio). Con la historia se abre una
  // portada con su título y sus créditos. En un álbum de fotos, ▶ es un pase de fotos con la
  // transición, el tiempo y el movimiento que se eligen en ✨ (SlideshowSheet.svelte).
  import { onDestroy, onMount, tick } from "svelte";
  import Viewer from "$imgx/viewer/Viewer.svelte";
  import type { Framing, ReadLayout } from "$imgx/reading/layout";
  import type { PlayerInfo } from "$imgx/action/player";
  import { ProjectStore } from "$imgx/action/project.svelte";
  import { projectSequence } from "$imgx/action/sequence.svelte";
  import { adjustUniforms, normalize } from "$imgx/adjust/adjust";
  import { levels } from "$imgx/adjust/levels.svelte";
  import { log } from "$imgx/log";
  import { director } from "$imgx/audio/director";
  import { trackSpecs } from "$imgx/audio/tracks";
  import PlayBar from "./PlayBar.svelte";
  import SlideshowSheet from "./SlideshowSheet.svelte";
  import { reducedMotion, show, showSequence } from "./lib/slideshow";
  import { audioFocus, audioPause, closeSource, detectPanels, library, onInterrupt, openSource, pictureRef, type Item, type SourceInfo } from "./lib/api";
  import { t, type Key } from "./lib/i18n";
  import { reading, saveReading, type Mode, type Quality, type StoryFit } from "./lib/reading";

  let { item, onclose }: { item: Item; onclose: () => void } = $props();

  let source = $state<SourceInfo | null>(null);
  let error = $state<string | null>(null);
  let index = $state(0);
  let bars = $state(true);
  let viewer = $state<Viewer>();
  let mode = $state<Mode>(reading.mode);
  let rtl = $state(reading.rtl);
  let storyFit = $state<StoryFit>(reading.storyFit);
  let quality = $state<Quality>(reading.quality);
  let muted = $state(reading.muted);
  /** Portada de la historia (título, créditos, ▶). */
  let intro = $state(false);
  /** Se está reproduciendo (desde que se pulsa ▶ hasta que se para). */
  let playing = $state(false);
  let playerInfo = $state<PlayerInfo | null>(null);
  const store = new ProjectStore();

  const count = $derived(source?.entries.length ?? 0);
  const story = $derived(source?.story ?? null);
  const current = $derived(source && count ? pictureRef(source, index) : null);
  const neighbours = $derived(
    source ? [index + 1, index - 1, index + 2].filter((i) => i >= 0 && i < count).map((i) => pictureRef(source!, i)) : [],
  );
  const layout = $derived<ReadLayout | null>(
    source && mode === "strip" && !playing ? { kind: "strip", id: String(source.generation), count, at: index } : null,
  );
  const framing = $derived<Framing>({ fit: mode === "width" ? "width" : "window", rtl, box: null });

  // Ajustes de imagen (🎨) que trae el proyecto, como en el escritorio (sin el modo noche).
  const adjustAt = $derived.by(() => {
    const s = source;
    const data = store.data;
    if (!s || !data) return null;
    void levels.rev;
    const own = new Map(data.items.filter((i) => i.adjust).map((i) => [i.entry, normalize($state.snapshot(i.adjust))]));
    const all = normalize($state.snapshot(data.defaults.adjust));
    if (!own.size && !all) return null;
    return (i: number) => {
      const name = s.entries[i]?.name;
      const adj = (name !== undefined ? own.get(name) : null) ?? all;
      return adjustUniforms(adj, adj?.clean ? levels.get(i) : null, null);
    };
  });

  let hideTimer = 0;
  function showBars(ms = 2500) {
    bars = true;
    clearTimeout(hideTimer);
    hideTimer = window.setTimeout(() => (bars = false), ms);
  }

  onMount(async () => {
    try {
      const s = await openSource(item.path);
      await store.load(s.generation, { kind: s.kind, title: s.title, fingerprint: s.fingerprint }, s.project);
      levels.reset(s.generation);
      index = Math.min(item.page, Math.max(0, s.entries.length - 1));
      // La primera vez, una historia se lee como se pensó (sin cambiar la preferencia de los cómics).
      if (s.story && !item.opened) {
        const r = s.story.reading;
        mode = r.strip ? "strip" : r.fit === "width" ? "width" : "page";
        rtl = !!r.rtl;
      }
      source = s;
      if (s.story) {
        intro = true;
        bars = false;
      } else showBars();
    } catch (e) {
      error = String(e);
    }
  });
  onDestroy(() => {
    clearTimeout(hideTimer);
    clearTimeout(saveTimer);
    clearTimeout(rotateTimer);
    releaseWake();
    void audioFocus(false);
    if (source) library.progress(item.id, index);
    store.unload();
    closeSource();
  });

  // Progreso: se guarda poco después de cambiar de página.
  let saveTimer = 0;
  $effect(() => {
    const i = index;
    if (!source) return;
    clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => library.progress(item.id, i), 600);
  });

  function go(i: number) {
    if (!count) return;
    const to = Math.max(0, Math.min(count - 1, i));
    const p = viewer?.playback();
    if (p) p.goTo(to);
    else index = to;
  }
  /** Página siguiente en el sentido de lectura (`dir` 1) o la anterior (−1). */
  const step = (dir: 1 | -1) => go(index + dir);

  function tap(x: number) {
    const w = window.innerWidth;
    if ((playing || mode !== "strip") && x < w * 0.3) return step(rtl ? 1 : -1);
    if ((playing || mode !== "strip") && x > w * 0.7) return step(rtl ? -1 : 1);
    if (bars) {
      bars = false;
      clearTimeout(hideTimer);
    } else showBars(4000);
  }

  function swipe(dx: number, dy: number): boolean {
    if (mode === "strip" && !playing) return false;
    // Con zoom (pellizco) el dedo mueve la imagen; ajustada al ancho, el desplazamiento es vertical.
    if (!playing && mode === "page" && viewer?.isZoomedIn()) return false;
    if (Math.abs(dx) < 50 || Math.abs(dx) < Math.abs(dy) * 1.5) return false;
    const forward = dx < 0 !== rtl;
    step(forward ? 1 : -1);
    return true;
  }

  const MODES: Mode[] = ["page", "width", "strip"];
  const modeLabel = { page: () => `📄 ${t("modePage")}`, width: () => `↔ ${t("modeWidth")}`, strip: () => `📜 ${t("modeStrip")}` };
  function nextMode() {
    mode = MODES[(MODES.indexOf(mode) + 1) % MODES.length];
    saveReading({ mode, rtl });
    showBars();
  }
  function toggleRtl() {
    rtl = !rtl;
    saveReading({ mode, rtl });
    showBars();
  }

  // ── Reproducción ──
  /** Proporción para la que se pensó la cámara (solo si la historia la trae). */
  const authored = $derived(story?.authoredAspect && story.authoredAspect > 0 ? story.authoredAspect : null);
  let winW = $state(window.innerWidth);
  let winH = $state(window.innerHeight);
  /** El móvil está en horizontal. */
  const landscape = $derived(winW > winH);
  /** «Como se pensó» de verdad: elegido, o automático con el móvil en horizontal. En vertical, la
   *  banda sería muy pequeña: toda la pantalla, y la cámara de la plantilla se acerca a la viñeta. */
  const asAuthored = $derived(storyFit === "authored" || (storyFit === "auto" && landscape));
  /** Al reproducir «como se pensó», el visor se estrecha a esa proporción (con bandas negras). */
  const frame = $derived.by(() => {
    if (!playing || !authored || !asAuthored || !winW || !winH) return null;
    let w = winW;
    let h = Math.round(winW / authored);
    if (h > winH) {
      h = winH;
      w = Math.round(winH * authored);
    }
    return { x: Math.round((winW - w) / 2), y: Math.round((winH - h) / 2), w, h };
  });

  /** Resolución al reproducir: la de la pantalla (alta), 1,25 (ahorro) o automática. */
  let autoRatio = $state(Math.min(window.devicePixelRatio || 1, 2));
  const pixelRatio = $derived(quality === "high" ? Infinity : quality === "saver" ? 1.25 : autoRatio);
  let slow = 0;
  /** En automático, si va a menos de 40 fps durante un segundo y medio, se baja la resolución. */
  function watchFps(fps: number) {
    if (!playing || quality !== "auto" || !fps || playerInfo?.state !== "playing") return;
    slow = fps < 40 ? slow + 1 : 0;
    if (slow >= 3 && autoRatio > 1) {
      autoRatio = Math.max(1, autoRatio - 0.5);
      slow = 0;
      log.info(`reproducción a ${fps} fps: resolución ×${autoRatio}`);
    }
  }

  const sequence = $derived(source ? projectSequence(store, source.entries.map((e) => e.name), (i) => detectPanels(source!.generation, i)) : null);

  // ── Pase de fotos (álbumes) ──
  const album = $derived(item.kind === "album" && !story);
  let showOpen = $state(false);
  /** Sube cada vez que cambia un ajuste del pase (está fuera de Svelte, en `show`). */
  let showRev = $state(0);
  const playSequence = $derived.by(() => {
    void showRev;
    if (!sequence) return null;
    return album ? showSequence(sequence, { ...show }, rtl, reducedMotion()) : sequence;
  });
  const showSummary = $derived.by(() => {
    void showRev;
    return album ? t("showSummary", { tr: t(`tr_${show.transition}` as Key), n: show.seconds }) : null;
  });
  function openShow() {
    showOpen = true;
    clearTimeout(hideTimer);
  }
  function closeShow() {
    showOpen = false;
    if (playing) showBars();
  }
  function showChanged() {
    showRev++;
    // Mientras suena, el cambio se ve ya: la foto actual vuelve a empezar con el pase nuevo.
    if (playing) void restart();
  }

  /** Empieza a reproducir desde la imagen `from` (por defecto, la actual). */
  async function play(from = index) {
    if (!source || !playSequence || !count) return;
    intro = false;
    playing = true;
    // El visor cambia de tamaño (encuadre «como se pensó») antes de que el reproductor mire su proporción.
    await tick();
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    if (!viewer || !playing) return;
    // Volúmenes y pistas del proyecto antes del primer sonido (el silencio es del móvil).
    director.levels($state.snapshot(store.mix), 1, muted);
    director.tracks(trackSpecs($state.snapshot(store.tracks)), []);
    // Los tamaños de las onomatopeyas y los subtítulos se pensaron para la proporción de la historia
    // (16:9 si no la dice): en vertical se miden sobre ella para que quepan.
    viewer.play({ ...playSequence!, start: from, count, onIndex: (i) => (index = i), sound: director, fxFit: authored ?? 16 / 9 });
    bars = false;
    void holdWake();
  }

  function toggleMute() {
    muted = !muted;
    saveReading({ muted });
    director.levels($state.snapshot(store.mix), 1, muted);
  }

  function stopPlay() {
    viewer?.stop();
    playing = false;
    playerInfo = null;
    releaseWake();
    void audioFocus(false);
    showBars();
  }

  function onPlayer(p: PlayerInfo | null) {
    const was = playerInfo?.state;
    playerInfo = p;
    if (!p) return;
    // Al acabar o al pausar, los controles a la vista; la pantalla puede volver a apagarse y el
    // sonido de otras apps, volver a sonar.
    if (p.state !== was && (p.state === "ended" || p.state === "paused")) {
      bars = true;
      clearTimeout(hideTimer);
      releaseWake();
      void audioFocus(false);
      // Pase de fotos con «Repetir al acabar»: vuelve a la primera foto.
      if (p.state === "ended" && album && show.loop && p.index >= count - 1 && !showOpen) {
        window.setTimeout(() => {
          if (playing && playerInfo?.state === "ended") void replay();
        }, 300);
      }
    } else if (p.state !== was && p.state === "playing") {
      void holdWake();
      void audioFocus(true);
    }
  }

  /** Pausa lo que suena (al salir de la app, una llamada, auriculares fuera). */
  function pauseNow() {
    const p = viewer?.playback();
    const info = playerInfo;
    if (!p || !info || ["paused", "ended"].includes(info.state)) return;
    p.toggle();
    // Sin fotogramas (la app en segundo plano) el reproductor no llegaría a decírselo al sonido.
    void audioPause(info.t, info.speed);
  }

  /** Cambiar el encuadre o la calidad mientras se reproduce: se vuelve a empezar la imagen actual. */
  async function restart() {
    if (!playing) return;
    const paused = playerInfo?.state === "paused";
    viewer?.stop();
    await play(index);
    if (paused) viewer?.playback()?.toggle();
  }
  // Al girar el móvil mientras se reproduce, la cámara se rehace para la pantalla nueva (las
  // plantillas dependen de la proporción y el encuadre automático cambia). Se espera un poco: al
  // girar, el ancho y el alto no llegan a la vez.
  let lastLandscape = window.innerWidth > window.innerHeight;
  let rotateTimer = 0;
  $effect(() => {
    const l = landscape;
    if (l === lastLandscape) return;
    lastLandscape = l;
    clearTimeout(rotateTimer);
    rotateTimer = window.setTimeout(() => {
      if (playing && playerInfo?.state !== "ended") void restart();
    }, 300);
  });

  async function replay() {
    viewer?.stop();
    await play(0);
  }
  function toggleFit() {
    const all: StoryFit[] = ["auto", "authored", "screen"];
    storyFit = all[(all.indexOf(storyFit) + 1) % all.length];
    saveReading({ storyFit });
    void restart();
  }
  const QUALITIES: Quality[] = ["auto", "high", "saver"];
  function nextQuality() {
    quality = QUALITIES[(QUALITIES.indexOf(quality) + 1) % QUALITIES.length];
    saveReading({ quality });
  }

  // La pantalla no se apaga mientras se reproduce.
  let wake: WakeLockSentinel | null = null;
  async function holdWake() {
    if (wake || document.visibilityState !== "visible") return;
    try {
      wake = (await navigator.wakeLock?.request("screen")) ?? null;
      wake?.addEventListener("release", () => (wake = null));
    } catch (e) {
      log.info("sin bloqueo de pantalla:", e);
    }
  }
  function releaseWake() {
    void wake?.release().catch(() => {});
    wake = null;
  }

  // Al salir de la app (otra app, bloquear el móvil), con una llamada, cuando otra app empieza a
  // sonar o al desconectar los auriculares, la historia se pausa.
  onMount(() => {
    const onVisible = () => document.visibilityState === "hidden" && pauseNow();
    document.addEventListener("visibilitychange", onVisible);
    let stopListening = () => {};
    let gone = false;
    void onInterrupt((reason) => {
      log.info(`sonido interrumpido (${reason})`);
      pauseNow();
    }).then((f) => (gone ? f() : (stopListening = f)));
    return () => {
      gone = true;
      document.removeEventListener("visibilitychange", onVisible);
      stopListening();
    };
  });
</script>

<svelte:window bind:innerWidth={winW} bind:innerHeight={winH} />

<div class="reader">
  {#if error}
    <div class="error">
      <p>{t("openError", { err: error })}</p>
      <button onclick={onclose}>{t("close")}</button>
    </div>
  {:else if source}
    <div
      class="stage"
      style:left={frame ? `${frame.x}px` : null}
      style:top={frame ? `${frame.y}px` : null}
      style:width={frame ? `${frame.w}px` : null}
      style:height={frame ? `${frame.h}px` : null}
    >
      <Viewer
        bind:this={viewer}
        item={current}
        prefetch={neighbours}
        refAt={(i) => pictureRef(source!, i)}
        background="#000000"
        {layout}
        {framing}
        adjust={adjustAt}
        playPixelRatio={pixelRatio}
        onindex={(i) => (index = i)}
        onplayer={onPlayer}
        onstatus={(s) => watchFps(s.fps)}
        ontap={(x) => tap(x)}
        onswipe={(dx, dy) => swipe(dx, dy)}
      />
    </div>
  {/if}

  {#if intro && source && story}
    <div class="intro">
      <div class="card">
        <small>🎬 {t("story")}</small>
        <h2>{story.title || source.title}</h2>
        {#if story.credits?.trim()}<p class="credits">{story.credits.trim()}</p>{/if}
        <button class="primary" onclick={() => play()}>▶ {index > 0 ? t("storyResume", { page: index + 1 }) : t("storyPlay")}</button>
        {#if index > 0}<button onclick={() => play(0)}>↻ {t("replay")}</button>{/if}
        <button onclick={() => { intro = false; showBars(); }}>📖 {t("storyRead")}</button>
      </div>
    </div>
  {/if}

  {#if bars && source && !intro}
    <div class="top">
      <button class="back" onclick={onclose} aria-label={t("back")}>←</button>
      <span class="title">{story?.title || item.title}</span>
      <span class="num">{index + 1} / {count}</span>
    </div>
    <div class="bottom">
      {#if playing && playerInfo}
        <PlayBar
          info={playerInfo}
          {count}
          player={() => viewer?.playback() ?? null}
          fit={authored ? storyFit : null}
          {quality}
          {muted}
          onmute={toggleMute}
          onstop={stopPlay}
          onfit={toggleFit}
          onquality={nextQuality}
          onreplay={replay}
          show={showSummary}
          onshow={openShow}
        />
      {:else}
        {#if count > 1}
          <input
            type="range"
            min="0"
            max={count - 1}
            value={index}
            dir={rtl ? "rtl" : "ltr"}
            oninput={(e) => {
              go(+(e.currentTarget as HTMLInputElement).value);
              showBars(4000);
            }}
          />
        {/if}
        <div class="row">
          <button class="play" onclick={() => play()} title={t("playHint")}>▶ {t("play")}</button>
          {#if album}
            <button onclick={openShow} title={t("showHint")} aria-haspopup="dialog">✨ {t("showOpen")}</button>
          {/if}
          <button onclick={nextMode} title={t("modeHint")}>{modeLabel[mode]()}</button>
          <button class:on={rtl} onclick={toggleRtl} title={t("mangaHint")}>⇄ {t("manga")}</button>
        </div>
      {/if}
    </div>
  {/if}

  {#if showOpen && source}
    <SlideshowSheet
      {playing}
      onchange={showChanged}
      onplay={() => {
        showOpen = false;
        void play();
      }}
      onclose={closeShow}
    />
  {/if}
</div>

<style>
  .reader { position: fixed; inset: 0; background: #000; }
  .stage { position: absolute; inset: 0; }
  .top, .bottom {
    position: absolute; left: 0; right: 0; z-index: 2;
    background: rgb(15 18 23 / 0.88); backdrop-filter: blur(8px);
  }
  .top {
    top: 0; display: flex; align-items: center; gap: 10px;
    padding: calc(env(safe-area-inset-top) + 6px) 12px 8px;
  }
  .back { background: none; border: 0; color: var(--text); font-size: 24px; padding: 4px 10px 4px 2px; }
  .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }
  .num { color: var(--muted); font-variant-numeric: tabular-nums; }
  .bottom { bottom: 0; padding: 10px 16px calc(env(safe-area-inset-bottom) + 10px); }
  input[type="range"] { width: 100%; accent-color: var(--accent); }
  .row { display: flex; gap: 8px; justify-content: center; flex-wrap: wrap; margin-top: 6px; }
  .row button {
    border: 0; border-radius: 999px; padding: 9px 14px; font: inherit; font-size: 15px;
    background: #2a313b; color: var(--text);
  }
  .row button.on, .row button.play { background: var(--accent); color: white; }
  .intro {
    position: absolute; inset: 0; z-index: 3; display: flex; align-items: flex-end; justify-content: center;
    background: linear-gradient(to top, rgb(0 0 0 / 0.92) 30%, rgb(0 0 0 / 0.35));
    padding: 24px 18px calc(env(safe-area-inset-bottom) + 24px);
  }
  .card { width: 100%; max-width: 460px; display: flex; flex-direction: column; gap: 10px; }
  .card small { color: var(--accent); font-weight: 600; }
  .card h2 { margin: 0; font-size: 24px; }
  .credits { margin: 0; color: var(--muted); white-space: pre-line; max-height: 30vh; overflow-y: auto; }
  .card button {
    border: 0; border-radius: 14px; padding: 14px; font: inherit; font-size: 16px; font-weight: 600;
    background: #2a313b; color: var(--text);
  }
  .card button.primary { background: var(--accent); color: white; font-size: 18px; }
  .error { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 14px; padding: 24px; text-align: center; }
  .error button { border: 0; border-radius: 12px; padding: 12px 22px; background: var(--panel); color: var(--text); font: inherit; }
</style>
