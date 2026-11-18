//! Lado de la app: escucha en el socket privado, pasa cada llamada a la interfaz (que tiene el
//! proyecto, el deshacer y el dibujo de fotogramas) y devuelve su respuesta al puente.

use super::wire::{self, Call, Reply};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

/// Imágenes que se conservan en la carpeta temporal (las más antiguas se borran).
const KEEP_IMAGES: u64 = 400;

/// Respuesta de la interfaz a una llamada.
#[derive(Debug, Deserialize)]
pub struct UiReply {
    ok: bool,
    #[serde(default)]
    text: String,
    /// JPEG o PNG en base64 (con o sin prefijo `data:`).
    #[serde(default)]
    images: Vec<String>,
    #[serde(default)]
    error: String,
}

#[derive(Default)]
struct Shared {
    /// `None` hasta que la interfaz carga los ajustes.
    enabled: Mutex<Option<bool>>,
    pending: Mutex<HashMap<u64, Sender<UiReply>>>,
    next: AtomicU64,
    conns: Mutex<HashMap<u64, UnixStream>>,
    conn_seq: AtomicU64,
    images: Mutex<Option<PathBuf>>,
    image_seq: AtomicU64,
    /// Socket en el que escucha esta app (vacío si no escucha).
    socket: Mutex<Option<PathBuf>>,
}

#[derive(Default)]
pub struct Host(Arc<Shared>);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Activity {
    /// `connected`, `disconnected` o `call`.
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ok: Option<bool>,
    connections: usize,
}

/// Tiempo máximo de cada herramienta en la interfaz.
fn timeout(tool: &str) -> Duration {
    match tool {
        "open_source" | "render_frames" | "summary" | "sound_report" | "import_sounds" | "inspect_video" => Duration::from_secs(600),
        _ => Duration::from_secs(180),
    }
}

/// Empieza a escuchar. Si ya hay otra imgxplorer escuchando, esta no (la IA maneja la primera).
pub fn start(app: &AppHandle) {
    let host = app.state::<Host>().0.clone();
    if let Ok(dir) = app.path().app_cache_dir() {
        let dir = dir.join("mcp");
        let _ = std::fs::remove_dir_all(&dir);
        if std::fs::create_dir_all(&dir).is_ok() {
            *host.images.lock().unwrap() = Some(dir);
        }
    }
    let path = match wire::prepare_dir() {
        Ok(d) => d.join("mcp.sock"),
        Err(e) => return log::warn!("control por IA: {e}"),
    };
    if UnixStream::connect(&path).is_ok() {
        return log::info!("control por IA: otra imgxplorer ya escucha en {}", path.display());
    }
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => return log::warn!("control por IA: no se pudo escuchar en {}: {e}", path.display()),
    };
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    *host.socket.lock().unwrap() = Some(path.clone());
    log::info!("control por IA: escuchando en {}", path.display());
    let app = app.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if !wire::same_user(&stream) {
                log::warn!("control por IA: conexión rechazada (otro usuario)");
                continue;
            }
            let (app, host) = (app.clone(), host.clone());
            std::thread::spawn(move || serve(app, host, stream));
        }
    });
}

/// Borra el socket al salir.
pub fn stop(app: &AppHandle) {
    if let Some(p) = app.state::<Host>().0.socket.lock().unwrap().take() {
        let _ = std::fs::remove_file(p);
    }
}

fn activity(app: &AppHandle, host: &Shared, kind: &'static str, tool: Option<String>, ok: Option<bool>) {
    let connections = host.conns.lock().unwrap().len();
    let _ = app.emit("mcp-activity", Activity { kind, tool, ok, connections });
}

fn serve(app: AppHandle, host: Arc<Shared>, stream: UnixStream) {
    let Ok(read) = stream.try_clone() else { return };
    let Ok(mut write) = stream.try_clone() else { return };
    let conn = host.conn_seq.fetch_add(1, Ordering::Relaxed);
    host.conns.lock().unwrap().insert(conn, stream);
    activity(&app, &host, "connected", None, None);
    for line in BufReader::new(read).lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Call>(&line) {
            Ok(call) => {
                let tool = call.tool.clone();
                let r = handle(&app, &host, call);
                if r.code.is_empty() {
                    activity(&app, &host, "call", Some(tool), Some(r.ok));
                }
                r
            }
            Err(e) => Reply::fail(0, "", format!("mensaje no válido: {e}")),
        };
        let Ok(mut json) = serde_json::to_string(&reply) else { break };
        json.push('\n');
        if write.write_all(json.as_bytes()).is_err() {
            break;
        }
    }
    // Esta conexión se va (la IA se cerró o el usuario la detuvo).
    if host.conns.lock().unwrap().remove(&conn).is_some() {
        activity(&app, &host, "disconnected", None, None);
    }
}

