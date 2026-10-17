//! Le canevas : un tampon de pixels physiques `0x00RRGGBB` (format de softbuffer) et une échelle.
//!
//! Les fonctions de dessin prennent des coordonnées **logiques** ; le canevas les convertit en pixels physiques
//! en arrondissant. Un trait de 1 pixel logique fait `round(échelle)` pixels physiques, si bien que les reliefs
//! restent nets même à 1,5x.

use crate::geom::{Point, Rect, Size};
use crate::theme::Theme;

/// Rectangle en pixels physiques, bords exclus à droite et en bas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl PRect {
    pub fn intersect(&self, o: &PRect) -> PRect {
        PRect { x0: self.x0.max(o.x0), y0: self.y0.max(o.y0), x1: self.x1.min(o.x1), y1: self.y1.min(o.y1) }
    }
    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }
}

/// Les sortes de relief façon Windows 98.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bevel {
    /// Relief saillant de 2 pixels (bouton, ascenseur, flèches) : blanc / clair en haut à gauche,
    /// noir / ombre en bas à droite.
    Raised,
    /// Bouton poussoir (identique à `Raised`).
    Button,
    /// Bouton enfoncé : l'inverse de `Button`.
    ButtonPressed,
    /// Champ de saisie, liste : creux de 2 pixels (ombre / noir en haut à gauche, blanc / clair en bas à droite).
    Field,
    /// Creux fin de 1 pixel (cases de barre d'état, bouton de barre d'outils enfoncé).
    Sunken,
    /// Saillie fine de 1 pixel (bouton de barre d'outils survolé, menus).
    RaisedThin,
    /// Trait gravé (cadre de groupe, séparateurs) : ombre puis blanc décalé d'un pixel.
    Etched,
    /// Cadre de fenêtre ou de boîte de dialogue : clair / noir à l'extérieur, blanc / ombre à l'intérieur.
    Window,
}

/// Une image RGBA (`0xAARRGGBB`, alpha non prémultiplié), pour les icônes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub pixels: Vec<u32>,
}

impl Image {
    /// Image transparente.
    pub fn new(w: u32, h: u32) -> Image {
        Image { w, h, pixels: vec![0; (w * h) as usize] }
    }

