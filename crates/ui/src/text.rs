//! Polices et texte : mise en forme par cosmic-text (repli de polices, emoji, écritures complexes),
//! rastérisation par swash à la taille physique, caches des glyphes et des textes mis en forme.
//!
//! Deux polices jouent les rôles de Windows 98 :
//! - la police des **discussions** (Fixedsys Excelsior : 8x16, nette à 1x et aux multiples entiers) ;
//! - la police de l'**interface** (Pixel Operator, régulière et grasse : le rôle de « MS Sans Serif »).
//!
//! Les polices « pixel » (avec une grille, voir [`FontSpec::grid`]) sont rendues sans anticrénelage : chaque
//! glyphe est recalé sur la grille de ses pixels de dessin puis seuillé, ce qui donne des pixels francs à toutes
//! les échelles (à 1,5x, l'effet est celui d'un agrandissement au plus proche voisin). Les avances sont
//! arrondies au pixel logique, comme le faisait GDI.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use cosmic_text::{Attrs, AttrsList, Family, FontSystem, Hinting, Metrics, ShapeLine, Shaping, Wrap, fontdb};
use swash::scale::{Render, ScaleContext, Source, StrikeWith, image::Content};
use swash::zeno::{Angle, Format, Transform, Vector};

use crate::canvas::{Canvas, blend};
use crate::geom::Rect;
use crate::theme::Theme;

/// Rôle d'une police.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontKind {
    /// Menus, boutons, boîtes de dialogue.
    #[default]
    Ui,
    /// Fenêtres de discussion, ligne de saisie, liste des pseudos, éditeur de scripts.
    Chat,
}

/// Rôle et graisse/italique d'un texte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextStyle {
    pub kind: FontKind,
    pub bold: bool,
    pub italic: bool,
}

impl TextStyle {
    pub const UI: TextStyle = TextStyle { kind: FontKind::Ui, bold: false, italic: false };
    pub const UI_BOLD: TextStyle = TextStyle { kind: FontKind::Ui, bold: true, italic: false };
    pub const CHAT: TextStyle = TextStyle { kind: FontKind::Chat, bold: false, italic: false };

    pub const fn bold(mut self, b: bool) -> TextStyle {
        self.bold = b;
        self
    }
    pub const fn italic(mut self, i: bool) -> TextStyle {
        self.italic = i;
        self
    }
}

/// Alignement horizontal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Description d'une police pour un rôle.
#[derive(Clone, Debug, PartialEq)]
pub struct FontSpec {
    /// Nom de famille (tel que la police le déclare).
    pub family: String,
    /// Taille de l'em, en pixels logiques.
    pub size: f32,
    /// Hauteur d'une ligne, en pixels logiques.
    pub line_height: i32,
    /// Distance du haut de la ligne à la ligne de base.
    pub ascent: i32,
    /// Police « pixel » : unités de la police par pixel de dessin (73,125 pour Fixedsys Excelsior, 100 pour
    /// Pixel Operator). `None` pour une police vectorielle ordinaire (anticrénelée).
    pub grid: Option<f32>,
}

/// Un glyphe mis en forme, en pixels logiques.
#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    /// Position de l'avance depuis le début du texte (entière : les avances sont arrondies).
    pub x: f32,
    /// Avance (entière).
    pub w: f32,
    /// Octets du texte couverts par ce glyphe.
    pub start: u32,
    pub end: u32,
    pub font_id: fontdb::ID,
    pub glyph_id: u16,
    /// Taille de l'em (logique).
    pub size: f32,
    pub weight: u16,
    /// Décalage fin du glyphe (en em), donné par la mise en forme.
    pub x_offset: f32,
    pub y_offset: f32,
    /// Italique synthétique (la police n'a pas d'italique).
    pub fake_italic: bool,
    /// Gras demandé mais absent de la police : dessiné deux fois, décalé d'un pixel.
    pub fake_bold: bool,
    /// Valeur libre de l'appelant (numéro du morceau de texte mis en forme).
    pub meta: u32,
}

impl Glyph {
    /// Fin de l'avance.
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
}

/// Un texte mis en forme sur une seule ligne.
#[derive(Clone, Debug, Default)]
pub struct ShapedText {
    /// Le texte réellement mis en forme (les caractères de contrôle remplacés par des espaces).
    pub text: String,
    /// Glyphes dans l'ordre visuel.
    pub glyphs: Vec<Glyph>,
    /// Largeur totale (entière).
    pub width: f32,
}

impl ShapedText {
    /// Largeur en pixels logiques.
    pub fn width_px(&self) -> i32 {
        self.width.ceil() as i32
    }

    /// Abscisse du curseur placé avant l'octet `index` (à la fin si `index` dépasse).
    pub fn x_of(&self, index: usize) -> f32 {
        for g in &self.glyphs {
            if g.start as usize >= index {
                return g.x;
            }
            if (g.start as usize) < index && index < g.end as usize {
                // Au milieu d'une grappe : interpoler.
                let n = (g.end - g.start) as f32;
                return g.x + g.w * (index as f32 - g.start as f32) / n;
            }
        }
        self.width
    }

    /// Octet du texte le plus proche de l'abscisse `x` (pour placer le curseur).
    pub fn index_at(&self, x: f32) -> usize {
        if x <= 0.0 {
            return 0;
        }
        for g in &self.glyphs {
            if x < g.right() {
                return if x < g.x + g.w / 2.0 { g.start as usize } else { g.end as usize };
            }
        }
        self.text.len()
    }

