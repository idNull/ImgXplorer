//! Decodificación en Rust de los formatos que el webview no sabe mostrar, y miniaturas.

use crate::{MediaError, extension, is_raw_name, raw};
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use std::io::Cursor;
use std::sync::{Arc, OnceLock};

/// Lado máximo al rasterizar SVG a tamaño completo.
const SVG_MAX_SIDE: u32 = 4096;

/// Píxeles RGBA de 8 bits, sin premultiplicar.
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Rgba {
    /// Serialización para el protocolo: `"RGBA"`, ancho y alto (u32 LE) y los píxeles con
    /// **alfa premultiplicado** (lo que espera el renderizador; así no dependemos de cómo trate
    /// cada WebGL `UNPACK_PREMULTIPLY_ALPHA` con arrays).
    pub fn into_wire(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(12 + self.data.len());
        out.extend_from_slice(b"RGBA");
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend(self.data.as_chunks::<4>().0.iter().flat_map(|p| {
            let a = p[3] as u16;
            let m = |c: u8| ((c as u16 * a + 127) / 255) as u8;
            [m(p[0]), m(p[1]), m(p[2]), p[3]]
        }));
        out
    }
}

fn err(e: impl std::fmt::Display) -> MediaError {
    MediaError::Decode(e.to_string())
}

/// Decodifica a RGBA a resolución completa.
pub fn decode(name: &str, bytes: &[u8]) -> Result<Rgba, MediaError> {
    let img = decode_dynamic(name, bytes, None)?.into_rgba8();
    let (width, height) = img.dimensions();
    Ok(Rgba { width, height, data: img.into_raw() })
}

/// Imagen reducida (lado mayor ≤ `max`) para análisis, como la detección de viñetas.
pub fn preview(name: &str, bytes: &[u8], max: u32) -> Result<DynamicImage, MediaError> {
    let img = decode_dynamic(name, bytes, Some(max))?;
    Ok(if img.width() > max || img.height() > max { img.thumbnail(max, max) } else { img })
}

/// Miniatura JPEG cuyo lado mayor no supera `max`. La transparencia se aplana sobre `bg`.
pub fn thumbnail_jpeg(name: &str, bytes: &[u8], max: u32, bg: [u8; 3]) -> Result<Vec<u8>, MediaError> {
    let rgba = preview(name, bytes, max)?.into_rgba8();
    let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
    for (d, s) in rgb.pixels_mut().zip(rgba.pixels()) {
        let a = s[3] as u32;
        for c in 0..3 {
            d[c] = ((s[c] as u32 * a + bg[c] as u32 * (255 - a)) / 255) as u8;
        }
    }
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82).encode_image(&rgb).map_err(err)?;
    Ok(out)
}

/// `hint`: si se sabe que solo hace falta una miniatura, lado máximo deseado
/// (permite a SVG y HEIF trabajar a menor resolución).
fn decode_dynamic(name: &str, bytes: &[u8], hint: Option<u32>) -> Result<DynamicImage, MediaError> {
    if is_raw_name(name) {
        let jpeg = raw::largest_preview(bytes).ok_or_else(|| MediaError::Decode("RAW sin vista previa".into()))?;
        let mut img = with_image_crate(jpeg, None)?;
        // La orientación suele estar en el EXIF del RAW, no en el de la vista previa.
        if let Some(o) = exif_orientation(bytes) {
            img.apply_orientation(o);
        }
        return Ok(img);
    }
    match extension(name).as_deref() {
        Some("svg" | "svgz") => svg(bytes, hint.unwrap_or(SVG_MAX_SIDE)),
        Some("heic" | "heif" | "hif" | "avif") => heif(bytes, hint),
        Some("jxl") => {
            let dec = jxl_oxide::integration::JxlDecoder::new(Cursor::new(bytes)).map_err(err)?;
            DynamicImage::from_decoder(dec).map_err(err)
        }
        Some("psd") => {
            let psd = psd::Psd::from_bytes(bytes).map_err(err)?;
            image::RgbaImage::from_raw(psd.width(), psd.height(), psd.rgba())
                .map(DynamicImage::ImageRgba8)
                .ok_or_else(|| MediaError::Decode("PSD: tamaño inconsistente".into()))
        }
        ext => with_image_crate(bytes, ext),
    }
}

/// Formatos del crate `image`, con orientación EXIF aplicada. Se detectan por contenido y, si
/// no tienen firma (TGA), por la extensión.
fn with_image_crate(bytes: &[u8], ext: Option<&str>) -> Result<DynamicImage, MediaError> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(err)?;
    if reader.format().is_none()
        && let Some(f) = ext.and_then(image::ImageFormat::from_extension)
    {
        reader.set_format(f);
    }
    let mut limits = Limits::default();
    limits.max_alloc = Some(4 << 30);
    reader.limits(limits);
    let mut dec = reader.into_decoder().map_err(err)?;
    let orientation = dec.orientation().ok();
    let mut img = DynamicImage::from_decoder(dec).map_err(err)?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Ok(img)
}

