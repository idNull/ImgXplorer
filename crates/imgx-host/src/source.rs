//! Órdenes de la fuente abierta: abrirla (carpeta, comprimido, imagen o historia `.imgxstory`),
//! cerrarla y analizar sus páginas (viñetas, niveles y recuadro con contenido).

use crate::{AppState, Current, audio, covers, project};
use imgx_media::SourceKind;
use imgx_media::panels::PanelRect;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInfo {
    name: String,
    size: u64,
    /// El webview no sabe mostrarla: se sirve como RGBA decodificado en Rust.
    decoded: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub generation: u64,
    title: String,
    pub path: String,
    pub kind: &'static str,
    /// Imagen en la que situarse al abrir (la pedida, si se abrió una imagen suelta).
    start: usize,
    /// Se abrió una imagen concreta: manda sobre la página recordada.
    picked: bool,
    entries: Vec<EntryInfo>,
    pub fingerprint: String,
    /// Animaciones guardadas de esta fuente.
    project: project::Loaded,
    /// Es una historia `.imgxstory`: su manifiesto (título, créditos, modo de lectura…).
    #[serde(skip_serializing_if = "Option::is_none")]
    story: Option<imgx_story::Manifest>,
    /// Es un cómic propio (una carpeta que crea y ordena imgxplorer): su título.
    #[serde(skip_serializing_if = "Option::is_none")]
    comic: Option<imgx_media::comic::ComicInfo>,
}

#[tauri::command]
pub async fn open_source(app: AppHandle, path: String, state: State<'_, AppState>) -> Result<SourceInfo, String> {
    // Ruta canónica: la misma fuente abierta de distintas formas se recuerda una sola vez.
    let requested = std::fs::canonicalize(&path).unwrap_or_else(|_| PathBuf::from(&path));
    // Listar un comprimido grande puede tardar: fuera del hilo principal.
    let (opened, story, comic) = tauri::async_runtime::spawn_blocking(move || {
        // Una historia se comprueba entera antes de abrirla: manifiesto, proyecto y sonidos.
        let story = if imgx_media::is_story_name(&requested.to_string_lossy()) { Some(imgx_story::read(&requested)?) } else { None };
        let opened = imgx_media::open_path(&requested).map_err(|e| e.to_string())?;
        // Un cómic propio cuyo último orden se cortó a medias: sus páginas vuelven a su nombre.
        let comic = if opened.kind == SourceKind::Folder { imgx_media::comic::info(&opened.path) } else { None };
        if comic.is_some()
            && let Ok(n @ 1..) = imgx_media::comic::recover(&opened.path)
        {
            log::warn!("{n} páginas recuperadas de un orden cortado en {}", opened.path.display());
            return Ok::<_, String>((imgx_media::open_path(&requested).map_err(|e| e.to_string())?, story, comic));
        }
        Ok::<_, String>((opened, story, comic))
    })
    .await
    .map_err(|e| e.to_string())?
    .inspect_err(|e| log::warn!("no se pudo abrir {path}: {e}"))?;
    let src = opened.source;
    log::info!("abierta {} ({:?}, {} imágenes, inicio {})", opened.path.display(), opened.kind, src.entries().len(), opened.start);
    let mut fingerprint = imgx_media::fingerprint(src.entries());
    if story.is_some() {
        // Distinta de la del cómic original (que puede tener las mismas páginas): sus cambios no
        // se mezclan con los del cómic.
        fingerprint = imgx_media::fingerprint(&[imgx_media::Entry { name: format!("imgxstory:{fingerprint}"), size: 0 }]);
    }
    let loaded = project::or_embedded(project::load(&app, &opened.path, opened.kind, &fingerprint), story.as_ref().map(|s| &s.project));
    let mut guard = state.current.write().unwrap();
    let generation = state.last_generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let info = SourceInfo {
        generation,
        title: src.title().to_owned(),
        path: opened.path.display().to_string(),
        kind: match opened.kind {
            SourceKind::Folder => "folder",
            SourceKind::Archive => "archive",
        },
        start: opened.start,
        picked: opened.picked,
        entries: src
            .entries()
            .iter()
            .map(|e| EntryInfo { name: e.name.clone(), size: e.size, decoded: imgx_media::needs_decode(&e.name) })
            .collect(),
        fingerprint: fingerprint.clone(),
        project: loaded,
        story: story.as_ref().map(|s| s.manifest.clone()),
        comic,
    };
    covers::ensure_on_open(&app, &info.path, src.clone());
    *guard = Some(Current { generation, source: src, path: opened.path, kind: opened.kind, fingerprint });
    state.thumbs.clear();
    state.panels.lock().unwrap().clear();
    state.levels.lock().unwrap().clear();
    state.boxes.lock().unwrap().clear();
    let audio = app.state::<audio::Audio>();
    match story {
        Some(st) => {
            // Lo ya calculado de cada página: no hace falta detectarlo otra vez.
            let mut panels = state.panels.lock().unwrap();
            let mut levels = state.levels.lock().unwrap();
            let mut boxes = state.boxes.lock().unwrap();
            for (i, e) in info.entries.iter().enumerate() {
                if let Some(d) = st.computed.pages.get(&e.name) {
                    if let Some(p) = &d.panels {
                        panels.insert(i, p.clone());
                    }
                    if let Some(l) = d.levels {
                        levels.insert(i, l);
                    }
                    if let Some(b) = d.content_box() {
                        boxes.insert(i, b);
                    }
                }
            }
            audio.set_story(Some(audio::StorySounds { path: PathBuf::from(&info.path), list: st.sounds }));
        }
        None => audio.set_story(None),
    }
    Ok(info)
}

