//! Salida de sonido en directo y grabación del micrófono (cpal: ALSA/PipeWire/Pulse en Linux,
//! CoreAudio en macOS). El hilo de audio solo mezcla; las órdenes llegan por un canal.
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use serde::Serialize;

use crate::AudioError;
use crate::decode::Pcm;
use crate::mixer::{Bus, Clip, FxRunner, Graph, Levels, TrackSpec, duck_amount, duck_gain, soft_clip};

/// Órdenes para el motor en directo.
#[derive(Debug, Clone)]
pub enum Command {
    /// Voz y efectos de la imagen actual, con tiempos relativos a la imagen.
    Scene(Vec<Clip>),
    /// Ambientes que deben sonar ahora (id estable → clip en bucle). Los que desaparecen se funden.
    Ambient(Vec<(String, Clip)>),
    /// Reloj de la imagen. `playing`: avanza la voz y los efectos. `running`: suena el ambiente
    /// (sigue aunque la imagen esté congelada por mover la cámara a mano). `seek`: saltar ya.
    Transport { t: f64, playing: bool, running: bool, rate: f64, seek: bool },
    /// Pistas del proyecto y las que están en solo.
    Tracks { tracks: Vec<TrackSpec>, solo: Vec<String> },
    Levels(Levels),
    Master(f32),
    Mute(bool),
    /// Escuchar un sonido de la biblioteca (None para parar).
    Preview(Option<Arc<Pcm>>),
    /// Calla todo (al salir del reproductor o del editor).
    Silence,
}

/// Paso de una ganancia hacia su objetivo, sin chasquidos.
#[derive(Debug, Clone, Copy)]
struct Ramp {
    v: f32,
    target: f32,
    step: f32,
}

impl Ramp {
    fn new(v: f32) -> Ramp {
        Ramp { v, target: v, step: 1.0 }
    }
    fn to(&mut self, target: f32, seconds: f64, rate: f64) {
        self.target = target;
        self.step = (1.0 / (seconds.max(0.002) * rate)) as f32;
    }
    #[inline]
    fn tick(&mut self) -> f32 {
        if self.v < self.target {
            self.v = (self.v + self.step).min(self.target);
        } else if self.v > self.target {
            self.v = (self.v - self.step).max(self.target);
        }
        self.v
    }
    fn silent(&self) -> bool {
        self.v <= 0.0 && self.target <= 0.0
    }
}

/// Un clip sonando, con su pista ya buscada y sus efectos.
#[derive(Debug)]
struct Voice {
    clip: Clip,
    track: usize,
    fx: Option<FxRunner>,
}

impl Voice {
    /// Pasa una muestra por los efectos del clip y la suma a su pista.
    #[inline]
    fn feed(&mut self, graph: &mut Graph, frame: usize, l: f32, r: f32) {
        let (l, r) = match self.fx.as_mut() {
            Some(fx) => fx.tick(l, r),
            None => (l, r),
        };
        if l != 0.0 || r != 0.0 {
            graph.add(self.track, frame, l, r);
        }
    }

    /// Sus efectos ya no suenan.
    fn quiet(&self) -> bool {
        self.fx.as_ref().is_none_or(FxRunner::idle)
    }
}

#[derive(Debug)]
struct AmbVoice {
    id: String,
    voice: Voice,
    pos: f64,
    gain: Ramp,
}

#[derive(Debug)]
struct Outgoing {
    voices: Vec<Voice>,
    t: f64,
    gain: Ramp,
}

/// Estado del mezclador en directo. Público para poder probarlo sin dispositivo.
#[derive(Debug)]
pub struct Live {
    rate: f64,
    graph: Graph,
    local: Vec<Voice>,
    /// Reloj de la imagen: `t0` más `n` muestras a `speed` (se cuenta en muestras en vez de
    /// sumar para que coincida exactamente con la exportación).
    t0: f64,
    n: u64,
    t: f64,
    playing: bool,
    running: bool,
    speed: f64,
    local_gain: Ramp,
    outgoing: Vec<Outgoing>,
    ambient: Vec<AmbVoice>,
    amb_gain: Ramp,
    master: f32,
    mute: Ramp,
    preview: Option<(Arc<Pcm>, f64)>,
    duck: Vec<f32>,
}

const CLICK: f64 = 0.012;

