//! Fonctions « pures » sur le texte et les jetons (`$left`, `$mid`, `$replace`, `$gettok`, `$sorttok`…).
//!
//! Les positions sont comptées en caractères (pas en octets), comme `$len` dans mIRC en Unicode.

use crate::wild::{fold, wildmatch};

/// Caractères d'un texte.
fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

pub fn len(s: &str) -> usize {
    s.chars().count()
}

/// `$left(texte, N)` : N négatif → tout sauf les N derniers.
pub fn left(s: &str, n: i64) -> String {
    let c = chars(s);
    let l = c.len() as i64;
    let k = if n >= 0 { n.min(l) } else { (l + n).max(0) };
    c[..k as usize].iter().collect()
}

/// `$right(texte, N)` : N négatif → tout sauf les N premiers.
pub fn right(s: &str, n: i64) -> String {
    let c = chars(s);
    let l = c.len() as i64;
    let k = if n >= 0 { n.min(l) } else { (l + n).max(0) };
    c[(l - k) as usize..].iter().collect()
}

/// `$mid(texte, S, N)` : S à partir de 1 (négatif : depuis la fin) ; N négatif → tout sauf les N derniers.
pub fn mid(s: &str, start: i64, n: Option<i64>) -> String {
    let c = chars(s);
    let l = c.len() as i64;
    let st = if start < 0 { (l + start).max(0) } else if start == 0 { 0 } else { start - 1 };
    if st >= l {
        return String::new();
    }
    let end = match n {
        None => l,
        Some(n) if n >= 0 => (st + n).min(l),
        Some(n) => (l + n).max(st),
    };
    c[st as usize..end as usize].iter().collect()
}

fn conv(s: &str, cs: bool) -> Vec<char> {
    s.chars().map(|c| if cs { c } else { fold(c) }).collect()
}

/// Cherche `needle` dans `hay` à partir de `from` (indices de caractères).
pub fn find_chars(hay: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == *needle)
}

/// `$pos(texte, sous-texte, N)` : position (à partir de 1) de la N-ième occurrence ; N = 0 → nombre d'occurrences.
pub fn pos(s: &str, sub: &str, n: i64, cs: bool) -> Option<usize> {
    let h = conv(s, cs);
    let nd = conv(sub, cs);
    if n == 0 {
        return Some(count_in(&h, &nd));
    }
    let mut found = 0;
    let mut from = 0;
    while let Some(i) = find_chars(&h, &nd, from) {
        found += 1;
        if found == n {
            return Some(i + 1);
        }
        from = i + nd.len();
    }
    None
}

fn count_in(h: &[char], nd: &[char]) -> usize {
    let mut k = 0;
    let mut from = 0;
    while let Some(i) = find_chars(h, nd, from) {
        k += 1;
        from = i + nd.len();
    }
    k
}

/// `$count(texte, a, b…)` : somme des occurrences.
pub fn count(s: &str, subs: &[String], cs: bool) -> usize {
    let h = conv(s, cs);
    subs.iter().map(|x| count_in(&h, &conv(x, cs))).sum()
}

/// Remplace toutes les occurrences de `a` par `b`.
pub fn replace_one(s: &str, a: &str, b: &str, cs: bool) -> String {
    if a.is_empty() {
        return s.to_string();
    }
    let orig = chars(s);
    let h = conv(s, cs);
    let nd = conv(a, cs);
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < orig.len() {
        if i + nd.len() <= h.len() && h[i..i + nd.len()] == *nd {
            out.push_str(b);
            i += nd.len();
        } else {
            out.push(orig[i]);
            i += 1;
        }
    }
    out
}

/// `$replace` : les paires s'appliquent l'une après l'autre.
pub fn replace_seq(s: &str, pairs: &[String], cs: bool) -> String {
    let mut out = s.to_string();
    for p in pairs.chunks(2) {
        let b = p.get(1).map(|x| x.as_str()).unwrap_or("");
        out = replace_one(&out, &p[0], b, cs);
    }
    out
}

