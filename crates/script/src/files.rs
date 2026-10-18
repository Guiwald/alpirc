//! Fichiers du bac à sable : `/write`, `/remove`, `/rename`, `/copy`, `/mkdir`, `/rmdir`, `/writeini`, `/remini`,
//! `$read`, `$lines`, `$isfile`, `$isdir`, `$exists`, `$file`, `$readini`, `$ini`.
//!
//! Tout chemin est lu par rapport au dossier racine du moteur, normalisé (`.`, `..`, `\` de Windows) et doit y
//! rester : sinon l'accès est refusé ([`crate::ScriptError::Sandbox`]). Un lien symbolique qui sort du dossier
//! est refusé aussi.

use std::path::{Component, Path, PathBuf};

use crate::calc::parse_num;
use crate::exec::{Abort, R, split_word};
use crate::idents::{rand_u64, tf};
use crate::wild::wildmatch;
use crate::{Ctx, Engine, Host, Msg};

/// Décode un fichier : UTF-8 (BOM retiré), sinon Windows-1252 (vieux scripts).
pub fn decode(bytes: &[u8]) -> String {
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    alpirc_proto::encoding::Encoding::Auto.decode(b).into_owned()
}

/// Prend un nom de fichier en tête (entre guillemets s'il contient des espaces) ; rend (nom, reste).
pub fn take_filename(s: &str) -> (String, &str) {
    let s = s.trim_start();
    if let Some(r) = s.strip_prefix('"')
        && let Some(e) = r.find('"')
    {
        return (r[..e].to_string(), r[e + 1..].trim_start());
    }
    let (a, b) = split_word(s);
    (a.to_string(), b)
}

/// Normalisation lexicale (sans toucher au disque).
pub(crate) fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Lignes d'un texte (fins de ligne `\r\n` ou `\n`).
fn lines_of(text: &str) -> Vec<String> {
    let mut v: Vec<String> = text.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect();
    if v.last().is_some_and(|l| l.is_empty()) {
        v.pop();
    }
    v
}

fn write_lines(path: &Path, lines: &[String], final_newline: bool) -> std::io::Result<()> {
    let mut out = lines.join("\n");
    if final_newline && !lines.is_empty() {
        out.push('\n');
    }
    std::fs::write(path, out)
}

/// Section et clés d'un `.ini`.
fn ini_parse(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut out: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for l in text.lines() {
        let t = l.trim_end_matches('\r').trim();
        if t.starts_with('[') && t.ends_with(']') {
            out.push((t[1..t.len() - 1].to_string(), Vec::new()));
        } else if let Some((k, v)) = t.split_once('=')
            && let Some(sec) = out.last_mut()
        {
            sec.1.push((k.to_string(), v.to_string()));
        }
    }
    out
}

fn ini_write(secs: &[(String, Vec<(String, String)>)]) -> String {
    let mut out = String::new();
    for (s, items) in secs {
        out.push_str(&format!("[{s}]\n"));
        for (k, v) in items {
            out.push_str(&format!("{k}={v}\n"));
        }
    }
    out
}

impl Engine {
    /// Chemin réel d'un fichier demandé par un script, ou refus s'il sort du dossier des scripts.
    pub(crate) fn sandbox_path(&self, user: &str) -> R<PathBuf> {
        let p = user.trim().trim_matches('"').replace('\\', "/");
        let deny = || Abort::Sandbox(user.trim().to_string());
        if p.is_empty() {
            return Err(deny());
        }
        // Lecteur Windows (`C:/…`) : jamais dans le dossier des scripts.
        if p.len() >= 2 && p.as_bytes()[1] == b':' && p.as_bytes()[0].is_ascii_alphabetic() {
            return Err(deny());
        }
        let root = normalize(self.root());
        let cand = if Path::new(&p).is_absolute() { PathBuf::from(&p) } else { root.join(&p) };
        let norm = normalize(&cand);
        if !norm.starts_with(&root) || norm == root {
            return Err(deny());
        }
        // Liens symboliques : le plus proche parent existant doit rester dans la racine réelle.
        if let Ok(real_root) = std::fs::canonicalize(&root) {
            let mut probe = norm.clone();
            loop {
                if probe.exists() {
                    match std::fs::canonicalize(&probe) {
                        Ok(r) if r.starts_with(&real_root) => break,
                        _ => return Err(deny()),
                    }
                }
                if !probe.pop() {
                    break;
                }
            }
        }
        Ok(norm)
    }

    fn read_file_text(&self, user: &str) -> R<Option<String>> {
        let p = self.sandbox_path(user)?;
        Ok(std::fs::read(&p).ok().map(|b| decode(&b)))
    }

