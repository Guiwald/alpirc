//! Essais d'intégration de la couche réseau, tous en local (127.0.0.1).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener as StdListener, TcpStream as StdStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use alpirc_net::{
    CertProblem, FIRST_ACCEPTED_ID, IoKind, MAX_LINE, Net, NetError, NetEvent, PemProblem, ServerTls, Target, Verify,
    client_config, fingerprint, generate_self_signed,
};

const WAIT: Duration = Duration::from_secs(10);

/// Une couche réseau dont les événements arrivent dans un canal.
struct Probe {
    net: Net,
    rx: mpsc::Receiver<NetEvent>,
}

impl Probe {
    fn new() -> Probe {
        let (tx, rx) = mpsc::channel();
        let net = Net::new(Box::new(move |ev| {
            let _ = tx.send(ev);
        }));
        Probe { net, rx }
    }

    fn next(&self) -> NetEvent {
        self.rx.recv_timeout(WAIT).expect("événement attendu")
    }

    /// Aucun événement pendant `d`.
    fn quiet(&self, d: Duration) {
        if let Ok(ev) = self.rx.recv_timeout(d) {
            panic!("événement inattendu : {ev:?}");
        }
    }

    fn line(&self, conn: u32) -> Vec<u8> {
        match self.next() {
            NetEvent::Line { conn: c, bytes } if c == conn => bytes,
            ev => panic!("ligne attendue sur {conn}, reçu {ev:?}"),
        }
    }

    fn connected(&self, conn: u32) -> (SocketAddr, SocketAddr, Option<alpirc_net::TlsInfo>) {
        match self.next() {
            NetEvent::Connected { conn: c, peer, local, tls } if c == conn => (peer, local, tls),
            ev => panic!("Connected attendu sur {conn}, reçu {ev:?}"),
        }
    }

    fn accepted(&self, listener: u32) -> (u32, SocketAddr, bool) {
        match self.next() {
            NetEvent::Accepted { listener: l, conn, peer, tls } if l == listener => (conn, peer, tls),
            ev => panic!("Accepted attendu sur {listener}, reçu {ev:?}"),
        }
    }

    fn closed(&self, conn: u32) -> Option<NetError> {
        match self.next() {
            NetEvent::Closed { conn: c, error } if c == conn => error,
            ev => panic!("Closed attendu sur {conn}, reçu {ev:?}"),
        }
    }

    fn failed(&self, conn: u32) -> (NetError, Option<String>) {
        match self.next() {
            NetEvent::Failed { conn: c, error, cert_sha256 } if c == conn => (error, cert_sha256),
            ev => panic!("Failed attendu sur {conn}, reçu {ev:?}"),
        }
    }
}

fn localhost() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

/// Serveur d'écoute en clair + client connecté : (serveur, client, port, id de la connexion acceptée).
fn plain_pair(client_id: u32) -> (Probe, Probe, u16, u32) {
    let srv = Probe::new();
    let addr = srv.net.listen(1, localhost(), None).unwrap();
    assert_ne!(addr.port(), 0);
    let cli = Probe::new();
    cli.net.connect(client_id, Target::new("127.0.0.1", addr.port(), false));
    let (peer, local, tls) = cli.connected(client_id);
    assert_eq!(peer, addr);
    assert!(tls.is_none());
    let (sconn, speer, stls) = srv.accepted(1);
    assert!(sconn >= FIRST_ACCEPTED_ID);
    assert_eq!(speer, local);
    assert!(!stls);
    (srv, cli, addr.port(), sconn)
}

