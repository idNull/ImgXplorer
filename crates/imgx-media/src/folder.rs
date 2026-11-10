//! Fuente de carpeta (recursiva, orden natural).

use crate::{Entry, MediaError, Source, is_image_name, natural};
use std::path::{Path, PathBuf};

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
