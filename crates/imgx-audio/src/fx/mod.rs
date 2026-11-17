//! Efectos de sonido: reverb, eco, ecualizador, filtros, compresor, tono, robot y distorsión.
//!
//! Todos procesan muestra a muestra y son deterministas: da igual en qué bloques llegue el sonido,
//! así que la mezcla en directo y la del vídeo exportado salen iguales.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

mod delay;
mod dynamics;
mod filters;
mod pitch;
mod reverb;
mod shape;

pub use filters::Biquad;

/// Tipo de efecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FxKind {
    Reverb,
    Delay,
    Eq,
    Filter,
    Compressor,
    Pitch,
    Robot,
    Distortion,
}

/// Un efecto tal como llega de la interfaz: tipo, encendido y sus mandos (los que falten toman
/// su valor por defecto).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxSpec {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type")]
    pub kind: FxKind,
    #[serde(default = "yes")]
    pub on: bool,
    #[serde(default)]
    pub params: BTreeMap<String, f32>,
}

fn yes() -> bool {
    true
}

impl FxSpec {
    pub fn new(kind: FxKind, params: &[(&str, f32)]) -> FxSpec {
        FxSpec { id: String::new(), kind, on: true, params: params.iter().map(|(k, v)| (k.to_string(), *v)).collect() }
    }
}

/// Lectura de mandos con valor por defecto y límites.
pub(crate) struct P<'a>(&'a BTreeMap<String, f32>);

impl P<'_> {
    pub fn get(&self, name: &str, default: f32, min: f32, max: f32) -> f32 {
        let v = self.0.get(name).copied().unwrap_or(default);
        if v.is_finite() { v.clamp(min, max) } else { default }
    }
}

/// Un efecto en marcha.
pub(crate) trait Unit: Send + std::fmt::Debug {
    /// Aplica mandos nuevos sin vaciar su memoria (la cola sigue sonando).
    fn set(&mut self, p: &P, rate: f32);
    fn process(&mut self, l: f32, r: f32) -> (f32, f32);
    /// Segundos que puede seguir sonando después de que la entrada se calle.
    fn tail(&self) -> f64;
    fn reset(&mut self);
}

fn unit(kind: FxKind, p: &P, rate: f32) -> Box<dyn Unit> {
    let mut u: Box<dyn Unit> = match kind {
        FxKind::Reverb => Box::new(reverb::Reverb::new(rate)),
        FxKind::Delay => Box::new(delay::Delay::new(rate)),
        FxKind::Eq => Box::new(filters::Eq::default()),
        FxKind::Filter => Box::new(filters::Filter::default()),
        FxKind::Compressor => Box::new(dynamics::Compressor::default()),
        FxKind::Pitch => Box::new(pitch::Pitch::new(rate)),
        FxKind::Robot => Box::new(shape::Robot::new(rate)),
        FxKind::Distortion => Box::new(shape::Distortion::default()),
    };
    u.set(p, rate);
    u
}

#[derive(Debug)]
struct Slot {
    id: String,
    kind: FxKind,
    on: bool,
    unit: Box<dyn Unit>,
}

/// Cadena de efectos (en orden) de una pista o de un clip.
#[derive(Debug, Default)]
pub struct Chain {
    rate: f32,
    slots: Vec<Slot>,
}

impl Chain {
    pub fn new(specs: &[FxSpec], rate: u32) -> Chain {
        let mut c = Chain { rate: rate as f32, slots: Vec::new() };
        c.configure(specs);
        c
    }

    /// Cambia la cadena. Los efectos que siguen (mismo id y tipo) conservan su memoria; un efecto
    /// que se enciende empieza limpio.
    pub fn configure(&mut self, specs: &[FxSpec]) {
        let mut old = std::mem::take(&mut self.slots);
        for (i, s) in specs.iter().enumerate() {
            let p = P(&s.params);
            let id = if s.id.is_empty() { format!("#{i}") } else { s.id.clone() };
            let found = old.iter().position(|o| o.id == id && o.kind == s.kind);
            let slot = match found {
                Some(k) => {
                    let mut o = old.swap_remove(k);
                    o.unit.set(&p, self.rate);
                    if s.on && !o.on {
                        o.unit.reset();
                    }
                    o.on = s.on;
                    o
                }
                None => Slot { id, kind: s.kind, on: s.on, unit: unit(s.kind, &p, self.rate) },
            };
            self.slots.push(slot);
        }
    }

    /// No hace nada (sin efectos encendidos).
    pub fn is_empty(&self) -> bool {
        !self.slots.iter().any(|s| s.on)
    }

    #[inline]
    pub fn process(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        for s in &mut self.slots {
            if s.on {
                (l, r) = s.unit.process(l, r);
            }
        }
        (l, r)
    }

    /// Procesa un bloque estéreo intercalado.
    pub fn process_block(&mut self, buf: &mut [f32]) {
        if self.is_empty() {
            return;
        }
        for f in buf.as_chunks_mut::<2>().0 {
            let (l, r) = self.process(f[0], f[1]);
            f[0] = l;
            f[1] = r;
        }
    }

    /// Lo que puede seguir sonando tras callarse la entrada.
    pub fn tail(&self) -> f64 {
        self.slots.iter().filter(|s| s.on).map(|s| s.unit.tail()).sum()
    }

