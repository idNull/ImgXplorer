//! Efectos especiales visuales colocados en la timeline de una imagen (temblor, destello, onda
//! expansiva, líneas de manga…).
//!
//! Cada efecto se guarda en `timeline.fx` con su tipo, inicio, duración, intensidad y mandos.
//! [`frame`] calcula, para un instante, todo lo que hay que aplicar al dibujar:
//! - un movimiento de cámara en pantalla (temblor, golpe de zoom), que se aplica a la matriz;
//! - los valores del shader de imagen (destello, onda, aberración, líneas, color…).
//!
//! Es determinista: el ruido del temblor y de las líneas sale del `id` del efecto y del tiempo,
//! así que la vista previa, las capturas y el MP4 dan exactamente lo mismo.

use crate::camera::{Affine, Camera};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Un efecto colocado en la timeline de una imagen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fx {
    pub id: String,
    /// Tipo (`shake`, `flash`…). Un tipo desconocido se conserva pero no se dibuja.
    #[serde(rename = "type")]
    pub kind: String,
    /// Segundo de la imagen en que empieza.
    pub start: f64,
    pub duration: f64,
    #[serde(default = "one")]
    pub intensity: f64,
    /// Ajuste predefinido del que salió (se quita al tocar un mando).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// Mandos; los que faltan valen lo de [`CATALOG`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Value>,
    /// Punto de la imagen (0..1) donde ocurre (centro de la onda, de las líneas…). Sin él, el
    /// centro de la pantalla.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

fn one() -> f64 {
    1.0
}

impl Fx {
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }
}

/// Final del último efecto (0 si no hay): la imagen dura al menos esto.
pub fn end(list: &[Fx]) -> f64 {
    list.iter().map(Fx::end).fold(0.0, f64::max)
}

// ---------------------------------------------------------------------------------------------
// Catálogo (la interfaz y la IA generan sus formularios a partir de aquí)

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ParamKind {
    Number { min: f64, max: f64, step: f64, default: f64, unit: &'static str },
    Choice { options: &'static [&'static str], default: &'static str },
    Color { default: &'static str },
    /// Texto libre (onomatopeyas y subtítulos), de hasta `max` caracteres.
    Text { default: &'static str, max: usize },
}

#[derive(Debug, Serialize)]
pub struct ParamSpec {
    pub key: &'static str,
    #[serde(flatten)]
    pub kind: ParamKind,
}

/// Ajuste predefinido de un tipo: valores de algunos mandos.
#[derive(Debug, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub duration: f64,
    pub params: &'static [(&'static str, PV)],
}

