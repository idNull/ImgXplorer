# Formato `.imgx` (v1)

Archivo JSON que guarda las animaciones (Modo Acción) de una carpeta, comprimido o imagen.
Lo define `crates/imgx-core/src/schema.rs`; este documento es un resumen.

## Ubicación
| Origen | Archivo |
|---|---|
| Carpeta | `<carpeta>/.imgxplorer.imgx` |
| Comprimido / imagen | `<archivo>.imgx` (al lado) |
| Sin permiso de escritura | directorio de datos de la app, indexado por `source.fingerprint` |

## Coordenadas de cámara
`{ cx, cy, zoom, rot }`
- `cx`, `cy`: punto de la imagen (0..1) en el centro de la pantalla.
- `zoom`: 1 = imagen completa visible; 2 = el doble.
- `rot`: grados, sentido horario.

Son independientes de la resolución, así que un clip vale para cualquier imagen o tamaño de vídeo.

## Estructura
```jsonc
{
  "format": "imgx", "version": 1,
  "source": { "kind": "archive", "name": "MiComic.cbz", "fingerprint": "…" },
  "defaults": {
    "transition": { "type": "fade", "duration": 0.5 }, "holdBetween": 0.3,
    "template": { "id": "comic", "params": { "mode": "auto", "dir": "rtl" } }
  },
  "clips": [ { "id": "zoom-titulo", "name": "Zoom al título", "clip": { "type": "keyframes", "keys": [] } } ],
  "items": [ {
    "entry": "001.jpg", "size": [2480, 3508],
    "template": { "id": "comic-blocks", "params": { "rows": 3, "cols": 2, "dir": "rtl" } },
    "timeline": {
      "camera": [ { "name": "Zoom al título", "start": 0, "duration": 2, "clip": { "ref": "zoom-titulo" }, "trimIn": 0, "trimOut": 0 } ],
      "audio": {
        "voice": [ { "sound": "3f2a…", "name": "Narración", "start": 0.5, "trimIn": 0, "duration": 4.2, "volume": 1, "fadeIn": 0, "fadeOut": 0.3 } ],
        "sfx": [ { "sound": "builtin:knock", "start": 3, "trimIn": 0, "duration": 1.2, "volume": 0.8, "fadeIn": 0, "fadeOut": 0 } ]
      }
    },
    "transitionOut": { "type": "continuous-camera", "duration": 0.8 },
    "panels": [ { "x": 0.05, "y": 0.04, "w": 0.9, "h": 0.3 } ]
  } ],
  "ambient": [ { "id": "amb-…", "sound": "builtin:rain", "from": "003.jpg", "to": "007.jpg", "volume": 1, "fadeIn": 1.5, "fadeOut": 1.5 } ],
  "tracks": [
    { "id": "voice", "bus": "voice", "name": "Ana", "volume": 1, "pan": -0.2,
      "effects": [ { "id": "fx-…", "type": "reverb", "on": true, "preset": "hall", "params": { "size": 0.7, "mix": 0.3 } } ] },
    { "id": "trk-…", "bus": "voice", "name": "Luis", "volume": 0.9, "pan": 0.2, "mute": true },
    { "id": "sfx", "bus": "sfx", "volume": 1, "pan": 0 },
    { "id": "ambient", "bus": "ambient", "volume": 1, "pan": 0 }
  ]
}
```

### Clips
| `type` | Campos | Uso |
|---|---|---|
| `keyframes` | `keys: [{ t, pose, easing }]` | Poses capturadas; `easing` rige el tramo hacia la siguiente clave |
| `recorded` | `samples: [{ t, pose }]` | Movimiento libre grabado |
| `hold` | `length`, `pose?` | Espera; sin `pose` mantiene la anterior |

Easing: `linear`, `ease-in`, `ease-out`, `ease-in-out`, `bounce`, `elastic`, `step`,
`cubic-bezier {x1,y1,x2,y2}`.

### Timeline
- Cada instancia de clip tiene `start` y `duration` (s). La duración natural del clip, menos
  `trimIn`/`trimOut`, se estira hasta ocupar `duration`.
- `name` (opcional) es el nombre que muestra el editor.
- `clip` es `{ "inline": { …clip… } }` (lo que guarda el editor) o `{ "ref": "id" }` (un clip de `clips`).
- Si dos clips se solapan, manda el que empieza después mientras dure; al terminar, sigue el de
  debajo. En un hueco se mantiene la última pose del clip que terminó más tarde, y antes del
  primero, su pose inicial.
- Una espera sin `pose` mantiene la pose con la que se llega a ella.
- `audio.voice` y `audio.sfx`: sonidos de la imagen (ver *Sonido*). Una timeline puede tener
  sonido y ninguna cámara; entonces la imagen sigue usando su plantilla para la cámara.
- `fx`: efectos especiales visuales de la imagen (ver *Efectos especiales*). Igual que el sonido,
  pueden ir sin cámara.

