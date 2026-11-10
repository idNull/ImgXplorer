// Evita una consola extra en Windows en release (no afecta a Linux/macOS).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    imgxplorer_lib::run()
}
