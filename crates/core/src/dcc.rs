//! DCC : point d'extension pour un jalon suivant (ADR 0004 : CHAT, SEND/GET avec reprise, DCC passif).
//!
//! Pour l'instant, le moteur lit les offres reçues par CTCP (`DCC SEND`, `DCC CHAT`) en [`DccOffer`] et les
//! affiche (`* DCC SEND offer from nick: fichier (taille)`) ; rien n'est accepté. Le jalon DCC ajoutera un
//! `Effect` (ouvrir/écouter une connexion directe) et les fenêtres `WinKind::DccChat`.

use std::net::{IpAddr, Ipv4Addr};

/// Sorte d'offre DCC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DccKind {
    Send { file: String, size: Option<u64> },
    Chat,
    /// `RESUME`, `ACCEPT` et autres sous-commandes, non gérées pour l'instant.
    Other(String),
}

/// Une offre DCC reçue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DccOffer {
    pub from: String,
    pub kind: DccKind,
    pub ip: Option<IpAddr>,
    /// 0 pour un DCC passif (le destinataire écoute).
    pub port: u16,
    /// Jeton du DCC passif.
    pub token: Option<String>,
}

/// Découpe les paramètres en tenant compte des guillemets (`"nom de fichier.txt"`).
fn split_quoted(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut had = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                had = true;
            }
            ' ' if !quoted => {
                if had || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    had = false;
                }
            }
            c => cur.push(c),
        }
    }
    if had || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Adresse d'une offre : entier IPv4 (`3232235777`) ou adresse écrite (`::1`, `192.168.1.1`).
fn parse_ip(s: &str) -> Option<IpAddr> {
    if let Ok(n) = s.parse::<u32>() {
        return Some(IpAddr::V4(Ipv4Addr::from(n)));
    }
    s.parse().ok()
}

/// Lit les paramètres d'un CTCP DCC (`SEND fichier ip port taille [jeton]`, `CHAT chat ip port`).
pub fn parse_offer(from: &str, params: &str) -> Option<DccOffer> {
    let p = split_quoted(params);
    let sub = p.first()?.to_ascii_uppercase();
    match sub.as_str() {
        "SEND" => {
            if p.len() < 4 {
                return None;
            }
            Some(DccOffer {
                from: from.to_string(),
                kind: DccKind::Send { file: p[1].clone(), size: p.get(4).and_then(|s| s.parse().ok()) },
                ip: parse_ip(&p[2]),
                port: p[3].parse().unwrap_or(0),
                token: p.get(5).cloned(),
            })
        }
        "CHAT" => {
            if p.len() < 4 {
                return None;
            }
            Some(DccOffer {
                from: from.to_string(),
                kind: DccKind::Chat,
                ip: parse_ip(&p[2]),
                port: p[3].parse().unwrap_or(0),
                token: p.get(4).cloned(),
            })
        }
        other => Some(DccOffer { from: from.to_string(), kind: DccKind::Other(other.to_string()), ip: None, port: 0, token: None }),
    }
}

/// Taille lisible (`1.5MB`).
pub fn human_size(n: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < units.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 { format!("{n}B") } else { format!("{v:.1}{}", units[u]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_offer() {
        let o = parse_offer("Pat", "SEND \"my file.txt\" 3232235777 5000 2048").unwrap();
        assert_eq!(o.kind, DccKind::Send { file: "my file.txt".into(), size: Some(2048) });
        assert_eq!(o.ip, Some("192.168.1.1".parse().unwrap()));
        assert_eq!(o.port, 5000);
        let o = parse_offer("Pat", "CHAT chat 2130706433 4000").unwrap();
        assert_eq!(o.kind, DccKind::Chat);
        assert!(parse_offer("Pat", "SEND x").is_none());
        assert_eq!(human_size(1536), "1.5KB");
    }
}
