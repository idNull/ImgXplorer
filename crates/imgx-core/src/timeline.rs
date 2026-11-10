//! Evaluador de timeline: tiempo → pose de cámara. Única fuente de verdad para
//! la vista previa (WASM) y el render MP4.

use crate::camera::Camera;
use crate::schema::{Clip, ClipInstance, ClipSource, Project, Timeline};

impl Clip {
    /// Duración natural del clip en segundos.
    pub fn length(&self) -> f64 {
        match self {
            Clip::Keyframes { keys } => keys.last().map_or(0.0, |k| k.t),
            Clip::Recorded { samples } => samples.last().map_or(0.0, |s| s.t),
            Clip::Hold { length, .. } => *length,
        }
    }

    /// Pose en el tiempo local `t`. `prev` es la pose con la que se llega al clip.
    pub fn sample(&self, t: f64, prev: Camera) -> Camera {
        match self {
            Clip::Hold { pose, .. } => pose.unwrap_or(prev),
            Clip::Keyframes { keys } => {
                let Some(first) = keys.first() else { return prev };
                if t <= first.t {
                    return first.pose;
                }
                for w in keys.windows(2) {
                    let (a, b) = (&w[0], &w[1]);
                    if t <= b.t {
                        let span = b.t - a.t;
                        let u = if span > 0.0 { (t - a.t) / span } else { 1.0 };
                        return a.pose.lerp(&b.pose, a.easing.apply(u));
                    }
                }
                keys.last().unwrap().pose
            }
            Clip::Recorded { samples } => {
                let Some(first) = samples.first() else { return prev };
                if t <= first.t {
                    return first.pose;
                }
                let i = samples.partition_point(|s| s.t < t);
                if i >= samples.len() {
                    return samples.last().unwrap().pose;
                }
                let (a, b) = (&samples[i - 1], &samples[i]);
                let span = b.t - a.t;
                let u = if span > 0.0 { (t - a.t) / span } else { 1.0 };
                a.pose.lerp(&b.pose, u)
            }
        }
    }
}

/// Resuelve las referencias a clips con nombre.
pub trait ClipResolver {
    fn resolve(&self, id: &str) -> Option<&Clip>;
}

impl ClipResolver for Project {
    fn resolve(&self, id: &str) -> Option<&Clip> {
        self.find_clip(id)
    }
}

/// Resolver vacío: solo clips inline.
pub struct NoClips;
impl ClipResolver for NoClips {
    fn resolve(&self, _: &str) -> Option<&Clip> {
        None
    }
}

impl ClipInstance {
    fn resolve<'a>(&'a self, lib: &'a dyn ClipResolver) -> Option<&'a Clip> {
        match &self.clip {
            ClipSource::Inline(c) => Some(c),
            ClipSource::Ref(id) => lib.resolve(id),
        }
    }

    /// Tiempo absoluto → tiempo local del clip (aplica recortes y estirado).
    fn local_time(&self, t: f64, clip: &Clip) -> f64 {
        let span = (clip.length() - self.trim_in - self.trim_out).max(0.0);
        let u = if self.duration > 0.0 { ((t - self.start) / self.duration).clamp(0.0, 1.0) } else { 1.0 };
        self.trim_in + u * span
    }
}

impl Timeline {
    pub fn duration(&self) -> f64 {
        self.camera.iter().map(ClipInstance::end).fold(0.0, f64::max)
    }

    fn ordered(&self) -> Vec<&ClipInstance> {
        let mut v: Vec<_> = self.camera.iter().collect();
        v.sort_by(|a, b| a.start.total_cmp(&b.start));
        v
    }

    /// Pose de cámara en el tiempo `t`. Entre clips se mantiene la última pose;
    /// si dos clips se solapan, manda el que empieza después.
    pub fn evaluate(&self, t: f64, lib: &dyn ClipResolver) -> Camera {
        let ordered = self.ordered();
        let mut pose = ordered
            .first()
            .and_then(|i| i.resolve(lib).map(|c| c.sample(i.trim_in, Camera::FIT)))
            .unwrap_or(Camera::FIT);
        for inst in ordered {
            if t < inst.start {
                break;
            }
            let Some(clip) = inst.resolve(lib) else { continue };
            let at = t.min(inst.end());
            let p = clip.sample(inst.local_time(at, clip), pose);
            if t <= inst.end() {
                return p;
            }
            pose = p;
        }
        pose
    }

