//! Identifiants fournis par le moteur : texte, jetons, calcul, temps, hachages, expressions régulières,
//! variables, groupes, scripts… Les identifiants d'état du client viennent de l'hôte ([`crate::Host::identifier`]).

use base64::Engine as _;
use md5::Digest as _;

use crate::calc::{calc, fmt_num, parse_num, parse_num_prefix};
use crate::exec::R;
use crate::regexid;
use crate::strings as st;
use crate::timefmt;
use crate::wild::wildmatch;
use crate::{Ctx, Engine, Host, Msg};

pub const TRUE: &str = "$true";
pub const FALSE: &str = "$false";

pub fn tf(b: bool) -> String {
    if b { TRUE.into() } else { FALSE.into() }
}

/// Nombre aléatoire (non cryptographique, mais tiré du système).
pub fn rand_u64() -> u64 {
    let mut b = [0u8; 8];
    if getrandom::fill(&mut b).is_err() {
        // Repli : l'horloge.
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        return (t as u64).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    }
    u64::from_le_bytes(b)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn int(s: &str) -> i64 {
    parse_num_prefix(s) as i64
}

/// `$ord(N)` : `1st`, `2nd`, `11th`…
fn ordinal(n: i64) -> String {
    let suf = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suf}")
}

impl Engine {
    /// Secondes depuis 1970 (horloge de l'hôte).
    pub(crate) fn now_secs(&self, h: &dyn Host) -> i64 {
        h.now_ms().div_euclid(1000)
    }

    /// Expression régulière refusée : paramètres invalides, avec la précision de la bibliothèque.
    fn bad_regex(&self, ident: &str, detail: String) -> crate::exec::Abort {
        self.err(Msg::IdentParams { ident: ident.to_string(), detail: Some(detail) })
    }

    /// Identifiants qui reçoivent leurs paramètres bruts : `$eval`/`$()`, `$iif`, `$regsubex`, `$regsub`.
    pub(crate) fn raw_ident(
        &mut self,
        h: &mut dyn Host,
        ctx: &Ctx,
        name: &str,
        raw: &[&str],
        _prop: Option<&str>,
    ) -> R<Option<String>> {
        match name {
            "" | "eval" => {
                let n = match raw.get(1) {
                    Some(x) => int(&self.eval_arg(h, ctx, x)?),
                    None => 1,
                };
                let text = raw.first().copied().unwrap_or("");
                if n <= 0 {
                    return Ok(Some(text.to_string()));
                }
                let mut v = self.eval_arg(h, ctx, text)?;
                for _ in 1..n.min(100) {
                    v = self.eval_text(h, ctx, &v)?;
                }
                Ok(Some(v))
            }
            "var" => {
                // `$var(%motif*, N)` : le motif n'est pas évalué.
                let pat = raw.first().copied().unwrap_or("").trim().to_string();
                let k = match raw.get(1) {
                    Some(x) => int(&self.eval_arg(h, ctx, x)?),
                    None => 1,
                };
                Ok(Some(self.var_ident(&pat, k, _prop)))
            }
            "iif" => {
                let cond = raw.first().copied().unwrap_or("");
                let ok = self.eval_cond(h, ctx, cond)?;
                let pick = if ok { raw.get(1) } else { raw.get(2) };
                match pick {
                    Some(x) => Ok(Some(self.eval_arg(h, ctx, x)?)),
                    None => Ok(Some(String::new())),
                }
            }
            "regsubex" => {
                // $regsubex([nom,] texte, /re/, remplacement) : le remplacement est évalué pour chaque occurrence.
                let (rname, rest) = if raw.len() >= 4 {
                    (self.eval_arg(h, ctx, raw[0])?, &raw[1..])
                } else {
                    (String::new(), raw)
                };
                if rest.len() < 3 {
                    return Err(self.err(Msg::IdentParams { ident: "regsubex".into(), detail: None }));
                }
                let text = self.eval_arg(h, ctx, rest[0])?;
                let pat = self.eval_arg(h, ctx, rest[1])?;
                let re = regexid::compile(&pat).map_err(|e| self.bad_regex("regsubex", e))?;
                let text = if re.strip { alpirc_proto::format::strip(&text) } else { text };
                let found = regexid::find_all(&re, &text);
                let total = found.len();
                let mut out = String::new();
                let mut last = 0;
                let mut ml = Vec::new();
                for (k, f) in found.iter().enumerate() {
                    out.push_str(&text[last..f.range.0]);
                    let sub = regexid::expand(rest[2], f, k + 1, total, true);
                    out.push_str(&self.eval_arg(h, ctx, &sub)?);
                    last = f.range.1;
                    push_regml(&mut ml, f);
                }
                out.push_str(&text[last..]);
                self.rt.regml.insert(rname.to_ascii_lowercase(), ml);
                Ok(Some(out))
            }
            "regsub" => {
                // $regsub([nom,] texte, /re/, remplacement, %var) : rend le nombre de remplacements.
                let (rname, rest) = if raw.len() >= 5 {
                    (self.eval_arg(h, ctx, raw[0])?, &raw[1..])
                } else {
                    (String::new(), raw)
                };
                if rest.len() < 4 {
                    return Err(self.err(Msg::IdentParams { ident: "regsub".into(), detail: None }));
                }
                let text = self.eval_arg(h, ctx, rest[0])?;
                let pat = self.eval_arg(h, ctx, rest[1])?;
                let sub = self.eval_arg(h, ctx, rest[2])?;
                let var = rest[3].trim().to_string();
                let re = regexid::compile(&pat).map_err(|e| self.bad_regex("regsub", e))?;
                let found = regexid::find_all(&re, &text);
                let mut out = String::new();
                let mut last = 0;
                let mut ml = Vec::new();
                for (k, f) in found.iter().enumerate() {
                    out.push_str(&text[last..f.range.0]);
                    out.push_str(&regexid::expand(&sub, f, k + 1, found.len(), false));
                    last = f.range.1;
                    push_regml(&mut ml, f);
                }
                out.push_str(&text[last..]);
                self.rt.regml.insert(rname.to_ascii_lowercase(), ml);
                if var.starts_with('%') {
                    self.var_assign(&var, out);
                }
                Ok(Some(found.len().to_string()))
            }
            _ => Ok(None),
        }
    }