/// `$replacex` : toutes les paires en une passe (un remplacement n'est jamais remplacé à nouveau).
pub fn replace_simul(s: &str, pairs: &[String], cs: bool) -> String {
    let orig = chars(s);
    let h = conv(s, cs);
    let pats: Vec<(Vec<char>, &str)> = pairs
        .chunks(2)
        .filter(|p| !p[0].is_empty())
        .map(|p| (conv(&p[0], cs), p.get(1).map(|x| x.as_str()).unwrap_or("")))
        .collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    'outer: while i < orig.len() {
        for (nd, rep) in &pats {
            if i + nd.len() <= h.len() && h[i..i + nd.len()] == **nd {
                out.push_str(rep);
                i += nd.len();
                continue 'outer;
            }
        }
        out.push(orig[i]);
        i += 1;
    }
    out
}

/// `$remove(texte, a, b…)`.
pub fn remove(s: &str, subs: &[String], cs: bool) -> String {
    let mut out = s.to_string();
    for x in subs {
        out = replace_one(&out, x, "", cs);
    }
    out
}

/// `$str(texte, N)`.
pub fn repeat(s: &str, n: i64) -> String {
    if n <= 0 { String::new() } else { s.repeat(n.min(100_000) as usize) }
}

// ---------------------------------------------------------------------------------------------------------------
// Jetons
// ---------------------------------------------------------------------------------------------------------------

/// Caractère séparateur d'un argument `C` (code ASCII/Unicode, comme `46` pour `.`).
pub fn sep_char(c: &str) -> Option<char> {
    let n: u32 = c.trim().parse().ok()?;
    char::from_u32(n)
}

/// Jetons non vides (les séparateurs répétés comptent pour un, comme dans mIRC).
pub fn toks(s: &str, c: char) -> Vec<&str> {
    s.split(c).filter(|t| !t.is_empty()).collect()
}

fn join(t: &[&str], c: char) -> String {
    let mut out = String::new();
    for (i, x) in t.iter().enumerate() {
        if i > 0 {
            out.push(c);
        }
        out.push_str(x);
    }
    out
}

/// Plage `N`, `N-`, `N-M`, avec des négatifs (`-1` : le dernier), résolue en indices 0..n (fin exclue).
pub fn tok_range(spec: &str, n: usize) -> Option<(usize, usize)> {
    let spec = spec.trim();
    let (a, rest) = parse_signed_prefix(spec)?;
    let resolve = |v: i64| -> i64 { if v < 0 { n as i64 + 1 + v } else { v } };
    let a = resolve(a);
    let b = if rest.is_empty() {
        a
    } else {
        let r = rest.strip_prefix('-')?;
        if r.is_empty() { n as i64 } else { resolve(r.parse::<i64>().ok()?) }
    };
    let a = a.max(1);
    let b = b.min(n as i64);
    if a > b || a > n as i64 {
        return Some((0, 0));
    }
    Some(((a - 1) as usize, b as usize))
}

fn parse_signed_prefix(s: &str) -> Option<(i64, &str)> {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && b[i] == b'-' {
        i += 1;
    }
    let st = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == st {
        return None;
    }
    Some((s[..i].parse().ok()?, &s[i..]))
}

/// `$gettok(texte, N, C)` ; N = 0 → nombre de jetons.
pub fn gettok(s: &str, spec: &str, c: char) -> String {
    let t = toks(s, c);
    if spec.trim() == "0" {
        return t.len().to_string();
    }
    match tok_range(spec, t.len()) {
        Some((a, b)) if a < b => join(&t[a..b], c),
        _ => String::new(),
    }
}

pub fn numtok(s: &str, c: char) -> usize {
    toks(s, c).len()
}

fn tok_eq(a: &str, b: &str, cs: bool) -> bool {
    if cs { a == b } else { a.chars().map(fold).eq(b.chars().map(fold)) }
}

/// `$addtok` : ajoute le jeton s'il n'y est pas déjà.
pub fn addtok(s: &str, tok: &str, c: char, cs: bool) -> String {
    let mut t = toks(s, c);
    if !t.iter().any(|x| tok_eq(x, tok, cs)) && !tok.is_empty() {
        t.push(tok);
    }
    join(&t, c)
}

/// `$deltok(texte, N[-M], C)`.
pub fn deltok(s: &str, spec: &str, c: char) -> String {
    let mut t = toks(s, c);
    if let Some((a, b)) = tok_range(spec, t.len())
        && a < b
    {
        t.drain(a..b);
    }
    join(&t, c)
}

