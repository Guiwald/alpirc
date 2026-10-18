//! Éditeur de scripts (Alt+R), à la manière de mIRC : onglets Aliases, Popups (avec le choix du menu : Status,
//! Canal, Privé, Liste des pseudos, Barre de menus), Remote, Users et Variables. Pour les trois premiers, une
//! liste des fichiers chargés de cette sorte, et les boutons Nouveau, Charger…, Décharger ; la grande zone de
//! texte montre le fichier choisi, la ligne et la colonne du curseur sont affichées en bas.
//!
//! Les modifications restent dans la boîte jusqu'à Enregistrer ou OK : chaque fichier modifié est alors relu par
//! le moteur (`Client::load_script_text`) puis tout est écrit sur le disque (`Client::save_scripts`). Les fichiers
//! d'alias et de popups sont enregistrés au format `.ini` de mIRC (`[aliases]`, `n0=…`), le seul que le moteur
//! reconnaît comme tel au démarrage suivant.

use std::collections::HashMap;

use alpirc_core::i18n::Lang;
use alpirc_script::ScriptKind;
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogView, Rect};

use super::*;

const W: i32 = 680;
const H: i32 = 480;

const TABS: [u32; 5] = [10, 11, 12, 13, 14];
const T_ALIASES: usize = 0;
const T_POPUPS: usize = 1;
const T_REMOTE: usize = 2;
const T_USERS: usize = 3;
const T_VARS: usize = 4;

const FILE_L: u32 = 20;
const FILE: u32 = 21;
const VIEW_L: u32 = 22;
const VIEW: u32 = 23;
const NEW: u32 = 24;
const LOAD: u32 = 25;
const UNLOAD: u32 = 26;
const HINT: u32 = 27;
const TEXT: u32 = 30;
const POS: u32 = 31;
const SAVE: u32 = 3;
const OK: u32 = 1;
const CANCEL: u32 = 2;

/// Cibles des popups, dans l'ordre de la liste « Menu », avec les noms de section `.ini` qui y mènent (le
/// premier est celui qu'on écrit).
const TARGETS: [(&str, &[&str]); 5] = [
    ("status", &["spopup", "bpopup", "status"]),
    ("channel", &["cpopup", "channel"]),
    ("query", &["qpopup", "query"]),
    ("nicklist", &["lpopup", "npopup", "nicklist"]),
    ("menubar", &["mpopup", "menubar"]),
];

// ---------------------------------------------------------------------------------------------------------------
// Format `.ini` de mIRC.

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

fn strip_n(t: &str) -> &str {
    if is_n_line(t) { &t[t.find('=').unwrap() + 1..] } else { t }
}

fn section_name(l: &str) -> Option<String> {
    let t = l.trim();
    (t.starts_with('[') && t.ends_with(']') && t.len() > 2).then(|| t[1..t.len() - 1].to_ascii_lowercase())
}

/// Vrai si le texte est au format `.ini` de mIRC (comme le moteur le reconnaît).
pub fn is_ini(text: &str) -> bool {
    let section = text.lines().filter_map(section_name).any(|s| matches!(s.as_str(), "script" | "aliases" | "users" | "variables") || s.ends_with("popup"));
    section && text.lines().any(|l| is_n_line(l.trim()))
}

/// Lignes d'une section (préfixes `nN=` retirés) ; `None` si elle n'y est pas.
pub fn ini_section(text: &str, names: &[&str]) -> Option<String> {
    let mut out: Option<Vec<String>> = None;
    let mut inside = false;
    for l in text.lines() {
        let l = l.trim_end_matches('\r');
        if let Some(s) = section_name(l) {
            inside = names.contains(&s.as_str());
            if inside && out.is_none() {
                out = Some(Vec::new());
            }
        } else if inside {
            let t = l.trim_start();
            if !t.is_empty() {
                out.as_mut().unwrap().push(strip_n(t).to_string());
            }
        }
    }
    out.map(|v| v.join("\n"))
}

