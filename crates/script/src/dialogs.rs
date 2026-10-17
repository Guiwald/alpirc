//! Dialogues personnalisés : tables `dialog [-l] nom { … }`, ouverture `/dialog -m nom table` (ou
//! `$dialog(nom, table)`), commandes `/did`, identifiants `$dialog`, `$did`, `$didwm`, `$didreg`, `$didtok`, et
//! événements `on *:DIALOG:nom:événement:id:` venus du client par [`Engine::dialog_event`].
//!
//! Le moteur tient l'état (textes, lignes, sélections, cases) ; le client dessine, met à jour l'état quand
//! l'utilisateur agit (par [`Engine::dialog_mut`]) et redessine quand [`Engine::dialogs_version`] change.
//! `$dialog(nom, table)` ouvre un dialogue « modal » sans attendre sa fermeture (le moteur ne bloque jamais) et
//! rend `$null`.

use crate::exec::{R, split_word};
use crate::idents::tf;
use crate::wild::wildmatch;
use crate::{Control, ControlKind, Ctx, Dialog, DialogEvent, Engine, EventInfo, EventKind, Host};

/// État interne d'un dialogue ouvert.
#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub initialized: bool,
    pub focus: Option<u32>,
}

/// Découpe aux virgules hors guillemets.
fn split_fields(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    for c in s.chars() {
        match c {
            '"' => {
                q = !q;
                cur.push(c);
            }
            ',' if !q => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') { t[1..t.len() - 1].to_string() } else { t.to_string() }
}

fn rect4(s: &str) -> Option<(i32, i32, i32, i32)> {
    let v: Vec<i32> = s.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    (v.len() >= 4).then(|| (v[0], v[1], v[2], v[3]))
}

fn control_kind(kw: &str) -> Option<ControlKind> {
    Some(match kw {
        "text" => ControlKind::Text,
        "edit" => ControlKind::Edit,
        "button" => ControlKind::Button,
        "check" => ControlKind::Check,
        "radio" => ControlKind::Radio,
        "box" => ControlKind::Box,
        "list" => ControlKind::List,
        "combo" => ControlKind::Combo,
        "icon" => ControlKind::Icon,
        "link" => ControlKind::Link,
        "tab" => ControlKind::Tab,
        "scroll" => ControlKind::Scroll,
        _ => return None,
    })
}

/// Construit un dialogue à partir des lignes de sa table.
pub fn build_dialog(name: &str, table: &str, lines: &[String]) -> Dialog {
    let mut d = Dialog {
        name: name.to_string(),
        table: table.to_string(),
        title: String::new(),
        rect: (-1, -1, 200, 100),
        dbu: false,
        modal: false,
        controls: Vec::new(),
        active_tab: None,
    };
    let mut last_tab_rect = (0, 0, 0, 0);
    for l in lines {
        let (kw, rest) = split_word(l);
        let kw = kw.to_ascii_lowercase();
        match kw.as_str() {
            "title" => d.title = unquote(rest),
            "size" => {
                if let Some(r) = rect4(rest) {
                    d.rect = r;
                }
            }
            "option" => {
                for w in rest.split([' ', ',']).filter(|w| !w.is_empty()) {
                    match w.to_ascii_lowercase().as_str() {
                        "dbu" => d.dbu = true,
                        "pixels" => d.dbu = false,
                        _ => {}
                    }
                }
            }
            "menu" | "item" => {}
            k => {
                let Some(kind) = control_kind(k) else { continue };
                let f = split_fields(rest);
                // `icon fichier, index` sans id : l'icône du dialogue.
                let mut i = 0;
                let mut text = String::new();
                if f.first().is_some_and(|x| x.starts_with('"')) {
                    text = unquote(&f[0]);
                    i = 1;
                }
                let Some(id) = f.get(i).and_then(|x| x.trim().parse::<u32>().ok()) else { continue };
                i += 1;
                let rect = match f.get(i).and_then(|x| rect4(x)) {
                    Some(r) => {
                        i += 1;
                        r
                    }
                    None if kind == ControlKind::Tab => last_tab_rect,
                    None => (0, 0, 0, 0),
                };
                if kind == ControlKind::Tab {
                    last_tab_rect = rect;
                    if d.active_tab.is_none() {
                        d.active_tab = Some(id);
                    }
                }
                let mut styles = Vec::new();
                let mut tab = None;
                for field in &f[i..] {
                    let words: Vec<&str> = field.split_whitespace().collect();
                    let mut k = 0;
                    while k < words.len() {
                        if words[k].eq_ignore_ascii_case("tab") && k + 1 < words.len() && words[k + 1].parse::<u32>().is_ok() {
                            tab = words[k + 1].parse().ok();
                            k += 2;
                            continue;
                        }
                        styles.push(words[k].to_string());
                        k += 1;
                    }
                }
                if kind == ControlKind::Icon && text.is_empty() && !styles.is_empty() {
                    // `icon id, x y w h, fichier, index` : le fichier dans le texte.
                    text = styles.remove(0);
                }
                let disabled = styles.iter().any(|s| s.eq_ignore_ascii_case("disable"));
                let hidden = styles.iter().any(|s| s.eq_ignore_ascii_case("hide"));
                let items = if kind == ControlKind::Edit && styles.iter().any(|s| s.eq_ignore_ascii_case("multi")) && !text.is_empty() {
                    text.split('\n').map(|x| x.to_string()).collect()
                } else {
                    Vec::new()
                };
                d.controls.push(Control {
                    id,
                    kind,
                    rect,
                    text,
                    items,
                    selected: Vec::new(),
                    checked: false,
                    enabled: !disabled,
                    visible: !hidden,
                    styles,
                    tab,
                });
            }
        }
    }
    d
}

/// Liste d'id `1,2,5-7`.
fn parse_ids(s: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for p in s.split(',') {
        match p.split_once('-') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                    out.extend(a.min(b)..=a.max(b).min(a.min(b) + 10_000));
                }
            }
            None => {
                if let Ok(v) = p.trim().parse() {
                    out.push(v);
                }
            }
        }
    }
    out
}

