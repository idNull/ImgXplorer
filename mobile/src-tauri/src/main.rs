// En el ordenador, el visor móvil se abre como una ventana normal (para probar la interfaz
// sin el teléfono). En el móvil no se usa: Tauri arranca `run` desde la biblioteca.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    imgxplorer_mobile_lib::run()
}