/// Écrit des sections au format `.ini` (`[nom]` puis `n0=`, `n1=`…).
pub fn to_ini(sections: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for (name, text) in sections {
        out.push_str(&format!("[{name}]\n"));
        let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            out.push_str("n0=\n");
        }
        for (i, l) in lines.iter().enumerate() {
            out.push_str(&format!("n{i}={l}\n"));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------

/// Ce que montre la zone de texte.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Doc {
    /// Fichier d'alias ou remote, en entier.
    Script(String),
    /// Section d'un fichier de popups (numéro de la cible).
    Popup(String, usize),
    Users,
    Vars,
}

/// Texte d'un document en cours de modification et texte d'origine.
#[derive(Clone, Debug)]
struct Buf {
    text: String,
    orig: String,
}

/// L'éditeur de scripts.
pub struct ScriptEditor {
    tab: usize,
    /// Fichiers des onglets Aliases, Popups, Remote.
    files: [Vec<String>; 3],
    cur: [Option<usize>; 3],
    target: usize,
    /// Fichiers créés par « Nouveau », pas encore dans le moteur.
    fresh: Vec<(usize, String)>,
    bufs: HashMap<Doc, Buf>,
    shown: Option<Doc>,
    /// Scripts du moteur vus la dernière fois.
    seen: Vec<String>,
}

fn kind_of_tab(tab: usize) -> ScriptKind {
    match tab {
        T_ALIASES => ScriptKind::Aliases,
        T_POPUPS => ScriptKind::Popups,
        _ => ScriptKind::Remote,
    }
}

fn tab_of_kind(k: ScriptKind) -> usize {
    match k {
        ScriptKind::Aliases => T_ALIASES,
        ScriptKind::Popups => T_POPUPS,
        ScriptKind::Remote => T_REMOTE,
    }
}

impl ScriptEditor {
    pub fn new(client: &Client) -> ScriptEditor {
        let mut e = ScriptEditor {
            tab: T_REMOTE,
            files: Default::default(),
            cur: [None; 3],
            target: 1,
            fresh: Vec::new(),
            bufs: HashMap::new(),
            shown: None,
            seen: Vec::new(),
        };
        e.read_files(client);
        for k in 0..3 {
            e.cur[k] = if e.files[k].is_empty() { None } else { Some(0) };
        }
        e
    }

    /// Relit la liste des scripts du moteur (plus les fichiers nouveaux pas encore enregistrés).
    fn read_files(&mut self, client: &Client) {
        let scripts = client.engine().scripts();
        self.seen = scripts.iter().map(|s| s.name.clone()).collect();
        for k in 0..3 {
            let mut v: Vec<String> = scripts.iter().filter(|s| tab_of_kind(s.kind) == k).map(|s| s.name.clone()).collect();
            let extra: Vec<String> = self.fresh.iter().filter(|(t, n)| *t == k && !v.contains(n)).map(|(_, n)| n.clone()).collect();
            v.extend(extra);
            self.files[k] = v;
        }
    }

    /// Document de l'onglet courant.
    fn doc(&self) -> Option<Doc> {
        match self.tab {
            T_USERS => Some(Doc::Users),
            T_VARS => Some(Doc::Vars),
            t => {
                let name = self.cur[t].and_then(|i| self.files[t].get(i))?.clone();
                Some(if t == T_POPUPS { Doc::Popup(name, self.target) } else { Doc::Script(name) })
            }
        }
    }

    fn is_fresh(&self, name: &str) -> bool {
        self.fresh.iter().any(|(_, n)| n == name)
    }

    /// Texte d'origine d'un document (tel que le moteur l'a).
    fn original(&self, client: &Client, d: &Doc) -> String {
        let e = client.engine();
        match d {
            Doc::Users => e.users_text(),
            Doc::Vars => e.vars_text(),
            Doc::Script(name) => {
                let raw = e.script_text(name).unwrap_or("").to_string();
                let is_alias = e.scripts().iter().any(|s| &s.name == name && s.kind == ScriptKind::Aliases);
                if is_alias && is_ini(&raw) { ini_section(&raw, &["aliases"]).unwrap_or_default() } else { raw }
            }
            Doc::Popup(name, target) => {
                let raw = e.script_text(name).unwrap_or("").to_string();
                if is_ini(&raw) { ini_section(&raw, TARGETS[*target].1).unwrap_or_default() } else { raw }
            }
        }
    }

    /// Le fichier de popups est-il découpé en sections (sinon : un seul menu, le choix de la cible est inutile) ?
    fn popup_sections(&self, client: &Client, name: &str) -> bool {
        self.is_fresh(name) || client.engine().script_text(name).is_none_or(|t| is_ini(t) || t.trim().is_empty())
    }

    /// Range le texte de la zone dans le tampon du document montré.
    fn stash(&mut self, view: &DialogView) {
        if let Some(d) = &self.shown {
            let text = text_of(view, TEXT);
            if let Some(b) = self.bufs.get_mut(d) {
                b.text = text;
            }
        }
    }

    /// Montre l'onglet et le document courants.
    fn show(&mut self, view: &mut DialogView, cx: &Cx) {
        let lang = cx.lang;
        let scripts_tab = self.tab <= T_REMOTE;
        for id in [FILE_L, FILE, NEW, LOAD, UNLOAD] {
            view.set_visible(id, scripts_tab);
        }
        view.set_visible(VIEW_L, self.tab == T_POPUPS);
        view.set_visible(VIEW, self.tab == T_POPUPS);
        view.set_visible(HINT, !scripts_tab);
        view.set_text(HINT, &t(lang, if self.tab == T_USERS { "se.hint.users" } else { "se.hint.vars" }));
        let top = if scripts_tab { 70 } else { 64 };
        view.set_rect(TEXT, Rect::new(20, top, W - 40, H - MARGIN - BTN_H - 8 - 8 - top));
        if scripts_tab {
            let t = self.tab;
            view.set_items(FILE, &self.files[t]);
            select_row(view, FILE, self.cur[t]);
            view.set_enabled(UNLOAD, self.cur[t].is_some());
            if t == T_POPUPS {
                view.set_selection(VIEW, &[self.target]);
                let sections = self.cur[t].and_then(|i| self.files[t].get(i)).is_none_or(|n| self.popup_sections(cx.client, n));
                view.set_enabled(VIEW, sections);
            }
        }
        let doc = self.doc();
        let (text, enabled) = match &doc {
            Some(d) => {
                if !self.bufs.contains_key(d) {
                    let orig = self.original(cx.client, d);
                    self.bufs.insert(d.clone(), Buf { text: orig.clone(), orig });
                }
                (self.bufs[d].text.clone(), true)
            }
            None => (String::new(), false),
        };
        view.set_text(TEXT, &text);
        view.set_enabled(TEXT, enabled);
        self.shown = doc;
        self.update_pos(view, lang);
    }

    fn update_pos(&self, view: &mut DialogView, lang: Lang) {
        let (l, c) = view.caret_line_col(TEXT).unwrap_or((0, 0));
        view.set_text(POS, &tf(lang, "se.pos", &[("line", &(l + 1).to_string()), ("col", &(c + 1).to_string())]));
    }

    /// Nom libre pour un nouveau fichier de l'onglet.
    fn fresh_name(&self, client: &Client) -> String {
        let (stem, ext) = match self.tab {
            T_ALIASES => ("aliases", "ini"),
            T_POPUPS => ("popups", "ini"),
            _ => ("script", "mrc"),
        };
        let taken =
            |n: &str| client.engine().scripts().iter().any(|s| s.name.eq_ignore_ascii_case(n)) || self.is_fresh(n) || client.engine().root().join(n).exists();
        (1..).map(|i| if i == 1 { format!("{stem}.{ext}") } else { format!("{stem}{i}.{ext}") }).find(|n| !taken(n)).unwrap()
    }

    /// Textes modifiés d'un fichier, tels que le moteur doit les relire ; `None` s'il n'a pas changé.
    fn file_text(&self, client: &Client, tab: usize, name: &str) -> Option<String> {
        let changed = |d: &Doc| self.bufs.get(d).is_some_and(|b| b.text != b.orig);
        match tab {
            T_POPUPS => {
                let docs: Vec<Doc> = (0..5).map(|k| Doc::Popup(name.to_string(), k)).collect();
                let dirty = docs.iter().any(changed) || self.is_fresh(name);
                if !dirty {
                    return None;
                }
                if !self.popup_sections(client, name) {
                    // Un seul menu, sans sections : le texte tel quel.
                    return Some(self.bufs.get(&docs[self.target]).map(|b| b.text.clone()).unwrap_or_default());
                }
                let texts: Vec<String> = docs.iter().map(|d| self.bufs.get(d).map(|b| b.text.clone()).unwrap_or_else(|| self.original(client, d))).collect();
                let mut sections: Vec<(&str, &str)> =
                    TARGETS.iter().zip(&texts).filter(|(_, t)| !t.trim().is_empty()).map(|((_, names), t)| (names[0], t.as_str())).collect();
                if sections.is_empty() {
                    sections.push(("cpopup", ""));
                }
                Some(to_ini(&sections))
            }
            T_ALIASES => {
                let d = Doc::Script(name.to_string());
                (changed(&d) || self.is_fresh(name)).then(|| to_ini(&[("aliases", self.bufs.get(&d).map(|b| b.text.as_str()).unwrap_or(""))]))
            }
            _ => {
                let d = Doc::Script(name.to_string());
                (changed(&d) || self.is_fresh(name)).then(|| self.bufs.get(&d).map(|b| b.text.clone()).unwrap_or_default())
            }
        }
    }

    /// Enregistre tout ce qui a changé ; `Err` : message d'erreur du moteur (le reste est enregistré).
    fn save(&mut self, view: &DialogView, cx: &mut Cx) -> Result<bool, String> {
        self.stash(view);
        let mut changed = false;
        let mut errors = Vec::new();
        for tab in [T_ALIASES, T_POPUPS, T_REMOTE] {
            for name in self.files[tab].clone() {
                let Some(text) = self.file_text(cx.client, tab, &name) else { continue };
                match cx.client.load_script_text(&name, kind_of_tab(tab), &text, cx.now) {
                    Ok(()) => {
                        changed = true;
                        self.fresh.retain(|(_, n)| *n != name);
                        // Les tampons de ce fichier repartent de ce que le moteur a maintenant.
                        let docs: Vec<Doc> = self.bufs.keys().filter(|d| matches!(d, Doc::Script(n) | Doc::Popup(n, _) if *n == name)).cloned().collect();
                        for d in docs {
                            let orig = self.original(cx.client, &d);
                            if let Some(b) = self.bufs.get_mut(&d) {
                                b.orig = orig;
                            }
                        }
                    }
                    Err(e) => errors.push(format!("{name} : {e}")),
                }
            }
        }
        for d in [Doc::Users, Doc::Vars] {
            if let Some(b) = self.bufs.get(&d).filter(|b| b.text != b.orig).cloned() {
                if d == Doc::Users {
                    cx.client.engine_mut().set_users_text(&b.text);
                } else {
                    cx.client.engine_mut().set_vars_text(&b.text);
                }
                let orig = self.original(cx.client, &d);
                self.bufs.insert(d, Buf { text: b.text, orig });
                changed = true;
            }
        }
        if changed && let Err(e) = cx.client.save_scripts() {
            errors.push(e.to_string());
        }
        self.read_files(cx.client);
        if errors.is_empty() { Ok(changed) } else { Err(errors.join("\n")) }
    }

    fn unload(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let t = self.tab;
        let Some(name) = self.cur[t].and_then(|i| self.files[t].get(i)).cloned() else { return };
        self.stash(view);
        if self.is_fresh(&name) {
            self.fresh.retain(|(_, n)| *n != name);
        } else if cx.client.engine_mut().unload(&name) {
            cx.client.run_pending_scripts(cx.now);
            if let Err(e) = cx.client.save_scripts() {
                log::warn!("scripts non enregistrés : {e}");
            }
        }
        self.bufs.retain(|d, _| !matches!(d, Doc::Script(n) | Doc::Popup(n, _) if *n == name));
        self.shown = None;
        self.read_files(cx.client);
        let n = self.files[t].len();
        self.cur[t] = self.cur[t].map(|i| i.min(n.saturating_sub(1))).filter(|_| n > 0);
        self.show(view, cx);
    }
}

impl DialogCtl for ScriptEditor {
    fn kind(&self) -> Option<String> {
        Some("scripts".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tr = |k: &str| t(lang, k);
        let page = (MARGIN, MARGIN, W - 2 * MARGIN, H - 2 * MARGIN - BTN_H - 8);
        let mut c: Vec<ControlSpec> = ["se.tab.aliases", "se.tab.popups", "se.tab.remote", "se.tab.users", "se.tab.variables"]
            .iter()
            .zip(TABS)
            .map(|(k, id)| ControlSpec::new(id, ControlKind::Tab, page, &tr(k)))
            .collect();
        let views = [tr("se.view.status"), tr("se.view.channel"), tr("se.view.query"), tr("se.view.nicklist"), tr("se.view.menubar")];
        let bw = 74;
        let bx = W - 20 - 3 * bw - 12;
        c.extend([
            field_label(FILE_L, 20, 40, 56, &tr("se.file")),
            drop_list(FILE, (80, 40, 190, EDIT_H + 10 * 16), &[] as &[&str]),
            field_label(VIEW_L, 280, 40, 50, &tr("se.view")),
            drop_list(VIEW, (332, 40, bx - 6 - 332, EDIT_H + 5 * 16 + 4), &views),
            button(NEW, (bx, 39, bw, BTN_H), &tr("se.new")),
            button(LOAD, (bx + bw + 6, 39, bw, BTN_H), &tr("se.load")),
            button(UNLOAD, (bx + 2 * (bw + 6), 39, bw, BTN_H), &tr("se.unload")),
            text(HINT, (20, 42, W - 40, LABEL_H), ""),
            text_area(TEXT, (20, 70, W - 40, H - MARGIN - BTN_H - 8 - 8 - 70), ""),
            text(POS, (MARGIN + 4, H - MARGIN - BTN_H + 4, 200, LABEL_H), "").style("nowrap"),
            button(SAVE, (W - MARGIN - 3 * BTN_W - 12, H - MARGIN - BTN_H, BTN_W, BTN_H), &tr("se.save")),
            button(OK, (W - MARGIN - 2 * BTN_W - 6, H - MARGIN - BTN_H, BTN_W, BTN_H), &tr("dlg.ok")).style("ok"),
            button(CANCEL, (W - MARGIN - BTN_W, H - MARGIN - BTN_H, BTN_W, BTN_H), &tr("dlg.cancel")).style("cancel"),
        ]);
        spec(&tr("se.title"), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        view.set_selection(TABS[self.tab], &[]);
        self.show(view, cx);
        view.set_focus(TEXT);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        let lang = cx.lang;
        match msg {
            DialogMsg::Ok(_) => match self.save(view, cx) {
                Ok(_) => Outcome::Close,
                Err(e) => {
                    cx.actions.push(Action::Open(Box::new(MessageBox::info(&t(lang, "se.title"), &e, lang))));
                    Outcome::Keep
                }
            },
            DialogMsg::Cancel => Outcome::Close,
            DialogMsg::Clicked(SAVE) => {
                if let Err(e) = self.save(view, cx) {
                    cx.actions.push(Action::Open(Box::new(MessageBox::info(&t(lang, "se.title"), &e, lang))));
                }
                self.show(view, cx);
                Outcome::Keep
            }
            DialogMsg::Clicked(id) if TABS.contains(&id) => {
                self.stash(view);
                self.tab = TABS.iter().position(|&t| t == id).unwrap();
                self.show(view, cx);
                Outcome::Keep
            }
            DialogMsg::SelectionChanged(FILE) => {
                self.stash(view);
                self.cur[self.tab] = sel(view, FILE);
                self.show(view, cx);
                Outcome::Keep
            }
            DialogMsg::SelectionChanged(VIEW) => {
                self.stash(view);
                self.target = sel(view, VIEW).unwrap_or(1).min(4);
                self.show(view, cx);
                Outcome::Keep
            }
            DialogMsg::Changed(TEXT) => {
                self.stash(view);
                self.update_pos(view, lang);
                Outcome::Keep
            }
            DialogMsg::SelectionChanged(TEXT) => {
                self.update_pos(view, lang);
                Outcome::Keep
            }
            DialogMsg::Clicked(NEW) if self.tab <= T_REMOTE => {
                self.stash(view);
                let name = self.fresh_name(cx.client);
                self.fresh.push((self.tab, name.clone()));
                self.read_files(cx.client);
                self.cur[self.tab] = self.files[self.tab].iter().position(|n| *n == name);
                self.show(view, cx);
                view.set_focus(TEXT);
                Outcome::Keep
            }
            DialogMsg::Clicked(UNLOAD) => {
                self.unload(view, cx);
                Outcome::Keep
            }
            DialogMsg::Clicked(LOAD) => {
                let dir = cx.client.engine().root().to_path_buf();
                cx.actions.push(Action::Open(Box::new(LoadScript { dir, path: None })));
                Outcome::Keep
            }
            _ => Outcome::Keep,
        }
    }

    fn refresh(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let names: Vec<String> = cx.client.engine().scripts().iter().map(|s| s.name.clone()).collect();
        if names == self.seen {
            return;
        }
        let new: Option<String> = names.iter().find(|n| !self.seen.contains(n)).cloned();
        self.stash(view);
        self.read_files(cx.client);
        // Un script vient d'être chargé (bouton Charger…, `/load`) : on le montre.
        if let Some(n) = new
            && let Some(tab) = (0..3).find(|&k| self.files[k].contains(&n))
        {
            self.tab = tab;
            self.cur[tab] = self.files[tab].iter().position(|x| *x == n);
            view.set_selection(TABS[tab], &[]);
        }
        for k in 0..3 {
            let n = self.files[k].len();
            if self.cur[k].is_none_or(|i| i >= n) {
                self.cur[k] = if n == 0 { None } else { Some(0) };
            }
        }
        self.shown = None;
        self.show(view, cx);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Charger un script.

/// Petite boîte « Charger un script » : le chemin du fichier (champ, ou bouton Parcourir… quand la plateforme sait
/// demander un fichier). Un script qui lance des commandes dès son chargement (`on LOAD`, `on START`) n'est
/// chargé qu'après confirmation, si les réglages le demandent (ADR 0007).
pub struct LoadScript {
    pub dir: PathBuf,
    pub path: Option<PathBuf>,
}

pub(crate) const L_PATH: u32 = 11;
pub(crate) const L_BROWSE: u32 = 12;

impl LoadScript {
    fn load(&self, path: &std::path::Path, cx: &mut Cx) -> Result<(), String> {
        let lang = cx.lang;
        if !path.is_file() {
            return Err(tf(lang, "se.err_file", &[("path", &path.display().to_string())]));
        }
        let already = cx.client.engine().scripts().iter().any(|s| s.path.as_deref() == Some(path));
        let name = cx.client.engine_mut().load_file(path, true).map_err(|e| e.to_string())?;
        let warn = cx.client.settings().scripts.warn_on_load && !already && cx.client.engine().runs_at_startup(&name);
        if warn {
            // On retire le script en attendant la réponse : rien ne doit s'exécuter avant.
            cx.client.engine_mut().discard_pending();
            cx.client.engine_mut().unload(&name);
            cx.client.engine_mut().discard_pending();
            cx.actions.push(Action::Open(Box::new(ConfirmScript { path: path.to_path_buf(), name })));
        } else {
            cx.client.run_pending_scripts(cx.now);
            cx.client.save_scripts().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

impl DialogCtl for LoadScript {
    fn kind(&self) -> Option<String> {
        Some("load-script".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let w = 460;
        let h = MARGIN + 4 + LABEL_H + 6 + EDIT_H + 14 + BTN_H + MARGIN;
        let mut c = vec![
            text(10, (MARGIN + 4, MARGIN + 4, w - 24, LABEL_H), &t(lang, "se.load_prompt")),
            edit(L_PATH, (MARGIN + 4, MARGIN + 4 + LABEL_H + 6, w - 24 - 96 - 6, EDIT_H), ""),
            button(L_BROWSE, (w - MARGIN - 4 - 96, MARGIN + 4 + LABEL_H + 5, 96, BTN_H), &t(lang, "se.browse")),
        ];
        c.extend(ok_cancel(lang, w, h, 1, 2, None));
        spec(&t(lang, "se.load_title"), w, h, c)
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        let p = self.path.clone().unwrap_or_else(|| self.dir.join(""));
        view.set_text(L_PATH, &p.display().to_string());
        view.set_focus(L_PATH);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) => {
                let p = text_of(view, L_PATH);
                let p = p.trim();
                if p.is_empty() {
                    return Outcome::Keep;
                }
                let path = {
                    let p = PathBuf::from(p);
                    if p.is_absolute() { p } else { self.dir.join(p) }
                };
                match self.load(&path, cx) {
                    Ok(()) => Outcome::Close,
                    Err(e) => {
                        cx.actions.push(Action::Open(Box::new(MessageBox::info(&t(cx.lang, "se.load_title"), &e, cx.lang))));
                        Outcome::Keep
                    }
                }
            }
            DialogMsg::Clicked(L_BROWSE) => {
                cx.actions.push(Action::PickFile { kind: "load-script".into(), save: false, suggested: None });
                Outcome::Keep
            }
            DialogMsg::Cancel => Outcome::Close,
            _ => Outcome::Keep,
        }
    }

    fn file_picked(&mut self, path: Option<PathBuf>, view: &mut DialogView, _cx: &mut Cx) {
        if let Some(p) = path {
            view.set_text(L_PATH, &p.display().to_string());
            self.path = Some(p);
        }
    }
}

/// Confirmation avant de charger un script qui lance des commandes à son chargement.
pub struct ConfirmScript {
    pub path: PathBuf,
    pub name: String,
}

impl DialogCtl for ConfirmScript {
    fn kind(&self) -> Option<String> {
        Some("confirm-script".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let mb = MessageBox {
            title: t(lang, "se.warn_title"),
            text: tf(lang, "se.warn_text", &[("name", &self.name)]),
            yes: t(lang, "dlg.yes"),
            no: Some(t(lang, "dlg.no")),
            on_yes: Vec::new(),
        };
        mb.spec(lang)
    }

    fn on(&mut self, msg: DialogMsg, _view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) | DialogMsg::Clicked(2) => {
                match cx.client.engine_mut().load_file(&self.path, true) {
                    Ok(_) => {
                        cx.client.run_pending_scripts(cx.now);
                        if let Err(e) = cx.client.save_scripts() {
                            log::warn!("scripts non enregistrés : {e}");
                        }
                    }
                    Err(e) => cx.actions.push(Action::Open(Box::new(MessageBox::info(&t(cx.lang, "se.load_title"), &e.to_string(), cx.lang)))),
                }
                Outcome::Close
            }
            DialogMsg::Cancel | DialogMsg::Clicked(3) => Outcome::Close,
            _ => Outcome::Keep,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};

    #[test]
    fn ini_helpers() {
        let t = to_ini(&[("aliases", "j join $1-\n\n/hop part # | join #\n")]);
        assert_eq!(t, "[aliases]\nn0=j join $1-\nn1=/hop part # | join #\n");
        assert!(is_ini(&t));
        assert_eq!(ini_section(&t, &["aliases"]).as_deref(), Some("j join $1-\n/hop part # | join #"));
        assert!(!is_ini("alias j join $1-"));
        let p = to_ini(&[("cpopup", "Op:op $$1"), ("mpopup", "Aide:help")]);
        assert_eq!(ini_section(&p, TARGETS[4].1).as_deref(), Some("Aide:help"));
        assert_eq!(ini_section(&p, TARGETS[0].1), None);
    }

    #[test]
    fn new_alias_file_then_remote_edit() {
        let mut k = Kit::new("editor");
        let mut e = ScriptEditor::new(&k.client);
        assert_translated(&e);
        let (mut v, _) = k.open(&mut e);
        // Onglet Remote : les scripts par défaut.
        assert!(v.items(FILE).iter().any(|n| n == "aliases.mrc"));
        // Onglet Aliases, nouveau fichier, un alias, OK.
        k.send(&mut e, &mut v, DialogMsg::Clicked(TABS[T_ALIASES]));
        k.send(&mut e, &mut v, DialogMsg::Clicked(NEW));
        assert_eq!(v.items(FILE), vec!["aliases.ini"]);
        v.set_text(TEXT, "/salut echo -a Salut $1 !\nbye quit");
        k.send(&mut e, &mut v, DialogMsg::Changed(TEXT));
        assert_eq!(text_of(&v, POS), "Ln 1, Col 1");
        let (o, _) = k.send(&mut e, &mut v, DialogMsg::Ok(OK));
        assert_eq!(o, Outcome::Close);
        assert!(k.client.engine().has_alias("salut"));
        let on_disk = std::fs::read_to_string(k.dir.join("scripts/aliases.ini")).unwrap();
        assert!(on_disk.starts_with("[aliases]\nn0=/salut"), "{on_disk}");
        // L'alias marche.
        let st = k.client.windows()[0].id;
        k.client.input(st, "/salut Elisa", k.now);
        assert!(k.client.window(st).unwrap().lines.iter().any(|l| l.text.contains("Salut Elisa")));
        // Remote : modifier un fichier existant ; une erreur de lecture garde la boîte ouverte.
        let mut e = ScriptEditor::new(&k.client);
        let (mut v, _) = k.open(&mut e);
        let i = v.items(FILE).iter().position(|n| n == "aliases.mrc").unwrap();
        v.set_selection(FILE, &[i]);
        k.send(&mut e, &mut v, DialogMsg::SelectionChanged(FILE));
        assert!(text_of(&v, TEXT).contains("alias j join"));
        v.set_text(TEXT, "alias coucou { echo -a coucou }\n");
        k.send(&mut e, &mut v, DialogMsg::Changed(TEXT));
        k.send(&mut e, &mut v, DialogMsg::Clicked(SAVE));
        assert!(k.client.engine().has_alias("coucou"));
        assert!(!k.client.engine().has_alias("j"));
    }

    #[test]
    fn popups_sections_users_vars_and_unload() {
        let mut k = Kit::new("editor2");
        let mut e = ScriptEditor::new(&k.client);
        let (mut v, _) = k.open(&mut e);
        k.send(&mut e, &mut v, DialogMsg::Clicked(TABS[T_POPUPS]));
        k.send(&mut e, &mut v, DialogMsg::Clicked(NEW));
        assert!(v.enabled(VIEW));
        v.set_selection(VIEW, &[1]);
        k.send(&mut e, &mut v, DialogMsg::SelectionChanged(VIEW));
        v.set_text(TEXT, "Bonjour:say bonjour");
        k.send(&mut e, &mut v, DialogMsg::Changed(TEXT));
        v.set_selection(VIEW, &[4]);
        k.send(&mut e, &mut v, DialogMsg::SelectionChanged(VIEW));
        v.set_text(TEXT, "Aide:help");
        k.send(&mut e, &mut v, DialogMsg::Changed(TEXT));
        // Retour au canal : le texte est gardé.
        v.set_selection(VIEW, &[1]);
        k.send(&mut e, &mut v, DialogMsg::SelectionChanged(VIEW));
        assert_eq!(text_of(&v, TEXT), "Bonjour:say bonjour");
        k.send(&mut e, &mut v, DialogMsg::Clicked(TABS[T_VARS]));
        assert!(!v.visible(FILE) && v.visible(HINT));
        v.set_text(TEXT, "%ville Lyon\n");
        k.send(&mut e, &mut v, DialogMsg::Changed(TEXT));
        k.send(&mut e, &mut v, DialogMsg::Clicked(SAVE));
        let text = k.client.engine().script_text("popups.ini").unwrap().to_string();
        assert_eq!(text, "[cpopup]\nn0=Bonjour:say bonjour\n[mpopup]\nn0=Aide:help\n");
        assert!(k.client.engine().vars_text().contains("%ville Lyon"));
        // Décharger.
        k.send(&mut e, &mut v, DialogMsg::Clicked(TABS[T_POPUPS]));
        k.send(&mut e, &mut v, DialogMsg::Clicked(UNLOAD));
        assert!(k.client.engine().script_text("popups.ini").is_none());
        assert!(v.items(FILE).is_empty());
    }

    #[test]
    fn load_script_with_confirmation() {
        let mut k = Kit::new("editor3");
        let path = k.dir.join("hello.mrc");
        std::fs::write(&path, "on *:LOAD:{ set %charge oui }\nalias hello echo -a hello\n").unwrap();
        let mut l = LoadScript { dir: k.dir.clone(), path: None };
        assert_translated(&l);
        let (mut v, _) = k.open(&mut l);
        v.set_text(L_PATH, &path.display().to_string());
        let (o, mut a) = k.send(&mut l, &mut v, DialogMsg::Ok(1));
        assert_eq!(o, Outcome::Close);
        assert!(!k.client.engine().has_alias("hello"), "pas avant la réponse");
        let Some(Action::Open(mut c)) = a.pop() else { panic!() };
        let (mut cv, _) = k.open(c.as_mut());
        k.send(c.as_mut(), &mut cv, DialogMsg::Ok(2));
        assert!(k.client.engine().has_alias("hello"));
        assert!(k.client.engine().vars_text().contains("%charge oui"));
    }
}
