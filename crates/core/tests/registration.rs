//! Connexion et enregistrement : CAP 302, SASL, pseudos de secours, PASS (Undernet LoC), NickServ, ISUPPORT,
//! STS, MOTD, perform, favoris, encodages.

mod common;

use alpirc_core::settings::{Favorite, SaslSettings, Server, StsPolicy, UndernetX};
use alpirc_core::{ConnState, Effect, LineKind, TlsVerify};
use common::*;

#[test]
fn connect_emits_effect_and_message() {
    let mut h = H::new();
    h.c.connect(h.status, "TestNet", None, h.now);
    let fx = h.effects();
    assert!(fx.contains(&Effect::Connect {
        conn: h.conn,
        host: "irc.test.net".into(),
        port: 6667,
        tls: false,
        verify: TlsVerify::Full,
        client_cert: None
    }));
    assert_eq!(h.last(h.status), "* Connecting to irc.test.net (6667)");
    assert_eq!(h.c.conn_info(h.conn).unwrap().state, ConnState::Connecting);
}

#[test]
fn registration_lines() {
    let mut h = H::new();
    h.connect();
    assert_eq!(h.sends(), vec!["CAP LS 302", "NICK Guiwald", "USER alpirc 0 * :AlpIRC user"]);
    assert_eq!(h.c.conn_info(h.conn).unwrap().state, ConnState::Registering);
}

#[test]
fn cap_negotiation() {
    let mut h = H::new();
    h.connect();
    h.sends();
    h.feed(":irc.test.net CAP * LS * :multi-prefix server-time");
    assert!(h.sends().is_empty());
    h.feed(":irc.test.net CAP * LS :echo-message unknown-cap sasl=PLAIN");
    assert_eq!(h.sends(), vec!["CAP REQ :echo-message multi-prefix server-time"]);
    h.feed(":irc.test.net CAP * ACK :echo-message multi-prefix server-time");
    assert_eq!(h.sends(), vec!["CAP END"]);
    h.register();
    let caps = h.c.conn_info(h.conn).unwrap().caps;
    assert_eq!(caps, vec!["echo-message", "multi-prefix", "server-time"]);
}

#[test]
fn cap_nak_still_ends() {
    let mut h = H::new();
    h.connect();
    h.sends();
    h.feed(":irc.test.net CAP * LS :away-notify");
    h.sends();
    h.feed(":irc.test.net CAP * NAK :away-notify");
    assert_eq!(h.sends(), vec!["CAP END"]);
}

#[test]
fn cap_new_and_del() {
    let mut h = H::new();
    h.up_with_caps("cap-notify");
    h.feed(":irc.test.net CAP Guiwald NEW :away-notify foo");
    assert_eq!(h.sends(), vec!["CAP REQ away-notify"]);
    h.feed(":irc.test.net CAP Guiwald ACK :away-notify");
    assert!(h.c.conn_info(h.conn).unwrap().caps.contains(&"away-notify".to_string()));
    h.feed(":irc.test.net CAP Guiwald DEL :away-notify");
    assert!(!h.c.conn_info(h.conn).unwrap().caps.contains(&"away-notify".to_string()));
    assert!(h.sends().is_empty(), "pas de CAP END après l'enregistrement");
}

fn sasl_settings(mech: &str, account: &str, password: &str) -> alpirc_core::Settings {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().sasl =
        Some(SaslSettings { mechanism: mech.into(), account: account.into(), password: password.into(), client_cert: None });
    s
}

#[test]
fn sasl_plain() {
    let mut h = H::with(sasl_settings("PLAIN", "gui", "secret"));
    h.connect();
    h.sends();
    h.feed(":irc.test.net CAP * LS :sasl=PLAIN,EXTERNAL server-time");
    assert_eq!(h.sends(), vec!["CAP REQ :sasl server-time"]);
    h.feed(":irc.test.net CAP * ACK :sasl server-time");
    assert_eq!(h.sends(), vec!["AUTHENTICATE PLAIN"]);
    h.feed("AUTHENTICATE +");
    let s = h.sends();
    assert_eq!(s.len(), 1);
    let payload = s[0].strip_prefix("AUTHENTICATE ").unwrap();
    assert_eq!(unb64(payload), "gui\0gui\0secret");
    h.feed(":irc.test.net 900 Guiwald Guiwald!alpirc@host gui :You are now logged in as gui");
    h.feed(":irc.test.net 903 Guiwald :SASL authentication successful");
    assert_eq!(h.sends(), vec!["CAP END"]);
    assert_eq!(h.c.session(h.conn).unwrap().account(), Some("gui"));
    assert!(h.c.session(h.conn).unwrap().sasl_succeeded());
}

