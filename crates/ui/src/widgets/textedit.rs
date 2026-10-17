//! Champ de saisie d'une ligne : curseur clignotant, sélection à la souris et au clavier, sauts de mot,
//! presse-papiers, annulation, mot de passe, lecture seule, longueur maximale, défilement horizontal,
//! composition de la méthode de saisie (soulignée).

use crate::canvas::{Bevel, Canvas};
use crate::geom::Rect;
use crate::input::{Cursor, Input, Key, MouseButton, Platform};
use crate::text::{FontKind, TextStyle};
use crate::Res;

use super::edit::{EditCore, EditResult};
use super::CARET_BLINK_MS;

/// Message d'un champ de saisie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEditMsg {
    /// Le texte a changé.
    Changed,
    /// Entrée.
    Submit,
}

/// Lettre affichée en vidéo inverse pour un code de mise en forme de mIRC (comme la ligne de saisie de mIRC).
pub fn code_letter(c: char) -> Option<char> {
    Some(match c {
        '\x02' => 'B',
        '\x03' => 'K',
        '\x04' => 'H',
        '\x0F' => 'O',
        '\x11' => 'M',
        '\x16' => 'R',
        '\x1D' => 'I',
        '\x1E' => 'S',
        '\x1F' => 'U',
        '\x01' => 'A',
        '\x07' => 'G',
        '\t' => 'T',
        _ => return None,
    })
}

/// Champ de saisie d'une ligne.
#[derive(Clone, Debug)]
pub struct TextEdit {
    pub core: EditCore,
    /// Afficher des `*`.
    pub password: bool,
    pub enabled: bool,
    pub focused: bool,
    /// Police : interface (dialogues) ou discussion (ligne de saisie).
    pub style: TextStyle,
    /// Cadre creux de 2 pixels (sinon aucun cadre).
    pub border: bool,
    /// Afficher les codes de mise en forme de mIRC en lettres inversées (B, K, U, I, R, O).
    pub show_codes: bool,
    /// Couleurs imposées (fond, texte) ; sinon celles du thème (`window`, `window_text`).
    pub colors: Option<(u32, u32)>,
    /// Décalage horizontal du texte (défilement), en pixels.
    pub scroll_x: i32,
    preedit: Option<(String, Option<(usize, usize)>)>,
    caret_on: bool,
    blink_start: i64,
    dragging: bool,
}

impl Default for TextEdit {
    fn default() -> TextEdit {
        TextEdit::new("")
    }
}

/// Vue affichée : texte (avec composition insérée), correspondance avec le texte réel.
struct View {
    text: String,
    caret: usize,
    sel: std::ops::Range<usize>,
    preedit: Option<std::ops::Range<usize>>,
}

impl TextEdit {
    pub fn new(text: &str) -> TextEdit {
        TextEdit {
            core: EditCore::new(text),
            password: false,
            enabled: true,
            focused: false,
            style: TextStyle::UI,
            border: true,
            show_codes: false,
            colors: None,
            scroll_x: 0,
            preedit: None,
            caret_on: true,
            blink_start: 0,
            dragging: false,
        }
    }

    pub fn text(&self) -> &str {
        self.core.text()
    }

    pub fn set_text(&mut self, s: &str) {
        self.core.set_text(s);
        self.scroll_x = 0;
    }

    /// Composition en cours de la méthode de saisie.
    pub fn preedit(&self) -> Option<&str> {
        self.preedit.as_ref().map(|p| p.0.as_str())
    }

    /// Hauteur idéale (texte + cadre).
    pub fn preferred_height(&self, res: &Res) -> i32 {
        res.fonts.line_height(self.style.kind) + if self.border { 6 } else { 2 }
    }

    /// Zone du texte dans le rectangle du widget.
    pub fn text_rect(&self, rect: Rect) -> Rect {
        if self.border { rect.inset_sides(4, 2, 3, 2) } else { rect.inset_sides(2, 0, 2, 0) }
    }

    fn base_index(&self, i: usize) -> usize {
        if self.password { self.core.text()[..i].chars().count() } else { i }
    }

