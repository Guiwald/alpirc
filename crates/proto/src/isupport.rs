//! `RPL_ISUPPORT` (005) : ce que le serveur annonce de lui-même (préfixes, types de modes, longueurs…).

use std::collections::BTreeMap;

use crate::casemap::CaseMapping;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ISupport {
    pub network: Option<String>,
    pub casemapping: CaseMapping,
    /// (mode, symbole), du plus fort au plus faible : `[('o','@'), ('v','+')]`.
    pub prefix: Vec<(char, char)>,
    /// Modes de canal par type : A (listes), B (paramètre toujours), C (paramètre à l'ajout), D (sans paramètre).
    pub chanmodes: [String; 4],
    pub chantypes: String,
    pub statusmsg: String,
    pub modes_per_line: usize,
    pub nicklen: usize,
    pub topiclen: Option<usize>,
    pub kicklen: Option<usize>,
    pub excepts: Option<char>,
    pub invex: Option<char>,
    pub monitor: Option<usize>,
    pub whox: bool,
    pub utf8only: bool,
    pub bot_mode: Option<char>,
    /// Toutes les valeurs reçues, telles quelles.
    pub raw: BTreeMap<String, String>,
}

impl Default for ISupport {
    fn default() -> ISupport {
        ISupport {
            network: None,
            casemapping: CaseMapping::Rfc1459,
            prefix: vec![('o', '@'), ('v', '+')],
            chanmodes: ["b".into(), "k".into(), "l".into(), "imnpst".into()],
            chantypes: "#&".into(),
            statusmsg: String::new(),
            modes_per_line: 3,
            nicklen: 9,
            topiclen: None,
            kicklen: None,
            excepts: None,
            invex: None,
            monitor: None,
            whox: false,
            utf8only: false,
            bot_mode: None,
            raw: BTreeMap::new(),
        }
    }
}

impl ISupport {
    /// Applique les paramètres d'une ligne 005 (sans le pseudo en tête ni le texte final).
    pub fn apply(&mut self, tokens: &[String]) {
        for tok in tokens {
            if let Some(name) = tok.strip_prefix('-') {
                self.raw.remove(name);
                continue;
            }
            let (k, v) = tok.split_once('=').unwrap_or((tok.as_str(), ""));
            let v = unescape(v);
            self.raw.insert(k.to_string(), v.clone());
            match k {
                "NETWORK" => self.network = Some(v),
                "CASEMAPPING" => self.casemapping = CaseMapping::from_token(&v),
                "PREFIX" => {
                    if let Some(rest) = v.strip_prefix('(') {
                        if let Some((modes, syms)) = rest.split_once(')') {
                            self.prefix = modes.chars().zip(syms.chars()).collect();
                        }
                    } else if v.is_empty() {
                        self.prefix.clear();
                    }
                }
                "CHANMODES" => {
                    let mut parts = v.split(',');
                    for slot in self.chanmodes.iter_mut() {
                        *slot = parts.next().unwrap_or("").to_string();
                    }
                }
                "CHANTYPES" => self.chantypes = v,
                "STATUSMSG" => self.statusmsg = v,
                "MODES" => self.modes_per_line = v.parse().unwrap_or(3),
                "NICKLEN" | "MAXNICKLEN" => self.nicklen = v.parse().unwrap_or(self.nicklen),
                "TOPICLEN" => self.topiclen = v.parse().ok(),
                "KICKLEN" => self.kicklen = v.parse().ok(),
                "EXCEPTS" => self.excepts = Some(v.chars().next().unwrap_or('e')),
                "INVEX" => self.invex = Some(v.chars().next().unwrap_or('I')),
                "MONITOR" => self.monitor = Some(v.parse().unwrap_or(100)),
                "WHOX" => self.whox = true,
                "UTF8ONLY" => self.utf8only = true,
                "BOT" => self.bot_mode = v.chars().next(),
                _ => {}
            }
        }
    }

    pub fn is_channel(&self, name: &str) -> bool {
        name.chars().next().is_some_and(|c| self.chantypes.contains(c))
    }

    /// Mode du préfixe `@` → `o`.
    pub fn mode_for_symbol(&self, sym: char) -> Option<char> {
        self.prefix.iter().find(|(_, s)| *s == sym).map(|(m, _)| *m)
    }

    pub fn symbol_for_mode(&self, mode: char) -> Option<char> {
        self.prefix.iter().find(|(m, _)| *m == mode).map(|(_, s)| *s)
    }

    /// Rang d'un mode de préfixe (0 = le plus fort).
    pub fn rank(&self, mode: char) -> Option<usize> {
        self.prefix.iter().position(|(m, _)| *m == mode)
    }

    /// Type (0 = A … 3 = D) d'un mode de canal ; les préfixes comptent comme B.
    pub fn chanmode_type(&self, mode: char) -> Option<usize> {
        if self.prefix.iter().any(|(m, _)| *m == mode) {
            return Some(1);
        }
        self.chanmodes.iter().position(|s| s.contains(mode))
    }

    /// Retire les symboles de préfixe d'un pseudo de NAMES (`@+Guiwald` → modes "ov", "Guiwald").
    pub fn split_prefixes<'a>(&self, name: &'a str) -> (String, &'a str) {
        let mut modes = String::new();
        let mut rest = name;
        while let Some(c) = rest.chars().next() {
            match self.mode_for_symbol(c) {
                Some(m) => {
                    modes.push(m);
                    rest = &rest[c.len_utf8()..];
                }
                None => break,
            }
        }
        (modes, rest)
    }
}

/// `\xHH` dans les valeurs (ex. `NETWORK=Rizon\x20IRC`).
fn unescape(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && b.get(i + 1) == Some(&b'x') && i + 4 <= b.len() {
            if let Some(h) = std::str::from_utf8(&b[i + 2..i + 4]).ok().and_then(|s| u8::from_str_radix(s, 16).ok()) {
                out.push(h);
                i += 4;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<String> {
        s.split(' ').map(str::to_string).collect()
    }

    #[test]
    fn parse() {
        let mut i = ISupport::default();
        i.apply(&toks("NETWORK=Undernet PREFIX=(ohv)@%+ CHANMODES=b,k,l,imnpstrDd CASEMAPPING=rfc1459 NICKLEN=12 EXCEPTS MONITOR=100 NETWORK=Rizon\\x20IRC"));
        assert_eq!(i.network.as_deref(), Some("Rizon IRC"));
        assert_eq!(i.prefix, vec![('o', '@'), ('h', '%'), ('v', '+')]);
        assert_eq!(i.nicklen, 12);
        assert_eq!(i.excepts, Some('e'));
        assert_eq!(i.chanmode_type('l'), Some(2));
        assert_eq!(i.chanmode_type('h'), Some(1));
        assert_eq!(i.split_prefixes("@+Guiwald"), ("ov".to_string(), "Guiwald"));
        assert!(i.is_channel("#lyon"));
        assert!(!i.is_channel("Guiwald"));
    }
}
