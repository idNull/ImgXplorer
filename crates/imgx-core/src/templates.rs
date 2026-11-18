//! Plantillas integradas: generadores deterministas que convierten parámetros + tamaño de
//! imagen + proporción del viewport en una timeline de keyframes.
//!
//! La timeline depende de la proporción del viewport (ajustar al ancho no es lo mismo en 16:9
//! que en 4:3), así que se regenera al cambiar el tamaño de la ventana o del vídeo. Por eso el
//! proyecto guarda la plantilla y sus parámetros, no la timeline generada.

use crate::camera::Camera;
use crate::easing::Easing;
use crate::schema::{Clip, ClipInstance, ClipSource, Direction, Keyframe, Rect, TemplateRef, Timeline};
use serde::Serialize;
use serde_json::Value;

/// Datos de la imagen sobre la que se genera.
pub struct Ctx<'a> {
    /// Tamaño de la imagen en píxeles.
    pub img: (f64, f64),
    /// Ancho / alto del viewport.
    pub aspect: f64,
    /// Viñetas detectadas (para `comic` en modo automático) o dibujadas a mano. Sin orden.
    pub panels: Option<&'a [Rect]>,
    /// `panels` las ha dibujado el usuario: se usan siempre (aunque sea una o el modo sea rejilla).
    pub manual: bool,
}

impl Ctx<'_> {
    pub(crate) fn vp(&self) -> (f64, f64) {
        (self.aspect, 1.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("plantilla desconocida: {0}")]
    Unknown(String),
}

// ---------------------------------------------------------------------------------------------
// Catálogo de parámetros (la interfaz genera el formulario a partir de aquí)

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ParamKind {
    Number { min: f64, max: f64, step: f64, default: f64 },
    Choice { options: &'static [&'static str], default: &'static str },
    Bool { default: bool },
}

#[derive(Debug, Serialize)]
pub struct ParamSpec {
    pub key: &'static str,
    #[serde(flatten)]
    pub kind: ParamKind,
    /// Solo aplica si el parámetro `.0` vale `.1` (p. ej. filas/columnas en modo rejilla).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<(&'static str, &'static str)>,
}

#[derive(Debug, Serialize)]
pub struct TemplateInfo {
    pub id: &'static str,
    pub params: &'static [ParamSpec],
}

const fn num(key: &'static str, min: f64, max: f64, step: f64, default: f64) -> ParamSpec {
    ParamSpec { key, kind: ParamKind::Number { min, max, step, default }, when: None }
}
const fn choice(key: &'static str, options: &'static [&'static str], default: &'static str) -> ParamSpec {
    ParamSpec { key, kind: ParamKind::Choice { options, default }, when: None }
}
const fn flag(key: &'static str, default: bool) -> ParamSpec {
    ParamSpec { key, kind: ParamKind::Bool { default }, when: None }
}
const fn only(mut p: ParamSpec, key: &'static str, value: &'static str) -> ParamSpec {
    p.when = Some((key, value));
    p
}

const EASINGS: &[&str] = &["ease-in-out", "linear", "bounce", "elastic"];

const SCROLL: &[ParamSpec] = &[
    choice("dir", &["down", "up", "right", "left"], "down"),
    num("speed", 0.05, 2.0, 0.05, 0.25),
    num("fill", 0.3, 1.0, 0.05, 1.0),
    num("pauseStart", 0.0, 10.0, 0.1, 1.0),
    num("pauseEnd", 0.0, 10.0, 0.1, 1.0),
];

const COMIC: &[ParamSpec] = &[
    choice("mode", &["auto", "grid"], "auto"),
    only(num("rows", 1.0, 8.0, 1.0, 3.0), "mode", "grid"),
    only(num("cols", 1.0, 6.0, 1.0, 2.0), "mode", "grid"),
    choice("dir", &["rtl", "ltr"], "rtl"),
    num("hold", 0.2, 10.0, 0.1, 2.0),
    num("move", 0.1, 5.0, 0.1, 0.8),
    num("margin", 0.0, 0.3, 0.01, 0.04),
    num("maxZoom", 1.0, 8.0, 0.1, 4.0),
    flag("overview", true),
];

const BOUNCE: &[ParamSpec] = &[
    num("cycles", 1.0, 10.0, 1.0, 2.0),
    num("period", 0.4, 10.0, 0.1, 2.0),
    num("zoom", 1.05, 4.0, 0.05, 1.5),
    num("lift", 0.0, 0.5, 0.01, 0.12),
    choice("easing", EASINGS, "ease-in-out"),
];

const KEN_BURNS: &[ParamSpec] = &[
    num("duration", 1.0, 30.0, 0.5, 6.0),
    num("zoom", 1.05, 3.0, 0.05, 1.3),
    choice("focus", &["center", "top", "bottom", "left", "right"], "center"),
    choice("way", &["in", "out"], "in"),
];

const FULL: &[ParamSpec] = &[num("duration", 0.5, 30.0, 0.5, 3.0)];

pub const CATALOG: &[TemplateInfo] = &[
    TemplateInfo { id: "comic", params: COMIC },
    TemplateInfo { id: "scroll", params: SCROLL },
    TemplateInfo { id: "bounce", params: BOUNCE },
    TemplateInfo { id: "ken-burns", params: KEN_BURNS },
    TemplateInfo { id: "full", params: FULL },
];

/// Lector de parámetros: valores del proyecto validados contra el catálogo, con su valor por
/// defecto si faltan o no son válidos. Así un `.imgx` editado a mano nunca rompe la generación.
struct Params<'a> {
    v: &'a Value,
    specs: &'static [ParamSpec],
}

