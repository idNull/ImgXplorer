//! Sonido en la app: la biblioteca de sonidos (copias en el directorio de datos, sin duplicados),
//! la caché de lo decodificado y las órdenes al motor de audio en directo.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use imgx_audio::device::{Command, Output};
use imgx_audio::fx::FxSpec;
use imgx_audio::{Bus, Clip, Levels, Pcm, TrackSpec};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

/// Prefijo de los efectos de serie (no son archivos: se generan al usarlos).
const BUILTIN: &str = "builtin:";
/// Memoria máxima para sonidos decodificados.
const CACHE_BYTES: usize = 768 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SoundKind {
    Imported,
    Recorded,
    Builtin,
}

/// Un sonido de la biblioteca (lo que guarda `sounds.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundEntry {
    pub id: String,
    pub name: String,
    /// Nombre del archivo dentro de `sounds/`.
    pub file: String,
    pub kind: SoundKind,
    pub seconds: f64,
    #[serde(default)]
    pub added: u64,
}

/// Lo que ve la interfaz.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundInfo {
    pub id: String,
    /// Vacío en los de serie: la interfaz pone el nombre traducido.
    pub name: String,
    pub kind: SoundKind,
    pub seconds: f64,
    /// Pensado para sonar en bucle (ambientes de serie).
    pub ambient: bool,
}

impl From<&SoundEntry> for SoundInfo {
    fn from(e: &SoundEntry) -> Self {
        SoundInfo { id: e.id.clone(), name: e.name.clone(), kind: e.kind, seconds: e.seconds, ambient: false }
    }
}

struct Cache {
    map: HashMap<String, Arc<Pcm>>,
    /// Uso más reciente al final.
    order: Vec<String>,
    bytes: usize,
    limit: usize,
}

impl Default for Cache {
    fn default() -> Self {
        Cache { map: HashMap::new(), order: Vec::new(), bytes: 0, limit: CACHE_BYTES }
    }
}

impl Cache {
    fn get(&mut self, id: &str) -> Option<Arc<Pcm>> {
        let p = self.map.get(id)?.clone();
        self.order.retain(|x| x != id);
        self.order.push(id.to_owned());
        Some(p)
    }
    fn put(&mut self, id: &str, p: Arc<Pcm>) {
        if self.map.contains_key(id) {
            return;
        }
        self.bytes += p.bytes();
        self.map.insert(id.to_owned(), p);
        self.order.push(id.to_owned());
        while self.bytes > self.limit && self.order.len() > 1 {
            let old = self.order.remove(0);
            if let Some(p) = self.map.remove(&old) {
                self.bytes -= p.bytes();
            }
        }
    }
    fn remove(&mut self, id: &str) {
        if let Some(p) = self.map.remove(id) {
            self.bytes -= p.bytes();
        }
        self.order.retain(|x| x != id);
    }
}

/// Estado del sonido de la app.
#[derive(Default)]
pub struct Audio {
    index: Mutex<Option<Vec<SoundEntry>>>,
    cache: Mutex<Cache>,
    out: Mutex<Option<Result<Output, String>>>,
    /// Última escena pedida: una decodificación lenta no pisa una escena más nueva.
    scene_seq: AtomicU64,
    ambient_seq: AtomicU64,
    pub(crate) recorder: Mutex<Option<imgx_audio::device::Recorder>>,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub(crate) fn dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("sounds"))
}

fn index_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(dir(app)?.join("sounds.json"))
}

impl Audio {
    /// Ejecuta `f` con el índice (lo carga la primera vez).
    fn with_index<R>(&self, app: &AppHandle, f: impl FnOnce(&mut Vec<SoundEntry>) -> R) -> Result<R, String> {
        let mut g = self.index.lock().unwrap();
        if g.is_none() {
            let list = match std::fs::read_to_string(index_path(app)?) {
                Ok(s) => serde_json::from_str::<Vec<SoundEntry>>(&s).unwrap_or_else(|e| {
                    log::warn!("sounds.json ilegible ({e}); se empieza vacío");
                    Vec::new()
                }),
                Err(_) => Vec::new(),
            };
            *g = Some(list);
        }
        Ok(f(g.as_mut().unwrap()))
    }

