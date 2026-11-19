//! Voces con IA: reparto, frases, efectos y música generados por un proveedor externo
//! (ElevenLabs) o por el simulado. Lo generado entra en la biblioteca de sonidos como cualquier
//! otro sonido, así que se mueve, se recorta y lleva efectos igual.
//!
//! - La clave se guarda en el llavero del sistema (Secret Service en Linux, Llavero en macOS). Si
//!   no hay llavero, en un archivo que solo puede leer el usuario. Nunca va al proyecto, a los
//!   registros ni a la interfaz.
//! - Caché por huella: la misma frase con la misma voz y los mismos ajustes no se vuelve a pagar.
//! - Límite por sesión de caracteres de voz (lo pone la interfaz desde los ajustes).
pub mod eleven;
pub mod mock;
pub mod provider;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::audio::{Audio, SoundInfo, SoundKind};
use provider::{Account, SpeakReq, VoiceInfo, VoiceProvider};

const SERVICE: &str = "com.imgxplorer.app";
const USER: &str = "elevenlabs";

/// Precios de la API (dólares por 1000 caracteres; elevenlabs.io/pricing/api, nov. 2026). Solo
/// para el resumen del pedido: los planes con suscripción cuentan caracteres.
pub fn usd_per_1k(model: &str) -> f64 {
    match model {
        m if m.contains("flash") || m.contains("turbo") || m.contains("conversational") => 0.04,
        _ => 0.08,
    }
}
/// Efectos y música: dólares por minuto.
const SFX_USD_MIN: f64 = 0.12;
const MUSIC_USD_MIN: f64 = 0.15;

#[derive(Default)]
pub struct Voices {
    mock: mock::Mock,
    /// Caracteres de voz enviados en esta sesión (sin contar lo que salió de la caché).
    spent: Mutex<u64>,
    /// Huella → sonido de la biblioteca.
    cache: Mutex<Option<BTreeMap<String, String>>>,
    /// Muestras de Voice Design, para escucharlas antes de elegir.
    previews: Mutex<HashMap<String, Arc<imgx_audio::Pcm>>>,
}

// ------------------------------------------------------------------ clave

fn key_file(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("voices-key"))
}

fn entry() -> Option<keyring::Entry> {
    keyring::Entry::new(SERVICE, USER).ok()
}

fn read_key(app: &AppHandle) -> Option<(String, &'static str)> {
    if let Some(k) = entry().and_then(|e| e.get_password().ok()).filter(|k| !k.is_empty()) {
        return Some((k, "keyring"));
    }
    let k = std::fs::read_to_string(key_file(app).ok()?).ok()?;
    let k = k.trim().to_owned();
    (!k.is_empty()).then_some((k, "file"))
}

fn write_key_file(app: &AppHandle, key: &str) -> Result<(), String> {
    let path = key_file(app)?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&path).map_err(|e| e.to_string())?;
        f.write_all(key.as_bytes()).map_err(|e| e.to_string())?;
        // Por si ya existía con otros permisos.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    std::fs::write(&path, key).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStatus {
    stored: bool,
    /// `keyring` (llavero del sistema) o `file` (archivo solo para el usuario).
    backend: Option<&'static str>,
}

#[tauri::command]
pub async fn voices_key_status(app: AppHandle) -> KeyStatus {
    tauri::async_runtime::spawn_blocking(move || {
        let k = read_key(&app);
        KeyStatus { stored: k.is_some(), backend: k.map(|(_, b)| b) }
    })
    .await
    .unwrap_or(KeyStatus { stored: false, backend: None })
}

