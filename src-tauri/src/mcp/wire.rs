//! Lo que comparten el puente (`imgxplorer --mcp`) y la app: dónde está el socket y qué mensajes
//! viajan por él (una línea JSON por mensaje).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Una llamada a una herramienta.
#[derive(Debug, Serialize, Deserialize)]
pub struct Call {
    pub id: u64,
    pub tool: String,
    #[serde(default)]
    pub args: serde_json::Value,
}

/// Respuesta de la app.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Reply {
    pub id: u64,
    pub ok: bool,
    /// Resultado para la IA (normalmente JSON).
    #[serde(default)]
    pub text: String,
    /// Imágenes del resultado, ya guardadas en disco (JPEG o PNG).
    #[serde(default)]
    pub images: Vec<String>,
    #[serde(default)]
    pub error: String,
    /// `starting`: la interfaz aún no está lista; `disabled`: el usuario no permite el control.
    #[serde(default)]
    pub code: String,
}

impl Reply {
    pub fn fail(id: u64, code: &str, error: impl Into<String>) -> Reply {
        Reply { id, ok: false, error: error.into(), code: code.into(), ..Default::default() }
    }
}

/// Límite de longitud de la ruta de un socket Unix (104 en macOS, 108 en Linux).
const MAX_SOCKET_PATH: usize = 100;

fn uid() -> u32 {
    // SAFETY: getuid no falla ni toca memoria.
    unsafe { libc::getuid() }
}

/// Carpeta privada del socket (solo el usuario puede entrar).
pub fn socket_dir() -> PathBuf {
    let preferred = {
        #[cfg(target_os = "macos")]
        {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/imgxplorer"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|d| d.is_dir()).map(|d| d.join("imgxplorer"))
        }
    };
    preferred
        .filter(|d| d.join("mcp.sock").as_os_str().len() < MAX_SOCKET_PATH)
        .unwrap_or_else(|| PathBuf::from(format!("/tmp/imgxplorer-{}", uid())))
}

pub fn socket_path() -> PathBuf {
    socket_dir().join("mcp.sock")
}

/// Crea la carpeta del socket con permisos 0700 y comprueba que es del usuario.
pub fn prepare_dir() -> Result<PathBuf, String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = socket_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let meta = std::fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.uid() != uid() {
        return Err(format!("{} no es una carpeta del usuario", dir.display()));
    }
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// El otro extremo del socket es del mismo usuario.
pub fn same_user(stream: &std::os::unix::net::UnixStream) -> bool {
    use std::os::fd::AsRawFd;
    let fd = stream.as_raw_fd();
    #[cfg(target_os = "linux")]
    {
        let mut cred = libc::ucred { pid: 0, uid: u32::MAX, gid: 0 };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: `cred` y `len` son válidos y del tamaño que espera SO_PEERCRED.
        let r = unsafe { libc::getsockopt(fd, libc::SOL_SOCKET, libc::SO_PEERCRED, (&mut cred as *mut libc::ucred).cast(), &mut len) };
        r == 0 && cred.uid == uid()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let (mut u, mut g) = (u32::MAX, 0);
        // SAFETY: punteros a variables locales válidas.
        let r = unsafe { libc::getpeereid(fd, &mut u, &mut g) };
        r == 0 && u == uid()
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn unbase64(s: &str) -> Option<Vec<u8>> {
    let s = s.trim_end_matches('=');
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for b in s.bytes() {
        let v = B64.iter().position(|&x| x == b)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_ida_y_vuelta() {
        for s in ["", "f", "fo", "foo", "foob", "fooba", "foobar"] {
            assert_eq!(unbase64(&base64(s.as_bytes())).unwrap(), s.as_bytes());
        }
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(b"fo"), "Zm8=");
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(unbase64(&base64(&all)).unwrap(), all);
        assert!(unbase64("**").is_none());
    }

    #[test]
    fn el_socket_cabe() {
        assert!(socket_path().as_os_str().len() < 104);
    }
}
