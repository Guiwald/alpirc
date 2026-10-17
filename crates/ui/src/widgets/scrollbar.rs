//! Barre de défilement de Windows 98 : flèches, piste tramée, ascenseur proportionnel, répétition quand on
//! maintient le bouton.

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect};
use crate::input::{Input, MouseButton, Platform};
use crate::Res;

use super::{ArrowDir, REPEAT_DELAY_MS, REPEAT_RATE_MS, dither, draw_arrow};

/// Épaisseur d'une barre de défilement (et côté des flèches).
pub const SIZE: i32 = 16;

/// Orientation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    Vertical,
    Horizontal,
}

/// Partie d'une barre de défilement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    ArrowDec,
    ArrowInc,
    PageDec,
    PageInc,
    Thumb,
}

/// Message d'une barre de défilement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollMsg {
    /// Nouvelle position.
    Changed(i32),
}

/// Barre de défilement. La position va de `min` à `max - page` (la page visible).
#[derive(Clone, Debug)]
pub struct ScrollBar {
    pub orientation: Orientation,
    pub min: i32,
    /// Fin du contenu (exclue).
    pub max: i32,
    /// Taille de la partie visible.
    pub page: i32,
    pub pos: i32,
    /// Pas des flèches.
    pub line_step: i32,
    pub enabled: bool,
    pressed: Option<Part>,
    hover_pressed: bool,
    drag_offset: i32,
    next_repeat: i64,
    mouse: Point,
}

impl Default for ScrollBar {
    fn default() -> ScrollBar {
        ScrollBar::new(Orientation::Vertical)
    }
}

impl ScrollBar {
    pub fn new(orientation: Orientation) -> ScrollBar {
        ScrollBar {
            orientation,
            min: 0,
            max: 0,
            page: 1,
            pos: 0,
            line_step: 1,
            enabled: true,
            pressed: None,
            hover_pressed: false,
            drag_offset: 0,
            next_repeat: 0,
            mouse: Point::default(),
        }
    }

    /// Règle l'étendue, la page et la position (bornée).
    pub fn set_range(&mut self, min: i32, max: i32, page: i32) {
        self.min = min;
        self.max = max.max(min);
        self.page = page.max(1);
        self.pos = self.clamp(self.pos);
    }

    /// Position maximale.
    pub fn max_pos(&self) -> i32 {
        (self.max - self.page).max(self.min)
    }

    /// Borne une position.
    pub fn clamp(&self, p: i32) -> i32 {
        p.clamp(self.min, self.max_pos())
    }

    /// Vrai si le contenu dépasse la page (sinon la barre est grisée).
    pub fn is_scrollable(&self) -> bool {
        self.enabled && self.max - self.min > self.page
    }

    /// Partie enfoncée, s'il y en a une.
    pub fn pressed_part(&self) -> Option<Part> {
        self.pressed
    }

    fn len_of(&self, r: Rect) -> i32 {
        match self.orientation {
            Orientation::Vertical => r.h,
            Orientation::Horizontal => r.w,
        }
    }

    fn arrow_len(&self, r: Rect) -> i32 {
        let thick = match self.orientation {
            Orientation::Vertical => r.w,
            Orientation::Horizontal => r.h,
        };
        thick.min(self.len_of(r) / 2)
    }

    /// Début et longueur de l'ascenseur le long de la piste (`None` si pas de défilement possible).
    pub fn thumb(&self, r: Rect) -> Option<(i32, i32)> {
        if !self.is_scrollable() {
            return None;
        }
        let a = self.arrow_len(r);
        let track = self.len_of(r) - 2 * a;
        let range = (self.max - self.min) as i64;
        let len = ((track as i64 * self.page as i64 / range.max(1)) as i32).max(8).min(track);
        if track < 8 {
            return None;
        }
        let span = (self.max_pos() - self.min).max(1) as i64;
        let off = ((track - len) as i64 * (self.pos - self.min) as i64 / span) as i32;
        Some((a + off, len))
    }