#[test]
fn plain_exchange_both_ways_in_order() {
    let (srv, cli, _, sconn) = plain_pair(7);
    assert!(cli.net.is_connected(7));
    assert!(srv.net.is_connected(sconn));

    cli.net.send(7, b"NICK alice".to_vec());
    cli.net.send(7, b"USER alice 0 * :Alice".to_vec());
    // Un CRLF fourni par l'appelant n'est pas doublé ; une ligne vide n'est pas envoyée.
    cli.net.send(7, b"PING :x\r\n".to_vec());
    cli.net.send(7, Vec::new());
    for i in 0..500 {
        cli.net.send(7, format!("PRIVMSG #c :{i}").into_bytes());
    }
    assert_eq!(srv.line(sconn), b"NICK alice");
    assert_eq!(srv.line(sconn), b"USER alice 0 * :Alice");
    assert_eq!(srv.line(sconn), b"PING :x");
    for i in 0..500 {
        assert_eq!(srv.line(sconn), format!("PRIVMSG #c :{i}").into_bytes());
    }

    srv.net.send(sconn, b":srv 001 alice :Bienvenue".to_vec());
    srv.net.send(sconn, "PRIVMSG alice :héhé \u{1F600}".as_bytes().to_vec());
    srv.net.send(sconn, vec![0xff, 0xfe, b'x']); // octets bruts, pas forcément UTF-8
    assert_eq!(cli.line(7), b":srv 001 alice :Bienvenue");
    assert_eq!(cli.line(7), "PRIVMSG alice :héhé \u{1F600}".as_bytes());
    assert_eq!(cli.line(7), vec![0xff, 0xfe, b'x']);

    // Fermeture : les lignes en file partent d'abord, puis Closed des deux côtés.
    cli.net.send(7, b"QUIT :bye".to_vec());
    cli.net.close(7);
    assert_eq!(cli.closed(7), None);
    assert!(!cli.net.is_connected(7));
    assert_eq!(srv.line(sconn), b"QUIT :bye");
    assert_eq!(srv.closed(sconn), None);
    assert!(!srv.net.is_connected(sconn));

    // Après la fin : écritures ignorées, aucun événement.
    cli.net.send(7, b"ignored".to_vec());
    cli.net.close(7);
    cli.quiet(Duration::from_millis(200));
    srv.quiet(Duration::from_millis(50));
}

#[test]
fn server_close_reaches_client() {
    let (srv, cli, _, sconn) = plain_pair(3);
    srv.net.send(sconn, b"ERROR :Closing link".to_vec());
    srv.net.close(sconn);
    assert_eq!(srv.closed(sconn), None);
    assert_eq!(cli.line(3), b"ERROR :Closing link");
    assert_eq!(cli.closed(3), None);
}

#[test]
fn line_splitting_crlf_empty_and_truncation() {
    let srv = Probe::new();
    let addr = srv.net.listen(2, localhost(), None).unwrap();
    let mut raw = StdStream::connect(addr).unwrap();
    let (sconn, _, _) = srv.accepted(2);

    let mut data = Vec::new();
    data.extend_from_slice(b"one\r\ntwo\n\r\n\n\r\nthree\r\n");
    data.extend(std::iter::repeat_n(b'a', MAX_LINE + 5000));
    data.extend_from_slice(b"\r\n");
    data.extend(std::iter::repeat_n(b'b', MAX_LINE));
    data.extend_from_slice(b"\r\n");
    data.extend_from_slice(b"after\r\nmid");
    raw.write_all(&data).unwrap();
    raw.flush().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    // Ligne coupée entre deux paquets, puis dernière ligne sans LF avant la fin du flux.
    raw.write_all(b"dle\r\ntail-without-lf").unwrap();
    raw.shutdown(std::net::Shutdown::Write).unwrap();

    assert_eq!(srv.line(sconn), b"one");
    assert_eq!(srv.line(sconn), b"two");
    assert_eq!(srv.line(sconn), b"three");
    let long = srv.line(sconn);
    assert_eq!(long.len(), MAX_LINE);
    assert!(long.iter().all(|&b| b == b'a'));
    let exact = srv.line(sconn);
    assert_eq!(exact.len(), MAX_LINE);
    assert!(exact.iter().all(|&b| b == b'b'));
    assert_eq!(srv.line(sconn), b"after");
    assert_eq!(srv.line(sconn), b"middle");
    assert_eq!(srv.line(sconn), b"tail-without-lf");
    assert_eq!(srv.closed(sconn), None);
}