impl Live {
    pub fn new(rate: u32) -> Live {
        Live {
            rate: rate as f64,
            graph: Graph::new(rate),
            local: Vec::new(),
            t0: 0.0,
            n: 0,
            t: 0.0,
            playing: false,
            running: false,
            speed: 1.0,
            local_gain: Ramp::new(0.0),
            outgoing: Vec::new(),
            ambient: Vec::new(),
            amb_gain: Ramp::new(0.0),
            master: 1.0,
            mute: Ramp::new(1.0),
            preview: None,
            duck: Vec::new(),
        }
    }

    /// Tiempo de la imagen según el motor.
    pub fn time(&self) -> f64 {
        self.t
    }

    fn voice(&self, clip: Clip) -> Voice {
        Voice { track: self.graph.resolve(&clip.track, clip.bus), fx: FxRunner::new(&clip.effects, self.rate as u32), clip }
    }

    fn voices(&self, clips: Vec<Clip>) -> Vec<Voice> {
        clips.into_iter().map(|c| self.voice(c)).collect()
    }

    /// Lo que sonaba se funde en `seconds` (sus efectos dejan sonar la cola).
    fn fade_out_local(&mut self, seconds: f64) {
        let old = std::mem::take(&mut self.local);
        if !old.is_empty() {
            let mut gain = Ramp::new(self.local_gain.v);
            gain.to(0.0, seconds, self.rate);
            self.outgoing.push(Outgoing { voices: old, t: self.t, gain });
        }
    }

    pub fn apply(&mut self, c: Command) {
        let r = self.rate;
        match c {
            Command::Scene(clips) => {
                self.fade_out_local(0.03);
                self.local = self.voices(clips);
                self.local_gain = Ramp::new(0.0);
                self.local_gain.to(if self.playing { 1.0 } else { 0.0 }, CLICK, r);
            }
            Command::Ambient(list) => {
                for v in &mut self.ambient {
                    if !list.iter().any(|(id, _)| *id == v.id) {
                        v.gain.to(0.0, v.voice.clip.fade_out.max(0.05), r);
                    }
                }
                for (id, clip) in list {
                    let i = self.ambient.iter().position(|v| v.id == id);
                    match i {
                        Some(i) => {
                            let track = self.graph.resolve(&clip.track, clip.bus);
                            let v = &mut self.ambient[i];
                            v.gain.to(clip.volume, clip.fade_in.max(0.05), r);
                            // Volumen, fundidos o efectos cambiados: sigue donde iba.
                            match v.voice.fx.as_mut() {
                                Some(fx) => fx.configure(&clip.effects, r as u32),
                                None => v.voice.fx = FxRunner::new(&clip.effects, r as u32),
                            }
                            v.voice.track = track;
                            v.voice.clip = clip;
                        }
                        None => {
                            let mut gain = Ramp::new(0.0);
                            gain.to(clip.volume, clip.fade_in.max(0.05), r);
                            let voice = self.voice(clip);
                            self.ambient.push(AmbVoice { id, voice, pos: 0.0, gain });
                        }
                    }
                }
            }
            Command::Transport { t, playing, running, rate, seek } => {
                // Pequeñas diferencias se toleran (cada reloj va a su ritmo); las grandes, se salta.
                if seek || (t - self.t).abs() > 0.05 {
                    if self.playing && playing && (t - self.t).abs() > 0.05 && !self.local.is_empty() {
                        // Salto en plena reproducción: lo de antes se funde para no chasquear.
                        let clips: Vec<Clip> = self.local.iter().map(|v| v.clip.clone()).collect();
                        self.fade_out_local(0.02);
                        self.local = self.voices(clips);
                        self.local_gain = Ramp::new(0.0);
                    }
                    self.t = t;
                }
                self.playing = playing;
                self.running = running;
                self.speed = rate.clamp(0.25, 4.0);
                self.t0 = self.t;
                self.n = 0;
                self.local_gain.to(if playing { 1.0 } else { 0.0 }, CLICK, r);
                self.amb_gain.to(if running { 1.0 } else { 0.0 }, 0.08, r);
            }
            Command::Tracks { tracks, solo } => {
                self.graph.set_tracks(&tracks, &solo);
                let g = &self.graph;
                let all = self.local.iter_mut().chain(self.outgoing.iter_mut().flat_map(|o| o.voices.iter_mut())).chain(self.ambient.iter_mut().map(|a| &mut a.voice));
                for v in all {
                    v.track = g.resolve(&v.clip.track, v.clip.bus);
                }
            }
            Command::Levels(l) => self.graph.levels = l,
            Command::Master(m) => self.master = m.clamp(0.0, 2.0),
            Command::Mute(m) => self.mute.to(if m { 0.0 } else { 1.0 }, 0.05, r),
            Command::Preview(p) => self.preview = p.map(|p| (p, 0.0)),
            Command::Silence => {
                self.local.clear();
                self.outgoing.clear();
                self.ambient.clear();
                self.graph.reset();
                self.playing = false;
                self.running = false;
                self.local_gain = Ramp::new(0.0);
                self.amb_gain = Ramp::new(0.0);
            }
        }
    }

