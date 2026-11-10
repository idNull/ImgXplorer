# imgxplorer

Visor de imágenes y cómics para **Linux y macOS** con un **Modo Acción**: animaciones de cámara (paneo, zoom, rotación) por imagen sobre una línea de tiempo, pensadas para leer cómics viñeta a viñeta y exportarlos a MP4.

> Estado: **en desarrollo**. Este README recoge lo que ya funciona y lo que viene. Se actualiza con cada fase.

## Funciones

### Visor

- **Fuentes**
  - Carpetas, recorridas de forma recursiva.
  - Cómics y comprimidos: `zip`/`cbz`, `7z`/`cb7`, `rar`/`cbr`.
  - Imágenes sueltas: al abrirlas se abre su carpeta, situada en esa imagen.
- **Orden natural** (`pagina 2` va antes que `pagina 10`). Se respetan las subcarpetas (capítulos) y se ignoran los archivos ocultos y `__MACOSX`.
- Se abren desde el botón, arrastrando o desde la línea de comandos: `imgxplorer <carpeta|cómic|imagen>`.
- **Comprimidos sólidos** (7z/rar): al leer una página se guardan también las siguientes, para que pasar páginas sea fluido.
- **Render WebGL2** con mipmaps y filtrado anisótropo.
  - Zoom suave al cursor o al punto del pellizco.
  - Rotación en pasos de 90°.
  - Imágenes más grandes que el máximo de la GPU: se dividen en teselas (p. ej. panorámicas de 20000 px).
- **Precarga** de las imágenes vecinas y caché limitada por memoria.
- **Tira de miniaturas** con carga perezosa, siempre centrada en la imagen actual.
- **Entrada**
  - Ratón: rueda para zoom y arrastrar para desplazar.
  - Touchpad: dos dedos para desplazar y pellizco para zoom (GTK en Linux, gestos nativos en macOS).
  - Teclado: atajos de la tabla de abajo.

### Formatos

| Tipo | Formatos | Quién decodifica |
|---|---|---|
| Web | JPG, PNG/APNG, GIF, WebP, BMP, AVIF | El webview (con respaldo en Rust si falla) |
| Modernos | HEIC/HEIF, JPEG XL | Rust (libheif, jxl-oxide) |
| Profesionales | TIFF, PSD, EXR, HDR, TGA, QOI, DDS, PNM, ICO | Rust (`image`, `psd`) |
| Vectorial | SVG/SVGZ, rasterizado nítido a 4096 px | Rust (resvg) |
| RAW de cámara | DNG, CR2, CR3, NEF, ARW, RAF, ORF, RW2, PEF, SRW… | Vista previa JPEG incrustada (resolución completa en casi todas las cámaras) |

Se aplica la orientación EXIF en todos los formatos.

### Atajos de teclado

| Tecla | Acción |
|---|---|
| `→` `↓` · `AvPág` · `Espacio` | Siguiente imagen. Con zoom, las flechas desplazan (más con `Mayús`) |
| `←` `↑` · `RePág` · `Retroceso` | Imagen anterior |
| `Inicio` / `Fin` | Primera / última imagen |
| `+` / `-` | Acercar / alejar |
| `0` · `F` | Ajustar a la ventana |
| `1` | Tamaño real (1:1 en píxeles de pantalla) |
| `R` / `Mayús+R` | Rotar 90° a la derecha / izquierda |
| `T` | Mostrar u ocultar miniaturas |
| `D` | Panel de diagnóstico |
| `F11` | Pantalla completa |
| `Ctrl+O` / `Ctrl+Mayús+O` | Abrir archivo / abrir carpeta |

Doble clic: alterna entre ajustar y tamaño real en el punto pulsado.

### Diagnóstico y registro de errores

- **Registro con rotación** (5 archivos de 2 MB) en:
  - Linux: `~/.local/share/com.imgxplorer.app/logs/imgxplorer.log`
  - macOS: `~/Library/Logs/com.imgxplorer.app/`
- Qué guarda:
  - Errores de Rust y de la interfaz.
  - *Panics* con su traza.
  - Aperturas, decodificaciones y gestos.
- **Detección de fallos**: si la app se cierra por un fallo (SIGSEGV, SIGABRT, panic…), al reabrirla se muestra un aviso con la causa, la ruta del registro y el comando para ver el volcado (`coredumpctl info <pid>`). Si la app se cerró a la fuerza (por ejemplo, al recompilar) no se muestra el aviso.
- Si el proceso web de WebKit muere, se registra y la interfaz se recarga sola.
- El panel `D` muestra:
  - Última entrada, FPS y tamaño máximo de textura.
  - Si la imagen la decodificó el webview o Rust, y cuánto tardó.
  - El número de teselas.

## Hoja de ruta

- [x] **Fase 0**: base, motor de cámara compartido (Rust → WASM), visor WebGL, gestos y registro de errores.
- [ ] **Fase 1**: visor completo.
  - [x] Comprimidos.
  - [x] Todos los formatos.
  - [x] Miniaturas.
  - [x] Teselado.
  - [x] Imagen suelta en su posición.
  - [ ] Ajustes e idioma (es/en).
  - [ ] Recientes y retomar la página.
  - [ ] Atajos configurables.
- [ ] **Fase 2**: Modo Acción.
  - Motor de animación y formato `.imgx`.
  - Plantillas:
    - Scroll vertical.
    - Cómic por bloques (derecha→izquierda o izquierda→derecha, con detección de viñetas).
    - Rebote.
  - Reproductor interactivo:
    - Pausa y saltos.
    - Si haces zoom a mano, la animación se interrumpe y luego vuelve a la pose donde estaba.
  - Transiciones entre imágenes.
- [ ] **Fase 3**: editor de línea de tiempo.
  - Capturar y grabar movimientos.
  - Clips con nombre, mover y recortar en la línea de tiempo.
  - Easing por tramo.
  - Deshacer.
  - Exportar e importar `.imgxpack`.
- [ ] **Fase 4**: render a MP4 (ffmpeg) y paquetes: AppImage, Flatpak/AUR, `.dmg` firmado.
- [ ] **Fase 5**: pistas de audio (voces, efectos, ambiente).

El formato de animaciones está especificado en [docs/format.md](docs/format.md).

## Desarrollo

**Requisitos**
- Rust 1.88 o superior, con el target `wasm32-unknown-unknown`.
- `wasm-pack`.
- Node 22 o superior y `pnpm`.
- `libheif`.
- En Linux, WebKitGTK 4.1.

En Arch/CachyOS:

```bash
sudo pacman -S rust rust-wasm wasm-pack libheif webkit2gtk-4.1 pnpm
```

**Arrancar**

```bash
pnpm install
pnpm wasm          # compila imgx-core a WASM para la interfaz
pnpm tauri dev
```

**Pruebas**

```bash
cargo test --workspace
pnpm check
```

Para probar la decodificación con tus propios archivos (RAW, HEIC…) sin abrir la app:

```bash
IMGX_SAMPLES=~/Imágenes cargo test -p imgx-media --release --test samples -- --ignored --nocapture
```

### Estructura

```
crates/imgx-core    Cámara, easing, esquema .imgx y evaluador de timeline (Rust puro, nativo + WASM)
crates/imgx-media   Fuentes (carpeta, zip, 7z, rar), decodificadores, RAW y miniaturas
crates/imgx-wasm    Enlace de imgx-core para la interfaz
src-tauri           App Tauri 2: protocolo imgx://, gestos, diagnóstico
ui                  Interfaz Svelte 5 + TypeScript: visor WebGL2, miniaturas
docs                Especificación del formato .imgx
```
