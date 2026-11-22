//! API mínima expuesta a la interfaz. Las poses viajan como `Float64Array`
//! `[cx, cy, zoom, rot]` y las matrices como `[a, b, c, d, e, f]` para evitar
//! serializar JSON en cada frame.

use imgx_core::Camera;
use imgx_core::record;
use imgx_core::schema::{Rect, Sample, TemplateRef, Timeline, Transition};
use imgx_core::shots;
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
/// `manual`: las viñetas las ha dibujado el usuario (mandan sobre el modo y la rejilla).
#[wasm_bindgen]
pub fn comic_panels(tpl: &str, img_w: f64, img_h: f64, aspect: f64, panels: &[f64], manual: bool) -> Result<Vec<f64>, JsError> {
    let det = rects(panels);
    let ctx = Ctx { img: (img_w, img_h), aspect, panels: Some(&det), manual };
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

/// Planos de director → `{timeline, shots: [{arrive, hold, end}], duration}` (JSON).
/// `plan` es un `ShotPlan` en JSON (rectángulos normalizados).
#[wasm_bindgen]
pub fn plan_shots(plan: &str, img_w: f64, img_h: f64, aspect: f64) -> Result<String, JsError> {
    let p: shots::ShotPlan = serde_json::from_str(plan)?;
    let ctx = Ctx { img: (img_w, img_h), aspect, panels: None, manual: false };
    Ok(serde_json::to_string(&shots::plan(&p, &ctx))?)
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
    pub fn from_template(tpl: &str, img_w: f64, img_h: f64, aspect: f64, panels: &[f64], manual: bool) -> Result<WasmTimeline, JsError> {
        let det = rects(panels);
        let ctx = Ctx { img: (img_w, img_h), aspect, panels: Some(&det), manual };
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
    /// La timeline en JSON (para "hornear" una plantilla en clips editables).
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.0).expect("timeline serializable")
    }
}

/// Suaviza y simplifica un movimiento grabado. Entrada y salida planas: `[t, cx, cy, zoom, rot, …]`.
#[wasm_bindgen]
pub fn simplify_recording(flat: &[f64], tolerance: f64) -> Vec<f64> {
    let samples: Vec<Sample> = flat
        .as_chunks::<5>()
        .0
        .iter()
        .map(|c| Sample { t: c[0], pose: Camera { cx: c[1], cy: c[2], zoom: c[3], rot: c[4] } })
        .collect();
    let tol = if tolerance > 0.0 { tolerance } else { record::DEFAULT_TOLERANCE };
    record::simplify(&samples, tol).iter().flat_map(|s| [s.t, s.pose.cx, s.pose.cy, s.pose.zoom, s.pose.rot]).collect()
}

/// Catálogo de efectos especiales: `{effects: [...], situations: [...]}` (JSON).
#[wasm_bindgen]
pub fn fx_catalog() -> String {
    imgx_core::fx::catalog_json()
}

/// Efecto nuevo de un tipo con su ajuste predefinido (`preset` vacío: valores de serie), en JSON.
#[wasm_bindgen]
pub fn fx_new(id: &str, kind: &str, preset: &str, start: f64) -> Result<String, JsError> {
    let preset = (!preset.is_empty()).then_some(preset);
    let fx = imgx_core::fx::new_fx(id, kind, preset, start).ok_or_else(|| JsError::new(&format!("efecto desconocido: {kind}")))?;
    Ok(serde_json::to_string(&fx)?)
}

/// Efectos de una situación colocados en `start` (ids `<base>-<n>`); punto en `at_x, at_y`
/// (normalizado) o ninguno si alguno no es un número. JSON con `{fx: [...], sound}`.
#[wasm_bindgen]
pub fn fx_situation(id: &str, base: &str, start: f64, at_x: f64, at_y: f64, with_text: bool) -> Result<String, JsError> {
    use imgx_core::fx;
    let sit = fx::situation(id).ok_or_else(|| JsError::new(&format!("situación desconocida: {id}")))?;
    let at = (at_x.is_finite() && at_y.is_finite()).then_some(fx::Point { x: at_x, y: at_y });
    Ok(serde_json::json!({ "fx": fx::expand_with(sit, base, start, at, with_text), "sound": sit.sound }).to_string())
}

/// Avisos sobre una lista de efectos (JSON de textos).
#[wasm_bindgen]
pub fn fx_warnings(list: &str, image_end: f64) -> Result<String, JsError> {
    let list: Vec<imgx_core::fx::Fx> = serde_json::from_str(list)?;
    Ok(serde_json::to_string(&imgx_core::fx::warnings(&list, image_end))?)
}

/// Efectos de una imagen ya parseados, para calcular cada fotograma sin volver a leer JSON.
#[wasm_bindgen]
pub struct WasmFx(Vec<imgx_core::fx::Fx>, Option<f64>);

#[wasm_bindgen]
impl WasmFx {
    #[wasm_bindgen(constructor)]
    pub fn new(json: &str) -> Result<WasmFx, JsError> {
        Ok(WasmFx(serde_json::from_str(json)?, None))
    }
    /// Proporción (ancho / alto) para la que se pensaron los tamaños de los efectos (0: ninguna,
    /// siempre la altura de la pantalla). El visor móvil la pone para que en vertical las
    /// onomatopeyas quepan; ver `imgx_core::fx::units`.
    pub fn set_fit(&mut self, aspect: f64) {
        self.1 = (aspect > 0.0).then_some(aspect);
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn end(&self) -> f64 {
        imgx_core::fx::end(&self.0)
    }
    /// Fotograma en `t` (segundos de la imagen) con la pose de cámara de ese instante. Valores en
    /// el orden de `FxFrame::to_vec` y luego los textos (`TextSprite::to_vec`); vacío si no hay
    /// ningún efecto activo.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&self, t: f64, img_w: f64, img_h: f64, pose: &[f64], vp_w: f64, vp_h: f64) -> Vec<f64> {
        use imgx_core::fx;
        if !self.0.iter().any(|f| t >= f.start && t < f.end()) {
            return Vec::new();
        }
        let view = fx::View { img: (img_w, img_h), pose: cam(pose), vp: (vp_w, vp_h), fit: self.1 };
        let f = fx::frame(&self.0, t, &view);
        // Detrás de los valores del shader, los textos: `TextSprite::LEN` valores cada uno.
        let mut v = f.to_vec();
        for s in &f.texts {
            v.extend_from_slice(&s.to_vec());
        }
        v
    }
}