    fn save_index(&self, app: &AppHandle) -> Result<(), String> {
        let json = self.with_index(app, |l| serde_json::to_string_pretty(l))?.map_err(|e| e.to_string())?;
        let path = index_path(app)?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        crate::project::write_atomic(&path, &json).map_err(|e| e.to_string())
    }

    /// El sonido decodificado (de la caché, generándolo o leyéndolo del disco).
    pub fn pcm(&self, app: &AppHandle, id: &str) -> Result<Arc<Pcm>, String> {
        if let Some(p) = self.cache.lock().unwrap().get(id) {
            return Ok(p);
        }
        let pcm = if let Some(b) = id.strip_prefix(BUILTIN) {
            imgx_audio::synth::builtin(b).ok_or_else(|| format!("efecto de serie desconocido: {b}"))?
        } else {
            let file = self.with_index(app, |l| l.iter().find(|e| e.id == id).map(|e| e.file.clone()))?.ok_or_else(|| format!("el sonido {id} no está en la biblioteca"))?;
            imgx_audio::decode_file(&dir(app)?.join(file)).map_err(|e| e.to_string())?
        };
        let p = Arc::new(pcm);
        self.cache.lock().unwrap().put(id, p.clone());
        Ok(p)
    }

    /// La salida de sonido (la arranca la primera vez).
    fn output(&self) -> std::sync::MutexGuard<'_, Option<Result<Output, String>>> {
        let mut g = self.out.lock().unwrap();
        if g.is_none() {
            *g = Some(Output::start().map_err(|e| {
                log::warn!("audio: sin salida de sonido: {e}");
                e.to_string()
            }));
        }
        g
    }

    /// Manda una orden al motor. Sin dispositivo, no hace nada.
    pub fn send(&self, c: Command) {
        if let Some(Ok(o)) = self.output().as_ref() {
            o.send(c);
        }
    }

    /// Importa un archivo: lo valida decodificándolo y lo copia (si no estaba ya).
    fn import_one(&self, app: &AppHandle, path: &Path, kind: SoundKind) -> Result<SoundInfo, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("bin").to_lowercase();
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        self.import_bytes(app, &bytes, &ext, &name, kind).map_err(|e| format!("{}: {e}", file_name(path)))
    }

    /// Guarda un sonido en la biblioteca (si no estaba ya: el id sale del contenido).
    pub(crate) fn import_bytes(&self, app: &AppHandle, bytes: &[u8], ext: &str, name: &str, kind: SoundKind) -> Result<SoundInfo, String> {
        let id = content_id(bytes);
        if let Some(e) = self.with_index(app, |l| l.iter().find(|e| e.id == id).cloned())? {
            return Ok((&e).into());
        }
        let ext: String = ext.chars().filter(char::is_ascii_alphanumeric).take(8).collect::<String>().to_lowercase();
        let file = format!("{id}.{}", if ext.is_empty() { "bin" } else { &ext });
        let d = dir(app)?;
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        let path = d.join(&file);
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        // Se comprueba que de verdad es sonido antes de darlo por bueno.
        let pcm = match imgx_audio::decode_file(&path) {
            Ok(p) => p,
            Err(e) => {
                std::fs::remove_file(&path).ok();
                return Err(e.to_string());
            }
        };
        let name = if name.trim().is_empty() { id.clone() } else { name.trim().chars().take(120).collect() };
        let entry = SoundEntry { id: id.clone(), name, file, kind, seconds: pcm.seconds(), added: now() };
        self.cache.lock().unwrap().put(&id, Arc::new(pcm));
        let info = (&entry).into();
        self.with_index(app, |l| l.push(entry))?;
        self.save_index(app)?;
        Ok(info)
    }

    /// Los sonidos propios (importados y grabados) con la ruta de su archivo.
    pub(crate) fn user_sounds(&self, app: &AppHandle) -> Result<Vec<(SoundEntry, PathBuf)>, String> {
        let d = dir(app)?;
        self.with_index(app, |l| l.iter().map(|e| (e.clone(), d.join(&e.file))).collect())
    }

    /// Guarda una grabación como WAV en la biblioteca.
    pub fn add_recording(&self, app: &AppHandle, pcm: Pcm, name: String) -> Result<SoundInfo, String> {
        let d = dir(app)?;
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        let tmp = d.join(format!(".grabacion-{}.wav", std::process::id()));
        imgx_audio::wav::write(&tmp, &pcm).map_err(|e| e.to_string())?;
        let res = self.import_one(app, &tmp, SoundKind::Recorded);
        std::fs::remove_file(&tmp).ok();
        let info = res?;
        self.with_index(app, |l| {
            if let Some(e) = l.iter_mut().find(|e| e.id == info.id) {
                e.name = name.clone();
            }
        })?;
        self.save_index(app)?;
        Ok(SoundInfo { name, ..info })
    }
}

