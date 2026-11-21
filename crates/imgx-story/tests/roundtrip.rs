//! Escribir una historia y volver a leerla: proyecto, páginas, sonidos y datos calculados.

use imgx_core::Project;
use imgx_story::{ExportInput, Manifest, PageData, StorySound, Variant};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("imgx-story-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn img(w: u32, h: u32, format: image::ImageFormat) -> Vec<u8> {
    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(w, h, image::Rgb([240, 230, 200]))).write_to(&mut Cursor::new(&mut out), format).unwrap();
    out
}

fn project() -> Project {
    Project::from_json(
        r#"{"format":"imgx","version":1,"source":{"kind":"folder","name":"Demo"},
        "items":[{"entry":"01.tif","size":[40,60],"timeline":{"camera":[],"audio":{"voice":[{"sound":"voz1","start":0.5,"duration":1}],"sfx":[]}}},
                 {"entry":"02.jpg","size":[3000,4000]}],
        "ambient":[{"id":"a","sound":"builtin:rain","from":"01.tif","to":"02.jpg"}]}"#,
    )
    .unwrap()
}

fn export(path: &Path, variant: Variant, sound_file: &Path) -> imgx_story::Written {
    let sound_file = sound_file.to_owned();
    let pages = [img(40, 60, image::ImageFormat::Tiff), img(3000, 4000, image::ImageFormat::Jpeg)];
    let read = move |i: usize| Ok((pages[i].clone(), PageData { panels: Some(vec![[0.0, 0.0, 0.5, 1.0], [0.5, 0.0, 0.5, 1.0]]), levels: None, content: Some(if i == 0 { [0.1, 0.05, 0.8, 0.9] } else { imgx_story::NO_MARGIN }) }));
    let input = ExportInput {
        pages: vec!["01.tif".into(), "02.jpg".into()],
        read_page: &read,
        project: imgx_story::prepare::project(&project(), &["01.tif".into(), "02.jpg".into()], 0, 1, false, "Mi historia"),
        sounds: vec![StorySound { id: "voz1".into(), name: "Hola".into(), file: "voz1.wav".into(), kind: "recorded".into(), seconds: 1.0 }],
        read_sound: &|_| std::fs::read(&sound_file).map_err(|e| e.to_string()),
        manifest: Manifest { title: "Mi historia".into(), variant, ..Manifest::default() },
    };
    let mut steps = Vec::new();
    let w = imgx_story::write(path, input, &mut |d, t| {
        steps.push((d, t));
        true
    })
    .unwrap();
    assert_eq!(steps.last(), Some(&(3, 3)));
    w
}

