// Niveles de cada página para «limpiar escaneo». Rust los calcula una vez (y los guarda mientras
// la fuente siga abierta); aquí se recuerdan y se avisa con `rev` cuando llega uno nuevo.
import { invoke } from "@tauri-apps/api/core";
import type { Levels } from "./adjust";
import { log } from "../log";

class LevelsCache {
  /** Cambia cada vez que llega un nivel nuevo: quien dibuja vuelve a pintar. */
  rev = $state(0);
  private generation = -1;
  private done = new Map<number, Levels>();
  private pending = new Map<number, Promise<Levels | null>>();

  /** Olvida lo de la fuente anterior. */
  reset(generation: number) {
    if (generation === this.generation) return;
    this.generation = generation;
    this.done.clear();
    this.pending.clear();
    this.rev++;
  }

  /** Los niveles si ya están; si no, los pide y devuelve `null` (se repinta al llegar). */
  get(index: number): Levels | null {
    const have = this.done.get(index);
    if (have) return have;
    void this.wait(index);
    return null;
  }

  /** Espera a tenerlos (exportar, capturas de la IA). `null` si no se pudieron calcular. */
  wait(index: number): Promise<Levels | null> {
    const have = this.done.get(index);
    if (have) return Promise.resolve(have);
    let p = this.pending.get(index);
    if (!p) {
      const generation = this.generation;
      p = invoke<Levels>("image_levels", { generation, index }).then(
        (l) => {
          if (generation === this.generation) {
            this.done.set(index, l);
            this.rev++;
          }
          return l;
        },
        (e) => {
          log.warn(`niveles de la imagen ${index}:`, e);
          return null;
        },
      );
      this.pending.set(index, p);
    }
    return p;
  }
}

export const levels = new LevelsCache();
