//! Curvas de easing para los tramos entre keyframes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    #[default]
    EaseInOut,
    /// Rebote al llegar.
    Bounce,
    /// Muelle que se pasa y vuelve.
    Elastic,
    /// Bézier cúbica estilo CSS `cubic-bezier(x1, y1, x2, y2)`.
    CubicBezier { x1: f64, y1: f64, x2: f64, y2: f64 },
    /// Salta al final del tramo (sin movimiento).
    Step,
}

impl Easing {
    /// Mapea el progreso lineal `t` (0..1) a progreso con easing.
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match *self {
            Easing::Linear => t,
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::Bounce => bounce_out(t),
            Easing::Elastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c4 = std::f64::consts::TAU / 3.0;
                    2f64.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }
            Easing::CubicBezier { x1, y1, x2, y2 } => cubic_bezier(x1, y1, x2, y2, t),
            Easing::Step => {
                if t >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

fn bounce_out(t: f64) -> f64 {
    const N: f64 = 7.5625;
    const D: f64 = 2.75;
    if t < 1.0 / D {
        N * t * t
    } else if t < 2.0 / D {
        let t = t - 1.5 / D;
        N * t * t + 0.75
    } else if t < 2.5 / D {
        let t = t - 2.25 / D;
        N * t * t + 0.9375
    } else {
        let t = t - 2.625 / D;
        N * t * t + 0.984375
    }
}

fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    let bez = |p1: f64, p2: f64, s: f64| {
        let u = 1.0 - s;
        3.0 * u * u * s * p1 + 3.0 * u * s * s * p2 + s * s * s
    };
    // Resolver s tal que bez_x(s) = x por bisección (robusta y suficiente en precisión).
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..50 {
        let mid = (lo + hi) / 2.0;
        if bez(x1, x2, mid) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez(y1, y2, (lo + hi) / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extremos() {
        let all = [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::Bounce,
            Easing::Elastic,
            Easing::CubicBezier { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1.0 },
        ];
        for e in all {
            assert!(e.apply(0.0).abs() < 1e-6, "{e:?}");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-6, "{e:?}");
        }
    }

    #[test]
    fn bezier_lineal() {
        let e = Easing::CubicBezier { x1: 1.0 / 3.0, y1: 1.0 / 3.0, x2: 2.0 / 3.0, y2: 2.0 / 3.0 };
        assert!((e.apply(0.37) - 0.37).abs() < 1e-6);
    }

    #[test]
    fn serde_formato() {
        let json = serde_json::to_string(&Easing::EaseInOut).unwrap();
        assert_eq!(json, r#"{"type":"ease-in-out"}"#);
    }
}
