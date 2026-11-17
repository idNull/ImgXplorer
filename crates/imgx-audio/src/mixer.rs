//! Mezclador: el mismo cálculo sirve para oír en directo y para exportar el vídeo, así que lo que
//! suena es lo que sale en el MP4.
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::decode::Pcm;
use crate::fx::{Chain, FxSpec};

/// Pista a la que va un clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bus {
    Voice,
    Sfx,
    Ambient,
}

impl Bus {
    pub const ALL: [Bus; 3] = [Bus::Voice, Bus::Sfx, Bus::Ambient];

    /// Nombre (y id de la pista por defecto del grupo).
    pub fn name(self) -> &'static str {
        match self {
            Bus::Voice => "voice",
            Bus::Sfx => "sfx",
            Bus::Ambient => "ambient",
        }
    }
}

/// Un sonido colocado en el tiempo.
#[derive(Debug, Clone)]
pub struct Clip {
    pub sound: Arc<Pcm>,
    pub bus: Bus,
    /// Segundo de la línea de tiempo en que empieza.
    pub start: f64,
    /// Segundos que se saltan del principio del sonido.
    pub trim_in: f64,
    /// Cuánto suena (en bucle puede ser más largo que el sonido).
    pub duration: f64,
    /// 1 = volumen original.
    pub volume: f32,
    pub fade_in: f64,
    pub fade_out: f64,
    /// Se repite mientras dure (ambientes).
    pub looped: bool,
    /// Pista en la que suena (si no existe, la primera de su grupo).
    pub track: String,
    /// Efectos del propio clip.
    pub effects: Vec<FxSpec>,
}

impl Clip {
    /// Clip con volumen 1, sin recorte, fundidos ni efectos, en la pista por defecto de su grupo.
    pub fn new(sound: Arc<Pcm>, bus: Bus, start: f64, duration: f64) -> Clip {
        Clip {
            sound,
            bus,
            start,
            trim_in: 0.0,
            duration,
            volume: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            looped: false,
            track: bus.name().into(),
            effects: Vec::new(),
        }
    }

    pub fn end(&self) -> f64 {
        self.start + self.duration
    }

    /// Envolvente (volumen y fundidos) en el instante `rel` desde el inicio del clip.
    #[inline]
    pub fn gain(&self, rel: f64) -> f32 {
        if rel < 0.0 || rel >= self.duration {
            return 0.0;
        }
        let mut g = 1.0f64;
        if self.fade_in > 0.0 && rel < self.fade_in {
            g *= rel / self.fade_in;
        }
        let left = self.duration - rel;
        if self.fade_out > 0.0 && left < self.fade_out {
            g *= left / self.fade_out;
        }
        g as f32 * self.volume
    }

    /// Muestra estéreo a `rel` segundos del inicio del clip, sin envolvente.
    #[inline]
    pub fn raw(&self, rel: f64) -> (f32, f32) {
        let s = &self.sound;
        let len = s.seconds();
        if len <= 0.0 {
            return (0.0, 0.0);
        }
        let mut pos = self.trim_in + rel;
        if self.looped {
            pos = pos.rem_euclid(len);
        } else if pos >= len {
            return (0.0, 0.0);
        }
        interp(s, pos * s.rate as f64, self.looped)
    }

    /// Muestra con envolvente en el instante `t` de la línea de tiempo.
    #[inline]
    pub fn sample(&self, t: f64) -> (f32, f32) {
        let rel = t - self.start;
        let g = self.gain(rel);
        if g == 0.0 {
            return (0.0, 0.0);
        }
        let (l, r) = self.raw(rel);
        (l * g, r * g)
    }
}

/// Interpolación cúbica (Catmull-Rom) entre muestras.
#[inline]
fn interp(s: &Pcm, x: f64, wrap: bool) -> (f32, f32) {
    let i = x.floor() as isize;
    let f = (x - i as f64) as f32;
    let n = s.frames() as isize;
    let at = |k: isize| if wrap && n > 0 { s.frame(k.rem_euclid(n)) } else { s.frame(k) };
    let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
    let c = |a: f32, b: f32, c: f32, d: f32| b + 0.5 * f * (c - a + f * (2.0 * a - 5.0 * b + 4.0 * c - d + f * (3.0 * (b - c) + d - a)));
    (c(p0.0, p1.0, p2.0, p3.0), c(p0.1, p1.1, p2.1, p3.1))
}

