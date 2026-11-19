# imgXplorer — Plan del primer lanzamiento (1.0)

## Contexto
imgxplorer ya hace mucho:
- visor rápido de carpetas y comprimidos;
- Modo Acción viñeta a viñeta;
- editor de línea de tiempo;
- sonido y efectos especiales;
- control por IA;
- vídeo MP4.

Antes del primer lanzamiento quieres:
- que lo básico de un visor de cómics esté completo y vaya rápido;
- unas pocas herramientas que **sorprendan**;
- **sin recargar** la interfaz.

Al final, un **documento de Word** ameno que cuente la historia del proyecto, el nombre y el porqué de cada tecnología, para gente técnica y no técnica.

Este plan reúne todas las ideas que comentamos. Las ordena en fases que se pueden probar una a una («tú programas, yo pruebo»). Cada fase termina contigo probándola y, si quieres, con un commit.

## Principios de diseño (para todo el plan)
0. **No se elimina ninguna función.** Todo el plan suma y mejora: cada botón, atajo, panel, menú y comportamiento que existe hoy sigue funcionando igual.
   - Los atajos de siempre no cambian: `0`/`F` ajustar, `1` tamaño real, `T`, `A`, `E`, `P`, `D`…
   - Si algo se amplía (por ejemplo, el botón «Ajustar» gana un menú o los recientes ganan portadas), lo anterior sigue a un clic, igual que ahora.
   - Lo nuevo que oculta algo, como la pantalla completa limpia, se puede desactivar en Ajustes.
1. **No recargar.** La cabecera no gana más de **dos botones**: «🎨» (ajustes de imagen) y «📖» (modo de lectura). Lo demás va en menús que ya existen, en atajos o aparece solo cuando hace falta.
2. **Todo se descubre:**
   - cada botón tiene su atajo en el tooltip;
   - la tecla `?` enseña todos los atajos;
   - el primer uso guiado enseña lo esencial.
3. **Nada destruye tus archivos.** Los ajustes, recortes y modos de lectura se guardan aparte, en el proyecto `.imgx` o en los ajustes. El original no se toca nunca; lo único que borra es «mover a la papelera», que es explícito.
4. **Rápido siempre:**
   - los efectos se calculan en la GPU, en el mismo shader;
   - los cálculos pesados (niveles, márgenes, portadas) se hacen en Rust, una sola vez, y se guardan en caché.
