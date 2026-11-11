//! Decodifica todas las imágenes de una carpeta o comprimido y muestra tamaño y tiempos.
//! Útil para probar archivos reales (RAW de tu cámara, HEIC del móvil…):
//!
//! ```sh
//! IMGX_SAMPLES=~/Imágenes cargo test -p imgx-media --release --test samples -- --ignored --nocapture
//! ```

use std::time::Instant;

#[test]
#[ignore = "necesita IMGX_SAMPLES"]
fn decodificar_muestras() {
    let Ok(dir) = std::env::var("IMGX_SAMPLES") else { return };
    let opened = imgx_media::open_path(std::path::Path::new(&dir)).expect("abrir");
    let src = opened.source;
    let mut failures = 0;
    for (i, e) in src.entries().iter().enumerate() {
        let t0 = Instant::now();
        let bytes = src.read(i).expect("leer");
        let read = t0.elapsed();
        let full = imgx_media::decode(&e.name, &bytes);
        let dec = t0.elapsed() - read;
        let thumb = imgx_media::thumbnail_jpeg(&e.name, &bytes, 320, [0, 0, 0]);
        let t1 = Instant::now();
        let panels = imgx_media::panels::detect_panels(&e.name, &bytes).map(|p| p.len()).unwrap_or(0);
        let panels_ms = t1.elapsed().as_millis();
        match (&full, &thumb) {
            (Ok(img), Ok(t)) => println!(
                "ok  {:<28} {:>5}×{:<5} lectura {:>4} ms · decodificación {:>5} ms · miniatura {} KB · {} viñetas ({} ms){}",
                e.name, img.width, img.height, read.as_millis(), dec.as_millis(), t.len() / 1024, panels, panels_ms,
                if imgx_media::needs_decode(&e.name) { "" } else { "  (en la app la decodifica el webview)" }
            ),
            _ => {
                failures += 1;
                println!("ERR {:<28} {:?} / {:?}", e.name, full.err(), thumb.err());
            }
        }
    }
    assert_eq!(failures, 0, "{failures} imágenes no se pudieron decodificar");
}
