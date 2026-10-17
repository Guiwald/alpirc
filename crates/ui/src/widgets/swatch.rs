//! Pastille de couleur cliquable (boîte « Couleurs ») : un carré de couleur en creux, entouré d'un cadre noir et
//! blanc quand il est choisi, comme les cases de la palette de mIRC.

use crate::Res;
use crate::canvas::{Bevel, Canvas};
use crate::geom::Rect;
use crate::input::{Input, Key, MouseButton, Platform};

/// Message d'une pastille.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwatchMsg {
    Clicked,
    DoubleClicked,
}

/// Pastille de couleur.
#[derive(Clone, Debug, Default)]
pub struct Swatch {
    /// Couleur affichée (`0x00RRGGBB`).
    pub color: u32,
    /// Choisie : cadre noir et blanc autour.
    pub selected: bool,
    pub enabled: bool,
    pub focused: bool,
}

impl Swatch {
    pub fn new(color: u32) -> Swatch {
        Swatch { color, selected: false, enabled: true, focused: false }
    }

    /// Lit une couleur `#RRGGBB` (ou `RRGGBB`).
    pub fn parse_hex(s: &str) -> Option<u32> {
        let h = s.trim().trim_start_matches('#');
        if h.len() != 6 {
            return None;
        }
        u32::from_str_radix(h, 16).ok()
    }

    /// Écrit une couleur en `#RRGGBB`.
    pub fn hex(color: u32) -> String {
        format!("#{:06X}", color & 0xFF_FFFF)
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        let inner = rect.inset(2);
        if self.selected {
            canvas.frame(rect, t.window_text);
            canvas.frame(rect.inset(1), t.window);
        }
        canvas.bevel(inner, Bevel::Field, t);
        let body = inner.inset(2);
        canvas.fill(body, if self.enabled { self.color } else { t.face });
        if self.focused {
            // Pointillés lisibles sur la couleur.
            let lum = ((self.color >> 16) & 0xFF) * 3 + ((self.color >> 8) & 0xFF) * 6 + (self.color & 0xFF);
            let c = if lum > 128 * 10 { 0x000000 } else { 0xFFFFFF };
            canvas.focus_rect(body.inset(2), c);
        }
    }

    pub fn event(&mut self, input: &Input, rect: Rect, _res: &Res, platform: &mut dyn Platform) -> Option<SwatchMsg> {
        if !self.enabled {
            return None;
        }
        match input {
            Input::MouseDown { pos, button: MouseButton::Left, clicks, .. } if rect.contains(*pos) => {
                platform.request_repaint();
                Some(if *clicks >= 2 { SwatchMsg::DoubleClicked } else { SwatchMsg::Clicked })
            }
            Input::Key { key: Key::Char(' '), .. } if self.focused => {
                platform.request_repaint();
                Some(SwatchMsg::Clicked)
            }
            Input::FocusIn => {
                self.focused = true;
                None
            }
            Input::FocusOut => {
                self.focused = false;
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::TestPlatform;

    #[test]
    fn hex_and_click() {
        assert_eq!(Swatch::parse_hex("#FF8000"), Some(0xFF8000));
        assert_eq!(Swatch::parse_hex("00ff00"), Some(0x00FF00));
        assert_eq!(Swatch::parse_hex("#FFF"), None);
        assert_eq!(Swatch::hex(0x12AB34), "#12AB34");
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let mut s = Swatch::new(0xFF0000);
        let r = Rect::new(0, 0, 20, 20);
        assert_eq!(s.event(&Input::click(5, 5), r, &res, &mut p), Some(SwatchMsg::Clicked));
        assert_eq!(s.event(&Input::click(50, 5), r, &res, &mut p), None);
    }
}
