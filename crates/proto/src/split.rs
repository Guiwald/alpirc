//! Découpage des longs messages pour qu'une ligne relayée ne dépasse pas 512 octets.

use crate::message::MAX_LINE;

/// Place disponible pour le texte d'un `PRIVMSG`/`NOTICE` vers `target`, telle que le serveur le relaiera
/// (`:pseudo!ident@hôte PRIVMSG cible :texte\r\n`). `source_len` : longueur de `pseudo!ident@hôte` (si inconnue,
/// prendre une estimation large, par ex. pseudo + 1 + 10 + 1 + 63).
pub fn text_budget(command: &str, target: &str, source_len: usize) -> usize {
    let overhead = 1 + source_len + 1 + command.len() + 1 + target.len() + 2 + 2;
    MAX_LINE.saturating_sub(overhead).max(32)
}

/// Coupe un texte en morceaux d'au plus `max` octets, de préférence sur une espace, sans couper un caractère.
pub fn split_text(text: &str, max: usize) -> Vec<String> {
    let max = max.max(4);
    let mut out = Vec::new();
    let mut rest = text;
    while rest.len() > max {
        let mut cut = max;
        while !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        // Une espace dans le dernier tiers du morceau : on coupe là.
        if let Some(sp) = rest[..cut].rfind(' ') {
            if sp > cut / 3 {
                cut = sp;
            }
        }
        out.push(rest[..cut].to_string());
        rest = rest[cut..].strip_prefix(' ').unwrap_or(&rest[cut..]);
    }
    if !rest.is_empty() || out.is_empty() {
        out.push(rest.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split() {
        let t = "aaaa bbbb cccc dddd";
        assert_eq!(split_text(t, 10), vec!["aaaa bbbb", "cccc dddd"]);
        let t = "éééééé";
        let parts = split_text(t, 5);
        assert!(parts.iter().all(|p| p.len() <= 5));
        assert_eq!(parts.concat(), t);
        assert_eq!(split_text("", 10), vec![""]);
        assert!(text_budget("PRIVMSG", "#lyon", 40) < 512);
    }
}
