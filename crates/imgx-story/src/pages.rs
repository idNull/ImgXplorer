//! Páginas de una historia: qué formatos van tal cual y cómo se convierten los demás.
//!
//! Una historia tiene que abrirse en cualquier sitio (también en un móvil), así que sus páginas
//! solo pueden ser JPEG, PNG, WebP o GIF:
//!
//! - **Escritorio (original):** esos formatos se copian byte a byte. Los demás (HEIC, PSD, TIFF,
//!   EXR, JPEG XL, SVG…) pasan a PNG, sin pérdida; los RAW de cámara, a JPEG de máxima calidad
//!   (lo que se ve de un RAW ya es su vista previa JPEG).
//! - **Móvil (optimizada):** además, ninguna página pasa de [`MOBILE_SIDE`] px de lado mayor. Las
//!   que se reducen salen en JPEG (o PNG si tienen transparencia).

use crate::Variant;
use image::{DynamicImage, ImageReader, RgbaImage, imageops::FilterType};
use std::io::Cursor;

/// Extensiones que una historia puede llevar tal cual.
pub const PAGE_EXTS: &[&str] = &["jpg", "jpeg", "jfif", "jpe", "png", "webp", "gif"];
/// Lado mayor de las páginas en la variante para móvil.
pub const MOBILE_SIDE: u32 = 2560;
const MOBILE_QUALITY: u8 = 88;
const RAW_QUALITY: u8 = 95;

/// Una página lista para guardar.
pub struct PageOut {
    /// Nombre dentro de la historia (el original, o con otra extensión si se convirtió).
    pub name: String,
    pub bytes: Vec<u8>,
    /// Tamaño nuevo si se reescaló.
    pub size: Option<[u32; 2]>,
}

pub fn is_page_name(name: &str) -> bool {
    imgx_media::extension(name).is_some_and(|e| PAGE_EXTS.contains(&e.as_str()))
}

/// Cambia la extensión de un nombre de entrada (`cap 1/003.heic` → `cap 1/003.png`).
pub fn with_extension(name: &str, ext: &str) -> String {
    let slash = name.rfind('/').map_or(0, |i| i + 1);
    match name[slash..].rfind('.') {
        Some(dot) if dot > 0 => format!("{}.{ext}", &name[..slash + dot]),
        _ => format!("{name}.{ext}"),
    }
}

fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?.into_dimensions().ok()
}

fn encode_jpeg(img: &RgbaImage, quality: u8) -> Result<Vec<u8>, String> {
    let rgb = DynamicImage::ImageRgba8(img.clone()).into_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality).encode_image(&rgb).map_err(|e| e.to_string())?;
    Ok(out)
}

fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    DynamicImage::ImageRgba8(img.clone()).write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

fn opaque(img: &RgbaImage) -> bool {
    img.pixels().all(|p| p[3] == 255)
}

/// Prepara una página para la historia. `name` es su nombre en el cómic.
pub fn convert(name: &str, bytes: Vec<u8>, variant: Variant) -> Result<PageOut, String> {
    let native = is_page_name(name);
    let fits = || dimensions(&bytes).is_some_and(|(w, h)| w.max(h) <= MOBILE_SIDE);
    if native && (variant == Variant::Desktop || fits()) {
        return Ok(PageOut { name: name.to_owned(), bytes, size: None });
    }
    let rgba = imgx_media::decode(name, &bytes).map_err(|e| e.to_string())?;
    let mut img = RgbaImage::from_raw(rgba.width, rgba.height, rgba.data).ok_or("imagen decodificada incompleta")?;
    let mut size = None;
    if variant == Variant::Mobile && img.width().max(img.height()) > MOBILE_SIDE {
        img = DynamicImage::ImageRgba8(img).resize(MOBILE_SIDE, MOBILE_SIDE, FilterType::Lanczos3).into_rgba8();
        size = Some([img.width(), img.height()]);
    }
    let (ext, out) = if imgx_media::is_raw_name(name) {
        ("jpg", encode_jpeg(&img, RAW_QUALITY)?)
    } else if variant == Variant::Mobile && opaque(&img) {
        ("jpg", encode_jpeg(&img, MOBILE_QUALITY)?)
    } else {
        ("png", encode_png(&img)?)
    };
    Ok(PageOut { name: with_extension(name, ext), bytes: out, size })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, alpha: u8) -> Vec<u8> {
        encode_png(&RgbaImage::from_pixel(w, h, image::Rgba([200, 180, 120, alpha]))).unwrap()
    }

    #[test]
    fn extensiones() {
        assert_eq!(with_extension("cap 1/003.heic", "png"), "cap 1/003.png");
        assert_eq!(with_extension("v1.2/sin", "jpg"), "v1.2/sin.jpg");
        assert_eq!(with_extension(".oculta", "png"), ".oculta.png");
        assert!(is_page_name("a/B.JPG") && is_page_name("x.webp") && !is_page_name("x.heic") && !is_page_name("x.avif"));
    }

    #[test]
    fn escritorio_copia_tal_cual() {
        let bytes = png(3000, 4000, 255);
        let out = convert("01.png", bytes.clone(), Variant::Desktop).unwrap();
        assert_eq!((out.name.as_str(), out.size), ("01.png", None));
        assert_eq!(out.bytes, bytes);
    }

    #[test]
    fn movil_reduce_las_grandes() {
        let small = png(800, 1200, 255);
        assert_eq!(convert("p.png", small.clone(), Variant::Mobile).unwrap().bytes, small);
        let big = convert("p.png", png(3000, 4000, 255), Variant::Mobile).unwrap();
        assert_eq!(big.name, "p.jpg");
        assert_eq!(big.size, Some([1920, 2560]));
        assert_eq!(dimensions(&big.bytes), Some((1920, 2560)));
        // Con transparencia sigue en PNG.
        assert_eq!(convert("t.png", png(3000, 4000, 100), Variant::Mobile).unwrap().name, "t.png");
    }

    #[test]
    fn formatos_que_no_van_tal_cual() {
        // Un TIFF (lo decodifica imgx-media) pasa a PNG sin pérdida en la variante de escritorio.
        let mut tiff = Vec::new();
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 3, image::Rgba([1, 2, 3, 255]))).write_to(&mut Cursor::new(&mut tiff), image::ImageFormat::Tiff).unwrap();
        let out = convert("cap/01.tif", tiff, Variant::Desktop).unwrap();
        assert_eq!(out.name, "cap/01.png");
        let back = image::load_from_memory(&out.bytes).unwrap().into_rgba8();
        assert_eq!(back.get_pixel(2, 1).0, [1, 2, 3, 255]);
    }
}