    /// Mezcla `frames` muestras estéreo en `out` (intercalado).
    pub fn render(&mut self, out: &mut [f32]) {
        let dt = 1.0 / self.rate;
        let frames = out.len() / 2;
        let duck_on = self.graph.levels.duck;
        self.graph.begin(frames);
        self.duck.clear();
        for f in 0..frames {
            // Voz y efectos de la imagen.
            let lg = self.local_gain.tick();
            for v in &mut self.local {
                let (l, r) = if lg > 0.0 { v.clip.sample(self.t) } else { (0.0, 0.0) };
                v.feed(&mut self.graph, f, l * lg, r * lg);
            }
            // Lo de la imagen anterior, fundiéndose.
            for o in &mut self.outgoing {
                let g = o.gain.tick();
                for v in &mut o.voices {
                    let (l, r) = if g > 0.0 { v.clip.sample(o.t) } else { (0.0, 0.0) };
                    v.feed(&mut self.graph, f, l * g, r * g);
                }
                if self.playing {
                    o.t += dt * self.speed;
                }
            }
            // Ambiente: sigue su propio reloj, a velocidad normal.
            let ag = self.amb_gain.tick();
            for a in &mut self.ambient {
                let g = a.gain.tick() * ag;
                let (l, r) = if g > 0.0 { a.voice.clip.raw(a.pos) } else { (0.0, 0.0) };
                a.voice.feed(&mut self.graph, f, l * g, r * g);
                if self.running {
                    a.pos += dt;
                }
            }
            let graph = &self.graph;
            let duck = if duck_on {
                duck_gain(duck_amount(self.local.iter().filter(|v| v.clip.bus == Bus::Voice && graph.audible(v.track)).map(|v| &v.clip), self.t))
            } else {
                1.0
            };
            self.duck.push(duck);
            if self.playing {
                self.n += 1;
                self.t = self.t0 + self.n as f64 / self.rate * self.speed;
            }
        }
        self.graph.mix(&self.duck, out);
        for frame in out.as_chunks_mut::<2>().0 {
            let m = self.master * self.mute.tick();
            let mut l = soft_clip(frame[0] * m);
            let mut r = soft_clip(frame[1] * m);
            if let Some((p, pos)) = &mut self.preview {
                let (a, b) = p.frame((*pos * p.rate as f64) as isize);
                l = soft_clip(l + a * self.master);
                r = soft_clip(r + b * self.master);
                *pos += dt;
                if *pos >= p.seconds() {
                    self.preview = None;
                }
            }
            frame[0] = l;
            frame[1] = r;
        }
        self.outgoing.retain(|o| !o.gain.silent() || o.voices.iter().any(|v| !v.quiet()));
        self.ambient.retain(|a| !a.gain.silent() || !a.voice.quiet());
    }
}

/// Dispositivo de entrada o salida.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

fn device_name(d: &cpal::Device) -> String {
    d.description().map(|x| x.name().to_owned()).unwrap_or_else(|_| "?".into())
}

/// Micrófonos disponibles. En Linux, ALSA lista también sus complementos internos
/// (conversores, «null»…): solo se dejan los servidores de sonido y las tarjetas.
pub fn input_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let Ok(list) = host.input_devices() else { return Vec::new() };
    list.filter_map(|d| Some(DeviceInfo { id: d.id().ok()?.to_string(), name: device_name(&d) }))
        .filter(|d| useful_input(&d.id))
        .collect()
}

fn useful_input(id: &str) -> bool {
    match id.strip_prefix("alsa:") {
        Some(rest) => rest == "default" || rest == "pipewire" || rest == "pulse" || rest.starts_with("sysdefault:"),
        None => true,
    }
}

