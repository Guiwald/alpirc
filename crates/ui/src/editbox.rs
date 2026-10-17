//! La ligne de saisie de mIRC : historique (Haut/Bas), complétion par Tab (demandée à l'application), codes
//! de mise en forme (Ctrl+B, K, U, I, R, O) affichés en lettres inversées, palette Ctrl+K, collage de
//! plusieurs lignes signalé à l'application, composition de la méthode de saisie.

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect};
use crate::input::{Input, Key, MouseButton, Platform};
use crate::text::{Align, TextStyle};
use crate::widgets::textedit::{TextEdit, TextEditMsg};
use crate::Res;

/// Message de la ligne de saisie.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditMsg {
    /// Entrée : le texte (déjà ajouté à l'historique ; le champ est vidé).
    Submit(String),
    /// Ctrl+Entrée : envoyer tel quel, sans interpréter de commande.
    SubmitPlain(String),
    /// Collage de plusieurs lignes : l'application demande confirmation puis les envoie (rien n'est inséré).
    PasteLines(Vec<String>),
    /// Tab : compléter le mot qui commence à l'octet `word_start`. `cycle` : Tab répété (proposer la
    /// complétion suivante pour le même mot d'origine). L'application répond par [`Editbox::complete`].
    TabComplete { word_start: usize, word: String, cycle: bool },
    /// Le texte a changé.
    Changed,
}

/// Palette Ctrl+K : les 16 couleurs de base de mIRC, numérotées.
#[derive(Clone, Debug)]
pub struct ColorPicker {
    /// Coin bas-gauche (la palette s'ouvre au-dessus de la ligne de saisie).
    pub anchor: Point,
    hover: Option<u8>,
    rect: Rect,
}

/// Message de la palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerMsg {
    /// Couleur choisie (clic droit : couleur de fond).
    Picked { index: u8, background: bool },
    Closed,
}

const SWATCH: i32 = 20;

impl ColorPicker {
    /// Palette au-dessus de `anchor` (gardée dans `bounds`).
    pub fn new(anchor: Point, bounds: Rect) -> ColorPicker {
        let w = 8 * (SWATCH + 2) + 2 + 6;
        let h = 2 * (SWATCH + 2) + 2 + 6;
        let rect = Rect::new(anchor.x, anchor.y - h, w, h).clamp_inside(&bounds);
        ColorPicker { anchor, hover: None, rect }
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    fn swatch(&self, i: u8) -> Rect {
        let (col, row) = (i as i32 % 8, i as i32 / 8);
        Rect::new(self.rect.x + 4 + col * (SWATCH + 2), self.rect.y + 4 + row * (SWATCH + 2), SWATCH, SWATCH)
    }

    /// Couleur sous un point.
    pub fn index_at(&self, p: Point) -> Option<u8> {
        (0..16u8).find(|&i| self.swatch(i).contains(p))
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res) {
        let t = &res.theme;
        canvas.fill(self.rect, t.face);
        canvas.bevel(self.rect, Bevel::Window, t);
        for i in 0..16u8 {
            let r = self.swatch(i);
            let c = t.palette[i as usize];
            canvas.fill(r.inset(2), c);
            if self.hover == Some(i) {
                canvas.bevel(r, Bevel::RaisedThin, t);
                canvas.frame(r.inset(1), t.dark);
            } else {
                canvas.bevel(r, Bevel::Sunken, t);
            }
            // Numéro en contraste.
            let lum = ((c >> 16 & 0xFF) * 299 + (c >> 8 & 0xFF) * 587 + (c & 0xFF) * 114) / 1000;
            let fg = if lum > 140 { 0x000000 } else { 0xFFFFFF };
            res.fonts.draw_label(canvas, r.inset(2), &i.to_string(), TextStyle::UI, fg, Align::Center, false, None);
        }
    }

    pub fn event(&mut self, input: &Input, platform: &mut dyn Platform) -> Option<PickerMsg> {
        match input {
            Input::MouseMove { pos } => {
                let h = self.index_at(*pos);
                if h != self.hover {
                    self.hover = h;
                    platform.request_repaint();
                }
                None
            }
            Input::MouseDown { pos, button, .. } => match self.index_at(*pos) {
                Some(i) => Some(PickerMsg::Picked { index: i, background: *button == MouseButton::Right }),
                None if self.rect.contains(*pos) => None,
                None => Some(PickerMsg::Closed),
            },
            Input::Key { key: Key::Escape, .. } => Some(PickerMsg::Closed),
            _ => None,
        }
    }
}

/// État de la complétion par Tab en cours.
#[derive(Clone, Debug)]
struct TabState {
    word_start: usize,
    word: String,
    /// Fin du texte inséré par la dernière complétion.
    end: usize,
}

/// La ligne de saisie.
#[derive(Clone, Debug)]
pub struct Editbox {
    pub edit: TextEdit,
    /// Historique des lignes envoyées (la plus récente en dernier). L'application peut le précharger.
    pub history: Vec<String>,
    pub history_max: usize,
    pub focused: bool,
    hist_pos: Option<usize>,
    draft: String,
    picker: Option<ColorPicker>,
    tab: Option<TabState>,
}

impl Default for Editbox {
    fn default() -> Editbox {
        Editbox::new()
    }
}

impl Editbox {
    pub fn new() -> Editbox {
        let mut edit = TextEdit::new("");
        edit.style = TextStyle::CHAT;
        edit.show_codes = true;
        Editbox { edit, history: Vec::new(), history_max: 100, focused: false, hist_pos: None, draft: String::new(), picker: None, tab: None }
    }

