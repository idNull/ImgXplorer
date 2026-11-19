# Lista de comprobación de la 1.0

Para probar la versión 1.0 en los tres equipos antes de publicarla:

- **CachyOS**: el equipo de desarrollo.
- **Ubuntu con Nvidia**: el de Alex.
- **Mac mini M1**.

Cada fila se marca con ✓ si va bien, con ✗ si falla (y se apunta qué pasa) o con — si no se aplica.

Antes de empezar, conviene tener a mano:

- Un cómic `.cbz` y uno `.cbr` o `.cb7`, una carpeta con fotos (alguna con datos de la cámara) y un webtoon o manhwa con páginas muy altas.
- Unos auriculares y un micrófono.
- Para las voces con IA, la clave de ElevenLabs. Se prueba con frases cortas para gastar poco.
- Para el control por IA, Claude Code, Codex o Gemini CLI instalado.

Si algo falla, el registro está en `~/.local/share/com.imgxplorer.app/logs/` (Linux) o en `~/Library/Logs/com.imgxplorer.app/` (Mac). En la app, `D` abre el panel de diagnóstico, y Ajustes → Acerca de copia los datos del sistema.

## Instalar

| Comprobación | CachyOS | Ubuntu Nvidia | Mac M1 |
|---|---|---|---|
| El instalador comprueba el sistema e instala lo que falta | ☐ | ☐ | ☐ |
| La app aparece en el menú o en Aplicaciones, con su icono | ☐ | ☐ | ☐ |
| Abrir un `.cbz` con doble clic en el gestor de archivos abre imgxplorer | ☐ | ☐ | ☐ |
| `imgxplorer <carpeta|cómic|imagen>` desde la terminal | ☐ | ☐ | ☐ |
| La primera vez sale la visita guiada; «Saltar» la cierra y no vuelve a salir | ☐ | ☐ | ☐ |

## Abrir y leer

| Comprobación | CachyOS | Ubuntu Nvidia | Mac M1 |
|---|---|---|---|
| Abrir carpeta, `.cbz`, `.cbr` y `.cb7` (también arrastrándolos a la ventana) | ☐ | ☐ | ☐ |
| La primera página sale en menos de 1 s | ☐ | ☐ | ☐ |
| Pasar página con flechas, `Espacio`, rueda y touchpad: sin esperas | ☐ | ☐ | ☐ |
| Zoom (rueda, `+`/`-`, pellizco), desplazar, rotar y doble clic | ☐ | ☐ | ☐ |
| Miniaturas (`T`) en sus tres tamaños | ☐ | ☐ | ☐ |
| Formatos: HEIC, JPEG XL, PSD, SVG, un RAW de cámara | ☐ | ☐ | ☐ |
| Una panorámica enorme (más de 16 000 px) se ve entera | ☐ | ☐ | ☐ |
| Retoma la página al volver a abrir un cómic | ☐ | ☐ | ☐ |
| Estantería con portadas, «Continuar leyendo», buscador e interruptor a lista | ☐ | ☐ | ☐ |
| Leer 300 páginas seguidas: la app no se vuelve lenta | ☐ | ☐ | ☐ |

## Ajustes de imagen y lectura

