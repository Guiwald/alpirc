//! Liste à colonnes : en-tête cliquable (indicateur de tri), colonnes redimensionnables, lignes de textes,
//! sélection, défilement vertical et horizontal.

use std::cmp::Ordering;

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect};
use crate::icons::IconId;
use crate::input::{Cursor, Input, Key, Mods, MouseButton, Platform};
use crate::text::{Align, TextStyle};
use crate::Res;

use super::listbox::{ListSel, SelectMode};
use super::scrollbar::{self, Orientation, ScrollBar};
use super::{ArrowDir, draw_arrow};

/// Une colonne.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub title: String,
    pub width: i32,
    pub align: Align,
}

impl Column {
    pub fn new(title: &str, width: i32) -> Column {
        Column { title: title.to_string(), width, align: Align::Left }
    }
}

/// Une ligne.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListRow {
    pub cells: Vec<String>,
    pub color: Option<u32>,
    pub icon: Option<IconId>,
    pub data: u64,
}

impl ListRow {
    pub fn new<S: AsRef<str>>(cells: &[S]) -> ListRow {
        ListRow { cells: cells.iter().map(|s| s.as_ref().to_string()).collect(), ..Default::default() }
    }
}

/// Message d'une liste à colonnes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListViewMsg {
    SelectionChanged,
    DoubleClicked(usize),
    /// Clic sur un en-tête : l'indicateur de tri a été mis à jour (voir [`ListView::sort`]) ; l'appelant trie
    /// ou appelle [`ListView::sort_rows`].
    HeaderClicked(usize),
    /// Colonne redimensionnée (index, nouvelle largeur).
    ColumnResized(usize, i32),
    RightClicked(Option<usize>, Point),
}

/// Liste à colonnes.
#[derive(Clone, Debug)]
pub struct ListView {
    pub columns: Vec<Column>,
    pub rows: Vec<ListRow>,
    pub sel: ListSel,
    pub top: usize,
    pub scroll_x: i32,
    /// Colonne de tri et sens (vrai = croissant).
    pub sort: Option<(usize, bool)>,
    pub enabled: bool,
    pub focused: bool,
    /// Lignes de grille.
    pub grid: bool,
    header_pressed: Option<usize>,
    header_hot: bool,
    resizing: Option<(usize, i32, i32)>,
    vbar: ScrollBar,
    hbar: ScrollBar,
}

impl Default for ListView {
    fn default() -> ListView {
        ListView::new(Vec::new())
    }
}

/// Comparaison « naturelle » : nombres comparés comme des nombres.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

impl ListView {
    pub fn new(columns: Vec<Column>) -> ListView {
        ListView {
            columns,
            rows: Vec::new(),
            sel: ListSel::new(SelectMode::Single),
            top: 0,
            scroll_x: 0,
            sort: None,
            enabled: true,
            focused: false,
            grid: false,
            header_pressed: None,
            header_hot: false,
            resizing: None,
            vbar: ScrollBar::new(Orientation::Vertical),
            hbar: ScrollBar::new(Orientation::Horizontal),
        }
    }

    /// Remplace les lignes.
    pub fn set_rows(&mut self, rows: Vec<ListRow>) {
        self.rows = rows;
        self.sel = ListSel::new(self.sel.mode);
        self.sel.resize(self.rows.len());
        self.top = 0;
    }

    /// Trie les lignes selon [`ListView::sort`] (la sélection suit les lignes).
    pub fn sort_rows(&mut self) {
        let Some((col, asc)) = self.sort else { return };
        let sel: Vec<bool> = (0..self.rows.len()).map(|i| self.sel.is_selected(i)).collect();
        let mut idx: Vec<usize> = (0..self.rows.len()).collect();
        let empty = String::new();
        idx.sort_by(|&a, &b| {
            let x = self.rows[a].cells.get(col).unwrap_or(&empty);
            let y = self.rows[b].cells.get(col).unwrap_or(&empty);
            let o = natural_cmp(x, y);
            if asc { o } else { o.reverse() }
        });
        let rows = idx.iter().map(|&i| self.rows[i].clone()).collect();
        self.rows = rows;
        self.sel.clear();
        for (new, &old) in idx.iter().enumerate() {
            if sel[old] {
                self.sel.set(new, true);
            }
        }
    }

