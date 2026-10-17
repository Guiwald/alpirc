//! Arbre : nœuds dépliables (boîtes +/-, traits pointillés), icônes, sélection, clavier.

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect};
use crate::icons::IconId;
use crate::input::{Input, Key, MouseButton, Platform};
use crate::text::TextStyle;
use crate::Res;

use super::scrollbar::{self, Orientation, ScrollBar};

/// Un nœud de l'arbre.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeNode {
    /// Identifiant choisi par l'appelant (unique dans l'arbre).
    pub id: u64,
    pub text: String,
    pub icon: Option<IconId>,
    pub expanded: bool,
    pub children: Vec<TreeNode>,
    /// Couleur du texte (sinon celle du thème).
    pub color: Option<u32>,
    pub bold: bool,
}

impl TreeNode {
    pub fn new(id: u64, text: &str) -> TreeNode {
        TreeNode { id, text: text.to_string(), expanded: true, ..Default::default() }
    }

    /// Avec des enfants.
    pub fn with_children(mut self, children: Vec<TreeNode>) -> TreeNode {
        self.children = children;
        self
    }

    /// Avec une icône.
    pub fn with_icon(mut self, icon: Option<IconId>) -> TreeNode {
        self.icon = icon;
        self
    }
}

/// Une ligne visible de l'arbre aplati.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatRow {
    pub id: u64,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    /// Le nœud est le dernier de ses frères.
    pub last: bool,
    /// Pour chaque niveau parent : y a-t-il encore des frères en dessous (trait vertical à prolonger) ?
    pub continues: Vec<bool>,
    pub parent: Option<u64>,
}

/// Message d'un arbre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeViewMsg {
    Selected(u64),
    /// Double-clic ou Entrée.
    Activated(u64),
    Expanded(u64, bool),
    RightClicked(Option<u64>, Point),
    MiddleClicked(u64),
}

/// Arbre.
#[derive(Clone, Debug)]
pub struct TreeView {
    pub roots: Vec<TreeNode>,
    pub selected: Option<u64>,
    pub top: usize,
    pub enabled: bool,
    pub focused: bool,
    /// Traits pointillés entre les nœuds.
    pub lines: bool,
    /// Boîtes +/-.
    pub buttons: bool,
    /// Traits et boîtes aussi pour les racines.
    pub root_lines: bool,
    pub indent: i32,
    pub border: bool,
    /// Couleurs imposées (fond, texte).
    pub colors: Option<(u32, u32)>,
    pub style: TextStyle,
    /// Garder la sélection visible sans le focus.
    pub show_selection_always: bool,
    vbar: ScrollBar,
}

impl Default for TreeView {
    fn default() -> TreeView {
        TreeView::new()
    }
}

fn flatten_into(nodes: &[TreeNode], depth: usize, continues: &mut Vec<bool>, parent: Option<u64>, out: &mut Vec<FlatRow>) {
    for (i, n) in nodes.iter().enumerate() {
        let last = i + 1 == nodes.len();
        out.push(FlatRow {
            id: n.id,
            depth,
            has_children: !n.children.is_empty(),
            expanded: n.expanded,
            last,
            continues: continues.clone(),
            parent,
        });
        if n.expanded && !n.children.is_empty() {
            continues.push(!last);
            flatten_into(&n.children, depth + 1, continues, Some(n.id), out);
            continues.pop();
        }
    }
}

fn find_in(nodes: &[TreeNode], id: u64) -> Option<&TreeNode> {
    for n in nodes {
        if n.id == id {
            return Some(n);
        }
        if let Some(f) = find_in(&n.children, id) {
            return Some(f);
        }
    }
    None
}

fn find_in_mut(nodes: &mut [TreeNode], id: u64) -> Option<&mut TreeNode> {
    for n in nodes {
        if n.id == id {
            return Some(n);
        }
        if let Some(f) = find_in_mut(&mut n.children, id) {
            return Some(f);
        }
    }
    None
}

impl TreeView {
    pub fn new() -> TreeView {
        TreeView {
            roots: Vec::new(),
            selected: None,
            top: 0,
            enabled: true,
            focused: false,
            lines: true,
            buttons: true,
            root_lines: true,
            indent: 19,
            border: true,
            colors: None,
            style: TextStyle::UI,
            show_selection_always: false,
            vbar: ScrollBar::new(Orientation::Vertical),
        }
    }

    /// Nœuds visibles (dépliés), dans l'ordre d'affichage.
    pub fn flatten(&self) -> Vec<FlatRow> {
        let mut out = Vec::new();
        flatten_into(&self.roots, 0, &mut Vec::new(), None, &mut out);
        out
    }

