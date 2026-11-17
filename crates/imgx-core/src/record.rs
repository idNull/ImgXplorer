//! Movimiento libre grabado a mano: suavizado y simplificación de las muestras.
//!
//! Al grabar se toma la pose en cada fotograma (~60 por segundo). Eso tiene el temblor de la
//! mano y muchísimas muestras, así que antes de guardarlo se suaviza un poco y se quitan las
//! muestras que la interpolación lineal entre sus vecinas ya reproduce (Ramer–Douglas–Peucker,
//! comparando en el mismo instante para conservar el ritmo).

use crate::camera::Camera;
use crate::schema::Sample;

/// Error máximo admitido al simplificar, en fracciones de pantalla.
pub const DEFAULT_TOLERANCE: f64 = 0.003;
/// Ventana del suavizado (s).
const SMOOTH_WINDOW: f64 = 0.12;

/// Suaviza y simplifica. Devuelve las muestras con el tiempo empezando en 0.
pub fn simplify(samples: &[Sample], tolerance: f64) -> Vec<Sample> {
    let mut v: Vec<Sample> = samples.iter().copied().filter(|s| s.t.is_finite() && valid(&s.pose)).collect();
    v.sort_by(|a, b| a.t.total_cmp(&b.t));
    v.dedup_by(|b, a| (b.t - a.t).abs() < 1e-6);
    let Some(t0) = v.first().map(|s| s.t) else { return v };
    for s in &mut v {
        s.t -= t0;
    }
    if v.len() <= 2 {
        return v;
    }
    let smooth = smoothed(&v);
    let mut keep = vec![false; smooth.len()];
    keep[0] = true;
    *keep.last_mut().unwrap() = true;
    let mut stack = vec![(0, smooth.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (sa, sb) = (&smooth[a], &smooth[b]);
        let span = sb.t - sa.t;
        let (mut worst, mut at) = (0.0, a);
        for (i, s) in smooth.iter().enumerate().take(b).skip(a + 1) {
            let u = if span > 0.0 { (s.t - sa.t) / span } else { 0.0 };
            let e = error(&sa.pose.lerp(&sb.pose, u), &s.pose);
            if e > worst {
                worst = e;
                at = i;
            }
        }
        if worst > tolerance {
            keep[at] = true;
            stack.push((a, at));
            stack.push((at, b));
        }
    }
    smooth.into_iter().zip(keep).filter_map(|(s, k)| k.then_some(s)).collect()
}

fn valid(c: &Camera) -> bool {
    c.cx.is_finite() && c.cy.is_finite() && c.rot.is_finite() && c.zoom.is_finite() && c.zoom > 0.0
}

/// Media ponderada (triangular) en una ventana de tiempo. Los extremos se conservan tal cual.
fn smoothed(v: &[Sample]) -> Vec<Sample> {
    let half = SMOOTH_WINDOW / 2.0;
    let last = v.len() - 1;
    (0..v.len())
        .map(|i| {
            if i == 0 || i == last {
                return v[i];
            }
            let t = v[i].t;
            // No se mira más allá de lo que hay a cada lado: así el final no se "adelanta".
            let reach = half.min(t - v[0].t).min(v[last].t - t);
            let (mut w, mut cx, mut cy, mut lz, mut rot) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for s in v.iter().filter(|s| (s.t - t).abs() <= reach) {
                let k = if reach > 0.0 { 1.0 - (s.t - t).abs() / (reach * 1.0001) } else { 1.0 };
                w += k;
                cx += k * s.pose.cx;
                cy += k * s.pose.cy;
                lz += k * s.pose.zoom.ln();
                rot += k * s.pose.rot;
            }
            Sample { t, pose: Camera { cx: cx / w, cy: cy / w, zoom: (lz / w).exp(), rot: rot / w } }
        })
        .collect()
}

/// Diferencia visible entre dos poses: desplazamiento en pantallas, zoom relativo y giro.
fn error(a: &Camera, b: &Camera) -> f64 {
    let z = a.zoom.max(b.zoom);
    let pan = (a.cx - b.cx).abs().max((a.cy - b.cy).abs()) * z;
    let zoom = (a.zoom.ln() - b.zoom.ln()).abs();
    let rot = (a.rot - b.rot).abs() / 90.0;
    pan.max(zoom).max(rot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t: f64, cx: f64, cy: f64, zoom: f64) -> Sample {
        Sample { t, pose: Camera { cx, cy, zoom, rot: 0.0 } }
    }

    #[test]
    fn una_recta_se_queda_en_dos_muestras() {
        let v: Vec<_> = (0..=120).map(|i| s(10.0 + i as f64 / 60.0, 0.2 + i as f64 / 200.0, 0.5, 2.0)).collect();
        let out = simplify(&v, DEFAULT_TOLERANCE);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].t, 0.0);
        assert!((out[1].t - 2.0).abs() < 1e-9);
        assert_eq!(out[1].pose, v[120].pose);
    }

    #[test]
    fn conserva_las_esquinas_y_el_ritmo() {
        // Va a la derecha 1 s, se para 1 s y baja 1 s.
        let mut v = Vec::new();
        for i in 0..=180 {
            let t = i as f64 / 60.0;
            let (cx, cy) = if t < 1.0 { (0.2 + 0.5 * t, 0.2) } else if t < 2.0 { (0.7, 0.2) } else { (0.7, 0.2 + 0.5 * (t - 2.0)) };
            v.push(s(t, cx, cy, 1.5));
        }
        let out = simplify(&v, DEFAULT_TOLERANCE);
        assert!(out.len() >= 4 && out.len() < 20, "{} muestras", out.len());
        let at = |t: f64| out.iter().min_by(|a, b| (a.t - t).abs().total_cmp(&(b.t - t).abs())).unwrap();
        assert!((at(1.5).pose.cx - 0.7).abs() < 0.01);
        assert!((at(1.5).pose.cy - 0.2).abs() < 0.01);
    }

    #[test]
    fn quita_el_temblor() {
        let v: Vec<_> = (0..=60).map(|i| s(i as f64 / 60.0, 0.5 + if i % 2 == 0 { 0.0005 } else { -0.0005 }, 0.5, 1.0)).collect();
        assert_eq!(simplify(&v, DEFAULT_TOLERANCE).len(), 2);
    }

    #[test]
    fn descarta_muestras_invalidas() {
        let v = vec![s(0.0, 0.5, 0.5, 1.0), s(0.5, f64::NAN, 0.5, 1.0), s(1.0, 0.6, 0.5, 0.0), s(1.0, 0.7, 0.5, 1.0)];
        let out = simplify(&v, DEFAULT_TOLERANCE);
        assert_eq!(out.len(), 2);
        assert!(simplify(&[], DEFAULT_TOLERANCE).is_empty());
    }
}