#[test]
fn sasl_scram_sha256_rfc7677() {
    let mut h = H::with(sasl_settings("SCRAM-SHA-256", "user", "pencil"));
    h.c.set_scram_nonce(Some("rOprNGfwEbeRWgbNEkqO"));
    h.connect();
    h.sends();
    h.feed(":irc.test.net CAP * LS :sasl=SCRAM-SHA-256");
    h.sends();
    h.feed(":irc.test.net CAP * ACK :sasl");
    assert_eq!(h.sends(), vec!["AUTHENTICATE SCRAM-SHA-256"]);
    h.feed("AUTHENTICATE +");
    let first = h.sends();
    assert_eq!(unb64(first[0].strip_prefix("AUTHENTICATE ").unwrap()), "n,,n=user,r=rOprNGfwEbeRWgbNEkqO");
    let server_first = "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096";
    h.feed(&format!("AUTHENTICATE {}", b64(server_first.as_bytes())));
    let fin = h.sends();
    assert_eq!(
        unb64(fin[0].strip_prefix("AUTHENTICATE ").unwrap()),
        "c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,p=dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ="
    );
    h.feed(&format!("AUTHENTICATE {}", b64(b"v=6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4=")));
    assert_eq!(h.sends(), vec!["AUTHENTICATE +"]);
    h.feed(":irc.test.net 903 Guiwald :SASL authentication successful");
    assert_eq!(h.sends(), vec!["CAP END"]);
}

#[test]
fn sasl_scram_bad_server_signature_aborts() {
    let mut h = H::with(sasl_settings("SCRAM-SHA-256", "user", "pencil"));
    h.c.set_scram_nonce(Some("rOprNGfwEbeRWgbNEkqO"));
    h.connect();
    h.feed(":irc.test.net CAP * LS :sasl");
    h.feed(":irc.test.net CAP * ACK :sasl");
    h.feed("AUTHENTICATE +");
    let server_first = "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096";
    h.feed(&format!("AUTHENTICATE {}", b64(server_first.as_bytes())));
    h.sends();
    h.feed(&format!("AUTHENTICATE {}", b64(b"v=AAAA")));
    assert_eq!(h.sends(), vec!["AUTHENTICATE *", "CAP END"]);
    assert!(h.status_lines().iter().any(|l| l.starts_with("* SASL authentication failed")));
}

#[test]
fn sasl_failure_continues_without() {
    let mut h = H::with(sasl_settings("PLAIN", "gui", "bad"));
    h.connect();
    h.feed(":irc.test.net CAP * LS :sasl");
    h.feed(":irc.test.net CAP * ACK :sasl");
    h.feed("AUTHENTICATE +");
    h.sends();
    h.feed(":irc.test.net 904 Guiwald :SASL authentication failed");
    assert_eq!(h.sends(), vec!["CAP END"]);
    assert!(h.has_line(h.status, "* SASL authentication failed: SASL authentication failed"));
}

#[test]
fn sasl_not_requested_if_mechanism_missing() {
    let mut h = H::with(sasl_settings("PLAIN", "gui", "pw"));
    h.connect();
    h.sends();
    h.feed(":irc.test.net CAP * LS :sasl=EXTERNAL");
    assert_eq!(h.sends(), vec!["CAP END"]);
}

#[test]
fn sasl_external_and_client_cert() {
    let mut s = sasl_settings("EXTERNAL", "", "");
    s.network_mut("TestNet").unwrap().sasl.as_mut().unwrap().client_cert = Some("/certs/me.pem".into());
    let mut h = H::with(s);
    h.c.connect(h.status, "TestNet", None, h.now);
    let fx = h.effects();
    assert!(fx.iter().any(|e| matches!(e, Effect::Connect { client_cert: Some(p), .. } if p.to_str() == Some("/certs/me.pem"))));
    h.c.on_connected(h.conn, h.now);
    h.feed(":irc.test.net CAP * LS :sasl=EXTERNAL");
    h.feed(":irc.test.net CAP * ACK :sasl");
    h.sends();
    h.feed("AUTHENTICATE +");
    assert_eq!(h.sends(), vec!["AUTHENTICATE +"]);
}