    /// Index du glyphe sous l'abscisse `x`.
    pub fn glyph_at(&self, x: f32) -> Option<usize> {
        self.glyphs.iter().position(|g| x >= g.x && x < g.right())
    }
}

/// Une ligne produite par le retour à la ligne : glyphes `start..end` de la ligne mise en forme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WrapRow {
    pub start: usize,
    pub end: usize,
    /// Abscisse (dans le texte mis en forme) du premier glyphe de la ligne.
    pub x0: f32,
    /// Retrait à gauche de cette ligne, en pixels logiques.
    pub indent: f32,
}

impl WrapRow {
    /// Octets couverts par la ligne.
    pub fn byte_range(&self, shaped: &ShapedText) -> Range<usize> {
        if self.start >= self.end {
            let b = shaped.glyphs.get(self.start).map_or(shaped.text.len(), |g| g.start as usize);
            return b..b;
        }
        let s = shaped.glyphs[self.start..self.end].iter().map(|g| g.start).min().unwrap_or(0) as usize;
        let e = shaped.glyphs[self.start..self.end].iter().map(|g| g.end).max().unwrap_or(0) as usize;
        s..e
    }
}

/// Coupe un texte mis en forme en lignes de largeur `width` au plus, aux espaces (ou au milieu d'un mot trop
/// long). Les lignes suivant la première sont décalées de `hang` pixels (retrait suspendu de mIRC).
pub fn wrap_glyphs(shaped: &ShapedText, width: f32, hang: f32) -> Vec<WrapRow> {
    let gl = &shaped.glyphs;
    let n = gl.len();
    let text = shaped.text.as_bytes();
    let is_space = |g: &Glyph| g.end == g.start + 1 && text.get(g.start as usize) == Some(&b' ');
    let width = width.max(1.0);
    let hang = hang.clamp(0.0, (width / 2.0).floor());
    let mut rows = Vec::new();
    if n == 0 || shaped.width <= width {
        rows.push(WrapRow { start: 0, end: n, x0: 0.0, indent: 0.0 });
        return rows;
    }
    let mut start = 0;
    while start < n {
        let indent = if rows.is_empty() { 0.0 } else { hang };
        let avail = width - indent;
        let x0 = gl[start].x;
        let mut end = start;
        let mut last_break = None;
        while end < n && (gl[end].right() - x0 <= avail || end == start) {
            if is_space(&gl[end]) {
                last_break = Some(end + 1);
            }
            end += 1;
        }
        if end >= n {
            rows.push(WrapRow { start, end: n, x0, indent });
            break;
        }
        let cut = if is_space(&gl[end]) {
            end
        } else if let Some(b) = last_break.filter(|&b| b > start) {
            b
        } else {
            // Mot plus long que la ligne : coupure forcée, sans séparer une grappe.
            let mut c = end.max(start + 1);
            while c < n && c > start + 1 && gl[c].start == gl[c - 1].start {
                c -= 1;
            }
            c
        };
        rows.push(WrapRow { start, end: cut, x0, indent });
        start = cut;
        while start < n && is_space(&gl[start]) {
            start += 1;
        }
    }
    rows
}

/// Coupe le texte d'une étiquette `&Fichier` : texte affiché et octet (dans le texte affiché) du caractère
/// souligné. `&&` donne un `&`.
pub fn parse_mnemonic(text: &str) -> (String, Option<usize>) {
    let mut out = String::with_capacity(text.len());
    let mut mark = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            match chars.peek() {
                Some('&') => {
                    chars.next();
                    out.push('&');
                }
                Some(_) if mark.is_none() => mark = Some(out.len()),
                _ => {}
            }
        } else {
            out.push(c);
        }
    }
    (out, mark)
}

/// La lettre de raccourci d'une étiquette `&Fichier` (en minuscule).
pub fn mnemonic_char(text: &str) -> Option<char> {
    let (s, m) = parse_mnemonic(text);
    m.and_then(|i| s[i..].chars().next()).map(|c| c.to_lowercase().next().unwrap_or(c))
}

/// Attributs cosmic-text d'un style (la famille est possédée, d'où ce détour).
struct AttrSpec {
    family: String,
    size: f32,
    line_height: f32,
    bold: bool,
    italic: bool,
    meta: usize,
}

impl AttrSpec {
    /// Attributs d'un style. Le gras n'est demandé à cosmic-text que si la famille a une face grasse : sinon il
    /// prendrait une autre famille ; on garde la police et on simule le gras au dessin.
    fn new(st: TextStyle, meta: u32, inner: &Inner) -> AttrSpec {
        let (spec, has_bold) = match st.kind {
            FontKind::Chat => (&inner.chat, inner.chat_has_bold),
            FontKind::Ui => (&inner.ui, inner.ui_has_bold),
        };
        AttrSpec {
            family: spec.family.clone(),
            size: spec.size,
            line_height: spec.line_height as f32,
            bold: st.bold && has_bold,
            italic: st.italic,
            meta: meta as usize,
        }
    }

