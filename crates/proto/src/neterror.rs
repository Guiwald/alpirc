//! Échecs du réseau décrits par leurs données (hôte, port, adresse, délai, sorte d'erreur, problème de
//! certificat…), sans texte figé : `alpirc-net` les produit, `alpirc-core` les rédige dans la langue de
//! l'interface. [`fmt::Display`] les écrit en anglais, pour les journaux et le relais.

use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

/// Sorte d'une erreur d'entrée-sortie (connexion ou fichier).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IoKind {
    Refused,
    Reset,
    Aborted,
    BrokenPipe,
    TimedOut,
    HostUnreachable,
    NetworkUnreachable,
    /// Adresse locale indisponible (vhost).
    AddrNotAvailable,
    AddrInUse,
    /// Fichier ou dossier introuvable.
    NotFound,
    PermissionDenied,
    /// Disque plein.
    StorageFull,
    /// Autre erreur : le texte du système, tel quel.
    Other(String),
}

impl IoKind {
    /// Sorte d'une erreur d'entrée-sortie de la bibliothèque standard.
    pub fn of(e: &std::io::Error) -> IoKind {
        use std::io::ErrorKind::*;
        match e.kind() {
            ConnectionRefused => IoKind::Refused,
            ConnectionReset => IoKind::Reset,
            ConnectionAborted => IoKind::Aborted,
            BrokenPipe => IoKind::BrokenPipe,
            TimedOut => IoKind::TimedOut,
            HostUnreachable => IoKind::HostUnreachable,
            NetworkUnreachable => IoKind::NetworkUnreachable,
            AddrNotAvailable => IoKind::AddrNotAvailable,
            AddrInUse => IoKind::AddrInUse,
            NotFound => IoKind::NotFound,
            PermissionDenied => IoKind::PermissionDenied,
            StorageFull => IoKind::StorageFull,
            _ => IoKind::Other(e.to_string()),
        }
    }
}

impl From<&std::io::Error> for IoKind {
    fn from(e: &std::io::Error) -> IoKind {
        IoKind::of(e)
    }
}

impl fmt::Display for IoKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            IoKind::Refused => "connection refused",
            IoKind::Reset => "connection reset by peer",
            IoKind::Aborted => "connection aborted",
            IoKind::BrokenPipe => "connection broken",
            IoKind::TimedOut => "timed out",
            IoKind::HostUnreachable => "host unreachable",
            IoKind::NetworkUnreachable => "network unreachable",
            IoKind::AddrNotAvailable => "local address not available",
            IoKind::AddrInUse => "address already in use",
            IoKind::NotFound => "no such file or folder",
            IoKind::PermissionDenied => "permission denied",
            IoKind::StorageFull => "disk full",
            IoKind::Other(s) => s,
        })
    }
}

/// Pourquoi le certificat d'un serveur est refusé. Les dates sont en secondes depuis 1970 (UTC).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CertProblem {
    Expired { not_after: Option<i64> },
    NotValidYet { not_before: Option<i64> },
    /// Émetteur inconnu : certificat autosigné ou autorité non reconnue.
    UnknownIssuer,
    /// Le certificat ne porte pas le nom du serveur ; `presented` : les noms qu'il porte (s'ils sont connus).
    NameMismatch { presented: Vec<String> },
    BadSignature,
    Revoked,
    BadEncoding,
    /// Certificat non prévu pour un serveur.
    WrongPurpose,
    UnsupportedAlgorithm,
    /// Empreinte différente de l'empreinte retenue (mode épinglé).
    PinMismatch,
    /// Autre problème (nom de rustls, en anglais).
    Other(String),
}

impl CertProblem {
    /// Le problème en anglais, `host` : le nom attendu.
    pub fn describe(&self, host: &str) -> String {
        match self {
            CertProblem::Expired { not_after: None } => "certificate expired".into(),
            CertProblem::Expired { not_after: Some(t) } => {
                format!("certificate expired (valid until {})", iso_date(*t))
            }
            CertProblem::NotValidYet { not_before: None } => "certificate not valid yet".into(),
            CertProblem::NotValidYet { not_before: Some(t) } => {
                format!("certificate not valid yet (valid from {})", iso_date(*t))
            }
            CertProblem::UnknownIssuer => "unknown issuer (self-signed certificate or unrecognised authority)".into(),
            CertProblem::NameMismatch { presented } if presented.is_empty() => {
                format!("the certificate does not match the name '{host}'")
            }
            CertProblem::NameMismatch { presented } => format!(
                "the certificate does not match the name '{host}' (certificate names: {})",
                presented.join(", ")
            ),
            CertProblem::BadSignature => "invalid certificate signature".into(),
            CertProblem::Revoked => "certificate revoked".into(),
            CertProblem::BadEncoding => "malformed certificate".into(),
            CertProblem::WrongPurpose => "certificate not intended for a server".into(),
            CertProblem::UnsupportedAlgorithm => "unsupported certificate signature algorithm".into(),
            CertProblem::PinMismatch => "the certificate fingerprint does not match the stored fingerprint".into(),
            CertProblem::Other(s) => s.clone(),
        }
    }
}