    fn view(&self) -> View {
        let t = self.core.text();
        let mut text: String = if self.password {
            "*".repeat(t.chars().count())
        } else if self.show_codes {
            t.chars().map(|c| code_letter(c).unwrap_or(c)).collect()
        } else {
            t.to_string()
        };
        let caret_b = self.base_index(self.core.caret());
        let sel = self.core.selection();
        let mut sel_d = self.base_index(sel.start)..self.base_index(sel.end);
        let mut caret = caret_b;
        let mut pre = None;
        if let Some((p, cur)) = &self.preedit {
            if !p.is_empty() {
                text.insert_str(caret_b, p);
                pre = Some(caret_b..caret_b + p.len());
                caret = caret_b + cur.map_or(p.len(), |c| c.1.min(p.len()));
                sel_d = caret..caret;
            }
        }
        View { text, caret, sel: sel_d, preedit: pre }
    }

    /// Index dans le texte réel d'un index de la vue (sans composition).
    fn from_view(&self, d: usize) -> usize {
        if self.password {
            self.core.text().char_indices().nth(d).map_or(self.core.text().len(), |(i, _)| i)
        } else {
            d.min(self.core.text().len())
        }
    }

    fn text_style_for_codes(&self) -> TextStyle {
        self.style
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        let (bg, fg) = self.colors.unwrap_or((t.window, t.window_text));
        let bg = if self.enabled && !self.core.read_only { bg } else if self.colors.is_some() { bg } else { t.face };
        canvas.fill(rect, bg);
        if self.border {
            canvas.bevel(rect, Bevel::Field, t);
        }
        let tr = self.text_rect(rect);
        let v = self.view();
        let style = self.text_style_for_codes();
        let shaped = res.fonts.shape(&v.text, style);
        let lh = res.fonts.line_height(style.kind);
        let top = tr.y + (tr.h - lh) / 2;
        let base = (top + res.fonts.ascent(style.kind)) as f32;
        let x0 = tr.x - self.scroll_x;
        let fg = if self.enabled { fg } else { t.disabled_text };
        canvas.with_clip(tr.inset_xy(-1, 0), |canvas| {
            let sel = v.sel.clone();
            let show_sel = self.focused && !sel.is_empty();
            if show_sel {
                let a = shaped.x_of(sel.start) as i32;
                let b = shaped.x_of(sel.end) as i32;
                canvas.fill(Rect::new(x0 + a, top, b - a, lh), t.selection);
            }
            let raw = self.core.text().as_bytes();
            let is_code = |g: &crate::text::Glyph| -> bool {
                if !self.show_codes || self.password {
                    return false;
                }
                let s = g.start as usize;
                // Hors de la composition, l'index de la vue est celui du texte.
                let s = match &v.preedit {
                    Some(p) if s >= p.end => s - (p.end - p.start),
                    Some(p) if s >= p.start => return false,
                    _ => s,
                };
                raw.get(s).is_some_and(|b| code_letter(*b as char).is_some())
            };
            for g in &shaped.glyphs {
                if is_code(g) {
                    let selected = show_sel && sel.contains(&(g.start as usize));
                    let box_c = if selected { t.selection_text } else { fg };
                    canvas.fill(Rect::new(x0 + g.x as i32, top + 1, g.w as i32, lh - 2), box_c);
                }
            }
            res.fonts.draw_glyphs(canvas, &shaped.glyphs, x0 as f32, base, |g| {
                let selected = show_sel && sel.contains(&(g.start as usize));
                Some(match (is_code(g), selected) {
                    (true, true) => t.selection,
                    (true, false) => bg,
                    (false, true) => t.selection_text,
                    (false, false) => fg,
                })
            });
            if let Some(p) = &v.preedit {
                let a = shaped.x_of(p.start) as i32;
                let b = shaped.x_of(p.end) as i32;
                canvas.hline(x0 + a, base as i32 + 2, b - a, fg);
            }
            if self.focused && self.caret_on && self.enabled {
                let cx = x0 + shaped.x_of(v.caret) as i32;
                canvas.vline(cx, top, lh, fg);
            }
        });
    }

    fn reset_blink(&mut self, platform: &mut dyn Platform) {
        self.caret_on = true;
        self.blink_start = platform.now_ms();
        platform.schedule_tick(self.blink_start + CARET_BLINK_MS);
        platform.request_repaint();
    }

