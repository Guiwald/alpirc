//! Variables `%nom` : locales (`/var`, propres à une routine) et globales (`/set`, enregistrées), avec effacement
//! différé (`-uN`), incrémentation continue (`inc -c`, `set -z`), et l'écriture `vars.ini` simplifiée.

use std::collections::HashMap;

use crate::calc::{fmt_num, parse_num, parse_num_prefix};
use crate::exec::{R, split_word, switch_num, switches};
use crate::source::strip_n;
use crate::wild::wildmatch;
use crate::{Ctx, Engine, Host, Msg};

/// Incrémentation automatique chaque seconde (`inc -c`, `set -z`).
#[derive(Debug, Clone)]
pub struct Cont {
    pub step: f64,
    pub last: i64,
    /// Décroît jusqu'à zéro puis s'efface (`-z`).
    pub to_zero: bool,
}

#[derive(Debug, Clone)]
pub struct Var {
    /// Nom d'origine, avec `%`.
    pub name: String,
    pub value: String,
    seq: u64,
    pub expires: Option<i64>,
    pub cont: Option<Cont>,
}

/// Variables globales.
#[derive(Debug, Default)]
pub struct Vars {
    map: HashMap<String, Var>,
    seq: u64,
}

impl Vars {
    pub fn get(&self, lname: &str) -> Option<&Var> {
        self.map.get(lname)
    }

    pub fn get_mut(&mut self, lname: &str) -> Option<&mut Var> {
        self.map.get_mut(lname)
    }

    /// Donne une valeur (crée la variable au besoin, garde son délai d'effacement).
    pub fn set(&mut self, name: &str, value: String) {
        let l = name.to_lowercase();
        match self.map.get_mut(&l) {
            Some(v) => v.value = value,
            None => {
                self.seq += 1;
                self.map.insert(l, Var { name: name.to_string(), value, seq: self.seq, expires: None, cont: None });
            }
        }
    }

    pub fn remove(&mut self, lname: &str) -> bool {
        self.map.remove(&lname.to_lowercase()).is_some()
    }

    /// Efface les variables qui correspondent au joker ; rend leurs noms.
    pub fn remove_matching(&mut self, pat: &str) -> Vec<String> {
        let names: Vec<String> =
            self.map.values().filter(|v| wildmatch(pat, &v.name, false)).map(|v| v.name.clone()).collect();
        for n in &names {
            self.map.remove(&n.to_lowercase());
        }
        names
    }

    /// (nom, valeur), dans l'ordre de création.
    pub fn list(&self) -> Vec<(String, String)> {
        let mut v: Vec<&Var> = self.map.values().collect();
        v.sort_by_key(|x| x.seq);
        v.into_iter().map(|x| (x.name.clone(), x.value.clone())).collect()
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Prochaine échéance (effacement ou incrémentation).
    pub fn next_due(&self) -> Option<i64> {
        self.map
            .values()
            .filter_map(|v| {
                let c = v.cont.as_ref().map(|c| c.last + 1000);
                match (v.expires, c) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                }
            })
            .min()
    }

    /// Effacements et incrémentations échus.
    pub fn tick(&mut self, now: i64) {
        let mut gone = Vec::new();
        for (k, v) in self.map.iter_mut() {
            if v.expires.is_some_and(|e| e <= now) {
                gone.push(k.clone());
                continue;
            }
            if let Some(c) = &mut v.cont {
                while now - c.last >= 1000 {
                    c.last += 1000;
                    let cur = parse_num_prefix(&v.value);
                    let next = cur + c.step;
                    if c.to_zero && next <= 0.0 {
                        gone.push(k.clone());
                        break;
                    }
                    v.value = fmt_num(next);
                }
            }
        }
        for k in gone {
            self.map.remove(&k);
        }
    }
}

/// `5 + 3` → `8` : `/set` et `/var` calculent une opération simple (sauf `-n`).
fn simple_math(v: &str) -> String {
    let w: Vec<&str> = v.split(' ').filter(|x| !x.is_empty()).collect();
    if w.len() == 3
        && w[1].len() == 1
        && "+-*/%^".contains(w[1])
        && let (Some(a), Some(b)) = (parse_num(w[0]), parse_num(w[2]))
    {
        let r = match w[1] {
            "+" => a + b,
            "-" => a - b,
            "*" => a * b,
            "/" => {
                if b == 0.0 {
                    0.0
                } else {
                    a / b
                }
            }
            "%" => {
                if b == 0.0 {
                    0.0
                } else {
                    a % b
                }
            }
            _ => a.powf(b),
        };
        return fmt_num(r);
    }
    v.to_string()
}