    pub fn row_height(&self, res: &Res) -> i32 {
        res.ui_line() + 1
    }

    pub fn header_height(&self, res: &Res) -> i32 {
        res.ui_line() + 4
    }

    fn total_width(&self) -> i32 {
        self.columns.iter().map(|c| c.width).sum()
    }

    /// Zones : en-tête, lignes, barre verticale, barre horizontale.
    fn layout(&self, res: &Res, rect: Rect) -> (Rect, Rect, Option<Rect>, Option<Rect>) {
        let inner = rect.inset(2);
        let hh = self.header_height(res);
        let rh = self.row_height(res);
        let mut body_h = inner.h - hh;
        let mut body_w = inner.w;
        let mut need_v = self.rows.len() as i32 * rh > body_h;
        if need_v {
            body_w -= scrollbar::SIZE;
        }
        let need_h = self.total_width() > body_w;
        if need_h {
            body_h -= scrollbar::SIZE;
            if !need_v && self.rows.len() as i32 * rh > body_h {
                need_v = true;
                body_w -= scrollbar::SIZE;
            }
        }
        let header = Rect::new(inner.x, inner.y, body_w, hh);
        let body = Rect::new(inner.x, inner.y + hh, body_w, body_h.max(0));
        let v = need_v.then(|| Rect::new(inner.x + body_w, inner.y, scrollbar::SIZE, inner.h - if need_h { scrollbar::SIZE } else { 0 }));
        let h = need_h.then(|| Rect::new(inner.x, inner.bottom() - scrollbar::SIZE, body_w, scrollbar::SIZE));
        (header, body, v, h)
    }

    fn visible_rows(&self, res: &Res, rect: Rect) -> usize {
        let (_, body, _, _) = self.layout(res, rect);
        (body.h / self.row_height(res)).max(1) as usize
    }

    fn sync_bars(&mut self, res: &Res, rect: Rect) {
        let (_, body, _, _) = self.layout(res, rect);
        let page = self.visible_rows(res, rect) as i32;
        self.vbar.set_range(0, self.rows.len() as i32, page);
        self.top = self.top.min(self.vbar.max_pos() as usize);
        self.vbar.pos = self.top as i32;
        self.hbar.set_range(0, self.total_width(), body.w.max(1));
        self.hbar.line_step = 16;
        self.scroll_x = self.hbar.clamp(self.scroll_x);
        self.hbar.pos = self.scroll_x;
    }

    /// Fait défiler pour montrer la ligne `i`.
    pub fn ensure_visible(&mut self, i: usize, res: &Res, rect: Rect) {
        let rows = self.visible_rows(res, rect);
        if i < self.top {
            self.top = i;
        } else if i >= self.top + rows {
            self.top = i + 1 - rows;
        }
        self.sync_bars(res, rect);
    }

