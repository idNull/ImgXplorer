//! Efectos que deforman la onda: saturación, distorsión y voz de robot.
use std::f32::consts::TAU;

use super::filters::{Biquad, Shape};
use super::{P, Unit, flush, wet};

/// Saturación suave. `drive` 0 no cambia nada; 1 satura mucho. Una señal de ±0,5 sale a ±0,5,
/// así que el volumen se mantiene más o menos.
#[inline]
pub fn saturate(x: f32, drive: f32) -> f32 {
    if drive <= 0.0 {
        return x;
    }
    let k = 1.0 + 24.0 * drive;
    (k * x).tanh() / (k * 0.5).tanh() * 0.5
}

/// Distorsión: satura (`drive`), quita lo más agudo (`tone`, Hz) y mezcla (`mix`).
#[derive(Debug)]
pub struct Distortion {
    drive: f32,
    mix: f32,
    tone: Biquad,
}

impl Default for Distortion {
    fn default() -> Self {
        Distortion { drive: 0.5, mix: 1.0, tone: Biquad::default() }
    }
}

impl Unit for Distortion {
    fn set(&mut self, p: &P, rate: f32) {
        self.drive = p.get("drive", 0.5, 0.0, 1.0);
        self.mix = p.get("mix", 1.0, 0.0, 1.0);
        self.tone.set(Shape::LowPass, p.get("tone", 6000.0, 300.0, 20_000.0), 0.707, 0.0, rate);
    }

    fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let (a, b) = self.tone.process(saturate(l, self.drive), saturate(r, self.drive));
        (wet(l, a, self.mix), wet(r, b, self.mix))
    }

    fn tail(&self) -> f64 {
        0.02
    }

    fn reset(&mut self) {
        self.tone.reset();
    }
}

/// Voz de robot: modulación en anillo (`freq`, Hz) y un peine corto que la vuelve metálica
/// (`metal`, 0–1).
#[derive(Debug)]
pub struct Robot {
    rate: f32,
    freq: f32,
    metal: f32,
    mix: f32,
    phase: f32,
    comb: [Vec<f32>; 2],
    pos: usize,
}

/// Retardo del peine (s).
const COMB: f32 = 0.0067;

impl Robot {
    pub fn new(rate: f32) -> Robot {
        let n = ((COMB * rate) as usize).max(1);
        Robot { rate, freq: 50.0, metal: 0.4, mix: 1.0, phase: 0.0, comb: [vec![0.0; n], vec![0.0; n]], pos: 0 }
    }
}

impl Unit for Robot {
    fn set(&mut self, p: &P, _rate: f32) {
        self.freq = p.get("freq", 50.0, 10.0, 1000.0);
        self.metal = p.get("metal", 0.4, 0.0, 0.9);
        self.mix = p.get("mix", 1.0, 0.0, 1.0);
    }

    fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let m = (TAU * self.phase).sin();
        self.phase = (self.phase + self.freq / self.rate).fract();
        let n = self.comb[0].len();
        let mut out = [0f32; 2];
        for (ch, x) in [l, r].into_iter().enumerate() {
            let ring = x * m;
            let d = self.comb[ch][self.pos];
            let y = ring + d * self.metal;
            self.comb[ch][self.pos] = flush(y);
            out[ch] = y * (1.0 - self.metal * 0.5);
        }
        self.pos = (self.pos + 1) % n;
        (wet(l, out[0], self.mix), wet(r, out[1], self.mix))
    }

    fn tail(&self) -> f64 {
        0.1
    }

    fn reset(&mut self) {
        self.comb.iter_mut().for_each(|c| c.fill(0.0));
        self.phase = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::super::{Chain, FxKind, FxSpec};
    use super::saturate;

    #[test]
    fn saturacion() {
        assert_eq!(saturate(0.3, 0.0), 0.3);
        assert!((saturate(0.5, 0.7) - 0.5).abs() < 1e-6);
        assert!(saturate(10.0, 1.0) < 0.6, "no se dispara");
    }

    #[test]
    fn robot_cambia_la_frecuencia() {
        // Un tono de 400 Hz modulado a 100 Hz da 300 y 500 Hz: deja de ser 400.
        let mut out = sine(400.0, 0.5, 0.5);
        Chain::new(&[FxSpec::new(FxKind::Robot, &[("freq", 100.0), ("metal", 0.0)])], RATE).process_block(&mut out);
        let f = freq(&out, 0.1, 0.5);
        assert!((f - 400.0).abs() > 20.0 || rms(&out, 0.1, 0.5) < 0.3, "{f}");
        assert!(finite(&out));
    }
}
