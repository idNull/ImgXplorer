// Recuadro con contenido de cada página (✂ ajustar al contenido). Rust lo calcula una vez por
// página; aquí se recuerda y se avisa con `rev` cuando llega uno nuevo (el visor reencuadra).
import { invoke } from "@tauri-apps/api/core";
import { log } from "../log";

/** `[x, y, w, h]` normalizados (0..1). */
export type Box = [number, number, number, number];

class BoxCache {
  rev = $state(0);
  private generation = -1;
  private done = new Map<number, Box | null>();
  private pending = new Set<number>();

  reset(generation: number) {
    if (generation === this.generation) return;
    this.generation = generation;
    this.done.clear();
    this.pending.clear();
    this.rev++;
  }

  /** El recuadro si ya se sabe (`null`: sin márgenes o aún no); si no, lo pide. */
  get(index: number): Box | null {
    if (this.done.has(index)) return this.done.get(index)!;
    if (!this.pending.has(index) && this.generation >= 0) {
      this.pending.add(index);
      const generation = this.generation;
      invoke<Box | null>("content_box", { generation, index })
        .then((b) => {
          if (generation !== this.generation) return;
          this.done.set(index, b);
          if (b) this.rev++;
        })
        .catch((e) => {
          log.warn(`contenido de la imagen ${index}:`, e);
          if (generation === this.generation) this.done.set(index, null);
        })
        .finally(() => this.pending.delete(index));
    }
    return null;
  }
}

export const boxes = new BoxCache();
