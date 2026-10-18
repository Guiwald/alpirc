//! Lecture des fichiers de scripts : remotes (`on`, `ctcp`, `raw`, `alias`, `menu`, `dialog`, groupes), fichiers
//! d'alias, fichiers de popups, et format `.ini` de mIRC (`[script]`, `[aliases]`, `[cpopup]`… avec `n0=…`).

use std::sync::Arc;

use crate::ast::{Block, parse_body};
use crate::{EventKind, ParseProblem, ScriptError, ScriptKind};

/// Un alias (`alias [-l] nom { … }`).
#[derive(Debug)]
pub struct Alias {
    /// Nom en minuscules.
    pub name: String,
    pub local: bool,
    pub body: Arc<Block>,
    pub group: Option<String>,
    pub line: usize,
}

/// Préfixes d'un événement (`on !@&$^+…:`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Prefixes {
    /// `!` : pas pour nos propres actions.
    pub not_me: bool,
    /// `@` : seulement si nous sommes opérateur du canal.
    pub op: bool,
    /// `&` : seulement si aucun script précédent n'a fait `/haltdef`.
    pub amp: bool,
    /// `$` : le texte à comparer est une expression régulière.
    pub regex: bool,
    /// `^` : avant l'affichage (sans effet particulier ici : le client tire l'événement avant d'afficher).
    pub pre: bool,
    /// `+` : niveau exact.
    pub exact: bool,
    /// `me:` : seulement pour nos propres actions.
    pub me_only: bool,
}

/// Un gestionnaire d'événement (`on`, `ctcp`, `raw`).
#[derive(Debug)]
pub struct Handler {
    /// `None` pour un événement que le moteur ne connaît pas (jamais déclenché).
    pub kind: Option<EventKind>,
    pub level: String,
    pub pfx: Prefixes,
    /// Champs propres à l'événement, dans l'ordre de mIRC (texte, cible, nom…).
    pub fields: Vec<String>,
    pub body: Arc<Block>,
    pub group: Option<String>,
    pub line: usize,
}

/// Une ligne de popup (`..Libellé:commandes`).
#[derive(Debug)]
pub struct PopupLine {
    pub depth: usize,
    /// Libellé brut (évalué à chaque ouverture du menu).
    pub label: String,
    pub body: Option<Arc<Block>>,
    pub line: usize,
}

/// Un menu (`menu channel,status { … }`).
#[derive(Debug)]
pub struct Menu {
    /// Cibles en minuscules : `status`, `channel`, `query`, `nicklist`, `menubar`, `@fenêtre`, `*`.
    pub targets: Vec<String>,
    pub items: Arc<Vec<PopupLine>>,
    pub group: Option<String>,
}

/// Une table de dialogue (`dialog [-l] nom { … }`), gardée en lignes brutes.
#[derive(Debug)]
pub struct DialogTable {
    pub name: String,
    pub local: bool,
    pub lines: Vec<String>,
}

/// Contenu lu d'un fichier.
#[derive(Debug, Default)]
pub struct Parsed {
    pub aliases: Vec<Arc<Alias>>,
    pub handlers: Vec<Arc<Handler>>,
    pub menus: Vec<Menu>,
    pub dialogs: Vec<Arc<DialogTable>>,
    /// Groupes (nom avec `#`, en minuscules ; actif au chargement ?).
    pub groups: Vec<(String, bool)>,
}

/// Sections d'un fichier `.ini` de mIRC, déjà ramenées à du texte de script.
#[derive(Debug, Default)]
pub struct IniSections {
    pub script: Option<String>,
    pub aliases: Option<String>,
    /// (cible, texte) : `[cpopup]` → `channel`…
    pub popups: Vec<(String, String)>,
    pub users: Option<String>,
    pub variables: Option<String>,
}

/// Prépare les lignes : retire les commentaires `;` et `/* … */`, recolle les lignes coupées par `$&`.
/// Le nombre de lignes est conservé (les lignes retirées deviennent vides) pour garder les numéros.
pub fn preprocess(text: &str) -> Vec<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines: Vec<String> = text.split('\n').map(|l| l.trim_end().to_string()).collect();
    let mut in_comment = false;
    for l in lines.iter_mut() {
        let t = l.trim();
        if in_comment {
            if t.ends_with("*/") || t.starts_with("*/") {
                in_comment = false;
            }
            l.clear();
        } else if t.starts_with("/*") {
            in_comment = !(t.len() > 2 && t.ends_with("*/"));
            l.clear();
        } else if t.starts_with(';') {
            l.clear();
        }
    }
    // `$&` en fin de ligne : la suivante est recollée.
    let mut i = 0;
    while i < lines.len() {
        while lines[i].trim_end().ends_with(" $&") || lines[i].trim() == "$&" {
            let t = lines[i].trim_end();
            let base = t[..t.len() - 2].to_string();
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if j >= lines.len() {
                lines[i] = base.trim_end().to_string();
                break;
            }
            let next = lines[j].trim_start().to_string();
            lines[j].clear();
            lines[i] = format!("{base}{next}");
        }
        i += 1;
    }
    lines
}