/// Guarda la clave (en el llavero si se puede). Devuelve dónde ha quedado.
#[tauri::command]
pub async fn voices_key_set(app: AppHandle, key: String) -> Result<&'static str, String> {
    let key = key.trim().to_owned();
    if key.is_empty() || key.len() > 200 || key.chars().any(char::is_whitespace) {
        return Err("la clave no parece válida".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let stored = entry().map(|e| e.set_password(&key).is_ok() && e.get_password().ok().as_deref() == Some(key.as_str())).unwrap_or(false);
        if stored {
            if let Ok(p) = key_file(&app) {
                std::fs::remove_file(p).ok();
            }
            log::info!("voces: clave guardada en el llavero del sistema");
            return Ok("keyring");
        }
        log::info!("voces: sin llavero del sistema; la clave va a un archivo solo para el usuario");
        write_key_file(&app, &key)?;
        Ok("file")
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn voices_key_clear(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(e) = entry() {
            e.delete_credential().ok();
        }
        if let Ok(p) = key_file(&app) {
            std::fs::remove_file(p).ok();
        }
        log::info!("voces: clave borrada");
    })
    .await
    .map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ proveedor

/// Ejecuta `f` con el proveedor elegido (`elevenlabs` o `mock`), fuera del hilo principal.
async fn with_provider<R: Send + 'static>(app: AppHandle, provider: String, f: impl FnOnce(&AppHandle, &dyn VoiceProvider) -> Result<R, String> + Send + 'static) -> Result<R, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if provider == "mock" || std::env::var_os("IMGX_VOICES_MOCK").is_some() {
            let v = app.state::<Voices>();
            return f(&app, &v.mock);
        }
        let (key, _) = read_key(&app).ok_or("falta la clave de ElevenLabs: ponla en Ajustes → Voces IA")?;
        let p = eleven::Eleven::new(key);
        f(&app, &p)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    #[serde(flatten)]
    account: Account,
    /// Caracteres de voz enviados en esta sesión.
    session: u64,
}

#[tauri::command]
pub async fn voices_account(app: AppHandle, provider: String) -> Result<AccountInfo, String> {
    with_provider(app, provider, |app, p| {
        let account = p.account()?;
        Ok(AccountInfo { account, session: *app.state::<Voices>().spent.lock().unwrap() })
    })
    .await
}

#[tauri::command]
pub async fn voices_list(app: AppHandle, provider: String) -> Result<Vec<VoiceInfo>, String> {
    with_provider(app, provider, |_, p| Ok(p.voices()?)).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewInfo {
    generated_id: String,
    seconds: f64,
}

/// Muestras de una voz nueva (se escuchan con `voices_preview`).
#[tauri::command]
pub async fn voices_design(app: AppHandle, provider: String, description: String, text: Option<String>) -> Result<Vec<PreviewInfo>, String> {
    let description: String = description.trim().chars().take(1000).collect();
    if description.chars().count() < 20 {
        return Err("describe la voz con al menos 20 caracteres (edad, acento, tono, forma de hablar…)".into());
    }
    with_provider(app, provider, move |app, p| {
        let list = p.design(&description, text.as_deref())?;
        let v = app.state::<Voices>();
        let mut out = Vec::new();
        for pr in list {
            let pcm = decode_bytes(app, &pr.audio)?;
            let seconds = if pr.seconds > 0.0 { pr.seconds } else { pcm.seconds() };
            v.previews.lock().unwrap().insert(pr.generated_id.clone(), Arc::new(pcm));
            out.push(PreviewInfo { generated_id: pr.generated_id, seconds });
        }
        Ok(out)
    })
    .await
}

/// Escucha una muestra de Voice Design (`None` para parar).
#[tauri::command]
pub fn voices_preview(voices: State<'_, Voices>, audio: State<'_, Audio>, generated_id: Option<String>) -> Result<(), String> {
    let pcm = match generated_id {
        Some(id) => Some(voices.previews.lock().unwrap().get(&id).cloned().ok_or("esa muestra ya no está: vuelve a crear la voz")?),
        None => None,
    };
    audio.send(imgx_audio::device::Command::Preview(pcm));
    Ok(())
}

#[tauri::command]
pub async fn voices_save_design(app: AppHandle, provider: String, name: String, description: String, generated_id: String) -> Result<VoiceInfo, String> {
    let name: String = name.trim().chars().take(80).collect();
    if name.is_empty() {
        return Err("falta el nombre de la voz".into());
    }
    with_provider(app, provider, move |app, p| {
        let v = p.save_design(&name, &description, &generated_id)?;
        app.state::<Voices>().previews.lock().unwrap().clear();
        Ok(v)
    })
    .await
}

fn decode_bytes(app: &AppHandle, a: &provider::Audio) -> Result<imgx_audio::Pcm, String> {
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("voices");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!(".muestra-{}-{}.{}", std::process::id(), rand_suffix(), a.ext));
    std::fs::write(&tmp, &a.bytes).map_err(|e| e.to_string())?;
    let r = imgx_audio::decode_file(&tmp).map_err(|e| e.to_string());
    std::fs::remove_file(&tmp).ok();
    r
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

// ------------------------------------------------------------------ generar

/// Un fragmento que se pide al proveedor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GenItem {
    Speech(SpeakReq),
    #[serde(rename_all = "camelCase")]
    Sfx {
        text: String,
        #[serde(default)]
        seconds: Option<f64>,
        #[serde(default, rename = "loop")]
        looped: bool,
    },
    #[serde(rename_all = "camelCase")]
    Music {
        prompt: String,
        seconds: f64,
        #[serde(default)]
        instrumental: bool,
    },
}