### Efectos especiales
```json
"fx": [
  { "id": "fxa1-0", "type": "flash", "start": 7.7, "duration": 0.5, "preset": "white", "at": { "x": 0.3, "y": 0.6 } },
  { "id": "fxa1-2", "type": "shake", "start": 7.7, "duration": 1.2, "intensity": 1,
    "params": { "strength": 3.5, "freq": 20, "decay": 0.85, "rot": 1.2 } }
]
```
- `id`: único en la imagen. `type`: el tipo (abajo); uno desconocido se conserva pero no se dibuja.
- `start` y `duration`: en segundos de la imagen. La imagen dura al menos hasta el final de su último efecto.
- `intensity`: 0–3 (1). `preset`: solo informativo (el ajuste del que salió).
- `params`: los mandos que falten toman su valor de serie y los que se salgan del rango se limitan. Los porcentajes son de la altura del vídeo.
- `at`: punto de la imagen (0–1) donde ocurre (centro de la onda, de las líneas o del desenfoque); sigue a la imagen aunque se mueva la cámara. Sin él, el centro de la pantalla.
- El catálogo completo, con rangos y ajustes predefinidos, sale de `imgx-core` (`fx::CATALOG`):

  | `type` | Qué hace | `params` (valor de serie) |
  |---|---|---|
  | `shake` | Temblor de la pantalla | `strength` % (2), `freq` Hz (18), `decay` 0–1 (0.8), `rot` ° (0.6), `axis` both/x/y |
  | `zoom-punch` | Golpe de zoom que vuelve | `amount` % (15), `way` in/out, `attack` s (0.06), `settle` smooth/bounce |
  | `flash` | Destello de un color | `color` #rrggbb (#ffffff), `amount` 0–1 (0.9), `attack` s (0.02), `blink` Hz 0–3 (0) |
  | `shockwave` | Anillo que deforma la imagen | `strength` % (3), `width` % (8), `size` (1.2, por la diagonal) |
  | `chroma` | Aberración cromática y glitch | `amount` % (0.7), `glitch` 0–1 (0), `pulse` Hz (0), `fadeIn`, `fadeOut` |
  | `radial-blur` | Desenfoque hacia el punto | `amount` 0–1 (0.35), `fadeIn`, `fadeOut` |
  | `focus-lines` | Líneas de concentración (manga) | `density` (110), `thickness` 0.1–1 (0.5), `inner` % (38), `color` black/white, `flicker` Hz (12), `fadeIn`, `fadeOut` |
  | `speed-lines` | Líneas de velocidad (manga) | `angle` ° (0), `density` (60), `thickness` (0.4), `color`, `speed` (3), `fadeIn`, `fadeOut` |
  | `impact` | Fotograma de impacto | `mode` negative/bw/both, `blink` Hz 0–3 (0) |
  | `color` | Sepia, gris o tinte | `mode` sepia/gray/cold/warm/tint, `amount` 0–1 (0.8), `tint` #rrggbb, `contrast` % (0), `fadeIn`, `fadeOut` |
  | `vignette` | Bordes oscuros (o claros) | `amount` 0–1 (0.6), `size` 0.2–1 (0.6), `pulse` Hz (0), `color` black/red/white, `fadeIn`, `fadeOut` |
  | `wave` | Ondulación | `amp` % (0.6), `freq` (6), `speed` Hz (1), `fadeIn`, `fadeOut` |

- Destellos y negativos no parpadean más de 3 veces por segundo (fotosensibilidad).
- El cálculo es determinista (el ruido sale del `id` y del tiempo): la vista previa y el vídeo dan lo mismo.

### Sonido
- **Clip de voz o efecto** (`timeline.audio.voice[]`, `timeline.audio.sfx[]`):
  - `sound`: id del sonido en la biblioteca del usuario (los 20 primeros caracteres hexadecimales del SHA-256 del archivo), o `builtin:<id>` para los efectos de serie.
  - `start`: segundo de la imagen en que empieza. `trimIn`: segundos que se saltan del sonido. `duration`: cuánto suena.
  - `volume`: 1 es el original. `fadeIn` y `fadeOut`: segundos de fundido.
  - `name`: opcional.
  - `track`: opcional, id de la pista (si falta o no existe, la primera de su grupo).
  - `effects`: opcional, efectos del propio clip (se aplican antes que los de la pista).
  - La imagen dura al menos hasta el final de su último sonido.