/// Identificador por contenido: el mismo archivo importado dos veces es el mismo sonido.
fn content_id(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(bytes);
    d.iter().take(10).map(|b| format!("{b:02x}")).collect()
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

// ------------------------------------------------------------------ biblioteca

#[tauri::command]
pub fn sound_list(app: AppHandle, audio: State<'_, Audio>) -> Result<Vec<SoundInfo>, String> {
    let mut out: Vec<SoundInfo> = imgx_audio::synth::BUILTINS
        .iter()
        .map(|b| SoundInfo { id: format!("{BUILTIN}{}", b.id), name: String::new(), kind: SoundKind::Builtin, seconds: b.seconds, ambient: b.ambient })
        .collect();
    out.extend(audio.with_index(&app, |l| l.iter().map(SoundInfo::from).collect::<Vec<_>>())?);
    Ok(out)
}

#[derive(Serialize)]
pub struct ImportResult {
    added: Vec<SoundInfo>,
    errors: Vec<String>,
}

#[tauri::command]
pub async fn sound_import(app: AppHandle, paths: Vec<String>) -> Result<ImportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let audio = app.state::<Audio>();
        let mut r = ImportResult { added: Vec::new(), errors: Vec::new() };
        for p in paths {
            match audio.import_one(&app, Path::new(&p), SoundKind::Imported) {
                Ok(i) => r.added.push(i),
                Err(e) => {
                    log::warn!("importar sonido: {e}");
                    r.errors.push(e);
                }
            }
        }
        r
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sound_rename(app: AppHandle, audio: State<'_, Audio>, id: String, name: String) -> Result<(), String> {
    let name = name.trim().chars().take(120).collect::<String>();
    if name.is_empty() {
        return Err("el nombre está vacío".into());
    }
    let found = audio.with_index(&app, |l| l.iter_mut().find(|e| e.id == id).map(|e| e.name = name).is_some())?;
    if !found {
        return Err("ese sonido no se puede renombrar".into());
    }
    audio.save_index(&app)
}

#[tauri::command]
pub fn sound_delete(app: AppHandle, audio: State<'_, Audio>, id: String) -> Result<(), String> {
    let entry = audio.with_index(&app, |l| l.iter().position(|e| e.id == id).map(|i| l.remove(i)))?;
    let Some(e) = entry else { return Err("ese sonido no se puede borrar".into()) };
    std::fs::remove_file(dir(&app)?.join(&e.file)).ok();
    audio.cache.lock().unwrap().remove(&id);
    audio.save_index(&app)
}

/// Forma de onda: `buckets` picos entre 0 y 1.
#[tauri::command]
pub async fn sound_peaks(app: AppHandle, id: String, buckets: usize) -> Result<Vec<f32>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = app.state::<Audio>().pcm(&app, &id)?;
        Ok(p.peaks(buckets.clamp(1, 20000)))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Escuchar un sonido de la biblioteca (`None` para parar).
#[tauri::command]
pub async fn sound_preview(app: AppHandle, id: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let audio = app.state::<Audio>();
        let pcm = match id {
            Some(id) => Some(audio.pcm(&app, &id)?),
            None => None,
        };
        audio.send(Command::Preview(pcm));
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ------------------------------------------------------------------ motor en directo

/// Un clip tal como lo describe la interfaz.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipSpec {
    pub sound: String,
    pub bus: Bus,
    pub start: f64,
    #[serde(default)]
    pub trim_in: f64,
    pub duration: f64,
    #[serde(default = "one")]
    pub volume: f32,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default)]
    pub looped: bool,
    /// Pista (si falta, la primera de su grupo).
    #[serde(default)]
    pub track: String,
    #[serde(default)]
    pub effects: Vec<FxSpec>,
}

