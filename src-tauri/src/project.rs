//! Archivos de animación `.imgx`: dónde viven, cargarlos y guardarlos de forma atómica.
//!
//! | Origen      | Archivo                                    |
//! |-------------|--------------------------------------------|
//! | Carpeta     | `<carpeta>/.imgxplorer.imgx`               |
//! | Comprimido  | `<archivo>.imgx` (al lado)                  |
//! | Sin permiso | `<datos de la app>/projects/<huella>.imgx` |

use crate::{AppState, Current};
use imgx_core::Project;
use imgx_media::SourceKind;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

pub const FOLDER_FILE: &str = ".imgxplorer.imgx";

pub fn sidecar(path: &Path, kind: SourceKind) -> PathBuf {
    match kind {
        SourceKind::Folder => path.join(FOLDER_FILE),
        SourceKind::Archive => {
            let mut s = path.as_os_str().to_owned();
            s.push(".imgx");
            PathBuf::from(s)
        }
    }
}

fn fallback(app: &AppHandle, fingerprint: &str) -> Option<PathBuf> {
    Some(app.path().app_data_dir().ok()?.join("projects").join(format!("{fingerprint}.imgx")))
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Loaded {
    /// Proyecto normalizado (JSON), si existe.
    pub json: Option<String>,
    pub path: Option<String>,
    /// El archivo existe pero no se puede usar (p. ej. de una versión más nueva). La interfaz no
    /// debe guardar encima para no destruirlo.
    pub error: Option<String>,
}

/// Busca el proyecto junto al origen y, si no, en el directorio de datos.
pub fn load(app: &AppHandle, path: &Path, kind: SourceKind, fingerprint: &str) -> Loaded {
    for candidate in [Some(sidecar(path, kind)), fallback(app, fingerprint)].into_iter().flatten() {
        let text = match std::fs::read_to_string(&candidate) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                log::warn!("no se pudo leer {}: {e}", candidate.display());
                continue;
            }
        };
        let shown = Some(candidate.display().to_string());
        return match Project::from_json(&text) {
            Ok(p) => {
                log::info!("animaciones cargadas de {}", candidate.display());
                Loaded { json: Some(p.to_json()), path: shown, error: None }
            }
            Err(e) => {
                log::warn!("{} no es válido: {e}", candidate.display());
                Loaded { json: None, path: shown, error: Some(e.to_string()) }
            }
        };
    }
    Loaded::default()
}

/// Escribe en un temporal y lo renombra: un corte a medias nunca deja el archivo roto.
pub(crate) fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Guarda el proyecto de la fuente abierta. Devuelve la ruta en la que quedó.
#[tauri::command]
pub async fn save_project(app: AppHandle, generation: u64, json: String, state: State<'_, AppState>) -> Result<String, String> {
    let project = Project::from_json(&json).map_err(|e| e.to_string())?;
    let Current { path, kind, fingerprint, .. } = state.current(generation).ok_or("la fuente ya no está abierta")?;
    let text = project.to_json();
    let main = sidecar(&path, kind);
    let saved = tauri::async_runtime::spawn_blocking(move || match write_atomic(&main, &text) {
        Ok(()) => Ok(main),
        Err(e) => {
            // Medio de solo lectura, carpeta sin permisos…: al directorio de la app.
            let alt = fallback(&app, &fingerprint).ok_or("sin directorio de datos")?;
            log::info!("no se pudo escribir {} ({e}); se guarda en {}", main.display(), alt.display());
            write_atomic(&alt, &text).map(|()| alt).map_err(|e| e.to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    log::debug!("animaciones guardadas en {}", saved.display());
    Ok(saved.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rutas() {
        assert_eq!(sidecar(Path::new("/c/Tomo 1.cbz"), SourceKind::Archive), Path::new("/c/Tomo 1.cbz.imgx"));
        assert_eq!(sidecar(Path::new("/fotos"), SourceKind::Folder), Path::new("/fotos/.imgxplorer.imgx"));
    }

    #[test]
    fn escritura_atomica() {
        let dir = std::env::temp_dir().join(format!("imgx-test-{}", std::process::id()));
        let f = dir.join("sub/a.imgx");
        write_atomic(&f, "uno").unwrap();
        write_atomic(&f, "dos").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "dos");
        assert!(!dir.join("sub/a.imgx.tmp").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