fn is_listlike(k: ControlKind) -> bool {
    matches!(k, ControlKind::List | ControlKind::Combo)
}

impl Engine {
    /// Dialogues ouverts.
    pub fn dialogs(&self) -> &[Dialog] {
        &self.dialogs
    }

    /// Accès au dialogue ouvert `name`, pour que le client y reporte ce que fait l'utilisateur (texte saisi,
    /// sélection, cases).
    pub fn dialog_mut(&mut self, name: &str) -> Option<&mut Dialog> {
        self.dialogs.iter_mut().find(|d| d.name.eq_ignore_ascii_case(name))
    }

    /// Change à chaque modification des dialogues par les scripts (ouverture, fermeture, `/did`, `/dialog -t`…).
    pub fn dialogs_version(&self) -> u64 {
        self.dialogs_version
    }

    fn dialog_index(&self, name: &str) -> Option<usize> {
        self.dialogs.iter().position(|d| d.name.eq_ignore_ascii_case(name))
    }

    /// Ouvre un dialogue à partir d'une table et déclenche `init`.
    fn open_dialog(&mut self, h: &mut dyn Host, ctx: &Ctx, name: &str, table: &str, modal: bool) -> R<()> {
        if self.dialog_index(name).is_some() {
            return Err(self.err(format!("/dialog: '{name}' name in use")));
        }
        let cur = self.frame().script.clone();
        let lt = table.to_ascii_lowercase();
        let found = self
            .scripts
            .iter()
            .flat_map(|s| s.p.dialogs.iter().map(move |t| (s.name.clone(), t.clone())))
            .find(|(sn, t)| t.name == lt && (!t.local || cur.as_deref() == Some(sn.as_str())));
        let Some((_, t)) = found else {
            return Err(self.err(format!("/dialog: no such table '{table}'")));
        };
        let mut d = build_dialog(name, table, &t.lines);
        d.modal = modal;
        self.dialogs.push(d);
        self.dmeta.push(Meta { initialized: true, focus: None });
        self.dialogs_version += 1;
        self.fire_dialog(h, ctx, name, "init", 0);
        Ok(())
    }

