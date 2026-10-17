//! DCC (ADR 0004 : CHAT, SEND/GET avec reprise, DCC passif) : le protocole, sans entrées-sorties.
//!
//! - lecture des CTCP `DCC` reçus ([`parse_offer`]) : `SEND`, `CHAT`, `RESUME`, `ACCEPT`, avec les noms entre
//!   guillemets (ou à espaces sans guillemets, lus depuis la fin), l'adresse IPv4 en entier sur 32 bits ou une
//!   adresse IPv6 écrite, le port 0 et le jeton du DCC passif (« reverse DCC ») ;
//! - écriture des CTCP envoyés ([`encode_send`], [`encode_chat`], [`encode_resume`], [`encode_accept`]) ;
//! - noms de fichiers reçus assainis ([`sanitize_filename`] : pas de chemin, pas de fichier caché, pas de
//!   caractère de contrôle ni de retournement de texte) et fichiers jamais acceptés tout seuls
//!   ([`is_dangerous`] : exécutables, scripts, paquets) ;
//! - description des flux que l'hôte ouvre pour le moteur ([`DccIo`]).
//!
//! Le moteur (`client::dcc`) s'en sert pour tenir les transferts et les discussions ; l'hôte ouvre les
//! connexions directes (`alpirc-net`).

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

/// Sorte d'un CTCP DCC reçu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DccKind {
    /// `SEND fichier ip port taille [jeton]` : offre d'envoi, ou réponse à notre offre passive (port non nul
    /// et jeton).
    Send { file: String, size: Option<u64> },
    /// `CHAT chat ip port [jeton]`.
    Chat,
    /// `RESUME fichier port position [jeton]` : le destinataire demande à reprendre à `position`.
    Resume { file: String, position: u64 },
    /// `ACCEPT fichier port position [jeton]` : l'envoyeur accepte la reprise.
    Accept { file: String, position: u64 },
    /// Autres sous-commandes (non gérées).
    Other(String),
}

/// Un CTCP DCC reçu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DccOffer {
    pub from: String,
    pub kind: DccKind,
    /// Adresse annoncée (`SEND` et `CHAT`).
    pub ip: Option<IpAddr>,
    /// 0 pour un DCC passif (le destinataire écoute).
    pub port: u16,
    /// Jeton du DCC passif.
    pub token: Option<String>,
}

impl DccOffer {
    /// Offre passive : port 0 et jeton (le destinataire doit écouter et répondre).
    pub fn is_passive(&self) -> bool {
        self.port == 0 && self.token.is_some()
    }
}

/// Le flux qu'ouvre l'hôte pour une connexion DCC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DccIo {
    /// DCC CHAT : des lignes dans les deux sens.
    Chat,
    /// Envoyer `path` à partir de `offset` (reprise) ; `size` : taille totale annoncée ; `packet` : taille des
    /// blocs écrits.
    Send { path: PathBuf, offset: u64, size: u64, packet: u32 },
    /// Recevoir dans `path` à partir de `offset` ; `size` : taille annoncée (`None` : jusqu'à la fermeture).
    Receive { path: PathBuf, offset: u64, size: Option<u64> },
}

/// Découpe les paramètres en tenant compte des guillemets (`"nom de fichier.txt"`) ; rend aussi, pour chaque
/// mot, s'il était entre guillemets.
fn split_quoted(s: &str) -> Vec<(String, bool)> {
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
                    out.push((std::mem::take(&mut cur), had));
                    had = false;
                }
            }
            c => cur.push(c),
        }
    }
    if had || !cur.is_empty() {
        out.push((cur, had));
    }
    out
}

/// Adresse d'une offre : entier IPv4 (`3232235777`) ou adresse écrite (`::1`, `192.168.1.1`).
pub fn parse_ip(s: &str) -> Option<IpAddr> {
    if let Ok(n) = s.parse::<u32>() {
        return Some(IpAddr::V4(Ipv4Addr::from(n)));
    }
    s.trim_start_matches('[').trim_end_matches(']').parse().ok()
}

/// Adresse telle qu'on l'écrit dans une offre : entier pour IPv4 (et IPv6 « mappée » IPv4), adresse écrite
/// pour IPv6.
pub fn wire_ip(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => u32::from(v4).to_string(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => u32::from(v4).to_string(),
            None => v6.to_string(),
        },
    }
}

/// Nom de fichier pour une offre : entre guillemets s'il contient des espaces (les guillemets du nom
/// deviennent des apostrophes : le protocole ne sait pas les échapper).
pub fn quote_name(name: &str) -> String {
    let n: String = name.chars().map(|c| if c == '"' { '\'' } else { c }).filter(|c| !c.is_control()).collect();
    if n.contains(' ') || n.is_empty() { format!("\"{n}\"") } else { n }
}