    /// Bord droit (canevas) de chaque colonne.
    fn column_edges(&self, header: Rect) -> Vec<i32> {
        let mut x = header.x - self.scroll_x;
        self.columns
            .iter()
            .map(|c| {
                x += c.width;
                x
            })
            .collect()
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        canvas.fill(rect, if self.enabled { t.window } else { t.face });
        canvas.bevel(rect, Bevel::Field, t);
        let (header, body, vr, hr) = self.layout(res, rect);
        // En-tête.
        canvas.with_clip(header, |canvas| {
            canvas.fill(header, t.face);
            let mut x = header.x - self.scroll_x;
            for (i, c) in self.columns.iter().enumerate() {
                let cr = Rect::new(x, header.y, c.width, header.h);
                let pressed = self.header_pressed == Some(i) && self.header_hot;
                if pressed {
                    canvas.frame(cr, t.shadow);
                } else {
                    canvas.bevel(cr, Bevel::Button, t);
                }
                let o = if pressed { 1 } else { 0 };
                let mut tr = cr.inset_sides(6, 0, 6, 0).offset(o, o);
                if self.sort.map(|s| s.0) == Some(i) {
                    let (ar, rest) = tr.split_right(12);
                    tr = rest;
                    let dir = if self.sort.unwrap().1 { ArrowDir::Up } else { ArrowDir::Down };
                    draw_arrow(canvas, ar, dir, t.shadow);
                }
                let label = res.fonts.ellipsize(&c.title, TextStyle::UI, tr.w);
                canvas.with_clip(tr, |canvas| {
                    res.fonts.draw_label(canvas, tr, &label.replace('&', "&&"), TextStyle::UI, t.text, c.align, false, None);
                });
                x += c.width;
            }
            if x < header.right() {
                let cr = Rect::from_corners(x, header.y, header.right() + 2, header.bottom());
                canvas.bevel(cr, Bevel::Button, t);
            }
        });
        // Lignes.
        let rh = self.row_height(res);
        let edges = self.column_edges(header);
        canvas.with_clip(body, |canvas| {
            let mut y = body.y;
            for i in self.top..self.rows.len() {
                if y >= body.bottom() {
                    break;
                }
                let row = &self.rows[i];
                let selected = self.sel.is_selected(i);
                let mut x = body.x - self.scroll_x;
                let fg = row.color.unwrap_or(t.window_text);
                for (ci, c) in self.columns.iter().enumerate() {
                    let cell = Rect::new(x, y, c.width, rh);
                    let text = row.cells.get(ci).map(String::as_str).unwrap_or("");
                    let mut tr = cell.inset_sides(6, 0, 6, 0);
                    if ci == 0 {
                        if let Some(img) = row.icon.and_then(|id| res.icons.image(id)) {
                            canvas.blit(img, cell.x + 2, y + (rh - img.h as i32) / 2);
                            tr = cell.inset_sides(img.w as i32 + 4, 0, 6, 0);
                        }
                    }
                    let label = res.fonts.ellipsize(text, TextStyle::UI, tr.w);
                    let lw = res.fonts.measure(&label, TextStyle::UI);
                    let lx = match c.align {
                        Align::Left => tr.x,
                        Align::Center => tr.x + (tr.w - lw) / 2,
                        Align::Right => tr.right() - lw,
                    };
                    let color = if !self.enabled {
                        t.disabled_text
                    } else if selected && ci == 0 {
                        // Comme Windows : seule la première colonne est surlignée (texte).
                        let hl = Rect::new(lx - 2, y, lw + 4, rh);
                        canvas.fill(hl, if self.focused { t.selection } else { t.selection_inactive });
                        if self.focused { t.selection_text } else { t.selection_inactive_text }
                    } else {
                        fg
                    };
                    canvas.with_clip(cell, |canvas| {
                        res.fonts.draw_text(canvas, lx, y, &label, TextStyle::UI, color);
                    });
                    if self.focused && ci == 0 && i == self.sel.cursor {
                        canvas.focus_rect(Rect::new(lx - 2, y, lw + 4, rh), if selected { t.selection_text } else { t.window_text });
                    }
                    x += c.width;
                }
                if self.grid {
                    canvas.hline(body.x, y + rh - 1, body.w, t.light);
                }
                y += rh;
            }
            if self.grid {
                for e in &edges {
                    canvas.vline(*e - 1, body.y, body.h, t.light);
                }
            }
        });
        let mut vb = self.vbar.clone();
        let mut hb = self.hbar.clone();
        vb.set_range(0, self.rows.len() as i32, self.visible_rows(res, rect) as i32);
        vb.pos = self.top as i32;
        hb.set_range(0, self.total_width(), body.w.max(1));
        hb.pos = self.scroll_x;
        if let Some(vr) = vr {
            vb.paint(canvas, res, vr);
        }
        if let Some(hr) = hr {
            hb.paint(canvas, res, hr);
        }
        if let (Some(vr), Some(hr)) = (vr, hr) {
            canvas.fill(Rect::new(vr.x, hr.y, vr.w, hr.h), t.face);
        }
    }