    /// `/write [-cidan] [-lN] [-sTexte] [-wJoker] fichier [texte]`.
    pub(crate) fn cmd_write(&mut self, args: &str) -> R<()> {
        let mut rest = args.trim_start();
        let (mut clear, mut del, mut ins, mut app, mut nonl) = (false, false, false, false, false);
        let mut line: Option<usize> = None;
        let mut scan: Option<(String, bool)> = None;
        while rest.starts_with('-') && rest.len() > 1 {
            let (w, r) = split_word(rest);
            let cs: Vec<char> = w[1..].chars().collect();
            let mut i = 0;
            while i < cs.len() {
                match cs[i] {
                    'c' => clear = true,
                    'd' => del = true,
                    'i' => ins = true,
                    'a' => app = true,
                    'n' => nonl = true,
                    'l' => {
                        let d: String = cs[i + 1..].iter().take_while(|c| c.is_ascii_digit()).collect();
                        i += d.len();
                        line = d.parse().ok();
                    }
                    's' | 'w' => {
                        let t: String = cs[i + 1..].iter().collect();
                        scan = Some((t, cs[i] == 'w'));
                        i = cs.len();
                        continue;
                    }
                    _ => {}
                }
                i += 1;
            }
            rest = r;
        }
        let (file, text) = take_filename(rest);
        if file.is_empty() {
            return Err(self.err(Msg::InsufficientParams { cmd: "write".into() }));
        }
        let path = self.sandbox_path(&file)?;
        let mut lines = if clear { Vec::new() } else { std::fs::read(&path).map(|b| lines_of(&decode(&b))).unwrap_or_default() };
        let target: Option<usize> = match (&scan, line) {
            (Some((t, wild)), _) => lines.iter().position(|l| {
                if *wild {
                    wildmatch(t, l, false)
                } else {
                    l.to_lowercase().starts_with(&t.to_lowercase())
                }
            }),
            (None, Some(n)) if n >= 1 => Some(n - 1),
            _ => None,
        };
        if del {
            match target {
                Some(i) if i < lines.len() => {
                    lines.remove(i);
                }
                None if line.is_none() && scan.is_none() => {
                    lines.pop();
                }
                _ => {}
            }
        } else if ins {
            let i = target.unwrap_or(lines.len()).min(lines.len());
            lines.insert(i, text.to_string());
        } else if app {
            match target {
                Some(i) if i < lines.len() => lines[i].push_str(text),
                _ => match lines.last_mut() {
                    Some(l) => l.push_str(text),
                    None => lines.push(text.to_string()),
                },
            }
        } else {
            match target {
                Some(i) if i < lines.len() => lines[i] = text.to_string(),
                Some(i) => {
                    while lines.len() < i {
                        lines.push(String::new());
                    }
                    lines.push(text.to_string());
                }
                None if scan.is_some() => lines.push(text.to_string()),
                None => {
                    if !(clear && text.is_empty()) {
                        lines.push(text.to_string());
                    }
                }
            }
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        write_lines(&path, &lines, !nonl)
            .map_err(|_| self.err(Msg::CannotOpen { cmd: "write".into(), file: file.to_string() }))
    }

    /// `/remove fichier`.
    pub(crate) fn cmd_remove(&mut self, args: &str) -> R<()> {
        let (file, _) = take_filename(args);
        let p = self.sandbox_path(&file)?;
        if p.is_file() {
            std::fs::remove_file(&p)
                .map_err(|_| self.err(Msg::CannotRemove { cmd: "remove".into(), file: file.to_string() }))?;
        }
        Ok(())
    }

    /// `/rename ancien nouveau`.
    pub(crate) fn cmd_rename(&mut self, args: &str) -> R<()> {
        let (a, rest) = take_filename(args);
        let (b, _) = take_filename(rest);
        let (pa, pb) = (self.sandbox_path(&a)?, self.sandbox_path(&b)?);
        std::fs::rename(&pa, &pb).map_err(|_| self.err(Msg::CannotRename { cmd: "rename".into(), file: a.to_string() }))
    }

    /// `/copy [-o] fichier destination` (destination : fichier ou dossier).
    pub(crate) fn cmd_copy(&mut self, args: &str) -> R<()> {
        let (sw, rest) = crate::exec::switches(args);
        let (a, rest) = take_filename(rest);
        let (b, _) = take_filename(rest);
        let pa = self.sandbox_path(&a)?;
        let mut pb = self.sandbox_path(&b)?;
        if pb.is_dir()
            && let Some(n) = pa.file_name()
        {
            pb.push(n);
        }
        if pb.exists() && !sw.contains('o') {
            return Err(self.err(Msg::FileExists { cmd: "copy".into(), file: b.to_string() }));
        }
        std::fs::copy(&pa, &pb).map(|_| ())
            .map_err(|_| self.err(Msg::CannotCopy { cmd: "copy".into(), file: a.to_string() }))
    }

    pub(crate) fn cmd_mkdir(&mut self, args: &str) -> R<()> {
        let (d, _) = take_filename(args);
        let p = self.sandbox_path(&d)?;
        std::fs::create_dir_all(&p)
            .map_err(|_| self.err(Msg::CannotCreate { cmd: "mkdir".into(), file: d.to_string() }))
    }

    pub(crate) fn cmd_rmdir(&mut self, args: &str) -> R<()> {
        let (d, _) = take_filename(args);
        let p = self.sandbox_path(&d)?;
        std::fs::remove_dir(&p).map_err(|_| self.err(Msg::CannotRemove { cmd: "rmdir".into(), file: d.to_string() }))
    }

    /// `/writeini [-n] fichier section élément valeur`.
    pub(crate) fn cmd_writeini(&mut self, args: &str) -> R<()> {
        let (_, rest) = crate::exec::switches(args);
        let (file, rest) = take_filename(rest);
        let (sec, rest) = split_word(rest);
        let (item, value) = split_word(rest);
        if item.is_empty() {
            return Err(self.err(Msg::InsufficientParams { cmd: "writeini".into() }));
        }
        let path = self.sandbox_path(&file)?;
        let mut secs = std::fs::read(&path).map(|b| ini_parse(&decode(&b))).unwrap_or_default();
        let si = match secs.iter().position(|(s, _)| s.eq_ignore_ascii_case(sec)) {
            Some(i) => i,
            None => {
                secs.push((sec.to_string(), Vec::new()));
                secs.len() - 1
            }
        };
        let items = &mut secs[si].1;
        match items.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(item)) {
            Some(kv) => kv.1 = value.to_string(),
            None => items.push((item.to_string(), value.to_string())),
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&path, ini_write(&secs))
            .map_err(|_| self.err(Msg::CannotWrite { cmd: "writeini".into(), file: file.to_string() }))
    }

