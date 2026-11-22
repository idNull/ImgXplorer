fn main() {
    android_unrar();
    tauri_build::build()
}

/// UnRAR (para `.cbr`) es C++ y su build pide `-lpthread`, que en Android no existe como archivo:
/// los hilos van dentro de la libc del sistema. Se le da una `libpthread.a` vacía y se enlaza la
/// biblioteca estándar de C++ del NDK de forma estática (el crate no la enlaza por su cuenta).
fn android_unrar() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("android-stubs");
    std::fs::create_dir_all(&out).unwrap();
    // Un archivo `ar` vacío es solo su firma.
    std::fs::write(out.join("libpthread.a"), b"!<arch>\n").unwrap();
    println!("cargo:rustc-link-search=native={}", out.display());
    // Las de C++ las encuentra el enlazador del NDK en su sysroot (por eso sin «static=», que
    // obligaría a rustc a buscarlas él mismo).
    println!("cargo:rustc-link-lib=c++_static");
    println!("cargo:rustc-link-lib=c++abi");
}
