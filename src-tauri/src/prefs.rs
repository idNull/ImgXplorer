//! Ajustes, lista de recientes y biblioteca de clips y plantillas propias: JSON en el directorio de configuración de la app. La interfaz
//! define y valida su contenido; aquí solo se leen y escriben de forma segura.

use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Archivos que la interfaz puede leer y escribir (nada de rutas arbitrarias).
const FILES: [&str; 3] = ["settings", "recents", "library"];

fn file(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    if !FILES.contains(&name) {
        return Err(format!("preferencia desconocida: {name}"));
    }
    // La biblioteca es contenido del usuario, no configuración: va al directorio de datos.
    let dir = if name == "library" { app.path().app_data_dir() } else { app.path().app_config_dir() }.map_err(|e| e.to_string())?;
    Ok(dir.join(format!("{name}.json")))
}

#[derive(Serialize)]
pub struct Prefs {
    settings: Option<String>,
    recents: Option<String>,
    library: Option<String>,
}

#[tauri::command]
pub fn load_prefs(app: AppHandle) -> Prefs {
    let read = |name: &str| {
        let path = file(&app, name).ok()?;
        std::fs::read_to_string(&path)
            .inspect_err(|e| {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("no se pudo leer {}: {e}", path.display());
                }
            })
            .ok()
    };
    Prefs { settings: read("settings"), recents: read("recents"), library: read("library") }
}

#[tauri::command]
pub async fn save_prefs(app: AppHandle, name: String, json: String) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(&json).map_err(|e| e.to_string())?;
    let path = file(&app, &name)?;
    tauri::async_runtime::spawn_blocking(move || crate::project::write_atomic(&path, &json))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Qué rutas siguen existiendo (para marcar recientes que ya no están).
#[tauri::command]
pub async fn paths_exist(paths: Vec<String>) -> Vec<bool> {
    tauri::async_runtime::spawn_blocking(move || paths.iter().map(|p| std::path::Path::new(p).exists()).collect())
        .await
        .unwrap_or_default()
}