/// Vrai si le texte est au format `.ini` de mIRC.
pub fn is_ini(text: &str) -> bool {
    let mut section = false;
    let mut nline = false;
    for l in text.lines() {
        let t = l.trim();
        if t.starts_with('[') && t.ends_with(']') {
            let s = t[1..t.len() - 1].to_ascii_lowercase();
            if matches!(s.as_str(), "script" | "aliases" | "users" | "variables") || s.ends_with("popup") {
                section = true;
            }
        } else if is_n_line(t) {
            nline = true;
        }
    }
    section && nline
}

fn is_n_line(t: &str) -> bool {
    let b = t.as_bytes();
    if b.len() < 3 || b[0] != b'n' {
        return false;
    }
    let mut i = 1;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    i > 1 && i < b.len() && b[i] == b'='
}

/// Retire le préfixe `nN=` d'une ligne d'`.ini`.
pub fn strip_n(t: &str) -> &str {
    if is_n_line(t) { &t[t.find('=').unwrap() + 1..] } else { t }
}

/// Section de popups `.ini` → cible.
fn popup_section_target(s: &str) -> Option<&'static str> {
    Some(match s {
        "mpopup" => "menubar",
        "bpopup" | "spopup" => "status",
        "cpopup" => "channel",
        "qpopup" => "query",
        "lpopup" | "npopup" => "nicklist",
        _ => return None,
    })
}

/// Découpe un `.ini` de mIRC en sections.
pub fn ini_sections(text: &str) -> IniSections {
    let mut out = IniSections::default();
    let mut cur: Option<String> = None;
    let mut buf: Vec<String> = Vec::new();
    let flush = |out: &mut IniSections, cur: &Option<String>, buf: &mut Vec<String>| {
        if let Some(s) = cur {
            let text = buf.join("\n");
            match s.as_str() {
                "script" => out.script = Some(text),
                "aliases" => out.aliases = Some(text),
                "users" => out.users = Some(text),
                "variables" => out.variables = Some(text),
                other => {
                    if let Some(t) = popup_section_target(other) {
                        out.popups.push((t.to_string(), text));
                    }
                }
            }
        }
        buf.clear();
    };
    for l in text.lines() {
        let t = l.trim_end_matches('\r');
        let tt = t.trim();
        if tt.starts_with('[') && tt.ends_with(']') {
            flush(&mut out, &cur, &mut buf);
            cur = Some(tt[1..tt.len() - 1].to_ascii_lowercase());
        } else if cur.is_some() && !tt.is_empty() {
            buf.push(strip_n(tt.trim_start()).to_string());
        }
    }
    flush(&mut out, &cur, &mut buf);
    out
}

