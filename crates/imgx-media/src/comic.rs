//! Cómics propios: carpetas que crea y ordena imgxplorer a partir de imágenes sueltas.
//!
//! Un cómic propio es una carpeta normal (se puede comprimir, mover y abrir con cualquier visor)
//! con las páginas numeradas y un marcador:
//!
//! ```text
//! Mi cómic/
//!   0001 - portada_ia.png
//!   0002 - escaneo 7.jpg
//!   .imgxplorer-comic.json   {"format":"imgxcomic","version":1,"title":"Mi cómic","created":…}
//!   .quitadas/               páginas quitadas (la app no las lee; se recuperan a mano)
//! ```
//!
//! El número fija el orden; el resto es el nombre original de la imagen. Al reordenar solo
//! cambia el número. Las imágenes de origen nunca se tocan: se copian.

use crate::{MediaError, Source, is_image_name};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

pub const MARKER: &str = ".imgxplorer-comic.json";
pub const REMOVED_DIR: &str = ".quitadas";
pub const FORMAT: &str = "imgxcomic";
pub const VERSION: u32 = 1;
/// Prefijo de los nombres temporales al reordenar (ocultos: la app no los lista). Lleva el
/// nombre original detrás para poder recuperarlo si el orden se corta a medias.
const TMP: &str = ".imgx-tmp-";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComicInfo {
    pub format: String,
    pub version: u32,
    pub title: String,
    /// Segundos desde 1970.
    #[serde(default)]
    pub created: u64,
}

/// Una página del orden nuevo: una que ya está en el cómic (por su nombre) o una imagen de fuera
/// que se copia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum Page {
    Existing { entry: String },
    New { path: PathBuf },
}

/// El marcador de un cómic propio, si `dir` lo es.
pub fn info(dir: &Path) -> Option<ComicInfo> {
    let text = std::fs::read_to_string(dir.join(MARKER)).ok()?;
    let info: ComicInfo = serde_json::from_str(&text).ok()?;
    (info.format == FORMAT).then_some(info)
}

pub fn is_comic(dir: &Path) -> bool {
    info(dir).is_some()
}

/// Cifras del número de página: cuatro, o más si hacen falta.
pub fn digits(count: usize) -> usize {
    count.to_string().len().max(4)
}

/// Quita el número que pone imgxplorer delante (`0003 - x.jpg` → `x.jpg`).
pub fn strip_number(name: &str) -> &str {
    let digits = name.bytes().take_while(u8::is_ascii_digit).count();
    match name[digits..].strip_prefix(" - ") {
        Some(rest) if digits > 0 && !rest.is_empty() => rest,
        _ => name,
    }
}

/// Nombre de la página `n` (desde 1): número, guion y el nombre original sin carpetas.
pub fn page_name(n: usize, width: usize, original: &str) -> String {
    let base = original.rsplit(['/', '\\']).next().unwrap_or(original);
    format!("{n:0width$} - {}", strip_number(base))
}

/// Un título usable como nombre de carpeta en Linux, macOS y Windows.
pub fn folder_name(title: &str) -> String {
    let clean: String = title.chars().map(|c| if "/\\:*?\"<>|".contains(c) || c.is_control() { ' ' } else { c }).collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = clean.trim_matches(|c: char| c == '.' || c == ' ');
    let clean: String = clean.chars().take(120).collect();
    if clean.is_empty() { "Comic".into() } else { clean }
}

/// Carpeta nueva para `title` dentro de `parent`: «Mi cómic», o «Mi cómic (2)» si ya existe.
pub fn unique_dir(parent: &Path, title: &str) -> PathBuf {
    let base = folder_name(title);
    let mut dir = parent.join(&base);
    let mut n = 2;
    while dir.exists() {
        dir = parent.join(format!("{base} ({n})"));
        n += 1;
    }
    dir
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn write_marker(dir: &Path, title: &str) -> Result<(), MediaError> {
    let info = ComicInfo { format: FORMAT.into(), version: VERSION, title: title.trim().into(), created: now() };
    let text = serde_json::to_string_pretty(&info).map_err(|e| MediaError::Archive(e.to_string()))?;
    std::fs::write(dir.join(MARKER), text)?;
    Ok(())
}

fn check_image(path: &Path) -> Result<String, MediaError> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if !is_image_name(&name) {
        return Err(MediaError::Unsupported(name));
    }
    if !path.is_file() {
        return Err(MediaError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, path.display().to_string())));
    }
    Ok(name)
}

/// Crea un cómic en una carpeta nueva dentro de `parent`, copiando `files` en ese orden.
/// Si algo falla, no deja la carpeta a medias.
pub fn create(parent: &Path, title: &str, files: &[PathBuf]) -> Result<PathBuf, MediaError> {
    let names = files.iter().map(|f| check_image(f)).collect::<Result<Vec<_>, _>>()?;
    let dir = unique_dir(parent, title);
    std::fs::create_dir_all(&dir)?;
    let width = digits(files.len());
    let done = (|| {
        for (i, (f, name)) in files.iter().zip(&names).enumerate() {
            std::fs::copy(f, dir.join(page_name(i + 1, width, name)))?;
        }
        write_marker(&dir, title)
    })();
    if let Err(e) = done {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(e);
    }
    Ok(dir)
}

