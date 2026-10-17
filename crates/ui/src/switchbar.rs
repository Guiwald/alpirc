//! Switchbar (une rangée de boutons, un par fenêtre, sur plusieurs lignes si besoin, ou en colonne à gauche
//! ou à droite) et treebar (arbre des fenêtres groupées par connexion, largeur réglable).
//!
//! La couleur du texte signale l'activité d'une fenêtre qu'on ne regarde pas (thème : événement, message,
//! on vous a nommé).

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect};
use crate::icons::IconId;
use crate::input::{Cursor, Input, MouseButton, Platform};
use crate::text::{Align, TextStyle};
use crate::theme::Theme;
use crate::widgets::treeview::{TreeNode, TreeView, TreeViewMsg};
use crate::widgets::dither;
use crate::Res;

/// Activité d'une fenêtre qu'on ne regarde pas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Activity {
    #[default]
    None,
    /// Arrivée, départ, changement de pseudo…
    Event,
    /// Nouveau message.
    Message,
    /// On vous a nommé.
    Highlight,
}

impl Activity {
    /// Couleur du texte pour cette activité (`None` : couleur normale).
    pub fn color(self, theme: &Theme) -> Option<u32> {
        match self {
            Activity::None => None,
            Activity::Event => Some(theme.color(theme.activity_event)),
            Activity::Message => Some(theme.color(theme.activity_message)),
            Activity::Highlight => Some(theme.color(theme.activity_highlight)),
        }
    }
}

/// Un bouton de la switchbar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SwitchItem {
    /// Identifiant de la fenêtre.
    pub id: u64,
    pub label: String,
    pub icon: Option<IconId>,
    pub activity: Activity,
}

impl SwitchItem {
    pub fn new(id: u64, label: &str, icon: Option<IconId>) -> SwitchItem {
        SwitchItem { id, label: label.to_string(), icon, activity: Activity::None }
    }
}

/// Message de la switchbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchMsg {
    Activate(u64),
    RightClick(u64, Point),
    MiddleClick(u64),
}

/// La switchbar.
#[derive(Clone, Debug)]
pub struct Switchbar {
    pub items: Vec<SwitchItem>,
    /// Fenêtre active (bouton enfoncé).
    pub active: Option<u64>,
    /// En colonne (switchbar à gauche ou à droite).
    pub vertical: bool,
    pub max_button_w: i32,
    pub min_button_w: i32,
    pressed: Option<u64>,
}

impl Default for Switchbar {
    fn default() -> Switchbar {
        Switchbar::new()
    }
}

impl Switchbar {
    pub fn new() -> Switchbar {
        Switchbar { items: Vec::new(), active: None, vertical: false, max_button_w: 140, min_button_w: 56, pressed: None }
    }

    /// Hauteur d'un bouton.
    pub fn button_h(res: &Res) -> i32 {
        res.ui_line() + 6
    }

    fn button_w(&self, res: &Res, it: &SwitchItem) -> i32 {
        let icon = it.icon.and_then(|i| res.icons.image(i)).map_or(0, |im| im.w as i32 + 4);
        (res.fonts.measure(&it.label, TextStyle::UI) + icon + 12).clamp(self.min_button_w, self.max_button_w)
    }

    /// Place les boutons dans `rect` (retour à la ligne si la rangée est pleine).
    pub fn layout(&self, res: &Res, rect: Rect) -> Vec<(u64, Rect)> {
        let h = Self::button_h(res);
        let mut out = Vec::with_capacity(self.items.len());
        if self.vertical {
            let mut y = rect.y + 2;
            for it in &self.items {
                out.push((it.id, Rect::new(rect.x + 2, y, rect.w - 4, h)));
                y += h + 1;
            }
            return out;
        }
        let mut x = rect.x + 2;
        let mut y = rect.y + 2;
        for it in &self.items {
            let w = self.button_w(res, it).min(rect.w - 4);
            if x + w > rect.right() - 2 && x > rect.x + 2 {
                x = rect.x + 2;
                y += h + 1;
            }
            out.push((it.id, Rect::new(x, y, w, h)));
            x += w + 1;
        }
        out
    }

