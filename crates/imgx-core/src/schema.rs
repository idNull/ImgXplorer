//! Formato de proyecto `.imgx` (JSON versionado). Ver `docs/format.md`.

use crate::camera::Camera;
use crate::easing::Easing;
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "imgx";
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub format: String,
    pub version: u32,
    pub source: Source,
    #[serde(default)]
    pub defaults: Defaults,
    /// Clips con nombre reutilizables dentro de este proyecto.
    #[serde(default)]
    pub clips: Vec<NamedClip>,
    #[serde(default)]
    pub items: Vec<Item>,
    /// Ambientes o música: cada uno suena de una imagen a otra, en bucle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ambient: Vec<AmbientClip>,
    /// Pistas de sonido (las mismas en todas las imágenes). Si no hay, una por grupo con el id
    /// del grupo (`voice`, `sfx`, `ambient`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<Track>,
}

/// Grupo de una pista de sonido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bus {
    Voice,
    Sfx,
    Ambient,
}

/// Pista de sonido: todo lo que suena en ella pasa por su volumen, panorama y efectos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub bus: Bus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "one")]
    pub volume: f64,
    /// −1 izquierda … 1 derecha.
    #[serde(default)]
    pub pan: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub mute: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Efecto de sonido (reverb, eco…) de una pista o de un clip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub id: String,
    /// `reverb`, `delay`, `eq`, `filter`, `compressor`, `pitch`, `robot` o `distortion`.
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default = "yes")]
    pub on: bool,
    /// Ajuste predefinido del que salió (solo informativo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// Mandos; los que falten toman su valor por defecto.
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, f64>,
}

/// Sonido de fondo que abarca varias imágenes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmbientClip {
    pub id: String,
    pub sound: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Primera y última imagen (por nombre de entrada) en que suena.
    pub from: String,
    pub to: String,
    #[serde(default = "one")]
    pub volume: f64,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
}

/// Volumen de cada grupo y si el ambiente baja cuando hay voz.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mix {
    #[serde(default = "one")]
    pub voice: f64,
    #[serde(default = "one")]
    pub sfx: f64,
    #[serde(default = "ambient_level")]
    pub ambient: f64,
    #[serde(default = "yes")]
    pub duck: bool,
}

fn ambient_level() -> f64 {
    0.7
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    Folder,
    Archive,
    Image,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub kind: SourceKind,
    pub name: String,
    /// Huella del contenido (nombres + tamaños) para reencontrar el origen si se renombra.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Defaults {
    pub transition: Transition,
    /// Pausa (s) al final de cada imagen antes de la transición.
    pub hold_between: f64,
    /// Plantilla para las imágenes sin animación propia ("aplicar a todas").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<TemplateRef>,
    /// Mezcla del sonido (si no está, la de fábrica).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix: Option<Mix>,
    /// Ajustes de imagen para todas (las imágenes con los suyos propios usan esos).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adjust: Option<crate::adjust::Adjust>,
}