impl GenItem {
    /// Caracteres de voz (lo que cuenta para el límite).
    fn chars(&self) -> u64 {
        match self {
            GenItem::Speech(s) => s.text.chars().count() as u64,
            _ => 0,
        }
    }

    /// Coste aproximado con los precios de la API.
    fn usd(&self) -> f64 {
        match self {
            GenItem::Speech(s) => self.chars() as f64 / 1000.0 * usd_per_1k(&s.model),
            GenItem::Sfx { seconds, .. } => seconds.unwrap_or(5.0).clamp(0.5, 30.0) / 60.0 * SFX_USD_MIN,
            GenItem::Music { seconds, .. } => seconds.clamp(3.0, 600.0) / 60.0 * MUSIC_USD_MIN,
        }
    }

    /// Huella: lo mismo pedido dos veces da el mismo sonido sin volver a pagar.
    pub fn hash(&self, provider: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut v = serde_json::to_value(self).unwrap_or_default();
        if let GenItem::Speech(s) = self {
            // Lo que no cambia el sonido no cuenta.
            v["text"] = serde_json::json!(s.text.trim());
        }
        let d = Sha256::digest(format!("{provider}\n{v}").as_bytes());
        d.iter().take(16).map(|b| format!("{b:02x}")).collect()
    }

    fn validate(&self) -> Result<(), String> {
        match self {
            GenItem::Speech(s) if s.text.trim().is_empty() => Err("hay una frase vacía".into()),
            GenItem::Speech(s) if s.voice.trim().is_empty() => Err("hay una frase sin voz: elige una voz para su personaje".into()),
            GenItem::Speech(s) if s.text.chars().count() > 5000 => Err("una frase pasa de 5000 caracteres: pártela".into()),
            GenItem::Sfx { text, .. } if text.trim().is_empty() => Err("falta la descripción del efecto".into()),
            GenItem::Music { prompt, .. } if prompt.trim().is_empty() => Err("falta la descripción de la música".into()),
            _ => Ok(()),
        }
    }
}

fn cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("voices").join("cache.json"))
}

impl Voices {
    fn with_cache<R>(&self, app: &AppHandle, f: impl FnOnce(&mut BTreeMap<String, String>) -> R) -> Result<R, String> {
        let mut g = self.cache.lock().unwrap();
        if g.is_none() {
            let m = std::fs::read_to_string(cache_path(app)?).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
            *g = Some(m);
        }
        Ok(f(g.as_mut().unwrap()))
    }

    fn save_cache(&self, app: &AppHandle) -> Result<(), String> {
        let json = self.with_cache(app, |m| serde_json::to_string(m))?.map_err(|e| e.to_string())?;
        let p = cache_path(app)?;
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        crate::project::write_atomic(&p, &json).map_err(|e| e.to_string())
    }