/// Position (0..=n) depuis un N de mIRC (négatif : depuis la fin).
fn resolve_pos(n: i64, len: usize) -> usize {
    if n < 0 { (len as i64 + 1 + n).max(0) as usize } else { (n.max(1) - 1) as usize }
}

/// `$instok(texte, jeton, N, C)` : insère en position N.
pub fn instok(s: &str, tok: &str, n: i64, c: char) -> String {
    let mut t = toks(s, c);
    let p = resolve_pos(n, t.len()).min(t.len());
    t.insert(p, tok);
    join(&t, c)
}

/// `$puttok(texte, jeton, N, C)` : remplace le N-ième.
pub fn puttok(s: &str, tok: &str, n: i64, c: char) -> String {
    let mut t = toks(s, c);
    let p = resolve_pos(n, t.len());
    if p < t.len() {
        t[p] = tok;
    }
    join(&t, c)
}

/// Indices des jetons égaux à `tok`.
fn matching(t: &[&str], tok: &str, cs: bool) -> Vec<usize> {
    t.iter().enumerate().filter(|(_, x)| tok_eq(x, tok, cs)).map(|(i, _)| i).collect()
}

/// `$reptok(texte, jeton, nouveau, N, C)` : N = 0 → tous.
pub fn reptok(s: &str, tok: &str, new: &str, n: i64, c: char, cs: bool) -> String {
    let mut t = toks(s, c);
    let m = matching(&t, tok, cs);
    if n == 0 {
        for i in m {
            t[i] = new;
        }
    } else if let Some(&i) = m.get(resolve_pos(n, m.len())) {
        t[i] = new;
    }
    join(&t, c)
}

/// `$remtok(texte, jeton, N, C)` : N = 0 → tous.
pub fn remtok(s: &str, tok: &str, n: i64, c: char, cs: bool) -> String {
    let t = toks(s, c);
    let m = matching(&t, tok, cs);
    let drop: Vec<usize> = if n == 0 { m } else { m.get(resolve_pos(n, m.len())).copied().into_iter().collect() };
    let kept: Vec<&str> = t.iter().enumerate().filter(|(i, _)| !drop.contains(i)).map(|(_, x)| *x).collect();
    join(&kept, c)
}

/// `$findtok(texte, jeton, N, C)` : position du N-ième jeton égal ; N = 0 → nombre.
pub fn findtok(s: &str, tok: &str, n: i64, c: char, cs: bool) -> String {
    let t = toks(s, c);
    let m = matching(&t, tok, cs);
    if n == 0 {
        return m.len().to_string();
    }
    m.get(resolve_pos(n, m.len())).map(|i| (i + 1).to_string()).unwrap_or_default()
}

pub fn istok(s: &str, tok: &str, c: char, cs: bool) -> bool {
    toks(s, c).iter().any(|x| tok_eq(x, tok, cs))
}

/// `$matchtok` (jetons qui contiennent le texte) et `$wildtok` (jetons qui correspondent au joker).
pub fn filtertok(s: &str, pat: &str, n: i64, c: char, cs: bool, wild: bool) -> String {
    let t = toks(s, c);
    let p = conv(pat, cs);
    let m: Vec<&str> = t
        .into_iter()
        .filter(|x| if wild { wildmatch(pat, x, cs) } else { find_chars(&conv(x, cs), &p, 0).is_some() })
        .collect();
    if n == 0 {
        return m.len().to_string();
    }
    m.get(resolve_pos(n, m.len())).map(|x| x.to_string()).unwrap_or_default()
}

/// Comparaison « naturelle » : les suites de chiffres comparées comme des nombres.
fn natural_cmp(a: &str, b: &str, cs: bool) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let a: Vec<char> = conv(a, cs);
    let b: Vec<char> = conv(b, cs);
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let si = i;
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            let sj = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let na: String = a[si..i].iter().collect::<String>().trim_start_matches('0').to_string();
            let nb: String = b[sj..j].iter().collect::<String>().trim_start_matches('0').to_string();
            let o = na.len().cmp(&nb.len()).then(na.cmp(&nb));
            if o != Ordering::Equal {
                return o;
            }
        } else {
            let o = a[i].cmp(&b[j]);
            if o != Ordering::Equal {
                return o;
            }
            i += 1;
            j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j))
}

