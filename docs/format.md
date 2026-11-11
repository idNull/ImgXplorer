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
      "camera": [ { "start": 0, "duration": 2, "clip": { "ref": "zoom-titulo" }, "trimIn": 0, "trimOut": 0 } ],
      "audio": { "voice": [], "sfx": [], "ambient": [] }
    },
    "transitionOut": { "type": "continuous-camera", "duration": 0.8 }
  } ]
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
- Entre clips se mantiene la última pose. Si dos se solapan, manda el que empieza después.
- `audio` está reservado para la fase de sonido: se conserva al guardar, pero aún no se usa.

### Plantillas
`defaults.template` se aplica a todas las imágenes sin animación propia; `items[].template`
la sustituye para una imagen. Se guardan la plantilla y sus parámetros, no la timeline: se
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
dos, se usa la rejilla `rows` × `cols`.

### Transiciones
`cut`, `fade`, `slide {dir}`, `continuous-camera {dir?}`. `dir` (`left|right|up|down`) es el
lado por el que llega la siguiente imagen. En `continuous-camera` es opcional: si falta, se
deduce de la plantilla de la imagen saliente (cómic D→I: `left`; scroll: su dirección; resto:
`right`). `items[].transitionOut` sustituye a `defaults.transition` para una imagen.

## Versionado
`version` sube con cada cambio incompatible y `Project::from_json` aplica las migraciones en
cadena. Un archivo con una versión mayor que la soportada se rechaza con un mensaje claro.