    pub fn find(&self, id: u64) -> Option<&TreeNode> {
        find_in(&self.roots, id)
    }

    pub fn find_mut(&mut self, id: u64) -> Option<&mut TreeNode> {
        find_in_mut(&mut self.roots, id)
    }

    /// Déplie ou replie un nœud.
    pub fn set_expanded(&mut self, id: u64, on: bool) {
        if let Some(n) = self.find_mut(id) {
            n.expanded = on;
        }
    }

    pub fn row_height(&self, res: &Res) -> i32 {
        res.fonts.line_height(self.style.kind) + 2
    }

    fn inner(&self, rect: Rect) -> Rect {
        if self.border { rect.inset(2) } else { rect }
    }

    fn visible_rows(&self, res: &Res, rect: Rect) -> usize {
        (self.inner(rect).h / self.row_height(res)).max(1) as usize
    }

    fn has_vbar(&self, res: &Res, rect: Rect, n: usize) -> bool {
        n > self.visible_rows(res, rect)
    }

    fn body(&self, res: &Res, rect: Rect, n: usize) -> Rect {
        let inner = self.inner(rect);
        if self.has_vbar(res, rect, n) { inner.split_right(scrollbar::SIZE).1 } else { inner }
    }

    fn level_x(&self, body: Rect, depth: usize) -> i32 {
        let base = if self.root_lines && (self.lines || self.buttons) { self.indent } else { 0 };
        body.x + 2 + base + depth as i32 * self.indent
    }

    /// Fait défiler pour montrer un nœud.
    pub fn ensure_visible(&mut self, id: u64, res: &Res, rect: Rect) {
        let rows = self.flatten();
        let Some(i) = rows.iter().position(|r| r.id == id) else { return };
        let vis = self.visible_rows(res, rect);
        if i < self.top {
            self.top = i;
        } else if i >= self.top + vis {
            self.top = i + 1 - vis;
        }
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        let (bg, fg) = self.colors.unwrap_or((t.window, t.window_text));
        canvas.fill(rect, if self.enabled { bg } else { t.face });
        if self.border {
            canvas.bevel(rect, Bevel::Field, t);
        }
        let rows = self.flatten();
        let body = self.body(res, rect, rows.len());
        let rh = self.row_height(res);
        let lh = res.fonts.line_height(self.style.kind);
        // Gris des traits (le même que l'ombre en classique, plus visible en sombre).
        let line_c = t.disabled_text;
        canvas.with_clip(body, |canvas| {
            let dotted_v = |canvas: &mut Canvas, x: i32, y0: i32, y1: i32| {
                let mut y = y0;
                while y < y1 {
                    if y % 2 == 0 {
                        canvas.pixel(x, y, line_c);
                    }
                    y += 1;
                }
            };
            for (k, row) in rows.iter().enumerate().skip(self.top) {
                let y = body.y + (k - self.top) as i32 * rh;
                if y >= body.bottom() {
                    break;
                }
                let Some(node) = self.find(row.id) else { continue };
                let x = self.level_x(body, row.depth);
                let cy = y + rh / 2;
                let has_lines = self.lines && (row.depth > 0 || self.root_lines);
                if has_lines {
                    let lx = x - self.indent / 2 - 1;
                    // Traits des niveaux parents.
                    for (d, &cont) in row.continues.iter().enumerate() {
                        if cont {
                            let px = self.level_x(body, d) - self.indent / 2 - 1;
                            dotted_v(canvas, px, y, y + rh);
                        }
                    }
                    let top_y = if k == 0 && row.depth == 0 { cy } else { y };
                    let bot_y = if row.last { cy } else { y + rh };
                    dotted_v(canvas, lx, top_y, bot_y + 1);
                    let mut xx = lx;
                    while xx < x - 2 {
                        if xx % 2 == 0 {
                            canvas.pixel(xx, cy, line_c);
                        }
                        xx += 1;
                    }
                }
                if self.buttons && row.has_children && (row.depth > 0 || self.root_lines) {
                    let bx = x - self.indent / 2 - 5;
                    let b = Rect::new(bx, cy - 4, 9, 9);
                    canvas.fill(b, bg);
                    canvas.frame(b, t.shadow);
                    canvas.hline(bx + 2, cy, 5, fg);
                    if !row.expanded {
                        canvas.vline(bx + 4, cy - 2, 5, fg);
                    }
                }
                let mut tx = x;
                if let Some(img) = node.icon.and_then(|id| res.icons.image(id)) {
                    canvas.blit(img, x, y + (rh - img.h as i32) / 2);
                    tx += img.w as i32 + 3;
                }
                let style = self.style.bold(node.bold);
                let tw = res.fonts.measure(&node.text, style);
                let tr = Rect::new(tx, y + (rh - lh - 2) / 2, tw + 4, lh + 2);
                let selected = self.selected == Some(row.id);
                let color = if !self.enabled {
                    t.disabled_text
                } else if selected && self.focused {
                    canvas.fill(tr, t.selection);
                    t.selection_text
                } else if selected && self.show_selection_always {
                    canvas.fill(tr, t.selection_inactive);
                    t.selection_inactive_text
                } else {
                    node.color.unwrap_or(fg)
                };
                res.fonts.draw_text(canvas, tx + 2, tr.y + 1, &node.text, style, color);
                if selected && self.focused {
                    canvas.focus_rect(tr, t.selection_text);
                }
            }
        });
        if self.has_vbar(res, rect, rows.len()) {
            let mut vb = self.vbar.clone();
            vb.set_range(0, rows.len() as i32, self.visible_rows(res, rect) as i32);
            vb.pos = self.top as i32;
            vb.paint(canvas, res, self.inner(rect).split_right(scrollbar::SIZE).0);
        }
    }

