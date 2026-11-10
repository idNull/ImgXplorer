//! Acceso a imágenes: fuentes (carpeta, zip/cbz, 7z/cb7, rar/cbr), clasificación de
//! formatos, decodificación a RGBA y miniaturas.

pub mod archive;
pub mod decode;
pub mod folder;
pub mod natural;
mod raw;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use archive::{RarSource, SevenZSource, ZipSource};
pub use decode::{Rgba, decode, thumbnail_jpeg};
pub use folder::FolderSource;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("índice {0} fuera de rango")]
    OutOfRange(usize),
    #[error("comprimido: {0}")]
    Archive(String),
    #[error("no se pudo decodificar: {0}")]
    Decode(String),
    #[error("formato no admitido: {0}")]
    Unsupported(String),
}

/// Formatos que el webview decodifica por sí mismo: se le sirven los bytes tal cual.
pub const WEB_NATIVE: &[&str] = &["jpg", "jpeg", "jfif", "jpe", "png", "apng", "gif", "webp", "bmp", "avif"];

/// Formatos que decodificamos en Rust y servimos como RGBA.
pub const DECODED: &[&str] = &[
    "svg", "svgz", // resvg
    "heic", "heif", "hif", // libheif
    "jxl", // jxl-oxide
    "psd", // psd
    "tif", "tiff", "tga", "ico", "qoi", "exr", "hdr", "pnm", "pbm", "pgm", "ppm", "pam", "dds", // image
];

/// Formatos RAW de cámara: se muestra la vista previa JPEG incrustada (suele ser de tamaño completo).
pub const RAW: &[&str] = &[
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "raf", "pef", "srw", "x3f",
    "3fr", "iiq", "mos", "erf", "kdc", "dcr", "mrw", "rwl", "raw",
];

/// Comprimidos que se abren como fuente.
pub const ARCHIVES: &[&str] = &["zip", "cbz", "7z", "cb7", "rar", "cbr"];

pub fn extension(name: &str) -> Option<String> {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase())
}

fn ext_in(name: &str, list: &[&str]) -> bool {
    extension(name).is_some_and(|e| list.contains(&e.as_str()))
}

pub fn is_image_name(name: &str) -> bool {
    ext_in(name, WEB_NATIVE) || ext_in(name, DECODED) || ext_in(name, RAW)
}

/// `true` si el webview no sabe mostrarla y hay que decodificarla en Rust.
pub fn needs_decode(name: &str) -> bool {
    !ext_in(name, WEB_NATIVE)
}

pub fn is_raw_name(name: &str) -> bool {
    ext_in(name, RAW)
}

pub fn is_archive_name(name: &str) -> bool {
    ext_in(name, ARCHIVES)
}

pub fn mime_for(name: &str) -> &'static str {
    match extension(name).as_deref() {
        Some("jpg" | "jpeg" | "jfif" | "jpe") => "image/jpeg",
        Some("png" | "apng") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("avif") => "image/avif",
        _ => "application/octet-stream",
    }
}

/// Descarta rutas ocultas y basura de macOS dentro de comprimidos y carpetas.
pub(crate) fn is_visible_path(name: &str) -> bool {
    !name.split('/').any(|c| c.starts_with('.') || c == "__MACOSX")
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Ruta relativa a la raíz de la fuente, con `/`.
    pub name: String,
    pub size: u64,
}

/// Una colección ordenada de imágenes (carpeta, comprimido o imagen suelta).
pub trait Source: Send + Sync {
    fn title(&self) -> &str;
    fn entries(&self) -> &[Entry];
    fn read(&self, index: usize) -> Result<Vec<u8>, MediaError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Folder,
    Archive,
}

pub struct Opened {
    pub source: Arc<dyn Source>,
    pub kind: SourceKind,
    /// Ruta abierta (carpeta o comprimido).
    pub path: PathBuf,
    /// Índice inicial: la imagen pedida si se abrió una imagen suelta.
    pub start: usize,
}

/// Abre una carpeta, un comprimido o una imagen suelta (abre su carpeta y se sitúa en ella).
pub fn open_path(path: &Path) -> Result<Opened, MediaError> {
    if path.is_dir() {
        let src = FolderSource::open(path)?;
        return Ok(Opened { source: Arc::new(src), kind: SourceKind::Folder, path: path.into(), start: 0 });
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if is_archive_name(&name) {
        let source: Arc<dyn Source> = match extension(&name).as_deref() {
            Some("zip" | "cbz") => Arc::new(ZipSource::open(path)?),
            Some("7z" | "cb7") => Arc::new(SevenZSource::open(path)?),
            _ => Arc::new(RarSource::open(path)?),
        };
        return Ok(Opened { source, kind: SourceKind::Archive, path: path.into(), start: 0 });
    }
    if is_image_name(&name) {
        let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let src = FolderSource::open(dir)?;
        let start = src.entries().iter().position(|e| Path::new(&e.name) == Path::new(&name)).unwrap_or(0);
        return Ok(Opened { source: Arc::new(src), kind: SourceKind::Folder, path: dir.into(), start });
    }
    Err(MediaError::Unsupported(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clasificacion() {
        assert!(is_image_name("a/B.JPG") && !needs_decode("x.jpg"));
        assert!(is_image_name("x.heic") && needs_decode("x.heic"));
        assert!(is_image_name("x.CR3") && is_raw_name("x.CR3"));
        assert!(is_archive_name("Tomo 1.cbr") && !is_image_name("Tomo 1.cbr"));
        assert!(!is_visible_path("__MACOSX/._a.jpg") && !is_visible_path("x/.thumb.jpg"));
        assert!(is_visible_path("cap 1/001.jpg"));
    }
}
