//! Fenêtres internes (MDI) façon Windows 98 : cadres avec barre de titre en dégradé, petite icône, titre
//! raccourci, boutons réduire/agrandir/restaurer/fermer ; déplacement par la barre de titre,
//! redimensionnement par les bords et les coins, double-clic sur le titre, fenêtres réduites alignées en bas,
//! cascade, mosaïques, fenêtre suivante/précédente (Ctrl+Tab). Une fenêtre agrandie remplit la zone sans
//! cadre ; ses boutons passent à droite de la barre de menus (voir [`MdiArea::maximized_controls_rect`]).
//!
//! La zone dessine les cadres et donne à l'application les rectangles de contenu ; l'application dessine le
//! contenu de chaque fenêtre (rappel de [`MdiArea::paint`]) et reçoit les événements qui lui reviennent
//! ([`MdiResponse::Content`]).

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect, Size};
use crate::icons::IconId;
use crate::input::{Cursor, Input, Key, MouseButton, Platform};
use crate::text::TextStyle;
use crate::widgets::draw_pattern;
use crate::Res;

/// Épaisseur du cadre redimensionnable.
pub const BORDER: i32 = 4;
/// Hauteur de la barre de titre.
pub const TITLE_H: i32 = 18;
/// Taille d'un bouton de barre de titre.
pub const CAPTION_BTN: Size = Size::new(16, 14);
/// Largeur d'une fenêtre réduite.
pub const MINIMIZED_W: i32 = 160;
/// Hauteur d'une fenêtre réduite (cadre fin de 3 pixels).
pub const MINIMIZED_H: i32 = TITLE_H + 6;
/// Taille du coin de redimensionnement le long d'un bord.
const CORNER: i32 = 16;

/// État d'une fenêtre interne.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowState {
    #[default]
    Normal,
    Minimized,
    Maximized,
}

/// Une fenêtre interne.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MdiChild {
    pub id: u64,
    pub title: String,
    pub icon: Option<IconId>,
    /// Rectangle à l'état normal, **relatif à la zone MDI**.
    pub rect: Rect,
    pub state: WindowState,
    /// Taille minimale (cadre compris).
    pub min_size: Size,
}

impl MdiChild {
    pub fn new(id: u64, title: &str, icon: Option<IconId>, rect: Rect) -> MdiChild {
        MdiChild { id, title: title.to_string(), icon, rect, state: WindowState::Normal, min_size: Size::new(120, TITLE_H + 2 * BORDER + 20) }
    }
}

/// Bouton de barre de titre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptionButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// Réponse de la zone MDI.
#[derive(Clone, Debug, PartialEq)]
pub enum MdiResponse {
    /// Une fenêtre est devenue active.
    Activated(u64),
    /// On a cliqué sur le bouton de fermeture (ou Ctrl+F4) : à l'application de fermer.
    CloseRequested(u64),
    /// Fenêtre déplacée ou redimensionnée (fin du geste).
    Moved(u64),
    /// Fenêtre réduite, agrandie ou restaurée.
    StateChanged(u64, WindowState),
    /// Clic sur la petite icône (menu de la fenêtre) : position du menu.
    SystemMenu(u64, Point),
    /// Événement pour le contenu, **position relative au coin du rectangle de contenu**.
    Content(u64, Input),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragKind {
    Move,
    /// Bords : gauche, haut, droite, bas.
    Resize(bool, bool, bool, bool),
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    id: u64,
    kind: DragKind,
    start: Point,
    start_rect: Rect,
}

/// Partie d'une fenêtre sous la souris.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MdiHit {
    Title,
    Icon,
    Button(CaptionButton),
    /// Bords (gauche, haut, droite, bas).
    Border(bool, bool, bool, bool),
    Content,
}

/// La zone des fenêtres internes.
#[derive(Clone, Debug, Default)]
pub struct MdiArea {
    /// Fenêtres, de la plus basse à la plus haute (ordre d'empilement).
    pub children: Vec<MdiChild>,
    /// L'application a le focus (barres de titre actives en couleur).
    pub app_focused: bool,
    active: Option<u64>,
    maximized: bool,
    drag: Option<Drag>,
    pressed: Option<(u64, CaptionButton)>,
    pressed_over: bool,
    capture: Option<u64>,
    /// Emplacements des fenêtres réduites (en bas de la zone), dans l'ordre.
    slots: Vec<Option<u64>>,
}

impl MdiArea {
    pub fn new() -> MdiArea {
        MdiArea { app_focused: true, ..Default::default() }
    }

    /// Ajoute une fenêtre (au premier plan, active ; agrandie si la zone est en mode agrandi).
    pub fn add(&mut self, mut child: MdiChild) -> Vec<MdiResponse> {
        if self.maximized && child.state == WindowState::Normal {
            child.state = WindowState::Maximized;
        }
        let id = child.id;
        let minimized = child.state == WindowState::Minimized;
        self.children.push(child);
        if minimized {
            self.take_slot(id);
            let i = self.children.len() - 1;
            let c = self.children.remove(i);
            self.children.insert(0, c);
            return Vec::new();
        }
        self.activate(id)
    }