    fn part_rect(&self, r: Rect, part: Part) -> Rect {
        let a = self.arrow_len(r);
        let len = self.len_of(r);
        let (ts, tl) = self.thumb(r).unwrap_or((a, 0));
        let seg = |s: i32, l: i32| match self.orientation {
            Orientation::Vertical => Rect::new(r.x, r.y + s, r.w, l),
            Orientation::Horizontal => Rect::new(r.x + s, r.y, l, r.h),
        };
        match part {
            Part::ArrowDec => seg(0, a),
            Part::ArrowInc => seg(len - a, a),
            Part::Thumb => seg(ts, tl),
            Part::PageDec => seg(a, ts - a),
            Part::PageInc => seg(ts + tl, len - a - ts - tl),
        }
    }

    /// Partie sous un point.
    pub fn hit(&self, r: Rect, p: Point) -> Option<Part> {
        if !r.contains(p) {
            return None;
        }
        [Part::ArrowDec, Part::ArrowInc, Part::Thumb, Part::PageDec, Part::PageInc]
            .into_iter()
            .find(|&part| self.part_rect(r, part).contains(p) && (part != Part::Thumb || self.thumb(r).is_some()))
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, r: Rect) {
        let t = &res.theme;
        let scrollable = self.is_scrollable();
        // Piste tramée (blanc et gris).
        dither(canvas, r, t.face, t.highlight);
        if let Some(p @ (Part::PageDec | Part::PageInc)) = self.pressed {
            if self.hover_pressed {
                dither(canvas, self.part_rect(r, p), t.dark, t.shadow);
            }
        }
        let (dec_dir, inc_dir) = match self.orientation {
            Orientation::Vertical => (ArrowDir::Up, ArrowDir::Down),
            Orientation::Horizontal => (ArrowDir::Left, ArrowDir::Right),
        };
        for (part, dir) in [(Part::ArrowDec, dec_dir), (Part::ArrowInc, inc_dir)] {
            let ar = self.part_rect(r, part);
            canvas.fill(ar, t.face);
            let down = self.pressed == Some(part) && self.hover_pressed;
            if down {
                canvas.frame(ar, t.shadow);
            } else {
                canvas.bevel(ar, Bevel::Button, t);
            }
            let o = if down { 1 } else { 0 };
            if scrollable {
                draw_arrow(canvas, ar.offset(o, o), dir, t.text);
            } else {
                draw_arrow(canvas, ar.offset(1, 1), dir, t.highlight);
                draw_arrow(canvas, ar, dir, t.disabled_text);
            }
        }
        if self.thumb(r).is_some() {
            let tr = self.part_rect(r, Part::Thumb);
            canvas.fill(tr, t.face);
            canvas.bevel(tr, Bevel::Button, t);
        }
    }

    fn set_pos(&mut self, p: i32, platform: &mut dyn Platform) -> Option<ScrollMsg> {
        let p = self.clamp(p);
        if p != self.pos {
            self.pos = p;
            platform.request_repaint();
            Some(ScrollMsg::Changed(p))
        } else {
            None
        }
    }

    fn step(&mut self, part: Part, r: Rect, platform: &mut dyn Platform) -> Option<ScrollMsg> {
        match part {
            Part::ArrowDec => self.set_pos(self.pos - self.line_step, platform),
            Part::ArrowInc => self.set_pos(self.pos + self.line_step, platform),
            Part::PageDec => {
                // Arrêter quand l'ascenseur atteint la souris.
                if self.hit(r, self.mouse) != Some(Part::PageDec) {
                    return None;
                }
                self.set_pos(self.pos - self.page.max(1), platform)
            }
            Part::PageInc => {
                if self.hit(r, self.mouse) != Some(Part::PageInc) {
                    return None;
                }
                self.set_pos(self.pos + self.page.max(1), platform)
            }
            Part::Thumb => None,
        }
    }