/// Découpe `%a = 1, %b = $f(x,y), %c` aux virgules suivies d'un `%` (hors parenthèses).
fn split_decls(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for i in 0..b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth <= 0 => {
                let mut k = i + 1;
                while k < b.len() && b[k] == b' ' {
                    k += 1;
                }
                if k < b.len() && b[k] == b'%' {
                    out.push(&s[start..i]);
                    start = i + 1;
                }
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

impl Engine {
    /// Valeur d'une variable (`%nom`) : locale d'abord, puis globale ; vide si elle n'existe pas.
    pub(crate) fn var_get(&self, name: &str) -> String {
        let l = name.to_lowercase();
        if let Some(f) = self.rt.frames.last()
            && let Some((_, v)) = f.locals.get(&l)
        {
            return v.clone();
        }
        self.vars.get(&l).map(|v| v.value.clone()).unwrap_or_default()
    }

    /// Donne une valeur : à la locale si elle existe, sinon à la globale.
    pub(crate) fn var_assign(&mut self, name: &str, value: String) {
        let l = name.to_lowercase();
        if let Some(f) = self.rt.frames.last_mut()
            && let Some(slot) = f.locals.get_mut(&l)
        {
            slot.1 = value;
            return;
        }
        self.vars.set(name, value);
    }

    fn is_local(&self, name: &str) -> bool {
        self.rt.frames.last().is_some_and(|f| f.locals.contains_key(&name.to_lowercase()))
    }

    /// Écho de `/set`… : seulement pour une commande tapée (pas dans un alias), ou avec `-s`.
    fn show_set(&self, sw: &str, quiet: bool) -> bool {
        sw.contains('s') || (self.rt.typed && self.rt.frames.len() == 1 && !quiet)
    }

    /// Arguments bruts d'une commande à nom de variable : crochets résolus, rien d'autre.
    fn prep_raw(&mut self, h: &mut dyn Host, ctx: &Ctx, raw: &str) -> R<String> {
        if !self.frame().no_eval && crate::eval::has_brackets(raw) {
            self.resolve_brackets(h, ctx, raw)
        } else {
            Ok(raw.to_string())
        }
    }

    fn eval_value(&mut self, h: &mut dyn Host, ctx: &Ctx, raw: &str) -> R<String> {
        if self.frame().no_eval { Ok(raw.to_string()) } else { self.eval_text(h, ctx, raw) }
    }

    /// `/var [-gnsp] %a = valeur, %b = valeur, %c`.
    pub(crate) fn cmd_var(&mut self, h: &mut dyn Host, ctx: &Ctx, raw: &str, quiet: bool) -> R<()> {
        let raw = self.prep_raw(h, ctx, raw)?;
        let (sw, rest) = switches(&raw);
        let rest = rest.to_string();
        for decl in split_decls(&rest) {
            let d = decl.trim();
            if d.is_empty() {
                continue;
            }
            let (mut name, mut vraw) = split_word(d);
            if let Some(eq) = name.find('=')
                && eq > 0
            {
                // `%a=1`
                vraw = &d[eq + 1..];
                name = &d[..eq];
            } else if let Some(r) = vraw.strip_prefix('=') {
                vraw = r.trim_start();
            }
            if !name.starts_with('%') || name.len() < 2 {
                return Err(self.err(Msg::InvalidParams { cmd: "var".into() }));
            }
            let mut value = self.eval_value(h, ctx, vraw)?;
            if !sw.contains('p') {
                value = value.trim_matches(' ').to_string();
            }
            if !sw.contains('n') {
                value = simple_math(&value);
            }
            if sw.contains('g') {
                self.vars.set(name, value.clone());
            } else {
                let f = self.frame_mut();
                f.locals.insert(name.to_lowercase(), (name.to_string(), value.clone()));
            }
            if sw.contains('s') && !quiet {
                self.info(h, ctx, Msg::VarSet { name: name.to_string(), value: value.to_string() });
            }
        }
        Ok(())
    }

    /// `/set`, `/unset`, `/unsetall`, `/inc`, `/dec`.
    pub(crate) fn cmd_setlike(&mut self, h: &mut dyn Host, ctx: &Ctx, cmd: &str, raw: &str, quiet: bool) -> R<()> {
        let raw = self.prep_raw(h, ctx, raw)?;
        let (sw, rest) = switches(&raw);
        let show = self.show_set(&sw, quiet);
        let now = h.now_ms();
        match cmd {
            "unsetall" => {
                self.vars.clear();
                if show {
                    self.info(h, ctx, Msg::VarUnsetAll);
                }
            }
            "unset" => {
                for w in rest.split(' ').filter(|w| !w.is_empty()) {
                    if !w.starts_with('%') {
                        continue;
                    }
                    let mut names = Vec::new();
                    if w.contains(['*', '?']) {
                        names = self.vars.remove_matching(w);
                        if let Some(f) = self.rt.frames.last_mut() {
                            let loc: Vec<String> =
                                f.locals.values().filter(|(n, _)| wildmatch(w, n, false)).map(|(n, _)| n.clone()).collect();
                            for n in loc {
                                f.locals.remove(&n.to_lowercase());
                                names.push(n);
                            }
                        }
                    } else if let Some(f) = self.rt.frames.last_mut()
                        && f.locals.remove(&w.to_lowercase()).is_some()
                    {
                        names.push(w.to_string());
                    } else if self.vars.remove(w) {
                        names.push(w.to_string());
                    }
                    if show {
                        for n in names {
                            self.info(h, ctx, Msg::VarUnset { name: n.to_string() });
                        }
                    }
                }
            }
            "set" => {
                let (name, vraw) = split_word(rest);
                if !name.starts_with('%') || name.len() < 2 {
                    return Err(self.err(Msg::InvalidParams { cmd: "set".into() }));
                }
                let mut value = self.eval_value(h, ctx, vraw)?;
                value = value.trim_matches(' ').to_string();
                if !sw.contains('n') {
                    value = simple_math(&value);
                }
                if sw.contains('l') && self.is_local(name) {
                    self.var_assign(name, value.clone());
                } else {
                    self.vars.set(name, value.clone());
                }
                self.apply_timeouts(&sw, name, now);
                if sw.contains('z')
                    && let Some(v) = self.vars.get_mut(&name.to_lowercase())
                {
                    v.cont = Some(Cont { step: -1.0, last: now, to_zero: true });
                }
                if show {
                    self.info(h, ctx, Msg::VarSet { name: name.to_string(), value: value.to_string() });
                }
            }
            _ => {
                // inc / dec
                let (name, vraw) = split_word(rest);
                if !name.starts_with('%') || name.len() < 2 {
                    return Err(self.err(Msg::InvalidParams { cmd: cmd.to_string() }));
                }
                let by = if vraw.trim().is_empty() {
                    1.0
                } else {
                    let v = self.eval_value(h, ctx, vraw)?;
                    parse_num_prefix(&v)
                };
                let by = if cmd == "dec" { -by } else { by };
                let cur = parse_num_prefix(&self.var_get(name));
                let value = fmt_num(cur + by);
                if self.is_local(name) {
                    self.var_assign(name, value.clone());
                } else {
                    self.vars.set(name, value.clone());
                    self.apply_timeouts(&sw, name, now);
                    if (sw.contains('c') || sw.contains('z'))
                        && let Some(v) = self.vars.get_mut(&name.to_lowercase())
                    {
                        let step = if sw.contains('z') { -1.0 } else { by };
                        v.cont = Some(Cont { step, last: now, to_zero: sw.contains('z') });
                    }
                }
                if show {
                    let (name, value) = (name.to_string(), value.to_string());
                    let m = if cmd == "dec" { Msg::VarDec { name, value } } else { Msg::VarInc { name, value } };
                    self.info(h, ctx, m);
                }
            }
        }
        Ok(())
    }

    /// `-uN` : effacement dans N secondes (`-u0` : à la fin du script).
    fn apply_timeouts(&mut self, sw: &str, name: &str, now: i64) {
        if sw.contains('u') {
            let secs = switch_num(sw, 'u').unwrap_or(0.0);
            if secs <= 0.0 {
                self.rt.u0.push(name.to_lowercase());
            } else if let Some(v) = self.vars.get_mut(&name.to_lowercase()) {
                v.expires = Some(now + (secs * 1000.0) as i64);
            }
        }
    }

    /// Variables globales au format `%nom valeur`, une par ligne (onglet Variables, `vars.ini`).
    pub fn vars_text(&self) -> String {
        let mut out = String::new();
        for (n, v) in self.vars.list() {
            out.push_str(&n);
            if !v.is_empty() {
                out.push(' ');
                out.push_str(&v);
            }
            out.push('\n');
        }
        out
    }

    /// Remplace les variables globales (lignes `%nom valeur` ; le format `.ini` de mIRC, `[variables]` et `n0=`,
    /// est accepté aussi).
    pub fn set_vars_text(&mut self, text: &str) {
        self.vars.clear();
        self.merge_vars_text(text);
    }

    /// Ajoute des variables sans effacer les autres (`/load -rv`).
    pub(crate) fn merge_vars_text(&mut self, text: &str) {
        for l in text.lines() {
            let t = strip_n(l.trim_end_matches('\r').trim_start());
            if t.starts_with('[') || !t.starts_with('%') {
                continue;
            }
            let (n, v) = match t.find(' ') {
                Some(i) => (&t[..i], &t[i + 1..]),
                None => (t, ""),
            };
            if n.len() > 1 {
                self.vars.set(n, v.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decls() {
        assert_eq!(split_decls("%a = 1, %b = $f(x, %y), %c"), vec!["%a = 1", " %b = $f(x, %y)", " %c"]);
        assert_eq!(split_decls("%a = a,b"), vec!["%a = a,b"]);
    }

    #[test]
    fn math() {
        assert_eq!(simple_math("5 + 3"), "8");
        assert_eq!(simple_math("5+3"), "5+3");
        assert_eq!(simple_math("hello + world"), "hello + world");
    }
}