impl Params<'_> {
    fn spec(&self, key: &str) -> &'static ParamKind {
        &self.specs.iter().find(|s| s.key == key).unwrap_or_else(|| panic!("parámetro {key} sin declarar")).kind
    }
    fn num(&self, key: &str) -> f64 {
        let ParamKind::Number { min, max, default, .. } = *self.spec(key) else { unreachable!() };
        self.v.get(key).and_then(Value::as_f64).filter(|x| x.is_finite()).unwrap_or(default).clamp(min, max)
    }
    fn choice(&self, key: &str) -> &'static str {
        let ParamKind::Choice { options, default } = self.spec(key) else { unreachable!() };
        let v = self.v.get(key).and_then(Value::as_str);
        options.iter().copied().find(|o| Some(*o) == v).unwrap_or(default)
    }
    fn flag(&self, key: &str) -> bool {
        let ParamKind::Bool { default } = *self.spec(key) else { unreachable!() };
        self.v.get(key).and_then(Value::as_bool).unwrap_or(default)
    }
}

fn info(id: &str) -> Result<&'static TemplateInfo, TemplateError> {
    CATALOG.iter().find(|t| t.id == id).ok_or_else(|| TemplateError::Unknown(id.into()))
}

// ---------------------------------------------------------------------------------------------
// Generación

pub fn generate(tpl: &TemplateRef, ctx: &Ctx) -> Result<Timeline, TemplateError> {
    let p = Params { v: &tpl.params, specs: info(&tpl.id)?.params };
    let keys = match tpl.id.as_str() {
        "comic" => comic(&p, ctx),
        "scroll" => scroll(&p, ctx),
        "bounce" => bounce(&p, ctx),
        "ken-burns" => ken_burns(&p, ctx),
        _ => {
            let mut k = Keys::new(Camera::FIT);
            k.hold(p.num("duration"));
            k
        }
    };
    Ok(keys.into_timeline())
}

/// Viñetas que recorrerá `comic`, ya en orden de lectura (para dibujarlas sobre la imagen).
pub fn comic_panels(tpl: &TemplateRef, ctx: &Ctx) -> Vec<Rect> {
    if tpl.id != "comic" {
        return Vec::new();
    }
    let p = Params { v: &tpl.params, specs: COMIC };
    panels(&p, ctx)
}

/// Lado por el que llega la siguiente imagen según la plantilla (para la cámara continua).
pub fn reading_direction(tpl: Option<&TemplateRef>) -> Direction {
    let Some(tpl) = tpl else { return Direction::Right };
    match tpl.id.as_str() {
        "comic" => match (Params { v: &tpl.params, specs: COMIC }).choice("dir") {
            "rtl" => Direction::Left,
            _ => Direction::Right,
        },
        "scroll" => match (Params { v: &tpl.params, specs: SCROLL }).choice("dir") {
            "up" => Direction::Up,
            "right" => Direction::Right,
            "left" => Direction::Left,
            _ => Direction::Down,
        },
        _ => Direction::Right,
    }
}

