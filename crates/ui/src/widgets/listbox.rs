//! Liste simple : sélection unique, multiple (clic qui bascule) ou étendue (Ctrl/Maj), navigation au
//! clavier, recherche en tapant, défilement, couleur par élément.

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect};
use crate::icons::IconId;
use crate::input::{Input, Key, Mods, MouseButton, Platform};
use crate::text::TextStyle;
use crate::Res;

use super::scrollbar::{self, Orientation, ScrollBar};

/// Mode de sélection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectMode {
    /// Un seul élément.
    #[default]
    Single,
    /// Chaque clic bascule l'élément (style `multsel`).
    Multi,
    /// Ctrl+clic bascule, Maj+clic étend (style `extsel`).
    Extended,
}

/// Sélection d'une liste : éléments choisis, élément courant (focus) et ancre des sélections étendues.
#[derive(Clone, Debug, Default)]
pub struct ListSel {
    pub mode: SelectMode,
    selected: Vec<bool>,
    /// Élément courant (rectangle de focus).
    pub cursor: usize,
    anchor: usize,
}

impl ListSel {
    pub fn new(mode: SelectMode) -> ListSel {
        ListSel { mode, ..Default::default() }
    }

    /// Ajuste au nombre d'éléments (la sélection des éléments restants est gardée).
    pub fn resize(&mut self, n: usize) {
        self.selected.resize(n, false);
        if n == 0 {
            self.cursor = 0;
            self.anchor = 0;
        } else {
            self.cursor = self.cursor.min(n - 1);
            self.anchor = self.anchor.min(n - 1);
        }
    }

    pub fn len(&self) -> usize {
        self.selected.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    pub fn is_selected(&self, i: usize) -> bool {
        self.selected.get(i).copied().unwrap_or(false)
    }

    /// Index des éléments choisis, dans l'ordre.
    pub fn selected(&self) -> Vec<usize> {
        self.selected.iter().enumerate().filter(|x| *x.1).map(|x| x.0).collect()
    }

    /// Premier élément choisi.
    pub fn first(&self) -> Option<usize> {
        self.selected.iter().position(|&s| s)
    }

    pub fn clear(&mut self) {
        self.selected.fill(false);
    }

    /// Choisit ou retire un élément.
    pub fn set(&mut self, i: usize, on: bool) {
        if i < self.selected.len() {
            if on && self.mode == SelectMode::Single {
                self.selected.fill(false);
            }
            self.selected[i] = on;
        }
    }

    /// Ne choisit que cet élément (et y met le focus).
    pub fn select_only(&mut self, i: usize) {
        self.selected.fill(false);
        if i < self.selected.len() {
            self.selected[i] = true;
            self.cursor = i;
            self.anchor = i;
        }
    }

    fn range(&mut self, a: usize, b: usize, keep: bool) {
        if !keep {
            self.selected.fill(false);
        }
        for i in a.min(b)..=a.max(b) {
            if i < self.selected.len() {
                self.selected[i] = true;
            }
        }
    }

    /// Clic sur l'élément `i` ; renvoie vrai si la sélection a changé.
    pub fn click(&mut self, i: usize, mods: Mods) -> bool {
        if i >= self.selected.len() {
            return false;
        }
        let before = self.selected.clone();
        match self.mode {
            SelectMode::Single => self.select_only(i),
            SelectMode::Multi => {
                self.selected[i] = !self.selected[i];
                self.cursor = i;
                self.anchor = i;
            }
            SelectMode::Extended => {
                if mods.shift {
                    let a = self.anchor;
                    self.range(a, i, mods.command());
                    self.cursor = i;
                } else if mods.command() {
                    self.selected[i] = !self.selected[i];
                    self.cursor = i;
                    self.anchor = i;
                } else {
                    self.select_only(i);
                }
            }
        }
        before != self.selected
    }

    /// Navigation au clavier vers `i` ; renvoie vrai si la sélection a changé.
    pub fn move_to(&mut self, i: usize, mods: Mods) -> bool {
        if self.selected.is_empty() {
            return false;
        }
        let i = i.min(self.selected.len() - 1);
        let before = self.selected.clone();
        match self.mode {
            SelectMode::Single => self.select_only(i),
            SelectMode::Multi => self.cursor = i,
            SelectMode::Extended => {
                if mods.shift {
                    let a = self.anchor;
                    self.range(a, i, false);
                    self.cursor = i;
                } else if mods.command() {
                    self.cursor = i;
                } else {
                    self.select_only(i);
                }
            }
        }
        before != self.selected
    }

    /// Touches de navigation (`page` : éléments par page). `Some(changé)` si la touche a été traitée.
    pub fn key(&mut self, key: Key, mods: Mods, page: usize) -> Option<bool> {
        let n = self.selected.len();
        if n == 0 {
            return None;
        }
        let c = self.cursor;
        let page = page.max(1);
        let target = match key {
            Key::Up => c.saturating_sub(1),
            Key::Down => (c + 1).min(n - 1),
            Key::Home => 0,
            Key::End => n - 1,
            Key::PageUp => c.saturating_sub(page - 1),
            Key::PageDown => (c + page - 1).min(n - 1),
            Key::Char(' ') if self.mode != SelectMode::Single => {
                self.selected[c] = !self.selected[c];
                return Some(true);
            }
            _ => return None,
        };
        Some(self.move_to(target, mods))
    }
}

/// Un élément de liste.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListItem {
    pub text: String,
    /// Couleur du texte (sinon celle du thème).
    pub color: Option<u32>,
    pub icon: Option<IconId>,
    /// Valeur libre de l'appelant.
    pub data: u64,
}