    /// Retire une fenêtre ; active la suivante.
    pub fn remove(&mut self, id: u64) -> Vec<MdiResponse> {
        self.children.retain(|c| c.id != id);
        self.free_slot(id);
        if self.capture == Some(id) {
            self.capture = None;
        }
        if self.active == Some(id) {
            self.active = None;
            if let Some(next) = self.children.iter().rev().find(|c| c.state != WindowState::Minimized).or(self.children.last()).map(|c| c.id) {
                return self.activate(next);
            }
            self.maximized = false;
        }
        Vec::new()
    }

    pub fn active(&self) -> Option<u64> {
        self.active
    }

    /// Vrai si la zone est en mode agrandi.
    pub fn is_maximized(&self) -> bool {
        self.maximized && self.active.is_some()
    }

    pub fn child(&self, id: u64) -> Option<&MdiChild> {
        self.children.iter().find(|c| c.id == id)
    }

    pub fn child_mut(&mut self, id: u64) -> Option<&mut MdiChild> {
        self.children.iter_mut().find(|c| c.id == id)
    }

    /// Active une fenêtre (premier plan).
    pub fn activate(&mut self, id: u64) -> Vec<MdiResponse> {
        let Some(i) = self.children.iter().position(|c| c.id == id) else { return Vec::new() };
        let c = self.children.remove(i);
        self.children.push(c);
        let mut out = Vec::new();
        if self.maximized {
            for c in &mut self.children {
                if c.id != id && c.state == WindowState::Maximized {
                    c.state = WindowState::Normal;
                }
            }
            let c = self.children.last_mut().unwrap();
            if c.state != WindowState::Maximized {
                c.state = WindowState::Maximized;
                out.push(MdiResponse::StateChanged(id, WindowState::Maximized));
            }
        }
        if self.active != Some(id) {
            self.active = Some(id);
            out.insert(0, MdiResponse::Activated(id));
        }
        out
    }

    /// Agrandit une fenêtre (mode agrandi pour toute la zone).
    pub fn maximize(&mut self, id: u64) -> Vec<MdiResponse> {
        self.maximized = true;
        let mut out = self.activate(id);
        if let Some(c) = self.child_mut(id) {
            if c.state != WindowState::Maximized {
                c.state = WindowState::Maximized;
                out.push(MdiResponse::StateChanged(id, WindowState::Maximized));
            }
        }
        out
    }

    /// Restaure une fenêtre (réduite : à l'état normal, ou agrandie si la zone l'est ; agrandie : quitte le
    /// mode agrandi).
    pub fn restore(&mut self, id: u64) -> Vec<MdiResponse> {
        let Some(c) = self.child(id) else { return Vec::new() };
        let mut out = Vec::new();
        match c.state {
            WindowState::Maximized => {
                self.maximized = false;
                for c in &mut self.children {
                    if c.state == WindowState::Maximized {
                        c.state = WindowState::Normal;
                        out.push(MdiResponse::StateChanged(c.id, WindowState::Normal));
                    }
                }
                out.extend(self.activate(id));
            }
            WindowState::Minimized => {
                self.free_slot(id);
                let st = if self.maximized { WindowState::Maximized } else { WindowState::Normal };
                self.child_mut(id).unwrap().state = st;
                out.push(MdiResponse::StateChanged(id, st));
                out.extend(self.activate(id));
            }
            WindowState::Normal => out.extend(self.activate(id)),
        }
        out
    }

    /// Réduit une fenêtre ; la suivante devient active.
    pub fn minimize(&mut self, id: u64) -> Vec<MdiResponse> {
        let Some(c) = self.child_mut(id) else { return Vec::new() };
        c.state = WindowState::Minimized;
        self.take_slot(id);
        let mut out = vec![MdiResponse::StateChanged(id, WindowState::Minimized)];
        // Passer en bas de la pile.
        let i = self.children.iter().position(|c| c.id == id).unwrap();
        let c = self.children.remove(i);
        self.children.insert(0, c);
        if self.active == Some(id) {
            if let Some(next) = self.children.iter().rev().find(|c| c.state != WindowState::Minimized).map(|c| c.id) {
                out.extend(self.activate(next));
            } else {
                self.maximized = false;
            }
        }
        out
    }

