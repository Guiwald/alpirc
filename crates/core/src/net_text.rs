//! Les échecs du réseau ([`NetError`], produits par `alpirc-net`) rédigés dans la langue de l'interface. Les
//! données (hôtes, adresses, chemins, textes du système ou de rustls) restent telles quelles.

use alpirc_proto::neterror::{CertProblem, IoKind, NetError, PemProblem, host_port, shows_addr};

use crate::i18n::{Key, Lang, trf};

/// Texte d'une erreur du réseau.
pub fn net_error(lang: Lang, e: &NetError) -> String {
    let t = |k: Key, args: &[(&str, &str)]| trf(lang, k, args);
    match e {
        NetError::ConnectTimeout { ms } => t(Key::NetConnectTimeout, &[("secs", &secs(lang, *ms))]),
        NetError::TlsTimeout { ms } => t(Key::NetTlsTimeout, &[("secs", &secs(lang, *ms))]),
        NetError::Dns { host, temporary: true, .. } => t(Key::NetDnsTemporary, &[("host", host)]),
        NetError::Dns { host, .. } => t(Key::NetDnsNotFound, &[("host", host)]),
        NetError::NoAddress { host } => t(Key::NetNoAddress, &[("host", host)]),
        NetError::NoAddressForBind { host, bind } => {
            t(Key::NetNoAddressForBind, &[("host", host), ("bind", &bind.to_string())])
        }
        NetError::Unreachable { host, port, addr, cause } => {
            let (target, cause) = (host_port(host, *port), io_kind(lang, cause));
            match addr {
                Some(a) if shows_addr(host, *port, a) => {
                    t(Key::NetUnreachableVia, &[("target", &target), ("addr", &a.to_string()), ("cause", &cause)])
                }
                _ => t(Key::NetUnreachable, &[("target", &target), ("cause", &cause)]),
            }
        }
        NetError::Io(k) => io_kind(lang, k),
        NetError::BadTlsName { host } => t(Key::NetBadTlsName, &[("host", host)]),
        NetError::CertRejected { host, problem } => {
            t(Key::NetCertRejected, &[("problem", &cert_problem(lang, host, problem))])
        }
        NetError::TlsAlert { alert } => t(Key::NetTlsAlert, &[("alert", alert)]),
        NetError::NoServerCert => t(Key::NetNoServerCert, &[]),
        NetError::Tls { detail } => t(Key::NetTls, &[("detail", detail)]),
        NetError::TlsIo(k) => t(Key::NetTls, &[("detail", &io_kind(lang, k))]),
        NetError::BadPin { pin } => t(Key::NetBadPin, &[("pin", pin)]),
        NetError::ClientCert(p) => t(Key::NetClientCert, &[("problem", &pem_problem(lang, p))]),
        NetError::TlsSetup { detail } => t(Key::NetTlsSetup, &[("detail", detail)]),
        NetError::Stopped => t(Key::NetStopped, &[]),
        NetError::DccListen { ip, cause } => {
            t(Key::NetDccListen, &[("ip", &ip.to_string()), ("cause", &io_kind(lang, cause))])
        }
        NetError::Timeout { ms } => t(Key::NetTimeout, &[("secs", &secs(lang, *ms))]),
        NetError::Cancelled => t(Key::NetCancelled, &[]),
        NetError::PeerClosed { pos, size } => {
            t(Key::NetPeerClosed, &[("pos", &pos.to_string()), ("size", &size.to_string())])
        }
        NetError::FileRead { path, cause } => {
            t(Key::NetFileRead, &[("path", &path.display().to_string()), ("cause", &io_kind(lang, cause))])
        }
        NetError::FileShrunk { path } => t(Key::NetFileShrunk, &[("path", &path.display().to_string())]),
        NetError::FileWrite { path, cause } => {
            t(Key::NetFileWrite, &[("path", &path.display().to_string()), ("cause", &io_kind(lang, cause))])
        }
        NetError::Other(s) => s.clone(),
    }
}

