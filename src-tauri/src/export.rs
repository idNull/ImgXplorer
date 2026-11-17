//! Exportar a vídeo. La interfaz pinta cada fotograma con el mismo motor que la vista previa,
//! lo convierte a YUV 4:2:0 en la GPU y lo envía aquí; se pasa a ffmpeg por su entrada estándar.
//! ffmpeg escribe en un archivo temporal junto al destino y, al terminar bien, se renombra.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tauri::ipc::{InvokeBody, Request};
use tauri::State;

/// Codificadores H.264 que se saben usar, por orden de preferencia.
const ENCODERS: [&str; 3] = ["libx264", "h264_videotoolbox", "libopenh264"];
/// Cuánto del final de los mensajes de ffmpeg se guarda para explicar un fallo.
const STDERR_TAIL: usize = 4000;
/// Fotogramas en cola hacia ffmpeg (si se llena, la interfaz espera).
const QUEUE: usize = 3;

#[derive(Default)]
pub struct Exporter {
    job: Mutex<Option<Job>>,
}

struct Job {
    child: Child,
    tx: Option<SyncSender<Vec<u8>>>,
    writer: Option<JoinHandle<std::io::Result<()>>>,
    stderr: Arc<Mutex<String>>,
    tmp: PathBuf,
    out: PathBuf,
    frame_bytes: usize,
    finished: bool,
}

impl Job {
    fn stderr_tail(&self) -> String {
        self.stderr.lock().unwrap().trim().to_owned()
    }

