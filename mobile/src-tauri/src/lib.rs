//! imgxplorer para el móvil: un visor de imágenes, cómics (cbz, cbr, cb7) e historias
//! `.imgxstory`, con su animación, sus efectos y su sonido.
//!
//! La fuente abierta, el protocolo `imgx://`, las portadas y el sonido son los del escritorio
//! (`crates/imgx-host`), con las mismas órdenes y los mismos nombres. Aquí solo se monta la app.

mod library;

use imgx_host::{AppState, audio, covers, serve, source};
use library::{Imported, Incoming, Item, Library};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_imgx_mobile::{ImgxMobileExt, Picked};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    os: &'static str,
    arch: &'static str,
    /// Se compiló con libheif (en el móvil, no).
    heif: bool,
    /// Se compiló con UnRAR (para `.cbr`).
    rar: bool,
}

/// Versión, sistema y formatos opcionales: lo enseña la pantalla de comprobación.
#[tauri::command]
fn app_info() -> AppInfo {
    log::info!("visor {} en {} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH);
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        heif: imgx_media::HAS_HEIF,
        rar: imgx_media::HAS_RAR,
    }
}

// ------------------------------------------------------------------ biblioteca

/// Carpeta nueva y vacía para una importación (en la caché: se borra al terminar).
fn import_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    Ok(app.path().app_cache_dir().map_err(|e| e.to_string())?.join("import").join(format!("{ms:x}")))
}

/// Copia, importa y limpia: `fetch` deja los archivos en la carpeta de importación.
async fn import_with(
    app: AppHandle,
    album_title: String,
    fetch: impl FnOnce(&AppHandle, &std::path::Path) -> Result<Vec<Picked>, String> + Send + 'static,
) -> Result<Imported, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = import_dir(&app)?;
        let picked = fetch(&app, &dir);
        let result = picked.map(|files| {
            let incoming = files.into_iter().map(|p| Incoming { path: p.path, name: p.name }).collect();
            app.state::<Library>().import(incoming, &album_title)
        });
        let _ = std::fs::remove_dir_all(&dir);
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn library_list(lib: State<'_, Library>) -> Vec<Item> {
    lib.list()
}

/// Elegir fotos del carrete (`images`) o archivos (`documents`) y meterlos en la biblioteca.
#[tauri::command]
async fn library_pick(app: AppHandle, kind: String, album_title: String) -> Result<Imported, String> {
    import_with(app, album_title, move |app, dir| match kind.as_str() {
        "images" => app.imgx_mobile().pick_images(dir),
        _ => app.imgx_mobile().pick_documents(dir),
    })
    .await
}

/// Lo que otra app ha pedido abrir con imgxplorer («Abrir con», «Compartir»).
#[tauri::command]
async fn library_take_opened(app: AppHandle, album_title: String) -> Result<Imported, String> {
    import_with(app, album_title, |app, dir| app.imgx_mobile().take_opened(dir)).await
}

/// Importar rutas del propio sistema de archivos (sin copiar el original). Sirve para probar la
/// interfaz como ventana en el ordenador, donde no hay selector del móvil.
#[tauri::command]
async fn library_import_paths(app: AppHandle, paths: Vec<String>, album_title: String) -> Result<Imported, String> {
    import_with(app, album_title, move |_, dir| {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        paths
            .iter()
            .map(|p| {
                let src = PathBuf::from(p);
                let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("ruta sin nombre")?;
                let dest = dir.join(&name);
                std::fs::copy(&src, &dest).map_err(|e| format!("{p}: {e}"))?;
                Ok(Picked { path: dest, name })
            })
            .collect()
    })
    .await
}

#[tauri::command]
fn library_progress(id: String, page: usize, lib: State<'_, Library>) -> Result<(), String> {
    lib.progress(&id, page)
}

#[tauri::command]
fn library_remove(app: AppHandle, id: String, lib: State<'_, Library>) -> Result<(), String> {
    if let Some(path) = lib.remove(&id)? {
        covers::forget_cover(app, path);
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .level_for("imgxplorer_mobile_lib", log::LevelFilter::Debug)
                .level_for("imgx_host", log::LevelFilter::Debug)
                .level_for("imgx_media", log::LevelFilter::Debug)
                .build(),
        )
        .plugin(tauri_plugin_imgx_mobile::init())
        .manage(AppState::default())
        .manage(audio::Audio::default())
        .setup(|app| {
            let root = app.path().app_data_dir()?.join("library");
            log::info!("biblioteca en {}", root.display());
            app.manage(Library::new(root));
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("imgx", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_owned();
            // Leer y decodificar fuera del hilo principal: comprimidos y RAW pueden tardar.
            std::thread::spawn(move || responder.respond(serve::handle(&app, &path)));
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            library_list,
            library_pick,
            library_take_opened,
            library_import_paths,
            library_progress,
            library_remove,
            source::open_source,
            source::close_source,
            source::detect_panels,
            source::image_levels,
            source::content_box,
            covers::cover,
            covers::forget_cover,
            audio::audio_scene,
            audio::audio_ambient,
            audio::audio_transport,
            audio::audio_tracks,
            audio::audio_levels,
            audio::audio_silence,
            audio::audio_status
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar imgxplorer");
}
