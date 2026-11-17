//! Filtros biquad (fórmulas de R. Bristow-Johnson), ecualizador de 3 bandas y filtro de paso
//! alto/bajo con saturación (teléfono, radio, bajo el agua…).
use super::{P, Unit, shape::saturate};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    LowPass,
    HighPass,
    LowShelf,
    HighShelf,
    Peak,
}

/// Filtro de segundo orden, estéreo.
#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    /// Memoria (forma directa II traspuesta) de cada canal.
    z: [[f64; 2]; 2],
}

impl Default for Biquad {
    fn default() -> Self {
        Biquad { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, z: [[0.0; 2]; 2] }
    }
}

impl Biquad {
    pub fn new(shape: Shape, freq: f32, q: f32, gain_db: f32, rate: f32) -> Biquad {
        let mut b = Biquad::default();
        b.set(shape, freq, q, gain_db, rate);
        b
    }

    /// Cambia los coeficientes sin borrar la memoria.
    pub fn set(&mut self, shape: Shape, freq: f32, q: f32, gain_db: f32, rate: f32) {
        let rate = rate as f64;
        let f = (freq as f64).clamp(5.0, rate * 0.49);
        let w = std::f64::consts::TAU * f / rate;
        let (sw, cw) = w.sin_cos();
        let q = (q as f64).max(0.05);
        let alpha = sw / (2.0 * q);
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let (b0, b1, b2, a0, a1, a2) = match shape {
            Shape::LowPass => ((1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0, 1.0 + alpha, -2.0 * cw, 1.0 - alpha),
            Shape::HighPass => ((1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0, 1.0 + alpha, -2.0 * cw, 1.0 - alpha),
            Shape::Peak => (1.0 + alpha * a, -2.0 * cw, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cw, 1.0 - alpha / a),
            Shape::LowShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cw + s),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cw),
                    a * ((a + 1.0) - (a - 1.0) * cw - s),
                    (a + 1.0) + (a - 1.0) * cw + s,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cw),
                    (a + 1.0) + (a - 1.0) * cw - s,
                )
            }
            Shape::HighShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cw + s),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cw),
                    a * ((a + 1.0) + (a - 1.0) * cw - s),
                    (a + 1.0) - (a - 1.0) * cw + s,
                    2.0 * ((a - 1.0) - (a + 1.0) * cw),
                    (a + 1.0) - (a - 1.0) * cw - s,
                )
            }
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    #[inline]
    pub fn tick(&mut self, ch: usize, x: f32) -> f32 {
        let x = x as f64;
        let z = &mut self.z[ch];
        let y = self.b0 * x + z[0];
        z[0] = self.b1 * x - self.a1 * y + z[1];
        z[1] = self.b2 * x - self.a2 * y;
        if z[0].abs() < 1e-25 {
            z[0] = 0.0;
        }
        if z[1].abs() < 1e-25 {
            z[1] = 0.0;
        }
        y as f32
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        (self.tick(0, l), self.tick(1, r))
    }

    pub fn reset(&mut self) {
        self.z = [[0.0; 2]; 2];
    }
}

/// Ecualizador: graves (150 Hz), medios (1 kHz) y agudos (5 kHz), en dB.
#[derive(Debug, Default)]
pub struct Eq {
    bands: [Biquad; 3],
    flat: bool,
}

impl Unit for Eq {
    fn set(&mut self, p: &P, rate: f32) {
        let (lo, mid, hi) = (p.get("low", 0.0, -24.0, 24.0), p.get("mid", 0.0, -24.0, 24.0), p.get("high", 0.0, -24.0, 24.0));
        self.bands[0].set(Shape::LowShelf, 150.0, 0.707, lo, rate);
        self.bands[1].set(Shape::Peak, 1000.0, 0.8, mid, rate);
        self.bands[2].set(Shape::HighShelf, 5000.0, 0.707, hi, rate);
        self.flat = lo == 0.0 && mid == 0.0 && hi == 0.0;
    }

