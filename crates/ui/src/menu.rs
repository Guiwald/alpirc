//! Menus surgissants (avec sous-menus) et barre de menus façon Windows 98.
//!
//! Sur macOS, l'application met ses menus dans la vraie barre du Mac et n'utilise que [`Menu`] pour les menus
//! contextuels ; ailleurs, [`MenuBar`] dessine la barre dans la fenêtre.

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect};
use crate::input::{Input, Key, MouseButton, Platform};
use crate::text::{Align, TextStyle, mnemonic_char, parse_mnemonic};
use crate::widgets::{ArrowDir, CHECK_MARK, MENU_DOT, draw_arrow, draw_pattern};
use crate::Res;

/// Un élément de menu.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuItem {
    /// Identifiant renvoyé quand l'élément est choisi.
    pub id: u32,
    /// Libellé avec raccourci (`&Ouvrir`).
    pub label: String,
    /// Raccourci clavier affiché à droite (`Ctrl+O`).
    pub shortcut: String,
    pub checked: bool,
    /// Coché avec un point (groupe d'options) plutôt qu'une coche.
    pub radio: bool,
    pub enabled: bool,
    pub separator: bool,
    /// Sous-menu (non vide = l'élément ouvre un sous-menu).
    pub submenu: Vec<MenuItem>,
}

impl MenuItem {
    pub fn new(id: u32, label: &str) -> MenuItem {
        MenuItem { id, label: label.to_string(), enabled: true, ..Default::default() }
    }

    pub fn separator() -> MenuItem {
        MenuItem { separator: true, enabled: false, ..Default::default() }
    }

    pub fn submenu(label: &str, items: Vec<MenuItem>) -> MenuItem {
        MenuItem { label: label.to_string(), enabled: true, submenu: items, ..Default::default() }
    }

    pub fn shortcut(mut self, s: &str) -> MenuItem {
        self.shortcut = s.to_string();
        self
    }

    pub fn checked(mut self, on: bool) -> MenuItem {
        self.checked = on;
        self
    }

    pub fn radio(mut self, on: bool) -> MenuItem {
        self.radio = true;
        self.checked = on;
        self
    }

    pub fn disabled(mut self) -> MenuItem {
        self.enabled = false;
        self
    }

    pub fn is_submenu(&self) -> bool {
        !self.submenu.is_empty()
    }

    fn selectable(&self) -> bool {
        !self.separator
    }
}

/// Message d'un menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuMsg {
    /// Un élément a été choisi : son identifiant et son chemin (index à chaque niveau).
    Activated { id: u32, path: Vec<usize> },
    /// Le menu s'est fermé sans choix.
    Closed,
    /// Flèche gauche/droite au premier niveau : passer au menu voisin de la barre (-1 ou +1).
    Navigate(i32),
}

/// Bordure d'un menu (relief de 2 pixels et 1 pixel de face).
const BORDER: i32 = 3;
/// Marge gauche (coche) et droite (flèche de sous-menu).
const LEFT: i32 = 19;
const RIGHT: i32 = 17;
const SEP_H: i32 = 9;

#[derive(Clone, Debug)]
struct Level {
    items: Vec<MenuItem>,
    rect: Rect,
    hover: Option<usize>,
}

/// Un menu surgissant ouvert (et ses sous-menus ouverts).
#[derive(Clone, Debug)]
pub struct Menu {
    levels: Vec<Level>,
    bounds: Rect,
    /// Souligner les lettres de raccourci (toujours sous Windows 98).
    pub show_mnemonics: bool,
    opened_at: Point,
    moved: bool,
}

fn item_h(res: &Res) -> i32 {
    res.ui_line() + 2
}

fn level_size(items: &[MenuItem], res: &Res) -> (i32, i32) {
    let mut lw = 0;
    let mut sw = 0;
    let mut h = 0;
    for it in items {
        if it.separator {
            h += SEP_H;
            continue;
        }
        h += item_h(res);
        lw = lw.max(res.fonts.measure(&parse_mnemonic(&it.label).0, TextStyle::UI));
        if !it.shortcut.is_empty() {
            sw = sw.max(res.fonts.measure(&it.shortcut, TextStyle::UI));
        }
    }
    let w = LEFT + lw + if sw > 0 { 18 + sw } else { 0 } + RIGHT + 2 * BORDER;
    (w.max(80), h + 2 * BORDER)
}

