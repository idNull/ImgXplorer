//! Aplicación Tauri: fuentes abiertas, protocolo `imgx://` y puente de gestos.

mod audio;
mod diagnostics;
mod export;
mod gestures;
pub mod mcp;
mod browse;
mod pack;
mod prefs;
mod project;
mod serve;

use imgx_media::panels::PanelRect;
use imgx_media::{Source, SourceKind};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Manager, RunEvent, State};

/// Fuente abierta.
#[derive(Clone)]
pub(crate) struct Current {
    /// Cambia en cada apertura para invalidar cachés del webview.
    generation: u64,
    source: Arc<dyn Source>,
    path: PathBuf,
    kind: SourceKind,
    fingerprint: String,
}

#[derive(Default)]
pub(crate) struct AppState {
    current: RwLock<Option<Current>>,
    thumbs: serve::ThumbCache,
    /// Viñetas detectadas por imagen de la fuente abierta.
    panels: Mutex<HashMap<usize, Vec<PanelRect>>>,
    /// Niveles («limpiar escaneo») por imagen de la fuente abierta.
    levels: Mutex<HashMap<usize, imgx_media::levels::Levels>>,
    /// Recuadro con contenido («✂ ajustar al contenido») por imagen de la fuente abierta.
    boxes: Mutex<HashMap<usize, Option<PanelRect>>>,
    /// Última generación usada; no se reinicia al cerrar para que las URL nunca se repitan.
    last_generation: std::sync::atomic::AtomicU64,
}

impl AppState {
    /// La fuente abierta, si sigue siendo la de esa generación.
    pub(crate) fn current(&self, generation: u64) -> Option<Current> {
        self.current.read().unwrap().as_ref().filter(|c| c.generation == generation).cloned()
    }
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
    /// Se abrió una imagen concreta: manda sobre la página recordada.
    picked: bool,
    entries: Vec<EntryInfo>,
    fingerprint: String,
    /// Animaciones guardadas de esta fuente.
    project: project::Loaded,
}

#[tauri::command]
async fn open_source(app: AppHandle, path: String, state: State<'_, AppState>) -> Result<SourceInfo, String> {
    // Ruta canónica: la misma fuente abierta de distintas formas se recuerda una sola vez.
    let requested = std::fs::canonicalize(&path).unwrap_or_else(|_| PathBuf::from(&path));
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
    let fingerprint = imgx_media::fingerprint(src.entries());
    let loaded = project::load(&app, &opened.path, opened.kind, &fingerprint);
    let mut guard = state.current.write().unwrap();
    let generation = state.last_generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let info = SourceInfo {
        generation,
        title: src.title().to_owned(),
        path: opened.path.display().to_string(),
        kind: match opened.kind {
            SourceKind::Folder => "folder",
            SourceKind::Archive => "archive",
        },
        start: opened.start,
        picked: opened.picked,
        entries: src
            .entries()
            .iter()
            .map(|e| EntryInfo { name: e.name.clone(), size: e.size, decoded: imgx_media::needs_decode(&e.name) })
            .collect(),
        fingerprint: fingerprint.clone(),
        project: loaded,
    };
    *guard = Some(Current { generation, source: src, path: opened.path, kind: opened.kind, fingerprint });
    state.thumbs.clear();
    state.panels.lock().unwrap().clear();
    state.levels.lock().unwrap().clear();
    state.boxes.lock().unwrap().clear();
    Ok(info)
}

/// Cierra la fuente abierta (vuelve a la pantalla de inicio).
#[tauri::command]
fn close_source(state: State<'_, AppState>) {
    if let Some(c) = state.current.write().unwrap().take() {
        log::info!("cerrada {}", c.path.display());
    }
    state.thumbs.clear();
    state.panels.lock().unwrap().clear();
    state.levels.lock().unwrap().clear();
    state.boxes.lock().unwrap().clear();
}

/// Viñetas de la imagen `index` (vacío si no se distinguen al menos dos).
#[tauri::command]
async fn detect_panels(generation: u64, index: usize, state: State<'_, AppState>) -> Result<Vec<PanelRect>, String> {
    let cur = state.current(generation).ok_or("la fuente ya no está abierta")?;
    if let Some(p) = state.panels.lock().unwrap().get(&index) {
        return Ok(p.clone());
    }
    let src = cur.source.clone();
    let found = tauri::async_runtime::spawn_blocking(move || {
        let t0 = std::time::Instant::now();
        let name = src.entries().get(index).ok_or("índice fuera de rango")?.name.clone();
        let bytes = src.read(index).map_err(|e| e.to_string())?;
        let found = imgx_media::panels::detect_panels(&name, &bytes).map_err(|e| e.to_string())?;
        log::debug!("viñetas de {name}: {} en {} ms", found.len(), t0.elapsed().as_millis());
        Ok::<_, String>(found)
    })
    .await
    .map_err(|e| e.to_string())??;
    if state.current(generation).is_some() {
        state.panels.lock().unwrap().insert(index, found.clone());
    }
    Ok(found)
}

/// Color del papel y de la tinta de la imagen `index`, para «limpiar escaneo».
#[tauri::command]
async fn image_levels(generation: u64, index: usize, state: State<'_, AppState>) -> Result<imgx_media::levels::Levels, String> {
    let cur = state.current(generation).ok_or("la fuente ya no está abierta")?;
    if let Some(l) = state.levels.lock().unwrap().get(&index) {
        return Ok(*l);
    }
    let src = cur.source.clone();
    let found = tauri::async_runtime::spawn_blocking(move || {
        let name = src.entries().get(index).ok_or("índice fuera de rango")?.name.clone();
        let bytes = src.read(index).map_err(|e| e.to_string())?;
        imgx_media::levels::levels(&name, &bytes).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    if state.current(generation).is_some() {
        state.levels.lock().unwrap().insert(index, found);
    }
    Ok(found)
}

/// Recuadro con contenido de la imagen `index` (`None` si apenas tiene márgenes).
#[tauri::command]
async fn content_box(generation: u64, index: usize, state: State<'_, AppState>) -> Result<Option<PanelRect>, String> {
    let cur = state.current(generation).ok_or("la fuente ya no está abierta")?;
    if let Some(b) = state.boxes.lock().unwrap().get(&index) {
        return Ok(*b);
    }
    let src = cur.source.clone();
    let found = tauri::async_runtime::spawn_blocking(move || {
        let name = src.entries().get(index).ok_or("índice fuera de rango")?.name.clone();
        let bytes = src.read(index).map_err(|e| e.to_string())?;
        imgx_media::panels::content_box(&name, &bytes).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    if state.current(generation).is_some() {
        state.boxes.lock().unwrap().insert(index, found);
    }
    Ok(found)
}

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
        .manage(AppState::default())
        .manage(diagnostics::Diagnostics::default())
        .manage(export::Exporter::default())
        .manage(audio::Audio::default())
        .manage(mcp::host::Host::default())
        .register_asynchronous_uri_scheme_protocol("imgx", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_owned();
            // Leer y decodificar fuera del hilo principal: comprimidos y RAW pueden tardar.
            std::thread::spawn(move || responder.respond(serve::handle(&app, &path)));
        })
        .invoke_handler(tauri::generate_handler![
            open_source,
            close_source,
            initial_path,
            detect_panels,
            image_levels,
            content_box,
            project::save_project,
            prefs::load_prefs,
            prefs::save_prefs,
            prefs::paths_exist,
            pack::export_pack,
            pack::import_pack,
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
