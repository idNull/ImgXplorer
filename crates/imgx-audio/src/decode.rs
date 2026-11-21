//! Decodificación de archivos de audio (mp3, wav, flac, ogg/vorbis, m4a/aac) a PCM estéreo.
use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::AudioError;

/// Extensiones que se pueden abrir.
pub const EXTENSIONS: &[&str] = &["mp3", "wav", "wave", "flac", "ogg", "oga", "m4a", "aac", "mp4"];

/// Audio estéreo en `f32` intercalado (izquierdo, derecho, izquierdo…), a su frecuencia original.
#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    pub rate: u32,
    pub data: Vec<f32>,
}

impl Pcm {
    /// Convierte muestras intercaladas de `channels` canales a estéreo (mono se duplica; con más
    /// de dos canales se toman los dos primeros).
    pub fn from_interleaved(rate: u32, channels: usize, samples: &[f32]) -> Pcm {
        let ch = channels.max(1);
        let mut data = Vec::with_capacity(samples.len() / ch * 2);
        for f in samples.chunks_exact(ch) {
            let (l, r) = if ch == 1 { (f[0], f[0]) } else { (f[0], f[1]) };
            data.push(l);
            data.push(r);
        }
        Pcm { rate: rate.max(1), data }
    }

    pub fn frames(&self) -> usize {
        self.data.len() / 2
    }

    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.rate as f64
    }

    /// Bytes que ocupa en memoria (para la caché).
    pub fn bytes(&self) -> usize {
        self.data.len() * 4
    }

    /// Muestra estéreo `i` (silencio fuera de rango).
    #[inline]
    pub fn frame(&self, i: isize) -> (f32, f32) {
        if i < 0 {
            return (0.0, 0.0);
        }
        let i = i as usize * 2;
        match self.data.get(i..i + 2) {
            Some(s) => (s[0], s[1]),
            None => (0.0, 0.0),
        }
    }

    /// Pico (valor absoluto máximo) de cada uno de `buckets` tramos iguales: la forma de onda.
    pub fn peaks(&self, buckets: usize) -> Vec<f32> {
        let n = self.frames();
        if n == 0 || buckets == 0 {
            return vec![0.0; buckets];
        }
        (0..buckets)
            .map(|b| {
                let a = b * n / buckets;
                let z = ((b + 1) * n / buckets).max(a + 1).min(n);
                self.data[a * 2..z * 2].iter().fold(0f32, |m, v| m.max(v.abs()))
            })
            .collect()
    }
}

/// Duración en segundos leyendo solo la cabecera (sin decodificar). `None` si el formato no la
/// declara o no se reconoce.
pub fn probe_seconds(path: &Path) -> Option<f64> {
    let file = File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(&ext.to_lowercase());
    }
    let format = symphonia::default::get_probe().probe(&hint, mss, FormatOptions::default(), MetadataOptions::default()).ok()?;
    let track = format.default_track(TrackType::Audio)?;
    let secs = track.time_base.zip(track.duration).and_then(|(tb, d)| tb.calc_duration(d)).map(|t| t.as_secs_f64());
    secs.filter(|s| s.is_finite() && *s > 0.0)
}

/// Decodifica un archivo completo.
pub fn decode_file(path: &Path) -> Result<Pcm, AudioError> {
    let file = File::open(path)?;
    let ext = path.extension().and_then(|e| e.to_str());
    decode_stream(MediaSourceStream::new(Box::new(file), Default::default()), ext, &path.display().to_string())
}

/// Decodifica un sonido que ya está en memoria (p. ej. dentro de una historia `.imgxstory`).
/// `ext` es la extensión de su archivo, como pista del formato.
pub fn decode_bytes(bytes: Vec<u8>, ext: Option<&str>) -> Result<Pcm, AudioError> {
    decode_stream(MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), Default::default()), ext, "memoria")
}

