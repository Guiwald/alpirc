//! Les messages techniques suivent la langue de l'interface : échecs du réseau (connexion, fermeture, DCC,
//! certificat), erreurs et informations des scripts, textes par défaut d'une nouvelle installation.

mod common;

use alpirc_core::{Client, Settings, TransferState};
use alpirc_proto::neterror::{CertProblem, IoKind, NetError};
use alpirc_script::ScriptKind;
use common::*;

fn lang(code: &str) -> Settings {
    let mut s = settings();
    s.language = Some(code.into());
    s
}

fn refused() -> NetError {
    NetError::Unreachable {
        host: "irc.test.net".into(),
        port: 6667,
        addr: Some("192.0.2.7:6667".parse().unwrap()),
        cause: IoKind::Refused,
    }
}

#[test]
fn connection_failures_in_french_and_german() {
    let mut h = H::with(lang("fr"));
    h.c.connect(h.status, "TestNet", None, h.now);
    h.c.on_connect_failed(h.conn, &refused(), None, h.now);
    let want = "* Impossible de se connecter au serveur (irc.test.net:6667 (192.0.2.7:6667) : connexion refusée)";
    assert!(h.has_line(h.status, want), "{:#?}", h.status_lines());
    assert!(h.has_line(h.status, "* Nouvel essai dans 10 secondes (essai 1)…"), "{:#?}", h.status_lines());
    h.advance(10_000);
    h.c.on_connect_failed(h.conn, &NetError::TlsTimeout { ms: 30_000 }, None, h.now);
    let want = "* Impossible de se connecter au serveur (délai dépassé pendant la négociation TLS (30 s))";
    assert!(h.has_line(h.status, want), "{:#?}", h.status_lines());

    let mut h = H::with(lang("de"));
    h.c.connect(h.status, "TestNet", None, h.now);
    let addr = Some("127.0.0.1:1".parse().unwrap());
    let e = NetError::Unreachable { host: "127.0.0.1".into(), port: 1, addr, cause: IoKind::Refused };
    h.c.on_connect_failed(h.conn, &e, None, h.now);
    let want = "* Verbindung zum Server nicht möglich (127.0.0.1:1: Verbindung abgelehnt)";
    assert!(h.has_line(h.status, want), "{:#?}", h.status_lines());
    assert!(h.has_line(h.status, "* Neuer Versuch in 10 Sekunden (1. Versuch)..."), "{:#?}", h.status_lines());
}

#[test]
fn certificate_failure_keeps_the_fingerprint() {
    let mut h = H::with(lang("es"));
    h.c.connect(h.status, "TestNet", None, h.now);
    let e = NetError::CertRejected { host: "irc.test.net".into(), problem: CertProblem::UnknownIssuer };
    h.c.on_connect_failed(h.conn, &e, Some("ab12"), h.now);
    let want = "* No se puede conectar con el servidor (certificado rechazado: emisor desconocido (certificado \
                autofirmado o autoridad no reconocida) [SHA-256 ab12])";
    assert!(h.has_line(h.status, want), "{:#?}", h.status_lines());
    // La boîte « faire confiance » montre le problème seul.
    assert_eq!(h.c.cert_reason_text(&e), "emisor desconocido (certificado autofirmado o autoridad no reconocida)");
}

#[test]
fn closed_connections_and_ping_timeout() {
    let mut h = H::with(lang("it"));
    h.up();
    h.c.on_closed(h.conn, Some(&NetError::Io(IoKind::Reset)), h.now);
    assert!(h.has_line(h.status, "* Disconnesso (connessione reimpostata dall'altro capo)"), "{:#?}", h.status_lines());

    let mut h = H::with(lang("fr"));
    h.up();
    h.advance(240_000);
    assert!(h.has_line(h.status, "* Déconnecté (Délai de ping dépassé)"), "{:#?}", h.status_lines());
}

