<script lang="ts">
  // Panel lateral del Modo Acción: plantilla (para todas o solo esta imagen), sus parámetros,
  // transición entre imágenes y reproducción.
  import { applies, catalog, fresh, paramValue, type ParamSpec, type TemplateRef } from "./catalog";
  import type { Dir, ProjectStore, Transition, TransitionType } from "./project.svelte";
  import { t, label } from "../i18n.svelte";
  import { prefs } from "../prefs.svelte";
  import { display, keysFor, type PlayerAction, type ViewerAction } from "../keys";

  const pk = (a: PlayerAction) => keysFor(prefs.settings.keys, "player", a).slice(0, 2).map(display);
  const vk = (a: ViewerAction) => keysFor(prefs.settings.keys, "viewer", a).slice(0, 1).map(display);
  const HELP: { keys: () => string[]; text: () => string }[] = [
    { keys: () => pk("toggle"), text: () => t("kPause") },
    { keys: () => [...pk("markPrev").slice(0, 1), ...pk("markNext").slice(0, 1)], text: () => t("kMarks") },
    { keys: () => [...pk("seekBack").slice(0, 1), ...pk("seekFwd").slice(0, 1)], text: () => t("kSeek") },
    { keys: () => [...pk("pagePrev").slice(0, 1), ...pk("pageNext").slice(0, 1)], text: () => t("kPages") },
    { keys: () => [...pk("slower").slice(0, 1), ...pk("faster").slice(0, 1)], text: () => t("kSpeed") },
    { keys: () => pk("stop").slice(0, 1), text: () => t("kStop") },
  ];

  interface Props {
    store: ProjectStore;
    /** Entrada y tamaño de la imagen actual (para "solo esta imagen"). */
    entry: string | null;
    size: [number, number] | null;
    /** Viñetas detectadas en la imagen actual (null: aún no se sabe). */
    detected: number | null;
    playing: boolean;
    onplay: () => void;
    onclose: () => void;
    /** Editando las viñetas sobre la imagen. */
    panelEdit: boolean;
    /** Viñetas dibujadas a mano en esta imagen (null: ninguna, se detectan). */
    manualPanels: number | null;
    onpaneledit: () => void;
    onresetpanels: () => void;
    oneditor: () => void;
  }

  let { store, entry, size, detected, playing, onplay, onclose, panelEdit, manualPanels, onpaneledit, onresetpanels, oneditor }: Props = $props();

  const ownTimeline = $derived(!!(entry && store.item(entry)?.timeline?.camera?.length));
  const effective = $derived(entry ? store.templateFor(entry) : null);

  let scope = $state<"all" | "item">("all");

  const own = $derived(entry ? store.item(entry)?.template ?? null : null);
  const editing = $derived<TemplateRef | null>(scope === "all" ? (store.data?.defaults.template ?? null) : own);

  function pick(id: string) {
    const tpl = id ? fresh(id) : null;
    if (scope === "all") store.setDefaultTemplate(tpl);
    else if (entry && size) store.setItemTemplate(entry, size, tpl ?? fresh("full"));
  }

  function customize() {
    if (!entry || !size) return;
    const base = store.data?.defaults.template;
    store.setItemTemplate(entry, size, base ? $state.snapshot(base) : fresh("full"));
  }

  function setParam(spec: ParamSpec, value: unknown) {
    if (!editing) return;
    editing.params[spec.key] = value;
    store.changed();
  }

  const transition = $derived(store.data?.defaults.transition);

  function setTransition(patch: Partial<Transition>) {
    const tr = store.data?.defaults.transition;
    if (!tr) return;
    Object.assign(tr, patch);
    // `dir` solo existe donde tiene sentido; deslizar la necesita.
    if (tr.type === "slide") tr.dir ??= "right";
    else if (tr.type !== "continuous-camera") delete tr.dir;
    store.changed();
  }

  function setHold(v: number) {
    if (!store.data) return;
    store.data.defaults.holdBetween = v;
    store.changed();
  }

  const TRANSITIONS: TransitionType[] = ["cut", "fade", "slide", "continuous-camera"];
  const DIRS: Dir[] = ["right", "left", "down", "up"];
