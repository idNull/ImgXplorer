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
- **Selector de archivos propio** para abrir y guardar: accesos a tus carpetas y unidades montadas, ruta editable, archivos ocultos y aviso antes de reemplazar. Se maneja con el teclado (flechas, `Intro` para entrar o aceptar, `Retroceso` para subir, letras para saltar, `Esc` para cancelar). Existe porque en algunos escritorios Linux (Hyprland/Omarchy…) el diálogo del sistema sale sin botón de Aceptar.
- **Recientes**: lo último que has abierto aparece en la pantalla de inicio y en el menú 🕘 (`Ctrl+R`). Al volver a abrir un cómic o una carpeta, se va a la imagen en la que lo dejaste (salvo que abras una imagen concreta). Los que ya no existen salen tachados.
- **Cerrar** (botón ✕ junto al título o `Ctrl+W`): cierra lo que estás viendo y vuelve a la pantalla de inicio con los recientes.
  - Las animaciones se guardan solas medio segundo después de cada cambio, así que normalmente cierra sin preguntar.
  - Si una grabación está en curso, la termina y la guarda.
  - Solo avisa si el guardado ha fallado (sin permiso, disco lleno…): dice cuántos cambios se perderían y por qué, y deja reintentar, cerrar sin guardar o cancelar.
  - El mismo aviso sale al abrir otro archivo o al cerrar la ventana. Al cerrar la ventana, además, se espera a que se guarde el último cambio.
  - El historial de deshacer es de la sesión: al cerrar o abrir otro archivo empieza de cero, pero lo hecho ya está guardado.
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
- **Selector de archivos**: el de imgxplorer, el del sistema o automático (el propio en Linux y el del sistema en macOS).
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
    - **Corregirlas a mano** ("Editar viñetas"): arrastra en la página para dibujar una, arrástrala para moverla o usa sus tiradores para cambiar el tamaño; `Supr` la borra. Las dibujadas a mano sustituyen a la detección en esa imagen y se pueden devolver a la automática.
  - **Desplazamiento**: ajusta al ancho (o al alto) y recorre la imagen, para webtoons y panorámicas.
  - **Rebote**: ciclos de acercar y alejar subiendo y bajando.
  - **Ken Burns** e **imagen completa**.
  - Cada parámetro (tiempos, márgenes, zoom máximo, curva…) se ajusta con su control.
- **Transiciones** entre imágenes: corte, fundido, deslizar y **cámara continua** (las páginas se colocan juntas y la cámara viaja de una a otra), con duración y pausa al final.
- **Reproductor interactivo**:
  - Pausa, pasos (inicio de cada movimiento), ±1 s, imagen anterior o siguiente, velocidad de 0,5× a 2×.
  - Si haces zoom o mueves la imagen a mano, la animación se detiene; un segundo después de soltar, la cámara vuelve sola a donde estaba y sigue.
  - Si cambias el tamaño de la ventana, los encuadres se recalculan. Los cambios de plantilla o de parámetros se aplican al momento, también mientras se reproduce.
- **Deshacer y rehacer** (`Ctrl+Z` / `Ctrl+Mayús+Z`) cualquier cambio del Modo Acción y del editor; un arrastre completo cuenta como un solo paso.
- **Guardado automático** en un archivo `.imgx` junto al original (`Tomo 1.cbz.imgx`, o `.imgxplorer.imgx` dentro de la carpeta). Si ahí no se puede escribir, va al directorio de datos de la app. Un archivo de una versión más nueva no se sobrescribe.

### Editor de línea de tiempo

Para hacer la animación de una imagen a medida. Se abre con el botón ✎ o la tecla `E` y aparece
debajo de la imagen: biblioteca a la izquierda, pista de cámara en el centro e inspector a la derecha.
La biblioteca y el inspector se ensanchan arrastrando su borde (doble clic o el botón ↔ ajustan la
biblioteca a los nombres largos).

- **Marco** (▭, arriba en el editor): el visor se estrecha a la proporción final y lo que ves dentro del recuadro ámbar es exactamente lo que se verá.
  - *Reproducción* (de fábrica): la del visor al reproducir (el editor se oculta y el visor crece, así que sin marco el encuadre cambiaría).
  - *Vídeo*: la del vídeo que vas a exportar (por ejemplo 1080×1920 en vertical).
  - *Sin marco*.
