<script lang="ts">
  // Información de la imagen (ℹ️, I): nombre, tamaño, peso, formato, posición y datos de la cámara.
  import { invoke } from "@tauri-apps/api/core";
  import { t } from "../i18n.svelte";
  import { log } from "../log";

  interface Props {
    generation: number;
    index: number;
    count: number;
    name: string;
    bytes: number;
    pixels: { w: number; h: number } | null;
    source: string;
    onreveal: () => void;
    onclose: () => void;
  }
  let { generation, index, count, name, bytes, pixels, source, onreveal, onclose }: Props = $props();

  interface Info {
    format: string;
    exif: [string, string][];
    gps: boolean;
  }
  let info = $state<Info | null>(null);
  const cache = new Map<string, Info>();

  $effect(() => {
    const key = `${generation}_${index}`;
    const have = cache.get(key);
    if (have) {
      info = have;
      return;
    }
    info = null;
    let cancelled = false;
    invoke<Info>("image_info", { generation, index })
      .then((i) => {
        cache.set(key, i);
        if (!cancelled) info = i;
      })
      .catch((e) => log.warn("información de la imagen:", e));
    return () => {
      cancelled = true;
    };
  });

  function size(n: number): string {
    const u = ["B", "KB", "MB", "GB"];
    let v = n;
    let k = 0;
    while (v >= 1024 && k < u.length - 1) {
      v /= 1024;
      k++;
    }
    return `${v.toLocaleString(undefined, { maximumFractionDigits: k ? 1 : 0 })} ${u[k]}`;
  }

  const megapixels = $derived(pixels ? (pixels.w * pixels.h) / 1e6 : 0);
  const file = $derived(name.slice(name.lastIndexOf("/") + 1));
</script>

<div class="info" role="dialog" aria-label={t("infoTitle")}>
  <header>
    <b>ℹ️ {t("infoTitle")}</b>
    <button class="x" onclick={onclose} title={t("close")}>✕</button>
  </header>
  <div class="name" title={name}>{file}</div>
  <dl>
    <dt>{t("infoPosition")}</dt><dd>{index + 1} / {count}</dd>
    {#if pixels}<dt>{t("infoPixels")}</dt><dd>{pixels.w} × {pixels.h} px · {megapixels.toFixed(1)} MP</dd>{/if}
    <dt>{t("infoWeight")}</dt><dd>{size(bytes)}</dd>
    {#if info}<dt>{t("infoFormat")}</dt><dd>{info.format || "–"}</dd>{/if}
    {#if name.includes("/")}<dt>{t("infoFolder")}</dt><dd title={name}>{name.slice(0, name.lastIndexOf("/"))}</dd>{/if}
    <dt>{t("infoSource")}</dt><dd class="path" title={source}>{source}</dd>
  </dl>
  {#if info?.exif.length}
    <h5>📷 {t("infoCamera")}</h5>
    <dl>
      {#each info.exif as [k, v] (k)}
        <dt>{t(`exif_${k}` as Parameters<typeof t>[0])}</dt><dd>{v}</dd>
      {/each}
    </dl>
  {/if}
  {#if info?.gps}<p class="note">📍 {t("infoGps")}</p>{/if}
  <footer>
    <button onclick={onreveal}>📂 {t("infoReveal")}</button>
  </footer>
</div>

<style>
  .info {
    position: absolute; top: 10px; left: 10px; z-index: 5; width: 280px; max-height: calc(100% - 20px); overflow-y: auto;
    display: flex; flex-direction: column; gap: 6px; padding: 10px 12px; font-size: 12.5px;
    background: rgb(20 23 28 / 0.94); border: 1px solid var(--border); border-radius: 10px;
    box-shadow: 0 8px 30px rgb(0 0 0 / 0.45); backdrop-filter: blur(6px);
  }
  header { display: flex; align-items: center; justify-content: space-between; }
  .x { padding: 1px 7px; }
  .name { font-weight: 600; overflow-wrap: anywhere; }
  dl { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 3px 10px; margin: 0; }
  dt { color: var(--muted); white-space: nowrap; }
  dd { margin: 0; overflow-wrap: anywhere; }
  .path { color: var(--muted); font-size: 11.5px; }
  h5 { margin: 6px 0 0; font-size: 12px; border-top: 1px solid var(--border); padding-top: 8px; }
  .note { margin: 0; color: var(--muted); font-size: 11.5px; }
  footer { border-top: 1px solid var(--border); padding-top: 8px; }
  footer button { padding: 2px 8px; font-size: 12px; }
</style>
