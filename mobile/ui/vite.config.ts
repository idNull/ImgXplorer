import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";

// En el móvil, Tauri pasa la IP del ordenador para que el teléfono cargue la interfaz por la red.
const host = process.env.TAURI_DEV_HOST;
const repo = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  root: import.meta.dirname,
  plugins: [svelte()],
  clearScreen: false,
  resolve: {
    // Los módulos de la app de escritorio que se reutilizan (visor WebGL, efectos, sonido…).
    alias: { $imgx: fileURLToPath(new URL("../../ui/src/lib", import.meta.url)) },
  },
  server: {
    // 1420/1421 son del escritorio: así se pueden tener los dos en marcha a la vez.
    port: 1430,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1431 } : undefined,
    fs: { allow: [repo] },
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // WKWebView de iOS 15 y el WebView de Android 8 actualizado.
    target: ["safari15", "chrome100"],
  },
});