fn is_ws(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

/// `{` isolée (début de bloc) en position `j`.
pub fn standalone_open(b: &[u8], j: usize) -> bool {
    b[j] == b'{'
        && (j == 0 || is_ws(b[j - 1]) || b[j - 1] == b')' || b[j - 1] == b':')
        && (j + 1 >= b.len() || is_ws(b[j + 1]))
}

/// `}` isolée (fin de bloc) en position `j`.
pub fn standalone_close(b: &[u8], j: usize) -> bool {
    b[j] == b'}' && (j == 0 || is_ws(b[j - 1])) && (j + 1 >= b.len() || is_ws(b[j + 1]))
}

/// Évènements connus : nom → sorte et nombre de champs entre `EVENT:` et les commandes.
pub fn event_kind(name: &str) -> Option<(EventKind, usize)> {
    use EventKind::*;
    Some(match name.to_ascii_uppercase().as_str() {
        "TEXT" => (Text, 2),
        "ACTION" => (Action, 2),
        "NOTICE" => (Notice, 2),
        "JOIN" => (Join, 1),
        "PART" => (Part, 1),
        "KICK" => (Kick, 1),
        "TOPIC" => (Topic, 1),
        "INVITE" => (Invite, 1),
        "MODE" => (Mode, 1),
        "RAWMODE" => (RawMode, 1),
        "OP" => (Op, 1),
        "DEOP" => (Deop, 1),
        "HELP" => (Help, 1),
        "DEHELP" => (Dehelp, 1),
        "VOICE" => (Voice, 1),
        "DEVOICE" => (Devoice, 1),
        "BAN" => (Ban, 1),
        "UNBAN" => (Unban, 1),
        "QUIT" => (Quit, 0),
        "NICK" => (Nick, 0),
        "CONNECT" => (Connect, 0),
        "DISCONNECT" => (Disconnect, 0),
        "START" => (Start, 0),
        "EXIT" => (Exit, 0),
        "LOAD" => (Load, 0),
        "UNLOAD" => (Unload, 0),
        "NOTIFY" => (Notify, 0),
        "UNOTIFY" => (Unotify, 0),
        "PING" => (Ping, 0),
        "PONG" => (Pong, 0),
        "USERMODE" => (UserMode, 0),
        "LOGON" => (Logon, 1),
        "INPUT" => (Input, 1),
        "CLOSE" => (Close, 1),
        "ACTIVE" => (Active, 1),
        "OPEN" => (Open, 2),
        "SNOTICE" => (Snotice, 1),
        "WALLOPS" => (Wallops, 1),
        "ERROR" => (Error, 1),
        "CHAT" => (Chat, 1),
        "FILESENT" => (FileSent, 1),
        "FILERCVD" => (FileRcvd, 1),
        "SENDFAIL" => (SendFail, 1),
        "GETFAIL" => (GetFail, 1),
        "SIGNAL" => (Signal, 1),
        "DIALOG" => (Dialog, 3),
        "KEYDOWN" => (KeyDown, 2),
        "KEYUP" => (KeyUp, 2),
        "CTCPREPLY" => (CtcpReply, 1),
        _ => return None,
    })
}

/// Nom mIRC (minuscules) d'une sorte d'événement, pour `$event`.
pub fn event_name(k: EventKind) -> &'static str {
    use EventKind::*;
    match k {
        Start => "start",
        Exit => "exit",
        Load => "load",
        Unload => "unload",
        Connect => "connect",
        Disconnect => "disconnect",
        Logon => "logon",
        Text => "text",
        Action => "action",
        Notice => "notice",
        Join => "join",
        Part => "part",
        Quit => "quit",
        Kick => "kick",
        Nick => "nick",
        Topic => "topic",
        Invite => "invite",
        Mode => "mode",
        RawMode => "rawmode",
        Op => "op",
        Deop => "deop",
        Help => "help",
        Dehelp => "dehelp",
        Voice => "voice",
        Devoice => "devoice",
        Ban => "ban",
        Unban => "unban",
        UserMode => "usermode",
        Ctcp => "ctcp",
        CtcpReply => "ctcpreply",
        Raw => "raw",
        Input => "input",
        Open => "open",
        Close => "close",
        Active => "active",
        Notify => "notify",
        Unotify => "unotify",
        Snotice => "snotice",
        Wallops => "wallops",
        Error => "error",
        Ping => "ping",
        Pong => "pong",
        Chat => "chat",
        FileSent => "filesent",
        FileRcvd => "filercvd",
        SendFail => "sendfail",
        GetFail => "getfail",
        Signal => "signal",
        Dialog => "dialog",
        KeyDown => "keydown",
        KeyUp => "keyup",
    }
}

/// Prend un champ jusqu'au prochain `:` ; une expression régulière `/…/flags` peut contenir des `:`.
fn take_field(rest: &str, regex: bool) -> (String, &str) {
    if regex && rest.starts_with('/') {
        let b = rest.as_bytes();
        let mut i = 1;
        while i < b.len() {
            if b[i] == b'\\' {
                i += 2;
                continue;
            }
            if b[i] == b'/' {
                break;
            }
            i += 1;
        }
        let mut j = (i + 1).min(b.len());
        while j < b.len() && b[j].is_ascii_alphabetic() {
            j += 1;
        }
        if j < b.len() && b[j] == b':' {
            return (rest[..j].to_string(), &rest[j + 1..]);
        }
    }
    match rest.split_once(':') {
        Some((a, b)) => (a.to_string(), b),
        None => (rest.to_string(), ""),
    }
}