    fn close_dialog(&mut self, name: &str) {
        if let Some(i) = self.dialog_index(name) {
            self.dialogs.remove(i);
            self.dmeta.remove(i);
            self.dialogs_version += 1;
        }
    }

    fn fire_dialog(&mut self, h: &mut dyn Host, ctx: &Ctx, name: &str, devent: &str, id: u32) -> bool {
        let ev = EventInfo {
            kind: Some(EventKind::Dialog),
            conn: ctx.conn,
            what: devent.to_string(),
            extra: vec![("dname".into(), name.to_string()), ("did".into(), id.to_string())],
            ..EventInfo::default()
        };
        let mut c = ctx.clone();
        c.event = Some(ev.clone());
        self.fire_scripts(h, &c, &ev, None)
    }

    /// `/dialog -m nom table`, `-x` (ferme sans événement), `-k`/`-c` (ferme comme `ok`/`cancel`), `-t` titre,
    /// `-s x y w h`, `-r` (centre), `-v`, `-o`, `-n`, `-e`, `-i`, `-d` (sans effet ici).
    pub(crate) fn cmd_dialog(&mut self, h: &mut dyn Host, ctx: &Ctx, args: &str) -> R<()> {
        let (sw, rest) = crate::exec::switches(args);
        let (name, rest) = split_word(rest);
        if name.is_empty() {
            return Err(self.err("/dialog: insufficient parameters"));
        }
        if sw.contains('m') {
            let (table, _) = split_word(rest);
            let table = if table.is_empty() { name } else { table };
            return self.open_dialog(h, ctx, name, table, false);
        }
        let Some(i) = self.dialog_index(name) else {
            return Err(self.err(format!("/dialog: no such dialog '{name}'")));
        };
        if sw.contains('x') {
            self.close_dialog(name);
            return Ok(());
        }
        if sw.contains('k') || sw.contains('c') {
            let dn = self.dialogs[i].name.clone();
            self.fire_dialog(h, ctx, &dn, "close", 0);
            self.close_dialog(&dn);
            return Ok(());
        }
        let d = &mut self.dialogs[i];
        if sw.contains('t') {
            d.title = rest.to_string();
        }
        if sw.contains('s')
            && let Some(r) = rect4(rest)
        {
            d.rect = r;
        }
        if sw.contains('r') {
            d.rect.0 = -1;
            d.rect.1 = -1;
        }
        self.dialogs_version += 1;
        Ok(())
    }