- **Claves con la vista actual** (`K`): encuadra la imagen con el ratón, el touchpad o el teclado y pulsa `K`. La clave se guarda en el cabezal; si estás al final, el cabezal avanza 1 s para la siguiente. Pulsar `K` sobre una clave existente la sustituye.
- **Grabar** (`G`): graba el movimiento real de la cámara mientras lo haces y lo guarda como clip ("Grabación 1"…). Se suaviza el temblor y se simplifica sin perder el ritmo. `G` o `Espacio` terminan, `Esc` descarta.
- **Pista de cámara**:
  - Arrastra un clip para moverlo y sus bordes para estirarlo (cambia la velocidad); con `Mayús`, los bordes recortan.
  - Las claves (◆) se arrastran por el tiempo sin pasar a sus vecinas.
  - Todo se pega al cabezal, a los bordes de otros clips y a las claves.
  - Si dos clips se solapan, manda el que empieza después; cuando termina, sigue el de debajo.
  - Rueda del ratón para moverse por la pista y `Ctrl+rueda` (o `−` `+`) para acercarla.
  - Las pistas de voz, efectos y ambiente aparecen ya, desactivadas, para la fase de sonido.
- **Cabezal**: clic o arrastre en la regla para ver cada momento; `,` y `.` saltan a la clave anterior o siguiente; `Espacio` reproduce la vista previa de la imagen.
- **Inspector**:
  - De un clip: nombre, inicio, duración, velocidad, recortes; guardarlo en la biblioteca, duplicarlo o borrarlo. En las esperas, fijar la vista actual o mantener la anterior.
  - De una clave: instante, **curvas** y la pose exacta (x, y, zoom, giro); "Usar la vista actual" la cambia por lo que se ve.
  - Cada clave tiene dos curvas: la de **llegada** (cómo se viene desde la clave anterior) y la de **salida** (cómo se va a la siguiente). Opciones: lineal, suave, acelerando, frenando, rebote, elástico, salto o Bézier con sus 4 valores. Cada una se dibuja con una explicación breve, y en la pista se ve la forma de la curva de cada tramo; al elegir una clave se resaltan sus dos tramos.
- **Plantillas y editor**: si la imagen usa una plantilla, el editor muestra su animación (con borde discontinuo). Al editarla (añadir una clave, grabar, mover un clip…) se convierte en clips propios de esa imagen, que mandan sobre las plantillas. "Quitar la animación propia" vuelve a la plantilla.
- **Biblioteca**:
  - Clips de fábrica (acercar, alejar, paneos, balanceo, esperas) y los tuyos. Arrástralos a la pista o pulsa `+` para añadirlos al final. Doble clic para renombrar.
  - Plantillas propias: guarda la plantilla de una imagen con sus parámetros y aplícala luego a una imagen o a todas.
  - **Exportar e importar** clips y plantillas en paquetes `.imgxpack` para compartirlos. También se importan soltando el archivo sobre la ventana. Al importar no se duplica lo que ya tienes.
  - Se guarda en `~/.local/share/com.imgxplorer.app/library.json` (Linux) o `~/Library/Application Support/com.imgxplorer.app/` (macOS).

### Exportar a vídeo

Botón 🎞 o `Ctrl+E`. Se exporta tal cual se reproduce: la animación de cada imagen, la pausa entre
imágenes y las transiciones, con el color de fondo del visor.

