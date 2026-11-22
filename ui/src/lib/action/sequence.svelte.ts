// Cómo se recorre una fuente al reproducir el Modo Acción, sacado de su proyecto: la timeline de
// cada imagen (la propia o la de su plantilla), las transiciones, la voz, los efectos y los
// ambientes. Lo usa el visor móvil para las historias `.imgxstory`. El escritorio hace el mismo
// recorrido en App.svelte, donde además lo comparte con el editor y la exportación.
import { wasm, type Size } from "../core";
import { ambientSpecs } from "../audio/sync";
import { FALLBACK, needsPanels, type TemplateRef } from "./catalog";
import type { PlaySpec } from "./player";
import type { ProjectStore } from "./project.svelte";

export type Sequence = Omit<PlaySpec, "start" | "count" | "onIndex">;

/**
 * `names`: las entradas de la fuente, en orden. `detected(i)`: las viñetas detectadas de la imagen
 * i (`[x, y, w, h, …]`), solo se piden si la plantilla las necesita.
 */
export function projectSequence(store: ProjectStore, names: readonly string[], detected: (i: number) => Promise<number[]>): Sequence {
  const index = new Map(names.map((n, i) => [n, i]));
  const indexOf = (name: string) => index.get(name) ?? -1;

  async function fromTemplate(i: number, tpl: TemplateRef, size: Size, aspect: number): Promise<wasm.WasmTimeline> {
    const own = store.item(names[i])?.panels;
    const manual = !!own?.length;
    const panels = manual ? own!.flatMap((r) => [r.x, r.y, r.w, r.h]) : needsPanels(tpl) ? await detected(i) : [];
    return wasm.WasmTimeline.from_template(JSON.stringify($state.snapshot(tpl)), size.w, size.h, aspect, new Float64Array(panels), manual);
  }

  return {
    async timeline(i, size, aspect) {
      const entry = names[i];
      const it = store.item(entry);
      const tpl = store.templateFor(entry);
      if (!tpl && it?.timeline?.camera?.length) return new wasm.WasmTimeline(JSON.stringify($state.snapshot(it.timeline)));
      return fromTemplate(i, tpl ?? FALLBACK, size, aspect);
    },
    transitionOut(i) {
      const entry = names[i];
      const tr = $state.snapshot(store.transitionFor(entry));
      const tpl = store.templateFor(entry);
      return { json: JSON.stringify(tr), tpl: tpl ? JSON.stringify($state.snapshot(tpl)) : "", duration: tr.duration, cut: tr.type === "cut" };
    },
    holdBetween: () => store.data?.defaults.holdBetween ?? 0.3,
    audio: (i) => store.audioFor(names[i]),
    fx: (i) => $state.snapshot(store.fxFor(names[i])),
    ambient: (i) => ambientSpecs(store.ambients, i, indexOf),
  };
}