    pub fn text(&self) -> &str {
        self.edit.text()
    }

    pub fn set_text(&mut self, s: &str) {
        self.edit.set_text(s);
        self.tab = None;
    }

    /// Insère du texte au curseur (par exemple après confirmation d'un collage).
    pub fn insert_text(&mut self, s: &str) {
        self.edit.core.insert(s);
    }

    /// Hauteur idéale.
    pub fn preferred_height(&self, res: &Res) -> i32 {
        self.edit.preferred_height(res)
    }

    /// Vrai si la palette Ctrl+K est ouverte.
    pub fn picker_open(&self) -> bool {
        self.picker.is_some()
    }

    /// Réponse à [`EditMsg::TabComplete`] : remplace le mot par `replacement` (curseur après).
    pub fn complete(&mut self, replacement: &str) {
        let Some(st) = self.tab.as_mut() else { return };
        let core = &mut self.edit.core;
        let end = st.end.min(core.text().len());
        core.select(st.word_start..end);
        core.insert(replacement);
        st.end = st.word_start + replacement.len();
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        let mut e = self.edit.clone();
        e.focused = self.focused;
        e.colors = Some((t.color(t.editbox_bg), t.color(t.editbox_fg)));
        e.paint(canvas, res, rect);
    }

    /// Dessine la palette Ctrl+K si elle est ouverte (à appeler après tout le reste).
    pub fn paint_popup(&self, canvas: &mut Canvas, res: &Res) {
        if let Some(p) = &self.picker {
            p.paint(canvas, res);
        }
    }

    fn submit_history(&mut self, text: &str) {
        if !text.is_empty() && self.history.last().map(String::as_str) != Some(text) {
            self.history.push(text.to_string());
            if self.history.len() > self.history_max {
                self.history.remove(0);
            }
        }
        self.hist_pos = None;
        self.draft.clear();
    }

    fn history_move(&mut self, up: bool, platform: &mut dyn Platform) {
        if self.history.is_empty() {
            return;
        }
        let n = self.history.len();
        let next = match (self.hist_pos, up) {
            (None, true) => {
                self.draft = self.edit.text().to_string();
                Some(n - 1)
            }
            (None, false) => return,
            (Some(p), true) => Some(p.saturating_sub(1)),
            (Some(p), false) if p + 1 < n => Some(p + 1),
            (Some(_), false) => None,
        };
        self.hist_pos = next;
        let s = match next {
            Some(p) => self.history[p].clone(),
            None => std::mem::take(&mut self.draft),
        };
        self.edit.set_text(&s);
        platform.request_repaint();
    }

    fn insert_code(&mut self, c: char) -> Option<EditMsg> {
        self.edit.core.insert(&c.to_string()).then_some(EditMsg::Changed)
    }