#[test]
fn dcc_failures_in_the_interface_language() {
    let dir = tmpdir();
    let mut s = lang("de");
    s.dcc.folder = Some(dir.join("downloads"));
    let mut h = H::with_dir(s, dir.clone());
    h.up();
    h.c.set_local_ip(h.conn, "192.168.1.10".parse().unwrap());
    let src = dir.join("c.txt");
    std::fs::write(&src, b"abc").unwrap();
    let id = h.c.dcc_send_file(h.conn, "Pat", &src, h.now).unwrap();
    h.c.on_dcc_ended(id, 0, Some(&NetError::PeerClosed { pos: 1, size: 3 }), h.now);
    let t = h.c.dcc_entry(id).unwrap();
    assert_eq!(t.state, TransferState::Failed);
    assert_eq!(t.error.as_deref(), Some("Verbindung von der Gegenseite bei 1 von 3 Bytes geschlossen"));
    assert!(h.status_lines().iter().any(|l| l.contains("Verbindung von der Gegenseite")), "{:#?}", h.status_lines());
}

#[test]
fn script_errors_and_infos_in_french() {
    let mut h = H::with(lang("fr"));
    h.up();
    h.input(h.status, "/hmake");
    assert_eq!(h.last(h.status), "* /hmake : paramètres insuffisants");
    h.input(h.status, "/hadd nope a 1");
    assert_eq!(h.last(h.status), "* /hadd : pas de table « nope »");
    h.input(h.status, "/timer1 0 60 echo -a x");
    assert_eq!(h.last(h.status), "* Timer 1 activé");
    h.input(h.status, "//echo -a $gettok(a.b)");
    assert_eq!(h.last(h.status), "* Paramètres invalides : $gettok");
    // Dans un script : la ligne et le fichier, dans la langue aussi.
    h.c.load_script_text("t.mrc", ScriptKind::Remote, "alias t {\n  noop\n  hmake\n}", h.now).unwrap();
    h.input(h.status, "/t");
    assert_eq!(h.last(h.status), "* /hmake : paramètres insuffisants (ligne 3, t.mrc)");
    // Erreur rendue au client : rédigée par lui.
    let e = h.c.load_script_text("u.mrc", ScriptKind::Remote, "alias u {\n  echo -a x\n", h.now).unwrap_err();
    assert_eq!(h.c.script_error_text(&e), "u.mrc:1 : « } » manquant");
}

#[test]
fn script_errors_in_german_and_irish() {
    let mut h = H::with(lang("de"));
    h.up();
    h.input(h.status, "/run notepad");
    assert_eq!(h.last(h.status), "* /run: von der Script-Sandbox nicht erlaubt");
    let mut h = H::with(lang("ga"));
    h.up();
    h.input(h.status, "/set %x 5");
    assert_eq!(h.last(h.status), "* Socraíodh %x go 5");
}

#[test]
fn new_install_defaults_in_the_interface_language() {
    let mut c = Client::new_localized(Settings::default(), tmpdir(), T0, Some("fr_FR.UTF-8"));
    c.localize_new_install();
    assert_eq!(c.settings().identity.realname, "Utilisateur d'AlpIRC");
    assert_eq!(c.settings().away.default_message, "Absent");
    // Des textes déjà choisis ne changent pas.
    let mut s = Settings::default();
    s.identity.realname = "Élisa de Lyon".into();
    let mut c = Client::new_localized(s, tmpdir(), T0, Some("de_DE"));
    c.localize_new_install();
    assert_eq!(c.settings().identity.realname, "Élisa de Lyon");
    assert_eq!(c.settings().away.default_message, "Abwesend");
    // En anglais : les textes d'origine.
    let mut c = Client::new_localized(Settings::default(), tmpdir(), T0, Some("en_US"));
    c.localize_new_install();
    assert_eq!(c.settings().identity.realname, "AlpIRC user");
    assert_eq!(c.settings().away.default_message, "Away");
}