    /// `/remini fichier section [élément]`.
    pub(crate) fn cmd_remini(&mut self, args: &str) -> R<()> {
        let (file, rest) = take_filename(args);
        let (sec, item) = split_word(rest);
        let path = self.sandbox_path(&file)?;
        let Ok(b) = std::fs::read(&path) else {
            return Ok(());
        };
        let mut secs = ini_parse(&decode(&b));
        if item.trim().is_empty() {
            secs.retain(|(s, _)| !s.eq_ignore_ascii_case(sec));
        } else if let Some(s) = secs.iter_mut().find(|(s, _)| s.eq_ignore_ascii_case(sec)) {
            s.1.retain(|(k, _)| !k.eq_ignore_ascii_case(item.trim()));
        }
        std::fs::write(&path, ini_write(&secs))
            .map_err(|_| self.err(Msg::CannotWrite { cmd: "remini".into(), file: file.to_string() }))
    }

    /// `$read(fichier, [ntswr], [texte], [N])`.
    pub(crate) fn read_ident(&mut self, h: &mut dyn Host, ctx: &Ctx, args: &[String]) -> R<String> {
        let a = |i: usize| args.get(i).map(|s| s.as_str()).unwrap_or("");
        let Some(text) = self.read_file_text(a(0))? else {
            self.rt.readn = 0;
            return Ok(String::new());
        };
        let mut lines = lines_of(&text);
        let (flags, matchtext, start): (String, String, Option<usize>) =
            if args.len() == 2 && parse_num(a(1)).is_some() {
                (String::new(), String::new(), parse_num(a(1)).map(|v| v as usize))
            } else {
                let f = a(1).to_string();
                if f.contains(['s', 'w', 'r']) {
                    (f, a(2).to_string(), parse_num(a(3)).map(|v| v as usize))
                } else {
                    (f, String::new(), parse_num(a(2)).map(|v| v as usize))
                }
            };
        // Première ligne numérique = nombre de lignes (sauf `t`).
        let mut offset = 0;
        if !flags.contains('t') && lines.first().is_some_and(|l| parse_num(l).is_some()) {
            if start == Some(0) && matchtext.is_empty() {
                self.rt.readn = 0;
                return Ok(lines[0].clone());
            }
            lines.remove(0);
            offset = 1;
        }
        let _ = offset;
        let found: Option<(usize, String)> = if flags.contains(['s', 'w', 'r']) {
            let from = start.unwrap_or(1).max(1) - 1;
            let re = if flags.contains('r') { crate::regexid::compile(&matchtext).ok() } else { None };
            lines.iter().enumerate().skip(from).find_map(|(i, l)| {
                if flags.contains('s') {
                    let lw = l.to_lowercase();
                    let mt = matchtext.to_lowercase();
                    if lw == mt {
                        return Some((i, String::new()));
                    }
                    if lw.starts_with(&format!("{mt} ")) {
                        return Some((i, l[matchtext.len() + 1..].to_string()));
                    }
                    None
                } else if flags.contains('w') {
                    wildmatch(&matchtext, l, false).then(|| (i, l.clone()))
                } else {
                    re.as_ref().is_some_and(|r| r.re.is_match(l).unwrap_or(false)).then(|| (i, l.clone()))
                }
            })
        } else if let Some(n) = start {
            if n >= 1 && n <= lines.len() { Some((n - 1, lines[n - 1].clone())) } else { None }
        } else if lines.is_empty() {
            None
        } else {
            let i = (rand_u64() % lines.len() as u64) as usize;
            Some((i, lines[i].clone()))
        };
        match found {
            Some((i, l)) => {
                self.rt.readn = i + 1;
                if flags.contains('n') { Ok(l) } else { self.eval_text(h, ctx, &l) }
            }
            None => {
                self.rt.readn = 0;
                Ok(String::new())
            }
        }
    }

