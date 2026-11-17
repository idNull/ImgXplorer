<script lang="ts">
  // Ventana de ajustes: general y atajos de teclado. Todo se aplica y se guarda al momento.
  import { BACKGROUNDS, DEFAULT_SETTINGS, DEFAULT_VIDEO, prefs } from "./prefs.svelte";
  import { MAX_KEYS, actionsOf, assign, comboOf, display, keysFor, type ActionOf, type Group } from "./keys";
  import { t, label } from "./i18n.svelte";
  import { invoke } from "@tauri-apps/api/core";

  // Micrófonos y salida de sonido (se piden al abrir los ajustes).
  let inputs = $state<{ id: string; name: string }[]>([]);
  let output = $state<string | null>(null);
  invoke<{ id: string; name: string }[]>("audio_inputs").then((l) => (inputs = l), () => {});
  invoke<{ device: string | null; error: string | null }>("audio_status").then((st) => (output = st.device ?? st.error), () => {});
  function setSound(k: "master" | "mute" | "input", v: number | boolean | string) {
    (prefs.settings.sound as Record<string, number | boolean | string>)[k] = v;
    prefs.saveSettings();
  }

  interface Props {
    onclose: () => void;
  }
  let { onclose }: Props = $props();

  let tab = $state<"general" | "keys">("general");
  let capturing = $state<{ group: Group; action: string } | null>(null);
  let notice = $state<string | null>(null);

  const s = $derived(prefs.settings);
  const save = () => prefs.saveSettings();

  function set<K extends keyof typeof prefs.settings>(k: K, v: (typeof prefs.settings)[K]) {
    prefs.settings[k] = v;
    save();
  }

  const GROUPS: { id: Group; title: () => string }[] = [
    { id: "viewer", title: () => t("groupViewer") },
    { id: "player", title: () => t("groupPlayer") },
    { id: "editor", title: () => t("groupEditor") },
  ];
  const PREFIX: Record<Group, string> = { viewer: "kv", player: "kp", editor: "ke" };
  const actionLabel = (g: Group, a: string) => label(`${PREFIX[g]}.${a}`);

  function keys(g: Group, a: string): readonly string[] {
    return keysFor(prefs.settings.keys, g, a as ActionOf<typeof g>);
  }

  function remove(g: Group, a: string, k: string) {
    notice = null;
    assign(prefs.settings.keys, g, a as ActionOf<typeof g>, keys(g, a).filter((x) => x !== k));
    save();
  }

  function overridden(g: Group, a: string): boolean {
    return !!(prefs.settings.keys[g] as Record<string, string[]> | undefined)?.[a];
  }

  function resetOne(g: Group, a: string) {
    const own = prefs.settings.keys[g] as Record<string, string[]> | undefined;
    if (own) delete own[a];
    notice = null;
    save();
  }

  function onKey(e: KeyboardEvent) {
    if (capturing) {
      e.preventDefault();
      e.stopImmediatePropagation();
      const combo = comboOf(e);
      if (!combo) return;
      const { group, action } = capturing;
      capturing = null;
      const cur = keys(group, action);
      if (cur.includes(combo)) return;
      const lost = assign(prefs.settings.keys, group, action as ActionOf<typeof group>, [...cur, combo].slice(-MAX_KEYS));
      notice = lost.length ? lost.map((o) => t("keyMoved", { key: display(combo), action: actionLabel(group, o as string) })).join(" ") : null;
      save();
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopImmediatePropagation();
      onclose();
    }
  }

  function resetAll() {
    prefs.settings.keys = {};
    notice = null;
    save();
  }

  function resetSettings() {
    prefs.settings = {
      ...DEFAULT_SETTINGS,
      lastDir: prefs.settings.lastDir,
      video: { ...DEFAULT_VIDEO, ffmpeg: prefs.settings.video.ffmpeg },
      sound: { ...DEFAULT_SETTINGS.sound, input: prefs.settings.sound.input },
      keys: $state.snapshot(prefs.settings.keys),
    };
    save();
  }

  const recentCount = $derived(prefs.recents.length);
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("settings")}>
    <header>
      <b>{t("settings")}</b>
      <div class="seg">
        <button class:on={tab === "general"} onclick={() => (tab = "general")}>{t("general")}</button>
        <button class:on={tab === "keys"} onclick={() => (tab = "keys")}>{t("shortcuts")}</button>
      </div>
      <button class="close" onclick={onclose} title="{t('close')} (Esc)">✕</button>
    </header>

    <div class="content">
      {#if tab === "general"}
        <label class="row">
          <span>{t("language")}</span>
          <select value={s.lang} onchange={(e) => set("lang", e.currentTarget.value as typeof s.lang)}>
            <option value="auto">{t("langAuto")}</option>
            <option value="es">Español</option>
            <option value="en">English</option>
          </select>
        </label>

        <div class="row">
          <span>{t("background")}</span>
          <div class="swatches">
            {#each BACKGROUNDS as c (c)}
              <button class="swatch" class:on={s.background === c} style:background={c} title={c} aria-label={c} onclick={() => set("background", c)}></button>
            {/each}
            <label class="custom" title={t("customColor")}>
              <input type="color" value={s.background} oninput={(e) => set("background", e.currentTarget.value)} />
            </label>
          </div>
        </div>

        <label class="row">
          <span>{t("thumbsShow")}</span>
          <input type="checkbox" checked={s.showThumbs} onchange={(e) => set("showThumbs", e.currentTarget.checked)} />
        </label>
        <div class="row">
          <span>{t("thumbsSize")}</span>
          <div class="seg">
            {#each [["s", "sizeS"], ["m", "sizeM"], ["l", "sizeL"]] as const as [v, k] (v)}
              <button class:on={s.thumbSize === v} onclick={() => set("thumbSize", v)}>{t(k)}</button>
            {/each}
          </div>
        </div>

        <label class="row">
          <span>{t("zoomStep")}</span>
          <input type="range" min="1.05" max="2" step="0.05" value={s.zoomStep} oninput={(e) => set("zoomStep", Number(e.currentTarget.value))} />
          <output>+{Math.round((s.zoomStep - 1) * 100)} %</output>
        </label>
        <label class="row">
          <span>{t("wheelSpeed")}</span>
          <input type="range" min="0.25" max="4" step="0.25" value={s.wheelSpeed} oninput={(e) => set("wheelSpeed", Number(e.currentTarget.value))} />
          <output>{s.wheelSpeed}×</output>
        </label>

        <label class="row">
          <span>{t("resume")}</span>
          <input type="checkbox" checked={s.resume} onchange={(e) => set("resume", e.currentTarget.checked)} />
        </label>
        <label class="row" title={t("fileDialogHelp")}>
          <span>{t("fileDialog")}</span>
          <select value={s.fileDialog} onchange={(e) => set("fileDialog", e.currentTarget.value as typeof s.fileDialog)}>
            <option value="auto">{t("fileDialogAuto")}</option>
            <option value="app">{t("fileDialogApp")}</option>
            <option value="system">{t("fileDialogSystem")}</option>
          </select>
        </label>
        <p class="note">{t("fileDialogHelp")}</p>
        <label class="row">
          <span>{t("masterVolume")}</span>
          <input type="range" min="0" max="150" step="5" value={Math.round(s.sound.master * 100)} oninput={(e) => setSound("master", Number(e.currentTarget.value) / 100)} />
          <output>{Math.round(s.sound.master * 100)} %</output>
        </label>
        <label class="row">
          <span>{t("mute")}</span>
          <input type="checkbox" checked={s.sound.mute} onchange={(e) => setSound("mute", e.currentTarget.checked)} />
        </label>
        <label class="row">
          <span>{t("microphone")}</span>
          <select value={s.sound.input} onchange={(e) => setSound("input", e.currentTarget.value)}>
            <option value="">{t("microphoneDefault")}</option>
            {#if s.sound.input && !inputs.some((d) => d.id === s.sound.input)}<option value={s.sound.input}>{s.sound.input}</option>{/if}
            {#each inputs as d (d.id)}<option value={d.id}>{d.name}</option>{/each}
          </select>
        </label>
        {#if output}<p class="note">{t("soundOutput", { name: output })}</p>{/if}
        <label class="row">
          <span>{t("showDiag")}</span>
          <input type="checkbox" checked={s.showDiag} onchange={(e) => set("showDiag", e.currentTarget.checked)} />
        </label>

        <div class="actions">
          <button onclick={() => prefs.clearRecents()} disabled={!recentCount}>{t("clearRecents")} ({recentCount})</button>
          <button onclick={resetSettings}>{t("resetSettings")}</button>
        </div>
      {:else}
        <p class="note">{t("keysHelp")}</p>
        {#if notice}<p class="notice">{notice}</p>{/if}
        {#each GROUPS as g (g.id)}
          <h4>{g.title()}</h4>
          {#each actionsOf(g.id) as a (a)}
            {@const list = keys(g.id, a as string)}
            <div class="key-row">
              <span class="name">{actionLabel(g.id, a as string)}</span>
              <span class="chips">
                {#each list as k (k)}
                  <button class="chip" title={t("removeKey", { key: display(k) })} onclick={() => remove(g.id, a as string, k)}>
                    {display(k)} <span class="x">✕</span>
                  </button>
                {:else}
                  <span class="none">{t("noKeys")}</span>
                {/each}
                {#if capturing?.group === g.id && capturing.action === a}
                  <span class="capture">{t("pressKey")}</span>
                  <button class="small" onclick={() => (capturing = null)}>{t("cancel")}</button>
                {:else if list.length < MAX_KEYS}
                  <button class="small" title={t("addKey")} onclick={() => ((capturing = { group: g.id, action: a as string }), (notice = null))}>+</button>
                {/if}
                {#if overridden(g.id, a as string)}
                  <button class="small link" onclick={() => resetOne(g.id, a as string)}>{t("resetOne")}</button>
                {/if}
              </span>
            </div>
          {/each}
        {/each}
        <div class="actions">
          <button onclick={resetAll}>{t("resetKeys")}</button>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: rgb(0 0 0 / 0.5); display: grid; place-items: center; z-index: 20; }
  .dialog {
    width: min(640px, calc(100vw - 40px)); max-height: calc(100vh - 60px); display: flex; flex-direction: column;
    background: var(--panel); border: 1px solid var(--border); border-radius: 10px; box-shadow: 0 10px 40px rgb(0 0 0 / 0.5);
  }
  header { display: flex; align-items: center; gap: 12px; padding: 10px 14px; border-bottom: 1px solid var(--border); }
  header b { flex: 1; }
  .close { padding: 2px 8px; }
  .content { overflow-y: auto; padding: 12px 14px 16px; display: flex; flex-direction: column; gap: 10px; font-size: 13px; }
  .row { display: grid; grid-template-columns: 1fr 180px 56px; align-items: center; gap: 10px; }
  .row > :nth-child(2):last-child { grid-column: 2 / 4; justify-self: start; }
  .row output { text-align: right; color: var(--muted); font-variant-numeric: tabular-nums; }
  select { font: inherit; color: var(--text); background: var(--panel-2); border: 1px solid var(--border); border-radius: 6px; padding: 4px 6px; }
  .seg { display: flex; }
  .seg button { border-radius: 0; }
  .seg button:first-child { border-radius: 6px 0 0 6px; }
  .seg button:last-child { border-radius: 0 6px 6px 0; }
  .seg button + button { border-left: none; }
  .seg button.on { background: var(--accent); color: #04121b; border-color: var(--accent); }
  .swatches { display: flex; gap: 6px; align-items: center; }
  .swatch { width: 26px; height: 26px; padding: 0; border-radius: 50%; border: 2px solid var(--border); }
  .swatch.on { border-color: var(--accent); box-shadow: 0 0 0 2px rgb(56 189 248 / 0.3); }
  .custom input { width: 30px; height: 26px; padding: 0; border: none; background: none; cursor: pointer; }
  .actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 6px; }
  h4 { margin: 10px 0 2px; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  .key-row { display: flex; align-items: center; gap: 10px; padding: 3px 0; border-bottom: 1px solid rgb(42 47 56 / 0.5); }
  .key-row .name { flex: 1; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; justify-content: flex-end; }
  .chip { padding: 1px 6px; font-size: 12px; background: var(--bg); }
  .chip .x { color: var(--muted); font-size: 10px; margin-left: 2px; }
  .chip:hover .x { color: #fca5a5; }
  .small { padding: 1px 7px; font-size: 12px; }
  .link { background: none; border-color: transparent; color: var(--accent); }
  .none { color: var(--muted); font-style: italic; font-size: 12px; }
  .capture { color: #fbbf24; font-size: 12px; }
  .note { margin: 0; color: var(--muted); font-size: 12px; line-height: 1.4; }
  .notice { margin: 0; color: #fbbf24; font-size: 12px; }
</style>