    /// Nœud sous un point, et vrai si le point est sur sa boîte +/-.
    pub fn hit(&self, res: &Res, rect: Rect, p: Point) -> Option<(u64, bool)> {
        let rows = self.flatten();
        let body = self.body(res, rect, rows.len());
        if !body.contains(p) {
            return None;
        }
        let k = self.top + ((p.y - body.y) / self.row_height(res)) as usize;
        let row = rows.get(k)?;
        let x = self.level_x(body, row.depth);
        let on_button = row.has_children && p.x >= x - self.indent && p.x < x - 1;
        Some((row.id, on_button))
    }

    fn select(&mut self, id: u64, res: &Res, rect: Rect, platform: &mut dyn Platform) -> Option<TreeViewMsg> {
        self.ensure_visible(id, res, rect);
        platform.request_repaint();
        if self.selected != Some(id) {
            self.selected = Some(id);
            Some(TreeViewMsg::Selected(id))
        } else {
            None
        }
    }

    fn toggle(&mut self, id: u64, platform: &mut dyn Platform) -> Option<TreeViewMsg> {
        let n = self.find_mut(id)?;
        if n.children.is_empty() {
            return None;
        }
        n.expanded = !n.expanded;
        let e = n.expanded;
        platform.request_repaint();
        Some(TreeViewMsg::Expanded(id, e))
    }