#[test]
fn written_bytes_are_crlf_terminated_and_cut_at_newlines() {
    let raw = StdListener::bind(localhost()).unwrap();
    let port = raw.local_addr().unwrap().port();
    let cli = Probe::new();
    cli.net.connect(1, Target::new("127.0.0.1", port, false));
    let (mut s, _) = raw.accept().unwrap();
    cli.connected(1);
    cli.net.send(1, b"PRIVMSG #a :hello".to_vec());
    cli.net.send(1, b"PRIVMSG #a :one\r\nQUIT :injected".to_vec());
    cli.net.send(1, b"\r\n".to_vec());
    cli.net.close(1);
    // Le client vide sa file puis envoie FIN ; il attend la fin du flux du serveur (2 s au plus).
    let mut got = Vec::new();
    s.set_read_timeout(Some(WAIT)).unwrap();
    s.read_to_end(&mut got).unwrap();
    assert_eq!(got, b"PRIVMSG #a :hello\r\nPRIVMSG #a :one\r\n");
    s.write_all(b"ERROR :Closing link\r\n").unwrap();
    let t0 = Instant::now();
    drop(s);
    assert_eq!(cli.line(1), b"ERROR :Closing link");
    assert_eq!(cli.closed(1), None);
    assert!(t0.elapsed() < Duration::from_secs(1));
}

#[test]
fn peer_reset_is_reported() {
    let raw = StdListener::bind(localhost()).unwrap();
    let port = raw.local_addr().unwrap().port();
    let cli = Probe::new();
    cli.net.connect(1, Target::new("127.0.0.1", port, false));
    let (s, _) = raw.accept().unwrap();
    cli.connected(1);
    // SO_LINGER à 0 : la fermeture envoie un RST.
    socket2::SockRef::from(&s).set_linger(Some(Duration::ZERO)).unwrap();
    drop(s);
    let err = cli.closed(1);
    assert_eq!(err, Some(NetError::Io(IoKind::Reset)));
    assert_eq!(err.unwrap().to_string(), "connection reset by peer");
}

#[test]
fn connect_refused_fails_quickly() {
    let port = {
        let l = StdListener::bind(localhost()).unwrap();
        l.local_addr().unwrap().port()
    };
    let cli = Probe::new();
    let t0 = Instant::now();
    cli.net.connect(9, Target::new("127.0.0.1", port, false));
    let (err, cert) = cli.failed(9);
    assert!(t0.elapsed() < Duration::from_secs(3));
    let addr = Some(SocketAddr::new("127.0.0.1".parse().unwrap(), port));
    assert_eq!(err, NetError::Unreachable { host: "127.0.0.1".into(), port, addr, cause: IoKind::Refused });
    assert_eq!(err.to_string(), format!("127.0.0.1:{port}: connection refused"));
    assert!(cert.is_none());
    assert!(!cli.net.is_connected(9));
    // Les envois sur une connexion échouée sont ignorés.
    cli.net.send(9, b"x".to_vec());
    cli.quiet(Duration::from_millis(100));
}

#[test]
fn unresolvable_host_fails() {
    let cli = Probe::new();
    let mut t = Target::new("nonexistent.invalid", 6667, false);
    t.connect_timeout = Duration::from_secs(3);
    cli.net.connect(1, t);
    let (err, _) = cli.failed(1);
    assert!(
        matches!(&err, NetError::Dns { host, .. } if host == "nonexistent.invalid")
            || matches!(err, NetError::ConnectTimeout { .. }),
        "{err:?}"
    );
}

#[test]
fn bind_family_mismatch_fails() {
    let cli = Probe::new();
    let mut t = Target::new("127.0.0.1", 6667, false);
    t.bind = Some("::1".parse().unwrap());
    cli.net.connect(1, t);
    let (err, _) = cli.failed(1);
    assert_eq!(err, NetError::NoAddressForBind { host: "127.0.0.1".into(), bind: "::1".parse().unwrap() });
}