/// Valor de un mando en un ajuste predefinido o una situación.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(untagged)]
pub enum PV {
    N(f64),
    S(&'static str),
}

#[derive(Debug, Serialize)]
pub struct FxInfo {
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// Los de impacto suben de golpe y se apagan solos; los sostenidos entran y salen con
    /// `fadeIn`/`fadeOut`.
    pub sustained: bool,
    pub duration: f64,
    pub params: &'static [ParamSpec],
    pub presets: &'static [Preset],
}

const fn num(key: &'static str, min: f64, max: f64, step: f64, default: f64, unit: &'static str) -> ParamSpec {
    ParamSpec { key, kind: ParamKind::Number { min, max, step, default, unit } }
}
const fn choice(key: &'static str, options: &'static [&'static str], default: &'static str) -> ParamSpec {
    ParamSpec { key, kind: ParamKind::Choice { options, default } }
}
const fn color(key: &'static str, default: &'static str) -> ParamSpec {
    ParamSpec { key, kind: ParamKind::Color { default } }
}
const fn text(key: &'static str, default: &'static str, max: usize) -> ParamSpec {
    ParamSpec { key, kind: ParamKind::Text { default, max } }
}
const FADE_IN: ParamSpec = num("fadeIn", 0.0, 3.0, 0.05, 0.15, "s");
const FADE_OUT: ParamSpec = num("fadeOut", 0.0, 3.0, 0.05, 0.2, "s");
const LINE_COLORS: &[&str] = &["black", "white"];

use PV::{N, S};

pub const CATALOG: &[FxInfo] = &[
    FxInfo {
        kind: "shake",
        sustained: false,
        duration: 0.8,
        params: &[
            num("strength", 0.0, 10.0, 0.1, 2.0, "%"),
            num("freq", 1.0, 40.0, 0.5, 18.0, "Hz"),
            num("decay", 0.0, 1.0, 0.05, 0.8, ""),
            num("rot", 0.0, 6.0, 0.1, 0.6, "°"),
            choice("axis", &["both", "x", "y"], "both"),
        ],
        presets: &[
            Preset { id: "explosion", duration: 1.0, params: &[("strength", N(3.5)), ("freq", N(20.0)), ("decay", N(0.85)), ("rot", N(1.2))] },
            Preset { id: "hit", duration: 0.35, params: &[("strength", N(2.0)), ("freq", N(26.0)), ("decay", N(0.9)), ("rot", N(0.8))] },
            Preset { id: "earthquake", duration: 3.0, params: &[("strength", N(1.6)), ("freq", N(7.0)), ("decay", N(0.2)), ("rot", N(0.4))] },
            Preset { id: "handheld", duration: 4.0, params: &[("strength", N(0.4)), ("freq", N(1.5)), ("decay", N(0.0)), ("rot", N(0.3))] },
        ],
    },
    FxInfo {
        kind: "zoom-punch",
        sustained: false,
        duration: 0.5,
        params: &[
            num("amount", 0.0, 100.0, 1.0, 15.0, "%"),
            choice("way", &["in", "out"], "in"),
            num("attack", 0.01, 1.0, 0.01, 0.06, "s"),
            choice("settle", &["smooth", "bounce"], "smooth"),
        ],
        presets: &[
            Preset { id: "punch", duration: 0.45, params: &[("amount", N(18.0)), ("attack", N(0.05)), ("settle", S("bounce"))] },
            Preset { id: "surprise", duration: 0.8, params: &[("amount", N(30.0)), ("attack", N(0.12))] },
            Preset { id: "recoil", duration: 0.5, params: &[("amount", N(12.0)), ("way", S("out")), ("attack", N(0.04))] },
        ],
    },
    FxInfo {
        kind: "flash",
        sustained: false,
        duration: 0.5,
        params: &[
            color("color", "#ffffff"),
            num("amount", 0.0, 1.0, 0.05, 0.9, ""),
            num("attack", 0.0, 0.5, 0.01, 0.02, "s"),
            num("blink", 0.0, MAX_BLINK, 0.5, 0.0, "Hz"),
        ],
        presets: &[
            Preset { id: "white", duration: 0.5, params: &[] },
            Preset { id: "damage", duration: 0.4, params: &[("color", S("#ff2020")), ("amount", N(0.55))] },
            Preset { id: "blackout", duration: 0.6, params: &[("color", S("#000000")), ("amount", N(1.0)), ("attack", N(0.08))] },
            Preset { id: "lightning", duration: 0.9, params: &[("color", S("#e8f0ff")), ("blink", N(3.0))] },
        ],
    },
    FxInfo {
        kind: "shockwave",
        sustained: false,
        duration: 0.9,
        params: &[
            num("strength", 0.0, 10.0, 0.1, 3.0, "%"),
            num("width", 1.0, 30.0, 0.5, 8.0, "%"),
            num("size", 0.2, 2.0, 0.05, 1.2, ""),
        ],
        presets: &[
            Preset { id: "blast", duration: 0.9, params: &[] },
            Preset { id: "magic", duration: 1.4, params: &[("strength", N(1.8)), ("width", N(14.0)), ("size", N(0.9))] },
            Preset { id: "shout", duration: 0.7, params: &[("strength", N(1.5)), ("width", N(5.0)), ("size", N(0.8))] },
        ],
    },
    FxInfo {
        kind: "chroma",
        sustained: true,
        duration: 1.0,
        params: &[num("amount", 0.0, 4.0, 0.05, 0.7, "%"), num("glitch", 0.0, 1.0, 0.05, 0.0, ""), num("pulse", 0.0, 10.0, 0.5, 0.0, "Hz"), FADE_IN, FADE_OUT],
        presets: &[
            Preset { id: "soft", duration: 1.0, params: &[] },
            Preset { id: "glitch", duration: 0.8, params: &[("amount", N(1.2)), ("glitch", N(0.7)), ("fadeIn", N(0.0)), ("fadeOut", N(0.05))] },
            Preset { id: "power", duration: 1.5, params: &[("amount", N(1.0)), ("pulse", N(4.0))] },
        ],
    },
    FxInfo {
        kind: "radial-blur",
        sustained: true,
        duration: 0.8,
        params: &[num("amount", 0.0, 1.0, 0.05, 0.35, ""), FADE_IN, FADE_OUT],
        presets: &[
            Preset { id: "speed", duration: 1.2, params: &[("amount", N(0.35))] },
            Preset { id: "impact", duration: 0.35, params: &[("amount", N(0.6)), ("fadeIn", N(0.0)), ("fadeOut", N(0.25))] },
            Preset { id: "dizzy", duration: 2.0, params: &[("amount", N(0.15)), ("fadeIn", N(0.5)), ("fadeOut", N(0.5))] },
        ],
    },
    FxInfo {
        kind: "focus-lines",
        sustained: true,
        duration: 1.2,
        params: &[
            num("density", 20.0, 240.0, 1.0, 110.0, ""),
            num("thickness", 0.1, 1.0, 0.05, 0.5, ""),
            num("inner", 5.0, 90.0, 1.0, 38.0, "%"),
            choice("color", LINE_COLORS, "black"),
            num("flicker", 0.0, 24.0, 1.0, 12.0, "Hz"),
            num("fadeIn", 0.0, 3.0, 0.05, 0.05, "s"),
            FADE_OUT,
        ],
        presets: &[
            Preset { id: "shock", duration: 1.2, params: &[] },
            Preset { id: "reveal", duration: 2.0, params: &[("density", N(160.0)), ("thickness", N(0.3)), ("inner", N(50.0)), ("flicker", N(6.0))] },
            Preset { id: "white", duration: 1.2, params: &[("color", S("white"))] },
        ],
    },
    FxInfo {
        kind: "speed-lines",
        sustained: true,
        duration: 1.5,
        params: &[
            num("angle", 0.0, 359.0, 1.0, 0.0, "°"),
            num("density", 10.0, 160.0, 1.0, 60.0, ""),
            num("thickness", 0.1, 1.0, 0.05, 0.4, ""),
            choice("color", LINE_COLORS, "black"),
            num("speed", 0.0, 10.0, 0.1, 3.0, ""),
            FADE_IN,
            FADE_OUT,
        ],
        presets: &[
            Preset { id: "horizontal", duration: 1.5, params: &[] },
            Preset { id: "vertical", duration: 1.5, params: &[("angle", N(90.0))] },
            Preset { id: "white", duration: 1.5, params: &[("color", S("white"))] },
        ],
    },
    FxInfo {
        kind: "impact",
        sustained: false,
        duration: 0.25,
        params: &[choice("mode", &["negative", "bw", "both"], "negative"), num("blink", 0.0, MAX_BLINK, 0.5, 0.0, "Hz")],
        presets: &[
            Preset { id: "negative", duration: 0.25, params: &[] },
            Preset { id: "ink", duration: 0.3, params: &[("mode", S("bw"))] },
            Preset { id: "strobe", duration: 1.0, params: &[("mode", S("both")), ("blink", N(3.0))] },
        ],
    },
    FxInfo {
        kind: "color",
        sustained: true,
        duration: 3.0,
        params: &[
            choice("mode", &["sepia", "gray", "cold", "warm", "tint"], "sepia"),
            num("amount", 0.0, 1.0, 0.05, 0.8, ""),
            color("tint", "#ff3030"),
            num("contrast", -50.0, 50.0, 1.0, 0.0, "%"),
            num("fadeIn", 0.0, 3.0, 0.05, 0.4, "s"),
            num("fadeOut", 0.0, 3.0, 0.05, 0.4, "s"),
        ],
        presets: &[
            Preset { id: "flashback", duration: 4.0, params: &[("contrast", N(-10.0))] },
            Preset { id: "memory", duration: 4.0, params: &[("mode", S("gray")), ("amount", N(0.9))] },
            Preset { id: "danger", duration: 2.0, params: &[("mode", S("tint")), ("amount", N(0.35)), ("contrast", N(15.0))] },
            Preset { id: "fear", duration: 3.0, params: &[("mode", S("cold")), ("amount", N(0.4))] },
        ],
    },
    FxInfo {
        kind: "vignette",
        sustained: true,
        duration: 3.0,
        params: &[
            num("amount", 0.0, 1.0, 0.05, 0.6, ""),
            num("size", 0.2, 1.0, 0.05, 0.6, ""),
            num("pulse", 0.0, 3.0, 0.1, 0.0, "Hz"),
            choice("color", &["black", "red", "white"], "black"),
            num("fadeIn", 0.0, 3.0, 0.05, 0.4, "s"),
            num("fadeOut", 0.0, 3.0, 0.05, 0.4, "s"),
        ],
        presets: &[
            Preset { id: "tension", duration: 3.0, params: &[] },
            Preset { id: "heartbeat", duration: 3.0, params: &[("amount", N(0.7)), ("pulse", N(1.2)), ("color", S("red"))] },
            Preset { id: "dream", duration: 3.0, params: &[("amount", N(0.5)), ("color", S("white")), ("size", N(0.7))] },
        ],
    },
    FxInfo {
        kind: "wave",
        sustained: true,
        duration: 2.0,
        params: &[num("amp", 0.0, 3.0, 0.05, 0.6, "%"), num("freq", 1.0, 20.0, 0.5, 6.0, ""), num("speed", 0.0, 5.0, 0.1, 1.0, "Hz"), num("fadeIn", 0.0, 3.0, 0.05, 0.4, "s"), num("fadeOut", 0.0, 3.0, 0.05, 0.4, "s")],
        presets: &[
            Preset { id: "dream", duration: 2.5, params: &[] },
            Preset { id: "heat", duration: 3.0, params: &[("amp", N(0.25)), ("freq", N(14.0)), ("speed", N(2.5))] },
            Preset { id: "dizzy", duration: 2.0, params: &[("amp", N(1.2)), ("freq", N(3.0)), ("speed", N(0.6))] },
        ],
    },
    // Onomatopeya («¡BOOM!», «¡ZAS!»): letras de cómic sobre la imagen, pegadas a su punto (se
    // mueven con la cámara y con el temblor).
    FxInfo {
        kind: "text",
        sustained: false,
        duration: 1.5,
        params: &[
            text("text", "¡BOOM!", 40),
            color("color", "#ffd400"),
            color("outline", "#000000"),
            num("height", 3.0, 60.0, 1.0, 18.0, "%"),
            num("rot", -45.0, 45.0, 1.0, -8.0, "°"),
            choice("anim", &["pop", "shake", "grow", "bounce", "slide"], "pop"),
            num("fadeOut", 0.0, 3.0, 0.05, 0.25, "s"),
        ],
        presets: &[
            Preset { id: "boom", duration: 1.4, params: &[("text", S("¡BOOM!")), ("color", S("#ff4a1c")), ("anim", S("shake")), ("height", N(24.0)), ("rot", N(-6.0))] },
            Preset { id: "pow", duration: 0.9, params: &[("text", S("¡POW!")), ("color", S("#ffd400")), ("anim", S("pop")), ("rot", N(-12.0))] },
            Preset { id: "bang", duration: 0.8, params: &[("text", S("¡BANG!")), ("color", S("#ffffff")), ("outline", S("#c40000")), ("anim", S("pop")), ("height", N(16.0))] },
            Preset { id: "zas", duration: 0.8, params: &[("text", S("¡ZAS!")), ("color", S("#7dd3fc")), ("anim", S("slide")), ("rot", N(-4.0))] },
            Preset { id: "surprise", duration: 1.0, params: &[("text", S("!?")), ("color", S("#ffffff")), ("anim", S("bounce")), ("height", N(22.0)), ("rot", N(0.0))] },
            Preset { id: "rumble", duration: 2.0, params: &[("text", S("BRRRR")), ("color", S("#a3a3a3")), ("anim", S("grow")), ("height", N(14.0)), ("rot", N(0.0))] },
        ],
    },
    // Subtítulo, cartel o cartela de narración: texto fijo en la pantalla (no tiembla) que se
    // puede exportar también como `.srt`.
    FxInfo {
        kind: "caption",
        sustained: true,
        duration: 3.0,
        params: &[
            text("text", "…", 300),
            choice("style", &["subtitle", "title", "narration"], "subtitle"),
            num("height", 2.0, 15.0, 0.5, 4.5, "%"),
            color("color", "#ffffff"),
            FADE_IN,
            FADE_OUT,
        ],
        presets: &[
            Preset { id: "subtitle", duration: 3.0, params: &[] },
            Preset { id: "title", duration: 3.0, params: &[("style", S("title")), ("height", N(9.0))] },
            Preset { id: "narration", duration: 4.0, params: &[("style", S("narration")), ("color", S("#111111")), ("height", N(4.0))] },
        ],
    },
];

/// Destellos y negativos parpadean como mucho 3 veces por segundo (pautas WCAG sobre
/// fotosensibilidad).
pub const MAX_BLINK: f64 = 3.0;

pub fn info(kind: &str) -> Option<&'static FxInfo> {
    CATALOG.iter().find(|i| i.kind == kind)
}

/// Lector de mandos validados contra el catálogo: un valor ausente o no válido vale el de serie.
struct P<'a> {
    fx: &'a Fx,
    info: &'static FxInfo,
}

