//! Historias `.imgxstory`: todo un cómic animado en un solo archivo para compartirlo.
//!
//! Es un zip con:
//!
//! | Entrada          | Qué es                                                              |
//! |------------------|---------------------------------------------------------------------|
//! | `manifest.json`  | [`Manifest`]: formato, versión, título, variante, modo de lectura…    |
//! | `project.imgx`   | el proyecto de siempre (formato `.imgx`), sin cambios                |
//! | `computed.json`  | [`Computed`]: viñetas detectadas y niveles, ya calculados             |
//! | `pages/…`        | las imágenes (JPEG, PNG, WebP o GIF), con su nombre en el cómic        |
//! | `sounds.json`    | [`StorySound`]s: los sonidos propios que usa                          |
//! | `sounds/…`       | sus archivos (los de serie no viajan: los fabrica el motor)            |
//! | `cover.jpg`      | portada pequeña de la primera página                                  |
//!
//! Páginas y sonidos van sin comprimir (ya lo están): se pueden leer sueltos sin descomprimir
//! el resto. La especificación completa está en `docs/imgxstory.md`.

pub mod pages;
pub mod prepare;

use imgx_core::Project;
use imgx_media::levels::Levels;
use imgx_media::panels::PanelRect;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub use imgx_media::{STORY_EXT as EXT, STORY_PAGES as PAGES};

pub const FORMAT: &str = "imgxstory";
pub const VERSION: u32 = 1;
const MANIFEST: &str = "manifest.json";
const PROJECT: &str = "project.imgx";
const COMPUTED: &str = "computed.json";
const SOUNDS: &str = "sounds.json";
const SOUNDS_DIR: &str = "sounds/";
const COVER: &str = "cover.jpg";
const COVER_SIDE: u32 = 512;
const COVER_BG: [u8; 3] = [0x15, 0x18, 0x1d];
/// Tamaño máximo de cada JSON de la historia (evita leer un zip malicioso enorme).
const MAX_JSON: u64 = 64 << 20;
/// Tamaño máximo de un sonido dentro de la historia.
const MAX_SOUND: u64 = 1 << 30;

/// Para qué se preparó la historia. Las dos se abren en cualquier sitio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    /// Páginas con su calidad original.
    #[default]
    Desktop,
    /// Páginas reducidas para que pese poco.
    Mobile,
}

/// Modo de lectura con el que se pensó (el visor lo usa la primera vez que se abre).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Reading {
    /// `window`, `width` o `height`.
    pub fit: String,
    pub crop: bool,
    pub double: bool,
    /// En doble página, la portada sola.
    pub cover: bool,
    /// Tira vertical (webtoon).
    pub strip: bool,
    /// Sentido manga (de derecha a izquierda).
    pub rtl: bool,
}

impl Default for Reading {
    fn default() -> Self {
        Self { fit: "window".into(), crop: false, double: false, cover: true, strip: false, rtl: false }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    /// Versión de imgxplorer que la creó (informativo).
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub variant: Variant,
    #[serde(default)]
    pub title: String,
    /// Texto libre de quien la comparte (autoría, voces, música…).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub credits: String,
    /// Segundos desde 1970.
    #[serde(default)]
    pub created: u64,
    /// Idioma de la interfaz con que se creó (`es`, `en`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default)]
    pub pages: usize,
    #[serde(default)]
    pub reading: Reading,
    /// Proporción (ancho / alto) del vídeo para el que se pensó la cámara.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authored_aspect: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            app: String::new(),
            variant: Variant::Desktop,
            title: String::new(),
            credits: String::new(),
            created: 0,
            lang: None,
            pages: 0,
            reading: Reading::default(),
            authored_aspect: None,
            cover: None,
        }
    }
}

/// Lo que ya se calculó de una página (así quien la abre no tiene que detectar nada).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageData {
    /// Viñetas detectadas (`[x, y, ancho, alto]` normalizados; vacío si no hay al menos dos).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panels: Option<Vec<PanelRect>>,
    /// Niveles de «limpiar escaneo».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<Levels>,
    /// Recuadro con contenido, sin los márgenes (para «✂ contenido»). [`NO_MARGIN`] si no hay
    /// margen que quitar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<PanelRect>,
}

/// Recuadro de `content` cuando la página no tiene márgenes que quitar.
pub const NO_MARGIN: PanelRect = [0.0, 0.0, 1.0, 1.0];

impl PageData {
    pub fn is_empty(&self) -> bool {
        self.panels.is_none() && self.levels.is_none() && self.content.is_none()
    }

    /// El recuadro tal como lo usa la app: `None` si no hay margen que quitar.
    pub fn content_box(&self) -> Option<Option<PanelRect>> {
        self.content.map(|c| if c == NO_MARGIN { None } else { Some(c) })
    }
}