fn handle(app: &AppHandle, host: &Shared, call: Call) -> Reply {
    match *host.enabled.lock().unwrap() {
        None => return Reply::fail(call.id, "starting", "imgxplorer is still starting"),
        Some(false) => {
            return Reply::fail(
                call.id,
                "disabled",
                "AI control is turned off in imgxplorer. Ask the user to turn it on in imgxplorer: Settings → AI control.",
            );
        }
        Some(true) => {}
    }
    let id = host.next.fetch_add(1, Ordering::Relaxed) + 1;
    let (tx, rx) = channel();
    host.pending.lock().unwrap().insert(id, tx);
    let payload = serde_json::json!({ "id": id, "tool": call.tool, "args": call.args });
    if let Err(e) = app.emit("mcp-call", payload) {
        host.pending.lock().unwrap().remove(&id);
        return Reply::fail(call.id, "", e.to_string());
    }
    let got = rx.recv_timeout(timeout(&call.tool));
    host.pending.lock().unwrap().remove(&id);
    match got {
        Ok(r) if r.ok => {
            let images = r.images.iter().filter_map(|b| save_image(host, b)).collect();
            Reply { id: call.id, ok: true, text: r.text, images, ..Default::default() }
        }
        Ok(r) => Reply::fail(call.id, "", if r.error.is_empty() { "error".to_owned() } else { r.error }),
        Err(_) => Reply::fail(call.id, "", format!("{} took too long (the app did not answer)", call.tool)),
    }
}

/// Guarda una imagen del resultado y devuelve su ruta.
fn save_image(host: &Shared, data: &str) -> Option<String> {
    let dir = host.images.lock().unwrap().clone()?;
    let b64 = data.split_once(";base64,").map(|(_, b)| b).unwrap_or(data);
    let bytes = wire::unbase64(b64)?;
    let ext = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) { "png" } else { "jpg" };
    let n = host.image_seq.fetch_add(1, Ordering::Relaxed) + 1;
    let path = dir.join(format!("img-{n:05}.{ext}"));
    std::fs::write(&path, bytes).ok()?;
    if n > KEEP_IMAGES {
        for e in ["jpg", "png"] {
            let _ = std::fs::remove_file(dir.join(format!("img-{:05}.{e}", n - KEEP_IMAGES)));
        }
    }
    Some(path.to_string_lossy().into_owned())
}

/// La interfaz responde a una llamada.
#[tauri::command]
pub fn mcp_reply(id: u64, reply: UiReply, state: State<'_, Host>) {
    if let Some(tx) = state.0.pending.lock().unwrap().remove(&id) {
        let _ = tx.send(reply);
    }
}

/// La interfaz dice si el usuario permite el control por IA. Al apagarlo se corta la conexión.
#[tauri::command]
pub fn mcp_enable(app: AppHandle, enabled: bool, state: State<'_, Host>) {
    *state.0.enabled.lock().unwrap() = Some(enabled);
    if !enabled {
        let conns = std::mem::take(&mut *state.0.conns.lock().unwrap());
        for c in conns.into_values() {
            let _ = c.shutdown(std::net::Shutdown::Both);
        }
        activity(&app, &state.0, "disconnected", None, None);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInfo {
    /// Socket en el que escucha (None si otra imgxplorer ya lo tenía).
    socket: Option<String>,
    connections: usize,
}

#[tauri::command]
pub fn mcp_host_info(state: State<'_, Host>) -> HostInfo {
    HostInfo {
        socket: state.0.socket.lock().unwrap().as_ref().map(|p| p.to_string_lossy().into_owned()),
        connections: state.0.conns.lock().unwrap().len(),
    }
}