    /// Cierra la entrada de ffmpeg y espera a que termine. `Ok(true)` si acabó bien.
    fn close(&mut self) -> std::io::Result<bool> {
        self.tx = None; // el hilo escritor termina y cierra stdin
        let wrote = self.writer.take().map(|h| h.join().unwrap_or_else(|_| Err(std::io::Error::other("hilo escritor"))));
        let status = self.child.wait()?;
        Ok(status.success() && !matches!(wrote, Some(Err(_))))
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if !self.finished {
            self.tx = None;
            let _ = self.child.kill();
            let _ = self.child.wait();
            if let Some(h) = self.writer.take() {
                let _ = h.join();
            }
            let _ = std::fs::remove_file(&self.tmp);
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    path: String,
    width: u32,
    height: u32,
    fps: u32,
    /// 0 (máxima calidad) .. 51 (mínima), como el CRF de x264.
    crf: u8,
    /// Ruta de ffmpeg elegida por el usuario (vacía: buscarlo).
    ffmpeg: Option<String>,
}

#[derive(Serialize)]
pub struct FfmpegInfo {
    path: String,
    version: String,
    /// Codificador H.264 que se usará (vacío si no hay ninguno).
    encoder: String,
}

/// Dónde buscar ffmpeg. En macOS las apps gráficas no heredan el PATH de la terminal, así que
/// también se prueban las rutas de Homebrew.
fn candidates(user: Option<&str>) -> Vec<String> {
    match user.map(str::trim).filter(|s| !s.is_empty()) {
        Some(p) => vec![p.to_owned()],
        None => ["ffmpeg", "/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg", "/usr/bin/ffmpeg"].map(String::from).to_vec(),
    }
}

fn run(bin: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).stdin(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn probe(user: Option<&str>) -> Result<FfmpegInfo, String> {
    for bin in candidates(user) {
        let Some(v) = run(&bin, &["-hide_banner", "-version"]) else { continue };
        let version = v.lines().next().unwrap_or("").trim().to_owned();
        let list = run(&bin, &["-hide_banner", "-encoders"]).unwrap_or_default();
        let has = |name: &str| list.lines().any(|l| l.split_whitespace().nth(1) == Some(name));
        let encoder = ENCODERS.iter().find(|e| has(e)).map(|e| e.to_string()).unwrap_or_default();
        return Ok(FfmpegInfo { path: bin, version, encoder });
    }
    Err(match user.filter(|s| !s.trim().is_empty()) {
        Some(p) => format!("no se pudo ejecutar «{p}»"),
        None => "ffmpeg no está instalado".into(),
    })
}

#[tauri::command]
pub async fn ffmpeg_info(path: Option<String>) -> Result<FfmpegInfo, String> {
    tauri::async_runtime::spawn_blocking(move || probe(path.as_deref())).await.map_err(|e| e.to_string())?
}

/// Argumentos de ffmpeg: YUV 4:2:0 crudo (BT.709, rango limitado) por stdin → MP4 H.264.
fn ffmpeg_args(o: &ExportOptions, encoder: &str, tmp: &Path) -> Vec<String> {
    let mut a: Vec<String> = [
        "-hide_banner", "-loglevel", "error", "-nostdin", "-y",
        "-f", "rawvideo", "-pix_fmt", "yuv420p",
    ]
    .map(String::from)
    .to_vec();
    a.extend(["-s".into(), format!("{}x{}", o.width, o.height), "-r".into(), o.fps.to_string(), "-i".into(), "pipe:0".into()]);
    a.extend(["-c:v".into(), encoder.into()]);
    let crf = o.crf.min(51);
    if encoder == "libx264" {
        a.extend(["-preset".into(), "medium".into(), "-crf".into(), crf.to_string()]);
    } else {
        // Sin CRF: tasa de bits según tamaño y calidad (≈0.15 bits por píxel con CRF 18).
        let bpp = 0.15 * 0.5f64.powf((f64::from(crf) - 18.0) / 6.0);
        let rate = (f64::from(o.width) * f64::from(o.height) * f64::from(o.fps) * bpp) as u64;
        a.extend(["-b:v".into(), rate.max(200_000).to_string()]);
    }
    a.extend(
        [
            "-pix_fmt", "yuv420p", "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
            "-color_range", "tv", "-movflags", "+faststart", "-f", "mp4",
        ]
        .map(String::from),
    );
    a.push(tmp.to_string_lossy().into_owned());
    a
}

fn temp_path(out: &Path) -> PathBuf {
    let name = out.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "video.mp4".into());
    out.with_file_name(format!(".{name}.part"))
}

fn writer_thread(mut stdin: ChildStdin, rx: std::sync::mpsc::Receiver<Vec<u8>>) -> JoinHandle<std::io::Result<()>> {
    std::thread::spawn(move || {
        for frame in rx {
            stdin.write_all(&frame)?;
        }
        stdin.flush()
    })
}

fn start(o: &ExportOptions) -> Result<Job, String> {
    if o.width < 16 || o.height < 16 || !o.width.is_multiple_of(4) || !o.height.is_multiple_of(4) || o.width > 8192 || o.height > 8192 {
        return Err(format!("tamaño no válido: {}×{} (múltiplos de 4, de 16 a 8192)", o.width, o.height));
    }
    if !(1..=120).contains(&o.fps) {
        return Err(format!("fotogramas por segundo no válidos: {}", o.fps));
    }
    let info = probe(o.ffmpeg.as_deref())?;
    if info.encoder.is_empty() {
        return Err(format!("{} no tiene ningún codificador H.264 ({})", info.path, ENCODERS.join(", ")));
    }
    let out = PathBuf::from(&o.path);
    let tmp = temp_path(&out);
    let mut child = Command::new(&info.path)
        .args(ffmpeg_args(o, &info.encoder, &tmp))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("no se pudo iniciar ffmpeg: {e}"))?;
    let stderr = Arc::new(Mutex::new(String::new()));
    if let Some(mut err) = child.stderr.take() {
        let buf = stderr.clone();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 1024];
            while let Ok(n) = err.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                let mut b = buf.lock().unwrap();
                b.push_str(&String::from_utf8_lossy(&chunk[..n]));
                if b.len() > STDERR_TAIL {
                    let cut = b.len() - STDERR_TAIL;
                    let cut = (cut..b.len()).find(|&i| b.is_char_boundary(i)).unwrap_or(b.len());
                    b.drain(..cut);
                }
            }
        });
    }
    let stdin = child.stdin.take().ok_or("ffmpeg sin entrada estándar")?;
    let (tx, rx) = sync_channel::<Vec<u8>>(QUEUE);
    let frame_bytes = (o.width * o.height * 3 / 2) as usize;
    log::info!("exportando {}×{} a {} fps con {} → {}", o.width, o.height, o.fps, info.encoder, out.display());
    Ok(Job { child, tx: Some(tx), writer: Some(writer_thread(stdin, rx)), stderr, tmp, out, frame_bytes, finished: false })
}

/// Empieza una exportación (cancela la anterior si quedaba alguna).
#[tauri::command]
pub async fn export_start(options: ExportOptions, state: State<'_, Exporter>) -> Result<(), String> {
    state.job.lock().unwrap().take();
    let job = tauri::async_runtime::spawn_blocking(move || start(&options)).await.map_err(|e| e.to_string())??;
    *state.job.lock().unwrap() = Some(job);
    Ok(())
}