    /// Hauteur nécessaire (switchbar horizontale) pour une largeur donnée.
    pub fn height_for(&self, res: &Res, width: i32) -> i32 {
        let h = Self::button_h(res);
        let rows = self.layout(res, Rect::new(0, 0, width, 1000)).iter().map(|(_, r)| r.y).collect::<std::collections::BTreeSet<_>>().len().max(1) as i32;
        rows * (h + 1) + 3
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        canvas.fill(rect, t.face);
        for (it, (_, r)) in self.items.iter().zip(self.layout(res, rect)) {
            let active = self.active == Some(it.id);
            let down = active || self.pressed == Some(it.id);
            if down {
                canvas.bevel(r, Bevel::ButtonPressed, t);
                if active {
                    dither(canvas, r.inset(2), t.face, t.highlight);
                }
            } else {
                canvas.bevel(r, Bevel::Button, t);
            }
            let o = if down { 1 } else { 0 };
            let inner = r.inset_sides(4, 2, 4, 2).offset(o, o);
            let mut tx = inner.x;
            if let Some(img) = it.icon.and_then(|i| res.icons.image(i)) {
                canvas.blit(img, inner.x, inner.y + (inner.h - img.h as i32) / 2);
                tx += img.w as i32 + 4;
            }
            let tr = Rect::from_corners(tx, inner.y, inner.right(), inner.bottom());
            let label = res.fonts.ellipsize(&it.label, TextStyle::UI, tr.w);
            let color = if active { t.text } else { it.activity.color(t).unwrap_or(t.text) };
            let style = if active { TextStyle::UI_BOLD } else { TextStyle::UI };
            canvas.with_clip(tr, |c| {
                res.fonts.draw_label(c, tr, &label.replace('&', "&&"), style, color, Align::Left, false, None);
            });
        }
    }

    pub fn event(&mut self, input: &Input, rect: Rect, res: &Res, platform: &mut dyn Platform) -> Option<SwitchMsg> {
        let layout = self.layout(res, rect);
        let hit = |p: Point| layout.iter().find(|(_, r)| r.contains(p)).map(|x| x.0);
        match input {
            Input::MouseDown { pos, button, .. } if rect.contains(*pos) => {
                let id = hit(*pos)?;
                match button {
                    MouseButton::Left => {
                        self.pressed = Some(id);
                        platform.request_repaint();
                    }
                    MouseButton::Right => return Some(SwitchMsg::RightClick(id, *pos)),
                    MouseButton::Middle => return Some(SwitchMsg::MiddleClick(id)),
                    _ => {}
                }
            }
            Input::MouseUp { pos, button: MouseButton::Left, .. } => {
                let p = self.pressed.take()?;
                platform.request_repaint();
                if hit(*pos) == Some(p) {
                    self.active = Some(p);
                    if let Some(it) = self.items.iter_mut().find(|i| i.id == p) {
                        it.activity = Activity::None;
                    }
                    return Some(SwitchMsg::Activate(p));
                }
            }
            _ => {}
        }
        None
    }
}

/// Un nœud de la treebar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TreebarNode {
    pub id: u64,
    pub label: String,
    pub icon: Option<IconId>,
    pub activity: Activity,
    pub expanded: bool,
    pub children: Vec<TreebarNode>,
}

impl TreebarNode {
    pub fn new(id: u64, label: &str, icon: Option<IconId>) -> TreebarNode {
        TreebarNode { id, label: label.to_string(), icon, activity: Activity::None, expanded: true, children: Vec::new() }
    }

    pub fn with_children(mut self, children: Vec<TreebarNode>) -> TreebarNode {
        self.children = children;
        self
    }
}

/// Message de la treebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreebarMsg {
    Activate(u64),
    RightClick(u64, Point),
    MiddleClick(u64),
    /// Nouvelle largeur (glisser la séparation).
    Resized(i32),
    /// Nœud déplié ou replié.
    Toggled(u64, bool),
}

/// Largeur de la poignée de redimensionnement.
pub const SPLITTER: i32 = 4;

/// La treebar : les fenêtres groupées par connexion (Status, puis canaux, privés, autres).
#[derive(Clone, Debug)]
pub struct Treebar {
    pub roots: Vec<TreebarNode>,
    pub active: Option<u64>,
    /// Largeur totale (poignée comprise).
    pub width: i32,
    pub min_width: i32,
    pub max_width: i32,
    /// La poignée est à droite (treebar à gauche) ; sinon à gauche.
    pub splitter_right: bool,
    tree: TreeView,
    drag: Option<(i32, i32)>,
}

impl Default for Treebar {
    fn default() -> Treebar {
        Treebar::new()
    }
}

fn to_tree(n: &TreebarNode, theme: &Theme) -> TreeNode {
    TreeNode {
        id: n.id,
        text: n.label.clone(),
        icon: n.icon,
        expanded: n.expanded,
        children: n.children.iter().map(|c| to_tree(c, theme)).collect(),
        color: n.activity.color(theme),
        bold: false,
    }
}

fn set_expanded(nodes: &mut [TreebarNode], id: u64, on: bool) {
    for n in nodes {
        if n.id == id {
            n.expanded = on;
        }
        set_expanded(&mut n.children, id, on);
    }
}

fn clear_activity(nodes: &mut [TreebarNode], id: u64) {
    for n in nodes {
        if n.id == id {
            n.activity = Activity::None;
        }
        clear_activity(&mut n.children, id);
    }
}

impl Treebar {
    pub fn new() -> Treebar {
        let mut tree = TreeView::new();
        tree.show_selection_always = true;
        tree.root_lines = false;
        Treebar { roots: Vec::new(), active: None, width: 150, min_width: 60, max_width: 400, splitter_right: true, tree, drag: None }
    }

