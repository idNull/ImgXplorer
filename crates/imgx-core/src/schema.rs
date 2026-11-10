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
}

impl Default for Defaults {
    fn default() -> Self {
        Self { transition: Transition { kind: TransitionKind::Fade, duration: 0.5 }, hold_between: 0.3 }
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
    Slide { dir: Direction },
    /// Las páginas se colocan contiguas y la cámara viaja de una a otra.
    ContinuousCamera,
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
    /// Reservado para la fase de sonido; se conserva al guardar pero aún no se usa.
    #[serde(default)]
    pub audio: AudioTracks,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AudioTracks {
    #[serde(default)]
    pub voice: Vec<serde_json::Value>,
    #[serde(default)]
    pub sfx: Vec<serde_json::Value>,
    #[serde(default)]
    pub ambient: Vec<serde_json::Value>,
}

/// Un clip colocado en la timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipInstance {
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
                    start: 0.0,
                    duration: 2.0,
                    clip: ClipSource::Inline(Clip::Hold { length: 2.0, pose: None }),
                    trim_in: 0.0,
                    trim_out: 0.0,
                }],
                audio: AudioTracks::default(),
            },
            transition_out: Some(Transition { kind: TransitionKind::ContinuousCamera, duration: 0.8 }),
        });
        let back = Project::from_json(&p.to_json()).unwrap();
        assert_eq!(p, back);
        assert!(p.to_json().contains(r#""type": "continuous-camera""#));
    }

    #[test]
    fn rechaza_otro_formato_y_version_futura() {
        assert!(matches!(Project::from_json(r#"{"format":"x","version":1}"#), Err(FormatError::WrongFormat(_))));
        assert!(matches!(
            Project::from_json(r#"{"format":"imgx","version":99,"source":{"kind":"image","name":"a"}}"#),
            Err(FormatError::TooNew(99))
        ));
    }
}
