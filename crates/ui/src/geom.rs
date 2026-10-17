//! Géométrie en pixels logiques : points, tailles, rectangles.
//!
//! Toute la géométrie des widgets s'exprime en pixels **logiques** (`i32`). Le [`Canvas`](crate::Canvas)
//! convertit en pixels physiques selon son échelle.

/// Un point en pixels logiques.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    /// Décale le point.
    pub const fn offset(self, dx: i32, dy: i32) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}

/// Une taille en pixels logiques.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Size {
    pub w: i32,
    pub h: i32,
}

impl Size {
    pub const fn new(w: i32, h: i32) -> Size {
        Size { w, h }
    }
}

/// Un rectangle en pixels logiques : coin haut-gauche `(x, y)`, largeur `w`, hauteur `h`.
///
/// Le bord droit (`x + w`) et le bord bas (`y + h`) sont exclus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    /// Rectangle de deux coins (le second exclu).
    pub fn from_corners(x0: i32, y0: i32, x1: i32, y1: i32) -> Rect {
        Rect { x: x0, y: y0, w: (x1 - x0).max(0), h: (y1 - y0).max(0) }
    }

    /// Rectangle de taille `size` placé en (0, 0).
    pub const fn from_size(size: Size) -> Rect {
        Rect { x: 0, y: 0, w: size.w, h: size.h }
    }

    /// Bord droit (exclu).
    pub const fn right(&self) -> i32 {
        self.x + self.w
    }

    /// Bord bas (exclu).
    pub const fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub const fn size(&self) -> Size {
        Size { w: self.w, h: self.h }
    }

    pub const fn origin(&self) -> Point {
        Point { x: self.x, y: self.y }
    }

    /// Centre (arrondi vers le haut-gauche).
    pub const fn center(&self) -> Point {
        Point { x: self.x + self.w / 2, y: self.y + self.h / 2 }
    }

    /// Vrai si le rectangle n'a pas d'aire.
    pub const fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }

    /// Vrai si le point est dans le rectangle.
    pub const fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.x + self.w && p.y < self.y + self.h
    }

    /// Intersection (rectangle vide si disjoints).
    pub fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        Rect::from_corners(x0, y0, x1, y1)
    }

    /// Vrai si les deux rectangles se recouvrent.
    pub fn intersects(&self, o: &Rect) -> bool {
        !self.intersect(o).is_empty()
    }

    /// Plus petit rectangle contenant les deux.
    pub fn union(&self, o: &Rect) -> Rect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        Rect::from_corners(self.x.min(o.x), self.y.min(o.y), self.right().max(o.right()), self.bottom().max(o.bottom()))
    }

    /// Rétrécit de `d` de chaque côté (agrandit si `d` est négatif).
    pub fn inset(&self, d: i32) -> Rect {
        self.inset_xy(d, d)
    }

    /// Rétrécit de `dx` à gauche et à droite, de `dy` en haut et en bas.
    pub fn inset_xy(&self, dx: i32, dy: i32) -> Rect {
        Rect { x: self.x + dx, y: self.y + dy, w: (self.w - 2 * dx).max(0), h: (self.h - 2 * dy).max(0) }
    }

    /// Rétrécit côté par côté.
    pub fn inset_sides(&self, left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect { x: self.x + left, y: self.y + top, w: (self.w - left - right).max(0), h: (self.h - top - bottom).max(0) }
    }

    /// Décale le rectangle.
    pub const fn offset(&self, dx: i32, dy: i32) -> Rect {
        Rect { x: self.x + dx, y: self.y + dy, w: self.w, h: self.h }
    }

    /// Le même rectangle, déplacé en `(x, y)`.
    pub const fn at(&self, x: i32, y: i32) -> Rect {
        Rect { x, y, w: self.w, h: self.h }
    }

    /// Bande de `w` pixels à gauche, et le reste.
    pub fn split_left(&self, w: i32) -> (Rect, Rect) {
        let w = w.clamp(0, self.w);
        (Rect::new(self.x, self.y, w, self.h), Rect::new(self.x + w, self.y, self.w - w, self.h))
    }

    /// Bande de `w` pixels à droite, et le reste (à gauche).
    pub fn split_right(&self, w: i32) -> (Rect, Rect) {
        let w = w.clamp(0, self.w);
        (Rect::new(self.right() - w, self.y, w, self.h), Rect::new(self.x, self.y, self.w - w, self.h))
    }

    /// Bande de `h` pixels en haut, et le reste.
    pub fn split_top(&self, h: i32) -> (Rect, Rect) {
        let h = h.clamp(0, self.h);
        (Rect::new(self.x, self.y, self.w, h), Rect::new(self.x, self.y + h, self.w, self.h - h))
    }

    /// Bande de `h` pixels en bas, et le reste (en haut).
    pub fn split_bottom(&self, h: i32) -> (Rect, Rect) {
        let h = h.clamp(0, self.h);
        (Rect::new(self.x, self.bottom() - h, self.w, h), Rect::new(self.x, self.y, self.w, self.h - h))
    }

    /// Un rectangle de taille `size` centré dans celui-ci.
    pub fn center_size(&self, size: Size) -> Rect {
        Rect::new(self.x + (self.w - size.w) / 2, self.y + (self.h - size.h) / 2, size.w, size.h)
    }

    /// Déplace le rectangle pour qu'il tienne dans `bounds` (si possible), sans changer sa taille.
    pub fn clamp_inside(&self, bounds: &Rect) -> Rect {
        let mut r = *self;
        if r.right() > bounds.right() {
            r.x = bounds.right() - r.w;
        }
        if r.bottom() > bounds.bottom() {
            r.y = bounds.bottom() - r.h;
        }
        r.x = r.x.max(bounds.x);
        r.y = r.y.max(bounds.y);
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_math() {
        let r = Rect::new(10, 10, 100, 50);
        assert!(r.contains(Point::new(10, 10)));
        assert!(!r.contains(Point::new(110, 10)));
        assert_eq!(r.intersect(&Rect::new(50, 0, 100, 20)), Rect::new(50, 10, 60, 10));
        assert!(r.intersect(&Rect::new(200, 0, 10, 10)).is_empty());
        assert_eq!(r.inset(2), Rect::new(12, 12, 96, 46));
        let (a, b) = r.split_left(30);
        assert_eq!((a.w, b.x, b.w), (30, 40, 70));
        let (t, rest) = r.split_bottom(20);
        assert_eq!((t.y, rest.h), (40, 30));
        assert_eq!(Rect::new(90, 90, 20, 20).clamp_inside(&Rect::new(0, 0, 100, 100)), Rect::new(80, 80, 20, 20));
        assert_eq!(r.union(&Rect::new(0, 0, 5, 5)), Rect::new(0, 0, 110, 60));
    }
}
