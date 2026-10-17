//! Liste déroulante, modifiable ou non. La liste ouverte se dessine par-dessus tout le reste : l'appelant
//! appelle [`ComboBox::paint_popup`] en dernier.

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect};
use crate::input::{Input, Key, MouseButton, Platform};
use crate::text::TextStyle;
use crate::Res;

use super::scrollbar::{self, Orientation, ScrollBar};
use super::textedit::{TextEdit, TextEditMsg};
use super::{ArrowDir, draw_arrow};

/// Message d'une liste déroulante.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComboMsg {
    /// Un élément a été choisi.
    SelectionChanged(usize),
    /// Le texte du champ (modifiable) a changé.
    EditChanged,
    /// Entrée dans le champ modifiable.
    Submit,
    /// La liste s'est ouverte ou fermée.
    Dropped(bool),
}

/// Liste déroulante.
#[derive(Clone, Debug)]
pub struct ComboBox {
    pub items: Vec<String>,
    pub selected: Option<usize>,
    /// Champ modifiable (sinon le texte de l'élément choisi).
    pub editable: bool,
    pub edit: TextEdit,
    pub enabled: bool,
    pub focused: bool,
    /// Nombre de lignes de la liste ouverte.
    pub drop_rows: usize,
    pub sorted: bool,
    open: bool,
    hover: usize,
    popup_top: usize,
    btn_pressed: bool,
    /// Rectangle de la liste ouverte, mémorisé à l'ouverture.
    popup: Rect,
    vbar: ScrollBar,
}

impl Default for ComboBox {
    fn default() -> ComboBox {
        ComboBox::new(false)
    }
}

impl ComboBox {
    pub fn new(editable: bool) -> ComboBox {
        let mut edit = TextEdit::new("");
        edit.border = false;
        ComboBox {
            items: Vec::new(),
            selected: None,
            editable,
            edit,
            enabled: true,
            focused: false,
            drop_rows: 8,
            sorted: false,
            open: false,
            hover: 0,
            popup_top: 0,
            btn_pressed: false,
            popup: Rect::default(),
            vbar: ScrollBar::new(Orientation::Vertical),
        }
    }

    pub fn set_items<S: AsRef<str>>(&mut self, items: &[S]) {
        self.items = items.iter().map(|s| s.as_ref().to_string()).collect();
        if self.sorted {
            self.items.sort_by_key(|a| a.to_lowercase());
        }
        self.selected = None;
    }

    /// Choisit un élément (et met son texte dans le champ modifiable).
    pub fn select(&mut self, i: Option<usize>) {
        self.selected = i.filter(|&i| i < self.items.len());
        if let Some(i) = self.selected {
            if self.editable {
                let s = self.items[i].clone();
                self.edit.set_text(&s);
                self.edit.core.select_all();
            }
        }
    }