- **Ambiente** (`ambient[]`, en el proyecto): suena en bucle desde el principio de la imagen `from` hasta el final de la imagen `to` (nombres de entrada), con su volumen y fundidos. Si `to` ya no existe, suena solo en `from`; si no existe `from`, no suena. También admite `track` y `effects`.
- **Pistas** (`tracks[]`, opcional): las mismas en todas las imágenes. `bus` es el grupo (`voice`, `sfx` o `ambient`), `volume` (1), `pan` (−1 izquierda … 1 derecha), `mute` (false) y `effects`. Si no hay ninguna de un grupo, existe una con el id del grupo (así se leen los proyectos de antes).
- **Efecto** (en `tracks[].effects`, `voice[]/sfx[].effects` y `ambient[].effects`): `{ id, type, on, preset?, params }`. `type` es `reverb`, `delay`, `eq`, `filter`, `compressor`, `pitch`, `robot` o `distortion`; `preset` es solo informativo; en `params` los mandos que falten toman su valor por defecto y los que se salgan del rango se limitan:

  | `type` | `params` (valor por defecto) |
  |---|---|
  | `reverb` | `size` 0–1 (0.5), `damping` 0–1 (0.5), `mix` 0–1 (0.3), `width` 0–1 (1), `predelay` ms (0) |
  | `delay` | `time` ms (350), `feedback` 0–0.95 (0.35), `tone` Hz (6000), `mix` 0–1 (0.35), `pingpong` 0/1 (0) |
  | `eq` | `low`, `mid`, `high` en dB (0) a 150 Hz, 1 kHz y 5 kHz |
  | `filter` | `low` Hz paso alto (20 = sin), `high` Hz paso bajo (20000 = sin), `drive` 0–1 (0) |
  | `compressor` | `threshold` dB (−18), `ratio` (4), `attack` ms (10), `release` ms (150), `makeup` dB (6) |
  | `pitch` | `semitones` −12–12 (−5), `mix` 0–1 (1) |
  | `robot` | `freq` Hz (50), `metal` 0–0.9 (0.4), `mix` 0–1 (1) |
  | `distortion` | `drive` 0–1 (0.5), `tone` Hz (6000), `mix` 0–1 (1) |

- **Mezcla** (`defaults.mix`, opcional): volumen de cada grupo, `{ "voice": 1, "sfx": 1, "ambient": 0.7, "duck": true }`. `duck` baja el ambiente al 35 % mientras hay voz (de pistas no silenciadas).
- Orden de la mezcla: clip → sus efectos → pista (efectos, volumen, panorama) → grupo (volumen, atenuación) → volumen general y limitador.
- Los sonidos no van dentro del `.imgx`: están en la biblioteca de sonidos del usuario y viajan en los paquetes `.imgxpack` (`sounds.json` y la carpeta `sounds/`).

### Plantillas
`defaults.template` se aplica a todas las imágenes sin animación propia; `items[].template`
la sustituye para una imagen. Una imagen con `timeline.camera` propia (hecha en el editor) no
usa la plantilla de todas. Se guardan la plantilla y sus parámetros, no la timeline: se
genera al reproducir, porque depende de la proporción de la pantalla o del vídeo.

| `id` | Parámetros (valor por defecto) |
|---|---|
| `comic` | `mode` auto\|grid (auto), `rows` (3), `cols` (2), `dir` rtl\|ltr (rtl), `hold` s por viñeta (2), `move` s de paso (0.8), `margin` (0.04), `maxZoom` (4), `overview` página entera al empezar y acabar (true) |
| `scroll` | `dir` down\|up\|right\|left (down), `speed` pantallas/s (0.25), `fill` ocupación transversal (1), `pauseStart` (1), `pauseEnd` (1) |
| `bounce` | `cycles` (2), `period` s (2), `zoom` (1.5), `lift` (0.12), `easing` ease-in-out\|linear\|bounce\|elastic |
| `ken-burns` | `duration` (6), `zoom` (1.3), `focus` center\|top\|bottom\|left\|right, `way` in\|out |
| `full` | `duration` (3) |

Los valores que faltan o están fuera de rango se sustituyen por el valor por defecto o se
limitan al rango, así que un archivo editado a mano nunca impide reproducir. El catálogo
completo, con rangos, está en `crates/imgx-core/src/templates.rs` (`CATALOG`).

En `comic` automático, las viñetas se detectan al reproducir; si no se distinguen al menos
dos, se usa la rejilla `rows` × `cols`. Si la imagen tiene `items[].panels` (dibujadas o
corregidas a mano, en coordenadas normalizadas), se usan esas siempre, sea cual sea el modo.

### Transiciones
`cut`, `fade`, `slide {dir}`, `continuous-camera {dir?}`. `dir` (`left|right|up|down`) es el
lado por el que llega la siguiente imagen. En `continuous-camera` es opcional: si falta, se
deduce de la plantilla de la imagen saliente (cómic D→I: `left`; scroll: su dirección; resto:
`right`). `items[].transitionOut` sustituye a `defaults.transition` para una imagen.

## Versionado
`version` sube con cada cambio incompatible y `Project::from_json` aplica las migraciones en
cadena. Un archivo con una versión mayor que la soportada se rechaza con un mensaje claro.

## Paquetes `.imgxpack`

Zip para compartir clips y plantillas propias:

- `manifest.json`: `{ "format": "imgxpack", "version": 1, "app": "0.1.0" }`. Un paquete de una
  versión más nueva no se importa.
- `library.json`: `{ "clips": [ { "id", "name", "clip" } ], "templates": [ { "id", "name", "template": { "id", "params" } } ] }`.
  Al importar se valida cada clip (tiempos crecientes, zoom positivo…) y se descarta lo que no
  es válido o ya existe.
