//! Listado de carpetas para el selector de archivos propio de la app. Existe porque en
//! algunos escritorios (Hyprland/Omarchy, entre otros) el diálogo GTK sale sin los botones
//! de Aceptar/Cancelar.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::Manager;

/// Más entradas que esto en una carpeta no se listan (se avisa con `truncated`).
const MAX_ENTRIES: usize = 20_000;

#[derive(Serialize)]
pub struct Entry {
    name: String,
    dir: bool,
    size: u64,
    /// Segundos desde la época, si se conoce.
    modified: Option<u64>,
}

#[derive(Serialize)]
pub struct Listing {
    /// Ruta canónica de la carpeta listada.
    path: String,
    parent: Option<String>,
    entries: Vec<Entry>,
    truncated: bool,
}

#[derive(Serialize)]
pub struct Place {
    /// Clave para traducir ("home", "desktop"…) o vacía si `name` ya es el nombre.
    key: &'static str,
    name: String,
    path: String,
}

fn list(path: &Path, hidden: bool) -> Result<Listing, String> {
    let dir = path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))?;
    if !dir.is_dir() {
        return Err(format!("{}: no es una carpeta", dir.display()));
    }
    let mut entries = Vec::new();
    let mut truncated = false;
    for e in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !hidden && name.starts_with('.') {
            continue;
        }
        if entries.len() >= MAX_ENTRIES {
            truncated = true;
            break;
        }
        // `metadata` sigue los enlaces: un enlace a una carpeta se navega como carpeta.
        let Ok(md) = std::fs::metadata(e.path()) else { continue };
        let modified = md.modified().ok().and_then(|m| m.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs());
        entries.push(Entry { name, dir: md.is_dir(), size: if md.is_dir() { 0 } else { md.len() }, modified });
    }
    let parent = dir.parent().map(|p| p.to_string_lossy().into_owned());
    Ok(Listing { path: dir.to_string_lossy().into_owned(), parent, entries, truncated })
}

#[tauri::command]
pub async fn list_dir(path: String, hidden: bool) -> Result<Listing, String> {
    tauri::async_runtime::spawn_blocking(move || list(Path::new(&path), hidden))
        .await
        .map_err(|e| e.to_string())?
}

/// Accesos rápidos: carpetas personales y unidades montadas.
#[tauri::command]
pub fn places(app: tauri::AppHandle) -> Vec<Place> {
    let p = app.path();
    let mut out = Vec::new();
    let mut push = |key: &'static str, name: String, path: Option<PathBuf>| {
        if let Some(path) = path.filter(|x| x.is_dir()) {
            let path = path.to_string_lossy().into_owned();
            if !out.iter().any(|o: &Place| o.path == path) {
                out.push(Place { key, name, path });
            }
        }
    };
    push("home", String::new(), p.home_dir().ok());
    push("desktop", String::new(), p.desktop_dir().ok());
    push("documents", String::new(), p.document_dir().ok());
    push("downloads", String::new(), p.download_dir().ok());
    push("pictures", String::new(), p.picture_dir().ok());
    push("root", String::new(), Some(PathBuf::from("/")));
    // Unidades montadas (Linux: /run/media/<usuario>/*, /media/*, /mnt/*; macOS: /Volumes/*).
    let user = std::env::var("USER").unwrap_or_default();
    let mounts = [format!("/run/media/{user}"), "/media".into(), "/mnt".into(), "/Volumes".into()];
    for m in mounts {
        let Ok(rd) = std::fs::read_dir(&m) else { continue };
        let mut found: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|x| x.is_dir()).collect();
        found.sort();
        for d in found {
            let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            push("", name, Some(d));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lista_y_oculta() {
        let dir = std::env::temp_dir().join(format!("imgx-browse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.imgxpack"), b"1234").unwrap();
        std::fs::write(dir.join(".oculto"), b"").unwrap();

        let l = list(&dir, false).unwrap();
        let mut names: Vec<_> = l.entries.iter().map(|e| (e.name.as_str(), e.dir, e.size)).collect();
        names.sort();
        assert_eq!(names, vec![("a.imgxpack", false, 4), ("sub", true, 0)]);
        assert!(l.parent.is_some());
        assert_eq!(list(&dir, true).unwrap().entries.len(), 3);
        assert!(list(&dir.join("a.imgxpack"), false).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
