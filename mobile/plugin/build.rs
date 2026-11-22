// La interfaz solo escucha los avisos del complemento (`addPluginListener`): por eso expone las
// órdenes de registrar y quitar oyentes. Lo demás lo usa Rust (crate imgxplorer-mobile).
const COMMANDS: &[&str] = &["register_listener", "remove_listener"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).android_path("android").build();
}