    /// Ligne sous un point.
    pub fn row_at(&self, res: &Res, rect: Rect, p: Point) -> Option<usize> {
        let (_, body, _, _) = self.layout(res, rect);
        if !body.contains(p) {
            return None;
        }
        let i = self.top + ((p.y - body.y) / self.row_height(res)) as usize;
        (i < self.rows.len()).then_some(i)
    }

    pub fn event(&mut self, input: &Input, rect: Rect, res: &Res, platform: &mut dyn Platform) -> Option<ListViewMsg> {
        if !self.enabled {
            return None;
        }
        self.sel.resize(self.rows.len());
        self.sync_bars(res, rect);
        let (header, body, vr, hr) = self.layout(res, rect);
        for (bar_rect, vertical) in [(vr, true), (hr, false)] {
            let Some(br) = bar_rect else { continue };
            let bar = if vertical { &mut self.vbar } else { &mut self.hbar };
            let grabbed = bar.pressed_part().is_some() || matches!(input, Input::MouseDown { pos, .. } if br.contains(*pos));
            if !grabbed && !matches!(input, Input::Tick) {
                continue;
            }
            bar.event(input, br, res, platform);
            if vertical {
                self.top = self.vbar.pos as usize;
            } else {
                self.scroll_x = self.hbar.pos;
            }
            platform.request_repaint();
            if grabbed {
                return None;
            }
        }
        match input {
            Input::MouseMove { pos } => {
                if let Some((c, sx, sw)) = self.resizing {
                    let w = (sw + pos.x - sx).max(8);
                    if w != self.columns[c].width {
                        self.columns[c].width = w;
                        platform.request_repaint();
                        return Some(ListViewMsg::ColumnResized(c, w));
                    }
                    return None;
                }
                if header.contains(*pos) && self.edge_at(header, pos.x).is_some() {
                    platform.set_cursor(Cursor::ResizeEW);
                }
                if let Some(c) = self.header_pressed {
                    let hot = self.header_cell(header, *pos) == Some(c);
                    if hot != self.header_hot {
                        self.header_hot = hot;
                        platform.request_repaint();
                    }
                }
            }
            Input::MouseDown { pos, button: MouseButton::Left, mods, clicks } if rect.contains(*pos) => {
                if header.contains(*pos) {
                    if let Some(c) = self.edge_at(header, pos.x) {
                        self.resizing = Some((c, pos.x, self.columns[c].width));
                        platform.set_cursor(Cursor::ResizeEW);
                    } else if let Some(c) = self.header_cell(header, *pos) {
                        self.header_pressed = Some(c);
                        self.header_hot = true;
                        platform.request_repaint();
                    }
                    return None;
                }
                let i = self.row_at(res, rect, *pos)?;
                if *clicks >= 2 {
                    return Some(ListViewMsg::DoubleClicked(i));
                }
                let changed = self.sel.click(i, *mods);
                platform.request_repaint();
                if changed {
                    return Some(ListViewMsg::SelectionChanged);
                }
            }
            Input::MouseDown { pos, button: MouseButton::Right, .. } if body.contains(*pos) => {
                let i = self.row_at(res, rect, *pos);
                if let Some(i) = i {
                    if !self.sel.is_selected(i) {
                        self.sel.select_only(i);
                    }
                }
                platform.request_repaint();
                return Some(ListViewMsg::RightClicked(i, *pos));
            }
            Input::MouseUp { pos, button: MouseButton::Left, .. } => {
                if self.resizing.take().is_some() {
                    return None;
                }
                if let Some(c) = self.header_pressed.take() {
                    platform.request_repaint();
                    if self.header_cell(header, *pos) == Some(c) {
                        self.sort = Some(match self.sort {
                            Some((s, asc)) if s == c => (c, !asc),
                            _ => (c, true),
                        });
                        return Some(ListViewMsg::HeaderClicked(c));
                    }
                }
            }
            Input::Wheel { pos, dy, dx, mods } if rect.contains(*pos) => {
                if mods.shift || dx.abs() > dy.abs() {
                    let d = if *dx != 0.0 { *dx } else { *dy };
                    self.scroll_x = self.hbar.clamp(self.scroll_x - (d * 48.0) as i32);
                } else {
                    let n = (-dy * 3.0).round() as i32;
                    self.top = (self.top as i32 + n).clamp(0, self.vbar.max_pos().max(0)) as usize;
                }
                self.sync_bars(res, rect);
                platform.request_repaint();
            }
            Input::Key { key, mods, .. } if self.focused => {
                let page = self.visible_rows(res, rect);
                if let Some(changed) = self.sel.key(*key, *mods, page) {
                    let c = self.sel.cursor;
                    self.ensure_visible(c, res, rect);
                    platform.request_repaint();
                    return changed.then_some(ListViewMsg::SelectionChanged);
                }
                match key {
                    Key::Enter => return self.sel.first().map(ListViewMsg::DoubleClicked),
                    Key::Left | Key::Right => {
                        let d = if *key == Key::Left { -16 } else { 16 };
                        self.scroll_x = self.hbar.clamp(self.scroll_x + d);
                        self.sync_bars(res, rect);
                        platform.request_repaint();
                    }
                    Key::Char('a') if mods.command() && self.sel.mode == SelectMode::Extended => {
                        for i in 0..self.rows.len() {
                            self.sel.set(i, true);
                        }
                        platform.request_repaint();
                        return Some(ListViewMsg::SelectionChanged);
                    }
                    _ => {}
                }
            }
            Input::FocusIn => self.focused = true,
            Input::FocusOut => self.focused = false,
            _ => {}
        }
        let _ = (body, Mods::NONE);
        None
    }

