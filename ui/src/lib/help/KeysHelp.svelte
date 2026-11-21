<script lang="ts">
  // Ayuda de atajos (?): todos los atajos agrupados, con buscador. Sale de `keys.ts` y de los
  // atajos cambiados en Ajustes, así que siempre está al día.
  import { onMount } from "svelte";
  import { actionsOf, display, keysFor, type Group } from "../keys";
  import { prefs } from "../prefs.svelte";
  import { label, t, type Key } from "../i18n.svelte";

  interface Props {
    onclose: () => void;
    /** Abre Ajustes → Atajos para cambiarlos. */
    onedit: () => void;
  }
  let { onclose, onedit }: Props = $props();

  // Apartados del visor. Una acción nueva que no esté aquí sale en «Otros»: nunca se pierde.
  const VIEWER_SECTIONS: [Key, string[]][] = [
    ["helpNav", ["next", "prev", "right", "left", "down", "up", "first", "last", "goTo", "bookmark"]],
    ["helpView", ["zoomIn", "zoomOut", "fit", "actual", "rotateCw", "rotateCcw", "loupe", "fullscreen", "thumbs"]],
    ["helpReading", ["reading", "fitWidth", "fitHeight", "double", "strip"]],
    ["helpImage", ["adjust", "night", "compare", "info", "copyImage", "saveImage", "reveal", "trash"]],
    ["helpAction", ["action", "play", "editor", "exportVideo", "exportStory", "mute", "undo", "redo"]],
    ["helpApp", ["openFile", "openFolder", "createComic", "organize", "recents", "closeSource", "settings", "help", "diag"]],
  ];
  const PREFIX: Record<Group, string> = { viewer: "kv", player: "kp", editor: "ke" };

  interface Row {
    name: string;
    keys: string[];
  }
  interface Section {
    title: string;
    rows: Row[];
  }

  /** «I» y «Mayús+I» en la misma acción es la misma letra: en la ayuda basta con una. */
  function simple(keys: readonly string[]): string[] {
    return keys.filter((k) => !(k.length === 1 && k !== k.toLowerCase() && keys.includes(k.toLowerCase())));
  }
  const row = (g: Group, a: string): Row => ({
    name: label(`${PREFIX[g]}.${a}`),
    keys: simple(keysFor(prefs.settings.keys, g, a as never)).map(display),
  });

  const sections = $derived.by((): Section[] => {
    const all = actionsOf("viewer") as string[];
    const placed = new Set(VIEWER_SECTIONS.flatMap(([, l]) => l));
    const out: Section[] = VIEWER_SECTIONS.map(([k, list]) => ({
      title: t(k),
      rows: list.filter((a) => all.includes(a)).map((a) => row("viewer", a)),
    }));
    const rest = all.filter((a) => !placed.has(a));
    if (rest.length) out.push({ title: t("helpOther"), rows: rest.map((a) => row("viewer", a)) });
    out.push({ title: t("groupPlayer"), rows: (actionsOf("player") as string[]).map((a) => row("player", a)) });
    out.push({ title: t("groupEditor"), rows: (actionsOf("editor") as string[]).map((a) => row("editor", a)) });
    out.push({
      title: t("helpMouse"),
      rows: (["mWheel", "mCtrlWheel", "mDrag", "mDouble", "mRight", "mRightHold", "mShiftDrag", "mPinch"] as const).map((k) => ({
        name: t(`${k}Do` as Key),
        keys: [t(k as Key)],
      })),
    });
    return out;
  });

  let query = $state("");
  const norm = (s: string) => s.toLowerCase().normalize("NFD").replace(/\p{Diacritic}/gu, "");
  const shown = $derived.by(() => {
    const q = norm(query.trim());
    if (!q) return sections;
    return sections
      .map((s) => (norm(s.title).includes(q) ? s : { ...s, rows: s.rows.filter((r) => norm(r.name).includes(q) || r.keys.some((k) => norm(k).includes(q))) }))
      .filter((s) => s.rows.length);
  });

  let input: HTMLInputElement;
  onMount(() => input.focus());

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopImmediatePropagation();
      if (query) query = "";
      else onclose();
    }
  }
</script>

<svelte:window onkeydowncapture={key} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("helpTitle")} tabindex="-1">
    <header>
      <b>⌨️ {t("helpTitle")}</b>
      <button class="x" onclick={onclose} title="{t('close')} (Esc)">✕</button>
    </header>
    <div class="search">
      <input bind:this={input} type="search" bind:value={query} placeholder={t("helpSearch")} aria-label={t("helpSearch")} />
    </div>
    <div class="content">
      <div class="cols">
      {#each shown as s (s.title)}
        <section>
          <h4>{s.title}</h4>
          {#each s.rows as r (r.name)}
            <div class="row">
              <span class="name">{r.name}</span>
              <span class="keys">
                {#each r.keys as k (k)}<kbd>{k}</kbd>{:else}<span class="none">{t("noKeys")}</span>{/each}
              </span>
            </div>
          {/each}
        </section>
      {:else}
        <p class="empty">{t("helpNone")}</p>
      {/each}
      </div>
    </div>
    <footer>
      <span class="note">{t("helpFoot")}</span>
      <button onclick={onedit}>{t("helpEdit")}</button>
    </footer>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 60; background: rgb(0 0 0 / 0.55); display: grid; place-items: center; }
  .dialog {
    width: min(880px, calc(100vw - 40px)); height: min(680px, calc(100vh - 60px)); display: flex; flex-direction: column; outline: none;
    background: var(--panel); border: 1px solid var(--border); border-radius: 10px; box-shadow: 0 12px 40px rgb(0 0 0 / 0.5); font-size: 13px;
  }
  header { display: flex; align-items: center; gap: 12px; padding: 8px 12px; border-bottom: 1px solid var(--border); }
  header b { flex: 1; }
  .search { padding: 8px 14px 0; }
  .search input {
    width: 100%; font: inherit; color: var(--text); background: var(--panel-2); border: 1px solid var(--border);
    border-radius: 6px; padding: 4px 8px;
  }
  .x { padding: 2px 8px; }
  .content { flex: 1; overflow-y: auto; padding: 4px 14px 14px; }
  /* Las columnas van en un bloque sin alto fijo: si no, se desbordan hacia el lado. */
  .cols { columns: 2 330px; column-gap: 28px; }
  section { break-inside: avoid; margin-top: 10px; }
  h4 { margin: 0 0 4px; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--accent); }
  .row { display: flex; align-items: baseline; gap: 10px; padding: 3px 0; border-bottom: 1px solid rgb(42 47 56 / 0.5); }
  .name { flex: 1; }
  .keys { display: flex; flex-wrap: wrap; gap: 4px; justify-content: flex-end; }
  kbd {
    font-family: inherit; font-size: 11.5px; padding: 1px 6px; border-radius: 4px; white-space: nowrap;
    background: var(--bg); border: 1px solid var(--border); border-bottom-width: 2px;
  }
  .none { color: var(--muted); font-style: italic; font-size: 12px; }
  .empty { color: var(--muted); }
  footer { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 8px 12px; border-top: 1px solid var(--border); }
  .note { flex: 1 1 260px; color: var(--muted); font-size: 12px; }
</style>
