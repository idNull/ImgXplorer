// La interfaz no llama al complemento: lo usa Rust (crate imgxplorer-mobile), así que no
// expone órdenes ni permisos propios.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).android_path("android").build();
}