    /// Décode un PNG (toutes profondeurs et palettes, ramenées en RGBA 8 bits).
    pub fn from_png(bytes: &[u8]) -> Option<Image> {
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = dec.read_info().ok()?;
        let mut buf = vec![0u8; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        let (w, h) = (info.width, info.height);
        let mut img = Image::new(w, h);
        let ch = match info.color_type {
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            png::ColorType::Indexed => return None,
        };
        for y in 0..h as usize {
            let row = &buf[y * info.line_size..];
            for x in 0..w as usize {
                let p = &row[x * ch..x * ch + ch];
                let (r, g, b, a) = match ch {
                    1 => (p[0], p[0], p[0], 255),
                    2 => (p[0], p[0], p[0], p[1]),
                    3 => (p[0], p[1], p[2], 255),
                    _ => (p[0], p[1], p[2], p[3]),
                };
                img.pixels[y * w as usize + x] = (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32;
            }
        }
        Some(img)
    }

    pub fn get(&self, x: u32, y: u32) -> u32 {
        self.pixels[(y * self.w + x) as usize]
    }

    /// Écrit un pixel opaque (`0xRRGGBB`).
    pub fn set_rgb(&mut self, x: u32, y: u32, rgb: u32) {
        if x < self.w && y < self.h {
            self.pixels[(y * self.w + x) as usize] = 0xFF00_0000 | (rgb & 0xFF_FFFF);
        }
    }

    /// Remplit un rectangle d'une couleur opaque.
    pub fn fill_rgb(&mut self, x: u32, y: u32, w: u32, h: u32, rgb: u32) {
        for yy in y..(y + h).min(self.h) {
            for xx in x..(x + w).min(self.w) {
                self.set_rgb(xx, yy, rgb);
            }
        }
    }

    pub fn size(&self) -> Size {
        Size::new(self.w as i32, self.h as i32)
    }
}

/// Tampon de pixels physiques et échelle.
pub struct Canvas {
    width: u32,
    height: u32,
    scale: f32,
    /// Pixels `0x00RRGGBB`, ligne par ligne.
    pub pixels: Vec<u32>,
    clip: Vec<PRect>,
}

impl Canvas {
    /// Canevas de `width` x `height` pixels physiques, à l'échelle `scale` (1.0, 1.5, 2.0…).
    pub fn new(width: u32, height: u32, scale: f32) -> Canvas {
        Canvas { width, height, scale: scale.max(0.5), pixels: vec![0; (width * height) as usize], clip: Vec::new() }
    }

    /// Change la taille (le contenu est perdu).
    pub fn resize(&mut self, width: u32, height: u32, scale: f32) {
        self.width = width;
        self.height = height;
        self.scale = scale.max(0.5);
        self.pixels.clear();
        self.pixels.resize((width * height) as usize, 0);
        self.clip.clear();
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Change l'échelle sans toucher aux pixels, pour dessiner une couche à une autre échelle (boîtes de
    /// dialogue réduites à la largeur d'un téléphone) ; les découpes en cours, physiques, restent valables.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.max(0.25);
    }

    /// Taille logique (pixels physiques divisés par l'échelle).
    pub fn logical_size(&self) -> Size {
        Size::new((self.width as f32 / self.scale).floor() as i32, (self.height as f32 / self.scale).floor() as i32)
    }

    /// Rectangle logique de tout le canevas.
    pub fn bounds(&self) -> Rect {
        Rect::from_size(self.logical_size())
    }

    /// Épaisseur physique d'un trait de 1 pixel logique.
    pub fn line_px(&self) -> i32 {
        (self.scale.round() as i32).max(1)
    }

    /// Abscisse ou ordonnée logique → physique.
    pub fn to_phys(&self, v: i32) -> i32 {
        (v as f32 * self.scale).round() as i32
    }

    /// Valeur logique fractionnaire → physique.
    pub fn to_phys_f(&self, v: f32) -> i32 {
        (v * self.scale).round() as i32
    }

    /// Point physique → logique.
    pub fn to_logical(&self, x: i32, y: i32) -> Point {
        Point::new((x as f32 / self.scale).floor() as i32, (y as f32 / self.scale).floor() as i32)
    }

    /// Rectangle logique → physique.
    pub fn phys(&self, r: Rect) -> PRect {
        PRect { x0: self.to_phys(r.x), y0: self.to_phys(r.y), x1: self.to_phys(r.right()), y1: self.to_phys(r.bottom()) }
    }

    /// Zone de découpe courante (physique).
    pub fn clip_phys(&self) -> PRect {
        let full = PRect { x0: 0, y0: 0, x1: self.width as i32, y1: self.height as i32 };
        self.clip.last().map_or(full, |c| c.intersect(&full))
    }

    /// Restreint le dessin à `r` (intersecté avec la découpe courante). À équilibrer par [`Canvas::pop_clip`].
    pub fn push_clip(&mut self, r: Rect) {
        let p = self.phys(r).intersect(&self.clip_phys());
        self.clip.push(p);
    }

    /// Retire la dernière découpe.
    pub fn pop_clip(&mut self) {
        self.clip.pop();
    }

    /// Dessine avec une découpe temporaire.
    pub fn with_clip<R>(&mut self, r: Rect, f: impl FnOnce(&mut Canvas) -> R) -> R {
        self.push_clip(r);
        let out = f(self);
        self.pop_clip();
        out
    }

    /// Rectangle logique visible (découpe courante), arrondi vers l'extérieur.
    pub fn clip_rect(&self) -> Rect {
        let c = self.clip_phys();
        let s = self.scale;
        Rect::from_corners(
            (c.x0 as f32 / s).floor() as i32,
            (c.y0 as f32 / s).floor() as i32,
            (c.x1 as f32 / s).ceil() as i32,
            (c.y1 as f32 / s).ceil() as i32,
        )
    }

    /// Remplit tout le canevas (sans tenir compte de la découpe).
    pub fn clear(&mut self, color: u32) {
        self.pixels.fill(color & 0xFF_FFFF);
    }

    /// Remplit un rectangle physique (découpé).
    pub fn fill_phys(&mut self, r: PRect, color: u32) {
        let r = r.intersect(&self.clip_phys());
        if r.is_empty() {
            return;
        }
        let w = self.width as usize;
        let c = color & 0xFF_FFFF;
        for y in r.y0..r.y1 {
            let row = y as usize * w;
            self.pixels[row + r.x0 as usize..row + r.x1 as usize].fill(c);
        }
    }

    /// Mélange un rectangle physique avec une couleur (`alpha` de 0 à 255).
    pub fn blend_phys(&mut self, r: PRect, color: u32, alpha: u32) {
        let r = r.intersect(&self.clip_phys());
        if r.is_empty() || alpha == 0 {
            return;
        }
        let w = self.width as usize;
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                let i = y as usize * w + x as usize;
                self.pixels[i] = blend(self.pixels[i], color, alpha);
            }
        }
    }