/// Crea un cómic con las páginas de otra fuente (un CBZ, una carpeta cualquiera…) en el orden
/// `order` (índices de la fuente). Devuelve la carpeta y los nombres nuevos de cada entrada.
pub fn from_source(source: &dyn Source, order: &[usize], parent: &Path, title: &str) -> Result<(PathBuf, BTreeMap<String, String>), MediaError> {
    let entries = source.entries();
    if let Some(&bad) = order.iter().find(|&&i| i >= entries.len()) {
        return Err(MediaError::OutOfRange(bad));
    }
    let dir = unique_dir(parent, title);
    std::fs::create_dir_all(&dir)?;
    let width = digits(order.len());
    let mut map = BTreeMap::new();
    let done = (|| {
        for (n, &i) in order.iter().enumerate() {
            let name = page_name(n + 1, width, &entries[i].name);
            std::fs::write(dir.join(&name), source.read(i)?)?;
            map.insert(entries[i].name.clone(), name);
        }
        write_marker(&dir, title)
    })();
    if let Err(e) = done {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(e);
    }
    Ok((dir, map))
}

/// Una entrada de la carpeta del cómic: relativa, sin `..` ni partes ocultas.
fn entry_path(dir: &Path, entry: &str) -> Result<PathBuf, MediaError> {
    let bad = entry.is_empty() || entry.split(['/', '\\']).any(|p| p.is_empty() || p == ".." || p.starts_with('.'));
    let path = dir.join(entry);
    if bad || !path.is_file() {
        return Err(MediaError::Unsupported(format!("página desconocida: {entry}")));
    }
    Ok(path)
}

/// Destino libre dentro de `dir` para `name` (añade « (2)»… si ya existe).
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    if !path.exists() {
        return path;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).expect("siempre hay un nombre libre")
}

/// Ordena el cómic: `pages` es el orden nuevo completo (las que ya estaban y las nuevas, que se
/// copian) y `removed` las que se quitan (van a `.quitadas/`). Devuelve el nombre nuevo de cada
/// página que ya estaba, para que la animación y los marcadores las sigan.
///
/// Se renombra en dos pasos (primero a nombres temporales) para que ninguna pise a otra. Si se
/// corta a medias, [`recover`] devuelve las temporales a su nombre.
pub fn arrange(dir: &Path, pages: &[Page], removed: &[String]) -> Result<BTreeMap<String, String>, MediaError> {
    if !is_comic(dir) {
        return Err(MediaError::Unsupported("no es un cómic propio".into()));
    }
    recover(dir)?;
    // Comprobar todo antes de tocar nada.
    let mut seen = HashSet::new();
    for p in pages {
        match p {
            Page::Existing { entry } => {
                entry_path(dir, entry)?;
                if !seen.insert(entry.as_str()) {
                    return Err(MediaError::Unsupported(format!("página repetida: {entry}")));
                }
            }
            Page::New { path } => {
                check_image(path)?;
            }
        }
    }
    for r in removed {
        entry_path(dir, r)?;
        if !seen.insert(r.as_str()) {
            return Err(MediaError::Unsupported(format!("página repetida: {r}")));
        }
    }

    // Las quitadas, a su carpeta (con su nombre original).
    if !removed.is_empty() {
        let bin = dir.join(REMOVED_DIR);
        std::fs::create_dir_all(&bin)?;
        for r in removed {
            let base = r.rsplit(['/', '\\']).next().unwrap_or(r);
            std::fs::rename(dir.join(r), free_name(&bin, strip_number(base)))?;
        }
    }
    // Paso 1: las que se quedan, a nombres temporales.
    let mut tmp = Vec::with_capacity(pages.len());
    for (k, p) in pages.iter().enumerate() {
        if let Page::Existing { entry } = p {
            let base = entry.rsplit(['/', '\\']).next().unwrap_or(entry);
            let t = dir.join(format!("{TMP}{k}-{base}"));
            std::fs::rename(dir.join(entry), &t)?;
            tmp.push(Some(t));
        } else {
            tmp.push(None);
        }
    }
    // Paso 2: cada una a su número.
    let width = digits(pages.len());
    let mut map = BTreeMap::new();
    for (k, p) in pages.iter().enumerate() {
        match p {
            Page::Existing { entry } => {
                let name = page_name(k + 1, width, entry);
                std::fs::rename(tmp[k].as_ref().expect("temporal del paso 1"), dir.join(&name))?;
                map.insert(entry.clone(), name);
            }
            Page::New { path } => {
                let original = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                std::fs::copy(path, dir.join(page_name(k + 1, width, &original)))?;
            }
        }
    }
    Ok(map)
}