    fn attrs(&self) -> Attrs<'_> {
        Attrs::new()
            .family(Family::Name(&self.family))
            .weight(if self.bold { cosmic_text::Weight::BOLD } else { cosmic_text::Weight::NORMAL })
            .style(if self.italic { cosmic_text::Style::Italic } else { cosmic_text::Style::Normal })
            .metrics(Metrics::new(self.size, self.line_height))
            .metadata(self.meta)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GlyphKey {
    font_id: fontdb::ID,
    glyph_id: u16,
    ppem_bits: u32,
    weight: u16,
    italic: bool,
}

/// Image d'un glyphe à la taille physique.
struct GlyphBitmap {
    left: i32,
    top: i32,
    w: u32,
    h: u32,
    /// Vrai : RGBA (emoji en couleur) ; faux : masque alpha.
    color: bool,
    data: Vec<u8>,
}

struct FaceMeta {
    upem: f32,
    grid: Option<f32>,
    bold: bool,
}

struct Inner {
    fs: FontSystem,
    scale_ctx: ScaleContext,
    glyphs: HashMap<GlyphKey, Option<Rc<GlyphBitmap>>>,
    shaped: HashMap<(TextStyle, String), Rc<ShapedText>>,
    faces: HashMap<fontdb::ID, FaceMeta>,
    chat: FontSpec,
    ui: FontSpec,
    chat_has_bold: bool,
    ui_has_bold: bool,
    /// Famille d'emoji en couleur, si le système en a une.
    emoji_family: Option<String>,
    generation: u64,
}

impl Inner {
    /// Recalcule ce qui dépend du contenu de la base de polices.
    fn refresh_families(&mut self) {
        let db = self.fs.db();
        let has_bold = |fam: &str| db.faces().any(|f| f.weight.0 >= 600 && f.families.iter().any(|(n, _)| n == fam));
        self.chat_has_bold = has_bold(&self.chat.family);
        self.ui_has_bold = has_bold(&self.ui.family);
        const EMOJI: [&str; 6] =
            ["Apple Color Emoji", "Noto Color Emoji", "Segoe UI Emoji", "Twemoji", "Twitter Color Emoji", "EmojiOne Color"];
        self.emoji_family = EMOJI
            .iter()
            .find(|name| db.faces().any(|f| f.families.iter().any(|(n, _)| n == *name)))
            .map(|s| s.to_string());
        self.shaped.clear();
        self.generation += 1;
    }
}

/// Plages d'octets des emoji « en image » (pictogrammes du plan 1, ou symboles suivis de U+FE0F), avec leurs
/// séquences (ZWJ, teintes, drapeaux).
fn emoji_ranges(text: &str) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let is_pict = |c: char| matches!(c as u32, 0x1F000..=0x1FAFF);
    let is_mod = |c: char| matches!(c as u32, 0xFE0F | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F | 0x20E3);
    while i < chars.len() {
        let (b, c) = chars[i];
        let next = chars.get(i + 1).map(|x| x.1);
        let start = if is_pict(c) {
            true
        } else {
            next == Some('\u{FE0F}') && (c as u32 >= 0x2000 || c.is_ascii_digit() || c == '#' || c == '*')
        };
        if !start {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        loop {
            match chars.get(j).map(|x| x.1) {
                Some(m) if is_mod(m) => j += 1,
                Some('\u{200D}') if j + 1 < chars.len() => j += 2,
                Some(r) if (0x1F1E6..=0x1F1FF).contains(&(r as u32)) && (0x1F1E6..=0x1F1FF).contains(&(c as u32)) && j == i + 1 => j += 1,
                _ => break,
            }
        }
        let e = chars.get(j).map_or(text.len(), |x| x.0);
        out.push(b..e);
        i = j;
    }
    out
}

/// Les polices d'AlpIRC, leurs caches et le dessin du texte.
///
/// S'utilise par référence partagée (`&Fonts`) : les caches sont à l'intérieur (`RefCell`). Un seul fil
/// d'exécution (celui de l'interface).
pub struct Fonts {
    inner: RefCell<Inner>,
}

/// Taille de l'em de Fixedsys Excelsior pour des pixels de dessin d'un pixel (1000 / 73,125).
pub const FIXEDSYS_GRID: f32 = 73.125;
/// Grille de Pixel Operator (1600 unités par em, pixels de 100 unités : nette à 16 px).
pub const PIXEL_OPERATOR_GRID: f32 = 100.0;

fn system_locale() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            let v = v.split('.').next().unwrap_or("").replace('_', "-");
            if v.len() >= 2 && v != "C" && v != "POSIX" {
                return v;
            }
        }
    }
    "en-US".to_string()
}

