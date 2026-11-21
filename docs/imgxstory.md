# Formato `.imgxstory` (v1)

Una **historia** es un cómic animado completo en un solo archivo, para compartirlo:

- las páginas;
- la animación (cámara, transiciones, efectos, onomatopeyas y subtítulos);
- todos los sonidos y voces que usa.

La crea imgxplorer (**🎞 Exportar → 📦 Historia**) y se abre en cualquier imgxplorer como un cómic más. Es también el formato que leerá el futuro reproductor para móvil.

El código que la lee y la escribe está en el crate `crates/imgx-story` (sin dependencias de Tauri, para poder usarlo también en el móvil).

## Contenido

Es un zip con estas entradas:

| Entrada | Qué es | Compresión |
|---|---|---|
| `manifest.json` | Formato, versión, título, variante, modo de lectura… (ver abajo) | deflate |
| `project.imgx` | El proyecto de siempre, en [formato `.imgx`](format.md), sin cambios | deflate |
| `computed.json` | Datos ya calculados de cada página | deflate |
| `pages/…` | Las imágenes, con su nombre en el cómic (puede haber subcarpetas) | sin comprimir |
| `sounds.json` | Lista de los sonidos propios que usa | deflate |
| `sounds/…` | Sus archivos | sin comprimir |
| `cover.jpg` | Portada: la primera página, de 512 px de lado mayor | sin comprimir |

Las páginas y los sonidos ya vienen comprimidos (JPEG, MP3…), así que van tal cual. Así se puede leer una página o un sonido suelto sin descomprimir el resto, que es lo que necesita un móvil.

El orden de las páginas es el **orden natural** de sus nombres (`2.jpg` antes que `10.jpg`), igual que en cualquier comprimido.

### `manifest.json`

```json
{
  "format": "imgxstory",
  "version": 1,
  "app": "1.0.0",
  "variant": "desktop",
  "title": "Mi historia",
  "credits": "Voces: …",
  "created": 1795139000,
  "lang": "es",
  "pages": 48,
  "reading": { "fit": "window", "crop": false, "double": false, "cover": true, "strip": false, "rtl": false },
  "authoredAspect": 1.7777,
  "cover": "cover.jpg"
}
```

| Campo | Qué es |
|---|---|
| `format`, `version` | Siempre `"imgxstory"`. Una versión mayor que la que entiende el programa se rechaza con un mensaje claro. |
| `app` | Versión de imgxplorer que la creó (informativo). |
| `variant` | `desktop` (calidad original) o `mobile` (optimizada). Las dos son el mismo formato y se abren en cualquier sitio. |
| `title`, `credits` | Lo que escribe quien la comparte. Los créditos son opcionales y van vacíos por defecto. |
| `created` | Segundos desde 1970. |
| `lang` | Idioma de la interfaz con que se creó. |
| `pages` | Número de páginas. |
| `reading` | Modo de lectura con el que se pensó: ajuste, ✂ contenido, doble página (con la portada sola o no), tira vertical y sentido manga. imgxplorer lo usa **la primera vez** que se abre; después manda lo que elija quien la lee. |
| `authoredAspect` | Proporción (ancho / alto) del vídeo para la que se pensó la cámara. Un reproductor en otra pantalla (un móvil en vertical) puede usarla para enmarcar igual o para adaptarse. |
| `cover` | Nombre de la portada. |

### `project.imgx`

Es el proyecto de siempre, con tres diferencias que hace la exportación:

- solo tiene las imágenes del rango exportado, y los ambientes se recortan a ese rango;
- `source` lleva el título de la historia, sin huella;
- si una página cambió de nombre al convertirla (`003.heic` → `003.png`), el proyecto ya usa el nombre nuevo, y en la variante móvil el tamaño (`size`) es el de la página reducida.

Las coordenadas de cámara y de las viñetas son normalizadas (0..1), así que reducir una página no mueve nada.

### `computed.json`