fn decode_stream(mss: MediaSourceStream, ext: Option<&str>, what: &str) -> Result<Pcm, AudioError> {
    let mut hint = Hint::new();
    if let Some(ext) = ext {
        hint.with_extension(&ext.to_lowercase());
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| AudioError::Unsupported(e.to_string()))?;
    let track = format.default_track(TrackType::Audio).ok_or_else(|| AudioError::Unsupported("sin pista de audio".into()))?;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| AudioError::Unsupported("pista sin parámetros de audio".into()))?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|e| AudioError::Unsupported(e.to_string()))?;
    let track_id = track.id;

    let mut rate = 0u32;
    let mut channels = 0usize;
    let mut all: Vec<f32> = Vec::new();
    let mut buf: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            // Algunos archivos acaban con basura: lo leído hasta ahí vale.
            Err(SymError::IoError(_)) => break,
            Err(e) => return Err(AudioError::Decode(e.to_string())),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(audio) => {
                let spec = audio.spec();
                let ch = spec.channels().count();
                if rate == 0 {
                    rate = spec.rate();
                    channels = ch;
                }
                if ch != channels || spec.rate() != rate {
                    continue; // cambio de formato a mitad: raro, se ignora ese paquete
                }
                buf.resize(audio.samples_interleaved(), 0.0);
                audio.copy_to_slice_interleaved(&mut buf);
                all.extend_from_slice(&buf);
            }
            Err(SymError::DecodeError(e)) => log::debug!("paquete dañado en {what}: {e}"),
            Err(e) => return Err(AudioError::Decode(e.to_string())),
        }
    }
    if rate == 0 || all.is_empty() {
        return Err(AudioError::Empty);
    }
    Ok(Pcm::from_interleaved(rate, channels, &all))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desde_memoria_igual_que_desde_archivo() {
        let path = std::env::temp_dir().join(format!("imgx-audio-mem-{}.wav", std::process::id()));
        crate::wav::write(&path, &crate::synth::sine(330.0, 0.2, 22050)).unwrap();
        let from_file = decode_file(&path).unwrap();
        let from_mem = decode_bytes(std::fs::read(&path).unwrap(), Some("wav")).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(from_file, from_mem);
        assert!(decode_bytes(b"no es audio".to_vec(), None).is_err());
    }

    #[test]
    fn mono_a_estereo_y_picos() {
        let p = Pcm::from_interleaved(10, 1, &[0.0, 0.5, -1.0, 0.25]);
        assert_eq!(p.data, vec![0.0, 0.0, 0.5, 0.5, -1.0, -1.0, 0.25, 0.25]);
        assert_eq!(p.frames(), 4);
        assert!((p.seconds() - 0.4).abs() < 1e-9);
        assert_eq!(p.peaks(2), vec![0.5, 1.0]);
        assert_eq!(p.frame(-1), (0.0, 0.0));
        assert_eq!(p.frame(9), (0.0, 0.0));
    }

    #[test]
    fn mas_de_dos_canales() {
        let p = Pcm::from_interleaved(8, 3, &[0.1, 0.2, 0.9, 0.3, 0.4, 0.9]);
        assert_eq!(p.data, vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn decodifica_wav() {
        let dir = std::env::temp_dir().join(format!("imgx-audio-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tono.wav");
        let tone = crate::synth::sine(440.0, 0.25, 22050);
        crate::wav::write(&path, &tone).unwrap();
        let p = decode_file(&path).unwrap();
        assert_eq!(p.rate, 22050);
        assert_eq!(p.frames(), tone.frames());
        assert!(p.data.iter().zip(&tone.data).all(|(a, b)| (a - b).abs() < 1e-3));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Con el ffmpeg del sistema se generan mp3, ogg, flac y m4a y se comprueba que se leen.
    #[test]
    fn decodifica_formatos_comprimidos_si_hay_ffmpeg() {
        if std::process::Command::new("ffmpeg").arg("-version").output().is_err() {
            eprintln!("sin ffmpeg: se omite");
            return;
        }
        let dir = std::env::temp_dir().join(format!("imgx-audio-fmt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.wav");
        crate::wav::write(&src, &crate::synth::sine(330.0, 1.0, 44100)).unwrap();
        for (ext, codec) in [("mp3", "libmp3lame"), ("ogg", "libvorbis"), ("flac", "flac"), ("m4a", "aac")] {
            let out = dir.join(format!("x.{ext}"));
            let ok = std::process::Command::new("ffmpeg")
                .args(["-v", "error", "-y", "-i"])
                .arg(&src)
                .args(["-c:a", codec])
                .arg(&out)
                .status()
                .is_ok_and(|s| s.success());
            if !ok {
                eprintln!("ffmpeg no tiene {codec}: se omite {ext}");
                continue;
            }
            let p = decode_file(&out).unwrap_or_else(|e| panic!("{ext}: {e}"));
            assert!((p.seconds() - 1.0).abs() < 0.1, "{ext}: {} s", p.seconds());
            let peak = p.peaks(1)[0];
            assert!(peak > 0.3 && peak < 1.01, "{ext}: pico {peak}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
