//! Case à cocher et bouton radio.

use crate::canvas::{Bevel, Canvas};
use crate::geom::Rect;
use crate::input::{Input, Key, MouseButton, Platform};
use crate::text::{Align, TextStyle, parse_mnemonic};
use crate::Res;

use super::{CHECK_MARK, draw_pattern};

/// Taille de la case (13x13) et du rond (12x12).
pub const BOX: i32 = 13;

/// Message d'une case à cocher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckMsg {
    /// Nouvel état.
    Toggled(bool),
}

/// Message d'un bouton radio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioMsg {
    /// Le bouton a été choisi (à l'appelant de décocher les autres du groupe, voir [`RadioButton::select_in`]).
    Selected,
}

/// État commun du clic.
#[derive(Clone, Debug, Default)]
struct Press {
    armed: bool,
    pressed: bool,
}

impl Press {
    /// Renvoie vrai quand un clic complet (appui et relâchement dans `rect`) a eu lieu.
    fn handle(&mut self, input: &Input, rect: Rect, focused: bool, platform: &mut dyn Platform) -> bool {
        match input {
            Input::MouseDown { pos, button: MouseButton::Left, .. } if rect.contains(*pos) => {
                self.armed = true;
                self.pressed = true;
                platform.request_repaint();
            }
            Input::MouseMove { pos } if self.armed => {
                let p = rect.contains(*pos);
                if p != self.pressed {
                    self.pressed = p;
                    platform.request_repaint();
                }
            }
            Input::MouseUp { pos, button: MouseButton::Left, .. } if self.armed => {
                self.armed = false;
                self.pressed = false;
                platform.request_repaint();
                return rect.contains(*pos);
            }
            Input::Key { key: Key::Char(' '), mods, .. } if focused && mods.is_empty() => return true,
            _ => {}
        }
        false
    }
}

/// Case à cocher.
#[derive(Clone, Debug, Default)]
pub struct CheckBox {
    pub text: String,
    pub checked: bool,
    pub enabled: bool,
    pub focused: bool,
    /// Apparence de bouton poussoir à bascule (style `push` des dialogues mSL).
    pub push_like: bool,
    press: Press,
}

impl CheckBox {
    pub fn new(text: &str, checked: bool) -> CheckBox {
        CheckBox { text: text.to_string(), checked, enabled: true, ..Default::default() }
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        if self.push_like {
            let mut b = super::Button::new(&self.text);
            b.toggled = self.checked || self.press.pressed;
            b.enabled = self.enabled;
            b.focused = self.focused;
            b.paint(canvas, res, rect);
            return;
        }
        let bx = Rect::new(rect.x, rect.y + (rect.h - BOX) / 2, BOX, BOX);
        let fill = if self.press.pressed || !self.enabled { t.face } else { t.window };
        canvas.fill(bx, fill);
        canvas.bevel(bx, Bevel::Field, t);
        if self.checked {
            let col = if self.enabled { t.window_text } else { t.disabled_text };
            draw_pattern(canvas, bx.x + 3, bx.y + 3, &CHECK_MARK, |c| (c == b'k').then_some(col));
        }
        paint_text(canvas, res, rect, &self.text, self.enabled, self.focused);
    }

    pub fn event(&mut self, input: &Input, rect: Rect, _res: &Res, platform: &mut dyn Platform) -> Option<CheckMsg> {
        if !self.enabled {
            return None;
        }
        if self.press.handle(input, rect, self.focused, platform) {
            self.checked = !self.checked;
            return Some(CheckMsg::Toggled(self.checked));
        }
        None
    }
}

/// Bouton radio.
#[derive(Clone, Debug, Default)]
pub struct RadioButton {
    pub text: String,
    pub checked: bool,
    pub enabled: bool,
    pub focused: bool,
    /// Apparence de bouton poussoir.
    pub push_like: bool,
    press: Press,
}