impl ListItem {
    pub fn new(text: &str) -> ListItem {
        ListItem { text: text.to_string(), ..Default::default() }
    }
}

/// Message d'une liste.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListMsg {
    SelectionChanged,
    DoubleClicked(usize),
    /// Clic droit (sur un élément ou dans le vide), position de la souris.
    RightClicked(Option<usize>, Point),
}

/// Liste simple.
#[derive(Clone, Debug)]
pub struct ListBox {
    pub items: Vec<ListItem>,
    pub sel: ListSel,
    /// Premier élément visible.
    pub top: usize,
    pub enabled: bool,
    pub focused: bool,
    pub style: TextStyle,
    pub border: bool,
    /// Couleurs imposées (fond, texte).
    pub colors: Option<(u32, u32)>,
    /// Garder les éléments triés à l'ajout (style `sort`).
    pub sorted: bool,
    vbar: ScrollBar,
    find: String,
    find_at: i64,
    dragging: bool,
}

impl Default for ListBox {
    fn default() -> ListBox {
        ListBox::new(SelectMode::Single)
    }
}

impl ListBox {
    pub fn new(mode: SelectMode) -> ListBox {
        ListBox {
            items: Vec::new(),
            sel: ListSel::new(mode),
            top: 0,
            enabled: true,
            focused: false,
            style: TextStyle::UI,
            border: true,
            colors: None,
            sorted: false,
            vbar: ScrollBar::new(Orientation::Vertical),
            find: String::new(),
            find_at: 0,
            dragging: false,
        }
    }

    /// Remplace les éléments par des textes.
    pub fn set_items<S: AsRef<str>>(&mut self, items: &[S]) {
        self.items = items.iter().map(|s| ListItem::new(s.as_ref())).collect();
        if self.sorted {
            self.items.sort_by_key(|a| a.text.to_lowercase());
        }
        self.sel = ListSel::new(self.sel.mode);
        self.sel.resize(self.items.len());
        self.top = 0;
    }

    /// Ajoute un élément (à sa place si la liste est triée) ; renvoie son index.
    pub fn add(&mut self, item: ListItem) -> usize {
        let i = if self.sorted {
            let k = item.text.to_lowercase();
            self.items.partition_point(|x| x.text.to_lowercase() <= k)
        } else {
            self.items.len()
        };
        self.items.insert(i, item);
        self.sel.selected.insert(i, false);
        self.sel.resize(self.items.len());
        i
    }

