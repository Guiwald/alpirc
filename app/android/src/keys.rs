//! Traduction des touches de winit (clavier physique, clavier du système pour les champs de Rust, clavier de
//! l'ordinateur dans le simulateur) en touches de la boîte à outils.

use alpirc_ui::{Key, Mods};
use winit::event::KeyEvent;
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};

pub fn mods(m: ModifiersState) -> Mods {
    Mods { shift: m.shift_key(), ctrl: m.control_key(), alt: m.alt_key(), meta: m.super_key() }
}

/// Touche Retour d'Android (ou Échap du simulateur, traitée à part par l'appli).
pub fn is_back(ev: &KeyEvent) -> bool {
    matches!(ev.logical_key, WKey::Named(NamedKey::GoBack | NamedKey::BrowserBack))
}

/// (touche, texte produit) d'un appui ; `None` pour les touches seules (Maj, Ctrl…).
pub fn map(ev: &KeyEvent, m: Mods) -> Option<(Key, Option<String>)> {
    let key = match &ev.logical_key {
        WKey::Named(n) => match n {
            NamedKey::Enter => Key::Enter,
            NamedKey::Escape => Key::Escape,
            NamedKey::Tab => Key::Tab,
            NamedKey::Backspace => Key::Backspace,
            NamedKey::Delete => Key::Delete,
            NamedKey::ArrowLeft => Key::Left,
            NamedKey::ArrowRight => Key::Right,
            NamedKey::ArrowUp => Key::Up,
            NamedKey::ArrowDown => Key::Down,
            NamedKey::Home => Key::Home,
            NamedKey::End => Key::End,
            NamedKey::PageUp => Key::PageUp,
            NamedKey::PageDown => Key::PageDown,
            NamedKey::Insert => Key::Insert,
            NamedKey::Space => Key::Char(' '),
            NamedKey::F1 => Key::F(1),
            NamedKey::F2 => Key::F(2),
            NamedKey::F3 => Key::F(3),
            NamedKey::F4 => Key::F(4),
            NamedKey::F5 => Key::F(5),
            NamedKey::F6 => Key::F(6),
            NamedKey::F7 => Key::F(7),
            NamedKey::F8 => Key::F(8),
            NamedKey::F9 => Key::F(9),
            NamedKey::F10 => Key::F(10),
            NamedKey::F11 => Key::F(11),
            NamedKey::F12 => Key::F(12),
            NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super | NamedKey::Meta | NamedKey::CapsLock | NamedKey::Fn => return None,
            _ => Key::Other,
        },
        WKey::Character(s) => match s.chars().next() {
            Some(c) => Key::Char(c.to_lowercase().next().unwrap_or(c)),
            None => Key::Other,
        },
        WKey::Dead(_) => return None,
        _ => Key::Other,
    };
    // Pas de texte avec Ctrl/Cmd (raccourcis) ; sinon le texte produit, sans caractères de contrôle.
    let text = if m.command() {
        None
    } else {
        ev.text
            .as_ref()
            .map(|t| t.to_string())
            .or_else(|| match &ev.logical_key {
                WKey::Character(s) => Some(s.to_string()),
                WKey::Named(NamedKey::Space) => Some(" ".to_string()),
                _ => None,
            })
            .filter(|t| !t.is_empty() && !t.chars().any(char::is_control))
    };
    Some((key, text))
}
