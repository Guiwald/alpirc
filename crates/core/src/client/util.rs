//! Petits outils du client : adresses web dans un texte, mots entiers, couleur d'un pseudo.

/// Adresses trouvées dans un texte (codes de mIRC retirés) : `http://`, `https://`, `ftp://`, `irc://`,
/// `ircs://`, `www.`.
pub fn find_urls(text: &str) -> Vec<String> {
    let clean = alpirc_proto::format::strip(text);
    let mut out = Vec::new();
    for word in clean.split_whitespace() {
        let w = word.trim_start_matches(['(', '<', '[', '"', '\'']);
        let lower = w.to_ascii_lowercase();
        let ok = ["http://", "https://", "ftp://", "irc://", "ircs://"].iter().any(|p| lower.starts_with(p) && lower.len() > p.len())
            || (lower.starts_with("www.") && lower.len() > 4);
        if !ok {
            continue;
        }
        let mut url = w.to_string();
        // Ponctuation finale, et parenthèse fermante sans ouvrante.
        loop {
            let Some(last) = url.chars().last() else { break };
            let trim = match last {
                '.' | ',' | ';' | ':' | '!' | '?' | '>' | '"' | '\'' | ']' | '}' => true,
                ')' => url.matches('(').count() < url.matches(')').count(),
                _ => false,
            };
            if trim {
                url.pop();
            } else {
                break;
            }
        }
        if !url.is_empty() && !out.contains(&url) {
            out.push(url);
        }
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '[' | ']' | '\\' | '`' | '^' | '{' | '}' | '|')
}

/// Le texte contient-il `needle` en mot entier, sans tenir compte de la casse ?
pub fn contains_word(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    let h: Vec<char> = alpirc_proto::format::strip(hay).chars().map(lower).collect();
    let n: Vec<char> = needle.chars().map(lower).collect();
    if n.len() > h.len() {
        return false;
    }
    let nw = n.iter().all(|c| is_word_char(*c));
    for i in 0..=h.len() - n.len() {
        if h[i..i + n.len()] == n[..] {
            if !nw {
                return true;
            }
            let before = i == 0 || !is_word_char(h[i - 1]);
            let after = i + n.len() == h.len() || !is_word_char(h[i + n.len()]);
            if before && after {
                return true;
            }
        }
    }
    false
}

/// Couleur (mIRC) d'un pseudo, stable d'une session à l'autre.
pub fn nick_color(nick: &str) -> u8 {
    const COLORS: [u8; 10] = [2, 3, 4, 5, 6, 7, 10, 12, 13, 14];
    let mut h: u32 = 5381;
    for b in nick.to_ascii_lowercase().bytes() {
        h = h.wrapping_mul(33) ^ b as u32;
    }
    COLORS[(h % COLORS.len() as u32) as usize]
}

/// Sépare le premier mot du reste.
pub fn split_word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    match s.find(' ') {
        Some(i) => (&s[..i], s[i + 1..].trim_start()),
        None => (s, ""),
    }
}

/// Switches en tête (`-abc -d reste`) : (lettres, reste).
pub fn switches(args: &str) -> (String, &str) {
    let mut sw = String::new();
    let mut rest = args.trim_start();
    while rest.starts_with('-') && rest.len() > 1 && !rest[1..].starts_with([' ', '-']) {
        let (w, r) = split_word(rest);
        if w[1..].chars().all(|c| c.is_ascii_digit()) {
            break;
        }
        sw.push_str(&w[1..]);
        rest = r;
    }
    (sw, rest)
}

/// Nombre qui suit une lettre de switch (`u30` → 30).
pub fn switch_num(sw: &str, letter: char) -> Option<u64> {
    let i = sw.find(letter)?;
    let d: String = sw[i + 1..].chars().take_while(|c| c.is_ascii_digit()).collect();
    d.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(find_urls("see https://example.org/a_(b) and (www.x.fr)."), vec!["https://example.org/a_(b)", "www.x.fr"]);
        assert_eq!(find_urls("irc://irc.undernet.org/lyon, ftp://f.org!"), vec!["irc://irc.undernet.org/lyon", "ftp://f.org"]);
        assert!(find_urls("http:// nothing www.").is_empty());
        assert_eq!(find_urls("\x02http://b.org\x02"), vec!["http://b.org"]);
    }

    #[test]
    fn words() {
        assert!(contains_word("hey Guiwald!", "guiwald"));
        assert!(contains_word("GUIWALD: hi", "Guiwald"));
        assert!(!contains_word("Guiwalds", "Guiwald"));
        assert!(!contains_word("xGuiwald", "Guiwald"));
        assert!(contains_word("c'est génial", "génial"));
    }

    #[test]
    fn sw() {
        assert_eq!(switches("-u30 #c nick"), ("u30".to_string(), "#c nick"));
        assert_eq!(switch_num("ku30", 'u'), Some(30));
        assert_eq!(switches("-5 x"), (String::new(), "-5 x"));
    }
}