    /// El sonido ya generado para esta huella, si sigue en la biblioteca.
    fn cached(&self, app: &AppHandle, audio: &Audio, hash: &str) -> Option<SoundInfo> {
        let id = self.with_cache(app, |m| m.get(hash).cloned()).ok()??;
        audio.info(app, &id)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateItem {
    chars: u64,
    usd: f64,
    cached: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    items: Vec<EstimateItem>,
    /// Caracteres de voz que se pagarían (sin los de la caché).
    chars: u64,
    usd: f64,
    cached: usize,
    /// Caracteres de voz ya enviados en esta sesión.
    session: u64,
}

/// Resumen del pedido antes de enviar nada: caracteres, coste aproximado y lo que sale gratis.
#[tauri::command]
pub async fn voices_estimate(app: AppHandle, provider: String, items: Vec<GenItem>) -> Result<Estimate, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let v = app.state::<Voices>();
        let audio = app.state::<Audio>();
        let prov = if std::env::var_os("IMGX_VOICES_MOCK").is_some() { "mock" } else { provider.as_str() };
        let mut out = Estimate { items: Vec::new(), chars: 0, usd: 0.0, cached: 0, session: *v.spent.lock().unwrap() };
        for it in &items {
            let cached = v.cached(&app, &audio, &it.hash(prov)).is_some();
            let (chars, usd) = (it.chars(), it.usd());
            if cached {
                out.cached += 1;
            } else {
                out.chars += chars;
                out.usd += usd;
            }
            out.items.push(EstimateItem { chars, usd, cached });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Generated {
    sound: SoundInfo,
    /// Salió de la caché: no ha costado nada.
    cached: bool,
    chars: u64,
}

/// Genera un fragmento y lo guarda en la biblioteca (o lo saca de la caché). `limit`: tope de
/// caracteres de voz de la sesión (0 = sin tope).
#[tauri::command]
pub async fn voices_generate(app: AppHandle, provider: String, item: GenItem, name: String, limit: u64) -> Result<Generated, String> {
    item.validate()?;
    let prov = if std::env::var_os("IMGX_VOICES_MOCK").is_some() { "mock".to_owned() } else { provider.clone() };
    with_provider(app, provider, move |app, p| {
        let v = app.state::<Voices>();
        let audio = app.state::<Audio>();
        let hash = item.hash(&prov);
        if let Some(sound) = v.cached(app, &audio, &hash) {
            return Ok(Generated { sound, cached: true, chars: item.chars() });
        }
        let chars = item.chars();
        {
            let spent = *v.spent.lock().unwrap();
            if limit > 0 && spent + chars > limit {
                return Err(format!("se pasaría del límite de la sesión ({spent} de {limit} caracteres usados): súbelo en Ajustes → Voces IA"));
            }
        }
        let a = match &item {
            GenItem::Speech(s) => p.speak(s)?,
            GenItem::Sfx { text, seconds, looped } => p.sfx(text, *seconds, *looped)?,
            GenItem::Music { prompt, seconds, instrumental } => p.music(prompt, *seconds, *instrumental)?,
        };
        *v.spent.lock().unwrap() += chars;
        let name: String = name.trim().chars().take(120).collect();
        let sound = audio.import_bytes(app, &a.bytes, a.ext, &name, SoundKind::Generated)?;
        v.with_cache(app, |m| m.insert(hash, sound.id.clone()))?;
        v.save_cache(app)?;
        log::info!("voces: generado {} ({chars} caracteres, {:.1} s)", sound.id, sound.seconds);
        Ok(Generated { sound, cached: false, chars })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speech(text: &str) -> GenItem {
        GenItem::Speech(SpeakReq { voice: "v1".into(), model: "eleven_v4".into(), text: text.into(), language: None, settings: BTreeMap::new(), seed: None })
    }

    #[test]
    fn huella_estable_y_distinta() {
        assert_eq!(speech("Hola").hash("elevenlabs"), speech(" Hola ").hash("elevenlabs"));
        assert_ne!(speech("Hola").hash("elevenlabs"), speech("Hola.").hash("elevenlabs"));
        assert_ne!(speech("Hola").hash("elevenlabs"), speech("Hola").hash("mock"));
        let mut s = BTreeMap::new();
        s.insert("speed".to_owned(), 1.1);
        let other = GenItem::Speech(SpeakReq { voice: "v1".into(), model: "eleven_v4".into(), text: "Hola".into(), language: None, settings: s, seed: None });
        assert_ne!(speech("Hola").hash("elevenlabs"), other.hash("elevenlabs"));
    }

    #[test]
    fn coste_aproximado() {
        let it = speech(&"a".repeat(1000));
        assert!((it.usd() - 0.08).abs() < 1e-9);
        let m = GenItem::Music { prompt: "x".into(), seconds: 60.0, instrumental: true };
        assert!((m.usd() - 0.15).abs() < 1e-9);
        assert_eq!(m.chars(), 0);
    }

    #[test]
    fn pedido_desde_json() {
        let it: GenItem = serde_json::from_str(r#"{"kind":"speech","voice":"a","model":"eleven_v3","text":"[gritando] ¡Corre!","settings":{"stability":0.5}}"#).unwrap();
        assert_eq!(it.chars(), 18);
        let s: GenItem = serde_json::from_str(r#"{"kind":"sfx","text":"multitud gritando","seconds":6,"loop":true}"#).unwrap();
        assert!(matches!(s, GenItem::Sfx { looped: true, .. }));
        assert!(speech("  ").validate().is_err());
    }
}
