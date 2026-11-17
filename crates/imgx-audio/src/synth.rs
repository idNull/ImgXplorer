//! Efectos de serie fabricados por síntesis: no hay archivos que descargar ni licencias.
//! Todo es determinista (misma semilla, mismo sonido) para que suene igual en todas partes.
use std::f32::consts::TAU;

use crate::decode::Pcm;

const RATE: u32 = 44100;

/// Un efecto de serie: `id` estable (se guarda en los proyectos) y si está pensado para repetirse
/// en bucle (ambientes).
#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub id: &'static str,
    pub seconds: f64,
    pub ambient: bool,
}

pub const BUILTINS: &[Builtin] = &[
    Builtin { id: "rain", seconds: 8.0, ambient: true },
    Builtin { id: "wind", seconds: 8.0, ambient: true },
    Builtin { id: "waves", seconds: 8.0, ambient: true },
    Builtin { id: "heartbeat", seconds: 2.0, ambient: true },
    Builtin { id: "thunder", seconds: 4.0, ambient: false },
    Builtin { id: "hit", seconds: 0.6, ambient: false },
    Builtin { id: "punch", seconds: 0.4, ambient: false },
    Builtin { id: "whoosh", seconds: 0.8, ambient: false },
    Builtin { id: "explosion", seconds: 3.0, ambient: false },
    Builtin { id: "knock", seconds: 1.2, ambient: false },
    Builtin { id: "steps", seconds: 2.0, ambient: false },
    Builtin { id: "bell", seconds: 3.0, ambient: false },
    Builtin { id: "ding", seconds: 1.6, ambient: false },
    Builtin { id: "shot", seconds: 1.0, ambient: false },
];

/// Genera el efecto `id` (None si no existe).
pub fn builtin(id: &str) -> Option<Pcm> {
    let b = BUILTINS.iter().find(|b| b.id == id)?;
    let n = (b.seconds * RATE as f64).round() as usize;
    let (l, r) = match id {
        "rain" => looped(n, |n| stereo(n, 11, rain)),
        "wind" => looped(n, |n| stereo(n, 21, wind)),
        "waves" => looped(n, |n| stereo(n, 31, waves)),
        "heartbeat" => mono(heartbeat(n)),
        "thunder" => stereo(n, 41, thunder),
        "hit" => mono(hit(n, 51)),
        "punch" => mono(punch(n, 61)),
        "whoosh" => whoosh(n, 71),
        "explosion" => stereo(n, 81, explosion),
        "knock" => mono(knock(n, 91)),
        "steps" => mono(steps(n, 101)),
        "bell" => mono(bell(n, 520.0)),
        "ding" => mono(ding(n)),
        "shot" => mono(shot(n, 111)),
        _ => return None,
    };
    Some(finish(l, r))
}

/// Un tono puro (para pruebas).
pub fn sine(freq: f32, seconds: f32, rate: u32) -> Pcm {
    let n = (seconds * rate as f32) as usize;
    let s: Vec<f32> = (0..n).map(|i| 0.5 * (TAU * freq * i as f32 / rate as f32).sin()).collect();
    Pcm::from_interleaved(rate, 1, &s)
}

// ------------------------------------------------------------------ piezas