#[test]
fn bind_local_address() {
    let srv = Probe::new();
    let addr = srv.net.listen(1, localhost(), None).unwrap();
    let cli = Probe::new();
    let mut t = Target::new("localhost", addr.port(), false);
    t.bind = Some("127.0.0.1".parse().unwrap());
    cli.net.connect(1, t);
    let (peer, local, _) = cli.connected(1);
    assert_eq!(peer, addr);
    assert_eq!(local.ip(), "127.0.0.1".parse::<std::net::IpAddr>().unwrap());
}

/// TLS demandé vers un serveur qui accepte TCP mais ne répond jamais : délai global dépassé.
#[test]
fn tls_handshake_timeout() {
    let raw = StdListener::bind(localhost()).unwrap();
    let port = raw.local_addr().unwrap().port();
    let cli = Probe::new();
    let mut t = Target::new("127.0.0.1", port, true);
    t.verify = Verify::AcceptInvalid;
    t.connect_timeout = Duration::from_millis(600);
    let t0 = Instant::now();
    cli.net.connect(4, t);
    let (err, cert) = cli.failed(4);
    assert!(t0.elapsed() >= Duration::from_millis(500), "{:?}", t0.elapsed());
    assert!(t0.elapsed() < Duration::from_secs(5));
    assert_eq!(err, NetError::TlsTimeout { ms: 600 });
    assert_eq!(err.to_string(), "TLS handshake timed out (0.6 s)");
    assert!(cert.is_none());
    drop(raw);
}

/// close() pendant la tentative : Closed sans erreur, rien d'autre ensuite.
#[test]
fn close_during_attempt() {
    let raw = StdListener::bind(localhost()).unwrap();
    let port = raw.local_addr().unwrap().port();
    let cli = Probe::new();
    let mut t = Target::new("127.0.0.1", port, true);
    t.verify = Verify::AcceptInvalid;
    cli.net.connect(4, t);
    std::thread::sleep(Duration::from_millis(200));
    assert!(!cli.net.is_connected(4));
    cli.net.close(4);
    assert_eq!(cli.closed(4), None);
    cli.quiet(Duration::from_millis(200));
    drop(raw);
}

#[test]
fn reconnect_with_same_id_closes_previous() {
    let (srv, cli, port, first) = plain_pair(5);
    cli.net.connect(5, Target::new("127.0.0.1", port, false));
    // L'ancienne connexion se termine avant que la nouvelle n'émette quoi que ce soit.
    assert_eq!(cli.closed(5), None);
    cli.connected(5);
    let mut closed_first = false;
    let mut second = None;
    for _ in 0..2 {
        match srv.next() {
            NetEvent::Closed { conn, error: None } if conn == first => closed_first = true,
            NetEvent::Accepted { conn, .. } => second = Some(conn),
            ev => panic!("inattendu : {ev:?}"),
        }
    }
    assert!(closed_first);
    let second = second.unwrap();
    assert_ne!(second, first);
    assert!(!srv.net.is_connected(first));
    cli.net.send(5, b"hello again".to_vec());
    assert_eq!(srv.line(second), b"hello again");

    // Remplacer une tentative qui échoue : Failed de l'ancienne, puis la nouvelle.
    let dead = {
        let l = StdListener::bind(localhost()).unwrap();
        l.local_addr().unwrap().port()
    };
    cli.net.connect(6, Target::new("127.0.0.1", dead, false));
    cli.net.connect(6, Target::new("127.0.0.1", port, false));
    match cli.next() {
        NetEvent::Failed { conn: 6, .. } | NetEvent::Closed { conn: 6, error: None } => {}
        ev => panic!("fin de la première tentative attendue, reçu {ev:?}"),
    }
    cli.connected(6);
}