impl Menu {
    /// Ouvre un menu dont le coin haut-gauche est en `at` (déplacé pour rester dans `bounds`).
    pub fn popup(items: Vec<MenuItem>, at: Point, bounds: Rect, res: &Res) -> Menu {
        let (w, h) = level_size(&items, res);
        let mut r = Rect::new(at.x, at.y, w, h);
        if r.right() > bounds.right() {
            r.x = at.x - w;
        }
        if r.bottom() > bounds.bottom() {
            r.y = at.y - h;
        }
        let r = r.clamp_inside(&bounds);
        Menu { levels: vec![Level { items, rect: r, hover: None }], bounds, show_mnemonics: true, opened_at: at, moved: false }
    }

    /// Ouvre un menu sous `anchor` (titre de la barre de menus), ou au-dessus s'il n'y a pas la place.
    pub fn popup_below(items: Vec<MenuItem>, anchor: Rect, bounds: Rect, res: &Res) -> Menu {
        let (w, h) = level_size(&items, res);
        let mut r = Rect::new(anchor.x, anchor.bottom(), w, h);
        if r.bottom() > bounds.bottom() && anchor.y - h >= bounds.y {
            r.y = anchor.y - h;
        }
        if r.right() > bounds.right() {
            r.x = (bounds.right() - w).max(bounds.x);
        }
        Menu { levels: vec![Level { items, rect: r, hover: None }], bounds, show_mnemonics: true, opened_at: Point::new(-1, -1), moved: false }
    }

    /// Rectangle du premier niveau.
    pub fn rect(&self) -> Rect {
        self.levels[0].rect
    }

    /// Rectangles de tous les niveaux ouverts.
    pub fn level_rects(&self) -> Vec<Rect> {
        self.levels.iter().map(|l| l.rect).collect()
    }

    /// Vrai si le point est sur un des niveaux ouverts.
    pub fn contains(&self, p: Point) -> bool {
        self.levels.iter().any(|l| l.rect.contains(p))
    }

    /// Chemin de l'élément survolé (index à chaque niveau).
    pub fn hover_path(&self) -> Vec<usize> {
        self.levels.iter().map_while(|l| l.hover).collect()
    }

    /// Survole le premier élément (ouverture au clavier).
    pub fn hover_first(&mut self) {
        let l = self.levels.last_mut().unwrap();
        l.hover = l.items.iter().position(|i| i.selectable());
    }

