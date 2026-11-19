<script lang="ts">
  // Pestaña «Acerca de» de los ajustes: autor, herramientas con las que se hizo y datos del sistema.
  import { getVersion } from "@tauri-apps/api/app";
  import { invoke } from "@tauri-apps/api/core";
  import { currentLang, t } from "./i18n.svelte";
  import icon from "../../../src-tauri/icons/128x128@2x.png";
  import licenseText from "../../../LICENSE?raw";
  import ThirdParty from "./about/ThirdParty.svelte";
  import { loadLicenses, type Licenses } from "./about/licenses";
  import { THANKS, type Thanks } from "./about/thanks";
  import { tour } from "./help/tourState.svelte";

  const AUTHOR = "Pedro López del Castillo";

  interface Info {
    os: string;
    arch: string;
    sessionType: string | null;
    webkit: string | null;
  }

  let version = $state("");
  let info = $state<Info | null>(null);
  let copied = $state(false);
  let showLicense = $state(false);
  let thirdParty = $state(false);
  // La lista de terceros pesa: se carga al abrir esta pestaña, no al arrancar la app.
  let licenses = $state.raw<Licenses | null>(null);
  loadLicenses().then((l) => (licenses = l), () => {});
  getVersion().then((v) => (version = v), () => {});
  invoke<Info>("diagnostics_info").then((i) => (info = i), () => {});

  // Agrupadas por lo que hacen en la app.
  const STACK: { group: () => string; items: string[] }[] = [
    { group: () => t("aboutApp"), items: ["Tauri 2", "Rust", "Svelte 5", "TypeScript", "Vite"] },
    { group: () => t("aboutImage"), items: ["WebGL 2", "WebAssembly", "image", "libheif", "jxl-oxide", "resvg", "psd"] },
    { group: () => t("aboutArchives"), items: ["zip", "unrar", "sevenz-rust2"] },
    { group: () => t("aboutSound"), items: ["Symphonia", "cpal", "hound"] },
    { group: () => t("aboutVideo"), items: ["FFmpeg"] },
    { group: () => t("aboutAi"), items: ["Model Context Protocol (MCP)"] },
  ];

  const system = $derived(
    info
      ? [`imgxplorer ${version}`, `${info.os} ${info.arch}`, info.sessionType, info.webkit && `WebKitGTK ${info.webkit}`]
          .filter(Boolean)
          .join(" · ")
      : "",
  );

  const roleOf = (p: Thanks) => (typeof p.role === "string" ? p.role : p.role?.[currentLang()]);

  async function copyInfo() {
    try {
      await navigator.clipboard.writeText(system);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      /* sin portapapeles: el texto sigue a la vista para copiarlo a mano */
    }
  }
</script>

