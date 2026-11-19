//! Lo que tiene que saber hacer un proveedor de voces con IA. ElevenLabs es el primero; el
//! simulado sirve para probar todo sin gastar. Caben otros (Piper, sin conexión) sin tocar esto.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Error de un proveedor, con un texto para enseñar tal cual.
#[derive(Debug)]
pub struct VoiceError(pub String);

impl std::fmt::Display for VoiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<VoiceError> for String {
    fn from(e: VoiceError) -> String {
        e.0
    }
}

pub type Result<T> = std::result::Result<T, VoiceError>;

/// Estado de la cuenta (caracteres del periodo).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub tier: String,
    pub used: u64,
    pub limit: u64,
    /// Cuándo se renuevan (segundos Unix).
    pub reset: Option<i64>,
}

/// Una voz de la cuenta.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInfo {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub preview_url: Option<String>,
}

/// Audio que devuelve el proveedor (mp3 o wav).
pub struct Audio {
    pub bytes: Vec<u8>,
    pub ext: &'static str,
}

/// Una muestra de Voice Design.
pub struct Preview {
    pub generated_id: String,
    pub audio: Audio,
    pub seconds: f64,
}

/// Lo necesario para decir una frase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakReq {
    pub voice: String,
    pub model: String,
    /// Texto con sus etiquetas de interpretación (`[susurrando] ...`).
    pub text: String,
    #[serde(default)]
    pub language: Option<String>,
    /// stability, similarity, style, speed (los que falten, los del proveedor).
    #[serde(default)]
    pub settings: BTreeMap<String, f64>,
    /// Otra toma: con otra semilla sale otra interpretación (y no se reutiliza la anterior).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
}

pub trait VoiceProvider: Send + Sync {
    fn account(&self) -> Result<Account>;
    fn voices(&self) -> Result<Vec<VoiceInfo>>;
    /// Varias muestras de una voz nueva a partir de una descripción.
    fn design(&self, description: &str, text: Option<&str>) -> Result<Vec<Preview>>;
    /// Guarda en la cuenta la muestra elegida.
    fn save_design(&self, name: &str, description: &str, generated_id: &str) -> Result<VoiceInfo>;
    fn speak(&self, r: &SpeakReq) -> Result<Audio>;
    /// Efecto o ambiente (multitudes, gritos de fondo…), de 0,5 a 30 s.
    fn sfx(&self, text: &str, seconds: Option<f64>, looped: bool) -> Result<Audio>;
    /// Música a partir de una descripción.
    fn music(&self, prompt: &str, seconds: f64, instrumental: bool) -> Result<Audio>;
}
