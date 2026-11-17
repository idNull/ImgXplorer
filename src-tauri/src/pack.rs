//! Paquetes `.imgxpack` para compartir clips y plantillas propias: un zip con `manifest.json`
//! (formato y versión) y `library.json` (lo define la interfaz, que lo valida al importar).

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;

pub const FORMAT: &str = "imgxpack";
pub const VERSION: u32 = 1;
const LIBRARY: &str = "library.json";
/// Un paquete de clips es pequeño; esto evita leer un zip malicioso enorme.
const MAX_LIBRARY: u64 = 32 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    /// Versión de imgxplorer que lo creó (informativo).
    #[serde(default)]
    app: String,
}

fn write(path: &Path, library: &str) -> Result<(), String> {
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
        z.finish()?;
        Ok(())
    })();
    result.and_then(|()| Ok(std::fs::rename(&tmp, path)?)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

fn read(path: &Path) -> Result<String, String> {
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
    Ok(library)
}

#[tauri::command]
pub async fn export_pack(path: String, library: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || write(Path::new(&path), &library)).await.map_err(|e| e.to_string())?
}

/// Devuelve el `library.json` del paquete.
#[tauri::command]
pub async fn import_pack(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || read(Path::new(&path))).await.map_err(|e| e.to_string())?
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
        write(&f, lib).unwrap();
        assert_eq!(read(&f).unwrap(), lib);
        assert!(write(&f, "no es json").is_err());

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