/// Rang d'un préfixe de canal/pseudo pour l'option `c` de `$sorttok` (`~&@%+` puis le reste).
fn prefix_rank(s: &str) -> usize {
    match s.chars().next() {
        Some('~') => 0,
        Some('&') => 1,
        Some('@') => 2,
        Some('%') => 3,
        Some('+') => 4,
        _ => 5,
    }
}

/// `$sorttok(texte, C, [ncra])`.
pub fn sorttok(s: &str, c: char, flags: &str, cs: bool) -> String {
    let mut t = toks(s, c);
    if flags.contains('n') {
        t.sort_by(|a, b| {
            let x = crate::calc::parse_num_prefix(a);
            let y = crate::calc::parse_num_prefix(b);
            x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
        });
    } else if flags.contains('c') {
        t.sort_by(|a, b| {
            prefix_rank(a).cmp(&prefix_rank(b)).then_with(|| {
                let ta = a.trim_start_matches(['~', '&', '@', '%', '+']);
                let tb = b.trim_start_matches(['~', '&', '@', '%', '+']);
                conv(ta, cs).cmp(&conv(tb, cs))
            })
        });
    } else if flags.contains('a') {
        t.sort_by(|a, b| natural_cmp(a, b, cs));
    } else {
        t.sort_by_key(|a| conv(a, cs));
    }
    if flags.contains('r') {
        t.reverse();
    }
    join(&t, c)
}

/// `$base(N, de, vers, [remplissage])` : entiers et fractions simples.
pub fn base(n: &str, from: u32, to: u32, pad: usize) -> String {
    if !(2..=36).contains(&from) || !(2..=36).contains(&to) {
        return String::new();
    }
    let s = n.trim();
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s),
    };
    let (ip, fp) = match s.split_once('.') {
        Some((a, b)) => (a, b),
        None => (s, ""),
    };
    let mut v: u128 = 0;
    for ch in ip.chars() {
        match ch.to_digit(from) {
            Some(d) => v = v.saturating_mul(from as u128).saturating_add(d as u128),
            None => break,
        }
    }
    let mut frac = 0f64;
    let mut scale = 1f64 / from as f64;
    for ch in fp.chars() {
        match ch.to_digit(from) {
            Some(d) => {
                frac += d as f64 * scale;
                scale /= from as f64;
            }
            None => break,
        }
    }
    let digits = |mut x: u128| -> String {
        if x == 0 {
            return "0".into();
        }
        let mut d = Vec::new();
        while x > 0 {
            d.push(std::char::from_digit((x % to as u128) as u32, to).unwrap().to_ascii_uppercase());
            x /= to as u128;
        }
        d.iter().rev().collect()
    };
    let mut out = digits(v);
    while out.len() < pad {
        out.insert(0, '0');
    }
    if frac > 0.0 {
        out.push('.');
        for _ in 0..6 {
            frac *= to as f64;
            let d = frac.floor() as u32;
            out.push(std::char::from_digit(d.min(to - 1), to).unwrap().to_ascii_uppercase());
            frac -= d as f64;
            if frac <= 1e-12 {
                break;
            }
        }
    }
    if neg { format!("-{out}") } else { out }
}

/// Nombre avec séparateurs de milliers (`1,234,567`).
pub fn commas(int_part: &str) -> String {
    let (neg, digits) = match int_part.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, int_part),
    };
    let mut out = String::new();
    let n = digits.len();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (n - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if neg { format!("-{out}") } else { out }
}