    /// Identifiants demandés à l'hôte d'abord, avec une valeur du moteur s'il ne les connaît pas.
    pub(crate) fn late_ident(
        &mut self,
        _h: &mut dyn Host,
        ctx: &Ctx,
        name: &str,
        _args: &[String],
        parens: bool,
        _prop: Option<&str>,
    ) -> R<Option<String>> {
        Ok(match name {
            "version" => Some("AlpIRC".into()),
            "active" if !parens => Some(ctx.window.clone().unwrap_or_default()),
            "os" => Some(std::env::consts::OS.into()),
            _ => None,
        })
    }

    /// Identifiants du moteur ; `None` si le nom n'en est pas un.
    pub(crate) fn builtin_ident(
        &mut self,
        h: &mut dyn Host,
        ctx: &Ctx,
        name: &str,
        args: &[String],
        parens: bool,
        prop: Option<&str>,
    ) -> R<Option<String>> {
        let a = |i: usize| -> &str { args.get(i).map(|s| s.as_str()).unwrap_or("") };
        let n = args.len();
        macro_rules! need {
            ($k:expr) => {
                if n < $k {
                    return Err(self.err(Msg::IdentParams { ident: name.to_string(), detail: None }));
                }
            };
        }
        macro_rules! sep {
            ($i:expr) => {
                match st::sep_char(a($i)) {
                    Some(c) => c,
                    None => return Err(self.err(Msg::IdentParams { ident: name.to_string(), detail: None })),
                }
            };
        }
        let v: String = match name {
            // --- constantes ---
            "null" => String::new(),
            "true" => TRUE.into(),
            "false" => FALSE.into(),
            "cr" => "\r".into(),
            "lf" => "\n".into(),
            "crlf" => "\r\n".into(),
            "tab" => "\t".into(),
            "pi" => "3.14159265358979323846".into(),
            "chr" => {
                let c = parse_num(a(0)).unwrap_or(0.0) as u32;
                char::from_u32(c).map(|c| c.to_string()).unwrap_or_default()
            }
            "asc" => a(0).chars().next().map(|c| (c as u32).to_string()).unwrap_or_default(),
            "+" => args.concat(),

            // --- calcul ---
            "calc" => fmt_num(calc(&args.join(","))),
            "int" => fmt_num(parse_num_prefix(a(0)).trunc()),
            "abs" => fmt_num(parse_num_prefix(a(0)).abs()),
            "sqrt" => fmt_num(parse_num_prefix(a(0)).sqrt()),
            "log" => fmt_num(parse_num_prefix(a(0)).ln()),
            "log10" => fmt_num(parse_num_prefix(a(0)).log10()),
            "floor" => fmt_num(parse_num_prefix(a(0)).floor()),
            "ceil" => fmt_num(parse_num_prefix(a(0)).ceil()),
            "round" => {
                let d = int(a(1)).clamp(0, 15) as i32;
                let m = 10f64.powi(d);
                fmt_num((parse_num_prefix(a(0)) * m).round() / m)
            }
            "sin" | "cos" | "tan" => {
                let mut x = parse_num_prefix(a(0));
                if prop == Some("deg") {
                    x = x.to_radians();
                }
                fmt_num(match name {
                    "sin" => x.sin(),
                    "cos" => x.cos(),
                    _ => x.tan(),
                })
            }
            "asin" | "acos" | "atan" => {
                let x = parse_num_prefix(a(0));
                let r = match name {
                    "asin" => x.asin(),
                    "acos" => x.acos(),
                    _ => x.atan(),
                };
                fmt_num(if prop == Some("deg") { r.to_degrees() } else { r })
            }
            "min" | "max" => {
                let vals: Vec<f64> = args.iter().filter_map(|x| parse_num(x)).collect();
                if vals.is_empty() {
                    String::new()
                } else {
                    let v = if name == "min" {
                        vals.iter().cloned().fold(f64::INFINITY, f64::min)
                    } else {
                        vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                    };
                    fmt_num(v)
                }
            }
            "rand" | "r" => {
                need!(2);
                let (x, y) = (a(0), a(1));
                let lx: Vec<char> = x.chars().collect();
                let ly: Vec<char> = y.chars().collect();
                if lx.len() == 1 && ly.len() == 1 && !lx[0].is_ascii_digit() && !ly[0].is_ascii_digit() {
                    let (lo, hi) = (lx[0].min(ly[0]) as u32, lx[0].max(ly[0]) as u32);
                    let v = lo + (rand_u64() % (hi - lo + 1) as u64) as u32;
                    char::from_u32(v).map(|c| c.to_string()).unwrap_or_default()
                } else {
                    let (p, q) = (int(x), int(y));
                    let (lo, hi) = (p.min(q), p.max(q));
                    let span = (hi - lo) as u64 + 1;
                    (lo + (rand_u64() % span) as i64).to_string()
                }
            }
            "base" => {
                need!(3);
                st::base(a(0), int(a(1)) as u32, int(a(2)) as u32, int(a(3)).max(0) as usize)
            }
            "bytes" => st::bytes(parse_num_prefix(a(0)), a(1), prop == Some("suf")),
            "ord" => ordinal(int(a(0))),

            // --- texte ---
            "len" => st::len(&args.join(",")).to_string(),
            "left" => {
                need!(2);
                st::left(a(0), int(a(1)))
            }
            "right" => {
                need!(2);
                st::right(a(0), int(a(1)))
            }
            "mid" => {
                need!(2);
                st::mid(a(0), int(a(1)), if n >= 3 { Some(int(a(2))) } else { None })
            }
            "pos" | "poscs" => {
                need!(2);
                let k = if n >= 3 { int(a(2)) } else { 1 };
                st::pos(a(0), a(1), k, name == "poscs").map(|x| x.to_string()).unwrap_or_default()
            }
            "count" | "countcs" => {
                need!(2);
                st::count(a(0), &args[1..], name == "countcs").to_string()
            }
            "replace" | "replacecs" => {
                need!(2);
                st::replace_seq(a(0), &args[1..], name == "replacecs")
            }
            "replacex" | "replacexcs" => {
                need!(2);
                st::replace_simul(a(0), &args[1..], name == "replacexcs")
            }
            "remove" | "removecs" => {
                need!(1);
                st::remove(a(0), &args[1..], name == "removecs")
            }
            "upper" => args.join(",").to_uppercase(),
            "lower" => args.join(",").to_lowercase(),
            "str" => st::repeat(a(0), int(a(1))),
            "strip" => alpirc_proto::format::strip(a(0)),
            "isupper" => tf(!a(0).is_empty() && !a(0).chars().any(|c| c.is_lowercase())),
            "islower" => tf(!a(0).is_empty() && !a(0).chars().any(|c| c.is_uppercase())),
            "qt" => {
                let t = a(0);
                if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 { t.to_string() } else { format!("\"{t}\"") }
            }
            "noqt" => {
                let t = a(0);
                let t = t.strip_prefix('"').unwrap_or(t);
                t.strip_suffix('"').unwrap_or(t).to_string()
            }
            "utfencode" | "utfdecode" => a(0).to_string(),
            "encode" | "decode" => {
                let opts = a(1);
                if !opts.is_empty() && !opts.contains('m') {
                    return Err(self.err(Msg::IdentOnlyMime { ident: name.to_string() }));
                }
                if name == "encode" {
                    base64::engine::general_purpose::STANDARD.encode(a(0).as_bytes())
                } else {
                    match base64::engine::general_purpose::STANDARD.decode(a(0).trim().as_bytes()) {
                        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
                        Err(_) => String::new(),
                    }
                }
            }
            "md5" | "sha1" | "sha256" | "sha512" | "crc" => {
                need!(1);
                // N : 0 = texte, 2 = fichier (pour $crc, 1 = fichier). Sans N : texte, sauf $crc (fichier s'il
                // existe dans le dossier des scripts).
                let mode = if n >= 2 { Some(int(a(1))) } else { None };
                let is_file = match (name, mode) {
                    ("crc", Some(m)) => m == 1,
                    ("crc", None) => self.sandbox_path(a(0)).map(|p| p.is_file()).unwrap_or(false),
                    (_, Some(m)) => m == 2,
                    (_, None) => false,
                };
                let data: Vec<u8> = if is_file {
                    match self.sandbox_path(a(0)).ok().and_then(|p| std::fs::read(p).ok()) {
                        Some(d) => d,
                        None => return Ok(Some(String::new())),
                    }
                } else {
                    a(0).as_bytes().to_vec()
                };
                match name {
                    "md5" => hex(&md5::Md5::digest(&data)),
                    "sha1" => hex(&sha1::Sha1::digest(&data)),
                    "sha256" => hex(&sha2::Sha256::digest(&data)),
                    "sha512" => hex(&sha2::Sha512::digest(&data)),
                    _ => format!("{:08X}", crc32fast::hash(&data)),
                }
            }
            "nopath" => {
                let t = a(0);
                t.rsplit(['/', '\\']).next().unwrap_or("").to_string()
            }
            "nofile" => {
                let t = a(0);
                match t.rfind(['/', '\\']) {
                    Some(i) => t[..=i].to_string(),
                    None => String::new(),
                }
            }
            "mkfn" => a(0).chars().map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect(),
            "longfn" | "shortfn" => a(0).to_string(),

            // --- jetons ---
            "gettok" => {
                need!(3);
                st::gettok(a(0), a(1), sep!(2))
            }
            "numtok" => {
                need!(2);
                st::numtok(a(0), sep!(1)).to_string()
            }
            "addtok" | "addtokcs" => {
                need!(3);
                st::addtok(a(0), a(1), sep!(2), name == "addtokcs")
            }
            "deltok" => {
                need!(3);
                st::deltok(a(0), a(1), sep!(2))
            }
            "instok" => {
                need!(4);
                st::instok(a(0), a(1), int(a(2)), sep!(3))
            }
            "puttok" => {
                need!(4);
                st::puttok(a(0), a(1), int(a(2)), sep!(3))
            }
            "reptok" | "reptokcs" => {
                need!(4);
                let (k, c) = if n >= 5 { (int(a(3)), sep!(4)) } else { (1, sep!(3)) };
                st::reptok(a(0), a(1), a(2), k, c, name == "reptokcs")
            }
            "remtok" | "remtokcs" => {
                need!(3);
                let (k, c) = if n >= 4 { (int(a(2)), sep!(3)) } else { (1, sep!(2)) };
                st::remtok(a(0), a(1), k, c, name == "remtokcs")
            }
            "findtok" | "findtokcs" => {
                need!(3);
                let (k, c) = if n >= 4 { (int(a(2)), sep!(3)) } else { (1, sep!(2)) };
                st::findtok(a(0), a(1), k, c, name == "findtokcs")
            }
            "istok" | "istokcs" => {
                need!(3);
                tf(st::istok(a(0), a(1), sep!(2), name == "istokcs"))
            }
            "matchtok" | "matchtokcs" | "wildtok" | "wildtokcs" => {
                need!(4);
                st::filtertok(a(0), a(1), int(a(2)), sep!(3), name.ends_with("cs"), name.starts_with("wild"))
            }
            "sorttok" | "sorttokcs" => {
                need!(2);
                st::sorttok(a(0), sep!(1), a(2), name == "sorttokcs")
            }

            // --- expressions régulières ---
            "regex" => {
                need!(2);
                let (rname, text, pat) = if n >= 3 { (a(0), a(1), a(2)) } else { ("", a(0), a(1)) };
                let re = regexid::compile(pat).map_err(|e| self.bad_regex("regex", e))?;
                let text = if re.strip { alpirc_proto::format::strip(text) } else { text.to_string() };
                let found = regexid::find_all(&re, &text);
                let mut ml = Vec::new();
                for f in &found {
                    push_regml(&mut ml, f);
                }
                self.rt.regml.insert(rname.to_ascii_lowercase(), ml);
                found.len().to_string()
            }
            "regml" => {
                let (rname, k) = if n >= 2 { (a(0), a(1)) } else { ("", a(0)) };
                let list = self.rt.regml.get(&rname.to_ascii_lowercase()).cloned().unwrap_or_default();
                let k = int(k);
                if k == 0 {
                    list.len().to_string()
                } else {
                    match list.get((k - 1).max(0) as usize) {
                        Some((t, p)) => {
                            if prop == Some("pos") {
                                p.to_string()
                            } else {
                                t.clone()
                            }
                        }
                        None => String::new(),
                    }
                }
            }

            // --- temps ---
            "ctime" => {
                if parens && n >= 1 {
                    let off = |t: i64| h.utc_offset(t);
                    timefmt::parse_ctime(a(0), off).map(|v| v.to_string()).unwrap_or_default()
                } else {
                    self.now_secs(h).to_string()
                }
            }
            "asctime" => {
                let now = self.now_secs(h);
                let (ts, fmt) = match n {
                    0 => (now, timefmt::DEFAULT_FORMAT.to_string()),
                    1 if parse_num(a(0)).is_some() => (int(a(0)), timefmt::DEFAULT_FORMAT.to_string()),
                    1 => (now, a(0).to_string()),
                    _ => (int(a(0)), a(1).to_string()),
                };
                timefmt::asctime(ts, h.utc_offset(ts), &fmt)
            }
            "time" | "date" | "day" | "fulldate" => {
                let now = self.now_secs(h);
                let def = match name {
                    "time" => "HH:nn:ss",
                    "date" => "dd/mm/yyyy",
                    "day" => "dddd",
                    _ => timefmt::DEFAULT_FORMAT,
                };
                let (ts, fmt) = match n {
                    0 => (now, def.to_string()),
                    1 if parse_num(a(0)).is_some() => (int(a(0)), def.to_string()),
                    1 => (now, a(0).to_string()),
                    _ => (int(a(0)), a(1).to_string()),
                };
                timefmt::asctime(ts, h.utc_offset(ts), &fmt)
            }
            "gmt" => {
                let now = self.now_secs(h);
                match n {
                    0 => now.to_string(),
                    1 if parse_num(a(0)).is_some() => timefmt::asctime(int(a(0)), 0, timefmt::DEFAULT_FORMAT),
                    1 => timefmt::asctime(now, 0, a(0)),
                    _ => timefmt::asctime(int(a(0)), 0, a(1)),
                }
            }
            "timezone" => (-(h.utc_offset(self.now_secs(h)) as i64)).to_string(),
            "ticks" => h.now_ms().to_string(),
            "duration" => {
                let t = a(0);
                match parse_num(t) {
                    Some(v) => timefmt::duration(v as i64, int(a(1)).max(1) as u32),
                    None => timefmt::parse_duration(t).map(|v| v.to_string()).unwrap_or_default(),
                }
            }

            // --- moteur ---
            "isid" => tf(self.frame().isid),
            "halted" => tf(self.rt.halted),
            "prop" => self.frame().prop.clone(),
            "result" => self.rt.result.clone(),
            "v1" | "ifmatch" => self.rt.v1.clone(),
            "v2" => self.rt.v2.clone(),
            "readn" => self.rt.readn.to_string(),
            "ltimer" => self.rt.ltimer.clone(),
            "menu" => self.rt.menu.clone(),
            "style" => {
                let k = int(a(0));
                self.rt.style.0 |= k & 1 != 0;
                self.rt.style.1 |= k & 2 != 0;
                String::new()
            }
            "submenu" => String::new(),
            "snicks" => ctx.selected.join(","),
            "cid" if !parens => match self.eff_ctx(ctx).conn {
                Some(c) => c.to_string(),
                None => return Ok(None),
            },
            "mask" => {
                need!(2);
                let addr = a(0);
                let (nick, rest) = addr.split_once('!').unwrap_or(("*", addr));
                let (user, host) = rest.split_once('@').unwrap_or(("*", rest));
                alpirc_proto::mask::address_mask(nick, user, host, int(a(1)).clamp(0, 19) as u8)
            }
            "scriptline" => self.frame().line.to_string(),
            "script" => self.script_ident(args, parens),
            "scriptdir" => self.scriptdir(),
            "mircdir" => {
                let mut s = self.root().display().to_string();
                if !s.ends_with('/') {
                    s.push('/');
                }
                s
            }
            "mircini" => self.root().join("mirc.ini").display().to_string(),
            "isalias" => {
                let nm = a(0).trim_start_matches('/').to_ascii_lowercase();
                match self.find_alias(&nm) {
                    Some((_, s)) if prop == Some("fname") => s.unwrap_or_default(),
                    Some(_) => TRUE.into(),
                    None => FALSE.into(),
                }
            }
            "group" => self.group_ident(a(0), prop),
            "dll" | "dllcall" | "com" | "comcall" | "comval" => {
                return Err(self.err(Msg::IdentSandboxed { ident: name.to_string() }));
            }

            // --- modules ---
            "hget" => return Ok(Some(self.hget_ident(h, args, prop))),
            "hfind" | "hmatch" => return Ok(Some(self.hfind_ident(args, prop))),
            "timer" => return Ok(Some(self.timer_ident(h, args, prop))),
            "read" => return self.read_ident(h, ctx, args).map(Some),
            "lines" => return self.lines_ident(a(0)).map(Some),
            "isfile" | "isdir" | "exists" | "file" => return self.file_ident(name, a(0), prop).map(Some),
            "readini" => return self.readini_ident(h, ctx, args).map(Some),
            "ini" => return self.ini_ident(args).map(Some),
            "ulevel" => self.frame().ulevel.clone(),
            "level" => {
                let addr = self.full_address_of(h, ctx, a(0));
                self.users.levels_of(&addr).join(",")
            }
            "ulist" => self.ulist_ident(args, prop),
            "dialog" => return self.dialog_ident(h, ctx, args, parens, prop).map(Some),
            "did" => {
                if !parens {
                    self.event_extra(ctx, "did").unwrap_or_default()
                } else {
                    self.did_ident(ctx, args, prop)
                }
            }
            "didwm" | "didreg" | "didtok" => self.did_misc_ident(ctx, name, args),
            _ => {
                if !parens && let Some(v) = self.event_ident(h, ctx, name) {
                    return Ok(Some(v));
                }
                return Ok(None);
            }
        };
        Ok(Some(v))
    }

