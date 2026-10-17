//! Jokers de mIRC : `*` (n'importe quelle suite), `?` (un caractère), `&` (un mot entier, sans espace).
//!
//! Sert à `iswm`, au texte des événements (`on *:TEXT:!seen &:#:`), à `$wildtok`, `$hfind(…,w)`, etc.

/// Forme « pliée » d'un caractère pour les comparaisons sans casse.
pub fn fold(c: char) -> char {
    if c.is_ascii() { c.to_ascii_lowercase() } else { c.to_lowercase().next().unwrap_or(c) }
}

/// Égalité sans tenir compte de la casse.
pub fn fold_eq(a: &str, b: &str) -> bool {
    a.chars().map(fold).eq(b.chars().map(fold))
}

/// Correspondance avec jokers ; `cs` : en tenant compte de la casse.
pub fn wildmatch(pattern: &str, text: &str, cs: bool) -> bool {
    let conv = |c: char| if cs { c } else { fold(c) };
    let p: Vec<char> = pattern.chars().map(conv).collect();
    let t: Vec<char> = text.chars().map(conv).collect();
    if p.contains(&'&') { dp_match(&p, &t) } else { star_match(&p, &t) }
}

/// Algorithme classique (retour sur la dernière étoile), linéaire en pratique.
fn star_match(p: &[char], t: &[char]) -> bool {
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        // L'étoile d'abord : le texte peut contenir lui-même un `*` (masques `*!*@hôte`).
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

/// Programmation dynamique quand le motif contient `&` (un mot : au moins un caractère, aucune espace).
fn dp_match(p: &[char], t: &[char]) -> bool {
    let (n, m) = (p.len(), t.len());
    // ok[i][j] : p[i..] correspond à t[j..]
    let mut ok = vec![vec![false; m + 1]; n + 1];
    ok[n][m] = true;
    for i in (0..n).rev() {
        for j in (0..=m).rev() {
            ok[i][j] = match p[i] {
                '*' => ok[i + 1][j] || (j < m && ok[i][j + 1]),
                '?' => j < m && ok[i + 1][j + 1],
                '&' => {
                    let mut k = j;
                    let mut found = false;
                    while k < m && t[k] != ' ' {
                        k += 1;
                        if ok[i + 1][k] {
                            found = true;
                            break;
                        }
                    }
                    found
                }
                c => j < m && t[j] == c && ok[i + 1][j + 1],
            };
        }
    }
    ok[0][0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jokers() {
        assert!(wildmatch("*hi*", "oh HI there", false));
        assert!(!wildmatch("*hi*", "oh HI there", true));
        assert!(wildmatch("!seen &", "!seen bob", false));
        assert!(!wildmatch("!seen &", "!seen bob alice", false));
        assert!(wildmatch("!seen & *", "!seen bob alice", false));
        assert!(wildmatch("a?c", "abc", false));
        assert!(wildmatch("*", "", false));
        assert!(!wildmatch("&", "", false));
        // Texte qui contient un `*`.
        assert!(wildmatch("*", "*!*@host", false));
        assert!(wildmatch("*@host", "*!*@host", false));
        assert!(wildmatch("a*c", "a*c", false));
    }
}
