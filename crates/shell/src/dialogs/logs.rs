//! Journaux (Outils › Journaux…) : la liste des fichiers du dossier des journaux (réseau, fenêtre, taille, date),
//! la lecture d'un journal en lecture seule avec recherche, et l'ouverture du dossier dans le gestionnaire de
//! fichiers du système.

use std::path::{Path, PathBuf};

use alpirc_core::Client;
use alpirc_core::i18n::Lang;
use alpirc_ui::dialog::Widget;
use alpirc_ui::{DialogMsg, DialogSpec, DialogView};

use super::*;

/// Taille au-delà de laquelle seule la fin d'un journal est montrée.
const MAX_SHOWN: usize = 2 * 1024 * 1024;

/// Un fichier de journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub path: PathBuf,
    /// Dossier du réseau (vide pour un journal posé à la racine).
    pub network: String,
    /// Nom du fichier sans `.log`.
    pub window: String,
    pub size: u64,
    /// Dernière modification (ms depuis 1970).
    pub modified_ms: i64,
}

/// Dossier des journaux (réglage, sinon `<données>/logs`).
pub fn log_folder(client: &Client) -> PathBuf {
    client.settings().logging.folder.clone().unwrap_or_else(|| client.data_dir().join("logs"))
}

fn entry(path: PathBuf, network: &str) -> Option<LogEntry> {
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || path.extension().is_none_or(|e| !e.eq_ignore_ascii_case("log")) {
        return None;
    }
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let window = path.file_stem()?.to_string_lossy().into_owned();
    Some(LogEntry { path, network: network.to_string(), window, size: meta.len(), modified_ms })
}

/// Journaux de `<dossier>/<réseau>/<fenêtre>.log` (et ceux posés à la racine), les plus récents d'abord.
pub fn scan(folder: &Path) -> Vec<LogEntry> {
    let mut v = Vec::new();
    let Ok(rd) = std::fs::read_dir(folder) else { return v };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            let net = e.file_name().to_string_lossy().into_owned();
            if let Ok(sub) = std::fs::read_dir(&p) {
                v.extend(sub.flatten().filter_map(|f| entry(f.path(), &net)));
            }
        } else if let Some(x) = entry(p, "") {
            v.push(x);
        }
    }
    v.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms).then_with(|| a.path.cmp(&b.path)));
    v
}

/// Taille lisible (`12 KB`, `1.4 MB`).
fn size_text(lang: Lang, n: u64) -> String {
    if n < 1024 {
        tf(lang, "logs.size.b", &[("n", &n.to_string())])
    } else if n < 1024 * 1024 {
        tf(lang, "logs.size.kb", &[("n", &n.div_ceil(1024).to_string())])
    } else {
        let mb = format!("{:.1}", n as f64 / (1024.0 * 1024.0));
        let mb = if lang == Lang::Fr { mb.replace('.', ",") } else { mb };
        tf(lang, "logs.size.mb", &[("n", &mb)])
    }
}

// ---------------------------------------------------------------------------------------------------------------
// La liste.

const LIST: u32 = 2;
const VIEW: u32 = 3;
const OPEN_DIR: u32 = 4;
const REFRESH: u32 = 5;
const CLOSE: u32 = 6;
const FOLDER_L: u32 = 7;
const EMPTY: u32 = 8;

const W: i32 = 540;
const H: i32 = 380;

/// Colonnes de la liste : texte et largeur.
const COLS: [(&str, i32); 4] = [("logs.col.network", 120), ("logs.col.window", 170), ("logs.col.size", 70), ("logs.col.date", 128)];

/// Outils › Journaux…
pub struct LogFiles {
    folder: PathBuf,
    entries: Vec<LogEntry>,
}

impl LogFiles {
    pub fn new(client: &Client) -> LogFiles {
        LogFiles { folder: log_folder(client), entries: Vec::new() }
    }