impl Fonts {
    /// Charge les polices intégrées : `chat` (Fixedsys Excelsior), `ui` et `ui_bold` (Pixel Operator).
    ///
    /// Aucune police du système n'est chargée (rapide) ; voir [`Fonts::load_system_fallbacks`] pour les
    /// emoji et les écritures absentes des polices intégrées.
    pub fn new(chat: &'static [u8], ui: &'static [u8], ui_bold: &'static [u8]) -> Fonts {
        let mut db = fontdb::Database::new();
        let mut faces = HashMap::new();
        let family_of = |db: &mut fontdb::Database, data: &'static [u8], grid: Option<f32>, faces: &mut HashMap<_, _>| {
            let ids = db.load_font_source(fontdb::Source::Binary(Arc::new(data)));
            let mut fam = String::new();
            for id in ids {
                if let Some(info) = db.face(id) {
                    if fam.is_empty() {
                        fam = info.families.first().map(|f| f.0.clone()).unwrap_or_default();
                    }
                    let upem = swash::FontRef::from_index(data, info.index as usize)
                        .map(|f| f.metrics(&[]).units_per_em as f32)
                        .unwrap_or(1000.0);
                    faces.insert(id, FaceMeta { upem, grid, bold: info.weight.0 >= 600 });
                }
            }
            fam
        };
        let chat_family = family_of(&mut db, chat, Some(FIXEDSYS_GRID), &mut faces);
        let ui_family = family_of(&mut db, ui, Some(PIXEL_OPERATOR_GRID), &mut faces);
        let _ = family_of(&mut db, ui_bold, Some(PIXEL_OPERATOR_GRID), &mut faces);
        db.set_monospace_family(chat_family.clone());
        db.set_sans_serif_family(ui_family.clone());
        let fs = FontSystem::new_with_locale_and_db(system_locale(), db);
        let fonts = Fonts {
            inner: RefCell::new(Inner {
                fs,
                scale_ctx: ScaleContext::new(),
                glyphs: HashMap::new(),
                shaped: HashMap::new(),
                faces,
                chat: FontSpec {
                    family: chat_family,
                    size: 1000.0 / FIXEDSYS_GRID,
                    line_height: 16,
                    ascent: 13,
                    grid: Some(FIXEDSYS_GRID),
                },
                ui: FontSpec { family: ui_family, size: 16.0, line_height: 16, ascent: 13, grid: Some(PIXEL_OPERATOR_GRID) },
                chat_has_bold: false,
                ui_has_bold: false,
                emoji_family: None,
                generation: 1,
            }),
        };
        fonts.inner.borrow_mut().refresh_families();
        fonts
    }

    /// Ajoute les polices du système comme polices de repli (emoji en couleur, CJK, arabe…). Peut prendre
    /// une seconde : à appeler une fois, éventuellement après le premier affichage.
    pub fn load_system_fallbacks(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.fs.db_mut().load_system_fonts();
        inner.refresh_families();
    }

    /// Ajoute des fichiers de polices comme polices de repli, lus à la demande (pas gardés en mémoire) : pour
    /// les systèmes que [`Fonts::load_system_fallbacks`] ne connaît pas (Android : `/system/fonts`). Renvoie le
    /// nombre de fichiers lus.
    pub fn load_font_files(&self, paths: &[std::path::PathBuf]) -> usize {
        let mut inner = self.inner.borrow_mut();
        let n = paths.iter().filter(|p| inner.fs.db_mut().load_font_file(p).is_ok()).count();
        inner.refresh_families();
        n
    }

    /// Ajoute une police (fichier TTF/OTF en mémoire), utilisable par son nom de famille dans
    /// [`Fonts::set_spec`] ou comme repli. Renvoie son nom de famille.
    pub fn load_font_data(&self, data: Vec<u8>) -> Option<String> {
        let mut inner = self.inner.borrow_mut();
        let ids = inner.fs.db_mut().load_font_source(fontdb::Source::Binary(Arc::new(data)));
        let fam = ids.first().and_then(|id| inner.fs.db().face(*id)).and_then(|f| f.families.first().map(|f| f.0.clone()));
        let metas: Vec<_> = ids
            .iter()
            .filter_map(|id| {
                let info = inner.fs.db().face(*id)?;
                let bold = info.weight.0 >= 600;
                let upem = inner
                    .fs
                    .db()
                    .with_face_data(*id, |d, i| swash::FontRef::from_index(d, i as usize).map(|f| f.metrics(&[]).units_per_em))
                    .flatten()
                    .unwrap_or(1000) as f32;
                Some((*id, FaceMeta { upem, grid: None, bold }))
            })
            .collect();
        inner.faces.extend(metas);
        inner.refresh_families();
        fam
    }

    /// Description de la police d'un rôle.
    pub fn spec(&self, kind: FontKind) -> FontSpec {
        let inner = self.inner.borrow();
        match kind {
            FontKind::Chat => inner.chat.clone(),
            FontKind::Ui => inner.ui.clone(),
        }
    }

    /// Change la police d'un rôle (taille, famille). Invalide les caches.
    pub fn set_spec(&self, kind: FontKind, spec: FontSpec) {
        let mut inner = self.inner.borrow_mut();
        match kind {
            FontKind::Chat => inner.chat = spec,
            FontKind::Ui => inner.ui = spec,
        }
        inner.glyphs.clear();
        inner.refresh_families();
    }

    /// Agrandit la police des discussions d'un facteur entier (1 = 8x16, 2 = 16x32…), en restant net.
    pub fn set_chat_zoom(&self, factor: u32) {
        let f = factor.max(1) as f32;
        let mut spec = self.spec(FontKind::Chat);
        spec.size = 1000.0 / FIXEDSYS_GRID * f;
        spec.line_height = 16 * factor.max(1) as i32;
        spec.ascent = 13 * factor.max(1) as i32;
        self.set_spec(FontKind::Chat, spec);
    }

    /// Compteur incrémenté à chaque changement de police (pour invalider les caches de mise en page).
    pub fn generation(&self) -> u64 {
        self.inner.borrow().generation
    }

    /// Hauteur de ligne d'un rôle.
    pub fn line_height(&self, kind: FontKind) -> i32 {
        let inner = self.inner.borrow();
        match kind {
            FontKind::Chat => inner.chat.line_height,
            FontKind::Ui => inner.ui.line_height,
        }
    }

    /// Distance du haut de la ligne à la ligne de base.
    pub fn ascent(&self, kind: FontKind) -> i32 {
        let inner = self.inner.borrow();
        match kind {
            FontKind::Chat => inner.chat.ascent,
            FontKind::Ui => inner.ui.ascent,
        }
    }

    /// Largeur moyenne d'un caractère (comme `GetTextMetrics` : moyenne de l'alphabet), pour les unités de
    /// dialogue.
    pub fn avg_char_width(&self, style: TextStyle) -> f32 {
        let s = self.shape("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz", style);
        s.width / 52.0
    }

    /// Largeur d'un texte, en pixels logiques.
    pub fn measure(&self, text: &str, style: TextStyle) -> i32 {
        self.shape(text, style).width_px()
    }

    /// Met en forme un texte d'un seul style (avec cache).
    pub fn shape(&self, text: &str, style: TextStyle) -> Rc<ShapedText> {
        let key = (style, text.to_string());
        if let Some(s) = self.inner.borrow().shaped.get(&key) {
            return s.clone();
        }
        let shaped = Rc::new(self.shape_rich(text, style, &[]));
        let mut inner = self.inner.borrow_mut();
        if inner.shaped.len() > 4096 {
            inner.shaped.clear();
        }
        inner.shaped.insert(key, shaped.clone());
        shaped
    }

    /// Met en forme un texte à plusieurs styles (sans cache) : `runs` donne, pour des plages d'octets, le
    /// style et une valeur `meta` recopiée dans chaque [`Glyph`]. Hors des plages : `base`, meta 0.
    pub fn shape_rich(&self, text: &str, base: TextStyle, runs: &[(Range<usize>, TextStyle, u32)]) -> ShapedText {
        let clean = sanitize(text);
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        let base_a = AttrSpec::new(base, 0, inner);
        let mut run_a: Vec<_> = runs.iter().map(|(r, st, m)| (r.clone(), AttrSpec::new(*st, *m, inner))).collect();
        if let Some(fam) = &inner.emoji_family {
            // Les emoji vont à la police en couleur, même si une autre police a un pictogramme monochrome.
            for r in emoji_ranges(&clean) {
                let (st, m) = runs.iter().rev().find(|x| x.0.contains(&r.start)).map_or((base, 0), |x| (x.1, x.2));
                let mut a = AttrSpec::new(st, m, inner);
                a.family = fam.clone();
                a.bold = false;
                run_a.push((r, a));
            }
        }
        let mut list = AttrsList::new(&base_a.attrs());
        for (r, a) in &run_a {
            let r = r.start.min(clean.len())..r.end.min(clean.len());
            if !r.is_empty() {
                list.add_span(r, &a.attrs());
            }
        }
        // Gras demandé pour l'octet `at` (la dernière plage qui le contient l'emporte, comme pour AttrsList).
        let bold_at = |at: usize| -> bool { runs.iter().rev().find(|r| r.0.contains(&at)).map_or(base.bold, |r| r.1.bold) };
        let line = ShapeLine::new(&mut inner.fs, &clean, &list, Shaping::Advanced, 8);
        let layout = line.layout(base_a.size, None, Wrap::None, None, None, Hinting::Disabled);
        let mut glyphs = Vec::new();
        let mut cum = 0.0f32;
        if let Some(l) = layout.first() {
            glyphs.reserve(l.glyphs.len());
            for g in &l.glyphs {
                // Tabulation : jusqu'au prochain arrêt (4 espaces).
                let adv = if text.as_bytes().get(g.start) == Some(&b'\t') {
                    let stop = g.w.round().max(1.0) * 4.0;
                    stop - (cum % stop)
                } else {
                    g.w.round()
                };
                let face_bold = inner.faces.get(&g.font_id).map(|f| f.bold).or_else(|| inner.fs.db().face(g.font_id).map(|f| f.weight.0 >= 600));
                let want_bold = bold_at(g.start);
                glyphs.push(Glyph {
                    x: cum,
                    w: adv,
                    start: g.start as u32,
                    end: g.end as u32,
                    font_id: g.font_id,
                    glyph_id: g.glyph_id,
                    size: g.font_size,
                    weight: g.font_weight.0,
                    x_offset: g.x_offset,
                    y_offset: g.y_offset,
                    fake_italic: g.cache_key_flags.contains(cosmic_text::CacheKeyFlags::FAKE_ITALIC),
                    fake_bold: want_bold && face_bold == Some(false),
                    meta: g.metadata as u32,
                });
                cum += adv;
            }
        }
        ShapedText { text: clean, glyphs, width: cum }
    }

    /// Dessine des glyphes : `origin_x` est l'abscisse logique du point d'avance 0 et `baseline` l'ordonnée
    /// logique de la ligne de base. `color_of` donne la couleur de chaque glyphe (`None` : ne pas le dessiner).
    pub fn draw_glyphs(
        &self,
        canvas: &mut Canvas,
        glyphs: &[Glyph],
        origin_x: f32,
        baseline: f32,
        mut color_of: impl FnMut(&Glyph) -> Option<u32>,
    ) {
        let scale = canvas.scale();
        let clip = canvas.clip_phys();
        if clip.is_empty() {
            return;
        }
        let t = canvas.line_px();
        let py = (baseline * scale).round() as i32;
        let mut inner = self.inner.borrow_mut();
        for g in glyphs {
            let Some(color) = color_of(g) else { continue };
            let px = ((origin_x + g.x + g.x_offset * g.size) * scale).round() as i32;
            if px > clip.x1 || px + ((g.w + g.size) * scale) as i32 + 2 < clip.x0 {
                continue;
            }
            let gy = py - (g.y_offset * g.size * scale).round() as i32;
            let key = GlyphKey {
                font_id: g.font_id,
                glyph_id: g.glyph_id,
                ppem_bits: (g.size * scale).to_bits(),
                weight: g.weight,
                italic: g.fake_italic,
            };
            let Some(bmp) = inner.raster(key) else { continue };
            blit_glyph(canvas, &bmp, px, gy, color);
            if g.fake_bold {
                blit_glyph(canvas, &bmp, px + t, gy, color);
            }
        }
    }

    /// Dessine un texte d'un seul style, haut de la ligne en `(x, y)`. Renvoie la largeur.
    pub fn draw_text(&self, canvas: &mut Canvas, x: i32, y: i32, text: &str, style: TextStyle, color: u32) -> i32 {
        let shaped = self.shape(text, style);
        let base = (y + self.ascent(style.kind)) as f32;
        self.draw_glyphs(canvas, &shaped.glyphs, x as f32, base, |_| Some(color));
        shaped.width_px()
    }

    /// Dessine un texte grisé en relief (blanc décalé d'un pixel, puis gris), comme Windows 98.
    pub fn draw_text_disabled(&self, canvas: &mut Canvas, x: i32, y: i32, text: &str, style: TextStyle, theme: &Theme) -> i32 {
        self.draw_text(canvas, x + 1, y + 1, text, style, theme.highlight);
        self.draw_text(canvas, x, y, text, style, theme.disabled_text)
    }

    /// Dessine une étiquette avec raccourci (`&Fichier`) dans `rect`, centrée verticalement et alignée selon
    /// `align`. `underline` : souligner la lettre du raccourci (toujours sous Windows 98 dans les boîtes de
    /// dialogue ; dans les menus, seulement quand Alt est enfoncée). `disabled` : texte en relief.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_label(
        &self,
        canvas: &mut Canvas,
        rect: Rect,
        text: &str,
        style: TextStyle,
        color: u32,
        align: Align,
        underline: bool,
        disabled: Option<&Theme>,
    ) {
        let (display, mark) = parse_mnemonic(text);
        let shaped = self.shape(&display, style);
        let lh = self.line_height(style.kind);
        let w = shaped.width_px();
        let x = match align {
            Align::Left => rect.x,
            Align::Center => rect.x + (rect.w - w) / 2,
            Align::Right => rect.right() - w,
        };
        let y = rect.y + (rect.h - lh) / 2;
        let base = (y + self.ascent(style.kind)) as f32;
        let pass = |canvas: &mut Canvas, dx: i32, col: u32| {
            self.draw_glyphs(canvas, &shaped.glyphs, (x + dx) as f32, base + dx as f32, |_| Some(col));
            if let (true, Some(m)) = (underline, mark) {
                if let Some(g) = shaped.glyphs.iter().find(|g| g.start as usize == m) {
                    canvas.hline(x + dx + g.x as i32, base as i32 + 1 + dx, g.w as i32, col);
                }
            }
        };
        match disabled {
            Some(theme) => {
                pass(canvas, 1, theme.highlight);
                pass(canvas, 0, theme.disabled_text);
            }
            None => pass(canvas, 0, color),
        }
    }

    /// Raccourcit un texte trop long avec « ... » pour tenir dans `max_w`.
    pub fn ellipsize(&self, text: &str, style: TextStyle, max_w: i32) -> String {
        let shaped = self.shape(text, style);
        if shaped.width_px() <= max_w {
            return text.to_string();
        }
        let ell = self.measure("...", style) as f32;
        let mut cut = 0;
        for g in &shaped.glyphs {
            if g.right() + ell > max_w as f32 {
                break;
            }
            cut = g.end as usize;
        }
        let clean = &shaped.text;
        let cut = (0..=cut.min(clean.len())).rev().find(|&i| clean.is_char_boundary(i)).unwrap_or(0);
        format!("{}...", clean[..cut].trim_end())
    }

    /// Coupe un texte en lignes de `width` pixels au plus (aux espaces ; les `\n` forcent une ligne).
    pub fn wrap_text(&self, text: &str, style: TextStyle, width: i32) -> Vec<String> {
        let mut out = Vec::new();
        for para in text.split('\n') {
            let para = para.trim_end_matches('\r');
            let shaped = self.shape(para, style);
            for row in wrap_glyphs(&shaped, width as f32, 0.0) {
                let r = row.byte_range(&shaped);
                out.push(shaped.text[r].trim_end().to_string());
            }
        }
        out
    }

    /// Vrai si une police (intégrée ou du système) a un glyphe pour ce caractère.
    pub fn has_glyph(&self, c: char) -> bool {
        let shaped = self.shape_rich(&c.to_string(), TextStyle::UI, &[]);
        shaped.glyphs.iter().all(|g| g.glyph_id != 0)
    }

    /// Nombre de polices connues (intégrées et système).
    pub fn face_count(&self) -> usize {
        self.inner.borrow().fs.db().len()
    }
}

