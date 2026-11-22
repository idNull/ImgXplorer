//! Biblioteca del visor: lo que el usuario ha abierto se copia dentro de la app.
//!
//! En el móvil no hay rutas como en el escritorio: Android entrega direcciones `content://` y UnRAR
//! solo sabe abrir archivos. Por eso cada cómic, historia o álbum de fotos se guarda en
//! `<datos de la app>/library/<id>/` y la lista (con el progreso de lectura) en `library.json`.
//!
//! | Tipo    | Qué hay en su carpeta                                    |
//! |---------|----------------------------------------------------------|
//! | `comic` | el comprimido (`cbz`, `cbr`, `cb7`, `zip`, `rar`, `7z`)    |
//! | `story` | la historia `.imgxstory`                                  |
//! | `album` | las fotos elegidas del carrete (o imágenes sueltas)       |

use imgx_host::project::write_atomic;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Comic,
    Story,
    Album,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub title: String,
    pub kind: Kind,
    /// Ruta absoluta que se abre: el archivo (cómic, historia) o la carpeta (álbum).
    pub path: String,
    pub pages: usize,
    /// Página en la que se quedó.
    pub page: usize,
    /// Segundos desde 1970.
    pub added: u64,
    pub opened: u64,
}

/// Resultado de una importación: lo que entró y lo que no se pudo abrir.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub items: Vec<Item>,
    /// Nombres que no se pudieron abrir, con el motivo.
    pub failed: Vec<(String, String)>,
}

/// Un archivo recién copiado a la carpeta de importación.
pub struct Incoming {
    pub path: PathBuf,
    pub name: String,
}

pub struct Library {
    root: PathBuf,
    items: Mutex<Option<Vec<Item>>>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Identificador corto y único (tiempo en milisegundos y un contador, en hexadecimal).
fn new_id() -> String {
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{ms:x}{n:04x}")
}

/// Título a partir del nombre del archivo: sin extensión y con espacios en vez de `_`.
fn title_of(name: &str) -> String {
    let stem = Path::new(name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| name.to_owned());
    let t = stem.replace('_', " ").trim().to_owned();
    if t.is_empty() { name.to_owned() } else { t }
}

/// Mueve (o, si no se puede, copia) `from` a `to`.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(d) = to.parent() {
        std::fs::create_dir_all(d)?;
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to)?;
    std::fs::remove_file(from)
}

/// Cuántas imágenes tiene lo que hay en `path` (y si se puede abrir).
fn count_pages(path: &Path) -> Result<usize, String> {
    let opened = imgx_media::open_path(path).map_err(|e| e.to_string())?;
    let n = opened.source.entries().len();
    if n == 0 { Err("no tiene imágenes".into()) } else { Ok(n) }
}

impl Library {
    pub fn new(root: PathBuf) -> Self {
        Library { root, items: Mutex::new(None) }
    }

    fn index_path(&self) -> PathBuf {
        self.root.join("library.json")
    }

    fn with<T>(&self, f: impl FnOnce(&mut Vec<Item>) -> T) -> T {
        let mut guard = self.items.lock().unwrap();
        let items = guard.get_or_insert_with(|| {
            std::fs::read_to_string(self.index_path())
                .ok()
                .and_then(|s| serde_json::from_str(&s).inspect_err(|e| log::warn!("library.json no es válido: {e}")).ok())
                .unwrap_or_default()
        });
        f(items)
    }

    fn save(&self, items: &[Item]) -> Result<(), String> {
        let json = serde_json::to_string_pretty(items).map_err(|e| e.to_string())?;
        write_atomic(&self.index_path(), &json).map_err(|e| e.to_string())
    }

    /// Lo último abierto primero; lo nunca abierto, por fecha de llegada.
    pub fn list(&self) -> Vec<Item> {
        let mut list = self.with(|items| {
            // Lo que se borró por fuera (o una importación a medias) no se enseña.
            items.retain(|i| Path::new(&i.path).exists());
            items.clone()
        });
        list.sort_by_key(|i| std::cmp::Reverse(i.opened.max(i.added)));
        list
    }

    /// Mete en la biblioteca lo copiado a la carpeta de importación. Los comprimidos y las
    /// historias entran cada uno por su lado; las imágenes sueltas, juntas en un álbum llamado
    /// `album_title`.
    pub fn import(&self, incoming: Vec<Incoming>, album_title: &str) -> Imported {
        let mut out = Imported::default();
        let mut images: Vec<Incoming> = Vec::new();
        for inc in incoming {
            let lower = inc.name.to_lowercase();
            if imgx_media::is_story_name(&lower) || imgx_media::is_archive_name(&lower) {
                let kind = if imgx_media::is_story_name(&lower) { Kind::Story } else { Kind::Comic };
                match self.add_file(&inc, kind) {
                    Ok(item) => out.items.push(item),
                    Err(e) => out.failed.push((inc.name.clone(), e)),
                }
            } else if imgx_media::is_image_name(&lower) || imgx_media::is_raw_name(&lower) {
                images.push(inc);
            } else {
                let _ = std::fs::remove_file(&inc.path);
                out.failed.push((inc.name, "no es una imagen, un cómic ni una historia".into()));
            }
        }
        if !images.is_empty() {
            match self.add_album(images, album_title) {
                Ok(item) => out.items.push(item),
                Err(e) => out.failed.push((album_title.to_owned(), e)),
            }
        }
        out
    }

