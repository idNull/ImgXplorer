// Evita una consola extra en Windows en release (no afecta a Linux/macOS).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(text) = imgxplorer_lib::cli() {
        println!("{text}");
        return;
    }
    imgxplorer_lib::run()
}