/// Generador pseudoaleatorio pequeño y determinista (xorshift).
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Uniforme en [-1, 1).
    fn noise(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
    /// Uniforme en [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Paso bajo de un polo.
struct Low {
    y: f32,
}
impl Low {
    fn new() -> Low {
        Low { y: 0.0 }
    }
    fn run(&mut self, x: f32, fc: f32) -> f32 {
        let a = 1.0 - (-TAU * fc / RATE as f32).exp();
        self.y += a * (x - self.y);
        self.y
    }
}

/// Filtro de variable de estado (paso banda), para resonancias y barridos.
struct Band {
    low: f32,
    band: f32,
}
impl Band {
    fn new() -> Band {
        Band { low: 0.0, band: 0.0 }
    }
    fn run(&mut self, x: f32, fc: f32, q: f32) -> f32 {
        let f = 2.0 * (std::f32::consts::PI * fc.min(RATE as f32 * 0.24) / RATE as f32).sin();
        let high = x - self.low - self.band / q;
        self.band += f * high;
        self.low += f * self.band;
        self.band
    }
}

fn t(i: usize) -> f32 {
    i as f32 / RATE as f32
}

fn mono(v: Vec<f32>) -> (Vec<f32>, Vec<f32>) {
    (v.clone(), v)
}

/// Dos canales independientes (texturas con amplitud estéreo).
fn stereo(n: usize, seed: u64, f: fn(usize, u64) -> Vec<f32>) -> (Vec<f32>, Vec<f32>) {
    (f(n, seed), f(n, seed + 1000))
}

/// Genera `n` + un tramo extra y lo funde con el principio para que el bucle no tenga costura.
fn looped(n: usize, f: impl Fn(usize) -> (Vec<f32>, Vec<f32>)) -> (Vec<f32>, Vec<f32>) {
    let xf = RATE as usize / 2;
    let (mut l, mut r) = f(n + xf);
    for ch in [&mut l, &mut r] {
        for i in 0..xf {
            let k = i as f32 / xf as f32;
            ch[i] = ch[i] * k + ch[n + i] * (1.0 - k);
        }
        ch.truncate(n);
    }
    (l, r)
}

/// Normaliza a un pico de 0,8 y monta el estéreo.
fn finish(l: Vec<f32>, r: Vec<f32>) -> Pcm {
    let peak = l.iter().chain(&r).fold(0f32, |m, v| m.max(v.abs())).max(1e-6);
    let g = 0.8 / peak;
    let mut data = Vec::with_capacity(l.len() * 2);
    for (a, b) in l.iter().zip(&r) {
        data.push(a * g);
        data.push(b * g);
    }
    Pcm { rate: RATE, data }
}

fn env_exp(x: f32, tau: f32) -> f32 {
    (-x / tau).exp()
}

// ------------------------------------------------------------------ efectos

fn rain(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let (mut lo, mut hi) = (Low::new(), Low::new());
    let mut drops: Vec<(usize, f32, f32)> = Vec::new(); // (inicio, frecuencia, volumen)
    let mut i = 0;
    while i < n {
        drops.push((i, 1500.0 + 3000.0 * rng.unit(), 0.2 + 0.8 * rng.unit()));
        i += (RATE as f32 * (0.005 + 0.03 * rng.unit())) as usize;
    }
    let mut out = vec![0f32; n];
    for o in out.iter_mut() {
        let w = rng.noise();
        let body = lo.run(w, 3500.0) - hi.run(w, 400.0) * 0.6;
        *o = body * 0.5;
    }
    let mut bp = Band::new();
    for (s, f, v) in drops {
        for k in 0..(RATE as usize / 60).min(n - s) {
            let e = env_exp(t(k), 0.004) * v;
            out[s + k] += bp.run(rng.noise(), f, 6.0) * e * 0.6;
        }
    }
    out
}

fn wind(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let mut bp = Band::new();
    let dur = n as f32 / RATE as f32;
    (0..n)
        .map(|i| {
            let x = t(i);
            // Ráfagas lentas con periodos que encajan en la duración (bucle limpio).
            let gust = 0.55 + 0.25 * (TAU * x / dur * 2.0).sin() + 0.2 * (TAU * x / dur * 5.0 + 1.3).sin();
            let fc = 300.0 + 500.0 * gust;
            bp.run(rng.noise(), fc, 3.0) * gust
        })
        .collect()
}

fn waves(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let mut lo = Low::new();
    let dur = n as f32 / RATE as f32;
    (0..n)
        .map(|i| {
            let x = t(i);
            let swell = (0.5 - 0.5 * (TAU * x / dur * 2.0).cos()).powf(1.5);
            lo.run(rng.noise(), 400.0 + 2500.0 * swell) * (0.15 + 0.85 * swell)
        })
        .collect()
}

fn heartbeat(n: usize) -> Vec<f32> {
    let mut out = vec![0f32; n];
    for beat in [0.0, 1.0] {
        for (off, amp) in [(0.0, 1.0), (0.28, 0.7)] {
            let s = ((beat + off) * RATE as f32) as usize;
            let mut ph = 0f32;
            for k in 0..(RATE as usize / 4).min(n.saturating_sub(s)) {
                let x = t(k);
                let f = 45.0 + 35.0 * env_exp(x, 0.03);
                ph += TAU * f / RATE as f32;
                out[s + k] += ph.sin() * env_exp(x, 0.06) * amp * (x / 0.004).min(1.0);
            }
        }
    }
    out
}

fn thunder(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let (mut lo, mut crack) = (Low::new(), Low::new());
    (0..n)
        .map(|i| {
            let x = t(i);
            let w = rng.noise();
            let rumble = lo.run(w, 120.0 + 200.0 * env_exp(x, 0.6)) * 3.0 * env_exp(x, 1.2) * (x / 0.05).min(1.0);
            let snap = (crack.run(w, 3000.0) * 0.8) * env_exp(x, 0.12);
            // Segundo trueno más lejano.
            let late = if x > 1.1 { lo.y * 1.5 * env_exp(x - 1.1, 0.9) } else { 0.0 };
            rumble + snap + late
        })
        .collect()
}

fn thump(n: usize, f0: f32, f1: f32, tau: f32) -> Vec<f32> {
    let mut ph = 0f32;
    (0..n)
        .map(|i| {
            let x = t(i);
            ph += TAU * (f1 + (f0 - f1) * env_exp(x, 0.02)) / RATE as f32;
            ph.sin() * env_exp(x, tau) * (x / 0.002).min(1.0)
        })
        .collect()
}

fn hit(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let mut lo = Low::new();
    thump(n, 140.0, 50.0, 0.12)
        .into_iter()
        .enumerate()
        .map(|(i, s)| s + lo.run(rng.noise(), 2500.0) * env_exp(t(i), 0.015) * 0.8)
        .collect()
}

fn punch(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let mut bp = Band::new();
    thump(n, 180.0, 70.0, 0.06)
        .into_iter()
        .enumerate()
        .map(|(i, s)| s * 0.9 + bp.run(rng.noise(), 1200.0, 1.5) * env_exp(t(i), 0.03) * 1.4)
        .collect()
}

fn whoosh(n: usize, seed: u64) -> (Vec<f32>, Vec<f32>) {
    let mut rng = Rng::new(seed);
    let (mut a, mut b) = (Band::new(), Band::new());
    let dur = n as f32 / RATE as f32;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        let u = t(i) / dur;
        let bell = (std::f32::consts::PI * u).sin().powi(2);
        let fc = 300.0 + 2200.0 * bell;
        let w = rng.noise();
        // Paneo de izquierda a derecha.
        let s = a.run(w, fc, 2.0) * bell;
        let s2 = b.run(w, fc * 1.1, 2.0) * bell;
        l.push(s * (1.0 - u * 0.7));
        r.push(s2 * (0.3 + u * 0.7));
    }
    (l, r)
}

