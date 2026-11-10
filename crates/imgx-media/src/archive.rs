//! Fuentes de comprimidos: zip/cbz, 7z/cb7, rar/cbr.
//!
//! Zip permite acceso aleatorio. En 7z y rar (sobre todo si son *sólidos*) llegar a una
//! entrada obliga a descomprimir las anteriores; para que la lectura secuencial (pasar
//! páginas, miniaturas) no sea cuadrática, al buscar una entrada guardamos en caché las
//! que quedan cerca de ella (`WINDOW_BEFORE`..`WINDOW_AFTER`).

use crate::{Entry, MediaError, Source, is_image_name, is_visible_path, natural};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

const WINDOW_BEFORE: usize = 2;
const WINDOW_AFTER: usize = 10;
const CACHE_BUDGET: usize = 384 << 20;

fn title_of(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into())
}

fn arc_err(e: impl std::fmt::Display) -> MediaError {
    MediaError::Archive(e.to_string())
}

/// Ordena las entradas y devuelve también, para cada una, su índice original en el comprimido.
fn sorted(mut list: Vec<(Entry, usize)>) -> (Vec<Entry>, Vec<usize>) {
    list.sort_by(|a, b| natural::natural_cmp(&a.0.name, &b.0.name));
    list.into_iter().unzip()
}

fn wanted(name: &str) -> bool {
    is_visible_path(name) && is_image_name(name)
}

/// LRU de bytes ya extraídos, limitada por tamaño total.
#[derive(Default)]
struct ByteCache {
    map: HashMap<usize, Vec<u8>>,
    order: VecDeque<usize>,
    bytes: usize,
}

impl ByteCache {
    fn get(&mut self, i: usize) -> Option<Vec<u8>> {
        let v = self.map.get(&i)?.clone();
        self.order.retain(|&k| k != i);
        self.order.push_back(i);
        Some(v)
    }
    fn put(&mut self, i: usize, data: Vec<u8>) {
        if let Some(old) = self.map.insert(i, data) {
            self.bytes -= old.len();
            self.order.retain(|&k| k != i);
        }
        self.bytes += self.map[&i].len();
        self.order.push_back(i);
        while self.bytes > CACHE_BUDGET && self.order.len() > 1 {
            let k = self.order.pop_front().unwrap();
            if let Some(v) = self.map.remove(&k) {
                self.bytes -= v.len();
            }
        }
    }
}

fn in_window(i: usize, target: usize) -> bool {
    i + WINDOW_BEFORE >= target && i <= target + WINDOW_AFTER
}

// ───────────────────────────── zip ─────────────────────────────

pub struct ZipSource {
    title: String,
    entries: Vec<Entry>,
    raw_index: Vec<usize>,
    zip: Mutex<zip::ZipArchive<File>>,
}

impl ZipSource {
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(arc_err)?;
        let mut list = Vec::new();
        for i in 0..zip.len() {
            let f = zip.by_index_raw(i).map_err(arc_err)?;
            let name = f.name().map_err(arc_err)?.replace('\\', "/");
            if f.is_file() && wanted(&name) {
                list.push((Entry { name, size: f.size() }, i));
            }
        }
        let (entries, raw_index) = sorted(list);
        Ok(Self { title: title_of(path), entries, raw_index, zip: Mutex::new(zip) })
    }
}

impl Source for ZipSource {
    fn title(&self) -> &str {
        &self.title
    }
    fn entries(&self) -> &[Entry] {
        &self.entries
    }
    fn read(&self, index: usize) -> Result<Vec<u8>, MediaError> {
        let raw = *self.raw_index.get(index).ok_or(MediaError::OutOfRange(index))?;
        let mut zip = self.zip.lock().unwrap();
        let mut f = zip.by_index(raw).map_err(arc_err)?;
        let mut out = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut out)?;
        Ok(out)
    }
}

// ───────────────────────────── 7z ─────────────────────────────

pub struct SevenZSource {
    title: String,
    entries: Vec<Entry>,
    /// Nombre (tal como está en el comprimido) → índice ordenado.
    by_name: HashMap<String, usize>,
    /// Bloque de cada entrada ordenada.
    block_of: Vec<usize>,
    inner: Mutex<(sevenz_rust2::Archive, File)>,
    cache: Mutex<ByteCache>,
}

