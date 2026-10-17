//! Zone de texte multiligne (éditeur de scripts) : milliers de lignes, défilement vertical et horizontal,
//! sélection, tabulation, ligne et colonne du curseur, recherche et remplacement.

use std::cell::RefCell;
use std::ops::Range;

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect};
use crate::input::{Cursor, Input, Key, MouseButton, Platform};
use crate::text::TextStyle;
use crate::Res;

use super::edit::{EditCore, EditResult};
use super::scrollbar::{self, Orientation, ScrollBar};
use super::CARET_BLINK_MS;

/// Message d'une zone de texte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAreaMsg {
    Changed,
}

/// Zone de texte multiligne.
#[derive(Clone, Debug)]
pub struct TextArea {
    pub core: EditCore,
    pub enabled: bool,
    pub focused: bool,
    /// Police : celle des discussions (chasse fixe) pour l'éditeur de scripts, ou celle de l'interface.
    pub style: TextStyle,
    pub border: bool,
    /// Tab insère des espaces (`Some(n)`) ou une tabulation (`None`).
    pub tab_spaces: Option<usize>,
    /// Première ligne visible.
    pub top_line: usize,
    pub scroll_x: i32,
    vbar: ScrollBar,
    hbar: ScrollBar,
    lines: RefCell<(u64, Vec<usize>)>,
    caret_on: bool,
    blink_start: i64,
    dragging: bool,
    preferred_x: Option<i32>,
    preedit: Option<String>,
}

impl Default for TextArea {
    fn default() -> TextArea {
        TextArea::new("")
    }
}

impl TextArea {
    pub fn new(text: &str) -> TextArea {
        let mut core = EditCore::new("");
        core.multiline = true;
        core.set_text(text);
        core.set_caret(0, false);
        TextArea {
            core,
            enabled: true,
            focused: false,
            style: TextStyle::CHAT,
            border: true,
            tab_spaces: None,
            top_line: 0,
            scroll_x: 0,
            vbar: ScrollBar::new(Orientation::Vertical),
            hbar: ScrollBar::new(Orientation::Horizontal),
            lines: RefCell::new((u64::MAX, Vec::new())),
            caret_on: true,
            blink_start: 0,
            dragging: false,
            preferred_x: None,
            preedit: None,
        }
    }

    pub fn text(&self) -> &str {
        self.core.text()
    }

    /// Remplace tout le texte (curseur au début).
    pub fn set_text(&mut self, s: &str) {
        self.core.set_text(s);
        self.core.set_caret(0, false);
        self.top_line = 0;
        self.scroll_x = 0;
    }

