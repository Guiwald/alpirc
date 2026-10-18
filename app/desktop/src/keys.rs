//! Traduction des touches de winit en touches de la boîte à outils.

use alpirc_ui::{Key, Mods};
use winit::event::KeyEvent;
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;

pub fn mods(m: ModifiersState) -> Mods {
    Mods { shift: m.shift_key(), ctrl: m.control_key(), alt: m.alt_key(), meta: m.super_key() }
}

/// Mac : Option sert à taper des caractères (Option+Maj+L → « | », Option+( → « { » sur un clavier français ;
/// Option+O → « ø »…). Quand la touche produit un caractère sans Cmd ni Ctrl, ce n'est donc pas un raccourci Alt
/// (Alt+O, mnémoniques des boîtes) : elle part comme une frappe ordinaire. Option garde son rôle avec les touches qui
/// ne produisent rien (Option+← : mot précédent, Option+Effacement : mot).
pub fn option_types_text(m: Mods, text: Option<&str>) -> Mods {
    let printable = text.is_some_and(|t| !t.is_empty() && !t.chars().any(char::is_control));
    if cfg!(target_os = "macos") && m.alt && !m.ctrl && !m.meta && printable { Mods { alt: false, ..m } } else { m }
}

/// (touche, modificateurs, texte produit) d'un appui ; `None` pour les touches seules (Maj, Ctrl…).
pub fn map(ev: &KeyEvent, m: Mods) -> Option<(Key, Mods, Option<String>)> {
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
        WKey::Character(_) => {
            // Le caractère de base de la touche (sans Option/Alt), pour les raccourcis (Option+O → « o »).
            let base = match ev.key_without_modifiers() {
                WKey::Character(s) => s.chars().next(),
                _ => None,
            };
            match base {
                Some(c) => Key::Char(c.to_lowercase().next().unwrap_or(c)),
                None => Key::Other,
            }
        }
        WKey::Dead(_) => return None,
        _ => Key::Other,
    };
    // Pas de texte avec Ctrl/Cmd (raccourcis) ; sinon le texte produit, sans caractères de contrôle.
    let text = if m.command() {
        None
    } else {
        ev.text.as_ref().map(|t| t.to_string()).filter(|t| !t.is_empty() && !t.chars().any(char::is_control))
    };
    let m = option_types_text(m, text.as_deref());
    Some((key, m, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_tape_des_caracteres_sur_mac() {
        let opt = Mods { alt: true, ..Mods::NONE };
        let opt_shift = Mods { alt: true, shift: true, ..Mods::NONE };
        let mac = cfg!(target_os = "macos");
        // Option+Maj+L (« | » sur un clavier français) : une frappe, pas Alt+L.
        assert_eq!(option_types_text(opt_shift, Some("|")).alt, !mac);
        assert_eq!(option_types_text(opt_shift, Some("|")).shift, true);
        assert_eq!(option_types_text(opt, Some("ø")).alt, !mac);
        // Sans caractère produit (flèches, effacement), Option reste Option.
        assert!(option_types_text(opt, None).alt);
        assert!(option_types_text(opt, Some("\u{8}")).alt);
        // Avec Cmd ou Ctrl, rien ne change.
        let cmd_opt = Mods { alt: true, meta: true, ..Mods::NONE };
        assert_eq!(option_types_text(cmd_opt, Some("ø")), cmd_opt);
        let ctrl_opt = Mods { alt: true, ctrl: true, ..Mods::NONE };
        assert_eq!(option_types_text(ctrl_opt, Some("ø")), ctrl_opt);
        assert_eq!(option_types_text(Mods::SHIFT, Some("A")), Mods::SHIFT);
    }
}