fn explosion(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let (mut lo, mut sub) = (Low::new(), Low::new());
    (0..n)
        .map(|i| {
            let x = t(i);
            let w = rng.noise();
            let body = lo.run(w, 200.0 + 5000.0 * env_exp(x, 0.15)) * env_exp(x, 0.7) * (x / 0.003).min(1.0);
            let boom = sub.run(w, 80.0) * 4.0 * env_exp(x, 1.0);
            body + boom
        })
        .collect()
}

fn knock(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let mut out = vec![0f32; n];
    for (k, at) in [0.0f32, 0.22, 0.44].iter().enumerate() {
        let s = (at * RATE as f32) as usize;
        let (mut a, mut b) = (Band::new(), Band::new());
        let amp = [1.0, 0.85, 0.95][k];
        for j in 0..(RATE as usize / 6).min(n.saturating_sub(s)) {
            let x = t(j);
            let w = rng.noise() * env_exp(x, 0.002);
            out[s + j] += (a.run(w, 190.0, 12.0) * 1.5 + b.run(w, 420.0, 8.0)) * amp * env_exp(x, 0.05);
        }
    }
    out
}

fn steps(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let mut out = vec![0f32; n];
    for k in 0..4 {
        let s = ((0.05 + 0.48 * k as f32 + 0.03 * rng.unit()) * RATE as f32) as usize;
        let mut lo = Low::new();
        let amp = 0.7 + 0.3 * rng.unit();
        for j in 0..(RATE as usize / 8).min(n.saturating_sub(s)) {
            let x = t(j);
            // Talón y punta.
            let e = env_exp(x, 0.02) + 0.6 * if x > 0.05 { env_exp(x - 0.05, 0.015) } else { 0.0 };
            out[s + j] += lo.run(rng.noise(), 700.0) * e * amp * 2.0;
        }
    }
    out
}

