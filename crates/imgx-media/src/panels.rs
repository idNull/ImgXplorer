//! Detección automática de viñetas por cortes recursivos XY sobre las calles (gutters).
//!
//! 1. Se reduce la página a ≤ 1024 px y a escala de grises.
//! 2. El color de fondo es la mediana del borde exterior (calles blancas o negras).
//! 3. Se marca como "tinta" todo lo que se aparta del fondo.
//! 4. Se busca recursivamente una franja horizontal sin tinta que corte la región; si no hay,
//!    una vertical. Lo que ya no se puede cortar es una viñeta.
//!
//! Funciona bien con viñetas con borde y calles limpias, que es lo habitual. Los globos que
//! cruzan una calle la anulan (esas viñetas salen juntas); se podrán corregir a mano en el editor.

use crate::{MediaError, decode};
use image::GrayImage;

/// Lado máximo de la imagen analizada.
const ANALYSIS_SIDE: u32 = 1024;
/// Diferencia de luminancia con el fondo a partir de la cual un píxel es tinta.
const INK_TOLERANCE: i16 = 48;

/// Rectángulo normalizado `[x, y, w, h]` (0..1), sin orden de lectura.
pub type PanelRect = [f64; 4];

/// Viñetas de una imagen codificada. Vacío si no se encuentran al menos dos.
pub fn detect_panels(name: &str, bytes: &[u8]) -> Result<Vec<PanelRect>, MediaError> {
    Ok(detect(&decode::preview(name, bytes, ANALYSIS_SIDE)?.to_luma8()))
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Region {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Region {
    fn w(&self) -> u32 {
        self.x1 - self.x0
    }
    fn h(&self) -> u32 {
        self.y1 - self.y0
    }
}

struct Mask {
    w: u32,
    h: u32,
    ink: Vec<bool>,
}

impl Mask {
    fn at(&self, x: u32, y: u32) -> bool {
        self.ink[(y * self.w + x) as usize]
    }
    fn row_ink(&self, y: u32, r: &Region) -> u32 {
        (r.x0..r.x1).filter(|&x| self.at(x, y)).count() as u32
    }
    fn col_ink(&self, x: u32, r: &Region) -> u32 {
        (r.y0..r.y1).filter(|&y| self.at(x, y)).count() as u32
    }
}

pub fn detect(img: &GrayImage) -> Vec<PanelRect> {
    let (w, h) = img.dimensions();
    if w < 32 || h < 32 {
        return Vec::new();
    }
    let bg = border_median(img);
    let ink = img.pixels().map(|p| (p[0] as i16 - bg as i16).abs() > INK_TOLERANCE).collect();
    let mask = Mask { w, h, ink };
    let mut found = Vec::new();
    cut(&mask, Region { x0: 0, y0: 0, x1: w, y1: h }, &mut found, 0);

    // Fuera lo que es demasiado pequeño para ser una viñeta (números de página, ruido).
    let (pw, ph) = (w as f64, h as f64);
    let panels: Vec<PanelRect> = found
        .into_iter()
        .filter(|r| {
            let (rw, rh) = (r.w() as f64 / pw, r.h() as f64 / ph);
            rw >= 0.06 && rh >= 0.04 && rw * rh >= 0.012
        })
        .map(|r| {
            // Un píxel de holgura: el recorte se queda justo en el borde de la viñeta.
            let x0 = r.x0.saturating_sub(1) as f64;
            let y0 = r.y0.saturating_sub(1) as f64;
            let x1 = (r.x1 + 1).min(w) as f64;
            let y1 = (r.y1 + 1).min(h) as f64;
            [x0 / pw, y0 / ph, (x1 - x0) / pw, (y1 - y0) / ph]
        })
        .collect();
    if panels.len() < 2 { Vec::new() } else { panels }
}

/// Recuadro con contenido de una imagen codificada (sin los márgenes del color del fondo), para
/// «✂ ajustar al contenido». `None` si apenas hay margen que quitar.
pub fn content_box(name: &str, bytes: &[u8]) -> Result<Option<PanelRect>, MediaError> {
    Ok(content(&decode::preview(name, bytes, ANALYSIS_SIDE)?.to_luma8()))
}

/// Viñetas y recuadro con contenido con una sola decodificación (al exportar una historia).
pub fn analyze(name: &str, bytes: &[u8]) -> Result<(Vec<PanelRect>, Option<PanelRect>), MediaError> {
    let gray = decode::preview(name, bytes, ANALYSIS_SIDE)?.to_luma8();
    Ok((detect(&gray), content(&gray)))
}

/// Margen que se deja alrededor del contenido (fracción del lado).
const CONTENT_PAD: f64 = 0.006;

pub fn content(img: &GrayImage) -> Option<PanelRect> {
    let (w, h) = img.dimensions();
    if w < 32 || h < 32 {
        return None;
    }
    let bg = border_median(img);
    let ink = img.pixels().map(|p| (p[0] as i16 - bg as i16).abs() > INK_TOLERANCE).collect();
    let m = Mask { w, h, ink };
    let all = Region { x0: 0, y0: 0, x1: w, y1: h };
    // Filas y columnas con algo de tinta (unos pocos píxeles sueltos son ruido del escaneo).
    let rows = runs(h, (h / 100).max(2), |y| m.row_ink(y, &all) <= (w / 200).max(1));
    let (y0, y1) = span(&rows, h)?;
    let band = Region { y0, y1, ..all };
    let cols = runs(w, (w / 100).max(2), |x| m.col_ink(x, &band) <= ((y1 - y0) / 200).max(1));
    let (x0, x1) = span(&cols, w)?;
    let (pw, ph) = (w as f64, h as f64);
    let bx0 = (x0 as f64 / pw - CONTENT_PAD).max(0.0);
    let by0 = (y0 as f64 / ph - CONTENT_PAD).max(0.0);
    let bx1 = (x1 as f64 / pw + CONTENT_PAD).min(1.0);
    let by1 = (y1 as f64 / ph + CONTENT_PAD).min(1.0);
    let (bw, bh) = (bx1 - bx0, by1 - by0);
    // Casi toda la página: no merece la pena moverse. Demasiado poco: algo ha salido mal.
    if (bw >= 0.97 && bh >= 0.97) || bw < 0.2 || bh < 0.2 {
        return None;
    }
    Some([bx0, by0, bw, bh])
}

/// Del primer al último tramo con tinta, sin los tramos diminutos de los bordes (números de
/// página, marcas de escaneo) si hay contenido de verdad entre ellos.
fn span(runs: &[(u32, u32)], len: u32) -> Option<(u32, u32)> {
    let small = |r: &(u32, u32)| ((r.1 - r.0) as f64) < len as f64 * 0.03;
    let mut a = 0;
    let mut b = runs.len();
    while b - a > 1 && small(&runs[a]) {
        a += 1;
    }
    while b - a > 1 && small(&runs[b - 1]) {
        b -= 1;
    }
    if a >= b {
        return None;
    }
    Some((runs[a].0, runs[b - 1].1))
}

fn border_median(img: &GrayImage) -> u8 {
    let (w, h) = img.dimensions();
    let ring = (w.min(h) / 50).max(1);
    let mut hist = [0u32; 256];
    let mut n = 0u32;
    for (x, y, p) in img.enumerate_pixels() {
        if x < ring || y < ring || x >= w - ring || y >= h - ring {
            hist[p[0] as usize] += 1;
            n += 1;
        }
    }
    let mut acc = 0;
    for (v, c) in hist.iter().enumerate() {
        acc += c;
        if acc * 2 >= n {
            return v as u8;
        }
    }
    255
}

/// Tramos con tinta a lo largo de un eje, separados por huecos de al menos `min_gap`.
fn runs(len: u32, min_gap: u32, empty: impl Fn(u32) -> bool) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut start: Option<u32> = None;
    let mut gap = 0;
    let mut last_ink = 0;
    for i in 0..len {
        if empty(i) {
            gap += 1;
            if let Some(s) = start
                && gap >= min_gap
            {
                out.push((s, last_ink + 1));
                start = None;
            }
        } else {
            if start.is_none() {
                start = Some(i);
            }
            gap = 0;
            last_ink = i;
        }
    }
    if let Some(s) = start {
        out.push((s, last_ink + 1));
    }
    out
}

fn cut(m: &Mask, r: Region, out: &mut Vec<Region>, depth: u32) {
    // Tolerancia de ruido: una fila con muy pocos píxeles de tinta cuenta como vacía.
    let noise_row = (r.w() / 500).max(1);
    let noise_col = (r.h() / 500).max(1);
    let gap_y = (m.h / 250).max(2);
    let gap_x = (m.w / 250).max(2);

    let rows = runs(r.h(), gap_y, |i| m.row_ink(r.y0 + i, &r) <= noise_row);
    if rows.is_empty() {
        return; // región vacía
    }
    if rows.len() >= 2 && depth < 10 {
        for (a, b) in rows {
            cut(m, Region { y0: r.y0 + a, y1: r.y0 + b, ..r }, out, depth + 1);
        }
        return;
    }
    let r = Region { y0: r.y0 + rows[0].0, y1: r.y0 + rows[0].1, ..r };
    let cols = runs(r.w(), gap_x, |i| m.col_ink(r.x0 + i, &r) <= noise_col);
    if cols.is_empty() {
        return;
    }
    if cols.len() >= 2 && depth < 10 {
        for (a, b) in cols {
            cut(m, Region { x0: r.x0 + a, x1: r.x0 + b, ..r }, out, depth + 1);
        }
        return;
    }
    out.push(Region { x0: r.x0 + cols[0].0, x1: r.x0 + cols[0].1, ..r });
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;

    /// Página blanca con viñetas de borde negro y "dibujo" gris dentro.
    fn page(w: u32, h: u32, panels: &[(u32, u32, u32, u32)], bg: u8, border: u8) -> GrayImage {
        let mut img = GrayImage::from_pixel(w, h, Luma([bg]));
        for &(x0, y0, x1, y1) in panels {
            for y in y0..y1 {
                for x in x0..x1 {
                    let edge = x < x0 + 3 || x >= x1 - 3 || y < y0 + 3 || y >= y1 - 3;
                    // Interior con zonas del color del fondo (cielo), que no deben partir la viñeta.
                    let fill = if (y / 40) % 2 == 0 { bg } else { 128 };
                    img.put_pixel(x, y, Luma([if edge { border } else { fill }]));
                }
            }
        }
        img
    }

    fn near(a: PanelRect, b: (u32, u32, u32, u32), w: u32, h: u32) -> bool {
        let e = [b.0 as f64 / w as f64, b.1 as f64 / h as f64, (b.2 - b.0) as f64 / w as f64, (b.3 - b.1) as f64 / h as f64];
        a.iter().zip(e).all(|(x, y)| (x - y).abs() < 0.01)
    }

    const LAYOUT: [(u32, u32, u32, u32); 4] =
        [(20, 20, 580, 280), (20, 300, 290, 880), (310, 300, 580, 580), (310, 600, 580, 880)];

    #[test]
    fn detecta_disposicion_con_vineta_alta() {
        let mut img = page(600, 900, &LAYOUT, 250, 10);
        // Número de página en la calle inferior: no es una viñeta.
        for y in 886..894 {
            for x in 296..304 {
                img.put_pixel(x, y, Luma([0]));
            }
        }
        let found = detect(&img);
        assert_eq!(found.len(), 4, "{found:?}");
        for p in LAYOUT {
            assert!(found.iter().any(|f| near(*f, p, 600, 900)), "falta {p:?} en {found:?}");
        }
    }

    #[test]
    fn calles_negras() {
        let found = detect(&page(600, 900, &LAYOUT, 5, 230));
        assert_eq!(found.len(), 4, "{found:?}");
    }

    #[test]
    fn contenido_sin_margenes_ni_numero_de_pagina() {
        let mut img = page(600, 900, &[(90, 60, 510, 820)], 245, 10);
        for y in 860..872 {
            for x in 296..304 {
                img.put_pixel(x, y, Luma([0]));
            }
        }
        let b = content(&img).expect("hay margen");
        assert!(near(b, (86, 56, 514, 825), 600, 900), "{b:?}");
    }

    #[test]
    fn contenido_a_sangre_no_recorta() {
        assert_eq!(content(&page(600, 900, &[(4, 4, 596, 896)], 245, 10)), None);
        assert_eq!(content(&GrayImage::from_pixel(600, 900, Luma([250]))), None);
    }

    #[test]
    fn pagina_completa_no_tiene_vinetas() {
        assert!(detect(&page(600, 900, &[(10, 10, 590, 890)], 250, 10)).is_empty());
    }
}