    fn starts(&self) -> std::cell::Ref<'_, (u64, Vec<usize>)> {
        {
            let mut l = self.lines.borrow_mut();
            if l.0 != self.core.revision() {
                let t = self.core.text();
                let mut v = Vec::with_capacity(t.len() / 30 + 1);
                v.push(0);
                v.extend(t.bytes().enumerate().filter(|x| x.1 == b'\n').map(|x| x.0 + 1));
                *l = (self.core.revision(), v);
            }
        }
        self.lines.borrow()
    }

    /// Nombre de lignes.
    pub fn line_count(&self) -> usize {
        self.starts().1.len()
    }

    /// Octets d'une ligne (sans le `\n`).
    pub fn line_range(&self, i: usize) -> Range<usize> {
        let s = self.starts();
        let st = &s.1;
        let i = i.min(st.len() - 1);
        let a = st[i];
        let b = if i + 1 < st.len() { st[i + 1] - 1 } else { self.core.text().len() };
        a..b
    }

    /// Ligne contenant un octet.
    pub fn line_of(&self, byte: usize) -> usize {
        let s = self.starts();
        s.1.partition_point(|&x| x <= byte).saturating_sub(1)
    }

    /// Ligne et colonne (en caractères, à partir de 0) d'un octet.
    pub fn line_col(&self, byte: usize) -> (usize, usize) {
        let l = self.line_of(byte);
        let r = self.line_range(l);
        let col = self.core.text()[r.start..byte.min(r.end).max(r.start)].chars().count();
        (l, col)
    }

    /// Ligne et colonne du curseur.
    pub fn caret_line_col(&self) -> (usize, usize) {
        self.line_col(self.core.caret())
    }

    /// Place le curseur au début de la ligne `n` (à partir de 0).
    pub fn goto_line(&mut self, n: usize) {
        let r = self.line_range(n);
        self.core.set_caret(r.start, false);
        let vis = 10;
        if n < self.top_line || n >= self.top_line + vis {
            self.top_line = n.saturating_sub(vis / 2);
        }
    }

    /// Cherche `needle` depuis l'octet `from`, vers l'avant ou l'arrière, en bouclant.
    pub fn find(&self, needle: &str, from: usize, forward: bool, case_sensitive: bool) -> Option<Range<usize>> {
        if needle.is_empty() {
            return None;
        }
        let (hay, pat) = if case_sensitive {
            (self.core.text().to_string(), needle.to_string())
        } else {
            (self.core.text().to_lowercase(), needle.to_lowercase())
        };
        if hay.len() != self.core.text().len() {
            // Minuscules de longueur différente (rare) : recherche exacte.
            return self.find(needle, from, forward, true).filter(|_| !case_sensitive);
        }
        let from = from.min(hay.len());
        let hit = if forward {
            hay[from..].find(&pat).map(|i| i + from).or_else(|| hay.find(&pat))
        } else {
            hay[..from].rfind(&pat).or_else(|| hay.rfind(&pat))
        };
        hit.map(|i| i..i + pat.len())
    }

    /// Sélectionne l'occurrence suivante (ou précédente) après le curseur ; renvoie vrai si trouvée.
    pub fn find_next(&mut self, needle: &str, forward: bool, case_sensitive: bool) -> bool {
        let sel = self.core.selection();
        let from = if forward { sel.end } else { sel.start };
        match self.find(needle, from, forward, case_sensitive) {
            Some(r) => {
                let line = self.line_of(r.start);
                self.core.select(r);
                if line < self.top_line || line >= self.top_line + 10 {
                    self.top_line = line.saturating_sub(5);
                }
                true
            }
            None => false,
        }
    }

    /// Remplace la sélection.
    pub fn replace_selection(&mut self, s: &str) -> bool {
        self.core.insert(s)
    }

    /// Remplace toutes les occurrences ; renvoie leur nombre.
    pub fn replace_all(&mut self, needle: &str, repl: &str, case_sensitive: bool) -> usize {
        if needle.is_empty() {
            return 0;
        }
        let text = self.core.text().to_string();
        let mut out = String::with_capacity(text.len());
        let mut n = 0;
        let mut i = 0;
        let (hay, pat) = if case_sensitive { (text.clone(), needle.to_string()) } else { (text.to_lowercase(), needle.to_lowercase()) };
        if hay.len() != text.len() {
            return 0;
        }
        while let Some(p) = hay[i..].find(&pat) {
            out.push_str(&text[i..i + p]);
            out.push_str(repl);
            i += p + pat.len();
            n += 1;
        }
        out.push_str(&text[i..]);
        if n > 0 {
            self.core.replace_all_text(&out);
        }
        n
    }

    fn inner(&self, rect: Rect) -> Rect {
        if self.border { rect.inset(2) } else { rect }
    }

    fn line_h(&self, res: &Res) -> i32 {
        res.fonts.line_height(self.style.kind)
    }

    /// Zones : texte, barre verticale, barre horizontale (toujours présentes, comme l'éditeur de mIRC).
    fn layout(&self, rect: Rect) -> (Rect, Rect, Rect) {
        let inner = self.inner(rect);
        let body = Rect::new(inner.x, inner.y, inner.w - scrollbar::SIZE, inner.h - scrollbar::SIZE);
        let v = Rect::new(body.right(), inner.y, scrollbar::SIZE, body.h);
        let h = Rect::new(inner.x, body.bottom(), body.w, scrollbar::SIZE);
        (body, v, h)
    }

    fn text_left(&self, body: Rect) -> i32 {
        body.x + 2
    }

    fn visible_lines(&self, res: &Res, rect: Rect) -> usize {
        let (body, _, _) = self.layout(rect);
        (body.h / self.line_h(res)).max(1) as usize
    }

    fn max_width(&self, res: &Res) -> i32 {
        // Approximation bon marché : la ligne la plus longue en caractères, à la largeur moyenne.
        let s = self.starts();
        let t = self.core.text();
        let longest = (0..s.1.len())
            .map(|i| {
                let a = s.1[i];
                let b = if i + 1 < s.1.len() { s.1[i + 1] - 1 } else { t.len() };
                b - a
            })
            .max()
            .unwrap_or(0);
        let cw = res.fonts.measure("0", self.style).max(1);
        longest as i32 * cw + 16
    }

    fn sync_bars(&mut self, res: &Res, rect: Rect) {
        let (body, _, _) = self.layout(rect);
        let n = self.line_count() as i32;
        let vis = self.visible_lines(res, rect) as i32;
        self.vbar.set_range(0, n, vis);
        self.top_line = self.top_line.min(self.vbar.max_pos().max(0) as usize);
        self.vbar.pos = self.top_line as i32;
        let mw = self.max_width(res);
        self.hbar.set_range(0, mw, body.w.max(1));
        self.hbar.line_step = 16;
        self.scroll_x = self.hbar.clamp(self.scroll_x);
        self.hbar.pos = self.scroll_x;
    }

    fn x_in_line(&self, res: &Res, line: usize, byte: usize) -> i32 {
        let r = self.line_range(line);
        let s = res.fonts.shape(&self.core.text()[r.clone()], self.style);
        s.x_of(byte.saturating_sub(r.start)) as i32
    }

    fn byte_at(&self, res: &Res, line: usize, x: i32) -> usize {
        let r = self.line_range(line);
        let s = res.fonts.shape(&self.core.text()[r.clone()], self.style);
        r.start + s.index_at(x as f32).min(r.len())
    }

    /// Fait défiler pour montrer le curseur.
    pub fn ensure_caret_visible(&mut self, res: &Res, rect: Rect, platform: &mut dyn Platform) {
        let (body, _, _) = self.layout(rect);
        let (line, _) = self.caret_line_col();
        let vis = self.visible_lines(res, rect);
        if line < self.top_line {
            self.top_line = line;
        } else if line >= self.top_line + vis {
            self.top_line = line + 1 - vis;
        }
        let cx = self.x_in_line(res, line, self.core.caret());
        let w = body.w - 4;
        if cx - self.scroll_x < 0 {
            self.scroll_x = (cx - w / 3).max(0);
        } else if cx - self.scroll_x > w {
            self.scroll_x = cx - w + w / 3;
        }
        self.sync_bars(res, rect);
        if self.focused {
            let lh = self.line_h(res);
            let y = body.y + (line as i32 - self.top_line as i32) * lh;
            platform.set_ime_area(Rect::new(self.text_left(body) + cx - self.scroll_x, y, 1, lh));
        }
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        let bg = if self.enabled && !self.core.read_only { t.window } else { t.face };
        let fg = if self.enabled { t.window_text } else { t.disabled_text };
        canvas.fill(rect, bg);
        if self.border {
            canvas.bevel(rect, Bevel::Field, t);
        }
        let (body, vr, hr) = self.layout(rect);
        let lh = self.line_h(res);
        let x0 = self.text_left(body) - self.scroll_x;
        let sel = self.core.selection();
        let caret = self.core.caret();
        let caret_line = self.line_of(caret);
        let n = self.line_count();
        let asc = res.fonts.ascent(self.style.kind);
        canvas.with_clip(body, |canvas| {
            let mut y = body.y;
            let mut line = self.top_line;
            while y < body.bottom() && line < n {
                let r = self.line_range(line);
                let text = &self.core.text()[r.clone()];
                let shaped = res.fonts.shape(text, self.style);
                // Sélection sur cette ligne (le saut de ligne compris s'il est sélectionné).
                let a = sel.start.max(r.start);
                let b = sel.end.min(r.end);
                let sel_here = !sel.is_empty() && sel.start <= r.end && sel.end > r.start;
                if sel_here {
                    let xa = shaped.x_of(a.saturating_sub(r.start)) as i32;
                    let mut xb = shaped.x_of(b.max(a) - r.start) as i32;
                    if sel.end > r.end {
                        xb += 6;
                    }
                    canvas.fill(Rect::new(x0 + xa, y, xb - xa, lh), t.selection);
                }
                let base = (y + asc) as f32;
                res.fonts.draw_glyphs(canvas, &shaped.glyphs, x0 as f32, base, |g| {
                    let gb = r.start + g.start as usize;
                    Some(if sel_here && gb >= sel.start && gb < sel.end { t.selection_text } else { fg })
                });
                if line == caret_line {
                    let cx = x0 + shaped.x_of(caret - r.start) as i32;
                    if let Some(p) = &self.preedit {
                        let w = res.fonts.draw_text(canvas, cx, y, p, self.style, fg);
                        canvas.hline(cx, y + asc + 2, w, fg);
                    }
                    if self.focused && self.caret_on && self.enabled {
                        canvas.vline(cx, y, lh, fg);
                    }
                }
                y += lh;
                line += 1;
            }
        });
        let mut vb = self.vbar.clone();
        vb.set_range(0, n as i32, (body.h / lh).max(1));
        vb.pos = self.top_line as i32;
        vb.paint(canvas, res, vr);
        let mut hb = self.hbar.clone();
        hb.set_range(0, self.max_width(res), body.w.max(1));
        hb.pos = self.scroll_x;
        hb.paint(canvas, res, hr);
        canvas.fill(Rect::new(vr.x, hr.y, vr.w, hr.h), t.face);
    }

    fn reset_blink(&mut self, platform: &mut dyn Platform) {
        self.caret_on = true;
        self.blink_start = platform.now_ms();
        platform.schedule_tick(self.blink_start + CARET_BLINK_MS);
        platform.request_repaint();
    }

    fn pos_at(&self, res: &Res, rect: Rect, p: Point) -> usize {
        let (body, _, _) = self.layout(rect);
        let lh = self.line_h(res);
        let rel = (p.y - body.y).div_euclid(lh);
        let line = (self.top_line as i32 + rel).clamp(0, self.line_count() as i32 - 1) as usize;
        self.byte_at(res, line, p.x - self.text_left(body) + self.scroll_x)
    }

    fn vertical(&mut self, res: &Res, delta: i32, extend: bool) {
        let caret = self.core.caret();
        let line = self.line_of(caret);
        let x = match self.preferred_x {
            Some(x) => x,
            None => {
                let x = self.x_in_line(res, line, caret);
                self.preferred_x = Some(x);
                x
            }
        };
        let target = (line as i32 + delta).clamp(0, self.line_count() as i32 - 1) as usize;
        let b = if target == line && delta < 0 {
            0
        } else if target == line && delta > 0 {
            self.core.text().len()
        } else {
            self.byte_at(res, target, x)
        };
        self.core.set_caret(b, extend);
    }

    pub fn event(&mut self, input: &Input, rect: Rect, res: &Res, platform: &mut dyn Platform) -> Option<TextAreaMsg> {
        if !self.enabled {
            return None;
        }
        self.sync_bars(res, rect);
        let (body, vr, hr) = self.layout(rect);
        for (br, vertical) in [(vr, true), (hr, false)] {
            let bar = if vertical { &mut self.vbar } else { &mut self.hbar };
            let grabbed = bar.pressed_part().is_some() || matches!(input, Input::MouseDown { pos, .. } if br.contains(*pos));
            if grabbed || matches!(input, Input::Tick) {
                bar.event(input, br, res, platform);
                if vertical {
                    self.top_line = self.vbar.pos as usize;
                } else {
                    self.scroll_x = self.hbar.pos;
                }
                if grabbed {
                    platform.request_repaint();
                    return None;
                }
            }
        }
        let mut msg = None;
        match input {
            Input::MouseMove { pos } => {
                if body.contains(*pos) || self.dragging {
                    platform.set_cursor(Cursor::Text);
                }
                if self.dragging {
                    let lh = self.line_h(res);
                    if pos.y < body.y && self.top_line > 0 {
                        self.top_line -= 1;
                    } else if pos.y >= body.bottom() {
                        self.top_line = (self.top_line + 1).min(self.vbar.max_pos().max(0) as usize);
                    }
                    let _ = lh;
                    let b = self.pos_at(res, rect, *pos);
                    self.core.set_caret(b, true);
                    self.preferred_x = None;
                    platform.request_repaint();
                }
            }
            Input::MouseDown { pos, button: MouseButton::Left, mods, clicks } if body.contains(*pos) => {
                let b = self.pos_at(res, rect, *pos);
                match clicks {
                    2 => {
                        let w = self.core.word_at(b);
                        self.core.select(w);
                    }
                    3.. => {
                        let l = self.line_of(b);
                        let r = self.line_range(l);
                        self.core.select(r.start..(r.end + 1).min(self.core.text().len()));
                    }
                    _ => {
                        self.core.set_caret(b, mods.shift);
                        self.dragging = true;
                    }
                }
                self.preferred_x = None;
                self.reset_blink(platform);
            }
            Input::MouseUp { button: MouseButton::Left, .. } => self.dragging = false,
            Input::Wheel { pos, dx, dy, mods } if rect.contains(*pos) => {
                if mods.shift || dx.abs() > dy.abs() {
                    let d = if *dx != 0.0 { *dx } else { *dy };
                    self.scroll_x = self.hbar.clamp(self.scroll_x - (d * 48.0) as i32);
                } else {
                    let n = (-dy * 3.0).round() as i32;
                    self.top_line = (self.top_line as i32 + n).clamp(0, self.vbar.max_pos().max(0)) as usize;
                }
                self.sync_bars(res, rect);
                platform.request_repaint();
                return None;
            }
            Input::Key { key, mods, text, .. } if self.focused => {
                let vis = self.visible_lines(res, rect) as i32;
                let caret = self.core.caret();
                let line = self.line_of(caret);
                let r = self.line_range(line);
                let mut handled = true;
                match key {
                    Key::Up if !mods.command() => self.vertical(res, -1, mods.shift),
                    Key::Down if !mods.command() => self.vertical(res, 1, mods.shift),
                    Key::Up | Key::Down => {
                        let d = if *key == Key::Up { -1 } else { 1 };
                        self.top_line = (self.top_line as i32 + d).clamp(0, self.vbar.max_pos().max(0)) as usize;
                        platform.request_repaint();
                        return None;
                    }
                    Key::PageUp => {
                        self.top_line = (self.top_line as i32 - (vis - 1)).max(0) as usize;
                        self.vertical(res, -(vis - 1), mods.shift);
                    }
                    Key::PageDown => {
                        self.top_line = (self.top_line + (vis - 1) as usize).min(self.vbar.max_pos().max(0) as usize);
                        self.vertical(res, vis - 1, mods.shift);
                    }
                    Key::Home if mods.command() => self.core.set_caret(0, mods.shift),
                    Key::End if mods.command() => self.core.set_caret(self.core.text().len(), mods.shift),
                    Key::Home | Key::Left if *key == Key::Home || mods.meta => {
                        // Début de ligne, après l'indentation d'abord.
                        let indent = r.start + self.core.text()[r.clone()].len() - self.core.text()[r.clone()].trim_start().len();
                        let p = if caret == indent { r.start } else { indent };
                        self.core.set_caret(p, mods.shift);
                    }
                    Key::End | Key::Right if *key == Key::End || mods.meta => self.core.set_caret(r.end, mods.shift),
                    Key::Enter => {
                        // Garder l'indentation de la ligne.
                        let line_text = &self.core.text()[r.start..caret.max(r.start)];
                        let indent: String = line_text.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
                        if self.core.insert(&format!("\n{indent}")) {
                            msg = Some(TextAreaMsg::Changed);
                        }
                    }
                    Key::Tab if !mods.command() => {
                        let s = self.tab_spaces.map_or("\t".to_string(), |n| " ".repeat(n));
                        if self.core.insert(&s) {
                            msg = Some(TextAreaMsg::Changed);
                        }
                    }
                    _ => handled = false,
                }
                if !handled {
                    match self.core.handle_key(*key, *mods, platform) {
                        EditResult::Changed => msg = Some(TextAreaMsg::Changed),
                        EditResult::Moved => {}
                        EditResult::Ignored => {
                            match text.as_deref().filter(|s| !s.is_empty() && !mods.command()) {
                                Some(s) if !s.chars().any(|c| c.is_control()) => {
                                    if self.core.insert(s) {
                                        msg = Some(TextAreaMsg::Changed);
                                    }
                                }
                                _ => return None,
                            }
                        }
                    }
                }
                if !matches!(key, Key::Up | Key::Down | Key::PageUp | Key::PageDown) {
                    self.preferred_x = None;
                }
                self.reset_blink(platform);
            }
            Input::Text(s) if self.focused => {
                self.preedit = None;
                if self.core.insert(s) {
                    msg = Some(TextAreaMsg::Changed);
                }
                self.reset_blink(platform);
            }
            Input::ImePreedit { text, .. } if self.focused => {
                self.preedit = if text.is_empty() { None } else { Some(text.clone()) };
                platform.request_repaint();
            }
            Input::FocusIn => {
                self.focused = true;
                self.reset_blink(platform);
            }
            Input::FocusOut => {
                self.focused = false;
                self.dragging = false;
                self.preedit = None;
                platform.request_repaint();
                return None;
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
                return None;
            }
            _ => return None,
        }
        self.ensure_caret_visible(res, rect, platform);
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Mods, TestPlatform};

    #[test]
    fn lines_columns_find_replace() {
        let mut ta = TextArea::new("alias hello {\n  echo -a Hello\n}\n");
        assert_eq!(ta.line_count(), 4);
        assert_eq!(ta.line_range(1), 14..29);
        assert_eq!(ta.line_col(16), (1, 2));
        assert_eq!(ta.find("hello", 0, true, false), Some(6..11));
        assert_eq!(ta.find("hello", 7, true, false), Some(24..29));
        assert_eq!(ta.find("hello", 7, true, true), Some(6..11), "sensible à la casse : boucle au début");
        assert!(ta.find_next("echo", true, true));
        assert_eq!(ta.core.selected_text(), "echo");
        assert_eq!(ta.replace_all("hello", "salut", false), 2);
        assert!(ta.text().starts_with("alias salut {"));
        ta.goto_line(2);
        assert_eq!(ta.caret_line_col(), (2, 0));
    }

    #[test]
    fn keyboard_editing_keeps_indent() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let rect = Rect::new(0, 0, 300, 200);
        let mut ta = TextArea::new("on *:TEXT:*:#:{\n  if (x) {");
        ta.focused = true;
        ta.core.set_caret(ta.text().len(), false);
        assert_eq!(ta.event(&Input::key(Key::Enter, Mods::NONE), rect, &res, &mut p), Some(TextAreaMsg::Changed));
        assert!(ta.text().ends_with("{\n  "));
        ta.tab_spaces = Some(2);
        ta.event(&Input::key(Key::Tab, Mods::NONE), rect, &res, &mut p);
        ta.event(&Input::key(Key::Char('x'), Mods::NONE), rect, &res, &mut p);
        assert!(ta.text().ends_with("\n    x"));
        assert_eq!(ta.caret_line_col(), (2, 5));
        ta.event(&Input::key(Key::Up, Mods::NONE), rect, &res, &mut p);
        assert_eq!(ta.caret_line_col().0, 1);
        ta.event(&Input::key(Key::Home, Mods::CTRL), rect, &res, &mut p);
        assert_eq!(ta.core.caret(), 0);
        // Beaucoup de lignes : défilement.
        let big: String = (0..3000).map(|i| format!("ligne {i}\n")).collect();
        ta.set_text(&big);
        ta.event(&Input::key(Key::End, Mods::CTRL), rect, &res, &mut p);
        assert!(ta.top_line > 2900);
    }
}
