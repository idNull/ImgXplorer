//! Sonido de imgxplorer: decodificar archivos, mezclar (en directo y para exportar), efectos de
//! serie por síntesis, salida al altavoz y grabación del micrófono.
pub mod decode;
pub mod device;
pub mod fx;
pub mod mixer;
pub mod synth;
pub mod wav;

pub use decode::{EXTENSIONS, Pcm, decode_bytes, decode_file};
pub use mixer::{Bus, Clip, Levels, TrackSpec};

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("formato no admitido: {0}")]
    Unsupported(String),
    #[error("no se pudo decodificar: {0}")]
    Decode(String),
    #[error("el archivo no tiene sonido")]
    Empty,
    #[error("dispositivo de sonido: {0}")]
    Device(String),
}
