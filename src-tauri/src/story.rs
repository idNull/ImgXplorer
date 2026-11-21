//! Exportar la fuente abierta como historia `.imgxstory` (páginas, animación y sonidos en un solo
//! archivo para compartir). El formato lo define `imgx-story`; aquí se reúne lo que hay en la app:
//! las páginas de la fuente, el proyecto que tiene la interfaz, los sonidos de la biblioteca (o de
//! la historia abierta) y lo ya calculado de cada página.

use crate::AppState;
use crate::audio::{Audio, SoundFile};
use imgx_core::Project;
use imgx_story::{ExportInput, Manifest, PageData, Reading, Variant};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, State};

/// Exportación en marcha (solo una a la vez).
#[derive(Default)]
pub struct StoryExport {
    running: AtomicBool,
    cancel: AtomicBool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryOptions {
    path: String,
    /// Primera y última imagen (índices de la fuente, incluidas).
    from: usize,
    to: usize,
    variant: Variant,
    #[serde(default)]
    title: String,
    #[serde(default)]
    credits: String,
    #[serde(default)]
    keep_takes: bool,
    #[serde(default)]
    reading: Reading,
    #[serde(default)]
    aspect: Option<f64>,
    #[serde(default)]
    lang: Option<String>,
    /// El proyecto tal como lo tiene la interfaz (con lo que aún no se haya guardado).
    project: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryResult {
    path: String,
    #[serde(flatten)]
    written: imgx_story::Written,
    /// Sonidos que usa el proyecto pero ya no están (no viajan; no sonarán).
    missing: Vec<String>,
}

#[derive(Clone, Serialize)]
struct ProgressEvent {
    done: usize,
    total: usize,
}

/// Exporta la historia. Avisa de cómo va con el evento `story-progress` (`{ done, total }`).
#[tauri::command]
pub async fn story_export(app: AppHandle, generation: u64, opts: StoryOptions, state: State<'_, AppState>) -> Result<StoryResult, String> {
    let cur = state.current(generation).ok_or("la fuente ya no está abierta")?;
    let job = app.state::<StoryExport>();
    if job.running.swap(true, Ordering::SeqCst) {
        return Err("ya se está exportando una historia".into());
    }
    job.cancel.store(false, Ordering::SeqCst);
    // Lo que ya se calculó (viñetas y niveles), para no repetirlo.
    let panels = state.panels.lock().unwrap().clone();
    let levels = state.levels.lock().unwrap().clone();
    let boxes = state.boxes.lock().unwrap().clone();
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let src = cur.source.clone();
        let order: Vec<String> = src.entries().iter().map(|e| e.name.clone()).collect();
        if order.is_empty() {
            return Err("no hay imágenes".to_string());
        }
        let to = opts.to.min(order.len() - 1);
        let from = opts.from.min(to);
        let mut path = PathBuf::from(&opts.path);
        if !imgx_media::is_story_name(&path.to_string_lossy()) {
            path.as_mut_os_string().push(".imgxstory");
        }
        let full = Project::from_json(&opts.project).map_err(|e| e.to_string())?;
        let title = if opts.title.trim().is_empty() { default_title(src.title(), cur.kind) } else { opts.title.trim().chars().take(200).collect() };
        let project = imgx_story::prepare::project(&full, &order, from, to, opts.keep_takes, &title);

        // Sonidos: de la biblioteca o de la historia abierta.
        let audio = app2.state::<Audio>();
        let mut sounds = Vec::new();
        let mut files = Vec::new();
        let mut missing = Vec::new();
        for id in imgx_story::prepare::sounds(&project) {
            match audio.locate(&app2, &id)? {
                Some((s, f)) => {
                    sounds.push(s);
                    files.push(f);
                }
                None => missing.push(id),
            }
        }
        if !missing.is_empty() {
            log::warn!("historia: {} sonidos no encontrados: {missing:?}", missing.len());
        }

        // «Limpiar escaneo» necesita los niveles de cada página.
        let clean = |a: &Option<imgx_core::adjust::Adjust>| a.as_ref().is_some_and(|a| a.clean > 0.0);
        let any_clean = clean(&project.defaults.adjust) || project.items.iter().any(|it| clean(&it.adjust));
        let manual: std::collections::HashSet<String> = project.items.iter().filter(|it| it.panels.is_some()).map(|it| it.entry.clone()).collect();
        let names: Vec<String> = order[from..=to].to_vec();
        let read_page = |i: usize| -> Result<(Vec<u8>, PageData), String> {
            let index = from + i;
            let name = &order[index];
            let bytes = src.read(index).map_err(|e| format!("{name}: {e}"))?;
            let mut data = PageData::default();
            let want_panels = !manual.contains(name);
            let (mut found_panels, mut found_box) = (panels.get(&index).cloned(), boxes.get(&index).copied());
            if (want_panels && found_panels.is_none()) || found_box.is_none() {
                // Lo que falta se calcula con una sola decodificación.
                if let Ok((p, b)) = imgx_media::panels::analyze(name, &bytes) {
                    found_panels.get_or_insert(p);
                    found_box.get_or_insert(b);
                }
            }
            if want_panels {
                data.panels = Some(found_panels.unwrap_or_default());
            }
            data.content = found_box.map(|b| b.unwrap_or(imgx_story::NO_MARGIN));
            if any_clean {
                data.levels = levels.get(&index).copied().or_else(|| imgx_media::levels::levels(name, &bytes).ok());
            }
            Ok((bytes, data))
        };
        let read_sound = |k: usize| -> Result<Vec<u8>, String> { files.get(k).ok_or("sonido fuera de rango")?.read() };
        let manifest = Manifest {
            app: env!("CARGO_PKG_VERSION").into(),
            variant: opts.variant,
            title,
            credits: opts.credits.trim().chars().take(2000).collect(),
            lang: opts.lang,
            reading: opts.reading,
            authored_aspect: opts.aspect.filter(|a| a.is_finite() && *a > 0.0),
            ..Manifest::default()
        };
        let input = ExportInput { pages: names, read_page: &read_page, project, sounds, read_sound: &read_sound, manifest };
        let job = app2.state::<StoryExport>();
        let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
        let written = imgx_story::write(&path, input, &mut |done, total| {
            // Como mucho unas 10 veces por segundo (y siempre al final).
            if done == total || last.elapsed().as_millis() >= 100 {
                last = std::time::Instant::now();
                let _ = app2.emit("story-progress", ProgressEvent { done, total });
            }
            !job.cancel.load(Ordering::SeqCst)
        })?;
        log::info!("historia exportada en {} ({} páginas, {} sonidos, {} bytes)", path.display(), written.pages, written.sounds, written.bytes);
        Ok(StoryResult { path: path.display().to_string(), written, missing })
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    job.running.store(false, Ordering::SeqCst);
    if let Err(e) = &result
        && e != imgx_story::CANCELLED
    {
        log::warn!("exportar historia: {e}");
    }
    result
}

/// Título por defecto: el nombre de la carpeta o del comprimido, sin su extensión.
fn default_title(title: &str, kind: imgx_media::SourceKind) -> String {
    match kind {
        imgx_media::SourceKind::Archive => std::path::Path::new(title).file_stem().map_or_else(|| title.to_owned(), |s| s.to_string_lossy().into_owned()),
        imgx_media::SourceKind::Folder => title.to_owned(),
    }
}

/// Cancela la exportación en marcha (el archivo a medias se borra).
#[tauri::command]
pub fn story_cancel(job: State<'_, StoryExport>) {
    job.cancel.store(true, Ordering::SeqCst);
}

/// Tamaño de la fuente abierta (para estimar lo que pesará la historia).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryEstimate {
    /// Bytes de las páginas del rango, tal como están.
    pages: u64,
    /// Bytes de los sonidos propios que usa el proyecto.
    sounds: u64,
}

#[tauri::command]
pub fn story_estimate(app: AppHandle, generation: u64, from: usize, to: usize, project: String, state: State<'_, AppState>) -> Result<StoryEstimate, String> {
    let cur = state.current(generation).ok_or("la fuente ya no está abierta")?;
    let entries = cur.source.entries();
    if entries.is_empty() {
        return Ok(StoryEstimate { pages: 0, sounds: 0 });
    }
    let to = to.min(entries.len() - 1);
    let pages = entries[from.min(to)..=to].iter().map(|e| e.size).sum();
    let order: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    let full = Project::from_json(&project).map_err(|e| e.to_string())?;
    let p = imgx_story::prepare::project(&full, &order, from.min(to), to, true, "");
    let audio = app.state::<Audio>();
    let mut sounds = 0;
    for id in imgx_story::prepare::sounds(&p) {
        sounds += match audio.locate(&app, &id)? {
            Some((_, SoundFile::Library(path))) => std::fs::metadata(path).map_or(0, |m| m.len()),
            Some((s, SoundFile::Story(..))) => (s.seconds * 32_000.0) as u64,
            None => 0,
        };
    }
    Ok(StoryEstimate { pages, sounds })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titulos() {
        assert_eq!(default_title("Tomo 1.cbz", imgx_media::SourceKind::Archive), "Tomo 1");
        assert_eq!(default_title("Mi historia.imgxstory", imgx_media::SourceKind::Archive), "Mi historia");
        assert_eq!(default_title("Vol.2", imgx_media::SourceKind::Folder), "Vol.2");
    }
}