- **Qué**: la imagen actual, todas o un rango ("desde la 3 hasta la 8").
- **Tamaño**: 720p, 1080p, 1440p o 4K, en horizontal (16:9), vertical (9:16, para móvil) o cuadrado; o a medida.
- **Fotogramas por segundo**: 24, 30 o 60. **Calidad**: alta, normal o ligera.
- **Archivo**: se propone junto al cómic o la carpeta, con su nombre. Si ya existe se avisa.
- Barra de progreso con el tiempo que queda; se puede cancelar y el archivo a medias se borra.
- Las plantillas se recalculan para la forma del vídeo. Para encuadrar a mano con esa forma, usa el marco *vídeo* del editor.
- **Cómo funciona**: cada fotograma se pinta con el mismo motor WebGL de la vista previa (así sale idéntico), se convierte a YUV 4:2:0 en la GPU y se pasa por tubería a **ffmpeg** (H.264 en MP4, BT.709). ffmpeg escribe en un archivo temporal que se renombra al terminar bien.
- **ffmpeg** es necesario y no viene incluido: se busca en el sistema (en macOS también en Homebrew) o se indica su ruta en la propia ventana. Si falta, la ventana explica cómo instalarlo (`sudo pacman -S ffmpeg`, `sudo apt install ffmpeg`, `brew install ffmpeg`). Usa `libx264` si está; si no, `h264_videotoolbox` (macOS) u `libopenh264`.

### Sonido

Cada imagen puede llevar **voz** y **efectos**, y el proyecto, **ambientes o música** que suenan de una imagen a otra. Se oye al reproducir en el Modo Acción y en el editor, y sale en el vídeo exportado.

- **Biblioteca de sonidos** (pestaña *Sonidos* del editor):
  - **Tuyos**: importa mp3, wav, flac, ogg o m4a con *Importar sonido* o arrastrando los archivos a la ventana. Se **copian** a tu biblioteca, así que no se rompen si mueves o borras el original. El mismo archivo importado dos veces es el mismo sonido.
  - **Grabaciones**: las voces grabadas con el micrófono.
  - **De serie**: 14 efectos que fabrica la propia app por síntesis, sin descargas ni licencias. Hay lluvia, viento, oleaje, latido, trueno, golpe, puñetazo, zas, explosión, llamar a la puerta, pasos, campana, timbre y disparo. Los marcados con 🔁 están pensados para sonar en bucle.
  - ▶ para escuchar, doble clic para renombrar y ✕ para quitarlo de la biblioteca (el archivo original no se toca).
- **Pistas**: hay tres grupos (**voces**, **efectos** y **ambiente**) y en cada uno puedes tener **varias pistas** («Voz Ana», «Voz Luis», «Golpes»…), las mismas en todas las imágenes, como en un programa de audio. Así se mezclan varios sonidos a la vez sin salir de la app.
  - **＋** en la cabecera de la última pista de un grupo añade otra. Doble clic en el nombre la renombra.
  - **M** silencia la pista (tampoco sale en el vídeo) y **S** deja solo esa mientras editas (no se guarda ni afecta al vídeo).
  - Al pulsar la cabecera, el inspector muestra su **volumen**, **panorama** (izquierda/derecha), sus **efectos de audio**, el orden dentro del grupo y *Borrar pista*. Borrar una pista borra lo que suena en ella (avisa de cuántos sonidos son) y se puede deshacer. Cada grupo conserva al menos una pista.
  - Con muchas pistas, la zona de pistas se desplaza en vertical: rueda sobre los nombres o `Mayús` + rueda. La altura del editor se cambia arrastrando su borde superior (doble clic la devuelve a la normal).
  - Los proyectos de antes tienen una pista por grupo y se abren igual.
- **Clips de sonido**: arrastra un sonido a la fila de la pista que quieras (o pulsa **+** en la biblioteca para ponerlo en el cabezal, en la última pista usada). Cada clip muestra su forma de onda y se mueve con ajuste magnético a claves y otros clips; arrastrándolo hacia arriba o abajo cambia de pista dentro de su grupo.
  - Sus bordes recortan: el izquierdo salta el principio del sonido y el derecho lo acorta.
  - En el inspector se ajustan el inicio, la duración, el recorte, el **volumen** (hasta el 200 %), los **fundidos** de entrada y salida, la **pista** (también de voz a efectos o al revés) y los **efectos de audio del propio clip**. Desde ahí también se duplica o se borra.
  - Todo entra en deshacer y rehacer.
  - **La imagen dura al menos lo que su sonido**: la voz nunca se corta. La cámara se queda en su última pose hasta que acaba.
  - El sonido no depende de la cámara: una imagen puede usar una plantilla y tener su propia voz.