    fn split(&self, rect: Rect) -> (Rect, Rect) {
        if self.splitter_right {
            let (s, t) = rect.split_right(SPLITTER);
            (t, s)
        } else {
            let (s, t) = rect.split_left(SPLITTER);
            (t, s)
        }
    }

    fn sync(&mut self, res: &Res) {
        self.tree.roots = self.roots.iter().map(|n| to_tree(n, &res.theme)).collect();
        self.tree.selected = self.active;
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let mut tv = self.tree.clone();
        tv.roots = self.roots.iter().map(|n| to_tree(n, &res.theme)).collect();
        tv.selected = self.active;
        let (tr, sr) = self.split(rect);
        tv.paint(canvas, res, tr);
        canvas.fill(sr, res.theme.face);
    }

    pub fn event(&mut self, input: &Input, rect: Rect, res: &Res, platform: &mut dyn Platform) -> Option<TreebarMsg> {
        self.sync(res);
        let (tr, sr) = self.split(rect);
        match input {
            Input::MouseMove { pos } => {
                if let Some((sx, sw)) = self.drag {
                    let d = if self.splitter_right { pos.x - sx } else { sx - pos.x };
                    let w = (sw + d).clamp(self.min_width, self.max_width);
                    platform.set_cursor(Cursor::ResizeEW);
                    if w != self.width {
                        self.width = w;
                        platform.request_repaint();
                        return Some(TreebarMsg::Resized(w));
                    }
                    return None;
                }
                if sr.contains(*pos) {
                    platform.set_cursor(Cursor::ResizeEW);
                }
            }
            Input::MouseDown { pos, button: MouseButton::Left, .. } if sr.contains(*pos) => {
                self.drag = Some((pos.x, self.width));
                return None;
            }
            Input::MouseUp { .. } if self.drag.is_some() => {
                self.drag = None;
                return None;
            }
            _ => {}
        }
        let m = self.tree.event(input, tr, res, platform)?;
        match m {
            TreeViewMsg::Selected(id) | TreeViewMsg::Activated(id) => {
                self.active = Some(id);
                clear_activity(&mut self.roots, id);
                Some(TreebarMsg::Activate(id))
            }
            TreeViewMsg::Expanded(id, on) => {
                set_expanded(&mut self.roots, id, on);
                Some(TreebarMsg::Toggled(id, on))
            }
            TreeViewMsg::RightClicked(Some(id), p) => Some(TreebarMsg::RightClick(id, p)),
            TreeViewMsg::RightClicked(None, _) => None,
            TreeViewMsg::MiddleClicked(id) => Some(TreebarMsg::MiddleClick(id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Mods, TestPlatform};

    #[test]
    fn switchbar_wraps_and_activates() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let mut sb = Switchbar::new();
        for i in 0..8 {
            sb.items.push(SwitchItem::new(i, &format!("#canal{i}"), None));
        }
        sb.items[3].activity = Activity::Highlight;
        let h1 = sb.height_for(&res, 2000);
        let h2 = sb.height_for(&res, 300);
        assert!(h2 > h1, "les boutons passent à la ligne");
        let rect = Rect::new(0, 0, 300, h2);
        let l = sb.layout(&res, rect);
        assert!(l.iter().all(|(_, r)| r.right() <= 300));
        let c = l[3].1.center();
        sb.event(&Input::click(c.x, c.y), rect, &res, &mut p);
        assert_eq!(sb.event(&Input::release(c.x, c.y), rect, &res, &mut p), Some(SwitchMsg::Activate(3)));
        assert_eq!(sb.items[3].activity, Activity::None);
        let rc = Input::MouseDown { pos: c, button: MouseButton::Right, mods: Mods::NONE, clicks: 1 };
        assert_eq!(sb.event(&rc, rect, &res, &mut p), Some(SwitchMsg::RightClick(3, c)));
        sb.vertical = true;
        let l = sb.layout(&res, Rect::new(0, 0, 120, 400));
        assert_eq!(l[1].1.x, l[0].1.x);
        assert!(l[1].1.y > l[0].1.y);
    }

    #[test]
    fn treebar_resize_and_activate() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let mut tb = Treebar::new();
        tb.roots = vec![TreebarNode::new(1, "Undernet", None).with_children(vec![TreebarNode::new(2, "#lyon", None)])];
        let rect = Rect::new(0, 0, tb.width, 300);
        let sx = rect.right() - 2;
        tb.event(&Input::click(sx, 50), rect, &res, &mut p);
        assert_eq!(tb.event(&Input::moved(sx + 30, 50), rect, &res, &mut p), Some(TreebarMsg::Resized(180)));
        tb.event(&Input::release(sx + 30, 50), rect, &res, &mut p);
        let rh = tb.tree.row_height(&res);
        let m = tb.event(&Input::click(40, 2 + rh + rh / 2), Rect::new(0, 0, tb.width, 300), &res, &mut p);
        assert_eq!(m, Some(TreebarMsg::Activate(2)));
    }
}
