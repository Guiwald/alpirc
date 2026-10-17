//! Expressions régulières de mIRC (`$regex`, `$regsub`, `$regsubex`, `on $*:TEXT:/…/:`), avec `fancy-regex`
//! (Rust pur, références arrière et assertions comprises).
//!
//! Formes acceptées : `/motif/options`, `m<délim>motif<délim>options`, ou le motif seul. Options : `i` (sans
//! casse), `g` (toutes les occurrences), `m` (multiligne), `s` (`.` prend les fins de ligne), `x` (espaces
//! ignorés), `U` (non gourmand), `S` (codes de couleur retirés du texte), `u` (sans effet : tout est en UTF-8).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use fancy_regex::{Regex, RegexBuilder};

/// Une expression compilée.
#[derive(Clone)]
pub struct Re {
    pub re: Regex,
    pub global: bool,
    pub strip: bool,
}

/// Cache des expressions compilées (les scripts réutilisent souvent les mêmes).
fn cache() -> &'static Mutex<HashMap<String, Re>> {
    static C: OnceLock<Mutex<HashMap<String, Re>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Compile un motif de mIRC.
pub fn compile(pat: &str) -> Result<Re, String> {
    if let Ok(c) = cache().lock()
        && let Some(r) = c.get(pat)
    {
        return Ok(r.clone());
    }
    let (body, flags) = split_delims(pat);
    let mut inline = String::new();
    for f in ['i', 'm', 's', 'x', 'U'] {
        if flags.contains(f) {
            inline.push(f);
        }
    }
    let src = if inline.is_empty() { body.to_string() } else { format!("(?{inline}){body}") };
    let re = RegexBuilder::new(&src).backtrack_limit(1_000_000).build().map_err(|e| e.to_string())?;
    let r = Re { re, global: flags.contains('g'), strip: flags.contains('S') };
    if let Ok(mut c) = cache().lock() {
        if c.len() > 500 {
            c.clear();
        }
        c.insert(pat.to_string(), r.clone());
    }
    Ok(r)
}

/// (motif, options).
fn split_delims(pat: &str) -> (&str, &str) {
    let (delim, start) = if pat.starts_with('/') {
        ('/', 1)
    } else if pat.len() > 2 && pat.starts_with('m') && !pat.as_bytes()[1].is_ascii_alphanumeric() {
        (pat[1..].chars().next().unwrap(), 1 + pat[1..].chars().next().unwrap().len_utf8())
    } else {
        return (pat, "");
    };
    match pat[start..].rfind(delim) {
        Some(e) => {
            let flags = &pat[start + e + delim.len_utf8()..];
            if flags.chars().all(|c| c.is_ascii_alphabetic()) {
                (&pat[start..start + e], flags)
            } else {
                (pat, "")
            }
        }
        None => (pat, ""),
    }
}

/// Une occurrence : texte et position (en caractères, à partir de 1) de chaque groupe, ou du tout s'il n'y a
/// pas de groupe.
#[derive(Debug, Clone, Default)]
pub struct Found {
    pub whole: (String, usize),
    /// Plage d'octets de l'occurrence entière.
    pub range: (usize, usize),
    pub groups: Vec<(String, usize)>,
}

fn char_pos(text: &str, byte: usize) -> usize {
    text[..byte].chars().count() + 1
}

/// Toutes les occurrences (une seule sans `g`).
pub fn find_all(r: &Re, text: &str) -> Vec<Found> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos <= text.len() {
        let caps = match r.re.captures_from_pos(text, pos) {
            Ok(Some(c)) => c,
            _ => break,
        };
        let m = caps.get(0).unwrap();
        let mut f = Found {
            whole: (m.as_str().to_string(), char_pos(text, m.start())),
            range: (m.start(), m.end()),
            groups: Vec::new(),
        };
        for g in 1..caps.len() {
            match caps.get(g) {
                Some(x) => f.groups.push((x.as_str().to_string(), char_pos(text, x.start()))),
                None => f.groups.push((String::new(), 0)),
            }
        }
        out.push(f);
        if !r.global {
            break;
        }
        pos = if m.end() == m.start() {
            // Occurrence vide : on avance d'un caractère.
            match text[m.end()..].chars().next() {
                Some(c) => m.end() + c.len_utf8(),
                None => break,
            }
        } else {
            m.end()
        };
    }
    out
}

/// Remplace `\0`…`\9` (et `\t` : l'occurrence, `\n` : son numéro) dans un texte de remplacement.
pub fn expand(sub: &str, f: &Found, n: usize, total: usize, zero_is_count: bool) -> String {
    let mut out = String::new();
    let mut it = sub.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.peek().copied() {
                Some(d @ '0'..='9') => {
                    it.next();
                    let k = d.to_digit(10).unwrap() as usize;
                    if k == 0 {
                        if zero_is_count {
                            out.push_str(&total.to_string());
                        } else {
                            out.push_str(&f.whole.0);
                        }
                    } else if let Some(g) = f.groups.get(k - 1) {
                        out.push_str(&g.0);
                    }
                    continue;
                }
                Some('t') => {
                    it.next();
                    out.push_str(&f.whole.0);
                    continue;
                }
                Some('n') => {
                    it.next();
                    out.push_str(&n.to_string());
                    continue;
                }
                Some('\\') => {
                    it.next();
                    out.push('\\');
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        let r = compile("/(b)/").unwrap();
        let f = find_all(&r, "abcb");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].groups[0], ("b".to_string(), 2));
        let r = compile("/B/gi").unwrap();
        assert_eq!(find_all(&r, "abcb").len(), 2);
        let r = compile("m#a/b#").unwrap();
        assert_eq!(find_all(&r, "xa/b").len(), 1);
        let r = compile("^x").unwrap();
        assert_eq!(find_all(&r, "xa").len(), 1);
    }

    #[test]
    fn expansion() {
        let r = compile("/(\\w)(\\w)/").unwrap();
        let f = &find_all(&r, "ab")[0];
        assert_eq!(expand("\\2\\1-\\0", f, 1, 1, false), "ba-ab");
    }
}
