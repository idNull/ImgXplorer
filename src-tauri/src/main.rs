// Evita una consola extra en Windows en release (no afecta a Linux/macOS).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Puente MCP para las IAs: habla por stdin/stdout y no abre ventana.
    if std::env::args().nth(1).as_deref() == Some("--mcp") {
        imgxplorer_lib::mcp::serve();
        return;
    }
    if let Some(text) = imgxplorer_lib::cli() {
        println!("{text}");
        return;
    }
    imgxplorer_lib::run()
}
