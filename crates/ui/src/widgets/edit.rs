//! Modèle d'édition de texte partagé par le champ d'une ligne, la zone multiligne, la ligne de saisie et la
//! liste déroulante modifiable : texte, curseur, sélection, annulation, presse-papiers, déplacements par
//! caractère (graphème) et par mot.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::input::{Key, Mods, Platform};

/// Résultat d'une opération d'édition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditResult {
    /// Touche non traitée.
    Ignored,
    /// Curseur ou sélection déplacés.
    Moved,
    /// Texte modifié.
    Changed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    text: String,
    caret: usize,
    anchor: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Coalesce {
    None,
    Typing,
    Deleting,
}

/// Texte modifiable, curseur et sélection (positions en octets, toujours sur une frontière de caractère).
#[derive(Clone, Debug)]
pub struct EditCore {
    text: String,
    caret: usize,
    anchor: usize,
    /// Nombre maximal de caractères (style `limit N` des dialogues).
    pub max_chars: Option<usize>,
    /// Accepter les retours à la ligne.
    pub multiline: bool,
    /// Lecture seule : sélection et copie possibles, pas de modification.
    pub read_only: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    coalesce: Coalesce,
    revision: u64,
}

impl Default for EditCore {
    fn default() -> EditCore {
        EditCore::new("")
    }
}