    /// `bounds` : zone où la palette peut s'ouvrir.
    pub fn event(&mut self, input: &Input, rect: Rect, bounds: Rect, res: &Res, platform: &mut dyn Platform) -> Option<EditMsg> {
        // La palette passe d'abord.
        if let Some(p) = self.picker.as_mut() {
            let mine = matches!(input, Input::MouseMove { .. } | Input::MouseDown { .. } | Input::Key { key: Key::Escape, .. });
            if mine {
                let m = p.event(input, platform);
                match m {
                    Some(PickerMsg::Picked { index, background }) => {
                        self.picker = None;
                        platform.request_repaint();
                        let s = if background { format!(",{index:02}") } else { format!("{index:02}") };
                        self.edit.core.insert(&s);
                        self.edit.ensure_caret_visible(res, rect, platform);
                        return Some(EditMsg::Changed);
                    }
                    Some(PickerMsg::Closed) => {
                        self.picker = None;
                        platform.request_repaint();
                        if matches!(input, Input::Key { .. }) {
                            return None;
                        }
                    }
                    None => return None,
                }
            } else if matches!(input, Input::Key { .. } | Input::Text(_)) {
                // Taper au clavier ferme la palette (le chiffre tapé suit le code couleur).
                self.picker = None;
                platform.request_repaint();
            }
        }
        if let Input::Key { key, mods, .. } = input {
            if self.focused {
                if *key != Key::Tab {
                    self.tab = None;
                }
                match key {
                    Key::Char(c) if mods.command() && !mods.alt && !mods.shift => {
                        let code = match c.to_ascii_lowercase() {
                            'b' => Some('\x02'),
                            'k' => Some('\x03'),
                            'u' => Some('\x1F'),
                            'i' => Some('\x1D'),
                            'r' => Some('\x16'),
                            'o' => Some('\x0F'),
                            _ => None,
                        };
                        if let Some(code) = code {
                            let m = self.insert_code(code);
                            self.edit.ensure_caret_visible(res, rect, platform);
                            platform.request_repaint();
                            if code == '\x03' {
                                let tr = self.edit.text_rect(rect);
                                let anchor = Point::new(tr.x, rect.y);
                                self.picker = Some(ColorPicker::new(anchor, bounds));
                            }
                            return m;
                        }
                        if c.eq_ignore_ascii_case(&'v') {
                            return self.paste(res, rect, platform);
                        }
                    }
                    Key::Insert if mods.shift => return self.paste(res, rect, platform),
                    Key::Up | Key::Down if !mods.command() => {
                        self.history_move(*key == Key::Up, platform);
                        return None;
                    }
                    Key::Tab if !mods.command() && !mods.alt => {
                        let core = &self.edit.core;
                        let caret = core.caret();
                        if let Some(st) = &self.tab {
                            if st.end == caret {
                                return Some(EditMsg::TabComplete { word_start: st.word_start, word: st.word.clone(), cycle: true });
                            }
                        }
                        let text = core.text();
                        let word_start = text[..caret].rfind(char::is_whitespace).map_or(0, |i| i + text[i..].chars().next().map_or(1, |c| c.len_utf8()));
                        let word = text[word_start..caret].to_string();
                        self.tab = Some(TabState { word_start, word: word.clone(), end: caret });
                        return Some(EditMsg::TabComplete { word_start, word, cycle: false });
                    }
                    Key::Enter => {
                        let text = self.edit.text().to_string();
                        self.submit_history(&text);
                        self.edit.set_text("");
                        platform.request_repaint();
                        return Some(if mods.command() { EditMsg::SubmitPlain(text) } else { EditMsg::Submit(text) });
                    }
                    _ => {}
                }
            }
        }
        if let Input::Text(s) = input {
            if self.focused && (s.contains('\n') || s.contains('\r')) {
                return Some(EditMsg::PasteLines(split_lines(s)));
            }
        }
        self.edit.focused = self.focused;
        let m = self.edit.event(input, rect, res, platform);
        self.focused = self.edit.focused;
        match m? {
            TextEditMsg::Changed => {
                self.hist_pos = None;
                Some(EditMsg::Changed)
            }
            TextEditMsg::Submit => None,
        }
    }