/// Lecteur de lignes, partagé par les remotes, les fichiers d'alias et de popups.
struct Reader<'a> {
    file: &'a str,
    lines: Vec<String>,
    i: usize,
    /// Numéro (dans le fichier) de la ligne qui précède la première de `lines`.
    base: usize,
}

impl<'a> Reader<'a> {
    fn err(&self, line: usize, problem: ParseProblem) -> ScriptError {
        ScriptError::Parse { file: self.file.to_string(), line: self.base + line, problem }
    }

    /// Lit un bloc dont le `{` vient d'être trouvé en ligne `li` (indice), colonne `col` (après le `{`).
    /// Rend le texte du bloc ; `self.i` passe à la ligne qui suit le `}`.
    fn collect(&mut self, li: usize, col: usize) -> Result<String, ScriptError> {
        let mut depth = 1;
        let mut out = String::new();
        let mut l = li;
        let mut c = col;
        loop {
            if l >= self.lines.len() {
                return Err(self.err(li + 1, ParseProblem::MissingClose));
            }
            let s = self.lines[l].clone();
            let b = s.as_bytes();
            let mut j = c;
            while j < b.len() {
                if standalone_open(b, j) {
                    depth += 1;
                } else if standalone_close(b, j) {
                    depth -= 1;
                    if depth == 0 {
                        out.push_str(&s[c..j]);
                        let rest = s[j + 1..].trim().to_string();
                        if rest.is_empty() {
                            self.i = l + 1;
                        } else {
                            // Texte après le `}` : relu comme une ligne à part.
                            self.lines[l] = rest;
                            self.i = l;
                        }
                        return Ok(out);
                    }
                }
                j += 1;
            }
            out.push_str(&s[c..]);
            out.push('\n');
            l += 1;
            c = 0;
        }
    }

    /// Commandes d'une définition dont l'en-tête est en ligne `li` : `{ … }` (même ligne ou ligne suivante)
    /// ou une seule ligne. Rend le bloc lu et avance `self.i`.
    fn commands(&mut self, li: usize, cmd: &str) -> Result<Block, ScriptError> {
        let t = cmd.trim_start();
        if t.starts_with('{') && (t.len() == 1 || is_ws(t.as_bytes()[1])) {
            // Position du `{` dans la ligne d'origine.
            let line = &self.lines[li];
            let off = line.len() - cmd.len() + (cmd.len() - t.len());
            let text = self.collect(li, off + 1)?;
            return Ok(parse_body(&text, self.base + li + 1));
        }
        self.i = li + 1;
        if t.trim().is_empty() {
            // Accolade sur la ligne suivante.
            let mut j = li + 1;
            while j < self.lines.len() && self.lines[j].trim().is_empty() {
                j += 1;
            }
            if j < self.lines.len() && self.lines[j].trim() == "{" {
                let col = self.lines[j].find('{').unwrap() + 1;
                let text = self.collect(j, col)?;
                return Ok(parse_body(&text, self.base + j + 1));
            }
            return Ok(Vec::new());
        }
        Ok(parse_body(t, self.base + li + 1))
    }

    /// Texte brut d'un bloc `{ … }` (menus, dialogues) ; `rest` est la fin de l'en-tête (suffixe de la ligne).
    /// Rend le texte et le numéro (dans le fichier) de la ligne du `{`.
    fn raw_block(&mut self, li: usize, rest: &str) -> Result<(String, usize), ScriptError> {
        let line = self.lines[li].clone();
        let off = line.len() - rest.len().min(line.len());
        let b = line.as_bytes();
        if let Some(j) = (off..b.len()).find(|&j| standalone_open(b, j)) {
            let text = self.collect(li, j + 1)?;
            return Ok((text, self.base + li + 1));
        }
        if line.ends_with('{') {
            // `menu channel{` : toléré.
            let text = self.collect(li, line.len())?;
            return Ok((text, self.base + li + 1));
        }
        let mut j = li + 1;
        while j < self.lines.len() && self.lines[j].trim().is_empty() {
            j += 1;
        }
        if j < self.lines.len() && self.lines[j].trim_start().starts_with('{') {
            let col = self.lines[j].find('{').unwrap() + 1;
            let text = self.collect(j, col)?;
            return Ok((text, self.base + j + 1));
        }
        Err(self.err(li + 1, ParseProblem::MissingOpen))
    }
}

