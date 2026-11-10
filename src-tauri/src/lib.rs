//! Aplicación Tauri: fuentes abiertas, protocolo `imgx://` y puente de gestos.

mod diagnostics;
mod gestures;

use imgx_media::{FolderSource, Source};
use serde::Serialize;
use std::sync::{Arc, RwLock};
use tauri::http::{Response, StatusCode};
use tauri::{Manager, RunEvent, State};

#[derive(Default)]
struct AppState {
    /// Fuente abierta y su "generación" (cambia en cada apertura para invalidar cachés del webview).
    source: RwLock<Option<(u64, Arc<dyn Source>)>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntryInfo {
    name: String,
    size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceInfo {
    generation: u64,
    title: String,
    entries: Vec<EntryInfo>,
}

#[tauri::command]
fn open_source(path: String, state: State<'_, AppState>) -> Result<SourceInfo, String> {
    let path = std::path::PathBuf::from(path);
    // Fase 0: solo carpetas. Una imagen suelta abre su carpeta (TODO: posicionarse en ella).
    let dir = if path.is_file() { path.parent().map(|p| p.to_path_buf()).unwrap_or(path) } else { path };
    let src = FolderSource::open(&dir).map_err(|e| {
        log::warn!("no se pudo abrir {}: {e}", dir.display());
        e.to_string()
    })?;
    log::info!("abierta {} ({} imágenes)", dir.display(), src.entries().len());
    let mut guard = state.source.write().unwrap();
    let generation = guard.as_ref().map_or(1, |(g, _)| g + 1);
    let info = SourceInfo {
        generation,
        title: src.title().to_owned(),
        entries: src.entries().iter().map(|e| EntryInfo { name: e.name.clone(), size: e.size }).collect(),
    };
    *guard = Some((generation, Arc::new(src)));
    Ok(info)
}

/// `imgx://localhost/<generación>_<índice>` → bytes de la imagen.
fn serve(app: &tauri::AppHandle, path: &str) -> Response<Vec<u8>> {
    let not_found = |msg: String| {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Access-Control-Allow-Origin", "*")
            .body(msg.into_bytes())
            .unwrap()
    };
    let Some((generation, index)) = path
        .trim_start_matches('/')
        .split_once('_')
        .and_then(|(g, i)| Some((g.parse::<u64>().ok()?, i.parse::<usize>().ok()?)))
    else {
        log::warn!("imgx://: ruta inválida {path}");
        return not_found(format!("ruta inválida: {path}"));
    };
    let state = app.state::<AppState>();
    let source = {
        let guard = state.source.read().unwrap();
        match guard.as_ref() {
            Some((g, s)) if *g == generation => s.clone(),
            _ => return not_found("fuente cerrada".into()),
        }
    };
    let Some(entry) = source.entries().get(index) else { return not_found(format!("índice {index}")) };
    match source.read(index) {
        Ok(bytes) => Response::builder()
            .header("Content-Type", imgx_media::mime_for(&entry.name))
            .header("Access-Control-Allow-Origin", "*")
            .header("Cache-Control", "max-age=31536000, immutable")
            .body(bytes)
            .unwrap(),
        Err(e) => {
            log::warn!("error leyendo {}: {e}", entry.name);
            not_found(e.to_string())
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    diagnostics::install_panic_hook();
    let app = tauri::Builder::default()
        .plugin(diagnostics::log_plugin())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .manage(diagnostics::Diagnostics::default())
        .register_asynchronous_uri_scheme_protocol("imgx", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_owned();
            // Leer fuera del hilo principal: los comprimidos pueden tardar.
            std::thread::spawn(move || responder.respond(serve(&app, &path)));
        })
        .invoke_handler(tauri::generate_handler![
            open_source,
            diagnostics::diagnostics_info,
            diagnostics::dismiss_crash_notice
        ])
        .setup(|app| {
            diagnostics::start_session(app.handle());
            let window = app.get_webview_window("main").expect("ventana main");
            diagnostics::watch_webview(&window);
            gestures::install(&window);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error al iniciar imgxplorer");
    app.run(|app, event| {
        if let RunEvent::Exit = event {
            diagnostics::end_session(app);
        }
    });
}