/// Salida de sonido en marcha. Al soltarla se cierra el dispositivo.
pub struct Output {
    tx: Sender<Command>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub rate: u32,
    pub device: String,
}

impl Output {
    /// Abre la salida por defecto.
    pub fn start() -> Result<Output, AudioError> {
        let (tx, rx) = mpsc::channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(u32, String), AudioError>>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        // El stream de cpal no siempre se puede mover entre hilos: vive en el suyo.
        let thread = std::thread::Builder::new()
            .name("imgx-audio".into())
            .spawn(move || {
                let res = open_output(rx);
                match res {
                    Ok((stream, rate, name)) => {
                        let _ = ready_tx.send(Ok((rate, name)));
                        while !stop2.load(Ordering::Relaxed) {
                            std::thread::park();
                        }
                        drop(stream);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            })
            .map_err(AudioError::Io)?;
        let (rate, device) = ready_rx.recv().map_err(|_| AudioError::Device("el hilo de audio terminó".into()))??;
        Ok(Output { tx, stop, thread: Some(thread), rate, device })
    }

    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

fn open_output(rx: Receiver<Command>) -> Result<(cpal::Stream, u32, String), AudioError> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or_else(|| AudioError::Device("no hay salida de sonido".into()))?;
    let name = device_name(&device);
    let supported = device.default_output_config().map_err(|e| AudioError::Device(e.to_string()))?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate;
    let channels = config.channels as usize;
    let live = Live::new(rate);
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, channels, live, rx),
        SampleFormat::I16 => build::<i16>(&device, &config, channels, live, rx),
        SampleFormat::U16 => build::<u16>(&device, &config, channels, live, rx),
        SampleFormat::I32 => build::<i32>(&device, &config, channels, live, rx),
        f => return Err(AudioError::Device(format!("formato de salida no admitido: {f}"))),
    }?;
    stream.play().map_err(|e| AudioError::Device(e.to_string()))?;
    log::info!("audio: salida «{name}» a {rate} Hz, {channels} canales, {format}");
    Ok((stream, rate, name))
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    mut live: Live,
    rx: Receiver<Command>,
) -> Result<cpal::Stream, AudioError> {
    let mut buf: Vec<f32> = Vec::new();
    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                while let Ok(c) = rx.try_recv() {
                    live.apply(c);
                }
                let frames = data.len() / channels.max(1);
                buf.resize(frames * 2, 0.0);
                live.render(&mut buf);
                for (f, out) in data.chunks_mut(channels.max(1)).enumerate() {
                    let (l, r) = (buf[f * 2], buf[f * 2 + 1]);
                    match out.len() {
                        1 => out[0] = T::from_sample((l + r) * 0.5),
                        _ => {
                            out[0] = T::from_sample(l);
                            out[1] = T::from_sample(r);
                            for s in &mut out[2..] {
                                *s = T::from_sample(0.0);
                            }
                        }
                    }
                }
            },
            |e| log::warn!("audio: error de salida: {e}"),
            None,
        )
        .map_err(|e| AudioError::Device(e.to_string()))
}

/// Grabación del micrófono en curso.
pub struct Recorder {
    samples: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    rate: u32,
    channels: usize,
}

impl Recorder {
    /// Empieza a grabar del micrófono `device` (id de `input_devices`) o del predeterminado.
    pub fn start(device: Option<&str>) -> Result<Recorder, AudioError> {
        let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
        let level = Arc::new(AtomicU32::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (s2, l2, st2) = (samples.clone(), level.clone(), stop.clone());
        let wanted = device.map(str::to_owned);
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(u32, usize), AudioError>>();
        let thread = std::thread::Builder::new()
            .name("imgx-mic".into())
            .spawn(move || match open_input(wanted.as_deref(), s2, l2) {
                Ok((stream, rate, ch)) => {
                    let _ = ready_tx.send(Ok((rate, ch)));
                    while !st2.load(Ordering::Relaxed) {
                        std::thread::park();
                    }
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            })
            .map_err(AudioError::Io)?;
        let (rate, channels) = ready_rx.recv().map_err(|_| AudioError::Device("el hilo del micrófono terminó".into()))??;
        Ok(Recorder { samples, level, stop, thread: Some(thread), rate, channels })
    }

    /// Nivel de pico desde la última consulta (0..1).
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.swap(0, Ordering::Relaxed))
    }

