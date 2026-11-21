//! «Limpiar escaneo»: color del papel y de la tinta de una página, para que el visor estire los
//! niveles (papel amarillento → blanco, tinta gris → negra) sin tocar el archivo.
//!
//! Se analiza una vista previa pequeña: el papel es la media de los píxeles más claros (el 10 %
//! más luminoso) y la tinta la de los más oscuros (el 2 %). Si la página no tiene un papel claro
//! (fondos oscuros, ilustraciones a sangre) no se corrige nada.

use crate::{MediaError, decode};
use serde::{Deserialize, Serialize};

const ANALYSIS_SIDE: u32 = 512;
/// Por debajo de esta luminosidad no hay «papel» que blanquear.
const MIN_PAPER: f32 = 0.45;
/// Por encima de esta, lo más oscuro no es tinta: no se toca el negro.
const MAX_INK: f32 = 0.4;
/// Separación mínima entre tinta y papel por canal, para no disparar el contraste.
const MIN_RANGE: f32 = 0.3;

/// Puntos negro y blanco por canal (0..1). Sin corrección: negro 0 y blanco 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Levels {
    pub black: [f32; 3],
    pub white: [f32; 3],
}

impl Levels {
    pub const NEUTRAL: Levels = Levels { black: [0.0; 3], white: [1.0; 3] };
}

/// Niveles de una imagen codificada.
pub fn levels(name: &str, bytes: &[u8]) -> Result<Levels, MediaError> {
    let img = decode::preview(name, bytes, ANALYSIS_SIDE)?.to_rgb8();
    Ok(analyze(img.pixels().map(|p| p.0)))
}

fn luma(c: [u8; 3]) -> f32 {
    (0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32) / 255.0
}

/// Media de color de los píxeles cuya luminosidad está en `lo..=hi` (índices de histograma 0..255).
fn mean_in(px: &[[u8; 3]], lo: usize, hi: usize) -> [f32; 3] {
    let mut sum = [0f64; 3];
    let mut n = 0u64;
    for &c in px {
        let l = (luma(c) * 255.0).round() as usize;
        if (lo..=hi).contains(&l) {
            for k in 0..3 {
                sum[k] += c[k] as f64;
            }
            n += 1;
        }
    }
    if n == 0 {
        return [0.0; 3];
    }
    sum.map(|s| (s / n as f64 / 255.0) as f32)
}

pub fn analyze(pixels: impl Iterator<Item = [u8; 3]>) -> Levels {
    let px: Vec<[u8; 3]> = pixels.collect();
    if px.is_empty() {
        return Levels::NEUTRAL;
    }
    let mut hist = [0u64; 256];
    for &c in &px {
        hist[(luma(c) * 255.0).round() as usize] += 1;
    }
    // Luminosidad del percentil `p` (0..1).
    let pct = |p: f64| {
        let goal = (px.len() as f64 * p).ceil() as u64;
        let mut acc = 0;
        for (i, &h) in hist.iter().enumerate() {
            acc += h;
            if acc >= goal.max(1) {
                return i;
            }
        }
        255
    };
    let paper = mean_in(&px, pct(0.90), 255);
    let ink = mean_in(&px, 0, pct(0.02));
    let paper_l = 0.2126 * paper[0] + 0.7152 * paper[1] + 0.0722 * paper[2];
    let ink_l = 0.2126 * ink[0] + 0.7152 * ink[1] + 0.0722 * ink[2];

    let white = if paper_l >= MIN_PAPER { paper.map(|v| v.max(0.5)) } else { [1.0; 3] };
    let black = if ink_l <= MAX_INK { ink } else { [0.0; 3] };
    let mut out = Levels { black, white };
    for k in 0..3 {
        if out.white[k] - out.black[k] < MIN_RANGE {
            out.black[k] = (out.white[k] - MIN_RANGE).max(0.0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(paper: [u8; 3], ink: [u8; 3]) -> Vec<[u8; 3]> {
        // 85 % papel, 15 % tinta: una página de cómic típica.
        (0..1000).map(|i| if i % 100 < 85 { paper } else { ink }).collect()
    }

    #[test]
    fn papel_amarillento_y_tinta_gris() {
        let l = analyze(page([240, 226, 186], [52, 46, 40]).into_iter());
        let close = |a: f32, b: u8| (a - b as f32 / 255.0).abs() < 0.01;
        assert!(close(l.white[0], 240) && close(l.white[1], 226) && close(l.white[2], 186), "{l:?}");
        assert!(close(l.black[0], 52) && close(l.black[2], 40), "{l:?}");
    }

    #[test]
    fn pagina_oscura_no_se_toca_el_blanco() {
        let l = analyze(page([60, 60, 70], [5, 5, 5]).into_iter());
        assert_eq!(l.white, [1.0; 3]);
    }

    #[test]
    fn imagen_clara_sin_tinta_no_se_toca_el_negro() {
        let l = analyze((0..100).map(|_| [230u8, 230, 230]));
        assert_eq!(l.black, [0.0; 3]);
    }

    #[test]
    fn vacia_es_neutra() {
        assert_eq!(analyze(std::iter::empty()), Levels::NEUTRAL);
    }
}