    fn item_rects(&self, level: usize, res: &Res) -> Vec<Rect> {
        let l = &self.levels[level];
        let mut y = l.rect.y + BORDER;
        let x = l.rect.x + BORDER;
        let w = l.rect.w - 2 * BORDER;
        l.items
            .iter()
            .map(|it| {
                let h = if it.separator { SEP_H } else { item_h(res) };
                let r = Rect::new(x, y, w, h);
                y += h;
                r
            })
            .collect()
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res) {
        let t = &res.theme;
        for (li, l) in self.levels.iter().enumerate() {
            canvas.fill(l.rect, t.menu);
            canvas.bevel(l.rect, Bevel::Window, t);
            let rects = self.item_rects(li, res);
            let max_sw = l.items.iter().map(|i| res.fonts.measure(&i.shortcut, TextStyle::UI)).max().unwrap_or(0);
            for (i, (it, r)) in l.items.iter().zip(rects).enumerate() {
                if it.separator {
                    canvas.etched_hline(r.x + 1, r.y + SEP_H / 2 - 1, r.w - 2, t);
                    continue;
                }
                let hot = l.hover == Some(i);
                if hot {
                    canvas.fill(r, t.menu_highlight);
                }
                let color = if hot { t.menu_highlight_text } else { t.menu_text };
                let disabled = !it.enabled && !hot;
                let fg = if !it.enabled && hot { t.disabled_text } else { color };
                if it.checked {
                    let cy = r.y + (r.h - 7) / 2;
                    let cx = r.x + 6;
                    let paint_mark = |canvas: &mut Canvas, dx: i32, col: u32| {
                        if it.radio {
                            draw_pattern(canvas, cx + 1 + dx, cy + 1 + dx, &MENU_DOT, |c| (c == b'k').then_some(col));
                        } else {
                            draw_pattern(canvas, cx + dx, cy + dx, &CHECK_MARK, |c| (c == b'k').then_some(col));
                        }
                    };
                    if disabled {
                        paint_mark(canvas, 1, t.highlight);
                        paint_mark(canvas, 0, t.disabled_text);
                    } else {
                        paint_mark(canvas, 0, fg);
                    }
                }
                let tr = Rect::new(r.x + LEFT, r.y, r.w - LEFT - RIGHT, r.h);
                res.fonts.draw_label(canvas, tr, &it.label, TextStyle::UI, fg, Align::Left, self.show_mnemonics, disabled.then_some(t));
                if !it.shortcut.is_empty() {
                    let sr = Rect::new(r.right() - RIGHT - max_sw, r.y, max_sw, r.h);
                    let label = it.shortcut.replace('&', "&&");
                    res.fonts.draw_label(canvas, sr, &label, TextStyle::UI, fg, Align::Left, false, disabled.then_some(t));
                }
                if it.is_submenu() {
                    let ar = Rect::new(r.right() - RIGHT, r.y, RIGHT - 4, r.h);
                    if disabled {
                        draw_arrow(canvas, ar.offset(1, 1), ArrowDir::Right, t.highlight);
                        draw_arrow(canvas, ar, ArrowDir::Right, t.disabled_text);
                    } else {
                        draw_arrow(canvas, ar, ArrowDir::Right, fg);
                    }
                }
            }
        }
    }

    /// Ouvre le sous-menu de l'élément survolé du niveau `li` (ferme les niveaux plus profonds).
    fn open_sub(&mut self, li: usize, res: &Res) {
        self.levels.truncate(li + 1);
        let Some(h) = self.levels[li].hover else { return };
        let it = &self.levels[li].items[h];
        if !it.is_submenu() || !it.enabled {
            return;
        }
        let items = it.submenu.clone();
        let (w, hgt) = level_size(&items, res);
        let ir = self.item_rects(li, res)[h];
        let parent = self.levels[li].rect;
        let mut r = Rect::new(parent.right() - 3, ir.y - BORDER, w, hgt);
        if r.right() > self.bounds.right() {
            r.x = parent.x - w + 3;
        }
        if r.bottom() > self.bounds.bottom() {
            r.y = self.bounds.bottom() - hgt;
        }
        let r = r.clamp_inside(&self.bounds);
        self.levels.push(Level { items, rect: r, hover: None });
    }

    fn hit(&self, p: Point, res: &Res) -> Option<(usize, Option<usize>)> {
        for li in (0..self.levels.len()).rev() {
            if self.levels[li].rect.contains(p) {
                let idx = self.item_rects(li, res).iter().position(|r| r.contains(p));
                return Some((li, idx));
            }
        }
        None
    }

    fn activate(&mut self, li: usize, i: usize, res: &Res) -> Option<MenuMsg> {
        let it = &self.levels[li].items[i];
        if it.separator || !it.enabled {
            return None;
        }
        if it.is_submenu() {
            self.levels[li].hover = Some(i);
            self.open_sub(li, res);
            self.hover_first();
            return None;
        }
        let id = it.id;
        let mut path: Vec<usize> = self.levels[..li].iter().map(|l| l.hover.unwrap_or(0)).collect();
        path.push(i);
        Some(MenuMsg::Activated { id, path })
    }