- **Grabar voz** (botón 🎙 *Voz* o `V`): graba del micrófono mientras la animación avanza desde el cabezal.
  - Muestra el nivel (en rojo si es demasiado alto) y los segundos grabados.
  - `V` o `Espacio` terminan, y `Esc` descarta.
  - La grabación se guarda en tu biblioteca («Voz 1»…) y se coloca en la pista de voz seleccionada (o en la primera); el medidor aparece en su cabecera.
  - Mientras grabas, el sonido se silencia para que el micrófono no lo recoja.
  - El micrófono se elige en los ajustes. En macOS, la primera vez el sistema pide permiso.
- **Ambientes y música** (sección *Sonido* del Modo Acción): elige un sonido y *Añadir*.
  - Suena desde la imagen actual hasta la última, en bucle; luego se cambia *desde* y *hasta*, el volumen y los fundidos.
  - También se puede soltar un sonido en una pista de ambiente del editor, y entonces suena en esa imagen. Cada ambiente puede ir en su propia pista y llevar sus efectos.
  - Al pasar de una imagen a otra del tramo, el ambiente **no se corta**: solo se funde al entrar y al salir.
  - Si mueves la cámara a mano, la voz y los efectos se congelan con la imagen, pero el ambiente sigue.
- **Mezclador** (sección *Sonido* del Modo Acción): volumen de cada grupo (voces, efectos y ambiente) y, debajo, de cada una de sus pistas, con **M** y **S**. Opción para **bajar el ambiente mientras hay voz** (de las pistas que no estén silenciadas), que se calcula según dónde están los clips de voz, así que suena igual en directo que en el vídeo.
- **Efectos de audio**, en una pista (afectan a todo lo que suena en ella) o en un clip concreto (solo a ese sonido; los de la pista se aplican después). Cada uno tiene ajustes predefinidos y mandos para afinar (doble clic en un mando lo devuelve a su valor), se enciende y apaga, y se reordena con ▲▼:

  | Efecto | Ajustes predefinidos | Mandos |
  |---|---|---|
  | ⛪ Reverb | Habitación, sala, catedral, cueva, baño | Tamaño, amortiguación, mezcla, anchura, retardo inicial |
  | 🔁 Eco | Corto, eco, montaña, ping-pong | Tiempo, repeticiones, brillo, mezcla, ping-pong |
  | 🎚 Ecualizador | Más cuerpo, más claridad, sin graves | Graves, medios y agudos (±12 dB) |
  | 📞 Filtro | Teléfono, radio, megáfono, bajo el agua, lejos, tras la pared | Quita graves, quita agudos, saturación |
  | 🗜 Compresor | Voz, suave, fuerte | Umbral, proporción, ataque, liberación, ganancia |
  | 🎵 Tono | Monstruo, grave, agudo, ardilla | Semitonos (±12, sin cambiar la velocidad), mezcla |
  | 🤖 Robot | Robot, alien, metálico | Frecuencia, metal, mezcla |
  | ⚡ Distorsión | Suave, fuerte, radio rota | Intensidad, brillo, mezcla |

  Las colas (la reverb de una cueva, las repeticiones de un eco) siguen sonando al pasar a la imagen siguiente, igual en directo que en el vídeo.
- **Velocidad**: a 0,5×–2× la voz y los efectos aceleran o frenan cambiando el tono (como una cinta); el ambiente no cambia.
- **Silenciar**: botón 🔊 de la barra de reproducción o `M`. En los ajustes están el **volumen general** y el **micrófono**.
- **En el vídeo**: casilla *Incluir el sonido* al exportar.
  - Se mezcla con el mismo mezclador que suena en directo, a 48 kHz.
  - Se añade al MP4 en AAC (192 kbps) con una segunda pasada rápida de ffmpeg que copia el vídeo sin volver a codificarlo.
