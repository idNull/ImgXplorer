//! Orden natural: "pag2.jpg" < "pag10.jpg", sin distinguir mayúsculas.

use std::cmp::Ordering;

pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.as_bytes(), b.as_bytes());
    loop {
        match (a.first(), b.first()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na = a.iter().take_while(|c| c.is_ascii_digit()).count();
                let nb = b.iter().take_while(|c| c.is_ascii_digit()).count();
                let (da, db) = (trim_zeros(&a[..na]), trim_zeros(&b[..nb]));
                let ord = da.len().cmp(&db.len()).then_with(|| da.cmp(db)).then_with(|| na.cmp(&nb));
                if ord != Ordering::Equal {
                    return ord;
                }
                a = &a[na..];
                b = &b[nb..];
            }
            (Some(x), Some(y)) => {
                let ord = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                a = &a[1..];
                b = &b[1..];
            }
        }
    }
}

fn trim_zeros(d: &[u8]) -> &[u8] {
    let i = d.iter().position(|&c| c != b'0').unwrap_or(d.len());
    &d[i..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordena() {
        let mut v = vec!["pag10.jpg", "Pag2.jpg", "pag1.jpg", "cap1/p3.png", "pag002.jpg", "a.png"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["a.png", "cap1/p3.png", "pag1.jpg", "Pag2.jpg", "pag002.jpg", "pag10.jpg"]);
    }
}
