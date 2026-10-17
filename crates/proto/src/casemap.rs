//! Comparaison des pseudos et des canaux selon `CASEMAPPING` (RFC 1459 : `[]\~` sont les minuscules de `{}|^`).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaseMapping {
    Ascii,
    #[default]
    Rfc1459,
    StrictRfc1459,
}

impl CaseMapping {
    pub fn from_token(s: &str) -> CaseMapping {
        match s.to_ascii_lowercase().as_str() {
            "ascii" => CaseMapping::Ascii,
            "strict-rfc1459" => CaseMapping::StrictRfc1459,
            _ => CaseMapping::Rfc1459,
        }
    }

    pub fn lower_char(self, c: char) -> char {
        match (self, c) {
            (_, 'A'..='Z') => c.to_ascii_lowercase(),
            (CaseMapping::Rfc1459 | CaseMapping::StrictRfc1459, '[') => '{',
            (CaseMapping::Rfc1459 | CaseMapping::StrictRfc1459, ']') => '}',
            (CaseMapping::Rfc1459 | CaseMapping::StrictRfc1459, '\\') => '|',
            (CaseMapping::Rfc1459, '~') => '^',
            _ => c,
        }
    }

    /// Forme canonique (minuscules) d'un nom.
    pub fn fold(self, s: &str) -> String {
        s.chars().map(|c| self.lower_char(c)).collect()
    }

    pub fn eq(self, a: &str, b: &str) -> bool {
        a.len() == b.len() && a.chars().zip(b.chars()).all(|(x, y)| self.lower_char(x) == self.lower_char(y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc1459() {
        let m = CaseMapping::Rfc1459;
        assert!(m.eq("Gui[w]ald", "gui{W}ALD"));
        assert!(m.eq("a~", "A^"));
        assert!(!CaseMapping::StrictRfc1459.eq("a~", "a^"));
        assert!(!CaseMapping::Ascii.eq("[", "{"));
        assert_eq!(m.fold("#Lyon"), "#lyon");
    }
}
