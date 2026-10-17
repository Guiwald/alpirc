//! Conditions de `if`, `while` et `$iif` : comparaisons (`==`, `isin`, `iswm`, `isop`…), `!`, `&&`, `||`,
//! parenthèses, valeur seule. `&&` et `||` s'enchaînent de gauche à droite, sans priorité, comme dans mIRC.
//! Chaque comparaison remplit `$v1` et `$v2` (`$ifmatch` = `$v1`).

use crate::ast::matching_paren;
use crate::calc::parse_num;
use crate::exec::R;
use crate::wild::{fold, wildmatch};
use crate::{Ctx, Engine, Host};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Logic {
    And,
    Or,
}

/// Découpe aux `&&` / `||` du niveau le plus haut.
fn split_logic(s: &str) -> Vec<(Option<Logic>, &str)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let mut op = None;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            c @ (b'&' | b'|') if depth == 0 && i + 1 < b.len() && b[i + 1] == c => {
                let before_ok = i == 0 || b[i - 1] == b' ' || b[i - 1] == b')';
                let after_ok = i + 2 >= b.len() || matches!(b[i + 2], b' ' | b'(' | b'!' | b'$' | b'%');
                if before_ok && after_ok {
                    out.push((op, &s[start..i]));
                    op = Some(if c == b'&' { Logic::And } else { Logic::Or });
                    start = i + 2;
                    i += 2;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out.push((op, &s[start..]));
    out
}

/// Mots du niveau le plus haut (hors parenthèses) : (début, fin) dans le texte.
fn top_words(s: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = None;
    for (i, c) in s.char_indices() {
        match c {
            '(' => {
                depth += 1;
                start.get_or_insert(i);
            }
            ')' => {
                depth -= 1;
                start.get_or_insert(i);
            }
            ' ' if depth <= 0 => {
                if let Some(st) = start.take() {
                    out.push((st, i));
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(st) = start {
        out.push((st, s.len()));
    }
    out
}

const WORD_OPS: &[&str] = &[
    "isin", "isincs", "iswm", "iswmcs", "isnum", "isletter", "isalnum", "isalpha", "islower", "isupper", "ison",
    "isop", "ishop", "isvoice", "isreg", "ischan", "isban", "isinvite", "isexcept", "isquiet", "isignore",
    "isprotect", "isaop", "isavoice", "isnotify", "isowner", "isadmin",
];
const SYM_OPS: &[&str] = &["==", "===", "!=", "!==", "<", ">", "<=", ">=", "//", "\\\\", "&"];

/// Opérateur (sans `!`) et négation, si le mot en est un.
fn as_operator(w: &str) -> Option<(&'static str, bool)> {
    if let Some(op) = SYM_OPS.iter().find(|o| **o == w) {
        return Some(match *op {
            "!=" => ("==", true),
            "!==" => ("===", true),
            o => (o, false),
        });
    }
    let lw = w.to_ascii_lowercase();
    let (neg, base) = match lw.strip_prefix('!') {
        Some(r) => (true, r.to_string()),
        None => (false, lw),
    };
    WORD_OPS.iter().find(|o| **o == base).map(|o| (*o, neg))
}

fn truthy(v: &str) -> bool {
    !(v.is_empty() || v == "0" || v.eq_ignore_ascii_case("$false"))
}

fn eq_ci(a: &str, b: &str) -> bool {
    a.chars().map(fold).eq(b.chars().map(fold))
}

/// Comparaison de deux valeurs : numérique si les deux sont des nombres, textuelle (sans casse) sinon.
fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    match (parse_num(a), parse_num(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        _ => {
            let x: String = a.chars().map(fold).collect();
            let y: String = b.chars().map(fold).collect();
            x.cmp(&y)
        }
    }
}

/// Plage de `isnum` : `N`, `N-M`, `N-`, `-M`.
fn in_range(v: f64, r: &str) -> bool {
    let r = r.trim();
    if r.is_empty() {
        return true;
    }
    for (i, _) in r.match_indices('-') {
        if i == 0 && r.len() > 1 && parse_num(r).is_some() {
            continue;
        }
        let (a, b) = (&r[..i], &r[i + 1..]);
        let lo = if a.is_empty() { Some(f64::MIN) } else { parse_num(a) };
        let hi = if b.is_empty() { Some(f64::MAX) } else { parse_num(b) };
        if let (Some(lo), Some(hi)) = (lo, hi) {
            return v >= lo && v <= hi;
        }
    }
    parse_num(r).is_some_and(|x| x == v)
}

impl Engine {
    /// Évalue une condition.
    pub(crate) fn eval_cond(&mut self, h: &mut dyn Host, ctx: &Ctx, cond: &str) -> R<bool> {
        let mut acc: Option<bool> = None;
        for (op, part) in split_logic(cond) {
            match (op, acc) {
                (Some(Logic::And), Some(false)) | (Some(Logic::Or), Some(true)) => {}
                _ => acc = Some(self.cond_atom(h, ctx, part)?),
            }
        }
        Ok(acc.unwrap_or(false))
    }

    fn cond_atom(&mut self, h: &mut dyn Host, ctx: &Ctx, s: &str) -> R<bool> {
        let s = s.trim();
        if s.is_empty() {
            return Ok(false);
        }
        if let Some(r) = s.strip_prefix('!')
            && !r.starts_with('=')
            && r.trim_start().starts_with(['$', '%', '(', '!'])
        {
            let r = r.trim_start();
            // `!$1 == x` : la négation porte sur la comparaison entière.
            return Ok(!self.cond_atom(h, ctx, r)?);
        }
        if s.starts_with('(') && matching_paren(s) == Some(s.len() - 1) {
            return self.eval_cond(h, ctx, &s[1..s.len() - 1]);
        }
        self.comparison(h, ctx, s)
    }

    fn comparison(&mut self, h: &mut dyn Host, ctx: &Ctx, s: &str) -> R<bool> {
        let words = top_words(s);
        let mut found = None;
        for (k, &(a, b)) in words.iter().enumerate() {
            if k == 0 {
                continue;
            }
            if let Some(op) = as_operator(&s[a..b]) {
                found = Some((a, b, op));
                break;
            }
        }
        let Some((a, b, (op, neg))) = found else {
            let v = self.eval_text(h, ctx, s)?;
            let t = truthy(&v);
            self.rt.v1 = v;
            self.rt.v2.clear();
            return Ok(t);
        };
        let left = self.eval_text(h, ctx, s[..a].trim())?;
        let right = self.eval_text(h, ctx, s[b..].trim())?;
        let r = self.apply_op(h, ctx, op, &left, &right);
        self.rt.v1 = left;
        self.rt.v2 = right;
        Ok(r != neg)
    }

    fn apply_op(&mut self, h: &mut dyn Host, ctx: &Ctx, op: &str, l: &str, r: &str) -> bool {
        use std::cmp::Ordering::*;
        let ctx = self.eff_ctx(ctx).into_owned();
        let ctx = &ctx;
        match op {
            "==" => match (parse_num(l), parse_num(r)) {
                (Some(x), Some(y)) => x == y,
                _ => eq_ci(l, r),
            },
            "===" => l == r,
            "<" => compare(l, r) == Less,
            ">" => compare(l, r) == Greater,
            "<=" => compare(l, r) != Greater,
            ">=" => compare(l, r) != Less,
            "//" | "\\\\" => {
                let (x, y) = (parse_num(l).unwrap_or(0.0), parse_num(r).unwrap_or(0.0));
                let multiple = x != 0.0 && (y % x) == 0.0;
                if op == "//" { multiple } else { !multiple }
            }
            "&" => {
                let x = parse_num(l).unwrap_or(0.0) as i64;
                let y = parse_num(r).unwrap_or(0.0) as i64;
                x & y != 0
            }
            "isin" => r.chars().map(fold).collect::<String>().contains(&l.chars().map(fold).collect::<String>()),
            "isincs" => r.contains(l),
            "iswm" => wildmatch(l, r, false),
            "iswmcs" => wildmatch(l, r, true),
            "isnum" => parse_num(l).is_some_and(|v| in_range(v, r)),
            "isletter" => {
                let mut c = l.chars();
                match (c.next(), c.next()) {
                    (Some(ch), None) => {
                        if r.is_empty() {
                            ch.is_alphabetic()
                        } else {
                            r.chars().any(|x| fold(x) == fold(ch))
                        }
                    }
                    _ => false,
                }
            }
            "isalnum" => !l.is_empty() && l.chars().all(|c| c.is_alphanumeric()),
            "isalpha" => !l.is_empty() && l.chars().all(|c| c.is_alphabetic()),
            "islower" => !l.is_empty() && !l.chars().any(|c| c.is_uppercase()),
            "isupper" => !l.is_empty() && !l.chars().any(|c| c.is_lowercase()),
            "ison" => h.member_modes(ctx, r, l).is_some(),
            "isop" => h.member_modes(ctx, r, l).is_some_and(|m| m.contains('o')),
            "ishop" => h.member_modes(ctx, r, l).is_some_and(|m| m.contains('h')),
            "isvoice" => h.member_modes(ctx, r, l).is_some_and(|m| m.contains('v')),
            "isowner" => h.member_modes(ctx, r, l).is_some_and(|m| m.contains('q')),
            "isadmin" => h.member_modes(ctx, r, l).is_some_and(|m| m.contains('a')),
            "isreg" => h.member_modes(ctx, r, l).is_some_and(|m| m.is_empty()),
            "ischan" => h.is_channel(ctx, l),
            "isban" | "isinvite" | "isexcept" | "isquiet" | "isignore" | "isnotify" => {
                let args = if r.is_empty() { vec![l.to_string()] } else { vec![l.to_string(), r.to_string()] };
                h.identifier(ctx, op, &args, None).is_some_and(|v| truthy(&v))
            }
            "isprotect" | "isaop" | "isavoice" => {
                let level = &op[2..];
                let addr = self.full_address_of(h, ctx, l);
                self.users.has_named_level(&addr, level, r)
            }
            _ => false,
        }
    }

    /// Adresse complète d'un pseudo (`nick!user@host`) pour la liste des utilisateurs : telle quelle si elle
    /// contient déjà `!`, sinon demandée à l'hôte (`$address(nick,5)`), sinon `nick!*@*`.
    pub(crate) fn full_address_of(&mut self, h: &mut dyn Host, ctx: &Ctx, who: &str) -> String {
        if who.contains('!') {
            return who.to_string();
        }
        if let Some(a) = h.identifier(ctx, "address", &[who.to_string(), "5".to_string()], None)
            && a.contains('!')
        {
            return a;
        }
        format!("{who}!*@*")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logic_split() {
        let p = split_logic("%a == 1 && (%b == 2 || %c) || $iif(a && b,1,0)");
        assert_eq!(p.len(), 3);
        assert_eq!(p[1].0, Some(Logic::And));
        assert_eq!(p[2].1.trim(), "$iif(a && b,1,0)");
    }

    #[test]
    fn words() {
        let s = "$len(a b) == 3";
        let w: Vec<&str> = top_words(s).into_iter().map(|(a, b)| &s[a..b]).collect();
        assert_eq!(w, vec!["$len(a b)", "==", "3"]);
    }

    #[test]
    fn ranges() {
        assert!(in_range(3.0, "1-5"));
        assert!(!in_range(6.0, "1-5"));
        assert!(in_range(6.0, "5-"));
        assert!(in_range(-3.0, "-5--1"));
        assert!(in_range(5.0, "5"));
    }
}