/// Cierra la fuente abierta (vuelve a la pantalla de inicio).
#[tauri::command]
pub fn close_source(app: AppHandle, state: State<'_, AppState>) {
    if let Some(c) = state.current.write().unwrap().take() {
        log::info!("cerrada {}", c.path.display());
    }
    app.state::<audio::Audio>().set_story(None);
    state.thumbs.clear();
    state.panels.lock().unwrap().clear();
    state.levels.lock().unwrap().clear();
    state.boxes.lock().unwrap().clear();
}

/// Viñetas de la imagen `index` (vacío si no se distinguen al menos dos).
#[tauri::command]
pub async fn detect_panels(generation: u64, index: usize, state: State<'_, AppState>) -> Result<Vec<PanelRect>, String> {
    let cur = state.current(generation).ok_or("la fuente ya no está abierta")?;
    if let Some(p) = state.panels.lock().unwrap().get(&index) {
        return Ok(p.clone());
    }
    let src = cur.source.clone();
    let found = tauri::async_runtime::spawn_blocking(move || {
        let t0 = std::time::Instant::now();
        let name = src.entries().get(index).ok_or("índice fuera de rango")?.name.clone();
        let bytes = src.read(index).map_err(|e| e.to_string())?;
        let found = imgx_media::panels::detect_panels(&name, &bytes).map_err(|e| e.to_string())?;
        log::debug!("viñetas de {name}: {} en {} ms", found.len(), t0.elapsed().as_millis());
        Ok::<_, String>(found)
    })
    .await
    .map_err(|e| e.to_string())??;
    if state.current(generation).is_some() {
        state.panels.lock().unwrap().insert(index, found.clone());
    }
    Ok(found)
}

/// Color del papel y de la tinta de la imagen `index`, para «limpiar escaneo».
#[tauri::command]
pub async fn image_levels(generation: u64, index: usize, state: State<'_, AppState>) -> Result<imgx_media::levels::Levels, String> {
    let cur = state.current(generation).ok_or("la fuente ya no está abierta")?;
    if let Some(l) = state.levels.lock().unwrap().get(&index) {
        return Ok(*l);
    }
    let src = cur.source.clone();
    let found = tauri::async_runtime::spawn_blocking(move || {
        let name = src.entries().get(index).ok_or("índice fuera de rango")?.name.clone();
        let bytes = src.read(index).map_err(|e| e.to_string())?;
        imgx_media::levels::levels(&name, &bytes).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    if state.current(generation).is_some() {
        state.levels.lock().unwrap().insert(index, found);
    }
    Ok(found)
}

/// Recuadro con contenido de la imagen `index` (`None` si apenas tiene márgenes).
#[tauri::command]
pub async fn content_box(generation: u64, index: usize, state: State<'_, AppState>) -> Result<Option<PanelRect>, String> {
    let cur = state.current(generation).ok_or("la fuente ya no está abierta")?;
    if let Some(b) = state.boxes.lock().unwrap().get(&index) {
        return Ok(*b);
    }
    let src = cur.source.clone();
    let found = tauri::async_runtime::spawn_blocking(move || {
        let name = src.entries().get(index).ok_or("índice fuera de rango")?.name.clone();
        let bytes = src.read(index).map_err(|e| e.to_string())?;
        imgx_media::panels::content_box(&name, &bytes).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    if state.current(generation).is_some() {
        state.boxes.lock().unwrap().insert(index, found);
    }
    Ok(found)
}