#[test]
fn listener_stop_and_restart_on_same_port() {
    let srv = Probe::new();
    let addr = srv.net.listen(1, localhost(), None).unwrap();
    let cli = Probe::new();
    cli.net.connect(1, Target::new("127.0.0.1", addr.port(), false));
    cli.connected(1);
    let (sconn, _, _) = srv.accepted(1);

    srv.net.stop_listener(1);
    match srv.next() {
        NetEvent::ListenerClosed { listener: 1, error: None } => {}
        ev => panic!("ListenerClosed attendu, reçu {ev:?}"),
    }
    // Les connexions acceptées continuent.
    cli.net.send(1, b"still here".to_vec());
    assert_eq!(srv.line(sconn), b"still here");
    // Plus personne n'écoute.
    cli.net.connect(2, Target::new("127.0.0.1", addr.port(), false));
    cli.failed(2);
    // Même port, de nouveau (le port est libéré au retour de stop_listener).
    let again = srv.net.listen(1, addr, None).unwrap();
    assert_eq!(again, addr);
    cli.net.connect(3, Target::new("127.0.0.1", addr.port(), false));
    cli.connected(3);
    srv.accepted(1);
    // Écouter deux fois sur le même port avec un autre identifiant échoue proprement.
    let err = srv.net.listen(2, addr, None).unwrap_err();
    assert!(err.contains("utilisée"), "{err}");
}

#[test]
fn shutdown_closes_everything_then_silence() {
    let (srv, cli, port, sconn) = plain_pair(1);
    cli.net.send(1, b"last words".to_vec());
    cli.net.shutdown();
    assert_eq!(cli.closed(1), None);
    assert_eq!(srv.line(sconn), b"last words");
    assert_eq!(srv.closed(sconn), None);
    cli.net.connect(2, Target::new("127.0.0.1", port, false));
    cli.net.send(2, b"x".to_vec());
    assert!(!cli.net.is_connected(1));
    assert!(cli.net.listen(9, localhost(), None).is_err());
    cli.quiet(Duration::from_millis(200));
    cli.net.shutdown();
    srv.net.shutdown();
    match srv.next() {
        NetEvent::ListenerClosed { listener: 1, error: None } => {}
        ev => panic!("ListenerClosed attendu, reçu {ev:?}"),
    }
}

// ——— TLS ———

struct TlsServer {
    srv: Probe,
    port: u16,
    fp: String,
}

fn tls_server() -> TlsServer {
    let (cert, key, fp) = generate_self_signed(&["localhost".into(), "127.0.0.1".into()]).unwrap();
    assert_eq!(fp.len(), 64);
    let srv = Probe::new();
    let addr = srv.net.listen(1, localhost(), Some(ServerTls { cert_chain_pem: cert, key_pem: key })).unwrap();
    TlsServer { srv, port: addr.port(), fp }
}

fn tls_target(host: &str, port: u16, verify: Verify) -> Target {
    let mut t = Target::new(host, port, true);
    t.verify = verify;
    t
}

#[test]
fn tls_pinned_with_separators_succeeds() {
    let s = tls_server();
    let pretty: Vec<String> = s.fp.as_bytes().chunks(2).map(|c| String::from_utf8_lossy(c).to_uppercase()).collect();
    let cli = Probe::new();
    cli.net.connect(1, tls_target("localhost", s.port, Verify::Pinned(pretty.join(":"))));
    let (_, _, tls) = cli.connected(1);
    let tls = tls.expect("informations TLS");
    assert_eq!(tls.protocol, "TLSv1.3");
    assert!(tls.cipher.starts_with("TLS13_"), "{}", tls.cipher);
    assert_eq!(tls.cert_sha256, s.fp);
    assert!(!tls.verified);
    let (sconn, _, stls) = s.srv.accepted(1);
    assert!(stls);

    cli.net.send(1, b"CAP LS 302".to_vec());
    assert_eq!(s.srv.line(sconn), b"CAP LS 302");
    s.srv.net.send(sconn, b":srv CAP * LS :sasl".to_vec());
    assert_eq!(cli.line(1), b":srv CAP * LS :sasl");
    cli.net.close(1);
    assert_eq!(cli.closed(1), None);
    // close_notify reçu : fin propre côté serveur.
    assert_eq!(s.srv.closed(sconn), None);
}