fn one() -> f32 {
    1.0
}

impl ClipSpec {
    pub fn resolve(&self, app: &AppHandle, audio: &Audio) -> Result<Clip, String> {
        Ok(Clip {
            sound: audio.pcm(app, &self.sound)?,
            bus: self.bus,
            start: self.start,
            trim_in: self.trim_in.max(0.0),
            duration: if self.looped && self.duration <= 0.0 { f64::MAX } else { self.duration.max(0.0) },
            volume: self.volume.clamp(0.0, 4.0),
            fade_in: self.fade_in.max(0.0),
            fade_out: self.fade_out.max(0.0),
            looped: self.looped,
            track: if self.track.is_empty() { self.bus.name().into() } else { self.track.clone() },
            effects: self.effects.clone(),
        })
    }
}

/// Convierte especificaciones en clips; los sonidos que faltan se saltan (y se avisa una vez).
fn resolve_all(app: &AppHandle, audio: &Audio, specs: &[ClipSpec]) -> Vec<Clip> {
    specs
        .iter()
        .filter_map(|s| s.resolve(app, audio).inspect_err(|e| log::warn!("audio: {e}")).ok())
        .collect()
}

/// Voz y efectos de la imagen actual (tiempos relativos a la imagen).
#[tauri::command]
pub async fn audio_scene(app: AppHandle, clips: Vec<ClipSpec>) -> Result<(), String> {
    let seq = app.state::<Audio>().scene_seq.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn_blocking(move || {
        let audio = app.state::<Audio>();
        let list = resolve_all(&app, &audio, &clips);
        if audio.scene_seq.load(Ordering::SeqCst) == seq {
            audio.send(Command::Scene(list));
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct AmbientSpec {
    pub id: String,
    #[serde(flatten)]
    pub clip: ClipSpec,
}

/// Ambientes que deben sonar ahora.
#[tauri::command]
pub async fn audio_ambient(app: AppHandle, list: Vec<AmbientSpec>) -> Result<(), String> {
    let seq = app.state::<Audio>().ambient_seq.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn_blocking(move || {
        let audio = app.state::<Audio>();
        let mut out = Vec::new();
        for a in &list {
            match a.clip.resolve(&app, &audio) {
                Ok(c) => out.push((a.id.clone(), Clip { looped: true, duration: f64::MAX, ..c })),
                Err(e) => log::warn!("audio: {e}"),
            }
        }
        if audio.ambient_seq.load(Ordering::SeqCst) == seq {
            audio.send(Command::Ambient(out));
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn audio_transport(audio: State<'_, Audio>, t: f64, playing: bool, running: bool, rate: f64, seek: bool) {
    audio.send(Command::Transport { t, playing, running, rate, seek });
}

/// Pistas del proyecto y las que están en solo.
#[tauri::command]
pub fn audio_tracks(audio: State<'_, Audio>, tracks: Vec<TrackSpec>, solo: Vec<String>) {
    audio.send(Command::Tracks { tracks, solo });
}

#[tauri::command]
pub fn audio_levels(audio: State<'_, Audio>, levels: Levels, master: f32, mute: bool) {
    audio.send(Command::Levels(levels));
    audio.send(Command::Master(master));
    audio.send(Command::Mute(mute));
}

#[tauri::command]
pub fn audio_silence(audio: State<'_, Audio>) {
    audio.send(Command::Silence);
}

// ------------------------------------------------------------------ grabar voz

/// Micrófonos disponibles.
#[tauri::command]
pub async fn audio_inputs() -> Vec<imgx_audio::device::DeviceInfo> {
    tauri::async_runtime::spawn_blocking(imgx_audio::device::input_devices).await.unwrap_or_default()
}

/// Empieza a grabar del micrófono (`None`: el del sistema).
#[tauri::command]
pub async fn record_start(app: AppHandle, device: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let audio = app.state::<Audio>();
        audio.recorder.lock().unwrap().take();
        let r = imgx_audio::device::Recorder::start(device.as_deref().filter(|d| !d.is_empty())).map_err(|e| e.to_string())?;
        *audio.recorder.lock().unwrap() = Some(r);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
pub struct RecordLevel {
    level: f32,
    seconds: f64,
}

/// Nivel del micrófono (pico desde la última consulta) y lo grabado hasta ahora.
#[tauri::command]
pub fn record_level(audio: State<'_, Audio>) -> Option<RecordLevel> {
    let g = audio.recorder.lock().unwrap();
    g.as_ref().map(|r| RecordLevel { level: r.level(), seconds: r.seconds() })
}

/// Para la grabación. Con `keep`, la guarda en la biblioteca como `name` y la devuelve.
#[tauri::command]
pub async fn record_stop(app: AppHandle, keep: bool, name: String) -> Result<Option<SoundInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let audio = app.state::<Audio>();
        let Some(r) = audio.recorder.lock().unwrap().take() else { return Ok(None) };
        let pcm = r.finish();
        if !keep {
            return Ok(None);
        }
        if pcm.seconds() < 0.2 {
            return Err("la grabación es demasiado corta".into());
        }
        audio.add_recording(&app, pcm, name).map(Some)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
pub struct AudioStatus {
    device: Option<String>,
    error: Option<String>,
}

/// Qué salida de sonido se usa (o por qué no hay).
#[tauri::command]
pub fn audio_status(audio: State<'_, Audio>) -> AudioStatus {
    match audio.output().as_ref() {
        Some(Ok(o)) => AudioStatus { device: Some(format!("{} · {} Hz", o.device, o.rate)), error: None },
        Some(Err(e)) => AudioStatus { device: None, error: Some(e.clone()) },
        None => AudioStatus { device: None, error: None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_pistas_y_efectos_como_los_manda_la_interfaz() {
        let fx = r#"{"id":"fx-1","type":"reverb","on":true,"preset":"room","params":{"size":0.35,"mix":0.2}}"#;
        let clip: ClipSpec = serde_json::from_str(&format!(r#"{{"sound":"builtin:hit","bus":"voice","start":1,"duration":2,"track":"trk-a","effects":[{fx}]}}"#)).unwrap();
        assert_eq!(clip.track, "trk-a");
        assert_eq!(clip.effects[0].kind, imgx_audio::fx::FxKind::Reverb);
        assert_eq!(clip.effects[0].params["size"], 0.35);
        let tracks: Vec<TrackSpec> = serde_json::from_str(&format!(r#"[{{"id":"voice","bus":"voice","volume":0.8,"pan":-0.5,"mute":false,"effects":[{fx}]}}]"#)).unwrap();
        assert_eq!(tracks[0].pan, -0.5);
        let old: ClipSpec = serde_json::from_str(r#"{"sound":"x","bus":"sfx","start":0,"duration":1}"#).unwrap();
        assert!(old.track.is_empty() && old.effects.is_empty(), "lo de antes sigue valiendo");
    }

    #[test]
    fn identificador_por_contenido() {
        assert_eq!(content_id(b"hola"), content_id(b"hola"));
        assert_ne!(content_id(b"hola"), content_id(b"adios"));
        assert_eq!(content_id(b"x").len(), 20);
    }

    #[test]
    fn cache_por_tamano() {
        let mut c = Cache { limit: 1000, ..Default::default() };
        let big = |n: usize| Arc::new(Pcm { rate: 1, data: vec![0.0; n] });
        c.put("a", big(125)); // 500 bytes
        c.put("b", big(125));
        assert!(c.get("a").is_some()); // «a» pasa a ser la más reciente
        c.put("c", big(125)); // 1500 bytes: sale «b», la menos usada
        assert!(c.get("b").is_none());
        assert!(c.get("a").is_some() && c.get("c").is_some());
        c.remove("a");
        assert!(c.get("a").is_none());
    }

    #[test]
    fn especificacion_de_clip() {
        let s: ClipSpec = serde_json::from_str(r#"{"sound":"builtin:rain","bus":"ambient","start":1,"duration":0,"looped":true}"#).unwrap();
        assert_eq!(s.volume, 1.0);
        assert_eq!(s.bus, Bus::Ambient);
        let a: AmbientSpec = serde_json::from_str(r#"{"id":"x","sound":"s","bus":"ambient","start":0,"duration":0,"fadeIn":2}"#).unwrap();
        assert_eq!(a.clip.fade_in, 2.0);
    }
}
