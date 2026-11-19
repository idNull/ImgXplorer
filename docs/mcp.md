# Control por IA (servidor MCP)

imgxplorer incluye un servidor [MCP](https://modelcontextprotocol.io) para que una IA (Claude Code, Codex, Gemini CLI
u otra que admita servidores MCP locales) maneje la app: mirar las viñetas, animarlas, poner sonido, revisar y
exportar el vídeo.

## Arquitectura

```text
IA (Claude Code / Codex / Gemini)
   │ stdio: JSON-RPC 2.0, un mensaje por línea
imgxplorer --mcp            puente: lista de herramientas y guía (src-tauri/src/mcp/server.rs)
   │ socket Unix privado: una línea JSON por llamada
imgxplorer (la app abierta) host: atiende el socket (src-tauri/src/mcp/host.rs)
   │ evento `mcp-call` → interfaz → comando `mcp_reply`
interfaz                    herramientas (ui/src/lib/mcp/tools.svelte.ts)
```

- **El puente** es el mismo ejecutable con `--mcp`, así que no hay nada aparte que instalar. Lo arranca la IA.
  - Por stdout solo escribe el protocolo.
  - Entiende las versiones 2025-11-25, 2025-06-18, 2025-03-26 y 2024-11-05.
  - Las herramientas y la guía para la IA están en `tools.json` y `guide.md`.
  - Si la app no está abierta, la abre: en un AppImage, el AppImage (`$APPIMAGE`), y en macOS, el `.app` con `open -a`. Después espera hasta 45 s a que esté lista.
- **El socket** está en `$XDG_RUNTIME_DIR/imgxplorer/mcp.sock` en Linux, o en `~/Library/Application Support/imgxplorer/mcp.sock` en macOS.
  - La carpeta es 0700 y el socket 0600.
  - Además se comprueba que quien se conecta es el mismo usuario (`SO_PEERCRED` / `getpeereid`).
  - Sin servidor HTTP no hay puertos, y ninguna página web puede llegar a él.
  - Si hay dos imgxplorer abiertas, escucha la primera.
- **La interfaz hace el trabajo**:
  - Las ediciones pasan por el mismo `ProjectStore` que las del usuario: se ven en directo, se guardan solas y cada llamada es un paso de deshacer.
  - Los fotogramas los pinta el mismo `GlViewer` que la exportación, en un lienzo aparte (`ui/src/lib/mcp/frames.ts`). Así la IA revisa exactamente lo que saldrá en el vídeo.
  - Las llamadas se atienden de una en una.
- **Permiso**: el ajuste `aiControl` viene apagado.
  - Hasta que la interfaz dice cómo está, el host responde `starting` y el puente espera.
  - Apagado, responde `disabled` con un mensaje para que la IA se lo diga al usuario.
  - **Detener la IA** apaga el ajuste y corta la conexión.
- **Imágenes**: cada herramienta visual devuelve la imagen de dos formas, como contenido MCP `image` (JPEG) y como archivo en la caché de la app (`…/mcp/img-NNNNN.jpg`, se guardan las últimas 400).
  - La ruta va en el texto, por si la IA no ve la imagen del resultado: Codex puede abrirla con `view_image`.
  - Por la misma razón el resultado no lleva `structuredContent`, porque Codex descarta las imágenes cuando lo hay.

## Herramientas

Los nombres y las descripciones están en inglés, que es lo que mejor entienden todas las IAs. Las imágenes se indican por índice (desde 0), los tiempos en segundos y los rectángulos normalizados de 0 a 1.

| Grupo | Herramientas |
|---|---|
| Estado | `get_guide`, `app_status`, `open_source`, `list_images`, `go_to`, `undo`, `redo` |
| Ver | `view_image` (página con viñetas numeradas), `view_panel` (viñeta o zona a alta resolución), `get_panels`, `set_panels` |
| Animar | `set_output`, `plan_shots`, `get_timeline`, `set_keyframes`, `apply_template`, `set_transition`, `set_hold_between` |
| Sonido | `list_sounds`, `find_audio_files`, `import_sounds`, `list_tracks`, `add_track`, `edit_track`, `add_audio`, `edit_audio`, `remove_audio`, `add_ambient`, `edit_ambient`, `remove_ambient`, `set_mix`, `effects_catalog` |
| Ajustes de imagen | `get_adjust`, `set_adjust` (limpiar escaneo, brillo, contraste, color, nitidez, blanco y negro o sepia; una imagen o todas) |
| Efectos especiales | `fx_catalog`, `add_fx` (situación o efecto suelto, en segundos o en un plano de `plan_shots` con desfase, y con punto), `edit_fx`, `remove_fx` |
| Voces con IA | `cast_get`, `cast_set`, `voice_list`, `voice_design`, `script_get`, `script_set`, `voices_estimate`, `voices_generate`, `sfx_generate`, `music_generate` |
| Revisar | `summary`, `render_frames`, `sound_report`, `play` |
| Terminar | `export_video`, `export_status`, `cancel_export`, `inspect_video` |