    pub fn event(&mut self, input: &Input, res: &Res, platform: &mut dyn Platform) -> Option<MenuMsg> {
        match input {
            Input::MouseMove { pos } => {
                if *pos != self.opened_at {
                    self.moved = true;
                }
                match self.hit(*pos, res) {
                    Some((li, idx)) => {
                        let idx = idx.filter(|&i| self.levels[li].items[i].selectable());
                        if self.levels[li].hover != idx || self.levels.len() > li + 1 {
                            // Garder le sous-menu ouvert si on revient sur son élément parent.
                            let same_parent = idx.is_some() && self.levels[li].hover == idx && self.levels.len() > li + 1;
                            self.levels[li].hover = idx;
                            if !same_parent {
                                self.open_sub(li, res);
                            }
                            platform.request_repaint();
                        }
                    }
                    None => {
                        let last = self.levels.last_mut().unwrap();
                        if last.hover.is_some() {
                            last.hover = None;
                            platform.request_repaint();
                        }
                    }
                }
                None
            }
            Input::MouseDown { pos, .. } => {
                if self.hit(*pos, res).is_none() {
                    return Some(MenuMsg::Closed);
                }
                None
            }
            Input::MouseUp { pos, .. } => {
                if !self.moved && *pos == self.opened_at {
                    return None;
                }
                let (li, idx) = self.hit(*pos, res)?;
                let i = idx?;
                if self.levels[li].items[i].is_submenu() {
                    return None;
                }
                self.activate(li, i, res)
            }
            Input::Key { key, mods, text, .. } => {
                let li = self.levels.len() - 1;
                let n = self.levels[li].items.len();
                platform.request_repaint();
                match key {
                    Key::Down | Key::Up => {
                        if n == 0 {
                            return None;
                        }
                        let dir = if *key == Key::Down { 1 } else { n - 1 };
                        let start = self.levels[li].hover.unwrap_or(if *key == Key::Down { n - 1 } else { 0 });
                        let mut i = start;
                        for _ in 0..n {
                            i = (i + dir) % n;
                            if self.levels[li].items[i].selectable() {
                                break;
                            }
                        }
                        self.levels[li].hover = Some(i);
                        None
                    }
                    Key::Right => {
                        if let Some(h) = self.levels[li].hover {
                            if self.levels[li].items[h].is_submenu() && self.levels[li].items[h].enabled {
                                self.open_sub(li, res);
                                self.hover_first();
                                return None;
                            }
                        }
                        Some(MenuMsg::Navigate(1))
                    }
                    Key::Left => {
                        if li > 0 {
                            self.levels.pop();
                            None
                        } else {
                            Some(MenuMsg::Navigate(-1))
                        }
                    }
                    Key::Escape => {
                        if li > 0 {
                            self.levels.pop();
                            None
                        } else {
                            Some(MenuMsg::Closed)
                        }
                    }
                    Key::Enter | Key::Char(' ') if !mods.command() => {
                        let h = self.levels[li].hover?;
                        self.activate(li, h, res)
                    }
                    _ => {
                        let c = text.as_deref().and_then(|s| s.chars().next()).or(match key {
                            Key::Char(c) => Some(*c),
                            _ => None,
                        })?;
                        let c = c.to_lowercase().next()?;
                        let matches: Vec<usize> = (0..n)
                            .filter(|&i| self.levels[li].items[i].selectable() && mnemonic_char(&self.levels[li].items[i].label) == Some(c))
                            .collect();
                        match matches.len() {
                            0 => None,
                            1 => {
                                self.levels[li].hover = Some(matches[0]);
                                self.activate(li, matches[0], res)
                            }
                            _ => {
                                let cur = self.levels[li].hover;
                                let next = matches.iter().copied().find(|&i| Some(i) > cur).unwrap_or(matches[0]);
                                self.levels[li].hover = Some(next);
                                None
                            }
                        }
                    }
                }
            }
            _ => None,
        }
    }
}

/// Barre de menus façon Windows 98 (Linux, Android ; sur macOS l'application utilise la barre du Mac).
#[derive(Clone, Debug, Default)]
pub struct MenuBar {
    /// Titres (avec raccourci, `&Fichier`) et éléments.
    pub menus: Vec<(String, Vec<MenuItem>)>,
    /// Alt est maintenue : souligner les raccourcis.
    pub alt_held: bool,
    open: Option<usize>,
    popup: Option<Menu>,
    hot: Option<usize>,
    keyboard: bool,
}