- **Paquetes `.imgxpack`**: además de clips y plantillas, llevan tus sonidos. Al importarlos se añaden a tu biblioteca y se comprueba que cada archivo es lo que dice ser.
- **Cómo funciona**: el sonido lo produce un motor propio en Rust (`crates/imgx-audio`), no el navegador interno.
  - Decodifica con `symphonia`, en Rust puro y sin GStreamer.
  - Mezcla con interpolación cúbica y sale por `cpal` (ALSA/PipeWire/Pulse en Linux, CoreAudio en macOS).
  - Los efectos (`crates/imgx-audio/src/fx`) son propios y deterministas: Freeverb, eco con realimentación filtrada, biquads, compresor, desplazador de tono granular, modulador en anillo y saturación. La misma mesa de mezclas (pistas → grupos → general) sirve para oír y para exportar, y una prueba comprueba que el directo y la exportación dan las mismas muestras.
  - La interfaz le manda qué suena en cada imagen y su reloj. Él lleva el suyo y lo corrige si se separa más de 50 ms, sin mandar una orden en cada fotograma.

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
| `E` | Editor de línea de tiempo |
| `Ctrl+E` | Exportar vídeo |
| `Ctrl+Z` / `Ctrl+Mayús+Z` · `Ctrl+Y` | Deshacer / rehacer |
| `P` | Reproducir desde la imagen actual / parar |
| `D` | Panel de diagnóstico |
| `F11` | Pantalla completa |
| `Ctrl+O` / `Ctrl+Mayús+O` | Abrir archivo / abrir carpeta |
| `Ctrl+R` | Recientes |
| `Ctrl+W` | Cerrar y volver al inicio |
| `M` | Silenciar o activar el sonido |
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

Con el editor abierto (tienen prioridad sobre las del visor):

| Tecla | Acción |
|---|---|
| `Espacio` | Reproducir / pausar la vista previa |
| `K` | Guardar la vista actual como clave en el cabezal |
| `G` | Grabar el movimiento / terminar |
| `Supr` | Borrar el clip o la clave seleccionados |
| `,` / `.` | Clave anterior / siguiente |
| `Esc` | Descartar la grabación, parar o deseleccionar |

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
- [x] **Fase 3**: editor de línea de tiempo.
  - [x] Capturar la vista como clave y grabar movimientos.
  - [x] Clips con nombre: mover, estirar y recortar, con ajuste magnético.
  - [x] Curva (easing) por tramo, con llegada y salida por clave, y pose exacta en el inspector.
  - [x] Marco de reproducción para encuadrar exactamente lo que se verá.
  - [x] Deshacer y rehacer.
  - [x] Biblioteca de clips y plantillas propias; exportar e importar `.imgxpack`.
  - [x] Corrección manual de viñetas.
- [x] **Fase 4**: vídeo e instaladores.
  - [x] Exportar a MP4 con ffmpeg: imagen, todo o un rango; tamaño, forma, fps y calidad; progreso y cancelar.
  - [x] Marco de vídeo en el editor.
  - [x] Paquetes de Linux: AppImage, `.deb`, `.rpm` y PKGBUILD para AUR; asociación con cbz/cbr/cb7 e imágenes.
  - [x] Instaladores que comprueban el sistema e instalan lo que falta: Linux (binario compatible desde Ubuntu 24.04) y macOS (compila en el propio Mac).
  - Probado en un Mac mini M1: instala, abre y exporta vídeo.
  - Opcional, solo para publicar: `.dmg` firmado y notarizado (necesita cuenta de Apple Developer) y Flatpak.
- [x] **Fase 5**: sonido.
  - [x] Motor de audio propio (decodificar, mezclar, salida y micrófono) y biblioteca de sonidos con efectos de serie.
  - [x] Pistas de voz y efectos en el editor, con forma de onda, recortes, volumen y fundidos; la imagen dura lo que su sonido.
  - [x] Ambientes y música entre imágenes, mezcla por pista y atenuación con voz.
  - [x] Sonido en el vídeo exportado (AAC).
  - [x] Grabar voz con el micrófono.
  - [x] Sonidos dentro de los paquetes `.imgxpack`.
  - [x] Varias pistas por grupo (las mismas en todo el proyecto), mezclador con volumen, panorama, silencio y solo.
  - [x] Efectos de audio en pistas y clips: reverb, eco, ecualizador, filtros, compresor, tono, robot y distorsión.

El formato de animaciones está especificado en [docs/format.md](docs/format.md).

## Instalar

Los paquetes para repartir se crean con `pnpm dist` y quedan en `dist/` (detalles en [docs/packaging.md](docs/packaging.md)). Los dos instaladores primero comprueban el sistema, enseñan qué van a hacer y no cambian nada hasta que respondes que sí.

