# Cambios

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/). La versión sigue [Semantic Versioning](https://semver.org/lang/es/).

## [1.0.0] — en pruebas, sin publicar

*imgXplorer — tu cómic, viñeta a viñeta.* La primera versión completa: un visor rápido de cómics e imágenes para Linux y macOS que, si quieres, convierte un cómic en un vídeo animado viñeta a viñeta, con sonido, efectos y voces.

### Visor

- **Fuentes:** carpetas, `cbz`/`zip`, `cbr`/`rar`, `cb7`/`7z` e imágenes sueltas, en orden natural.
- **Formatos:** JPG, PNG, GIF, WebP, AVIF, HEIC, JPEG XL, TIFF, PSD, EXR, SVG y RAW de cámara, con la orientación EXIF.
- **Dibujo** con WebGL2: zoom suave, rotación, imágenes enormes en teselas, precarga de las vecinas y miniaturas.
- **Estantería** con portadas, progreso y «Continuar leyendo». Retoma la página donde lo dejaste.
- **Selector de archivos** propio para escritorios sin diálogo usable, y atajos configurables.

### Lectura

- **Ajustes de imagen (🎨):** limpiar escaneo, blanco y negro, sepia, modo noche, brillo, contraste, nitidez… sin tocar el archivo.
- **Modos de lectura (📖):**
  - ajustar al ancho, al alto o al contenido;
  - doble página y sentido manga;
  - tira vertical para webtoons.
- **Para moverse:** lupa, ir a página, marcadores y barra de progreso.
- **Utilidades:** información y datos de la cámara, copiar y guardar la imagen, recortar una zona, mostrar en la carpeta y mover a la papelera.

### Modo Acción y editor

- **Plantillas de cámara:** cómic por viñetas (con detección automática y corrección a mano), desplazamiento, rebote, Ken Burns e imagen completa.
- **Reproductor y transiciones:** reproductor interactivo; transiciones de corte, fundido, deslizar y cámara continua.
- **Editor de línea de tiempo:**
  - claves con la vista actual y grabación del movimiento;
  - clips con nombre, curvas por tramo y marco de reproducción o de vídeo;
  - biblioteca de clips y paquetes `.imgxpack`.
- **Deshacer y guardado:** se puede deshacer todo, y el guardado automático va a un `.imgx` junto al cómic.

### Sonido, efectos y voces

- **Motor de sonido propio:**
  - voz, efectos y ambientes en varias pistas;
  - mezclador y efectos de audio (reverb, eco, filtros, tono, robot…);
  - grabación con el micrófono y 14 sonidos de serie.
- **Efectos especiales:**
  - temblor, destello, onda expansiva, líneas de velocidad…;
  - 13 situaciones listas, como explosión, puñetazo o flashback;
  - onomatopeyas animadas, subtítulos y cartelas, con botón «✓ Aplicar» para el texto;
  - la aberración y el glitch se ven en toda la imagen, no solo en los bordes.
- **Voces con IA (ElevenLabs):**
  - reparto de personajes con voces creadas a partir de una descripción;
  - guion por imagen con etiquetas de interpretación;
  - efectos y música;
  - resumen del pedido antes de gastar, caché, clave en el llavero del sistema y un modo simulado para probar sin gastar.
- **Borrar en la línea de tiempo:**
  - `Supr` y `Retroceso` borran lo seleccionado;
  - un ambiente que pasa por varias imágenes se puede quitar solo de una.

### Vídeo

- **Formatos:** MP4 (H.264 + AAC), WebM (VP9 + Opus) y GIF, con «Para redes (9:16)» y `.srt` de los subtítulos.
- **Fidelidad:** se dibuja con el mismo motor que la vista previa, así que sale idéntico.

### Tus propios cómics

- **Crear un cómic con imágenes sueltas** (de una IA, escaneadas…): se copian numeradas en una carpeta nueva de `Imágenes/imgxplorer`, sin tocar las originales, y el cómic queda en la estantería.
- **Ordenar las páginas** arrastrando las miniaturas o en la mesa de luz: añadir delante o detrás de otra, ordenar por nombre, quitar y deshacer. La animación y los marcadores siguen a su página.
- **Botón «📂 Abrir ▾»** con un menú (abrir archivo o carpeta, crear cómic, organizar páginas) y tarjetas grandes en la pantalla de inicio.

### Compartir historias

- **Historias `.imgxstory`:** el cómic animado completo en un solo archivo (páginas, animación, efectos, textos, sonidos y voces) para compartirlo. Quien lo recibe lo abre en imgxplorer, lo ve tal cual y puede retocarlo sin cambiar el original.
- **Dos variantes del mismo formato:** escritorio (calidad original) y móvil (páginas reducidas, mucho más ligera). Es la base de la futura versión para el móvil.

### Control por IA

- **Servidor MCP** para Claude Code, Codex y Gemini CLI, con 59 herramientas: crear y ordenar cómics, ver páginas y viñetas, planos de director, sonido, efectos, ajustes de imagen, voces, revisión y exportación.
- **Bajo tu control:** está apagado hasta que lo permites, los cambios se ven en directo y se pueden deshacer, y nada que gaste dinero se envía sin tu aprobación.

### Comodidad

- **Ayuda de atajos** (`?`) con buscador.
- **Pantalla completa limpia** con barras que se esconden solas.
- **Visita guiada** de siete pasos la primera vez, con aviso de novedades para quien vio una anterior.
- **Cabecera adaptable** con el menú «⋯» cuando no caben los botones.
- **En inglés y en español**, con foco visible para el teclado. El idioma cambia al momento en toda la app, también en los mensajes de error, y una comprobación automática vigila que no falte ningún texto.

### Icono

- **Icono nuevo:** una página de cómic con la cámara de la app enfocando una viñeta y el botón de reproducir, con los colores de la interfaz y el azul de la X de imgXplorer.
- **Tres versiones en SVG** (`src-tauri/icons/svg`): completa desde 128 px, intermedia para 48-64 px (sin rayos ni detalles finos) y simple para 16-32 px, para que se reconozca en cualquier tamaño. En el Mac todas llevan el margen de su rejilla. `pnpm icons` genera los PNG, el `.ico` y el `.icns`.

### Instalar

- **Linux:** binario compatible desde Ubuntu 24.04, con un instalador que comprueba el sistema y pone lo que falta.
- **macOS:** un instalador que compila la app en el propio Mac.
- **Licencias:** licencia MIT, y las de terceros en «Acerca de» y en `THIRD-PARTY.md`.