impl MenuBar {
    pub fn new(menus: Vec<(String, Vec<MenuItem>)>) -> MenuBar {
        MenuBar { menus, ..Default::default() }
    }

    /// Hauteur de la barre.
    pub fn height(res: &Res) -> i32 {
        res.ui_line() + 4
    }

    /// Menu ouvert, s'il y en a un.
    pub fn open_index(&self) -> Option<usize> {
        self.open
    }

    /// Le menu ouvert (pour le dessiner ou le tester).
    pub fn popup(&self) -> Option<&Menu> {
        self.popup.as_ref()
    }

    /// Vrai si la barre capte le clavier (menu ouvert ou navigation au clavier après F10/Alt).
    pub fn is_active(&self) -> bool {
        self.open.is_some() || self.keyboard
    }

    /// Rectangles des titres.
    pub fn title_rects(&self, res: &Res, rect: Rect) -> Vec<Rect> {
        let mut x = rect.x + 2;
        self.menus
            .iter()
            .map(|(label, _)| {
                let w = res.fonts.measure(&parse_mnemonic(label).0, TextStyle::UI) + 12;
                let r = Rect::new(x, rect.y + 1, w, rect.h - 2);
                x += w;
                r
            })
            .collect()
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        canvas.fill(rect, t.face);
        let underline = self.alt_held || self.keyboard || self.open.is_some();
        for (i, r) in self.title_rects(res, rect).into_iter().enumerate() {
            let open = self.open == Some(i);
            if open {
                canvas.bevel(r, Bevel::Sunken, t);
            } else if self.hot == Some(i) {
                canvas.bevel(r, Bevel::RaisedThin, t);
            }
            let o = if open { 1 } else { 0 };
            res.fonts.draw_label(canvas, r.offset(o, o), &self.menus[i].0, TextStyle::UI, t.menu_text, Align::Center, underline, None);
        }
    }

    /// Dessine le menu ouvert (à appeler après tout le reste).
    pub fn paint_popup(&self, canvas: &mut Canvas, res: &Res) {
        if let Some(p) = &self.popup {
            p.paint(canvas, res);
        }
    }

    fn open_menu(&mut self, i: usize, res: &Res, rect: Rect, bounds: Rect, keyboard: bool) {
        let r = self.title_rects(res, rect)[i];
        let mut m = Menu::popup_below(self.menus[i].1.clone(), r, bounds, res);
        if keyboard {
            m.hover_first();
        }
        self.popup = Some(m);
        self.open = Some(i);
        self.hot = Some(i);
    }

    /// Ferme tout.
    pub fn close(&mut self) {
        self.open = None;
        self.popup = None;
        self.keyboard = false;
        self.hot = None;
    }

