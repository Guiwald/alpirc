//! Une ligne d'IRC : `@étiquettes :source COMMANDE param1 param2 :dernier paramètre`.

use std::fmt;

/// Longueur maximale d'une ligne sans les étiquettes, fin de ligne comprise (RFC 1459).
pub const MAX_LINE: usize = 512;
/// Longueur maximale de la partie étiquettes (IRCv3 message-tags), `@` et espace compris.
pub const MAX_TAGS: usize = 8191;

/// Erreur de lecture d'une ligne.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("ligne vide")]
    Empty,
    #[error("commande manquante")]
    NoCommand,
}

/// Les étiquettes IRCv3 d'un message (`@clé=valeur;clé2`), dans l'ordre reçu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags(pub Vec<(String, Option<String>)>);

impl Tags {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Valeur d'une étiquette (`Some("")` pour une étiquette sans valeur).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_deref().unwrap_or(""))
    }

    pub fn has(&self, key: &str) -> bool {
        self.0.iter().any(|(k, _)| k == key)
    }

    /// Ajoute ou remplace une étiquette.
    pub fn set(&mut self, key: &str, value: Option<&str>) {
        let v = value.map(str::to_string);
        if let Some(slot) = self.0.iter_mut().find(|(k, _)| k == key) {
            slot.1 = v;
        } else {
            self.0.push((key.to_string(), v));
        }
    }

    pub fn remove(&mut self, key: &str) {
        self.0.retain(|(k, _)| k != key);
    }

    fn parse(raw: &str) -> Tags {
        let mut out = Vec::new();
        for item in raw.split(';') {
            if item.is_empty() {
                continue;
            }
            match item.split_once('=') {
                Some((k, v)) => {
                    let v = unescape_tag(v);
                    out.push((k.to_string(), if v.is_empty() { None } else { Some(v) }));
                }
                None => out.push((item.to_string(), None)),
            }
        }
        Tags(out)
    }

    fn write(&self, out: &mut String) {
        for (i, (k, v)) in self.0.iter().enumerate() {
            if i > 0 {
                out.push(';');
            }
            out.push_str(k);
            if let Some(v) = v {
                if !v.is_empty() {
                    out.push('=');
                    escape_tag(v, out);
                }
            }
        }
    }
}

