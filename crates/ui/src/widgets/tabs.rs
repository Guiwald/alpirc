//! Onglets : la bande d'onglets et le cadre de la page. Le contenu des pages appartient à l'appelant (voir
//! [`Tabs::page_rect`]).

use crate::canvas::{Bevel, Canvas};
use crate::geom::Rect;
use crate::input::{Input, Key, MouseButton, Platform};
use crate::text::{Align, TextStyle, parse_mnemonic};
use crate::Res;

/// Message des onglets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabsMsg {
    Selected(usize),
}

/// Bande d'onglets.
#[derive(Clone, Debug, Default)]
pub struct Tabs {
    pub labels: Vec<String>,
    pub selected: usize,
    pub enabled: bool,
    pub focused: bool,
}

impl Tabs {
    pub fn new<S: AsRef<str>>(labels: &[S]) -> Tabs {
        Tabs { labels: labels.iter().map(|s| s.as_ref().to_string()).collect(), selected: 0, enabled: true, focused: false }
    }

    /// Hauteur de la bande.
    pub fn strip_height(&self, res: &Res) -> i32 {
        res.ui_line() + 4
    }

    /// Rectangles des onglets (l'onglet choisi n'est pas agrandi ici).
    pub fn tab_rects(&self, res: &Res, rect: Rect) -> Vec<Rect> {
        let h = self.strip_height(res);
        let mut x = rect.x + 2;
        self.labels
            .iter()
            .map(|l| {
                let w = res.fonts.measure(&parse_mnemonic(l).0, TextStyle::UI) + 12;
                let r = Rect::new(x, rect.y + 2, w.max(24), h - 2);
                x += r.w;
                r
            })
            .collect()
    }

    /// Zone intérieure de la page (sous la bande, dans le cadre).
    pub fn page_rect(&self, res: &Res, rect: Rect) -> Rect {
        let h = self.strip_height(res);
        Rect::from_corners(rect.x, rect.y + h, rect.right(), rect.bottom()).inset(4)
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        let h = self.strip_height(res);
        let page = Rect::from_corners(rect.x, rect.y + h, rect.right(), rect.bottom());
        canvas.fill(page, t.face);
        canvas.bevel(page, Bevel::Raised, t);
        let rects = self.tab_rects(res, rect);
        for (i, r0) in rects.iter().enumerate() {
            let sel = i == self.selected;
            let r = if sel { Rect::new(r0.x - 2, rect.y, r0.w + 4, h + 1) } else { *r0 };
            canvas.fill(Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 1), t.face);
            // Bord gauche et haut éclairés, coin arrondi d'un pixel.
            canvas.vline(r.x, r.y + 2, r.h - 2, t.highlight);
            canvas.pixel(r.x + 1, r.y + 1, t.highlight);
            canvas.hline(r.x + 2, r.y, r.w - 4, t.highlight);
            // Bord droit : ombre et noir.
            canvas.vline(r.right() - 2, r.y + 1, r.h - 1, t.shadow);
            canvas.vline(r.right() - 1, r.y + 2, r.h - 2, t.dark);
            canvas.pixel(r.right() - 2, r.y + 1, t.dark);
            if sel {
                // L'onglet choisi s'ouvre sur la page.
                canvas.fill(Rect::new(r.x + 1, page.y, r.w - 3, 2), t.face);
            }
            let tr = Rect::new(r.x + 6, r.y + 2, r.w - 12, r.h - 3);
            let tr = if sel { tr.offset(0, -1) } else { tr };
            res.fonts.draw_label(canvas, tr, &self.labels[i], TextStyle::UI, t.text, Align::Center, true, if self.enabled { None } else { Some(t) });
            if sel && self.focused {
                let lw = res.fonts.measure(&parse_mnemonic(&self.labels[i]).0, TextStyle::UI);
                let lh = res.ui_line();
                canvas.focus_rect(Rect::new(tr.x + (tr.w - lw) / 2 - 2, tr.y + (tr.h - lh) / 2 - 1, lw + 4, lh + 2), t.text);
            }
        }
    }

    pub fn event(&mut self, input: &Input, rect: Rect, res: &Res, platform: &mut dyn Platform) -> Option<TabsMsg> {
        if !self.enabled || self.labels.is_empty() {
            return None;
        }
        let n = self.labels.len();
        let new = match input {
            Input::MouseDown { pos, button: MouseButton::Left, .. } => self.tab_rects(res, rect).iter().position(|r| r.contains(*pos))?,
            Input::Key { key: Key::Left, .. } if self.focused => self.selected.saturating_sub(1),
            Input::Key { key: Key::Right, .. } if self.focused => (self.selected + 1).min(n - 1),
            Input::Key { key: Key::Tab, mods, .. } if mods.ctrl => {
                if mods.shift { (self.selected + n - 1) % n } else { (self.selected + 1) % n }
            }
            Input::FocusIn => {
                self.focused = true;
                return None;
            }
            Input::FocusOut => {
                self.focused = false;
                return None;
            }
            _ => return None,
        };
        if new != self.selected {
            self.selected = new;
            platform.request_repaint();
            return Some(TabsMsg::Selected(new));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Mods, TestPlatform};

    #[test]
    fn select_by_click_and_keys() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let mut t = Tabs::new(&["&Général", "Connexion", "IRC"]);
        let rect = Rect::new(0, 0, 300, 200);
        let r = t.tab_rects(&res, rect);
        assert_eq!(t.event(&Input::click(r[2].x + 3, r[2].y + 3), rect, &res, &mut p), Some(TabsMsg::Selected(2)));
        t.focused = true;
        assert_eq!(t.event(&Input::key(Key::Left, Mods::NONE), rect, &res, &mut p), Some(TabsMsg::Selected(1)));
        assert_eq!(t.event(&Input::key(Key::Tab, Mods::CTRL), rect, &res, &mut p), Some(TabsMsg::Selected(2)));
        assert!(t.page_rect(&res, rect).y > r[0].bottom());
    }
}