/// Lit les lignes d'un menu (`Libellé:commandes`, `.Sous:…`, `-`, blocs `{ }`). `first_line` : numéro dans le
/// fichier de la première ligne du texte.
pub fn parse_popup_lines(text: &str, first_line: usize, file: &str) -> Result<Vec<PopupLine>, ScriptError> {
    let mut r = Reader { file, lines: preprocess(text), i: 0, base: first_line.saturating_sub(1) };
    let mut out = Vec::new();
    while r.i < r.lines.len() {
        let li = r.i;
        let line = r.lines[li].trim().to_string();
        r.i += 1;
        if line.is_empty() {
            continue;
        }
        let depth = line.chars().take_while(|&c| c == '.').count();
        let body_txt = &line[depth..];
        // Séparation libellé / commandes au premier `:` hors parenthèses.
        let mut d = 0i32;
        let mut split = None;
        for (i, c) in body_txt.char_indices() {
            match c {
                '(' => d += 1,
                ')' => d -= 1,
                ':' if d <= 0 => {
                    split = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let (label, cmd) = match split {
            Some(i) => (body_txt[..i].trim().to_string(), Some(body_txt[i + 1..].to_string())),
            None => (body_txt.trim().to_string(), None),
        };
        let body = match cmd {
            Some(_) => {
                // Position des commandes dans la ligne d'origine pour retrouver le `{`.
                let orig = r.lines[li].clone();
                let off = orig.len() - orig.trim_start().len() + depth + split.unwrap() + 1;
                let tail = orig[off.min(orig.len())..].to_string();
                let b = r.commands(li, &tail)?;
                Some(Arc::new(b))
            }
            None => None,
        };
        out.push(PopupLine { depth, label, body, line: r.base + li + 1 });
    }
    Ok(out)
}

/// Lit un fichier remote.
pub fn parse_remote(text: &str, file: &str) -> Result<Parsed, ScriptError> {
    let mut r = Reader { file, lines: preprocess(text), i: 0, base: 0 };
    let mut p = Parsed::default();
    let mut group: Option<String> = None;
    while r.i < r.lines.len() {
        let li = r.i;
        let line = r.lines[li].trim().to_string();
        if line.is_empty() {
            r.i += 1;
            continue;
        }
        if line.starts_with('#') {
            let w: Vec<&str> = line.split_whitespace().collect();
            if w.len() == 2 {
                match w[1].to_ascii_lowercase().as_str() {
                    "end" => group = None,
                    s @ ("on" | "off") => {
                        let name = w[0].to_ascii_lowercase();
                        if !p.groups.iter().any(|(n, _)| *n == name) {
                            p.groups.push((name.clone(), s == "on"));
                        }
                        group = Some(name);
                    }
                    _ => {}
                }
            }
            r.i += 1;
            continue;
        }
        let (kw, rest) = match line.split_once([' ', '\t']) {
            Some((a, b)) => (a.to_ascii_lowercase(), b.trim_start().to_string()),
            None => (line.to_ascii_lowercase(), String::new()),
        };
        // `rest` vu comme un suffixe de la ligne d'origine (pour retrouver les colonnes).
        match kw.as_str() {
            "on" | "ctcp" | "raw" => {
                let h = parse_event(&mut r, li, &kw, &rest, group.clone())?;
                if let Some(h) = h {
                    p.handlers.push(Arc::new(h));
                }
            }
            "alias" => {
                let a = parse_alias(&mut r, li, &rest, true, group.clone())?;
                if let Some(a) = a {
                    p.aliases.push(Arc::new(a));
                }
            }
            "menu" => {
                let (targets, tail) = match rest.split_once([' ', '\t']) {
                    Some((a, b)) => (a.to_string(), b.to_string()),
                    None => (rest.clone(), String::new()),
                };
                let tail = if targets.ends_with('{') && tail.is_empty() {
                    // `menu channel{` : rare, toléré.
                    "{".to_string()
                } else {
                    tail
                };
                let targets: Vec<String> = targets
                    .trim_end_matches('{')
                    .split(',')
                    .map(|t| t.trim().to_ascii_lowercase())
                    .filter(|t| !t.is_empty())
                    .collect();
                let (text, first) = r.raw_block(li, &tail)?;
                let items = parse_popup_lines(&text, first, file)?;
                p.menus.push(Menu { targets, items: Arc::new(items), group: group.clone() });
            }
            "dialog" => {
                let mut words = rest.split_whitespace();
                let mut local = false;
                let mut name = words.next().unwrap_or("").to_string();
                if name.eq_ignore_ascii_case("-l") {
                    local = true;
                    name = words.next().unwrap_or("").to_string();
                }
                let tail: String = words.collect::<Vec<_>>().join(" ");
                let (text, first) = r.raw_block(li, &tail)?;
                let lines = preprocess(&text).into_iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty());
                let _ = first;
                p.dialogs.push(Arc::new(DialogTable { name: name.to_ascii_lowercase(), local, lines: lines.collect() }));
            }
            _ => {
                // Ligne inconnue : ignorée, avec son bloc éventuel.
                let b = line.as_bytes();
                if !b.is_empty() && standalone_open(b, b.len() - 1) {
                    let col = r.lines[li].rfind('{').unwrap() + 1;
                    r.collect(li, col)?;
                } else {
                    r.i += 1;
                }
                log::debug!("{file}:{}: ligne ignorée : {line}", li + 1);
            }
        }
    }
    Ok(p)
}

/// `on …`, `ctcp …`, `raw …`.
fn parse_event(
    r: &mut Reader,
    li: usize,
    kw: &str,
    rest: &str,
    group: Option<String>,
) -> Result<Option<Handler>, ScriptError> {
    let orig = r.lines[li].clone();
    let mut pfx = Prefixes::default();
    let (kind, level, fields, cmd): (Option<EventKind>, String, Vec<String>, String);
    let mut s = rest;
    if kw == "raw" {
        while let Some(c) = s.chars().next() {
            match c {
                '$' => pfx.regex = true,
                '&' => pfx.amp = true,
                '^' => pfx.pre = true,
                _ => break,
            }
            s = &s[1..];
        }
        let (what, r1) = take_field(s, false);
        let (mt, r2) = take_field(r1, pfx.regex);
        kind = Some(EventKind::Raw);
        level = "*".into();
        fields = vec![what, mt];
        cmd = r2.to_string();
    } else {
        while let Some(c) = s.chars().next() {
            match c {
                '!' => pfx.not_me = true,
                '@' => pfx.op = true,
                '&' => pfx.amp = true,
                '$' => pfx.regex = true,
                '^' => pfx.pre = true,
                '+' => pfx.exact = true,
                '=' => {}
                _ => break,
            }
            s = &s[1..];
        }
        let (mut lv, mut r1) = take_field(s, false);
        if lv.eq_ignore_ascii_case("me") {
            pfx.me_only = true;
            let (l2, r2) = take_field(r1, false);
            lv = l2;
            r1 = r2;
        }
        level = lv.trim().to_string();
        if kw == "ctcp" {
            let (mt, r2) = take_field(r1, pfx.regex);
            let (tg, r3) = take_field(r2, false);
            kind = Some(EventKind::Ctcp);
            fields = vec![mt, tg];
            cmd = r3.to_string();
        } else {
            let (ev, r2) = take_field(r1, false);
            match event_kind(ev.trim()) {
                Some((k, n)) => {
                    let mut f = Vec::new();
                    let mut rr = r2;
                    for i in 0..n {
                        // Le texte à comparer est le premier champ des événements de texte.
                        let is_match = i == 0 && pfx.regex;
                        let (x, nx) = take_field(rr, is_match);
                        f.push(x);
                        rr = nx;
                    }
                    kind = Some(k);
                    fields = f;
                    cmd = rr.to_string();
                }
                None => {
                    // Événement inconnu : les commandes commencent au premier champ `{`, ou au dernier.
                    let parts: Vec<&str> = r2.split(':').collect();
                    let idx = parts.iter().position(|p| p.trim_start().starts_with('{')).unwrap_or(parts.len() - 1);
                    kind = None;
                    fields = parts[..idx].iter().map(|x| x.to_string()).collect();
                    cmd = parts[idx..].join(":");
                }
            }
        }
    }
    // `cmd` est un suffixe de la ligne d'origine.
    let cmd_in_orig = &orig[orig.len() - cmd.len().min(orig.len())..];
    let body = r.commands(li, cmd_in_orig)?;
    Ok(Some(Handler { kind, level, pfx, fields, body: Arc::new(body), group, line: r.base + li + 1 }))
}

/// `alias [-l] nom commandes` (le mot `alias` déjà retiré si `with_kw`).
fn parse_alias(
    r: &mut Reader,
    li: usize,
    rest: &str,
    _with_kw: bool,
    group: Option<String>,
) -> Result<Option<Alias>, ScriptError> {
    let orig = r.lines[li].clone();
    let mut s = rest.trim_start();
    let mut local = false;
    if s.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("-l")) && s[2..].starts_with([' ', '\t']) {
        local = true;
        s = s[2..].trim_start();
    }
    let (name, cmd) = match s.split_once([' ', '\t']) {
        Some((a, b)) => (a, b),
        None => (s, ""),
    };
    let name = name.trim_start_matches('/').to_ascii_lowercase();
    if name.is_empty() {
        r.i = li + 1;
        return Ok(None);
    }
    let cmd_in_orig = &orig[orig.len() - cmd.len().min(orig.len())..];
    let body = r.commands(li, cmd_in_orig)?;
    Ok(Some(Alias { name, local, body: Arc::new(body), group, line: r.base + li + 1 }))
}

/// Lit un fichier d'alias (`nom commandes` par ligne, `nom { … }`, ou `alias nom …`).
pub fn parse_aliases(text: &str, file: &str) -> Result<Parsed, ScriptError> {
    let mut r = Reader { file, lines: preprocess(text), i: 0, base: 0 };
    let mut p = Parsed::default();
    while r.i < r.lines.len() {
        let li = r.i;
        let line = r.lines[li].trim().to_string();
        if line.is_empty() {
            r.i += 1;
            continue;
        }
        let rest = match line.split_once([' ', '\t']) {
            Some((kw, rest)) if kw.eq_ignore_ascii_case("alias") => rest.to_string(),
            _ => line.clone(),
        };
        if let Some(a) = parse_alias(&mut r, li, &rest, false, None)? {
            p.aliases.push(Arc::new(a));
        }
    }
    Ok(p)
}

/// Lit un fichier de popups : sections `[status]`, `[channel]`… (ou `[cpopup]`…), blocs `menu x { }`, ou un
/// seul menu sans section (alors pour `default_target`).
pub fn parse_popups(text: &str, file: &str, default_target: &str) -> Result<Parsed, ScriptError> {
    let mut p = Parsed::default();
    let lines: Vec<&str> = text.split('\n').collect();
    let mut cur = default_target.to_string();
    let mut buf: Vec<String> = Vec::new();
    let mut first = 1usize;
    let mut has_menu_kw = false;
    let flush = |p: &mut Parsed, cur: &str, buf: &mut Vec<String>, first: usize| -> Result<(), ScriptError> {
        if buf.iter().any(|l| !l.trim().is_empty()) {
            let items = parse_popup_lines(&buf.join("\n"), first, file)?;
            p.menus.push(Menu { targets: vec![cur.to_string()], items: Arc::new(items), group: None });
        }
        buf.clear();
        Ok(())
    };
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim_end_matches('\r').trim();
        if t.starts_with('[') && t.ends_with(']') && !t.contains(':') {
            flush(&mut p, &cur, &mut buf, first)?;
            let s = t[1..t.len() - 1].to_ascii_lowercase();
            cur = popup_section_target(&s).map(|x| x.to_string()).unwrap_or(s);
            first = i + 2;
        } else {
            if t.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("menu ")) {
                has_menu_kw = true;
            }
            buf.push(strip_n(l.trim_end_matches('\r')).to_string());
        }
    }
    if has_menu_kw && p.menus.is_empty() {
        // Fichier de popups écrit comme un remote (`menu channel { … }`).
        return parse_remote(text, file);
    }
    flush(&mut p, &cur, &mut buf, first)?;
    Ok(p)
}

