//! Barre d'outils plate de Windows 98 : icônes 16x16 dans des boutons de 24 pixels qui se soulèvent au
//! survol, séparateurs gravés, boutons à bascule, info-bulles.

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect};
use crate::icons::IconId;
use crate::input::{Input, MouseButton, Platform};
use crate::widgets::{TooltipState, dither};
use crate::Res;

/// Largeur d'un bouton.
pub const BUTTON_W: i32 = 24;
/// Hauteur d'un bouton.
pub const BUTTON_H: i32 = 22;
/// Largeur d'un séparateur.
pub const SEPARATOR_W: i32 = 8;

/// Un élément de la barre d'outils.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolItem {
    Button {
        id: u32,
        icon: Option<IconId>,
        tooltip: String,
        /// Bouton à bascule.
        toggle: bool,
        /// État d'un bouton à bascule.
        checked: bool,
        enabled: bool,
    },
    Separator,
}

impl ToolItem {
    pub fn button(id: u32, icon: Option<IconId>, tooltip: &str) -> ToolItem {
        ToolItem::Button { id, icon, tooltip: tooltip.to_string(), toggle: false, checked: false, enabled: true }
    }

    pub fn toggle(id: u32, icon: Option<IconId>, tooltip: &str, checked: bool) -> ToolItem {
        ToolItem::Button { id, icon, tooltip: tooltip.to_string(), toggle: true, checked, enabled: true }
    }

    pub fn separator() -> ToolItem {
        ToolItem::Separator
    }
}

/// Message de la barre d'outils.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolMsg {
    Clicked(u32),
    /// Bouton à bascule : nouvel état.
    Toggled(u32, bool),
}

/// Barre d'outils.
#[derive(Clone, Debug, Default)]
pub struct Toolbar {
    pub items: Vec<ToolItem>,
    hover: Option<usize>,
    pressed: Option<usize>,
    tip: TooltipState,
}

impl Toolbar {
    pub fn new(items: Vec<ToolItem>) -> Toolbar {
        Toolbar { items, ..Default::default() }
    }

    /// Hauteur de la barre (trait gravé du haut compris).
    pub fn height() -> i32 {
        BUTTON_H + 6
    }

    /// Change l'état d'un bouton à bascule.
    pub fn set_checked(&mut self, id: u32, on: bool) {
        for it in &mut self.items {
            if let ToolItem::Button { id: i, checked, .. } = it {
                if *i == id {
                    *checked = on;
                }
            }
        }
    }

    /// Active ou grise un bouton.
    pub fn set_enabled(&mut self, id: u32, on: bool) {
        for it in &mut self.items {
            if let ToolItem::Button { id: i, enabled, .. } = it {
                if *i == id {
                    *enabled = on;
                }
            }
        }
    }

    /// Rectangles des éléments.
    pub fn item_rects(&self, rect: Rect) -> Vec<Rect> {
        let mut x = rect.x + 4;
        let y = rect.y + 3 + (rect.h - 3 - BUTTON_H) / 2;
        self.items
            .iter()
            .map(|it| {
                let w = if matches!(it, ToolItem::Separator) { SEPARATOR_W } else { BUTTON_W };
                let r = Rect::new(x, y, w, BUTTON_H);
                x += w;
                r
            })
            .collect()
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        canvas.fill(rect, t.face);
        // Trait gravé sous la barre de menus.
        canvas.etched_hline(rect.x, rect.y, rect.w, t);
        for (i, (it, r)) in self.items.iter().zip(self.item_rects(rect)).enumerate() {
            match it {
                ToolItem::Separator => canvas.etched_vline(r.x + SEPARATOR_W / 2 - 1, r.y + 1, r.h - 2, t),
                ToolItem::Button { icon, checked, enabled, .. } => {
                    let down = self.pressed == Some(i) && self.hover == Some(i);
                    if *checked || down {
                        canvas.bevel(r, Bevel::Sunken, t);
                        if *checked && !down {
                            dither(canvas, r.inset(1), t.face, t.highlight);
                        }
                    } else if self.hover == Some(i) && *enabled {
                        canvas.bevel(r, Bevel::RaisedThin, t);
                    }
                    let o = if *checked || down { 1 } else { 0 };
                    if let Some(img) = icon.and_then(|id| res.icons.image(id)) {
                        let ix = r.x + (r.w - img.w as i32) / 2 + o;
                        let iy = r.y + (r.h - img.h as i32) / 2 + o;
                        if *enabled {
                            canvas.blit(img, ix, iy);
                        } else {
                            canvas.blit_disabled(img, ix, iy, t);
                        }
                    }
                }
            }
        }
    }