/// Volumen de cada pista y si el ambiente baja cuando hay voz.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Levels {
    pub voice: f32,
    pub sfx: f32,
    pub ambient: f32,
    pub duck: bool,
}

impl Default for Levels {
    fn default() -> Self {
        Levels { voice: 1.0, sfx: 1.0, ambient: 0.7, duck: true }
    }
}

/// Hasta dónde baja el ambiente mientras hay voz.
pub const DUCK_LEVEL: f32 = 0.35;
const DUCK_ATTACK: f64 = 0.15;
const DUCK_RELEASE: f64 = 0.4;

/// Cuánto se atenúa el ambiente (0 nada, 1 del todo) en `t`, según dónde hay clips de voz.
/// Sube en `DUCK_ATTACK` antes de que empiece la voz y vuelve en `DUCK_RELEASE` tras acabar.
pub fn duck_amount<'a>(voices: impl IntoIterator<Item = &'a Clip>, t: f64) -> f32 {
    let mut w = 0f64;
    for c in voices {
        let a = if t < c.start { 1.0 - (c.start - t) / DUCK_ATTACK } else if t <= c.end() { 1.0 } else { 1.0 - (t - c.end()) / DUCK_RELEASE };
        w = w.max(a.clamp(0.0, 1.0));
    }
    w as f32
}

/// Ganancia final del ambiente según la atenuación.
#[inline]
pub fn duck_gain(amount: f32) -> f32 {
    1.0 - amount * (1.0 - DUCK_LEVEL)
}

/// Limitador suave: deja pasar lo normal y redondea lo que se pasaría de ±1.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.9 { x } else { x.signum() * (0.9 + 0.1 * ((a - 0.9) / 0.1).tanh()) }
}

/// Una pista: grupo, volumen, panorama (−1 izquierda … 1 derecha), silencio y efectos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSpec {
    pub id: String,
    pub bus: Bus,
    #[serde(default = "one")]
    pub volume: f32,
    #[serde(default)]
    pub pan: f32,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub effects: Vec<FxSpec>,
}

fn one() -> f32 {
    1.0
}

impl TrackSpec {
    /// Pista por defecto de un grupo (la de los proyectos sin pistas).
    pub fn default_for(bus: Bus) -> TrackSpec {
        TrackSpec { id: bus.name().into(), bus, volume: 1.0, pan: 0.0, mute: false, effects: Vec::new() }
    }
}

/// Cadena de efectos que se apaga sola: cuando la entrada lleva callada más que la cola de los
/// efectos, deja de calcular (y se vacía para empezar limpia la próxima vez). Decide muestra a
/// muestra, así que el resultado no depende del tamaño de los bloques.
#[derive(Debug)]
pub struct FxRunner {
    chain: Chain,
    tail: u64,
    quiet: u64,
    idle: bool,
}

impl FxRunner {
    /// `None` si no hay ningún efecto encendido.
    pub fn new(specs: &[FxSpec], rate: u32) -> Option<FxRunner> {
        let chain = Chain::new(specs, rate);
        if chain.is_empty() {
            return None;
        }
        let tail = (chain.tail() * rate as f64).ceil() as u64 + 1;
        Some(FxRunner { chain, tail, quiet: u64::MAX / 2, idle: true })
    }

