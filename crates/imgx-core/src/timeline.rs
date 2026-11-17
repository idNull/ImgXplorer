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

fn needs_prev(clip: &Clip) -> bool {
    match clip {
        Clip::Hold { pose, .. } => pose.is_none(),
        Clip::Keyframes { keys } => keys.is_empty(),
        Clip::Recorded { samples } => samples.is_empty(),
    }
}

/// Evalúa sobre clips ya ordenados. `depth` limita la búsqueda de la pose de llegada.
fn eval(ordered: &[(&ClipInstance, &Clip)], t: f64, depth: u32) -> Camera {
    let sample = |idx: usize, at: f64| {
        let (inst, clip) = ordered[idx];
        // La pose con la que se llega solo hace falta en esperas sin pose (y clips vacíos).
        let prev = if needs_prev(clip) && depth < 16 { eval(&ordered[..idx], inst.start, depth + 1) } else { Camera::FIT };
        clip.sample(inst.local_time(at, clip), prev)
    };
    if let Some(idx) = ordered.iter().rposition(|(i, _)| i.start <= t && t <= i.end()) {
        return sample(idx, t);
    }
    // Hueco o después de todo: el que terminó más tarde (a igualdad, el que empezó después).
    let ended = ordered.iter().enumerate().filter(|(_, (i, _))| i.end() < t).max_by(|a, b| a.1.0.end().total_cmp(&b.1.0.end()));
    match ended {
        Some((idx, (inst, _))) => sample(idx, inst.end()),
        None if !ordered.is_empty() => sample(0, ordered[0].0.start),
        None => Camera::FIT,
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

    /// Clips que se pueden evaluar (las referencias que no existen se ignoran), por inicio.
    fn ordered<'a>(&'a self, lib: &'a dyn ClipResolver) -> Vec<(&'a ClipInstance, &'a Clip)> {
        let mut v: Vec<_> = self.camera.iter().filter_map(|i| i.resolve(lib).map(|c| (i, c))).collect();
        v.sort_by(|a, b| a.0.start.total_cmp(&b.0.start));
        v
    }

    /// Pose de cámara en el tiempo `t`.
    /// - Si hay clips activos en `t`, manda el que empieza más tarde (así se puede grabar o poner
    ///   un clip encima de otro sin borrarlo).
    /// - En un hueco se mantiene la última pose del clip que terminó más tarde.
    /// - Antes del primero, su pose inicial.
    pub fn evaluate(&self, t: f64, lib: &dyn ClipResolver) -> Camera {
        eval(&self.ordered(lib), t, 0)
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
        ClipInstance { name: None, start, duration, clip: ClipSource::Inline(clip), trim_in: 0.0, trim_out: 0.0 }
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
    fn solapados_manda_el_que_empieza_despues() {
        let a = cam(0.1, 0.5, 1.0);
        let b = cam(0.9, 0.5, 1.0);
        let tl = Timeline {
            camera: vec![
                inst(0.0, 10.0, Clip::Hold { length: 1.0, pose: Some(a) }),
                inst(2.0, 2.0, Clip::Hold { length: 1.0, pose: Some(b) }),
            ],
            ..Default::default()
        };
        assert_eq!(tl.evaluate(1.0, &NoClips), a);
        assert_eq!(tl.evaluate(3.0, &NoClips), b);
        assert_eq!(tl.evaluate(6.0, &NoClips), a, "al acabar el de encima, sigue el de debajo");
        assert_eq!(tl.evaluate(12.0, &NoClips), a);
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