impl Inner {
    fn raster(&mut self, key: GlyphKey) -> Option<Rc<GlyphBitmap>> {
        if let Some(b) = self.glyphs.get(&key) {
            return b.clone();
        }
        if self.glyphs.len() > 20_000 {
            self.glyphs.clear();
        }
        let bmp = self.raster_uncached(key).map(Rc::new);
        self.glyphs.insert(key, bmp.clone());
        bmp
    }

    fn raster_uncached(&mut self, key: GlyphKey) -> Option<GlyphBitmap> {
        let ppem = f32::from_bits(key.ppem_bits);
        let font = self.fs.get_font(key.font_id, fontdb::Weight(key.weight))?;
        let meta = self.faces.get(&key.font_id);
        let grid = meta.and_then(|m| m.grid.map(|g| (g, m.upem)));
        let sw = font.as_swash();
        let mut scaler = self.scale_ctx.builder(sw).size(ppem).hint(false).build();
        let mut offset = Vector::new(0.0, 0.0);
        if let Some((g, upem)) = grid {
            // Recaler le bord gauche du glyphe sur sa grille de dessin : certains glyphes de largeur impaire sont
            // centrés à un demi-pixel près dans leur cellule.
            if let Some(outline) = scaler.scale_outline(key.glyph_id) {
                let b = outline.bounds();
                let unit = ppem * g / upem;
                if unit > 0.0 && b.min.x.is_finite() && b.max.x > b.min.x {
                    let d = b.min.x / unit;
                    offset.x = (d.round() - d) * unit;
                }
            }
        }
        let sources = [Source::ColorOutline(0), Source::ColorBitmap(StrikeWith::BestFit), Source::Outline];
        let mut render = Render::new(&sources);
        render.format(Format::Alpha).offset(offset);
        if key.italic {
            render.transform(Some(Transform::skew(Angle::from_degrees(14.0), Angle::from_degrees(0.0))));
        }
        let img = render.render(&mut scaler, key.glyph_id)?;
        let p = img.placement;
        let color = img.content == Content::Color;
        let mut data = img.data;
        if img.content == Content::SubpixelMask {
            // Ne devrait pas arriver avec Format::Alpha : garder un canal.
            data = data.chunks(4).map(|c| c[1]).collect();
        }
        if grid.is_some() && !color {
            for a in data.iter_mut() {
                *a = if *a >= 128 { 255 } else { 0 };
            }
        }
        Some(GlyphBitmap { left: p.left, top: p.top, w: p.width, h: p.height, color, data })
    }
}

