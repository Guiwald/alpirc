//! Les touches pour les scripts : `on KEYDOWN` et `on KEYUP` (`$keyval`, `$keychar`) dans les fenêtres `@` et dans
//! la ligne de saisie des autres fenêtres, comme dans mIRC.
//!
//! La boîte à outils ne signale que les appuis : `on KEYUP` suit donc tout de suite `on KEYDOWN`.

use alpirc_core::{WinId, WinKind};
use alpirc_ui::{Input, Key};

use crate::{Shell, ShellPlatform};

/// Code de touche à la manière de Windows (comme `$keyval` de mIRC) et caractère produit, s'il y en a un.
pub(crate) fn mirc_key(key: &Key, text: Option<&str>) -> Option<(u32, Option<char>)> {
    let ch = text.and_then(|t| t.chars().next()).filter(|c| !c.is_control());
    let code = match key {
        Key::Char(c) if c.is_ascii_alphabetic() => c.to_ascii_uppercase() as u32,
        Key::Char(c) => *c as u32,
        Key::Enter => 13,
        Key::Escape => 27,
        Key::Tab => 9,
        Key::Backspace => 8,
        Key::Delete => 46,
        Key::Insert => 45,
        Key::Left => 37,
        Key::Up => 38,
        Key::Right => 39,
        Key::Down => 40,
        Key::Home => 36,
        Key::End => 35,
        Key::PageUp => 33,
        Key::PageDown => 34,
        Key::F(n) => 111 + *n as u32,
        Key::Other => return None,
    };
    Some((code, ch))
}

impl Shell {
    /// Une touche dans une fenêtre : `on KEYDOWN` puis `on KEYUP`. Rend vrai si un script a fait `/haltdef` (la
    /// touche n'est alors pas traitée).
    pub(crate) fn script_key(&mut self, win: WinId, input: &Input, p: &mut dyn ShellPlatform) -> bool {
        let Input::Key { key, text, .. } = input else { return false };
        let Some(w) = self.client.window(win) else { return false };
        // Fenêtres `@`, et celles qui ont une ligne de saisie (pas les listes : canaux, adresses, Notify).
        if !matches!(w.kind, WinKind::Custom | WinKind::Status | WinKind::Channel | WinKind::Query | WinKind::DccChat) {
            return false;
        }
        let Some((code, ch)) = mirc_key(key, text.as_deref()) else { return false };
        let now = p.now_ms();
        let halted = self.client.key_event(win, code, ch, false, now);
        self.client.key_event(win, code, ch, true, now);
        if halted {
            self.after(p);
        }
        halted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_codes_like_windows() {
        assert_eq!(mirc_key(&Key::Char('a'), Some("a")), Some((65, Some('a'))));
        assert_eq!(mirc_key(&Key::Char('a'), Some("A")), Some((65, Some('A'))));
        assert_eq!(mirc_key(&Key::Char('7'), Some("7")), Some((55, Some('7'))));
        assert_eq!(mirc_key(&Key::Enter, None), Some((13, None)));
        assert_eq!(mirc_key(&Key::F(1), None), Some((112, None)));
        assert_eq!(mirc_key(&Key::Down, None), Some((40, None)));
        assert_eq!(mirc_key(&Key::Other, None), None);
    }
}
