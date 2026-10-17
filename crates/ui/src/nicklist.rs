//! Liste des pseudos d'un canal : préfixes de mode, absents en gris, couleur par pseudo, sélection multiple
//! (Ctrl, Maj), double-clic et clic droit avec les pseudos choisis, recherche en tapant.
//!
//! L'ordre est celui que donne l'application (déjà trié : opérateurs, voix, autres).

use crate::canvas::Canvas;
use crate::geom::{Point, Rect};
use crate::input::{Input, Platform};
use crate::text::TextStyle;
use crate::widgets::listbox::{ListBox, ListItem, ListMsg, SelectMode};
use crate::Res;

/// Un pseudo de la liste.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NickEntry {
    /// Préfixe de mode : `"@"`, `"+"`, `""`…
    pub prefix: String,
    pub nick: String,
    /// Absent (affiché en gris).
    pub away: bool,
    /// Couleur imposée (`0xRRGGBB`).
    pub color: Option<u32>,
}

impl NickEntry {
    pub fn new(prefix: &str, nick: &str) -> NickEntry {
        NickEntry { prefix: prefix.to_string(), nick: nick.to_string(), ..Default::default() }
    }
}

/// Message de la liste des pseudos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NickMsg {
    SelectionChanged,
    /// Double-clic (ou Entrée) sur un pseudo.
    DoubleClick(String),
    /// Clic droit : position et pseudos choisis (l'application ouvre son menu).
    RightClick { pos: Point, nicks: Vec<String> },
}

/// Liste des pseudos.
#[derive(Clone, Debug)]
pub struct NickList {
    entries: Vec<NickEntry>,
    list: ListBox,
    /// Cadre creux autour de la liste.
    pub border: bool,
    pub focused: bool,
}

impl Default for NickList {
    fn default() -> NickList {
        NickList::new()
    }
}

impl NickList {
    pub fn new() -> NickList {
        let mut list = ListBox::new(SelectMode::Extended);
        list.style = TextStyle::CHAT;
        NickList { entries: Vec::new(), list, border: true, focused: false }
    }

    /// Pseudos affichés.
    pub fn entries(&self) -> &[NickEntry] {
        &self.entries
    }

    /// Remplace les pseudos ; la sélection suit les pseudos (par nom), le défilement est gardé.
    pub fn set_entries(&mut self, entries: Vec<NickEntry>) {
        let selected: Vec<String> = self.selected_nicks();
        let cursor_nick = self.entries.get(self.list.sel.cursor).map(|e| e.nick.clone());
        let top = self.list.top;
        self.entries = entries;
        self.list.items = self.entries.iter().map(|e| ListItem::new(&format!("{}{}", e.prefix, e.nick))).collect();
        self.list.sel.resize(0);
        self.list.sel.resize(self.entries.len());
        for (i, e) in self.entries.iter().enumerate() {
            if selected.contains(&e.nick) {
                self.list.sel.set(i, true);
            }
            if cursor_nick.as_ref() == Some(&e.nick) {
                self.list.sel.cursor = i;
            }
        }
        self.list.top = top.min(self.entries.len().saturating_sub(1));
    }

    /// Pseudos choisis.
    pub fn selected_nicks(&self) -> Vec<String> {
        self.list.sel.selected().into_iter().filter_map(|i| self.entries.get(i).map(|e| e.nick.clone())).collect()
    }

    /// Choisit un seul pseudo.
    pub fn select_nick(&mut self, nick: &str) {
        if let Some(i) = self.entries.iter().position(|e| e.nick.eq_ignore_ascii_case(nick)) {
            self.list.sel.select_only(i);
        }
    }

    pub fn clear_selection(&mut self) {
        self.list.sel.clear();
    }

    fn sync_colors(&self, res: &Res) -> ListBox {
        let t = &res.theme;
        let mut l = self.list.clone();
        l.focused = self.focused;
        l.border = self.border;
        l.colors = Some((t.color(t.nicklist_bg), t.color(t.nicklist_fg)));
        for (item, e) in l.items.iter_mut().zip(&self.entries) {
            item.color = if e.away { Some(t.color(t.nicklist_away)) } else { e.color };
        }
        l
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        self.sync_colors(res).paint(canvas, res, rect);
    }

    pub fn event(&mut self, input: &Input, rect: Rect, res: &Res, platform: &mut dyn Platform) -> Option<NickMsg> {
        self.list.focused = self.focused;
        self.list.border = self.border;
        let msg = self.list.event(input, rect, res, platform);
        self.focused = self.list.focused;
        match msg? {
            ListMsg::SelectionChanged => Some(NickMsg::SelectionChanged),
            ListMsg::DoubleClicked(i) => self.entries.get(i).map(|e| NickMsg::DoubleClick(e.nick.clone())),
            ListMsg::RightClicked(_, pos) => Some(NickMsg::RightClick { pos, nicks: self.selected_nicks() }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Key, Mods, MouseButton, TestPlatform};

    #[test]
    fn selection_follows_nicks_and_messages() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let rect = Rect::new(0, 0, 120, 200);
        let mut nl = NickList::new();
        nl.set_entries(vec![NickEntry::new("@", "Alice"), NickEntry::new("+", "Bob"), NickEntry::new("", "Zoe")]);
        // Ligne de 16 px, cadre de 2 px.
        nl.event(&Input::click(10, 2 + 16 + 4), rect, &res, &mut p);
        nl.event(&Input::release(10, 2 + 16 + 4), rect, &res, &mut p);
        let ctrl = Input::MouseDown { pos: Point::new(10, 2 + 32 + 4), button: MouseButton::Left, mods: Mods::CTRL, clicks: 1 };
        nl.event(&ctrl, rect, &res, &mut p);
        assert_eq!(nl.selected_nicks(), vec!["Bob", "Zoe"]);
        // Un nouveau venu : la sélection reste sur les mêmes pseudos.
        nl.set_entries(vec![NickEntry::new("@", "Alice"), NickEntry::new("@", "Arthur"), NickEntry::new("+", "Bob"), NickEntry::new("", "Zoe")]);
        assert_eq!(nl.selected_nicks(), vec!["Bob", "Zoe"]);
        let dbl = Input::MouseDown { pos: Point::new(10, 2 + 4), button: MouseButton::Left, mods: Mods::NONE, clicks: 2 };
        assert_eq!(nl.event(&dbl, rect, &res, &mut p), Some(NickMsg::DoubleClick("Alice".into())));
        let rc = Input::MouseDown { pos: Point::new(10, 2 + 48 + 4), button: MouseButton::Right, mods: Mods::NONE, clicks: 1 };
        assert!(matches!(nl.event(&rc, rect, &res, &mut p), Some(NickMsg::RightClick { nicks, .. }) if nicks == vec!["Zoe".to_string()] || nicks.contains(&"Zoe".to_string())));
        nl.focused = true;
        p.now = 10_000;
        nl.event(&Input::key(Key::Char('b'), Mods::NONE), rect, &res, &mut p);
        assert_eq!(nl.selected_nicks(), vec!["Bob"], "recherche en tapant sans le préfixe");
    }
}
