//! Acceso a imágenes. Fase 0: carpetas. Fase 1: zip/7z/rar, decodificadores y caché.

pub mod natural;

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("índice {0} fuera de rango")]
    OutOfRange(usize),
}

/// Formatos que el webview decodifica por sí mismo (se sirven tal cual).
pub const WEB_NATIVE: &[&str] = &["jpg", "jpeg", "jfif", "png", "apng", "gif", "webp", "bmp", "avif", "svg"];

pub fn extension(name: &str) -> Option<String> {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase())
}

pub fn is_image_name(name: &str) -> bool {
    extension(name).is_some_and(|e| WEB_NATIVE.contains(&e.as_str()))
}

pub fn mime_for(name: &str) -> &'static str {
    match extension(name).as_deref() {
        Some("jpg" | "jpeg" | "jfif") => "image/jpeg",
        Some("png" | "apng") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("avif") => "image/avif",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
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

pub struct FolderSource {
    root: PathBuf,
    title: String,
    entries: Vec<Entry>,
}

impl FolderSource {
    /// Lista recursivamente las imágenes de `root` en orden natural.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, MediaError> {
        let root = root.into();
        let mut entries = Vec::new();
        walk(&root, &root, &mut entries)?;
        entries.sort_by(|a, b| natural::natural_cmp(&a.name, &b.name));
        let title = root.file_name().map_or_else(|| root.display().to_string(), |n| n.to_string_lossy().into());
        Ok(Self { root, title, entries })
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<Entry>) -> Result<(), MediaError> {
    for item in std::fs::read_dir(dir)? {
        let item = item?;
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = item.path();
        let meta = item.metadata()?;
        if meta.is_dir() {
            walk(root, &path, out)?;
        } else if is_image_name(&name) {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            out.push(Entry { name: rel, size: meta.len() });
        }
    }
    Ok(())
}

impl Source for FolderSource {
    fn title(&self) -> &str {
        &self.title
    }
    fn entries(&self) -> &[Entry] {
        &self.entries
    }
    fn read(&self, index: usize) -> Result<Vec<u8>, MediaError> {
        let e = self.entries.get(index).ok_or(MediaError::OutOfRange(index))?;
        Ok(std::fs::read(self.root.join(&e.name))?)
    }
}