impl P<'_> {
    fn spec(&self, key: &str) -> Option<&'static ParamSpec> {
        self.info.params.iter().find(|p| p.key == key)
    }

    fn num(&self, key: &str) -> f64 {
        let Some(ParamSpec { kind: ParamKind::Number { min, max, default, .. }, .. }) = self.spec(key) else { return 0.0 };
        self.fx.params.get(key).and_then(Value::as_f64).filter(|v| v.is_finite()).map_or(*default, |v| v.clamp(*min, *max))
    }

    fn choice(&self, key: &str) -> &str {
        let Some(ParamSpec { kind: ParamKind::Choice { options, default }, .. }) = self.spec(key) else { return "" };
        match self.fx.params.get(key).and_then(Value::as_str) {
            Some(v) if options.contains(&v) => v,
            _ => default,
        }
    }

    fn text(&self, key: &str) -> &str {
        let Some(ParamSpec { kind: ParamKind::Text { default, .. }, .. }) = self.spec(key) else { return "" };
        self.fx.params.get(key).and_then(Value::as_str).unwrap_or(default)
    }

    fn color(&self, key: &str) -> [f64; 3] {
        let Some(ParamSpec { kind: ParamKind::Color { default }, .. }) = self.spec(key) else { return [1.0; 3] };
        self.fx.params.get(key).and_then(Value::as_str).and_then(parse_color).or_else(|| parse_color(default)).unwrap_or([1.0; 3])
    }
}

/// `#rrggbb` → componentes 0..1.
pub fn parse_color(s: &str) -> Option<[f64; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|v| v as f64 / 255.0);
    Some([c(0)?, c(2)?, c(4)?])
}

// ---------------------------------------------------------------------------------------------
// Ajustes predefinidos y situaciones

/// Un efecto nuevo de un tipo, con su ajuste predefinido (o los valores de serie).
pub fn new_fx(id: &str, kind: &str, preset: Option<&str>, start: f64) -> Option<Fx> {
    let info = info(kind)?;
    let p = preset.and_then(|p| info.presets.iter().find(|x| x.id == p));
    Some(Fx {
        id: id.to_owned(),
        kind: kind.to_owned(),
        start: start.max(0.0),
        duration: p.map_or(info.duration, |p| p.duration),
        intensity: 1.0,
        preset: p.map(|p| p.id.to_owned()),
        params: p.map(|p| to_params(p.params)).unwrap_or_default(),
        at: None,
    })
}

fn to_params(list: &[(&str, PV)]) -> BTreeMap<String, Value> {
    list.iter()
        .map(|(k, v)| {
            let v = match v {
                N(n) => Value::from(*n),
                S(s) => Value::from(*s),
            };
            ((*k).to_owned(), v)
        })
        .collect()
}

/// Una pieza de una situación: un efecto de un tipo y ajuste, desplazado `offset` segundos.
#[derive(Debug, Serialize)]
pub struct Part {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub preset: &'static str,
    pub offset: f64,
    /// 0: la del ajuste predefinido.
    pub duration: f64,
    pub intensity: f64,
}

/// Combinación lista para usar («Explosión», «Flashback»…) con su sonido de serie opcional.
#[derive(Debug, Serialize)]
pub struct Situation {
    pub id: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound: Option<&'static str>,
    /// Onomatopeya opcional: ajuste predefinido del tipo `text` («¡BOOM!»…).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<&'static str>,
    pub parts: &'static [Part],
}

const fn part(kind: &'static str, preset: &'static str, offset: f64, duration: f64, intensity: f64) -> Part {
    Part { kind, preset, offset, duration, intensity }
}