impl RadioButton {
    pub fn new(text: &str, checked: bool) -> RadioButton {
        RadioButton { text: text.to_string(), checked, enabled: true, ..Default::default() }
    }

    /// Coche le bouton `index` du groupe et décoche les autres.
    pub fn select_in(group: &mut [RadioButton], index: usize) {
        for (i, r) in group.iter_mut().enumerate() {
            r.checked = i == index;
        }
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        if self.push_like {
            let mut b = super::Button::new(&self.text);
            b.toggled = self.checked || self.press.pressed;
            b.enabled = self.enabled;
            b.focused = self.focused;
            b.paint(canvas, res, rect);
            return;
        }
        let x = rect.x;
        let y = rect.y + (rect.h - 12) / 2;
        let inside = if self.press.pressed || !self.enabled { t.face } else { t.window };
        const RADIO: [&str; 12] = [
            "....ssss....",
            "..sskkkkss..",
            ".skkwwwwkkh.",
            ".skwwwwwwlh.",
            "skwwwwwwwwlh",
            "skwwwwwwwwlh",
            "skwwwwwwwwlh",
            "skwwwwwwwwlh",
            ".slwwwwwwlh.",
            ".hllwwwwllh.",
            "..hhllllhh..",
            "....hhhh....",
        ];
        draw_pattern(canvas, x, y, &RADIO, |c| match c {
            b's' => Some(t.shadow),
            b'k' => Some(t.dark),
            b'w' => Some(inside),
            b'l' => Some(t.light),
            b'h' => Some(t.highlight),
            _ => None,
        });
        if self.checked {
            let col = if self.enabled { t.window_text } else { t.disabled_text };
            draw_pattern(canvas, x + 4, y + 4, &[".kk.", "kkkk", "kkkk", ".kk."], |c| (c == b'k').then_some(col));
        }
        paint_text(canvas, res, rect, &self.text, self.enabled, self.focused);
    }

    pub fn event(&mut self, input: &Input, rect: Rect, _res: &Res, platform: &mut dyn Platform) -> Option<RadioMsg> {
        if !self.enabled {
            return None;
        }
        if self.press.handle(input, rect, self.focused, platform) {
            let was = self.checked;
            self.checked = true;
            if !was {
                platform.request_repaint();
            }
            return Some(RadioMsg::Selected);
        }
        None
    }
}

/// Texte d'une case ou d'un radio, à droite de la case, avec le rectangle de focus autour du texte.
fn paint_text(canvas: &mut Canvas, res: &Res, rect: Rect, text: &str, enabled: bool, focused: bool) {
    let t = &res.theme;
    let tx = rect.x + BOX + 4;
    let tr = Rect::from_corners(tx, rect.y, rect.right(), rect.bottom());
    res.fonts.draw_label(canvas, tr, text, TextStyle::UI, t.text, Align::Left, true, if enabled { None } else { Some(t) });
    if focused && !text.is_empty() {
        let w = res.fonts.measure(&parse_mnemonic(text).0, TextStyle::UI);
        let lh = res.ui_line();
        let fy = rect.y + (rect.h - lh) / 2 - 1;
        canvas.focus_rect(Rect::new(tx - 2, fy, (w + 4).min(tr.w + 2), lh + 2), t.text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::TestPlatform;

    #[test]
    fn toggles_and_radio_groups() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let r = Rect::new(0, 0, 100, 16);
        let mut c = CheckBox::new("&Gras", false);
        c.event(&Input::click(5, 5), r, &res, &mut p);
        assert_eq!(c.event(&Input::release(5, 5), r, &res, &mut p), Some(CheckMsg::Toggled(true)));
        c.focused = true;
        assert_eq!(c.event(&Input::key(Key::Char(' '), Default::default()), r, &res, &mut p), Some(CheckMsg::Toggled(false)));
        let mut g = vec![RadioButton::new("A", true), RadioButton::new("B", false)];
        RadioButton::select_in(&mut g, 1);
        assert!(!g[0].checked && g[1].checked);
    }
}
