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
- **Recientes**: lo último que has abierto aparece en la pantalla de inicio y en el menú 🕘 (`Ctrl+R`). Al volver a abrir un cómic o una carpeta, se va a la imagen en la que lo dejaste (salvo que abras una imagen concreta). Los que ya no existen salen tachados.
- **Comprimidos sólidos** (7z/rar): al leer una página se guardan también las siguientes, para que pasar páginas sea fluido.
- **Render WebGL2** con mipmaps y filtrado anisótropo.
  - Zoom suave al cursor o al punto del pellizco.
  - Rotación en pasos de 90°.
  - Imágenes más grandes que el máximo de la GPU: se dividen en teselas (p. ej. panorámicas de 20000 px).
- **Precarga** de las imágenes vecinas y caché limitada por memoria.
- **Tira de miniaturas** con carga perezosa, siempre centrada en la imagen actual, en tres tamaños.
- **Entrada**
  - Ratón: rueda para zoom y arrastrar para desplazar.
  - Touchpad: dos dedos para desplazar y pellizco para zoom (GTK en Linux, gestos nativos en macOS).
  - Teclado: atajos de la tabla de abajo.

### Ajustes

Botón ⚙ o `Ctrl+,`. Todo se aplica y se guarda al momento.

- **Idioma**: español, inglés o el del sistema. Cambia sin reiniciar.
- **Fondo del visor**: oscuro, negro, gris, gris claro o cualquier color.
- **Miniaturas**: mostrar u ocultar la tira y su tamaño.
- **Zoom**: cuánto acerca cada paso con teclado y botones, y la velocidad de zoom con la rueda.
- **Retomar**: volver a la imagen en la que lo dejaste (se puede desactivar).
- **Diagnóstico**: mostrar u ocultar el panel.
- **Atajos**: cada acción admite hasta 3 teclas. Pulsa `+` y luego la tecla. Si ya la usaba otra acción del mismo grupo, se le quita y se avisa. Se pueden restablecer una a una o todas.

Los ajustes y los recientes se guardan en `~/.config/com.imgxplorer.app/` (Linux) o `~/Library/Application Support/com.imgxplorer.app/` (macOS).

### Modo Acción

Animaciones de cámara para leer cómics viñeta a viñeta o presentar imágenes. Se abre con el
botón 🎬 o la tecla `A`.

- **Plantillas**, para todas las imágenes o solo para la actual:
  - **Cómic por viñetas**: recorre las viñetas en orden de lectura, de derecha a izquierda (manga) o de izquierda a derecha.
    - Viñetas **detectadas automáticamente** (calles blancas o negras) o por **rejilla** de filas × columnas.
    - Una viñeta alta que ocupa dos filas se lee en su sitio.
    - Las viñetas se ven numeradas sobre la página mientras se ajusta la plantilla.
    - Si una página no tiene viñetas claras (ilustración a página completa), se usa la rejilla.
  - **Desplazamiento**: ajusta al ancho (o al alto) y recorre la imagen, para webtoons y panorámicas.
  - **Rebote**: ciclos de acercar y alejar subiendo y bajando.
  - **Ken Burns** e **imagen completa**.
  - Cada parámetro (tiempos, márgenes, zoom máximo, curva…) se ajusta con su control.
- **Transiciones** entre imágenes: corte, fundido, deslizar y **cámara continua** (las páginas se colocan juntas y la cámara viaja de una a otra), con duración y pausa al final.
- **Reproductor interactivo**:
  - Pausa, pasos (inicio de cada movimiento), ±1 s, imagen anterior o siguiente, velocidad de 0,5× a 2×.
  - Si haces zoom o mueves la imagen a mano, la animación se detiene; un segundo después de soltar, la cámara vuelve sola a donde estaba y sigue.
  - Si cambias el tamaño de la ventana, los encuadres se recalculan. Los cambios de plantilla o de parámetros se aplican al momento, también mientras se reproduce.
- **Guardado automático** en un archivo `.imgx` junto al original (`Tomo 1.cbz.imgx`, o `.imgxplorer.imgx` dentro de la carpeta). Si ahí no se puede escribir, va al directorio de datos de la app. Un archivo de una versión más nueva no se sobrescribe.

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

Estos son los de fábrica; todos se pueden cambiar en los ajustes.

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
| `A` | Panel del Modo Acción |
| `P` | Reproducir desde la imagen actual / parar |
| `D` | Panel de diagnóstico |
| `F11` | Pantalla completa |
| `Ctrl+O` / `Ctrl+Mayús+O` | Abrir archivo / abrir carpeta |
| `Ctrl+R` | Recientes |
| `Ctrl+,` | Ajustes |

Doble clic: alterna entre ajustar y tamaño real en el punto pulsado.

Durante la reproducción:

| Tecla | Acción |
|---|---|
| `Espacio` | Pausa / continuar |
| `←` / `→` | Paso anterior / siguiente |
| `Mayús+←` / `Mayús+→` | −1 s / +1 s |
| `RePág` / `AvPág` · `↑` / `↓` | Imagen anterior / siguiente |
| `Inicio` / `Fin` | Primera / última imagen |
| `[` / `]` | Más lento / más rápido |
| `Esc` · `P` | Parar |

Zoom, desplazamiento y rotación (ratón, touchpad o teclado) toman el control manual.

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
- [x] **Fase 1**: visor completo.
  - [x] Comprimidos.
  - [x] Todos los formatos.
  - [x] Miniaturas.
  - [x] Teselado.
  - [x] Imagen suelta en su posición.
  - [x] Ajustes e idioma (es/en).
  - [x] Recientes y retomar la página.
  - [x] Atajos configurables.
- [x] **Fase 2**: Modo Acción.
  - [x] Motor de animación y formato `.imgx` con guardado automático.
  - [x] Plantillas: cómic por viñetas (con detección automática), desplazamiento, rebote, Ken Burns e imagen completa.
  - [x] Reproductor interactivo: pausa, pasos, velocidad, y control manual que luego vuelve a la animación.
  - [x] Transiciones: corte, fundido, deslizar y cámara continua.
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

Para probar la decodificación y la detección de viñetas con tus propios archivos (RAW, HEIC, cómics…) sin abrir la app (la ruta debe ser absoluta):

```bash
IMGX_SAMPLES=~/Imágenes cargo test -p imgx-media --release --test samples -- --ignored --nocapture
```

### Estructura

```
crates/imgx-core    Cámara, easing, esquema .imgx, timeline, plantillas, orden de viñetas y transiciones (Rust puro, nativo + WASM)
crates/imgx-media   Fuentes (carpeta, zip, 7z, rar), decodificadores, RAW, miniaturas y detección de viñetas
crates/imgx-wasm    Enlace de imgx-core para la interfaz
src-tauri           App Tauri 2: protocolo imgx://, gestos, diagnóstico, archivos .imgx, ajustes y recientes
ui                  Interfaz Svelte 5 + TypeScript: visor WebGL2, miniaturas, ajustes y atajos, Modo Acción (ui/src/lib/action)
docs                Especificación del formato .imgx
```
