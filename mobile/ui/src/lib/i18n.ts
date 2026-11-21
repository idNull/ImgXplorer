// Textos del visor móvil, en español e inglés (mismas claves en los dos: lo comprueba `pnpm check`).
const es = {
  subtitle: "Visor de cómics e historias",
  phase: "Fase 0: la app arranca en el móvil",
  bridge: "Puente con Rust",
  version: "Versión",
  system: "Sistema",
  webgl: "WebGL2",
  maxTexture: "Textura máxima",
  screen: "Pantalla",
  formats: "Formatos opcionales",
  yes: "sí",
  no: "no",
  waiting: "comprobando…",
};

const en: typeof es = {
  subtitle: "Comic and story viewer",
  phase: "Phase 0: the app starts on the phone",
  bridge: "Rust bridge",
  version: "Version",
  system: "System",
  webgl: "WebGL2",
  maxTexture: "Max texture",
  screen: "Screen",
  formats: "Optional formats",
  yes: "yes",
  no: "no",
  waiting: "checking…",
};

export type Key = keyof typeof es;
export const lang: "es" | "en" = navigator.language.toLowerCase().startsWith("es") ? "es" : "en";
const dict = lang === "es" ? es : en;
export const t = (k: Key) => dict[k];