#[test]
fn nick_in_use_alternates() {
    let mut h = H::new();
    h.connect();
    h.sends();
    h.feed(":irc.test.net 433 * Guiwald :Nickname is already in use");
    assert_eq!(h.sends(), vec!["NICK Guiwald_"]);
    assert!(h.has_line(h.status, "* Nickname Guiwald is unavailable, trying Guiwald_"));
    h.feed(":irc.test.net 433 * Guiwald_ :Nickname is already in use");
    assert_eq!(h.sends(), vec!["NICK Guiwald1"]);
    h.feed(":irc.test.net 432 * Guiwald1 :Erroneous nickname");
    assert_eq!(h.sends(), vec!["NICK Guiwald2"]);
    h.feed(":irc.test.net 001 Guiwald2 :Welcome");
    assert_eq!(h.c.conn_info(h.conn).unwrap().nick, "Guiwald2");
    // Après l'enregistrement, 433 n'entraîne plus d'essai automatique.
    h.feed(":irc.test.net 433 Guiwald2 Bob :Nickname is already in use");
    assert!(h.sends().is_empty());
}

#[test]
fn welcome_sets_state_and_host() {
    let mut h = H::new();
    h.connect();
    h.feed(":irc.test.net 001 Guiwald :Welcome to TestNet Guiwald!alpirc@host.test");
    let s = h.c.session(h.conn).unwrap();
    assert_eq!(s.host(), Some("host.test"));
    assert_eq!(h.c.conn_info(h.conn).unwrap().state, ConnState::Connected);
    assert!(h.has_line(h.status, "Welcome to TestNet Guiwald!alpirc@host.test"));
}

#[test]
fn undernet_login_on_connect() {
    let mut s = settings();
    s.network_mut("Undernet").unwrap().undernet_x =
        Some(UndernetX { username: "guiwald".into(), password: "s3cret".into(), hide_host: true, require_x: true });
    let mut h = H::with(s);
    h.c.connect(h.status, "Undernet", None, h.now);
    assert_eq!(h.connects()[0].1, "irc.undernet.org");
    h.c.on_connected(h.conn, h.now);
    assert_eq!(h.sends()[0], "PASS :+x! guiwald s3cret");
}

#[test]
fn undernet_loc_wins_over_server_password() {
    let mut s = settings();
    let n = s.network_mut("Undernet").unwrap();
    n.servers[0].password = Some("serverpw".into());
    n.undernet_x = Some(UndernetX { username: "gui".into(), password: "pw".into(), hide_host: true, require_x: false });
    let mut h = H::with(s);
    h.c.connect(h.status, "Undernet", None, h.now);
    h.c.on_connected(h.conn, h.now);
    assert_eq!(h.sends()[0], "PASS :-!+x gui pw");
}

#[test]
fn server_password() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().servers[0].password = Some("srvpw".into());
    let mut h = H::with(s);
    h.connect();
    assert_eq!(h.sends()[0], "PASS srvpw");
}

#[test]
fn nickserv_identify_after_welcome() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().nickserv_password = Some("nspw".into());
    let mut h = H::with(s);
    h.connect();
    h.sends();
    h.feed(":irc.test.net 001 Guiwald :Welcome");
    assert_eq!(h.sends(), vec!["PRIVMSG NickServ :IDENTIFY nspw"]);
    // Une seule fois, même si NickServ le redemande.
    h.feed(":NickServ!s@services. NOTICE Guiwald :This nickname is registered. Please identify via /msg NickServ IDENTIFY");
    assert!(h.sends().is_empty());
}

#[test]
fn nickserv_skipped_after_sasl() {
    let mut s = sasl_settings("PLAIN", "gui", "pw");
    s.network_mut("TestNet").unwrap().nickserv_password = Some("nspw".into());
    let mut h = H::with(s);
    h.connect();
    h.feed(":irc.test.net CAP * LS :sasl");
    h.feed(":irc.test.net CAP * ACK :sasl");
    h.feed("AUTHENTICATE +");
    h.feed(":irc.test.net 903 Guiwald :SASL authentication successful");
    h.sends();
    h.feed(":irc.test.net 001 Guiwald :Welcome");
    assert!(h.sends().iter().all(|l| !l.contains("IDENTIFY")));
}

