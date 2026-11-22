// Cómo se lee (para todos los cómics): ajuste de la página y sentido manga. Se recuerda en el
// almacenamiento del WebView.
export type Mode = "page" | "width" | "strip";

export interface Reading {
  mode: Mode;
  rtl: boolean;
}

const KEY = "imgx.reading";

function load(): Reading {
  try {
    const r = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return { mode: ["page", "width", "strip"].includes(r.mode) ? r.mode : "page", rtl: !!r.rtl };
  } catch {
    return { mode: "page", rtl: false };
  }
}

export const reading: Reading = load();

export function saveReading(r: Reading) {
  Object.assign(reading, r);
  localStorage.setItem(KEY, JSON.stringify(reading));
}