    /// Écrit un pixel physique (découpé).
    #[inline]
    pub fn put_phys(&mut self, x: i32, y: i32, color: u32) {
        let c = self.clip_phys();
        if x >= c.x0 && y >= c.y0 && x < c.x1 && y < c.y1 {
            self.pixels[y as usize * self.width as usize + x as usize] = color & 0xFF_FFFF;
        }
    }

    /// Lit un pixel physique (0 hors du canevas).
    pub fn get_phys(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return 0;
        }
        self.pixels[y as usize * self.width as usize + x as usize]
    }

    /// Lit la couleur au centre d'un pixel logique (pour les tests).
    pub fn get(&self, x: i32, y: i32) -> u32 {
        let px = ((x as f32 + 0.5) * self.scale) as i32;
        let py = ((y as f32 + 0.5) * self.scale) as i32;
        self.get_phys(px, py)
    }

    /// Remplit un rectangle logique.
    pub fn fill(&mut self, r: Rect, color: u32) {
        let p = self.phys(r);
        self.fill_phys(p, color);
    }

    /// Mélange un rectangle logique avec une couleur.
    pub fn blend_fill(&mut self, r: Rect, color: u32, alpha: u32) {
        let p = self.phys(r);
        self.blend_phys(p, color, alpha);
    }

    /// Trait horizontal de 1 pixel logique d'épaisseur, de `x` à `x + w`.
    pub fn hline(&mut self, x: i32, y: i32, w: i32, color: u32) {
        let t = self.line_px();
        let y0 = self.to_phys(y);
        self.fill_phys(PRect { x0: self.to_phys(x), y0, x1: self.to_phys(x + w), y1: y0 + t }, color);
    }

    /// Trait vertical de 1 pixel logique d'épaisseur, de `y` à `y + h`.
    pub fn vline(&mut self, x: i32, y: i32, h: i32, color: u32) {
        let t = self.line_px();
        let x0 = self.to_phys(x);
        self.fill_phys(PRect { x0, y0: self.to_phys(y), x1: x0 + t, y1: self.to_phys(y + h) }, color);
    }

    /// Un pixel logique.
    pub fn pixel(&mut self, x: i32, y: i32, color: u32) {
        let t = self.line_px();
        let (px, py) = (self.to_phys(x), self.to_phys(y));
        self.fill_phys(PRect { x0: px, y0: py, x1: px + t, y1: py + t }, color);
    }

    /// Contour de 1 pixel logique.
    pub fn frame(&mut self, r: Rect, color: u32) {
        if r.is_empty() {
            return;
        }
        let p = self.phys(r);
        let t = self.line_px();
        self.frame_phys(p, t, color, color);
    }

    /// Contour physique : `tl` en haut et à gauche, `br` en bas et à droite (qui recouvre les coins
    /// haut-droit et bas-gauche, comme `DrawEdge`).
    fn frame_phys(&mut self, p: PRect, t: i32, tl: u32, br: u32) {
        self.fill_phys(PRect { x0: p.x0, y0: p.y0, x1: p.x1 - t, y1: p.y0 + t }, tl);
        self.fill_phys(PRect { x0: p.x0, y0: p.y0, x1: p.x0 + t, y1: p.y1 - t }, tl);
        self.fill_phys(PRect { x0: p.x0, y0: p.y1 - t, x1: p.x1, y1: p.y1 }, br);
        self.fill_phys(PRect { x0: p.x1 - t, y0: p.y0, x1: p.x1, y1: p.y1 }, br);
    }

    /// Dessine un relief sur le pourtour de `r` (sans remplir l'intérieur).
    pub fn bevel(&mut self, r: Rect, kind: Bevel, theme: &Theme) {
        if r.is_empty() {
            return;
        }
        let p = self.phys(r);
        let t = self.line_px();
        let inner = PRect { x0: p.x0 + t, y0: p.y0 + t, x1: p.x1 - t, y1: p.y1 - t };
        match kind {
            Bevel::Raised | Bevel::Button => {
                self.frame_phys(p, t, theme.highlight, theme.dark);
                self.frame_phys(inner, t, theme.light, theme.shadow);
            }
            Bevel::ButtonPressed => {
                self.frame_phys(p, t, theme.dark, theme.highlight);
                self.frame_phys(inner, t, theme.shadow, theme.light);
            }
            Bevel::Field => {
                self.frame_phys(p, t, theme.shadow, theme.highlight);
                self.frame_phys(inner, t, theme.dark, theme.light);
            }
            Bevel::Window => {
                self.frame_phys(p, t, theme.light, theme.dark);
                self.frame_phys(inner, t, theme.highlight, theme.shadow);
            }
            Bevel::Sunken => self.frame_phys(p, t, theme.shadow, theme.highlight),
            Bevel::RaisedThin => self.frame_phys(p, t, theme.highlight, theme.shadow),
            Bevel::Etched => {
                let a = PRect { x0: p.x0, y0: p.y0, x1: p.x1 - t, y1: p.y1 - t };
                let b = PRect { x0: p.x0 + t, y0: p.y0 + t, x1: p.x1, y1: p.y1 };
                // Blanc décalé d'un pixel, puis l'ombre par-dessus : partout « gris puis blanc », comme
                // EDGE_ETCHED.
                self.frame_phys(b, t, theme.highlight, theme.highlight);
                self.frame_phys(a, t, theme.shadow, theme.shadow);
            }
        }
    }

    /// Ligne gravée horizontale (séparateur de menu ou de dialogue) : ombre puis blanc.
    pub fn etched_hline(&mut self, x: i32, y: i32, w: i32, theme: &Theme) {
        self.hline(x, y, w, theme.shadow);
        self.hline(x, y + 1, w, theme.highlight);
    }

    /// Ligne gravée verticale.
    pub fn etched_vline(&mut self, x: i32, y: i32, h: i32, theme: &Theme) {
        self.vline(x, y, h, theme.shadow);
        self.vline(x + 1, y, h, theme.highlight);
    }

    /// Rectangle de focus en pointillés (un pixel logique sur deux).
    pub fn focus_rect(&mut self, r: Rect, color: u32) {
        if r.is_empty() {
            return;
        }
        for x in r.x..r.right() {
            if (x - r.x) % 2 == 0 {
                self.pixel(x, r.y, color);
            }
            if (x - r.x + r.h - 1) % 2 == 0 {
                self.pixel(x, r.bottom() - 1, color);
            }
        }
        for y in r.y..r.bottom() {
            if (y - r.y) % 2 == 0 {
                self.pixel(r.x, y, color);
            }
            if (y - r.y + r.w - 1) % 2 == 0 {
                self.pixel(r.right() - 1, y, color);
            }
        }
    }

    /// Dégradé horizontal de `from` (à gauche) à `to` (à droite).
    pub fn gradient_h(&mut self, r: Rect, from: u32, to: u32) {
        let p = self.phys(r).intersect(&self.clip_phys());
        if p.is_empty() {
            return;
        }
        let full = self.phys(r);
        let span = (full.x1 - full.x0 - 1).max(1) as u32;
        let w = self.width as usize;
        for x in p.x0..p.x1 {
            let t = ((x - full.x0) as u32 * 255) / span;
            let c = crate::theme::mix(from, to, t);
            for y in p.y0..p.y1 {
                self.pixels[y as usize * w + x as usize] = c;
            }
        }
    }

    /// Facteur entier d'agrandissement des icônes (pixel art net) : `floor(échelle)`, au moins 1.
    pub fn icon_factor(&self) -> i32 {
        ((self.scale + 0.01).floor() as i32).max(1)
    }

    /// Copie une image avec son alpha, agrandie d'un facteur **entier** (plus proche voisin), centrée dans la
    /// boîte logique `(x, y, img.w, img.h)` : le pixel art reste net à toutes les échelles.
    pub fn blit(&mut self, img: &Image, x: i32, y: i32) {
        let f = self.icon_factor();
        let bx = self.to_phys(x);
        let by = self.to_phys(y);
        let bw = self.to_phys(x + img.w as i32) - bx;
        let bh = self.to_phys(y + img.h as i32) - by;
        let ox = bx + (bw - img.w as i32 * f) / 2;
        let oy = by + (bh - img.h as i32 * f) / 2;
        self.blit_phys(img, ox, oy, f, None);
    }

    /// Comme [`Canvas::blit`], chaque pixel opaque remplacé par `color` (silhouettes, icônes grisées).
    pub fn blit_tinted(&mut self, img: &Image, x: i32, y: i32, color: u32) {
        let f = self.icon_factor();
        let bx = self.to_phys(x);
        let by = self.to_phys(y);
        let bw = self.to_phys(x + img.w as i32) - bx;
        let bh = self.to_phys(y + img.h as i32) - by;
        let ox = bx + (bw - img.w as i32 * f) / 2;
        let oy = by + (bh - img.h as i32 * f) / 2;
        self.blit_phys(img, ox, oy, f, Some(color));
    }

    /// Icône grisée façon Windows 98 : silhouette blanche décalée d'un pixel, puis silhouette d'ombre.
    pub fn blit_disabled(&mut self, img: &Image, x: i32, y: i32, theme: &Theme) {
        self.blit_tinted(img, x + 1, y + 1, theme.highlight);
        self.blit_tinted(img, x, y, theme.shadow);
    }

    fn blit_phys(&mut self, img: &Image, ox: i32, oy: i32, f: i32, tint: Option<u32>) {
        let clip = self.clip_phys();
        let w = self.width as usize;
        for iy in 0..img.h as i32 {
            for ix in 0..img.w as i32 {
                let p = img.pixels[(iy * img.w as i32 + ix) as usize];
                let a = p >> 24;
                if a == 0 {
                    continue;
                }
                let c = tint.unwrap_or(p & 0xFF_FFFF);
                for dy in 0..f {
                    let y = oy + iy * f + dy;
                    if y < clip.y0 || y >= clip.y1 {
                        continue;
                    }
                    for dx in 0..f {
                        let x = ox + ix * f + dx;
                        if x < clip.x0 || x >= clip.x1 {
                            continue;
                        }
                        let i = y as usize * w + x as usize;
                        self.pixels[i] = if a >= 255 { c } else { blend(self.pixels[i], c, a) };
                    }
                }
            }
        }
    }

    /// Copie une image étirée (plus proche voisin) dans le rectangle logique `r`.
    pub fn draw_image(&mut self, img: &Image, r: Rect) {
        let p = self.phys(r);
        let c = p.intersect(&self.clip_phys());
        if c.is_empty() || img.w == 0 || img.h == 0 {
            return;
        }
        let (pw, ph) = ((p.x1 - p.x0).max(1), (p.y1 - p.y0).max(1));
        let w = self.width as usize;
        for y in c.y0..c.y1 {
            let iy = (((y - p.y0) as i64 * img.h as i64) / ph as i64) as u32;
            for x in c.x0..c.x1 {
                let ix = (((x - p.x0) as i64 * img.w as i64) / pw as i64) as u32;
                let px = img.get(ix.min(img.w - 1), iy.min(img.h - 1));
                let a = px >> 24;
                if a == 0 {
                    continue;
                }
                let i = y as usize * w + x as usize;
                self.pixels[i] = if a >= 255 { px & 0xFF_FFFF } else { blend(self.pixels[i], px, a) };
            }
        }
    }

    /// Encode le canevas en PNG (RGB 8 bits).
    pub fn encode_png(&self) -> Vec<u8> {
        encode_png_rgb(self.width, self.height, &self.pixels)
    }

    /// Enregistre le canevas en PNG (pour les tests et les captures).
    pub fn to_png(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, self.encode_png())
    }

    /// Copie le contenu d'un autre canevas en `(x, y)` physiques (pour assembler des planches).
    pub fn paste(&mut self, other: &Canvas, x: i32, y: i32) {
        for oy in 0..other.height as i32 {
            for ox in 0..other.width as i32 {
                let (tx, ty) = (x + ox, y + oy);
                if tx >= 0 && ty >= 0 && tx < self.width as i32 && ty < self.height as i32 {
                    self.pixels[ty as usize * self.width as usize + tx as usize] =
                        other.pixels[oy as usize * other.width as usize + ox as usize];
                }
            }
        }
    }
}

