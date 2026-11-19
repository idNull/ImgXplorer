//! Ajustes de imagen (🎨): brillo, contraste, color, «limpiar escaneo»… No tocan el archivo; se
//! guardan en el proyecto (por imagen o para todas) y los aplica la GPU al dibujar.
//!
//! Todos los valores neutros son 0, así que un ajuste vacío no se guarda.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tone {
    #[default]
    None,
    Gray,
    Sepia,
}

impl Tone {
    fn is_none(&self) -> bool {
        *self == Tone::None
    }
}

fn zero(v: &f64) -> bool {
    *v == 0.0
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Adjust {
    /// «Limpiar escaneo»: cuánto se estiran los niveles hacia papel blanco y tinta negra (0..1).
    #[serde(skip_serializing_if = "zero")]
    pub clean: f64,
    /// −1..1
    #[serde(skip_serializing_if = "zero")]
    pub brightness: f64,
    /// −1..1
    #[serde(skip_serializing_if = "zero")]
    pub contrast: f64,
    /// −1 (gris) .. 1 (doble de color).
    #[serde(skip_serializing_if = "zero")]
    pub saturation: f64,
    /// Tonos medios, −1..1 (aclara u oscurece sin quemar blancos ni negros).
    #[serde(skip_serializing_if = "zero")]
    pub exposure: f64,
    /// −1 (frío) .. 1 (cálido).
    #[serde(skip_serializing_if = "zero")]
    pub temperature: f64,
    /// Nitidez, 0..1.
    #[serde(skip_serializing_if = "zero")]
    pub sharpen: f64,
    #[serde(skip_serializing_if = "Tone::is_none")]
    pub tone: Tone,
}

impl Adjust {
    /// Rango válido de cada mando, para la interfaz y la IA.
    pub const RANGES: [(&'static str, f64, f64); 7] = [
        ("clean", 0.0, 1.0),
        ("brightness", -1.0, 1.0),
        ("contrast", -1.0, 1.0),
        ("saturation", -1.0, 1.0),
        ("exposure", -1.0, 1.0),
        ("temperature", -1.0, 1.0),
        ("sharpen", 0.0, 1.0),
    ];

    pub fn is_neutral(&self) -> bool {
        *self == Adjust::default()
    }

    /// Lleva cada valor a su rango (lo que venga de fuera: archivo o IA).
    pub fn clamped(mut self) -> Self {
        let f = |v: f64, lo: f64, hi: f64| if v.is_finite() { v.clamp(lo, hi) } else { 0.0 };
        self.clean = f(self.clean, 0.0, 1.0);
        self.brightness = f(self.brightness, -1.0, 1.0);
        self.contrast = f(self.contrast, -1.0, 1.0);
        self.saturation = f(self.saturation, -1.0, 1.0);
        self.exposure = f(self.exposure, -1.0, 1.0);
        self.temperature = f(self.temperature, -1.0, 1.0);
        self.sharpen = f(self.sharpen, 0.0, 1.0);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutro_no_guarda_nada() {
        assert_eq!(serde_json::to_string(&Adjust::default()).unwrap(), "{}");
        assert!(Adjust::default().is_neutral());
    }

    #[test]
    fn ida_y_vuelta_y_rangos() {
        let a = Adjust { clean: 1.0, contrast: 0.2, tone: Tone::Sepia, ..Default::default() };
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, r#"{"clean":1.0,"contrast":0.2,"tone":"sepia"}"#);
        assert_eq!(serde_json::from_str::<Adjust>(&json).unwrap(), a);
        let wild = Adjust { brightness: 7.0, sharpen: -1.0, exposure: f64::NAN, ..Default::default() }.clamped();
        assert_eq!((wild.brightness, wild.sharpen, wild.exposure), (1.0, 0.0, 0.0));
    }
}