    /// Texte affiché : celui du champ modifiable, ou de l'élément choisi.
    pub fn text(&self) -> String {
        if self.editable {
            self.edit.text().to_string()
        } else {
            self.selected.and_then(|i| self.items.get(i)).cloned().unwrap_or_default()
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn preferred_height(&self, res: &Res) -> i32 {
        res.ui_line() + 6
    }

    fn button_rect(&self, rect: Rect) -> Rect {
        let inner = rect.inset(2);
        Rect::new(inner.right() - scrollbar::SIZE, inner.y, scrollbar::SIZE, inner.h)
    }

    fn field_rect(&self, rect: Rect) -> Rect {
        let inner = rect.inset(2);
        Rect::from_corners(inner.x, inner.y, inner.right() - scrollbar::SIZE, inner.bottom())
    }

    fn item_h(&self, res: &Res) -> i32 {
        res.ui_line()
    }

    /// Rectangle de la liste ouverte, sous le champ (ou au-dessus s'il n'y a pas la place dans `bounds`).
    pub fn popup_rect(&self, res: &Res, rect: Rect, bounds: Rect) -> Rect {
        let rows = self.items.len().clamp(1, self.drop_rows.max(1)) as i32;
        let h = rows * self.item_h(res) + 2;
        let mut r = Rect::new(rect.x, rect.bottom(), rect.w, h);
        if r.bottom() > bounds.bottom() && rect.y - h >= bounds.y {
            r.y = rect.y - h;
        }
        r
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        canvas.fill(rect, if self.enabled { t.window } else { t.face });
        canvas.bevel(rect, Bevel::Field, t);
        let fr = self.field_rect(rect);
        if self.editable {
            let mut e = self.edit.clone();
            e.enabled = self.enabled;
            e.focused = self.focused && !self.open;
            e.paint(canvas, res, fr);
        } else {
            let text = self.text();
            let tr = fr.inset_sides(1, 1, 1, 1);
            let color = if !self.enabled {
                t.disabled_text
            } else if self.focused && !self.open {
                canvas.fill(tr, t.selection);
                t.selection_text
            } else {
                t.window_text
            };
            canvas.with_clip(tr, |c| {
                let y = tr.y + (tr.h - res.ui_line()) / 2;
                res.fonts.draw_text(c, tr.x + 2, y, &text, TextStyle::UI, color);
            });
            if self.focused && !self.open {
                canvas.focus_rect(tr, t.selection_text);
            }
        }
        let br = self.button_rect(rect);
        canvas.fill(br, t.face);
        let down = self.btn_pressed;
        if down {
            canvas.frame(br, t.shadow);
        } else {
            canvas.bevel(br, Bevel::Button, t);
        }
        let o = if down { 1 } else { 0 };
        if self.enabled {
            draw_arrow(canvas, br.offset(o, o), ArrowDir::Down, t.text);
        } else {
            draw_arrow(canvas, br.offset(1, 1), ArrowDir::Down, t.highlight);
            draw_arrow(canvas, br, ArrowDir::Down, t.disabled_text);
        }
    }

    /// Dessine la liste ouverte (à appeler après tout le reste).
    pub fn paint_popup(&self, canvas: &mut Canvas, res: &Res) {
        if !self.open {
            return;
        }
        let t = &res.theme;
        let r = self.popup;
        canvas.fill(r, t.window);
        canvas.frame(r, t.dark);
        let inner = r.inset(1);
        let rows = (inner.h / self.item_h(res)).max(1) as usize;
        let need_bar = self.items.len() > rows;
        let list = if need_bar { inner.split_right(scrollbar::SIZE).1 } else { inner };
        let ih = self.item_h(res);
        canvas.with_clip(list, |c| {
            for (k, item) in self.items.iter().enumerate().skip(self.popup_top).take(rows) {
                let y = list.y + (k - self.popup_top) as i32 * ih;
                let row = Rect::new(list.x, y, list.w, ih);
                let color = if k == self.hover {
                    c.fill(row, t.selection);
                    t.selection_text
                } else {
                    t.window_text
                };
                res.fonts.draw_text(c, row.x + 2, y, item, TextStyle::UI, color);
            }
        });
        if need_bar {
            let mut vb = self.vbar.clone();
            vb.set_range(0, self.items.len() as i32, rows as i32);
            vb.pos = self.popup_top as i32;
            vb.paint(canvas, res, inner.split_right(scrollbar::SIZE).0);
        }
    }

    fn open_popup(&mut self, res: &Res, rect: Rect, bounds: Rect, platform: &mut dyn Platform) -> Option<ComboMsg> {
        if self.items.is_empty() {
            return None;
        }
        self.open = true;
        self.popup = self.popup_rect(res, rect, bounds);
        self.hover = self.selected.unwrap_or(0);
        let rows = ((self.popup.h - 2) / self.item_h(res)).max(1) as usize;
        self.popup_top = self.hover.saturating_sub(rows - 1).min(self.items.len().saturating_sub(rows));
        platform.request_repaint();
        Some(ComboMsg::Dropped(true))
    }

    fn close_popup(&mut self, platform: &mut dyn Platform) -> Option<ComboMsg> {
        self.open = false;
        self.btn_pressed = false;
        platform.request_repaint();
        Some(ComboMsg::Dropped(false))
    }

    fn choose(&mut self, i: usize, platform: &mut dyn Platform) -> Option<ComboMsg> {
        self.open = false;
        self.btn_pressed = false;
        platform.request_repaint();
        self.select(Some(i));
        Some(ComboMsg::SelectionChanged(i))
    }

    fn popup_index(&self, res: &Res, p: Point) -> Option<usize> {
        let inner = self.popup.inset(1);
        let rows = (inner.h / self.item_h(res)).max(1) as usize;
        let list = if self.items.len() > rows { inner.split_right(scrollbar::SIZE).1 } else { inner };
        if !list.contains(p) {
            return None;
        }
        let i = self.popup_top + ((p.y - list.y) / self.item_h(res)) as usize;
        (i < self.items.len()).then_some(i)
    }

    /// `bounds` : zone où la liste peut s'ouvrir (le canevas, la boîte de dialogue).
    pub fn event(&mut self, input: &Input, rect: Rect, bounds: Rect, res: &Res, platform: &mut dyn Platform) -> Option<ComboMsg> {
        if !self.enabled {
            return None;
        }
        if self.open {
            let rows = ((self.popup.h - 2) / self.item_h(res)).max(1) as usize;
            let bar = self.popup.inset(1).split_right(scrollbar::SIZE).0;
            if self.items.len() > rows {
                self.vbar.set_range(0, self.items.len() as i32, rows as i32);
                self.vbar.pos = self.popup_top as i32;
                let grabbed = self.vbar.pressed_part().is_some() || matches!(input, Input::MouseDown { pos, .. } if bar.contains(*pos));
                if grabbed || matches!(input, Input::Tick) {
                    self.vbar.event(input, bar, res, platform);
                    self.popup_top = self.vbar.pos as usize;
                    platform.request_repaint();
                    if grabbed {
                        return None;
                    }
                }
            }
            match input {
                Input::MouseMove { pos } => {
                    if let Some(i) = self.popup_index(res, *pos) {
                        if i != self.hover {
                            self.hover = i;
                            platform.request_repaint();
                        }
                    }
                }
                Input::MouseDown { pos, .. } => {
                    if self.button_rect(rect).contains(*pos) {
                        return self.close_popup(platform);
                    }
                    if !self.popup.contains(*pos) {
                        return self.close_popup(platform);
                    }
                }
                Input::MouseUp { pos, .. } => {
                    self.btn_pressed = false;
                    if let Some(i) = self.popup_index(res, *pos) {
                        return self.choose(i, platform);
                    }
                }
                Input::Wheel { dy, .. } => {
                    let n = (-dy * 3.0).round() as i32;
                    self.popup_top = (self.popup_top as i32 + n).clamp(0, self.items.len().saturating_sub(rows) as i32) as usize;
                    platform.request_repaint();
                }
                Input::Key { key, mods, .. } => match key {
                    Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::Home | Key::End if !mods.alt => {
                        let n = self.items.len();
                        self.hover = match key {
                            Key::Up => self.hover.saturating_sub(1),
                            Key::Down => (self.hover + 1).min(n - 1),
                            Key::PageUp => self.hover.saturating_sub(rows - 1),
                            Key::PageDown => (self.hover + rows - 1).min(n - 1),
                            Key::Home => 0,
                            _ => n - 1,
                        };
                        if self.hover < self.popup_top {
                            self.popup_top = self.hover;
                        } else if self.hover >= self.popup_top + rows {
                            self.popup_top = self.hover + 1 - rows;
                        }
                        platform.request_repaint();
                    }
                    Key::Enter => {
                        let h = self.hover;
                        return self.choose(h, platform);
                    }
                    Key::Escape | Key::F(4) | Key::Up | Key::Down => return self.close_popup(platform),
                    _ => {}
                },
                Input::FocusOut => return self.close_popup(platform),
                _ => {}
            }
            return None;
        }
        match input {
            Input::MouseDown { pos, button: MouseButton::Left, .. } if rect.contains(*pos) => {
                let on_button = self.button_rect(rect).contains(*pos);
                if on_button || !self.editable {
                    self.btn_pressed = on_button;
                    return self.open_popup(res, rect, bounds, platform);
                }
                let fr = self.field_rect(rect);
                return self.edit_event(input, fr, res, platform);
            }
            Input::MouseUp { .. } if self.btn_pressed => {
                self.btn_pressed = false;
                platform.request_repaint();
            }
            Input::Key { key, mods, .. } if self.focused => {
                let n = self.items.len();
                match key {
                    Key::Down if mods.alt => return self.open_popup(res, rect, bounds, platform),
                    Key::F(4) => return self.open_popup(res, rect, bounds, platform),
                    Key::Up | Key::Down if n > 0 => {
                        let cur = self.selected;
                        let i = match (key, cur) {
                            (Key::Up, Some(c)) => c.saturating_sub(1),
                            (Key::Down, Some(c)) => (c + 1).min(n - 1),
                            _ => 0,
                        };
                        if Some(i) != cur {
                            self.select(Some(i));
                            platform.request_repaint();
                            return Some(ComboMsg::SelectionChanged(i));
                        }
                        return None;
                    }
                    _ => {}
                }
                if self.editable {
                    let fr = self.field_rect(rect);
                    return self.edit_event(input, fr, res, platform);
                }
                // Non modifiable : la première lettre choisit un élément.
                if let Input::Key { text: Some(s), .. } = input {
                    if let Some(c) = s.chars().next().map(|c| c.to_lowercase().to_string()) {
                        let start = self.selected.map_or(0, |s| s + 1);
                        if let Some(i) = (0..n).map(|k| (start + k) % n).find(|&i| self.items[i].to_lowercase().starts_with(&c)) {
                            self.select(Some(i));
                            platform.request_repaint();
                            return Some(ComboMsg::SelectionChanged(i));
                        }
                    }
                }
            }
            Input::FocusIn => {
                self.focused = true;
                if self.editable {
                    let fr = self.field_rect(rect);
                    self.edit.event(input, fr, res, platform);
                }
            }
            Input::FocusOut => {
                self.focused = false;
                if self.editable {
                    let fr = self.field_rect(rect);
                    self.edit.event(input, fr, res, platform);
                }
            }
            _ if self.editable => {
                let fr = self.field_rect(rect);
                return self.edit_event(input, fr, res, platform);
            }
            _ => {}
        }
        None
    }

    fn edit_event(&mut self, input: &Input, fr: Rect, res: &Res, platform: &mut dyn Platform) -> Option<ComboMsg> {
        self.edit.focused = self.focused;
        match self.edit.event(input, fr, res, platform)? {
            TextEditMsg::Changed => Some(ComboMsg::EditChanged),
            TextEditMsg::Submit => Some(ComboMsg::Submit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Mods, TestPlatform};

    #[test]
    fn open_choose_close() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let bounds = Rect::new(0, 0, 400, 400);
        let rect = Rect::new(10, 10, 150, 22);
        let mut c = ComboBox::new(false);
        c.set_items(&["Undernet", "EFnet", "Libera.Chat"]);
        assert_eq!(c.event(&Input::click(20, 20), rect, bounds, &res, &mut p), Some(ComboMsg::Dropped(true)));
        assert!(c.is_open());
        let pr = c.popup_rect(&res, rect, bounds);
        assert_eq!(pr.y, rect.bottom());
        let y = pr.y + 1 + res.ui_line() + 3;
        c.event(&Input::moved(20, y), rect, bounds, &res, &mut p);
        assert_eq!(c.event(&Input::release(20, y), rect, bounds, &res, &mut p), Some(ComboMsg::SelectionChanged(1)));
        assert!(!c.is_open());
        assert_eq!(c.text(), "EFnet");
        c.focused = true;
        assert_eq!(c.event(&Input::key(Key::Down, Mods::NONE), rect, bounds, &res, &mut p), Some(ComboMsg::SelectionChanged(2)));
        let mut e = ComboBox::new(true);
        e.focused = true;
        e.set_items(&["a", "b"]);
        e.event(&Input::FocusIn, rect, bounds, &res, &mut p);
        assert_eq!(e.event(&Input::key(Key::Char('x'), Mods::NONE), rect, bounds, &res, &mut p), Some(ComboMsg::EditChanged));
        assert_eq!(e.text(), "x");
    }
}