    pub(crate) fn lines_ident(&mut self, file: &str) -> R<String> {
        Ok(match self.read_file_text(file)? {
            Some(t) => lines_of(&t).len().to_string(),
            None => "0".into(),
        })
    }

    /// `$isfile`, `$isdir`, `$exists`, `$file(…).size|mtime|name|ext|path`.
    pub(crate) fn file_ident(&mut self, name: &str, file: &str, prop: Option<&str>) -> R<String> {
        let p = match self.sandbox_path(file) {
            Ok(p) => p,
            Err(_) if name != "file" => return Ok(tf(false)),
            Err(e) => return Err(e),
        };
        Ok(match name {
            "isfile" => tf(p.is_file()),
            "isdir" => tf(p.is_dir()),
            "exists" => tf(p.exists()),
            _ => {
                let Ok(md) = std::fs::metadata(&p) else {
                    return Ok(String::new());
                };
                match prop {
                    Some("mtime") | Some("ctime") | Some("atime") => md
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_default(),
                    Some("name") => p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                    Some("ext") => p.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default(),
                    Some("path") => p.parent().map(|d| format!("{}/", d.display())).unwrap_or_default(),
                    Some("longfn") | Some("shortfn") => p.display().to_string(),
                    _ => md.len().to_string(),
                }
            }
        })
    }

    /// `$readini(fichier, [np], section, élément)`.
    pub(crate) fn readini_ident(&mut self, h: &mut dyn Host, ctx: &Ctx, args: &[String]) -> R<String> {
        let a = |i: usize| args.get(i).map(|s| s.as_str()).unwrap_or("");
        let (flags, sec, item) = if args.len() >= 4 { (a(1), a(2), a(3)) } else { ("", a(1), a(2)) };
        let Some(text) = self.read_file_text(a(0))? else {
            return Ok(String::new());
        };
        let secs = ini_parse(&text);
        let v = secs
            .iter()
            .find(|(s, _)| s.eq_ignore_ascii_case(sec))
            .and_then(|(_, items)| items.iter().find(|(k, _)| k.eq_ignore_ascii_case(item)))
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        if flags.contains('n') { Ok(v) } else { self.eval_text(h, ctx, &v) }
    }

    /// `$ini(fichier, section|N, [élément|N])`.
    pub(crate) fn ini_ident(&mut self, args: &[String]) -> R<String> {
        let a = |i: usize| args.get(i).map(|s| s.as_str()).unwrap_or("");
        let Some(text) = self.read_file_text(a(0))? else {
            return Ok(String::new());
        };
        let secs = ini_parse(&text);
        let sec_idx = match parse_num(a(1)) {
            Some(n) if args.len() == 2 => {
                if n == 0.0 {
                    return Ok(secs.len().to_string());
                }
                return Ok(secs.get(n as usize - 1).map(|s| s.0.clone()).unwrap_or_default());
            }
            Some(n) => (n as usize).checked_sub(1),
            None => secs.iter().position(|(s, _)| s.eq_ignore_ascii_case(a(1))),
        };
        let Some(si) = sec_idx.filter(|i| *i < secs.len()) else {
            return Ok(String::new());
        };
        if args.len() == 2 {
            return Ok((si + 1).to_string());
        }
        let items = &secs[si].1;
        Ok(match parse_num(a(2)) {
            Some(0.0) => items.len().to_string(),
            Some(n) => items.get(n as usize - 1).map(|i| i.0.clone()).unwrap_or_default(),
            None => items.iter().position(|(k, _)| k.eq_ignore_ascii_case(a(2))).map(|i| (i + 1).to_string()).unwrap_or_default(),
        })
    }
}
