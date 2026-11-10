//! Registro de errores y diagnóstico de cierres inesperados.
//!
//! - Registro en archivo con rotación (`<log_dir>/imgxplorer.log`), también por stdout.
//!   La interfaz envía sus errores al mismo archivo (`@tauri-apps/plugin-log`).
//! - Los *panics* de Rust se registran con backtrace antes de abortar.
//! - Un fallo nativo (SIGSEGV en GTK/WebKit) no se puede registrar en el instante en que
//!   ocurre, así que dejamos un marcador `session.lock` al arrancar y lo borramos al salir
//!   bien. Si al arrancar sigue ahí, la sesión anterior murió: se avisa al usuario y se
//!   registra, junto con las últimas "migas" (breadcrumbs) que quedaron en el log.
//! - En Linux se vigila el proceso web de WebKit: si muere, se registra y se recarga.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, Runtime};

const MARKER: &str = "session.lock";

pub fn log_plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    use log::LevelFilter;
    use tauri_plugin_log::{Builder, RotationStrategy, Target, TargetKind, TimezoneStrategy};
    Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: Some("imgxplorer".into()) }),
        ])
        .level(LevelFilter::Info)
        .level_for("imgxplorer_lib", LevelFilter::Debug)
        .level_for("imgx_media", LevelFilter::Debug)
        .level_for("imgx_core", LevelFilter::Debug)
        .max_file_size(2_000_000)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build()
}

pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::force_capture();
        log::error!("PANIC: {info}\n{bt}");
        default(info);
    }));
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviousCrash {
    pub pid: u32,
    /// Segundos UNIX en que arrancó la sesión que murió.
    pub started: u64,
}

#[derive(Default)]
pub struct Diagnostics {
    log_dir: Mutex<Option<PathBuf>>,
    previous_crash: Mutex<Option<PreviousCrash>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsInfo {
    log_dir: Option<String>,
    previous_crash: Option<PreviousCrash>,
    os: &'static str,
    arch: &'static str,
    session_type: Option<String>,
}

/// Registra el entorno y comprueba si la sesión anterior terminó de forma inesperada.
pub fn start_session(app: &AppHandle) {
    let state = app.state::<Diagnostics>();
    log::info!(
        "imgxplorer {} · {} {} · sesión {} · escritorio {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "?".into()),
        std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "?".into()),
    );
    #[cfg(target_os = "linux")]
    {
        // SAFETY: funciones sin argumentos que solo devuelven constantes de la librería.
        let (ma, mi, pa) = unsafe {
            use webkit2gtk::ffi as w;
            (w::webkit_get_major_version(), w::webkit_get_minor_version(), w::webkit_get_micro_version())
        };
        log::info!("WebKitGTK {ma}.{mi}.{pa}");
    }

    let Ok(dir) = app.path().app_log_dir() else {
        log::warn!("sin directorio de registros; no se detectarán cierres inesperados");
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let marker = dir.join(MARKER);
    if let Ok(text) = std::fs::read_to_string(&marker) {
        let field = |k: &str| text.split_whitespace().find_map(|kv| kv.strip_prefix(k)?.parse::<u64>().ok());
        let crash = PreviousCrash { pid: field("pid=").unwrap_or(0) as u32, started: field("start=").unwrap_or(0) };
        log::error!(
            "La sesión anterior (pid {}) terminó de forma inesperada. Las últimas líneas antes de esta \
             sesión en el registro muestran qué estaba ocurriendo{}",
            crash.pid,
            if cfg!(target_os = "linux") { format!("; volcado nativo: `coredumpctl info {}`", crash.pid) } else { String::new() }
        );
        *state.previous_crash.lock().unwrap() = Some(crash);
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if let Err(e) = std::fs::write(&marker, format!("pid={} start={now}", std::process::id())) {
        log::warn!("no se pudo escribir {}: {e}", marker.display());
    }
    log::info!("registros en {}", dir.display());
    *state.log_dir.lock().unwrap() = Some(dir);
}

/// Salida limpia: borra el marcador de sesión.
pub fn end_session(app: &AppHandle) {
    if let Some(dir) = app.state::<Diagnostics>().log_dir.lock().unwrap().as_ref() {
        let _ = std::fs::remove_file(dir.join(MARKER));
    }
    log::info!("salida normal");
}

#[cfg(target_os = "linux")]
pub fn watch_webview(window: &tauri::WebviewWindow) {
    use webkit2gtk::{WebProcessTerminationReason as R, WebViewExt};
    let result = window.with_webview(|wv| {
        wv.inner().connect_web_process_terminated(|view, reason| {
            let why = match reason {
                R::Crashed => "fallo (crash)",
                R::ExceededMemoryLimit => "límite de memoria excedido",
                R::TerminatedByApi => "terminado por la API",
                _ => "desconocido",
            };
            log::error!("el proceso web de WebKit terminó: {why}; recargando la interfaz");
            view.reload();
        });
    });
    if let Err(e) = result {
        log::warn!("no se pudo vigilar el proceso web: {e}");
    }
}

#[cfg(not(target_os = "linux"))]
pub fn watch_webview(_window: &tauri::WebviewWindow) {}

#[tauri::command]
pub fn diagnostics_info(state: tauri::State<'_, Diagnostics>) -> DiagnosticsInfo {
    DiagnosticsInfo {
        log_dir: state.log_dir.lock().unwrap().as_ref().map(|p| p.display().to_string()),
        previous_crash: state.previous_crash.lock().unwrap().clone(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        session_type: std::env::var("XDG_SESSION_TYPE").ok(),
    }
}

/// Descarta el aviso de cierre inesperado (ya mostrado al usuario).
#[tauri::command]
pub fn dismiss_crash_notice(state: tauri::State<'_, Diagnostics>) {
    *state.previous_crash.lock().unwrap() = None;
}