fn unescape_tag(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(':') => out.push(';'),
                Some('s') => out.push(' '),
                Some('\\') => out.push('\\'),
                Some('r') => out.push('\r'),
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn escape_tag(v: &str, out: &mut String) {
    for c in v.chars() {
        match c {
            ';' => out.push_str("\\:"),
            ' ' => out.push_str("\\s"),
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
}

/// L'auteur d'un message : un serveur (`irc.example.org`) ou un utilisateur (`pseudo!ident@hôte`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub raw: String,
}

impl Source {
    pub fn new(raw: impl Into<String>) -> Source {
        Source { raw: raw.into() }
    }

    /// Le pseudo, ou le nom du serveur.
    pub fn nick(&self) -> &str {
        let end = self.raw.find(['!', '@']).unwrap_or(self.raw.len());
        &self.raw[..end]
    }

    pub fn user(&self) -> Option<&str> {
        let start = self.raw.find('!')? + 1;
        let end = self.raw[start..].find('@').map(|i| start + i).unwrap_or(self.raw.len());
        Some(&self.raw[start..end])
    }

    pub fn host(&self) -> Option<&str> {
        let at = self.raw.find('@')?;
        Some(&self.raw[at + 1..])
    }

    /// `ident@hôte`, si connus.
    pub fn address(&self) -> Option<String> {
        match (self.user(), self.host()) {
            (Some(u), Some(h)) => Some(format!("{u}@{h}")),
            (None, Some(h)) => Some(format!("@{h}")),
            _ => None,
        }
    }

    /// Vrai si la source est un serveur (pas de `!`/`@`, et un point dans le nom).
    pub fn is_server(&self) -> bool {
        !self.raw.contains(['!', '@']) && self.raw.contains('.')
    }
}

/// Un message IRC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub tags: Tags,
    pub source: Option<Source>,
    /// Commande en capitales (`PRIVMSG`) ou numérique (`001`).
    pub command: String,
    pub params: Vec<String>,
}

impl Message {
    /// Construit un message à envoyer.
    pub fn new<S: Into<String>>(command: &str, params: impl IntoIterator<Item = S>) -> Message {
        Message {
            tags: Tags::default(),
            source: None,
            command: command.to_string(),
            params: params.into_iter().map(Into::into).collect(),
        }
    }

    pub fn with_tag(mut self, key: &str, value: Option<&str>) -> Message {
        self.tags.set(key, value);
        self
    }

    pub fn with_source(mut self, source: &str) -> Message {
        self.source = Some(Source::new(source));
        self
    }

    /// Lit une ligne (sans la fin de ligne ; `\r\n` final toléré).
    pub fn parse(line: &str) -> Result<Message, ParseError> {
        let mut rest = line.trim_end_matches(['\r', '\n']);
        rest = rest.trim_start_matches(' ');
        if rest.is_empty() {
            return Err(ParseError::Empty);
        }
        let mut tags = Tags::default();
        if let Some(r) = rest.strip_prefix('@') {
            let (t, r) = r.split_once(' ').unwrap_or((r, ""));
            tags = Tags::parse(t);
            rest = r.trim_start_matches(' ');
        }
        let mut source = None;
        if let Some(r) = rest.strip_prefix(':') {
            let (s, r) = r.split_once(' ').unwrap_or((r, ""));
            source = Some(Source::new(s));
            rest = r.trim_start_matches(' ');
        }
        let (cmd, mut rest) = rest.split_once(' ').unwrap_or((rest, ""));
        if cmd.is_empty() {
            return Err(ParseError::NoCommand);
        }
        let command = cmd.to_ascii_uppercase();
        let mut params = Vec::new();
        loop {
            rest = rest.trim_start_matches(' ');
            if rest.is_empty() {
                break;
            }
            if let Some(trailing) = rest.strip_prefix(':') {
                params.push(trailing.to_string());
                break;
            }
            let (p, r) = rest.split_once(' ').unwrap_or((rest, ""));
            params.push(p.to_string());
            rest = r;
        }
        Ok(Message { tags, source, command, params })
    }

    /// Paramètre n (0 = premier), ou "".
    pub fn param(&self, n: usize) -> &str {
        self.params.get(n).map(String::as_str).unwrap_or("")
    }

    /// Dernier paramètre, ou "".
    pub fn trailing(&self) -> &str {
        self.params.last().map(String::as_str).unwrap_or("")
    }

    /// Pseudo de la source, ou "".
    pub fn nick(&self) -> &str {
        self.source.as_ref().map(Source::nick).unwrap_or("")
    }

    /// Écrit la ligne, sans `\r\n`.
    pub fn to_line(&self) -> String {
        let mut out = String::new();
        if !self.tags.is_empty() {
            out.push('@');
            self.tags.write(&mut out);
            out.push(' ');
        }
        if let Some(s) = &self.source {
            out.push(':');
            out.push_str(&s.raw);
            out.push(' ');
        }
        out.push_str(&self.command);
        let n = self.params.len();
        for (i, p) in self.params.iter().enumerate() {
            out.push(' ');
            let last = i + 1 == n;
            if last && (p.is_empty() || p.contains(' ') || p.starts_with(':')) {
                out.push(':');
            }
            // Un paramètre intermédiaire ne peut contenir d'espace : on le coupe par sécurité.
            if !last && p.contains(' ') {
                out.push_str(&p.replace(' ', "_"));
            } else {
                out.push_str(p);
            }
        }
        // Jamais de fin de ligne dans un paramètre : elle injecterait une commande.
        out.retain(|c| c != '\r' && c != '\n' && c != '\0');
        out
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_line())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let m = Message::parse("PING :irc.example.org\r\n").unwrap();
        assert_eq!(m.command, "PING");
        assert_eq!(m.params, vec!["irc.example.org"]);
        assert!(m.source.is_none());
    }

    #[test]
    fn full() {
        let m = Message::parse(
            "@time=2026-10-17T10:00:00.000Z;msgid=abc;+draft/reply=x\\sy :Guiwald!~gui@host.fr PRIVMSG #lyon :salut à tous",
        )
        .unwrap();
        assert_eq!(m.tags.get("time"), Some("2026-10-17T10:00:00.000Z"));
        assert_eq!(m.tags.get("+draft/reply"), Some("x y"));
        assert_eq!(m.nick(), "Guiwald");
        assert_eq!(m.source.as_ref().unwrap().user(), Some("~gui"));
        assert_eq!(m.source.as_ref().unwrap().host(), Some("host.fr"));
        assert_eq!(m.params, vec!["#lyon", "salut à tous"]);
    }

    #[test]
    fn spaces_and_empty_trailing() {
        let m = Message::parse(":srv  001   moi  :").unwrap();
        assert_eq!(m.params, vec!["moi", ""]);
        let m = Message::parse("CAP * LS :").unwrap();
        assert_eq!(m.params, vec!["*", "LS", ""]);
    }

    #[test]
    fn roundtrip() {
        let m = Message::new("PRIVMSG", ["#chan", ":) hello"]).with_tag("+typing", Some("active"));
        assert_eq!(m.to_line(), "@+typing=active PRIVMSG #chan ::) hello");
        let m2 = Message::parse(&m.to_line()).unwrap();
        assert_eq!(m, m2);
        let m = Message::new("NICK", ["Guiwald"]);
        assert_eq!(m.to_line(), "NICK Guiwald");
    }

    #[test]
    fn no_injection() {
        let m = Message::new("PRIVMSG", ["#a", "x\r\nQUIT :bye"]);
        assert_eq!(m.to_line(), "PRIVMSG #a :xQUIT :bye");
    }

    #[test]
    fn tag_escape() {
        let mut t = Tags::default();
        t.set("k", Some("a;b c\\d"));
        let mut s = String::new();
        t.write(&mut s);
        assert_eq!(s, "k=a\\:b\\sc\\\\d");
        assert_eq!(Tags::parse(&s).get("k"), Some("a;b c\\d"));
    }

    #[test]
    fn server_source() {
        let m = Message::parse(":irc.undernet.org NOTICE * :*** Looking up your hostname").unwrap();
        assert!(m.source.unwrap().is_server());
    }
}
