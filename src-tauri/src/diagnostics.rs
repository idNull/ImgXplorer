//! Registro de errores y diagnóstico de cierres inesperados.
//!
//! - Registro en archivo con rotación (`<log_dir>/imgxplorer.log`), también por stdout.
//!   La interfaz envía sus errores al mismo archivo (`@tauri-apps/plugin-log`).
//! - Los *panics* de Rust se registran con backtrace antes de abortar.
//! - Un fallo nativo (SIGSEGV en GTK/WebKit) no se puede registrar con normalidad en el
//!   instante en que ocurre. Dejamos un marcador `session.lock` al arrancar y lo borramos al
//!   salir bien; un manejador de señales fatales (y el hook de panic) anota en él la causa con
//!   una sola llamada `write` (segura dentro de un manejador de señal). Al arrancar:
//!   - marcador con causa → la sesión anterior falló: aviso al usuario + registro;
//!   - marcador sin causa → se terminó a la fuerza (SIGKILL, p. ej. al recompilar en
//!     desarrollo, o apagado): solo una línea informativa.
//! - En Linux se vigila el proceso web de WebKit: si muere, se registra y se recarga.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, Runtime};

const MARKER: &str = "session.lock";

/// Descriptor del marcador abierto en modo *append*, para escribir desde el manejador de señales.
static MARKER_FD: AtomicI32 = AtomicI32::new(-1);

/// Añade `text` al marcador. Solo usa `write(2)`: apto para manejadores de señal.
fn mark(text: &[u8]) {
    #[cfg(unix)]
    {
        let fd = MARKER_FD.load(Ordering::Relaxed);
        if fd >= 0 {
            // SAFETY: fd abierto por nosotros y nunca cerrado; el buffer es válido.
            unsafe { libc::write(fd, text.as_ptr().cast(), text.len()) };
        }
    }
    #[cfg(not(unix))]
    let _ = text;
}

#[cfg(unix)]
fn install_signal_handlers() {
    extern "C" fn on_fatal(sig: libc::c_int) {
        let msg: &[u8] = match sig {
            libc::SIGSEGV => b" crash=SIGSEGV",
            libc::SIGBUS => b" crash=SIGBUS",
            libc::SIGILL => b" crash=SIGILL",
            libc::SIGFPE => b" crash=SIGFPE",
            libc::SIGABRT => b" crash=SIGABRT",
            _ => b" crash=signal",
        };
        static ONCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !ONCE.swap(true, Ordering::SeqCst) {
            mark(msg);
        }
        // JavaScriptCore instala su propio manejador encima y encadena al nuestro llamándolo
        // directamente, así que SA_RESETHAND no basta: restauramos la acción por defecto a mano
        // y relanzamos. La señal queda pendiente (está bloqueada dentro del manejador) y, al
        // volver, termina el proceso con volcado; también si llegó con `kill` sin fallo real.
        // SAFETY: sigaction y raise son async-signal-safe.
        unsafe {
            let mut dfl: libc::sigaction = std::mem::zeroed();
            dfl.sa_sigaction = libc::SIG_DFL;
            libc::sigaction(sig, &dfl, std::ptr::null_mut());
            libc::raise(sig);
        }
    }
    for sig in [libc::SIGSEGV, libc::SIGBUS, libc::SIGILL, libc::SIGFPE, libc::SIGABRT] {
        // SAFETY: estructura inicializada a cero y manejador `extern "C"` async-signal-safe.
        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = on_fatal as *const () as usize;
            // SA_ONSTACK: usa la pila alternativa que Rust instala (desbordamientos de pila).
            sa.sa_flags = libc::SA_RESETHAND | libc::SA_ONSTACK;
            libc::sigemptyset(&mut sa.sa_mask);
            libc::sigaction(sig, &sa, std::ptr::null_mut());
        }
    }
}

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
        mark(b" crash=panic");
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
    /// Causa anotada: `SIGSEGV`, `SIGABRT`, `panic`…
    pub reason: String,
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
    webkit: Option<String>,
}

/// Versión de WebKitGTK con la que se dibuja la interfaz (solo en Linux; macOS usa el WebKit del sistema).
pub fn webkit_version() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: funciones sin argumentos que solo devuelven constantes de la biblioteca.
        let (ma, mi, pa) = unsafe {
            use webkit2gtk::ffi as w;
            (w::webkit_get_major_version(), w::webkit_get_minor_version(), w::webkit_get_micro_version())
        };
        Some(format!("{ma}.{mi}.{pa}"))
    }
    #[cfg(not(target_os = "linux"))]
    None
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
    if let Some(v) = webkit_version() {
        log::info!("WebKitGTK {v}");
    }

    let Ok(dir) = app.path().app_log_dir() else {
        log::warn!("sin directorio de registros; no se detectarán cierres inesperados");
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let marker = dir.join(MARKER);
    if let Ok(text) = std::fs::read_to_string(&marker) {
        let field = |k: &str| text.split_whitespace().find_map(|kv| kv.strip_prefix(k));
        let num = |k: &str| field(k).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        let pid = num("pid=") as u32;
        match field("crash=") {
            Some(reason) => {
                log::error!(
                    "La sesión anterior (pid {pid}) falló ({reason}). Las últimas líneas antes de esta \
                     sesión en el registro muestran qué estaba ocurriendo{}",
                    if cfg!(target_os = "linux") { format!("; volcado nativo: `coredumpctl info {pid}`") } else { String::new() }
                );
                let crash = PreviousCrash { pid, started: num("start="), reason: reason.to_owned() };
                *state.previous_crash.lock().unwrap() = Some(crash);
            }
            None => log::info!(
                "la sesión anterior (pid {pid}) no se cerró limpiamente pero no falló: se terminó a la fuerza \
                 (SIGKILL, recompilación en desarrollo o apagado)"
            ),
        }
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    match std::fs::File::create(&marker).and_then(|mut f| {
        use std::io::Write;
        write!(f, "pid={} start={now}", std::process::id())?;
        Ok(f)
    }) {
        Ok(f) => {
            #[cfg(unix)]
            {
                use std::os::fd::IntoRawFd;
                // Se mantiene abierto todo el proceso para poder anotar la causa de un fallo.
                MARKER_FD.store(f.into_raw_fd(), Ordering::Relaxed);
                install_signal_handlers();
            }
            #[cfg(not(unix))]
            drop(f);
        }
        Err(e) => log::warn!("no se pudo escribir {}: {e}", marker.display()),
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
        webkit: webkit_version(),
    }
}

/// Descarta el aviso de cierre inesperado (ya mostrado al usuario).
#[tauri::command]
pub fn dismiss_crash_notice(state: tauri::State<'_, Diagnostics>) {
    *state.previous_crash.lock().unwrap() = None;
}