#[test]
fn tls_accept_invalid_succeeds() {
    let s = tls_server();
    let cli = Probe::new();
    cli.net.connect(1, tls_target("127.0.0.1", s.port, Verify::AcceptInvalid));
    let (_, _, tls) = cli.connected(1);
    let tls = tls.unwrap();
    assert_eq!(tls.cert_sha256, s.fp);
    assert!(!tls.verified);
    let (sconn, _, _) = s.srv.accepted(1);
    // Fermeture par le serveur en TLS.
    s.srv.net.send(sconn, b"ERROR :bye".to_vec());
    s.srv.net.close(sconn);
    assert_eq!(cli.line(1), b"ERROR :bye");
    assert_eq!(cli.closed(1), None);
}

#[test]
fn tls_full_verification_rejects_self_signed_with_fingerprint() {
    let s = tls_server();
    let cli = Probe::new();
    for (id, host) in [(1, "localhost"), (2, "127.0.0.1")] {
        cli.net.connect(id, tls_target(host, s.port, Verify::Full));
        let (err, cert) = cli.failed(id);
        assert_eq!(err, NetError::CertRejected { host: host.into(), problem: CertProblem::UnknownIssuer });
        assert_eq!(cert.as_deref(), Some(s.fp.as_str()));
    }
    // Le serveur n'a vu aucune connexion aboutir.
    s.srv.quiet(Duration::from_millis(200));
}

#[test]
fn tls_wrong_pin_fails_with_fingerprint() {
    let s = tls_server();
    let cli = Probe::new();
    cli.net.connect(1, tls_target("localhost", s.port, Verify::Pinned("00".repeat(32))));
    let (err, cert) = cli.failed(1);
    assert_eq!(err, NetError::CertRejected { host: "localhost".into(), problem: CertProblem::PinMismatch });
    assert_eq!(cert.as_deref(), Some(s.fp.as_str()));
    // Empreinte mal saisie : échec immédiat, sans connexion.
    cli.net.connect(2, tls_target("localhost", s.port, Verify::Pinned("abc".into())));
    let (err, cert) = cli.failed(2);
    assert_eq!(err, NetError::BadPin { pin: "abc".into() });
    assert!(cert.is_none());
}

#[test]
fn tls_to_plain_server_fails() {
    let srv = Probe::new();
    let addr = srv.net.listen(1, localhost(), None).unwrap();
    let cli = Probe::new();
    cli.net.connect(1, tls_target("127.0.0.1", addr.port(), Verify::AcceptInvalid));
    // Le serveur en clair reçoit le ClientHello comme une « ligne » et peut répondre n'importe quoi ;
    // on ferme côté serveur pour que la négociation échoue.
    let (sconn, _, _) = srv.accepted(1);
    srv.net.send(sconn, b"NOTICE * :not tls".to_vec());
    srv.net.close(sconn);
    let (err, cert) = cli.failed(1);
    assert!(matches!(err, NetError::Tls { .. } | NetError::TlsIo(_) | NetError::TlsAlert { .. }), "{err:?}");
    assert!(err.to_string().contains("TLS"), "{err}");
    assert!(cert.is_none());
}

#[test]
fn plain_client_to_tls_server_gets_nothing() {
    let s = tls_server();
    let mut raw = StdStream::connect(("127.0.0.1", s.port)).unwrap();
    raw.write_all(b"NICK x\r\nUSER x 0 * :x\r\n").unwrap();
    raw.set_read_timeout(Some(WAIT)).unwrap();
    let mut buf = Vec::new();
    let _ = raw.read_to_end(&mut buf);
    // Pas d'Accepted pour un client qui ne parle pas TLS.
    s.srv.quiet(Duration::from_millis(200));
}

