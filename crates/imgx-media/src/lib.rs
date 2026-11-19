//! Acceso a imágenes: fuentes (carpeta, zip/cbz, 7z/cb7, rar/cbr), clasificación de
//! formatos, decodificación a RGBA y miniaturas.

pub mod archive;
pub mod decode;
pub mod folder;
pub mod info;
pub mod levels;
pub mod natural;
pub mod panels;
mod raw;

use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(feature = "rar")]
pub use archive::RarSource;
pub use archive::{SevenZSource, ZipSource};
pub use decode::{Rgba, decode, heif_support, thumbnail_jpeg};
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

/// Huella del contenido (nombres y tamaños de las entradas, FNV-1a de 64 bits en hexadecimal).
/// Estable entre versiones y plataformas: sirve para reencontrar las animaciones de un cómic
/// aunque se renombre o se mueva.
pub fn fingerprint(entries: &[Entry]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for e in entries {
        eat(e.name.as_bytes());
        eat(&[0]);
        eat(&e.size.to_le_bytes());
    }
    format!("{h:016x}")
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
    /// Se abrió una imagen concreta (no la carpeta o el comprimido entero).
    pub picked: bool,
}

#[cfg(feature = "rar")]
fn open_rar(path: &Path) -> Result<Arc<dyn Source>, MediaError> {
    Ok(Arc::new(RarSource::open(path)?))
}

/// Compilado sin la función «rar» (para paquetes 100 % libres): UnRAR no está incluido.
#[cfg(not(feature = "rar"))]
fn open_rar(_: &Path) -> Result<Arc<dyn Source>, MediaError> {
    Err(MediaError::Unsupported("RAR (esta versión se ha compilado sin soporte RAR)".into()))
}

/// Abre una carpeta, un comprimido o una imagen suelta (abre su carpeta y se sitúa en ella).
pub fn open_path(path: &Path) -> Result<Opened, MediaError> {
    if path.is_dir() {
        let src = FolderSource::open(path)?;
        return Ok(Opened { source: Arc::new(src), kind: SourceKind::Folder, path: path.into(), start: 0, picked: false });
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if is_archive_name(&name) {
        let source: Arc<dyn Source> = match extension(&name).as_deref() {
            Some("zip" | "cbz") => Arc::new(ZipSource::open(path)?),
            Some("7z" | "cb7") => Arc::new(SevenZSource::open(path)?),
            _ => open_rar(path)?,
        };
        return Ok(Opened { source, kind: SourceKind::Archive, path: path.into(), start: 0, picked: false });
    }
    if is_image_name(&name) {
        let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let src = FolderSource::open(dir)?;
        let start = src.entries().iter().position(|e| Path::new(&e.name) == Path::new(&name)).unwrap_or(0);
        return Ok(Opened { source: Arc::new(src), kind: SourceKind::Folder, path: dir.into(), start, picked: true });
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
        let e = |n: &str, size| Entry { name: n.into(), size };
        assert_eq!(fingerprint(&[e("a.jpg", 1)]), fingerprint(&[e("a.jpg", 1)]));
        assert_ne!(fingerprint(&[e("a.jpg", 1)]), fingerprint(&[e("a.jpg", 2)]));
        assert_eq!(fingerprint(&[]), "cbf29ce484222325");
    }

    /// Compilado sin la función «rar»: un .cbr da un aviso claro en vez de fallar raro.
    #[cfg(not(feature = "rar"))]
    #[test]
    fn sin_rar_avisa() {
        let path = std::env::temp_dir().join(format!("imgx-sin-rar-{}.cbr", std::process::id()));
        std::fs::write(&path, b"Rar!").unwrap();
        let err = open_path(&path).err().expect("debería fallar");
        std::fs::remove_file(&path).ok();
        assert!(matches!(err, MediaError::Unsupported(ref m) if m.contains("RAR")), "{err}");
    }
}