fn is_port(s: &str) -> bool {
    s.parse::<u16>().is_ok()
}

fn is_num(s: &str) -> bool {
    s.parse::<u64>().is_ok()
}

/// Nom et champs qui suivent : le nom est le premier mot s'il était entre guillemets, sinon tout ce qui
/// précède les `n` derniers champs (noms à espaces envoyés sans guillemets).
fn name_and_fields(rest: &[(String, bool)], n: usize) -> Option<(String, Vec<String>)> {
    if rest.is_empty() {
        return None;
    }
    if rest[0].1 || rest.len() <= n {
        let fields = rest[1..].iter().map(|x| x.0.clone()).collect();
        return Some((rest[0].0.clone(), fields));
    }
    let cut = rest.len() - n;
    let name = rest[..cut].iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(" ");
    Some((name, rest[cut..].iter().map(|x| x.0.clone()).collect()))
}

/// Lit les paramètres d'un CTCP DCC (`SEND fichier ip port taille [jeton]`, `CHAT chat ip port [jeton]`,
/// `RESUME fichier port position [jeton]`, `ACCEPT fichier port position [jeton]`).
pub fn parse_offer(from: &str, params: &str) -> Option<DccOffer> {
    let p = split_quoted(params.trim());
    let sub = p.first()?.0.to_ascii_uppercase();
    let rest = &p[1..];
    let words: Vec<&str> = rest.iter().map(|x| x.0.as_str()).collect();
    let n = words.len();
    match sub.as_str() {
        "SEND" => {
            if n < 3 {
                return None;
            }
            // Combien de champs après le nom : ip port taille [jeton].
            let k = if rest[0].1 {
                n - 1
            } else if n >= 5 && parse_ip(words[n - 4]).is_some() && is_port(words[n - 3]) && is_num(words[n - 2]) {
                4
            } else if n >= 4 && parse_ip(words[n - 3]).is_some() && is_port(words[n - 2]) && is_num(words[n - 1]) {
                3
            } else {
                n - 1
            };
            let (file, f) = name_and_fields(rest, k)?;
            if f.len() < 2 {
                return None;
            }
            Some(DccOffer {
                from: from.to_string(),
                kind: DccKind::Send { file, size: f.get(2).and_then(|s| s.parse().ok()) },
                ip: parse_ip(&f[0]),
                port: f[1].parse().ok()?,
                token: f.get(3).cloned().filter(|t| !t.is_empty()),
            })
        }
        "CHAT" => {
            if n < 3 {
                return None;
            }
            Some(DccOffer {
                from: from.to_string(),
                kind: DccKind::Chat,
                ip: parse_ip(words[1]),
                port: words[2].parse().ok()?,
                token: words.get(3).map(|t| t.to_string()).filter(|t| !t.is_empty()),
            })
        }
        "RESUME" | "ACCEPT" => {
            if n < 3 {
                return None;
            }
            // port position [jeton] : le jeton n'accompagne que le port 0 (DCC passif).
            let k = if rest[0].1 {
                n - 1
            } else if n >= 4 && words[n - 3] == "0" && is_num(words[n - 2]) {
                3
            } else {
                2
            };
            let (file, f) = name_and_fields(rest, k)?;
            if f.len() < 2 {
                return None;
            }
            let position = f[1].parse().ok()?;
            let kind = if sub == "RESUME" { DccKind::Resume { file, position } } else { DccKind::Accept { file, position } };
            Some(DccOffer { from: from.to_string(), kind, ip: None, port: f[0].parse().ok()?, token: f.get(2).cloned() })
        }
        other => Some(DccOffer { from: from.to_string(), kind: DccKind::Other(other.to_string()), ip: None, port: 0, token: None }),
    }
}

/// Paramètres d'un `DCC SEND` (sans `DCC`) : `SEND nom ip port taille [jeton]`.
pub fn encode_send(name: &str, ip: IpAddr, port: u16, size: u64, token: Option<&str>) -> String {
    let mut s = format!("SEND {} {} {port} {size}", quote_name(name), wire_ip(ip));
    if let Some(t) = token {
        s.push(' ');
        s.push_str(t);
    }
    s
}

/// Paramètres d'un `DCC CHAT` : `CHAT chat ip port [jeton]`.
pub fn encode_chat(ip: IpAddr, port: u16, token: Option<&str>) -> String {
    let mut s = format!("CHAT chat {} {port}", wire_ip(ip));
    if let Some(t) = token {
        s.push(' ');
        s.push_str(t);
    }
    s
}

/// Paramètres d'un `DCC RESUME` : `RESUME nom port position [jeton]`.
pub fn encode_resume(name: &str, port: u16, position: u64, token: Option<&str>) -> String {
    let mut s = format!("RESUME {} {port} {position}", quote_name(name));
    if let Some(t) = token {
        s.push(' ');
        s.push_str(t);
    }
    s
}

