//! Info-bulle (jaune pâle, cadre noir) et gestion du délai d'apparition au survol.

use crate::canvas::Canvas;
use crate::geom::{Point, Rect, Size};
use crate::input::Platform;
use crate::text::TextStyle;
use crate::Res;

/// Délai avant l'apparition d'une info-bulle.
pub const TOOLTIP_DELAY_MS: i64 = 500;

/// Une info-bulle à dessiner.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tooltip {
    pub text: String,
}

impl Tooltip {
    pub fn new(text: &str) -> Tooltip {
        Tooltip { text: text.to_string() }
    }

    pub fn size(&self, res: &Res) -> Size {
        Size::new(res.fonts.measure(&self.text, TextStyle::UI) + 8, res.ui_line() + 4)
    }

    /// Rectangle sous le pointeur (décalé comme Windows), gardé dans `bounds`.
    pub fn rect_at(&self, res: &Res, pointer: Point, bounds: Rect) -> Rect {
        let s = self.size(res);
        let mut r = Rect::new(pointer.x, pointer.y + 20, s.w, s.h);
        if r.bottom() > bounds.bottom() {
            r.y = pointer.y - s.h - 4;
        }
        r.clamp_inside(&bounds)
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        canvas.fill(rect, t.tooltip);
        canvas.frame(rect, t.dark);
        res.fonts.draw_text(canvas, rect.x + 4, rect.y + 2, &self.text, TextStyle::UI, t.tooltip_text);
    }
}

/// Suit le survol d'une cible et décide quand montrer l'info-bulle.
#[derive(Clone, Debug, Default)]
pub struct TooltipState {
    key: Option<u64>,
    since: i64,
    /// L'info-bulle est visible.
    pub visible: bool,
    pub text: String,
    /// Position du pointeur au moment du survol.
    pub pointer: Point,
}

impl TooltipState {
    /// Le pointeur survole la cible `key` (`None` : rien). Programme l'apparition après le délai.
    pub fn hover(&mut self, key: Option<u64>, text: &str, pointer: Point, platform: &mut dyn Platform) {
        if key != self.key {
            let was = self.visible;
            self.key = key;
            self.visible = false;
            self.text = text.to_string();
            self.pointer = pointer;
            self.since = platform.now_ms();
            if key.is_some() {
                platform.schedule_tick(self.since + TOOLTIP_DELAY_MS);
            }
            if was {
                platform.request_repaint();
            }
        }
    }

    /// Cache l'info-bulle (clic, sortie).
    pub fn hide(&mut self, platform: &mut dyn Platform) {
        if self.visible {
            platform.request_repaint();
        }
        self.visible = false;
        self.key = None;
    }

    /// À appeler sur [`crate::Input::Tick`].
    pub fn tick(&mut self, platform: &mut dyn Platform) {
        if self.key.is_some() && !self.visible && !self.text.is_empty() {
            if platform.now_ms() - self.since >= TOOLTIP_DELAY_MS {
                self.visible = true;
                platform.request_repaint();
            } else {
                platform.schedule_tick(self.since + TOOLTIP_DELAY_MS);
            }
        }
    }

    /// Dessine l'info-bulle si elle est visible.
    pub fn paint(&self, canvas: &mut Canvas, res: &Res, bounds: Rect) {
        if self.visible && !self.text.is_empty() {
            let tip = Tooltip::new(&self.text);
            let r = tip.rect_at(res, self.pointer, bounds);
            tip.paint(canvas, res, r);
        }
    }
}