    pub fn seconds(&self) -> f64 {
        self.samples.lock().map(|s| s.len()).unwrap_or(0) as f64 / (self.rate as f64 * self.channels.max(1) as f64)
    }

    /// Para y devuelve lo grabado.
    pub fn finish(mut self) -> Pcm {
        self.halt();
        let data = std::mem::take(&mut *self.samples.lock().unwrap());
        Pcm::from_interleaved(self.rate, self.channels, &data)
    }

    fn halt(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.halt();
    }
}

fn open_input(wanted: Option<&str>, samples: Arc<Mutex<Vec<f32>>>, level: Arc<AtomicU32>) -> Result<(cpal::Stream, u32, usize), AudioError> {
    let host = cpal::default_host();
    let device = wanted
        .and_then(|id| id.parse().ok())
        .and_then(|id| host.device_by_id(&id))
        .or_else(|| host.default_input_device())
        .ok_or_else(|| AudioError::Device("no hay micrófono".into()))?;
    let name = device_name(&device);
    let supported = device.default_input_config().map_err(|e| AudioError::Device(e.to_string()))?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let (rate, ch) = (config.sample_rate, config.channels as usize);
    let stream = match format {
        SampleFormat::F32 => build_in::<f32>(&device, &config, samples, level),
        SampleFormat::I16 => build_in::<i16>(&device, &config, samples, level),
        SampleFormat::U16 => build_in::<u16>(&device, &config, samples, level),
        SampleFormat::I32 => build_in::<i32>(&device, &config, samples, level),
        f => return Err(AudioError::Device(format!("formato de micrófono no admitido: {f}"))),
    }?;
    stream.play().map_err(|e| AudioError::Device(e.to_string()))?;
    log::info!("audio: grabando de «{name}» a {rate} Hz, {ch} canales");
    Ok((stream, rate, ch))
}

