//! Tables de hachage : `/hmake`, `/hadd`, `/hdel`, `/hfree`, `/hinc`, `/hdec`, `/hload`, `/hsave`, `$hget`,
//! `$hfind`. Les éléments gardent leur ordre d'ajout ; les noms sont insensibles à la casse.

use std::collections::{BTreeMap, HashMap};

use crate::calc::{fmt_num, parse_num, parse_num_prefix};
use crate::exec::{R, split_word, switch_num, switches};
use crate::wild::wildmatch;
use crate::{Ctx, Engine, Host, Msg};

#[derive(Debug, Clone)]
struct Item {
    key: String,
    data: String,
    expires: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub size: usize,
    items: BTreeMap<u64, Item>,
    index: HashMap<String, u64>,
    next: u64,
}

impl Table {
    fn new(name: &str, size: usize) -> Table {
        Table { name: name.to_string(), size, items: BTreeMap::new(), index: HashMap::new(), next: 0 }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.index.get(&key.to_lowercase()).and_then(|s| self.items.get(s)).map(|i| i.data.as_str())
    }

    pub fn set(&mut self, key: &str, data: String, expires: Option<i64>) {
        let l = key.to_lowercase();
        if let Some(s) = self.index.get(&l)
            && let Some(it) = self.items.get_mut(s)
        {
            it.data = data;
            it.expires = expires;
            return;
        }
        self.next += 1;
        self.index.insert(l, self.next);
        self.items.insert(self.next, Item { key: key.to_string(), data, expires });
    }

    pub fn del(&mut self, key: &str) -> bool {
        match self.index.remove(&key.to_lowercase()) {
            Some(s) => self.items.remove(&s).is_some(),
            None => false,
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// (élément, donnée) dans l'ordre.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.items.values().map(|i| (i.key.as_str(), i.data.as_str()))
    }

    fn nth(&self, k: usize) -> Option<(&str, &str)> {
        self.entries().nth(k)
    }

    fn expire(&mut self, now: i64) {
        let gone: Vec<String> =
            self.items.values().filter(|i| i.expires.is_some_and(|e| e <= now)).map(|i| i.key.clone()).collect();
        for k in gone {
            self.del(&k);
        }
    }

    fn next_due(&self) -> Option<i64> {
        self.items.values().filter_map(|i| i.expires).min()
    }
}

/// Toutes les tables.
#[derive(Debug, Default)]
pub struct Tables {
    map: HashMap<String, Table>,
    order: Vec<String>,
}

impl Tables {
    pub fn get(&self, name: &str) -> Option<&Table> {
        self.map.get(&name.to_lowercase())
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Table> {
        self.map.get_mut(&name.to_lowercase())
    }

    pub fn make(&mut self, name: &str, size: usize) -> bool {
        let l = name.to_lowercase();
        if self.map.contains_key(&l) {
            return false;
        }
        self.map.insert(l.clone(), Table::new(name, size));
        self.order.push(l);
        true
    }

    pub fn free(&mut self, name: &str) -> bool {
        let l = name.to_lowercase();
        self.order.retain(|x| *x != l);
        self.map.remove(&l).is_some()
    }

    pub fn names(&self) -> Vec<String> {
        self.order.iter().filter_map(|l| self.map.get(l)).map(|t| t.name.clone()).collect()
    }

    pub fn tick(&mut self, now: i64) {
        for t in self.map.values_mut() {
            t.expire(now);
        }
    }

    pub fn next_due(&self) -> Option<i64> {
        self.map.values().filter_map(|t| t.next_due()).min()
    }
}

impl Engine {
    fn no_table(&self, cmd: &str, name: &str) -> crate::exec::Abort {
        self.err(Msg::NoSuchTable { cmd: cmd.to_string(), name: name.to_string() })
    }

    /// `/hmake [-s] nom [N]`.
    pub(crate) fn cmd_hmake(&mut self, h: &mut dyn Host, ctx: &Ctx, args: &str, quiet: bool) -> R<()> {
        let (sw, rest) = switches(args);
        let (name, size) = split_word(rest);
        if name.is_empty() {
            return Err(self.err(Msg::InsufficientParams { cmd: "hmake".into() }));
        }
        let size = size.trim().parse().unwrap_or(100);
        if !self.tables.make(name, size) {
            return Err(self.err(Msg::TableExists { cmd: "hmake".into(), name: name.to_string() }));
        }
        if sw.contains('s') && !quiet {
            self.info(h, ctx, Msg::MadeTable { name: name.to_string(), size });
        }
        Ok(())
    }