    /// Active la fenêtre suivante (Ctrl+Tab) : l'active passe derrière toutes les autres.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Vec<MdiResponse> {
        if self.children.len() < 2 {
            return Vec::new();
        }
        let top = self.children.pop().unwrap();
        self.children.insert(0, top);
        let id = self.children.last().unwrap().id;
        self.active = None;
        self.reveal(id)
    }

    /// Active la fenêtre précédente (Ctrl+Maj+Tab) : celle du fond revient devant.
    pub fn prev(&mut self) -> Vec<MdiResponse> {
        if self.children.len() < 2 {
            return Vec::new();
        }
        let id = self.children[0].id;
        self.reveal(id)
    }

    fn reveal(&mut self, id: u64) -> Vec<MdiResponse> {
        if self.child(id).is_some_and(|c| c.state == WindowState::Minimized) {
            self.restore(id)
        } else {
            self.activate(id)
        }
    }

    /// Fenêtres non réduites, de bas en haut.
    fn normal_ids(&self) -> Vec<u64> {
        self.children.iter().filter(|c| c.state != WindowState::Minimized).map(|c| c.id).collect()
    }

    fn leave_maximized(&mut self) {
        if self.maximized {
            self.maximized = false;
            for c in &mut self.children {
                if c.state == WindowState::Maximized {
                    c.state = WindowState::Normal;
                }
            }
        }
    }

    /// Hauteur réservée en bas aux fenêtres réduites.
    fn icons_height(&self, area: Size) -> i32 {
        let n = self.slots.len() as i32;
        if n == 0 {
            return 0;
        }
        let per_row = (area.w / MINIMIZED_W).max(1);
        ((n + per_row - 1) / per_row) * MINIMIZED_H
    }

    /// Cascade : chaque fenêtre décalée de la hauteur d'une barre de titre.
    pub fn cascade(&mut self, area: Size) {
        self.leave_maximized();
        let ids = self.normal_ids();
        let step = TITLE_H + BORDER;
        let avail = Size::new(area.w, area.h - self.icons_height(area));
        let w = (avail.w * 3 / 4).max(160);
        let h = (avail.h * 3 / 4).max(TITLE_H + 2 * BORDER + 40);
        let fit = (((avail.h - h) / step).max(0) + 1) as usize;
        for (k, id) in ids.iter().enumerate() {
            let i = (k % fit) as i32;
            if let Some(c) = self.child_mut(*id) {
                c.rect = Rect::new(i * step, i * step, w, h);
            }
        }
        self.arrange_icons(area);
    }

    /// Mosaïque horizontale : les fenêtres empilées les unes au-dessus des autres, sur toute la largeur.
    pub fn tile_horizontal(&mut self, area: Size) {
        self.leave_maximized();
        let ids = self.normal_ids();
        let n = ids.len() as i32;
        if n == 0 {
            return;
        }
        let avail_h = area.h - self.icons_height(area);
        for (k, id) in ids.iter().rev().enumerate() {
            let k = k as i32;
            let y0 = avail_h * k / n;
            let y1 = avail_h * (k + 1) / n;
            if let Some(c) = self.child_mut(*id) {
                c.rect = Rect::new(0, y0, area.w, y1 - y0);
            }
        }
        self.arrange_icons(area);
    }

    /// Mosaïque verticale : les fenêtres côte à côte, sur toute la hauteur.
    pub fn tile_vertical(&mut self, area: Size) {
        self.leave_maximized();
        let ids = self.normal_ids();
        let n = ids.len() as i32;
        if n == 0 {
            return;
        }
        let avail_h = area.h - self.icons_height(area);
        for (k, id) in ids.iter().rev().enumerate() {
            let k = k as i32;
            let x0 = area.w * k / n;
            let x1 = area.w * (k + 1) / n;
            if let Some(c) = self.child_mut(*id) {
                c.rect = Rect::new(x0, 0, x1 - x0, avail_h);
            }
        }
        self.arrange_icons(area);
    }

    /// Range les fenêtres réduites en bas, de gauche à droite puis en remontant, sans trous (« Réorganiser
    /// les icônes »). Sinon, une fenêtre réduite garde sa place quand les autres sont restaurées.
    pub fn arrange_icons(&mut self, _area: Size) {
        let ids: Vec<u64> = self.slots.iter().flatten().copied().filter(|id| self.child(*id).is_some_and(|c| c.state == WindowState::Minimized)).collect();
        self.slots = ids.into_iter().map(Some).collect();
    }

    fn take_slot(&mut self, id: u64) {
        if self.slots.contains(&Some(id)) {
            return;
        }
        match self.slots.iter().position(|s| s.is_none()) {
            Some(i) => self.slots[i] = Some(id),
            None => self.slots.push(Some(id)),
        }
    }

    fn free_slot(&mut self, id: u64) {
        for s in &mut self.slots {
            if *s == Some(id) {
                *s = None;
            }
        }
        while self.slots.last() == Some(&None) {
            self.slots.pop();
        }
    }

    /// Rectangle d'une fenêtre réduite (relatif à la zone), d'après son emplacement.
    fn minimized_rect(&self, id: u64, area: Size) -> Rect {
        let per_row = (area.w / MINIMIZED_W).max(1);
        let k = self.slots.iter().position(|s| *s == Some(id)).unwrap_or(0) as i32;
        Rect::new((k % per_row) * MINIMIZED_W, area.h - MINIMIZED_H * (k / per_row + 1), MINIMIZED_W, MINIMIZED_H)
    }

    /// Rectangle (absolu) du cadre d'une fenêtre.
    pub fn frame_rect(&self, id: u64, area: Rect) -> Option<Rect> {
        let c = self.child(id)?;
        Some(match c.state {
            WindowState::Maximized => area,
            WindowState::Minimized => self.minimized_rect(id, area.size()).offset(area.x, area.y),
            WindowState::Normal => c.rect.offset(area.x, area.y),
        })
    }

    fn border_of(state: WindowState) -> i32 {
        if state == WindowState::Minimized { 3 } else { BORDER }
    }

    /// Barre de titre (absolue) d'une fenêtre visible avec cadre.
    pub fn title_rect(&self, id: u64, area: Rect) -> Option<Rect> {
        let c = self.child(id)?;
        if c.state == WindowState::Maximized {
            return None;
        }
        let f = self.frame_rect(id, area)?;
        let b = Self::border_of(c.state);
        Some(Rect::new(f.x + b, f.y + b, f.w - 2 * b, TITLE_H))
    }

    /// Rectangle (absolu) du contenu d'une fenêtre (`None` si réduite).
    pub fn content_rect(&self, id: u64, area: Rect) -> Option<Rect> {
        let c = self.child(id)?;
        match c.state {
            WindowState::Minimized => None,
            WindowState::Maximized => Some(area),
            WindowState::Normal => {
                let f = c.rect.offset(area.x, area.y);
                Some(Rect::new(f.x + BORDER, f.y + BORDER + TITLE_H + 1, f.w - 2 * BORDER, f.h - 2 * BORDER - TITLE_H - 1))
            }
        }
    }

    /// Boutons de la barre de titre (absolus).
    pub fn caption_buttons(&self, id: u64, area: Rect) -> Vec<(CaptionButton, Rect)> {
        let Some(t) = self.title_rect(id, area) else { return Vec::new() };
        let c = self.child(id).unwrap();
        let y = t.y + (TITLE_H - CAPTION_BTN.h) / 2;
        let close = Rect::new(t.right() - 2 - CAPTION_BTN.w, y, CAPTION_BTN.w, CAPTION_BTN.h);
        let max = close.offset(-CAPTION_BTN.w - 2, 0);
        let min = max.offset(-CAPTION_BTN.w, 0);
        let (a, b) = match c.state {
            WindowState::Minimized => (CaptionButton::Restore, CaptionButton::Maximize),
            _ => (CaptionButton::Minimize, CaptionButton::Maximize),
        };
        vec![(a, min), (b, max), (CaptionButton::Close, close)]
    }

    /// Partie d'une fenêtre sous un point (absolu).
    pub fn hit(&self, id: u64, area: Rect, p: Point) -> Option<MdiHit> {
        let c = self.child(id)?;
        let f = self.frame_rect(id, area)?;
        if !f.contains(p) {
            return None;
        }
        if c.state == WindowState::Maximized {
            return Some(MdiHit::Content);
        }
        if let Some((b, _)) = self.caption_buttons(id, area).into_iter().find(|(_, r)| r.contains(p)) {
            return Some(MdiHit::Button(b));
        }
        let t = self.title_rect(id, area)?;
        if t.contains(p) {
            if p.x < t.x + 18 {
                return Some(MdiHit::Icon);
            }
            return Some(MdiHit::Title);
        }
        if c.state == WindowState::Minimized {
            return Some(MdiHit::Title);
        }
        let inner = f.inset(BORDER);
        if !inner.contains(p) {
            let l = p.x < inner.x || (p.x < f.x + CORNER && (p.y < inner.y || p.y >= inner.bottom()));
            let r = p.x >= inner.right() || (p.x >= f.right() - CORNER && (p.y < inner.y || p.y >= inner.bottom()));
            let tp = p.y < inner.y || (p.y < f.y + CORNER && (p.x < inner.x || p.x >= inner.right()));
            let bt = p.y >= inner.bottom() || (p.y >= f.bottom() - CORNER && (p.x < inner.x || p.x >= inner.right()));
            return Some(MdiHit::Border(l, tp, r, bt));
        }
        Some(MdiHit::Content)
    }

    /// Fenêtre visible la plus haute sous un point.
    pub fn child_at(&self, area: Rect, p: Point) -> Option<u64> {
        if self.is_maximized() {
            return area.contains(p).then_some(self.active?);
        }
        self.children.iter().rev().find(|c| self.frame_rect(c.id, area).is_some_and(|f| f.contains(p))).map(|c| c.id)
    }

    fn cursor_for(hit: MdiHit) -> Cursor {
        match hit {
            MdiHit::Border(l, t, r, b) => match (l || r, t || b) {
                (true, false) => Cursor::ResizeEW,
                (false, true) => Cursor::ResizeNS,
                _ if (l && t) || (r && b) => Cursor::ResizeNWSE,
                _ => Cursor::ResizeNESW,
            },
            _ => Cursor::Default,
        }
    }

    /// Dessine la zone : fond, cadres, puis le contenu de chaque fenêtre par `content(id, canvas, rect)`.
    pub fn paint(&self, canvas: &mut Canvas, res: &Res, area: Rect, content: &mut dyn FnMut(u64, &mut Canvas, Rect)) {
        let t = &res.theme;
        canvas.with_clip(area, |canvas| {
            canvas.fill(area, t.workspace);
            if self.is_maximized() {
                let id = self.active.unwrap();
                canvas.with_clip(area, |c| content(id, c, area));
                return;
            }
            for c in &self.children {
                let Some(f) = self.frame_rect(c.id, area) else { continue };
                let active = self.active == Some(c.id);
                canvas.fill(f, t.face);
                canvas.bevel(f, Bevel::Window, t);
                let tr = self.title_rect(c.id, area).unwrap();
                let (from, to, fg) = if active && self.app_focused {
                    (t.title_active_from, t.title_active_to, t.title_active_text)
                } else {
                    (t.title_inactive_from, t.title_inactive_to, t.title_inactive_text)
                };
                canvas.gradient_h(tr, from, to);
                let mut tx = tr.x + 3;
                if let Some(img) = c.icon.and_then(|i| res.icons.image(i)) {
                    canvas.blit(img, tr.x + 1, tr.y + (TITLE_H - img.h as i32) / 2);
                    tx = tr.x + img.w as i32 + 3;
                }
                let btns = self.caption_buttons(c.id, area);
                let tw = btns.first().map_or(tr.right(), |b| b.1.x) - tx - 4;
                let label = res.fonts.ellipsize(&c.title, TextStyle::UI_BOLD, tw);
                canvas.with_clip(tr, |cv| {
                    res.fonts.draw_text(cv, tx, tr.y + (TITLE_H - res.ui_line()) / 2, &label, TextStyle::UI_BOLD, fg);
                });
                for (b, r) in btns {
                    let down = self.pressed == Some((c.id, b)) && self.pressed_over;
                    paint_caption_button(canvas, res, r, b, down);
                }
                if let Some(cr) = self.content_rect(c.id, area) {
                    canvas.with_clip(cr, |cv| content(c.id, cv, cr));
                }
            }
        });
    }

    /// Fin d'un geste de déplacement ou de redimensionnement.
    fn apply_drag(&mut self, d: Drag, p: Point, area: Rect) {
        let dx = p.x - d.start.x;
        let dy = p.y - d.start.y;
        let Some(c) = self.child_mut(d.id) else { return };
        let min = c.min_size;
        let s = d.start_rect;
        let mut r = s;
        match d.kind {
            DragKind::Move => {
                r = s.offset(dx, dy);
                // Garder la barre de titre atteignable.
                r.y = r.y.clamp(0, (area.h - TITLE_H - BORDER).max(0));
                r.x = r.x.clamp(-r.w + 48, (area.w - 48).max(0));
            }
            DragKind::Resize(l, t, rt, b) => {
                if l {
                    let nx = (s.x + dx).min(s.right() - min.w);
                    r.x = nx;
                    r.w = s.right() - nx;
                }
                if rt {
                    r.w = (s.w + dx).max(min.w);
                }
                if t {
                    let ny = (s.y + dy).min(s.bottom() - min.h).max(0);
                    r.y = ny;
                    r.h = s.bottom() - ny;
                }
                if b {
                    r.h = (s.h + dy).max(min.h);
                }
            }
        }
        c.rect = r;
    }

    /// Traite un événement. `area` : rectangle (absolu) de la zone.
    pub fn event(&mut self, input: &Input, area: Rect, _res: &Res, platform: &mut dyn Platform) -> Vec<MdiResponse> {
        let mut out = Vec::new();
        let local = |id: u64, me: &MdiArea, inp: &Input| -> Option<MdiResponse> {
            let cr = me.content_rect(id, area)?;
            Some(MdiResponse::Content(id, inp.translated(-cr.x, -cr.y)))
        };
        match input {
            Input::MouseMove { pos } => {
                if let Some(d) = self.drag {
                    self.apply_drag(d, *pos, area);
                    platform.request_repaint();
                    return out;
                }
                if let Some((id, b)) = self.pressed {
                    let over = self.caption_buttons(id, area).iter().any(|(bb, r)| *bb == b && r.contains(*pos));
                    if over != self.pressed_over {
                        self.pressed_over = over;
                        platform.request_repaint();
                    }
                    return out;
                }
                if let Some(id) = self.capture {
                    out.extend(local(id, self, input));
                    return out;
                }
                match self.child_at(area, *pos) {
                    Some(id) => {
                        let hit = self.hit(id, area, *pos);
                        if hit == Some(MdiHit::Content) {
                            out.extend(local(id, self, input));
                        } else if let Some(h) = hit {
                            platform.set_cursor(Self::cursor_for(h));
                        }
                    }
                    None => platform.set_cursor(Cursor::Default),
                }
            }
            Input::MouseDown { pos, button, clicks, .. } => {
                let Some(id) = self.child_at(area, *pos) else { return out };
                let hit = self.hit(id, area, *pos);
                if self.active != Some(id) {
                    out.extend(self.activate(id));
                    platform.request_repaint();
                }
                match hit {
                    Some(MdiHit::Button(b)) if *button == MouseButton::Left => {
                        self.pressed = Some((id, b));
                        self.pressed_over = true;
                        platform.request_repaint();
                    }
                    Some(MdiHit::Icon) if *clicks >= 2 => out.push(MdiResponse::CloseRequested(id)),
                    Some(MdiHit::Icon) => out.push(MdiResponse::SystemMenu(id, *pos)),
                    Some(MdiHit::Title) if *button == MouseButton::Left => {
                        let st = self.child(id).unwrap().state;
                        if *clicks >= 2 {
                            out.extend(match st {
                                WindowState::Normal => self.maximize(id),
                                _ => self.restore(id),
                            });
                            platform.request_repaint();
                        } else if st == WindowState::Normal {
                            self.drag = Some(Drag { id, kind: DragKind::Move, start: *pos, start_rect: self.child(id).unwrap().rect });
                        }
                    }
                    Some(MdiHit::Title) => out.push(MdiResponse::SystemMenu(id, *pos)),
                    Some(MdiHit::Border(l, t, r, b)) if *button == MouseButton::Left => {
                        self.drag = Some(Drag { id, kind: DragKind::Resize(l, t, r, b), start: *pos, start_rect: self.child(id).unwrap().rect });
                        platform.set_cursor(Self::cursor_for(MdiHit::Border(l, t, r, b)));
                    }
                    Some(MdiHit::Content) => {
                        self.capture = Some(id);
                        out.extend(local(id, self, input));
                    }
                    _ => {}
                }
            }
            Input::MouseUp { pos, .. } => {
                if let Some(d) = self.drag.take() {
                    self.apply_drag(d, *pos, area);
                    out.push(MdiResponse::Moved(d.id));
                    platform.request_repaint();
                    return out;
                }
                if let Some((id, b)) = self.pressed.take() {
                    platform.request_repaint();
                    let over = self.caption_buttons(id, area).iter().any(|(bb, r)| *bb == b && r.contains(*pos));
                    if over {
                        out.extend(match b {
                            CaptionButton::Minimize => self.minimize(id),
                            CaptionButton::Maximize => self.maximize(id),
                            CaptionButton::Restore => self.restore(id),
                            CaptionButton::Close => vec![MdiResponse::CloseRequested(id)],
                        });
                    }
                    return out;
                }
                if let Some(id) = self.capture.take() {
                    out.extend(local(id, self, input));
                }
            }
            Input::Wheel { pos, .. } => {
                if let Some(id) = self.child_at(area, *pos).filter(|&id| self.hit(id, area, *pos) == Some(MdiHit::Content)) {
                    out.extend(local(id, self, input));
                }
            }
            Input::Key { key, mods, .. } => {
                let ctrl = mods.ctrl;
                match key {
                    Key::Tab | Key::F(6) if ctrl => {
                        out.extend(if mods.shift { self.prev() } else { self.next() });
                        platform.request_repaint();
                    }
                    Key::F(4) if ctrl => out.extend(self.active.map(MdiResponse::CloseRequested)),
                    _ => out.extend(self.active.and_then(|id| local(id, self, input).or(Some(MdiResponse::Content(id, input.clone()))))),
                }
            }
            Input::MouseLeave => {
                for c in &self.children {
                    out.push(MdiResponse::Content(c.id, Input::MouseLeave));
                }
            }
            _ => out.extend(self.active.map(|id| MdiResponse::Content(id, input.clone()))),
        }
        out
    }

    /// Mode agrandi : rectangle (dans la barre de menus) des boutons réduire, restaurer, fermer de la fenêtre
    /// active, à droite de la barre. `None` hors du mode agrandi.
    pub fn maximized_controls_rect(&self, menubar: Rect) -> Option<Rect> {
        if !self.is_maximized() {
            return None;
        }
        let w = 3 * CAPTION_BTN.w + 2;
        Some(Rect::new(menubar.right() - w - 2, menubar.y + (menubar.h - CAPTION_BTN.h) / 2, w, CAPTION_BTN.h))
    }

    /// Mode agrandi : place de l'icône de la fenêtre active à gauche de la barre de menus (menu de la
    /// fenêtre). L'application décale ses titres de menu d'autant.
    pub fn maximized_icon_rect(&self, menubar: Rect) -> Option<Rect> {
        if !self.is_maximized() {
            return None;
        }
        Some(Rect::new(menubar.x + 2, menubar.y + (menubar.h - 16) / 2, 16, 16))
    }

    fn maximized_buttons(&self, menubar: Rect) -> Vec<(CaptionButton, Rect)> {
        let Some(r) = self.maximized_controls_rect(menubar) else { return Vec::new() };
        let min = Rect::new(r.x, r.y, CAPTION_BTN.w, CAPTION_BTN.h);
        let restore = min.offset(CAPTION_BTN.w, 0);
        let close = restore.offset(CAPTION_BTN.w + 2, 0);
        vec![(CaptionButton::Minimize, min), (CaptionButton::Restore, restore), (CaptionButton::Close, close)]
    }

    /// Dessine l'icône et les boutons de la fenêtre agrandie dans la barre de menus.
    pub fn paint_maximized_controls(&self, canvas: &mut Canvas, res: &Res, menubar: Rect) {
        let Some(id) = self.active.filter(|_| self.is_maximized()) else { return };
        if let (Some(ir), Some(img)) = (self.maximized_icon_rect(menubar), self.child(id).and_then(|c| c.icon).and_then(|i| res.icons.image(i))) {
            canvas.blit(img, ir.x, ir.y);
        }
        for (b, r) in self.maximized_buttons(menubar) {
            let down = self.pressed == Some((id, b)) && self.pressed_over;
            paint_caption_button(canvas, res, r, b, down);
        }
    }

    /// Événements pour l'icône et les boutons de la fenêtre agrandie dans la barre de menus.
    pub fn maximized_controls_event(&mut self, input: &Input, menubar: Rect, platform: &mut dyn Platform) -> Vec<MdiResponse> {
        let mut out = Vec::new();
        let Some(id) = self.active.filter(|_| self.is_maximized()) else { return out };
        let btns = self.maximized_buttons(menubar);
        let at = |p: Point| btns.iter().find(|(_, r)| r.contains(p)).map(|x| x.0);
        match input {
            Input::MouseDown { pos, button: MouseButton::Left, clicks, .. } => {
                if let Some(b) = at(*pos) {
                    self.pressed = Some((id, b));
                    self.pressed_over = true;
                    platform.request_repaint();
                } else if self.maximized_icon_rect(menubar).is_some_and(|r| r.contains(*pos)) {
                    out.push(if *clicks >= 2 { MdiResponse::CloseRequested(id) } else { MdiResponse::SystemMenu(id, *pos) });
                }
            }
            Input::MouseMove { pos } => {
                if let Some((_, b)) = self.pressed {
                    let over = at(*pos) == Some(b);
                    if over != self.pressed_over {
                        self.pressed_over = over;
                        platform.request_repaint();
                    }
                }
            }
            Input::MouseUp { pos, .. } => {
                if let Some((pid, b)) = self.pressed.take() {
                    platform.request_repaint();
                    if pid == id && at(*pos) == Some(b) {
                        out.extend(match b {
                            CaptionButton::Minimize => self.minimize(id),
                            CaptionButton::Restore => self.restore(id),
                            CaptionButton::Close => vec![MdiResponse::CloseRequested(id)],
                            CaptionButton::Maximize => self.maximize(id),
                        });
                    }
                }
            }
            _ => {}
        }
        out
    }
}

