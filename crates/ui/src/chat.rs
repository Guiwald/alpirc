//! La vue de discussion : lignes fournies par l'application ([`ChatSource`]), retour à la ligne avec retrait
//! suspendu façon mIRC, codes de mise en forme de mIRC, historique avec barre de défilement, « collé en bas »,
//! sélection à la souris (copie sans les codes), liens et canaux cliquables, recherche, ligne de repère,
//! indicateur de nouvelles lignes.
//!
//! Seules les lignes visibles sont mises en page ; la mise en forme de chaque ligne est gardée en cache par son
//! numéro de séquence (`seq`), qui doit croître strictement le long de la source. La position de défilement
//! est ancrée sur un `seq` : elle ne bouge pas quand l'application retire de vieilles lignes en tête.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;

use alpirc_proto::format::{self, Style};

use crate::canvas::Canvas;
use crate::geom::{Point, Rect};
use crate::input::{Cursor, Input, Key, MouseButton, Platform};
use crate::text::{FontKind, ShapedText, TextStyle, WrapRow, wrap_glyphs};
use crate::widgets::scrollbar::{self, Orientation, ScrollBar, ScrollMsg};
use crate::Res;

/// Une ligne de discussion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatLine<'a> {
    /// Numéro de séquence, strictement croissant dans la source.
    pub seq: u64,
    /// Texte avec les codes de mise en forme de mIRC.
    pub text: &'a str,
    /// Couleur par défaut (numéro de palette).
    pub default_fg: u8,
    /// On vous a nommé : la couleur par défaut devient celle de [`crate::Theme::activity_highlight`].
    pub highlight: bool,
}

/// Les lignes d'une fenêtre, fournies par l'application.
pub trait ChatSource {
    /// Nombre de lignes.
    fn len(&self) -> usize;
    /// Ligne `i` (0 = la plus ancienne).
    fn line(&self, i: usize) -> ChatLine<'_>;
    /// Change à chaque modification (ajout, retrait).
    fn version(&self) -> u64;
    /// Vrai si le mot est un pseudo présent (pour le double-clic). Par défaut : faux.
    fn is_nick(&self, word: &str) -> bool {
        let _ = word;
        false
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Une source simple en mémoire (tests, exemples, fenêtres sans moteur).
#[derive(Clone, Debug, Default)]
pub struct VecSource {
    pub lines: Vec<(u64, String, u8, bool)>,
    pub nicks: Vec<String>,
    next_seq: u64,
    version: u64,
}

impl VecSource {
    pub fn new() -> VecSource {
        VecSource::default()
    }

    /// Ajoute une ligne (couleur par défaut : 1, noir) ; renvoie son `seq`.
    pub fn push(&mut self, text: &str) -> u64 {
        self.push_colored(text, 1, false)
    }

    /// Ajoute une ligne avec sa couleur par défaut et l'indicateur « on vous a nommé ».
    pub fn push_colored(&mut self, text: &str, fg: u8, highlight: bool) -> u64 {
        let s = self.next_seq;
        self.next_seq += 1;
        self.lines.push((s, text.to_string(), fg, highlight));
        self.version += 1;
        s
    }

    /// Retire les `n` plus vieilles lignes.
    pub fn drop_front(&mut self, n: usize) {
        self.lines.drain(..n.min(self.lines.len()));
        self.version += 1;
    }
}

impl ChatSource for VecSource {
    fn len(&self) -> usize {
        self.lines.len()
    }
    fn line(&self, i: usize) -> ChatLine<'_> {
        let l = &self.lines[i];
        ChatLine { seq: l.0, text: &l.1, default_fg: l.2, highlight: l.3 }
    }
    fn version(&self) -> u64 {
        self.version
    }
    fn is_nick(&self, word: &str) -> bool {
        self.nicks.iter().any(|n| n.eq_ignore_ascii_case(word))
    }
}

/// Retrait des lignes coupées.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndentMode {
    /// Pas de retrait.
    None,
    /// Largeur du premier mot (l'heure, ou `<pseudo>`), comme mIRC.
    FirstWord,
    /// Retrait fixe, en pixels logiques.
    Fixed(i32),
}

/// Nature d'un mot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordKind {
    Url,
    Channel,
    Nick,
    Word,
}

/// Message de la vue de discussion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatMsg {
    /// Double-clic sur un mot.
    DoubleClick(String, WordKind),
    /// Clic sur une adresse.
    OpenUrl(String),
    /// Clic sur un nom de canal.
    JoinChannel(String),
    /// Clic droit : position et mot sous le pointeur (l'application ouvre son menu).
    RightClick { pos: Point, word: Option<(String, WordKind)> },
    /// Une sélection vient d'être faite (et copiée si [`ChatView::copy_on_select`]).
    Selected(String),
}

/// Position dans le texte (sans codes) d'une ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextPos {
    pub seq: u64,
    pub byte: usize,
}

/// Un lien détecté dans le texte (sans codes) d'une ligne.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkSpan {
    pub range: Range<usize>,
    pub kind: WordKind,
    pub target: String,
}

/// Trouve les adresses (http, https, www., irc, ircs, ftp) et les canaux (`#nom`) d'un texte.
pub fn find_links(text: &str) -> Vec<LinkSpan> {
    let mut out = Vec::new();
    let mut i = 0;
    let b = text.as_bytes();
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut s = start;
        let mut e = i;
        // Ponctuation d'ouverture et de fermeture autour du mot.
        while s < e && matches!(b[s], b'(' | b'<' | b'[' | b'"' | b'\'' | b'{') {
            s += 1;
        }
        let tok = &text[s..e];
        let lower = tok.to_ascii_lowercase();
        let kind = if ["http://", "https://", "irc://", "ircs://", "ftp://"].iter().any(|p| lower.starts_with(p)) || lower.starts_with("www.") {
            WordKind::Url
        } else if tok.starts_with('#') && tok.len() > 1 {
            WordKind::Channel
        } else {
            continue;
        };
        let opens = tok.matches('(').count();
        while e > s {
            let c = b[e - 1];
            let trim = match c {
                b'.' | b',' | b';' | b':' | b'!' | b'?' | b'>' | b']' | b'}' | b'"' | b'\'' => true,
                b')' => text[s..e].matches(')').count() > opens,
                _ => false,
            };
            if !trim {
                break;
            }
            e -= 1;
        }
        if kind == WordKind::Channel {
            // Liste de canaux séparés par des virgules : un lien par canal.
            let mut p = s;
            for part in text[s..e].split(',') {
                if part.len() > 1 && part.starts_with('#') {
                    out.push(LinkSpan { range: p..p + part.len(), kind, target: part.to_string() });
                }
                p += part.len() + 1;
            }
            continue;
        }
        if e > s && text.is_char_boundary(s) && text.is_char_boundary(e) {
            let mut target = text[s..e].to_string();
            if kind == WordKind::Url && target.to_ascii_lowercase().starts_with("www.") {
                target = format!("http://{target}");
            }
            out.push(LinkSpan { range: s..e, kind, target });
        }
    }
    out
}

