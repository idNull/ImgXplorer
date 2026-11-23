<script lang="ts">
  // Ajustes del pase de fotos (álbumes): transición, tiempo por foto, velocidad de la transición,
  // movimiento de la foto y repetir. Es un diálogo: el foco entra en él al abrirse y vuelve al
  // botón que lo abrió al cerrarse; se cierra con ✕, «Listo», tocando fuera o con Escape (teclado).
  // Todo son controles nativos (botones de opción, deslizador, casilla), así VoiceOver y TalkBack
  // los leen y manejan sin nada especial, y cada uno tiene al menos 48 px de alto.
  import { onDestroy, onMount, tick } from "svelte";
  import { t, type Key } from "./lib/i18n";
  import { MAX_SECONDS, MIN_SECONDS, MOVING, PACES, TRANSITIONS, clampSeconds, reducedMotion, saveShow, show, type Show } from "./lib/slideshow";

  let { playing, onchange, onplay, onclose }: {
    /** Se está reproduciendo: el botón final es «Listo» (los cambios ya se ven) en vez de ▶. */
    playing: boolean;
    onchange: () => void;
    onplay: () => void;
    onclose: () => void;
  } = $props();

  let s = $state<Show>({ ...show });
  const reduce = reducedMotion();
  let sheet = $state<HTMLElement>();
  let opener: Element | null = null;

  function set(p: Partial<Show>) {
    Object.assign(s, p);
    saveShow(p);
    onchange();
  }
  /** El deslizador cambia en cada paso del dedo; el pase se rehace al soltarlo. */
  let pending = 0;
  function setSeconds(v: number, now = false) {
    s.seconds = clampSeconds(v);
    clearTimeout(pending);
    if (now) set({ seconds: s.seconds });
    else pending = window.setTimeout(() => set({ seconds: s.seconds }), 350);
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }

  onMount(async () => {
    opener = document.activeElement;
    await tick();
    sheet?.querySelector<HTMLElement>("h2")?.focus();
  });
  onDestroy(() => {
    clearTimeout(pending);
    if (opener instanceof HTMLElement) opener.focus();
  });

  const trName = (x: string) => t(`tr_${x}` as Key);
  const trDesc = (x: string) => t(`trd_${x}` as Key);
</script>

<svelte:window onkeydown={onkey} />

