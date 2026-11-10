//! API mínima expuesta a la interfaz. Las poses viajan como `Float64Array`
//! `[cx, cy, zoom, rot]` y las matrices como `[a, b, c, d, e, f]` para evitar
//! serializar JSON en cada frame.

use imgx_core::schema::Timeline;
use imgx_core::timeline::NoClips;
use imgx_core::Camera;
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

/// Timeline ya parseada, para evaluar en cada frame sin volver a leer JSON.
#[wasm_bindgen]
pub struct WasmTimeline(Timeline);

#[wasm_bindgen]
impl WasmTimeline {
    #[wasm_bindgen(constructor)]
    pub fn new(json: &str) -> Result<WasmTimeline, JsError> {
        Ok(WasmTimeline(serde_json::from_str(json)?))
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