/// Mise en page d'une ligne : texte sans codes mis en forme, styles des morceaux, coupure.
struct LaidLine {
    shaped: ShapedText,
    styles: Vec<Style>,
    links: Vec<LinkSpan>,
    hang: f32,
    rows: Vec<WrapRow>,
    rows_width: i32,
    used: u64,
}

struct Cache {
    generation: u64,
    lines: HashMap<u64, LaidLine>,
    frame: u64,
}

/// Une rangée visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisRow {
    /// Index de la ligne dans la source.
    pub index: usize,
    pub seq: u64,
    /// Rangée dans la ligne coupée.
    pub row: usize,
    /// Haut de la rangée.
    pub y: i32,
}

/// La vue de discussion.
pub struct ChatView {
    /// Retrait des lignes coupées.
    pub indent: IndentMode,
    /// Copier la sélection dès qu'on relâche la souris, puis l'effacer (comme mIRC).
    pub copy_on_select: bool,
    /// Texte de l'indicateur de nouvelles lignes (`{n}` = nombre).
    pub new_lines_text: String,
    /// Ligne de repère : trait rouge avant la ligne de ce `seq`.
    pub marker: Option<u64>,
    /// Marge gauche du texte.
    pub margin: i32,
    stick: bool,
    top: (u64, usize),
    sel: Option<(TextPos, TextPos)>,
    press: Option<(Point, Option<TextPos>)>,
    selecting: bool,
    hover_link: Option<(u64, Range<usize>)>,
    search: Option<String>,
    search_hit: Option<(u64, Range<usize>)>,
    last_seen: Cell<Option<u64>>,
    unseen: Cell<usize>,
    last_version: Cell<u64>,
    vbar: ScrollBar,
    cache: RefCell<Cache>,
    mouse: Point,
    autoscroll: i32,
}

impl Default for ChatView {
    fn default() -> ChatView {
        ChatView::new()
    }
}