    /// `/did -abcdefhijkmnortuv nom id[,id…] [N] [texte]`.
    pub(crate) fn cmd_did(&mut self, args: &str) -> R<()> {
        let (sw, rest) = crate::exec::switches(args);
        let (name, rest) = split_word(rest);
        let (ids, rest) = split_word(rest);
        let Some(di) = self.dialog_index(name) else {
            return Err(self.err(format!("/did: no such dialog '{name}'")));
        };
        let ids = parse_ids(ids);
        if ids.is_empty() {
            return Err(self.err("/did: invalid parameters"));
        }
        // Numéro de ligne pour -i -o -d -c -u -k.
        let (mut line, mut text) = (None, rest.to_string());
        if sw.contains(['i', 'o', 'd', 'c', 'u', 'k']) {
            let (w, r) = split_word(rest);
            if let Ok(n) = w.parse::<usize>() {
                line = Some(n);
                text = r.to_string();
            }
        }
        for id in ids {
            let d = &mut self.dialogs[di];
            if sw.contains('t') {
                for c in d.controls.iter_mut() {
                    c.styles.retain(|s| !s.eq_ignore_ascii_case("default"));
                }
            }
            let Some(c) = d.controls.iter_mut().find(|c| c.id == id) else { continue };
            let list = is_listlike(c.kind);
            if sw.contains('r') {
                c.text.clear();
                c.items.clear();
                c.selected.clear();
            }
            if sw.contains('d') {
                if list {
                    if let Some(n) = line.filter(|n| *n >= 1 && *n <= c.items.len()) {
                        c.items.remove(n - 1);
                        c.selected.retain(|s| *s != n);
                        for s in c.selected.iter_mut() {
                            if *s > n {
                                *s -= 1;
                            }
                        }
                    }
                } else {
                    c.text.clear();
                }
            }
            if sw.contains('i') {
                let n = line.unwrap_or(c.items.len() + 1).clamp(1, c.items.len() + 1);
                if list {
                    c.items.insert(n - 1, text.clone());
                    for s in c.selected.iter_mut() {
                        if *s >= n {
                            *s += 1;
                        }
                    }
                } else {
                    c.text.insert_str(0, &text);
                }
            }
            if sw.contains('o') {
                if list {
                    let n = line.unwrap_or(1).max(1);
                    while c.items.len() < n {
                        c.items.push(String::new());
                    }
                    c.items[n - 1] = text.clone();
                } else {
                    c.text = text.clone();
                }
            }
            if sw.contains('a') {
                if list {
                    c.items.push(text.clone());
                } else {
                    c.text.push_str(&text);
                }
            }
            if sw.contains('c') {
                if list {
                    if let Some(n) = line.filter(|n| *n >= 1 && *n <= c.items.len()) {
                        let multi = c.styles.iter().any(|s| s.eq_ignore_ascii_case("extsel") || s.eq_ignore_ascii_case("multsel"));
                        if !(multi || sw.contains('k')) {
                            c.selected.clear();
                        }
                        if !c.selected.contains(&n) {
                            c.selected.push(n);
                            c.selected.sort_unstable();
                        }
                        if c.kind == ControlKind::Combo {
                            c.text = c.items[n - 1].clone();
                        }
                    }
                } else {
                    c.checked = true;
                }
            }
            if sw.contains('u') {
                if list {
                    match line {
                        Some(n) => c.selected.retain(|s| *s != n),
                        None => c.selected.clear(),
                    }
                } else {
                    c.checked = false;
                }
            }
            if sw.contains('e') {
                c.enabled = true;
            }
            if sw.contains('b') {
                c.enabled = false;
            }
            if sw.contains('v') {
                c.visible = true;
            }
            if sw.contains('h') {
                c.visible = false;
            }
            if sw.contains('m') && !c.styles.iter().any(|s| s.eq_ignore_ascii_case("read")) {
                c.styles.push("read".into());
            }
            if sw.contains('n') {
                c.styles.retain(|s| !s.eq_ignore_ascii_case("read"));
            }
            if sw.contains('t') {
                c.styles.push("default".into());
            }
            if sw.contains('f') {
                self.dmeta[di].focus = Some(id);
            }
        }
        self.dialogs_version += 1;
        Ok(())
    }

    /// Nom et paramètres de `$did(…)` : `$did(nom, id, N)` ou, dans un événement de dialogue, `$did(id, N)`.
    fn did_target(&self, ctx: &Ctx, args: &[String]) -> (String, Vec<String>) {
        let dname = self.event_extra(ctx, "dname");
        let first_is_id = args.first().is_some_and(|a| a.trim().parse::<u32>().is_ok());
        match dname {
            Some(dn) if first_is_id && self.dialog_index(&args[0]).is_none() => (dn, args.to_vec()),
            _ => (args.first().cloned().unwrap_or_default(), args.iter().skip(1).cloned().collect()),
        }
    }