| Comprobación | CachyOS | Ubuntu Nvidia | Mac M1 |
|---|---|---|---|
| 🎨 (`J`): Limpiar escaneo, Blanco y negro, Sepia y los deslizadores | ☐ | ☐ | ☐ |
| Mantener `\` enseña el original; 🌙 Noche (`N`) | ☐ | ☐ | ☐ |
| 📖 Ajustar al ancho (`W`), al alto (`H`) y al contenido | ☐ | ☐ | ☐ |
| Doble página (`2`) con «Portada sola»; sentido manga | ☐ | ☐ | ☐ |
| Tira vertical (`V`) en un webtoon; aviso «¿Leer como webtoon?» | ☐ | ☐ | ☐ |
| Lupa (`Z` o mantener el botón derecho), Ir a página (`Ctrl+G`), marcadores (`B`) y barra de progreso | ☐ | ☐ | ☐ |

## Utilidades y comodidad

| Comprobación | CachyOS | Ubuntu Nvidia | Mac M1 |
|---|---|---|---|
| Información (`I`) con datos de la cámara de una foto | ☐ | ☐ | ☐ |
| Copiar (`Ctrl+C`) y pegar en otra app; Guardar como (`Ctrl+S`) en PNG y JPEG | ☐ | ☐ | ☐ |
| `Mayús` + arrastrar para recortar una viñeta y guardarla | ☐ | ☐ | ☐ |
| Mostrar en la carpeta; mover a la papelera (`Supr`) y recuperarla de la papelera | ☐ | ☐ | ☐ |
| Ayuda de atajos (`?`, `F1`) con buscador | ☐ | ☐ | ☐ |
| Pantalla completa (`F11`): las barras se esconden y vuelven al acercar el ratón; `Esc` sale | ☐ | ☐ | ☐ |
| Ventana estrecha: los botones pasan al menú «⋯» | ☐ | ☐ | ☐ |
| Ajustes: idioma inglés y español, fondo, atajos cambiados | ☐ | ☐ | ☐ |

## Modo Acción y editor

| Comprobación | CachyOS | Ubuntu Nvidia | Mac M1 |
|---|---|---|---|
| Plantilla «Cómic por viñetas» en todas las imágenes; reproducir (`P`) con pausa y velocidad | ☐ | ☐ | ☐ |
| Corregir viñetas a mano | ☐ | ☐ | ☐ |
| Editor (`E`): claves con `K`, grabar con `G`, mover y recortar clips | ☐ | ☐ | ☐ |
| `Ctrl+Z` / `Ctrl+Mayús+Z` en todo lo anterior | ☐ | ☐ | ☐ |
| Cerrar y volver a abrir: todo sigue (guardado automático en `.imgx`) | ☐ | ☐ | ☐ |
| `Supr` y `Retroceso` borran lo seleccionado (también tras tocar un deslizador) | ☐ | ☐ | ☐ |

## Sonido y efectos

| Comprobación | CachyOS | Ubuntu Nvidia | Mac M1 |
|---|---|---|---|
| Importar un mp3 y ponerlo en la pista de voz; se oye al reproducir | ☐ | ☐ | ☐ |
| Grabar voz con el micrófono (`V`); en el Mac pide permiso la primera vez | ☐ | ☐ | ☐ |
| Ambiente de varias imágenes; quitarlo solo de una; mezclador y efectos de audio | ☐ | ☐ | ☐ |
| Situación «Explosión» con su sonido y su onomatopeya | ☐ | ☐ | ☐ |
| Subtítulos y cartelas de narración | ☐ | ☐ | ☐ |

## Voces con IA (ElevenLabs)

| Comprobación | CachyOS | Ubuntu Nvidia | Mac M1 |
|---|---|---|---|
| Ajustes → Voces IA: guardar la clave (dice si va al llavero) y «Probar conexión» | ☐ | ☐ | ☐ |
| Con el proveedor simulado, todo el flujo sin gastar | ☐ | ☐ | ☐ |
| Crear un personaje y su voz con «Crear voz» (escuchar las 3 muestras y elegir) | ☐ | ☐ | ☐ |
| Guion de una página con una etiqueta; «Revisar y generar» enseña el pedido antes de gastar | ☐ | ☐ | ☐ |
| Las voces quedan en la pista de voz, una detrás de otra; «↻ Otra toma» | ☐ | ☐ | ☐ |
| Pedir lo mismo otra vez sale de la caché («ya generado») | ☐ | ☐ | ☐ |
| Un efecto y una música desde la pestaña «Sonidos» | ☐ | ☐ | ☐ |

## Control por IA

| Comprobación | CachyOS | Ubuntu Nvidia | Mac M1 |
|---|---|---|---|
| Ajustes → IA: «Conectar» con la IA instalada y «Probar» | ☐ | ☐ | ☐ |
| Pedirle que anime una página y ponga un ambiente: se ve en directo y se deshace | ☐ | ☐ | ☐ |
| Pedirle voces: enseña el pedido y espera a «Generar» | ☐ | ☐ | ☐ |
| «Detener la IA» corta la conexión | ☐ | ☐ | ☐ |

## Exportar

| Comprobación | CachyOS | Ubuntu Nvidia | Mac M1 |
|---|---|---|---|
| MP4 1080p con sonido: se ve y se oye igual que en la app | ☐ | ☐ | ☐ |
| WebM, GIF y «Para redes (9:16)» | ☐ | ☐ | ☐ |
| `.srt` junto al vídeo cuando hay subtítulos | ☐ | ☐ | ☐ |
| Cancelar a medias borra el archivo incompleto | ☐ | ☐ | ☐ |

## Desinstalar

| Comprobación | CachyOS | Ubuntu Nvidia | Mac M1 |
|---|---|---|---|
| Linux: `~/.local/share/imgxplorer/install.sh --uninstall` quita la app, el icono, la asociación de archivos y la conexión con las IAs | ☐ | ☐ | — |
| Mac: arrastrar la app a la papelera | — | — | ☐ |
| Los ajustes, proyectos y sonidos se quedan (se borran a mano si se quiere) | ☐ | ☐ | ☐ |
| Antes, Ajustes → Voces IA → «Borrar clave» quita la clave del llavero | ☐ | ☐ | ☐ |

## Notas

Lo que falle, con el equipo, los pasos para repetirlo y la parte del registro que corresponda:

-
