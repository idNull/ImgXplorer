//! Lo que el visor móvil necesita del propio sistema y Tauri no trae:
//!
//! - **Carrete:** el selector de fotos del sistema (sin permiso de acceso a todas las fotos). Las
//!   fotos HEIC/HEIF llegan como JPEG, porque el móvil compila sin libheif.
//! - **Archivos:** el selector de documentos, conservando el nombre (la extensión dice si es un
//!   cbz, un cbr…).
//! - **«Abrir con imgxplorer»:** lo que otra app envía al visor (Archivos, WhatsApp…). En Android
//!   lo guarda el complemento Kotlin; en iOS llega como `RunEvent::Opened` (iOS ya lo ha copiado a
//!   `Documents/Inbox`) y lo guarda este crate, que avisa a la interfaz con el evento de la app
//!   [`OPENED_EVENT`].
//! - **Foco de sonido:** mientras suena una historia, el visor pide el foco de audio de Android o
//!   activa la sesión de audio de iOS. Si lo pierde (llamada, otra app que suena) o se desconectan
//!   los auriculares, avisa a la interfaz con el evento `interrupt`
//!   (`addPluginListener("imgx-mobile", "interrupt", …)`).
//!
//! Todo se copia a una carpeta que elige Rust (la de importación): Android entrega direcciones
//! `content://` y iOS archivos que solo existen un momento; el resto de imgxplorer trabaja con
//! rutas.
//!
//! En el ordenador (para probar la interfaz como ventana) no hay selector del sistema: cada
//! función devuelve una lista vacía.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

/// Un archivo ya copiado a la carpeta de importación.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Picked {
    pub path: PathBuf,
    /// Nombre original (para el título).
    pub name: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct PickedList {
    files: Vec<Picked>,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct DirArgs<'a> {
    dir: &'a Path,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct FocusArgs {
    on: bool,
}

/// Evento de la app (`listen("imgx-opened", …)`): otra app ha pedido abrir algo con imgxplorer y
/// está esperando en [`ImgxMobile::take_opened`]. Solo en iOS; en Android la interfaz lo recoge al
/// volver a primer plano.
pub const OPENED_EVENT: &str = "imgx-opened";

#[cfg(any(target_os = "android", target_os = "ios"))]
type Handle<R> = tauri::plugin::PluginHandle<R>;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
type Handle<R> = std::marker::PhantomData<fn() -> R>;

pub struct ImgxMobile<R: Runtime> {
    #[allow(dead_code)]
    handle: Handle<R>,
    /// iOS: archivos que otra app ha pedido abrir y aún no se han recogido.
    #[allow(dead_code)]
    opened: std::sync::Mutex<Vec<PathBuf>>,
}

impl<R: Runtime> ImgxMobile<R> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    fn call(&self, method: &str, dir: &Path) -> Result<Vec<Picked>, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        self.handle.run_mobile_plugin::<PickedList>(method, DirArgs { dir }).map(|l| l.files).map_err(|e| e.to_string())
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn call(&self, _method: &str, _dir: &Path) -> Result<Vec<Picked>, String> {
        Ok(Vec::new())
    }

    /// Fotos del carrete elegidas por el usuario, copiadas a `dir` (vacío si cancela).
    pub fn pick_images(&self, dir: &Path) -> Result<Vec<Picked>, String> {
        self.call("pickImages", dir)
    }

    /// Archivos elegidos por el usuario (cómics, historias, imágenes), copiados a `dir`.
    pub fn pick_documents(&self, dir: &Path) -> Result<Vec<Picked>, String> {
        self.call("pickDocuments", dir)
    }

    /// Lo que otra app ha pedido abrir con imgxplorer y aún no se ha recogido, copiado a `dir`.
    pub fn take_opened(&self, dir: &Path) -> Result<Vec<Picked>, String> {
        #[cfg(target_os = "ios")]
        return Ok(move_all(std::mem::take(&mut *self.opened.lock().unwrap_or_else(|e| e.into_inner())), dir));
        #[cfg(not(target_os = "ios"))]
        self.call("takeOpened", dir)
    }

    /// Pide (`on`) o suelta el foco de sonido (Android) o la sesión de audio (iOS). En el
    /// ordenador no hace nada.
    pub fn audio_focus(&self, on: bool) -> Result<(), String> {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        return self.handle.run_mobile_plugin::<serde::de::IgnoredAny>("audioFocus", FocusArgs { on }).map(|_| ()).map_err(|e| e.to_string());
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let _ = on;
            Ok(())
        }
    }
}

