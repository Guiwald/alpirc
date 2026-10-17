//! CTCP : messages encadrés par `\x01` dans un PRIVMSG (requête) ou un NOTICE (réponse).

pub const DELIM: char = '\x01';

/// Une requête ou réponse CTCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ctcp<'a> {
    /// Commande en capitales (`ACTION`, `VERSION`, `PING`, `DCC`…).
    pub command: String,
    /// Le reste, sans l'espace qui suit la commande.
    pub params: &'a str,
}

/// Lit un CTCP dans le texte d'un message ; `None` si ce n'en est pas un.
pub fn parse(text: &str) -> Option<Ctcp<'_>> {
    let inner = text.strip_prefix(DELIM)?;
    let inner = inner.strip_suffix(DELIM).unwrap_or(inner);
    let (cmd, params) = inner.split_once(' ').unwrap_or((inner, ""));
    if cmd.is_empty() {
        return None;
    }
    Some(Ctcp { command: cmd.to_ascii_uppercase(), params })
}

/// Encadre une commande CTCP.
pub fn encode(command: &str, params: &str) -> String {
    if params.is_empty() {
        format!("{DELIM}{command}{DELIM}")
    } else {
        format!("{DELIM}{command} {params}{DELIM}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action() {
        let c = parse("\x01ACTION danse\x01").unwrap();
        assert_eq!(c.command, "ACTION");
        assert_eq!(c.params, "danse");
        let c = parse("\x01version").unwrap();
        assert_eq!(c.command, "VERSION");
        assert!(parse("bonjour").is_none());
        assert_eq!(encode("PING", "123"), "\x01PING 123\x01");
    }
}