    /// `/hadd [-smuN] nom élément [donnée]`.
    pub(crate) fn cmd_hadd(&mut self, h: &mut dyn Host, ctx: &Ctx, args: &str) -> R<()> {
        let (sw, rest) = switches(args);
        let (name, rest) = split_word(rest);
        let (item, data) = split_word(rest);
        if item.is_empty() {
            return Err(self.err(Msg::InsufficientParams { cmd: "hadd".into() }));
        }
        if sw.contains('m') {
            self.tables.make(name, 100);
        }
        let exp = self.expiry(h, &sw);
        let Some(t) = self.tables.get_mut(name) else {
            return Err(self.no_table("hadd", name));
        };
        t.set(item, data.to_string(), exp);
        if sw.contains('s') {
            self.info(h, ctx, Msg::AddedItem { item: item.to_string(), table: name.to_string() });
        }
        Ok(())
    }

    fn expiry(&self, h: &dyn Host, sw: &str) -> Option<i64> {
        if sw.contains('u') {
            switch_num(sw, 'u').filter(|s| *s > 0.0).map(|s| h.now_ms() + (s * 1000.0) as i64)
        } else {
            None
        }
    }

    /// `/hdel [-w] nom élément`.
    pub(crate) fn cmd_hdel(&mut self, args: &str) -> R<()> {
        let (sw, rest) = switches(args);
        let (name, item) = split_word(rest);
        let item = item.trim();
        let Some(t) = self.tables.get_mut(name) else {
            return Err(self.no_table("hdel", name));
        };
        if sw.contains('w') {
            let keys: Vec<String> = t.entries().filter(|(k, _)| wildmatch(item, k, false)).map(|(k, _)| k.to_string()).collect();
            for k in keys {
                t.del(&k);
            }
        } else {
            t.del(item);
        }
        Ok(())
    }

    /// `/hfree [-w] nom`.
    pub(crate) fn cmd_hfree(&mut self, args: &str) -> R<()> {
        let (sw, rest) = switches(args);
        let name = rest.trim();
        if sw.contains('w') {
            for n in self.tables.names() {
                if wildmatch(name, &n, false) {
                    self.tables.free(&n);
                }
            }
            return Ok(());
        }
        if !self.tables.free(name) {
            return Err(self.no_table("hfree", name));
        }
        Ok(())
    }

    /// `/hinc` et `/hdec [-muN] nom élément [N]`.
    pub(crate) fn cmd_hinc(&mut self, h: &mut dyn Host, args: &str, sign: f64) -> R<()> {
        let (sw, rest) = switches(args);
        let (name, rest) = split_word(rest);
        let (item, by) = split_word(rest);
        if sw.contains('m') {
            self.tables.make(name, 100);
        }
        let exp = self.expiry(h, &sw);
        let cmd = if sign > 0.0 { "hinc" } else { "hdec" };
        let Some(t) = self.tables.get_mut(name) else {
            return Err(self.no_table(cmd, name));
        };
        let by = if by.trim().is_empty() { 1.0 } else { parse_num_prefix(by) };
        let cur = t.get(item).map(parse_num_prefix).unwrap_or(0.0);
        t.set(item, fmt_num(cur + sign * by), exp);
        Ok(())
    }

    /// `/hload [-mni] nom fichier [section]` : par défaut, lignes alternées élément / donnée.
    pub(crate) fn cmd_hload(&mut self, args: &str) -> R<()> {
        let (sw, rest) = switches(args);
        let (name, rest) = split_word(rest);
        let (file, section) = crate::files::take_filename(rest);
        let path = self.sandbox_path(&file)?;
        let text = std::fs::read(&path)
            .map_err(|_| self.err(Msg::CannotOpen { cmd: "hload".into(), file: file.to_string() }))?;
        let text = crate::files::decode(&text);
        if sw.contains('m') {
            self.tables.make(name, 100);
        }
        let Some(t) = self.tables.get_mut(name) else {
            return Err(self.no_table("hload", name));
        };
        let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
        if sw.contains('i') {
            let want = section.trim().to_lowercase();
            let mut cur = String::new();
            for l in lines {
                let tl = l.trim();
                if tl.starts_with('[') && tl.ends_with(']') {
                    cur = tl[1..tl.len() - 1].to_lowercase();
                } else if (want.is_empty() || cur == want)
                    && !tl.is_empty()
                    && let Some((k, v)) = tl.split_once('=')
                {
                    t.set(k, v.to_string(), None);
                }
            }
        } else if sw.contains('n') {
            for (i, l) in lines.iter().enumerate() {
                t.set(&(i + 1).to_string(), l.to_string(), None);
            }
        } else {
            for pair in lines.chunks(2) {
                if !pair[0].is_empty() {
                    t.set(pair[0], pair.get(1).copied().unwrap_or("").to_string(), None);
                }
            }
        }
        Ok(())
    }

