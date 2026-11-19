//! Proveedor simulado: no sale a internet ni gasta nada. Devuelve sonido sintético con la duración
//! que tendría de verdad (unos 15 caracteres por segundo), para probar el flujo entero.
use std::collections::BTreeMap;
use std::sync::Mutex;

use super::provider::*;

#[derive(Default)]
pub struct Mock {
    used: Mutex<u64>,
    saved: Mutex<Vec<VoiceInfo>>,
}

/// Tono con «sílabas» (sube y baja como una voz) en WAV de 16 bits.
fn tone(seconds: f64, freq: f64, syllables: bool) -> Audio {
    let rate = 24_000u32;
    let n = (seconds.max(0.2) * rate as f64) as usize;
    let mut data = Vec::with_capacity(n * 2);
    for i in 0..n {
        let t = i as f64 / rate as f64;
        let env = if syllables { (0.5 - 0.5 * (t * std::f64::consts::TAU * 4.0).cos()).powf(0.7) } else { 1.0 };
        let fade = (t / 0.02).min(1.0) * ((seconds - t) / 0.05).clamp(0.0, 1.0);
        let s = (0.25 * env * fade * (t * freq * std::f64::consts::TAU).sin()) as f32;
        data.push(s);
        data.push(s);
    }
    Audio { bytes: wav_bytes(rate, &data), ext: "wav" }
}

fn wav_bytes(rate: u32, data: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(44 + data.len() * 2);
    let len = (data.len() * 2) as u32;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&len.to_le_bytes());
    for &s in data {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    out
}

/// Tono propio de cada voz (para oír que son distintas).
fn freq_of(id: &str) -> f64 {
    let h = id.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    140.0 + (h % 160) as f64
}

/// Duración aproximada de una frase: unos 15 caracteres por segundo, sin contar las etiquetas.
pub fn speech_seconds(text: &str) -> f64 {
    let mut depth = 0;
    let spoken: String = text
        .chars()
        .filter(|&c| {
            match c {
                '[' => depth += 1,
                ']' => depth = (depth - 1).max(0),
                _ => return depth == 0,
            }
            false
        })
        .collect();
    let chars = spoken.split_whitespace().collect::<Vec<_>>().join(" ").chars().count();
    0.4 + chars as f64 / 15.0
}

fn voice(id: &str, name: &str, desc: &str) -> VoiceInfo {
    VoiceInfo { id: id.into(), name: name.into(), category: Some("premade".into()), description: Some(desc.into()), labels: BTreeMap::new(), preview_url: None }
}

impl VoiceProvider for Mock {
    fn account(&self) -> Result<Account> {
        Ok(Account { tier: "simulado".into(), used: *self.used.lock().unwrap(), limit: 100_000, reset: None })
    }

    fn voices(&self) -> Result<Vec<VoiceInfo>> {
        let mut v = vec![
            voice("mock-narrador", "Narrador (simulado)", "Voz grave y pausada"),
            voice("mock-joven", "Joven (simulada)", "Voz clara y enérgica"),
            voice("mock-anciana", "Anciana (simulada)", "Voz suave y temblorosa"),
        ];
        v.extend(self.saved.lock().unwrap().iter().cloned());
        Ok(v)
    }

    fn design(&self, description: &str, _text: Option<&str>) -> Result<Vec<Preview>> {
        if description.trim().chars().count() < 20 {
            return Err(VoiceError("la descripción de la voz necesita al menos 20 caracteres".into()));
        }
        Ok((0..3)
            .map(|i| {
                let id = format!("mock-gen-{}-{i}", description.len());
                Preview { audio: tone(2.5, freq_of(&id), true), seconds: 2.5, generated_id: id }
            })
            .collect())
    }

    fn save_design(&self, name: &str, description: &str, generated_id: &str) -> Result<VoiceInfo> {
        let v = VoiceInfo { category: Some("generated".into()), ..voice(&format!("mock-voz-{generated_id}"), name, description) };
        self.saved.lock().unwrap().push(v.clone());
        Ok(v)
    }

    fn speak(&self, r: &SpeakReq) -> Result<Audio> {
        if r.text.trim().is_empty() {
            return Err(VoiceError("la frase está vacía".into()));
        }
        *self.used.lock().unwrap() += r.text.chars().count() as u64;
        let speed = r.settings.get("speed").copied().unwrap_or(1.0).clamp(0.7, 1.2);
        Ok(tone(speech_seconds(&r.text) / speed, freq_of(&r.voice) + r.seed.map_or(0.0, |s| (s % 24) as f64), true))
    }

    fn sfx(&self, text: &str, seconds: Option<f64>, _looped: bool) -> Result<Audio> {
        Ok(tone(seconds.unwrap_or(3.0).clamp(0.5, 30.0), 90.0 + (text.len() % 50) as f64, false))
    }

    fn music(&self, prompt: &str, seconds: f64, _instrumental: bool) -> Result<Audio> {
        Ok(tone(seconds.clamp(3.0, 600.0), 220.0 + (prompt.len() % 40) as f64, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duracion_sin_etiquetas() {
        let a = speech_seconds("[susurrando] Hola, ¿qué tal?");
        let b = speech_seconds("Hola, ¿qué tal?");
        assert!((a - b).abs() < 1e-9);
        assert!(b > 1.0 && b < 2.0);
    }

    #[test]
    fn habla_lo_que_toca() {
        let m = Mock::default();
        let r = SpeakReq { voice: "mock-joven".into(), model: "x".into(), text: "Una frase de prueba para medir".into(), language: None, settings: BTreeMap::new(), seed: None };
        let a = m.speak(&r).unwrap();
        let samples = (a.bytes.len() - 44) / 4;
        let secs = samples as f64 / 24_000.0;
        assert!((secs - speech_seconds(&r.text)).abs() < 0.01, "{secs}");
        assert_eq!(m.account().unwrap().used, 30);
    }
}
