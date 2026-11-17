//! Reverb tipo Freeverb (Jezar, dominio público): 8 filtros peine con amortiguación en paralelo y
//! 4 pasa-todo en serie por canal, con un predelay delante.
use super::{P, Unit, flush};

const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
/// Diferencia entre canales (muestras a 44,1 kHz) para que suene ancho.
const SPREAD: usize = 23;
const FIXED_GAIN: f32 = 0.015;
const WET_SCALE: f32 = 3.0;
const MAX_PREDELAY: f32 = 0.25;

#[derive(Debug)]
struct Comb {
    buf: Vec<f32>,
    pos: usize,
    store: f32,
}

impl Comb {
    #[inline]
    fn tick(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let y = self.buf[self.pos];
        self.store = flush(y * (1.0 - damp) + self.store * damp);
        self.buf[self.pos] = flush(x + self.store * feedback);
        self.pos = (self.pos + 1) % self.buf.len();
        y
    }
}

#[derive(Debug)]
struct AllPass {
    buf: Vec<f32>,
    pos: usize,
}

impl AllPass {
    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let b = self.buf[self.pos];
        self.buf[self.pos] = flush(x + b * 0.5);
        self.pos = (self.pos + 1) % self.buf.len();
        b - x
    }
}

#[derive(Debug)]
pub struct Reverb {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<AllPass>; 2],
    pre: Vec<f32>,
    pre_pos: usize,
    pre_len: usize,
    feedback: f32,
    damp: f32,
    mix: f32,
    wet1: f32,
    wet2: f32,
    size: f32,
    predelay: f32,
}

impl Reverb {
    pub fn new(rate: f32) -> Reverb {
        let k = rate / 44_100.0;
        let len = |n: usize| ((n as f32 * k) as usize).max(1);
        let combs = [0, SPREAD].map(|s| COMBS.iter().map(|&n| Comb { buf: vec![0.0; len(n + s)], pos: 0, store: 0.0 }).collect());
        let allpasses = [0, SPREAD].map(|s| ALLPASSES.iter().map(|&n| AllPass { buf: vec![0.0; len(n + s)], pos: 0 }).collect());
        Reverb {
            combs,
            allpasses,
            pre: vec![0.0; (MAX_PREDELAY * rate) as usize + 1],
            pre_pos: 0,
            pre_len: 0,
            feedback: 0.84,
            damp: 0.2,
            mix: 0.3,
            wet1: 1.0,
            wet2: 0.0,
            size: 0.5,
            predelay: 0.0,
        }
    }
}

impl Unit for Reverb {
    fn set(&mut self, p: &P, rate: f32) {
        self.size = p.get("size", 0.5, 0.0, 1.0);
        self.feedback = self.size * 0.28 + 0.7;
        self.damp = p.get("damping", 0.5, 0.0, 1.0) * 0.4;
        self.mix = p.get("mix", 0.3, 0.0, 1.0);
        let width = p.get("width", 1.0, 0.0, 1.0);
        self.wet1 = WET_SCALE * (width / 2.0 + 0.5);
        self.wet2 = WET_SCALE * ((1.0 - width) / 2.0);
        self.predelay = p.get("predelay", 0.0, 0.0, MAX_PREDELAY * 1000.0) / 1000.0;
        self.pre_len = ((self.predelay * rate) as usize).min(self.pre.len() - 1);
    }

    fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let mut input = (l + r) * FIXED_GAIN;
        if self.pre_len > 0 {
            let n = self.pre.len();
            self.pre[self.pre_pos] = input;
            input = self.pre[(self.pre_pos + n - self.pre_len) % n];
            self.pre_pos = (self.pre_pos + 1) % n;
        }
        let (fb, damp) = (self.feedback, self.damp);
        let out: [f32; 2] = std::array::from_fn(|ch| {
            let mut acc: f32 = self.combs[ch].iter_mut().map(|c| c.tick(input, fb, damp)).sum();
            for a in &mut self.allpasses[ch] {
                acc = a.tick(acc);
            }
            acc
        });
        let wl = out[0] * self.wet1 + out[1] * self.wet2;
        let wr = out[1] * self.wet1 + out[0] * self.wet2;
        // La señal original baja un poco menos de lo que sube la reverb, para no perder volumen.
        let dry = 1.0 - self.mix * 0.5;
        (l * dry + wl * self.mix, r * dry + wr * self.mix)
    }

    fn tail(&self) -> f64 {
        // Tiempo de caída de 60 dB del peine más largo, más el predelay.
        let d = 1617.0 / 44_100.0;
        let g = (self.feedback * (1.0 - self.damp * 0.5)) as f64;
        (d * (0.001f64).ln() / g.ln() + self.predelay as f64).min(12.0)
    }

    fn reset(&mut self) {
        for side in &mut self.combs {
            for c in side {
                c.buf.fill(0.0);
                c.store = 0.0;
            }
        }
        for side in &mut self.allpasses {
            for a in side {
                a.buf.fill(0.0);
            }
        }
        self.pre.fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::super::{Chain, FxKind, FxSpec};

    fn tail_energy(size: f32) -> (f32, f32) {
        let mut out = impulse(4.0);
        Chain::new(&[FxSpec::new(FxKind::Reverb, &[("size", size), ("mix", 1.0), ("damping", 0.3)])], RATE).process_block(&mut out);
        assert!(finite(&out));
        (rms(&out, 0.2, 0.6), rms(&out, 2.5, 3.5))
    }

    #[test]
    fn la_cola_decae_y_crece_con_el_tamano() {
        let (early, late) = tail_energy(0.5);
        assert!(early > 1e-4, "suena: {early}");
        assert!(late < early * 0.05, "decae: {early} → {late}");
        let (_, late_big) = tail_energy(0.95);
        assert!(late_big > late * 5.0, "una catedral dura más: {late} / {late_big}");
    }

    #[test]
    fn predelay_retrasa_la_reverb() {
        let mut out = impulse(0.5);
        Chain::new(&[FxSpec::new(FxKind::Reverb, &[("mix", 1.0), ("predelay", 100.0)])], RATE).process_block(&mut out);
        // Antes de predelay + el peine más corto no hay nada (más allá del impulso seco).
        let quiet = (2..(0.12 * RATE as f32) as usize).all(|k| out[k * 2].abs() < 1e-6);
        assert!(quiet);
        assert!(rms(&out, 0.13, 0.3) > 1e-4);
    }
}
