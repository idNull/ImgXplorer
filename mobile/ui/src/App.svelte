<script lang="ts">
  // Fase 0: pantalla de comprobación. Enseña que la interfaz habla con Rust y lo que ofrece el
  // aparato (WebGL2 y el tamaño máximo de textura, que decide cuándo hacen falta teselas).
  import { invoke } from "@tauri-apps/api/core";
  import logo from "../../../src-tauri/icons/svg/icono.svg?url";
  import { t } from "./lib/i18n";

  type AppInfo = { version: string; os: string; arch: string; heif: boolean; rar: boolean };

  let info = $state<AppInfo | null>(null);
  let error = $state<string | null>(null);

  invoke<AppInfo>("app_info").then((i) => (info = i)).catch((e) => (error = String(e)));

  const gl = document.createElement("canvas").getContext("webgl2");
  const maxTexture = gl ? (gl.getParameter(gl.MAX_TEXTURE_SIZE) as number) : 0;
  const screen = `${window.screen.width}×${window.screen.height} @${window.devicePixelRatio}x`;
</script>

<main>
  <img class="logo" src={logo} alt="" />
  <h1>imgxplorer</h1>
  <p class="subtitle">{t("subtitle")}</p>
  <p class="phase">{t("phase")}</p>

  <dl>
    <dt>{t("bridge")}</dt>
    <dd class:ok={info} class:bad={error}>{info ? "✓" : error ? `✕ ${error}` : t("waiting")}</dd>
    {#if info}
      <dt>{t("version")}</dt>
      <dd>{info.version}</dd>
      <dt>{t("system")}</dt>
      <dd>{info.os} · {info.arch}</dd>
      <dt>{t("formats")}</dt>
      <dd>HEIC {info.heif ? t("yes") : t("no")} · RAR {info.rar ? t("yes") : t("no")}</dd>
    {/if}
    <dt>{t("webgl")}</dt>
    <dd class:ok={gl} class:bad={!gl}>{gl ? "✓" : "✕"}</dd>
    <dt>{t("maxTexture")}</dt>
    <dd>{maxTexture ? `${maxTexture} px` : "—"}</dd>
    <dt>{t("screen")}</dt>
    <dd>{screen}</dd>
  </dl>
</main>

<style>
  main {
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 24px;
    text-align: center;
  }
  .logo { width: 112px; height: 112px; }
  h1 { margin: 8px 0 0; font-size: 30px; letter-spacing: 0.5px; }
  .subtitle { margin: 0; color: var(--muted); }
  .phase { margin: 14px 0 6px; color: var(--accent); font-weight: 600; }
  dl {
    display: grid;
    grid-template-columns: auto auto;
    gap: 8px 16px;
    margin: 12px 0 0;
    padding: 16px 20px;
    background: var(--panel);
    border-radius: 14px;
    text-align: left;
    font-size: 15px;
  }
  dt { color: var(--muted); }
  dd { margin: 0; }
  .ok { color: var(--ok); }
  .bad { color: var(--bad); }
</style>