/// Un fotograma YUV 4:2:0 (cuerpo binario de la petición).
#[tauri::command]
pub async fn export_frame(request: Request<'_>, state: State<'_, Exporter>) -> Result<(), String> {
    let InvokeBody::Raw(data) = request.body() else { return Err("se esperaba un fotograma binario".into()) };
    let (tx, expected, stderr) = {
        let job = state.job.lock().unwrap();
        let job = job.as_ref().ok_or("no hay ninguna exportación en curso")?;
        (job.tx.clone().ok_or("la exportación ya terminó")?, job.frame_bytes, job.stderr.clone())
    };
    if data.len() != expected {
        return Err(format!("fotograma de {} bytes, se esperaban {expected}", data.len()));
    }
    let frame = data.clone();
    let sent = tauri::async_runtime::spawn_blocking(move || tx.send(frame)).await.map_err(|e| e.to_string())?;
    sent.map_err(|_| {
        let msg = stderr.lock().unwrap().trim().to_owned();
        if msg.is_empty() { "ffmpeg se detuvo".to_owned() } else { format!("ffmpeg se detuvo: {msg}") }
    })
}

/// Termina: cierra la entrada de ffmpeg, espera y deja el vídeo en su sitio. Devuelve la ruta.
#[tauri::command]
pub async fn export_finish(state: State<'_, Exporter>) -> Result<String, String> {
    let mut job = state.job.lock().unwrap().take().ok_or("no hay ninguna exportación en curso")?;
    tauri::async_runtime::spawn_blocking(move || {
        let ok = job.close().map_err(|e| e.to_string())?;
        if !ok {
            let msg = job.stderr_tail();
            return Err(if msg.is_empty() { "ffmpeg terminó con error".into() } else { msg });
        }
        std::fs::rename(&job.tmp, &job.out).map_err(|e| format!("{}: {e}", job.out.display()))?;
        job.finished = true;
        Ok(job.out.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Cancela la exportación en curso y borra el archivo a medias.
#[tauri::command]
pub async fn export_cancel(state: State<'_, Exporter>) -> Result<(), String> {
    let job = state.job.lock().unwrap().take();
    tauri::async_runtime::spawn_blocking(move || drop(job)).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(path: &Path, crf: u8) -> ExportOptions {
        ExportOptions { path: path.to_string_lossy().into_owned(), width: 64, height: 48, fps: 10, crf, ffmpeg: None }
    }

    #[test]
    fn argumentos() {
        let o = opts(Path::new("/x/v.mp4"), 23);
        let tmp = temp_path(Path::new("/x/v.mp4"));
        assert_eq!(tmp, PathBuf::from("/x/.v.mp4.part"));
        let a = ffmpeg_args(&o, "libx264", &tmp).join(" ");
        assert!(a.contains("-s 64x48 -r 10 -i pipe:0"), "{a}");
        assert!(a.contains("-crf 23") && a.ends_with("-f mp4 /x/.v.mp4.part"), "{a}");
        let b = ffmpeg_args(&o, "h264_videotoolbox", &tmp).join(" ");
        assert!(b.contains("-b:v") && !b.contains("-crf"), "{b}");
    }

    /// Codifica unos fotogramas de verdad si hay ffmpeg con H.264 en el sistema.
    #[test]
    fn codifica_si_hay_ffmpeg() {
        let Ok(info) = probe(None) else { return };
        if info.encoder.is_empty() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("imgx-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("prueba.mp4");
        let o = opts(&out, 30);
        let mut job = start(&o).unwrap();
        let tx = job.tx.clone().unwrap();
        for i in 0..10u8 {
            let mut f = vec![128u8; job.frame_bytes];
            f[..64 * 48].fill(16 + i * 20);
            tx.send(f).unwrap();
        }
        drop(tx);
        assert!(job.close().unwrap(), "{}", job.stderr_tail());
        std::fs::rename(&job.tmp, &job.out).unwrap();
        job.finished = true;
        assert!(std::fs::metadata(&out).unwrap().len() > 100);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rechaza_tamanos_raros() {
        let mut o = opts(Path::new("/tmp/x.mp4"), 23);
        o.width = 63;
        assert!(start(&o).is_err());
    }
}
