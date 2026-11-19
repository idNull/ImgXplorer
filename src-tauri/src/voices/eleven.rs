//! ElevenLabs (https://elevenlabs.io/docs/api-reference). Todas las llamadas salen de aquí, con
//! la clave en la cabecera `xi-api-key`; la interfaz nunca la ve ni sale a internet.
use std::collections::BTreeMap;
use std::time::Duration;

use base64::Engine as _;
use serde::Deserialize;
use serde_json::{Value, json};

use super::provider::*;

const API: &str = "https://api.elevenlabs.io";
/// Formato que admiten todos los planes.
const FORMAT: &str = "mp3_44100_128";
/// Lo más grande que se acepta de una respuesta (música larga).
const MAX_BYTES: u64 = 200 << 20;

pub struct Eleven {
    key: String,
    agent: ureq::Agent,
}

impl Eleven {
    pub fn new(key: String) -> Eleven {
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_global(Some(Duration::from_secs(300)))
            .http_status_as_error(false)
            .user_agent("imgxplorer")
            .build()
            .into();
        Eleven { key, agent }
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        let mut req = self.agent.get(format!("{API}{path}")).header("xi-api-key", &self.key);
        for (k, v) in query {
            req = req.query(*k, *v);
        }
        let res = req.call().map_err(net)?;
        json_of(res)
    }

    fn post(&self, path: &str, query: &[(&str, &str)], body: &Value) -> Result<ureq::http::Response<ureq::Body>> {
        let mut req = self.agent.post(format!("{API}{path}")).header("xi-api-key", &self.key);
        for (k, v) in query {
            req = req.query(*k, *v);
        }
        req.send_json(body).map_err(net)
    }

    fn post_audio(&self, path: &str, body: &Value) -> Result<Audio> {
        let mut res = self.post(path, &[("output_format", FORMAT)], body)?;
        if !res.status().is_success() {
            return Err(api_error(res));
        }
        let bytes = res.body_mut().with_config().limit(MAX_BYTES).read_to_vec().map_err(net)?;
        if bytes.is_empty() {
            return Err(VoiceError("ElevenLabs no ha devuelto sonido".into()));
        }
        Ok(Audio { bytes, ext: "mp3" })
    }
}

fn net(e: ureq::Error) -> VoiceError {
    VoiceError(format!("no se pudo conectar con ElevenLabs: {e}"))
}

fn json_of(mut res: ureq::http::Response<ureq::Body>) -> Result<Value> {
    if !res.status().is_success() {
        return Err(api_error(res));
    }
    res.body_mut().with_config().limit(MAX_BYTES).read_json::<Value>().map_err(net)
}

/// El mensaje de error de la API, en una línea.
fn api_error(mut res: ureq::http::Response<ureq::Body>) -> VoiceError {
    let status = res.status().as_u16();
    let body: Value = res.body_mut().read_json().unwrap_or(Value::Null);
    let detail = &body["detail"];
    let msg = match detail {
        Value::String(s) => s.clone(),
        Value::Object(o) => o.get("message").and_then(Value::as_str).unwrap_or_default().to_owned(),
        Value::Array(a) => a.iter().filter_map(|x| x["msg"].as_str()).collect::<Vec<_>>().join("; "),
        _ => String::new(),
    };
    let hint = match status {
        401 => "la clave no es válida o no tiene permiso para esto",
        402 => "no quedan créditos suficientes en la cuenta",
        429 => "demasiadas peticiones seguidas o cuota agotada; espera un poco",
        _ => "",
    };
    let text = [hint, msg.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(": ");
    VoiceError(format!("ElevenLabs ({status}){}", if text.is_empty() { String::new() } else { format!(": {text}") }))
}

/// Solo letras, números, `-` y `_` (los ids de ElevenLabs), para meterlos en la ruta.
fn safe_id(id: &str) -> Result<&str> {
    if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        Ok(id)
    } else {
        Err(VoiceError(format!("id de voz no válido: {id}")))
    }
}

#[derive(Deserialize)]
struct RawVoice {
    voice_id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    labels: Option<BTreeMap<String, String>>,
    #[serde(default)]
    preview_url: Option<String>,
}

impl From<RawVoice> for VoiceInfo {
    fn from(v: RawVoice) -> VoiceInfo {
        VoiceInfo {
            name: v.name.unwrap_or_else(|| v.voice_id.clone()),
            id: v.voice_id,
            category: v.category,
            description: v.description,
            labels: v.labels.unwrap_or_default(),
            preview_url: v.preview_url,
        }
    }
}

/// Ajustes de la voz con los nombres de la API.
pub fn voice_settings(s: &BTreeMap<String, f64>) -> Option<Value> {
    let mut m = serde_json::Map::new();
    for (ours, theirs, lo, hi) in [("stability", "stability", 0.0, 1.0), ("similarity", "similarity_boost", 0.0, 1.0), ("style", "style", 0.0, 1.0), ("speed", "speed", 0.7, 1.2)] {
        if let Some(v) = s.get(ours).filter(|v| v.is_finite()) {
            m.insert(theirs.into(), json!(v.clamp(lo, hi)));
        }
    }
    (!m.is_empty()).then_some(Value::Object(m))
}

