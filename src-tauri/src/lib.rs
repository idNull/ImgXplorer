//! Aplicación Tauri: fuentes abiertas, protocolo `imgx://` y puente de gestos.

mod diagnostics;
mod gestures;
mod serve;

use imgx_media::{Source, SourceKind};
use serde::Serialize;
use std::sync::{Arc, RwLock};
use tauri::{Manager, RunEvent, State};

#[derive(Default)]
pub(crate) struct AppState {
    /// Fuente abierta y su "generación" (cambia en cada apertura para invalidar cachés del webview).
    source: RwLock<Option<(u64, Arc<dyn Source>)>>,
    thumbs: serve::ThumbCache,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntryInfo {
    name: String,
    size: u64,
    /// El webview no sabe mostrarla: se sirve como RGBA decodificado en Rust.
    decoded: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceInfo {
    generation: u64,
    title: String,
    path: String,
    kind: &'static str,
    /// Imagen en la que situarse al abrir (la pedida, si se abrió una imagen suelta).
    start: usize,
    entries: Vec<EntryInfo>,
}

#[tauri::command]
async fn open_source(path: String, state: State<'_, AppState>) -> Result<SourceInfo, String> {
    let requested = std::path::PathBuf::from(&path);
    // Listar un comprimido grande puede tardar: fuera del hilo principal.
    let opened = tauri::async_runtime::spawn_blocking(move || imgx_media::open_path(&requested))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| {
            log::warn!("no se pudo abrir {path}: {e}");
            e.to_string()
        })?;
    let src = opened.source;
    log::info!("abierta {} ({:?}, {} imágenes, inicio {})", opened.path.display(), opened.kind, src.entries().len(), opened.start);
    let mut guard = state.source.write().unwrap();
    let generation = guard.as_ref().map_or(1, |(g, _)| g + 1);
    let info = SourceInfo {
        generation,
        title: src.title().to_owned(),
        path: opened.path.display().to_string(),
        kind: match opened.kind {
            SourceKind::Folder => "folder",
            SourceKind::Archive => "archive",
        },
        start: opened.start,
        entries: src
            .entries()
            .iter()
            .map(|e| EntryInfo { name: e.name.clone(), size: e.size, decoded: imgx_media::needs_decode(&e.name) })
            .collect(),
    };
    *guard = Some((generation, src));
    state.thumbs.clear();
    Ok(info)
}

/// Ruta pasada por línea de comandos (`imgxplorer <carpeta|comprimido|imagen>`).
#[tauri::command]
fn initial_path() -> Option<String> {
    std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).find(|a| !a.starts_with('-'))
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
            // Leer y decodificar fuera del hilo principal: comprimidos y RAW pueden tardar.
            std::thread::spawn(move || responder.respond(serve::handle(&app, &path)));
        })
        .invoke_handler(tauri::generate_handler![
            open_source,
            initial_path,
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