/// Constructor de keyframes: el easing de cada clave rige el tramo hacia la siguiente.
pub(crate) struct Keys {
    pub(crate) keys: Vec<Keyframe>,
}

impl Keys {
    pub(crate) fn new(pose: Camera) -> Self {
        Self { keys: vec![Keyframe { t: 0.0, pose, easing: Easing::Linear }] }
    }
    pub(crate) fn last(&self) -> &Keyframe {
        self.keys.last().unwrap()
    }
    pub(crate) fn go(&mut self, pose: Camera, dur: f64, easing: Easing) {
        let t = self.last().t + dur.max(0.0);
        self.keys.last_mut().unwrap().easing = easing;
        self.keys.push(Keyframe { t, pose, easing: Easing::Linear });
    }
    pub(crate) fn hold(&mut self, dur: f64) {
        if dur > 0.0 {
            self.go(self.last().pose, dur, Easing::Linear);
        }
    }
    pub(crate) fn into_timeline(self) -> Timeline {
        let duration = self.last().t;
        Timeline {
            camera: vec![ClipInstance {
                name: None,
                start: 0.0,
                duration,
                clip: ClipSource::Inline(Clip::Keyframes { keys: self.keys }),
                trim_in: 0.0,
                trim_out: 0.0,
            }],
            ..Default::default()
        }
    }
}

/// Mantiene el centro de forma que no se vea fuera de la imagen si esta cubre el viewport.
pub(crate) fn clamp_center(mut cam: Camera, ctx: &Ctx) -> Camera {
    let s = Camera::fit_scale(ctx.img, ctx.vp()) * cam.zoom;
    let vis = (ctx.vp().0 / (s * ctx.img.0), ctx.vp().1 / (s * ctx.img.1));
    let clamp = |c: f64, v: f64| if v >= 1.0 { 0.5 } else { c.clamp(v / 2.0, 1.0 - v / 2.0) };
    cam.cx = clamp(cam.cx, vis.0);
    cam.cy = clamp(cam.cy, vis.1);
    cam
}

/// Pose que encuadra `r` con un margen (fracción del rectángulo por cada lado).
pub fn frame_rect(r: Rect, ctx: &Ctx, margin: f64, max_zoom: f64) -> Camera {
    let fit = Camera::fit_scale(ctx.img, ctx.vp());
    let k = 1.0 + 2.0 * margin;
    let s = (ctx.vp().0 / (r.w * ctx.img.0 * k)).min(ctx.vp().1 / (r.h * ctx.img.1 * k));
    let zoom = (s / fit).clamp(1.0, max_zoom.max(1.0));
    clamp_center(Camera { cx: r.x + r.w / 2.0, cy: r.y + r.h / 2.0, zoom, rot: 0.0 }, ctx)
}

fn easing_named(name: &str) -> Easing {
    match name {
        "linear" => Easing::Linear,
        "bounce" => Easing::Bounce,
        "elastic" => Easing::Elastic,
        _ => Easing::EaseInOut,
    }
}

/// Arranque y frenada suaves para recorridos largos.
const GLIDE: Easing = Easing::CubicBezier { x1: 0.35, y1: 0.0, x2: 0.65, y2: 1.0 };

fn scroll(p: &Params, ctx: &Ctx) -> Keys {
    let dir = p.choice("dir");
    let vertical = matches!(dir, "down" | "up");
    let fit = Camera::fit_scale(ctx.img, ctx.vp());
    // Escala que hace que la imagen ocupe `fill` del viewport en el eje transversal.
    let s = if vertical { ctx.vp().0 * p.num("fill") / ctx.img.0 } else { ctx.vp().1 * p.num("fill") / ctx.img.1 };
    let zoom = (s / fit).max(1.0);
    // Fracción de la imagen visible a lo largo del recorrido.
    let vis = if vertical { ctx.vp().1 / (fit * zoom * ctx.img.1) } else { ctx.vp().0 / (fit * zoom * ctx.img.0) };
    if vis >= 0.999 {
        // No hay nada que recorrer: se muestra entera.
        let mut k = Keys::new(Camera::FIT);
        k.hold((p.num("pauseStart") + p.num("pauseEnd")).max(1.0));
        return k;
    }
    let (from, to) = (vis / 2.0, 1.0 - vis / 2.0);
    let (a, b) = if matches!(dir, "down" | "right") { (from, to) } else { (to, from) };
    let pose = |c: f64| {
        let cam = if vertical { Camera { cx: 0.5, cy: c, zoom, rot: 0.0 } } else { Camera { cx: c, cy: 0.5, zoom, rot: 0.0 } };
        clamp_center(cam, ctx)
    };
    let screens = (1.0 - vis) / vis;
    let mut k = Keys::new(pose(a));
    k.hold(p.num("pauseStart"));
    k.go(pose(b), screens / p.num("speed"), GLIDE);
    k.hold(p.num("pauseEnd"));
    k
}