/// `computed.json`: datos por página, por su nombre dentro de la historia.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Computed {
    #[serde(default)]
    pub pages: BTreeMap<String, PageData>,
}

/// Un sonido de la historia (`sounds.json`): los mismos campos que la biblioteca de la app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorySound {
    /// Huella del contenido: la que usa el proyecto.
    pub id: String,
    pub name: String,
    /// Archivo dentro de `sounds/`.
    pub file: String,
    /// `imported`, `recorded` o `generated`.
    pub kind: String,
    pub seconds: f64,
}

/// Lo que hace falta para escribir una historia.
pub struct ExportInput<'a> {
    /// Nombres de las páginas en el cómic, en orden.
    pub pages: Vec<String>,
    /// Lee la página `i` de `pages` y devuelve sus bytes y lo ya calculado de ella.
    pub read_page: &'a (dyn Fn(usize) -> Result<(Vec<u8>, PageData), String> + Sync),
    /// Proyecto ya preparado ([`prepare::project`]).
    pub project: Project,
    /// Sonidos que viajan.
    pub sounds: Vec<StorySound>,
    /// Lee el archivo del sonido `i` de `sounds`.
    pub read_sound: &'a (dyn Fn(usize) -> Result<Vec<u8>, String> + Sync),
    /// Título, variante, créditos, lectura… (formato, versión, páginas, fecha y
    /// portada los pone [`write`]).
    pub manifest: Manifest,
}

/// Cómo va una exportación: `false` en el retorno la cancela.
pub type Progress<'a> = &'a mut dyn FnMut(usize, usize) -> bool;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Written {
    pub pages: usize,
    pub sounds: usize,
    /// Páginas que se convirtieron o reescalaron.
    pub converted: usize,
    pub bytes: u64,
}

pub const CANCELLED: &str = "cancelado";

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Un nombre libre para `name` entre los ya usados (`003.png`, `003-2.png`…).
fn unique(name: String, used: &mut std::collections::HashSet<String>) -> String {
    if used.insert(name.clone()) {
        return name;
    }
    let slash = name.rfind('/').map_or(0, |i| i + 1);
    let (stem, ext) = match name[slash..].rfind('.') {
        Some(d) if d > 0 => (&name[..slash + d], &name[slash + d..]),
        _ => (name.as_str(), ""),
    };
    (2..).map(|n| format!("{stem}-{n}{ext}")).find(|c| used.insert(c.clone())).expect("siempre hay un nombre libre")
}