    /// Fait défiler pour que le curseur reste visible, et place la zone de la méthode de saisie.
    pub fn ensure_caret_visible(&mut self, res: &Res, rect: Rect, platform: &mut dyn Platform) {
        let tr = self.text_rect(rect);
        let v = self.view();
        let shaped = res.fonts.shape(&v.text, self.style);
        let cx = shaped.x_of(v.caret) as i32;
        let w = tr.w.max(1);
        if cx - self.scroll_x < 0 {
            self.scroll_x = (cx - w / 3).max(0);
        } else if cx - self.scroll_x > w - 1 {
            self.scroll_x = cx - w + 1 + w / 3;
        }
        let max_scroll = (shaped.width_px() - w + 1).max(0);
        self.scroll_x = self.scroll_x.clamp(0, max_scroll.max(cx - w + 1).max(0));
        if self.focused {
            let lh = res.fonts.line_height(self.style.kind);
            platform.set_ime_area(Rect::new(tr.x + cx - self.scroll_x, tr.y + (tr.h - lh) / 2, 1, lh));
        }
    }

    /// Index du texte réel sous l'abscisse `x` (coordonnées du canevas).
    pub fn index_at_x(&self, res: &Res, rect: Rect, x: i32) -> usize {
        let tr = self.text_rect(rect);
        let disp: String = if self.password {
            "*".repeat(self.core.text().chars().count())
        } else if self.show_codes {
            self.core.text().chars().map(|c| code_letter(c).unwrap_or(c)).collect()
        } else {
            self.core.text().to_string()
        };
        let shaped = res.fonts.shape(&disp, self.style);
        let d = shaped.index_at((x - tr.x + self.scroll_x) as f32);
        self.from_view(d)
    }

    pub fn event(&mut self, input: &Input, rect: Rect, res: &Res, platform: &mut dyn Platform) -> Option<TextEditMsg> {
        if !self.enabled {
            return None;
        }
        let mut msg = None;
        match input {
            Input::MouseMove { pos } => {
                if rect.contains(*pos) || self.dragging {
                    platform.set_cursor(Cursor::Text);
                }
                if self.dragging {
                    let i = self.index_at_x(res, rect, pos.x);
                    self.core.set_caret(i, true);
                    self.ensure_caret_visible(res, rect, platform);
                    platform.request_repaint();
                }
            }
            Input::MouseDown { pos, button: MouseButton::Left, mods, clicks } if rect.contains(*pos) => {
                let i = self.index_at_x(res, rect, pos.x);
                match clicks {
                    2 => {
                        let w = self.core.word_at(i);
                        self.core.select(w);
                    }
                    3.. => self.core.select_all(),
                    _ => {
                        self.core.set_caret(i, mods.shift);
                        self.dragging = true;
                    }
                }
                self.reset_blink(platform);
                self.ensure_caret_visible(res, rect, platform);
            }
            Input::MouseUp { button: MouseButton::Left, .. } => self.dragging = false,
            Input::Key { key, mods, text, .. } if self.focused => {
                match key {
                    Key::Enter => return Some(TextEditMsg::Submit),
                    Key::Tab | Key::Escape | Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::F(_) => return None,
                    _ => {}
                }
                let r = self.core.handle_key(*key, *mods, platform);
                match r {
                    EditResult::Changed => msg = Some(TextEditMsg::Changed),
                    EditResult::Moved => {}
                    EditResult::Ignored => {
                        if let Some(s) = text.as_deref().filter(|s| !s.is_empty() && !mods.command()) {
                            if !s.chars().any(|c| c.is_control()) && self.core.insert(s) {
                                msg = Some(TextEditMsg::Changed);
                            }
                        } else {
                            return None;
                        }
                    }
                }
                self.reset_blink(platform);
                self.ensure_caret_visible(res, rect, platform);
            }
            Input::Text(s) if self.focused => {
                self.preedit = None;
                if self.core.insert(s) {
                    msg = Some(TextEditMsg::Changed);
                }
                self.reset_blink(platform);
                self.ensure_caret_visible(res, rect, platform);
            }
            Input::ImePreedit { text, cursor } if self.focused => {
                self.preedit = if text.is_empty() { None } else { Some((text.clone(), *cursor)) };
                self.reset_blink(platform);
                self.ensure_caret_visible(res, rect, platform);
            }
            Input::FocusIn => {
                self.focused = true;
                self.reset_blink(platform);
                self.ensure_caret_visible(res, rect, platform);
            }
            Input::FocusOut => {
                self.focused = false;
                self.dragging = false;
                self.preedit = None;
                platform.request_repaint();
            }
            Input::Tick if self.focused => {
                let now = platform.now_ms();
                let phase = (now - self.blink_start).max(0) / CARET_BLINK_MS;
                let on = phase % 2 == 0;
                if on != self.caret_on {
                    self.caret_on = on;
                    platform.request_repaint();
                }
                platform.schedule_tick(self.blink_start + (phase + 1) * CARET_BLINK_MS);
            }
            _ => {}
        }
        let _ = FontKind::Ui;
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Mods, TestPlatform};