    /// Dessine l'info-bulle (à appeler après tout le reste).
    pub fn paint_tooltip(&self, canvas: &mut Canvas, res: &Res, bounds: Rect) {
        self.tip.paint(canvas, res, bounds);
    }

    fn index_at(&self, rect: Rect, p: Point) -> Option<usize> {
        self.item_rects(rect)
            .iter()
            .position(|r| r.contains(p))
            .filter(|&i| matches!(self.items[i], ToolItem::Button { enabled: true, .. }))
    }

    pub fn event(&mut self, input: &Input, rect: Rect, _res: &Res, platform: &mut dyn Platform) -> Option<ToolMsg> {
        match input {
            Input::MouseMove { pos } => {
                let h = if rect.contains(*pos) { self.index_at(rect, *pos) } else { None };
                if h != self.hover {
                    self.hover = h;
                    platform.request_repaint();
                }
                let tip = h.and_then(|i| match &self.items[i] {
                    ToolItem::Button { tooltip, .. } if !tooltip.is_empty() => Some((i as u64, tooltip.clone())),
                    _ => None,
                });
                match tip {
                    Some((k, s)) => self.tip.hover(Some(k), &s, *pos, platform),
                    None => self.tip.hover(None, "", *pos, platform),
                }
            }
            Input::MouseLeave => {
                self.hover = None;
                self.tip.hide(platform);
                platform.request_repaint();
            }
            Input::MouseDown { pos, button: MouseButton::Left, .. } if rect.contains(*pos) => {
                self.tip.hide(platform);
                self.pressed = self.index_at(rect, *pos);
                self.hover = self.pressed;
                platform.request_repaint();
            }
            Input::MouseUp { pos, button: MouseButton::Left, .. } => {
                let p = self.pressed.take()?;
                platform.request_repaint();
                if self.index_at(rect, *pos) != Some(p) {
                    return None;
                }
                if let ToolItem::Button { id, toggle, checked, .. } = &mut self.items[p] {
                    if *toggle {
                        *checked = !*checked;
                        return Some(ToolMsg::Toggled(*id, *checked));
                    }
                    return Some(ToolMsg::Clicked(*id));
                }
            }
            Input::Tick => self.tip.tick(platform),
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
    fn click_toggle_and_tooltip() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let rect = Rect::new(0, 0, 400, Toolbar::height());
        let mut tb = Toolbar::new(vec![
            ToolItem::button(1, res.icons.get("connect"), "Connecter"),
            ToolItem::separator(),
            ToolItem::toggle(2, res.icons.get("options"), "Options", false),
        ]);
        let r = tb.item_rects(rect);
        assert_eq!(r[2].x, r[0].x + BUTTON_W + SEPARATOR_W);
        let c = r[0].center();
        tb.event(&Input::moved(c.x, c.y), rect, &res, &mut p);
        p.now = 600;
        tb.event(&Input::Tick, rect, &res, &mut p);
        assert!(tb.tip.visible);
        tb.event(&Input::click(c.x, c.y), rect, &res, &mut p);
        assert_eq!(tb.event(&Input::release(c.x, c.y), rect, &res, &mut p), Some(ToolMsg::Clicked(1)));
        let c2 = r[2].center();
        tb.event(&Input::click(c2.x, c2.y), rect, &res, &mut p);
        assert_eq!(tb.event(&Input::release(c2.x, c2.y), rect, &res, &mut p), Some(ToolMsg::Toggled(2, true)));
        tb.set_enabled(1, false);
        tb.event(&Input::click(c.x, c.y), rect, &res, &mut p);
        assert_eq!(tb.event(&Input::release(c.x, c.y), rect, &res, &mut p), None);
    }
}
