import { mount } from "svelte";
import "./app.css";
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
mount(App, { target: document.getElementById("app")! });