    /// `$did(nom, id[, N]).text|len|lines|sel|seltext|state|enabled|visible|isid`.
    pub(crate) fn did_ident(&mut self, ctx: &Ctx, args: &[String], prop: Option<&str>) -> String {
        let (name, rest) = self.did_target(ctx, args);
        let Some(d) = self.dialogs.iter().find(|d| d.name.eq_ignore_ascii_case(&name)) else {
            return String::new();
        };
        let Some(id) = rest.first().and_then(|x| x.trim().parse::<u32>().ok()) else {
            return String::new();
        };
        let n: Option<usize> = rest.get(1).and_then(|x| x.trim().parse().ok());
        let Some(c) = d.controls.iter().find(|c| c.id == id) else {
            return if prop == Some("isid") { tf(false) } else { String::new() };
        };
        let list = is_listlike(c.kind);
        let line_text = |k: usize| -> String {
            if list {
                c.items.get(k.wrapping_sub(1)).cloned().unwrap_or_default()
            } else {
                c.text.split('\n').nth(k.wrapping_sub(1)).unwrap_or("").to_string()
            }
        };
        let main_text = || -> String {
            match n {
                Some(k) => line_text(k),
                None if c.kind == ControlKind::List => c.selected.first().map(|&s| line_text(s)).unwrap_or_default(),
                None => c.text.clone(),
            }
        };
        match prop {
            None | Some("text") => main_text(),
            Some("len") => main_text().chars().count().to_string(),
            Some("lines") => {
                if list {
                    c.items.len().to_string()
                } else if c.text.is_empty() {
                    "0".into()
                } else {
                    c.text.split('\n').count().to_string()
                }
            }
            Some("sel") => match n {
                Some(0) => c.selected.len().to_string(),
                Some(k) => c.selected.get(k - 1).map(|v| v.to_string()).unwrap_or_default(),
                None => c.selected.first().map(|v| v.to_string()).unwrap_or_default(),
            },
            Some("seltext") => c.selected.first().map(|&s| line_text(s)).unwrap_or_default(),
            Some("state") => (if c.checked { "1" } else { "0" }).into(),
            Some("enabled") => tf(c.enabled),
            Some("visible") => tf(c.visible),
            Some("isid") => tf(true),
            Some("csel") | Some("cstate") => String::new(),
            Some(_) => String::new(),
        }
    }

    /// `$didwm(nom, id, joker, [N])`, `$didreg(nom, id, re, [N])`, `$didtok(nom, id, C)`.
    pub(crate) fn did_misc_ident(&mut self, ctx: &Ctx, which: &str, args: &[String]) -> String {
        let (name, rest) = self.did_target(ctx, args);
        let Some(d) = self.dialogs.iter().find(|d| d.name.eq_ignore_ascii_case(&name)) else {
            return String::new();
        };
        let Some(c) = rest.first().and_then(|x| x.trim().parse::<u32>().ok()).and_then(|id| d.controls.iter().find(|c| c.id == id))
        else {
            return String::new();
        };
        let pat = rest.get(1).cloned().unwrap_or_default();
        if which == "didtok" {
            let sep = crate::strings::sep_char(&pat).unwrap_or(',');
            return c.items.join(&sep.to_string());
        }
        let from = rest.get(2).and_then(|x| x.parse::<usize>().ok()).unwrap_or(1).max(1);
        let re = if which == "didreg" { crate::regexid::compile(&pat).ok() } else { None };
        c.items
            .iter()
            .enumerate()
            .skip(from - 1)
            .find(|(_, it)| match &re {
                Some(r) => r.re.is_match(it).unwrap_or(false),
                None => wildmatch(&pat, it, false),
            })
            .map(|(i, _)| (i + 1).to_string())
            .unwrap_or_else(|| "0".into())
    }

    /// `$dialog(nom)`, `$dialog(N)`, `$dialog(0)`, `$dialog(nom).table|title|x|y|w|h|modal`, et
    /// `$dialog(nom, table[, parent])` qui ouvre un dialogue modal sans attendre (rend `$null`).
    pub(crate) fn dialog_ident(&mut self, h: &mut dyn Host, ctx: &Ctx, args: &[String], parens: bool, prop: Option<&str>) -> R<String> {
        if !parens {
            return Ok(self.event_extra(ctx, "dname").unwrap_or_default());
        }
        let a0 = args.first().cloned().unwrap_or_default();
        if args.len() >= 2 {
            self.open_dialog(h, ctx, &a0, &args[1], true)?;
            return Ok(String::new());
        }
        let d = match a0.parse::<usize>() {
            Ok(0) if self.dialog_index("0").is_none() => return Ok(self.dialogs.len().to_string()),
            Ok(k) if self.dialog_index(&a0).is_none() => self.dialogs.get(k - 1),
            _ => self.dialog_index(&a0).map(|i| &self.dialogs[i]),
        };
        let Some(d) = d else { return Ok(String::new()) };
        Ok(match prop {
            Some("table") => d.table.clone(),
            Some("title") => d.title.clone(),
            Some("x") => d.rect.0.to_string(),
            Some("y") => d.rect.1.to_string(),
            Some("w") => d.rect.2.to_string(),
            Some("h") => d.rect.3.to_string(),
            Some("modal") => tf(d.modal),
            Some("hwnd") => "0".into(),
            _ => d.name.clone(),
        })
    }

