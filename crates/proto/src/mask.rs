//! Masques `pseudo!ident@hôte` : correspondance avec jokers (`*`, `?`) et types de masques de mIRC (`$mask`).

use crate::casemap::CaseMapping;

/// Correspondance avec jokers (`*` : n'importe quoi, `?` : un caractère), sans tenir compte de la casse.
pub fn wildcard_match(pattern: &str, text: &str, cm: CaseMapping) -> bool {
    let p: Vec<char> = pattern.chars().map(|c| cm.lower_char(c)).collect();
    let t: Vec<char> = text.chars().map(|c| cm.lower_char(c)).collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        // L'étoile d'abord : un `*` dans le texte ne doit pas être pris pour un caractère ordinaire du motif.
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Masque d'adresse selon les types 0 à 19 de mIRC (`$mask(adresse, type)`) :
/// 0 `*!ident@hôte`, 1 `*!*ident@hôte`, 2 `*!*@hôte`, 3 `*!*ident@*.domaine`, 4 `*!*@*.domaine`,
/// 5 `pseudo!ident@hôte`, 6 `pseudo!*ident@hôte`, 7 `pseudo!*@hôte`, 8 `pseudo!*ident@*.domaine`,
/// 9 `pseudo!*@*.domaine` ; 10 à 19 : idem en remplaçant les chiffres de l'hôte par `?`.
pub fn address_mask(nick: &str, user: &str, host: &str, kind: u8) -> String {
    let base = kind % 10;
    let numeric = kind >= 10;
    let user_star = user.trim_start_matches('~');
    let domain = wild_domain(host);
    let host_q = |h: &str| if numeric { h.chars().map(|c| if c.is_ascii_digit() { '?' } else { c }).collect() } else { h.to_string() };
    let n = if base >= 5 { nick } else { "*" };
    let (u, h): (String, String) = match base % 5 {
        0 => (user.to_string(), host_q(host)),
        1 => (format!("*{user_star}"), host_q(host)),
        2 => ("*".into(), host_q(host)),
        3 => (format!("*{user_star}"), host_q(&domain)),
        _ => ("*".into(), host_q(&domain)),
    };
    format!("{n}!{u}@{h}")
}

/// `a.b.example.org` → `*.example.org` ; adresse IPv4 `1.2.3.4` → `1.2.3.*`.
fn wild_domain(host: &str) -> String {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
        return format!("{}.{}.{}.*", parts[0], parts[1], parts[2]);
    }
    if host.contains(':') {
        // IPv6 : on garde les quatre premiers groupes.
        let g: Vec<&str> = host.split(':').collect();
        return format!("{}:*", g.iter().take(4).cloned().collect::<Vec<_>>().join(":"));
    }
    if parts.len() <= 2 {
        return host.to_string();
    }
    format!("*.{}", parts[1..].join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wild() {
        let cm = CaseMapping::Rfc1459;
        assert!(wildcard_match("*!*@*.fr", "Gui!~g@host.wanadoo.fr", cm));
        assert!(wildcard_match("gui*", "GUIWALD", cm));
        assert!(wildcard_match("g?i", "gui", cm));
        assert!(!wildcard_match("*.fr", "host.com", cm));
        assert!(wildcard_match("*", "", cm));
        assert!(wildcard_match("*", "*!*@host", cm));
        assert!(wildcard_match("*!*@*", "*!*@host", cm));
    }

    #[test]
    fn masks() {
        assert_eq!(address_mask("Gui", "~g", "a.b.fr", 2), "*!*@a.b.fr");
        assert_eq!(address_mask("Gui", "~g", "a.b.fr", 3), "*!*g@*.b.fr");
        assert_eq!(address_mask("Gui", "~g", "1.2.3.4", 4), "*!*@1.2.3.*");
        assert_eq!(address_mask("Gui", "~g", "a.b.fr", 5), "Gui!~g@a.b.fr");
        assert_eq!(address_mask("Gui", "g", "h12.b.fr", 12), "*!*@h??.b.fr");
    }
}
