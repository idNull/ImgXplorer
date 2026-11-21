//! Cómics propios: crear uno con imágenes sueltas, convertir otra fuente en cómic y ordenar sus
//! páginas. El trabajo con archivos está en `imgx_media::comic`; aquí, la animación `.imgx`
//! sigue a sus páginas y se guarda junto al cómic.

use crate::{AppState, Current, covers, project};
use base64::Engine;
use imgx_core::Project;
use imgx_media::SourceKind;
use imgx_media::comic::{self, Page};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

/// Carpeta de los cómics propios por defecto: `Imágenes/imgxplorer` (en el Mac, `Pictures`).
#[tauri::command]
pub fn comic_default_dir(app: AppHandle) -> Option<String> {
    let base = app.path().picture_dir().or_else(|_| app.path().home_dir().map(|h| h.join("Pictures"))).ok()?;
    Some(base.join("imgxplorer").display().to_string())
}

fn parent_dir(app: &AppHandle, dir: Option<String>) -> Result<PathBuf, String> {
    let dir = match dir.filter(|d| !d.trim().is_empty()) {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(comic_default_dir(app.clone()).ok_or("no se encontró la carpeta de imágenes")?),
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// Crea un cómic copiando `files` en ese orden. Devuelve su carpeta.
#[tauri::command]
pub async fn comic_create(app: AppHandle, title: String, dir: Option<String>, files: Vec<String>) -> Result<String, String> {
    if files.is_empty() {
        return Err("no hay imágenes".into());
    }
    let parent = parent_dir(&app, dir)?;
    let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
    let out = tauri::async_runtime::spawn_blocking(move || comic::create(&parent, &title, &files))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    log::info!("cómic creado en {}", out.display());
    Ok(out.display().to_string())
}

/// Una página para un cómic hecho con otra fuente: una de esa fuente (por su índice) o una
/// imagen de fuera.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum SourcePage {
    Index { index: usize },
    New { path: PathBuf },
}

/// La animación de la fuente, con las páginas renombradas: solo las que van al cómic.
fn carry_project(json: &str, map: &BTreeMap<String, String>, title: &str) -> Result<Project, String> {
    let mut p = Project::from_json(json).map_err(|e| e.to_string())?;
    p.items.retain(|it| map.contains_key(&it.entry));
    p.ambient.retain(|a| map.contains_key(&a.from) && map.contains_key(&a.to));
    p.rename_entries(map);
    p.source = imgx_core::schema::Source { kind: imgx_core::schema::SourceKind::Folder, name: title.into(), fingerprint: None };
    Ok(p)
}

/// Crea un cómic con las páginas de la fuente abierta (un CBZ, una carpeta cualquiera…) en el
/// orden de `pages`, con imágenes de fuera si las hay. La animación va con él. La fuente no cambia.
#[tauri::command]
pub async fn comic_from_source(
    app: AppHandle,
    generation: u64,
    title: String,
    dir: Option<String>,
    pages: Vec<SourcePage>,
    project: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let Current { source, .. } = state.current(generation).ok_or("la fuente ya no está abierta")?;
    let parent = parent_dir(&app, dir)?;
    let out = tauri::async_runtime::spawn_blocking(move || {
        let order: Vec<usize> = pages.iter().filter_map(|p| if let SourcePage::Index { index } = p { Some(*index) } else { None }).collect();
        let (out, map) = comic::from_source(source.as_ref(), &order, &parent, &title).map_err(|e| e.to_string())?;
        // Las imágenes de fuera, en su sitio.
        if pages.iter().any(|p| matches!(p, SourcePage::New { .. })) {
            let names = source.entries();
            let plan: Vec<Page> = pages
                .iter()
                .map(|p| match p {
                    SourcePage::Index { index } => Page::Existing { entry: map[&names[*index].name].clone() },
                    SourcePage::New { path } => Page::New { path: path.clone() },
                })
                .collect();
            let again = comic::arrange(&out, &plan, &[]).map_err(|e| e.to_string())?;
            let map: BTreeMap<String, String> = map.iter().map(|(k, v)| (k.clone(), again.get(v).cloned().unwrap_or_else(|| v.clone()))).collect();
            write_project(&out, project.as_deref(), &map, &title)?;
        } else {
            write_project(&out, project.as_deref(), &map, &title)?;
        }
        Ok::<_, String>(out)
    })
    .await
    .map_err(|e| e.to_string())??;
    log::info!("cómic creado desde la fuente abierta en {}", out.display());
    Ok(out.display().to_string())
}

fn write_project(dir: &Path, json: Option<&str>, map: &BTreeMap<String, String>, title: &str) -> Result<(), String> {
    let Some(json) = json else { return Ok(()) };
    let p = carry_project(json, map, title)?;
    if p.items.is_empty() && p.ambient.is_empty() {
        return Ok(());
    }
    project::write_atomic(&project::sidecar(dir, SourceKind::Folder), &p.to_json()).map_err(|e| e.to_string())
}

/// Ordena el cómic propio abierto. Devuelve el nombre nuevo de cada página que ya estaba. La
/// animación guardada junto al cómic se actualiza (la interfaz guarda la suya antes y vuelve a
/// abrir el cómic después).
#[tauri::command]
pub async fn comic_arrange(app: AppHandle, generation: u64, pages: Vec<Page>, removed: Vec<String>, state: State<'_, AppState>) -> Result<BTreeMap<String, String>, String> {
    let Current { path, kind, .. } = state.current(generation).ok_or("la fuente ya no está abierta")?;
    if kind != SourceKind::Folder || !comic::is_comic(&path) {
        return Err("no es un cómic propio".into());
    }
    let dir = path.clone();
    let map = tauri::async_runtime::spawn_blocking(move || {
        let map = comic::arrange(&dir, &pages, &removed).map_err(|e| e.to_string())?;
        let side = project::sidecar(&dir, SourceKind::Folder);
        if let Ok(text) = std::fs::read_to_string(&side) {
            match Project::from_json(&text) {
                Ok(mut p) => {
                    p.rename_entries(&map);
                    project::write_atomic(&side, &p.to_json()).map_err(|e| e.to_string())?;
                }
                // Un archivo que no se entiende no se toca.
                Err(e) => log::warn!("{} no es válido, no se renombra: {e}", side.display()),
            }
        }
        Ok::<_, String>(map)
    })
    .await
    .map_err(|e| e.to_string())??;
    // La portada puede ser otra.
    covers::forget_cover(app, path.display().to_string());
    Ok(map)
}

/// Miniatura de una imagen que aún no está en el cómic (para el diálogo «Crear cómic»), como
/// `data:` URL.
#[tauri::command]
pub async fn thumb_file(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let name = Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let jpg = imgx_media::decode::thumbnail_jpeg(&name, &bytes, 256, [24, 24, 28]).map_err(|e| e.to_string())?;
        Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(jpg)))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_animacion_sigue_a_sus_paginas() {
        let mut p = Project::new(imgx_core::schema::Source { kind: imgx_core::schema::SourceKind::Archive, name: "Tomo".into(), fingerprint: Some("x".into()) });
        for e in ["1.png", "2.png", "3.png"] {
            p.items.push(serde_json::from_value(serde_json::json!({ "entry": e, "size": [10, 10] })).unwrap());
        }
        let map: BTreeMap<String, String> = [("3.png", "0001 - 3.png"), ("1.png", "0002 - 1.png")].into_iter().map(|(a, b)| (a.into(), b.into())).collect();
        let out = carry_project(&p.to_json(), &map, "Mío").unwrap();
        let names: Vec<&str> = out.items.iter().map(|i| i.entry.as_str()).collect();
        assert_eq!(names, ["0002 - 1.png", "0001 - 3.png"], "la 2 no va al cómic");
        assert_eq!(out.source.name, "Mío");
        assert!(out.source.fingerprint.is_none());
    }
}