    fn typ(e: &mut TextEdit, s: &str, rect: Rect, res: &Res, p: &mut TestPlatform) {
        for c in s.chars() {
            e.event(&Input::key(Key::Char(c), Mods::NONE), rect, res, p);
        }
    }

    #[test]
    fn typing_scrolls_and_mouse_selects() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let rect = Rect::new(0, 0, 60, 22);
        let mut e = TextEdit::new("");
        e.style = TextStyle::CHAT;
        e.event(&Input::FocusIn, rect, &res, &mut p);
        typ(&mut e, "bonjour tout le monde", rect, &res, &mut p);
        assert_eq!(e.text(), "bonjour tout le monde");
        assert!(e.scroll_x > 0, "le texte dépasse : défilement");
        e.event(&Input::key(Key::Home, Mods::NONE), rect, &res, &mut p);
        assert_eq!(e.scroll_x, 0);
        // Glisser.
        let tr = e.text_rect(rect);
        e.event(&Input::click(tr.x + 1, 8), rect, &res, &mut p);
        e.event(&Input::moved(tr.x + 8 * 3 + 1, 8), rect, &res, &mut p);
        e.event(&Input::release(tr.x + 8 * 3 + 1, 8), rect, &res, &mut p);
        assert_eq!(e.core.selected_text(), "bon");
        // Double-clic : un mot (le champ défile pour montrer la fin du mot).
        e.event(&Input::MouseDown { pos: crate::Point::new(tr.x + 3 * 8 + 2, 8), button: MouseButton::Left, mods: Mods::NONE, clicks: 2 }, rect, &res, &mut p);
        assert_eq!(e.core.selected_text(), "bonjour");
        assert_eq!(e.event(&Input::key(Key::Enter, Mods::NONE), rect, &res, &mut p), Some(TextEditMsg::Submit));
    }

    #[test]
    fn caret_blinks_and_ime() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let rect = Rect::new(0, 0, 100, 22);
        let mut e = TextEdit::new("ab");
        e.event(&Input::FocusIn, rect, &res, &mut p);
        assert_eq!(p.tick_at, Some(CARET_BLINK_MS));
        p.now = CARET_BLINK_MS + 1;
        e.event(&Input::Tick, rect, &res, &mut p);
        assert!(!e.caret_on);
        e.event(&Input::ImePreedit { text: "に".into(), cursor: None }, rect, &res, &mut p);
        assert_eq!(e.preedit(), Some("に"));
        assert_eq!(e.text(), "ab");
        e.event(&Input::Text("日".into()), rect, &res, &mut p);
        assert_eq!(e.text(), "ab日");
        assert!(p.ime_area.is_some());
    }

    #[test]
    fn password_mask() {
        let res = crate::test_res();
        let mut e = TextEdit::new("sécret");
        e.password = true;
        let v = e.view();
        assert_eq!(v.text, "******");
        let rect = Rect::new(0, 0, 200, 22);
        e.style = TextStyle::CHAT;
        let tr = e.text_rect(rect);
        assert_eq!(e.index_at_x(&res, rect, tr.x + 8 * 2 + 1), "sé".len());
    }
}