impl Default for Defaults {
    fn default() -> Self {
        Self { transition: Transition { kind: TransitionKind::Fade, duration: 0.5 }, hold_between: 0.3, template: None, mix: None, adjust: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    #[serde(flatten)]
    pub kind: TransitionKind,
    pub duration: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TransitionKind {
    Cut,
    Fade,
    /// `dir`: lado por el que llega la siguiente imagen.
    Slide { dir: Direction },
    /// Las páginas se colocan contiguas y la cámara viaja de una a otra. Sin `dir`, se deduce
    /// de la plantilla (cómic D→I: izquierda; scroll: abajo; resto: derecha).
    ContinuousCamera {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dir: Option<Direction>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedClip {
    pub id: String,
    pub name: String,
    pub clip: Clip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    /// Ruta de la entrada dentro de la carpeta/comprimido.
    pub entry: String,
    pub size: [u32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<TemplateRef>,
    #[serde(default)]
    pub timeline: Timeline,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_out: Option<Transition>,
    /// Viñetas dibujadas o corregidas a mano (sustituyen a la detección automática).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panels: Option<Vec<Rect>>,
    /// Ajustes de imagen propios (brillo, limpiar escaneo…); si no hay, los de `defaults`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adjust: Option<crate::adjust::Adjust>,
}

/// Rectángulo en coordenadas normalizadas de imagen (0..1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const FULL: Rect = Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateRef {
    pub id: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    #[serde(default)]
    pub camera: Vec<ClipInstance>,
    /// Voces y efectos de sonido de la imagen.
    #[serde(default)]
    pub audio: AudioTracks,
    /// Efectos especiales visuales (temblor, destello…).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx: Vec<crate::fx::Fx>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AudioTracks {
    #[serde(default)]
    pub voice: Vec<AudioClip>,
    #[serde(default)]
    pub sfx: Vec<AudioClip>,
    /// Sin uso: el ambiente va en el proyecto (`ambient`), porque puede abarcar varias imágenes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ambient: Vec<serde_json::Value>,
}

/// Un sonido colocado en la timeline de una imagen (voz o efecto).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioClip {
    /// Id del sonido en la biblioteca (`builtin:<id>` para los de serie).
    pub sound: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Segundo de la imagen en que empieza.
    pub start: f64,
    /// Segundos que se saltan del principio del sonido.
    #[serde(default)]
    pub trim_in: f64,
    pub duration: f64,
    #[serde(default = "one")]
    pub volume: f64,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    /// Pista (si falta, la primera de su grupo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
}

fn one() -> f64 {
    1.0
}

impl Timeline {
    /// Final del último sonido (0 si no hay): la imagen dura al menos esto.
    pub fn audio_end(&self) -> f64 {
        self.audio.voice.iter().chain(&self.audio.sfx).map(|c| c.start + c.duration).fold(0.0, f64::max)
    }
}

/// Un clip colocado en la timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipInstance {
    /// Nombre visible en el editor (p. ej. el del clip de la biblioteca del que salió).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub start: f64,
    pub duration: f64,
    pub clip: ClipSource,
    /// Segundos recortados al principio/final de la duración natural del clip.
    #[serde(default)]
    pub trim_in: f64,
    #[serde(default)]
    pub trim_out: f64,
}

impl ClipInstance {
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClipSource {
    /// Referencia a `Project::clips[].id` o a la biblioteca del usuario.
    Ref(String),
    Inline(Clip),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Clip {
    /// Poses con easing por tramo (el easing de cada clave rige el tramo hacia la siguiente).
    Keyframes { keys: Vec<Keyframe> },
    /// Movimiento libre grabado a mano.
    Recorded { samples: Vec<Sample> },
    /// Espera. Sin `pose`, mantiene la pose anterior.
    Hold {
        length: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pose: Option<Camera>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub t: f64,
    pub pose: Camera,
    #[serde(default)]
    pub easing: Easing,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub t: f64,
    pub pose: Camera,
}

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no es un archivo imgx (format = {0:?})")]
    WrongFormat(Option<String>),
    #[error("versión {0} no soportada (máxima {VERSION}); actualiza imgxplorer")]
    TooNew(u32),
}

impl Project {
    pub fn new(source: Source) -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            source,
            defaults: Defaults::default(),
            clips: Vec::new(),
            items: Vec::new(),
            ambient: Vec::new(),
            tracks: Vec::new(),
        }
    }

    /// Lee un proyecto aplicando migraciones si la versión es antigua.
    pub fn from_json(text: &str) -> Result<Self, FormatError> {
        let mut value: serde_json::Value = serde_json::from_str(text)?;
        let format = value.get("format").and_then(|v| v.as_str()).map(str::to_owned);
        if format.as_deref() != Some(FORMAT) {
            return Err(FormatError::WrongFormat(format));
        }
        let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if version > VERSION {
            return Err(FormatError::TooNew(version));
        }
        migrate(&mut value, version);
        Ok(serde_json::from_value(value)?)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("el proyecto siempre es serializable")
    }

    pub fn find_clip(&self, id: &str) -> Option<&Clip> {
        self.clips.iter().find(|c| c.id == id).map(|c| &c.clip)
    }
}

/// Migraciones encadenadas `vN → vN+1`. Hoy solo existe la v1.
fn migrate(value: &mut serde_json::Value, from: u32) {
    let _ = from;
    value["version"] = VERSION.into();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        let mut p = Project::new(Source { kind: SourceKind::Archive, name: "MiComic.cbz".into(), fingerprint: None });
        p.items.push(Item {
            entry: "001.jpg".into(),
            size: [2480, 3508],
            template: Some(TemplateRef { id: "comic-blocks".into(), params: serde_json::json!({"rows": 3}) }),
            timeline: Timeline {
                camera: vec![ClipInstance {
                    name: Some("Espera".into()),
                    start: 0.0,
                    duration: 2.0,
                    clip: ClipSource::Inline(Clip::Hold { length: 2.0, pose: None }),
                    trim_in: 0.0,
                    trim_out: 0.0,
                }],
                audio: AudioTracks::default(),
                fx: crate::fx::expand(crate::fx::situation("explosion").unwrap(), "e", 1.0, Some(crate::fx::Point { x: 0.4, y: 0.6 })),
            },
            transition_out: Some(Transition { kind: TransitionKind::ContinuousCamera { dir: None }, duration: 0.8 }),
            panels: Some(vec![Rect { x: 0.1, y: 0.1, w: 0.4, h: 0.3 }]),
            adjust: Some(crate::adjust::Adjust { clean: 1.0, tone: crate::adjust::Tone::Gray, ..Default::default() }),
        });
        p.defaults.adjust = Some(crate::adjust::Adjust { saturation: -0.3, ..Default::default() });
        let back = Project::from_json(&p.to_json()).unwrap();
        assert_eq!(p, back);
        assert!(p.to_json().contains(r#""type": "continuous-camera""#));
        assert!(p.to_json().contains(r#""type": "shockwave""#), "los efectos se guardan");
        assert!(p.to_json().contains(r#""tone": "gray""#), "los ajustes de imagen se guardan");
    }

    #[test]
    fn acepta_lo_que_escribe_la_interfaz() {
        let json = r#"{"format":"imgx","version":1,
          "source":{"kind":"archive","name":"Tomo 1","fingerprint":"cbf29ce484222325"},
          "defaults":{"transition":{"type":"continuous-camera","duration":0.8},"holdBetween":0.3,
                      "template":{"id":"comic","params":{"mode":"auto","dir":"rtl","hold":2}}},
          "clips":[],
          "items":[{"entry":"p01.jpg","size":[1600,2400],"template":{"id":"scroll","params":{}}},
                   {"entry":"p02.jpg","size":[1600,2400],"transitionOut":{"type":"slide","dir":"left","duration":0.5}}]}"#;
        let p = Project::from_json(json).unwrap();
        assert_eq!(p.defaults.transition.kind, TransitionKind::ContinuousCamera { dir: None });
        assert_eq!(p.defaults.template.as_ref().unwrap().id, "comic");
        assert_eq!(p.items[1].transition_out.unwrap().kind, TransitionKind::Slide { dir: Direction::Left });
        assert_eq!(Project::from_json(&p.to_json()).unwrap(), p);
    }

    #[test]
    fn rechaza_otro_formato_y_version_futura() {
        assert!(matches!(Project::from_json(r#"{"format":"x","version":1}"#), Err(FormatError::WrongFormat(_))));
        assert!(matches!(
            Project::from_json(r#"{"format":"imgx","version":99,"source":{"kind":"image","name":"a"}}"#),
            Err(FormatError::TooNew(99))
        ));
    }

    #[test]
    fn sonido_de_la_imagen() {
        let json = r#"{"camera":[],"audio":{"voice":[{"sound":"abc","start":1,"duration":2.5,"fadeIn":0.2}],"sfx":[{"sound":"builtin:hit","start":4,"duration":0.6,"volume":0.5,"trimIn":0.1}]}}"#;
        let tl: Timeline = serde_json::from_str(json).unwrap();
        assert_eq!(tl.audio.voice[0].volume, 1.0, "volumen por defecto");
        assert_eq!(tl.audio.voice[0].fade_in, 0.2);
        assert_eq!(tl.audio.sfx[0].trim_in, 0.1);
        assert!((tl.audio_end() - 4.6).abs() < 1e-9);
        let back: Timeline = serde_json::from_str(&serde_json::to_string(&tl).unwrap()).unwrap();
        assert_eq!(back, tl);
        assert!(!serde_json::to_string(&tl).unwrap().contains("ambient"), "el hueco sin uso no se escribe");
        assert_eq!(Timeline::default().audio_end(), 0.0);
    }

    #[test]
    fn ambiente_y_mezcla_se_conservan() {
        let json = r#"{"format":"imgx","version":1,"source":{"kind":"archive","name":"a.cbz"},
            "defaults":{"transition":{"type":"fade","duration":0.5},"holdBetween":0.3,"mix":{"voice":1,"sfx":0.8,"ambient":0.5,"duck":false}},
            "ambient":[{"id":"x","sound":"builtin:rain","from":"01.jpg","to":"05.jpg","fadeIn":2}]}"#;
        let p = Project::from_json(json).unwrap();
        assert_eq!(p.ambient[0].volume, 1.0);
        assert_eq!(p.ambient[0].fade_in, 2.0);
        assert_eq!(p.defaults.mix.unwrap().sfx, 0.8);
        let back = Project::from_json(&p.to_json()).unwrap();
        assert_eq!(back, p);
        let empty = Project::from_json(r#"{"format":"imgx","version":1,"source":{"kind":"image","name":"a"}}"#).unwrap();
        assert!(!empty.to_json().contains("ambient"), "sin ambiente no se escribe nada");
    }

    #[test]
    fn pistas_y_efectos_se_conservan() {
        let json = r#"{"format":"imgx","version":1,"source":{"kind":"archive","name":"a.cbz"},
            "tracks":[{"id":"voice","bus":"voice","name":"Ana","pan":-0.3,"effects":[{"id":"e1","type":"reverb","preset":"sala","params":{"size":0.6,"mix":0.3}}]},
                      {"id":"t2","bus":"voice","volume":0.8,"mute":true}],
            "items":[{"entry":"01.jpg","size":[100,100],"timeline":{"camera":[],"audio":{"voice":[{"sound":"abc","start":0,"duration":1,"track":"t2","effects":[{"id":"e2","type":"pitch","on":false,"params":{"semitones":-5}}]}]}}}],
            "ambient":[{"id":"x","sound":"builtin:rain","from":"01.jpg","to":"01.jpg","track":"ambient","effects":[{"id":"e3","type":"eq","params":{}}]}]}"#;
        let p = Project::from_json(json).unwrap();
        assert_eq!(p.tracks.len(), 2);
        assert!(p.tracks[0].effects[0].on, "encendido por defecto");
        assert_eq!(p.tracks[1].volume, 0.8);
        let back = Project::from_json(&p.to_json()).unwrap();
        assert_eq!(back, p);
        let out = p.to_json();
        assert!(!out.contains("\"mute\":false"), "lo que está por defecto no se escribe");
    }
}
