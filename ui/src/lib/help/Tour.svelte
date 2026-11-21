<script lang="ts">
  // Visita guiada (7 pasos, con «Saltar»). Cada paso ilumina sus botones (`data-tour="…"`); si no
  // se ven, los de `fallback` y, si tampoco, el menú «⋯» (ventana estrecha).
  import { t, type Key } from "../i18n.svelte";
  import { tour } from "./tourState.svelte";

  const STEPS: { icon: string; title: Key; text: Key; targets: string[]; fallback?: string[] }[] = [
    { icon: "📂", title: "tour1Title", text: "tour1Text", targets: ["open"] },
    { icon: "➕", title: "tour2Title", text: "tour2Text", targets: ["create", "organize"], fallback: ["open"] },
    { icon: "◀▶", title: "tour3Title", text: "tour3Text", targets: ["nav"] },
    { icon: "🎨", title: "tour4Title", text: "tour4Text", targets: ["adjust", "reading"] },
    { icon: "🎬", title: "tour5Title", text: "tour5Text", targets: ["action"] },
    { icon: "✎", title: "tour6Title", text: "tour6Text", targets: ["editor"] },
    { icon: "🎞", title: "tour7Title", text: "tour7Text", targets: ["export", "settings"] },
  ];

  interface Box {
    x: number;
    y: number;
    w: number;
    h: number;
  }
  let box = $state<Box | null>(null);
  let rev = $state(0);
  let cardW = $state(360);
  let cardH = $state(200);

  const step = $derived(STEPS[tour.step]);

  /** Recuadro que abarca los botones visibles del paso. */
  function measure(targets: string[], fallback: string[] = []): Box | null {
    const rects = (list: string[]) =>
      list
        .flatMap((k) => [...document.querySelectorAll<HTMLElement>(`[data-tour="${k}"]`)])
        .map((el) => el.getBoundingClientRect())
        .filter((r) => r.width > 0 && r.height > 0);
    let rs = rects(targets);
    if (!rs.length) rs = rects(fallback);
    if (!rs.length) rs = rects(["more"]);
    if (!rs.length) return null;
    const x = Math.min(...rs.map((r) => r.left));
    const y = Math.min(...rs.map((r) => r.top));
    const right = Math.max(...rs.map((r) => r.right));
    const bottom = Math.max(...rs.map((r) => r.bottom));
    const pad = 5;
    return { x: x - pad, y: y - pad, w: right - x + pad * 2, h: bottom - y + pad * 2 };
  }

  $effect(() => {
    void rev;
    const st = step;
    if (!st) return;
    box = measure(st.targets, st.fallback);
    // La cabecera puede recolocarse justo después (ventana estrecha, menú «⋯»): se vuelve a medir.
    const id = setInterval(() => {
      const b = measure(st.targets, st.fallback);
      if (JSON.stringify(b) !== JSON.stringify(box)) box = b;
    }, 250);
    return () => clearInterval(id);
  });

  const cardPos = $derived.by(() => {
    if (!box) return null;
    const left = Math.max(12, Math.min(box.x + box.w / 2 - cardW / 2, window.innerWidth - cardW - 12));
    // Debajo de lo señalado o, si no cabe (la tira de miniaturas, abajo), encima.
    const below = box.y + box.h + 14 + cardH < window.innerHeight - 8;
    const top = below ? box.y + box.h + 14 : Math.max(8, box.y - 14 - cardH);
    return { left, top, below, arrow: Math.max(18, Math.min(box.x + box.w / 2 - left, cardW - 18)) };
  });

  function next() {
    if (tour.step >= STEPS.length - 1) tour.end();
    else tour.step++;
  }
  function back() {
    if (tour.step > 0) tour.step--;
  }

  function key(e: KeyboardEvent) {
    if (tour.step < 0) return; // sin visita, las teclas siguen su camino
    e.stopImmediatePropagation();
    if (e.target instanceof HTMLButtonElement && (e.key === "Enter" || e.key === " ")) return;
    if (e.key === "Escape") tour.end();
    else if (e.key === "ArrowRight" || e.key === "Enter") next();
    else if (e.key === "ArrowLeft") back();
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydowncapture={key} onresize={() => rev++} />

{#if step}
  <div class="tour" role="dialog" aria-modal="true" aria-label={t("tourTitle")}>
    {#if box}
      <div class="hole" style:left="{box.x}px" style:top="{box.y}px" style:width="{box.w}px" style:height="{box.h}px"></div>
    {:else}
      <div class="dim"></div>
    {/if}
    <div class="card" class:center={!cardPos} bind:offsetWidth={cardW} bind:offsetHeight={cardH}
      style:left={cardPos ? `${cardPos.left}px` : null} style:top={cardPos ? `${cardPos.top}px` : null}>
      {#if cardPos}<span class="arrow" class:up={!cardPos.below} style:left="{cardPos.arrow}px"></span>{/if}
      <h3><span class="icon">{step.icon}</span> {t(step.title)}</h3>
      <p>{t(step.text)}</p>
      {#if tour.step === STEPS.length - 1}<p class="tip">{t("tourHelpTip")}</p>{/if}
      <footer>
        <span class="dots" aria-label={t("tourStep", { n: tour.step + 1, total: STEPS.length })}>
          {#each STEPS as _, i (i)}<span class:on={i === tour.step}></span>{/each}
        </span>
        <button class="link" onclick={() => tour.end()}>{t("tourSkip")}</button>
        {#if tour.step > 0}<button onclick={back}>{t("tourBack")}</button>{/if}
        <!-- svelte-ignore a11y_autofocus -->
        <button class="primary" onclick={next} autofocus>{tour.step === STEPS.length - 1 ? t("tourDone") : t("tourNext")}</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .tour { position: fixed; inset: 0; z-index: 80; }
  .dim { position: absolute; inset: 0; background: rgb(0 0 0 / 0.55); }
  .hole {
    position: absolute; border-radius: 10px; border: 2px solid var(--accent); pointer-events: none;
    box-shadow: 0 0 0 9999px rgb(0 0 0 / 0.55), 0 0 18px rgb(56 189 248 / 0.6);
    transition: left 0.25s, top 0.25s, width 0.25s, height 0.25s;
  }
  .card {
    position: absolute; width: min(380px, calc(100vw - 24px)); padding: 14px 16px 12px;
    background: var(--panel); border: 1px solid var(--accent); border-radius: 10px; box-shadow: 0 12px 40px rgb(0 0 0 / 0.5);
    transition: left 0.25s, top 0.25s;
  }
  .card.center { left: 50%; top: 50%; transform: translate(-50%, -50%); }
  .arrow {
    position: absolute; top: -7px; width: 12px; height: 12px; margin-left: -6px; transform: rotate(45deg);
    background: var(--panel); border-left: 1px solid var(--accent); border-top: 1px solid var(--accent);
  }
  .arrow.up { top: auto; bottom: -7px; border: none; border-right: 1px solid var(--accent); border-bottom: 1px solid var(--accent); }
  h3 { margin: 0 0 6px; font-size: 16px; }
  .icon { margin-right: 2px; }
  p { margin: 0 0 8px; line-height: 1.5; font-size: 13.5px; }
  .tip { color: var(--muted); font-size: 12.5px; }
  footer { display: flex; align-items: center; gap: 8px; margin-top: 10px; }
  .dots { display: flex; gap: 5px; flex: 1; }
  .dots span { width: 7px; height: 7px; border-radius: 50%; background: var(--border); }
  .dots span.on { background: var(--accent); }
  .link { background: none; border-color: transparent; color: var(--muted); }
  .link:hover { color: var(--text); }
  .primary { background: var(--accent); color: #04121b; border-color: var(--accent); font-weight: 600; }
</style>