fn panels(p: &Params, ctx: &Ctx) -> Vec<Rect> {
    let rtl = p.choice("dir") == "rtl";
    let detected = ctx.panels.filter(|v| if ctx.manual { !v.is_empty() } else { v.len() >= 2 && p.choice("mode") == "auto" });
    let rects = match detected {
        Some(v) => v.to_vec(),
        None => grid(p.num("rows") as usize, p.num("cols") as usize),
    };
    reading_order(rects, rtl)
}

fn comic(p: &Params, ctx: &Ctx) -> Keys {
    let (hold, mv) = (p.num("hold"), p.num("move"));
    let overview = p.flag("overview");
    let mut k = Keys::new(Camera::FIT);
    let list = panels(p, ctx);
    for (i, r) in list.iter().enumerate() {
        let pose = frame_rect(*r, ctx, p.num("margin"), p.num("maxZoom"));
        if i == 0 && !overview {
            k = Keys::new(pose);
        } else {
            if i == 0 {
                k.hold(hold * 0.6);
            }
            k.go(pose, mv, Easing::EaseInOut);
        }
        k.hold(hold);
    }
    if overview {
        k.go(Camera::FIT, mv, Easing::EaseInOut);
        k.hold(0.4);
    }
    k
}

fn bounce(p: &Params, ctx: &Ctx) -> Keys {
    let e = easing_named(p.choice("easing"));
    let quarter = p.num("period") / 2.0;
    let lift = p.num("lift");
    let mut k = Keys::new(Camera::FIT);
    for i in 0..p.num("cycles") as usize {
        // Ciclos alternos: sube y acerca; luego baja y acerca.
        let dy = if i % 2 == 0 { -lift } else { lift };
        let pose = clamp_center(Camera { cx: 0.5, cy: 0.5 + dy, zoom: p.num("zoom"), rot: 0.0 }, ctx);
        k.go(pose, quarter, e);
        k.go(Camera::FIT, quarter, e);
    }
    k.hold(0.3);
    k
}

fn ken_burns(p: &Params, ctx: &Ctx) -> Keys {
    let (cx, cy) = match p.choice("focus") {
        "top" => (0.5, 0.0),
        "bottom" => (0.5, 1.0),
        "left" => (0.0, 0.5),
        "right" => (1.0, 0.5),
        _ => (0.5, 0.5),
    };
    let near = clamp_center(Camera { cx, cy, zoom: p.num("zoom"), rot: 0.0 }, ctx);
    let (a, b) = if p.choice("way") == "out" { (near, Camera::FIT) } else { (Camera::FIT, near) };
    let mut k = Keys::new(a);
    k.go(b, p.num("duration"), Easing::CubicBezier { x1: 0.3, y1: 0.0, x2: 0.7, y2: 1.0 });
    k
}

// ---------------------------------------------------------------------------------------------
// Viñetas

/// Rejilla de `rows` × `cols` celdas iguales.
pub fn grid(rows: usize, cols: usize) -> Vec<Rect> {
    let (rows, cols) = (rows.max(1), cols.max(1));
    let (w, h) = (1.0 / cols as f64, 1.0 / rows as f64);
    (0..rows).flat_map(|r| (0..cols).map(move |c| Rect { x: c as f64 * w, y: r as f64 * h, w, h })).collect()
}