### Planos de director (`plan_shots`)

La IA decide **qué** encuadrar y **cuánto tiempo**; las poses las calcula `imgx-core` (`crates/imgx-core/src/shots.rs`) con el mismo encuadre que la plantilla de cómic (`templates::frame_rect`) y la proporción del vídeo.

```json
{
  "image": 0,
  "direction": "ltr",
  "start": "overview",
  "shots": [
    { "panel": 1, "hold": 2.5 },
    { "panel": 2, "hold": 3, "move": 0.8, "easing": "ease-in-out" },
    { "rect": { "x": 0.12, "y": 0.5, "w": 0.4, "h": 0.1 }, "hold": 1.2, "move": 0, "zoom": 1.1 },
    { "full": true, "hold": 2, "push": 1.05 }
  ]
}
```

- **Cada plano**:
  - llega en `move` segundos con su curva (`move: 0` es un corte seco);
  - se queda `hold` segundos;
  - `push` es un acercamiento lento durante ese tiempo;
  - `margin`, `zoom`, `rot` y `maxZoom` afinan el encuadre.
- **Inicio y final**: `start`/`end` `overview` enseñan la página entera al empezar o acabar.
- **Lo que devuelve**: los tiempos de cada plano (`arrive`, `hold`, `end`), que la IA usa para colocar voces y efectos.
- **El resultado** es un clip de keyframes normal («Planos»), editable en el editor. Conserva los sonidos de la imagen.

### Ajustes de imagen

- `get_adjust` dice los ajustes propios de una imagen, los de todas y los que usa.
- `set_adjust` los cambia para una imagen o, sin `image`, para todas. Lo que no se indica se queda igual; `reset: true` empieza de cero.
- `render_frames` y el vídeo los aplican; `view_image` y `view_panel` enseñan la página original, para leer bien el texto.
- `summary` incluye los ajustes de cada imagen.

### Efectos especiales

- `add_fx` con `situation` añade varios efectos sincronizados y su sonido de serie (salvo `sound: false`) y, con `text: true`, su onomatopeya. Con `type` (y `preset`, `params`) añade uno suelto.
- Textos: `type: "text"` es una onomatopeya animada (`params.text`, `anim`…) y `type: "caption"` un subtítulo, cartel o narración (`params.text`, `style`). `export_video` guarda los subtítulos también como `.srt` (salvo `srt: false`) y admite `format`: `mp4`, `webm` o `gif`.
- `shot` cuenta como `plan_shots`: la vista de la página entera del principio no es un plano. El efecto empieza cuando la cámara llega a ese plano, más `offset`.
- `render_frames` añade por su cuenta un fotograma en el momento fuerte de cada efecto, y `summary` avisa de destellos demasiado seguidos o de efectos que alargan la imagen.

### Voces con IA

- **Reparto y guion** se guardan en el proyecto (`cast` y `items[].lines`, ver [format.md](format.md)). Escribirlos (`cast_set`, `script_set`) no gasta nada.
- **Lo que gasta** (`voice_design`, `voices_generate`, `sfx_generate`, `music_generate`) abre en la app el **resumen del pedido** y espera a que el usuario pulse Aceptar. Si cancela, la herramienta devuelve un error y no se envía nada. Solo se salta el resumen si todo sale de la caché o si cabe en el «presupuesto que la IA puede gastar sin preguntar» (Ajustes → Voces IA, 0 de fábrica).
- **La clave** no la ve la IA ni la interfaz: la usa Rust (`src-tauri/src/voices/`), que es quien llama a ElevenLabs.
- **Caché**: la misma frase con la misma voz, modelo y ajustes no se vuelve a pagar. «Otra toma» usa otra semilla.
- **Tiempo máximo**: estas herramientas esperan hasta 20 minutos (aprobación más generación).