    /// Événement venu du client pour un dialogue ouvert : déclenche `on DIALOG` (`init`, `sclick`, `dclick`,
    /// `edit`, `close`). Un clic sur un onglet l'affiche ; un clic sur un bouton `ok` ou `cancel` ferme le dialogue
    /// après son `sclick` (avec `close`). `init` est déjà déclenché à l'ouverture par le moteur : un
    /// `DialogEvent::Init` du client n'a d'effet que si ce n'est pas encore fait.
    pub fn dialog_event(&mut self, host: &mut dyn Host, name: &str, event: DialogEvent) {
        self.flush_pending(host);
        let Some(i) = self.dialog_index(name) else { return };
        let dn = self.dialogs[i].name.clone();
        let ctx = Ctx::default();
        let style_of = |e: &Engine, id: u32| -> (Option<ControlKind>, bool) {
            match e.dialogs.get(i).and_then(|d| d.controls.iter().find(|c| c.id == id)) {
                Some(c) => (Some(c.kind), c.has_style("ok") || c.has_style("cancel")),
                None => (None, false),
            }
        };
        match event {
            DialogEvent::Init => {
                if !self.dmeta[i].initialized {
                    self.dmeta[i].initialized = true;
                    self.fire_dialog(host, &ctx, &dn, "init", 0);
                }
            }
            DialogEvent::Click(id) | DialogEvent::Ok(id) => {
                let (kind, closes) = style_of(self, id);
                if kind == Some(ControlKind::Tab) {
                    self.dialogs[i].active_tab = Some(id);
                    self.dialogs_version += 1;
                }
                self.fire_dialog(host, &ctx, &dn, "sclick", id);
                if closes || matches!(event, DialogEvent::Ok(_)) {
                    if self.dialog_index(&dn).is_some() {
                        self.fire_dialog(host, &ctx, &dn, "close", 0);
                    }
                    self.close_dialog(&dn);
                }
            }
            DialogEvent::DoubleClick(id) => {
                self.fire_dialog(host, &ctx, &dn, "dclick", id);
            }
            DialogEvent::Edit(id) => {
                self.fire_dialog(host, &ctx, &dn, "edit", id);
            }
            DialogEvent::Select(id) => {
                self.fire_dialog(host, &ctx, &dn, "sclick", id);
            }
            DialogEvent::Close => {
                self.fire_dialog(host, &ctx, &dn, "close", 0);
                self.close_dialog(&dn);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table() {
        let lines: Vec<String> = [
            "title \"Test, dialog\"",
            "size -1 -1 200 120",
            "option dbu",
            "tab \"One\", 1, 5 5 190 100",
            "tab \"Two\", 2",
            "text \"Hello\", 3, 10 20 50 10, tab 1",
            "edit \"\", 4, 10 30 80 10, autohs, tab 1",
            "list 5, 10 20 80 50, tab 2 sort",
            "button \"OK\", 6, 150 105 40 12, ok",
            "icon 7, 1 1 32 32, alp.ico, 0",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let d = build_dialog("t", "t", &lines);
        assert_eq!(d.title, "Test, dialog");
        assert!(d.dbu);
        assert_eq!(d.rect, (-1, -1, 200, 120));
        assert_eq!(d.controls.len(), 7);
        assert_eq!(d.controls[1].rect, (5, 5, 190, 100));
        assert_eq!(d.active_tab, Some(1));
        assert_eq!(d.controls[2].tab, Some(1));
        assert_eq!(d.controls[4].tab, Some(2));
        assert!(d.controls[4].has_style("sort"));
        assert!(d.controls[5].has_style("ok"));
        assert_eq!(d.controls[6].text, "alp.ico");
        assert_eq!(parse_ids("1,3-5"), vec![1, 3, 4, 5]);
    }
}
