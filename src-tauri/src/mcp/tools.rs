//! Comandos de apoyo para las herramientas de la IA que necesitan Rust: buscar audio en una
//! carpeta, analizar la mezcla y revisar el MP4 exportado.

use crate::export::ExportAudio;
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioFile {
    path: String,
    name: String,
    /// Duración leída de la cabecera (si el formato la declara).
    seconds: Option<f64>,
    bytes: u64,
}

const MAX_DEPTH: usize = 8;

fn is_audio(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| imgx_audio::EXTENSIONS.contains(&e.to_lowercase().as_str()))
}

fn walk(dir: &Path, recursive: bool, depth: usize, limit: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if out.len() >= limit {
            return;
        }
        let p = e.path();
        let hidden = e.file_name().to_string_lossy().starts_with('.');
        match e.file_type() {
            Ok(t) if t.is_dir() && recursive && !hidden && depth < MAX_DEPTH => walk(&p, recursive, depth + 1, limit, out),
            Ok(t) if t.is_file() && is_audio(&p) => out.push(p),
            _ => {}
        }
    }
}

/// Archivos de audio de una carpeta (para importarlos luego a la biblioteca).
#[tauri::command]
pub async fn mcp_find_audio(folder: String, recursive: Option<bool>, limit: Option<usize>) -> Result<Vec<AudioFile>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = PathBuf::from(&folder);
        if !dir.is_dir() {
            return Err(format!("not a folder: {folder}"));
        }
        let mut found = Vec::new();
        walk(&dir, recursive.unwrap_or(true), 0, limit.unwrap_or(300).clamp(1, 2000), &mut found);
        Ok(found
            .into_iter()
            .map(|p| AudioFile {
                name: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                seconds: imgx_audio::decode::probe_seconds(&p).map(|s| (s * 100.0).round() / 100.0),
                bytes: std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0),
                path: p.to_string_lossy().into_owned(),
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Frecuencia del análisis (basta para medir niveles y es rápido).
const REPORT_RATE: u32 = 24_000;

fn db(x: f64) -> f64 {
    if x <= 1e-7 { -140.0 } else { (20.0 * x.log10() * 10.0).round() / 10.0 }
}

/// Análisis de una mezcla estéreo intercalada.
pub(crate) fn analyse(mix: &[f32], rate: u32) -> Value {
    let frames = mix.len() / 2;
    let seconds = frames as f64 / f64::from(rate);
    let (mut peak, mut sum) = (0f64, 0f64);
    let mut loud = 0usize;
    for s in mix {
        let a = f64::from(s.abs());
        peak = peak.max(a);
        sum += a * a;
        if a > 0.89 {
            loud += 1; // por encima de −1 dBFS: el limitador está trabajando
        }
    }
    let rms = (sum / mix.len().max(1) as f64).sqrt();
    // Nivel de cada segundo (o de cada tramo si es muy largo).
    let step_s = (seconds / 900.0).ceil().max(1.0);
    let step = (step_s * f64::from(rate)) as usize;
    let mut per: Vec<f64> = Vec::new();
    for chunk in mix.chunks((step * 2).max(2)) {
        let e = chunk.iter().map(|s| f64::from(*s) * f64::from(*s)).sum::<f64>() / chunk.len() as f64;
        per.push(db(e.sqrt()));
    }
    // Silencios de al menos 1,5 s (por ventanas de 0,25 s).
    let win = (f64::from(rate) * 0.25) as usize;
    let mut silences = Vec::new();
    let mut start: Option<usize> = None;
    let windows: Vec<f64> = mix.chunks((win * 2).max(2)).map(|c| (c.iter().map(|s| f64::from(*s) * f64::from(*s)).sum::<f64>() / c.len() as f64).sqrt()).collect();
    for (i, r) in windows.iter().chain(std::iter::once(&1.0)).enumerate() {
        let quiet = *r < 0.0018; // −55 dBFS
        match (quiet, start) {
            (true, None) => start = Some(i),
            (false, Some(a)) => {
                if (i - a) as f64 * 0.25 >= 1.5 {
                    silences.push(json!({ "from": a as f64 * 0.25, "to": (i as f64 * 0.25).min(seconds) }));
                }
                start = None;
            }
            _ => {}
        }
    }
    let mut ranked: Vec<(usize, f64)> = per.iter().copied().enumerate().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let loudest: Vec<Value> = ranked.iter().take(5).filter(|x| x.1 > -100.0).map(|(i, d)| json!({ "at": *i as f64 * step_s, "db": d })).collect();
    json!({
        "seconds": (seconds * 100.0).round() / 100.0,
        "peakDb": db(peak),
        "rmsDb": db(rms),
        "limiterSeconds": ((loud / 2) as f64 / f64::from(rate) * 100.0).round() / 100.0,
        "levelEvery": step_s,
        "levelsDb": per,
        "silences": silences,
        "loudest": loudest,
    })
}

/// Mezcla el sonido como al exportar y lo analiza.
#[tauri::command]
pub async fn mcp_sound_report(app: AppHandle, audio: ExportAudio) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<crate::audio::Audio>();
        let mut missing = Vec::new();
        let clips: Vec<imgx_audio::Clip> = audio
            .clips
            .iter()
            .filter_map(|c| {
                c.resolve(&app, &state)
                    .inspect_err(|e| {
                        if !missing.contains(&c.sound) {
                            missing.push(c.sound.clone());
                        }
                        log::warn!("informe de sonido: {e}");
                    })
                    .ok()
            })
            .collect();
        let mix = imgx_audio::mixer::render(&clips, &audio.tracks, audio.levels, REPORT_RATE, audio.seconds);
        let mut out = analyse(&mix, REPORT_RATE);
        out["clips"] = json!(clips.len());
        out["missingSounds"] = json!(missing);
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn ffprobe_for(ffmpeg: &str) -> String {
    let p = Path::new(ffmpeg);
    if p.components().count() > 1 {
        let sib = p.with_file_name("ffprobe");
        if sib.exists() {
            return sib.to_string_lossy().into_owned();
        }
    }
    "ffprobe".into()
}

fn frame_jpeg(ffmpeg: &str, video: &str, t: f64, width: u32) -> Option<Vec<u8>> {
    let o = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-ss", &format!("{t:.3}"), "-i", video, "-frames:v", "1", "-vf"])
        .arg(format!("scale={width}:-2"))
        .args(["-q:v", "4", "-f", "image2pipe", "-c:v", "mjpeg", "-"])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    (o.status.success() && o.stdout.len() > 100).then_some(o.stdout)
}

/// Volumen medio y máximo del sonido del vídeo (filtro volumedetect de ffmpeg).
fn volume(ffmpeg: &str, video: &str) -> Value {
    let Ok(o) = Command::new(ffmpeg).args(["-hide_banner", "-nostdin", "-i", video, "-vn", "-af", "volumedetect", "-f", "null", "-"]).stdin(Stdio::null()).output() else {
        return Value::Null;
    };
    let err = String::from_utf8_lossy(&o.stderr);
    let find = |key: &str| err.lines().find_map(|l| l.split(key).nth(1)).and_then(|v| v.trim().trim_end_matches("dB").trim().parse::<f64>().ok());
    match (find("mean_volume:"), find("max_volume:")) {
        (Some(mean), Some(max)) => json!({ "meanDb": mean, "maxDb": max }),
        _ => Value::Null,
    }
}

/// Revisa un vídeo: datos de ffprobe, nivel del sonido y fotogramas en los tiempos pedidos.
#[tauri::command]
pub async fn mcp_inspect_video(path: String, times: Vec<f64>, width: Option<u32>, ffmpeg: Option<String>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !Path::new(&path).is_file() {
            return Err(format!("file not found: {path}"));
        }
        let ffmpeg = crate::export::ffmpeg_path(ffmpeg.as_deref())?;
        let probe = Command::new(ffprobe_for(&ffmpeg))
            .args(["-v", "error", "-show_entries", "format=duration,size,bit_rate:stream=codec_type,codec_name,width,height,r_frame_rate,duration,channels,sample_rate", "-of", "json"])
            .arg(&path)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("ffprobe: {e}"))?;
        let info: Value = serde_json::from_slice(&probe.stdout).unwrap_or(Value::Null);
        let width = width.unwrap_or(480).clamp(64, 1920);
        let frames: Vec<Value> = times
            .iter()
            .take(16)
            .map(|&t| match frame_jpeg(&ffmpeg, &path, t.max(0.0), width) {
                Some(b) => json!({ "t": t, "image": super::wire::base64(&b) }),
                None => json!({ "t": t, "image": null }),
            })
            .collect();
        Ok(json!({ "probe": info, "audio": volume(&ffmpeg, &path), "frames": frames }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analiza_picos_niveles_y_silencios() {
        let rate = 1000;
        // 2 s de tono fuerte, 2 s de silencio, 1 s suave.
        let mut mix = Vec::new();
        for i in 0..rate * 2 {
            let s = (i as f32 * 0.3).sin() * 0.95;
            mix.extend([s, s]);
        }
        mix.extend(std::iter::repeat_n(0.0, rate as usize * 4));
        for i in 0..rate {
            let s = (i as f32 * 0.3).sin() * 0.05;
            mix.extend([s, s]);
        }
        let r = analyse(&mix, rate);
        assert!((r["seconds"].as_f64().unwrap() - 5.0).abs() < 0.01);
        assert!(r["peakDb"].as_f64().unwrap() > -1.0);
        assert!(r["limiterSeconds"].as_f64().unwrap() > 0.1);
        let s = r["silences"].as_array().unwrap();
        assert_eq!(s.len(), 1, "{r}");
        assert!((s[0]["from"].as_f64().unwrap() - 2.0).abs() < 0.3 && (s[0]["to"].as_f64().unwrap() - 4.0).abs() < 0.3);
        let levels = r["levelsDb"].as_array().unwrap();
        assert_eq!(levels.len(), 5);
        assert!(levels[0].as_f64().unwrap() > levels[4].as_f64().unwrap() + 20.0);
        assert_eq!(r["loudest"][0]["at"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn busca_solo_audio() {
        let dir = std::env::temp_dir().join(format!("imgx-find-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::create_dir_all(dir.join(".oculta")).unwrap();
        for f in ["a.wav", "b.txt", "sub/c.MP3", ".oculta/d.ogg"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let mut out = Vec::new();
        walk(&dir, true, 0, 100, &mut out);
        let names: Vec<_> = out.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["a.wav", "c.MP3"]);
        out.clear();
        walk(&dir, false, 0, 100, &mut out);
        assert_eq!(out.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