/// Copie l'image d'un glyphe, son origine (point d'avance, ligne de base) en `(px, py)` physiques.
fn blit_glyph(canvas: &mut Canvas, bmp: &GlyphBitmap, px: i32, py: i32, color: u32) {
    let clip = canvas.clip_phys();
    let x0 = px + bmp.left;
    let y0 = py - bmp.top;
    let cw = canvas.width() as usize;
    let ys = (clip.y0 - y0).max(0)..(clip.y1 - y0).min(bmp.h as i32).max(0);
    let xs = (clip.x0 - x0).max(0)..(clip.x1 - x0).min(bmp.w as i32).max(0);
    if ys.is_empty() || xs.is_empty() {
        return;
    }
    let c = color & 0xFF_FFFF;
    for gy in ys {
        let row = (y0 + gy) as usize * cw;
        for gx in xs.clone() {
            let i = row + (x0 + gx) as usize;
            if bmp.color {
                let o = (gy as usize * bmp.w as usize + gx as usize) * 4;
                let a = bmp.data[o + 3] as u32;
                if a == 0 {
                    continue;
                }
                let src = (bmp.data[o] as u32) << 16 | (bmp.data[o + 1] as u32) << 8 | bmp.data[o + 2] as u32;
                canvas.pixels[i] = blend(canvas.pixels[i], src, a);
            } else {
                let a = bmp.data[gy as usize * bmp.w as usize + gx as usize] as u32;
                if a == 0 {
                    continue;
                }
                canvas.pixels[i] = if a == 255 { c } else { blend(canvas.pixels[i], c, a) };
            }
        }
    }
}

