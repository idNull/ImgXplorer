//! Aplicación Tauri de escritorio. Las fuentes abiertas, el protocolo `imgx://`, las portadas,
//! los proyectos y el sonido están en `crates/imgx-host` (compartido con el visor móvil).

mod diagnostics;
mod export;
mod gestures;
pub mod mcp;
mod browse;
mod pack;
mod prefs;
mod comic;
mod story;
mod utils;
mod voices;

// Lo común con el visor móvil vive en `crates/imgx-host`; se reexporta para que el resto del
// escritorio siga usando las mismas rutas (`crate::AppState`, `crate::audio::Audio`…).
pub(crate) use imgx_host::{AppState, Current, audio, covers, project, serve};
use std::sync::Mutex;
use tauri::{Manager, RunEvent};

/// Archivo que macOS pidió abrir antes de que la interfaz estuviera lista (doble clic en Finder).
static OPENED: Mutex<Option<String>> = Mutex::new(None);

/// Ruta a abrir al arrancar: la de la línea de comandos (`imgxplorer <carpeta|comprimido|imagen>`)
/// o la que pidió el sistema al abrir un archivo asociado.
#[tauri::command]
fn initial_path() -> Option<String> {
    OPENED.lock().unwrap().take().or_else(|| std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).find(|a| !a.starts_with('-')))
}

/// `--version` y `--self-check` (para el instalador): responden sin abrir ventana. Que el programa
/// llegue a ejecutarse ya demuestra que el sistema tiene todas las bibliotecas que necesita.
pub fn cli() -> Option<String> {
    let arg = std::env::args().nth(1)?;
    let version = format!("imgxplorer {}", env!("CARGO_PKG_VERSION"));
    match arg.as_str() {
        "--version" | "-V" => Some(version),
        // Conectar o quitar imgxplorer de las IAs (Claude Code, Codex, Gemini CLI).
        "--mcp-install" | "--mcp-uninstall" => Some(mcp::install::cli(arg == "--mcp-install", std::env::args().nth(2).as_deref())),
        "--self-check" => {
            let (heif, hevc, av1) = imgx_media::heif_support();
            let yn = |b: bool| if b { "si" } else { "no" };
            let mut out = format!("{version}\nlibheif={heif}\nheic={}\navif={}", yn(hevc), yn(av1));
            if let Some(v) = diagnostics::webkit_version() {
                out += &format!("\nwebkitgtk={v}");
            }
            Some(out)
        }
        _ => None,
    }
}

/// Variables de WebKitGTK que cambian cómo dibuja. Si el usuario ya puso alguna, no se toca nada.
#[cfg(target_os = "linux")]
const WEBKIT_RENDER_VARS: [&str; 4] =
    ["WEBKIT_DISABLE_DMABUF_RENDERER", "WEBKIT_DMABUF_RENDERER_FORCE_SHM", "WEBKIT_DISABLE_COMPOSITING_MODE", "__NV_DISABLE_EXPLICIT_SYNC"];

/// ¿Alguna gráfica usa el controlador propietario de Nvidia? Se mira en sysfs, que también se ve
/// dentro de un AppImage o un Flatpak; nouveau no cuenta (no tiene el problema).
#[cfg(target_os = "linux")]
fn nvidia_driver() -> bool {
    let Ok(cards) = std::fs::read_dir("/sys/class/drm") else { return false };
    cards.flatten().any(|c| {
        let name = c.file_name();
        let name = name.to_string_lossy();
        name.starts_with("card")
            && !name.contains('-')
            && std::fs::read_link(c.path().join("device/driver")).ok().and_then(|d| d.file_name().map(|n| n == "nvidia")).unwrap_or(false)
    })
}