/// Devuelve a su nombre las páginas que se quedaron con nombre temporal (un orden cortado a
/// medias). Así ninguna desaparece de la vista.
pub fn recover(dir: &Path) -> Result<usize, MediaError> {
    let mut n = 0;
    for item in std::fs::read_dir(dir)? {
        let name = item?.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix(TMP) else { continue };
        let Some((_, original)) = rest.split_once('-') else { continue };
        if original.is_empty() {
            continue;
        }
        std::fs::rename(dir.join(&name), free_name(dir, original))?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folder::FolderSource;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("imgx-comic-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn names(dir: &Path) -> Vec<String> {
        FolderSource::open(dir).unwrap().entries().iter().map(|e| e.name.clone()).collect()
    }

    #[test]
    fn nombres() {
        assert_eq!(page_name(3, 4, "escaneo 7.jpg"), "0003 - escaneo 7.jpg");
        assert_eq!(page_name(12, 4, "0003 - escaneo 7.jpg"), "0012 - escaneo 7.jpg");
        assert_eq!(page_name(1, 5, "sub/dir/a.png"), "00001 - a.png");
        assert_eq!(strip_number("2024 - 01 - x.png"), "01 - x.png");
        assert_eq!(strip_number("007.jpg"), "007.jpg");
        assert_eq!(strip_number("0001 - "), "0001 - ");
        assert_eq!(digits(12), 4);
        assert_eq!(digits(12_345), 5);
        assert_eq!(folder_name(" a/b:c*? . "), "a b c");
        assert_eq!(folder_name("..."), "Comic");
    }

    #[test]
    fn crear_ordenar_y_quitar() {
        let base = tmp("crear");
        let src = base.join("origen");
        std::fs::create_dir_all(&src).unwrap();
        for n in ["z.png", "a.jpg", "m.webp", "extra.png"] {
            std::fs::write(src.join(n), n).unwrap();
        }
        let files: Vec<PathBuf> = ["z.png", "a.jpg", "m.webp"].iter().map(|n| src.join(n)).collect();
        let dir = create(&base, "Mi cómic", &files).unwrap();
        assert_eq!(dir.file_name().unwrap(), "Mi cómic");
        assert_eq!(names(&dir), ["0001 - z.png", "0002 - a.jpg", "0003 - m.webp"]);
        assert_eq!(info(&dir).unwrap().title, "Mi cómic");
        // Las originales siguen ahí.
        assert!(src.join("z.png").exists());
        // Otro con el mismo nombre no pisa al primero.
        assert_eq!(create(&base, "Mi cómic", &files[..1]).unwrap().file_name().unwrap(), "Mi cómic (2)");

        // Nuevo orden: a, extra (nueva), z; m se quita.
        let pages = [
            Page::Existing { entry: "0002 - a.jpg".into() },
            Page::New { path: src.join("extra.png") },
            Page::Existing { entry: "0001 - z.png".into() },
        ];
        let map = arrange(&dir, &pages, &["0003 - m.webp".into()]).unwrap();
        assert_eq!(names(&dir), ["0001 - a.jpg", "0002 - extra.png", "0003 - z.png"]);
        assert_eq!(map["0002 - a.jpg"], "0001 - a.jpg");
        assert_eq!(map["0001 - z.png"], "0003 - z.png");
        assert_eq!(std::fs::read_to_string(dir.join("0003 - z.png")).unwrap(), "z.png", "cada página conserva su contenido");
        assert!(dir.join(REMOVED_DIR).join("m.webp").exists());

        // Errores: nada cambia.
        assert!(arrange(&dir, &[Page::Existing { entry: "../x.png".into() }], &[]).is_err());
        assert!(arrange(&dir, &[Page::Existing { entry: "0001 - a.jpg".into() }, Page::Existing { entry: "0001 - a.jpg".into() }], &[]).is_err());
        assert!(arrange(&src, &[], &[]).is_err(), "solo en cómics propios");
        assert_eq!(names(&dir).len(), 3);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn recupera_un_orden_cortado() {
        let base = tmp("recuperar");
        let dir = base.join("c");
        std::fs::create_dir_all(&dir).unwrap();
        write_marker(&dir, "c").unwrap();
        std::fs::write(dir.join(".imgx-tmp-3-0002 - b.png"), "b").unwrap();
        std::fs::write(dir.join("0001 - a.png"), "a").unwrap();
        assert_eq!(recover(&dir).unwrap(), 1);
        assert_eq!(names(&dir), ["0001 - a.png", "0002 - b.png"]);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn desde_otra_fuente() {
        let base = tmp("fuente");
        let src = base.join("tomo");
        std::fs::create_dir_all(&src).unwrap();
        for n in ["1.png", "2.png", "10.png"] {
            std::fs::write(src.join(n), n).unwrap();
        }
        let s = FolderSource::open(&src).unwrap();
        // Orden natural de la fuente: 1, 2, 10. Lo queremos: 10, 1, 2.
        let (dir, map) = from_source(&s, &[2, 0, 1], &base, "Tomo").unwrap();
        assert_eq!(names(&dir), ["0001 - 10.png", "0002 - 1.png", "0003 - 2.png"]);
        assert_eq!(map["10.png"], "0001 - 10.png");
        assert!(is_comic(&dir));
        assert!(from_source(&s, &[7], &base, "Mal").is_err());
        std::fs::remove_dir_all(base).unwrap();
    }
}
