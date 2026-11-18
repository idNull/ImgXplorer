//! Planos de «director»: quien monta (una persona o una IA) dice qué encuadrar y cuánto tiempo, y
//! aquí se calculan las poses exactas para la proporción del vídeo. El resultado son keyframes
//! normales, editables luego en el editor.
//!
//! Cada plano: llega a su encuadre en `move` segundos con su curva, se queda `hold` segundos
//! (opcionalmente acercándose un poco, `push`) y pasa al siguiente.

use crate::camera::Camera;
use crate::easing::Easing;
use crate::schema::{Rect, Timeline};
use crate::templates::{Ctx, Keys, clamp_center, frame_rect};
use serde::{Deserialize, Serialize};

/// Un plano.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    /// Qué encuadrar (normalizado). Sin rectángulo, la imagen entera.
    #[serde(default)]
    pub rect: Option<Rect>,
    /// Segundos quieto en el encuadre.
    pub hold: f64,
    /// Segundos para llegar desde el plano anterior.
    #[serde(default = "default_move", rename = "move")]
    pub travel: f64,
    /// Curva de la llegada.
    #[serde(default)]
    pub easing: Easing,
    /// Aire alrededor del rectángulo (fracción por lado).
    #[serde(default = "default_margin")]
    pub margin: f64,
    /// Acercamiento extra sobre el encuadre justo (1 = justo).
    #[serde(default = "one")]
    pub zoom: f64,
    /// Giro en grados (horario).
    #[serde(default)]
    pub rot: f64,
    /// Acercamiento lento durante el tiempo quieto (1 = ninguno; 1,05 = un 5 %).
    #[serde(default = "one")]
    pub push: f64,
    /// Zoom máximo permitido al encuadrar (los detalles pequeños piden más).
    #[serde(default = "default_max_zoom")]
    pub max_zoom: f64,
}

fn default_move() -> f64 {
    0.8
}
fn default_margin() -> f64 {
    0.04
}
fn one() -> f64 {
    1.0
}
fn default_max_zoom() -> f64 {
    6.0
}

/// Cómo empieza y acaba la imagen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Edge {
    /// Se ve la página entera antes del primer plano (o después del último).
    Overview,
    /// Se empieza directamente en el primer plano (o se acaba en el último).
    #[default]
    Direct,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotPlan {
    pub shots: Vec<Shot>,
    #[serde(default)]
    pub start: Edge,
    #[serde(default)]
    pub end: Edge,
    /// Segundos de página entera al empezar o acabar con `overview`.
    #[serde(default = "default_overview")]
    pub overview_hold: f64,
}

fn default_overview() -> f64 {
    1.0
}

/// Cuándo ocurre cada plano dentro de la imagen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotTimes {
    /// Empieza a moverse hacia el encuadre.
    pub arrive: f64,
    /// Ya está encuadrado (empieza el tiempo quieto).
    pub hold: f64,
    /// Termina el tiempo quieto.
    pub end: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Planned {
    pub timeline: Timeline,
    pub shots: Vec<ShotTimes>,
    pub duration: f64,
}

/// Límites para que una entrada disparatada no rompa nada.
const MAX_HOLD: f64 = 120.0;
const MAX_MOVE: f64 = 20.0;

fn finite(v: f64, default: f64) -> f64 {
    if v.is_finite() { v } else { default }
}

/// Rectángulo dentro de la imagen y con tamaño mínimo.
fn clean_rect(r: Rect) -> Rect {
    let x = finite(r.x, 0.0).clamp(0.0, 0.99);
    let y = finite(r.y, 0.0).clamp(0.0, 0.99);
    let w = finite(r.w, 1.0).clamp(0.01, 1.0 - x);
    let h = finite(r.h, 1.0).clamp(0.01, 1.0 - y);
    Rect { x, y, w, h }
}

/// Pose del plano (sin el acercamiento lento).
fn pose_of(s: &Shot, ctx: &Ctx) -> Camera {
    let base = match s.rect {
        Some(r) => frame_rect(clean_rect(r), ctx, finite(s.margin, 0.04).clamp(0.0, 0.5), finite(s.max_zoom, 6.0).clamp(1.0, 64.0)),
        None => Camera::FIT,
    };
    let zoom = (base.zoom * finite(s.zoom, 1.0).clamp(0.2, 16.0)).clamp(0.05, 64.0);
    let cam = Camera { zoom, rot: finite(s.rot, 0.0).clamp(-360.0, 360.0), ..base };
    // Con giro no se recoloca el centro (la cuenta de bordes es para cámara recta).
    if cam.rot == 0.0 { clamp_center(cam, ctx) } else { cam }
}

