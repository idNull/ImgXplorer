import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import { initCore } from "$imgx/core";

// El núcleo de imgxplorer (cámara, encuadre) en WebAssembly: lo usa el visor del escritorio.
await initCore();
mount(App, { target: document.getElementById("app")! });
