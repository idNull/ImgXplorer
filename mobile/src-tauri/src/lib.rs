//! imgxplorer para el móvil: un visor de imágenes, cómics (cbz, cbr, cb7) e historias
//! `.imgxstory`, con su animación, sus efectos y su sonido.
//!
//! La fuente abierta, el protocolo `imgx://`, las portadas y el sonido son los del escritorio
//! (`crates/imgx-host`), con las mismas órdenes y los mismos nombres. Aquí solo se monta la app.

use imgx_host::{AppState, audio, covers, serve, source};
use serde::Serialize;

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
        .manage(AppState::default())
        .manage(audio::Audio::default())
        .register_asynchronous_uri_scheme_protocol("imgx", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_owned();
            // Leer y decodificar fuera del hilo principal: comprimidos y RAW pueden tardar.
            std::thread::spawn(move || responder.respond(serve::handle(&app, &path)));
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
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