/// Paramètres d'un `DCC ACCEPT` : `ACCEPT nom port position [jeton]`.
pub fn encode_accept(name: &str, port: u16, position: u64, token: Option<&str>) -> String {
    encode_resume(name, port, position, token).replacen("RESUME", "ACCEPT", 1)
}

/// Extensions jamais acceptées toutes seules (programmes, scripts, paquets, raccourcis), quels que soient les
/// réglages : Windows, macOS, Linux, Android, et les scripts de mIRC.
pub const DANGEROUS_EXTENSIONS: &[&str] = &[
    // Windows.
    "exe", "bat", "cmd", "com", "scr", "pif", "vbs", "vbe", "js", "jse", "wsf", "wsh", "msi", "msp", "cpl", "hta", "jar", "ps1", "psm1",
    "reg", "lnk", "inf", "dll", "url", "scf", "shs", // macOS.
    "app", "command", "tool", "pkg", "mpkg", "dmg", "workflow", "scpt", "terminal", "webloc", // Linux et Unix.
    "sh", "bash", "zsh", "csh", "ksh", "run", "appimage", "desktop", "deb", "rpm", "so", "dylib", "py", "pl", "rb", "php",
    // Android.
    "apk", "aab", "xapk", "apks", // mIRC et pages web.
    "mrc", "ini", "htm", "html", "svg",
];

/// Vrai si le fichier ne doit jamais être accepté sans demander (extension dangereuse, ou aucune extension
/// lisible après assainissement).
pub fn is_dangerous(name: &str) -> bool {
    let n = sanitize_filename(name).to_ascii_lowercase();
    let n = n.trim_end_matches(['.', ' ']);
    match n.rsplit_once('.') {
        Some((_, ext)) => DANGEROUS_EXTENSIONS.contains(&ext),
        None => false,
    }
}

/// Noms réservés de Windows (un fichier ne peut pas s'appeler ainsi).
const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4",
    "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Nom de fichier reçu rendu sûr : dernier élément du chemin seulement, sans caractère de contrôle, de
/// retournement de texte (U+202E…) ni caractère interdit (`<>:"|?*`), sans point au début (pas de fichier
/// caché) ni à la fin, 200 octets au plus (extension gardée), jamais vide (`file`).
pub fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let mut s: String = base
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
        })
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') { '_' } else { c })
        .collect();
    s = s.trim().trim_start_matches(['.', ' ']).trim_end_matches(['.', ' ']).to_string();
    if s.len() > 200 {
        let (stem, ext) = match s.rsplit_once('.') {
            Some((a, b)) if b.len() <= 16 && !a.is_empty() => (a.to_string(), format!(".{b}")),
            _ => (s.clone(), String::new()),
        };
        let mut cut = 200 - ext.len();
        while !stem.is_char_boundary(cut.min(stem.len())) {
            cut -= 1;
        }
        s = format!("{}{ext}", &stem[..cut.min(stem.len())]);
    }
    let stem = s.split('.').next().unwrap_or("").to_ascii_lowercase();
    if RESERVED.contains(&stem.as_str()) {
        s = format!("_{s}");
    }
    if s.is_empty() { "file".to_string() } else { s }
}