/// Le problème d'un certificat refusé (`host` : le nom attendu), sans le « certificat refusé : » du début : la
/// boîte « faire confiance à ce certificat » le montre seul.
pub fn cert_problem(lang: Lang, host: &str, p: &CertProblem) -> String {
    let t = |k: Key, args: &[(&str, &str)]| trf(lang, k, args);
    match p {
        CertProblem::Expired { not_after: None } => t(Key::CertExpired, &[]),
        CertProblem::Expired { not_after: Some(d) } => t(Key::CertExpiredOn, &[("date", &date(lang, *d))]),
        CertProblem::NotValidYet { not_before: None } => t(Key::CertNotYetValid, &[]),
        CertProblem::NotValidYet { not_before: Some(d) } => t(Key::CertNotYetValidFrom, &[("date", &date(lang, *d))]),
        CertProblem::UnknownIssuer => t(Key::CertUnknownIssuer, &[]),
        CertProblem::NameMismatch { presented } if presented.is_empty() => t(Key::CertNameMismatch, &[("host", host)]),
        CertProblem::NameMismatch { presented } => {
            t(Key::CertNameMismatchNames, &[("host", host), ("names", &presented.join(", "))])
        }
        CertProblem::BadSignature => t(Key::CertBadSignature, &[]),
        CertProblem::Revoked => t(Key::CertRevoked, &[]),
        CertProblem::BadEncoding => t(Key::CertBadEncoding, &[]),
        CertProblem::WrongPurpose => t(Key::CertWrongPurpose, &[]),
        CertProblem::UnsupportedAlgorithm => t(Key::CertUnsupportedAlgorithm, &[]),
        CertProblem::PinMismatch => t(Key::CertPinMismatch, &[]),
        CertProblem::Other(s) => s.clone(),
    }
}

/// Raison courte d'un échec pour la boîte du certificat refusé : le problème du certificat seul, sinon tout le
/// texte de l'erreur.
pub fn cert_reason(lang: Lang, e: &NetError) -> String {
    match e {
        NetError::CertRejected { host, problem } => cert_problem(lang, host, problem),
        other => net_error(lang, other),
    }
}

/// Sorte d'erreur d'entrée-sortie.
pub fn io_kind(lang: Lang, k: &IoKind) -> String {
    let key = match k {
        IoKind::Refused => Key::IoRefused,
        IoKind::Reset => Key::IoReset,
        IoKind::Aborted => Key::IoAborted,
        IoKind::BrokenPipe => Key::IoBrokenPipe,
        IoKind::TimedOut => Key::IoTimedOut,
        IoKind::HostUnreachable => Key::IoHostUnreachable,
        IoKind::NetworkUnreachable => Key::IoNetworkUnreachable,
        IoKind::AddrNotAvailable => Key::IoAddrNotAvailable,
        IoKind::AddrInUse => Key::IoAddrInUse,
        IoKind::NotFound => Key::IoNotFound,
        IoKind::PermissionDenied => Key::IoPermissionDenied,
        IoKind::StorageFull => Key::IoStorageFull,
        IoKind::Other(s) => return s.clone(),
    };
    trf(lang, key, &[])
}

fn pem_problem(lang: Lang, p: &PemProblem) -> String {
    match p {
        PemProblem::BadCert(d) => trf(lang, Key::NetPemBadCert, &[("detail", d)]),
        PemProblem::NoCert => trf(lang, Key::NetPemNoCert, &[]),
        PemProblem::BadKey(d) => trf(lang, Key::NetPemBadKey, &[("detail", d)]),
        PemProblem::Rejected(d) => d.clone(),
    }
}

/// Secondes, avec la virgule décimale des langues qui l'utilisent (`30`, `0,6`, `0.6`).
fn secs(lang: Lang, ms: u64) -> String {
    let s = alpirc_proto::neterror::secs_text(ms);
    match lang {
        Lang::En | Lang::Ga => s,
        _ => s.replace('.', ","),
    }
}