pub const SITUATIONS: &[Situation] = &[
    Situation {
        id: "explosion",
        sound: Some("builtin:explosion"),
        text: Some("boom"),
        parts: &[
            part("flash", "white", 0.0, 0.5, 1.0),
            part("shockwave", "blast", 0.0, 0.0, 1.0),
            part("shake", "explosion", 0.0, 1.2, 1.0),
            part("chroma", "glitch", 0.0, 0.5, 0.7),
        ],
    },
    Situation {
        id: "punch",
        sound: Some("builtin:punch"),
        text: Some("pow"),
        parts: &[part("zoom-punch", "punch", 0.0, 0.0, 1.0), part("shake", "hit", 0.0, 0.0, 1.0), part("impact", "negative", 0.0, 0.12, 1.0)],
    },
    Situation {
        id: "shot",
        sound: Some("builtin:shot"),
        text: Some("bang"),
        parts: &[part("flash", "white", 0.0, 0.18, 0.7), part("shake", "hit", 0.0, 0.25, 0.5)],
    },
    Situation {
        id: "lightning",
        sound: Some("builtin:thunder"),
        text: None,
        parts: &[part("flash", "lightning", 0.0, 0.0, 1.0), part("impact", "negative", 0.1, 0.12, 1.0)],
    },
    Situation {
        id: "earthquake",
        sound: None,
        text: Some("rumble"),
        parts: &[part("shake", "earthquake", 0.0, 0.0, 1.0), part("radial-blur", "dizzy", 0.0, 3.0, 0.5)],
    },
    Situation {
        id: "shout",
        sound: None,
        text: None,
        parts: &[part("shockwave", "shout", 0.0, 0.0, 1.0), part("shake", "hit", 0.0, 0.6, 0.7), part("focus-lines", "shock", 0.0, 1.0, 1.0)],
    },
    Situation {
        id: "speed",
        sound: Some("builtin:whoosh"),
        text: Some("zas"),
        parts: &[part("speed-lines", "horizontal", 0.0, 0.0, 1.0), part("radial-blur", "speed", 0.0, 1.5, 0.6)],
    },
    Situation {
        id: "surprise",
        sound: Some("builtin:ding"),
        text: Some("surprise"),
        parts: &[part("focus-lines", "shock", 0.0, 0.0, 1.0), part("zoom-punch", "surprise", 0.0, 0.0, 1.0)],
    },
    Situation {
        id: "fear",
        sound: Some("builtin:heartbeat"),
        text: None,
        parts: &[part("vignette", "heartbeat", 0.0, 0.0, 1.0), part("color", "fear", 0.0, 3.0, 1.0)],
    },
    Situation {
        id: "flashback",
        sound: None,
        text: None,
        parts: &[part("color", "flashback", 0.0, 0.0, 1.0), part("vignette", "tension", 0.0, 4.0, 0.7), part("wave", "dream", 0.0, 0.8, 0.6)],
    },
    Situation {
        id: "dream",
        sound: None,
        text: None,
        parts: &[part("wave", "dizzy", 0.0, 3.0, 1.0), part("radial-blur", "dizzy", 0.0, 3.0, 1.0), part("color", "memory", 0.0, 3.0, 0.5)],
    },
    Situation {
        id: "power",
        sound: None,
        text: None,
        parts: &[part("chroma", "power", 0.0, 0.0, 1.0), part("shockwave", "magic", 0.0, 0.0, 1.0), part("flash", "white", 0.0, 0.4, 0.4)],
    },
    Situation {
        id: "damage",
        sound: Some("builtin:hit"),
        text: None,
        parts: &[part("flash", "damage", 0.0, 0.0, 1.0), part("shake", "hit", 0.0, 0.0, 1.0), part("chroma", "glitch", 0.0, 0.35, 0.8)],
    },
];

pub fn situation(id: &str) -> Option<&'static Situation> {
    SITUATIONS.iter().find(|s| s.id == id)
}

/// Los efectos de una situación colocados en `start`, con ids `<base>-<n>` y su punto.
pub fn expand(sit: &Situation, base: &str, start: f64, at: Option<Point>) -> Vec<Fx> {
    expand_with(sit, base, start, at, false)
}

/// Como [`expand`], y con `with_text` también su onomatopeya (si la tiene).
pub fn expand_with(sit: &Situation, base: &str, start: f64, at: Option<Point>, with_text: bool) -> Vec<Fx> {
    let mut out: Vec<Fx> = sit
        .parts
        .iter()
        .enumerate()
        .filter_map(|(n, p)| {
            let mut fx = new_fx(&format!("{base}-{n}"), p.kind, Some(p.preset), start + p.offset)?;
            if p.duration > 0.0 {
                fx.duration = p.duration;
            }
            fx.intensity = p.intensity;
            fx.at = at;
            Some(fx)
        })
        .collect();
    if with_text
        && let Some(t) = sit.text
        && let Some(mut fx) = new_fx(&format!("{base}-t"), "text", Some(t), start)
    {
        fx.at = at;
        out.push(fx);
    }
    out
}

#[derive(Serialize)]
struct Catalog {
    effects: &'static [FxInfo],
    situations: &'static [Situation],
}

/// Catálogo completo en JSON (efectos con sus mandos y ajustes, y situaciones).
pub fn catalog_json() -> String {
    serde_json::to_string(&Catalog { effects: CATALOG, situations: SITUATIONS }).expect("catálogo serializable")
}

// ---------------------------------------------------------------------------------------------
// Cálculo de un fotograma

/// Todo lo que hay que aplicar al dibujar un instante. Distancias en píxeles del viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct FxFrame {
    /// Transformación de pantalla que se aplica después de la cámara (temblor, golpe de zoom).
    pub cam: Affine,
    /// rgb, cantidad.
    pub flash: [f64; 4],
    /// x, y, radio, ancho, fuerza (desplazamiento).
    pub shock: [f64; 5],
    /// separación, glitch (0..1), semilla del glitch.
    pub chroma: [f64; 3],
    /// x, y, cantidad.
    pub blur: [f64; 3],
    /// tipo (0 nada, 1 concentración, 2 velocidad), x, y, densidad, grosor, hueco, color
    /// (0 negro, 1 blanco), opacidad, semilla, ángulo (rad), desplazamiento.
    pub lines: [f64; 11],
    /// negativo, blanco y negro.
    pub impact: [f64; 2],
    /// sepia, gris, tinte r, g, b, cantidad de tinte, contraste (-0.5..0.5).
    pub color: [f64; 7],
    /// cantidad, tamaño, r, g, b.
    pub vignette: [f64; 5],
    /// amplitud, ondas por pantalla, fase.
    pub wave: [f64; 3],
    /// Textos de ese instante (onomatopeyas y subtítulos), en el orden de la lista.
    pub texts: Vec<TextSprite>,
}

/// Un texto en pantalla. La interfaz dibuja las letras (con el texto y colores del efecto
/// `index`) y las coloca así.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSprite {
    /// Posición del efecto en la lista.
    pub index: usize,
    /// Encima de todo (subtítulos): sin temblor ni efectos de imagen.
    pub overlay: bool,
    /// Punto de anclaje en la pantalla (px) y qué punto del texto va ahí (0..1: 0.5, 0.5 es el
    /// centro; 0.5, 1 el centro de abajo).
    pub x: f64,
    pub y: f64,
    pub ax: f64,
    pub ay: f64,
    /// Alto de una línea de letras (px) y ancho máximo antes de partir líneas (0: sin partir).
    pub line: f64,
    pub wrap: f64,
    pub scale: f64,
    /// Giro (grados).
    pub rot: f64,
    pub alpha: f64,
}

impl TextSprite {
    pub const LEN: usize = 11;

    pub fn to_vec(&self) -> [f64; Self::LEN] {
        [
            self.index as f64,
            if self.overlay { 1.0 } else { 0.0 },
            self.x,
            self.y,
            self.ax,
            self.ay,
            self.line,
            self.wrap,
            self.scale,
            self.rot,
            self.alpha,
        ]
    }
}

/// Curva que se pasa un poco y vuelve (el «pop» de las letras).
fn ease_out_back(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0) - 1.0;
    1.0 + u * u * (2.7 * u + 1.7)
}

