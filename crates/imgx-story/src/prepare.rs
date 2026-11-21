//! El proyecto que va dentro de una historia: solo las imágenes del rango, los ambientes
//! recortados a él y, si se pide, sin las tomas anteriores de las voces.

use imgx_core::Project;
use std::collections::BTreeSet;

pub const BUILTIN: &str = "builtin:";

/// Copia del proyecto para las imágenes `order[from..=to]` (`order`: todas, en el orden de la
/// fuente). Con `keep_takes = false`, cada frase del guion se queda solo con la toma que suena.
pub fn project(project: &Project, order: &[String], from: usize, to: usize, keep_takes: bool, title: &str) -> Project {
    let pos = |name: &str| order.iter().position(|n| n == name);
    let inside = |name: &str| pos(name).is_some_and(|i| (from..=to).contains(&i));
    let mut p = project.clone();
    p.source = imgx_core::schema::Source { kind: imgx_core::schema::SourceKind::Archive, name: title.to_owned(), fingerprint: None };
    p.items.retain(|it| inside(&it.entry));
    p.ambient = p
        .ambient
        .into_iter()
        .filter_map(|mut a| {
            let (start, end) = (pos(&a.from)?.max(from), pos(&a.to)?.min(to));
            if start > end {
                return None;
            }
            a.from.clone_from(&order[start]);
            a.to.clone_from(&order[end]);
            Some(a)
        })
        .collect();
    if !keep_takes {
        for line in p.items.iter_mut().flat_map(|it| it.lines.iter_mut()) {
            if line.takes.len() > 1 {
                line.takes.drain(..line.takes.len() - 1);
            }
        }
    }
    p
}

/// Los sonidos propios que usa el proyecto (los de serie los fabrica el motor y no viajan).
pub fn sounds(project: &Project) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for it in &project.items {
        out.extend(it.timeline.audio.voice.iter().chain(&it.timeline.audio.sfx).map(|c| c.sound.clone()));
        out.extend(it.lines.iter().flat_map(|l| l.takes.iter().cloned()));
    }
    out.extend(project.ambient.iter().map(|a| a.sound.clone()));
    out.retain(|s| !s.is_empty() && !s.starts_with(BUILTIN));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Project {
        Project::from_json(
            r#"{"format":"imgx","version":1,"source":{"kind":"folder","name":"Mi cómic","fingerprint":"abc"},
            "items":[
              {"entry":"01.jpg","size":[10,20],"timeline":{"camera":[],"audio":{"voice":[{"sound":"v2","start":0,"duration":1,"gen":"l1"}],"sfx":[{"sound":"builtin:hit","start":0,"duration":1}]}},
               "lines":[{"id":"l1","text":"Hola","takes":["v1","v2"]}]},
              {"entry":"02.jpg","size":[10,20]},
              {"entry":"03.jpg","size":[10,20],"timeline":{"camera":[],"audio":{"voice":[],"sfx":[{"sound":"s3","start":0,"duration":1}]}}}],
            "ambient":[{"id":"a","sound":"lluvia","from":"01.jpg","to":"03.jpg"},{"id":"b","sound":"builtin:rain","from":"03.jpg","to":"03.jpg"}]}"#,
        )
        .unwrap()
    }

    fn order() -> Vec<String> {
        ["01.jpg", "02.jpg", "03.jpg"].map(String::from).to_vec()
    }

    #[test]
    fn todo_el_comic() {
        let p = project(&sample(), &order(), 0, 2, true, "Historia");
        assert_eq!(p.items.len(), 3);
        assert_eq!(p.source.name, "Historia");
        assert_eq!(p.source.fingerprint, None);
        assert_eq!(p.items[0].lines[0].takes, ["v1", "v2"]);
        assert_eq!(sounds(&p).into_iter().collect::<Vec<_>>(), ["lluvia", "s3", "v1", "v2"]);
    }

    #[test]
    fn rango_y_tomas() {
        let p = project(&sample(), &order(), 0, 1, false, "T");
        assert_eq!(p.items.iter().map(|i| i.entry.as_str()).collect::<Vec<_>>(), ["01.jpg", "02.jpg"]);
        // El ambiente que llegaba a la 03 se recorta a la 02; el que solo sonaba en la 03 se va.
        assert_eq!(p.ambient.len(), 1);
        assert_eq!((p.ambient[0].from.as_str(), p.ambient[0].to.as_str()), ("01.jpg", "02.jpg"));
        assert_eq!(p.items[0].lines[0].takes, ["v2"]);
        assert_eq!(sounds(&p).into_iter().collect::<Vec<_>>(), ["lluvia", "v2"]);
    }

    #[test]
    fn rango_por_el_final() {
        let p = project(&sample(), &order(), 2, 2, true, "T");
        assert_eq!(p.items.len(), 1);
        assert_eq!(p.ambient.iter().map(|a| (a.from.as_str(), a.to.as_str())).collect::<Vec<_>>(), [("03.jpg", "03.jpg"), ("03.jpg", "03.jpg")]);
    }
}