/// Con el controlador de Nvidia, el renderizado DMA-BUF de WebKitGTK deja ventanas en blanco o hace
/// caer el proceso web (en Ubuntu 26.04 con WebKitGTK 2.52: «WebKitWebProcess crashed with SIGFPE»).
/// Se desactiva aquí, antes de que arranque GTK, para que valga se abra como se abra la app.
/// Devuelve si se aplicó.
#[cfg(target_os = "linux")]
fn nvidia_quirk() -> bool {
    if WEBKIT_RENDER_VARS.iter().any(|v| std::env::var_os(v).is_some()) || !nvidia_driver() {
        return false;
    }
    // SAFETY: se llama al principio de `run`, antes de crear hilos o iniciar GTK/WebKit.
    unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    true
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    let nvidia = nvidia_quirk();
    diagnostics::install_panic_hook();
    let app = tauri::Builder::default()
        .plugin(diagnostics::log_plugin())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState::default())
        .manage(diagnostics::Diagnostics::default())
        .manage(export::Exporter::default())
        .manage(audio::Audio::default())
        .manage(story::StoryExport::default())
        .manage(voices::Voices::default())
        .manage(mcp::host::Host::default())
        .register_asynchronous_uri_scheme_protocol("imgx", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_owned();
            // Leer y decodificar fuera del hilo principal: comprimidos y RAW pueden tardar.
            std::thread::spawn(move || responder.respond(serve::handle(&app, &path)));
        })
        .invoke_handler(tauri::generate_handler![
            imgx_host::source::open_source,
            imgx_host::source::close_source,
            initial_path,
            imgx_host::source::detect_panels,
            imgx_host::source::image_levels,
            imgx_host::source::content_box,
            covers::cover,
            covers::forget_cover,
            utils::show_in_folder,
            utils::image_info,
            utils::copy_image,
            utils::save_image,
            utils::save_original,
            utils::reveal_entry,
            utils::trash_entry,
            project::save_project,
            prefs::load_prefs,
            prefs::save_prefs,
            prefs::paths_exist,
            pack::export_pack,
            pack::import_pack,
            story::story_export,
            comic::comic_default_dir,
            comic::comic_create,
            comic::comic_from_source,
            comic::comic_arrange,
            comic::thumb_file,
            story::story_cancel,
            story::story_estimate,
            browse::list_dir,
            browse::places,
            export::ffmpeg_info,
            export::export_start,
            export::export_frame,
            export::export_finish,
            export::export_cancel,
            audio::sound_list,
            audio::sound_import,
            audio::sound_rename,
            audio::sound_delete,
            audio::sound_keep,
            audio::sound_peaks,
            audio::sound_preview,
            audio::audio_scene,
            audio::audio_ambient,
            audio::audio_transport,
            audio::audio_tracks,
            audio::audio_levels,
            audio::audio_silence,
            audio::audio_status,
            audio::audio_inputs,
            audio::record_start,
            audio::record_level,
            audio::record_stop,
            voices::voices_key_status,
            voices::voices_key_set,
            voices::voices_key_clear,
            voices::voices_account,
            voices::voices_list,
            voices::voices_design,
            voices::voices_preview,
            voices::voices_save_design,
            voices::voices_estimate,
            voices::voices_generate,
            mcp::host::mcp_reply,
            mcp::host::mcp_enable,
            mcp::host::mcp_host_info,
            mcp::tools::mcp_find_audio,
            mcp::tools::mcp_sound_report,
            mcp::tools::mcp_inspect_video,
            mcp::install::mcp_clients,
            mcp::install::mcp_client_action,
            diagnostics::diagnostics_info,
            diagnostics::dismiss_crash_notice,
            diagnostics::open_url
        ])
        .setup(move |app| {
            diagnostics::start_session(app.handle());
            #[cfg(target_os = "linux")]
            if nvidia {
                log::info!("controlador de Nvidia: WEBKIT_DISABLE_DMABUF_RENDERER=1");
            }
            let window = app.get_webview_window("main").expect("ventana main");
            diagnostics::watch_webview(&window);
            gestures::install(&window);
            mcp::host::start(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error al iniciar imgxplorer");
    app.run(|app, event| match event {
        RunEvent::Exit => {
            mcp::host::stop(app);
            diagnostics::end_session(app);
        }
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        RunEvent::Opened { urls } => {
            if let Some(path) = urls.iter().find_map(|u| u.to_file_path().ok()) {
                let path = path.to_string_lossy().into_owned();
                // Si la interfaz ya está en marcha se le avisa; si no, lo recoge `initial_path`.
                *OPENED.lock().unwrap() = Some(path.clone());
                use tauri::Emitter;
                let _ = app.emit("open-path", path);
            }
        }
        _ => {}
    });
}