5. **Igual en todas partes.** Lo que se ve en el visor sale igual en el editor, en las capturas de la IA y en el MP4, usando el mismo camino `GlViewer.draw` que ya usan los efectos.
6. **Todo en español e inglés**, con atajos configurables, README al día y herramientas MCP cuando tenga sentido para la IA.
7. **Atajos nuevos sin choques.** Los propuestos (`J`, `W`, `H`, `V`, `2`, `Z`, `B`, `I`, `Ctrl+G`, `Ctrl+C`, `Ctrl+S`, `\`, `?`) se comprueban contra los de `ui/src/lib/keys.ts`. Los del editor tienen prioridad cuando está abierto, como ahora.

---

## Fase 0 — Guardar el plan ✅
- Copiar este plan a `docs/plan-lanzamiento.md`.
- Actualizar el README:
  - en «Propuestas de mejora», enlazar el plan;
  - en la hoja de ruta, añadir la «Fase 8: lanzamiento 1.0» con sus puntos sin marcar.

## Fase 1 — Ajustes rápidos de imagen 🎨 ✅
**Objetivo:** mejorar lo que ves con uno o dos clics.

**Interfaz.** Botón «🎨» en la cabecera (atajo propuesto `J`) que abre un **panel flotante pequeño**, no una barra fija.
- **Fila de un clic:**
  - **Original**;
  - **✨ Limpiar escaneo**;
  - **Blanco y negro**;
  - **Sepia**;
  - **🌙 Noche** (invertido suave, con el papel gris oscuro en vez de negro puro).
- **«Más ajustes ▾»** (plegado): brillo, contraste, saturación, exposición/gamma, temperatura y nitidez. Doble clic en un deslizador lo devuelve a su valor.
- **A qué se aplica:** «Esta página · Todo el cómic».
- **«Restablecer».**
- **Comparar:** mantener pulsada `\` enseña el original mientras la tengas pulsada.
- **Indicador:** un punto de color en el botón 🎨 cuando hay ajustes activos, para que no se olvide que la imagen está retocada.

**✨ Limpiar escaneo (la sorpresa).** Rust analiza el histograma de la página (función nueva en `imgx-media`, junto a `panels.rs`) y calcula:
- el **punto blanco** (color del papel);
- el **punto negro** (tinta).

El shader estira los niveles para que el papel amarillento quede blanco y la tinta negra, sin quemar los colores. Se calcula una vez por página y se guarda en caché.

**Datos:**
- `Item.adjust` y `Defaults.adjust` (opcionales) en `crates/imgx-core/src/schema.rs`. Los proyectos antiguos se abren igual, como pasó con `fx`.
- El modo **Noche** es de lectura: va en Ajustes globales y no sale en el vídeo salvo que lo pidas.

**Dibujo:**
- Uniforms nuevos **por capa** en el shader de capas de `ui/src/lib/viewer/gl.ts` (`Layer` gana `adjust`). Así, en una transición, cada imagen lleva su ajuste.
- La nitidez es un *unsharp* de 4 muestras en el mismo shader.
- Se aprovechan las fórmulas que ya existen en `FX_FS` (gris, sepia, contraste y tinte que conserva la luminancia).

**Exportar:** casilla «Aplicar ajustes de imagen», activa por defecto.

**IA:**
- `set_adjust` y `get_adjust`;
- `summary` avisa de las páginas retocadas.

**Pruebas:**
- Rust: niveles en una página sintética amarillenta;
- esquema que conserva `adjust` al guardar y volver a abrir;
- captura de la IA y fotograma del MP4 iguales.

## Fase 2 — Lectura de cómics 📖 ✅
**Interfaz.** Botón nuevo «📖» con un menú de lectura, al lado de los de siempre. El botón «Ajustar» y el «1:1» se quedan como están; «Ajustar» encuadra según el modo elegido (por defecto, la ventana, como ahora).
- **Ajustar a:**
  - **ventana** (`0`/`F`, como ahora);
  - **ancho** (`W`);
  - **alto** (`H`);
  - **✂ contenido**: recorta los márgenes al encuadrar.
- **Páginas:** **una** o **doble** (`2`; la `D` ya abre el Diagnóstico), con la casilla «Portada sola».
- **Tira vertical (webtoon)** (`V`).
- **Sentido:** occidental o **manga** (derecha a izquierda). Cambia las flechas y el orden de la doble página.

Se recuerda **por cómic**. Al hacerlo, se guardó en un archivo propio, `reading.json` (hasta 400 cómics), en vez de en `Recent`: los recientes solo guardan 20 y los marcadores se perderían al salir de la lista. El menú también se abre con `Ctrl+B`.

**Detalles de cada modo:**
- **✂ Ajustar al contenido:** Rust detecta el recuadro con contenido, reutilizando `border_median` y las pasadas de `panels.rs`, y la cámara encuadra ese recuadro. No recorta el archivo y el Modo Acción no cambia.
- **Doble página:**
  - el visor compone dos capas, una al lado de la otra;
  - las páginas que ya son dobles (más anchas que altas) se ven solas, automáticamente;
  - el zoom, el desplazamiento y la lupa funcionan sobre el conjunto.
- **Tira vertical:**
  - páginas apiladas a lo ancho y con desplazamiento continuo; solo se dibujan las visibles (2–4 texturas);
  - el número de página sigue a la que está en el centro.
  - **Sorpresa:** si al abrir algo las páginas son muy altas (más de 2,5 veces su ancho), aparece un aviso discreto: «¿Leer como webtoon?».

**Utilidades de lectura:**
- **🔍 Lupa:** mantener pulsado el botón derecho, o `Z`, enseña un círculo ampliado ×2,5 que sigue al ratón. Es una segunda pasada en la GPU con máscara circular, sin coste fuera de uso.
- **Ir a página:** `Ctrl+G`, una cajita con el número; `Intro` salta a esa página.
- **Marcadores:**
  - `B` marca o desmarca la página;
  - salen como una marca en la tira de miniaturas y en la barra de progreso;
  - se guardan por cómic.
- **Barra de progreso** fina en la parte de abajo:
  - muestra «12 / 48» al pasar el ratón;
  - un clic salta a ese punto;
  - los marcadores se ven como muescas.

**Pruebas:** detección de contenido y webtoon en Rust; navegación manga y doble página con portada sola.

## Fase 3 — Estantería (pantalla de inicio) 📚 ✅
**Objetivo:** la mejora más vistosa. Es la **misma lista de recientes, ampliada con portadas**.
- Conserva todo lo de ahora: abrir, retomar la página, tachar lo que ya no existe y el menú 🕘 de la cabecera.
- Un interruptor «▦ Portadas · ☰ Lista» deja volver a la vista de lista de siempre, y se recuerda.
- **Rejilla de portadas** de lo que has abierto: título, barra de progreso (`index/count` ya existen en `Recent`) y «hace 2 días».
- **Tarjeta grande «Continuar leyendo»** con el último cómic.
- **Portadas:**
  - Rust genera una miniatura de la primera página, con `thumbnail_jpeg`, la primera vez que se abre algo;
  - se guarda en `~/.cache/…/covers/<huella>.jpg`;
  - para las entradas antiguas, se generan en segundo plano.
- **Si ya no existe:** la portada sale en gris con «No encontrado». Un clic derecho permite «Quitar de la lista» o «Abrir carpeta».
- **Buscador** (solo si hay más de 12) y orden por «recientes · nombre · progreso».
- Se mantienen la zona para soltar archivos y los botones Abrir.

## Fase 4 — Utilidades ℹ️ ✅
Al hacerlo: las utilidades se abren con un **clic corto del botón derecho** (mantenerlo sigue siendo la lupa), sin botón nuevo en la cabecera. Al guardar, en vez de teselas, el lado mayor se limita a 8192 px (lo que admiten la GPU y el lienzo de WebKit), y se añadió «Original» para guardar una copia exacta del archivo.

- **Información** (`I`): panel lateral pequeño con:
  - nombre, tamaño en píxeles, peso, formato y posición en el cómic;
  - **datos de la cámara (EXIF)**: modelo, fecha, objetivo y exposición, con `kamadak-exif`, que ya es dependencia;
  - comando nuevo `image_info` en `src-tauri`.
- **Copiar imagen** (`Ctrl+C`): al portapapeles, con los ajustes aplicados, mediante `tauri-plugin-clipboard-manager`, la vía fiable en WebKitGTK y en macOS.
- **Guardar imagen como…** (`Ctrl+S`):
  - PNG o JPEG, con o sin ajustes;
  - «la página entera» o «lo que se ve»;
  - se dibuja fuera de pantalla con el `render` de `ui/src/lib/mcp/frames.ts`, troceado en teselas si es enorme.
- **Recortar y guardar:** arrastrar con `Mayús` para elegir una zona, por ejemplo una viñeta que quieras compartir, y guardarla.
- **Abrir la carpeta que la contiene:** reutiliza `open_url`, ampliado a rutas locales con «mostrar en la carpeta» (`open -R` en Mac, `xdg-open` de la carpeta en Linux).
- **Mover a la papelera** (`Supr`, solo en carpetas):
  - con confirmación;
  - usa el crate `trash`, que manda a la papelera del sistema; no es un borrado definitivo, y se dice así en el aviso.

## Fase 5 — Modo Acción y vídeo 💥 ✅
Al hacerlo: los subtítulos son un tipo de efecto más (`caption`) en la fila *✨ Efectos*, en vez de una pista aparte, así que se mueven, se estiran, se deshacen y la IA los pone con `add_fx` como los demás (sin `add_text` aparte). La fuente incluida es Bangers (SIL OFL).

- **Onomatopeyas animadas («¡KABOOM!», «¡ZAS!»).** Es la gran sorpresa.
  - **Qué es:** un tipo de efecto nuevo, `text`, en `crates/imgx-core/src/fx.rs`, con texto, color, contorno, tamaño, giro y animación (aparecer de golpe, temblar, crecer o rebotar). Se coloca con «⌖ Elegir punto», como los demás.
  - **Cómo se dibuja:** el texto se pinta una vez en un canvas 2D a textura, y el núcleo calcula su movimiento por fotograma. Así sale igual en el visor, el editor, la IA y el MP4.
  - **Fuente:** una de estilo cómic con licencia libre (SIL OFL, por ejemplo Bangers o Komika), incluida en la app y añadida a `packaging/licenses-extra.json`.
  - **Situaciones:** las de serie ganan su onomatopeya opcional (Explosión → «¡BOOM!», Puñetazo → «¡POW!», Sorpresa → «!?»), con la casilla «Con onomatopeya» junto a «Con su sonido».
  - **IA:** `add_fx` admite `text`.
- **Subtítulos y textos:**
  - pista «💬 Textos» por imagen, con inicio, duración y estilo (subtítulo abajo, cartel o bocadillo de narración); usa el mismo motor de texto;
  - al exportar, opción de guardar también un `.srt`, para YouTube;
  - herramientas MCP `add_text`, `edit_text` y `remove_text`, para que la IA pueda narrar.
- **Más formatos de vídeo:**
  - **WebM** (VP9 + Opus);
  - **GIF** (en dos pasadas, con paleta, y aviso si va a pesar mucho);
  - ajuste rápido **«Para redes (vertical 9:16)»**, sobre la forma «vertical» que ya existe.

## Fase 6 — Comodidad y diseño ✨ ✅
Al hacerlo: la ayuda también se abre con `F1`, desde el menú del clic derecho y desde la pantalla de inicio, e incluye los gestos del ratón y del touchpad. En pantalla completa limpia las barras flotan sobre la imagen (así no se recoloca al aparecer), el puntero se esconde y `Esc` sale. La visita guiada señala los botones de la cabecera (o «⋯» si están plegados). Los botones de la cabecera ya no se encogen ni parten su texto en dos líneas: pasan a «⋯» en este orden: Exportar, Editor, Modo Acción, pantalla completa, miniaturas, rotar, 1:1, Ajustar, zoom y abrir. Los paneles flotantes comparten las variables `--float` y `--shadow`, y hay un foco visible común para el teclado.
- **Ayuda de atajos** (`?`): ventana con los atajos agrupados y un buscador. Se genera desde `keys.ts`, así siempre está al día, y respeta los atajos cambiados.
- **Pantalla completa limpia:**
  - la cabecera y las miniaturas se ocultan solas al cabo de 2 s sin mover el ratón;
  - aparecen al acercarse al borde;
  - un interruptor en Ajustes («Ocultar barras en pantalla completa») vuelve al comportamiento de ahora.
- **Primer uso guiado** (4 pasos, con «Saltar»):
  1. Abrir un cómic.
  2. Ajustes de imagen y lectura.
  3. Modo Acción.
  4. Editor, voces e IA.

  Se puede volver a ver desde el «Acerca de».
- **Avisos y menús coherentes:** revisar que todos los paneles nuevos usen los mismos estilos y variables CSS, y que todo se maneje con el teclado (foco visible, `Esc` cierra).
- **Cabecera adaptable:** con un tamaño de ventana normal se ven todos los botones como ahora. Solo si la ventana es tan estrecha que no caben (hoy se cortarían), los que no caben pasan a un menú «⋯», sin desaparecer.

## Fase 7 — Voces con IA (ElevenLabs) 🗣️ ✅
> **Hecha.** Cambios respecto a lo previsto:
> - El guion es **por imagen** (cada frase con su segundo de inicio o el plano de `plan_shots` más un desfase), no por viñeta.
> - El panel tiene una tercera pestaña, **Sonidos**, para pedir efectos y música a mano (no solo desde la IA).
> - Las tomas guardan solo el sonido de cada una; «↻ Otra toma» usa otra semilla.
> - Si una toma nueva es más larga, las voces generadas que van detrás se corren para no pisarse.
> - El efecto de los pensamientos es una sala pequeña (reverb), no un eco.
> - Sin llavero del sistema, la clave va a un archivo `0600` y se avisa en Ajustes.
> - Los precios del resumen del pedido son los de la API (dólares por carácter o por minuto). Los planes con suscripción cuentan caracteres, y así se enseñan.
> - Falta probarlo con ElevenLabs de verdad, juntos y con tu cuenta.

**Cuándo:** es la última fase de programación, antes de preparar el lanzamiento, como pediste.

**Objetivo:**
- **Tú, a mano:** desde una viñeta del editor creas el **reparto** (personajes con su descripción), escribes o revisas lo que dice cada uno, **compruebas los fragmentos antes de pedirlos** y la IA externa (ElevenLabs por defecto) genera las voces. Quedan en la línea de tiempo como clips normales, que se mueven, se recortan y llevan los efectos de audio de siempre.
- **O una IA (Claude, Codex, Gemini) por MCP**, que lo automatiza:
  - mira las viñetas e identifica a los personajes;
  - decide qué necesita voz (diálogos, narración, gritos de fondo);
  - prepara el guion, pide las voces (con tu aprobación) y las coloca;
  - si el momento lo pide, añade ambiente, efectos o música.

### 7.1 Conexión con ElevenLabs (segura y sin sorpresas en la factura)
- **Proveedor intercambiable.** En Rust, `src-tauri/src/voices/`, con una interfaz `VoiceProvider` (diseñar voz, listar voces, hablar, efecto de sonido, música) y **ElevenLabs** como primera implementación.
  - Más adelante caben otros sin tocar la interfaz; por ejemplo, **Piper**, gratuito y sin conexión, para ensayar.
  - Todas las llamadas de red las hace Rust; la interfaz no sale a internet y su CSP no cambia.
- **Clave de la API:**
  - la escribes tú en **Ajustes → Voces IA**;
  - se guarda en el **llavero del sistema** (crate `keyring`: Secret Service en Linux, Llavero en macOS);
  - nunca va en el proyecto `.imgx`, en los registros ni en lo que ve la IA;
  - botón «Probar conexión», que muestra los créditos que quedan.
- **Qué se usa de ElevenLabs** (comprobar contra su referencia de la API al programarlo; han cambiado nombres entre versiones):
  - **Voice Design:** crear una voz a partir de una descripción (acento, edad, tono, forma de hablar). Da varias muestras para elegir.
  - **Text to Speech:** con el modelo más expresivo (v3/v4), que admite **etiquetas de interpretación** entre corchetes, como `[gritando]`, `[susurrando]`, `[riendo]` o `[con voz temblorosa]`.
  - **Sound Effects** (`/v1/sound-generation`, hasta 30 s): multitudes, gritos de guerreros de fondo, ambientes.
  - **Music** (`/v1/music`): banda sonora para un momento emotivo.

  Todo lo generado se convierte a los formatos que ya maneja el motor de sonido.
- **Control del gasto:**
  - antes de enviar nada, el **resumen del pedido**: fragmentos, caracteres y créditos estimados;
  - **caché por huella**: el mismo texto, con la misma voz y los mismos ajustes, no se vuelve a pagar;
  - **límite** opcional por sesión, en Ajustes;
  - la primera vez, un aviso de que los textos y descripciones se envían a ElevenLabs.

### 7.2 Datos (en el proyecto `.imgx`)
- **`Project.cast`:** los personajes.
  - id, nombre, color, **descripción** (el «prompt» de la voz), idioma;
  - voz asignada: proveedor, `voiceId`, nombre;
  - ajustes de la voz (estabilidad, estilo, velocidad).
- **`Item.lines`:** el guion de cada imagen.
  - por frase: personaje, texto con etiquetas, tipo (**diálogo · pensamiento · narración · multitud/fondo**), viñeta (plano) y desfase;
  - estado (**borrador · generada · desactualizada**, cuando cambias el texto después de generarla);
  - las **tomas** generadas, para poder volver a una anterior.
- **El audio generado:**
  - es un **clip normal de la pista de voz**, que ya se mueve, se recorta y lleva fundidos y efectos (reverb, eco, robot, tono…);
  - lleva una marca `gen` que lo une a su frase;
  - el archivo va a la biblioteca de sonidos del proyecto, como una grabación de micrófono.
- **Compatibilidad:** todo es opcional y los proyectos antiguos se abren igual, como pasó con `fx`.

### 7.3 Interfaz en el editor (compacta, con el estilo actual)
- **Botón «🗣 Voces IA»** en la etiqueta de la pista de voz y en el inspector de la viñeta o la clave seleccionada. Abre el panel en la **columna del inspector** que ya existe: no hay ventanas nuevas ni se tapa la línea de tiempo.
- **Pestaña «Reparto»:**
  - tarjetas plegables por personaje: punto de color, nombre y descripción;
  - **«🎙 Crear voz»**: Voice Design da 3 muestras; las escuchas, eliges una y se guarda;
  - alternativa: **«Elegir de mis voces»**, de tu biblioteca de ElevenLabs;
  - **«Probar»**: una frase corta de prueba con esa voz.
- **Pestaña «Guion» (de la viñeta actual):**
  - una fila por frase: personaje (desplegable con su color), texto y tipo;
  - **chips de interpretación** que insertan etiquetas (`gritando`, `susurrando`, `riendo`, `llorando`, `enfadado`, `emocionado`…);
  - un contador de caracteres.
- **«Revisar y generar»** abre la lista exacta de fragmentos que se van a pedir: texto, voz, caracteres y si ya están en caché (gratis). Desde ahí puedes quitar o editar cualquiera; luego, **Generar**.
- **Al generar:**
  - progreso por frase;
  - cada clip se coloca solo en la pista de voz al llegar la cámara a su viñeta, uno detrás de otro y sin solaparse;
  - la imagen dura lo que su sonido, como ya pasa;
  - los clips llevan el color de su personaje y su nombre.
- **Después**, todo lo de siempre:
  - **arrastrarlos** por la línea de tiempo, recortarlos y aplicar **efectos de audio**; por ejemplo, el eco para los pensamientos se pone solo según el tipo;
  - **«↻ Otra toma»** regenera solo esa frase.

### 7.4 Automatización por IA (MCP) y sus instrucciones
- **Herramientas nuevas:**
  - `cast_get` / `cast_set` (personajes y descripciones);
  - `voice_design` (muestras de una voz nueva) y `voice_list` (voces de la cuenta);
  - `script_get` / `script_set` (guion de una imagen o de un plano de `plan_shots`);
  - `voices_estimate` (fragmentos, caracteres y créditos);
  - `voices_generate` (genera y coloca);
  - `sfx_generate` y `music_generate` (fondo y banda sonora).
- **Aprobación obligatoria:**
  - todo lo que gasta créditos (`voices_generate`, `voice_design`, `sfx_generate`, `music_generate`) **abre en la app el resumen del pedido** y espera a que pulses **Aceptar**;
  - la IA no puede gastar sin ti;
  - opcionalmente, Ajustes permite un «presupuesto aprobado por sesión».
- **Sección nueva «## Voices» en `src-tauri/src/mcp/guide.md`**, el conjunto de instrucciones para que la IA lo haga bien y gastando poco:
  1. **Leer antes de escribir.** Mirar cada viñeta (`view_panel`), transcribir los bocadillos en orden de lectura (manga u occidental) e identificar quién habla por la cola del bocadillo.
  2. **Reparto coherente.** Un personaje es siempre el mismo en todo el cómic.
     - Describirlo por su aspecto y su papel: edad, carácter, «guerrero veterano, voz grave y ronca».
     - Revisar `cast_get` antes de crear uno nuevo.
     - Crear la voz una sola vez.
  3. **Qué lleva voz y qué no:**
     - **bocadillo** → diálogo;
     - **bocadillo de pensamiento** → pensamiento (susurrado, con eco);
     - **cartela** → narración;
     - **onomatopeyas** → no son voz: son efectos (de sonido o los visuales de la fase 5);
     - **gritos de fondo** o un ejército → **multitud** con `sfx_generate`, a menos volumen y en la pista de efectos;
     - **momento emotivo** → banda sonora o ambiente: primero la biblioteca; si no hay nada adecuado, `music_generate`.
  4. **Interpretación** con etiquetas de emoción y signos de puntuación. Sin exagerar, y con el idioma del cómic.
  5. **Tiempos:**
     - cada frase empieza poco después de que llegue la cámara a su viñeta;
     - las frases no se pisan, salvo una interrupción intencionada;
     - hay que dejar tiempo de lectura;
     - comprobar con `summary` y `sound_report`.
  6. **Ahorro:**
     - estimar primero (`voices_estimate`);
     - generar por lotes de una página;
     - no regenerar lo que no cambia, porque la caché es gratis;
     - probar voces con frases cortas.
  7. **Revisar** con `render_frames` / `sound_report` y corregir antes de exportar.
- **Plantilla de petición** en `docs/mcp.md`, para que puedas decirle a tu IA, por ejemplo: «pon voces a todo el capítulo 1, con narración en las cartelas y música en la última página».

### 7.5 Pruebas
- **Sin gastar:** un **proveedor simulado** en Rust, que devuelve audio sintético con la duración que tocaría. Con él se prueba:
  - todo el flujo: reparto, guion, resumen, aprobación, colocación, mover y efectos;
  - la prueba de la IA en una copia aislada de la app, como con los efectos.
- **Pruebas en Rust:**
  - esquema que conserva `cast` y `lines`;
  - huella de la caché;
  - la clave nunca aparece en el proyecto ni en los registros.
- **Con ElevenLabs de verdad, juntos y con tu cuenta.** Tú pones la clave y apruebas cada pedido, y la usamos con frases cortas para gastar poco.

## Fase 8 — Preparar el lanzamiento 🚀
- **Pulido de rendimiento**, con estos objetivos medidos:
  - la primera página de un cómic, en menos de 1 s;
  - pasar página ya precargada, en menos de 50 ms;
  - memoria estable tras leer 300 páginas.
- **Prueba completa en los tres equipos:**
  - tu CachyOS;
  - el Ubuntu con Nvidia de Alex;
  - tu Mac mini M1.

  Con una lista de comprobación en `docs/` (abrir, leer, ajustes, Modo Acción, sonido, efectos, voces con IA, IA, exportar e instalar o desinstalar).
- **Versión 1.0.0** en `Cargo.toml`, `package.json` y `tauri.conf.json`.
- **`CHANGELOG.md`** con lo que trae la 1.0.
- **Paquetes nuevos** con `pnpm dist`, con las licencias de terceros regeneradas (`pnpm third-party`).
- **Material para GitHub:**
  - capturas;
  - un GIF corto del Modo Acción;
  - una infografía de «qué hace»;
  - una frase que acompañe al nombre, para que se recuerde, por ejemplo «imgXplorer — tu cómic, viñeta a viñeta» o «Lee, anima y cuenta tus cómics». La eliges tú; el nombre no cambia.

## Fase 9 — Documento de Word: «imgXplorer, la historia detrás» 📄
Con la skill de documentos Word (docx), en `docs/imgXplorer-la-historia.docx`. No es una entrevista: son capítulos cortos que responden a lo que preguntaría un entrevistador.

**Tono:**
- cercano y con humor ligero, como lo cuentas tú;
- párrafos cortos, analogías («Rust es como un corrector que no te deja salir de casa con los cordones desatados»);
- recuadros **«Para curiosos técnicos»** con los detalles, que se pueden saltar sin perder el hilo.

**Capítulos:**
1. **Portada:** nombre, la frase y tu nombre.
2. **imgXplorer en una frase**, y qué puedes hacer con él en 1 minuto.
3. **El nombre:** *img* (imagen) + *eXplorer* (explorar). La X mayúscula como sello del proyecto. Por qué te gusta aunque no sea un nombre «comercial».
4. **Cómo empezó**, contado como historia:
   1. Un visor rápido de carpetas y comprimidos.
   2. Tu pasión, leer cómics, y el cansancio de hacer zoom viñeta a viñeta.
   3. El paso automático.
   4. «Mejor si creo yo la animación»: la línea de tiempo.
   5. «Ya puestos… efectos y sonido» 😄.
   6. La inspiración de los vídeos de YouTube que narran cómics viñeta a viñeta, con voces, ambiente, efectos sonoros y visuales.
   7. El resultado: un visor que, si quieres, construye tu propio vídeo de un cómic, un manga o un manhwa.
5. **Qué hace hoy**, por bloques, con capturas y frases sencillas: visor, lectura, Modo Acción, editor, sonido, efectos especiales, **voces con IA (ElevenLabs)**, control por IA y vídeo.
6. **¿Por qué Rust y no C?**
   - El debate con Alex, contado con cariño y siendo justo con su punto de vista:
     - C es rapidísimo y está en todas partes.
     - Rust da esa misma velocidad, pero el compilador evita toda una familia de errores de memoria (los que cuelgan programas o abren agujeros de seguridad).
     - Rust trae Cargo y un ecosistema enorme de bibliotecas.
     - El mismo código del motor de cámara funciona dentro de la interfaz gracias a WebAssembly.
   - Y la razón de verdad: te apetecía explorar Rust.
7. **Cada tecnología y por qué.** Tabla amable con «qué es (analogía) · para qué se usa aquí · por qué esta y no otra»:
   - Tauri frente a Electron (app ligera, usa el navegador del sistema);
   - Svelte 5 y TypeScript;
   - Vite, WebGL 2 (la GPU dibuja todo) y WebAssembly;
   - los decodificadores de imagen y los comprimidos;
   - Symphonia y cpal (sonido), FFmpeg (vídeo) y MCP (IA).
8. **Cómo funciona por dentro:** un dibujo sencillo de las piezas (Rust, interfaz, GPU, FFmpeg, IA) y el camino de una página desde el archivo comprimido hasta la pantalla.
9. **Programar con una IA:** cómo se ha hecho con Claude Code («yo pruebo, la IA programa»), qué funciona bien y qué hace falta poner de tu parte (probar, decidir, tener criterio).
10. **Retos y anécdotas:**
    - el cierre de WebKit con Nvidia en Ubuntu, que se resolvió gracias a Alex;
    - los nombres de archivo que el Mac confunde por mayúsculas;
    - los comprimidos «sólidos»;
    - el límite de destellos por las personas fotosensibles.
11. **En números:** líneas de código, pruebas automáticas, 44+ herramientas para la IA, 452 bibliotecas de terceros, 12 efectos y 13 situaciones… Se calculan del proyecto al escribirlo.
12. **Lo que viene** (la sección «Después del lanzamiento»).
13. **Gracias:** Alex, Gerard y la comunidad de código abierto.
14. **Glosario** de bolsillo: WebGL, WASM, MCP, keyframe, códec…

**Formato:**
- estilos de título coherentes y un índice;
- capturas reales de la app y un diagrama hecho para el documento;
- unas 12–18 páginas;
- se revisa convirtiéndolo a PDF y mirando cada página antes de dártelo.

## Después del lanzamiento (1.x), no entra en la 1.0
- Versión para Windows, actualizaciones automáticas y compilación automática en GitHub (cuando exista el repositorio).
- Tema claro.
- Partículas (chispas, lluvia, nieve) y efectos en las transiciones.
- Instrucciones de estilo para la IA (ya en «Propuestas de mejora»).
- Idea para más adelante: que la IA lea los bocadillos (OCR) para subtitular o traducir.

## Archivos principales que se tocarán
- **Núcleo:**
  - `crates/imgx-core/src/schema.rs` (`adjust`, textos);
  - `crates/imgx-core/src/fx.rs` (tipo `text`);
  - un módulo nuevo `adjust.rs`.
- **Medios:** `crates/imgx-media/src/` (niveles, recuadro de contenido, portadas).
- **App Tauri:** `src-tauri/src/` (`image_info`, portadas, papelera, portapapeles, mostrar en carpeta) y `src-tauri/src/mcp/tools.json` + `guide.md`.
- **Visor:**
  - `ui/src/lib/viewer/gl.ts` (ajustes por capa, lupa);
  - `ui/src/lib/viewer/Viewer.svelte` (modos de lectura, doble página, tira vertical).
- **Interfaz:**
  - `ui/src/App.svelte` (cabecera: 🎨 y 📖, barra de progreso, pantalla completa);
  - `ui/src/lib/Recents.svelte` → estantería;
  - paneles nuevos en `ui/src/lib/view/`;
  - `ui/src/lib/keys.ts`, `prefs.svelte.ts` e `i18n.svelte.ts`.
- **Voces con IA:** `src-tauri/src/voices/` (proveedor ElevenLabs y simulado, llavero, caché) y un panel nuevo en `ui/src/lib/editor/voices/` (reparto, guion y resumen del pedido).
- **Exportación e IA:** `ui/src/lib/export/` (ajustes, WebM, GIF, SRT) y `ui/src/lib/mcp/tools.svelte.ts`.
- **Documentación:** `README.md`, `docs/format.md`, `docs/mcp.md` y `docs/plan-lanzamiento.md`.

## Verificación (en cada fase)
- **Comprobaciones automáticas:**
  - `cargo test --workspace`, clippy sin avisos, `pnpm check` y `pnpm build`;
  - `node packaging/licenses.mjs --check` (si cambian dependencias, `pnpm third-party`).
- **Prueba con la IA de prueba en una copia aislada** (como con los efectos). Comprobar que la captura de la IA y el fotograma del MP4 coinciden con lo que se ve.
- **Revisión visual** de cada panel nuevo en el navegador; luego tú en tu app.
- **Rendimiento:** medir pasar página y abrir cómic antes y después de cada fase.
- **Documento de Word:** convertir a PDF y revisar cada página (maquetación, imágenes y que no queden huecos).
