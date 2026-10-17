//! Encodage des lignes : UTF-8 par défaut, repli Windows-1252 pour les vieux canaux (ADR 0012).

use std::borrow::Cow;

/// Encodage choisi pour un réseau.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Encoding {
    /// UTF-8, et Windows-1252 si une ligne n'est pas de l'UTF-8 valide.
    #[default]
    Auto,
    Utf8,
    /// Windows-1252 (Latin-1 étendu), à l'envoi comme à la réception.
    Latin1,
}

impl Encoding {
    pub fn from_name(s: &str) -> Encoding {
        match s.to_ascii_lowercase().as_str() {
            "utf-8" | "utf8" => Encoding::Utf8,
            "latin1" | "latin-1" | "iso-8859-1" | "windows-1252" | "cp1252" => Encoding::Latin1,
            _ => Encoding::Auto,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Encoding::Auto => "auto",
            Encoding::Utf8 => "utf-8",
            Encoding::Latin1 => "latin1",
        }
    }

    /// Lit une ligne reçue.
    pub fn decode(self, bytes: &[u8]) -> Cow<'_, str> {
        match self {
            Encoding::Latin1 => encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes).0,
            Encoding::Utf8 => String::from_utf8_lossy(bytes),
            Encoding::Auto => match std::str::from_utf8(bytes) {
                Ok(s) => Cow::Borrowed(s),
                Err(_) => encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes).0,
            },
        }
    }

    /// Prépare une ligne à envoyer.
    pub fn encode(self, s: &str) -> Cow<'_, [u8]> {
        match self {
            Encoding::Latin1 => encoding_rs::WINDOWS_1252.encode(s).0,
            _ => Cow::Borrowed(s.as_bytes()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto() {
        assert_eq!(Encoding::Auto.decode("café".as_bytes()), "café");
        assert_eq!(Encoding::Auto.decode(b"caf\xe9 \x80"), "café €");
        assert_eq!(&*Encoding::Latin1.encode("é"), b"\xe9");
    }
}