/// Caída con rebotes (0 → 1).
fn bounce_out(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    let (n, d) = (7.5625, 2.75);
    if u < 1.0 / d {
        n * u * u
    } else if u < 2.0 / d {
        let v = u - 1.5 / d;
        n * v * v + 0.75
    } else if u < 2.5 / d {
        let v = u - 2.25 / d;
        n * v * v + 0.9375
    } else {
        let v = u - 2.625 / d;
        n * v * v + 0.984375
    }
}

const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

impl Default for FxFrame {
    fn default() -> Self {
        FxFrame {
            cam: IDENTITY,
            flash: [0.0; 4],
            shock: [0.0; 5],
            chroma: [0.0; 3],
            blur: [0.0; 3],
            lines: [0.0; 11],
            impact: [0.0; 2],
            color: [0.0; 7],
            vignette: [0.0; 5],
            wave: [0.0; 3],
            texts: Vec::new(),
        }
    }
}

/// Número de valores de [`FxFrame::to_vec`].
pub const FRAME_LEN: usize = 1 + 6 + 4 + 5 + 3 + 3 + 11 + 2 + 7 + 5 + 3;

impl FxFrame {
    /// ¿Hace falta la pasada de efectos de imagen (todo menos el movimiento de cámara)?
    pub fn post(&self) -> bool {
        self.flash[3] > 1e-4
            || self.shock[4].abs() > 1e-4
            || self.chroma[0] > 1e-4
            || self.chroma[1] > 1e-4
            || self.blur[2] > 1e-4
            || (self.lines[0] > 0.0 && self.lines[7] > 1e-4)
            || self.impact.iter().any(|v| *v > 1e-4)
            || self.color[0] > 1e-4
            || self.color[1] > 1e-4
            || self.color[5] > 1e-4
            || self.color[6].abs() > 1e-4
            || self.vignette[0] > 1e-4
            || self.wave[0] > 1e-4
    }

    /// Valores en orden fijo para la interfaz: `[post, cam(6), flash(4), shock(5), chroma(3),
    /// blur(3), lines(11), impact(2), color(7), vignette(5), wave(3)]`.
    pub fn to_vec(&self) -> Vec<f64> {
        let mut v = Vec::with_capacity(FRAME_LEN);
        v.push(if self.post() { 1.0 } else { 0.0 });
        for s in [&self.cam[..], &self.flash, &self.shock, &self.chroma, &self.blur, &self.lines, &self.impact, &self.color, &self.vignette, &self.wave] {
            v.extend_from_slice(s);
        }
        v
    }
}

/// Composición de afines: primero `m`, luego `s`.
pub fn then(m: Affine, s: Affine) -> Affine {
    [
        s[0] * m[0] + s[2] * m[1],
        s[1] * m[0] + s[3] * m[1],
        s[0] * m[2] + s[2] * m[3],
        s[1] * m[2] + s[3] * m[3],
        s[0] * m[4] + s[2] * m[5] + s[4],
        s[1] * m[4] + s[3] * m[5] + s[5],
    ]
}

fn apply(m: Affine, p: (f64, f64)) -> (f64, f64) {
    (m[0] * p.0 + m[2] * p.1 + m[4], m[1] * p.0 + m[3] * p.1 + m[5])
}

/// Giro `deg` y escala `s` alrededor de `c`, más un desplazamiento.
fn around(c: (f64, f64), s: f64, deg: f64, dx: f64, dy: f64) -> Affine {
    let (sin, cos) = deg.to_radians().sin_cos();
    let (a, b, cc, d) = (cos * s, sin * s, -sin * s, cos * s);
    [a, b, cc, d, c.0 - (a * c.0 + cc * c.1) + dx, c.1 - (b * c.0 + d * c.1) + dy]
}

// Ruido determinista ------------------------------------------------------------------------