/// Remplace les caractères de contrôle et les séparateurs de ligne par des espaces de même longueur en octets :
/// le texte reste sur une ligne et toutes les positions d'octets sont gardées.
fn sanitize(text: &str) -> String {
    let bad = |c: char| c.is_control() || c == '\u{2028}' || c == '\u{2029}';
    if !text.chars().any(bad) {
        return text.to_string();
    }
    text.chars()
        .map(|c| {
            if !bad(c) {
                c
            } else {
                match c.len_utf8() {
                    1 => ' ',
                    2 => '\u{A0}',
                    _ => '\u{3000}',
                }
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn fonts() -> Fonts {
        Fonts::new(
            include_bytes!("../../../assets/fonts/FSEX300.ttf"),
            include_bytes!("../../../assets/fonts/PixelOperator.ttf"),
            include_bytes!("../../../assets/fonts/PixelOperator-Bold.ttf"),
        )
    }

    #[test]
    fn chat_font_is_8px_monospace() {
        let f = fonts();
        assert_eq!(f.measure("abcdefghij", TextStyle::CHAT), 80);
        assert_eq!(f.measure("MMMMM", TextStyle::CHAT), 40);
        assert_eq!(f.measure("é", TextStyle::CHAT), 8);
        let ui = f.measure("File", TextStyle::UI);
        assert!(ui > 10 && ui < 30, "{ui}");
        assert!(f.measure("Bold", TextStyle::UI_BOLD) >= f.measure("Bold", TextStyle::UI));
    }

    #[test]
    fn mnemonic_parsing() {
        assert_eq!(parse_mnemonic("&Fichier"), ("Fichier".to_string(), Some(0)));
        assert_eq!(parse_mnemonic("Ou&vrir"), ("Ouvrir".to_string(), Some(2)));
        assert_eq!(parse_mnemonic("A && B"), ("A & B".to_string(), None));
        assert_eq!(mnemonic_char("&Edit"), Some('e'));
        assert_eq!(mnemonic_char("No mnemonic"), None);
    }

    #[test]
    fn wrap_with_hanging_indent() {
        let f = fonts();
        let s = f.shape("<nick> un deux trois quatre cinq six", TextStyle::CHAT);
        // 8 px par caractère : 15 caractères par ligne.
        let rows = wrap_glyphs(&s, 120.0, 56.0);
        assert!(rows.len() >= 3, "{rows:?}");
        assert_eq!(rows[0].indent, 0.0);
        assert_eq!(rows[1].indent, 56.0);
        let texts: Vec<_> = rows.iter().map(|r| s.text[r.byte_range(&s)].to_string()).collect();
        assert_eq!(texts[0], "<nick> un deux ");
        for (r, t) in rows.iter().zip(&texts) {
            let w = s.glyphs[r.start..r.end].iter().filter(|g| s.text.as_bytes()[g.start as usize] != b' ').map(|g| g.right()).fold(0.0f32, f32::max) - r.x0;
            assert!(w + r.indent <= 120.0, "{t:?} trop large");
        }
        // Mot plus long que la ligne : coupure forcée.
        let s = f.shape("aaaaaaaaaaaaaaaaaaaaaaaaa", TextStyle::CHAT);
        let rows = wrap_glyphs(&s, 80.0, 0.0);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].end - rows[0].start, 10);
    }

    #[test]
    fn caret_positions() {
        let f = fonts();
        let s = f.shape("héllo", TextStyle::CHAT);
        assert_eq!(s.x_of(0), 0.0);
        assert_eq!(s.x_of(3), 16.0); // après « hé » (é = 2 octets)
        assert_eq!(s.index_at(17.0), 3);
        assert_eq!(s.index_at(1000.0), s.text.len());
        assert_eq!(s.x_of(100), 40.0);
    }

    #[test]
    fn crisp_rendering_at_1x_and_2x() {
        let f = fonts();
        for scale in [1.0, 2.0] {
            let mut c = Canvas::new((100.0 * scale) as u32, (20.0 * scale) as u32, scale);
            c.clear(0xFFFFFF);
            f.draw_text(&mut c, 2, 2, "Hello #lyon", TextStyle::CHAT, 0x000000);
            f.draw_text(&mut c, 2, 2, "", TextStyle::UI, 0x000000);
            let mut dark = 0;
            for &p in &c.pixels {
                assert!(p == 0xFFFFFF || p == 0, "pixel gris {p:06X} à {scale}x : rendu pas net");
                if p == 0 {
                    dark += 1;
                }
            }
            assert!(dark > 50);
        }
    }

    #[test]
    fn ellipsis() {
        let f = fonts();
        let e = f.ellipsize("Un titre beaucoup trop long", TextStyle::UI, 60);
        assert!(e.ends_with("..."));
        assert!(f.measure(&e, TextStyle::UI) <= 60);
        assert_eq!(f.ellipsize("court", TextStyle::UI, 200), "court");
    }

    #[test]
    fn bold_chat_keeps_fixedsys() {
        let f = fonts();
        let s = f.shape_rich("ab cd", TextStyle::CHAT, &[(0..2, TextStyle::CHAT.bold(true), 1)]);
        assert_eq!(s.width, 40.0);
        assert!(s.glyphs[0].fake_bold && !s.glyphs[3].fake_bold);
        assert_eq!(s.glyphs[0].font_id, s.glyphs[3].font_id);
        let u = f.shape("Gras", TextStyle::UI_BOLD);
        assert!(!u.glyphs[0].fake_bold, "Pixel Operator a une vraie graisse");
    }

    #[test]
    fn emoji_sequences() {
        assert_eq!(emoji_ranges("a😀b"), vec![1..5]);
        assert_eq!(emoji_ranges("👍🏽!"), vec![0..8]);
        assert_eq!(emoji_ranges("☺ ok"), Vec::<Range<usize>>::new());
        assert_eq!(emoji_ranges("❤\u{FE0F}x").len(), 1);
        assert_eq!(emoji_ranges("🇫🇷"), vec![0..8]);
    }

    #[test]
    fn control_chars_keep_offsets() {
        let f = fonts();
        let s = f.shape("a\x02b", TextStyle::CHAT);
        assert_eq!(s.text.len(), 3);
        assert_eq!(s.glyphs.len(), 3);
        let s = f.shape("a\u{2028}b\u{85}c", TextStyle::CHAT);
        assert_eq!(s.text.len(), "a\u{2028}b\u{85}c".len());
        let s = f.shape("a\tb", TextStyle::CHAT);
        assert_eq!(s.glyphs[2].x, 32.0, "tabulation jusqu'à 4 colonnes");
    }
}
