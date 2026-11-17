//! Eco: retardo estéreo con realimentación filtrada y modo ping-pong.
use super::filters::{Biquad, Shape};
use super::{P, Unit, flush};

/// Retardo máximo (s).
const MAX: f32 = 2.0;

#[derive(Debug)]
pub struct Delay {
    buf: [Vec<f32>; 2],
    pos: usize,
    len: usize,
    feedback: f32,
    mix: f32,
    pingpong: bool,
    tone: Biquad,
    use_tone: bool,
    seconds: f32,
}

impl Delay {
    pub fn new(rate: f32) -> Delay {
        let n = (MAX * rate) as usize + 1;
        Delay { buf: [vec![0.0; n], vec![0.0; n]], pos: 0, len: 1, feedback: 0.35, mix: 0.35, pingpong: false, tone: Biquad::default(), use_tone: true, seconds: 0.35 }
    }
}

impl Unit for Delay {
    fn set(&mut self, p: &P, rate: f32) {
        self.seconds = p.get("time", 350.0, 10.0, MAX * 1000.0) / 1000.0;
        self.len = ((self.seconds * rate).round() as usize).clamp(1, self.buf[0].len() - 1);
        self.feedback = p.get("feedback", 0.35, 0.0, 0.95);
        self.mix = p.get("mix", 0.35, 0.0, 1.0);
        self.pingpong = p.get("pingpong", 0.0, 0.0, 1.0) >= 0.5;
        let tone = p.get("tone", 6000.0, 500.0, 20_000.0);
        self.use_tone = tone < 19_999.0;
        self.tone.set(Shape::LowPass, tone, 0.707, 0.0, rate);
    }

    fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let n = self.buf[0].len();
        let read = (self.pos + n - self.len) % n;
        let (dl, dr) = (self.buf[0][read], self.buf[1][read]);
        let (fl, fr) = if self.use_tone { self.tone.process(dl, dr) } else { (dl, dr) };
        let (wl, wr) = if self.pingpong {
            // La entrada entra por la izquierda y cada repetición cambia de lado.
            ((l + r) * 0.5 + fr * self.feedback, fl * self.feedback)
        } else {
            (l + fl * self.feedback, r + fr * self.feedback)
        };
        self.buf[0][self.pos] = flush(wl);
        self.buf[1][self.pos] = flush(wr);
        self.pos = (self.pos + 1) % n;
        (l + dl * self.mix, r + dr * self.mix)
    }

    fn tail(&self) -> f64 {
        let fb = self.feedback.max(0.01) as f64;
        // Hasta que las repeticiones bajan 60 dB (o 12 s como mucho).
        (self.seconds as f64 * (1.0 + (0.001f64).ln() / fb.ln())).min(12.0)
    }

    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|b| b.fill(0.0));
        self.tone.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::super::{Chain, FxKind, FxSpec};

    #[test]
    fn eco_a_su_tiempo_y_cada_vez_mas_bajo() {
        let mut out = impulse(1.0);
        Chain::new(&[FxSpec::new(FxKind::Delay, &[("time", 200.0), ("feedback", 0.5), ("mix", 1.0), ("tone", 20_000.0)])], RATE).process_block(&mut out);
        let at = |s: f32| out[(s * RATE as f32).round() as usize * 2];
        assert!((at(0.2) - 1.0).abs() < 1e-3, "primera repetición a 200 ms: {}", at(0.2));
        let around = |s: f32| (-30..30).map(|d| out[(((s * RATE as f32) as i32 + d) as usize) * 2].abs()).fold(0f32, f32::max);
        assert!(around(0.4) > 0.3 && around(0.4) < 0.7, "segunda, a la mitad: {}", around(0.4));
        assert!(around(0.6) < around(0.4));
        assert!(around(0.1) < 1e-6, "nada antes de tiempo");
    }

    #[test]
    fn ping_pong_alterna() {
        let mut out = impulse(0.5);
        Chain::new(&[FxSpec::new(FxKind::Delay, &[("time", 100.0), ("feedback", 0.6), ("mix", 1.0), ("pingpong", 1.0), ("tone", 20_000.0)])], RATE).process_block(&mut out);
        let i = (0.1 * RATE as f32) as usize * 2;
        assert!(out[i].abs() > 0.4 && out[i + 1].abs() < 1e-3, "primero a la izquierda");
        let j = (0.2 * RATE as f32) as usize * 2;
        let right = (j - 60..j + 60).step_by(2).map(|k| out[k + 1].abs()).fold(0f32, f32::max);
        assert!(right > 0.2, "luego a la derecha: {right}");
    }
}