</script>

<aside class="panel">
  <header>
    <b>{t("action")}</b>
    <button class="close" onclick={onclose} title={t("close")}>✕</button>
  </header>

  <button class="play" onclick={onplay}>{playing ? `■ ${t("stop")}` : `▶ ${t("playHere")}`} {#each vk("play") as k (k)}<kbd>{k}</kbd>{/each}</button>

  <section>
    <h4>{t("template")}</h4>
    <div class="seg">
      <button class:on={scope === "all"} onclick={() => (scope = "all")}>{t("scopeAll")}</button>
      <button class:on={scope === "item"} onclick={() => (scope = "item")} disabled={!entry}>{t("scopeItem")}</button>
    </div>

    {#if scope === "item" && ownTimeline && !own}
      <p class="note">{t("hasOwnTimeline")}</p>
      <button onclick={oneditor}>✎ {t("openEditor")}</button>
      <button onclick={() => entry && size && store.setTimeline(entry, size, null)}>{t("clearOwn")}</button>
    {:else if scope === "item" && !own}
      <p class="note">{t("usesAll")}</p>
      <button onclick={customize}>{t("customize")}</button>
    {:else}
      <select value={editing?.id ?? ""} onchange={(e) => pick(e.currentTarget.value)}>
        {#if scope === "all"}<option value="">{t("noTemplate")}</option>{/if}
        {#each catalog() as tpl (tpl.id)}
          <option value={tpl.id}>{label(`tpl.${tpl.id}`)}</option>
        {/each}
      </select>
      {#if editing}
        <p class="note">{label(`tpl.${editing.id}.help`)}</p>
        {#if editing.id === "comic" && (editing.params.mode ?? "auto") === "auto" && manualPanels === null}
          <p class="note">
            {detected === null ? t("detecting") : detected >= 2 ? t("detected", { n: detected }) : t("notDetected")}
          </p>
        {/if}
        {#each catalog().find((c) => c.id === editing.id)?.params ?? [] as spec (spec.key)}
          {#if applies(editing, spec)}
            {@const v = paramValue(editing, spec)}
            <label class="param">
              <span>{label(`p.${spec.key}`)}</span>
              {#if spec.kind === "number"}
                <input
                  type="range" min={spec.min} max={spec.max} step={spec.step} value={v as number}
                  oninput={(e) => setParam(spec, Number(e.currentTarget.value))}
                />
                <output>{Number(v).toLocaleString(undefined, { maximumFractionDigits: 2 })}</output>
              {:else if spec.kind === "choice"}
                <select value={v as string} onchange={(e) => setParam(spec, e.currentTarget.value)}>
                  {#each spec.options as o (o)}<option value={o}>{label(`o.${o}`)}</option>{/each}
                </select>
              {:else}
                <input type="checkbox" checked={v as boolean} onchange={(e) => setParam(spec, e.currentTarget.checked)} />
              {/if}
            </label>
          {/if}
        {/each}
        {#if scope === "item"}
          <button onclick={() => entry && size && store.setItemTemplate(entry, size, null)}>{t("useAll")}</button>
        {/if}
      {:else}
        <p class="note">{t("noTemplateHelp")}</p>
      {/if}
    {/if}
  </section>

  {#if effective?.id === "comic" && !ownTimeline}
    <section>
      <h4>{t("panelsHere")}</h4>
      <p class="note">{manualPanels !== null ? t("panelsManual", { n: manualPanels }) : t("panelsAuto")}</p>
      <button class:on={panelEdit} onclick={onpaneledit} disabled={!size || !!store.readOnly}>
        {panelEdit ? `✓ ${t("panelsDone")}` : `✎ ${t("panelsEdit")}`}
      </button>
      {#if manualPanels !== null}
        <button onclick={onresetpanels}>{t("panelsReset")}</button>
      {/if}
    </section>
  {/if}

  {#if ownTimeline && scope === "all"}
    <p class="note">{t("hasOwnTimeline")}</p>
  {/if}

  {#if transition}
    <section>
      <h4>{t("transition")}</h4>
      <select value={transition.type} onchange={(e) => setTransition({ type: e.currentTarget.value as TransitionType })}>
        {#each TRANSITIONS as tr (tr)}<option value={tr}>{label(`tr.${tr}`)}</option>{/each}
      </select>
      {#if transition.type === "slide" || transition.type === "continuous-camera"}
        <label class="param">
          <span>{t("direction")}</span>
          <select
            value={transition.dir ?? ""}
            onchange={(e) => setTransition({ dir: (e.currentTarget.value || undefined) as Dir | undefined })}
          >
            {#if transition.type === "continuous-camera"}<option value="">{t("auto")}</option>{/if}
            {#each DIRS as d (d)}<option value={d}>{label(`o.${d}`)}</option>{/each}
          </select>
        </label>
      {/if}
      {#if transition.type !== "cut"}
        <label class="param">
          <span>{t("duration")}</span>
          <input type="range" min="0.1" max="3" step="0.1" value={transition.duration}
            oninput={(e) => setTransition({ duration: Number(e.currentTarget.value) })} />
          <output>{transition.duration.toFixed(1)} s</output>
        </label>
      {/if}
      <label class="param">
        <span>{t("holdBetween")}</span>
        <input type="range" min="0" max="5" step="0.1" value={store.data?.defaults.holdBetween ?? 0.3}
          oninput={(e) => setHold(Number(e.currentTarget.value))} />
        <output>{(store.data?.defaults.holdBetween ?? 0).toFixed(1)} s</output>
      </label>
    </section>
  {/if}

  <section class="keys">
    <h4>{t("playKeys")}</h4>
    {#each HELP as h, i (i)}
      <div>{#each h.keys() as k, j (j)}<kbd>{k}</kbd>{/each} {h.text()}</div>
    {/each}
    <p class="note">{t("overrideHelp")}</p>
  </section>

  <footer class="note">
    {#if store.readOnly}
      <span class="warn">{t("projectReadOnly", { error: store.readOnly })}</span>
    {:else if store.saveError}
      <span class="warn">{t("saveError", { error: store.saveError })}</span>
    {:else if store.path}
      {t("savedIn")} <code title={store.path}>{store.path}</code>
    {:else}
      {t("notSaved")}
    {/if}
  </footer>
</aside>

<style>
  .panel {
    width: 300px; flex: none; overflow-y: auto; padding: 10px 12px; display: flex; flex-direction: column; gap: 12px;
    background: var(--panel); border-left: 1px solid var(--border); font-size: 13px;
  }
  header { display: flex; align-items: center; justify-content: space-between; }
  .close { padding: 2px 8px; }
  h4 { margin: 0 0 6px; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  section { display: flex; flex-direction: column; gap: 6px; }
  .play { padding: 8px; font-weight: 600; border-color: var(--accent); }
  button.on { border-color: #fbbf24; color: #fde68a; }
  .seg { display: flex; gap: 0; }
  .seg button { flex: 1; border-radius: 0; }
  .seg button:first-child { border-radius: 6px 0 0 6px; }
  .seg button:last-child { border-radius: 0 6px 6px 0; border-left: none; }
  .seg button.on { background: var(--accent); color: #04121b; border-color: var(--accent); }
  select { font: inherit; color: var(--text); background: var(--panel-2); border: 1px solid var(--border); border-radius: 6px; padding: 4px 6px; }
  .param { display: grid; grid-template-columns: 1fr 96px 40px; align-items: center; gap: 6px; }
  .param select { grid-column: 2 / 4; }
  .param input[type="checkbox"] { justify-self: start; }
  .param output { text-align: right; color: var(--muted); font-variant-numeric: tabular-nums; }
  .note { margin: 0; color: var(--muted); font-size: 12px; line-height: 1.4; }
  .keys div { color: var(--muted); }
  kbd {
    display: inline-block; padding: 0 5px; margin-right: 2px; border: 1px solid var(--border); border-radius: 4px;
    font: 11px/16px system-ui, sans-serif; color: var(--text); background: var(--bg);
  }
  footer { margin-top: auto; word-break: break-all; }
  footer code { color: var(--text); }
  .warn { color: #fca5a5; }
</style>