#[test]
fn bad_server_cert_is_rejected_by_listen() {
    let srv = Probe::new();
    let err = srv
        .net
        .listen(1, localhost(), Some(ServerTls { cert_chain_pem: b"nope".to_vec(), key_pem: b"nope".to_vec() }))
        .unwrap_err();
    assert!(err.contains("certificat"), "{err}");
    // Clé d'un autre certificat.
    let (cert, _, _) = generate_self_signed(&["a".into()]).unwrap();
    let (_, key, _) = generate_self_signed(&["b".into()]).unwrap();
    let err = srv.net.listen(1, localhost(), Some(ServerTls { cert_chain_pem: cert, key_pem: key })).unwrap_err();
    assert!(err.contains("certificat du serveur"), "{err}");
}

#[test]
fn client_config_modes() {
    assert!(client_config(&Verify::Full, None).is_ok());
    assert!(client_config(&Verify::AcceptInvalid, None).is_ok());
    let fp = fingerprint(b"abc");
    assert_eq!(fp, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    let colons: Vec<String> = fp.as_bytes().chunks(2).map(|c| String::from_utf8_lossy(c).into_owned()).collect();
    assert!(client_config(&Verify::Pinned(colons.join(":")), None).is_ok());
    assert!(client_config(&Verify::Pinned(fp.to_uppercase()), None).is_ok());
    assert!(client_config(&Verify::Pinned("xyz".into()), None).is_err());
    let (cert, key, _) = generate_self_signed(&["me".into()]).unwrap();
    assert!(client_config(&Verify::Full, Some((&cert, &key))).is_ok());
    let err = client_config(&Verify::Full, Some((&cert, b"garbage"))).unwrap_err();
    assert!(matches!(err, NetError::ClientCert(PemProblem::BadKey(_))), "{err:?}");
    assert!(err.to_string().starts_with("client certificate: "), "{err}");
}

/// Certificat client (SASL EXTERNAL / CertFP) : un serveur rustls qui exige un certificat client reçoit
/// bien celui de la cible, avec une clé PKCS#8 puis SEC1.
#[test]
fn client_certificate_is_presented() {
    use base64::Engine as _;
    use std::sync::Arc;

    let (scert, skey, sfp) = generate_self_signed(&["localhost".into()]).unwrap();
    let (ccert, ckey_pkcs8, cfp) = generate_self_signed(&["alpirc-client".into()]).unwrap();

    // Même clé en SEC1 (« EC PRIVATE KEY »).
    let pkcs8_der = {
        let body: String =
            String::from_utf8(ckey_pkcs8.clone()).unwrap().lines().filter(|l| !l.starts_with("-----")).collect();
        base64::engine::general_purpose::STANDARD.decode(body).unwrap()
    };
    let key =
        graviola::signing::ecdsa::SigningKey::<graviola::signing::ecdsa::P256>::from_pkcs8_der(&pkcs8_der).unwrap();
    let mut buf = [0u8; 256];
    let sec1 = key.to_sec1_der(&mut buf).unwrap();
    let ckey_sec1 = format!(
        "-----BEGIN EC PRIVATE KEY-----\n{}\n-----END EC PRIVATE KEY-----\n",
        base64::engine::general_purpose::STANDARD.encode(sec1)
    )
    .into_bytes();

    // Serveur rustls (graviola) qui exige un certificat client signé par… le certificat client lui-même.
    let provider = Arc::new(rustls_graviola::default_provider());
    let mut roots = rustls::RootCertStore::empty();
    use rustls::pki_types::pem::PemObject;
    let client_der = rustls::pki_types::CertificateDer::from_pem_slice(&ccert).unwrap();
    roots.add(client_der).unwrap();
    let verifier =
        rustls::server::WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider.clone()).build().unwrap();
    let server_cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_client_cert_verifier(verifier)
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from_pem_slice(&scert).unwrap()],
            rustls::pki_types::PrivateKeyDer::from_pem_slice(&skey).unwrap(),
        )
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_cfg));

    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let port = listener.local_addr().unwrap().port();
    let (seen_tx, seen_rx) = mpsc::channel();
    rt.spawn(async move {
        use tokio::io::AsyncWriteExt;
        for _ in 0..2 {
            let (sock, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            let seen_tx = seen_tx.clone();
            tokio::spawn(async move {
                let mut tls = acceptor.accept(sock).await.unwrap();
                let fp = tls.get_ref().1.peer_certificates().map(|c| fingerprint(c[0].as_ref()));
                let _ = seen_tx.send(fp);
                tls.write_all(b"OK\r\n").await.unwrap();
                tls.shutdown().await.unwrap();
            });
        }
    });

    let cli = Probe::new();
    for (id, key) in [(1, ckey_pkcs8), (2, ckey_sec1)] {
        let mut t = tls_target("localhost", port, Verify::Pinned(sfp.clone()));
        t.client_cert = Some((ccert.clone(), key));
        cli.net.connect(id, t);
        cli.connected(id);
        assert_eq!(cli.line(id), b"OK");
        assert_eq!(cli.closed(id), None);
        assert_eq!(seen_rx.recv_timeout(WAIT).unwrap().as_deref(), Some(cfp.as_str()));
    }
    drop(cli);
    rt.shutdown_timeout(Duration::from_secs(1));
}

