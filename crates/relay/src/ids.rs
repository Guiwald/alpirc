//! Identifiants de messages (`msgid`) et petits outils d'heure du relais.
//!
//! Un `msgid` du relais est l'écriture base64 (alphabet URL, sans remplissage) de 16 octets : l'heure du message
//! en millisecondes (8 octets, gros-boutiste), un compteur (4 octets) et un sel tiré au démarrage (4 octets).
//! L'heure incluse permet de retrouver tout de suite le fichier du jour qui contient le message
//! (`CHATHISTORY … msgid=…`) ; le compteur et le sel le rendent unique.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Heure réelle en millisecondes depuis 1970.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// `2026-10-17T10:00:00.000Z`.
pub fn format_time(ms: i64) -> String {
    alpirc_core::timeutil::format_iso8601(ms)
}

/// Lit une heure ISO 8601 (étiquette `time`, `timestamp=` de CHATHISTORY et MARKREAD).
pub fn parse_time(s: &str) -> Option<i64> {
    alpirc_core::timeutil::parse_iso8601(s)
}

/// Jour (depuis le 1/1/1970, UTC) d'un instant.
pub fn day_of(ms: i64) -> i64 {
    ms.div_euclid(86_400_000)
}

/// `AAAA-MM-JJ` d'un jour.
pub fn day_name(day: i64) -> String {
    let (y, m, d) = alpirc_core::timeutil::civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Jour d'un nom `AAAA-MM-JJ`.
pub fn parse_day_name(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let m: u32 = s.get(5..7)?.parse().ok()?;
    let d: u32 = s.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(alpirc_core::timeutil::days_from_civil(y, m, d))
}

/// Fabrique de `msgid`.
#[derive(Debug, Clone)]
pub struct MsgIdGen {
    counter: u32,
    salt: u32,
}

impl Default for MsgIdGen {
    fn default() -> Self {
        Self::new()
    }
}

impl MsgIdGen {
    /// Compteur et sel tirés au hasard (deux relais lancés à la même milliseconde ne se marchent pas dessus).
    pub fn new() -> MsgIdGen {
        let mut b = [0u8; 8];
        if getrandom::fill(&mut b).is_err() {
            // Repli : l'heure (l'unicité reste assurée par le compteur dans ce processus).
            b = (now_ms() as u64).to_le_bytes();
        }
        MsgIdGen {
            counter: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            salt: u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        }
    }

    /// Fabrique déterministe (essais).
    pub fn with_seed(counter: u32, salt: u32) -> MsgIdGen {
        MsgIdGen { counter, salt }
    }

    /// Nouvel identifiant pour un message daté `time_ms`.
    pub fn next(&mut self, time_ms: i64) -> String {
        self.counter = self.counter.wrapping_add(1);
        let mut raw = [0u8; 16];
        raw[..8].copy_from_slice(&(time_ms as u64).to_be_bytes());
        raw[8..12].copy_from_slice(&self.counter.to_be_bytes());
        raw[12..].copy_from_slice(&self.salt.to_be_bytes());
        URL_SAFE_NO_PAD.encode(raw)
    }
}

/// Heure contenue dans un `msgid` du relais (`None` pour un identifiant d'une autre origine).
pub fn msgid_time(id: &str) -> Option<i64> {
    if id.len() != 22 {
        return None;
    }
    let raw = URL_SAFE_NO_PAD.decode(id).ok()?;
    let b: [u8; 8] = raw.get(..8)?.try_into().ok()?;
    Some(u64::from_be_bytes(b) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msgids_are_unique_and_carry_time() {
        let mut g = MsgIdGen::with_seed(1, 42);
        let a = g.next(1_760_000_000_123);
        let b = g.next(1_760_000_000_123);
        assert_ne!(a, b);
        assert_eq!(a.len(), 22);
        assert!(a.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'));
        assert_eq!(msgid_time(&a), Some(1_760_000_000_123));
        assert_eq!(msgid_time(&b), Some(1_760_000_000_123));
        assert_eq!(msgid_time("pas-un-msgid"), None);
        assert_eq!(msgid_time("@@@@@@@@@@@@@@@@@@@@@@"), None);
    }

    #[test]
    fn random_generators_differ() {
        let mut a = MsgIdGen::new();
        let mut b = MsgIdGen::new();
        assert_ne!(a.next(5), b.next(5));
    }

    #[test]
    fn days_and_times() {
        let t = parse_time("2026-10-17T23:59:59.999Z").unwrap();
        assert_eq!(format_time(t), "2026-10-17T23:59:59.999Z");
        let d = day_of(t);
        assert_eq!(day_name(d), "2026-10-17");
        assert_eq!(parse_day_name("2026-10-17"), Some(d));
        assert_eq!(day_of(t + 1), d + 1);
        assert_eq!(parse_day_name("2026-13-01"), None);
        assert_eq!(parse_day_name("x"), None);
        assert_eq!(day_of(-1), -1);
    }
}