fn seed_of(id: &str) -> u64 {
    // FNV-1a
    id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// Número pseudoaleatorio 0..1 para `(seed, i)` (splitmix64).
fn hash(seed: u64, i: i64) -> f64 {
    let mut z = seed.wrapping_add((i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// Ruido suave -1..1 en función de `x` (valores al azar en los enteros, interpolación suave).
fn noise(seed: u64, x: f64) -> f64 {
    let i = x.floor();
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(seed, i as i64) * 2.0 - 1.0;
    let b = hash(seed, i as i64 + 1) * 2.0 - 1.0;
    a + (b - a) * u
}

// Envolventes ---------------------------------------------------------------------------------

/// Sube en `attack` y se apaga hasta el final con forma (1-x)².
fn pulse(local: f64, dur: f64, attack: f64) -> f64 {
    if local < attack {
        return if attack > 0.0 { local / attack } else { 1.0 };
    }
    let x = ((local - attack) / (dur - attack).max(1e-6)).clamp(0.0, 1.0);
    (1.0 - x) * (1.0 - x)
}

/// Entra en `fade_in`, se mantiene y sale en `fade_out` (curvas suaves).
fn hold(local: f64, dur: f64, fade_in: f64, fade_out: f64) -> f64 {
    let s = |x: f64| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let a = if fade_in > 0.0 { s(local / fade_in) } else { 1.0 };
    let b = if fade_out > 0.0 { s((dur - local) / fade_out) } else { 1.0 };
    a.min(b)
}

/// Onda cuadrada de `hz` (como mucho [`MAX_BLINK`]): 1 encendido, 0 apagado.
fn blink(local: f64, hz: f64) -> f64 {
    if hz <= 0.0 {
        return 1.0;
    }
    if (local * hz.min(MAX_BLINK)).fract() < 0.5 { 1.0 } else { 0.0 }
}

/// Lo que se le pasa a [`frame`] para situar los efectos en pantalla.
pub struct View {
    /// Tamaño de la imagen en píxeles.
    pub img: (f64, f64),
    /// Pose de la cámara en ese instante.
    pub pose: Camera,
    /// Tamaño del viewport en píxeles.
    pub vp: (f64, f64),
}

/// Efectos activos en `t` (segundos de la imagen) combinados en un fotograma.
pub fn frame(list: &[Fx], t: f64, view: &View) -> FxFrame {
    let mut out = FxFrame::default();
    let (w, h) = view.vp;
    let unit = h / 100.0; // los porcentajes son de la altura del viewport
    let m = view.pose.to_viewport(view.img, view.vp);
    let center = (w / 2.0, h / 2.0);
    let mut cam = IDENTITY;
    let mut flash_w = 0.0;
    let mut flash_rgb = [0.0; 3];

    for (index, fx) in list.iter().enumerate() {
        let local = t - fx.start;
        if !(0.0..fx.duration).contains(&local) || fx.duration <= 0.0 {
            continue;
        }
        let Some(info) = info(&fx.kind) else { continue };
        let p = P { fx, info };
        let k = fx.intensity.clamp(0.0, 3.0);
        let seed = seed_of(&fx.id);
        let at = fx.at.map_or(center, |a| apply(m, (a.x * view.img.0, a.y * view.img.1)));
        let sustained = if info.sustained { hold(local, fx.duration, p.num("fadeIn"), p.num("fadeOut")) } else { 1.0 };
        match fx.kind.as_str() {
            "shake" => {
                let decay = p.num("decay");
                // Sin amortiguación sale y entra suave para que no dé un salto al final.
                let env = (1.0 - decay + decay * pulse(local, fx.duration, 0.02)) * hold(local, fx.duration, 0.03, 0.12) * k;
                let a = p.num("strength") * unit * env;
                let x = local * p.num("freq");
                let (mut dx, mut dy) = (noise(seed, x) * a, noise(seed ^ 0x5555, x + 31.7) * a);
                match p.choice("axis") {
                    "x" => dy = 0.0,
                    "y" => dx = 0.0,
                    _ => {}
                }
                let rot = noise(seed ^ 0xaaaa, x + 77.3) * p.num("rot") * env;
                cam = then(cam, around(center, 1.0, rot, dx, dy));
            }
            "zoom-punch" => {
                let attack = p.num("attack").min(fx.duration * 0.9);
                let env = if local < attack {
                    let u = local / attack;
                    1.0 - (1.0 - u) * (1.0 - u)
                } else {
                    let u = (local - attack) / (fx.duration - attack).max(1e-6);
                    if p.choice("settle") == "bounce" {
                        (1.0 - u) * (1.0 - u) * (u * 14.0).cos()
                    } else {
                        let s = u * u * (3.0 - 2.0 * u);
                        1.0 - s
                    }
                };
                let amount = p.num("amount") / 100.0 * env * k;
                let s = if p.choice("way") == "out" { 1.0 / (1.0 + amount) } else { 1.0 + amount };
                cam = then(cam, around(at, s, 0.0, 0.0, 0.0));
            }
            "flash" => {
                let env = pulse(local, fx.duration, p.num("attack")) * blink(local, p.num("blink"));
                let a = (p.num("amount") * env * k).min(1.0);
                if a > 0.0 {
                    let c = p.color("color");
                    for (i, v) in c.iter().enumerate() {
                        flash_rgb[i] += v * a;
                    }
                    flash_w += a;
                    out.flash[3] = out.flash[3].max(a);
                }
            }
            "shockwave" => {
                let u = local / fx.duration;
                let diag = (w * w + h * h).sqrt();
                let radius = p.num("size") * diag * (1.0 - (1.0 - u).powi(3));
                let strength = p.num("strength") * unit * (1.0 - u) * k;
                if strength.abs() > out.shock[4].abs() {
                    out.shock = [at.0, at.1, radius, p.num("width") * unit, strength];
                }
            }
            "chroma" => {
                let pulse_hz = p.num("pulse");
                let beat = if pulse_hz > 0.0 { 0.5 + 0.5 * (local * pulse_hz * std::f64::consts::TAU).sin() } else { 1.0 };
                let a = p.num("amount") * unit * sustained * beat * k;
                out.chroma[0] = out.chroma[0].max(a);
                out.chroma[1] = out.chroma[1].max(p.num("glitch") * sustained * k.min(1.0));
                out.chroma[2] = (local * 12.0).floor() + (seed % 997) as f64;
            }
            "radial-blur" => {
                let a = (p.num("amount") * sustained * k).min(1.0);
                if a > out.blur[2] {
                    out.blur = [at.0, at.1, a];
                }
            }
            "focus-lines" | "speed-lines" => {
                let a = (sustained * k).min(1.0);
                if a > out.lines[7] {
                    let focus = fx.kind == "focus-lines";
                    let flicker = if focus { p.num("flicker") } else { 0.0 };
                    let step = if flicker > 0.0 { (local * flicker).floor() } else { 0.0 };
                    let lines = [
                        if focus { 1.0 } else { 2.0 },
                        at.0,
                        at.1,
                        p.num("density"),
                        p.num("thickness"),
                        if focus { p.num("inner") * unit } else { 0.0 },
                        if p.choice("color") == "white" { 1.0 } else { 0.0 },
                        a,
                        (seed % 1000) as f64 + step * 17.0,
                        if focus { 0.0 } else { p.num("angle").to_radians() },
                        if focus { 0.0 } else { local * p.num("speed") },
                    ];
                    out.lines = lines;
                }
            }
            "impact" => {
                let on = blink(local, p.num("blink")) * k.min(1.0);
                match p.choice("mode") {
                    "bw" => out.impact[1] = out.impact[1].max(on),
                    "both" => {
                        // Alterna: negativo en un parpadeo, tinta en el siguiente.
                        let odd = p.num("blink") > 0.0 && (local * p.num("blink").min(MAX_BLINK)).floor() as i64 % 2 == 1;
                        out.impact[1] = out.impact[1].max(k.min(1.0));
                        if !odd {
                            out.impact[0] = out.impact[0].max(k.min(1.0));
                        }
                    }
                    _ => out.impact[0] = out.impact[0].max(on),
                }
            }
            "color" => {
                let a = (p.num("amount") * sustained * k).min(1.0);
                match p.choice("mode") {
                    "sepia" => out.color[0] = out.color[0].max(a),
                    "gray" => out.color[1] = out.color[1].max(a),
                    mode => {
                        let tint = match mode {
                            "cold" => [0.35, 0.55, 1.0],
                            "warm" => [1.0, 0.6, 0.25],
                            _ => p.color("tint"),
                        };
                        if a > out.color[5] {
                            out.color[2..5].copy_from_slice(&tint);
                            out.color[5] = a;
                        }
                    }
                }
                out.color[6] += p.num("contrast") / 100.0 * sustained * k.min(1.0);
            }
            "vignette" => {
                let hz = p.num("pulse");
                // Latido: dos golpes seguidos por ciclo.
                let beat = if hz > 0.0 {
                    let ph = (local * hz).fract();
                    let b = |c: f64| (-((ph - c) / 0.06).powi(2)).exp();
                    0.55 + 0.45 * (b(0.1) + 0.7 * b(0.3)).min(1.0)
                } else {
                    1.0
                };
                let a = (p.num("amount") * sustained * beat * k).min(1.0);
                if a > out.vignette[0] {
                    let c = match p.choice("color") {
                        "red" => [0.55, 0.0, 0.0],
                        "white" => [1.0, 1.0, 1.0],
                        _ => [0.0, 0.0, 0.0],
                    };
                    out.vignette = [a, p.num("size"), c[0], c[1], c[2]];
                }
            }
            "wave" => {
                let a = p.num("amp") * unit * sustained * k;
                if a > out.wave[0] {
                    out.wave = [a, p.num("freq"), local * p.num("speed") * std::f64::consts::TAU];
                }
            }
            "text" => {
                if p.text("text").trim().is_empty() {
                    continue;
                }
                let line = p.num("height") * unit;
                let fade = p.num("fadeOut");
                let mut alpha = if fade > 0.0 { ((fx.duration - local) / fade).clamp(0.0, 1.0) } else { 1.0 };
                let (mut x, mut y) = at;
                let mut scale = 1.0;
                let mut rot = p.num("rot");
                match p.choice("anim") {
                    "shake" => {
                        scale = ease_out_back(local / 0.14);
                        let a = 1.2 * unit * k;
                        x += noise(seed, local * 22.0) * a;
                        y += noise(seed ^ 0x77, local * 22.0 + 9.1) * a;
                        rot += noise(seed ^ 0x33, local * 15.0) * 3.0;
                    }
                    "grow" => {
                        let u = (local / fx.duration).clamp(0.0, 1.0);
                        scale = 0.5 + 0.8 * (1.0 - (1.0 - u) * (1.0 - u));
                        alpha *= (local / 0.12).min(1.0);
                    }
                    "bounce" => y -= (1.0 - bounce_out(local / 0.6)) * line * 2.5,
                    "slide" => {
                        let u = ease_out_back(local / 0.25);
                        x -= (1.0 - u) * w * 0.35;
                        alpha *= (local / 0.08).min(1.0);
                    }
                    _ => scale = ease_out_back(local / 0.2),
                }
                alpha *= k.min(1.0);
                if alpha > 1e-3 {
                    out.texts.push(TextSprite { index, overlay: false, x, y, ax: 0.5, ay: 0.5, line, wrap: 0.0, scale: scale.max(0.0), rot, alpha });
                }
            }
            "caption" => {
                if p.text("text").trim().is_empty() {
                    continue;
                }
                let line = p.num("height") * unit;
                let alpha = sustained * k.min(1.0);
                let (x, y, ax, ay, wrap) = match p.choice("style") {
                    "title" => (w / 2.0, h / 2.0, 0.5, 0.5, w * 0.86),
                    "narration" => (w * 0.04, h * 0.05, 0.0, 0.0, w * 0.46),
                    _ => (w / 2.0, h * 0.93, 0.5, 1.0, w * 0.86),
                };
                if alpha > 1e-3 {
                    out.texts.push(TextSprite { index, overlay: true, x, y, ax, ay, line, wrap, scale: 1.0, rot: 0.0, alpha });
                }
            }
            _ => {}
        }
    }

    if flash_w > 0.0 {
        for (o, c) in out.flash.iter_mut().zip(flash_rgb) {
            *o = c / flash_w;
        }
    }
    out.color[6] = out.color[6].clamp(-0.5, 0.5);

    // Los puntos de los efectos se mueven con el temblor y el golpe de zoom.
    if cam != IDENTITY {
        let move_pt = |x: &mut f64, y: &mut f64| {
            let p = apply(cam, (*x, *y));
            *x = p.0;
            *y = p.1;
        };
        let [sx, sy, ..] = &mut out.shock;
        move_pt(sx, sy);
        let [bx, by, _] = &mut out.blur;
        move_pt(bx, by);
        let [_, lx, ly, ..] = &mut out.lines;
        move_pt(lx, ly);
        // Las onomatopeyas, pegadas a la imagen, tiemblan con ella (y giran con el golpe).
        let turn = cam[1].atan2(cam[0]).to_degrees();
        let grow = (cam[0] * cam[0] + cam[1] * cam[1]).sqrt();
        for s in out.texts.iter_mut().filter(|s| !s.overlay) {
            move_pt(&mut s.x, &mut s.y);
            s.rot += turn;
            s.scale *= grow;
        }
    }
    out.cam = cam;
    out
}

/// Avisos para el resumen (IA y editor): destellos demasiado seguidos y efectos que se salen
/// de la imagen.
pub fn warnings(list: &[Fx], image_end: f64) -> Vec<String> {
    let mut out = Vec::new();
    let mut flashes: Vec<f64> = list.iter().filter(|f| f.kind == "flash" || f.kind == "impact").map(|f| f.start).collect();
    flashes.sort_by(f64::total_cmp);
    if flashes.windows(4).any(|w| w[3] - w[0] < 1.0) {
        out.push("more than 3 flashes/impact frames within one second: risky for photosensitive viewers".to_owned());
    }
    for f in list {
        if info(&f.kind).is_none() {
            out.push(format!("effect {} has unknown type {}", f.id, f.kind));
        } else if f.end() > image_end + 1e-6 {
            out.push(format!("effect {} ({}) ends at {:.2} s, after the image ({:.2} s): it makes the image longer", f.id, f.kind, f.end(), image_end));
        }
        if ["flash", "impact"].contains(&f.kind.as_str())
            && f.params.get("blink").and_then(Value::as_f64).is_some_and(|b| b > MAX_BLINK)
        {
            out.push(format!("effect {} blinks faster than {MAX_BLINK} Hz: limited to {MAX_BLINK} Hz", f.id));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        View { img: (1000.0, 1500.0), pose: Camera::FIT, vp: (1280.0, 720.0) }
    }

    fn fx(kind: &str, start: f64, dur: f64) -> Fx {
        let mut f = new_fx("f1", kind, None, start).unwrap();
        f.duration = dur;
        f
    }

    #[test]
    fn sin_efectos_no_cambia_nada() {
        let f = frame(&[], 1.0, &view());
        assert_eq!(f, FxFrame::default());
        assert!(!f.post());
        // Fuera de su tiempo, tampoco.
        let list = [fx("flash", 2.0, 0.5)];
        assert_eq!(frame(&list, 1.9, &view()), FxFrame::default());
        assert_eq!(frame(&list, 2.5, &view()), FxFrame::default());
        assert_eq!(to_len(), FRAME_LEN);
    }

    fn to_len() -> usize {
        FxFrame::default().to_vec().len()
    }

    #[test]
    fn el_destello_sube_y_se_apaga() {
        let list = [fx("flash", 1.0, 0.5)];
        let a = |t| frame(&list, t, &view()).flash[3];
        assert!(a(1.0) < 0.1);
        assert!(a(1.02) > 0.85);
        assert!(a(1.2) < a(1.05));
        assert!(a(1.49) < 0.01);
        assert_eq!(&frame(&list, 1.1, &view()).flash[..3], &[1.0, 1.0, 1.0]);
    }

    #[test]
    fn el_temblor_es_determinista_y_se_amortigua() {
        let list = [fx("shake", 0.0, 1.0)];
        let a = frame(&list, 0.3, &view());
        let b = frame(&list, 0.3, &view());
        assert_eq!(a, b);
        assert!(a.cam != IDENTITY);
        assert!(!a.post(), "el temblor solo mueve la cámara");
        // Otro id → otro temblor.
        let mut other = list.clone();
        other[0].id = "f2".into();
        assert_ne!(frame(&other, 0.3, &view()).cam, a.cam);
        // Desplazamiento máximo al principio mayor que al final.
        let off = |t: f64| (0..20).map(|i| frame(&list, t + i as f64 * 0.005, &view()).cam[4].abs()).fold(0.0, f64::max);
        assert!(off(0.05) > off(0.85));
        // Nunca más de la fuerza pedida (2 % de 720 px, más el giro).
        assert!(off(0.05) < 720.0 * 0.02 * 1.2);
    }

    #[test]
    fn la_onomatopeya_aparece_en_su_punto_y_se_desvanece() {
        let mut f = new_fx("t1", "text", Some("pow"), 1.0).unwrap();
        f.at = Some(Point { x: 0.25, y: 0.5 });
        let list = [f];
        let v = view();
        assert!(frame(&list, 0.9, &v).texts.is_empty());
        let a = frame(&list, 1.05, &v).texts;
        let b = frame(&list, 1.3, &v).texts;
        assert_eq!(a.len(), 1);
        assert!(a[0].scale < b[0].scale, "crece al aparecer");
        assert!(!a[0].overlay);
        let p = apply(v.pose.to_viewport(v.img, v.vp), (250.0, 750.0));
        assert!((b[0].x - p.0).abs() < 1e-6 && (b[0].y - p.1).abs() < 1e-6, "pegada a su punto");
        assert!((b[0].line - 720.0 * 0.18).abs() < 1e-6, "tamaño de serie: 18 % del alto");
        let end = frame(&list, 1.0 + 0.89, &v).texts;
        assert!(end[0].alpha < 0.2, "se desvanece al final");
        // Con el temblor, se mueve con la imagen.
        let mut shake = fx("shake", 1.0, 1.0);
        shake.id = "s".into();
        let both = [list[0].clone(), shake];
        let c = frame(&both, 1.3, &v).texts;
        assert!((c[0].x - b[0].x).abs() + (c[0].y - b[0].y).abs() > 1e-3);
    }

    #[test]
    fn el_subtitulo_va_encima_y_sin_texto_no_hay_nada() {
        let mut c = new_fx("c1", "caption", None, 0.0).unwrap();
        c.params.insert("text".into(), Value::from("Hola"));
        let v = view();
        let s = frame(&[c.clone()], 1.0, &v).texts;
        assert_eq!(s.len(), 1);
        assert!(s[0].overlay);
        assert_eq!((s[0].ax, s[0].ay), (0.5, 1.0));
        assert!(s[0].wrap > 0.0);
        c.params.insert("text".into(), Value::from("   "));
        assert!(frame(&[c], 1.0, &v).texts.is_empty());
    }

    #[test]
    fn las_situaciones_con_onomatopeya() {
        let sit = situation("explosion").unwrap();
        let without = expand(sit, "e", 0.0, None);
        let with = expand_with(sit, "e", 0.0, None, true);
        assert_eq!(with.len(), without.len() + 1);
        assert_eq!(with.last().unwrap().kind, "text");
        assert_eq!(with.last().unwrap().params.get("text").and_then(Value::as_str), Some("¡BOOM!"));
        for s in SITUATIONS {
            if let Some(t) = s.text {
                assert!(info("text").unwrap().presets.iter().any(|p| p.id == t), "{} → {t}", s.id);
            }
        }
    }

    #[test]
    fn la_aberracion_se_ve_con_cada_ajuste() {
        let v = view();
        for preset in ["soft", "glitch", "power"] {
            let f = new_fx("c", "chroma", Some(preset), 0.0).unwrap();
            let fr = frame(&[f], 0.3, &v);
            assert!(fr.chroma[0] > 1.0, "{preset}: separación de más de 1 px");
            assert!(fr.post(), "{preset}");
        }
        let g = frame(&[new_fx("c", "chroma", Some("glitch"), 0.0).unwrap()], 0.3, &v);
        assert!(g.chroma[1] > 0.5, "el glitch rompe la imagen en franjas");
        // Solo glitch, sin separación: también hace falta la pasada de efectos.
        let mut only = new_fx("c", "chroma", Some("glitch"), 0.0).unwrap();
        only.params.insert("amount".into(), Value::from(0.0));
        let o = frame(&[only], 0.3, &v);
        assert!(o.chroma[0] == 0.0 && o.post());
    }

    #[test]
    fn el_golpe_de_zoom_vuelve_a_su_sitio() {
        let list = [fx("zoom-punch", 0.0, 0.5)];
        let s = |t| frame(&list, t, &view()).cam[0];
        assert!(s(0.06) > 1.14);
        assert!((s(0.499) - 1.0).abs() < 0.01);
    }

    #[test]
    fn la_onda_crece_desde_su_punto_y_lo_sigue_con_la_camara() {
        let mut f = fx("shockwave", 0.0, 1.0);
        f.at = Some(Point { x: 0.25, y: 0.5 });
        let list = [f];
        let v = view();
        let a = frame(&list, 0.2, &v);
        let b = frame(&list, 0.6, &v);
        assert!(b.shock[2] > a.shock[2], "el radio crece");
        assert!(b.shock[4] < a.shock[4], "y pierde fuerza");
        assert!(a.post());
        let m = v.pose.to_viewport(v.img, v.vp);
        let p = apply(m, (250.0, 750.0));
        assert!((a.shock[0] - p.0).abs() < 1e-9 && (a.shock[1] - p.1).abs() < 1e-9);
        // Con la cámara movida y acercada, el punto se mueve con la imagen.
        let moved = View { pose: Camera { cx: 0.3, cy: 0.5, zoom: 2.0, rot: 0.0 }, ..v };
        let c = frame(&list, 0.2, &moved);
        let m2 = moved.pose.to_viewport(moved.img, moved.vp);
        let p2 = apply(m2, (250.0, 750.0));
        assert!((c.shock[0] - p2.0).abs() < 1e-9 && (c.shock[1] - p2.1).abs() < 1e-9);
        assert!((c.shock[0] - a.shock[0]).abs() > 10.0);
    }

    #[test]
    fn los_parpadeos_no_pasan_de_3_por_segundo() {
        let mut f = fx("flash", 0.0, 2.0);
        f.params.insert("blink".into(), Value::from(20.0)); // fuera de rango → 3 Hz
        let list = [f];
        let on: Vec<bool> = (0..200).map(|i| frame(&list, i as f64 * 0.01, &view()).flash[3] > 0.0).collect();
        let changes = on.windows(2).filter(|w| w[0] != w[1]).count();
        // 2 s a 3 Hz: 6 encendidos y 6 apagados como mucho.
        assert!(changes <= 12, "{changes} cambios");
        let warn = warnings(&list, 5.0);
        assert!(warn.iter().any(|w| w.contains("Hz")));
        let many: Vec<Fx> = (0..4).map(|i| fx("flash", i as f64 * 0.2, 0.1)).collect();
        assert!(warnings(&many, 5.0).iter().any(|w| w.contains("photosensitive")));
    }

    #[test]
    fn efectos_sostenidos_entran_y_salen() {
        let list = [fx("color", 0.0, 3.0)];
        let a = |t| frame(&list, t, &view()).color[0];
        assert!(a(0.05) < 0.2);
        assert!((a(1.5) - 0.8).abs() < 1e-9);
        assert!(a(2.97) < 0.1);
    }

    #[test]
    fn mandos_fuera_de_rango_o_desconocidos() {
        let mut f = fx("shake", 0.0, 1.0);
        f.params.insert("strength".into(), Value::from(1e9));
        f.params.insert("axis".into(), Value::from("diagonal"));
        let p = P { fx: &f, info: info("shake").unwrap() };
        assert_eq!(p.num("strength"), 10.0);
        assert_eq!(p.choice("axis"), "both");
        let unknown = Fx { kind: "teleport".into(), ..f.clone() };
        assert_eq!(frame(std::slice::from_ref(&unknown), 0.5, &view()), FxFrame::default());
        assert!(warnings(&[unknown], 5.0).iter().any(|w| w.contains("unknown")));
        assert_eq!(parse_color("#ff8000"), Some([1.0, 128.0 / 255.0, 0.0]));
        assert_eq!(parse_color("rojo"), None);
    }

    #[test]
    fn situaciones_y_catalogo() {
        for s in SITUATIONS {
            let list = expand(s, "x", 2.0, None);
            assert_eq!(list.len(), s.parts.len(), "{}: todas sus piezas existen", s.id);
            assert!(list.iter().all(|f| f.start >= 2.0 && f.duration > 0.0));
            assert!(warnings(&list, 100.0).is_empty(), "{}: {:?}", s.id, warnings(&list, 100.0));
        }
        for i in CATALOG {
            for p in i.presets {
                for (k, _) in p.params {
                    assert!(i.params.iter().any(|s| s.key == *k), "{}.{}: mando {k} desconocido", i.kind, p.id);
                }
            }
        }
        let json: Value = serde_json::from_str(&catalog_json()).unwrap();
        assert_eq!(json["effects"].as_array().unwrap().len(), CATALOG.len());
        // Ida y vuelta por JSON.
        let list = expand(situation("explosion").unwrap(), "e", 1.0, Some(Point { x: 0.4, y: 0.6 }));
        let back: Vec<Fx> = serde_json::from_str(&serde_json::to_string(&list).unwrap()).unwrap();
        assert_eq!(back, list);
        assert!((end(&list) - 2.2).abs() < 1e-9);
    }
}