/// Ordena viñetas en orden de lectura con cortes recursivos XY: primero por franjas
/// horizontales (de arriba abajo) y, dentro de cada una, por columnas (D→I si `rtl`).
/// Así una viñeta alta que ocupa dos filas se lee antes o después de su columna vecina entera.
pub fn reading_order(rects: Vec<Rect>, rtl: bool) -> Vec<Rect> {
    let mut out = Vec::with_capacity(rects.len());
    cut(rects, rtl, &mut out, 0);
    out
}

/// Solape tolerado entre viñetas al buscar cortes (las detectadas no son exactas).
const CUT_TOLERANCE: f64 = 0.01;

fn cut(rects: Vec<Rect>, rtl: bool, out: &mut Vec<Rect>, depth: usize) {
    if rects.len() <= 1 || depth > 16 {
        out.extend(rects);
        return;
    }
    if let Some(rows) = split(&rects, |r| (r.y, r.bottom())) {
        for g in rows {
            cut(g, rtl, out, depth + 1);
        }
    } else if let Some(mut cols) = split(&rects, |r| (r.x, r.right())) {
        if rtl {
            cols.reverse();
        }
        for g in cols {
            cut(g, rtl, out, depth + 1);
        }
    } else {
        // Sin cortes limpios (disposición irregular): por filas aproximadas.
        let mut v = rects;
        let key = |r: &Rect| ((r.y + r.h / 2.0) * 20.0).round();
        v.sort_by(|a, b| {
            key(a).total_cmp(&key(b)).then(if rtl { b.x.total_cmp(&a.x) } else { a.x.total_cmp(&b.x) })
        });
        out.extend(v);
    }
}