    pub fn configure(&mut self, specs: &[FxSpec], rate: u32) {
        self.chain.configure(specs);
        self.tail = (self.chain.tail() * rate as f64).ceil() as u64 + 1;
    }

    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }

    /// Segundos de cola.
    pub fn tail_seconds(&self, rate: u32) -> f64 {
        self.tail as f64 / rate as f64
    }

    /// Ya no suena nada.
    pub fn idle(&self) -> bool {
        self.idle
    }

    #[inline]
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        if l == 0.0 && r == 0.0 {
            self.quiet = self.quiet.saturating_add(1);
            if self.quiet > self.tail {
                if !self.idle {
                    self.chain.reset();
                    self.idle = true;
                }
                return (0.0, 0.0);
            }
        } else {
            self.quiet = 0;
            self.idle = false;
        }
        self.chain.process(l, r)
    }

    pub fn reset(&mut self) {
        self.chain.reset();
        self.quiet = u64::MAX / 2;
        self.idle = true;
    }
}

#[derive(Debug)]
struct TrackState {
    spec: TrackSpec,
    fx: Option<FxRunner>,
    audible: bool,
    buf: Vec<f32>,
}

/// Mesa de mezclas: pistas (con sus efectos, volumen y panorama), grupos (voz, efectos y
/// ambiente, con la atenuación del ambiente) y limitador. La usan el directo y la exportación.
#[derive(Debug)]
pub struct Graph {
    rate: u32,
    tracks: Vec<TrackState>,
    pub levels: Levels,
    frames: usize,
}

impl Graph {
    pub fn new(rate: u32) -> Graph {
        let mut g = Graph { rate, tracks: Vec::new(), levels: Levels::default(), frames: 0 };
        g.set_tracks(&[], &[]);
        g
    }

    /// Cambia las pistas. Las que siguen (mismo id) conservan sus efectos sonando; cada grupo
    /// tiene al menos una pista. `solo`: si no está vacío, solo suenan esas.
    pub fn set_tracks(&mut self, specs: &[TrackSpec], solo: &[String]) {
        let mut list: Vec<TrackSpec> = specs.to_vec();
        for bus in Bus::ALL {
            if !list.iter().any(|t| t.bus == bus) {
                list.push(TrackSpec::default_for(bus));
            }
        }
        let mut old = std::mem::take(&mut self.tracks);
        for spec in list {
            let audible = !spec.mute && (solo.is_empty() || solo.contains(&spec.id));
            let fx = match old.iter().position(|o| o.spec.id == spec.id) {
                Some(k) => {
                    let mut o = old.swap_remove(k);
                    match o.fx.as_mut() {
                        Some(f) => {
                            f.configure(&spec.effects, self.rate);
                            if f.is_empty() { None } else { o.fx }
                        }
                        None => FxRunner::new(&spec.effects, self.rate),
                    }
                }
                None => FxRunner::new(&spec.effects, self.rate),
            };
            self.tracks.push(TrackState { spec, fx, audible, buf: vec![0.0; self.frames * 2] });
        }
    }

    /// Índice de la pista `id`; si no existe (o es de otro grupo), la primera de `bus`.
    pub fn resolve(&self, id: &str, bus: Bus) -> usize {
        self.tracks
            .iter()
            .position(|t| t.spec.id == id && t.spec.bus == bus)
            .or_else(|| self.tracks.iter().position(|t| t.spec.bus == bus))
            .unwrap_or(0)
    }

    /// Suena (no está silenciada ni fuera del solo).
    pub fn audible(&self, track: usize) -> bool {
        self.tracks.get(track).is_some_and(|t| t.audible)
    }

    /// Prepara un bloque de `frames` muestras estéreo vacío.
    pub fn begin(&mut self, frames: usize) {
        self.frames = frames;
        for t in &mut self.tracks {
            t.buf.clear();
            t.buf.resize(frames * 2, 0.0);
        }
    }

    /// Suma una muestra a la entrada de una pista.
    #[inline]
    pub fn add(&mut self, track: usize, frame: usize, l: f32, r: f32) {
        if let Some(t) = self.tracks.get_mut(track) {
            t.buf[frame * 2] += l;
            t.buf[frame * 2 + 1] += r;
        }
    }

