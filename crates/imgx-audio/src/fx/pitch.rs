//! Cambio de tono sin cambiar la velocidad: dos lecturas de una línea de retardo que se
//! desplazan a otro ritmo y se cruzan en ventanas de 50 ms (desplazador granular clásico).
use std::f32::consts::PI;

use super::{P, Unit, wet};

/// Tamaño de la ventana (s).
const WINDOW: f32 = 0.05;

#[derive(Debug)]
pub struct Pitch {
    buf: [Vec<f32>; 2],
    pos: usize,
    window: f32,
    phase: f32,
    step: f32,
    mix: f32,
    active: bool,
}

impl Pitch {
    pub fn new(rate: f32) -> Pitch {
        let window = (WINDOW * rate).round().max(8.0);
        let n = window as usize + 8;
        Pitch { buf: [vec![0.0; n], vec![0.0; n]], pos: 0, window, phase: 0.0, step: 0.0, mix: 1.0, active: true }
    }

    /// Muestra con retardo fraccionario `d` (≥ 1) del canal `ch` (interpolación lineal).
    #[inline]
    fn read(&self, ch: usize, d: f32) -> f32 {
        let n = self.buf[ch].len();
        let i = d.floor();
        let f = d - i;
        let a = (self.pos + 2 * n - i as usize) % n;
        let b = (a + n - 1) % n;
        self.buf[ch][a] * (1.0 - f) + self.buf[ch][b] * f
    }
}

impl Unit for Pitch {
    fn set(&mut self, p: &P, _rate: f32) {
        let semis = p.get("semitones", -5.0, -12.0, 12.0);
        let ratio = 2f32.powf(semis / 12.0);
        // Leer a `ratio` veces la velocidad: el retardo cambia (1 − ratio) muestras por muestra.
        self.step = (1.0 - ratio) / self.window;
        self.mix = p.get("mix", 1.0, 0.0, 1.0);
        self.active = semis != 0.0;
    }

    fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        self.buf[0][self.pos] = l;
        self.buf[1][self.pos] = r;
        let out = if self.active {
            let p1 = self.phase;
            let p2 = (p1 + 0.5).fract();
            // sin² y cos²: las dos ventanas suman siempre 1.
            let (g1, g2) = ((PI * p1).sin().powi(2), (PI * p2).sin().powi(2));
            let (d1, d2) = (1.0 + p1 * self.window, 1.0 + p2 * self.window);
            let o = [0, 1].map(|ch| self.read(ch, d1) * g1 + self.read(ch, d2) * g2);
            self.phase = (self.phase + self.step).rem_euclid(1.0);
            (wet(l, o[0], self.mix), wet(r, o[1], self.mix))
        } else {
            (l, r)
        };
        self.pos = (self.pos + 1) % self.buf[0].len();
        out
    }

    fn tail(&self) -> f64 {
        WINDOW as f64
    }

    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|b| b.fill(0.0));
        self.phase = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::super::{Chain, FxKind, FxSpec};

    fn shifted(semis: f32) -> (f32, usize) {
        let input = sine(300.0, 1.0, 0.5);
        let mut out = input.clone();
        Chain::new(&[FxSpec::new(FxKind::Pitch, &[("semitones", semis)])], RATE).process_block(&mut out);
        assert!(finite(&out));
        (freq(&out, 0.2, 1.0), out.len())
    }

    #[test]
    fn sube_y_baja_sin_cambiar_la_duracion() {
        let (up, n) = shifted(12.0);
        assert!((up / 600.0 - 1.0).abs() < 0.06, "una octava arriba: {up}");
        assert_eq!(n, (RATE as usize) * 2, "misma duración");
        let (down, _) = shifted(-12.0);
        assert!((down / 150.0 - 1.0).abs() < 0.08, "una octava abajo: {down}");
        let (same, _) = shifted(0.0);
        assert!((same / 300.0 - 1.0).abs() < 0.02);
    }
}