**Plantilla para pedírselo a tu IA:**

```text
Usa imgxplorer para poner voces al capítulo 1 (~/Cómics/capitulo1.cbz). Lee los bocadillos de cada
página, crea el reparto (una voz por personaje, descrita por su aspecto) y escribe el guion en orden
de lectura. Las cartelas, como narración; los gritos del ejército, como multitud de fondo; y música
suave en la última página. Estima el coste antes de generar y genera página a página.
```

### Tiempos de lectura

La guía (`get_guide`) le da a la IA valores de partida, que adapta al ritmo que pida el usuario:
- **Por viñeta**: 1,5 s más 0,3 s por palabra.
- **Arte importante sin texto**: de 2 a 3,5 s.
- **Mínimo**: 2 s.
- **Viajes entre viñetas**: de 0,6 a 1,2 s.
- **Viñetas con mucho texto**: se parten en varios planos.

## Conectar con cada IA

En la app: **Ajustes → IA → Conectar**. Desde la terminal: `imgxplorer --mcp-install [claude|codex|gemini|all]` y `--mcp-uninstall`. Por debajo se usa el comando oficial de cada IA, para todo el usuario:

| IA | Comando | Configuración |
|---|---|---|
| Claude Code | `claude mcp add -s user imgxplorer -- <ruta> --mcp` | `~/.claude.json` |
| Codex (y la app de escritorio de ChatGPT) | `codex mcp add imgxplorer -- <ruta> --mcp` | `~/.codex/config.toml` |
| Gemini CLI | `gemini mcp add -s user imgxplorer <ruta> --mcp` | `~/.gemini/settings.json` |

- **Rutas de la app**:
  - Linux: `~/.local/bin/imgxplorer` (o `/usr/local/bin`); con AppImage, la ruta del AppImage.
  - macOS: `/Applications/imgxplorer.app/Contents/MacOS/imgxplorer`.
- **Programas de las IAs**: se buscan con el `PATH` de la terminal del usuario (las apps de macOS no lo heredan) y en las carpetas habituales (`~/.local/bin`, Homebrew…).
- **Si el programa de Codex o Gemini no está pero su configuración sí**: se escribe la entrada directamente, con copia de seguridad (`*.imgxplorer-backup`).
- **Codex**: se le añade `tool_timeout_sec = 600` para que revisar o exportar no se corte a los 60 s.
- **Probar**: arranca el puente con la ruta registrada, pide la lista de herramientas y `app_status`.
- **Gemini CLI**: solo activa servidores MCP en carpetas de confianza.
- **ChatGPT en la web**: solo admite servidores remotos por HTTPS, así que no se usa (habría que abrir el ordenador a internet). Con una cuenta de ChatGPT se usa Codex.
- **Permisos**: conectar no da permisos especiales. Cada IA sigue pidiendo confirmación antes de usar las herramientas, salvo que el usuario le diga que no lo haga.

## Pruebas

- **`cargo test`**:
  - `shots.rs`: encuadre en 16:9 y en vertical, tiempos, curvas, detalle con zoom y entradas disparatadas.
  - `fx.rs`: forma de cada efecto en el tiempo, mismo resultado al repetir, límite de parpadeos, punto con la cámara movida y situaciones.
  - `mcp::server`: protocolo, lista de herramientas, imágenes en los dos formatos y conversación con una app simulada por el socket, incluido el «aún arranca».
  - `mcp::tools`: análisis de la mezcla y búsqueda de audio.
  - `mcp::install`: tabla de Codex y comillas.
  - `mcp::wire`: base64 y longitud del socket.
- **De principio a fin**: un cliente MCP de prueba arranca `imgxplorer --mcp` con un entorno aislado (otra carpeta de ajustes y otro socket). El puente abre la app y se recorre todo: ver la página, leer una viñeta, planos, hoja de miniaturas, pista, sonidos, ambiente, efectos, resumen, informe de sonido, deshacer, exportar e `inspect_video`.
- **Instalación**: con un `HOME` temporal y los tres programas reales: conectar, que `claude mcp list` diga *Connected* y que Gemini conecte en una carpeta de confianza.
