//! Modelo de cámara compartido por la vista previa (WASM) y el render MP4 (nativo).
//!
//! La pose se expresa en coordenadas **normalizadas de imagen** para que clips y
//! plantillas sean independientes de la resolución:
//! - `cx`, `cy`: punto de la imagen (0..1) que queda en el centro del viewport.
//! - `zoom`: multiplicador sobre el ajuste "imagen completa" (1.0 = cabe entera).
//! - `rot`: rotación en grados, sentido horario, alrededor del centro del viewport.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub cx: f64,
    pub cy: f64,
    pub zoom: f64,
    #[serde(default)]
    pub rot: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self::FIT
    }
}

/// Matriz afín 2D `[a, b, c, d, e, f]`: `x' = a*x + c*y + e`, `y' = b*x + d*y + f`
/// (mismo orden que `CanvasRenderingContext2D.setTransform`).
pub type Affine = [f64; 6];

pub const MIN_ZOOM: f64 = 0.05;
pub const MAX_ZOOM: f64 = 64.0;

impl Camera {
    /// Imagen completa, centrada, sin rotación.
    pub const FIT: Camera = Camera { cx: 0.5, cy: 0.5, zoom: 1.0, rot: 0.0 };

    pub fn lerp(&self, other: &Camera, t: f64) -> Camera {
        if t <= 0.0 {
            return *self;
        }
        if (1.0..=1.0 + f64::EPSILON).contains(&t) {
            return *other; // exacto en el extremo (el easing elástico puede pasarse de 1)
        }
        // El zoom se interpola en escala logarítmica: así un zoom 1→4 se percibe uniforme.
        let lz = self.zoom.ln() + (other.zoom.ln() - self.zoom.ln()) * t;
        Camera {
            cx: self.cx + (other.cx - self.cx) * t,
            cy: self.cy + (other.cy - self.cy) * t,
            zoom: lz.exp(),
            rot: self.rot + (other.rot - self.rot) * t,
        }
    }

    pub fn clamped(mut self) -> Camera {
        self.zoom = self.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self
    }

    /// Escala "imagen completa" para un viewport, sin tener en cuenta la rotación.
    pub fn fit_scale(img: (f64, f64), vp: (f64, f64)) -> f64 {
        (vp.0 / img.0).min(vp.1 / img.1)
    }

    /// Píxeles de pantalla por píxel de imagen.
    pub fn scale(&self, img: (f64, f64), vp: (f64, f64)) -> f64 {
        Self::fit_scale(img, vp) * self.zoom
    }

    /// Transformación de píxel de imagen → píxel de viewport.
    pub fn to_viewport(&self, img: (f64, f64), vp: (f64, f64)) -> Affine {
        let s = self.scale(img, vp);
        let (sin, cos) = self.rot.to_radians().sin_cos();
        let (a, b, c, d) = (cos * s, sin * s, -sin * s, cos * s);
        let (px, py) = (self.cx * img.0, self.cy * img.1);
        let e = vp.0 / 2.0 - (a * px + c * py);
        let f = vp.1 / 2.0 - (b * px + d * py);
        [a, b, c, d, e, f]
    }

    /// Punto del viewport (píxeles) → punto normalizado de la imagen.
    pub fn viewport_to_image(&self, p: (f64, f64), img: (f64, f64), vp: (f64, f64)) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.to_viewport(img, vp);
        let det = a * d - b * c;
        let (x, y) = (p.0 - e, p.1 - f);
        let ix = (d * x - c * y) / det;
        let iy = (-b * x + a * y) / det;
        (ix / img.0, iy / img.1)
    }

    /// Zoom manteniendo fijo el punto del viewport `anchor` (zoom al cursor / pellizco).
    pub fn zoom_at(&self, factor: f64, anchor: (f64, f64), img: (f64, f64), vp: (f64, f64)) -> Camera {
        let before = self.viewport_to_image(anchor, img, vp);
        let mut next = Camera { zoom: self.zoom * factor, ..*self }.clamped();
        let after = next.viewport_to_image(anchor, img, vp);
        next.cx += before.0 - after.0;
        next.cy += before.1 - after.1;
        next
    }

    /// Desplaza la cámara según un arrastre en píxeles de pantalla.
    pub fn pan_by(&self, dx: f64, dy: f64, img: (f64, f64), vp: (f64, f64)) -> Camera {
        let s = self.scale(img, vp);
        let (sin, cos) = self.rot.to_radians().sin_cos();
        // Deshacer la rotación para expresar el arrastre en ejes de imagen.
        let ix = (cos * dx + sin * dy) / s;
        let iy = (-sin * dx + cos * dy) / s;
        Camera { cx: self.cx - ix / img.0, cy: self.cy - iy / img.1, ..*self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMG: (f64, f64) = (2000.0, 3000.0);
    const VP: (f64, f64) = (1280.0, 800.0);

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn centro_de_camara_queda_en_centro_de_viewport() {
        let cam = Camera { cx: 0.3, cy: 0.7, zoom: 2.5, rot: 33.0 };
        let p = cam.viewport_to_image((VP.0 / 2.0, VP.1 / 2.0), IMG, VP);
        assert!(close(p, (0.3, 0.7)));
    }

    #[test]
    fn zoom_al_cursor_mantiene_el_punto() {
        let cam = Camera { cx: 0.4, cy: 0.6, zoom: 1.3, rot: 15.0 };
        let anchor = (200.0, 650.0);
        let before = cam.viewport_to_image(anchor, IMG, VP);
        let after = cam.zoom_at(1.7, anchor, IMG, VP).viewport_to_image(anchor, IMG, VP);
        assert!(close(before, after));
    }

    #[test]
    fn pan_sigue_al_raton() {
        let cam = Camera { cx: 0.5, cy: 0.5, zoom: 2.0, rot: 90.0 };
        let grab = (500.0, 300.0);
        let under = cam.viewport_to_image(grab, IMG, VP);
        let moved = cam.pan_by(40.0, -25.0, IMG, VP);
        let now = moved.viewport_to_image((grab.0 + 40.0, grab.1 - 25.0), IMG, VP);
        assert!(close(under, now));
    }

    #[test]
    fn lerp_zoom_logaritmico() {
        let a = Camera { zoom: 1.0, ..Camera::FIT };
        let b = Camera { zoom: 4.0, ..Camera::FIT };
        assert!((a.lerp(&b, 0.5).zoom - 2.0).abs() < 1e-12);
    }
}
