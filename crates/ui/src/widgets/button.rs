//! Bouton poussoir (avec style « par défaut » et « annuler ») et bouton plat de barre d'outils.

use crate::canvas::{Bevel, Canvas};
use crate::geom::Rect;
use crate::icons::IconId;
use crate::input::{Input, Key, MouseButton, Platform};
use crate::text::{Align, TextStyle};
use crate::Res;

/// Apparence d'un bouton.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonKind {
    /// Bouton poussoir en relief.
    #[default]
    Push,
    /// Bouton plat qui se soulève au survol (barres d'outils, style `flat` des dialogues).
    Flat,
}

/// Message d'un bouton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonMsg {
    Clicked,
}

/// Un bouton.
#[derive(Clone, Debug, Default)]
pub struct Button {
    /// Texte, avec raccourci éventuel (`&OK`).
    pub text: String,
    pub icon: Option<IconId>,
    pub kind: ButtonKind,
    /// Bouton par défaut (Entrée) : cadre noir supplémentaire.
    pub is_default: bool,
    /// Bouton d'annulation (Échap) : aucune différence visuelle, pour les boîtes de dialogue.
    pub is_cancel: bool,
    pub enabled: bool,
    pub focused: bool,
    /// Reste enfoncé (bouton à bascule).
    pub toggled: bool,
    /// Souligner la lettre du raccourci.
    pub show_mnemonic: bool,
    hover: bool,
    armed: bool,
    pressed: bool,
}

impl Button {
    pub fn new(text: &str) -> Button {
        Button { text: text.to_string(), enabled: true, show_mnemonic: true, ..Default::default() }
    }

    /// Bouton par défaut.
    pub fn default_button(mut self) -> Button {
        self.is_default = true;
        self
    }

    /// Bouton plat.
    pub fn flat(mut self) -> Button {
        self.kind = ButtonKind::Flat;
        self
    }

    /// Avec une icône.
    pub fn with_icon(mut self, icon: Option<IconId>) -> Button {
        self.icon = icon;
        self
    }

    /// Vrai pendant que la souris le maintient enfoncé.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Vrai si la souris le survole.
    pub fn is_hover(&self) -> bool {
        self.hover
    }

    /// Taille idéale (texte + marges, au moins 75x23 comme les boutons de Windows).
    pub fn preferred_size(&self, res: &Res) -> crate::Size {
        let tw = res.fonts.measure(&crate::text::parse_mnemonic(&self.text).0, TextStyle::UI);
        crate::Size::new((tw + 16).max(75), (res.ui_line() + 7).max(23))
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        let down = (self.pressed && self.armed) || self.toggled;
        let mut r = rect;
        match self.kind {
            ButtonKind::Push => {
                canvas.fill(rect, t.face);
                if self.is_default || (self.focused && self.enabled) {
                    canvas.frame(rect, t.dark);
                    r = rect.inset(1);
                }
                if down && !self.toggled && (self.is_default || self.focused) {
                    // Bouton par défaut enfoncé : simple trait d'ombre.
                    canvas.frame(r, t.shadow);
                } else if down {
                    canvas.bevel(r, Bevel::ButtonPressed, t);
                    if self.toggled {
                        super::dither(canvas, r.inset(2), t.face, t.highlight);
                    }
                } else {
                    canvas.bevel(r, Bevel::Button, t);
                }
            }
            ButtonKind::Flat => {
                canvas.fill(rect, t.face);
                if down {
                    canvas.bevel(rect, Bevel::Sunken, t);
                    if self.toggled && !self.pressed {
                        super::dither(canvas, rect.inset(1), t.face, t.highlight);
                    }
                } else if self.hover && self.enabled {
                    canvas.bevel(rect, Bevel::RaisedThin, t);
                }
            }
        }
        let off = if down { 1 } else { 0 };
        let inner = rect.inset(3).offset(off, off);
        let mut text_rect = inner;
        if let Some(img) = self.icon.and_then(|i| res.icons.image(i)) {
            let iw = img.w as i32;
            let ih = img.h as i32;
            let ix = if self.text.is_empty() { inner.x + (inner.w - iw) / 2 } else { inner.x + 2 };
            let iy = inner.y + (inner.h - ih) / 2;
            if self.enabled {
                canvas.blit(img, ix, iy);
            } else {
                canvas.blit_disabled(img, ix, iy, t);
            }
            text_rect = Rect::from_corners(ix + iw + 4, inner.y, inner.right(), inner.bottom());
        }
        if !self.text.is_empty() {
            let align = if self.icon.is_some() { Align::Left } else { Align::Center };
            canvas.with_clip(rect.inset(2), |c| {
                res.fonts.draw_label(
                    c,
                    text_rect,
                    &self.text,
                    TextStyle::UI,
                    t.text,
                    align,
                    self.show_mnemonic,
                    if self.enabled { None } else { Some(t) },
                );
            });
        }
        if self.focused && self.kind == ButtonKind::Push && self.enabled {
            canvas.focus_rect(rect.inset(4).offset(off, off), t.text);
        }
    }

    pub fn event(&mut self, input: &Input, rect: Rect, _res: &Res, platform: &mut dyn Platform) -> Option<ButtonMsg> {
        if !self.enabled {
            if self.hover || self.pressed {
                self.hover = false;
                self.pressed = false;
                self.armed = false;
                platform.request_repaint();
            }
            return None;
        }
        match input {
            Input::MouseMove { pos } => {
                let inside = rect.contains(*pos);
                let pressed = self.armed && inside;
                if inside != self.hover || pressed != self.pressed {
                    self.hover = inside;
                    self.pressed = pressed;
                    platform.request_repaint();
                }
            }
            Input::MouseDown { pos, button: MouseButton::Left, .. } if rect.contains(*pos) => {
                self.armed = true;
                self.pressed = true;
                self.hover = true;
                platform.request_repaint();
            }
            Input::MouseUp { pos, button: MouseButton::Left, .. } if self.armed => {
                self.armed = false;
                self.pressed = false;
                platform.request_repaint();
                if rect.contains(*pos) {
                    return Some(ButtonMsg::Clicked);
                }
            }
            Input::MouseLeave => {
                if self.hover {
                    self.hover = false;
                    self.pressed = false;
                    platform.request_repaint();
                }
            }
            Input::Key { key: Key::Char(' '), mods, .. } if self.focused && mods.is_empty() => return Some(ButtonMsg::Clicked),
            Input::Key { key: Key::Enter, mods, .. } if self.focused && mods.is_empty() => return Some(ButtonMsg::Clicked),
            Input::FocusOut => {
                self.armed = false;
                self.pressed = false;
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
    fn click_requires_release_inside() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let r = Rect::new(10, 10, 75, 23);
        let mut b = Button::new("&OK");
        assert_eq!(b.event(&Input::click(20, 20), r, &res, &mut p), None);
        assert!(b.is_pressed());
        assert_eq!(b.event(&Input::release(20, 20), r, &res, &mut p), Some(ButtonMsg::Clicked));
        b.event(&Input::click(20, 20), r, &res, &mut p);
        b.event(&Input::moved(200, 20), r, &res, &mut p);
        assert!(!b.is_pressed());
        assert_eq!(b.event(&Input::release(200, 20), r, &res, &mut p), None);
        b.enabled = false;
        b.event(&Input::click(20, 20), r, &res, &mut p);
        assert_eq!(b.event(&Input::release(20, 20), r, &res, &mut p), None);
    }
}
