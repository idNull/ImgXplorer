// Envía los errores de la interfaz al mismo archivo de registro que Rust.
import * as plog from "@tauri-apps/plugin-log";

const fmt = (args: unknown[]) =>
  args.map((a) => (a instanceof Error ? `${a.message}\n${a.stack ?? ""}` : typeof a === "string" ? a : JSON.stringify(a))).join(" ");

export const log = {
  info: (...a: unknown[]) => void plog.info(fmt(a)).catch(() => {}),
  warn: (...a: unknown[]) => void plog.warn(fmt(a)).catch(() => {}),
  error: (...a: unknown[]) => void plog.error(fmt(a)).catch(() => {}),
};

export function installErrorLogging(): void {
  window.addEventListener("error", (e) => log.error(`JS: ${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`, e.error ?? ""));
  window.addEventListener("unhandledrejection", (e) => log.error("promesa rechazada sin manejar:", e.reason ?? ""));
  // console.error/warn también acaban en el registro (Svelte y librerías los usan).
  const { error: origError, warn: origWarn } = console;
  console.error = (...a: unknown[]) => {
    origError(...a);
    log.error("console.error:", ...a);
  };
  console.warn = (...a: unknown[]) => {
    origWarn(...a);
    log.warn("console.warn:", ...a);
  };
}