    pub fn event(&mut self, input: &Input, r: Rect, _res: &Res, platform: &mut dyn Platform) -> Option<ScrollMsg> {
        match input {
            Input::MouseDown { pos, button: MouseButton::Left, .. } if r.contains(*pos) => {
                if !self.is_scrollable() {
                    return None;
                }
                self.mouse = *pos;
                let part = self.hit(r, *pos)?;
                self.pressed = Some(part);
                self.hover_pressed = true;
                platform.request_repaint();
                if part == Part::Thumb {
                    let (ts, _) = self.thumb(r)?;
                    let along = match self.orientation {
                        Orientation::Vertical => pos.y - r.y,
                        Orientation::Horizontal => pos.x - r.x,
                    };
                    self.drag_offset = along - ts;
                    return None;
                }
                self.next_repeat = platform.now_ms() + REPEAT_DELAY_MS;
                platform.schedule_tick(self.next_repeat);
                self.step(part, r, platform)
            }
            Input::MouseMove { pos } if self.pressed.is_some() => {
                self.mouse = *pos;
                let part = self.pressed?;
                if part == Part::Thumb {
                    let (_, tl) = self.thumb(r)?;
                    let a = self.arrow_len(r);
                    let track = self.len_of(r) - 2 * a - tl;
                    let along = match self.orientation {
                        Orientation::Vertical => pos.y - r.y,
                        Orientation::Horizontal => pos.x - r.x,
                    } - self.drag_offset
                        - a;
                    // Lâcher l'ascenseur si la souris s'éloigne trop (comme Windows).
                    let far = match self.orientation {
                        Orientation::Vertical => (pos.x - r.center().x).abs() > 150,
                        Orientation::Horizontal => (pos.y - r.center().y).abs() > 150,
                    };
                    if far {
                        return None;
                    }
                    let span = (self.max_pos() - self.min) as i64;
                    let p = self.min + if track > 0 { (along.clamp(0, track) as i64 * span / track as i64) as i32 } else { 0 };
                    return self.set_pos(p, platform);
                }
                let over = self.hit(r, *pos) == Some(part) || matches!(part, Part::PageDec | Part::PageInc) && r.contains(*pos);
                if over != self.hover_pressed {
                    self.hover_pressed = over;
                    platform.request_repaint();
                }
                None
            }
            Input::MouseUp { button: MouseButton::Left, .. } if self.pressed.is_some() => {
                self.pressed = None;
                platform.request_repaint();
                None
            }
            Input::Tick => {
                let part = self.pressed?;
                if part == Part::Thumb {
                    return None;
                }
                let now = platform.now_ms();
                if now < self.next_repeat {
                    platform.schedule_tick(self.next_repeat);
                    return None;
                }
                self.next_repeat = now + REPEAT_RATE_MS;
                platform.schedule_tick(self.next_repeat);
                if self.hover_pressed { self.step(part, r, platform) } else { None }
            }
            Input::Wheel { pos, dy, dx, .. } if r.contains(*pos) && self.is_scrollable() => {
                let d = if self.orientation == Orientation::Vertical { *dy } else { if *dx != 0.0 { *dx } else { *dy } };
                let lines = (-d * 3.0).round() as i32;
                self.set_pos(self.pos + lines * self.line_step, platform)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::TestPlatform;

    #[test]
    fn thumb_geometry_and_paging() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let r = Rect::new(0, 0, 16, 216);
        let mut sb = ScrollBar::new(Orientation::Vertical);
        sb.set_range(0, 100, 20);
        assert_eq!(sb.max_pos(), 80);
        // Piste de 184 px, ascenseur 184 * 20 / 100 = 36 px.
        assert_eq!(sb.thumb(r), Some((16, 36)));
        // Déjà en haut : la flèche du haut ne change rien.
        assert_eq!(sb.event(&Input::click(8, 8), r, &res, &mut p), None);
        sb.event(&Input::release(8, 8), r, &res, &mut p);
        assert_eq!(sb.event(&Input::click(8, 210), r, &res, &mut p), Some(ScrollMsg::Changed(1)));
        // Maintenir : répétition après le délai.
        p.now = 1000;
        assert_eq!(sb.event(&Input::Tick, r, &res, &mut p), Some(ScrollMsg::Changed(2)));
        sb.event(&Input::release(8, 210), r, &res, &mut p);
        assert_eq!(sb.event(&Input::Tick, r, &res, &mut p), None);
        // Page suivante.
        assert_eq!(sb.event(&Input::click(8, 150), r, &res, &mut p), Some(ScrollMsg::Changed(22)));
        sb.event(&Input::release(8, 150), r, &res, &mut p);
        // Glisser l'ascenseur jusqu'en bas.
        let (ts, _) = sb.thumb(r).unwrap();
        sb.event(&Input::click(8, ts + 2), r, &res, &mut p);
        sb.event(&Input::moved(8, 400), r, &res, &mut p);
        assert_eq!(sb.pos, 80);
        sb.event(&Input::release(8, 400), r, &res, &mut p);
        sb.set_range(0, 10, 20);
        assert!(!sb.is_scrollable());
        assert_eq!(sb.pos, 0);
    }
}