    fn header_cell(&self, header: Rect, p: Point) -> Option<usize> {
        if !header.contains(p) {
            return None;
        }
        let mut x = header.x - self.scroll_x;
        for (i, c) in self.columns.iter().enumerate() {
            if p.x >= x && p.x < x + c.width {
                return Some(i);
            }
            x += c.width;
        }
        None
    }

    fn edge_at(&self, header: Rect, x: i32) -> Option<usize> {
        self.column_edges(header).iter().position(|&e| (x - e).abs() <= 3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::TestPlatform;

    #[test]
    fn header_sort_indicator_and_resize() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let mut lv = ListView::new(vec![Column::new("Canal", 100), Column::new("Users", 60)]);
        lv.set_rows(vec![ListRow::new(&["#lyon", "12"]), ListRow::new(&["#grenoble", "3"]), ListRow::new(&["#chambery", "100"])]);
        let rect = Rect::new(0, 0, 300, 200);
        let hy = 2 + 5;
        lv.event(&Input::click(130, hy), rect, &res, &mut p);
        assert_eq!(lv.event(&Input::release(130, hy), rect, &res, &mut p), Some(ListViewMsg::HeaderClicked(1)));
        assert_eq!(lv.sort, Some((1, true)));
        lv.sort_rows();
        assert_eq!(lv.rows[0].cells[0], "#grenoble");
        assert_eq!(lv.rows[2].cells[0], "#chambery", "tri numérique");
        lv.event(&Input::click(130, hy), rect, &res, &mut p);
        lv.event(&Input::release(130, hy), rect, &res, &mut p);
        assert_eq!(lv.sort, Some((1, false)));
        lv.event(&Input::click(10, hy), rect, &res, &mut p);
        lv.event(&Input::release(10, hy), rect, &res, &mut p);
        assert_eq!(lv.sort, Some((0, true)));
        // Redimensionner la première colonne (bord à x = 2 + 100).
        lv.event(&Input::click(102, hy), rect, &res, &mut p);
        assert_eq!(lv.event(&Input::moved(142, hy), rect, &res, &mut p), Some(ListViewMsg::ColumnResized(0, 140)));
        lv.event(&Input::release(142, hy), rect, &res, &mut p);
        assert_eq!(lv.sort, Some((0, true)), "redimensionner ne trie pas");
    }
}
