//! Liste des utilisateurs (onglet Users de mIRC) : `niveaux:masque [info]`, une entrée par ligne.
//! Commandes `/auser`, `/guser`, `/ruser`, `/rlevel`, `/flush` ; identifiants `$level`, `$ulevel`, `$ulist` ;
//! niveaux des événements (`on 5:TEXT`, `on +friend:JOIN`…). Un utilisateur absent de la liste a le niveau 1.

use crate::exec::{R, split_word, switches};
use crate::source::strip_n;
use crate::wild::wildmatch;
use crate::{Ctx, Engine, Host};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub levels: Vec<String>,
    pub mask: String,
    pub info: String,
}

#[derive(Debug, Default)]
pub struct Users {
    pub list: Vec<User>,
}

/// Niveau par défaut d'un utilisateur inconnu.
pub const DEFAULT_LEVEL: i64 = 1;

fn mask_matches(mask: &str, addr: &str) -> bool {
    if mask.contains('!') {
        wildmatch(mask, addr, false)
    } else if mask.contains('@') {
        wildmatch(&format!("*!{mask}"), addr, false)
    } else {
        let nick = addr.split('!').next().unwrap_or(addr);
        wildmatch(mask, nick, false)
    }
}

fn parse_levels(s: &str) -> Vec<String> {
    s.split(',').map(|l| l.trim().trim_start_matches('=').to_string()).filter(|l| !l.is_empty()).collect()
}

impl Users {
    /// Premier utilisateur qui correspond à l'adresse `nick!user@host`.
    pub fn find(&self, addr: &str) -> Option<&User> {
        self.list.iter().find(|u| mask_matches(&u.mask, addr))
    }

    pub fn levels_of(&self, addr: &str) -> Vec<String> {
        match self.find(addr) {
            Some(u) => u.levels.clone(),
            None => vec![DEFAULT_LEVEL.to_string()],
        }
    }

    /// Plus haut niveau numérique.
    pub fn numeric_level(&self, addr: &str) -> i64 {
        self.find(addr)
            .and_then(|u| u.levels.iter().filter_map(|l| l.parse::<i64>().ok()).max())
            .unwrap_or(DEFAULT_LEVEL)
    }

    /// Le niveau d'un événement correspond-il ? Rend `$ulevel` si oui.
    pub fn event_level(&self, level: &str, exact: bool, addr: &str) -> Option<String> {
        let level = level.trim();
        if level.is_empty() || level == "*" {
            return Some(self.numeric_level(addr).to_string());
        }
        if let Ok(n) = level.parse::<i64>() {
            let ok = if exact {
                self.levels_of(addr).iter().any(|l| l.parse::<i64>().ok() == Some(n))
            } else {
                self.numeric_level(addr) >= n
            };
            return ok.then(|| self.numeric_level(addr).to_string());
        }
        let has = self.levels_of(addr).iter().any(|l| l.eq_ignore_ascii_case(level));
        has.then(|| level.to_string())
    }

    /// Niveau nommé (`protect`, `aop`, `avoice`…), éventuellement limité à un canal (`aop:#canal` dans le niveau).
    pub fn has_named_level(&self, addr: &str, level: &str, chan: &str) -> bool {
        self.list.iter().filter(|u| mask_matches(&u.mask, addr)).any(|u| {
            u.levels.iter().any(|l| {
                let (base, ch) = l.split_once('#').map(|(b, c)| (b.trim_end_matches(':'), Some(c))).unwrap_or((l, None));
                base.eq_ignore_ascii_case(level)
                    && match (ch, chan.is_empty()) {
                        (Some(c), false) => format!("#{c}").eq_ignore_ascii_case(chan),
                        _ => true,
                    }
            })
        })
    }

    pub fn text(&self) -> String {
        let mut out = String::new();
        for u in &self.list {
            out.push_str(&u.levels.join(","));
            out.push(':');
            out.push_str(&u.mask);
            if !u.info.is_empty() {
                out.push(' ');
                out.push_str(&u.info);
            }
            out.push('\n');
        }
        out
    }

    pub fn parse_line(l: &str) -> Option<User> {
        let t = strip_n(l.trim_end_matches('\r').trim());
        if t.is_empty() || t.starts_with('[') || t.starts_with(';') {
            return None;
        }
        // Les niveaux s'arrêtent au dernier `:` avant le masque (`5,friend:nick!u@h info`).
        let (lv, rest) = t.split_once(':')?;
        let (mask, info) = split_word(rest);
        if mask.is_empty() {
            return None;
        }
        Some(User { levels: parse_levels(lv), mask: mask.to_string(), info: info.to_string() })
    }

    /// Ajoute ou remplace (`add` : ajoute les niveaux à ceux qui existent).
    pub fn add(&mut self, levels: &str, mask: &str, info: &str, add: bool) {
        let lv = parse_levels(levels);
        if let Some(u) = self.list.iter_mut().find(|u| u.mask.eq_ignore_ascii_case(mask)) {
            if add {
                for l in lv {
                    if !u.levels.iter().any(|x| x.eq_ignore_ascii_case(&l)) {
                        u.levels.push(l);
                    }
                }
            } else {
                u.levels = lv;
            }
            if !info.is_empty() {
                u.info = info.to_string();
            }
            return;
        }
        self.list.push(User { levels: lv, mask: mask.to_string(), info: info.to_string() });
    }
}

impl Engine {
    /// La liste des utilisateurs, une entrée `niveaux:masque [info]` par ligne.
    pub fn users_text(&self) -> String {
        self.users.text()
    }