    fn add_file(&self, inc: &Incoming, kind: Kind) -> Result<Item, String> {
        let id = new_id();
        let dir = self.root.join(&id);
        let dest = dir.join(&inc.name);
        move_file(&inc.path, &dest).map_err(|e| e.to_string())?;
        let pages = count_pages(&dest).inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&dir);
        })?;
        let item = Item {
            id,
            title: title_of(&inc.name),
            kind,
            path: dest.display().to_string(),
            pages,
            page: 0,
            added: now(),
            opened: 0,
        };
        self.push(item.clone())?;
        log::info!("biblioteca: «{}» ({} páginas)", item.title, item.pages);
        Ok(item)
    }

    fn add_album(&self, images: Vec<Incoming>, title: &str) -> Result<Item, String> {
        let id = new_id();
        let dir = self.root.join(&id);
        for inc in &images {
            move_file(&inc.path, &dir.join(&inc.name)).map_err(|e| e.to_string())?;
        }
        let pages = count_pages(&dir).inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&dir);
        })?;
        let item = Item {
            id,
            title: title.to_owned(),
            kind: Kind::Album,
            path: dir.display().to_string(),
            pages,
            page: 0,
            added: now(),
            opened: 0,
        };
        self.push(item.clone())?;
        log::info!("biblioteca: álbum «{}» ({} fotos)", item.title, item.pages);
        Ok(item)
    }

    fn push(&self, item: Item) -> Result<(), String> {
        self.with(|items| {
            items.push(item);
            self.save(items)
        })
    }

    /// Anota la página en la que va y que se acaba de abrir.
    pub fn progress(&self, id: &str, page: usize) -> Result<(), String> {
        self.with(|items| {
            let Some(it) = items.iter_mut().find(|i| i.id == id) else { return Ok(()) };
            it.page = page.min(it.pages.saturating_sub(1));
            it.opened = now();
            self.save(items)
        })
    }

    /// Quita un elemento y borra su carpeta. Devuelve la ruta que tenía (para olvidar su portada).
    pub fn remove(&self, id: &str) -> Result<Option<String>, String> {
        self.with(|items| {
            let Some(pos) = items.iter().position(|i| i.id == id) else { return Ok(None) };
            let item = items.remove(pos);
            self.save(items)?;
            // Solo se borra dentro de la biblioteca (el id no puede salirse de ella).
            if !item.id.is_empty() && item.id.bytes().all(|b| b.is_ascii_hexdigit()) {
                let _ = std::fs::remove_dir_all(self.root.join(&item.id));
            }
            log::info!("biblioteca: borrado «{}»", item.title);
            Ok(Some(item.path))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("imgx-lib-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// PNG de 1×1.
    const PNG: [u8; 67] = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63,
        0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60,
        0x82,
    ];

    fn cbz(path: &Path, pages: usize) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for i in 0..pages {
            w.start_file(format!("{i:02}.png"), zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut w, &PNG).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn importa_comic_fotos_y_rechaza_lo_demas() {
        let root = tmp("root");
        let inbox = tmp("inbox");
        let lib = Library::new(root.clone());
        cbz(&inbox.join("Mi_comic.cbz"), 3);
        std::fs::write(inbox.join("a.png"), PNG).unwrap();
        std::fs::write(inbox.join("b.png"), PNG).unwrap();
        std::fs::write(inbox.join("nota.txt"), "hola").unwrap();
        std::fs::write(inbox.join("roto.cbz"), "no es un zip").unwrap();
        let inc = |n: &str| Incoming { path: inbox.join(n), name: n.into() };
        let r = lib.import(vec![inc("Mi_comic.cbz"), inc("a.png"), inc("nota.txt"), inc("b.png"), inc("roto.cbz")], "Fotos");
        assert_eq!(r.items.len(), 2, "{:?}", r.failed);
        let comic = r.items.iter().find(|i| i.kind == Kind::Comic).unwrap();
        assert_eq!((comic.title.as_str(), comic.pages), ("Mi comic", 3));
        let album = r.items.iter().find(|i| i.kind == Kind::Album).unwrap();
        assert_eq!((album.title.as_str(), album.pages), ("Fotos", 2));
        let failed: Vec<_> = r.failed.iter().map(|f| f.0.as_str()).collect();
        assert_eq!(failed, ["nota.txt", "roto.cbz"]);
        // Nada se queda a medias: ni en la importación ni una carpeta del cómic roto.
        assert_eq!(std::fs::read_dir(&inbox).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 3, "dos carpetas y library.json");

        // Progreso y orden: lo último abierto primero.
        lib.progress(&comic.id, 7).unwrap();
        let again = Library::new(root.clone());
        let list = again.list();
        assert_eq!(list[0].id, comic.id);
        assert_eq!(list[0].page, 2, "no pasa de la última página");

        // Borrar quita la carpeta.
        let path = again.remove(&comic.id).unwrap().unwrap();
        assert!(!Path::new(&path).exists());
        assert_eq!(again.list().len(), 1);
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(inbox).unwrap();
    }

    #[test]
    fn titulos() {
        assert_eq!(title_of("Spider_Man 001.cbz"), "Spider Man 001");
        assert_eq!(title_of("historia.imgxstory"), "historia");
    }
}