    /// `rect` : la barre ; `bounds` : où les menus peuvent s'ouvrir (le canevas).
    pub fn event(&mut self, input: &Input, rect: Rect, bounds: Rect, res: &Res, platform: &mut dyn Platform) -> Option<MenuMsg> {
        let n = self.menus.len();
        if n == 0 {
            return None;
        }
        let titles = self.title_rects(res, rect);
        let title_at = |p: Point| titles.iter().position(|r| r.contains(p));
        // Un menu est ouvert : il passe d'abord, sauf la souris sur les titres.
        if let Some(open) = self.open {
            match input {
                Input::MouseMove { pos } if rect.contains(*pos) => {
                    if let Some(i) = title_at(*pos) {
                        if i != open {
                            self.open_menu(i, res, rect, bounds, false);
                            platform.request_repaint();
                        }
                    }
                    return None;
                }
                Input::MouseDown { pos, .. } if rect.contains(*pos) => {
                    if title_at(*pos) == Some(open) {
                        self.close();
                        platform.request_repaint();
                        return Some(MenuMsg::Closed);
                    }
                    if let Some(i) = title_at(*pos) {
                        self.open_menu(i, res, rect, bounds, false);
                        platform.request_repaint();
                    }
                    return None;
                }
                Input::MouseUp { pos, .. } if rect.contains(*pos) => return None,
                _ => {}
            }
            let msg = self.popup.as_mut()?.event(input, res, platform);
            platform.request_repaint();
            return match msg {
                Some(MenuMsg::Navigate(d)) => {
                    let i = ((open as i32 + d).rem_euclid(n as i32)) as usize;
                    self.open_menu(i, res, rect, bounds, true);
                    None
                }
                Some(MenuMsg::Closed) => {
                    let by_key = matches!(input, Input::Key { .. });
                    self.open = None;
                    self.popup = None;
                    // Échap revient à la barre (titre en surbrillance), un clic ailleurs ferme tout.
                    self.keyboard = by_key;
                    self.hot = if by_key { Some(open) } else { None };
                    Some(MenuMsg::Closed)
                }
                Some(m @ MenuMsg::Activated { .. }) => {
                    let MenuMsg::Activated { id, path } = m else { unreachable!() };
                    self.close();
                    let mut full = vec![open];
                    full.extend(path);
                    Some(MenuMsg::Activated { id, path: full })
                }
                None => None,
            };
        }
        match input {
            Input::MouseMove { pos } => {
                let h = if rect.contains(*pos) { title_at(*pos) } else { None };
                if h != self.hot && !self.keyboard {
                    self.hot = h;
                    platform.request_repaint();
                }
            }
            Input::MouseLeave if !self.keyboard => {
                if self.hot.take().is_some() {
                    platform.request_repaint();
                }
            }
            Input::MouseDown { pos, button: MouseButton::Left, .. } => {
                if let Some(i) = title_at(*pos) {
                    self.keyboard = false;
                    self.open_menu(i, res, rect, bounds, false);
                    platform.request_repaint();
                } else if self.keyboard {
                    self.close();
                    platform.request_repaint();
                }
            }
            Input::Key { key: Key::F(10), mods, .. } if mods.is_empty() => {
                self.keyboard = !self.keyboard;
                self.hot = if self.keyboard { Some(0) } else { None };
                platform.request_repaint();
            }
            Input::Key { key: Key::Char(c), mods, .. } if mods.alt && !mods.command() => {
                let c = c.to_lowercase().next()?;
                let i = self.menus.iter().position(|(l, _)| mnemonic_char(l) == Some(c))?;
                self.keyboard = true;
                self.open_menu(i, res, rect, bounds, true);
                platform.request_repaint();
            }
            Input::Key { key, text, .. } if self.keyboard => {
                let h = self.hot.unwrap_or(0);
                match key {
                    Key::Left => self.hot = Some((h + n - 1) % n),
                    Key::Right => self.hot = Some((h + 1) % n),
                    Key::Down | Key::Enter | Key::Up => self.open_menu(h, res, rect, bounds, true),
                    Key::Escape => self.close(),
                    _ => {
                        let c = text.as_deref().and_then(|s| s.chars().next())?.to_lowercase().next()?;
                        let i = self.menus.iter().position(|(l, _)| mnemonic_char(l) == Some(c))?;
                        self.open_menu(i, res, rect, bounds, true);
                    }
                }
                platform.request_repaint();
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Mods, TestPlatform};

    fn items() -> Vec<MenuItem> {
        vec![
            MenuItem::new(1, "&Connecter").shortcut("Ctrl+N"),
            MenuItem::new(2, "&Déconnecter").disabled(),
            MenuItem::separator(),
            MenuItem::submenu("&Couleurs", vec![MenuItem::new(10, "&Rouge"), MenuItem::new(11, "&Bleu").checked(true)]),
            MenuItem::new(3, "&Quitter"),
        ]
    }

    #[test]
    fn keyboard_navigation() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let bounds = Rect::new(0, 0, 640, 480);
        let mut m = Menu::popup(items(), Point::new(10, 10), bounds, &res);
        let k = |key| Input::key(key, Mods::NONE);
        m.event(&k(Key::Down), &res, &mut p);
        assert_eq!(m.hover_path(), vec![0]);
        m.event(&k(Key::Down), &res, &mut p);
        assert_eq!(m.hover_path(), vec![1], "les éléments grisés se survolent");
        m.event(&k(Key::Down), &res, &mut p);
        assert_eq!(m.hover_path(), vec![3], "le séparateur est sauté");
        m.event(&k(Key::Right), &res, &mut p);
        assert_eq!(m.hover_path(), vec![3, 0]);
        m.event(&k(Key::Down), &res, &mut p);
        assert_eq!(m.event(&k(Key::Enter), &res, &mut p), Some(MenuMsg::Activated { id: 11, path: vec![3, 1] }));
        let mut m = Menu::popup(items(), Point::new(10, 10), bounds, &res);
        assert_eq!(m.event(&k(Key::Char('q')), &res, &mut p), Some(MenuMsg::Activated { id: 3, path: vec![4] }));
        // Deux raccourcis « c » : on alterne.
        assert_eq!(m.event(&k(Key::Char('c')), &res, &mut p), None);
        assert_eq!(m.hover_path(), vec![0]);
        m.event(&k(Key::Char('c')), &res, &mut p);
        assert_eq!(m.hover_path(), vec![3]);
        m.event(&k(Key::Right), &res, &mut p);
        m.event(&k(Key::Escape), &res, &mut p);
        assert_eq!(m.hover_path(), vec![3]);
        assert_eq!(m.event(&k(Key::Escape), &res, &mut p), Some(MenuMsg::Closed));
        assert_eq!(m.event(&k(Key::Left), &res, &mut p), Some(MenuMsg::Navigate(-1)));
    }

    #[test]
    fn stays_inside_bounds_and_mouse() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let bounds = Rect::new(0, 0, 300, 200);
        let mut m = Menu::popup(items(), Point::new(290, 190), bounds, &res);
        let r = m.rect();
        assert!(r.right() <= 300 && r.bottom() <= 200 && r.x >= 0 && r.y >= 0, "{r:?}");
        // Survoler « Couleurs » ouvre le sous-menu, lui aussi dans les limites.
        let ir = m.item_rects(0, &res)[3];
        m.event(&Input::moved(ir.x + 5, ir.y + 3), &res, &mut p);
        let levels = m.level_rects();
        assert_eq!(levels.len(), 2);
        assert!(levels[1].right() <= 300 && levels[1].x >= 0);
        let sub = m.item_rects(1, &res)[0];
        m.event(&Input::moved(sub.x + 5, sub.y + 3), &res, &mut p);
        assert_eq!(m.event(&Input::release(sub.x + 5, sub.y + 3), &res, &mut p), Some(MenuMsg::Activated { id: 10, path: vec![3, 0] }));
        assert_eq!(m.event(&Input::click(1, 1), &res, &mut p), Some(MenuMsg::Closed));
    }