#[test]
fn isupport_prefix_sorting() {
    let mut h = H::new();
    h.connect();
    h.register_with("PREFIX=(qaohv)~&@%+ CHANTYPES=#");
    let w = h.join("#lyon", "~Boss &Adm @Op %Half +Voice zed Alice Guiwald");
    assert_eq!(h.nicks(w), vec!["~Boss", "&Adm", "@Op", "%Half", "+Voice", "Alice", "Guiwald", "zed"]);
}

#[test]
fn motd_lines_and_connect_perform() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().perform = vec!["/join #perf".into(), "msg Bob I am $me".into()];
    let mut h = H::with(s);
    h.connect();
    h.sends();
    h.feed(":irc.test.net 001 Guiwald :Welcome");
    h.feed(":irc.test.net 375 Guiwald :- irc.test.net Message of the day -");
    h.feed(":irc.test.net 372 Guiwald :- Bienvenue");
    assert!(h.sends().is_empty(), "perform attend la fin du MOTD");
    h.feed(":irc.test.net 376 Guiwald :End of /MOTD command.");
    let s = h.sends();
    assert!(s.contains(&"JOIN #perf".to_string()), "{s:?}");
    assert!(s.contains(&"PRIVMSG Bob :I am Guiwald".to_string()), "{s:?}");
    assert!(h.has_line(h.status, "- Bienvenue"));
}

#[test]
fn favorites_join_on_connect() {
    let mut s = settings();
    s.favorites.push(Favorite { channel: "#secret".into(), network: "TestNet".into(), key: Some("k".into()), ..Favorite::default() });
    s.favorites.push(Favorite { channel: "#open".into(), network: "testnet".into(), join_on_connect: true, ..Favorite::default() });
    s.favorites.push(Favorite { channel: "#other".into(), network: "Undernet".into(), join_on_connect: true, ..Favorite::default() });
    s.favorites[3].join_on_connect = true;
    let mut h = H::with(s);
    h.connect();
    h.register();
    let s = h.sends();
    assert!(s.contains(&"JOIN #secret,#open k".to_string()), "{s:?}");
    assert!(!s.iter().any(|l| l.contains("#other") || l.contains("#lyon")));
}

#[test]
fn sts_upgrade_from_plaintext() {
    let mut h = H::new();
    h.connect();
    h.clear();
    h.feed(":irc.test.net CAP * LS :sts=port=6697,duration=300 multi-prefix");
    let fx = h.effects();
    assert!(fx.contains(&Effect::Close { conn: h.conn }));
    assert!(fx.iter().any(|e| matches!(e, Effect::Connect { port: 6697, tls: true, .. })), "{fx:?}");
    assert!(fx.contains(&Effect::SaveSettings));
    assert_eq!(h.c.settings().network("TestNet").unwrap().sts, Some(StsPolicy { port: 6697, expires_ms: None }));
    assert!(h.has_line(h.status, "* STS: switching to a secure connection on port 6697"));
    // La fin de l'ancienne connexion, signalée par l'hôte, est ignorée.
    h.c.on_closed(h.conn, None, h.now);
    assert_eq!(h.c.conn_info(h.conn).unwrap().state, ConnState::Connecting);
    h.c.on_connected(h.conn, h.now);
    assert_eq!(h.sends()[0], "CAP LS 302");
}

#[test]
fn sts_duration_stored_on_tls() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().servers = vec![Server::new("irc.test.net", 6697, true)];
    let mut h = H::with(s);
    h.connect();
    h.clear();
    h.feed(":irc.test.net CAP * LS :sts=duration=86400");
    let pol = h.c.settings().network("TestNet").unwrap().sts.clone().unwrap();
    assert_eq!(pol.port, 6697);
    assert_eq!(pol.expires_ms, Some(h.now + 86_400_000));
    assert!(h.effects().contains(&Effect::SaveSettings));
}