    pub fn event(&mut self, input: &Input, rect: Rect, res: &Res, platform: &mut dyn Platform) -> Option<TreeViewMsg> {
        if !self.enabled {
            return None;
        }
        let rows = self.flatten();
        let vis = self.visible_rows(res, rect);
        self.top = self.top.min(rows.len().saturating_sub(vis));
        if self.has_vbar(res, rect, rows.len()) {
            let vr = self.inner(rect).split_right(scrollbar::SIZE).0;
            self.vbar.set_range(0, rows.len() as i32, vis as i32);
            self.vbar.pos = self.top as i32;
            let grabbed = self.vbar.pressed_part().is_some() || matches!(input, Input::MouseDown { pos, .. } if vr.contains(*pos));
            if grabbed || matches!(input, Input::Tick) {
                self.vbar.event(input, vr, res, platform);
                self.top = self.vbar.pos as usize;
                platform.request_repaint();
                if grabbed {
                    return None;
                }
            }
        }
        match input {
            Input::MouseDown { pos, button, clicks, .. } if rect.contains(*pos) => {
                let hit = self.hit(res, rect, *pos);
                match button {
                    MouseButton::Left => {
                        let (id, on_button) = hit?;
                        if on_button {
                            return self.toggle(id, platform);
                        }
                        if *clicks >= 2 {
                            let has_children = self.find(id).is_some_and(|n| !n.children.is_empty());
                            if has_children {
                                let _ = self.toggle(id, platform);
                            }
                            return Some(TreeViewMsg::Activated(id));
                        }
                        return self.select(id, res, rect, platform);
                    }
                    MouseButton::Right => return Some(TreeViewMsg::RightClicked(hit.map(|h| h.0), *pos)),
                    MouseButton::Middle => return hit.map(|h| TreeViewMsg::MiddleClicked(h.0)),
                    _ => {}
                }
            }
            Input::Wheel { pos, dy, .. } if rect.contains(*pos) => {
                let n = (-dy * 3.0).round() as i32;
                self.top = (self.top as i32 + n).clamp(0, rows.len().saturating_sub(vis) as i32) as usize;
                platform.request_repaint();
            }
            Input::Key { key, text, .. } if self.focused => {
                let cur = self.selected.and_then(|s| rows.iter().position(|r| r.id == s));
                let n = rows.len();
                if n == 0 {
                    return None;
                }
                let target = match key {
                    Key::Up => cur.map_or(0, |c| c.saturating_sub(1)),
                    Key::Down => cur.map_or(0, |c| (c + 1).min(n - 1)),
                    Key::Home => 0,
                    Key::End => n - 1,
                    Key::PageUp => cur.map_or(0, |c| c.saturating_sub(vis.saturating_sub(1).max(1))),
                    Key::PageDown => cur.map_or(0, |c| (c + vis.saturating_sub(1).max(1)).min(n - 1)),
                    Key::Left => {
                        let c = cur?;
                        if rows[c].expanded && rows[c].has_children {
                            return self.toggle(rows[c].id, platform);
                        }
                        let p = rows[c].parent?;
                        rows.iter().position(|r| r.id == p)?
                    }
                    Key::Right => {
                        let c = cur?;
                        if !rows[c].has_children {
                            return None;
                        }
                        if !rows[c].expanded {
                            return self.toggle(rows[c].id, platform);
                        }
                        c + 1
                    }
                    Key::Enter => return self.selected.map(TreeViewMsg::Activated),
                    _ => {
                        let c = text.as_deref().and_then(|s| s.chars().next())?;
                        let id = self.selected?;
                        let expanded = self.find(id).is_some_and(|n| n.expanded);
                        return match c {
                            '+' if !expanded => self.toggle(id, platform),
                            '-' if expanded => self.toggle(id, platform),
                            _ => None,
                        };
                    }
                };
                let id = rows[target.min(n - 1)].id;
                return self.select(id, res, rect, platform);
            }
            Input::FocusIn => self.focused = true,
            Input::FocusOut => self.focused = false,
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Mods, TestPlatform};

    fn tree() -> TreeView {
        let mut t = TreeView::new();
        t.roots = vec![
            TreeNode::new(1, "Undernet").with_children(vec![TreeNode::new(2, "#lyon"), TreeNode::new(3, "#grenoble")]),
            TreeNode::new(4, "EFnet").with_children(vec![TreeNode::new(5, "#chambery")]),
        ];
        t
    }

    #[test]
    fn keyboard_expand_collapse() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let mut t = tree();
        t.focused = true;
        let rect = Rect::new(0, 0, 200, 200);
        assert_eq!(t.flatten().len(), 5);
        assert_eq!(t.event(&Input::key(Key::Down, Mods::NONE), rect, &res, &mut p), Some(TreeViewMsg::Selected(1)));
        assert_eq!(t.event(&Input::key(Key::Left, Mods::NONE), rect, &res, &mut p), Some(TreeViewMsg::Expanded(1, false)));
        assert_eq!(t.flatten().len(), 3);
        assert_eq!(t.event(&Input::key(Key::Down, Mods::NONE), rect, &res, &mut p), Some(TreeViewMsg::Selected(4)));
        // Déjà déplié : Droite descend au premier enfant.
        assert_eq!(t.event(&Input::key(Key::Right, Mods::NONE), rect, &res, &mut p), Some(TreeViewMsg::Selected(5)));
        assert_eq!(t.event(&Input::key(Key::Right, Mods::NONE), rect, &res, &mut p), None);
        assert_eq!(t.event(&Input::key(Key::Left, Mods::NONE), rect, &res, &mut p), Some(TreeViewMsg::Selected(4)));
        let rows = t.flatten();
        assert_eq!(rows[1].id, 4);
        assert!(rows[2].last && rows[2].depth == 1);
    }

    #[test]
    fn click_button_toggles() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let mut t = tree();
        let rect = Rect::new(0, 0, 200, 200);
        let body = t.body(&res, rect, 5);
        let x = t.level_x(body, 0) - t.indent / 2;
        let y = body.y + t.row_height(&res) / 2;
        assert_eq!(t.event(&Input::click(x, y), rect, &res, &mut p), Some(TreeViewMsg::Expanded(1, false)));
        assert_eq!(t.event(&Input::click(t.level_x(body, 0) + 5, y), rect, &res, &mut p), Some(TreeViewMsg::Selected(1)));
    }
}