/// Certificat client (PEM) inutilisable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PemProblem {
    /// Certificat PEM illisible (texte de l'analyseur, en anglais).
    BadCert(String),
    /// Aucun certificat dans le fichier PEM.
    NoCert,
    /// Clé privée PEM illisible (texte de l'analyseur).
    BadKey(String),
    /// Certificat et clé refusés par TLS (clé d'un autre certificat…) ; texte de rustls.
    Rejected(String),
}

impl fmt::Display for PemProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PemProblem::BadCert(d) => write!(f, "unreadable PEM certificate ({d})"),
            PemProblem::NoCert => f.write_str("no certificate in the PEM file"),
            PemProblem::BadKey(d) => write!(f, "unreadable PEM private key ({d})"),
            PemProblem::Rejected(d) => f.write_str(d),
        }
    }
}

/// Un échec du réseau : tentative de connexion, connexion établie, DCC.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NetError {
    /// Délai global de connexion dépassé (résolution et connexion TCP), en millisecondes.
    ConnectTimeout { ms: u64 },
    /// Délai dépassé pendant la négociation TLS.
    TlsTimeout { ms: u64 },
    /// Résolution du nom impossible ; `temporary` : échec passager (pas de réseau…) ; `detail` : texte du
    /// système.
    Dns { host: String, temporary: bool, detail: String },
    /// Le nom ne donne aucune adresse.
    NoAddress { host: String },
    /// Aucune adresse de la même famille que l'adresse locale imposée.
    NoAddressForBind { host: String, bind: IpAddr },
    /// Aucune adresse n'a répondu : la dernière essayée (`addr`, si un essai a eu lieu) et la cause.
    Unreachable { host: String, port: u16, addr: Option<SocketAddr>, cause: IoKind },
    /// Erreur d'entrée-sortie d'une connexion (établie ou en cours d'établissement).
    Io(IoKind),
    /// Nom d'hôte inutilisable pour TLS.
    BadTlsName { host: String },
    /// Certificat du serveur refusé.
    CertRejected { host: String, problem: CertProblem },
    /// Le serveur a interrompu la négociation TLS (nom de l'alerte, `HandshakeFailure`…).
    TlsAlert { alert: String },
    /// Le serveur n'a présenté aucun certificat.
    NoServerCert,
    /// Autre échec de la négociation TLS (texte de rustls, en anglais).
    Tls { detail: String },
    /// Erreur d'entrée-sortie pendant la négociation TLS.
    TlsIo(IoKind),
    /// Empreinte SHA-256 épinglée mal saisie.
    BadPin { pin: String },
    /// Certificat client inutilisable.
    ClientCert(PemProblem),
    /// Configuration TLS impossible (racines de confiance, versions) ; texte en anglais.
    TlsSetup { detail: String },
    /// Couche réseau arrêtée.
    Stopped,
    /// DCC : écoute impossible sur `ip`.
    DccListen { ip: IpAddr, cause: IoKind },
    /// DCC : délai dépassé (pair attendu, transfert immobile), en millisecondes.
    Timeout { ms: u64 },
    /// DCC : annulé.
    Cancelled,
    /// DCC : connexion fermée par l'autre avant la fin (position atteinte, taille annoncée).
    PeerClosed { pos: u64, size: u64 },
    /// DCC : lecture du fichier à envoyer.
    FileRead { path: PathBuf, cause: IoKind },
    /// DCC : le fichier a raccourci pendant l'envoi.
    FileShrunk { path: PathBuf },
    /// DCC : écriture du fichier reçu.
    FileWrite { path: PathBuf, cause: IoKind },
    /// Autre échec (texte en anglais).
    Other(String),
}

impl NetError {
    /// Erreur d'entrée-sortie d'une connexion.
    pub fn io(e: &std::io::Error) -> NetError {
        NetError::Io(IoKind::of(e))
    }
}

/// `host:port`, l'hôte entre crochets s'il s'agit d'une adresse IPv6.
pub fn host_port(host: &str, port: u16) -> String {
    if host.contains(':') { format!("[{host}]:{port}") } else { format!("{host}:{port}") }
}

/// L'adresse essayée vaut-elle d'être montrée à côté de `host:port` (nom résolu, pas l'adresse elle-même) ?
pub fn shows_addr(host: &str, port: u16, addr: &SocketAddr) -> bool {
    !(addr.port() == port && host.parse::<IpAddr>().is_ok_and(|ip| ip.to_canonical() == addr.ip().to_canonical()))
}

/// Durée en secondes, avec un point décimal si besoin (`30`, `0.6`, `1.25`).
pub fn secs_text(ms: u64) -> String {
    if ms.is_multiple_of(1000) {
        (ms / 1000).to_string()
    } else {
        let s = format!("{}.{:03}", ms / 1000, ms % 1000);
        s.trim_end_matches('0').to_string()
    }
}