/// Nom libre dans un dossier : `nom.ext`, puis `nom (1).ext`, `nom (2).ext`…
pub fn unique_name(name: &str, exists: impl Fn(&str) -> bool) -> String {
    if !exists(name) {
        return name.to_string();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((a, b)) if !a.is_empty() => (a, format!(".{b}")),
        _ => (name, String::new()),
    };
    for i in 1..10_000 {
        let n = format!("{stem} ({i}){ext}");
        if !exists(&n) {
            return n;
        }
    }
    format!("{stem} ({}){ext}", u32::MAX)
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
        assert!(!o.is_passive());
        let o = parse_offer("Pat", "CHAT chat 2130706433 4000").unwrap();
        assert_eq!(o.kind, DccKind::Chat);
        assert_eq!(o.ip, Some("127.0.0.1".parse().unwrap()));
        assert!(parse_offer("Pat", "SEND x").is_none());
        assert_eq!(human_size(1536), "1.5KB");
    }

    #[test]
    fn unquoted_names_with_spaces() {
        let o = parse_offer("Pat", "SEND my holiday photo.jpg 3232235777 5000 2048").unwrap();
        assert_eq!(o.kind, DccKind::Send { file: "my holiday photo.jpg".into(), size: Some(2048) });
        assert_eq!(o.port, 5000);
        let o = parse_offer("Pat", "SEND two words.zip 3232235777 0 99 123").unwrap();
        assert_eq!(o.kind, DccKind::Send { file: "two words.zip".into(), size: Some(99) });
        assert_eq!(o.token.as_deref(), Some("123"));
        assert!(o.is_passive());
    }

    #[test]
    fn passive_and_ipv6() {
        let o = parse_offer("Pat", "SEND file.zip 3232235777 0 1000 77").unwrap();
        assert!(o.is_passive());
        assert_eq!(o.token.as_deref(), Some("77"));
        let o = parse_offer("Pat", "SEND file.zip 2001:db8::1 6000 1000").unwrap();
        assert_eq!(o.ip, Some("2001:db8::1".parse().unwrap()));
        assert_eq!(o.port, 6000);
        let o = parse_offer("Pat", "CHAT chat ::1 0 42").unwrap();
        assert!(o.is_passive());
        assert_eq!(o.ip, Some("::1".parse().unwrap()));
        // Réponse à une offre passive : port non nul et jeton.
        let o = parse_offer("Pat", "SEND file.zip 2130706433 7000 1000 77").unwrap();
        assert_eq!((o.port, o.token.as_deref()), (7000, Some("77")));
    }

    #[test]
    fn resume_accept() {
        let o = parse_offer("Pat", "RESUME file.zip 5000 1024").unwrap();
        assert_eq!(o.kind, DccKind::Resume { file: "file.zip".into(), position: 1024 });
        assert_eq!((o.port, o.token), (5000, None));
        let o = parse_offer("Pat", "ACCEPT \"a b.zip\" 0 2048 77").unwrap();
        assert_eq!(o.kind, DccKind::Accept { file: "a b.zip".into(), position: 2048 });
        assert_eq!((o.port, o.token.as_deref()), (0, Some("77")));
        let o = parse_offer("Pat", "RESUME a b.zip 0 2048 77").unwrap();
        assert_eq!(o.kind, DccKind::Resume { file: "a b.zip".into(), position: 2048 });
        assert_eq!(o.token.as_deref(), Some("77"));
        assert!(parse_offer("Pat", "RESUME x 5000").is_none());
    }

    #[test]
    fn encode_round_trip() {
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let s = encode_send("my file.txt", ip, 5000, 2048, None);
        assert_eq!(s, "SEND \"my file.txt\" 3232235777 5000 2048");
        let o = parse_offer("x", &s).unwrap();
        assert_eq!(o.kind, DccKind::Send { file: "my file.txt".into(), size: Some(2048) });
        let s = encode_send("a.zip", "::1".parse().unwrap(), 0, 10, Some("9"));
        assert_eq!(s, "SEND a.zip ::1 0 10 9");
        assert_eq!(encode_chat(ip, 4000, None), "CHAT chat 3232235777 4000");
        assert_eq!(encode_resume("a b", 0, 5, Some("9")), "RESUME \"a b\" 0 5 9");
        assert_eq!(encode_accept("a.zip", 5000, 5, None), "ACCEPT a.zip 5000 5");
        assert_eq!(wire_ip("::ffff:127.0.0.1".parse().unwrap()), "2130706433");
    }

    #[test]
    fn sanitize() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("C:\\Windows\\evil.exe"), "evil.exe");
        assert_eq!(sanitize_filename(".bashrc"), "bashrc");
        assert_eq!(sanitize_filename("..."), "file");
        assert_eq!(sanitize_filename(""), "file");
        assert_eq!(sanitize_filename("a\u{202E}gpj.exe"), "agpj.exe");
        assert_eq!(sanitize_filename("what?.txt"), "what_.txt");
        assert_eq!(sanitize_filename("nul.txt"), "_nul.txt");
        assert_eq!(sanitize_filename("photo de vacances.jpg "), "photo de vacances.jpg");
        let long = format!("{}.zip", "a".repeat(300));
        let s = sanitize_filename(&long);
        assert!(s.len() <= 200 && s.ends_with(".zip"));
    }

    #[test]
    fn dangerous() {
        for n in [
            "setup.exe",
            "run.BAT",
            "x.cmd",
            "a.com",
            "s.scr",
            "v.vbs",
            "j.js",
            "Foo.app",
            "i.sh",
            "t.command",
            "p.pkg",
            "d.dmg",
            "a.apk",
            "evil.exe.",
            "x.mrc",
        ] {
            assert!(is_dangerous(n), "{n}");
        }
        for n in ["photo.jpg", "notes.txt", "archive.zip", "README", "song.mp3"] {
            assert!(!is_dangerous(n), "{n}");
        }
        assert!(is_dangerous("photo\u{202E}gpj.exe"));
    }

    #[test]
    fn unique() {
        let taken = ["a.txt", "a (1).txt"];
        assert_eq!(unique_name("a.txt", |n| taken.contains(&n)), "a (2).txt");
        assert_eq!(unique_name("b.txt", |n| taken.contains(&n)), "b.txt");
        assert_eq!(unique_name("noext", |n| n == "noext"), "noext (1)");
    }
}
