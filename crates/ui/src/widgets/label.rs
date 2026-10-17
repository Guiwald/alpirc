//! Étiquette (texte statique), cadre de groupe, lien.

use crate::canvas::{Bevel, Canvas};
use crate::geom::Rect;
use crate::input::{Cursor, Input, MouseButton, Platform};
use crate::text::{Align, TextStyle, parse_mnemonic};
use crate::Res;

/// Texte statique, sur une ligne ou plusieurs (retour à la ligne aux espaces).
#[derive(Clone, Debug, Default)]
pub struct Label {
    /// Texte, avec raccourci éventuel (`&Pseudo :`) : le raccourci donne le focus au contrôle suivant.
    pub text: String,
    pub align: Align,
    /// Retour à la ligne automatique (sinon une seule ligne, coupée).
    pub wrap: bool,
    pub enabled: bool,
    /// Couleur imposée (sinon celle du thème).
    pub color: Option<u32>,
    pub style: TextStyle,
    /// Fond imposé (sinon transparent : la face de la boîte).
    pub background: Option<u32>,
}

impl Label {
    pub fn new(text: &str) -> Label {
        Label { text: text.to_string(), enabled: true, wrap: true, ..Default::default() }
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        if let Some(bg) = self.background {
            canvas.fill(rect, bg);
        }
        let color = self.color.unwrap_or(t.text);
        let disabled = if self.enabled { None } else { Some(t) };
        let lh = res.fonts.line_height(self.style.kind);
        canvas.with_clip(rect, |canvas| {
            let one_line = !self.wrap || res.fonts.measure(&parse_mnemonic(&self.text).0, self.style) <= rect.w;
            if one_line && !self.text.contains('\n') {
                let r = Rect::new(rect.x, rect.y, rect.w, lh);
                res.fonts.draw_label(canvas, r, &self.text, self.style, color, self.align, true, disabled);
                return;
            }
            // Plusieurs lignes : on garde le raccourci sur le texte affiché.
            let (display, _) = parse_mnemonic(&self.text);
            for (i, line) in res.fonts.wrap_text(&display, self.style, rect.w).iter().enumerate() {
                let r = Rect::new(rect.x, rect.y + i as i32 * lh, rect.w, lh);
                if r.y >= rect.bottom() {
                    break;
                }
                let escaped = line.replace('&', "&&");
                res.fonts.draw_label(canvas, r, &escaped, self.style, color, self.align, false, disabled);
            }
        });
    }
}

/// Cadre de groupe : trait gravé avec un titre en haut à gauche.
#[derive(Clone, Debug, Default)]
pub struct GroupBox {
    pub text: String,
    pub enabled: bool,
}

impl GroupBox {
    pub fn new(text: &str) -> GroupBox {
        GroupBox { text: text.to_string(), enabled: true }
    }

    /// Rectangle intérieur (sous le titre).
    pub fn content_rect(&self, res: &Res, rect: Rect) -> Rect {
        rect.inset_sides(6, res.ui_line() + 2, 6, 6)
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        let lh = res.ui_line();
        let frame = Rect::from_corners(rect.x, rect.y + lh / 2, rect.right(), rect.bottom());
        canvas.bevel(frame, Bevel::Etched, t);
        if !self.text.is_empty() {
            let w = res.fonts.measure(&parse_mnemonic(&self.text).0, TextStyle::UI);
            let tr = Rect::new(rect.x + 8, rect.y, (w + 4).min(rect.w - 12).max(0), lh);
            canvas.fill(tr, t.face);
            res.fonts.draw_label(
                canvas,
                tr.inset_xy(2, 0),
                &self.text,
                TextStyle::UI,
                t.text,
                Align::Left,
                true,
                if self.enabled { None } else { Some(t) },
            );
        }
    }
}

/// Message d'un lien.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkMsg {
    Clicked,
}

/// Lien cliquable (texte souligné bleu, pointeur en main).
#[derive(Clone, Debug, Default)]
pub struct Link {
    pub text: String,
    pub enabled: bool,
    pub focused: bool,
    hover: bool,
    armed: bool,
}

impl Link {
    pub fn new(text: &str) -> Link {
        Link { text: text.to_string(), enabled: true, ..Default::default() }
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        let color = if self.enabled { t.link } else { t.disabled_text };
        let w = res.fonts.draw_text(canvas, rect.x, rect.y, &self.text, TextStyle::UI, color);
        let base = rect.y + res.fonts.ascent(crate::FontKind::Ui);
        canvas.hline(rect.x, base + 1, w, color);
        if self.focused {
            canvas.focus_rect(Rect::new(rect.x - 1, rect.y - 1, w + 2, res.ui_line() + 2), t.text);
        }
    }

    pub fn event(&mut self, input: &Input, rect: Rect, res: &Res, platform: &mut dyn Platform) -> Option<LinkMsg> {
        if !self.enabled {
            return None;
        }
        let w = res.fonts.measure(&self.text, TextStyle::UI);
        let hit = Rect::new(rect.x, rect.y, w.min(rect.w), res.ui_line());
        match input {
            Input::MouseMove { pos } => {
                let h = hit.contains(*pos);
                if h {
                    platform.set_cursor(Cursor::Hand);
                } else if self.hover {
                    platform.set_cursor(Cursor::Default);
                }
                self.hover = h;
            }
            Input::MouseDown { pos, button: MouseButton::Left, .. } if hit.contains(*pos) => self.armed = true,
            Input::MouseUp { pos, button: MouseButton::Left, .. } if self.armed => {
                self.armed = false;
                if hit.contains(*pos) {
                    return Some(LinkMsg::Clicked);
                }
            }
            Input::Key { key: crate::Key::Enter | crate::Key::Char(' '), .. } if self.focused => return Some(LinkMsg::Clicked),
            _ => {}
        }
        None
    }
}