    /// Tiempos absolutos "interesantes" (inicio/fin de clip y keyframes), ordenados,
    /// para saltar con ←/→ durante la reproducción.
    pub fn marks(&self, lib: &dyn ClipResolver) -> Vec<f64> {
        let mut out = Vec::new();
        for inst in &self.camera {
            out.push(inst.start);
            out.push(inst.end());
            if let Some(Clip::Keyframes { keys }) = inst.resolve(lib) {
                let len = (keys.last().map_or(0.0, |k| k.t) - inst.trim_in - inst.trim_out).max(f64::EPSILON);
                for k in keys {
                    let u = (k.t - inst.trim_in) / len;
                    if (0.0..=1.0).contains(&u) {
                        out.push(inst.start + u * inst.duration);
                    }
                }
            }
        }
        out.sort_by(f64::total_cmp);
        out.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::easing::Easing;
    use crate::schema::Keyframe;

    fn cam(cx: f64, cy: f64, zoom: f64) -> Camera {
        Camera { cx, cy, zoom, rot: 0.0 }
    }

    fn kf(keys: &[(f64, Camera)]) -> Clip {
        Clip::Keyframes {
            keys: keys.iter().map(|&(t, pose)| Keyframe { t, pose, easing: Easing::Linear }).collect(),
        }
    }

    fn inst(start: f64, duration: f64, clip: Clip) -> ClipInstance {
        ClipInstance { start, duration, clip: ClipSource::Inline(clip), trim_in: 0.0, trim_out: 0.0 }
    }

    #[test]
    fn interpola_y_mantiene_entre_clips() {
        let a = cam(0.5, 0.1, 2.0);
        let b = cam(0.5, 0.9, 2.0);
        let tl = Timeline {
            camera: vec![
                inst(0.0, 4.0, kf(&[(0.0, a), (1.0, b)])), // estirado de 1 s a 4 s
                inst(6.0, 1.0, Clip::Hold { length: 1.0, pose: Some(Camera::FIT) }),
            ],
            ..Default::default()
        };
        assert_eq!(tl.evaluate(-1.0, &NoClips), a);
        assert!((tl.evaluate(2.0, &NoClips).cy - 0.5).abs() < 1e-12);
        assert_eq!(tl.evaluate(5.0, &NoClips), b); // hueco: mantiene la última pose
        assert_eq!(tl.evaluate(6.5, &NoClips), Camera::FIT);
        assert_eq!(tl.duration(), 7.0);
    }

    #[test]
    fn hold_sin_pose_mantiene_la_anterior() {
        let b = cam(0.2, 0.8, 3.0);
        let tl = Timeline {
            camera: vec![
                inst(0.0, 1.0, kf(&[(0.0, Camera::FIT), (1.0, b)])),
                inst(1.0, 2.0, Clip::Hold { length: 2.0, pose: None }),
            ],
            ..Default::default()
        };
        assert_eq!(tl.evaluate(2.0, &NoClips), b);
    }

    #[test]
    fn recorte() {
        let tl = Timeline {
            camera: vec![ClipInstance {
                trim_in: 1.0,
                trim_out: 1.0,
                ..inst(0.0, 1.0, kf(&[(0.0, cam(0.0, 0.0, 1.0)), (3.0, cam(1.0, 0.0, 1.0))]))
            }],
            ..Default::default()
        };
        assert!((tl.evaluate(0.0, &NoClips).cx - 1.0 / 3.0).abs() < 1e-12);
        assert!((tl.evaluate(1.0, &NoClips).cx - 2.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn marcas() {
        let tl = Timeline {
            camera: vec![inst(1.0, 2.0, kf(&[(0.0, Camera::FIT), (0.5, Camera::FIT), (1.0, Camera::FIT)]))],
            ..Default::default()
        };
        assert_eq!(tl.marks(&NoClips), vec![1.0, 2.0, 3.0]);
    }
}