    /// `/hsave [-nia] nom fichier [section]` (`-a` : ajoute au fichier).
    pub(crate) fn cmd_hsave(&mut self, args: &str) -> R<()> {
        let (sw, rest) = switches(args);
        let (name, rest) = split_word(rest);
        let (file, section) = crate::files::take_filename(rest);
        let path = self.sandbox_path(&file)?;
        let Some(t) = self.tables.get(name) else {
            return Err(self.no_table("hsave", name));
        };
        let mut out = String::new();
        if sw.contains('i') {
            let sec = if section.trim().is_empty() { "hashtable" } else { section.trim() };
            out.push_str(&format!("[{sec}]\n"));
            for (k, v) in t.entries() {
                out.push_str(&format!("{k}={v}\n"));
            }
        } else if sw.contains('n') {
            for (_, v) in t.entries() {
                out.push_str(v);
                out.push('\n');
            }
        } else {
            for (k, v) in t.entries() {
                out.push_str(k);
                out.push('\n');
                out.push_str(v);
                out.push('\n');
            }
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let r = if sw.contains('a') {
            use std::io::Write;
            std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| f.write_all(out.as_bytes()))
        } else {
            std::fs::write(&path, out)
        };
        r.map_err(|_| self.err(Msg::CannotWrite { cmd: "hsave".into(), file: file.to_string() }))
    }

    /// `$hget(nom)`, `$hget(N)`, `$hget(nom).size`, `$hget(nom, élément)`, `$hget(nom, N).item|.data`.
    pub(crate) fn hget_ident(&mut self, _h: &mut dyn Host, args: &[String], prop: Option<&str>) -> String {
        let a = |i: usize| args.get(i).map(|s| s.as_str()).unwrap_or("");
        if args.len() <= 1 {
            let name = a(0);
            if let Some(n) = parse_num(name).filter(|_| self.tables.get(name).is_none()) {
                let names = self.tables.names();
                if n == 0.0 {
                    return names.len().to_string();
                }
                return names.get(n as usize - 1).cloned().unwrap_or_default();
            }
            return match self.tables.get(name) {
                Some(t) if prop == Some("size") => t.size.to_string(),
                Some(t) => t.name.clone(),
                None => String::new(),
            };
        }
        let Some(t) = self.tables.get(a(0)) else {
            return String::new();
        };
        let item = a(1);
        if matches!(prop, Some("item") | Some("data"))
            && let Some(k) = parse_num(item)
        {
            if k == 0.0 {
                return t.len().to_string();
            }
            return match t.nth(k as usize - 1) {
                Some((key, data)) => (if prop == Some("item") { key } else { data }).to_string(),
                None => String::new(),
            };
        }
        t.get(item).unwrap_or("").to_string()
    }

    /// `$hfind(nom, texte, N, [nwWrR])` (`.data` : cherche dans les données) → élément ; N = 0 → nombre.
    pub(crate) fn hfind_ident(&mut self, args: &[String], prop: Option<&str>) -> String {
        let a = |i: usize| args.get(i).map(|s| s.as_str()).unwrap_or("");
        let Some(t) = self.tables.get(a(0)) else {
            return String::new();
        };
        let text = a(1);
        let n = parse_num(a(2)).unwrap_or(1.0) as i64;
        let mode = if a(3).is_empty() { "n" } else { a(3) };
        let re = if mode.contains(['r', 'R']) {
            crate::regexid::compile(if mode.contains('r') { text } else { "" }).ok()
        } else {
            None
        };
        let on_data = prop == Some("data");
        let hits: Vec<String> = t
            .entries()
            .filter(|(k, d)| {
                let subject = if on_data { *d } else { *k };
                if mode.contains('w') {
                    wildmatch(text, subject, false)
                } else if mode.contains('W') {
                    wildmatch(subject, text, false)
                } else if mode.contains('r') {
                    re.as_ref().is_some_and(|r| r.re.is_match(subject).unwrap_or(false))
                } else if mode.contains('R') {
                    crate::regexid::compile(subject).ok().is_some_and(|r| r.re.is_match(text).unwrap_or(false))
                } else {
                    subject.eq_ignore_ascii_case(text)
                }
            })
            .map(|(k, _)| k.to_string())
            .collect();
        if n == 0 {
            return hits.len().to_string();
        }
        hits.get((n - 1).max(0) as usize).cloned().unwrap_or_default()
    }
}
