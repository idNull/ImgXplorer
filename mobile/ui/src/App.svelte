<script lang="ts">
  // Pantallas del visor: estantería, lector y comprobación. El botón «atrás» de Android vuelve a
  // la estantería (cada pantalla abierta es una entrada del historial del WebView).
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import Check from "./Check.svelte";
  import Reader from "./Reader.svelte";
  import Shelf from "./Shelf.svelte";
  import { library, type Imported, type Item } from "./lib/api";
  import { t } from "./lib/i18n";

  type Screen = { kind: "shelf" } | { kind: "reader"; item: Item } | { kind: "check" };
  let screen = $state<Screen>({ kind: "shelf" });
  let items = $state<Item[]>([]);
  let busy = $state(false);
  let toast = $state<string | null>(null);

  async function refresh() {
    items = await library.list().catch(() => []);
  }

  function show(s: Screen) {
    if (screen.kind === "shelf" && s.kind !== "shelf") history.pushState({ screen: s.kind }, "");
    screen = s;
  }
  function back() {
    if (screen.kind !== "shelf") history.back();
  }

  function report(r: Imported) {
    if (r.failed.length) {
      toast = t("failed", { list: r.failed.map(([n, why]) => `${n} (${why})`).join(", ") });
      setTimeout(() => (toast = null), 6000);
    }
  }

  async function run(job: () => Promise<Imported>): Promise<Imported | null> {
    busy = true;
    try {
      const r = await job();
      report(r);
      await refresh();
      return r;
    } catch (e) {
      toast = String(e);
      setTimeout(() => (toast = null), 6000);
      return null;
    } finally {
      busy = false;
    }
  }

  async function onimport(kind: "images" | "documents") {
    const r = await run(() => library.pick(kind));
    // Una sola cosa elegida: se abre directamente.
    if (r?.items.length === 1) show({ kind: "reader", item: r.items[0] });
  }

  /**
   * Lo que otra app pidió abrir con imgxplorer: al arrancar, al volver a la app y, en iOS, en
   * cuanto llega (evento `imgx-opened`).
   */
  async function takeOpened() {
    if (busy) return;
    const r = await library.takeOpened().catch(() => null);
    if (!r) return;
    report(r);
    if (!r.items.length) return;
    await refresh();
    if (screen.kind === "reader") history.back();
    // Se espera a que el «atrás» llegue antes de abrir lo nuevo.
    setTimeout(() => show({ kind: "reader", item: r.items[0] }), screen.kind === "reader" ? 80 : 0);
  }

  onMount(() => {
    refresh().then(takeOpened);
    const onPop = () => {
      screen = { kind: "shelf" };
      refresh();
    };
    const onVisible = () => document.visibilityState === "visible" && takeOpened();
    window.addEventListener("popstate", onPop);
    document.addEventListener("visibilitychange", onVisible);
    const unlisten = listen("imgx-opened", () => takeOpened()).catch(() => () => {});
    return () => {
      void unlisten.then((u) => u());
      window.removeEventListener("popstate", onPop);
      document.removeEventListener("visibilitychange", onVisible);
    };
  });
</script>

{#if screen.kind === "reader"}
  {#key screen.item.id}
    <Reader item={screen.item} onclose={back} />
  {/key}
{:else if screen.kind === "check"}
  <Check onclose={back} />
{:else}
  <Shelf
    {items}
    {busy}
    onopen={(item) => show({ kind: "reader", item })}
    {onimport}
    oninfo={() => show({ kind: "check" })}
    onchanged={refresh}
  />
{/if}

{#if toast}
  <div class="toast" role="status">{toast}</div>
{/if}

<style>
  .toast {
    position: fixed; left: 14px; right: 14px; bottom: calc(env(safe-area-inset-bottom) + 16px); z-index: 10;
    padding: 12px 14px; border-radius: 12px; background: #3a1f22; color: #ffd7d5; font-size: 14px;
  }
</style>