    /// `$script`, `$script(N)`, `$script(0)`, `$script(nom)`.
    fn script_ident(&self, args: &[String], parens: bool) -> String {
        if !parens {
            return self.frame().script.clone().unwrap_or_default();
        }
        let a = args.first().map(|s| s.as_str()).unwrap_or("");
        match a.parse::<usize>() {
            Ok(0) => self.loaded.len().to_string(),
            Ok(k) => self.loaded.get(k - 1).map(|s| s.name.clone()).unwrap_or_default(),
            Err(_) => self
                .loaded
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(a) || wildmatch(a, &s.name, false))
                .map(|s| s.name.clone())
                .unwrap_or_default(),
        }
    }

    /// Dossier du script courant (avec `/` final), ou la racine.
    fn scriptdir(&self) -> String {
        let cur = self.frame().script.clone();
        let dir = cur
            .and_then(|n| self.loaded.iter().find(|s| s.name == n).and_then(|s| s.path.clone()))
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| self.root().to_path_buf());
        let mut s = dir.display().to_string();
        if !s.ends_with('/') {
            s.push('/');
        }
        s
    }

    /// `$var(%motif, N)` : nom de la N-ième variable qui correspond (`0` : nombre) ; `.value`, `.local`.
    fn var_ident(&self, pat: &str, k: i64, prop: Option<&str>) -> String {
        let pat = if pat.starts_with('%') { pat.to_string() } else { format!("%{pat}") };
        let mut found: Vec<(String, String, bool)> = Vec::new();
        if let Some(f) = self.rt.frames.last() {
            let mut l: Vec<_> = f.locals.values().filter(|(nm, _)| wildmatch(&pat, nm, false)).collect();
            l.sort();
            found.extend(l.into_iter().map(|(nm, v)| (nm.clone(), v.clone(), true)));
        }
        for (nm, v) in self.vars.list() {
            if wildmatch(&pat, &nm, false) && !found.iter().any(|x| x.0.eq_ignore_ascii_case(&nm)) {
                found.push((nm, v, false));
            }
        }
        if k == 0 {
            return found.len().to_string();
        }
        match found.get((k - 1).max(0) as usize) {
            Some((nm, v, local)) => match prop {
                Some("value") => v.clone(),
                Some("local") => tf(*local),
                _ => nm.clone(),
            },
            None => String::new(),
        }
    }

    /// `$group(#nom)` → `on`/`off` ; `$group(0)` : nombre ; `$group(N)` : nom ; `.status`, `.name`, `.fname`.
    fn group_ident(&self, a: &str, prop: Option<&str>) -> String {
        let all: Vec<(String, bool, String)> = self
            .scripts
            .iter()
            .flat_map(|s| s.p.groups.iter().map(move |(g, on)| (g.clone(), *on, s.name.clone())))
            .collect();
        let item = match a.parse::<usize>() {
            Ok(0) => return all.len().to_string(),
            Ok(k) => all.get(k - 1).cloned(),
            Err(_) => {
                let nm = if a.starts_with('#') { a.to_ascii_lowercase() } else { format!("#{}", a.to_ascii_lowercase()) };
                all.into_iter().find(|g| g.0 == nm)
            }
        };
        match (item, prop) {
            (Some((g, _, _)), Some("name")) => g,
            (Some((_, _, f)), Some("fname")) => f,
            (Some((g, on, _)), None) if a.parse::<usize>().is_ok() => {
                let _ = on;
                g
            }
            (Some((_, on, _)), _) => (if on { "on" } else { "off" }).into(),
            (None, _) => String::new(),
        }
    }
}

/// Ajoute une occurrence à `$regml` : ses groupes, ou l'occurrence entière s'il n'y en a pas.
pub fn push_regml(ml: &mut Vec<(String, usize)>, f: &regexid::Found) {
    if f.groups.is_empty() {
        ml.push(f.whole.clone());
    } else {
        ml.extend(f.groups.iter().cloned());
    }
}