    /// Pasa cada pista por sus efectos, volumen y panorama y suma los grupos en `out` (sin
    /// limitar). `duck`: ganancia del ambiente en cada muestra.
    pub fn mix(&mut self, duck: &[f32], out: &mut [f32]) {
        out.fill(0.0);
        let lv = self.levels;
        for t in &mut self.tracks {
            let group = match t.spec.bus {
                Bus::Voice => lv.voice,
                Bus::Sfx => lv.sfx,
                Bus::Ambient => lv.ambient,
            };
            let pan = t.spec.pan.clamp(-1.0, 1.0);
            let k = group * t.spec.volume.clamp(0.0, 4.0);
            let (gl, gr) = (k * (1.0 - pan).min(1.0), k * (1.0 + pan).min(1.0));
            let amb = t.spec.bus == Bus::Ambient;
            for f in 0..self.frames.min(out.len() / 2) {
                let (mut l, mut r) = if t.audible { (t.buf[f * 2], t.buf[f * 2 + 1]) } else { (0.0, 0.0) };
                if let Some(fx) = t.fx.as_mut() {
                    (l, r) = fx.tick(l, r);
                }
                let d = if amb { duck.get(f).copied().unwrap_or(1.0) } else { 1.0 };
                out[f * 2] += l * gl * d;
                out[f * 2 + 1] += r * gr * d;
            }
        }
    }

    /// Calla las colas de los efectos.
    pub fn reset(&mut self) {
        for t in &mut self.tracks {
            if let Some(f) = t.fx.as_mut() {
                f.reset();
            }
        }
    }
}

