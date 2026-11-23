//! Transiciones entre imágenes: dado el progreso `u` (0..1), cómo se pinta cada imagen.
//! Lo usan la reproducción (WASM) y, más adelante, el render MP4.

use crate::camera::{Affine, Camera};
use crate::easing::Easing;
use crate::schema::{Direction, Transition, TransitionKind};

/// Una imagen con su pose.
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub img: (f64, f64),
    pub pose: Camera,
}

/// Cómo pintar una imagen: matriz imagen→viewport y opacidad.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layer {
    pub m: Affine,
    pub alpha: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Imagen saliente.
    pub a: Layer,
    /// Imagen entrante.
    pub b: Layer,
    /// `b` se mezcla con `a` (fundido real, con `b.alpha` como peso) en vez de pintarse encima.
    pub mix: bool,
}

/// `m ∘ n`: primero `n`, luego `m`.
pub fn compose(m: Affine, n: Affine) -> Affine {
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}

fn translated(mut m: Affine, dx: f64, dy: f64) -> Affine {
    m[4] += dx;
    m[5] += dy;
    m
}

/// Fotograma de la transición `tr` en el progreso `u`. `fallback` es la dirección a usar si la
/// transición no la fija (se deduce de la plantilla de la imagen saliente).
pub fn frame(tr: &Transition, fallback: Direction, u: f64, a: &View, b: &View, vp: (f64, f64)) -> Frame {
    let u = u.clamp(0.0, 1.0);
    let e = Easing::EaseInOut.apply(u);
    let ma = a.pose.to_viewport(a.img, vp);
    let mb = b.pose.to_viewport(b.img, vp);
    let layer = |m, alpha| Layer { m, alpha };
    match tr.kind {
        TransitionKind::Cut => Frame { a: layer(ma, 0.0), b: layer(mb, 1.0), mix: false },
        TransitionKind::Fade => Frame { a: layer(ma, 1.0), b: layer(mb, e), mix: true },
        TransitionKind::Dip => {
            // Primera mitad: A se apaga; segunda: B se enciende (sobre el fondo negro).
            let (fa, fb) = if u < 0.5 { (1.0 - Easing::EaseInOut.apply(u * 2.0), 0.0) } else { (0.0, Easing::EaseInOut.apply(u * 2.0 - 1.0)) };
            Frame { a: layer(ma, fa), b: layer(mb, fb), mix: false }
        }
        TransitionKind::Zoom => {
            let a_scale = 1.0 + ZOOM_OUT * e;
            let b_scale = ZOOM_IN + (1.0 - ZOOM_IN) * e;
            Frame { a: layer(scaled(ma, a_scale, vp), 1.0), b: layer(scaled(mb, b_scale, vp), e), mix: true }
        }
        TransitionKind::Slide { dir } => {
            let (dx, dy) = offset(dir, vp);
            Frame {
                a: layer(translated(ma, -dx * e, -dy * e), 1.0),
                b: layer(translated(mb, dx * (1.0 - e), dy * (1.0 - e)), 1.0),
                mix: false,
            }
        }
        TransitionKind::ContinuousCamera { dir } => continuous(dir.unwrap_or(fallback), e, a, b, vp),
    }
}

/// Acercar: cuánto crece la saliente y de qué tamaño llega la entrante.
const ZOOM_OUT: f64 = 0.3;
const ZOOM_IN: f64 = 0.8;

/// `m` escalada `s` veces desde el centro de la pantalla.
fn scaled(m: Affine, s: f64, vp: (f64, f64)) -> Affine {
    let (cx, cy) = (vp.0 / 2.0, vp.1 / 2.0);
    compose([s, 0.0, 0.0, s, cx * (1.0 - s), cy * (1.0 - s)], m)
}

/// Desplazamiento de pantalla completa hacia el lado por el que llega la siguiente imagen.
fn offset(dir: Direction, vp: (f64, f64)) -> (f64, f64) {
    match dir {
        Direction::Right => (vp.0, 0.0),
        Direction::Left => (-vp.0, 0.0),
        Direction::Down => (0.0, vp.1),
        Direction::Up => (0.0, -vp.1),
    }
}