    /// Retire un élément.
    pub fn remove(&mut self, i: usize) {
        if i < self.items.len() {
            self.items.remove(i);
            self.sel.selected.remove(i);
            self.sel.resize(self.items.len());
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.sel.resize(0);
        self.top = 0;
    }

    /// Premier élément choisi.
    pub fn selected(&self) -> Option<usize> {
        self.sel.first()
    }

    pub fn item_height(&self, res: &Res) -> i32 {
        res.fonts.line_height(self.style.kind)
    }

    fn inner(&self, rect: Rect) -> Rect {
        if self.border { rect.inset(2) } else { rect }
    }

    /// Nombre d'éléments entièrement visibles.
    pub fn visible_rows(&self, res: &Res, rect: Rect) -> usize {
        (self.inner(rect).h / self.item_height(res).max(1)).max(1) as usize
    }

    fn has_vbar(&self, res: &Res, rect: Rect) -> bool {
        self.items.len() > self.visible_rows(res, rect)
    }

    fn list_rect(&self, res: &Res, rect: Rect) -> Rect {
        let inner = self.inner(rect);
        if self.has_vbar(res, rect) { inner.split_right(scrollbar::SIZE).1 } else { inner }
    }

    fn vbar_rect(&self, rect: Rect) -> Rect {
        self.inner(rect).split_right(scrollbar::SIZE).0
    }

    fn sync_vbar(&mut self, res: &Res, rect: Rect) {
        let page = self.visible_rows(res, rect) as i32;
        self.vbar.set_range(0, self.items.len() as i32, page);
        self.top = self.top.min(self.vbar.max_pos() as usize);
        self.vbar.pos = self.top as i32;
    }

    /// Fait défiler pour montrer l'élément `i`.
    pub fn ensure_visible(&mut self, i: usize, res: &Res, rect: Rect) {
        let rows = self.visible_rows(res, rect);
        if i < self.top {
            self.top = i;
        } else if i >= self.top + rows {
            self.top = i + 1 - rows;
        }
        self.sync_vbar(res, rect);
    }

    /// Élément sous un point.
    pub fn index_at(&self, res: &Res, rect: Rect, p: Point) -> Option<usize> {
        let lr = self.list_rect(res, rect);
        if !lr.contains(p) {
            return None;
        }
        let i = self.top + ((p.y - lr.y) / self.item_height(res)) as usize;
        (i < self.items.len()).then_some(i)
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        let (bg, fg) = self.colors.unwrap_or((t.window, t.window_text));
        let bg = if self.enabled { bg } else { t.face };
        canvas.fill(rect, bg);
        if self.border {
            canvas.bevel(rect, Bevel::Field, t);
        }
        let lr = self.list_rect(res, rect);
        let ih = self.item_height(res);
        canvas.with_clip(lr, |canvas| {
            let mut y = lr.y;
            for i in self.top..self.items.len() {
                if y >= lr.bottom() {
                    break;
                }
                let it = &self.items[i];
                let row = Rect::new(lr.x, y, lr.w, ih);
                let selected = self.sel.is_selected(i);
                let color = if !self.enabled {
                    t.disabled_text
                } else if selected {
                    canvas.fill(row, t.selection);
                    t.selection_text
                } else {
                    it.color.unwrap_or(fg)
                };
                let mut x = row.x + 2;
                if let Some(img) = it.icon.and_then(|id| res.icons.image(id)) {
                    canvas.blit(img, x, y + (ih - img.h as i32) / 2);
                    x += img.w as i32 + 2;
                }
                res.fonts.draw_text(canvas, x, y, &it.text, self.style, color);
                if self.focused && i == self.sel.cursor {
                    canvas.focus_rect(row, if selected { t.selection_text } else { fg });
                }
                y += ih;
            }
            if self.focused && self.items.is_empty() {
                canvas.focus_rect(Rect::new(lr.x, lr.y, lr.w, ih), fg);
            }
        });
        if self.has_vbar(res, rect) {
            let mut vb = self.vbar.clone();
            vb.set_range(0, self.items.len() as i32, self.visible_rows(res, rect) as i32);
            vb.pos = self.top as i32;
            vb.paint(canvas, res, self.vbar_rect(rect));
        }
    }

    /// Recherche en tapant : préfixe du texte (sans les préfixes de mode `@+%&~`), remis à zéro après 1 s.
    fn type_to_find(&mut self, c: char, now: i64) -> Option<usize> {
        if now - self.find_at > 1000 {
            self.find.clear();
        }
        self.find_at = now;
        self.find.extend(c.to_lowercase());
        let n = self.items.len();
        if n == 0 {
            return None;
        }
        let start = if self.find.chars().count() == 1 { self.sel.cursor + 1 } else { self.sel.cursor };
        (0..n)
            .map(|k| (start + k) % n)
            .find(|&i| self.items[i].text.trim_start_matches(['@', '+', '%', '&', '~', '!']).to_lowercase().starts_with(&self.find))
    }

    pub fn event(&mut self, input: &Input, rect: Rect, res: &Res, platform: &mut dyn Platform) -> Option<ListMsg> {
        if !self.enabled {
            return None;
        }
        self.sel.resize(self.items.len());
        self.sync_vbar(res, rect);
        if self.has_vbar(res, rect) {
            let vr = self.vbar_rect(rect);
            let before = self.vbar.pos;
            let consumed = matches!(input, Input::MouseDown { pos, .. } if vr.contains(*pos)) || self.vbar.pressed_part().is_some();
            self.vbar.event(input, vr, res, platform);
            if self.vbar.pos != before {
                self.top = self.vbar.pos as usize;
                platform.request_repaint();
            }
            if consumed && !matches!(input, Input::Tick) {
                return None;
            }
        }
        match input {
            Input::MouseDown { pos, button, mods, clicks } if rect.contains(*pos) => {
                let i = self.index_at(res, rect, *pos);
                match button {
                    MouseButton::Left => {
                        let i = i?;
                        if *clicks >= 2 {
                            return Some(ListMsg::DoubleClicked(i));
                        }
                        self.dragging = self.sel.mode != SelectMode::Multi;
                        let changed = self.sel.click(i, *mods);
                        platform.request_repaint();
                        if changed {
                            return Some(ListMsg::SelectionChanged);
                        }
                    }
                    MouseButton::Right => {
                        if let Some(i) = i {
                            if !self.sel.is_selected(i) {
                                self.sel.select_only(i);
                            }
                            platform.request_repaint();
                        }
                        return Some(ListMsg::RightClicked(i, *pos));
                    }
                    _ => {}
                }
            }
            Input::MouseMove { pos } if self.dragging => {
                let lr = self.list_rect(res, rect);
                let p = Point::new(pos.x.clamp(lr.x, lr.right() - 1), pos.y);
                let target = if pos.y < lr.y {
                    self.top.saturating_sub(1)
                } else if pos.y >= lr.bottom() {
                    (self.top + self.visible_rows(res, rect)).min(self.items.len().saturating_sub(1))
                } else {
                    self.index_at(res, rect, p)?
                };
                let mods = Mods { shift: self.sel.mode == SelectMode::Extended, ..Mods::NONE };
                if target != self.sel.cursor {
                    let changed = self.sel.move_to(target, mods);
                    self.ensure_visible(target, res, rect);
                    platform.request_repaint();
                    if changed {
                        return Some(ListMsg::SelectionChanged);
                    }
                }
            }
            Input::MouseUp { button: MouseButton::Left, .. } => self.dragging = false,
            Input::Wheel { pos, dy, .. } if rect.contains(*pos) => {
                let n = (-dy * 3.0).round() as i32;
                let top = (self.top as i32 + n).clamp(0, self.vbar.max_pos().max(0));
                if top as usize != self.top {
                    self.top = top as usize;
                    self.sync_vbar(res, rect);
                    platform.request_repaint();
                }
            }
            Input::Key { key, mods, text, .. } if self.focused => {
                let page = self.visible_rows(res, rect);
                if let Some(changed) = self.sel.key(*key, *mods, page) {
                    let c = self.sel.cursor;
                    self.ensure_visible(c, res, rect);
                    platform.request_repaint();
                    return changed.then_some(ListMsg::SelectionChanged);
                }
                if *key == Key::Enter {
                    return self.sel.first().map(ListMsg::DoubleClicked);
                }
                if let Some(c) = text.as_deref().and_then(|s| s.chars().next()).filter(|c| !c.is_control() && !mods.command()) {
                    if let Some(i) = self.type_to_find(c, platform.now_ms()) {
                        let changed = self.sel.move_to(i, Mods::NONE);
                        self.ensure_visible(i, res, rect);
                        platform.request_repaint();
                        return changed.then_some(ListMsg::SelectionChanged);
                    }
                }
            }
            Input::FocusIn => self.focused = true,
            Input::FocusOut => {
                self.focused = false;
                self.dragging = false;
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::TestPlatform;

    #[test]
    fn extended_selection() {
        let mut s = ListSel::new(SelectMode::Extended);
        s.resize(10);
        s.click(2, Mods::NONE);
        s.click(5, Mods::SHIFT);
        assert_eq!(s.selected(), vec![2, 3, 4, 5]);
        s.click(8, Mods::CTRL);
        assert_eq!(s.selected(), vec![2, 3, 4, 5, 8]);
        s.key(Key::Down, Mods::SHIFT, 5);
        assert_eq!(s.selected(), vec![8, 9]);
        let mut m = ListSel::new(SelectMode::Multi);
        m.resize(3);
        m.click(0, Mods::NONE);
        m.click(2, Mods::NONE);
        m.click(0, Mods::NONE);
        assert_eq!(m.selected(), vec![2]);
    }

    #[test]
    fn keyboard_scroll_and_find() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let mut l = ListBox::new(SelectMode::Single);
        let names: Vec<String> = (0..30).map(|i| format!("item{i:02}")).collect();
        l.set_items(&names);
        l.items[20].text = "zebre".into();
        l.focused = true;
        let rect = Rect::new(0, 0, 120, 4 + 16 * 5);
        assert_eq!(l.visible_rows(&res, rect), 5);
        l.event(&Input::key(Key::End, Mods::NONE), rect, &res, &mut p);
        assert_eq!(l.selected(), Some(29));
        assert_eq!(l.top, 25);
        p.now = 5000;
        assert_eq!(l.event(&Input::key(Key::Char('z'), Mods::NONE), rect, &res, &mut p), Some(ListMsg::SelectionChanged));
        assert_eq!(l.selected(), Some(20));
        assert_eq!(l.event(&Input::MouseDown { pos: Point::new(10, 3), button: MouseButton::Left, mods: Mods::NONE, clicks: 2 }, rect, &res, &mut p), Some(ListMsg::DoubleClicked(l.top)));
        let mut sorted = ListBox::new(SelectMode::Single);
        sorted.sorted = true;
        sorted.add(ListItem::new("b"));
        sorted.add(ListItem::new("a"));
        assert_eq!(sorted.add(ListItem::new("c")), 2);
        assert_eq!(sorted.items[0].text, "a");
    }
}