#[test]
fn sts_policy_used_when_connecting() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().sts = Some(StsPolicy { port: 6697, expires_ms: Some(T0 + 1000) });
    let mut h = H::with(s);
    h.c.connect(h.status, "TestNet", None, h.now);
    assert_eq!(h.connects(), vec![(h.conn, "irc.test.net".into(), 6697, true)]);
    // Politique expirée : retour au clair.
    let mut s = settings();
    s.network_mut("TestNet").unwrap().sts = Some(StsPolicy { port: 6697, expires_ms: Some(T0 - 1) });
    let mut h = H::with(s);
    h.c.connect(h.status, "TestNet", None, h.now);
    assert_eq!(h.connects(), vec![(h.conn, "irc.test.net".into(), 6667, false)]);
}

#[test]
fn ping_pong() {
    let mut h = H::new();
    h.up();
    h.feed("PING :irc.test.net");
    assert_eq!(h.sends(), vec!["PONG irc.test.net"]);
    h.feed("PING :a b");
    assert_eq!(h.sends(), vec!["PONG :a b"]);
}

#[test]
fn unknown_numeric_in_status() {
    let mut h = H::new();
    h.up();
    h.feed(":irc.test.net 999 Guiwald foo :bar baz");
    assert_eq!(h.last(h.status), "foo bar baz");
}

#[test]
fn error_numeric_in_active_window() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.feed(":irc.test.net 401 Guiwald Bob :No such nick/channel");
    assert_eq!(h.last(w), "Bob No such nick/channel");
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().kind, LineKind::Error);
}

#[test]
fn latin1_auto_decoding() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald Pat");
    h.c.on_line(h.conn, b":Pat!p@h PRIVMSG #lyon :caf\xe9 cr\xe8me \x80", h.now);
    assert_eq!(h.last(w), "<Pat> café crème €");
}

#[test]
fn forced_encodings() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().encoding = "latin1".into();
    let mut h = H::with(s);
    h.up();
    assert_eq!(h.c.conn_encoding(h.conn), alpirc_proto::encoding::Encoding::Latin1);
    let w = h.join("#lyon", "Guiwald Pat");
    // Même de l'UTF-8 valide est lu en Windows-1252.
    h.c.on_line(h.conn, "<\u{e9}>".as_bytes(), h.now);
    h.c.on_line(h.conn, b":Pat!p@h PRIVMSG #lyon :\xc3\xa9", h.now);
    assert_eq!(h.last(w), "<Pat> Ã©");
    let mut s = settings();
    s.network_mut("TestNet").unwrap().encoding = "utf-8".into();
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald Pat");
    h.c.on_line(h.conn, b":Pat!p@h PRIVMSG #lyon :caf\xe9", h.now);
    assert_eq!(h.last(w), "<Pat> caf\u{fffd}");
}

#[test]
fn status_title() {
    let mut h = H::new();
    assert_eq!(h.c.window(h.status).unwrap().title, "Status: Guiwald (not connected)");
    h.up();
    h.feed(":Guiwald MODE Guiwald :+ix");
    assert_eq!(h.c.window(h.status).unwrap().title, "Status: Guiwald [+ix] on TestNet (irc.test.net:6667)");
}

#[test]
fn server_notice_before_registration() {
    let mut h = H::new();
    h.connect();
    h.feed(":irc.test.net NOTICE * :*** Looking up your hostname");
    assert_eq!(h.last(h.status), "-irc.test.net- *** Looking up your hostname");
    h.feed("NOTICE AUTH :*** Checking Ident");
    assert!(h.last(h.status).ends_with("*** Checking Ident"));
}

#[test]
fn hidden_host_396() {
    let mut h = H::new();
    h.up();
    h.feed(":irc.test.net 396 Guiwald gui.users.undernet.org :is now your hidden host");
    assert_eq!(h.c.session(h.conn).unwrap().host(), Some("gui.users.undernet.org"));
}

#[test]
fn error_line_shown() {
    let mut h = H::new();
    h.up();
    h.feed("ERROR :Closing Link: Guiwald (Ping timeout)");
    assert_eq!(h.last(h.status), "ERROR Closing Link: Guiwald (Ping timeout)");
}

#[test]
fn standard_replies() {
    let mut h = H::new();
    h.up();
    h.feed(":irc.test.net FAIL CHATHISTORY INVALID_TARGET #nope :No such channel");
    assert_eq!(h.last(h.status), "FAIL CHATHISTORY INVALID_TARGET #nope: No such channel");
}
