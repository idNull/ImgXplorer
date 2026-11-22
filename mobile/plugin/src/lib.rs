//! Lo que el visor móvil necesita del propio sistema y Tauri no trae:
//!
//! - **Carrete:** el selector de fotos de Android (sin permiso de acceso a todas las fotos). Las
//!   fotos HEIC/HEIF se convierten a JPEG al copiarlas, porque el móvil compila sin libheif.
//! - **Archivos:** el selector de documentos, conservando el nombre (la extensión dice si es un
//!   cbz, un cbr…).
//! - **«Abrir con imgxplorer»:** lo que otra app envía al visor (Archivos, WhatsApp…).
//!
//! Todo se copia a una carpeta que elige Rust (la de importación): Android entrega direcciones
//! `content://`, no rutas, y el resto de imgxplorer trabaja con rutas.
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

#[cfg(target_os = "android")]
pub struct ImgxMobile<R: Runtime>(tauri::plugin::PluginHandle<R>);

#[cfg(not(target_os = "android"))]
pub struct ImgxMobile<R: Runtime>(std::marker::PhantomData<fn() -> R>);

impl<R: Runtime> ImgxMobile<R> {
    #[cfg(target_os = "android")]
    fn call(&self, method: &str, dir: &Path) -> Result<Vec<Picked>, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        self.0.run_mobile_plugin::<PickedList>(method, DirArgs { dir }).map(|l| l.files).map_err(|e| e.to_string())
    }

    #[cfg(not(target_os = "android"))]
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
        self.call("takeOpened", dir)
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

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("imgx-mobile")
        .setup(|app, _api| {
            #[cfg(target_os = "android")]
            let handle = ImgxMobile(_api.register_android_plugin("com.imgxplorer.plugin", "ImgxPlugin")?);
            #[cfg(not(target_os = "android"))]
            let handle = ImgxMobile::<R>(std::marker::PhantomData);
            app.manage(handle);
            Ok(())
        })
        .build()
}