    #[test]
    fn menubar_open_and_navigate() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let bounds = Rect::new(0, 0, 640, 480);
        let bar_r = Rect::new(0, 0, 640, MenuBar::height(&res));
        let mut bar = MenuBar::new(vec![("&Fichier".into(), items()), ("&Outils".into(), items())]);
        let t = bar.title_rects(&res, bar_r);
        bar.event(&Input::click(t[0].x + 3, 5), bar_r, bounds, &res, &mut p);
        assert_eq!(bar.open_index(), Some(0));
        bar.event(&Input::moved(t[1].x + 3, 5), bar_r, bounds, &res, &mut p);
        assert_eq!(bar.open_index(), Some(1));
        bar.event(&Input::key(Key::Right, Mods::NONE), bar_r, bounds, &res, &mut p);
        assert_eq!(bar.open_index(), Some(0), "Droite sans sous-menu : menu suivant (boucle)");
        assert_eq!(bar.event(&Input::key(Key::Enter, Mods::NONE), bar_r, bounds, &res, &mut p), Some(MenuMsg::Activated { id: 1, path: vec![0, 0] }));
        assert_eq!(bar.open_index(), None);
        bar.event(&Input::Key { key: Key::Char('o'), mods: Mods::ALT, text: None, repeat: false }, bar_r, bounds, &res, &mut p);
        assert_eq!(bar.open_index(), Some(1));
        bar.event(&Input::key(Key::Escape, Mods::NONE), bar_r, bounds, &res, &mut p);
        assert_eq!(bar.open_index(), None);
        assert!(bar.is_active(), "Échap revient à la barre");
    }
}