<!-- El fondo cierra al tocarlo; para el lector de pantalla está el botón ✕ (el fondo no es un control). -->
<div class="backdrop" onclick={onclose} aria-hidden="true"></div>
<div class="sheet" bind:this={sheet} role="dialog" aria-modal="true" aria-labelledby="show-title">
  <header>
    <h2 id="show-title" tabindex="-1">✨ {t("showTitle")}</h2>
    <button class="x" onclick={onclose} aria-label={t("close")}>✕</button>
  </header>

  {#if reduce}
    <p class="note" role="note">♿ {t("reduceNote")}</p>
  {/if}

  <fieldset>
    <legend>{t("showTransition")}</legend>
    <div class="options">
      {#each TRANSITIONS as tr (tr)}
        <label class="opt" class:on={s.transition === tr}>
          <input type="radio" name="show-tr" value={tr} checked={s.transition === tr} onchange={() => set({ transition: tr })} />
          <span class="txt">
            <span class="name">{trName(tr)}{#if reduce && MOVING.has(tr)}<span class="tag"> · {t("moving")}</span>{/if}</span>
            <span class="desc">{trDesc(tr)}</span>
          </span>
        </label>
      {/each}
    </div>
  </fieldset>

  <fieldset>
    <legend id="show-seconds">{t("showSeconds")}: <strong>{t("secondsN", { n: s.seconds })}</strong></legend>
    <div class="stepper">
      <button onclick={() => setSeconds(s.seconds - 1, true)} disabled={s.seconds <= MIN_SECONDS} aria-label={t("fewerSeconds")}>−</button>
      <input
        type="range"
        min={MIN_SECONDS}
        max={MAX_SECONDS}
        step="1"
        value={s.seconds}
        aria-labelledby="show-seconds"
        aria-valuetext={t("secondsLong", { n: s.seconds })}
        oninput={(e) => setSeconds(+(e.currentTarget as HTMLInputElement).value)}
        onchange={(e) => setSeconds(+(e.currentTarget as HTMLInputElement).value, true)}
      />
      <button onclick={() => setSeconds(s.seconds + 1, true)} disabled={s.seconds >= MAX_SECONDS} aria-label={t("moreSeconds")}>+</button>
    </div>
    <div class="chips">
      {#each [2, 3, 5, 8, 10] as n (n)}
        <button class="chip" class:on={s.seconds === n} aria-pressed={s.seconds === n} onclick={() => setSeconds(n, true)}>{t("secondsN", { n })}</button>
      {/each}
    </div>
  </fieldset>

  {#if s.transition !== "cut"}
    <fieldset>
      <legend>{t("showPace")}</legend>
      <div class="segments">
        {#each PACES as p (p)}
          <label class="seg" class:on={s.pace === p}>
            <input type="radio" name="show-pace" value={p} checked={s.pace === p} onchange={() => set({ pace: p })} />
            {t(`pace_${p}` as Key)}
          </label>
        {/each}
      </div>
    </fieldset>
  {/if}

  <fieldset>
    <legend>{t("showMotion")}</legend>
    <div class="options">
      {#each ["still", "gentle"] as const as m (m)}
        <label class="opt" class:on={s.motion === m}>
          <input type="radio" name="show-motion" value={m} checked={s.motion === m} onchange={() => set({ motion: m })} />
          <span class="txt">
            <span class="name">{t(`motion_${m}` as Key)}{#if reduce && m === "gentle"}<span class="tag"> · {t("moving")}</span>{/if}</span>
            <span class="desc">{t(`motiond_${m}` as Key)}</span>
          </span>
        </label>
      {/each}
    </div>
  </fieldset>

  <label class="switch">
    <input type="checkbox" role="switch" checked={s.loop} onchange={(e) => set({ loop: (e.currentTarget as HTMLInputElement).checked })} />
    <span>🔁 {t("showLoop")}</span>
  </label>

  <div class="actions">
    {#if playing}
      <button class="primary" onclick={onclose}>{t("done")}</button>
    {:else}
      <button class="primary" onclick={onplay}>▶ {t("showPlay")}</button>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: absolute; inset: 0; z-index: 4; background: rgb(0 0 0 / 0.5); }
  .sheet {
    position: absolute; left: 0; right: 0; bottom: 0; z-index: 5;
    max-height: 85vh; overflow-y: auto; overscroll-behavior: contain;
    background: var(--panel, #161a20); color: var(--text);
    border-radius: 18px 18px 0 0;
    padding: 6px 16px calc(env(safe-area-inset-bottom) + 16px);
    display: flex; flex-direction: column; gap: 14px;
    animation: up 0.22s ease-out;
  }
  @keyframes up { from { transform: translateY(40px); opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .sheet { animation: none; } }
  header { display: flex; align-items: center; gap: 8px; position: sticky; top: 0; background: inherit; padding-top: 10px; }
  h2 { flex: 1; margin: 0; font-size: 20px; outline: none; }
  .x { min-width: 48px; min-height: 48px; border: 0; border-radius: 999px; background: #2a313b; color: var(--text); font-size: 18px; }
  .note { margin: 0; padding: 10px 12px; border-radius: 12px; background: #1f2b3a; line-height: 1.4; font-size: 15px; }
  fieldset { border: 0; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  legend { font-weight: 600; font-size: 16px; padding: 0 0 6px; }
  .options { display: flex; flex-direction: column; gap: 6px; }
  .opt {
    display: flex; align-items: center; gap: 12px; min-height: 56px; padding: 8px 12px;
    border-radius: 14px; background: #20262e; border: 2px solid transparent;
  }
  .opt.on { border-color: var(--accent); background: #1d2a3a; }
  .opt input, .seg input, .switch input { width: 22px; height: 22px; accent-color: var(--accent); flex: none; margin: 0; }
  .txt { display: flex; flex-direction: column; gap: 2px; }
  .name { font-size: 16px; font-weight: 600; }
  .tag { font-weight: 400; color: var(--muted); }
  .desc { font-size: 14px; color: var(--muted); line-height: 1.3; }
  .stepper { display: flex; align-items: center; gap: 10px; }
  .stepper button {
    min-width: 48px; min-height: 48px; border: 0; border-radius: 999px; background: #2a313b; color: var(--text); font-size: 22px;
  }
  .stepper button:disabled { opacity: 0.35; }
  .stepper input { flex: 1; min-height: 48px; accent-color: var(--accent); }
  .chips { display: flex; gap: 6px; flex-wrap: wrap; }
  .chip { min-height: 44px; min-width: 56px; border: 0; border-radius: 999px; background: #2a313b; color: var(--text); font: inherit; font-size: 15px; }
  .chip.on { background: var(--accent); color: white; }
  .segments { display: flex; gap: 6px; }
  .seg {
    flex: 1; display: flex; align-items: center; justify-content: center; gap: 8px; min-height: 48px;
    border-radius: 12px; background: #20262e; border: 2px solid transparent; font-size: 15px;
  }
  .seg.on { border-color: var(--accent); background: #1d2a3a; }
  .switch { display: flex; align-items: center; gap: 12px; min-height: 52px; padding: 0 12px; border-radius: 14px; background: #20262e; font-size: 16px; }
  .actions { display: flex; }
  .primary { flex: 1; min-height: 52px; border: 0; border-radius: 14px; background: var(--accent); color: white; font: inherit; font-size: 17px; font-weight: 600; }
  button:focus-visible, label:focus-within { outline: 3px solid #9cc7ff; outline-offset: 2px; }
</style>
