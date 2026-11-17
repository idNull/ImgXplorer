//! Paquetes `.imgxpack` para compartir clips, plantillas y sonidos propios: un zip con
//! `manifest.json` (formato y versión), `library.json` (lo define la interfaz, que lo valida al
//! importar) y, si los hay, `sounds.json` con los archivos de sonido en `sounds/`.

use crate::audio::{Audio, SoundEntry, SoundInfo};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

pub const FORMAT: &str = "imgxpack";
pub const VERSION: u32 = 1;
const LIBRARY: &str = "library.json";
/// Un paquete de clips es pequeño; esto evita leer un zip malicioso enorme.
const MAX_LIBRARY: u64 = 32 * 1024 * 1024;
const SOUNDS: &str = "sounds.json";
/// Tamaño máximo de un sonido dentro de un paquete.
const MAX_SOUND: u64 = 1024 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    /// Versión de imgxplorer que lo creó (informativo).
    #[serde(default)]
    app: String,
}

fn write(path: &Path, library: &str, sounds: &[(SoundEntry, PathBuf)]) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(library).map_err(|e| e.to_string())?;
    let manifest = Manifest { format: FORMAT.into(), version: VERSION, app: env!("CARGO_PKG_VERSION").into() };
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    let result = (|| -> zip::result::ZipResult<()> {
        let mut z = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        z.start_file("manifest.json", opts)?;
        z.write_all(serde_json::to_string_pretty(&manifest).expect("manifiesto serializable").as_bytes())?;
        z.start_file(LIBRARY, opts)?;
        z.write_all(library.as_bytes())?;
        if !sounds.is_empty() {
            let list: Vec<&SoundEntry> = sounds.iter().map(|(e, _)| e).collect();
            z.start_file(SOUNDS, opts)?;
            z.write_all(serde_json::to_string_pretty(&list).expect("lista serializable").as_bytes())?;
            // El audio ya va comprimido (mp3, ogg…): se guarda tal cual.
            let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(true);
            for (e, file) in sounds {
                z.start_file(format!("sounds/{}", e.file), stored)?;
                std::io::copy(&mut std::fs::File::open(file)?, &mut z)?;
            }
        }
        z.finish()?;
        Ok(())
    })();
    result.and_then(|()| Ok(std::fs::rename(&tmp, path)?)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Un sonido dentro del paquete.
#[derive(Debug)]
struct PackedSound {
    entry: SoundEntry,
    bytes: Vec<u8>,
}

fn read(path: &Path) -> Result<(String, Vec<PackedSound>), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(file).map_err(|_| "no es un paquete .imgxpack (zip)".to_string())?;
    let text = |z: &mut zip::ZipArchive<std::fs::File>, name: &str| -> Result<String, String> {
        let entry = z.by_name(name).map_err(|_| format!("al paquete le falta {name}"))?;
        if entry.size() > MAX_LIBRARY {
            return Err(format!("{name} es demasiado grande"));
        }
        let mut s = String::new();
        entry.take(MAX_LIBRARY).read_to_string(&mut s).map_err(|e| e.to_string())?;
        Ok(s)
    };
    let manifest: Manifest = serde_json::from_str(&text(&mut z, "manifest.json")?).map_err(|e| format!("manifiesto inválido: {e}"))?;
    if manifest.format != FORMAT {
        return Err("no es un paquete .imgxpack".into());
    }
    if manifest.version > VERSION {
        return Err(format!("paquete de una versión más nueva ({}); actualiza imgxplorer", manifest.version));
    }
    let library = text(&mut z, LIBRARY)?;
    serde_json::from_str::<serde_json::Value>(&library).map_err(|e| format!("{LIBRARY} inválido: {e}"))?;
    let mut sounds = Vec::new();
    if z.by_name(SOUNDS).is_ok() {
        let list: Vec<SoundEntry> = serde_json::from_str(&text(&mut z, SOUNDS)?).map_err(|e| format!("{SOUNDS} inválido: {e}"))?;
        for entry in list {
            // Solo el nombre del archivo: nada de rutas que salgan de `sounds/`.
            if entry.file.contains(['/', '\\']) || entry.file.starts_with('.') {
                return Err(format!("nombre de sonido no válido: {}", entry.file));
            }
            let mut f = z.by_name(&format!("sounds/{}", entry.file)).map_err(|_| format!("al paquete le falta el sonido {}", entry.file))?;
            if f.size() > MAX_SOUND {
                return Err(format!("{} es demasiado grande", entry.file));
            }
            let mut bytes = Vec::with_capacity(f.size() as usize);
            (&mut f).take(MAX_SOUND).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            sounds.push(PackedSound { entry, bytes });
        }
    }
    Ok((library, sounds))
}

/// Exporta la biblioteca y, con `sounds`, también los sonidos propios. Devuelve cuántos sonidos.
#[tauri::command]
pub async fn export_pack(app: AppHandle, path: String, library: String, sounds: bool) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let list = if sounds { app.state::<Audio>().user_sounds(&app)? } else { Vec::new() };
        write(Path::new(&path), &library, &list)?;
        Ok(list.len())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
pub struct Imported {
    library: String,
    sounds: Vec<SoundInfo>,
}

/// Devuelve el `library.json` del paquete y añade sus sonidos a la biblioteca.
#[tauri::command]
pub async fn import_pack(app: AppHandle, path: String) -> Result<Imported, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (library, packed) = read(Path::new(&path))?;
        let audio = app.state::<Audio>();
        let mut sounds = Vec::new();
        for p in packed {
            let ext = Path::new(&p.entry.file).extension().and_then(|e| e.to_str()).unwrap_or("bin").to_owned();
            match audio.import_bytes(&app, &p.bytes, &ext, &p.entry.name, p.entry.kind) {
                Ok(info) => sounds.push(info),
                Err(e) => log::warn!("paquete: el sonido {} no se pudo importar: {e}", p.entry.name),
            }
        }
        Ok(Imported { library, sounds })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta_y_rechazos() {
        let dir = std::env::temp_dir().join(format!("imgx-pack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("mis clips.imgxpack");
        let lib = r#"{"clips":[{"id":"a","name":"Zoom"}],"templates":[]}"#;
        write(&f, lib, &[]).unwrap();
        let (back, sounds) = read(&f).unwrap();
        assert_eq!(back, lib);
        assert!(sounds.is_empty());
        assert!(write(&f, "no es json", &[]).is_err());

        // Con un sonido: viaja dentro y sale igual.
        let wav = dir.join("voz.wav");
        imgx_audio::wav::write(&wav, &imgx_audio::synth::sine(220.0, 0.3, 22050)).unwrap();
        let entry = SoundEntry { id: "abc".into(), name: "Voz 1".into(), file: "abc.wav".into(), kind: crate::audio::SoundKind::Recorded, seconds: 0.3, added: 0 };
        let with = dir.join("con sonido.imgxpack");
        write(&with, lib, &[(entry, wav.clone())]).unwrap();
        let (_, sounds) = read(&with).unwrap();
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].entry.name, "Voz 1");
        assert_eq!(sounds[0].bytes, std::fs::read(&wav).unwrap());

        let bad = dir.join("otro.zip");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&bad).unwrap());
        z.start_file("manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(br#"{"format":"imgxpack","version":99}"#).unwrap();
        z.finish().unwrap();
        assert!(read(&bad).unwrap_err().contains("más nueva"));
        std::fs::write(&bad, "hola").unwrap();
        assert!(read(&bad).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