fn build_in<T: SizedSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
) -> Result<cpal::Stream, AudioError>
where
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            *config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let mut peak = f32::from_bits(level.load(Ordering::Relaxed));
                if let Ok(mut s) = samples.lock() {
                    s.reserve(data.len());
                    for &x in data {
                        let v: f32 = x.to_sample();
                        peak = peak.max(v.abs());
                        s.push(v);
                    }
                }
                level.store(peak.to_bits(), Ordering::Relaxed);
            },
            |e| log::warn!("audio: error del micrófono: {e}"),
            None,
        )
        .map_err(|e| AudioError::Device(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dc(v: f32, seconds: f64, rate: u32) -> Arc<Pcm> {
        Arc::new(Pcm { rate, data: vec![v; (seconds * rate as f64) as usize * 2] })
    }

    fn clip(sound: Arc<Pcm>, bus: Bus, start: f64, duration: f64) -> Clip {
        Clip::new(sound, bus, start, duration)
    }

    fn run(live: &mut Live, seconds: f64) -> Vec<f32> {
        let mut out = vec![0f32; (seconds * live.rate) as usize * 2];
        live.render(&mut out);
        out
    }

    #[test]
    fn reloj_y_pausa() {
        let mut live = Live::new(1000);
        live.apply(Command::Scene(vec![clip(dc(0.5, 10.0, 1000), Bus::Sfx, 1.0, 5.0)]));
        live.apply(Command::Transport { t: 0.0, playing: true, running: true, rate: 1.0, seek: true });
        let a = run(&mut live, 0.5);
        assert!(a.iter().all(|v| *v == 0.0), "antes del clip, silencio");
        let b = run(&mut live, 1.0);
        assert!((b.last().unwrap() - 0.5).abs() < 1e-3, "dentro del clip suena");
        assert!((live.time() - 1.5).abs() < 1e-6);
        live.apply(Command::Transport { t: 1.5, playing: false, running: false, rate: 1.0, seek: false });
        let c = run(&mut live, 0.1);
        assert!(c.last().unwrap().abs() < 1e-6, "en pausa se calla (tras el fundido corto)");
        assert!((live.time() - 1.5).abs() < 1e-6, "y el reloj no avanza");
    }

    #[test]
    fn velocidad_y_correccion_del_reloj() {
        let mut live = Live::new(1000);
        live.apply(Command::Transport { t: 0.0, playing: true, running: true, rate: 2.0, seek: true });
        run(&mut live, 1.0);
        assert!((live.time() - 2.0).abs() < 1e-6, "a 2× el reloj va al doble");
        // Desfase pequeño: se tolera; grande: se salta.
        live.apply(Command::Transport { t: 2.03, playing: true, running: true, rate: 1.0, seek: false });
        assert!((live.time() - 2.0).abs() < 1e-6);
        live.apply(Command::Transport { t: 3.0, playing: true, running: true, rate: 1.0, seek: false });
        assert!((live.time() - 3.0).abs() < 1e-6);
    }

    #[test]
    fn el_ambiente_sigue_al_cambiar_de_imagen_y_se_funde_al_quitarlo() {
        let mut live = Live::new(1000);
        live.apply(Command::Levels(Levels { ambient: 1.0, duck: false, ..Default::default() }));
        let amb = Clip { looped: true, fade_in: 0.1, fade_out: 0.2, ..clip(dc(0.4, 1.0, 1000), Bus::Ambient, 0.0, f64::MAX) };
        live.apply(Command::Ambient(vec![("lluvia".into(), amb.clone())]));
        live.apply(Command::Transport { t: 0.0, playing: true, running: true, rate: 1.0, seek: true });
        let a = run(&mut live, 0.5);
        assert!((a.last().unwrap() - 0.4).abs() < 1e-3);
        // Nueva imagen: el ambiente no se reinicia ni baja.
        live.apply(Command::Scene(vec![]));
        live.apply(Command::Ambient(vec![("lluvia".into(), amb)]));
        live.apply(Command::Transport { t: 0.0, playing: true, running: true, rate: 1.0, seek: true });
        let b = run(&mut live, 0.05);
        assert!(b.chunks(2).all(|f| (f[0] - 0.4).abs() < 1e-3), "sin cortes al cambiar de imagen");
        // Se quita: se funde en 0,2 s y desaparece.
        live.apply(Command::Ambient(vec![]));
        let c = run(&mut live, 0.1);
        let mid = c[c.len() - 2];
        assert!(mid > 0.1 && mid < 0.3, "a mitad del fundido: {mid}");
        run(&mut live, 0.2);
        assert!(live.ambient.is_empty());
    }

    #[test]
    fn al_mover_la_camara_la_voz_se_congela_y_el_ambiente_sigue() {
        let mut live = Live::new(1000);
        live.apply(Command::Levels(Levels { ambient: 1.0, duck: false, ..Default::default() }));
        live.apply(Command::Scene(vec![clip(dc(0.3, 10.0, 1000), Bus::Voice, 0.0, 10.0)]));
        let amb = Clip { looped: true, ..clip(dc(0.2, 1.0, 1000), Bus::Ambient, 0.0, f64::MAX) };
        live.apply(Command::Ambient(vec![("a".into(), amb)]));
        live.apply(Command::Transport { t: 0.0, playing: false, running: true, rate: 1.0, seek: true });
        let out = run(&mut live, 0.3);
        assert!((out.last().unwrap() - 0.2).abs() < 1e-3, "solo el ambiente");
        assert_eq!(live.time(), 0.0);
    }

    #[test]
    fn el_directo_suena_igual_que_la_exportacion() {
        use crate::fx::{FxKind, FxSpec};
        let rate = 8000u32;
        // Un tono y un ruido determinista, con efectos de clip y de pista.
        let tone = Arc::new(Pcm { rate, data: (0..rate as usize).flat_map(|i| { let v = 0.4 * (i as f32 * 0.07).sin(); [v, v * 0.8] }).collect() });
        let noise = Arc::new(Pcm { rate, data: (0..rate as usize / 2).flat_map(|i| { let v = ((i * 7919 % 101) as f32 / 101.0 - 0.5) * 0.6; [v, -v] }).collect() });
        let voz = Clip {
            track: "ana".into(),
            fade_in: 0.05,
            fade_out: 0.1,
            effects: vec![FxSpec::new(FxKind::Pitch, &[("semitones", 4.0)])],
            ..clip(tone, Bus::Voice, 0.1, 0.8)
        };
        let golpe = Clip { effects: vec![FxSpec::new(FxKind::Delay, &[("time", 120.0)])], volume: 0.7, ..clip(noise, Bus::Sfx, 0.3, 0.4) };
        let tracks = vec![
            TrackSpec { id: "ana".into(), pan: 0.3, volume: 0.9, effects: vec![FxSpec::new(FxKind::Reverb, &[("size", 0.7)]), FxSpec::new(FxKind::Compressor, &[])], ..TrackSpec::default_for(Bus::Voice) },
            TrackSpec { effects: vec![FxSpec::new(FxKind::Eq, &[("high", -6.0)])], ..TrackSpec::default_for(Bus::Sfx) },
        ];
        let lv = Levels { voice: 1.0, sfx: 0.8, ambient: 1.0, duck: true };
        let clips = vec![voz, golpe];
        let export = crate::mixer::render(&clips, &tracks, lv, rate, 2.0);

        let mut live = Live::new(rate);
        live.apply(Command::Levels(lv));
        live.apply(Command::Tracks { tracks, solo: vec![] });
        live.apply(Command::Scene(clips));
        live.apply(Command::Transport { t: 0.0, playing: true, running: true, rate: 1.0, seek: true });
        // En bloques de tamaños distintos, como los que pide la tarjeta de sonido.
        let mut got = Vec::new();
        for n in [64usize, 100, 256, 37, 1000, 512].iter().cycle() {
            if got.len() >= export.len() {
                break;
            }
            let mut b = vec![0f32; n * 2];
            live.render(&mut b);
            got.extend(b);
        }
        got.truncate(export.len());
        let worst = got.iter().zip(&export).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        assert!(worst < 1e-4, "diferencia máxima {worst} (−80 dB es inaudible)");
        assert!(export.iter().any(|x| x.abs() > 0.1), "y suena algo");
    }

    #[test]
    fn la_reverb_de_la_pista_sigue_al_cambiar_de_imagen() {
        use crate::fx::{FxKind, FxSpec};
        let mut live = Live::new(8000);
        let tracks = vec![TrackSpec { effects: vec![FxSpec::new(FxKind::Reverb, &[("size", 0.9), ("mix", 1.0)])], ..TrackSpec::default_for(Bus::Sfx) }];
        live.apply(Command::Tracks { tracks, solo: vec![] });
        live.apply(Command::Scene(vec![clip(dc(0.5, 0.2, 8000), Bus::Sfx, 0.0, 0.2)]));
        live.apply(Command::Transport { t: 0.0, playing: true, running: true, rate: 1.0, seek: true });
        run(&mut live, 0.3);
        // Imagen nueva sin sonido: la cola de la reverb sigue sonando.
        live.apply(Command::Scene(vec![]));
        live.apply(Command::Transport { t: 0.0, playing: true, running: true, rate: 1.0, seek: true });
        let out = run(&mut live, 0.5);
        assert!(out.iter().any(|x| x.abs() > 0.01), "la cola sigue");
    }

    #[test]
    fn solo() {
        let mut live = Live::new(1000);
        let a = Clip { track: "a".into(), ..clip(dc(0.2, 10.0, 1000), Bus::Sfx, 0.0, 10.0) };
        let b = Clip { track: "b".into(), ..clip(dc(0.3, 10.0, 1000), Bus::Sfx, 0.0, 10.0) };
        let tracks = vec![TrackSpec { id: "a".into(), ..TrackSpec::default_for(Bus::Sfx) }, TrackSpec { id: "b".into(), ..TrackSpec::default_for(Bus::Sfx) }];
        live.apply(Command::Tracks { tracks: tracks.clone(), solo: vec!["b".into()] });
        live.apply(Command::Scene(vec![a, b]));
        live.apply(Command::Transport { t: 0.0, playing: true, running: true, rate: 1.0, seek: true });
        let out = run(&mut live, 0.1);
        assert!((out.last().unwrap() - 0.3).abs() < 1e-3, "solo suena b");
        live.apply(Command::Tracks { tracks, solo: vec![] });
        let out = run(&mut live, 0.1);
        assert!((out.last().unwrap() - 0.5).abs() < 1e-3, "sin solo, las dos");
    }

    #[test]
    fn microfonos_utiles() {
        assert!(useful_input("alsa:default") && useful_input("alsa:pipewire") && useful_input("alsa:sysdefault:CARD=PCH"));
        assert!(!useful_input("alsa:null") && !useful_input("alsa:speexrate") && !useful_input("alsa:hw:CARD=0,DEV=0"));
        assert!(useful_input("coreaudio:BuiltInMicrophoneDevice"));
    }

    #[test]
    fn escuchar_un_sonido_de_la_biblioteca() {
        let mut live = Live::new(1000);
        live.apply(Command::Preview(Some(dc(0.6, 0.2, 1000))));
        let a = run(&mut live, 0.1);
        assert!((a.last().unwrap() - 0.6).abs() < 1e-3);
        run(&mut live, 0.2);
        assert!(live.preview.is_none(), "acaba solo");
    }

    #[test]
    fn cambio_de_imagen_sin_chasquido() {
        let mut live = Live::new(1000);
        live.apply(Command::Scene(vec![clip(dc(0.8, 10.0, 1000), Bus::Sfx, 0.0, 10.0)]));
        live.apply(Command::Transport { t: 0.0, playing: true, running: true, rate: 1.0, seek: true });
        run(&mut live, 0.2);
        live.apply(Command::Scene(vec![]));
        let out = run(&mut live, 0.05);
        let first = out[0];
        assert!(first > 0.5, "lo anterior se funde, no se corta de golpe: {first}");
        assert!(out[out.len() - 2].abs() < 1e-3);
    }
}

/// Abre de verdad la salida y el micrófono (sin sonar ni guardar nada). Necesita un equipo con
/// sonido: `cargo test -p imgx-audio dispositivos_reales -- --ignored --nocapture`.
#[cfg(test)]
mod real {
    use super::*;

    #[test]
    #[ignore]
    fn dispositivos_reales() {
        let out = Output::start().expect("salida");
        println!("salida: {} a {} Hz", out.device, out.rate);
        out.send(Command::Silence);
        drop(out);
        println!("micrófonos: {:?}", input_devices());
        match Recorder::start(None) {
            Ok(r) => {
                std::thread::sleep(std::time::Duration::from_millis(300));
                let level = r.level();
                let pcm = r.finish();
                println!("micrófono: {:.2} s grabados a {} Hz, nivel {level:.3}", pcm.seconds(), pcm.rate);
            }
            Err(e) => println!("micrófono no disponible: {e}"),
        }
    }
}

#[cfg(test)]
mod perf {
    use super::*;
    use crate::fx::{FxKind, FxSpec};

    /// Cuánto cuesta mezclar 10 s con 6 pistas cargadas de efectos (no comprueba nada; informa).
    #[test]
    #[ignore]
    fn coste_de_la_mezcla() {
        let rate = 48_000;
        let tone = Arc::new(Pcm { rate, data: (0..rate as usize * 10).flat_map(|i| { let v = 0.3 * (i as f32 * 0.03).sin(); [v, v] }).collect() });
        let heavy = vec![
            FxSpec::new(FxKind::Reverb, &[("size", 0.9)]),
            FxSpec::new(FxKind::Delay, &[]),
            FxSpec::new(FxKind::Compressor, &[]),
            FxSpec::new(FxKind::Eq, &[("low", 3.0)]),
        ];
        let mut tracks = Vec::new();
        let mut clips = Vec::new();
        for (i, bus) in [Bus::Voice, Bus::Voice, Bus::Sfx, Bus::Sfx, Bus::Ambient, Bus::Ambient].into_iter().enumerate() {
            let id = format!("t{i}");
            tracks.push(TrackSpec { id: id.clone(), effects: heavy.clone(), ..TrackSpec::default_for(bus) });
            if bus != Bus::Ambient {
                clips.push(Clip { track: id, effects: vec![FxSpec::new(FxKind::Pitch, &[("semitones", 3.0)])], ..Clip::new(tone.clone(), bus, 0.0, 10.0) });
            }
        }
        let mut live = Live::new(rate);
        live.apply(Command::Tracks { tracks, solo: vec![] });
        live.apply(Command::Scene(clips));
        live.apply(Command::Transport { t: 0.0, playing: true, running: true, rate: 1.0, seek: true });
        let t0 = std::time::Instant::now();
        let mut buf = vec![0f32; 512 * 2];
        for _ in 0..(rate as usize * 10 / 512) {
            live.render(&mut buf);
        }
        let el = t0.elapsed().as_secs_f64();
        println!("10 s de mezcla en {el:.2} s → {:.0} % de un núcleo", el / 10.0 * 100.0);
    }
}