/// Mezcla completa sin conexión (exportar): `seconds` de audio estéreo a `rate`.
pub fn render(clips: &[Clip], tracks: &[TrackSpec], levels: Levels, rate: u32, seconds: f64) -> Vec<f32> {
    let frames = (seconds * rate as f64).ceil().max(0.0) as usize;
    let mut g = Graph::new(rate);
    g.set_tracks(tracks, &[]);
    g.levels = levels;
    g.begin(frames);
    let mut voices: Vec<&Clip> = Vec::new();
    for c in clips {
        let track = g.resolve(&c.track, c.bus);
        if !g.audible(track) {
            continue;
        }
        if c.bus == Bus::Voice {
            voices.push(c);
        }
        let mut fx = FxRunner::new(&c.effects, rate);
        let tail = fx.as_ref().map_or(0.0, |f| f.tail_seconds(rate));
        let a = ((c.start * rate as f64).floor().max(0.0) as usize).min(frames);
        let z = (((c.end() + tail) * rate as f64).ceil().max(0.0) as usize).min(frames);
        for f in a..z {
            let (mut l, mut r) = c.sample(f as f64 / rate as f64);
            if let Some(x) = fx.as_mut() {
                (l, r) = x.tick(l, r);
            }
            g.add(track, f, l, r);
        }
    }
    let duck: Vec<f32> = if levels.duck && !voices.is_empty() {
        (0..frames).map(|f| duck_gain(duck_amount(voices.iter().copied(), f as f64 / rate as f64))).collect()
    } else {
        Vec::new()
    };
    let mut out = vec![0f32; frames * 2];
    g.mix(&duck, &mut out);
    out.iter_mut().for_each(|x| *x = soft_clip(*x));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un sonido de valor constante (fácil de comprobar).
    fn dc(v: f32, seconds: f64, rate: u32) -> Arc<Pcm> {
        let n = (seconds * rate as f64) as usize;
        Arc::new(Pcm { rate, data: vec![v; n * 2] })
    }

    fn clip(sound: Arc<Pcm>, bus: Bus, start: f64, duration: f64) -> Clip {
        Clip::new(sound, bus, start, duration)
    }

    #[test]
    fn posicion_y_duracion() {
        let c = clip(dc(0.5, 1.0, 100), Bus::Sfx, 2.0, 1.0);
        let out = render(&[c], &[], Levels { sfx: 1.0, ..Default::default() }, 100, 4.0);
        let at = |s: f64| out[(s * 100.0) as usize * 2];
        assert_eq!(at(1.5), 0.0);
        assert!((at(2.5) - 0.5).abs() < 1e-6);
        assert_eq!(at(3.5), 0.0);
    }

    #[test]
    fn recorte_y_final_del_sonido() {
        // Rampa: el valor dice en qué segundo del sonido estamos.
        let rate = 100;
        let data: Vec<f32> = (0..200).flat_map(|i| [i as f32 / 200.0, i as f32 / 200.0]).collect();
        let s = Arc::new(Pcm { rate, data });
        let c = Clip { trim_in: 0.5, ..clip(s, Bus::Voice, 0.0, 5.0) };
        assert!((c.raw(0.0).0 - 0.25).abs() < 1e-3, "empieza a los 0,5 s del sonido");
        assert_eq!(c.raw(1.6).0, 0.0, "pasado el final del sonido, silencio");
    }

    #[test]
    fn fundidos() {
        let c = Clip { fade_in: 1.0, fade_out: 0.5, ..clip(dc(1.0, 4.0, 100), Bus::Sfx, 0.0, 4.0) };
        assert!((c.gain(0.5) - 0.5).abs() < 1e-6);
        assert!((c.gain(2.0) - 1.0).abs() < 1e-6);
        assert!((c.gain(3.75) - 0.5).abs() < 1e-6);
        assert_eq!(c.gain(4.0), 0.0);
        assert_eq!(c.gain(-0.1), 0.0);
    }

    #[test]
    fn bucle() {
        let rate = 100;
        let data: Vec<f32> = (0..100).flat_map(|i| [i as f32, 0.0]).collect();
        let c = Clip { looped: true, ..clip(Arc::new(Pcm { rate, data }), Bus::Ambient, 0.0, 10.0) };
        assert!((c.raw(0.25).0 - 25.0).abs() < 1e-3);
        assert!((c.raw(3.25).0 - 25.0).abs() < 1e-3, "vuelve a empezar cada segundo");
    }

    #[test]
    fn interpolacion_entre_muestras() {
        let rate = 10;
        let data: Vec<f32> = (0..10).flat_map(|i| [i as f32, i as f32]).collect();
        let c = clip(Arc::new(Pcm { rate, data }), Bus::Sfx, 0.0, 1.0);
        assert!((c.raw(0.35).0 - 3.5).abs() < 1e-3, "{}", c.raw(0.35).0);
    }

    #[test]
    fn atenuacion_del_ambiente_con_voz() {
        let amb = Clip { looped: true, ..clip(dc(0.4, 1.0, 100), Bus::Ambient, 0.0, 10.0) };
        let voz = clip(dc(0.0, 2.0, 100), Bus::Voice, 4.0, 2.0);
        let lv = Levels { voice: 1.0, sfx: 1.0, ambient: 1.0, duck: true };
        let out = render(&[amb.clone(), voz.clone()], &[], lv, 100, 10.0);
        let at = |s: f64| out[(s * 100.0) as usize * 2];
        assert!((at(1.0) - 0.4).abs() < 1e-4, "sin voz, entero");
        assert!((at(5.0) - 0.4 * DUCK_LEVEL).abs() < 1e-4, "con voz, atenuado");
        assert!((at(9.0) - 0.4).abs() < 1e-4, "se recupera");
        assert!(at(3.9) < 0.4 && at(3.9) > 0.4 * DUCK_LEVEL, "baja antes de que hable");
        // Sin atenuación activada, siempre entero.
        let out2 = render(&[amb, voz], &[], Levels { duck: false, ..lv }, 100, 10.0);
        assert!((out2[500 * 2] - 0.4).abs() < 1e-4);
    }

    #[test]
    fn volumen_por_pista_y_limitador() {
        let a = clip(dc(0.8, 1.0, 100), Bus::Voice, 0.0, 1.0);
        let b = clip(dc(0.8, 1.0, 100), Bus::Sfx, 0.0, 1.0);
        let out = render(&[a.clone(), b], &[], Levels { voice: 1.0, sfx: 1.0, ambient: 1.0, duck: false }, 100, 1.0);
        assert!(out[100] <= 1.0 && out[100] > 0.9, "1,6 se limita por debajo de 1: {}", out[100]);
        let half = render(&[a], &[], Levels { voice: 0.5, ..Default::default() }, 100, 1.0);
        assert!((half[100] - 0.4).abs() < 1e-6);
    }

    fn track(id: &str, bus: Bus) -> TrackSpec {
        TrackSpec { id: id.into(), ..TrackSpec::default_for(bus) }
    }

    #[test]
    fn pistas_volumen_panorama_y_silencio() {
        let lv = Levels { voice: 1.0, sfx: 1.0, ambient: 1.0, duck: false };
        let ana = Clip { track: "ana".into(), ..clip(dc(0.5, 1.0, 100), Bus::Voice, 0.0, 1.0) };
        let luis = Clip { track: "luis".into(), ..clip(dc(0.2, 1.0, 100), Bus::Voice, 0.0, 1.0) };
        let tracks = vec![
            TrackSpec { volume: 0.5, pan: -1.0, ..track("ana", Bus::Voice) },
            TrackSpec { pan: 1.0, ..track("luis", Bus::Voice) },
        ];
        let out = render(&[ana.clone(), luis.clone()], &tracks, lv, 100, 1.0);
        assert!((out[100] - 0.25).abs() < 1e-6, "Ana a la izquierda, a la mitad: {}", out[100]);
        assert!((out[101] - 0.2).abs() < 1e-6, "Luis a la derecha: {}", out[101]);
        let muted = vec![TrackSpec { mute: true, ..tracks[0].clone() }, tracks[1].clone()];
        let out = render(&[ana, luis], &muted, lv, 100, 1.0);
        assert_eq!(out[100], 0.0, "Ana silenciada");
        // Un clip de una pista que no existe va a la primera de su grupo.
        let lost = Clip { track: "nadie".into(), ..clip(dc(0.3, 1.0, 100), Bus::Voice, 0.0, 1.0) };
        let out = render(&[lost], &tracks, lv, 100, 1.0);
        assert!((out[100] - 0.15).abs() < 1e-6 && out[101] == 0.0, "a la pista de Ana");
    }

    #[test]
    fn la_voz_silenciada_no_atenua_el_ambiente() {
        let lv = Levels { voice: 1.0, sfx: 1.0, ambient: 1.0, duck: true };
        let amb = Clip { looped: true, ..clip(dc(0.4, 1.0, 100), Bus::Ambient, 0.0, 4.0) };
        let voz = clip(dc(0.3, 4.0, 100), Bus::Voice, 0.0, 4.0);
        let out = render(&[amb, voz], &[TrackSpec { mute: true, ..TrackSpec::default_for(Bus::Voice) }], lv, 100, 4.0);
        assert!((out[200] - 0.4).abs() < 1e-4, "{}", out[200]);
    }

    #[test]
    fn el_eco_de_un_clip_suena_despues_de_su_final() {
        let rate = 8000;
        let eco = crate::fx::FxSpec::new(crate::fx::FxKind::Delay, &[("time", 300.0), ("feedback", 0.3), ("mix", 1.0)]);
        let c = Clip { effects: vec![eco.clone()], ..clip(dc(0.5, 0.1, rate), Bus::Sfx, 0.0, 0.1) };
        let out = render(&[c], &[], Levels::default(), rate, 1.0);
        let around = |s: f64| out[(s * rate as f64) as usize * 2];
        assert!(around(0.2).abs() < 1e-6, "silencio entre el sonido y el eco");
        assert!(around(0.35).abs() > 0.2, "el eco a los 300 ms: {}", around(0.35));
        // El mismo eco en la pista.
        let c = clip(dc(0.5, 0.1, rate), Bus::Sfx, 0.0, 0.1);
        let out = render(&[c], &[TrackSpec { effects: vec![eco], ..TrackSpec::default_for(Bus::Sfx) }], Levels::default(), rate, 1.0);
        assert!(out[(0.35 * rate as f64) as usize * 2].abs() > 0.2);
    }
}