/// `$bytes(N, [bkmgt3d])` avec la propriété `.suf`.
pub fn bytes(n: f64, opts: &str, suf: bool) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut unit = None;
    for (i, ch) in ['b', 'k', 'm', 'g', 't'].iter().enumerate() {
        if opts.contains(*ch) {
            unit = Some(i);
        }
    }
    let three = opts.contains('3');
    let drop = opts.contains('d');
    let unit = match unit {
        Some(u) => u,
        None if three || suf => {
            let mut u = 0;
            let mut v = n.abs();
            while v >= 1024.0 && u < 4 {
                v /= 1024.0;
                u += 1;
            }
            u
        }
        None => 0,
    };
    let v = n / 1024f64.powi(unit as i32);
    let mut s = if unit == 0 && !three {
        commas(&format!("{:.0}", v.trunc()))
    } else if three {
        let decimals = if v.abs() >= 100.0 { 0 } else if v.abs() >= 10.0 { 1 } else { 2 };
        let f = format!("{v:.decimals$}");
        let (i, d) = f.split_once('.').map(|(a, b)| (a.to_string(), Some(b.to_string()))).unwrap_or((f.clone(), None));
        match d {
            Some(d) => format!("{}.{}", commas(&i), d),
            None => commas(&i),
        }
    } else {
        let f = format!("{v:.2}");
        let (i, d) = f.split_once('.').unwrap();
        format!("{}.{}", commas(i), d)
    };
    if drop && s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if suf {
        s.push_str(UNITS[unit]);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens() {
        assert_eq!(gettok("a.b.c", "2", '.'), "b");
        assert_eq!(gettok("a.b.c", "2-", '.'), "b.c");
        assert_eq!(gettok("a.b.c", "-1", '.'), "c");
        assert_eq!(gettok("a.b.c", "0", '.'), "3");
        assert_eq!(gettok("a..b", "2", '.'), "b");
        assert_eq!(gettok("a.b.c.d", "-2-", '.'), "c.d");
        assert_eq!(gettok("a.b.c.d", "2--1", '.'), "b.c.d");
        assert_eq!(gettok("a.b.c", "5", '.'), "");
        assert_eq!(deltok("a.b.c", "2", '.'), "a.c");
        assert_eq!(instok("a.c", "b", 2, '.'), "a.b.c");
        assert_eq!(puttok("a.x.c", "b", 2, '.'), "a.b.c");
        assert_eq!(remtok("a.b.a.c", "a", 0, '.', false), "b.c");
        assert_eq!(remtok("a.b.a.c", "a", 1, '.', false), "b.a.c");
        assert_eq!(reptok("a.b.a", "a", "z", 2, '.', false), "a.b.z");
        assert_eq!(findtok("a.b.a", "A", 2, '.', false), "3");
        assert_eq!(findtok("a.b.a", "A", 0, '.', false), "2");
        assert_eq!(sorttok("c b a", ' ', "", false), "a b c");
        assert_eq!(sorttok("10 9 100", ' ', "n", false), "9 10 100");
        assert_eq!(sorttok("10 9 100", ' ', "nr", false), "100 10 9");
        assert_eq!(sorttok("a10 a9 a100", ' ', "a", false), "a9 a10 a100");
        assert_eq!(sorttok("bob @al +cy", ' ', "c", false), "@al +cy bob");
    }

    #[test]
    fn texts() {
        assert_eq!(left("abcdef", 2), "ab");
        assert_eq!(left("abcdef", -2), "abcd");
        assert_eq!(right("abcdef", 2), "ef");
        assert_eq!(right("abcdef", -2), "cdef");
        assert_eq!(mid("abcdef", 2, Some(3)), "bcd");
        assert_eq!(mid("abcdef", -2, None), "ef");
        assert_eq!(mid("abcdef", 2, Some(-1)), "bcde");
        assert_eq!(pos("abcabc", "C", 2, false), Some(6));
        assert_eq!(pos("abcabc", "c", 0, false), Some(2));
        assert_eq!(replace_simul("ab", &["a".into(), "b".into(), "b".into(), "c".into()], false), "bc");
        assert_eq!(replace_seq("ab", &["a".into(), "b".into(), "b".into(), "c".into()], false), "cc");
        assert_eq!(base("255", 10, 16, 0), "FF");
        assert_eq!(base("ff", 16, 2, 0), "11111111");
        assert_eq!(base("5", 10, 2, 8), "00000101");
        assert_eq!(commas("1234567"), "1,234,567");
        assert_eq!(bytes(1234567.0, "", false), "1,234,567");
        assert_eq!(bytes(1536.0, "k", false), "1.50");
        assert_eq!(bytes(1536.0, "kd", true), "1.5KB");
        assert_eq!(bytes(1234567.0, "3", true), "1.18MB");
    }
}