```json
{ "pages": { "01.png": { "panels": [[0.05, 0.04, 0.9, 0.3]], "levels": { "black": [0.1, 0.1, 0.1], "white": [0.95, 0.93, 0.88] }, "content": [0.03, 0.02, 0.94, 0.96] } } }
```

- **`panels`:** viñetas detectadas automáticamente, como `[x, y, ancho, alto]` normalizados. Si está vacío, no se distinguen al menos dos viñetas. Falta en las páginas con viñetas dibujadas a mano, que ya están en el proyecto.
- **`levels`:** los puntos negro y blanco de «limpiar escaneo». Solo están si el proyecto usa ese ajuste.
- **`content`:** el recuadro con contenido, sin los márgenes del color del fondo, para «✂ contenido» (`[x, y, ancho, alto]` normalizados). `[0, 0, 1, 1]` si no hay margen que quitar. Es opcional: las historias que no lo traen se abren igual y se calcula al leerlas.

Así quien abre la historia no tiene que detectar nada, y el resultado es exactamente el que vio quien la hizo.

### `sounds.json` y `sounds/`

```json
[{ "id": "3f2a…", "name": "Narración 1", "file": "3f2a….mp3", "kind": "generated", "seconds": 4.2 }]
```

- `id` es la huella del contenido (los 20 primeros caracteres hexadecimales de su SHA-256), la misma que usa el proyecto en `"sound"`. Un sonido usado diez veces va una sola vez.
- `kind`: `imported`, `recorded` o `generated` (hecho con IA).
- `file` es solo un nombre, sin rutas.
- Los sonidos de serie (`builtin:rain`…) **no van dentro**: los fabrica el motor, igual que la fuente de las onomatopeyas va con la app.

## Las páginas

Una historia solo puede llevar páginas en **JPEG, PNG, WebP o GIF**, para que cualquier aparato las abra. AVIF se convierte también, porque muchos móviles de hace unos años no lo leen.

| Variante | Páginas en esos formatos | Otros formatos (AVIF, HEIC, PSD, TIFF, EXR, JPEG XL, SVG…) | RAW de cámara |
|---|---|---|---|
| `desktop` | Se copian byte a byte | A PNG, sin pérdida | A JPEG de calidad 95 (lo que se ve de un RAW ya es su vista previa JPEG) |
| `mobile` | Tal cual si no pasan de 2.560 px de lado mayor; si no, se reducen | Se reducen si hace falta y van a JPEG (calidad 88), o a PNG si tienen transparencia | A JPEG |

## Abrirla en imgxplorer

- **Se abre como un comprimido** (selector, arrastrar o doble clic): las páginas son las de `pages/`.
- **Se comprueba entera antes**: el manifiesto, el proyecto y que estén todos los sonidos.
- **El proyecto** se busca en este orden:
  1. tus cambios, guardados **al lado** (`<historia>.imgxstory.imgx`);
  2. la carpeta de datos de la app, si al lado no se puede escribir;
  3. el `project.imgx` de dentro.

  Al retocarla, los cambios van al lado, **y la historia nunca cambia**.
- **Los sonidos** suenan desde dentro del zip. No se copian a tu biblioteca, salvo que pulses «⤓ Guardar en mi biblioteca» en el editor.
- **Las viñetas y los niveles** de `computed.json` se usan directamente.
- **La huella** de una historia es distinta de la del cómic del que salió, aunque tenga las mismas páginas. Así sus cambios no se mezclan con los del cómic.

## Seguridad

Al leer una historia:

- cada JSON tiene un tamaño máximo (64 MB) y cada sonido otro (1 GB);
- los nombres de los sonidos no pueden tener rutas (`/`, `\`) ni empezar por punto;
- nada se extrae al disco: páginas y sonidos se leen a memoria;
- el proyecto se valida igual que un `.imgx`.

## Versionado

`version` sube con cada cambio incompatible. Un campo nuevo y opcional no cambia la versión. Quien lee ignora los campos que no conoce.
