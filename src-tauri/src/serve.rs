//! Protocolo `imgx://localhost/...`:
//!
//! - `/<gen>_<i>`   imagen `i`: bytes originales si el webview la sabe mostrar; si no, RGBA.
//! - `/d/<gen>_<i>` fuerza la decodificación en Rust (respaldo si el webview falla).
//! - `/t/<gen>_<i>` miniatura JPEG.
//!
//! RGBA se sirve como `application/x-imgx-rgba`: `"RGBA"`, ancho, alto (u32 LE) y píxeles.

use crate::AppState;
use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex};
use std::time::Instant;
use tauri::Manager;
use tauri::http::{Response, StatusCode};

const THUMB_SIDE: u32 = 320;
const THUMB_BG: [u8; 3] = [0x15, 0x18, 0x1d];
const THUMB_CACHE: usize = 4000;

/// Semáforo para limitar decodificaciones simultáneas (cada petición tiene su hilo).
struct Semaphore {
    free: Mutex<usize>,
    cv: Condvar,
}

impl Semaphore {
    const fn new(n: usize) -> Self {
        Self { free: Mutex::new(n), cv: Condvar::new() }
    }
    fn run<T>(&self, f: impl FnOnce() -> T) -> T {
        {
            let mut free = self.cv.wait_while(self.free.lock().unwrap(), |n| *n == 0).unwrap();
            *free -= 1;
        }
        let out = f();
        *self.free.lock().unwrap() += 1;
        self.cv.notify_one();
        out
    }
}

/// Las miniaturas tienen menos prioridad: su propio cupo, para no bloquear la imagen actual.
static FULL: Semaphore = Semaphore::new(3);
static THUMBS: Semaphore = Semaphore::new(3);

type ThumbKey = (u64, usize);

#[derive(Default)]
struct Lru {
    map: HashMap<ThumbKey, Vec<u8>>,
    order: VecDeque<ThumbKey>,
}

#[derive(Default)]
pub struct ThumbCache(Mutex<Lru>);

impl ThumbCache {
    fn get(&self, k: ThumbKey) -> Option<Vec<u8>> {
        self.0.lock().unwrap().map.get(&k).cloned()
    }
    fn put(&self, k: ThumbKey, v: Vec<u8>) {
        let mut g = self.0.lock().unwrap();
        if g.map.insert(k, v).is_none() {
            g.order.push_back(k);
        }
        while g.order.len() > THUMB_CACHE {
            let old = g.order.pop_front().unwrap();
            g.map.remove(&old);
        }
    }
    pub fn clear(&self) {
        let mut g = self.0.lock().unwrap();
        g.map.clear();
        g.order.clear();
    }
}

fn respond(status: StatusCode, mime: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    let mut b = Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Access-Control-Allow-Origin", "*");
    if status.is_success() {
        b = b.header("Cache-Control", "max-age=31536000, immutable");
    }
    b.body(body).unwrap()
}

fn not_found(msg: String) -> Response<Vec<u8>> {
    respond(StatusCode::NOT_FOUND, "text/plain; charset=utf-8", msg.into_bytes())
}

enum Route {
    Image,
    Decoded,
    Thumb,
}

pub fn handle(app: &tauri::AppHandle, path: &str) -> Response<Vec<u8>> {
    let path = path.trim_start_matches('/');
    let (route, rest) = match path.split_once('/') {
        Some(("d", r)) => (Route::Decoded, r),
        Some(("t", r)) => (Route::Thumb, r),
        _ => (Route::Image, path),
    };
    let Some((generation, index)) =
        rest.split_once('_').and_then(|(g, i)| Some((g.parse::<u64>().ok()?, i.parse::<usize>().ok()?)))
    else {
        log::warn!("imgx://: ruta inválida {path}");
        return not_found(format!("ruta inválida: {path}"));
    };
    let state = app.state::<AppState>();
    let source = {
        let guard = state.source.read().unwrap();
        match guard.as_ref() {
            Some((g, s)) if *g == generation => s.clone(),
            _ => return not_found("fuente cerrada".into()),
        }
    };
    let Some(entry) = source.entries().get(index) else { return not_found(format!("índice {index}")) };
    let name = entry.name.as_str();

    if let Route::Thumb = route
        && let Some(t) = state.thumbs.get((generation, index))
    {
        return respond(StatusCode::OK, "image/jpeg", t);
    }

    let t0 = Instant::now();
    let bytes = match source.read(index) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("error leyendo {name}: {e}");
            return not_found(e.to_string());
        }
    };
    let read_ms = t0.elapsed().as_millis();

    let result = match route {
        Route::Image if !imgx_media::needs_decode(name) => return respond(StatusCode::OK, imgx_media::mime_for(name), bytes),
        Route::Image | Route::Decoded => FULL
            .run(|| imgx_media::decode(name, &bytes))
            .map(|rgba| ("application/x-imgx-rgba", rgba.into_wire())),
        Route::Thumb => THUMBS.run(|| imgx_media::thumbnail_jpeg(name, &bytes, THUMB_SIDE, THUMB_BG)).map(|jpg| {
            state.thumbs.put((generation, index), jpg.clone());
            ("image/jpeg", jpg)
        }),
    };
    match result {
        Ok((mime, body)) => {
            if matches!(route, Route::Image | Route::Decoded) {
                log::debug!("decodificada {name} (lectura {read_ms} ms, total {} ms)", t0.elapsed().as_millis());
            }
            respond(StatusCode::OK, mime, body)
        }
        Err(e) => {
            log::warn!("{name}: {e}");
            respond(StatusCode::UNPROCESSABLE_ENTITY, "text/plain; charset=utf-8", e.to_string().into_bytes())
        }
    }
}