**Linux** (`imgxplorer-<versión>-linux-x86_64.tar.gz`): se descomprime y se ejecuta `./install.sh`.
- Comprueba el procesador, la distribución y su versión, la glibc y que el gestor de paquetes tenga las bibliotecas.
- Instala lo que falte con pacman, apt, dnf o zypper (WebKitGTK, libheif con sus complementos, ffmpeg).
- Arranca la app en modo comprobación antes de copiarla y avisa si no abrirá HEIC.
- Copia la app en `~/.local` (o en `/usr/local` con `--system`) y la añade al menú.
- Funciona en distribuciones de 2024 en adelante: Ubuntu 24.04, Mint 22, Debian 13, Fedora 40, openSUSE Tumbleweed, Arch y derivadas.
- `--check` solo comprueba, y `--uninstall` desinstala.

**macOS** (`imgxplorer-<versión>-mac.tar.gz`): en Terminal, `bash "Instalar imgxplorer.command"`.
- Comprueba la versión de macOS (11 o posterior), el procesador (también si la Terminal va con Rosetta), el espacio libre y la conexión.
- Instala lo que falte (herramientas de Xcode, Homebrew, Rust, Node.js, pnpm, wasm-pack, libheif, ffmpeg).
- Compila la app en el propio Mac, la firma localmente y la deja en Aplicaciones. Así no hace falta cuenta de Apple Developer y Gatekeeper no la bloquea.
- La primera vez tarda entre 20 y 40 minutos.

`imgxplorer --self-check` dice si la app encuentra sus bibliotecas, la versión de libheif, si abre HEIC/AVIF y la versión de WebKitGTK. `imgxplorer --version` muestra la versión.

## Desarrollo

**Requisitos**
- Rust 1.88 o superior, con el target `wasm32-unknown-unknown`.
- `wasm-pack`.
- Node 22 o superior y `pnpm`.
- `libheif`.
- En Linux, WebKitGTK 4.1 y ALSA (`alsa-lib` o `libasound2-dev`, para el sonido).

En Arch/CachyOS:

```bash
sudo pacman -S rust rust-wasm wasm-pack libheif webkit2gtk-4.1 alsa-lib pnpm
```

**Arrancar**

```bash
pnpm install
pnpm wasm          # compila imgx-core a WASM para la interfaz
pnpm tauri dev
```

**Instaladores** (ver [docs/packaging.md](docs/packaging.md))

```bash
pnpm dist          # instaladores para repartir (Linux y macOS) en dist/; el de Linux necesita Docker
pnpm tauri build --bundles appimage,deb,rpm   # Linux: quedan en target/release/bundle/
cd packaging/aur && IMGX_REPO="file://$(realpath ../..)" makepkg -si   # Arch: instalar desde el código
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
crates/imgx-core    Cámara, easing, esquema .imgx, timeline, plantillas, orden de viñetas, transiciones y simplificación de grabaciones (Rust puro, nativo + WASM)
crates/imgx-media   Fuentes (carpeta, zip, 7z, rar), decodificadores, RAW, miniaturas y detección de viñetas
crates/imgx-audio   Sonido: decodificación (symphonia), mezclador por pistas (en directo y para exportar), efectos de audio (fx), sonidos de serie por síntesis, salida y micrófono (cpal)
crates/imgx-wasm    Enlace de imgx-core para la interfaz
src-tauri           App Tauri 2: protocolo imgx://, gestos, diagnóstico, archivos .imgx, ajustes, recientes, biblioteca, paquetes .imgxpack, listado de carpetas del selector propio, exportación a vídeo con ffmpeg y biblioteca de sonidos
ui                  Interfaz Svelte 5 + TypeScript: visor WebGL2, miniaturas, ajustes y atajos, Modo Acción (ui/src/lib/action), editor (ui/src/lib/editor), selector de archivos (ui/src/lib/files), exportar vídeo (ui/src/lib/export), sonido (ui/src/lib/audio)
docs                Especificación del formato .imgx y cómo crear los instaladores
packaging           Instaladores de Linux y macOS, dist.sh (crea los paquetes), PKGBUILD de AUR y lanzador .desktop
```