/// Escribe la historia en `path` (en un temporal que se renombra al terminar: un corte a medias
/// nunca deja un archivo roto).
pub fn write(path: &Path, input: ExportInput, progress: Progress) -> Result<Written, String> {
    if input.pages.is_empty() {
        return Err("no hay páginas que exportar".into());
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let result = write_to(&tmp, input, progress);
    match result {
        Ok(mut w) => {
            std::fs::rename(&tmp, path).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                e.to_string()
            })?;
            w.bytes = std::fs::metadata(path).map_or(0, |m| m.len());
            Ok(w)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn write_to(tmp: &Path, input: ExportInput, progress: Progress) -> Result<Written, String> {
    let ExportInput { pages, read_page, mut project, sounds, read_sound, mut manifest } = input;
    let file = std::fs::File::create(tmp).map_err(|e| e.to_string())?;
    let mut z = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(true);
    let deflated = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let zerr = |e: zip::result::ZipError| e.to_string();
    let total = pages.len() + sounds.len();

    let mut renamed = BTreeMap::new();
    let mut sizes = BTreeMap::new();
    let mut computed = Computed::default();
    let mut used = std::collections::HashSet::new();
    let mut converted = 0;
    for (i, original) in pages.iter().enumerate() {
        if !progress(i, total) {
            return Err(CANCELLED.into());
        }
        let (bytes, data) = read_page(i)?;
        if i == 0 {
            let cover = imgx_media::thumbnail_jpeg(original, &bytes, COVER_SIDE, COVER_BG).map_err(|e| e.to_string())?;
            z.start_file(COVER, stored).map_err(zerr)?;
            z.write_all(&cover).map_err(|e| e.to_string())?;
            manifest.cover = Some(COVER.into());
        }
        let page = pages::convert(original, bytes, manifest.variant).map_err(|e| format!("{original}: {e}"))?;
        let name = unique(page.name, &mut used);
        if name != *original {
            converted += 1;
            renamed.insert(original.clone(), name.clone());
        } else if page.size.is_some() {
            converted += 1;
        }
        if let Some(s) = page.size {
            sizes.insert(name.clone(), s);
        }
        if !data.is_empty() {
            computed.pages.insert(name.clone(), data);
        }
        z.start_file(format!("{PAGES}{name}"), stored).map_err(zerr)?;
        z.write_all(&page.bytes).map_err(|e| e.to_string())?;
    }

    let mut list = Vec::with_capacity(sounds.len());
    for (k, sound) in sounds.iter().enumerate() {
        if !progress(pages.len() + k, total) {
            return Err(CANCELLED.into());
        }
        check_sound_name(&sound.file)?;
        let bytes = read_sound(k).map_err(|e| format!("{}: {e}", sound.name))?;
        z.start_file(format!("{SOUNDS_DIR}{}", sound.file), stored).map_err(zerr)?;
        z.write_all(&bytes).map_err(|e| e.to_string())?;
        list.push(sound.clone());
    }

    project.rename_entries(&renamed);
    for it in &mut project.items {
        if let Some(s) = sizes.get(&it.entry) {
            it.size = *s;
        }
    }
    manifest.format = FORMAT.into();
    manifest.version = VERSION;
    manifest.pages = pages.len();
    manifest.created = now();
    for (name, text) in [(MANIFEST, pretty(&manifest)), (PROJECT, project.to_json()), (COMPUTED, pretty(&computed)), (SOUNDS, pretty(&list))] {
        z.start_file(name, deflated).map_err(zerr)?;
        z.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    }
    z.finish().map_err(zerr)?.flush().map_err(|e| e.to_string())?;
    progress(total, total);
    Ok(Written { pages: pages.len(), sounds: list.len(), converted, bytes: 0 })
}

fn pretty<T: Serialize>(v: &T) -> String {
    serde_json::to_string_pretty(v).expect("serializable")
}

fn check_sound_name(file: &str) -> Result<(), String> {
    if file.is_empty() || file.contains(['/', '\\']) || file.starts_with('.') {
        return Err(format!("nombre de sonido no válido: {file}"));
    }
    Ok(())
}

/// Una historia leída (sin sus páginas, que se leen con `imgx_media::open_path`, ni el audio).
#[derive(Debug, Clone)]
pub struct Story {
    pub manifest: Manifest,
    pub project: Project,
    pub computed: Computed,
    pub sounds: Vec<StorySound>,
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    zip::ZipArchive::new(file).map_err(|_| "no es una historia .imgxstory (zip)".to_string())
}

fn text(z: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Option<String>, String> {
    let entry = match z.by_name(name) {
        Ok(e) => e,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if entry.size() > MAX_JSON {
        return Err(format!("{name} es demasiado grande"));
    }
    let mut s = String::new();
    entry.take(MAX_JSON).read_to_string(&mut s).map_err(|e| format!("{name}: {e}"))?;
    Ok(Some(s))
}

/// Lee y comprueba una historia.
pub fn read(path: &Path) -> Result<Story, String> {
    let mut z = open_zip(path)?;
    let manifest: Manifest = serde_json::from_str(&text(&mut z, MANIFEST)?.ok_or("a la historia le falta manifest.json")?).map_err(|e| format!("manifiesto inválido: {e}"))?;
    if manifest.format != FORMAT {
        return Err("no es una historia .imgxstory".into());
    }
    if manifest.version > VERSION {
        return Err(format!("historia de una versión más nueva ({}); actualiza imgxplorer", manifest.version));
    }
    let project = Project::from_json(&text(&mut z, PROJECT)?.ok_or("a la historia le falta project.imgx")?).map_err(|e| format!("project.imgx: {e}"))?;
    let computed = match text(&mut z, COMPUTED)? {
        Some(t) => serde_json::from_str(&t).map_err(|e| format!("computed.json inválido: {e}"))?,
        None => Computed::default(),
    };
    let sounds: Vec<StorySound> = match text(&mut z, SOUNDS)? {
        Some(t) => serde_json::from_str(&t).map_err(|e| format!("sounds.json inválido: {e}"))?,
        None => Vec::new(),
    };
    for s in &sounds {
        check_sound_name(&s.file)?;
        let size = z.by_name(&format!("{SOUNDS_DIR}{}", s.file)).map_err(|_| format!("a la historia le falta el sonido {}", s.file))?.size();
        if size > MAX_SOUND {
            return Err(format!("{} es demasiado grande", s.file));
        }
    }
    if !z.file_names().any(|n| n.is_ok_and(|n| n.starts_with(PAGES))) {
        return Err("la historia no tiene páginas".into());
    }
    Ok(Story { manifest, project, computed, sounds })
}

/// Los bytes de un sonido de la historia.
pub fn read_sound(path: &Path, sound: &StorySound) -> Result<Vec<u8>, String> {
    check_sound_name(&sound.file)?;
    let mut z = open_zip(path)?;
    let f = z.by_name(&format!("{SOUNDS_DIR}{}", sound.file)).map_err(|_| format!("a la historia le falta el sonido {}", sound.file))?;
    let mut bytes = Vec::with_capacity(f.size().min(MAX_SOUND) as usize);
    f.take(MAX_SOUND).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}