    fn fill(&mut self, view: &mut DialogView, cx: &Cx) {
        self.entries = scan(&self.folder);
        let rows: Vec<String> = self
            .entries
            .iter()
            .map(|e| {
                let date = alpirc_core::timeutil::format_time(e.modified_ms, cx.utc_offset, "dd/mm/yyyy HH:nn");
                row(&[e.network.clone(), e.window.clone(), size_text(cx.lang, e.size), date])
            })
            .collect();
        view.set_items(LIST, &rows);
        let has = !rows.is_empty();
        select_row(view, LIST, has.then_some(0));
        view.set_visible(EMPTY, !has);
        self.update(view);
    }

    fn update(&self, view: &mut DialogView) {
        view.set_enabled(VIEW, sel(view, LIST).is_some());
    }

    fn view_selected(&self, view: &DialogView, cx: &mut Cx) {
        if let Some(e) = sel(view, LIST).and_then(|i| self.entries.get(i)) {
            cx.actions.push(Action::Open(Box::new(LogViewer::new(e.clone()))));
        }
    }
}

impl DialogCtl for LogFiles {
    fn kind(&self) -> Option<String> {
        Some("logs".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let folder = tf(lang, "logs.folder", &[("path", &self.folder.display().to_string())]);
        let cols = COLS.map(|(k, _)| t(lang, k));
        let lh = H - 2 * MARGIN - LABEL_H - 6 - BTN_H - 10;
        let by = H - MARGIN - BTN_H;
        let bx = |k: i32| MARGIN + k * (BTN_W + 26 + 6);
        let c = vec![
            text(FOLDER_L, (MARGIN, MARGIN, W - 2 * MARGIN, LABEL_H), &folder).style("nowrap"),
            list_view(LIST, (MARGIN, MARGIN + LABEL_H + 6, W - 2 * MARGIN, lh), &cols),
            text(EMPTY, (MARGIN + 12, MARGIN + LABEL_H + 6 + 28, W - 2 * MARGIN - 24, 32), &t(lang, "logs.empty")),
            button(VIEW, (bx(0), by, BTN_W + 26, BTN_H), &t(lang, "logs.view")).style("default"),
            button(OPEN_DIR, (bx(1), by, BTN_W + 26, BTN_H), &t(lang, "logs.open_folder")),
            button(REFRESH, (bx(2), by, BTN_W + 26, BTN_H), &t(lang, "logs.refresh")),
            button(CLOSE, (W - MARGIN - BTN_W, by, BTN_W, BTN_H), &t(lang, "dlg.close")).style("cancel"),
        ];
        spec(&t(lang, "logs.title"), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let lang = cx.lang;
        view.set_columns(LIST, &COLS.map(|(k, w)| (t(lang, k), w)));
        self.fill(view, cx);
        view.set_focus(LIST);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::SelectionChanged(LIST) => self.update(view),
            DialogMsg::Clicked(VIEW) | DialogMsg::DoubleClicked(LIST) => self.view_selected(view, cx),
            DialogMsg::Clicked(OPEN_DIR) => {
                // Le dossier d'un journal choisi, sinon celui des journaux (créé pour que le système l'ouvre).
                let entry = sel(view, LIST).and_then(|i| self.entries.get(i));
                let dir = entry.and_then(|e| e.path.parent().map(Path::to_path_buf));
                let dir = dir.unwrap_or_else(|| {
                    let _ = std::fs::create_dir_all(&self.folder);
                    self.folder.clone()
                });
                cx.actions.push(Action::OpenPath(dir));
            }
            DialogMsg::Clicked(REFRESH) => self.fill(view, cx),
            DialogMsg::Cancel | DialogMsg::Clicked(CLOSE) => return Outcome::Close,
            _ => {}
        }
        Outcome::Keep
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Lecture d'un journal.

const TEXT: u32 = 2;
const FIND_L: u32 = 3;
const FIND: u32 = 4;
const NEXT: u32 = 5;
const PREV: u32 = 6;
const V_OPEN_DIR: u32 = 7;
const V_CLOSE: u32 = 8;
const NOT_FOUND: u32 = 9;

const VW: i32 = 680;
const VH: i32 = 480;

/// Un journal ouvert en lecture seule.
pub struct LogViewer {
    entry: LogEntry,
}

impl LogViewer {
    pub fn new(entry: LogEntry) -> LogViewer {
        LogViewer { entry }
    }

    /// Le texte montré : le fichier (UTF-8, sinon Latin-1), sans codes de couleur ; seulement la fin s'il est grand.
    fn content(&self, lang: Lang) -> String {
        let bytes = match std::fs::read(&self.entry.path) {
            Ok(b) => b,
            Err(e) => return tf(lang, "logs.read_error", &[("error", &e.to_string())]),
        };
        let (cut, bytes) = if bytes.len() > MAX_SHOWN {
            let start = bytes.len() - MAX_SHOWN;
            // Au début d'une ligne.
            let start = bytes[start..].iter().position(|&b| b == b'\n').map(|i| start + i + 1).unwrap_or(start);
            (start, &bytes[start..])
        } else {
            (0, &bytes[..])
        };
        let text = match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => bytes.iter().map(|&b| b as char).collect(),
        };
        let text = alpirc_proto::format::strip(&text).replace("\r\n", "\n");
        if cut > 0 {
            format!("{}\n{}", tf(lang, "logs.cut", &[("size", &size_text(lang, cut as u64))]), text)
        } else {
            text
        }
    }

    fn find(&self, view: &mut DialogView, forward: bool) {
        let needle = text_of(view, FIND);
        if needle.is_empty() {
            return;
        }
        let found = match view.widget_mut(TEXT) {
            Some(Widget::TextArea(ta)) => ta.find_next(&needle, forward, false),
            _ => false,
        };
        view.set_visible(NOT_FOUND, !found);
    }
}

impl DialogCtl for LogViewer {
    fn kind(&self) -> Option<String> {
        Some(format!("log:{}", self.entry.path.display()))
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let title = if self.entry.network.is_empty() {
            tf(lang, "logs.view_title_root", &[("win", &self.entry.window)])
        } else {
            tf(lang, "logs.view_title", &[("win", &self.entry.window), ("net", &self.entry.network)])
        };
        let by = VH - MARGIN - BTN_H;
        let fy = by - 6 - EDIT_H;
        let c = vec![
            text_area(TEXT, (MARGIN, MARGIN, VW - 2 * MARGIN, fy - 8 - MARGIN), "").style("read"),
            field_label(FIND_L, MARGIN, fy, 70, &t(lang, "logs.find")),
            edit(FIND, (MARGIN + 74, fy, 250, EDIT_H), ""),
            button(NEXT, (MARGIN + 74 + 256, fy - 1, BTN_W + 10, BTN_H), &t(lang, "find.next")).style("default"),
            button(PREV, (MARGIN + 74 + 256 + BTN_W + 16, fy - 1, BTN_W + 10, BTN_H), &t(lang, "find.prev")),
            text(NOT_FOUND, (MARGIN + 74, by + 4, 300, LABEL_H), &t(lang, "logs.not_found")).style("hide"),
            button(V_OPEN_DIR, (VW - MARGIN - 2 * BTN_W - 32, by, BTN_W + 26, BTN_H), &t(lang, "logs.open_folder")),
            button(V_CLOSE, (VW - MARGIN - BTN_W, by, BTN_W, BTN_H), &t(lang, "dlg.close")).style("cancel"),
        ];
        spec(&title, VW, VH, c)
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let text = self.content(cx.lang);
        view.set_text(TEXT, &text);
        // À la fin, comme une fenêtre de discussion.
        if let Some(Widget::TextArea(ta)) = view.widget_mut(TEXT) {
            let last = ta.line_count().saturating_sub(1);
            ta.goto_line(last);
        }
        view.set_focus(FIND);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Clicked(NEXT) | DialogMsg::Ok(_) => self.find(view, true),
            DialogMsg::Clicked(PREV) => self.find(view, false),
            DialogMsg::Changed(FIND) => view.set_visible(NOT_FOUND, false),
            DialogMsg::Clicked(V_OPEN_DIR) => {
                if let Some(dir) = self.entry.path.parent() {
                    cx.actions.push(Action::OpenPath(dir.to_path_buf()));
                }
            }
            DialogMsg::Cancel | DialogMsg::Clicked(V_CLOSE) => return Outcome::Close,
            _ => {}
        }
        Outcome::Keep
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};

    fn write(dir: &Path, rel: &str, text: &str) -> PathBuf {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    #[test]
    fn lists_logs_and_opens_one_read_only() {
        let mut k = Kit::new("logs");
        let logs = k.dir.join("logs");
        write(&logs, "undernet/#lyon.log", "Session Start: x\n[10:00] <Elisa> bonjour \u{3}4rouge\u{3}\n[10:01] <Nemo> salut\n");
        write(&logs, "libera/elisa.log", "[09:00] <Elisa> coucou\n");
        write(&logs, "notes.txt", "pas un journal");
        let found = scan(&logs);
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|e| e.network == "undernet" && e.window == "#lyon"));

        let mut d = LogFiles::new(&k.client);
        assert_translated(&d);
        let (mut v, _) = k.open(&mut d);
        assert_eq!(v.items(LIST).len(), 2);
        assert!(!v.visible(EMPTY));
        let i = d.entries.iter().position(|e| e.window == "#lyon").unwrap();
        v.set_selection(LIST, &[i]);
        let (_, a) = k.send(&mut d, &mut v, DialogMsg::DoubleClicked(LIST));
        let Some(Action::Open(mut viewer)) = a.into_iter().next() else { panic!("pas de lecteur") };
        assert_eq!(viewer.kind().as_deref(), Some(format!("log:{}", logs.join("undernet/#lyon.log").display()).as_str()));
        let (mut vv, _) = k.open(viewer.as_mut());
        let text = vv.text(TEXT).unwrap();
        assert!(text.contains("<Elisa> bonjour rouge"), "codes retirés : {text}");
        // Recherche.
        vv.set_text(FIND, "nemo");
        k.send(viewer.as_mut(), &mut vv, DialogMsg::Clicked(NEXT));
        assert!(!vv.visible(NOT_FOUND));
        vv.set_text(FIND, "absent");
        k.send(viewer.as_mut(), &mut vv, DialogMsg::Clicked(NEXT));
        assert!(vv.visible(NOT_FOUND));
        // Ouvrir le dossier.
        let (_, a) = k.send(viewer.as_mut(), &mut vv, DialogMsg::Clicked(V_OPEN_DIR));
        assert!(matches!(a.first(), Some(Action::OpenPath(p)) if p.ends_with("undernet")));
    }

