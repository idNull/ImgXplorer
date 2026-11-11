//! API mínima expuesta a la interfaz. Las poses viajan como `Float64Array`
//! `[cx, cy, zoom, rot]` y las matrices como `[a, b, c, d, e, f]` para evitar
//! serializar JSON en cada frame.

use imgx_core::Camera;
use imgx_core::schema::{Rect, TemplateRef, Timeline, Transition};
use imgx_core::templates::{self, Ctx};
use imgx_core::timeline::NoClips;
use imgx_core::transition::{self, View};
use wasm_bindgen::prelude::*;

fn cam(p: &[f64]) -> Camera {
    Camera { cx: p[0], cy: p[1], zoom: p[2], rot: p.get(3).copied().unwrap_or(0.0) }
}

fn arr(c: Camera) -> Vec<f64> {
    vec![c.cx, c.cy, c.zoom, c.rot]
}

#[wasm_bindgen]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

/// Matriz imagen→viewport para la pose `pose`.
#[wasm_bindgen]
pub fn view_matrix(pose: &[f64], img_w: f64, img_h: f64, vp_w: f64, vp_h: f64) -> Vec<f64> {
    cam(pose).to_viewport((img_w, img_h), (vp_w, vp_h)).to_vec()
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn zoom_at(pose: &[f64], factor: f64, ax: f64, ay: f64, img_w: f64, img_h: f64, vp_w: f64, vp_h: f64) -> Vec<f64> {
    arr(cam(pose).zoom_at(factor, (ax, ay), (img_w, img_h), (vp_w, vp_h)))
}

#[wasm_bindgen]
pub fn pan_by(pose: &[f64], dx: f64, dy: f64, img_w: f64, img_h: f64, vp_w: f64, vp_h: f64) -> Vec<f64> {
    arr(cam(pose).pan_by(dx, dy, (img_w, img_h), (vp_w, vp_h)))
}

/// Interpolación entre poses (zoom logarítmico), para animar acciones de teclado.
#[wasm_bindgen]
pub fn lerp_pose(a: &[f64], b: &[f64], t: f64) -> Vec<f64> {
    arr(cam(a).lerp(&cam(b), t))
}

/// `[x, y, w, h, x, y, w, h, …]` → rectángulos.
fn rects(flat: &[f64]) -> Vec<Rect> {
    flat.as_chunks::<4>().0.iter().map(|c| Rect { x: c[0], y: c[1], w: c[2], h: c[3] }).collect()
}

fn template(json: &str) -> Result<TemplateRef, JsError> {
    Ok(serde_json::from_str(json)?)
}

/// Catálogo de plantillas y sus parámetros (JSON), para generar los formularios.
#[wasm_bindgen]
pub fn template_catalog() -> String {
    serde_json::to_string(templates::CATALOG).expect("catálogo serializable")
}

/// Viñetas que recorrerá la plantilla `comic`, en orden de lectura (planas, como `panels`).
#[wasm_bindgen]
pub fn comic_panels(tpl: &str, img_w: f64, img_h: f64, aspect: f64, panels: &[f64]) -> Result<Vec<f64>, JsError> {
    let det = rects(panels);
    let ctx = Ctx { img: (img_w, img_h), aspect, panels: Some(&det) };
    Ok(templates::comic_panels(&template(tpl)?, &ctx).iter().flat_map(|r| [r.x, r.y, r.w, r.h]).collect())
}

/// Fotograma de una transición: `[mA(6), alfaA, mB(6), alfaB, mezcla(0|1)]`.
/// `tpl` es la plantilla de la imagen saliente (`""` si no tiene), para deducir la dirección.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn transition_frame(
    tr: &str,
    tpl: &str,
    u: f64,
    a_w: f64,
    a_h: f64,
    a_pose: &[f64],
    b_w: f64,
    b_h: f64,
    b_pose: &[f64],
    vp_w: f64,
    vp_h: f64,
) -> Result<Vec<f64>, JsError> {
    let tr: Transition = serde_json::from_str(tr)?;
    let tpl = if tpl.is_empty() { None } else { Some(template(tpl)?) };
    let a = View { img: (a_w, a_h), pose: cam(a_pose) };
    let b = View { img: (b_w, b_h), pose: cam(b_pose) };
    let f = transition::frame(&tr, templates::reading_direction(tpl.as_ref()), u, &a, &b, (vp_w, vp_h));
    let mut out = Vec::with_capacity(15);
    out.extend_from_slice(&f.a.m);
    out.push(f.a.alpha);
    out.extend_from_slice(&f.b.m);
    out.push(f.b.alpha);
    out.push(if f.mix { 1.0 } else { 0.0 });
    Ok(out)
}

/// Timeline ya parseada, para evaluar en cada frame sin volver a leer JSON.
#[wasm_bindgen]
pub struct WasmTimeline(Timeline);

#[wasm_bindgen]
impl WasmTimeline {
    #[wasm_bindgen(constructor)]
    pub fn new(json: &str) -> Result<WasmTimeline, JsError> {
        Ok(WasmTimeline(serde_json::from_str(json)?))
    }
    /// Genera la timeline de una plantilla para una imagen y una proporción de viewport.
    pub fn from_template(tpl: &str, img_w: f64, img_h: f64, aspect: f64, panels: &[f64]) -> Result<WasmTimeline, JsError> {
        let det = rects(panels);
        let ctx = Ctx { img: (img_w, img_h), aspect, panels: Some(&det) };
        Ok(WasmTimeline(templates::generate(&template(tpl)?, &ctx)?))
    }
    pub fn duration(&self) -> f64 {
        self.0.duration()
    }
    pub fn evaluate(&self, t: f64) -> Vec<f64> {
        arr(self.0.evaluate(t, &NoClips))
    }
    pub fn marks(&self) -> Vec<f64> {
        self.0.marks(&NoClips)
    }
}
