//! Barre de progression de Windows 98 (blocs).

use crate::canvas::{Bevel, Canvas};
use crate::geom::Rect;
use crate::Res;

/// Barre de progression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressBar {
    pub value: i32,
    pub max: i32,
    /// Remplissage continu au lieu des blocs.
    pub smooth: bool,
}

impl Default for ProgressBar {
    fn default() -> ProgressBar {
        ProgressBar { value: 0, max: 100, smooth: false }
    }
}

impl ProgressBar {
    pub fn new(value: i32, max: i32) -> ProgressBar {
        ProgressBar { value, max, smooth: false }
    }

    /// Largeur remplie pour une largeur intérieure donnée.
    pub fn filled(&self, inner_w: i32) -> i32 {
        if self.max <= 0 {
            return 0;
        }
        ((inner_w as i64 * self.value.clamp(0, self.max) as i64) / self.max as i64) as i32
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect) {
        let t = &res.theme;
        canvas.fill(rect, t.face);
        canvas.bevel(rect, Bevel::Sunken, t);
        let inner = rect.inset(2);
        let fill = self.filled(inner.w);
        if self.smooth {
            canvas.fill(Rect::new(inner.x, inner.y, fill, inner.h), t.progress);
            return;
        }
        let bw = ((inner.h * 2) / 3).max(4);
        let mut x = inner.x;
        while x < inner.x + fill {
            let w = bw.min(inner.right() - x);
            canvas.fill(Rect::new(x, inner.y, w, inner.h), t.progress);
            x += bw + 2;
        }
    }
}
