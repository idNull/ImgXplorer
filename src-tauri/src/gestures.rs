//! Puente de gestos del touchpad.
//!
//! - **Linux (WebKitGTK):** el pellizco no llega a JavaScript; WebKit lo usa para el
//!   zoom de página. Colocamos un `GtkGestureZoom` en fase *capture* sobre el webview,
//!   reclamamos la secuencia (así WebKit no hace zoom de página) y la reenviamos como
//!   evento `pinch` a la interfaz.
//! - **macOS (WKWebView):** la interfaz recibe `gesturestart/gesturechange` directamente.

#[cfg(target_os = "linux")]
#[derive(Clone, serde::Serialize)]
struct Pinch {
    phase: &'static str,
    /// Escala acumulada desde el inicio del gesto.
    scale: f64,
    /// Centro del gesto en píxeles CSS del webview.
    x: f64,
    y: f64,
}

#[cfg(target_os = "linux")]
pub fn install(window: &tauri::WebviewWindow) {
    use gtk::prelude::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use tauri::{Emitter, Manager};

    let app = window.app_handle().clone();
    let result = window.with_webview(move |wv| {
        let view = wv.inner();
        let gesture = gtk::GestureZoom::new(&view);
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

        // Último centro y escala conocidos. IMPORTANTE: no consultar a GTK en `end`/`cancel`:
        // con gestos de touchpad, `gtk_gesture_get_bounding_box_center` durante el fin del
        // gesto accede a un evento ya liberado y provoca SIGSEGV (GTK 3). Solo se consulta
        // en `begin` y `scale-changed`, cuando el gesto tiene puntos activos.
        let last = Rc::new(Cell::new((0.0f64, 0.0f64, 1.0f64)));

        let emit = {
            let app = app.clone();
            move |phase: &'static str, (x, y, scale): (f64, f64, f64)| {
                let _ = app.emit("pinch", Pinch { phase, scale, x, y });
            }
        };
        let center = |g: &gtk::GestureZoom, prev: (f64, f64, f64)| {
            if g.is_active() { g.bounding_box_center().unwrap_or((prev.0, prev.1)) } else { (prev.0, prev.1) }
        };

        {
            let (last, emit) = (last.clone(), emit.clone());
            gesture.connect_begin(move |g, _| {
                g.set_state(gtk::EventSequenceState::Claimed);
                let (x, y) = center(g, last.get());
                last.set((x, y, 1.0));
                log::debug!("pellizco: inicio en ({x:.0}, {y:.0})");
                emit("begin", last.get());
            });
        }
        {
            let (last, emit) = (last.clone(), emit.clone());
            gesture.connect_scale_changed(move |g, scale| {
                if !scale.is_finite() || scale <= 0.0 {
                    log::warn!("pellizco: escala inválida {scale}");
                    return;
                }
                let (x, y) = center(g, last.get());
                last.set((x, y, scale));
                emit("update", last.get());
            });
        }
        {
            let (last, emit) = (last.clone(), emit.clone());
            gesture.connect_end(move |_, _| {
                log::debug!("pellizco: fin (escala {:.3})", last.get().2);
                emit("end", last.get());
            });
        }
        gesture.connect_cancel(move |_, _| {
            log::debug!("pellizco: cancelado");
            emit("end", last.get());
        });

        // En GTK3 el widget no es dueño del gesto: hay que mantenerlo vivo
        // mientras exista la ventana (una sola ventana → vive todo el proceso).
        std::mem::forget(gesture);
    });
    if let Err(e) = result {
        log::error!("no se pudo instalar el gesto de pellizco: {e}");
    }
}

#[cfg(not(target_os = "linux"))]
pub fn install(_window: &tauri::WebviewWindow) {}