    fn process(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        if self.flat {
            return (l, r);
        }
        for b in &mut self.bands {
            (l, r) = b.process(l, r);
        }
        (l, r)
    }

    fn tail(&self) -> f64 {
        0.02
    }

    fn reset(&mut self) {
        self.bands.iter_mut().for_each(Biquad::reset);
    }
}

/// Paso alto (`low`, Hz) y paso bajo (`high`, Hz), dos etapas cada uno (24 dB/octava), y algo
/// de saturación (`drive`, 0–1) para el carácter de teléfono o megáfono.
#[derive(Debug, Default)]
pub struct Filter {
    hp: [Biquad; 2],
    lp: [Biquad; 2],
    use_hp: bool,
    use_lp: bool,
    drive: f32,
}

impl Unit for Filter {
    fn set(&mut self, p: &P, rate: f32) {
        let low = p.get("low", 20.0, 20.0, 20_000.0);
        let high = p.get("high", 20_000.0, 20.0, 20_000.0);
        self.use_hp = low > 21.0;
        self.use_lp = high < 19_999.0;
        for b in &mut self.hp {
            b.set(Shape::HighPass, low, 0.707, 0.0, rate);
        }
        for b in &mut self.lp {
            b.set(Shape::LowPass, high, 0.707, 0.0, rate);
        }
        self.drive = p.get("drive", 0.0, 0.0, 1.0);
    }

    fn process(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        if self.use_hp {
            for b in &mut self.hp {
                (l, r) = b.process(l, r);
            }
        }
        if self.drive > 0.0 {
            l = saturate(l, self.drive);
            r = saturate(r, self.drive);
        }
        if self.use_lp {
            for b in &mut self.lp {
                (l, r) = b.process(l, r);
            }
        }
        (l, r)
    }

    fn tail(&self) -> f64 {
        0.05
    }

    fn reset(&mut self) {
        self.hp.iter_mut().chain(self.lp.iter_mut()).for_each(Biquad::reset);
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::super::{Chain, FxKind, FxSpec};

    /// Ganancia (dB) del efecto a una frecuencia.
    fn gain_db(spec: &FxSpec, freq: f32) -> f32 {
        let input = sine(freq, 0.5, 0.25);
        let mut out = input.clone();
        Chain::new(std::slice::from_ref(spec), RATE).process_block(&mut out);
        20.0 * (rms(&out, 0.2, 0.5) / rms(&input, 0.2, 0.5)).log10()
    }

    #[test]
    fn ecualizador() {
        let eq = FxSpec::new(FxKind::Eq, &[("low", 9.0), ("mid", 0.0), ("high", -9.0)]);
        assert!((gain_db(&eq, 40.0) - 9.0).abs() < 1.0, "graves +9 dB: {}", gain_db(&eq, 40.0));
        assert!(gain_db(&eq, 1000.0).abs() < 2.0, "medios casi igual: {}", gain_db(&eq, 1000.0));
        assert!((gain_db(&eq, 15_000.0) + 9.0).abs() < 1.0, "agudos −9 dB: {}", gain_db(&eq, 15_000.0));
        let mid = FxSpec::new(FxKind::Eq, &[("mid", 6.0)]);
        assert!((gain_db(&mid, 1000.0) - 6.0).abs() < 0.5);
    }

    #[test]
    fn filtro_de_telefono() {
        let tel = FxSpec::new(FxKind::Filter, &[("low", 300.0), ("high", 3400.0)]);
        assert!(gain_db(&tel, 1000.0).abs() < 1.5, "deja pasar la voz: {}", gain_db(&tel, 1000.0));
        assert!(gain_db(&tel, 80.0) < -30.0, "quita graves: {}", gain_db(&tel, 80.0));
        assert!(gain_db(&tel, 12_000.0) < -30.0, "quita agudos: {}", gain_db(&tel, 12_000.0));
        let under = FxSpec::new(FxKind::Filter, &[("high", 500.0)]);
        assert!(gain_db(&under, 4000.0) < -40.0);
    }
}
