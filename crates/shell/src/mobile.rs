//! Disposition du téléphone (ADR 0006) : une fenêtre à la fois en plein écran, barre du haut avec le nom de la
//! fenêtre et son sujet, tiroir des fenêtres à gauche, liste des pseudos à droite.
//!
//! Première version : barre du haut et fenêtre active ; les tiroirs et le champ de saisie natif d'Android
//! arrivent avec la version Android.

use alpirc_core::WinId;
use alpirc_ui::{Canvas, Input, Rect, TextStyle};

use crate::layout::paint_window;
use crate::{Shell, ShellPlatform};

/// Hauteur de la barre du haut (pixels logiques).
pub const TOP_BAR_H: i32 = 28;

#[derive(Default)]
pub struct MobileState {
    /// Fenêtre affichée.
    pub active: Option<WinId>,
    /// Tiroir ouvert : `Some(false)` = fenêtres (gauche), `Some(true)` = pseudos (droite).
    #[allow(dead_code)]
    pub drawer: Option<bool>,
    /// Marges imposées par le système (barre d'état, barre de navigation, clavier), en pixels logiques :
    /// haut, bas.
    pub insets: (i32, i32),
}

impl Shell {
    pub(crate) fn mobile_content_rect(&self) -> Rect {
        let (top, bottom) = self.mobile.insets;
        Rect::from_corners(0, top + TOP_BAR_H, self.viewport.w, (self.viewport.h - bottom).max(top + TOP_BAR_H + 1))
    }

    /// Marges imposées par le système (Android : barres et clavier), en pixels logiques.
    pub fn set_insets(&mut self, top: i32, bottom: i32) {
        self.mobile.insets = (top.max(0), bottom.max(0));
    }

    pub(crate) fn mobile_input(&mut self, input: Input, p: &mut dyn ShellPlatform) {
        if !self.modals.is_empty() {
            let bounds = self.dialog_bounds();
            if let Some((key, msg)) = self.modals.event(&input, &self.res, p, bounds) {
                self.dialog_msg(key, msg, p);
            }
            p.request_repaint();
            return;
        }
        if let Some(w) = self.mobile.active.or(self.client.active()) {
            self.content_event(w, &input, p);
        }
    }

    pub(crate) fn mobile_paint(&mut self, canvas: &mut Canvas) {
        let t = &self.res.theme;
        let full = Rect::new(0, 0, self.viewport.w, self.viewport.h);
        canvas.fill(full, t.face);
        let (top, _) = self.mobile.insets;
        let bar = Rect::new(0, top, self.viewport.w, TOP_BAR_H);
        canvas.gradient_h(bar, t.title_active_from, t.title_active_to);
        let active = self.mobile.active.or(self.client.active());
        let title = active.and_then(|a| self.client.window(a)).map(|w| w.title.clone()).unwrap_or_else(|| "AlpIRC".into());
        let title = self.res.fonts.ellipsize(&title, TextStyle::UI_BOLD, bar.w - 16);
        self.res.fonts.draw_text(canvas, bar.x + 8, bar.y + (TOP_BAR_H - 16) / 2, &title, TextStyle::UI_BOLD, t.title_active_text);
        if let Some(a) = active {
            let rect = self.mobile_content_rect();
            if let (Some(v), Some(w)) = (self.views.get(&a), self.client.window(a)) {
                paint_window(canvas, &self.res, &self.client, w, v, rect, false);
            }
            if let Some(v) = self.views.get(&a) {
                v.editbox.paint_popup(canvas, &self.res);
            }
        }
        self.modals.paint(canvas, &self.res);
    }
}