#[test]
fn ida_y_vuelta() {
    let d = dir("ida");
    let wav = d.join("voz.wav");
    std::fs::write(&wav, b"RIFF falso: no se decodifica aqui").unwrap();

    for variant in [Variant::Desktop, Variant::Mobile] {
        let path = d.join(format!("{variant:?}.imgxstory"));
        let w = export(&path, variant, &wav);
        assert_eq!((w.pages, w.sounds), (2, 1));
        assert!(w.bytes > 0 && !d.join(format!("{variant:?}.imgxstory.tmp")).exists());

        let story = imgx_story::read(&path).unwrap();
        assert_eq!(story.manifest.title, "Mi historia");
        assert_eq!(story.manifest.variant, variant);
        assert_eq!(story.manifest.pages, 2);
        assert_eq!(story.manifest.cover.as_deref(), Some("cover.jpg"));
        // El TIFF pasa a PNG (en la de móvil, a JPEG, porque es opaco) y el proyecto lo sigue; el
        // ambiente de serie viaja como referencia.
        let first = if variant == Variant::Desktop { "01.png" } else { "01.jpg" };
        assert_eq!(story.project.items[0].entry, first);
        assert_eq!(story.project.ambient[0].from, first);
        assert_eq!(story.project.source.name, "Mi historia");
        assert_eq!(story.sounds[0].id, "voz1");
        assert_eq!(imgx_story::read_sound(&path, &story.sounds[0]).unwrap(), std::fs::read(&wav).unwrap());
        assert_eq!(story.computed.pages[first].panels.as_ref().unwrap().len(), 2);
        assert_eq!(story.computed.pages[first].content_box(), Some(Some([0.1, 0.05, 0.8, 0.9])));
        assert!(story.computed.pages.values().any(|d| d.content_box() == Some(None)), "sin márgenes");

        // Las páginas se abren como cualquier comprimido.
        let opened = imgx_media::open_path(&path).unwrap();
        let names: Vec<_> = opened.source.entries().iter().map(|e| e.name.clone()).collect();
        let big = image::load_from_memory(&opened.source.read(1).unwrap()).unwrap();
        match variant {
            Variant::Desktop => {
                assert_eq!(names, ["01.png", "02.jpg"]);
                assert_eq!((big.width(), big.height()), (3000, 4000));
                assert_eq!(story.project.items[1].size, [3000, 4000]);
            }
            Variant::Mobile => {
                assert_eq!(names, ["01.jpg", "02.jpg"]);
                assert_eq!((big.width(), big.height()), (1920, 2560));
                assert_eq!(story.project.items[1].size, [1920, 2560]);
            }
        }
    }
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn cancelar_no_deja_nada() {
    let d = dir("cancelar");
    let path = d.join("x.imgxstory");
    let read = |_: usize| Ok((img(10, 10, image::ImageFormat::Png), PageData::default()));
    let input = ExportInput { pages: vec!["a.png".into()], read_page: &read, project: project(), sounds: Vec::new(), read_sound: &|_| Err("sin sonidos".into()), manifest: Manifest::default() };
    let e = imgx_story::write(&path, input, &mut |_, _| false).unwrap_err();
    assert_eq!(e, imgx_story::CANCELLED);
    assert!(!path.exists() && !d.join("x.imgxstory.tmp").exists());
    std::fs::remove_dir_all(d).unwrap();
}

fn zip_with(path: &Path, files: &[(&str, &[u8])]) {
    let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (n, b) in files {
        z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(b).unwrap();
    }
    z.finish().unwrap();
}

#[test]
fn rechazos() {
    let d = dir("rechazos");
    let p = d.join("r.imgxstory");
    let project = project().to_json();
    let page = img(4, 4, image::ImageFormat::Png);
    let ok_manifest = br#"{"format":"imgxstory","version":1}"#;

    zip_with(&p, &[("manifest.json", br#"{"format":"imgxstory","version":99}"#), ("project.imgx", project.as_bytes()), ("pages/a.png", &page)]);
    assert!(imgx_story::read(&p).unwrap_err().contains("más nueva"));

    zip_with(&p, &[("manifest.json", br#"{"format":"imgxpack","version":1}"#), ("project.imgx", project.as_bytes()), ("pages/a.png", &page)]);
    assert!(imgx_story::read(&p).is_err());

    // Un sonido con ruta que intenta salir de `sounds/`.
    let bad = br#"[{"id":"x","name":"x","file":"../x.wav","kind":"recorded","seconds":1}]"#;
    zip_with(&p, &[("manifest.json", ok_manifest), ("project.imgx", project.as_bytes()), ("sounds.json", bad), ("pages/a.png", &page)]);
    assert!(imgx_story::read(&p).unwrap_err().contains("no válido"));

    // Un sonido anunciado que no está.
    let missing = br#"[{"id":"x","name":"x","file":"x.wav","kind":"recorded","seconds":1}]"#;
    zip_with(&p, &[("manifest.json", ok_manifest), ("project.imgx", project.as_bytes()), ("sounds.json", missing), ("pages/a.png", &page)]);
    assert!(imgx_story::read(&p).unwrap_err().contains("falta el sonido"));

    zip_with(&p, &[("manifest.json", ok_manifest), ("project.imgx", b"{}"), ("pages/a.png", &page)]);
    assert!(imgx_story::read(&p).unwrap_err().contains("project.imgx"));

    zip_with(&p, &[("manifest.json", ok_manifest), ("project.imgx", project.as_bytes())]);
    assert!(imgx_story::read(&p).unwrap_err().contains("páginas"));

    std::fs::write(&p, "hola").unwrap();
    assert!(imgx_story::read(&p).is_err());

    // Lo mínimo es válido.
    zip_with(&p, &[("manifest.json", ok_manifest), ("project.imgx", project.as_bytes()), ("pages/a.png", &page)]);
    assert!(imgx_story::read(&p).is_ok());
    std::fs::remove_dir_all(d).unwrap();
}
