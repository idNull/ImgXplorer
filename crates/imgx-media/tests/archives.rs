//! Comprimidos generados al vuelo: orden natural, filtrado y lectura (incluido 7z sólido).

use imgx_media::{Source, SevenZSource, ZipSource, open_path};
use std::io::{Cursor, Write};

/// Nombres desordenados, con subcarpeta, basura de macOS y un archivo que no es imagen.
const NAMES: &[&str] = &["cap 2/p10.png", "cap 1/p2.png", "__MACOSX/._p1.png", "cap 1/p1.png", "leeme.txt", "cap 2/p9.png"];
const EXPECTED: &[&str] = &["cap 1/p1.png", "cap 1/p2.png", "cap 2/p9.png", "cap 2/p10.png"];

/// Contenido distinto por entrada para comprobar que se lee la correcta.
fn payload(name: &str) -> Vec<u8> {
    format!("contenido de {name} ").repeat(500).into_bytes()
}

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("imgx-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn check(src: &dyn Source) {
    let names: Vec<_> = src.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, EXPECTED);
    // Orden de lectura salteado: hacia atrás y adelante.
    for i in [3, 0, 2, 1, 3] {
        assert_eq!(src.read(i).unwrap(), payload(EXPECTED[i]), "entrada {i}");
    }
}

#[test]
fn zip() {
    let path = tmp("t.cbz");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for n in NAMES {
        w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(&payload(n)).unwrap();
    }
    w.finish().unwrap();
    check(&ZipSource::open(&path).unwrap());
    assert_eq!(open_path(&path).unwrap().source.entries().len(), 4);
}

/// Una historia `.imgxstory`: solo cuentan las imágenes de `pages/`, con el prefijo quitado.
#[test]
fn historia_solo_paginas() {
    let path = tmp("t.imgxstory");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for n in NAMES {
        w.start_file(format!("pages/{n}"), zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(&payload(n)).unwrap();
    }
    // Fuera de `pages/`: la portada y los sonidos no son páginas.
    for extra in ["cover.jpg", "sounds/a.png", "manifest.json"] {
        w.start_file(extra, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(b"x").unwrap();
    }
    w.finish().unwrap();
    let opened = open_path(&path).unwrap();
    assert_eq!(opened.kind, imgx_media::SourceKind::Archive);
    check(opened.source.as_ref());
}

#[test]
fn sevenz_solido() {
    let path = tmp("t.cb7");
    let mut w = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
    let entries = NAMES.iter().map(|n| sevenz_rust2::ArchiveEntry::new_file(n)).collect();
    let readers = NAMES.iter().map(|n| Cursor::new(payload(n)).into()).collect();
    w.push_archive_entries(entries, readers).unwrap();
    w.finish().unwrap();
    let archive = sevenz_rust2::Archive::open(&path).unwrap();
    assert!(archive.is_solid, "la prueba debe cubrir el caso sólido");
    check(&SevenZSource::open(&path).unwrap());
}