/// Encode un tampon `0x00RRGGBB` en PNG.
pub fn encode_png_rgb(width: u32, height: u32, pixels: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().expect("en-tête PNG");
        let mut data = Vec::with_capacity(pixels.len() * 3);
        for &p in pixels {
            data.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
        }
        w.write_image_data(&data).expect("données PNG");
    }
    out
}

/// Mélange `src` sur `dst` avec une opacité `a` (0 à 255).
#[inline]
pub fn blend(dst: u32, src: u32, a: u32) -> u32 {
    if a >= 255 {
        return src & 0xFF_FFFF;
    }
    let ia = 255 - a;
    let r = (((src >> 16) & 0xFF) * a + ((dst >> 16) & 0xFF) * ia + 127) / 255;
    let g = (((src >> 8) & 0xFF) * a + ((dst >> 8) & 0xFF) * ia + 127) / 255;
    let b = ((src & 0xFF) * a + (dst & 0xFF) * ia + 127) / 255;
    (r << 16) | (g << 8) | b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_and_lines() {
        let mut c = Canvas::new(30, 30, 1.5);
        assert_eq!(c.line_px(), 2);
        assert_eq!(c.logical_size(), Size::new(20, 20));
        c.clear(0);
        c.hline(2, 2, 4, 0xFF0000);
        // x de 3 à 9 physiques, y de 3 à 5.
        assert_eq!(c.get_phys(3, 3), 0xFF0000);
        assert_eq!(c.get_phys(8, 4), 0xFF0000);
        assert_eq!(c.get_phys(9, 3), 0);
        assert_eq!(c.get_phys(3, 5), 0);
    }

    #[test]
    fn clip_stack() {
        let mut c = Canvas::new(10, 10, 1.0);
        c.push_clip(Rect::new(2, 2, 3, 3));
        c.fill(Rect::new(0, 0, 10, 10), 0xFFFFFF);
        c.pop_clip();
        assert_eq!(c.get(1, 1), 0);
        assert_eq!(c.get(2, 2), 0xFFFFFF);
        assert_eq!(c.get(5, 5), 0);
    }

    #[test]
    fn button_bevel_corners() {
        let t = Theme::classic();
        let mut c = Canvas::new(10, 10, 1.0);
        c.fill(Rect::new(0, 0, 10, 10), t.face);
        c.bevel(Rect::new(0, 0, 10, 10), Bevel::Button, &t);
        assert_eq!(c.get(0, 0), t.highlight);
        assert_eq!(c.get(9, 0), t.dark, "coin haut-droit noir comme DrawEdge");
        assert_eq!(c.get(9, 9), t.dark);
        assert_eq!(c.get(1, 1), t.light);
        assert_eq!(c.get(8, 8), t.shadow);
        assert_eq!(c.get(4, 4), t.face);
    }

    #[test]
    fn integer_icon_blit() {
        let mut img = Image::new(2, 2);
        img.set_rgb(0, 0, 0xFF0000);
        let mut c = Canvas::new(6, 6, 1.5);
        c.blit(&img, 0, 0);
        // Facteur 1 à 1,5x, centré dans la boîte de 3x3 physiques.
        assert_eq!(c.icon_factor(), 1);
        let mut c2 = Canvas::new(8, 8, 2.0);
        c2.blit(&img, 0, 0);
        assert_eq!(c2.get_phys(0, 0), 0xFF0000);
        assert_eq!(c2.get_phys(1, 1), 0xFF0000);
        assert_eq!(c2.get_phys(2, 2), 0);
    }

    #[test]
    fn png_roundtrip() {
        let mut c = Canvas::new(4, 3, 1.0);
        c.fill(Rect::new(1, 1, 2, 1), 0x123456);
        let bytes = c.encode_png();
        let img = Image::from_png(&bytes).unwrap();
        assert_eq!((img.w, img.h), (4, 3));
        assert_eq!(img.get(1, 1) & 0xFF_FFFF, 0x123456);
    }
}