/// Classe d'un caractère pour les mots : espace, lettre/chiffre, ponctuation.
fn class(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

impl EditCore {
    pub fn new(text: &str) -> EditCore {
        EditCore {
            text: text.to_string(),
            caret: text.len(),
            anchor: text.len(),
            max_chars: None,
            multiline: false,
            read_only: false,
            undo: Vec::new(),
            redo: Vec::new(),
            coalesce: Coalesce::None,
            revision: 0,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Compteur incrémenté à chaque modification du texte (pour invalider les caches de l'appelant).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Remplace tout le texte (efface l'historique d'annulation), curseur à la fin.
    pub fn set_text(&mut self, text: &str) {
        self.text = self.filter(text);
        self.revision += 1;
        self.caret = self.text.len();
        self.anchor = self.caret;
        self.undo.clear();
        self.redo.clear();
        self.coalesce = Coalesce::None;
    }

    /// Remplace tout le texte comme une modification annulable.
    pub fn replace_all_text(&mut self, text: &str) {
        self.push_undo(Coalesce::None);
        self.text = self.filter(text);
        self.revision += 1;
        self.caret = self.text.len();
        self.anchor = self.caret;
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    /// Place le curseur ; `extend` étend la sélection depuis l'ancre.
    pub fn set_caret(&mut self, pos: usize, extend: bool) {
        let pos = self.floor_boundary(pos.min(self.text.len()));
        self.caret = pos;
        if !extend {
            self.anchor = pos;
        }
        self.coalesce = Coalesce::None;
    }

    /// Sélectionne une plage (l'ancre au début, le curseur à la fin).
    pub fn select(&mut self, r: Range<usize>) {
        self.anchor = self.floor_boundary(r.start.min(self.text.len()));
        self.caret = self.floor_boundary(r.end.min(self.text.len()));
        self.coalesce = Coalesce::None;
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.len();
    }

    /// Sélection, dans l'ordre.
    pub fn selection(&self) -> Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }

    pub fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selection()]
    }

    fn floor_boundary(&self, mut i: usize) -> usize {
        while i > 0 && !self.text.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    /// Position du graphème précédent.
    pub fn prev_grapheme(&self, i: usize) -> usize {
        self.text[..i].grapheme_indices(true).next_back().map_or(0, |(p, _)| p)
    }

    /// Position du graphème suivant.
    pub fn next_grapheme(&self, i: usize) -> usize {
        self.text[i..].graphemes(true).next().map_or(i, |g| i + g.len())
    }

    /// Début du mot précédent (comme Ctrl+Gauche sous Windows).
    pub fn prev_word(&self, i: usize) -> usize {
        let mut it = self.text[..i].char_indices().rev().peekable();
        while let Some(&(_, c)) = it.peek() {
            if class(c) != 0 {
                break;
            }
            it.next();
        }
        let Some(&(mut p, c0)) = it.peek() else { return 0 };
        let cl = class(c0);
        for (q, c) in it {
            if class(c) != cl {
                break;
            }
            p = q;
        }
        p
    }

    /// Début du mot suivant (comme Ctrl+Droite sous Windows).
    pub fn next_word(&self, i: usize) -> usize {
        let mut it = self.text[i..].char_indices().peekable();
        let mut p = self.text.len();
        if let Some(&(_, c0)) = it.peek() {
            let cl = class(c0);
            if cl != 0 {
                while let Some(&(_, c)) = it.peek() {
                    if class(c) != cl {
                        break;
                    }
                    it.next();
                }
            }
        }
        while let Some(&(q, c)) = it.peek() {
            if class(c) != 0 || c == '\n' {
                p = i + q;
                break;
            }
            it.next();
        }
        if it.peek().is_none() {
            p = self.text.len();
        }
        p
    }

    /// Mot (suite de caractères de même classe) contenant la position.
    pub fn word_at(&self, i: usize) -> Range<usize> {
        let i = self.floor_boundary(i.min(self.text.len()));
        let c = self.text[i..].chars().next().or_else(|| self.text[..i].chars().next_back());
        let Some(c) = c else { return i..i };
        let cl = class(c);
        let mut s = i;
        for (q, ch) in self.text[..i].char_indices().rev() {
            if class(ch) != cl || ch == '\n' {
                break;
            }
            s = q;
        }
        let mut e = i;
        for (q, ch) in self.text[i..].char_indices() {
            if class(ch) != cl || ch == '\n' {
                break;
            }
            e = i + q + ch.len_utf8();
        }
        s..e.max(s)
    }

    fn filter(&self, s: &str) -> String {
        if self.multiline {
            s.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            s.chars().filter(|c| *c != '\n' && *c != '\r').collect()
        }
    }

    fn push_undo(&mut self, kind: Coalesce) {
        if kind != Coalesce::None && kind == self.coalesce {
            return;
        }
        self.undo.push(Snapshot { text: self.text.clone(), caret: self.caret, anchor: self.anchor });
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.coalesce = kind;
    }

    /// Insère du texte à la place de la sélection (respecte `max_chars`, `multiline`, `read_only`).
    pub fn insert(&mut self, s: &str) -> bool {
        self.insert_kind(s, if s.chars().count() == 1 && !s.contains(' ') { Coalesce::Typing } else { Coalesce::None })
    }

    fn insert_kind(&mut self, s: &str, kind: Coalesce) -> bool {
        if self.read_only {
            return false;
        }
        let mut s = self.filter(s);
        let sel = self.selection();
        if let Some(max) = self.max_chars {
            let remaining = self.text.chars().count() - self.text[sel.clone()].chars().count();
            let room = max.saturating_sub(remaining);
            if s.chars().count() > room {
                s = s.chars().take(room).collect();
            }
        }
        if s.is_empty() && sel.is_empty() {
            return false;
        }
        self.push_undo(kind);
        self.text.replace_range(sel.clone(), &s);
        self.revision += 1;
        self.caret = sel.start + s.len();
        self.anchor = self.caret;
        true
    }

    /// Efface la sélection, ou le caractère (le mot si `word`) avant le curseur.
    pub fn backspace(&mut self, word: bool) -> bool {
        if self.read_only {
            return false;
        }
        if !self.has_selection() {
            if self.caret == 0 {
                return false;
            }
            self.anchor = if word { self.prev_word(self.caret) } else { self.text[..self.caret].chars().next_back().map_or(0, |c| self.caret - c.len_utf8()) };
        }
        self.delete_selection(Coalesce::Deleting)
    }

    /// Efface la sélection, ou le caractère (le mot si `word`) après le curseur.
    pub fn delete(&mut self, word: bool) -> bool {
        if self.read_only {
            return false;
        }
        if !self.has_selection() {
            if self.caret >= self.text.len() {
                return false;
            }
            self.anchor = if word { self.next_word(self.caret) } else { self.next_grapheme(self.caret) };
        }
        self.delete_selection(Coalesce::Deleting)
    }

    fn delete_selection(&mut self, kind: Coalesce) -> bool {
        let sel = self.selection();
        if sel.is_empty() {
            return false;
        }
        self.push_undo(kind);
        self.text.replace_range(sel.clone(), "");
        self.revision += 1;
        self.caret = sel.start;
        self.anchor = sel.start;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else { return false };
        self.redo.push(Snapshot { text: std::mem::take(&mut self.text), caret: self.caret, anchor: self.anchor });
        self.text = s.text;
        self.revision += 1;
        self.caret = s.caret;
        self.anchor = s.anchor;
        self.coalesce = Coalesce::None;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else { return false };
        self.undo.push(Snapshot { text: std::mem::take(&mut self.text), caret: self.caret, anchor: self.anchor });
        self.text = s.text;
        self.revision += 1;
        self.caret = s.caret;
        self.anchor = s.anchor;
        self.coalesce = Coalesce::None;
        true
    }

    pub fn copy(&self, platform: &mut dyn Platform) {
        if self.has_selection() {
            platform.clipboard_set(self.selected_text());
        }
    }

    pub fn cut(&mut self, platform: &mut dyn Platform) -> bool {
        if self.read_only || !self.has_selection() {
            return false;
        }
        platform.clipboard_set(self.selected_text());
        self.delete_selection(Coalesce::None)
    }

    /// Colle le presse-papiers (une seule ligne : seulement la première ligne).
    pub fn paste(&mut self, platform: &mut dyn Platform) -> bool {
        let Some(s) = platform.clipboard_get() else { return false };
        let s = if self.multiline { s } else { s.lines().next().unwrap_or("").to_string() };
        self.insert_kind(&s, Coalesce::None)
    }

    /// Traite les touches d'édition communes : flèches gauche/droite (par mot avec Ctrl ou Alt), Origine/Fin
    /// (du texte entier), effacement, presse-papiers, annulation. Le texte tapé et les touches propres au
    /// widget (Haut/Bas, Entrée, Tab) sont à traiter par l'appelant.
    pub fn handle_key(&mut self, key: Key, mods: Mods, platform: &mut dyn Platform) -> EditResult {
        let word = mods.ctrl || mods.alt;
        let shift = mods.shift;
        let cmd = mods.command();
        let moved = |s: &mut EditCore, p: usize| {
            s.set_caret(p, shift);
            EditResult::Moved
        };
        let changed = |b: bool| if b { EditResult::Changed } else { EditResult::Moved };
        match key {
            Key::Left if mods.meta => moved(self, 0),
            Key::Right if mods.meta => moved(self, self.text.len()),
            Key::Left => {
                if self.has_selection() && !shift && !word {
                    let p = self.selection().start;
                    return moved(self, p);
                }
                let p = if word { self.prev_word(self.caret) } else { self.prev_grapheme(self.caret) };
                moved(self, p)
            }
            Key::Right => {
                if self.has_selection() && !shift && !word {
                    let p = self.selection().end;
                    return moved(self, p);
                }
                let p = if word { self.next_word(self.caret) } else { self.next_grapheme(self.caret) };
                moved(self, p)
            }
            Key::Home => moved(self, 0),
            Key::End => moved(self, self.text.len()),
            Key::Backspace if mods.meta => {
                self.anchor = 0;
                changed(self.delete_selection(Coalesce::None))
            }
            Key::Backspace => changed(self.backspace(word)),
            Key::Delete if shift && !cmd => changed(self.cut(platform)),
            Key::Delete => changed(self.delete(word)),
            Key::Insert if cmd => {
                self.copy(platform);
                EditResult::Moved
            }
            Key::Insert if shift => changed(self.paste(platform)),
            Key::Char(c) if cmd && !mods.alt => match c.to_ascii_lowercase() {
                'a' => {
                    self.select_all();
                    EditResult::Moved
                }
                'c' => {
                    self.copy(platform);
                    EditResult::Moved
                }
                'x' => changed(self.cut(platform)),
                'v' => changed(self.paste(platform)),
                'z' if shift => changed(self.redo()),
                'z' => changed(self.undo()),
                'y' => changed(self.redo()),
                _ => EditResult::Ignored,
            },
            _ => EditResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::TestPlatform;

    #[test]
    fn typing_undo_and_words() {
        let mut e = EditCore::new("");
        for c in "salut".chars() {
            e.insert(&c.to_string());
        }
        e.insert(" ");
        for c in "toi".chars() {
            e.insert(&c.to_string());
        }
        assert_eq!(e.text(), "salut toi");
        assert_eq!(e.prev_word(9), 6);
        assert_eq!(e.prev_word(6), 0);
        assert_eq!(e.next_word(0), 6);
        assert_eq!(e.word_at(7), 6..9);
        e.undo();
        assert_eq!(e.text(), "salut ");
        e.undo();
        e.undo();
        assert_eq!(e.text(), "");
        e.redo();
        assert_eq!(e.text(), "salut");
    }

    #[test]
    fn selection_clipboard_and_limits() {
        let mut p = TestPlatform::new();
        let mut e = EditCore::new("héllo wörld");
        e.set_caret(0, false);
        e.handle_key(Key::Right, Mods::SHIFT, &mut p);
        e.handle_key(Key::Right, Mods::SHIFT, &mut p);
        assert_eq!(e.selected_text(), "hé");
        e.handle_key(Key::Char('c'), Mods::CTRL, &mut p);
        assert_eq!(p.clipboard.as_deref(), Some("hé"));
        e.handle_key(Key::Char('x'), Mods::META, &mut p);
        assert_eq!(e.text(), "llo wörld");
        e.handle_key(Key::End, Mods::NONE, &mut p);
        p.clipboard = Some("a\nb".into());
        e.handle_key(Key::Char('v'), Mods::CTRL, &mut p);
        assert_eq!(e.text(), "llo wörlda", "une ligne : seulement la première ligne collée");
        e.handle_key(Key::Backspace, Mods::CTRL, &mut p);
        assert_eq!(e.text(), "llo ");
        let mut l = EditCore::new("");
        l.max_chars = Some(3);
        l.insert("abcdef");
        assert_eq!(l.text(), "abc");
        l.read_only = true;
        assert!(!l.backspace(false));
    }
}