/// Dessine un bouton de barre de titre (16x14) avec son pictogramme.
pub fn paint_caption_button(canvas: &mut Canvas, res: &Res, r: Rect, b: CaptionButton, down: bool) {
    let t = &res.theme;
    canvas.fill(r, t.face);
    canvas.bevel(r, if down { Bevel::ButtonPressed } else { Bevel::Button }, t);
    let o = if down { 1 } else { 0 };
    let col = |c: u8| (c == b'k').then_some(t.text);
    match b {
        CaptionButton::Close => {
            const X: [&str; 7] = ["kk....kk", ".kk..kk.", "..kkkk..", "...kk...", "..kkkk..", ".kk..kk.", "kk....kk"];
            draw_pattern(canvas, r.x + 4 + o, r.y + 3 + o, &X, col);
        }
        CaptionButton::Minimize => {
            draw_pattern(canvas, r.x + 4 + o, r.y + 9 + o, &["kkkkkk", "kkkkkk"], col);
        }
        CaptionButton::Maximize => {
            const M: [&str; 9] = ["kkkkkkkkk", "kkkkkkkkk", "k.......k", "k.......k", "k.......k", "k.......k", "k.......k", "k.......k", "kkkkkkkkk"];
            draw_pattern(canvas, r.x + 3 + o, r.y + 2 + o, &M, col);
        }
        CaptionButton::Restore => {
            const R: [&str; 9] = ["..kkkkkk", "..kkkkkk", "..k....k", "kkkkkk.k", "kkkkkk.k", "k....kkk", "k....k..", "k....k..", "kkkkkk.."];
            draw_pattern(canvas, r.x + 4 + o, r.y + 2 + o, &R, col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Mods, TestPlatform};

    fn area() -> (MdiArea, Rect) {
        let mut m = MdiArea::new();
        m.add(MdiChild::new(1, "Status", None, Rect::new(0, 0, 300, 200)));
        m.add(MdiChild::new(2, "#lyon", None, Rect::new(40, 40, 300, 200)));
        m.add(MdiChild::new(3, "Elisa", None, Rect::new(80, 80, 300, 200)));
        (m, Rect::new(0, 50, 800, 500))
    }

    #[test]
    fn move_and_resize() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let (mut m, a) = area();
        assert_eq!(m.active(), Some(3));
        // Cliquer le titre de la fenêtre 2 (visible à gauche de la 3) l'active, puis la déplacer.
        let t2 = m.title_rect(2, a).unwrap();
        let r = m.event(&Input::click(t2.x + 25, t2.y + 5), a, &res, &mut p);
        assert_eq!(r, vec![MdiResponse::Activated(2)]);
        m.event(&Input::moved(t2.x + 45, t2.y + 15), a, &res, &mut p);
        let r = m.event(&Input::release(t2.x + 45, t2.y + 15), a, &res, &mut p);
        assert_eq!(r, vec![MdiResponse::Moved(2)]);
        assert_eq!(m.child(2).unwrap().rect, Rect::new(60, 50, 300, 200));
        // Redimensionner par le coin bas-droit.
        let f = m.frame_rect(2, a).unwrap();
        let corner = Point::new(f.right() - 2, f.bottom() - 2);
        m.event(&Input::moved(corner.x, corner.y), a, &res, &mut p);
        assert_eq!(p.cursor, Cursor::ResizeNWSE);
        m.event(&Input::click(corner.x, corner.y), a, &res, &mut p);
        m.event(&Input::moved(corner.x + 50, corner.y - 1000), a, &res, &mut p);
        m.event(&Input::release(corner.x + 50, corner.y - 1000), a, &res, &mut p);
        let c = m.child(2).unwrap();
        assert_eq!(c.rect.w, 350);
        assert_eq!(c.rect.h, c.min_size.h, "taille minimale");
        // Bord gauche.
        let f = m.frame_rect(2, a).unwrap();
        m.event(&Input::click(f.x + 1, f.y + 30), a, &res, &mut p);
        m.event(&Input::moved(f.x - 19, f.y + 30), a, &res, &mut p);
        m.event(&Input::release(f.x - 19, f.y + 30), a, &res, &mut p);
        assert_eq!(m.child(2).unwrap().rect.x, 40);
        assert_eq!(m.child(2).unwrap().rect.w, 370);
    }

    #[test]
    fn maximize_minimize_and_content_events() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let (mut m, a) = area();
        let t3 = m.title_rect(3, a).unwrap();
        let r = m.event(&Input::MouseDown { pos: Point::new(t3.x + 40, t3.y + 4), button: MouseButton::Left, mods: Mods::NONE, clicks: 2 }, a, &res, &mut p);
        assert_eq!(r, vec![MdiResponse::StateChanged(3, WindowState::Maximized)]);
        assert!(m.is_maximized());
        assert_eq!(m.content_rect(3, a), Some(a));
        // Le contenu reçoit des positions relatives.
        let r = m.event(&Input::click(a.x + 10, a.y + 20), a, &res, &mut p);
        assert_eq!(r, vec![MdiResponse::Content(3, Input::click(10, 20))]);
        m.event(&Input::release(a.x + 10, a.y + 20), a, &res, &mut p);
        // Ctrl+Tab : la suivante, agrandie à son tour.
        let r = m.event(&Input::key(Key::Tab, Mods::CTRL), a, &res, &mut p);
        assert_eq!(r[0], MdiResponse::Activated(2));
        assert_eq!(m.child(2).unwrap().state, WindowState::Maximized);
        assert_eq!(m.child(3).unwrap().state, WindowState::Normal);
        // Boutons dans la barre de menus.
        let bar = Rect::new(0, 0, 800, 20);
        let ctl = m.maximized_controls_rect(bar).unwrap();
        assert!(ctl.right() <= 800);
        let restore = Point::new(ctl.x + CAPTION_BTN.w + 4, ctl.y + 4);
        m.maximized_controls_event(&Input::click(restore.x, restore.y), bar, &mut p);
        let r = m.maximized_controls_event(&Input::release(restore.x, restore.y), bar, &mut p);
        assert!(r.contains(&MdiResponse::StateChanged(2, WindowState::Normal)));
        assert!(!m.is_maximized());
        // Réduire la 2 : la 3 (ou la 1) devient active, la 2 va en bas de la zone.
        let btn = m.caption_buttons(2, a)[0].1;
        m.event(&Input::click(btn.x + 3, btn.y + 3), a, &res, &mut p);
        let r = m.event(&Input::release(btn.x + 3, btn.y + 3), a, &res, &mut p);
        assert_eq!(r[0], MdiResponse::StateChanged(2, WindowState::Minimized));
        let f = m.frame_rect(2, a).unwrap();
        assert_eq!((f.x, f.bottom()), (a.x, a.bottom()));
        assert_ne!(m.active(), Some(2));
        // Double-clic sur la fenêtre réduite : restaurée.
        let r = m.event(&Input::MouseDown { pos: Point::new(f.x + 40, f.y + 8), button: MouseButton::Left, mods: Mods::NONE, clicks: 2 }, a, &res, &mut p);
        assert!(r.contains(&MdiResponse::StateChanged(2, WindowState::Normal)));
        let close = m.caption_buttons(2, a)[2].1;
        m.event(&Input::click(close.x + 3, close.y + 3), a, &res, &mut p);
        assert_eq!(m.event(&Input::release(close.x + 3, close.y + 3), a, &res, &mut p), vec![MdiResponse::CloseRequested(2)]);
    }

    #[test]
    fn tile_and_cascade() {
        let (mut m, a) = area();
        let size = a.size();
        m.tile_vertical(size);
        let rs: Vec<Rect> = m.children.iter().map(|c| c.rect).collect();
        assert!(rs.iter().all(|r| r.h == 500 && r.w >= 266 && r.w <= 267));
        assert_eq!(rs.iter().map(|r| r.w).sum::<i32>(), 800);
        m.tile_horizontal(size);
        assert!(m.children.iter().all(|c| c.rect.w == 800));
        assert_eq!(m.children.iter().map(|c| c.rect.h).sum::<i32>(), 500);
        // L'active est en haut de la mosaïque horizontale.
        assert_eq!(m.child(m.active().unwrap()).unwrap().rect.y, 0);
        m.cascade(size);
        let mut ys: Vec<i32> = m.children.iter().map(|c| c.rect.y).collect();
        ys.sort();
        assert_eq!(ys, vec![0, 22, 44]);
        m.minimize(1);
        m.minimize(2);
        m.tile_horizontal(size);
        assert_eq!(m.children.iter().filter(|c| c.state == WindowState::Normal).map(|c| c.rect.h).sum::<i32>(), 500 - MINIMIZED_H);
        // Restaurer la première réduite : la seconde garde sa place jusqu'à « Réorganiser les icônes ».
        let before = m.frame_rect(2, a).unwrap();
        m.restore(1);
        assert_eq!(m.frame_rect(2, a).unwrap(), before);
        m.arrange_icons(size);
        assert_eq!(m.frame_rect(2, a).unwrap().x, a.x);
    }
}