fn exif_orientation(bytes: &[u8]) -> Option<image::metadata::Orientation> {
    let exif = exif::Reader::new().read_from_container(&mut Cursor::new(bytes)).ok()?;
    let v = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?.value.get_uint(0)?;
    image::metadata::Orientation::from_exif(v as u8)
}

#[cfg(feature = "heif")]
fn libheif() -> &'static libheif_rs::LibHeif {
    static LIB: OnceLock<libheif_rs::LibHeif> = OnceLock::new();
    LIB.get_or_init(libheif_rs::LibHeif::new)
}

/// Lo que puede abrir la libheif instalada: versión y si tiene decodificador HEVC (HEIC) y AV1
/// (AVIF). Lo usa el instalador para avisar si falta algún complemento.
#[cfg(feature = "heif")]
pub fn heif_support() -> (String, bool, bool) {
    use libheif_rs::CompressionFormat;
    let lib = libheif();
    let [a, b, c] = lib.version();
    let has = |f| !lib.decoder_descriptors(1, Some(f)).is_empty();
    (format!("{a}.{b}.{c}"), has(CompressionFormat::Hevc), has(CompressionFormat::Av1))
}

/// Compilado sin libheif (visor móvil): no hay versión ni decodificadores.
#[cfg(not(feature = "heif"))]
pub fn heif_support() -> (String, bool, bool) {
    ("no".into(), false, false)
}

#[cfg(not(feature = "heif"))]
fn heif(_: &[u8], _: Option<u32>) -> Result<DynamicImage, MediaError> {
    Err(MediaError::Unsupported("HEIC/HEIF/AVIF (esta versión se ha compilado sin libheif)".into()))
}

#[cfg(feature = "heif")]
fn heif(bytes: &[u8], hint: Option<u32>) -> Result<DynamicImage, MediaError> {
    use libheif_rs::{ColorSpace, HeifContext, RgbChroma};
    let lib = libheif();
    let ctx = HeifContext::read_from_bytes(bytes).map_err(err)?;
    let handle = ctx.primary_image_handle().map_err(err)?;
    let mut image = lib.decode(&handle, ColorSpace::Rgb(RgbChroma::Rgba), None).map_err(err)?;
    if let Some(max) = hint {
        let (w, h) = (image.width(), image.height());
        if w > max * 2 || h > max * 2 {
            // Reducir antes en libheif ahorra trabajo a `thumbnail`.
            let k = (max * 2) as f64 / w.max(h) as f64;
            image = image.scale(((w as f64 * k) as u32).max(1), ((h as f64 * k) as u32).max(1), None).map_err(err)?;
        }
    }
    let (w, h) = (image.width(), image.height());
    let plane = image.planes().interleaved.ok_or_else(|| MediaError::Decode("HEIF sin plano RGBA".into()))?;
    let row = w as usize * 4;
    let mut data = Vec::with_capacity(row * h as usize);
    for y in 0..h as usize {
        data.extend_from_slice(&plane.data[y * plane.stride..y * plane.stride + row]);
    }
    image::RgbaImage::from_raw(w, h, data).map(DynamicImage::ImageRgba8).ok_or_else(|| err("HEIF: tamaño"))
}

fn svg(bytes: &[u8], max_side: u32) -> Result<DynamicImage, MediaError> {
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    let fonts = FONTS.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    });
    let opt = resvg::usvg::Options { fontdb: fonts.clone(), ..Default::default() };
    let tree = resvg::usvg::Tree::from_data(bytes, &opt).map_err(err)?;
    let size = tree.size();
    let k = max_side as f32 / size.width().max(size.height());
    let (w, h) = (((size.width() * k).round() as u32).max(1), ((size.height() * k).round() as u32).max(1));
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).ok_or_else(|| err("SVG: tamaño"))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(k, k), &mut pixmap.as_mut());
    // tiny-skia trabaja premultiplicado; la interfaz espera RGBA sin premultiplicar.
    let data = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(w, h, data).map(DynamicImage::ImageRgba8).ok_or_else(|| err("SVG: tamaño"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sin libheif (visor móvil), un HEIC da «no admitido» en vez de fallar al enlazar.
    #[cfg(not(feature = "heif"))]
    #[test]
    fn sin_libheif_el_heic_no_se_admite() {
        assert!(matches!(decode("foto.heic", b"no importa"), Err(MediaError::Unsupported(_))));
        assert_eq!(heif_support(), ("no".into(), false, false));
    }

    #[test]
    fn svg_se_rasteriza_al_lado_maximo() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="red"/></svg>"#;
        let img = decode("a.svg", svg).unwrap();
        assert_eq!((img.width, img.height), (SVG_MAX_SIDE, SVG_MAX_SIDE / 2));
        assert_eq!(&img.data[..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn miniatura_respeta_el_maximo() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(400, 100, image::Rgba([0, 0, 0, 0]))
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let jpg = thumbnail_jpeg("a.png", &png, 128, [10, 20, 30]).unwrap();
        let t = image::load_from_memory(&jpg).unwrap();
        assert_eq!((t.width(), t.height()), (128, 32));
    }
}