/// Lit un texte selon sa sorte (le format `.ini` est reconnu tout seul).
pub fn parse_any(text: &str, kind: ScriptKind, file: &str) -> Result<Parsed, ScriptError> {
    if is_ini(text) {
        let s = ini_sections(text);
        let mut p = Parsed::default();
        if let Some(t) = &s.script {
            merge(&mut p, parse_remote(t, file)?);
        }
        if let Some(t) = &s.aliases {
            merge(&mut p, parse_aliases(t, file)?);
        }
        for (target, t) in &s.popups {
            let items = parse_popup_lines(t, 1, file)?;
            p.menus.push(Menu { targets: vec![target.clone()], items: Arc::new(items), group: None });
        }
        return Ok(p);
    }
    match kind {
        ScriptKind::Remote => parse_remote(text, file),
        ScriptKind::Aliases => parse_aliases(text, file),
        ScriptKind::Popups => {
            let lower = file.to_ascii_lowercase();
            let def = ["channel", "query", "nicklist", "menubar", "status"]
                .into_iter()
                .find(|k| lower.contains(k))
                .unwrap_or("status");
            parse_popups(text, file, def)
        }
    }
}

fn merge(a: &mut Parsed, b: Parsed) {
    a.aliases.extend(b.aliases);
    a.handlers.extend(b.handlers);
    a.menus.extend(b.menus);
    a.dialogs.extend(b.dialogs);
    for g in b.groups {
        if !a.groups.iter().any(|(n, _)| *n == g.0) {
            a.groups.push(g);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Stmt;

    #[test]
    fn remote_basics() {
        let src = "; commentaire\non *:TEXT:*hi*:#:{\n  msg # hello $nick\n}\non *:JOIN:#test:echo joined\n\
                   alias -l foo { return bar }\nalias bar echo $1-\n#grp off\non 1:NICK:{ echo nick }\n#grp end\n\
                   raw 001:*:echo welcome\nctcp *:VERSION:*:notice $nick VERSION x\n/*\nbloc { commenté\n*/\n\
                   on *:SIGNAL:test:{ echo sig }";
        let p = parse_remote(src, "t.mrc").unwrap();
        assert_eq!(p.handlers.len(), 6);
        assert_eq!(p.handlers[0].kind, Some(EventKind::Text));
        assert_eq!(p.handlers[0].fields, vec!["*hi*", "#"]);
        assert_eq!(p.handlers[0].line, 2);
        assert!(matches!(&p.handlers[0].body[0], Stmt::Cmd { line: 3, .. }));
        assert_eq!(p.handlers[1].fields, vec!["#test"]);
        assert_eq!(p.handlers[2].group.as_deref(), Some("#grp"));
        assert_eq!(p.handlers[3].kind, Some(EventKind::Raw));
        assert_eq!(p.handlers[3].fields, vec!["001", "*"]);
        assert_eq!(p.handlers[4].kind, Some(EventKind::Ctcp));
        assert_eq!(p.aliases.len(), 2);
        assert!(p.aliases[0].local);
        assert_eq!(p.groups, vec![("#grp".to_string(), false)]);
    }

    #[test]
    fn prefixes_and_regex() {
        let p = parse_remote("on $&!@*:TEXT:/^!(\\w+):x/i:#:{ echo $regml(1) }", "t").unwrap();
        let h = &p.handlers[0];
        assert!(h.pfx.regex && h.pfx.amp && h.pfx.not_me && h.pfx.op);
        assert_eq!(h.fields, vec!["/^!(\\w+):x/i", "#"]);
        let p = parse_remote("on me:*:JOIN:#:{ echo me }", "t").unwrap();
        assert!(p.handlers[0].pfx.me_only);
        assert_eq!(p.handlers[0].level, "*");
    }

    #[test]
    fn menus_and_dialogs() {
        let src = "menu channel,nicklist {\n  Op:mode # +o $$1\n  Sub\n  .Item:{\n    echo a\n    echo b\n  }\n  -\n}\n\
                   dialog -l test {\n  title \"T\"\n  size -1 -1 100 50\n}";
        let p = parse_remote(src, "t").unwrap();
        assert_eq!(p.menus.len(), 1);
        assert_eq!(p.menus[0].targets, vec!["channel", "nicklist"]);
        let items = &p.menus[0].items;
        assert_eq!(items.len(), 4);
        assert_eq!(items[2].depth, 1);
        assert_eq!(items[2].body.as_ref().unwrap().len(), 2);
        assert_eq!(items[3].label, "-");
        assert_eq!(p.dialogs[0].lines, vec!["title \"T\"", "size -1 -1 100 50"]);
        assert!(p.dialogs[0].local);
    }

    #[test]
    fn ini_format() {
        let src = "[script]\nn0=on *:JOIN:#:{\nn1=  echo joined\nn2=}\n[aliases]\nn0=/hi echo hi";
        assert!(is_ini(src));
        let p = parse_any(src, ScriptKind::Remote, "remote.ini").unwrap();
        assert_eq!(p.handlers.len(), 1);
        assert_eq!(p.aliases[0].name, "hi");
    }

    #[test]
    fn alias_file() {
        let p = parse_aliases("/op /mode # +ooo $$1 $2 $3\nj join #$$1\ntest {\n echo a\n}", "a").unwrap();
        assert_eq!(p.aliases.len(), 3);
        assert_eq!(p.aliases[2].name, "test");
    }

    #[test]
    fn continuation() {
        let l = preprocess("echo a $&\n  b\nnext");
        assert_eq!(l, vec!["echo a b", "", "next"]);
    }
}
