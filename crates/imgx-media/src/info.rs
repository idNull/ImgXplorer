//! Información de una imagen para el panel ℹ️: el formato y los datos de la cámara (EXIF) que
//! interesan a una persona. La ubicación GPS no se enseña: solo se avisa de que la foto la lleva.

use crate::extension;
use serde::Serialize;
use std::io::Cursor;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    /// Extensión en mayúsculas (JPG, PNG, CR3…).
    pub format: String,
    /// Campos EXIF: clave estable (para traducir la etiqueta) y valor ya legible.
    pub exif: Vec<(String, String)>,
    /// La foto lleva coordenadas GPS.
    pub gps: bool,
}

/// Los campos que se enseñan, en este orden.
const FIELDS: [(exif::Tag, &str); 11] = [
    (exif::Tag::Make, "make"),
    (exif::Tag::Model, "model"),
    (exif::Tag::LensModel, "lens"),
    (exif::Tag::DateTimeOriginal, "date"),
    (exif::Tag::ExposureTime, "exposure"),
    (exif::Tag::FNumber, "aperture"),
    (exif::Tag::PhotographicSensitivity, "iso"),
    (exif::Tag::FocalLength, "focal"),
    (exif::Tag::Flash, "flash"),
    (exif::Tag::Software, "software"),
    (exif::Tag::Artist, "artist"),
];

pub fn info(name: &str, bytes: &[u8]) -> ImageInfo {
    let format = extension(name).unwrap_or_default().to_uppercase();
    let Ok(ex) = exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) else {
        return ImageInfo { format, exif: Vec::new(), gps: false };
    };
    let mut out = Vec::new();
    for (tag, key) in FIELDS {
        let Some(f) = ex.get_field(tag, exif::In::PRIMARY) else { continue };
        let v = f.display_value().with_unit(&ex).to_string();
        let v = v.trim_matches(|c: char| c == '"' || c.is_whitespace()).to_owned();
        if !v.is_empty() && v != "0" {
            out.push((key.to_owned(), v));
        }
    }
    let gps = ex.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY).is_some();
    ImageInfo { format, exif: out, gps }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_exif_da_solo_el_formato() {
        let i = info("pagina.png", b"no es una imagen");
        assert_eq!(i, ImageInfo { format: "PNG".into(), exif: vec![], gps: false });
    }
}
