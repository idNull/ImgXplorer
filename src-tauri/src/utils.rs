//! Utilidades de la imagen actual (ℹ️): información y EXIF, copiar al portapapeles, guardar como,
//! mostrarla en su carpeta y moverla a la papelera.

use crate::AppState;
use imgx_media::SourceKind;
use std::path::{Path, PathBuf};
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Archivo de la imagen `index` en disco (solo en carpetas: en un comprimido no hay archivo).
fn entry_file(state: &AppState, generation: u64, index: usize) -> Result<(PathBuf, SourceKind, PathBuf), String> {
    let cur = state.current(generation).ok_or("la fuente ya no está abierta")?;
    let name = cur.source.entries().get(index).ok_or("índice fuera de rango")?.name.clone();
    let file = cur.path.join(&name);
    Ok((file, cur.kind, cur.path.clone()))
}

/// Formato y datos de la cámara de la imagen `index`.
#[tauri::command]
pub async fn image_info(generation: u64, index: usize, state: State<'_, AppState>) -> Result<imgx_media::info::ImageInfo, String> {
    let cur = state.current(generation).ok_or("la fuente ya no está abierta")?;
    let src = cur.source.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let name = src.entries().get(index).ok_or("índice fuera de rango")?.name.clone();
        let bytes = src.read(index).map_err(|e| e.to_string())?;
        Ok(imgx_media::info::info(&name, &bytes))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn raw(request: &Request<'_>) -> Result<Vec<u8>, String> {
    match request.body() {
        InvokeBody::Raw(data) => Ok(data.clone()),
        _ => Err("se esperaban bytes".into()),
    }
}

/// Copia al portapapeles una imagen PNG (la interfaz la dibuja con los ajustes aplicados).
#[tauri::command]
pub async fn copy_image(app: AppHandle, request: Request<'_>) -> Result<(), String> {
    let png = raw(&request)?;
    let rgba = tauri::async_runtime::spawn_blocking(move || imgx_media::decode("copia.png", &png))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let img = tauri::image::Image::new_owned(rgba.data, rgba.width, rgba.height);
    app.clipboard().write_image(&img).map_err(|e| e.to_string())
}

/// Ruta de destino que manda la interfaz (cabecera `path`, codificada como URI).
fn target(request: &Request<'_>) -> Result<PathBuf, String> {
    let enc = request.headers().get("path").and_then(|v| v.to_str().ok()).ok_or("falta la ruta")?;
    let path = PathBuf::from(decode_uri(enc)?);
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(format!("ruta no válida: {}", path.display()));
    }
    Ok(path)
}

fn decode_uri(s: &str) -> Result<String, String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let h = std::str::from_utf8(&b[i + 1..i + 3]).map_err(|e| e.to_string())?;
            out.push(u8::from_str_radix(h, 16).map_err(|e| e.to_string())?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|e| e.to_string())
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Guarda una imagen ya codificada (PNG o JPEG que dibuja la interfaz) en la ruta de la cabecera.
#[tauri::command]
pub async fn save_image(request: Request<'_>) -> Result<(), String> {
    let path = target(&request)?;
    let bytes = raw(&request)?;
    tauri::async_runtime::spawn_blocking(move || write(&path, &bytes)).await.map_err(|e| e.to_string())?
}

/// Guarda una copia exacta del archivo original de la imagen `index` (también desde un comprimido).
#[tauri::command]
pub async fn save_original(generation: u64, index: usize, path: String, state: State<'_, AppState>) -> Result<(), String> {
    let dest = PathBuf::from(&path);
    if !dest.is_absolute() {
        return Err(format!("ruta no válida: {path}"));
    }
    let src = state.current(generation).ok_or("la fuente ya no está abierta")?.source.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = src.read(index).map_err(|e| e.to_string())?;
        write(&dest, &bytes)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Enseña la imagen en el gestor de archivos (en un comprimido, el comprimido).
#[tauri::command]
pub fn reveal_entry(generation: u64, index: usize, state: State<'_, AppState>) -> Result<(), String> {
    let (file, kind, root) = entry_file(&state, generation, index)?;
    let shown = if kind == SourceKind::Folder { file } else { root };
    crate::covers::show_in_folder(shown.display().to_string())
}

/// Mueve la imagen `index` a la papelera del sistema (se puede recuperar desde allí). Solo en
/// carpetas: dentro de un comprimido no se puede borrar una página.
#[tauri::command]
pub async fn trash_entry(generation: u64, index: usize, state: State<'_, AppState>) -> Result<(), String> {
    let (file, kind, _) = entry_file(&state, generation, index)?;
    if kind != SourceKind::Folder {
        return Err("solo se pueden mover a la papelera imágenes de una carpeta".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        trash::delete(&file).map_err(|e| e.to_string())?;
        log::info!("a la papelera: {}", file.display());
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rutas_codificadas() {
        assert_eq!(decode_uri("%2Fhome%2Fy%C3%B1%20o.png").unwrap(), "/home/yñ o.png");
        assert_eq!(decode_uri("/a/b.png").unwrap(), "/a/b.png");
    }
}