/// Agrupa por intervalos que no se solapan a lo largo de un eje. `None` si sale un solo grupo.
fn split(rects: &[Rect], span: impl Fn(&Rect) -> (f64, f64)) -> Option<Vec<Vec<Rect>>> {
    let mut v = rects.to_vec();
    v.sort_by(|a, b| span(a).0.total_cmp(&span(b).0));
    let mut groups: Vec<Vec<Rect>> = Vec::new();
    let mut end = f64::NEG_INFINITY;
    for r in v {
        let (s, e) = span(&r);
        if groups.is_empty() || s >= end - CUT_TOLERANCE {
            groups.push(vec![r]);
            end = e;
        } else {
            groups.last_mut().unwrap().push(r);
            end = end.max(e);
        }
    }
    (groups.len() > 1).then_some(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::NoClips;
    use serde_json::json;

    const PAGE: Ctx = Ctx { img: (2000.0, 3000.0), aspect: 16.0 / 9.0, panels: None, manual: false };

    fn tpl(id: &str, params: Value) -> TemplateRef {
        TemplateRef { id: id.into(), params }
    }

    fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn orden_de_lectura_con_vineta_alta() {
        // L ocupa dos filas a la izquierda; A y B se apilan a la derecha.
        let l = r(0.0, 0.0, 0.5, 1.0);
        let a = r(0.5, 0.0, 0.5, 0.5);
        let b = r(0.5, 0.5, 0.5, 0.5);
        assert_eq!(reading_order(vec![b, l, a], false), vec![l, a, b]);
        assert_eq!(reading_order(vec![b, l, a], true), vec![a, b, l]);
    }

    #[test]
    fn rejilla_dcha_a_izda() {
        let order = reading_order(grid(2, 2), true);
        assert_eq!(order[0], r(0.5, 0.0, 0.5, 0.5));
        assert_eq!(order[1], r(0.0, 0.0, 0.5, 0.5));
        assert_eq!(order[2], r(0.5, 0.5, 0.5, 0.5));
    }

    #[test]
    fn encuadre_cabe_y_no_se_sale() {
        let cell = r(0.5, 0.0, 0.5, 1.0 / 3.0);
        let cam = frame_rect(cell, &PAGE, 0.0, 10.0);
        let vp = (PAGE.aspect, 1.0);
        // Las esquinas de la viñeta quedan dentro del viewport.
        let m = cam.to_viewport(PAGE.img, vp);
        for (x, y) in [(cell.x, cell.y), (cell.right(), cell.bottom())] {
            let (px, py) = (x * PAGE.img.0, y * PAGE.img.1);
            let (sx, sy) = (m[0] * px + m[2] * py + m[4], m[1] * px + m[3] * py + m[5]);
            assert!((-1e-9..=vp.0 + 1e-9).contains(&sx) && (-1e-9..=vp.1 + 1e-9).contains(&sy), "{sx},{sy}");
        }
        assert!(cam.zoom > 1.0);
    }

    #[test]
    fn comic_recorre_todas_las_vinetas() {
        let t = tpl("comic", json!({"mode": "grid", "rows": 3, "cols": 2, "hold": 1.0, "move": 0.5, "overview": false}));
        let tl = generate(&t, &PAGE).unwrap();
        // 6 viñetas: 6 esperas de 1 s + 5 movimientos de 0,5 s.
        assert!((tl.duration() - 8.5).abs() < 1e-9, "{}", tl.duration());
        // Empieza en la viñeta superior derecha (D→I por defecto).
        assert!(tl.evaluate(0.0, &NoClips).cx > 0.5);
    }

    #[test]
    fn comic_usa_las_detectadas_si_hay() {
        let det = [r(0.0, 0.0, 1.0, 0.5), r(0.0, 0.5, 1.0, 0.5)];
        let ctx = Ctx { panels: Some(&det), ..PAGE };
        let t = tpl("comic", json!({}));
        assert_eq!(comic_panels(&t, &ctx), det.to_vec());
        let one = [r(0.0, 0.0, 1.0, 1.0)];
        let ctx = Ctx { panels: Some(&one), ..PAGE };
        assert_eq!(comic_panels(&t, &ctx).len(), 6, "con una sola detectada se usa la rejilla");
        let ctx = Ctx { panels: Some(&one), manual: true, ..PAGE };
        let grid = tpl("comic", serde_json::json!({"mode": "grid"}));
        assert_eq!(comic_panels(&grid, &ctx), one.to_vec(), "las dibujadas a mano mandan siempre");
    }

    #[test]
    fn scroll_baja_de_arriba_abajo() {
        let tall = Ctx { img: (800.0, 8000.0), ..PAGE };
        let tl = generate(&tpl("scroll", json!({"pauseStart": 0, "pauseEnd": 0})), &tall).unwrap();
        let (a, b) = (tl.evaluate(0.0, &NoClips), tl.evaluate(tl.duration(), &NoClips));
        assert!(a.cy < 0.1 && b.cy > 0.9, "{a:?} {b:?}");
        assert!((a.zoom - b.zoom).abs() < 1e-12);
        // Ajustado al ancho: 800 px ocupan todo el ancho del viewport.
        let s = Camera::fit_scale(tall.img, (tall.aspect, 1.0)) * a.zoom;
        assert!((s * 800.0 - tall.aspect).abs() < 1e-9);
    }

    #[test]
    fn scroll_de_imagen_que_cabe() {
        let wide = Ctx { img: (4000.0, 1000.0), ..PAGE };
        let tl = generate(&tpl("scroll", json!({})), &wide).unwrap();
        assert_eq!(tl.evaluate(1.0, &NoClips), Camera::FIT);
    }

    #[test]
    fn parametros_invalidos_usan_el_valor_por_defecto() {
        let t = tpl("bounce", json!({"cycles": "muchos", "zoom": 99, "easing": "rarísimo"}));
        let tl = generate(&t, &PAGE).unwrap();
        assert!((tl.duration() - (2.0 * 2.0 + 0.3)).abs() < 1e-9);
        let peak = tl.evaluate(1.0, &NoClips);
        assert!((peak.zoom - 4.0).abs() < 1e-9, "zoom limitado al máximo del catálogo");
        assert!(generate(&tpl("nada", json!({})), &PAGE).is_err());
    }

    #[test]
    fn catalogo_serializa() {
        let s = serde_json::to_string(CATALOG).unwrap();
        assert!(s.contains(r#""key":"rows","kind":"number""#), "{s}");
        assert!(s.contains(r#""when":["mode","grid"]"#));
    }

    #[test]
    fn direccion_de_lectura() {
        assert_eq!(reading_direction(Some(&tpl("comic", json!({})))), Direction::Left);
        assert_eq!(reading_direction(Some(&tpl("comic", json!({"dir": "ltr"})))), Direction::Right);
        assert_eq!(reading_direction(Some(&tpl("scroll", json!({})))), Direction::Down);
        assert_eq!(reading_direction(None), Direction::Right);
    }
}