/// Serveur limité à TLS 1.2 (serveurs IRC anciens) : la négociation aboutit et le rapporte.
#[test]
fn tls12_server() {
    use rustls::pki_types::pem::PemObject;
    use std::sync::Arc;

    let (cert, key, fp) = generate_self_signed(&["localhost".into()]).unwrap();
    let cfg = rustls::ServerConfig::builder_with_provider(Arc::new(rustls_graviola::default_provider()))
        .with_protocol_versions(&[&rustls::version::TLS12])
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from_pem_slice(&cert).unwrap()],
            rustls::pki_types::PrivateKeyDer::from_pem_slice(&key).unwrap(),
        )
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(cfg));
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let port = listener.local_addr().unwrap().port();
    rt.spawn(async move {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        let (sock, _) = listener.accept().await.unwrap();
        let tls = acceptor.accept(sock).await.unwrap();
        let mut tls = tokio::io::BufReader::new(tls);
        let mut line = String::new();
        tls.read_line(&mut line).await.unwrap();
        tls.write_all(format!("echo {line}").as_bytes()).await.unwrap();
        tls.shutdown().await.unwrap();
    });
    let cli = Probe::new();
    cli.net.connect(1, tls_target("localhost", port, Verify::Pinned(fp.clone())));
    let (_, _, tls) = cli.connected(1);
    let tls = tls.unwrap();
    assert_eq!(tls.protocol, "TLSv1.2");
    assert!(tls.cipher.starts_with("TLS_ECDHE_ECDSA_"), "{}", tls.cipher);
    assert_eq!(tls.cert_sha256, fp);
    cli.net.send(1, b"PING :x".to_vec());
    assert_eq!(cli.line(1), b"echo PING :x");
    assert_eq!(cli.closed(1), None);
    drop(cli);
    rt.shutdown_timeout(Duration::from_secs(1));
}

#[test]
fn accepted_ids_are_distinct() {
    let srv = Probe::new();
    let addr = srv.net.listen(1, localhost(), None).unwrap();
    let addr2 = srv.net.listen(2, localhost(), None).unwrap();
    let cli = Probe::new();
    let mut ids = std::collections::HashSet::new();
    for i in 0..10u32 {
        let port = if i % 2 == 0 { addr.port() } else { addr2.port() };
        cli.net.connect(i, Target::new("127.0.0.1", port, false));
        cli.connected(i);
        let (conn, _, _) = srv.accepted(1 + i % 2);
        assert!(conn >= FIRST_ACCEPTED_ID);
        assert!(ids.insert(conn));
    }
}