pub trait ImgxMobileExt<R: Runtime> {
    fn imgx_mobile(&self) -> &ImgxMobile<R>;
}

impl<R: Runtime, T: Manager<R>> ImgxMobileExt<R> for T {
    fn imgx_mobile(&self) -> &ImgxMobile<R> {
        self.state::<ImgxMobile<R>>().inner()
    }
}

/// Mueve (o copia, si no se puede mover) cada archivo a `dir`, con su nombre y sin pisar otro.
/// Lo que no se pueda leer se salta y se apunta en el registro.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn move_all(files: Vec<PathBuf>, dir: &Path) -> Vec<Picked> {
    if let Err(e) = std::fs::create_dir_all(dir) {
        log::error!("imgx: no se pudo crear {}: {e}", dir.display());
        return Vec::new();
    }
    files
        .into_iter()
        .filter_map(|src| {
            let name = src.file_name()?.to_string_lossy().into_owned();
            let dest = unique(dir, &name);
            let moved = std::fs::rename(&src, &dest).or_else(|_| std::fs::copy(&src, &dest).map(|_| ()));
            match moved {
                Ok(()) => {
                    let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(name);
                    Some(Picked { path: dest, name })
                }
                Err(e) => {
                    log::error!("imgx: no se pudo recoger {}: {e}", src.display());
                    None
                }
            }
        })
        .collect()
}

/// `dir/name`, o `name (2)`, `name (3)`… si ya existe.
fn unique(dir: &Path, name: &str) -> PathBuf {
    let p = Path::new(name);
    let base = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| name.to_owned());
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let mut out = dir.join(name);
    let mut i = 2;
    while out.exists() {
        out = dir.join(format!("{base} ({i}){ext}"));
        i += 1;
    }
    out
}

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_imgx_mobile);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    let builder = Builder::new("imgx-mobile").setup(|app, _api| {
        #[cfg(target_os = "android")]
        let handle = _api.register_android_plugin("com.imgxplorer.plugin", "ImgxPlugin")?;
        #[cfg(target_os = "ios")]
        let handle = _api.register_ios_plugin(init_plugin_imgx_mobile)?;
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let handle = std::marker::PhantomData;
        app.manage(ImgxMobile::<R> { handle, opened: Default::default() });
        Ok(())
    });
    // iOS: «Abrir con» llega a la app como un evento. Se guarda hasta que la interfaz lo recoja.
    #[cfg(target_os = "ios")]
    let builder = builder.on_event(|app, event| {
        use tauri::Emitter;
        if let tauri::RunEvent::Opened { urls } = event {
            let files: Vec<PathBuf> = urls.iter().filter_map(|u| u.to_file_path().ok()).collect();
            if files.is_empty() {
                return;
            }
            log::info!("imgx: abrir con imgxplorer: {files:?}");
            app.imgx_mobile().opened.lock().unwrap_or_else(|e| e.into_inner()).extend(files);
            let _ = app.emit(OPENED_EVENT, ());
        }
    });
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lo_abierto_se_mueve_sin_pisar_otro() {
        let tmp = std::env::temp_dir().join(format!("imgx-opened-{}", std::process::id()));
        let inbox = tmp.join("Inbox");
        let dir = tmp.join("import");
        std::fs::create_dir_all(&inbox).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(inbox.join("comic.cbz"), b"a").unwrap();
        std::fs::write(dir.join("comic.cbz"), b"ya estaba").unwrap();
        let got = move_all(vec![inbox.join("comic.cbz"), inbox.join("no-existe.cbr")], &dir);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "comic (2).cbz");
        assert_eq!(std::fs::read(&got[0].path).unwrap(), b"a");
        assert!(!inbox.join("comic.cbz").exists(), "sale del Inbox");
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
