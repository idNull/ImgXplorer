<script lang="ts">
  // 🗣 Voces IA (en la columna del inspector del editor). «Reparto»: los personajes y su voz
  // (crearla con Voice Design o elegir una de la cuenta). «Guion»: las frases de esta imagen, con
  // etiquetas de interpretación; «Revisar y generar» enseña el pedido antes de enviar nada.
  import { untrack } from "svelte";
  import type { ProjectStore } from "../action/project.svelte";
  import { prefs } from "../prefs.svelte";
  import { t, label, type Key } from "../i18n.svelte";
  import { sounds } from "../audio/sounds.svelte";
  import { CAST_COLORS, LINE_KINDS, TAGS, newId, type CastMember, type GenItem, type Line, type LineKind, type VoiceInfo } from "./types";
  import { newAudioClip } from "../editor/model";
  import { approveAndGenerate, design, generateLines, lineItem, listVoices, markStale, placeLine, playPreview, saveDesign, spoken } from "./voices.svelte";

  interface Props {
    store: ProjectStore;
    entry: string;
    size: [number, number] | null;
    /** Segundo del cabezal (para «empieza aquí»). */
    playhead: number;
    onseek: (t: number) => void;
    onnotice: (msg: string) => void;
    onclose: () => void;
    /** Quitar el clip de una frase al borrarla. */
    onremoveclip: (lineId: string) => void;
  }
  let { store, entry, size, playhead, onseek, onnotice, onclose, onremoveclip }: Props = $props();

  let tab = $state<"cast" | "script" | "sounds">(untrack(() => (store.cast.length ? "script" : "cast")));
  const cast = $derived(store.cast);
  const lines = $derived(store.linesFor(entry));
  const ro = $derived(!!store.readOnly || !size);
  const KIND_ICON: Record<LineKind, string> = { dialogue: "💬", thought: "💭", narration: "📜", crowd: "👥" };

  // ------------------------------------------------------------ reparto
  let open = $state<string | null>(null);

  function addCast() {
    const id = newId("c");
    store.editCast((l) => l.push({ id, name: t("voicesNewCast", { n: l.length + 1 }), color: CAST_COLORS[l.length % CAST_COLORS.length] }));
    open = id;
  }
  /** `voice`: el cambio afecta a cómo suena (voz, ajustes, idioma): sus frases quedan «cambiadas». */
  function editCast(id: string, fn: (c: CastMember) => void, voice = false) {
    store.batch(true);
    try {
      store.editCast((l) => {
        const c = l.find((x) => x.id === id);
        if (c) fn(c);
      });
      if (voice) store.markCastStale(id);
    } finally {
      store.batch(false);
    }
  }
  /** Borrar pide una segunda pulsación (las frases de ese personaje se quedan sin voz). */
  let removing = $state<string | null>(null);
  function removeCast(c: CastMember) {
    if (removing !== c.id) {
      removing = c.id;
      setTimeout(() => removing === c.id && (removing = null), 3000);
      return;
    }
    removing = null;
    store.editCast((l) => l.splice(l.findIndex((x) => x.id === c.id), 1));
  }
  function cycleColor(c: CastMember) {
    const i = CAST_COLORS.indexOf(c.color ?? "");
    editCast(c.id, (x) => (x.color = CAST_COLORS[(i + 1) % CAST_COLORS.length]));
  }

  // Crear voz (Voice Design): 3 muestras para escuchar y elegir una.
  let designing = $state<{ cast: string; busy: boolean; previews: { generatedId: string; seconds: number }[]; playing: string | null } | null>(null);
  const real = $derived(prefs.settings.voicesProvider !== "mock");

  async function startDesign(c: CastMember) {
    const desc = (c.description ?? "").trim();
    if (desc.length < 20) return onnotice(t("voicesDescShort"));
    if (real && !prefs.settings.voicesConsent) return onnotice(t("voicesConsentFirst"));
    designing = { cast: c.id, busy: true, previews: [], playing: null };
    try {
      const previews = await design(desc);
      if (designing?.cast === c.id) designing = { ...designing, busy: false, previews };
    } catch (e) {
      designing = null;
      onnotice(String(e));
    }
  }
  function togglePreview(id: string) {
    if (!designing) return;
    const stop = designing.playing === id;
    designing.playing = stop ? null : id;
    playPreview(stop ? null : id).catch((e) => onnotice(String(e)));
  }
  async function pickPreview(c: CastMember, generatedId: string) {
    if (!designing) return;
    designing.busy = true;
    playPreview(null).catch(() => {});
    try {
      const v = await saveDesign(c.name, c.description ?? "", generatedId);
      editCast(c.id, (x) => (x.voice = { provider: prefs.settings.voicesProvider, id: v.id, name: v.name }), true);
      onnotice(t("voicesVoiceSaved", { name: v.name }));
      designing = null;
    } catch (e) {
      if (designing) designing.busy = false;
      onnotice(String(e));
    }
  }

  // Elegir una voz de la cuenta.
  let voices = $state<VoiceInfo[] | null>(null);
  let loadingVoices = $state(false);
  async function loadVoices() {
    loadingVoices = true;
    try {
      voices = await listVoices();
    } catch (e) {
      onnotice(String(e));
    } finally {
      loadingVoices = false;
    }
  }
  function setVoice(c: CastMember, id: string) {
    const v = voices?.find((x) => x.id === id);
    editCast(c.id, (x) => {
      if (!v) delete x.voice;
      else x.voice = { provider: prefs.settings.voicesProvider, id: v.id, name: v.name };
    }, true);
  }

  // Probar la voz con una frase corta (pasa por el resumen del pedido).
  let testing = $state<string | null>(null);
  async function testVoice(c: CastMember) {
    const r = lineItem({ id: "test", cast: c.id, text: t("voicesTestLine", { name: c.name }), kind: "dialogue" }, cast);
    if ("error" in r) return onnotice(r.error);
    testing = c.id;
    try {
      const res = await approveAndGenerate([{ who: c.name, color: c.color, item: r.item, cached: false, on: true }], false);
      const s = res?.results[0];
      if (s?.error) onnotice(s.error);
      else if (s?.sound) sounds.preview(s.sound.id);
    } catch (e) {
      onnotice(String(e));
    } finally {
      testing = null;
    }
  }

  const SETTINGS = [
    { key: "stability", min: 0, max: 1, step: 0.05, def: 0.5 },
    { key: "style", min: 0, max: 1, step: 0.05, def: 0 },
    { key: "speed", min: 0.7, max: 1.2, step: 0.05, def: 1 },
  ] as const;
  function setSetting(c: CastMember, key: string, v: number | null) {
    editCast(c.id, (x) => {
      const s = { ...(x.settings ?? {}) };
      if (v === null) delete s[key];
      else s[key] = v;
      if (Object.keys(s).length) x.settings = s;
      else delete x.settings;
    }, true);
  }

  // ------------------------------------------------------------ guion
  let lastCast = $state<string | undefined>(undefined);
  let focused = $state<{ id: string; el: HTMLTextAreaElement } | null>(null);

  function editLine(id: string, fn: (l: Line) => void, stale = true) {
    if (!size) return;
    store.editLines(entry, size, (list) => {
      const l = list.find((x) => x.id === id);
      if (!l) return;
      fn(l);
      if (stale) markStale(l);
    });
  }
  function addLine() {
    if (!size) return;
    const c = lastCast ?? cast[0]?.id;
    store.editLines(entry, size, (list) => list.push({ id: newId("l"), ...(c ? { cast: c } : {}), text: "", kind: c ? "dialogue" : "narration", status: "draft" }));
  }
  function removeLine(l: Line) {
    if (!size) return;
    store.batch(true);
    try {
      onremoveclip(l.id);
      store.editLines(entry, size, (list) => list.splice(list.findIndex((x) => x.id === l.id), 1));
    } finally {
      store.batch(false);
    }
  }
  function moveLine(l: Line, d: -1 | 1) {
    if (!size) return;
    store.editLines(entry, size, (list) => {
      const i = list.findIndex((x) => x.id === l.id);
      const j = i + d;
      if (i < 0 || j < 0 || j >= list.length) return;
      [list[i], list[j]] = [list[j], list[i]];
    });
  }
  function insertTag(tag: string) {
    const f = focused;
    if (!f) return onnotice(t("voicesTagHint"));
    const el = f.el;
    const a = el.selectionStart ?? el.value.length;
    const b = el.selectionEnd ?? a;
    const ins = `[${tag}] `;
    const text = el.value.slice(0, a) + ins + el.value.slice(b);
    editLine(f.id, (l) => (l.text = text));
    requestAnimationFrame(() => {
      el.focus();
      el.selectionStart = el.selectionEnd = a + ins.length;
    });
  }

  let progress = $state<string | null>(null);
  async function generateAll(ids: string[] | null) {
    if (!size) return;
    progress = "…";
    try {
      const r = await generateLines({ store, entry, size }, ids, false, (d, n) => (progress = t("voicesProgress", { d, n })));
      if (!r) return;
      if (r.errors.length) onnotice(r.errors[0]);
      else if (r.done) onnotice(t("voicesDone", { n: r.done, cached: r.cached }));
      else onnotice(t("voicesNothing"));
    } catch (e) {
      onnotice(String(e));
    } finally {
      progress = null;
    }
  }

  /** Otra toma: la misma frase con otra semilla (no sale de la caché). */
  async function retake(l: Line) {
    if (!size) return;
    const r = lineItem(l, cast);
    if ("error" in r) return onnotice(r.error);
    if (r.item.kind === "speech") r.item.seed = Math.floor(Math.random() * 4_000_000_000);
    progress = "…";
    try {
      const res = await approveAndGenerate([{ who: r.who, color: r.color, item: r.item, cached: false, on: true, line: l.id }], false);
      const s = res?.results[0];
      if (s?.error) return onnotice(s.error);
      if (!s?.sound) return;
      placeLine({ store, entry, size }, l, s.sound, r.who);
      onnotice(t("voicesRetaken"));
    } finally {
      progress = null;
    }
  }

  // ------------------------------------------------------------ efectos y música
  let gen = $state({ kind: "sfx" as "sfx" | "music", text: "", seconds: 5, loop: false, instrumental: true });

  /** Genera un efecto o música para la biblioteca; con `place`, también en el cabezal de esta imagen. */
  async function generateSoundHere(place: boolean) {
    const text = gen.text.trim();
    if (!text) return;
    const item: GenItem =
      gen.kind === "sfx"
        ? { kind: "sfx", text, seconds: Math.min(30, Math.max(0.5, gen.seconds || 5)), loop: gen.loop }
        : { kind: "music", prompt: text, seconds: Math.min(600, Math.max(3, gen.seconds || 30)), instrumental: gen.instrumental };
    progress = "…";
    try {
      const who = gen.kind === "sfx" ? t("voicesSfxWho") : t("voicesMusicWho");
      const r = await approveAndGenerate([{ who, item, cached: false, on: true }], false);
      const res = r?.results[0];
      if (!res) return;
      if (res.error || !res.sound) return onnotice(res.error ?? "?");
      if (place && size) {
        const c = newAudioClip(res.sound, playhead);
        if (gen.kind === "music") c.volume = 0.6;
        store.editAudio(entry, size, (a) => a.sfx.push(c));
      }
      onnotice(place ? t("voicesSoundPlaced") : t("voicesSoundInLibrary", { name: res.sound.name }));
    } catch (e) {
      onnotice(String(e));
    } finally {
      progress = null;
    }
  }

  const pending = $derived(lines.filter((l) => l.status !== "generated" && l.text.trim()).length);
  const castOf = (id?: string) => cast.find((c) => c.id === id);
  const statusKey = (l: Line): Key => (l.status === "generated" ? "voicesStGenerated" : l.status === "stale" ? "voicesStStale" : "voicesStDraft");
  const num = (e: Event) => Number((e.currentTarget as HTMLInputElement).value);