/// Cámara continua: `b` se coloca junto a `a` (misma altura si van en horizontal, mismo ancho
/// si van en vertical) y la cámara viaja por ese "mundo", en píxeles de `a`.
fn continuous(dir: Direction, e: f64, a: &View, b: &View, vp: (f64, f64)) -> Frame {
    let (aw, ah) = a.img;
    let (bw, bh) = b.img;
    let horizontal = matches!(dir, Direction::Left | Direction::Right);
    let k = if horizontal { ah / bh } else { aw / bw };
    let gap = 0.03 * if horizontal { aw } else { ah };
    let (ox, oy) = match dir {
        Direction::Right => (aw + gap, 0.0),
        Direction::Left => (-gap - bw * k, 0.0),
        Direction::Down => (0.0, ah + gap),
        Direction::Up => (0.0, -gap - bh * k),
    };
    // B en coordenadas del mundo.
    let place: Affine = [k, 0.0, 0.0, k, ox, oy];

    let fit_a = Camera::fit_scale(a.img, vp);
    let start = (a.pose.cx * aw, a.pose.cy * ah, a.pose.zoom);
    let end = (
        ox + b.pose.cx * bw * k,
        oy + b.pose.cy * bh * k,
        // Misma escala de pantalla que tendría B con su propia pose.
        Camera::fit_scale(b.img, vp) * b.pose.zoom / k / fit_a,
    );
    // Si el viaje es largo, la cámara se aleja un poco a mitad de camino para dar contexto.
    let dist = ((end.0 - start.0).powi(2) + (end.1 - start.1).powi(2)).sqrt() * fit_a * start.2 / vp.0.max(vp.1);
    let hop = (dist * 0.25).min(0.45);
    let zoom = (start.2.ln() + (end.2.ln() - start.2.ln()) * e).exp() / (1.0 + hop * (std::f64::consts::PI * e).sin());
    let world = Camera {
        cx: (start.0 + (end.0 - start.0) * e) / aw,
        cy: (start.1 + (end.1 - start.1) * e) / ah,
        zoom,
        rot: a.pose.rot + (b.pose.rot - a.pose.rot) * e,
    };
    let ma = world.to_viewport(a.img, vp);
    Frame { a: Layer { m: ma, alpha: 1.0 }, b: Layer { m: compose(ma, place), alpha: 1.0 }, mix: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VP: (f64, f64) = (1600.0, 900.0);

    fn close(a: Affine, b: Affine) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
    }

    fn views() -> (View, View) {
        (
            View { img: (2000.0, 3000.0), pose: Camera { cx: 0.2, cy: 0.8, zoom: 2.5, rot: 0.0 } },
            View { img: (1000.0, 1600.0), pose: Camera { cx: 0.7, cy: 0.1, zoom: 3.0, rot: 0.0 } },
        )
    }

    #[test]
    fn fundido_a_negro_pasa_por_negro() {
        let (a, b) = views();
        let tr = Transition { kind: TransitionKind::Dip, duration: 1.0 };
        let at = |u| frame(&tr, Direction::Right, u, &a, &b, VP);
        assert_eq!((at(0.0).a.alpha, at(0.0).b.alpha), (1.0, 0.0));
        assert!(at(0.5).a.alpha < 1e-9 && at(0.5).b.alpha < 1e-9);
        assert_eq!((at(1.0).a.alpha, at(1.0).b.alpha), (0.0, 1.0));
        assert!(!at(0.3).mix);
    }

    #[test]
    fn acercar_acaba_en_la_pose_de_la_siguiente() {
        let (a, b) = views();
        let tr = Transition { kind: TransitionKind::Zoom, duration: 1.0 };
        let start = frame(&tr, Direction::Right, 0.0, &a, &b, VP);
        let end = frame(&tr, Direction::Right, 1.0, &a, &b, VP);
        assert!(close(start.a.m, a.pose.to_viewport(a.img, VP)));
        assert!(close(end.b.m, b.pose.to_viewport(b.img, VP)));
        assert_eq!((start.b.alpha, end.b.alpha), (0.0, 1.0));
        assert!(start.mix);
        // A mitad, la saliente ya es más grande que al empezar.
        let mid = frame(&tr, Direction::Right, 0.5, &a, &b, VP);
        assert!(mid.a.m[0] > start.a.m[0]);
    }

    #[test]
    fn las_transiciones_nuevas_se_leen_del_proyecto() {
        let dip: Transition = serde_json::from_str(r#"{"type":"dip","duration":0.8}"#).unwrap();
        let zoom: Transition = serde_json::from_str(r#"{"type":"zoom","duration":0.6}"#).unwrap();
        assert_eq!(dip.kind, TransitionKind::Dip);
        assert_eq!(zoom.kind, TransitionKind::Zoom);
    }

    #[test]
    fn continua_empieza_y_acaba_en_las_poses() {
        let (a, b) = views();
        for dir in [Direction::Left, Direction::Right, Direction::Up, Direction::Down] {
            let tr = Transition { kind: TransitionKind::ContinuousCamera { dir: Some(dir) }, duration: 1.0 };
            let f0 = frame(&tr, Direction::Right, 0.0, &a, &b, VP);
            let f1 = frame(&tr, Direction::Right, 1.0, &a, &b, VP);
            assert!(close(f0.a.m, a.pose.to_viewport(a.img, VP)), "{dir:?} inicio");
            assert!(close(f1.b.m, b.pose.to_viewport(b.img, VP)), "{dir:?} final");
        }
    }

    #[test]
    fn deslizar_entra_por_el_lado_indicado() {
        let (a, b) = views();
        let tr = Transition { kind: TransitionKind::Slide { dir: Direction::Right }, duration: 1.0 };
        let f = frame(&tr, Direction::Right, 0.0, &a, &b, VP);
        assert!((f.b.m[4] - b.pose.to_viewport(b.img, VP)[4] - VP.0).abs() < 1e-9);
        let f = frame(&tr, Direction::Right, 1.0, &a, &b, VP);
        assert!(close(f.b.m, b.pose.to_viewport(b.img, VP)));
    }

    #[test]
    fn fundido_mezcla() {
        let (a, b) = views();
        let tr = Transition { kind: TransitionKind::Fade, duration: 1.0 };
        let f = frame(&tr, Direction::Right, 0.5, &a, &b, VP);
        assert!(f.mix && (f.b.alpha - 0.5).abs() < 1e-9);
    }
}