impl SevenZSource {
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        let archive = sevenz_rust2::Archive::open(path).map_err(arc_err)?;
        let mut list = Vec::new();
        for (i, f) in archive.files.iter().enumerate() {
            let name = f.name.replace('\\', "/");
            if f.has_stream && !f.is_directory && wanted(&name) {
                list.push((Entry { name, size: f.size }, i));
            }
        }
        let (entries, raw) = sorted(list);
        let by_name = raw.iter().enumerate().map(|(s, &i)| (archive.files[i].name.clone(), s)).collect();
        let block_of = raw.iter().map(|&i| archive.stream_map.file_block_index[i].unwrap_or(0)).collect();
        log::debug!("7z {}: {} entradas, sólido={}", path.display(), entries.len(), archive.is_solid);
        Ok(Self {
            title: title_of(path),
            entries,
            by_name,
            block_of,
            inner: Mutex::new((archive, File::open(path)?)),
            cache: Mutex::default(),
        })
    }
}

impl Source for SevenZSource {
    fn title(&self) -> &str {
        &self.title
    }
    fn entries(&self) -> &[Entry] {
        &self.entries
    }
    fn read(&self, index: usize) -> Result<Vec<u8>, MediaError> {
        let block = *self.block_of.get(index).ok_or(MediaError::OutOfRange(index))?;
        if let Some(v) = self.cache.lock().unwrap().get(index) {
            return Ok(v);
        }
        let mut inner = self.inner.lock().unwrap();
        // Otro hilo pudo extraerla mientras esperábamos el candado.
        if let Some(v) = self.cache.lock().unwrap().get(index) {
            return Ok(v);
        }
        let (archive, file) = &mut *inner;
        let password = sevenz_rust2::Password::empty();
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(4) as u32);
        let mut found = None;
        sevenz_rust2::BlockDecoder::new(threads, block, archive, &password, file)
            .for_each_entries(&mut |entry, reader| {
                let Some(&s) = self.by_name.get(&entry.name) else { return Ok(true) };
                if !in_window(s, index) {
                    // Pasada la ventana tras encontrarla: no hace falta seguir descomprimiendo.
                    return Ok(found.is_none() || s < index);
                }
                let mut data = Vec::with_capacity(entry.size as usize);
                reader.read_to_end(&mut data)?;
                if s == index {
                    found = Some(data.clone());
                }
                self.cache.lock().unwrap().put(s, data);
                Ok(true)
            })
            .map_err(arc_err)?;
        found.ok_or_else(|| MediaError::Archive(format!("entrada {} no encontrada", self.entries[index].name)))
    }
}

// ───────────────────────────── rar ─────────────────────────────

pub struct RarSource {
    path: std::path::PathBuf,
    title: String,
    entries: Vec<Entry>,
    by_name: HashMap<String, usize>,
    /// Serializa el acceso: cada lectura recorre el comprimido desde el principio.
    lock: Mutex<()>,
    cache: Mutex<ByteCache>,
}

fn rar_name(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

impl RarSource {
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        let listing = unrar::Archive::new(path).open_for_listing().map_err(arc_err)?;
        let mut list = Vec::new();
        for (i, h) in listing.enumerate() {
            let h = h.map_err(arc_err)?;
            let name = rar_name(&h.filename);
            if h.is_file() && wanted(&name) {
                list.push((Entry { name, size: h.unpacked_size }, i));
            }
        }
        let (entries, _) = sorted(list);
        let by_name = entries.iter().enumerate().map(|(s, e)| (e.name.clone(), s)).collect();
        Ok(Self {
            path: path.into(),
            title: title_of(path),
            entries,
            by_name,
            lock: Mutex::new(()),
            cache: Mutex::default(),
        })
    }
}

impl Source for RarSource {
    fn title(&self) -> &str {
        &self.title
    }
    fn entries(&self) -> &[Entry] {
        &self.entries
    }
    fn read(&self, index: usize) -> Result<Vec<u8>, MediaError> {
        if index >= self.entries.len() {
            return Err(MediaError::OutOfRange(index));
        }
        if let Some(v) = self.cache.lock().unwrap().get(index) {
            return Ok(v);
        }
        let _guard = self.lock.lock().unwrap();
        if let Some(v) = self.cache.lock().unwrap().get(index) {
            return Ok(v);
        }
        let mut found = None;
        let mut cursor = unrar::Archive::new(&self.path).open_for_processing().map_err(arc_err)?;
        while let Some(header) = cursor.read_header().map_err(arc_err)? {
            let slot = header.entry().is_file().then(|| self.by_name.get(&rar_name(&header.entry().filename))).flatten();
            match slot {
                Some(&s) if in_window(s, index) => {
                    let (data, next) = header.read().map_err(arc_err)?;
                    if s == index {
                        found = Some(data.clone());
                    }
                    self.cache.lock().unwrap().put(s, data);
                    cursor = next;
                }
                Some(&s) if found.is_some() && s > index => break,
                _ => cursor = header.skip().map_err(arc_err)?,
            }
        }
        found.ok_or_else(|| MediaError::Archive(format!("entrada {} no encontrada", self.entries[index].name)))
    }
}
