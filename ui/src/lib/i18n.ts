// i18n mínimo (es/en). El selector de idioma llegará con los ajustes.
const dict = {
  es: {
    open: "Abrir carpeta…",
    openFile: "Abrir archivo…",
    openHint: "Abre una carpeta, un cómic (cbz/cbr/cb7) o una imagen, o arrástralos aquí",
    filterAll: "Imágenes y cómics",
    filterArchives: "Comprimidos",
    filterImages: "Imágenes",
    thumbs: "Miniaturas",
    loading: "Cargando…",
    prev: "Anterior",
    next: "Siguiente",
    fit: "Ajustar",
    actual: "Tamaño real",
    rotate: "Rotar",
    fullscreen: "Pantalla completa",
    noImages: "No hay imágenes en esta carpeta",
    tooLarge: "No hay memoria de GPU suficiente para esta imagen (textura máx. {max} px)",
    loadError: "No se pudo cargar la imagen",
    diag: "Diagnóstico",
    lastInput: "Última entrada",
    crashNotice: "La sesión anterior falló ({reason}). Detalles en el registro:",
    dismiss: "Entendido",
  },
  en: {
    open: "Open folder…",
    openFile: "Open file…",
    openHint: "Open a folder, a comic (cbz/cbr/cb7) or an image, or drop them here",
    filterAll: "Images and comics",
    filterArchives: "Archives",
    filterImages: "Images",
    thumbs: "Thumbnails",
    loading: "Loading…",
    prev: "Previous",
    next: "Next",
    fit: "Fit",
    actual: "Actual size",
    rotate: "Rotate",
    fullscreen: "Fullscreen",
    noImages: "No images in this folder",
    tooLarge: "Not enough GPU memory for this image (max texture {max} px)",
    loadError: "Could not load the image",
    diag: "Diagnostics",
    lastInput: "Last input",
    crashNotice: "The previous session crashed ({reason}). Details in the log:",
    dismiss: "Got it",
  },
} as const;

export type Key = keyof (typeof dict)["es"];
const lang: keyof typeof dict = navigator.language.toLowerCase().startsWith("es") ? "es" : "en";

export function t(key: Key, vars: Record<string, string | number> = {}): string {
  return dict[lang][key].replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? ""));
}