    pub fn reset(&mut self) {
        for s in &mut self.slots {
            s.unit.reset();
        }
    }
}

/// Mezcla seca/procesada.
#[inline]
pub(crate) fn wet(dry: f32, wet: f32, mix: f32) -> f32 {
    dry * (1.0 - mix) + wet * mix
}

/// Evita números subnormales (muy lentos) en las memorias que se realimentan.
#[inline]
pub(crate) fn flush(x: f32) -> f32 {
    if x.abs() < 1e-20 { 0.0 } else { x }
}

#[cfg(test)]
pub(crate) mod test_util {
    pub const RATE: u32 = 48_000;

    pub fn sine(freq: f32, seconds: f32, amp: f32) -> Vec<f32> {
        let n = (seconds * RATE as f32) as usize;
        (0..n).flat_map(|i| {
            let v = amp * (std::f32::consts::TAU * freq * i as f32 / RATE as f32).sin();
            [v, v]
        }).collect()
    }

    pub fn impulse(seconds: f32) -> Vec<f32> {
        let mut v = vec![0f32; (seconds * RATE as f32) as usize * 2];
        v[0] = 1.0;
        v[1] = 1.0;
        v
    }

    /// Valor eficaz del canal izquierdo en un tramo [a, b) de segundos.
    pub fn rms(buf: &[f32], a: f32, b: f32) -> f32 {
        let (i, j) = ((a * RATE as f32) as usize, ((b * RATE as f32) as usize).min(buf.len() / 2));
        let s: f32 = (i..j).map(|k| buf[k * 2] * buf[k * 2]).sum();
        (s / (j - i).max(1) as f32).sqrt()
    }

    /// Frecuencia del canal izquierdo contando cruces por cero (de subida) en [a, b).
    pub fn freq(buf: &[f32], a: f32, b: f32) -> f32 {
        let (i, j) = ((a * RATE as f32) as usize, (b * RATE as f32) as usize);
        let ups = (i + 1..j).filter(|&k| buf[(k - 1) * 2] < 0.0 && buf[k * 2] >= 0.0).count();
        ups as f32 / (b - a)
    }

    pub fn finite(buf: &[f32]) -> bool {
        buf.iter().all(|x| x.is_finite() && x.abs() < 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::*;
    use super::*;

    fn run(specs: &[FxSpec], input: &[f32]) -> Vec<f32> {
        let mut c = Chain::new(specs, RATE);
        let mut out = input.to_vec();
        c.process_block(&mut out);
        out
    }

    const ALL: [FxKind; 8] = [FxKind::Reverb, FxKind::Delay, FxKind::Eq, FxKind::Filter, FxKind::Compressor, FxKind::Pitch, FxKind::Robot, FxKind::Distortion];

    #[test]
    fn apagado_no_cambia_nada() {
        let input = sine(330.0, 0.2, 0.5);
        for k in ALL {
            let spec = FxSpec { on: false, ..FxSpec::new(k, &[]) };
            assert_eq!(run(&[spec], &input), input, "{k:?}");
        }
    }

    #[test]
    fn todos_estables_y_deterministas() {
        let mut input = sine(220.0, 0.5, 0.8);
        input.extend(vec![0.0; RATE as usize]);
        for k in ALL {
            let a = run(&[FxSpec::new(k, &[])], &input);
            assert!(finite(&a), "{k:?} da valores raros");
            assert_eq!(a, run(&[FxSpec::new(k, &[])], &input), "{k:?} no es determinista");
        }
    }

    #[test]
    fn da_igual_el_tamano_del_bloque() {
        let input = sine(440.0, 0.3, 0.6);
        let specs = [FxSpec::new(FxKind::Reverb, &[]), FxSpec::new(FxKind::Delay, &[("time", 50.0)]), FxSpec::new(FxKind::Pitch, &[("semitones", 5.0)])];
        let whole = run(&specs, &input);
        let mut c = Chain::new(&specs, RATE);
        let mut parts = input.clone();
        for b in parts.chunks_mut(2 * 127) {
            c.process_block(b);
        }
        assert_eq!(whole, parts);
    }

    #[test]
    fn reconfigurar_conserva_la_cola() {
        let spec = FxSpec { id: "a".into(), ..FxSpec::new(FxKind::Delay, &[("time", 100.0), ("feedback", 0.5), ("mix", 1.0)]) };
        let mut c = Chain::new(std::slice::from_ref(&spec), RATE);
        let mut buf = impulse(0.05);
        c.process_block(&mut buf);
        // Cambia la mezcla: el eco que ya está dentro sigue saliendo.
        let mut s2 = spec.clone();
        s2.params.insert("mix".into(), 0.8);
        c.configure(&[s2]);
        let mut rest = vec![0f32; RATE as usize / 5 * 2];
        c.process_block(&mut rest);
        assert!(rest.iter().any(|x| x.abs() > 0.1), "el eco no debe perderse");
    }

    #[test]
    fn cola_de_la_cadena() {
        let c = Chain::new(&[FxSpec::new(FxKind::Reverb, &[("size", 0.9)]), FxSpec::new(FxKind::Eq, &[])], RATE);
        assert!(c.tail() > 1.0);
        assert!(Chain::new(&[], RATE).is_empty());
    }
}
