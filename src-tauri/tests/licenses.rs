//! La lista de licencias de terceros del «Acerca de» (ui/src/lib/about/licenses.json) tiene que
//! corresponder a las dependencias actuales. Si esta prueba falla: `pnpm third-party`.
//!
//! La huella es la misma que calcula `lockHash()` en packaging/licenses.mjs.

use std::path::Path;

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

fn lock_hash(root: &Path) -> String {
    let cargo = std::fs::read_to_string(root.join("Cargo.lock")).unwrap().replace('\r', "");
    let mut pkgs: Vec<String> = cargo
        .split("[[package]]\n")
        .skip(1)
        .filter_map(|block| {
            let mut lines = block.lines();
            let name = lines.next()?.strip_prefix("name = \"")?.strip_suffix('"')?;
            let version = lines.next()?.strip_prefix("version = \"")?.strip_suffix('"')?;
            (!name.starts_with("imgx")).then(|| format!("{name} {version}"))
        })
        .collect();
    pkgs.sort();
    let pnpm = std::fs::read_to_string(root.join("pnpm-lock.yaml")).unwrap().replace('\r', "");
    format!("{:016x}", fnv1a(format!("{}\n--\n{pnpm}", pkgs.join("\n")).as_bytes()))
}

#[test]
fn la_lista_de_licencias_esta_al_dia() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("ui/src/lib/about/licenses.json")).unwrap()).unwrap();
    assert_eq!(
        json["lockHash"].as_str(),
        Some(lock_hash(root).as_str()),
        "la lista de licencias de terceros no corresponde a Cargo.lock / pnpm-lock.yaml: ejecuta «pnpm third-party»"
    );
    let names: Vec<&str> = json["packages"].as_array().unwrap().iter().filter_map(|p| p["name"].as_str()).collect();
    for must in ["tauri", "image", "symphonia", "svelte"] {
        assert!(names.contains(&must), "falta {must} en la lista");
    }
    assert!(json["special"].as_array().unwrap().iter().any(|s| s["name"] == "UnRAR"), "falta el aviso de UnRAR");
}