/// Keyframes de un plan de planos para una imagen y una proporción de vídeo.
pub fn plan(p: &ShotPlan, ctx: &Ctx) -> Planned {
    let mut times = Vec::with_capacity(p.shots.len());
    let overview = finite(p.overview_hold, 1.0).clamp(0.0, MAX_HOLD);
    let mut k: Option<Keys> = None;
    if p.start == Edge::Overview || p.shots.is_empty() {
        let mut keys = Keys::new(Camera::FIT);
        keys.hold(if p.shots.is_empty() { overview.max(1.0) } else { overview });
        k = Some(keys);
    }
    for s in &p.shots {
        let pose = pose_of(s, ctx);
        let hold = finite(s.hold, 2.0).clamp(0.0, MAX_HOLD);
        let travel = finite(s.travel, 0.8).clamp(0.0, MAX_MOVE);
        let arrive;
        match k.as_mut() {
            None => {
                // Primer plano sin vista general: se empieza ya encuadrado.
                k = Some(Keys::new(pose));
                arrive = 0.0;
            }
            Some(keys) => {
                arrive = keys.last().t;
                if travel > 0.0 {
                    keys.go(pose, travel, s.easing);
                } else {
                    // Corte seco: el mismo instante no puede tener dos poses; casi a la vez.
                    keys.go(pose, 0.001, Easing::Step);
                }
            }
        }
        let keys = k.as_mut().unwrap();
        let start_hold = keys.last().t;
        let push = finite(s.push, 1.0).clamp(0.5, 2.0);
        if hold > 0.0 {
            if (push - 1.0).abs() > 1e-6 {
                let end = Camera { zoom: (pose.zoom * push).clamp(0.05, 64.0), ..pose };
                let end = if end.rot == 0.0 { clamp_center(end, ctx) } else { end };
                keys.go(end, hold, Easing::Linear);
            } else {
                keys.hold(hold);
            }
        }
        times.push(ShotTimes { arrive, hold: start_hold, end: keys.last().t });
    }
    let mut keys = k.expect("hay al menos la vista general");
    if p.end == Edge::Overview && !p.shots.is_empty() {
        let travel = p.shots.last().map(|s| finite(s.travel, 0.8).clamp(0.0, MAX_MOVE)).unwrap_or(0.8).max(0.3);
        keys.go(Camera::FIT, travel, Easing::EaseInOut);
        keys.hold(overview);
    }
    // Una imagen sin duración no se vería: al menos una décima.
    if keys.last().t < 0.1 {
        keys.hold(0.1 - keys.last().t);
    }
    let duration = keys.last().t;
    let mut timeline = keys.into_timeline();
    if let Some(c) = timeline.camera.first_mut() {
        c.name = Some("Planos".into());
    }
    Planned { timeline, shots: times, duration }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::NoClips;

    fn ctx(aspect: f64) -> Ctx<'static> {
        Ctx { img: (1000.0, 1500.0), aspect, panels: None, manual: false }
    }

    fn shot(rect: Option<Rect>, hold: f64) -> Shot {
        serde_json::from_value(serde_json::json!({ "rect": rect, "hold": hold })).unwrap()
    }

    const TOP: Rect = Rect { x: 0.05, y: 0.05, w: 0.9, h: 0.3 };
    const DETAIL: Rect = Rect { x: 0.6, y: 0.6, w: 0.1, h: 0.08 };

    #[test]
    fn tiempos_de_cada_plano() {
        let p = ShotPlan { shots: vec![shot(Some(TOP), 3.0), shot(Some(DETAIL), 2.0)], start: Edge::Overview, end: Edge::Overview, overview_hold: 1.0 };
        let out = plan(&p, &ctx(16.0 / 9.0));
        // 1 s de página, 0,8 s de llegada, 3 s, 0,8 s, 2 s, 0,8 s de vuelta y 1 s de página.
        assert_eq!(out.shots[0], ShotTimes { arrive: 1.0, hold: 1.8, end: 4.8 });
        assert!((out.shots[1].hold - 5.6).abs() < 1e-9 && (out.shots[1].end - 7.6).abs() < 1e-9);
        assert!((out.duration - 9.4).abs() < 1e-9);
        assert!((out.timeline.duration() - out.duration).abs() < 1e-9);
    }

    #[test]
    fn encuadra_la_vineta_en_16_9_y_en_vertical() {
        for aspect in [16.0 / 9.0, 9.0 / 16.0] {
            let c = ctx(aspect);
            let out = plan(&ShotPlan { shots: vec![shot(Some(TOP), 2.0)], start: Edge::Direct, end: Edge::Direct, overview_hold: 1.0 }, &c);
            let pose = out.timeline.evaluate(1.0, &NoClips);
            let want = frame_rect(TOP, &c, 0.04, 6.0);
            assert!((pose.cx - want.cx).abs() + (pose.cy - want.cy).abs() + (pose.zoom - want.zoom).abs() < 1e-9, "proporción {aspect}");
            assert!((pose.cx - 0.5).abs() < 1e-9);
            assert!(pose.zoom >= 1.0);
        }
    }

    #[test]
    fn un_detalle_pequeno_acerca_mucho_mas() {
        let c = ctx(16.0 / 9.0);
        let p = ShotPlan { shots: vec![shot(Some(TOP), 1.0), shot(Some(DETAIL), 1.0)], start: Edge::Direct, end: Edge::Direct, overview_hold: 1.0 };
        let out = plan(&p, &c);
        let a = out.timeline.evaluate(out.shots[0].hold + 0.5, &NoClips);
        let b = out.timeline.evaluate(out.shots[1].hold + 0.5, &NoClips);
        assert!(b.zoom > a.zoom * 2.0, "{} → {}", a.zoom, b.zoom);
        assert!((b.cx - 0.65).abs() < 0.02 && (b.cy - 0.64).abs() < 0.02);
    }

    #[test]
    fn la_curva_y_el_acercamiento_lento() {
        let c = ctx(16.0 / 9.0);
        let mut s = shot(Some(TOP), 4.0);
        s.easing = Easing::Linear;
        s.push = 1.1;
        let out = plan(&ShotPlan { shots: vec![s], start: Edge::Overview, end: Edge::Direct, overview_hold: 0.0 }, &c);
        let t = out.shots[0];
        let mid = out.timeline.evaluate((t.arrive + t.hold) / 2.0, &NoClips);
        // Lineal: a mitad de camino el centro está a medias entre la página y la viñeta.
        let target = frame_rect(TOP, &c, 0.04, 6.0);
        assert!((mid.cy - (0.5 + target.cy) / 2.0).abs() < 0.01, "{}", mid.cy);
        let a = out.timeline.evaluate(t.hold, &NoClips);
        let b = out.timeline.evaluate(t.end, &NoClips);
        assert!((b.zoom / a.zoom - 1.1).abs() < 1e-6);
    }

    #[test]
    fn corte_seco_y_entradas_raras() {
        let c = ctx(16.0 / 9.0);
        let json = serde_json::json!({
            "shots": [
                { "rect": { "x": -1, "y": 2, "w": 9, "h": 0 }, "hold": 1e9 },
                { "hold": 1, "move": 0, "zoom": 1.5, "easing": { "type": "bounce" } }
            ],
            "start": "overview"
        });
        let p: ShotPlan = serde_json::from_value(json).unwrap();
        let out = plan(&p, &c);
        assert!(out.duration.is_finite() && out.duration <= 1.0 + 0.8 + MAX_HOLD + 0.01 + 1.0);
        let t = out.shots[1];
        assert!(t.hold - t.arrive < 0.01, "corte: {t:?}");
        let pose = out.timeline.evaluate(t.hold + 0.5, &NoClips);
        assert!((pose.zoom - 1.5).abs() < 1e-9);
    }

    #[test]
    fn sin_planos_se_ve_la_pagina() {
        let out = plan(&ShotPlan { shots: vec![], start: Edge::Direct, end: Edge::Direct, overview_hold: 2.0 }, &ctx(1.0));
        assert!((out.duration - 2.0).abs() < 1e-9);
        assert_eq!(out.timeline.evaluate(1.0, &NoClips), Camera::FIT);
    }
}