</script>

<div class="panel">
  <header>
    <b>🗣 {t("voicesTitle")}</b>
    {#if prefs.settings.voicesProvider === "mock"}<span class="mock" title={t("voicesMockHelp")}>{t("voicesMockBadge")}</span>{/if}
    <button class="x" onclick={onclose} title={t("close")}>✕</button>
  </header>
  <div class="tabs">
    <button class:on={tab === "cast"} onclick={() => (tab = "cast")}>{t("voicesCast")} ({cast.length})</button>
    <button class:on={tab === "script"} onclick={() => (tab = "script")}>{t("voicesScript")} ({lines.length})</button>
    <button class:on={tab === "sounds"} onclick={() => (tab = "sounds")} title={t("voicesSoundsHelp")}>{t("voicesSounds")}</button>
  </div>

  {#if tab === "cast"}
    {#if !cast.length}<p class="note">{t("voicesCastEmpty")}</p>{/if}
    {#each cast as c (c.id)}
      <div class="card" style:--c={c.color ?? "var(--muted)"}>
        <div class="chead">
          <button class="dot" title={t("voicesColor")} onclick={() => cycleColor(c)} disabled={ro}></button>
          <button class="cname" onclick={() => (open = open === c.id ? null : c.id)}>
            {open === c.id ? "▾" : "▸"} {c.name}
            <span class="vstate" class:ok={!!c.voice}>{c.voice ? `· ${c.voice.name ?? c.voice.id}` : `· ${t("voicesNoVoice")}`}</span>
          </button>
        </div>
        {#if open === c.id}
          <label class="field"><span>{t("voicesName")}</span>
            <input value={c.name} disabled={ro} onchange={(e) => editCast(c.id, (x) => (x.name = e.currentTarget.value.trim().slice(0, 60) || x.name))} />
          </label>
          <label class="field col"><span>{t("voicesDesc")}</span>
            <textarea rows="3" value={c.description ?? ""} disabled={ro} placeholder={t("voicesDescPlaceholder")}
              onchange={(e) => editCast(c.id, (x) => { const v = e.currentTarget.value.trim().slice(0, 1000); if (v) x.description = v; else delete x.description; })}></textarea>
          </label>
          <label class="field"><span>{t("voicesLanguage")}</span>
            <select value={c.language ?? ""} disabled={ro} onchange={(e) => editCast(c.id, (x) => { const v = e.currentTarget.value; if (v) x.language = v; else delete x.language; }, true)}>
              <option value="">{t("voicesLangAuto")}</option>
              {#each ["es", "en", "fr", "it", "de", "pt", "ja", "ko", "zh"] as l (l)}<option value={l}>{label(`lang.${l}`)}</option>{/each}
            </select>
          </label>
          <div class="btns">
            <button disabled={ro || !!designing?.busy} onclick={() => startDesign(c)} title={t("voicesDesignHelp")}>🎙 {t("voicesDesign")}</button>
            <button disabled={ro || loadingVoices} onclick={loadVoices}>{loadingVoices ? "…" : t("voicesPick")}</button>
            {#if c.voice}<button disabled={!!testing} onclick={() => testVoice(c)}>{testing === c.id ? "…" : `▶ ${t("voicesTry")}`}</button>{/if}
          </div>
          {#if real && !prefs.settings.voicesConsent}
            <label class="consent"><input type="checkbox" onchange={(e) => { prefs.settings.voicesConsent = e.currentTarget.checked; prefs.saveSettings(); }} /> {t("voicesConsent")}</label>
          {:else if real}<p class="note">{t("voicesDesignNote")}</p>{/if}
          {#if designing?.cast === c.id}
            <div class="previews">
              {#if designing.busy && !designing.previews.length}<p class="note">{t("voicesDesigning")}</p>{/if}
              {#each designing.previews as p, i (p.generatedId)}
                <div class="prev">
                  <button onclick={() => togglePreview(p.generatedId)}>{designing.playing === p.generatedId ? "■" : "▶"}</button>
                  <span>{t("voicesSample", { n: i + 1 })} · {p.seconds.toFixed(1)} s</span>
                  <button class="primary" disabled={designing.busy} onclick={() => pickPreview(c, p.generatedId)}>{t("voicesChoose")}</button>
                </div>
              {/each}
              {#if designing.previews.length}<button class="link" onclick={() => { playPreview(null); designing = null; }}>{t("cancel")}</button>{/if}
            </div>
          {/if}
          {#if voices}
            <label class="field"><span>{t("voicesVoice")}</span>
              <select value={c.voice?.id ?? ""} disabled={ro} onchange={(e) => setVoice(c, e.currentTarget.value)}>
                <option value="">—</option>
                {#if c.voice && !voices.some((v) => v.id === c.voice?.id)}<option value={c.voice.id}>{c.voice.name ?? c.voice.id}</option>{/if}
                {#each voices as v (v.id)}<option value={v.id}>{v.name}{v.category ? ` (${v.category})` : ""}</option>{/each}
              </select>
            </label>
          {/if}
          <details>
            <summary>{t("voicesTuning")}</summary>
            {#each SETTINGS as s (s.key)}
              {@const v = c.settings?.[s.key] ?? s.def}
              <label class="field">
                <span>{label(`vs.${s.key}`)}</span>
                <input type="range" min={s.min} max={s.max} step={s.step} value={v} disabled={ro}
                  onchange={(e) => setSetting(c, s.key, num(e))} ondblclick={() => setSetting(c, s.key, null)} />
                <output>{s.key === "speed" ? `${v.toFixed(2)}×` : `${Math.round(v * 100)}%`}</output>
              </label>
            {/each}
          </details>
          <div class="btns"><button class="danger" disabled={ro} onclick={() => removeCast(c)}>{removing === c.id ? t("voicesCastRemoveSure") : t("voicesCastRemove")}</button></div>
        {/if}
      </div>
    {/each}
    <button class="add" disabled={ro} onclick={addCast}>＋ {t("voicesAddCast")}</button>
  {:else if tab === "sounds"}
    <p class="note">{t("voicesSoundsHelp")}</p>
    <div class="seg">
      <button class:on={gen.kind === "sfx"} onclick={() => (gen.kind = "sfx")}>💥 {t("voicesSfx")}</button>
      <button class:on={gen.kind === "music"} onclick={() => (gen.kind = "music")}>🎵 {t("voicesMusic")}</button>
    </div>
    <textarea rows="3" bind:value={gen.text} placeholder={gen.kind === "sfx" ? t("voicesSfxPlaceholder") : t("voicesMusicPlaceholder")}></textarea>
    <label class="field"><span>{t("voicesSeconds")}</span>
      <input type="number" min={gen.kind === "sfx" ? 0.5 : 3} max={gen.kind === "sfx" ? 30 : 600} step="0.5" bind:value={gen.seconds} />
      <output>s</output>
    </label>
    {#if gen.kind === "sfx"}
      <label class="check"><input type="checkbox" bind:checked={gen.loop} /> {t("voicesLoop")}</label>
    {:else}
      <label class="check"><input type="checkbox" bind:checked={gen.instrumental} /> {t("voicesInstrumental")}</label>
    {/if}
    <div class="gen">
      <button class="primary" disabled={!gen.text.trim() || !!progress} onclick={() => generateSoundHere(false)}>{progress ?? t("voicesSoundGenerate")}</button>
      <button disabled={ro || !gen.text.trim() || !!progress} title={t("voicesSoundPlaceHelp")} onclick={() => generateSoundHere(true)}>{t("voicesSoundPlace")}</button>
    </div>
    <p class="note">{t("voicesSoundsNote")}</p>
  {:else}
    {#if !lines.length}<p class="note">{t("voicesScriptEmpty")}</p>{/if}
    {#if lines.length}
      <div class="tags" title={t("voicesTagHint")}>
        {#each TAGS as tag (tag)}<button class="chip" disabled={ro} onpointerdown={(e) => e.preventDefault()} onclick={() => insertTag(tag)}>{label(`vt.${tag}`)}</button>{/each}
      </div>
    {/if}
    {#each lines as l, i (l.id)}
      {@const c = castOf(l.cast)}
      <div class="line" style:--c={c?.color ?? "var(--muted)"}>
        <div class="lhead">
          <select value={l.cast ?? ""} disabled={ro} onchange={(e) => { const v = e.currentTarget.value; lastCast = v || undefined; editLine(l.id, (x) => { if (v) x.cast = v; else delete x.cast; }); }}>
            <option value="">{t("voicesNobody")}</option>
            {#each cast as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
          </select>
          <select class="kind" value={l.kind} disabled={ro} title={t("voicesKindHelp")} onchange={(e) => editLine(l.id, (x) => (x.kind = e.currentTarget.value as LineKind))}>
            {#each LINE_KINDS as k (k)}<option value={k}>{KIND_ICON[k]} {label(`vk.${k}`)}</option>{/each}
          </select>
          <span class="st {l.status ?? 'draft'}">{t(statusKey(l))}</span>
        </div>
        <textarea rows="2" value={l.text} disabled={ro} placeholder={t("voicesLinePlaceholder")}
          onfocus={(e) => (focused = { id: l.id, el: e.currentTarget })}
          onchange={(e) => editLine(l.id, (x) => (x.text = e.currentTarget.value))}></textarea>
        <div class="lfoot">
          <span class="count" title={t("voicesCharsHelp")}>{[...l.text].length} {t("voicesChars")}</span>
          <span class="time" title={t("voicesStartHelp")}>
            ⏱ <input type="number" min="0" step="0.1" value={l.start ?? ""} placeholder={t("voicesAuto")} disabled={ro}
              onchange={(e) => editLine(l.id, (x) => { const v = e.currentTarget.value; if (v === "") delete x.start; else x.start = Math.max(0, Number(v) || 0); }, false)} />
            <button class="mini" disabled={ro} title={t("voicesStartHere")} onclick={() => editLine(l.id, (x) => (x.start = Number(playhead.toFixed(2))), false)}>⌖</button>
            {#if l.start !== undefined}<button class="mini" title={t("voicesSeek")} onclick={() => onseek(l.start ?? 0)}>↦</button>{/if}
          </span>
          <span class="grow"></span>
          {#if l.takes?.length}<button class="mini" title={t("voicesPlayTake")} onclick={() => sounds.preview(l.takes!.at(-1)!)}>▶</button>{/if}
          {#if l.status && l.status !== "draft"}<button class="mini" disabled={ro || !!progress} title={t("voicesRetakeHelp")} onclick={() => retake(l)}>↻</button>{/if}
          <button class="mini" disabled={ro || i === 0} title={t("voicesUp")} onclick={() => moveLine(l, -1)}>▲</button>
          <button class="mini" disabled={ro || i === lines.length - 1} title={t("voicesDown")} onclick={() => moveLine(l, 1)}>▼</button>
          <button class="mini" disabled={ro} title={t("voicesRemoveLine")} onclick={() => removeLine(l)}>🗑</button>
        </div>
        {#if l.text && !spoken(l.text)}<p class="warn">{t("voicesOnlyTags")}</p>{/if}
      </div>
    {/each}
    <button class="add" disabled={ro} onclick={addLine}>＋ {t("voicesAddLine")}</button>
    <div class="gen">
      <button class="primary" disabled={ro || !pending || !!progress} onclick={() => generateAll(null)}>
        {progress ?? t("voicesReview", { n: pending })}
      </button>
    </div>
    <p class="note">{t("voicesScriptNote")}</p>
  {/if}
</div>

<style>
  .panel { display: flex; flex-direction: column; gap: 6px; font-size: 12px; }
  header { display: flex; align-items: center; gap: 6px; }
  header b { flex: 1; font-size: 13px; }
  .mock { font-size: 10.5px; padding: 0 5px; border-radius: 4px; background: rgb(251 191 36 / 0.15); color: #fbbf24; }
  .x { padding: 0 6px; }
  .tabs { display: flex; gap: 4px; }
  .tabs button { flex: 1; padding: 2px 6px; font-size: 12px; }
  .tabs button.on { background: var(--accent); color: #04121b; border-color: var(--accent); }
  .note { margin: 0; color: var(--muted); font-size: 11.5px; line-height: 1.4; }
  .warn { margin: 0; color: #fbbf24; font-size: 11px; }
  .card, .line { border: 1px solid var(--border); border-left: 3px solid var(--c); border-radius: 6px; padding: 5px 6px; display: flex; flex-direction: column; gap: 5px; }
  .chead { display: flex; align-items: center; gap: 6px; }
  .dot { width: 14px; height: 14px; padding: 0; border-radius: 50%; background: var(--c); border: 1px solid rgb(255 255 255 / 0.3); flex: none; }
  .cname { flex: 1; text-align: left; background: none; border: none; padding: 0; font-weight: 600; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .vstate { font-weight: 400; color: #fbbf24; }
  .vstate.ok { color: #86efac; }
  .field { display: grid; grid-template-columns: 76px 1fr auto; align-items: center; gap: 6px; }
  .field.col { grid-template-columns: 1fr; }
  input:not([type="range"]):not([type="checkbox"]), select, textarea {
    width: 100%; box-sizing: border-box; font: inherit; color: var(--text); background: var(--panel-2);
    border: 1px solid var(--border); border-radius: 5px; padding: 2px 5px;
  }
  textarea { resize: vertical; line-height: 1.4; }
  output { color: var(--muted); font-variant-numeric: tabular-nums; min-width: 36px; text-align: right; }
  .btns { display: flex; flex-wrap: wrap; gap: 4px; }
  .btns button, .gen button, .add { padding: 2px 8px; font-size: 12px; }
  .previews { display: flex; flex-direction: column; gap: 4px; padding: 4px; background: var(--bg); border-radius: 5px; }
  .prev { display: flex; align-items: center; gap: 6px; }
  .prev span { flex: 1; }
  .prev button { padding: 1px 8px; font-size: 12px; }
  details summary { cursor: pointer; color: var(--muted); }
  details { display: flex; flex-direction: column; gap: 4px; }
  .danger { color: #fca5a5; }
  .consent { display: flex; gap: 6px; align-items: flex-start; font-size: 11.5px; color: #fde68a; line-height: 1.4; }
  .primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
  .link { background: none; border-color: transparent; color: var(--muted); align-self: flex-start; }
  .tags { display: flex; flex-wrap: wrap; gap: 3px; }
  .chip { padding: 0 6px; font-size: 11px; border-radius: 10px; }
  .lhead { display: grid; grid-template-columns: 1fr 1fr auto; gap: 4px; align-items: center; }
  .st { font-size: 10.5px; padding: 0 5px; border-radius: 4px; background: var(--bg); color: var(--muted); white-space: nowrap; }
  .st.generated { color: #86efac; }
  .st.stale { color: #fbbf24; }
  .lfoot { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; }
  .count { color: var(--muted); font-variant-numeric: tabular-nums; }
  .time { display: flex; align-items: center; gap: 2px; color: var(--muted); }
  .time input { width: 58px !important; }
  .grow { flex: 1; }
  .mini { padding: 0 5px; font-size: 11.5px; }
  .gen { display: flex; gap: 4px; flex-wrap: wrap; }
  .add { align-self: flex-start; }
  .seg { display: flex; gap: 4px; }
  .seg button { flex: 1; padding: 2px 6px; font-size: 12px; }
  .seg button.on { border-color: var(--accent); color: var(--accent); }
  .check { display: flex; gap: 6px; align-items: center; }
</style>