impl VoiceProvider for Eleven {
    fn account(&self) -> Result<Account> {
        let v = self.get("/v1/user/subscription", &[])?;
        Ok(Account {
            tier: v["tier"].as_str().unwrap_or("?").to_owned(),
            used: v["character_count"].as_u64().unwrap_or(0),
            limit: v["character_limit"].as_u64().unwrap_or(0),
            reset: v["next_character_count_reset_unix"].as_i64(),
        })
    }

    fn voices(&self) -> Result<Vec<VoiceInfo>> {
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        for _ in 0..10 {
            let mut q = vec![("page_size", "100"), ("sort", "name"), ("sort_direction", "asc")];
            if let Some(t) = &token {
                q.push(("next_page_token", t));
            }
            let v = self.get("/v2/voices", &q)?;
            let list: Vec<RawVoice> = serde_json::from_value(v["voices"].clone()).map_err(|e| VoiceError(format!("respuesta inesperada de ElevenLabs: {e}")))?;
            out.extend(list.into_iter().map(VoiceInfo::from));
            token = v["next_page_token"].as_str().map(str::to_owned);
            if !v["has_more"].as_bool().unwrap_or(false) || token.is_none() {
                break;
            }
        }
        Ok(out)
    }

    fn design(&self, description: &str, text: Option<&str>) -> Result<Vec<Preview>> {
        let mut body = json!({ "voice_description": description, "model_id": "eleven_ttv_v3" });
        // La frase de muestra tiene que tener de 100 a 1000 caracteres; si no, la escribe ElevenLabs.
        match text.filter(|t| (100..=1000).contains(&t.chars().count())) {
            Some(t) => body["text"] = json!(t),
            None => body["auto_generate_text"] = json!(true),
        }
        let res = self.post("/v1/text-to-voice/design", &[("output_format", FORMAT)], &body)?;
        let v = json_of(res)?;
        let b64 = base64::engine::general_purpose::STANDARD;
        let previews = v["previews"].as_array().cloned().unwrap_or_default();
        let out: Vec<Preview> = previews
            .iter()
            .filter_map(|p| {
                Some(Preview {
                    generated_id: p["generated_voice_id"].as_str()?.to_owned(),
                    audio: Audio { bytes: b64.decode(p["audio_base_64"].as_str()?).ok()?, ext: "mp3" },
                    seconds: p["duration_secs"].as_f64().unwrap_or(0.0),
                })
            })
            .collect();
        if out.is_empty() {
            return Err(VoiceError("ElevenLabs no ha devuelto muestras de voz".into()));
        }
        Ok(out)
    }

    fn save_design(&self, name: &str, description: &str, generated_id: &str) -> Result<VoiceInfo> {
        let body = json!({ "voice_name": name, "voice_description": description, "generated_voice_id": generated_id });
        let v = json_of(self.post("/v1/text-to-voice", &[], &body)?)?;
        let raw: RawVoice = serde_json::from_value(v).map_err(|e| VoiceError(format!("respuesta inesperada de ElevenLabs: {e}")))?;
        Ok(raw.into())
    }

    fn speak(&self, r: &SpeakReq) -> Result<Audio> {
        let mut body = json!({ "text": r.text, "model_id": r.model });
        if let Some(l) = r.language.as_deref().filter(|l| !l.is_empty() && r.model != "eleven_multilingual_v2") {
            body["language_code"] = json!(l);
        }
        if let Some(s) = voice_settings(&r.settings) {
            body["voice_settings"] = s;
        }
        if let Some(seed) = r.seed {
            body["seed"] = json!(seed);
        }
        self.post_audio(&format!("/v1/text-to-speech/{}", safe_id(&r.voice)?), &body)
    }

    fn sfx(&self, text: &str, seconds: Option<f64>, looped: bool) -> Result<Audio> {
        let mut body = json!({ "text": text, "loop": looped, "model_id": "eleven_text_to_sound_v3" });
        if let Some(s) = seconds {
            body["duration_seconds"] = json!(s.clamp(0.5, 30.0));
        }
        self.post_audio("/v1/sound-generation", &body)
    }

    fn music(&self, prompt: &str, seconds: f64, instrumental: bool) -> Result<Audio> {
        let ms = (seconds.clamp(3.0, 600.0) * 1000.0).round() as u64;
        let body = json!({ "prompt": prompt, "music_length_ms": ms, "model_id": "music_v2_5", "force_instrumental": instrumental });
        self.post_audio("/v1/music", &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ajustes_con_nombres_de_la_api() {
        let mut s = BTreeMap::new();
        s.insert("similarity".to_owned(), 0.9);
        s.insert("speed".to_owned(), 3.0);
        let v = voice_settings(&s).unwrap();
        assert_eq!(v["similarity_boost"], json!(0.9));
        assert_eq!(v["speed"], json!(1.2));
        assert!(voice_settings(&BTreeMap::new()).is_none());
    }

    #[test]
    fn ids_seguros() {
        assert!(safe_id("21m00Tcm4TlvDq8ikWAM").is_ok());
        assert!(safe_id("../v1/user").is_err());
        assert!(safe_id("").is_err());
    }
}
