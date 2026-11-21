//! Portadas de la estantería (pantalla de inicio): una miniatura JPEG de la primera página de cada
//! cómic o carpeta abierta, guardada en la caché de la app (`<caché>/covers/<huella>.jpg`).
//!
//! Se crea al abrir algo por primera vez y, para lo que ya estaba en los recientes, la primera vez
//! que la estantería la pide (en segundo plano). Nunca se toca el cómic.

use imgx_media::Source;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Lado mayor de la portada (px): nítida en la rejilla y en la tarjeta grande.
const COVER_SIDE: u32 = 480;
const COVER_BG: [u8; 3] = [0x15, 0x18, 0x1d];

/// Huella de una ruta (FNV-1a de 64 bits): nombre del archivo de portada.
pub fn key(path: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in path.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn dir(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_cache_dir().ok()?.join("covers"))
}

/// Archivo de la portada de una huella (comprobando que es una huella y no una ruta).
pub fn file(app: &AppHandle, key: &str) -> Option<PathBuf> {
    if key.len() != 16 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(dir(app)?.join(format!("{key}.jpg")))
}

/// Crea la portada a partir de la primera página.
fn make(source: &dyn Source, out: &Path) -> Result<(), String> {
    let entry = source.entries().first().ok_or("sin imágenes")?;
    let bytes = source.read(0).map_err(|e| e.to_string())?;
    let jpg = imgx_media::thumbnail_jpeg(&entry.name, &bytes, COVER_SIDE, COVER_BG).map_err(|e| e.to_string())?;
    if let Some(d) = out.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = out.with_extension("jpg.tmp");
    std::fs::write(&tmp, &jpg).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, out).map_err(|e| e.to_string())
}

/// Al abrir una fuente: si aún no tiene portada, se hace en segundo plano.
pub fn ensure_on_open(app: &AppHandle, path: &str, source: std::sync::Arc<dyn Source>) {
    let Some(out) = file(app, &key(path)) else { return };
    if out.exists() {
        return;
    }
    let shown = path.to_owned();
    std::thread::spawn(move || {
        let _turn = BUSY.lock().unwrap();
        if let Err(e) = make(source.as_ref(), &out) {
            log::debug!("portada de {shown}: {e}");
        }
    });
}

/// Una portada a la vez (abrir un comprimido grande ya cuesta).
static BUSY: Mutex<()> = Mutex::new(());

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cover {
    key: String,
    /// Cambia si se rehace (para que la interfaz no use la vieja de su caché).
    version: u64,
}

fn found(out: &Path, key: String) -> Option<Cover> {
    let modified = std::fs::metadata(out).and_then(|m| m.modified()).ok()?;
    let version = modified.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    Some(Cover { key, version })
}

/// Portada de un reciente; si falta y la ruta existe, la crea (abriendo la fuente). `None` si no
/// se puede (ya no existe, sin imágenes…).
#[tauri::command]
pub async fn cover(app: AppHandle, path: String) -> Option<Cover> {
    let k = key(&path);
    let out = file(&app, &k)?;
    if let Some(c) = found(&out, k.clone()) {
        return Some(c);
    }
    tauri::async_runtime::spawn_blocking(move || {
        if !Path::new(&path).exists() {
            return None;
        }
        let _turn = BUSY.lock().unwrap();
        // Puede que otra petición la haya hecho mientras se esperaba el turno.
        if let Some(c) = found(&out, k.clone()) {
            return Some(c);
        }
        let opened = imgx_media::open_path(Path::new(&path)).inspect_err(|e| log::debug!("portada de {path}: {e}")).ok()?;
        make(opened.source.as_ref(), &out).inspect_err(|e| log::debug!("portada de {path}: {e}")).ok()?;
        found(&out, k)
    })
    .await
    .ok()
    .flatten()
}

/// Bytes de una portada para el protocolo `imgx://localhost/c/<huella>`.
pub fn read(app: &AppHandle, key: &str) -> Option<Vec<u8>> {
    std::fs::read(file(app, key)?).ok()
}

/// Olvida la portada de una ruta (al quitarla de la lista).
#[tauri::command]
pub fn forget_cover(app: AppHandle, path: String) {
    if let Some(out) = file(&app, &key(&path)) {
        let _ = std::fs::remove_file(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huella_estable_y_solo_hexadecimal() {
        assert_eq!(key("/a/b.cbz"), key("/a/b.cbz"));
        assert_ne!(key("/a/b.cbz"), key("/a/c.cbz"));
        assert_eq!(key("x").len(), 16);
        assert!(key("/ruta/ñ.cbz").bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn portada_de_la_primera_pagina() {
        // PNG de 1×1 píxel.
        const PNG: [u8; 67] = [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
            0xae, 0x42, 0x60, 0x82,
        ];
        let dir = std::env::temp_dir().join(format!("imgx-cover-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("01.png"), PNG).unwrap();
        std::fs::write(dir.join("02.png"), PNG).unwrap();
        let opened = imgx_media::open_path(&dir).unwrap();
        let out = dir.join("covers").join("x.jpg");
        make(opened.source.as_ref(), &out).unwrap();
        let jpg = std::fs::read(&out).unwrap();
        assert_eq!(&jpg[..2], &[0xff, 0xd8], "es un JPEG");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
