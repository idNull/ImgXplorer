// i18n mínimo (es/en). Se ampliará en la fase 1 con selector de idioma en ajustes.
const dict = {
  es: {
    open: "Abrir carpeta…",
    openHint: "Abre una carpeta o arrastra una aquí",
    prev: "Anterior",
    next: "Siguiente",
    fit: "Ajustar",
    actual: "Tamaño real",
    rotate: "Rotar",
    fullscreen: "Pantalla completa",
    noImages: "No hay imágenes en esta carpeta",
    tooLarge: "Imagen demasiado grande para la GPU (máx. {max} px); llegará en la fase 1",
    loadError: "No se pudo cargar la imagen",
    diag: "Diagnóstico",
    lastInput: "Última entrada",
    crashNotice: "La sesión anterior se cerró de forma inesperada. Detalles en el registro:",
    dismiss: "Entendido",
  },
  en: {
    open: "Open folder…",
    openHint: "Open a folder or drop one here",
    prev: "Previous",
    next: "Next",
    fit: "Fit",
    actual: "Actual size",
    rotate: "Rotate",
    fullscreen: "Fullscreen",
    noImages: "No images in this folder",
    tooLarge: "Image too large for the GPU (max {max} px); coming in phase 1",
    loadError: "Could not load the image",
    diag: "Diagnostics",
    lastInput: "Last input",
    crashNotice: "The previous session closed unexpectedly. Details in the log:",
    dismiss: "Got it",
  },
} as const;

export type Key = keyof (typeof dict)["es"];
const lang: keyof typeof dict = navigator.language.toLowerCase().startsWith("es") ? "es" : "en";

export function t(key: Key, vars: Record<string, string | number> = {}): string {
  return dict[lang][key].replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? ""));
}
