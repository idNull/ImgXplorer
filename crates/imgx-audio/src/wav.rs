//! Escribir PCM estéreo como WAV de 16 bits (grabaciones y mezcla para ffmpeg).
use std::path::Path;

use crate::{AudioError, decode::Pcm};

pub fn write(path: &Path, pcm: &Pcm) -> Result<(), AudioError> {
    write_samples(path, pcm.rate, &pcm.data)
}

/// Escribe muestras estéreo intercaladas.
pub fn write_samples(path: &Path, rate: u32, data: &[f32]) -> Result<(), AudioError> {
    let spec = hound::WavSpec { channels: 2, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| AudioError::Decode(e.to_string()))?;
    for &s in data {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).map_err(|e| AudioError::Decode(e.to_string()))?;
    }
    w.finalize().map_err(|e| AudioError::Decode(e.to_string()))
}
