import type { Size } from "../core";
import type { InputSource } from "./input";

export interface ViewerStatus {
  /** % respecto a píxeles físicos (100% = 1:1 en pantalla). */
  zoomPercent: number;
  image: Size | null;
  lastInput: InputSource | "teclado" | null;
  fps: number;
  maxTexture: number;
  error: string | null;
}