/// Index de la première ligne de `seq` supérieur ou égal (recherche dichotomique).
fn lower_bound(src: &dyn ChatSource, seq: u64) -> usize {
    let (mut lo, mut hi) = (0, src.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if src.line(mid).seq < seq {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

impl ChatView {
    pub fn new() -> ChatView {
        ChatView {
            indent: IndentMode::FirstWord,
            copy_on_select: true,
            new_lines_text: "{n} new lines \u{25BC}".to_string(),
            marker: None,
            margin: 3,
            stick: true,
            top: (0, 0),
            sel: None,
            press: None,
            selecting: false,
            hover_link: None,
            search: None,
            search_hit: None,
            last_seen: Cell::new(None),
            unseen: Cell::new(0),
            last_version: Cell::new(u64::MAX),
            vbar: ScrollBar::new(Orientation::Vertical),
            cache: RefCell::new(Cache { generation: 0, lines: HashMap::new(), frame: 0 }),
            mouse: Point::default(),
            autoscroll: 0,
        }
    }

    /// Vrai si la vue suit les nouvelles lignes (collée en bas).
    pub fn is_at_bottom(&self) -> bool {
        self.stick
    }

    /// Nombre de lignes arrivées sous la vue depuis qu'on a remonté.
    pub fn unseen(&self) -> usize {
        self.unseen.get()
    }

    /// Recolle la vue en bas.
    pub fn scroll_to_bottom(&mut self) {
        self.stick = true;
        self.unseen.set(0);
    }

    /// Remonte tout en haut.
    pub fn scroll_to_top(&mut self, src: &dyn ChatSource) {
        if !src.is_empty() {
            self.top = (src.line(0).seq, 0);
            self.stick = false;
        }
    }

    /// Ancre de la vue quand elle n'est pas collée en bas : `seq` de la ligne du haut et rangée.
    pub fn top_anchor(&self) -> (u64, usize) {
        self.top
    }

    /// Efface la sélection.
    pub fn clear_selection(&mut self) {
        self.sel = None;
    }

    /// Sélection (début, fin) dans l'ordre.
    pub fn selection(&self) -> Option<(TextPos, TextPos)> {
        let (a, b) = self.sel?;
        if a == b {
            return None;
        }
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    /// Texte sélectionné, sans codes de mise en forme (une ligne par ligne de discussion).
    pub fn selected_text(&self, src: &dyn ChatSource) -> Option<String> {
        let (a, b) = self.selection()?;
        let i0 = lower_bound(src, a.seq);
        let mut out = String::new();
        let mut i = i0;
        while i < src.len() {
            let l = src.line(i);
            if l.seq > b.seq {
                break;
            }
            let plain = format::strip(l.text);
            let s = if l.seq == a.seq { a.byte.min(plain.len()) } else { 0 };
            let e = if l.seq == b.seq { b.byte.min(plain.len()) } else { plain.len() };
            if i > i0 {
                out.push('\n');
            }
            if s < e && plain.is_char_boundary(s) && plain.is_char_boundary(e) {
                out.push_str(&plain[s..e]);
            }
            i += 1;
        }
        Some(out)
    }

    /// Texte recherché (surligné) ; `None` efface la recherche.
    pub fn set_search(&mut self, needle: Option<&str>) {
        self.search = needle.filter(|s| !s.is_empty()).map(|s| s.to_lowercase());
        self.search_hit = None;
    }

    /// Occurrence courante de la recherche.
    pub fn search_hit(&self) -> Option<(u64, Range<usize>)> {
        self.search_hit.clone()
    }

    fn text_rect(&self, rect: Rect) -> Rect {
        let (_, body) = rect.split_right(scrollbar::SIZE);
        body.inset_sides(self.margin, 0, 2, 0)
    }

    fn vbar_rect(&self, rect: Rect) -> Rect {
        rect.split_right(scrollbar::SIZE).0
    }

    fn line_h(&self, res: &Res) -> i32 {
        res.fonts.line_height(FontKind::Chat)
    }

    /// Met en page la ligne `i` (avec cache) et appelle `f`.
    fn with_line<R>(&self, res: &Res, src: &dyn ChatSource, i: usize, width: i32, f: impl FnOnce(&LaidLine) -> R) -> R {
        let l = src.line(i);
        let mut cache = self.cache.borrow_mut();
        let generation = res.fonts.generation();
        if cache.generation != generation {
            cache.lines.clear();
            cache.generation = generation;
        }
        let frame = cache.frame;
        let entry = cache.lines.entry(l.seq).or_insert_with(|| layout_line(res, l.text, self.indent));
        if entry.rows_width != width {
            entry.rows = wrap_glyphs(&entry.shaped, width as f32, entry.hang);
            entry.rows_width = width;
        }
        entry.used = frame;
        f(entry)
    }

    fn row_count(&self, res: &Res, src: &dyn ChatSource, i: usize, width: i32) -> usize {
        self.with_line(res, src, i, width, |l| l.rows.len().max(1))
    }

    /// Ancre du haut quand la vue est collée en bas (et vrai si tout tient dans la vue).
    fn bottom_anchor(&self, res: &Res, src: &dyn ChatSource, width: i32, rows: usize) -> (usize, usize, bool) {
        let n = src.len();
        let mut need = rows;
        let mut i = n;
        while i > 0 {
            i -= 1;
            let rc = self.row_count(res, src, i, width);
            if rc >= need {
                return (i, rc - need, false);
            }
            need -= rc;
        }
        (0, 0, true)
    }

    /// Nombre de rangées entières visibles.
    fn view_rows(&self, res: &Res, rect: Rect) -> usize {
        (self.text_rect(rect).h / self.line_h(res)).max(1) as usize
    }

    /// Ancre courante du haut, en index de ligne.
    fn top_index(&self, res: &Res, src: &dyn ChatSource, rect: Rect) -> (usize, usize) {
        let tr = self.text_rect(rect);
        if self.stick {
            let (i, r, _) = self.bottom_anchor(res, src, tr.w, self.view_rows(res, rect));
            return (i, r);
        }
        let i = lower_bound(src, self.top.0);
        if i >= src.len() {
            return (src.len().saturating_sub(1), 0);
        }
        let row = if src.line(i).seq == self.top.0 { self.top.1 } else { 0 };
        let rc = self.row_count(res, src, i, tr.w);
        (i, row.min(rc - 1))
    }

    /// Rangées visibles, de haut en bas.
    pub fn visible_rows(&self, res: &Res, rect: Rect, src: &dyn ChatSource) -> Vec<VisRow> {
        let tr = self.text_rect(rect);
        let lh = self.line_h(res);
        let mut out = Vec::new();
        if src.is_empty() || tr.h <= 0 {
            return out;
        }
        let (mut i, mut row) = self.top_index(res, src, rect);
        let mut y = tr.y;
        if self.stick {
            // Collée en bas : la dernière rangée touche le bas de la vue ; la rangée du haut peut n'être
            // visible qu'en partie.
            let rows = ((tr.h + lh - 1) / lh) as usize;
            let (bi, br, fits) = self.bottom_anchor(res, src, tr.w, rows);
            if fits {
                let total: usize = (0..src.len()).map(|k| self.row_count(res, src, k, tr.w)).sum();
                y = tr.bottom() - total as i32 * lh;
                (i, row) = (0, 0);
            } else {
                y = tr.bottom() - rows as i32 * lh;
                (i, row) = (bi, br);
            }
        }
        while i < src.len() && y < tr.bottom() {
            let rc = self.row_count(res, src, i, tr.w);
            let seq = src.line(i).seq;
            while row < rc && y < tr.bottom() {
                if y + lh > tr.y {
                    out.push(VisRow { index: i, seq, row, y });
                }
                y += lh;
                row += 1;
            }
            i += 1;
            row = 0;
        }
        out
    }

    /// Fait défiler de `delta` rangées (négatif = vers le haut).
    pub fn scroll_rows(&mut self, delta: i32, res: &Res, rect: Rect, src: &dyn ChatSource) {
        if src.is_empty() {
            return;
        }
        let tr = self.text_rect(rect);
        let rows = self.view_rows(res, rect);
        let (mut i, mut row) = self.top_index(res, src, rect);
        let (bi, br, fits) = self.bottom_anchor(res, src, tr.w, rows);
        if fits {
            self.stick = true;
            return;
        }
        if delta < 0 {
            let mut d = -delta as usize;
            while d > 0 {
                if row > 0 {
                    row -= 1;
                } else if i > 0 {
                    i -= 1;
                    row = self.row_count(res, src, i, tr.w) - 1;
                } else {
                    break;
                }
                d -= 1;
            }
        } else {
            let mut d = delta as usize;
            while d > 0 && (i, row) < (bi, br) {
                let rc = self.row_count(res, src, i, tr.w);
                if row + 1 < rc {
                    row += 1;
                } else {
                    i += 1;
                    row = 0;
                }
                d -= 1;
            }
        }
        if (i, row) >= (bi, br) {
            self.stick = true;
            self.unseen.set(0);
        } else {
            self.stick = false;
            self.top = (src.line(i).seq, row);
        }
    }

    /// Met à jour le compte des lignes arrivées sous la vue.
    fn sync(&self, src: &dyn ChatSource) {
        if src.version() == self.last_version.get() {
            return;
        }
        self.last_version.set(src.version());
        let last = (!src.is_empty()).then(|| src.line(src.len() - 1).seq);
        if self.stick {
            self.unseen.set(0);
        } else if let (Some(seen), Some(_)) = (self.last_seen.get(), last) {
            let from = lower_bound(src, seen + 1);
            let newer = src.len() - from;
            self.unseen.set(self.unseen.get() + newer);
        }
        self.last_seen.set(last);
    }

    fn sync_vbar(&mut self, res: &Res, rect: Rect, src: &dyn ChatSource) {
        let vis = self.visible_rows(res, rect, src);
        let page = vis.iter().map(|r| r.index).collect::<std::collections::BTreeSet<_>>().len().max(1);
        self.vbar.set_range(0, src.len() as i32, page as i32);
        self.vbar.pos = if self.stick { self.vbar.max_pos() } else { vis.first().map_or(0, |r| r.index as i32) };
    }

    /// Position dans le texte sous un point (bornée à la vue).
    pub fn pos_at(&self, res: &Res, rect: Rect, src: &dyn ChatSource, p: Point) -> Option<TextPos> {
        let vis = self.visible_rows(res, rect, src);
        let tr = self.text_rect(rect);
        let lh = self.line_h(res);
        let first = vis.first()?;
        let last = vis.last()?;
        let vr = if p.y < first.y {
            first
        } else if p.y >= last.y + lh {
            last
        } else {
            vis.iter().find(|r| p.y >= r.y && p.y < r.y + lh)?
        };
        let byte = self.with_line(res, src, vr.index, tr.w, |l| {
            let row = l.rows[vr.row.min(l.rows.len() - 1)];
            let x = (p.x - tr.x) as f32 - row.indent + row.x0;
            let gl = &l.shaped.glyphs[row.start..row.end];
            if p.y < first.y {
                return row.byte_range(&l.shaped).start;
            }
            if p.y >= last.y + lh {
                return row.byte_range(&l.shaped).end;
            }
            for g in gl {
                if x < g.right() {
                    return if x < g.x + g.w / 2.0 { g.start as usize } else { g.end as usize };
                }
            }
            if row.end < l.shaped.glyphs.len() {
                // Fin d'une rangée coupée : avant le premier glyphe de la suivante.
                l.shaped.glyphs[row.end - 1].end as usize
            } else {
                l.shaped.text.len()
            }
        });
        Some(TextPos { seq: vr.seq, byte })
    }

    /// Mot (ou lien) sous un point.
    pub fn word_at(&self, res: &Res, rect: Rect, src: &dyn ChatSource, p: Point) -> Option<(String, WordKind, u64, Range<usize>)> {
        let vis = self.visible_rows(res, rect, src);
        let lh = self.line_h(res);
        let tr = self.text_rect(rect);
        let vr = *vis.iter().find(|r| p.y >= r.y && p.y < r.y + lh)?;
        let (text, links, byte) = self.with_line(res, src, vr.index, tr.w, |l| {
            let row = l.rows[vr.row.min(l.rows.len() - 1)];
            let x = (p.x - tr.x) as f32 - row.indent + row.x0;
            let g = l.shaped.glyphs[row.start..row.end].iter().find(|g| x >= g.x && x < g.right()).copied();
            (l.shaped.text.clone(), l.links.clone(), g.map(|g| g.start as usize))
        });
        let byte = byte?;
        if let Some(link) = links.iter().find(|k| k.range.contains(&byte)) {
            return Some((link.target.clone(), link.kind, vr.seq, link.range.clone()));
        }
        let b = text.as_bytes();
        if b.get(byte).is_none_or(|c| c.is_ascii_whitespace()) {
            return None;
        }
        let mut s = byte;
        while s > 0 && !b[s - 1].is_ascii_whitespace() {
            s -= 1;
        }
        let mut e = byte;
        while e < b.len() && !b[e].is_ascii_whitespace() {
            e += 1;
        }
        let raw = &text[s..e];
        let trimmed = raw.trim_matches(|c: char| "<>()[]{}\"',.:;!?".contains(c));
        let trimmed = trimmed.trim_start_matches(['@', '+', '%', '&', '~']);
        if trimmed.is_empty() {
            return None;
        }
        let kind = if src.is_nick(trimmed) { WordKind::Nick } else { WordKind::Word };
        let off = raw.find(trimmed).unwrap_or(0);
        Some((trimmed.to_string(), kind, vr.seq, s + off..s + off + trimmed.len()))
    }

    /// Cherche le texte de [`ChatView::set_search`] vers les lignes plus récentes (`forward`) ou plus
    /// anciennes, à partir de l'occurrence courante (ou du bas). Fait défiler jusqu'à l'occurrence.
    pub fn find(&mut self, forward: bool, res: &Res, rect: Rect, src: &dyn ChatSource) -> bool {
        let Some(needle) = self.search.clone() else { return false };
        let n = src.len();
        if n == 0 {
            return false;
        }
        let (start_i, start_b) = match &self.search_hit {
            Some((seq, r)) => (lower_bound(src, *seq).min(n - 1), if forward { r.end } else { r.start }),
            None if forward => (0, 0),
            None => (n - 1, usize::MAX),
        };
        for k in 0..=n {
            let i = if forward { start_i + k } else { start_i.wrapping_sub(k) };
            if i >= n {
                break;
            }
            let l = src.line(i);
            let plain = format::strip(l.text).to_lowercase();
            let hit = if k == 0 {
                if forward {
                    plain.get(start_b.min(plain.len())..).and_then(|s| s.find(&needle)).map(|p| p + start_b.min(plain.len()))
                } else {
                    plain.get(..start_b.min(plain.len())).and_then(|s| s.rfind(&needle))
                }
            } else if forward {
                plain.find(&needle)
            } else {
                plain.rfind(&needle)
            };
            if let Some(p) = hit {
                self.search_hit = Some((l.seq, p..p + needle.len()));
                self.reveal(i, p, res, rect, src);
                return true;
            }
        }
        false
    }

    /// Fait défiler pour montrer l'octet `byte` de la ligne `i` (vers le milieu de la vue).
    fn reveal(&mut self, i: usize, byte: usize, res: &Res, rect: Rect, src: &dyn ChatSource) {
        let vis = self.visible_rows(res, rect, src);
        let tr = self.text_rect(rect);
        let row = self.with_line(res, src, i, tr.w, |l| {
            l.rows.iter().position(|r| r.byte_range(&l.shaped).end > byte).unwrap_or(l.rows.len() - 1)
        });
        let seq = src.line(i).seq;
        if vis.iter().any(|r| r.seq == seq && r.row == row) {
            return;
        }
        self.stick = false;
        self.top = (seq, row);
        let half = (self.view_rows(res, rect) / 2) as i32;
        self.scroll_rows(-half, res, rect, src);
    }

    pub fn paint(&self, canvas: &mut Canvas, res: &Res, rect: Rect, src: &dyn ChatSource) {
        self.sync(src);
        let t = &res.theme;
        let bg = t.chat_background();
        canvas.fill(rect, bg);
        let tr = self.text_rect(rect);
        let lh = self.line_h(res);
        let asc = res.fonts.ascent(FontKind::Chat);
        let vis = self.visible_rows(res, rect, src);
        let sel = self.selection();
        let search = self.search.clone();
        canvas.with_clip(Rect::from_corners(rect.x, tr.y, tr.right(), tr.bottom()), |canvas| {
            for vr in &vis {
                let line = src.line(vr.index);
                let default_fg = if line.highlight { t.color(t.activity_highlight) } else { t.color(line.default_fg) };
                // Occurrences de la recherche dans cette ligne.
                let hits: Vec<Range<usize>> = match &search {
                    Some(n) => {
                        let plain = format::strip(line.text).to_lowercase();
                        plain.match_indices(n.as_str()).map(|(p, s)| p..p + s.len()).collect()
                    }
                    None => Vec::new(),
                };
                let current = self.search_hit.as_ref().filter(|h| h.0 == vr.seq).map(|h| h.1.clone());
                let link_hover = self.hover_link.as_ref().filter(|h| h.0 == vr.seq).map(|h| h.1.clone());
                self.with_line(res, src, vr.index, tr.w, |l| {
                    let row = l.rows[vr.row.min(l.rows.len() - 1)];
                    let gl = &l.shaped.glyphs[row.start..row.end];
                    let ox = tr.x as f32 + row.indent - row.x0;
                    let in_sel = |b: usize| sel.is_some_and(|(a, z)| TextPos { seq: vr.seq, byte: b } >= a && TextPos { seq: vr.seq, byte: b } < z);
                    let colors = |g: &crate::text::Glyph| -> (u32, Option<u32>) {
                        let st = l.styles.get(g.meta as usize).copied().unwrap_or_default();
                        let mut fg = st.fg.map_or(default_fg, |c| t.mirc_rgb(c));
                        let mut bgc = st.bg.map(|c| t.mirc_rgb(c));
                        if st.reverse {
                            let nb = fg;
                            fg = bgc.unwrap_or(bg);
                            bgc = Some(nb);
                        }
                        let b = g.start as usize;
                        if in_sel(b) || current.as_ref().is_some_and(|r| r.contains(&b)) {
                            return (t.selection_text, Some(t.selection));
                        }
                        if hits.iter().any(|r| r.contains(&b)) {
                            return (t.search_fg, Some(t.search_bg));
                        }
                        (fg, bgc)
                    };
                    // Fonds.
                    for g in gl {
                        if let (_, Some(c)) = colors(g) {
                            let x0 = (ox + g.x) as i32;
                            canvas.fill(Rect::new(x0, vr.y, g.w as i32, lh), c);
                        }
                    }
                    // Glyphes.
                    let base = (vr.y + asc) as f32;
                    res.fonts.draw_glyphs(canvas, gl, ox, base, |g| Some(colors(g).0));
                    // Soulignés, barrés, lien survolé.
                    for g in gl {
                        let st = l.styles.get(g.meta as usize).copied().unwrap_or_default();
                        let hovered = link_hover.as_ref().is_some_and(|r| r.contains(&(g.start as usize)));
                        if st.underline || st.strike || hovered {
                            let (fg, _) = colors(g);
                            let x0 = (ox + g.x) as i32;
                            if st.underline || hovered {
                                canvas.hline(x0, vr.y + asc + 1, g.w as i32, fg);
                            }
                            if st.strike {
                                canvas.hline(x0, vr.y + asc - 4, g.w as i32, fg);
                            }
                        }
                    }
                });
                if self.marker == Some(vr.seq) && vr.row == 0 {
                    canvas.hline(rect.x, vr.y, tr.right() - rect.x, t.marker_line);
                }
            }
        });
        // Barre de défilement.
        let mut vb = self.vbar.clone();
        let page = vis.iter().map(|r| r.index).collect::<std::collections::BTreeSet<_>>().len().max(1);
        vb.set_range(0, src.len() as i32, page as i32);
        vb.pos = if self.stick { vb.max_pos() } else { vis.first().map_or(0, |r| r.index as i32) };
        vb.paint(canvas, res, self.vbar_rect(rect));
        // Indicateur de nouvelles lignes.
        if !self.stick && self.unseen.get() > 0 {
            let r = self.indicator_rect(res, rect);
            canvas.fill(r, t.tooltip);
            canvas.frame(r, t.dark);
            let s = self.new_lines_text.replace("{n}", &self.unseen.get().to_string());
            res.fonts.draw_text(canvas, r.x + 4, r.y + 2, &s, TextStyle::UI, t.tooltip_text);
        }
        let mut cache = self.cache.borrow_mut();
        cache.frame += 1;
        let frame = cache.frame;
        if cache.lines.len() > 4 * vis.len() + 600 {
            cache.lines.retain(|_, l| l.used + 2 >= frame);
        }
    }

    fn indicator_rect(&self, res: &Res, rect: Rect) -> Rect {
        let tr = self.text_rect(rect);
        let s = self.new_lines_text.replace("{n}", &self.unseen.get().to_string());
        let w = res.fonts.measure(&s, TextStyle::UI) + 8;
        let h = res.ui_line() + 4;
        Rect::new(tr.right() - w - 2, tr.bottom() - h - 2, w, h)
    }

    fn update_selection(&mut self, res: &Res, rect: Rect, src: &dyn ChatSource, p: Point) {
        if let (Some((_, Some(anchor))), Some(cur)) = (self.press, self.pos_at(res, rect, src, p)) {
            self.sel = Some((anchor, cur));
        }
    }

    pub fn event(&mut self, input: &Input, rect: Rect, res: &Res, platform: &mut dyn Platform, src: &dyn ChatSource) -> Option<ChatMsg> {
        self.sync(src);
        let tr = self.text_rect(rect);
        let vr = self.vbar_rect(rect);
        // Barre de défilement.
        self.sync_vbar(res, rect, src);
        let grabbed = self.vbar.pressed_part().is_some() || matches!(input, Input::MouseDown { pos, .. } if vr.contains(*pos));
        if grabbed || matches!(input, Input::Tick) && self.vbar.pressed_part().is_some() {
            if let Some(ScrollMsg::Changed(p)) = self.vbar.event(input, vr, res, platform) {
                if p >= self.vbar.max_pos() {
                    self.scroll_to_bottom();
                } else if (p as usize) < src.len() {
                    self.stick = false;
                    self.top = (src.line(p as usize).seq, 0);
                }
                platform.request_repaint();
            }
            return None;
        }
        match input {
            Input::Wheel { pos, dy, .. } if rect.contains(*pos) => {
                let n = (-dy * 3.0).round() as i32;
                if n != 0 {
                    self.scroll_rows(n, res, rect, src);
                    platform.request_repaint();
                }
            }
            Input::Key { key, mods, .. } => {
                let page = self.view_rows(res, rect) as i32 - 1;
                match key {
                    Key::PageUp => self.scroll_rows(-page.max(1), res, rect, src),
                    Key::PageDown => self.scroll_rows(page.max(1), res, rect, src),
                    Key::Home if mods.command() => self.scroll_to_top(src),
                    Key::End if mods.command() => self.scroll_to_bottom(),
                    Key::Up if mods.command() => self.scroll_rows(-1, res, rect, src),
                    Key::Down if mods.command() => self.scroll_rows(1, res, rect, src),
                    Key::Char('c') if mods.command() => {
                        if let Some(s) = self.selected_text(src) {
                            platform.clipboard_set(&s);
                        }
                        return None;
                    }
                    _ => return None,
                }
                platform.request_repaint();
            }
            Input::MouseMove { pos } => {
                self.mouse = *pos;
                if self.press.is_some() {
                    let (start, _) = self.press.unwrap();
                    if !self.selecting && ((pos.x - start.x).abs() > 2 || (pos.y - start.y).abs() > 2) {
                        self.selecting = true;
                    }
                    if self.selecting {
                        self.autoscroll = if pos.y < tr.y { -1 } else if pos.y >= tr.bottom() { 1 } else { 0 };
                        if self.autoscroll != 0 {
                            platform.schedule_tick(platform.now_ms() + 50);
                        }
                        self.update_selection(res, rect, src, *pos);
                        platform.request_repaint();
                    }
                    return None;
                }
                // Survol des liens.
                let link = if tr.contains(*pos) {
                    self.word_at(res, rect, src, *pos).filter(|w| matches!(w.1, WordKind::Url | WordKind::Channel)).map(|w| (w.2, w.3))
                } else {
                    None
                };
                if link.is_some() {
                    platform.set_cursor(Cursor::Hand);
                } else if self.hover_link.is_some() {
                    platform.set_cursor(Cursor::Default);
                }
                if link != self.hover_link {
                    self.hover_link = link;
                    platform.request_repaint();
                }
            }
            Input::MouseLeave => {
                if self.hover_link.take().is_some() {
                    platform.request_repaint();
                }
            }
            Input::Tick if self.selecting && self.autoscroll != 0 => {
                self.scroll_rows(self.autoscroll, res, rect, src);
                let p = self.mouse;
                self.update_selection(res, rect, src, p);
                platform.schedule_tick(platform.now_ms() + 50);
                platform.request_repaint();
            }
            Input::MouseDown { pos, button, clicks, .. } if rect.contains(*pos) => {
                if !self.stick && self.unseen.get() > 0 && self.indicator_rect(res, rect).contains(*pos) {
                    self.scroll_to_bottom();
                    platform.request_repaint();
                    return None;
                }
                match button {
                    MouseButton::Left => {
                        if *clicks >= 2 {
                            self.press = None;
                            self.selecting = false;
                            let (w, k, _, _) = self.word_at(res, rect, src, *pos)?;
                            return Some(ChatMsg::DoubleClick(w, k));
                        }
                        let anchor = self.pos_at(res, rect, src, *pos);
                        self.press = Some((*pos, anchor));
                        self.selecting = false;
                        if self.sel.take().is_some() {
                            platform.request_repaint();
                        }
                    }
                    MouseButton::Right => {
                        let word = self.word_at(res, rect, src, *pos).map(|w| (w.0, w.1));
                        return Some(ChatMsg::RightClick { pos: *pos, word });
                    }
                    _ => {}
                }
            }
            Input::MouseUp { pos, button: MouseButton::Left, .. } if self.press.is_some() => {
                let selecting = self.selecting;
                self.selecting = false;
                self.autoscroll = 0;
                let (start, _) = self.press.take().unwrap();
                if selecting {
                    self.update_selection(res, rect, src, *pos);
                    let text = self.selected_text(src)?;
                    if self.copy_on_select {
                        platform.clipboard_set(&text);
                        self.sel = None;
                    }
                    platform.request_repaint();
                    return Some(ChatMsg::Selected(text));
                }
                if (pos.x - start.x).abs() <= 2 && (pos.y - start.y).abs() <= 2 {
                    if let Some((w, kind, _, _)) = self.word_at(res, rect, src, *pos) {
                        match kind {
                            WordKind::Url => return Some(ChatMsg::OpenUrl(w)),
                            WordKind::Channel => return Some(ChatMsg::JoinChannel(w)),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }
}

/// Dessine une ligne de texte avec codes de mise en forme de mIRC (sujet d'un canal, barre de titre d'Android…),
/// sans retour à la ligne, centrée verticalement dans `rect` et coupée à sa largeur. Renvoie la largeur
/// dessinée.
pub fn draw_mirc_line(canvas: &mut Canvas, res: &Res, rect: Rect, text: &str, kind: FontKind, default_fg: u32) -> i32 {
    let t = &res.theme;
    let (x, max_w) = (rect.x, rect.w);
    let y = rect.y + ((rect.h - res.fonts.line_height(kind)) / 2).max(0);
    let base = TextStyle { kind, bold: false, italic: false };
    let spans = format::spans(text);
    let mut plain = String::with_capacity(text.len());
    let mut runs = Vec::with_capacity(spans.len());
    for (k, sp) in spans.iter().enumerate() {
        let s = plain.len();
        plain.push_str(&sp.text);
        runs.push((s..plain.len(), base.bold(sp.style.bold).italic(sp.style.italic), k as u32));
    }
    let shaped = res.fonts.shape_rich(&plain, base, &runs);
    let lh = res.fonts.line_height(kind);
    let asc = res.fonts.ascent(kind);
    let colors = |g: &crate::text::Glyph| -> (u32, Option<u32>, Style) {
        let st = spans.get(g.meta as usize).map(|s| s.style).unwrap_or_default();
        let mut fg = st.fg.map_or(default_fg, |c| t.mirc_rgb(c));
        let mut bg = st.bg.map(|c| t.mirc_rgb(c));
        if st.reverse {
            let nb = fg;
            fg = bg.unwrap_or(t.chat_background());
            bg = Some(nb);
        }
        (fg, bg, st)
    };
    canvas.with_clip(Rect::new(x, y, max_w, lh), |c| {
        for g in &shaped.glyphs {
            let (fg, bg, st) = colors(g);
            let gx = x + g.x as i32;
            if let Some(bg) = bg {
                c.fill(Rect::new(gx, y, g.w as i32, lh), bg);
            }
            if st.underline {
                c.hline(gx, y + asc + 1, g.w as i32, fg);
            }
            if st.strike {
                c.hline(gx, y + asc - 4, g.w as i32, fg);
            }
        }
        res.fonts.draw_glyphs(c, &shaped.glyphs, x as f32, (y + asc) as f32, |g| Some(colors(g).0));
    });
    shaped.width_px().min(max_w)
}

/// Met en page une ligne (sans coupure).
fn layout_line(res: &Res, text: &str, indent: IndentMode) -> LaidLine {
    let spans = format::spans(text);
    let mut plain = String::with_capacity(text.len());
    let mut runs = Vec::with_capacity(spans.len());
    let mut styles = Vec::with_capacity(spans.len());
    for (k, sp) in spans.iter().enumerate() {
        let s = plain.len();
        plain.push_str(&sp.text);
        let st = TextStyle::CHAT.bold(sp.style.bold).italic(sp.style.italic);
        runs.push((s..plain.len(), st, k as u32));
        styles.push(sp.style);
    }
    let shaped = res.fonts.shape_rich(&plain, TextStyle::CHAT, &runs);
    let hang = match indent {
        IndentMode::None => 0.0,
        IndentMode::Fixed(px) => px as f32,
        IndentMode::FirstWord => {
            let b = shaped.text.as_bytes();
            let mut seen_word = false;
            let mut hang = 0.0;
            for g in &shaped.glyphs {
                let sp = b.get(g.start as usize) == Some(&b' ');
                if !sp {
                    if seen_word && hang > 0.0 {
                        break;
                    }
                    seen_word = true;
                } else if seen_word {
                    hang = g.right();
                }
            }
            hang
        }
    };
    let links = find_links(&shaped.text);
    LaidLine { shaped, styles, links, hang, rows: Vec::new(), rows_width: -1, used: 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Mods, TestPlatform};

    fn src(n: usize) -> VecSource {
        let mut s = VecSource::new();
        for i in 0..n {
            s.push(&format!("[12:{:02}] <nick{}> message numéro {i}", i % 60, i % 7));
        }
        s
    }

    #[test]
    fn links() {
        let l = find_links("va sur (http://alpirc.org/a_(b)), www.x.fr. ou #lyon,#grenoble! #");
        assert_eq!(l.len(), 4, "{l:?}");
        assert_eq!(l[0].target, "http://alpirc.org/a_(b)");
        assert_eq!(l[1].target, "http://www.x.fr");
        assert_eq!(l[2].target, "#lyon");
        assert_eq!(l[2].kind, WordKind::Channel);
        assert_eq!(l[3].target, "#grenoble");
    }

    #[test]
    fn sticky_bottom_and_append() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let rect = Rect::new(0, 0, 600, 16 * 10);
        let mut s = src(50);
        let mut v = ChatView::new();
        let vis = v.visible_rows(&res, rect, &s);
        assert_eq!(vis.len(), 10);
        assert_eq!(vis.last().unwrap().index, 49);
        assert!(v.is_at_bottom());
        // Ajouter une ligne en bas : la vue suit.
        s.push("nouvelle");
        let vis = v.visible_rows(&res, rect, &s);
        assert_eq!(vis.last().unwrap().index, 50);
        // Remonter : les nouvelles lignes ne bougent plus la vue.
        v.event(&Input::Wheel { pos: Point::new(10, 10), dx: 0.0, dy: 1.0, mods: Mods::NONE }, rect, &res, &mut p, &s);
        assert!(!v.is_at_bottom());
        let before = v.visible_rows(&res, rect, &s)[0];
        assert_eq!(before.index, 38);
        s.push("encore");
        s.push("et encore");
        let after = v.visible_rows(&res, rect, &s)[0];
        assert_eq!(after.seq, before.seq);
        v.paint(&mut Canvas::new(600, 160, 1.0), &res, rect, &s);
        assert_eq!(v.unseen(), 2);
        // Retirer des vieilles lignes en tête : la vue reste sur les mêmes lignes.
        s.drop_front(20);
        let after = v.visible_rows(&res, rect, &s)[0];
        assert_eq!(after.seq, before.seq);
        assert_eq!(after.index, 18);
        // Ctrl+Fin : recollé.
        v.event(&Input::key(Key::End, Mods::CTRL), rect, &res, &mut p, &s);
        assert!(v.is_at_bottom());
        // Redescendre à la molette jusqu'en bas recolle aussi.
        v.event(&Input::key(Key::PageUp, Mods::NONE), rect, &res, &mut p, &s);
        assert!(!v.is_at_bottom());
        for _ in 0..5 {
            v.event(&Input::Wheel { pos: Point::new(10, 10), dx: 0.0, dy: -1.0, mods: Mods::NONE }, rect, &res, &mut p, &s);
        }
        assert!(v.is_at_bottom());
    }

    #[test]
    fn dropped_anchor_line_falls_back_to_first() {
        let res = crate::test_res();
        let rect = Rect::new(0, 0, 600, 160);
        let mut s = src(50);
        let mut v = ChatView::new();
        v.scroll_to_top(&s);
        assert_eq!(v.visible_rows(&res, rect, &s)[0].index, 0);
        s.drop_front(5);
        let vis = v.visible_rows(&res, rect, &s);
        assert_eq!(vis[0].index, 0);
        assert_eq!(vis[0].seq, 5);
    }

    #[test]
    fn wrapping_with_hanging_indent() {
        let res = crate::test_res();
        let rect = Rect::new(0, 0, 200 + scrollbar::SIZE + 5, 160);
        let mut s = VecSource::new();
        s.push("<Guiwald> un texte assez long pour être coupé sur plusieurs rangées dans la vue");
        let v = ChatView::new();
        let vis = v.visible_rows(&res, rect, &s);
        assert!(vis.len() >= 3);
        let tr = v.text_rect(rect);
        v.with_line(&res, &s, 0, tr.w, |l| {
            assert_eq!(l.hang, 80.0, "retrait = « <Guiwald> »");
            assert_eq!(l.rows[1].indent, 80.0);
        });
    }

    #[test]
    fn drag_selection_and_copy() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let rect = Rect::new(0, 0, 600, 160);
        let mut s = VecSource::new();
        s.push("\x02<a>\x02 premiere \x034ligne");
        s.push("<b> deuxieme ligne");
        let mut v = ChatView::new();
        v.copy_on_select = false;
        let vis = v.visible_rows(&res, rect, &s);
        let tr = v.text_rect(rect);
        let y0 = vis[0].y + 4;
        let y1 = vis[1].y + 4;
        v.event(&Input::click(tr.x + 4 * 8 + 1, y0), rect, &res, &mut p, &s);
        v.event(&Input::moved(tr.x + 8 * 8 + 1, y1), rect, &res, &mut p, &s);
        let m = v.event(&Input::release(tr.x + 8 * 8 + 1, y1), rect, &res, &mut p, &s);
        assert_eq!(m, Some(ChatMsg::Selected("premiere ligne\n<b> deux".into())));
        assert_eq!(v.selected_text(&s).as_deref(), Some("premiere ligne\n<b> deux"));
        v.copy_on_select = true;
        v.event(&Input::click(tr.x + 1, y0), rect, &res, &mut p, &s);
        v.event(&Input::moved(tr.x + 3 * 8 + 1, y0), rect, &res, &mut p, &s);
        v.event(&Input::release(tr.x + 3 * 8 + 1, y0), rect, &res, &mut p, &s);
        assert_eq!(p.clipboard.as_deref(), Some("<a>"));
        assert!(v.selection().is_none());
    }

    #[test]
    fn drag_past_edge_autoscrolls() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let rect = Rect::new(0, 0, 600, 160);
        let s = src(100);
        let mut v = ChatView::new();
        v.copy_on_select = false;
        v.scroll_to_top(&s);
        let tr = v.text_rect(rect);
        v.event(&Input::click(tr.x + 1, tr.y + 4), rect, &res, &mut p, &s);
        v.event(&Input::moved(tr.x + 40, tr.bottom() + 30), rect, &res, &mut p, &s);
        assert!(p.tick_at.is_some(), "un battement est demandé");
        for i in 0..20 {
            p.now = 100 * (i + 1);
            v.event(&Input::Tick, rect, &res, &mut p, &s);
        }
        let top = v.visible_rows(&res, rect, &s)[0].index;
        assert!(top >= 20, "la vue a défilé : {top}");
        let m = v.event(&Input::release(tr.x + 40, tr.bottom() + 30), rect, &res, &mut p, &s);
        let Some(ChatMsg::Selected(text)) = m else { panic!("{m:?}") };
        assert!(text.lines().count() > 25);
    }

    #[test]
    fn clicks_on_words_links_and_nicks() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let rect = Rect::new(0, 0, 600, 160);
        let mut s = VecSource::new();
        s.nicks = vec!["Elisa".into()];
        s.push("<@Elisa> voir http://alpirc.org et #lyon");
        let mut v = ChatView::new();
        let vis = v.visible_rows(&res, rect, &s);
        let tr = v.text_rect(rect);
        let y = vis[0].y + 4;
        let at = |col: i32| tr.x + col * 8 + 2;
        let dbl = |x| Input::MouseDown { pos: Point::new(x, y), button: MouseButton::Left, mods: Mods::NONE, clicks: 2 };
        assert_eq!(v.event(&dbl(at(3)), rect, &res, &mut p, &s), Some(ChatMsg::DoubleClick("Elisa".into(), WordKind::Nick)));
        assert_eq!(v.event(&dbl(at(10)), rect, &res, &mut p, &s), Some(ChatMsg::DoubleClick("voir".into(), WordKind::Word)));
        v.event(&Input::release(at(10), y), rect, &res, &mut p, &s);
        v.event(&Input::moved(at(16), y), rect, &res, &mut p, &s);
        assert_eq!(p.cursor, Cursor::Hand);
        v.event(&Input::click(at(16), y), rect, &res, &mut p, &s);
        assert_eq!(v.event(&Input::release(at(16), y), rect, &res, &mut p, &s), Some(ChatMsg::OpenUrl("http://alpirc.org".into())));
        v.event(&Input::click(at(37), y), rect, &res, &mut p, &s);
        assert_eq!(v.event(&Input::release(at(37), y), rect, &res, &mut p, &s), Some(ChatMsg::JoinChannel("#lyon".into())));
        let rc = Input::MouseDown { pos: Point::new(at(3), y), button: MouseButton::Right, mods: Mods::NONE, clicks: 1 };
        assert!(matches!(v.event(&rc, rect, &res, &mut p, &s), Some(ChatMsg::RightClick { word: Some((w, WordKind::Nick)), .. }) if w == "Elisa"));
    }

    #[test]
    fn search_scrolls_to_hit() {
        let res = crate::test_res();
        let rect = Rect::new(0, 0, 600, 160);
        let mut s = src(200);
        s.lines[30].1 = "<x> le MOT magique".into();
        s.lines[150].1 = "<y> encore le mot".into();
        let mut v = ChatView::new();
        v.set_search(Some("mot"));
        assert!(v.find(false, &res, rect, &s));
        assert_eq!(v.search_hit().unwrap().0, 150);
        assert!(v.find(false, &res, rect, &s));
        assert_eq!(v.search_hit().unwrap().0, 30);
        let vis = v.visible_rows(&res, rect, &s);
        assert!(vis.iter().any(|r| r.seq == 30));
        assert!(!v.is_at_bottom());
        assert!(v.find(true, &res, rect, &s));
        assert_eq!(v.search_hit().unwrap().0, 150);
    }

    #[test]
    fn renders_codes_marker_and_indicator() {
        let res = crate::test_res();
        let mut s = VecSource::new();
        for i in 0..30 {
            s.push(&format!("\x02gras\x02 \x0304,08rouge sur jaune\x03 \x1Fsouligné\x1F \x16inverse\x16 {i}"));
        }
        let mut v = ChatView::new();
        v.marker = Some(25);
        let rect = Rect::new(0, 0, 400, 160);
        let mut c = Canvas::new(400, 160, 1.0);
        v.paint(&mut c, &res, rect, &s);
        assert!(c.pixels.contains(&0xFF0000) && c.pixels.contains(&0xFFFF00), "rouge sur jaune");
        assert!(c.pixels.contains(&res.theme.marker_line));
        v.scroll_to_top(&s);
        s.push("nouvelle");
        v.paint(&mut c, &res, rect, &s);
        v.paint(&mut c, &res, rect, &s);
        assert!(c.pixels.contains(&res.theme.tooltip), "indicateur de nouvelles lignes");
    }

    #[test]
    fn single_mirc_line() {
        let res = crate::test_res();
        let mut c = Canvas::new(200, 20, 1.0);
        c.clear(0xFFFFFF);
        let w = draw_mirc_line(&mut c, &res, Rect::new(0, 0, 200, 20), "\x02a\x02 \x034,8rouge\x03", FontKind::Chat, 0);
        assert_eq!(w, 8 * 7);
        assert!(c.pixels.contains(&0xFF0000) && c.pixels.contains(&0xFFFF00));
        assert_eq!(draw_mirc_line(&mut c, &res, Rect::new(0, 0, 20, 20), "très long sujet", FontKind::Ui, 0), 20);
    }

    #[test]
    fn perf_smoke_5000_lines() {
        let res = crate::test_res();
        let mut s = VecSource::new();
        for i in 0..5000 {
            s.push(&format!(
                "[{:02}:{:02}] <\x0312nick{}\x03> un message assez long pour remplir la ligne, avec http://example.org/{i} et du \x02gras\x02 et des \x034couleurs\x03, numéro {i}",
                i / 60 % 24,
                i % 60,
                i % 13
            ));
        }
        let mut p = TestPlatform::new();
        let rect = Rect::new(0, 0, 1024, 768);
        let mut c = Canvas::new(2048, 1536, 2.0);
        let mut v = ChatView::new();
        let t0 = std::time::Instant::now();
        for f in 0..100 {
            if f % 10 == 0 {
                v.event(&Input::key(Key::PageUp, Mods::NONE), rect, &res, &mut p, &s);
            }
            v.paint(&mut c, &res, rect, &s);
        }
        let dt = t0.elapsed();
        eprintln!("100 images 2048x1536 : {dt:?}");
        if cfg!(debug_assertions) {
            assert!(dt.as_secs() < 60);
        } else {
            // En release : moins de 10 ms par image (mesuré : ~2 ms).
            assert!(dt.as_millis() < 1000, "{dt:?}");
        }
    }
}