    #[test]
    fn empty_folder_and_big_logs() {
        let mut k = Kit::new("logs2");
        let mut d = LogFiles::new(&k.client);
        let (mut v, _) = k.open(&mut d);
        assert!(v.visible(EMPTY) && !v.enabled(VIEW));
        let (_, a) = k.send(&mut d, &mut v, DialogMsg::Clicked(OPEN_DIR));
        assert!(matches!(a.first(), Some(Action::OpenPath(p)) if p.ends_with("logs") && p.is_dir()));
        // Un gros journal : seulement la fin, à partir d'une ligne entière.
        let line = "[12:00] <Bot> ".to_string() + &"x".repeat(80) + "\n";
        let big = line.repeat(MAX_SHOWN / line.len() + 100) + "[23:59] <Bot> fin\n";
        let p = write(&k.dir, "logs/net/#gros.log", &big);
        let e = scan(&k.dir.join("logs")).into_iter().find(|e| e.path == p).unwrap();
        let shown = LogViewer::new(e).content(Lang::En);
        assert!(shown.len() <= MAX_SHOWN + 200);
        assert!(shown.lines().nth(1).unwrap().starts_with("[12:00]"));
        assert!(shown.ends_with("<Bot> fin\n"));
        assert_eq!(size_text(Lang::Fr, 1536), "2 Ko");
        assert_eq!(size_text(Lang::En, 3 * 1024 * 1024 / 2), "1.5 MB");
    }
}
