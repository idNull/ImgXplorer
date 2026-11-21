//! Capa Tauri común a imgxplorer de escritorio y al visor móvil: la fuente abierta y su estado,
//! las órdenes para abrirla y analizar sus páginas, el protocolo `imgx://`, las portadas de la
//! estantería, los proyectos `.imgx` y el motor de sonido.
//!
//! Las dos apps registran estas mismas órdenes, con los mismos nombres, así que la interfaz
//! (`ui/src/lib`) funciona igual en las dos.

pub mod audio;
pub mod covers;
pub mod project;
pub mod serve;
pub mod source;

use imgx_media::panels::PanelRect;
use imgx_media::{Source, SourceKind};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

/// Fuente abierta.
#[derive(Clone)]
pub struct Current {
    /// Cambia en cada apertura para invalidar cachés del webview.
    pub generation: u64,
    pub source: Arc<dyn Source>,
    pub path: PathBuf,
    pub kind: SourceKind,
    pub fingerprint: String,
}

#[derive(Default)]
pub struct AppState {
    pub current: RwLock<Option<Current>>,
    pub thumbs: serve::ThumbCache,
    /// Viñetas detectadas por imagen de la fuente abierta.
    pub panels: Mutex<HashMap<usize, Vec<PanelRect>>>,
    /// Niveles («limpiar escaneo») por imagen de la fuente abierta.
    pub levels: Mutex<HashMap<usize, imgx_media::levels::Levels>>,
    /// Recuadro con contenido («✂ ajustar al contenido») por imagen de la fuente abierta.
    pub boxes: Mutex<HashMap<usize, Option<PanelRect>>>,
    /// Última generación usada; no se reinicia al cerrar para que las URL nunca se repitan.
    pub last_generation: std::sync::atomic::AtomicU64,
}

impl AppState {
    /// La fuente abierta, si sigue siendo la de esa generación.
    pub fn current(&self, generation: u64) -> Option<Current> {
        self.current.read().unwrap().as_ref().filter(|c| c.generation == generation).cloned()
    }
}