    fn paste(&mut self, res: &Res, rect: Rect, platform: &mut dyn Platform) -> Option<EditMsg> {
        let s = platform.clipboard_get()?;
        if s.contains('\n') || s.contains('\r') {
            let lines = split_lines(&s);
            if lines.len() > 1 {
                return Some(EditMsg::PasteLines(lines));
            }
            self.edit.core.insert(lines.first().map(String::as_str).unwrap_or(""));
        } else {
            self.edit.core.insert(&s);
        }
        self.edit.ensure_caret_visible(res, rect, platform);
        platform.request_repaint();
        Some(EditMsg::Changed)
    }
}

/// Coupe un texte collé en lignes (sans les lignes vides de fin).
fn split_lines(s: &str) -> Vec<String> {
    let mut v: Vec<String> = s.replace("\r\n", "\n").replace('\r', "\n").split('\n').map(str::to_string).collect();
    while v.last().is_some_and(|l| l.is_empty()) {
        v.pop();
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Mods, TestPlatform};

    fn setup() -> (Res, TestPlatform, Editbox, Rect, Rect) {
        let res = crate::test_res();
        let mut eb = Editbox::new();
        eb.focused = true;
        (res, TestPlatform::new(), eb, Rect::new(0, 400, 500, 22), Rect::new(0, 0, 500, 422))
    }

    fn typ(eb: &mut Editbox, s: &str, rect: Rect, b: Rect, res: &Res, p: &mut TestPlatform) {
        for c in s.chars() {
            eb.event(&Input::key(Key::Char(c), Mods::NONE), rect, b, res, p);
        }
    }

    #[test]
    fn history_and_submit() {
        let (res, mut p, mut eb, r, b) = setup();
        typ(&mut eb, "salut", r, b, &res, &mut p);
        assert_eq!(eb.event(&Input::key(Key::Enter, Mods::NONE), r, b, &res, &mut p), Some(EditMsg::Submit("salut".into())));
        assert_eq!(eb.text(), "");
        typ(&mut eb, "/join #lyon", r, b, &res, &mut p);
        eb.event(&Input::key(Key::Enter, Mods::NONE), r, b, &res, &mut p);
        typ(&mut eb, "brouillon", r, b, &res, &mut p);
        eb.event(&Input::key(Key::Up, Mods::NONE), r, b, &res, &mut p);
        assert_eq!(eb.text(), "/join #lyon");
        eb.event(&Input::key(Key::Up, Mods::NONE), r, b, &res, &mut p);
        assert_eq!(eb.text(), "salut");
        eb.event(&Input::key(Key::Up, Mods::NONE), r, b, &res, &mut p);
        assert_eq!(eb.text(), "salut");
        eb.event(&Input::key(Key::Down, Mods::NONE), r, b, &res, &mut p);
        eb.event(&Input::key(Key::Down, Mods::NONE), r, b, &res, &mut p);
        assert_eq!(eb.text(), "brouillon", "le brouillon revient");
        assert_eq!(eb.event(&Input::key(Key::Enter, Mods::CTRL), r, b, &res, &mut p), Some(EditMsg::SubmitPlain("brouillon".into())));
    }

    #[test]
    fn control_codes_and_picker() {
        let (res, mut p, mut eb, r, b) = setup();
        eb.event(&Input::key(Key::Char('b'), Mods::CTRL), r, b, &res, &mut p);
        typ(&mut eb, "gras", r, b, &res, &mut p);
        eb.event(&Input::key(Key::Char('b'), Mods::META), r, b, &res, &mut p);
        eb.event(&Input::key(Key::Char('u'), Mods::CTRL), r, b, &res, &mut p);
        eb.event(&Input::key(Key::Char('i'), Mods::CTRL), r, b, &res, &mut p);
        eb.event(&Input::key(Key::Char('r'), Mods::CTRL), r, b, &res, &mut p);
        eb.event(&Input::key(Key::Char('o'), Mods::CTRL), r, b, &res, &mut p);
        assert_eq!(eb.text(), "\x02gras\x02\x1F\x1D\x16\x0F");
        eb.event(&Input::key(Key::Char('k'), Mods::CTRL), r, b, &res, &mut p);
        assert!(eb.picker_open());
        let pr = eb.picker.as_ref().unwrap();
        assert!(pr.rect().bottom() <= r.y, "la palette s'ouvre au-dessus");
        let red = pr.swatch(4).center();
        assert_eq!(eb.event(&Input::click(red.x, red.y), r, b, &res, &mut p), Some(EditMsg::Changed));
        assert!(!eb.picker_open());
        assert!(eb.text().ends_with("\x0304"));
        // Taper un chiffre ferme la palette et suit le code.
        eb.event(&Input::key(Key::Char('k'), Mods::CTRL), r, b, &res, &mut p);
        typ(&mut eb, "12", r, b, &res, &mut p);
        assert!(!eb.picker_open());
        assert!(eb.text().ends_with("\x0312"));
    }

    #[test]
    fn tab_completion_and_paste() {
        let (res, mut p, mut eb, r, b) = setup();
        typ(&mut eb, "salut gui", r, b, &res, &mut p);
        let m = eb.event(&Input::key(Key::Tab, Mods::NONE), r, b, &res, &mut p);
        assert_eq!(m, Some(EditMsg::TabComplete { word_start: 6, word: "gui".into(), cycle: false }));
        eb.complete("Guiwald");
        assert_eq!(eb.text(), "salut Guiwald");
        let m = eb.event(&Input::key(Key::Tab, Mods::NONE), r, b, &res, &mut p);
        assert_eq!(m, Some(EditMsg::TabComplete { word_start: 6, word: "gui".into(), cycle: true }));
        eb.complete("Guillaume");
        assert_eq!(eb.text(), "salut Guillaume");
        p.clipboard = Some("ligne 1\r\nligne 2\n".into());
        assert_eq!(eb.event(&Input::key(Key::Char('v'), Mods::CTRL), r, b, &res, &mut p), Some(EditMsg::PasteLines(vec!["ligne 1".into(), "ligne 2".into()])));
        assert_eq!(eb.text(), "salut Guillaume", "rien n'est inséré");
        p.clipboard = Some(" !".into());
        eb.event(&Input::key(Key::Char('v'), Mods::CTRL), r, b, &res, &mut p);
        assert_eq!(eb.text(), "salut Guillaume !");
    }
}