/// Date UTC `AAAA-MM-JJ` d'un instant (secondes depuis 1970).
pub fn iso_date(unix: i64) -> String {
    let (y, m, d) = civil_from_days(unix.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Jour civil (année, mois, jour) d'un nombre de jours depuis 1970 (algorithme de H. Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetError::ConnectTimeout { ms } => write!(f, "connection timed out ({} s)", secs_text(*ms)),
            NetError::TlsTimeout { ms } => write!(f, "TLS handshake timed out ({} s)", secs_text(*ms)),
            NetError::Dns { host, temporary: true, detail } => {
                write!(f, "temporary failure resolving '{host}' ({detail})")
            }
            NetError::Dns { host, detail, .. } => write!(f, "address of '{host}' not found ({detail})"),
            NetError::NoAddress { host } => write!(f, "no address for '{host}'"),
            NetError::NoAddressForBind { host, bind } => {
                write!(f, "no address of '{host}' in the same family as the local address {bind}")
            }
            NetError::Unreachable { host, port, addr, cause } => match addr {
                Some(a) if shows_addr(host, *port, a) => write!(f, "{} ({a}): {cause}", host_port(host, *port)),
                _ => write!(f, "{}: {cause}", host_port(host, *port)),
            },
            NetError::Io(k) => write!(f, "{k}"),
            NetError::BadTlsName { host } => write!(f, "invalid host name for TLS: '{host}'"),
            NetError::CertRejected { host, problem } => write!(f, "certificate rejected: {}", problem.describe(host)),
            NetError::TlsAlert { alert } => write!(f, "the server aborted the TLS handshake (alert {alert})"),
            NetError::NoServerCert => f.write_str("the server presented no certificate"),
            NetError::Tls { detail } => write!(f, "TLS handshake: {detail}"),
            NetError::TlsIo(k) => write!(f, "TLS handshake: {k}"),
            NetError::BadPin { pin } => {
                write!(f, "invalid SHA-256 fingerprint '{pin}' (64 hexadecimal digits expected)")
            }
            NetError::ClientCert(p) => write!(f, "client certificate: {p}"),
            NetError::TlsSetup { detail } => write!(f, "TLS setup: {detail}"),
            NetError::Stopped => f.write_str("network layer stopped"),
            NetError::DccListen { ip, cause } => write!(f, "cannot listen for DCC on {ip}: {cause}"),
            NetError::Timeout { ms } => write!(f, "timed out ({} s)", secs_text(*ms)),
            NetError::Cancelled => f.write_str("cancelled"),
            NetError::PeerClosed { pos, size } => {
                write!(f, "connection closed by the other side at {pos} of {size} bytes")
            }
            NetError::FileRead { path, cause } => write!(f, "reading {}: {cause}", path.display()),
            NetError::FileShrunk { path } => write!(f, "{} got shorter during the transfer", path.display()),
            NetError::FileWrite { path, cause } => write!(f, "writing {}: {cause}", path.display()),
            NetError::Other(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for NetError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_texts() {
        let e = NetError::Unreachable {
            host: "irc.undernet.org".into(),
            port: 6697,
            addr: Some("192.0.2.1:6697".parse().unwrap()),
            cause: IoKind::TimedOut,
        };
        assert_eq!(e.to_string(), "irc.undernet.org:6697 (192.0.2.1:6697): timed out");
        let e = NetError::Unreachable {
            host: "127.0.0.1".into(),
            port: 1,
            addr: Some("127.0.0.1:1".parse().unwrap()),
            cause: IoKind::Refused,
        };
        assert_eq!(e.to_string(), "127.0.0.1:1: connection refused");
        let e = NetError::Unreachable { host: "::1".into(), port: 7, addr: None, cause: IoKind::TimedOut };
        assert_eq!(e.to_string(), "[::1]:7: timed out");
        assert_eq!(NetError::ConnectTimeout { ms: 30_000 }.to_string(), "connection timed out (30 s)");
        assert_eq!(NetError::TlsTimeout { ms: 600 }.to_string(), "TLS handshake timed out (0.6 s)");
        let p = NetError::PeerClosed { pos: 4000, size: 10_000 };
        assert_eq!(p.to_string(), "connection closed by the other side at 4000 of 10000 bytes");
        let problem = CertProblem::Expired { not_after: Some(4_102_444_800) };
        let c = NetError::CertRejected { host: "irc.x".into(), problem };
        assert_eq!(c.to_string(), "certificate rejected: certificate expired (valid until 2100-01-01)");
        let reset = std::io::Error::from(std::io::ErrorKind::ConnectionReset);
        assert_eq!(NetError::io(&reset).to_string(), "connection reset by peer");
    }

    #[test]
    fn helpers() {
        assert_eq!(secs_text(1250), "1.25");
        assert_eq!(secs_text(500), "0.5");
        assert_eq!(iso_date(0), "1970-01-01");
        assert_eq!(iso_date(951_782_400), "2000-02-29");
        assert!(!shows_addr("::1", 7, &"[::1]:7".parse().unwrap()));
        assert!(shows_addr("localhost", 7, &"[::1]:7".parse().unwrap()));
    }
}
