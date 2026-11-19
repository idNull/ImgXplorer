import { mount } from "svelte";
import "./app.css";
import "@fontsource/bangers/latin.css";
import "@fontsource/bangers/latin-ext.css";
import { loadTextFonts } from "./lib/fx/text";
import App from "./App.svelte";
import { initCore } from "./lib/core";
import { installErrorLogging, log } from "./lib/log";

installErrorLogging();
try {
  await initCore();
} catch (e) {
  log.error("no se pudo cargar el núcleo WASM:", e);
  throw e;
}
await loadTextFonts();
mount(App, { target: document.getElementById("app")! });