{#if thirdParty && licenses}
  <ThirdParty data={licenses} onback={() => (thirdParty = false)} />
{:else}
<div class="about">
  <div class="head">
    <img src={icon} alt="" width="72" height="72" />
    <div>
      <h3>imgxplorer <span class="ver">{version}</span></h3>
      <p class="desc">{t("aboutDesc")}</p>
    </div>
  </div>

  <dl class="credits">
    <dt>{t("aboutAuthor")}</dt>
    <dd><b>{AUTHOR}</b></dd>
    <dt>{t("aboutBuiltWith")}</dt>
    <dd><b>Claude Code</b> <span class="muted">· Anthropic</span> <span class="tag">{t("aboutAiTag")}</span></dd>
    <dt>{t("aboutLicense")}</dt>
    <dd>
      <b>MIT</b>
      <button class="small link" onclick={() => (showLicense = !showLicense)}>
        {showLicense ? t("aboutLicenseHide") : t("aboutLicenseView")}
      </button>
    </dd>
  </dl>
  {#if showLicense}<pre class="license">{licenseText}</pre>{/if}

  {#if THANKS.length}
    <section class="thanks">
      <h4>💛 {t("aboutThanksTitle")}</h4>
      <p class="muted small-text">{t("aboutThanksHelp")}</p>
      <div class="people">
        {#each THANKS as p (p.name)}
          <span class="person"><b>{p.name}</b>{#if roleOf(p)}<span class="role">{roleOf(p)}</span>{/if}</span>
        {/each}
      </div>
    </section>
  {/if}

  <h4>{t("aboutStack")}</h4>
  <div class="stack">
    {#each STACK as s (s.items[0])}
      <span class="group">{s.group()}</span>
      <span class="chips">{#each s.items as it (it)}<span class="chip">{it}</span>{/each}</span>
    {/each}
  </div>
  <button class="third" onclick={() => tour.start()}>🧭 {t("tourAgain")}</button>
  {#if licenses}
    <button class="third" onclick={() => (thirdParty = true)}>
      📜 {t("aboutThirdParty", { n: licenses.packages.length + licenses.special.length })}
    </button>
  {/if}

  {#if system}
    <div class="system">
      <span>{system}</span>
      <button class="small" onclick={copyInfo}>{copied ? t("aboutCopied") : t("copy")}</button>
    </div>
  {/if}
</div>
{/if}

<style>
  .about { display: flex; flex-direction: column; gap: 12px; }
  .head { display: flex; gap: 14px; align-items: center; }
  .head img { border-radius: 16px; flex: none; }
  h3 { margin: 0; font-size: 18px; font-weight: 600; }
  .ver { font-size: 13px; font-weight: 400; color: var(--muted); margin-left: 4px; }
  .desc { margin: 4px 0 0; color: var(--muted); line-height: 1.4; }
  .credits { display: grid; grid-template-columns: max-content 1fr; gap: 6px 14px; margin: 0; padding: 10px 12px; background: var(--panel-2); border: 1px solid var(--border); border-radius: 8px; }
  .credits dt { color: var(--muted); }
  .credits dd { margin: 0; }
  .muted { color: var(--muted); }
  .tag { margin-left: 6px; padding: 1px 6px; border-radius: 4px; font-size: 11px; background: rgb(56 189 248 / 0.15); color: var(--accent); }
  h4 { margin: 4px 0 0; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  .stack { display: grid; grid-template-columns: max-content 1fr; gap: 6px 12px; align-items: baseline; }
  .group { color: var(--muted); font-size: 12px; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .chip { padding: 1px 7px; border: 1px solid var(--border); border-radius: 10px; font-size: 12px; background: var(--bg); }
  .system { display: flex; align-items: center; gap: 10px; justify-content: space-between; padding-top: 8px; border-top: 1px solid var(--border); color: var(--muted); font-size: 12px; }
  .small { padding: 1px 8px; font-size: 12px; }
  .link { margin-left: 8px; }
  .license {
    margin: -4px 0 0; padding: 8px 10px; max-height: 200px; overflow: auto; white-space: pre-wrap; font-size: 11px;
    line-height: 1.4; background: var(--bg); border: 1px solid var(--border); border-radius: 8px;
  }
  .thanks {
    display: flex; flex-direction: column; gap: 6px; padding: 10px 12px; border-radius: 8px;
    background: rgb(251 191 36 / 0.06); border: 1px solid rgb(251 191 36 / 0.25);
  }
  .thanks h4 { margin: 0; color: #fbbf24; }
  .small-text { margin: 0; font-size: 12px; }
  .people { display: flex; flex-wrap: wrap; gap: 6px; }
  .person {
    display: flex; flex-direction: column; gap: 2px; flex: 1 1 220px; padding: 6px 10px; border-radius: 8px; font-size: 13px;
    background: var(--bg); border: 1px solid var(--border);
  }
  .role { color: var(--muted); font-size: 11px; line-height: 1.4; }
  .third { align-self: flex-start; padding: 3px 10px; font-size: 12px; }
</style>