/// Date (UTC) d'un instant en secondes : `2100-01-01` en anglais, `01/01/2100` ou `01.01.2100` ailleurs.
fn date(lang: Lang, unix: i64) -> String {
    let fmt = match lang {
        Lang::En => "yyyy-mm-dd",
        Lang::De => "dd.mm.yyyy",
        _ => "dd/mm/yyyy",
    };
    crate::timeutil::format_time_in(unix.saturating_mul(1000), 0, fmt, lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> Option<std::net::SocketAddr> {
        Some(s.parse().unwrap())
    }

    fn samples() -> Vec<NetError> {
        let io = IoKind::Refused;
        let expired = CertProblem::Expired { not_after: Some(4_102_444_800) };
        vec![
            NetError::ConnectTimeout { ms: 30_000 },
            NetError::TlsTimeout { ms: 600 },
            NetError::Dns { host: "irc.x".into(), temporary: true, detail: "Temporary failure".into() },
            NetError::Dns { host: "irc.x".into(), temporary: false, detail: "Name or service not known".into() },
            NetError::NoAddress { host: "irc.x".into() },
            NetError::NoAddressForBind { host: "irc.x".into(), bind: "::1".parse().unwrap() },
            NetError::Unreachable { host: "irc.x".into(), port: 6697, addr: addr("192.0.2.1:6697"), cause: io.clone() },
            NetError::Unreachable { host: "127.0.0.1".into(), port: 1, addr: addr("127.0.0.1:1"), cause: io.clone() },
            NetError::Io(IoKind::Reset),
            NetError::BadTlsName { host: "a b".into() },
            NetError::CertRejected { host: "irc.x".into(), problem: CertProblem::UnknownIssuer },
            NetError::CertRejected { host: "irc.x".into(), problem: expired },
            NetError::CertRejected {
                host: "irc.x".into(),
                problem: CertProblem::NameMismatch { presented: vec!["a".into(), "b".into()] },
            },
            NetError::TlsAlert { alert: "HandshakeFailure".into() },
            NetError::NoServerCert,
            NetError::Tls { detail: "peer is incompatible".into() },
            NetError::TlsIo(IoKind::BrokenPipe),
            NetError::BadPin { pin: "abc".into() },
            NetError::ClientCert(PemProblem::BadKey("bad".into())),
            NetError::ClientCert(PemProblem::NoCert),
            NetError::TlsSetup { detail: "x".into() },
            NetError::Stopped,
            NetError::DccListen { ip: "0.0.0.0".parse().unwrap(), cause: IoKind::AddrInUse },
            NetError::Timeout { ms: 300 },
            NetError::Cancelled,
            NetError::PeerClosed { pos: 4000, size: 10_000 },
            NetError::FileRead { path: "/tmp/a".into(), cause: IoKind::NotFound },
            NetError::FileShrunk { path: "/tmp/a".into() },
            NetError::FileWrite { path: "/tmp/a".into(), cause: IoKind::StorageFull },
            NetError::Other("boom".into()),
        ]
    }

    #[test]
    fn english_matches_the_display_of_alpirc_net() {
        // L'anglais de l'interface est celui des journaux, sauf le texte du système pour la résolution.
        for e in samples() {
            if matches!(e, NetError::Dns { .. }) {
                continue;
            }
            assert_eq!(net_error(Lang::En, &e), e.to_string(), "{e:?}");
        }
    }

    #[test]
    fn each_language() {
        let e = NetError::Unreachable {
            host: "irc.undernet.org".into(),
            port: 6697,
            addr: Some("192.0.2.1:6697".parse().unwrap()),
            cause: IoKind::TimedOut,
        };
        assert_eq!(net_error(Lang::Fr, &e), "irc.undernet.org:6697 (192.0.2.1:6697) : délai dépassé");
        assert_eq!(net_error(Lang::De, &e), "irc.undernet.org:6697 (192.0.2.1:6697): Zeitüberschreitung");
        let tls = NetError::TlsTimeout { ms: 600 };
        assert_eq!(net_error(Lang::Fr, &tls), "délai dépassé pendant la négociation TLS (0,6 s)");
        assert!(net_error(Lang::Ga, &NetError::TlsTimeout { ms: 600 }).contains("0.6 s"));
        let problem = CertProblem::Expired { not_after: Some(4_102_444_800) };
        let c = NetError::CertRejected { host: "x".into(), problem };
        assert_eq!(net_error(Lang::Fr, &c), "certificat refusé : certificat expiré (valable jusqu'au 01/01/2100)");
        assert_eq!(net_error(Lang::De, &c), "Zertifikat abgelehnt: Zertifikat abgelaufen (gültig bis 01.01.2100)");
        assert_eq!(cert_reason(Lang::Es, &c), "certificado caducado (válido hasta el 01/01/2100)");
        // Aucune langue ne laisse de marque `{…}` ni ne retombe sur l'anglais.
        for l in Lang::ALL {
            for e in samples() {
                let s = net_error(l, &e);
                assert!(!s.contains('{'), "{} : {s}", l.code());
            }
        }
        for l in [Lang::Fr, Lang::Es, Lang::De, Lang::It, Lang::Va, Lang::Ga] {
            assert_ne!(net_error(l, &NetError::Io(IoKind::Refused)), "connection refused", "{}", l.code());
        }
    }
}
