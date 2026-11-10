//! RAW de cámara: extracción de la vista previa JPEG incrustada.
//!
//! Casi todos los RAW (CR2/CR3, NEF, ARW, DNG, RAF, ORF, RW2, PEF…) llevan una vista previa
//! JPEG, normalmente a resolución completa. Revelar el sensor (demosaico, balance de blancos,
//! perfil de color) queda para más adelante; para *ver* el archivo la vista previa basta y es
//! instantánea. Buscamos todos los JPEG baseline/progresivos del archivo y nos quedamos con
//! el de mayor resolución (los datos del sensor en JPEG sin pérdida, SOF3, se descartan).

/// Devuelve el JPEG incrustado de mayor resolución.
pub fn largest_preview(data: &[u8]) -> Option<&[u8]> {
    let mut best: Option<(&[u8], u64)> = None;
    let mut i = 0;
    while i + 4 <= data.len() {
        if data[i] == 0xFF
            && data[i + 1] == 0xD8
            && data[i + 2] == 0xFF
            && let Some((len, w, h)) = parse_jpeg(&data[i..])
        {
            let px = w as u64 * h as u64;
            if best.is_none_or(|(_, b)| px > b) {
                best = Some((&data[i..i + len], px));
            }
            i += len;
            continue;
        }
        i += 1;
    }
    best.map(|(j, _)| j)
}

/// Recorre los segmentos de un JPEG que empieza en `d[0]`. Devuelve (longitud, ancho, alto)
/// si es un JPEG con pérdida (SOF0/1/2) completo.
fn parse_jpeg(d: &[u8]) -> Option<(usize, u16, u16)> {
    let mut p = 2;
    let mut size = None;
    loop {
        // Saltar bytes de relleno 0xFF.
        while p < d.len() && d[p] == 0xFF && d.get(p + 1) == Some(&0xFF) {
            p += 1;
        }
        if p + 4 > d.len() || d[p] != 0xFF {
            return None;
        }
        let marker = d[p + 1];
        let len = u16::from_be_bytes([d[p + 2], d[p + 3]]) as usize;
        match marker {
            0xC0..=0xC2 => {
                if p + 9 > d.len() {
                    return None;
                }
                let h = u16::from_be_bytes([d[p + 5], d[p + 6]]);
                let w = u16::from_be_bytes([d[p + 7], d[p + 8]]);
                size = Some((w, h));
            }
            0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return None, // sin pérdida / jerárquico / aritmético
            0xDA => {
                let (w, h) = size?;
                if w == 0 || h == 0 {
                    return None;
                }
                // Datos de entropía (y más barridos si es progresivo) hasta EOI.
                let mut q = p + 2 + len;
                while q + 1 < d.len() {
                    if d[q] == 0xFF {
                        match d[q + 1] {
                            0xD9 => return Some((q + 2, w, h)),
                            0x00 | 0xD0..=0xD7 | 0xFF => {}
                            _ => {} // otros marcadores (DHT/SOS de barridos progresivos): se siguen
                        }
                    }
                    q += 1;
                }
                return None;
            }
            0xD9 => return None,
            _ => {}
        }
        if len < 2 {
            return None;
        }
        p += 2 + len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 100, 50]));
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut out).encode_image(&img).unwrap();
        out
    }

    #[test]
    fn elige_la_vista_previa_mayor() {
        let mut raw = b"II*\0 cabecera tiff falsa".to_vec();
        raw.extend(tiny_jpeg(16, 12));
        raw.extend([0u8; 37]);
        let big = tiny_jpeg(64, 48);
        raw.extend(&big);
        raw.extend([0xFF, 0xD8, 0xFF, 0x00]); // basura
        let got = largest_preview(&raw).unwrap();
        assert_eq!(got, &big[..]);
    }
}