fn bell(n: usize, f0: f32) -> Vec<f32> {
    let partials = [(1.0, 1.0, 1.4), (2.0, 0.6, 1.0), (2.4, 0.45, 0.8), (3.0, 0.35, 0.6), (4.5, 0.25, 0.4), (5.3, 0.15, 0.3)];
    (0..n)
        .map(|i| {
            let x = t(i);
            partials.iter().map(|(r, a, tau)| (TAU * f0 * r * x).sin() * a * env_exp(x, *tau)).sum::<f32>() * (x / 0.002).min(1.0)
        })
        .collect()
}

fn ding(n: usize) -> Vec<f32> {
    let mut out = vec![0f32; n];
    for (at, f) in [(0.0f32, 659.0f32), (0.5, 523.0)] {
        let s = (at * RATE as f32) as usize;
        for j in 0..n.saturating_sub(s) {
            let x = t(j);
            let v = (TAU * f * x).sin() + 0.3 * (TAU * 2.0 * f * x).sin() + 0.1 * (TAU * 3.0 * f * x).sin();
            out[s + j] += v * env_exp(x, 0.45) * (x / 0.003).min(1.0);
        }
    }
    out
}

fn shot(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let (mut lo, mut tail) = (Low::new(), Low::new());
    (0..n)
        .map(|i| {
            let x = t(i);
            let w = rng.noise();
            let crack = lo.run(w, 6000.0) * env_exp(x, 0.012) * 1.5;
            let body = tail.run(w, 400.0) * 3.0 * env_exp(x, 0.25) * (x / 0.002).min(1.0);
            crack + body
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_existen_duran_lo_dicho_y_no_saturan() {
        for b in BUILTINS {
            let p = builtin(b.id).unwrap_or_else(|| panic!("{}", b.id));
            assert!((p.seconds() - b.seconds).abs() < 0.01, "{}: {} s", b.id, p.seconds());
            let peak = p.peaks(1)[0];
            assert!(peak > 0.7 && peak <= 0.8001, "{}: pico {peak}", b.id);
            assert!(p.data.iter().all(|v| v.is_finite()), "{}", b.id);
            // Algo de energía repartida (no un único chasquido).
            let loud = p.peaks(20).iter().filter(|v| **v > 0.05).count();
            assert!(loud >= 1, "{}", b.id);
        }
        assert!(builtin("nada").is_none());
    }

    #[test]
    fn es_determinista() {
        assert_eq!(builtin("rain"), builtin("rain"));
        assert_eq!(builtin("knock"), builtin("knock"));
    }

    #[test]
    fn los_ambientes_enlazan_sin_salto() {
        for id in ["rain", "wind", "waves"] {
            let p = builtin(id).unwrap();
            let (a, b) = (p.frame(p.frames() as isize - 1), p.frame(0));
            // Media de la diferencia entre muestras vecinas en el interior, para comparar.
            let typical = (1..2000).map(|i| (p.frame(i).0 - p.frame(i - 1).0).abs()).sum::<f32>() / 2000.0;
            assert!((a.0 - b.0).abs() < typical * 8.0 + 0.02, "{id}: salto {} (típico {typical})", (a.0 - b.0).abs());
        }
    }
}
