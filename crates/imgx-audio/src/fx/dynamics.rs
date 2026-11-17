//! Compresor: baja lo que pasa del umbral para igualar el volumen (sobre todo de la voz).
use super::{P, Unit};

#[derive(Debug)]
pub struct Compressor {
    threshold: f32,
    ratio: f32,
    attack: f32,
    release: f32,
    makeup: f32,
    release_s: f32,
    /// Reducción actual (dB, ≥ 0).
    gr: f32,
}

impl Default for Compressor {
    fn default() -> Self {
        Compressor { threshold: -18.0, ratio: 4.0, attack: 0.0, release: 0.0, makeup: 1.0, release_s: 0.15, gr: 0.0 }
    }
}

/// Coeficiente de un filtro de un polo con constante de tiempo `t` (s).
fn coef(t: f32, rate: f32) -> f32 {
    (-1.0 / (t.max(0.0001) * rate)).exp()
}

impl Unit for Compressor {
    fn set(&mut self, p: &P, rate: f32) {
        self.threshold = p.get("threshold", -18.0, -60.0, 0.0);
        self.ratio = p.get("ratio", 4.0, 1.0, 20.0);
        self.attack = coef(p.get("attack", 10.0, 0.1, 200.0) / 1000.0, rate);
        self.release_s = p.get("release", 150.0, 10.0, 2000.0) / 1000.0;
        self.release = coef(self.release_s, rate);
        self.makeup = 10f32.powf(p.get("makeup", 6.0, 0.0, 24.0) / 20.0);
    }

    fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let peak = l.abs().max(r.abs()).max(1e-6);
        let db = 20.0 * peak.log10();
        let over = (db - self.threshold).max(0.0);
        let target = over * (1.0 - 1.0 / self.ratio);
        let c = if target > self.gr { self.attack } else { self.release };
        self.gr = target + (self.gr - target) * c;
        let g = 10f32.powf(-self.gr / 20.0) * self.makeup;
        (l * g, r * g)
    }

    fn tail(&self) -> f64 {
        self.release_s as f64
    }

    fn reset(&mut self) {
        self.gr = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::super::{Chain, FxKind, FxSpec};

    #[test]
    fn baja_lo_fuerte_y_respeta_lo_flojo() {
        let spec = FxSpec::new(FxKind::Compressor, &[("threshold", -20.0), ("ratio", 4.0), ("makeup", 0.0)]);
        let gain = |amp: f32| {
            let input = sine(200.0, 0.6, amp);
            let mut out = input.clone();
            Chain::new(std::slice::from_ref(&spec), RATE).process_block(&mut out);
            20.0 * (rms(&out, 0.3, 0.6) / rms(&input, 0.3, 0.6)).log10()
        };
        assert!(gain(0.02).abs() < 0.5, "por debajo del umbral no cambia: {}", gain(0.02));
        // 0,9 de pico son −0,9 dB: 19 dB por encima, con 4:1 baja unos 14 dB.
        let g = gain(0.9);
        assert!(g < -9.0 && g > -16.0, "comprime: {g}");
    }
}