    /// Remplace la liste des utilisateurs (format de [`Engine::users_text`] ; `[users]`/`n0=` acceptés).
    pub fn set_users_text(&mut self, text: &str) {
        self.users.list = text.lines().filter_map(Users::parse_line).collect();
    }

    pub(crate) fn merge_users_text(&mut self, text: &str) {
        for u in text.lines().filter_map(Users::parse_line) {
            let lv = u.levels.join(",");
            self.users.add(&lv, &u.mask, &u.info, true);
        }
    }

    /// `/auser`, `/guser`, `/ruser`, `/rlevel`, `/flush`.
    pub(crate) fn cmd_users(&mut self, h: &mut dyn Host, ctx: &Ctx, name: &str, args: &str) -> R<()> {
        let (sw, rest) = switches(args);
        match name {
            "auser" | "guser" => {
                let (levels, rest) = split_word(rest);
                let (who, rest) = split_word(rest);
                if who.is_empty() {
                    return Err(self.err(format!("/{name}: insufficient parameters")));
                }
                let (mask, info) = if name == "guser" {
                    let (ty, info) = match split_word(rest) {
                        (t, i) if t.parse::<u8>().is_ok() => (t.to_string(), i),
                        _ => (String::new(), rest),
                    };
                    let args = if ty.is_empty() { vec![who.to_string()] } else { vec![who.to_string(), ty] };
                    let addr = h.identifier(ctx, "address", &args, None).filter(|a| !a.is_empty());
                    match addr {
                        Some(a) => (a, info.to_string()),
                        None => return Err(self.err(format!("/guser: unable to find address of {who}"))),
                    }
                } else {
                    (who.to_string(), rest.to_string())
                };
                self.users.add(levels, &mask, &info, sw.contains('a'));
            }
            "ruser" => {
                let w: Vec<&str> = rest.split_whitespace().collect();
                match w.as_slice() {
                    [] => return Err(self.err("/ruser: insufficient parameters")),
                    [who] => self.users.list.retain(|u| !u.mask.eq_ignore_ascii_case(who) && !nick_of(&u.mask).eq_ignore_ascii_case(who)),
                    [a, b, ..] => {
                        if b.parse::<u8>().is_ok() {
                            // pseudo + type : on retire l'entrée qui correspond à son adresse.
                            let addr = self.full_address_of(h, ctx, a);
                            self.users.list.retain(|u| !mask_matches(&u.mask, &addr));
                        } else {
                            let lv = parse_levels(a);
                            for u in self.users.list.iter_mut().filter(|u| u.mask.eq_ignore_ascii_case(b)) {
                                u.levels.retain(|l| !lv.iter().any(|x| x.eq_ignore_ascii_case(l)));
                            }
                            self.users.list.retain(|u| !u.levels.is_empty());
                        }
                    }
                }
            }
            "rlevel" => {
                let lv = parse_levels(rest.trim());
                if sw.contains('r') {
                    for u in self.users.list.iter_mut() {
                        u.levels.retain(|l| !lv.iter().any(|x| x.eq_ignore_ascii_case(l)));
                    }
                    self.users.list.retain(|u| !u.levels.is_empty());
                } else {
                    self.users.list.retain(|u| !u.levels.iter().any(|l| lv.iter().any(|x| x.eq_ignore_ascii_case(l))));
                }
            }
            _ => {
                // /flush : les entrées « pseudo seul » des pseudos absents ; le moteur ne connaît pas les canaux,
                // il ne fait rien.
            }
        }
        Ok(())
    }

    /// `$ulist(adresse, [niveau], N).info`.
    pub(crate) fn ulist_ident(&self, args: &[String], prop: Option<&str>) -> String {
        let a = |i: usize| args.get(i).map(|s| s.as_str()).unwrap_or("");
        let addr = a(0);
        let (level, n) = if args.len() >= 3 { (a(1), a(2)) } else { ("", a(1)) };
        let n: i64 = n.parse().unwrap_or(1);
        let hits: Vec<&User> = self
            .users
            .list
            .iter()
            .filter(|u| wildmatch(addr, &u.mask, false) || mask_matches(&u.mask, addr))
            .filter(|u| level.is_empty() || u.levels.iter().any(|l| l.eq_ignore_ascii_case(level)))
            .collect();
        if n == 0 {
            return hits.len().to_string();
        }
        match hits.get((n - 1).max(0) as usize) {
            Some(u) if prop == Some("info") => u.info.clone(),
            Some(u) => u.mask.clone(),
            None => String::new(),
        }
    }
}

fn nick_of(mask: &str) -> &str {
    mask.split('!').next().unwrap_or(mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        let u = Users { list: "5,friend:*!*@good.host\n10:Boss\n".lines().filter_map(Users::parse_line).collect() };
        assert_eq!(u.numeric_level("x!y@good.host"), 5);
        assert_eq!(u.numeric_level("x!y@bad.host"), 1);
        assert_eq!(u.numeric_level("Boss!b@any"), 10);
        assert!(u.event_level("3", false, "x!y@good.host").is_some());
        assert!(u.event_level("6", false, "x!y@good.host").is_none());
        assert!(u.event_level("friend", false, "x!y@good.host").is_some());
        assert!(u.event_level("1", true, "z!z@else").is_some());
        assert!(u.event_level("5", true, "x!y@good.host").is_some());
        assert_eq!(u.text(), "5,friend:*!*@good.host\n10:Boss\n");
    }
}
