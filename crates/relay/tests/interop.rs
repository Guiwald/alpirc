//! Essai d'interopérabilité : le vrai moteur du client AlpIRC (`alpirc_core::Client`), mené comme le fait
//! l'appli de bureau (ses `Effect` appliqués sur une couche `alpirc-net`), se sert du relais comme d'un bouncer
//! soju : liste des réseaux, une connexion liée par réseau, canaux, messages, historique, marques de lecture.

mod common;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use alpirc_core::settings::{Network, Server, Settings, Verify as SVerify};
use alpirc_core::{Client, ConnId, Effect, TlsVerify, WinId};
use alpirc_net::{Net, NetEvent, Target, Verify};
use alpirc_relay::ids::now_ms;
use common::*;

/// Un client AlpIRC et sa couche réseau.
struct App {
    client: Client,
    net: Net,
    rx: Receiver<NetEvent>,
}

impl App {
    fn new(env: &Env, home: PathBuf) -> App {
        let mut s = Settings::default();
        s.identity.nick = "Gui".into();
        s.favorites.clear();
        let mut srv = Server::new(&env.addr.ip().to_string(), env.addr.port(), true);
        srv.verify = SVerify::Pinned;
        srv.fingerprint = Some(env.fp.clone());
        srv.password = Some(format!("gui:{PASSWORD}"));
        let mut n = Network::new("Relais", vec![srv]);
        n.auto_connect = true;
        n.bouncer = true;
        s.networks = vec![n];
        let (tx, rx) = mpsc::channel();
        let net = Net::new(Box::new(move |ev| {
            let _ = tx.send(ev);
        }));
        let mut client = Client::new(s, home, now_ms());
        client.connect_startup(now_ms());
        App { client, net, rx }
    }

    /// Applique les effets (comme `alpirc-shell`) et livre les événements du réseau pendant `d`, ou jusqu'à
    /// ce que `done` soit vrai.
    fn pump_until(&mut self, what: &str, done: impl Fn(&Client) -> bool) {
        let deadline = Instant::now() + WAIT;
        loop {
            for e in self.client.take_effects() {
                match e {
                    Effect::Connect { conn, host, port, tls, verify, .. } => {
                        let mut t = Target::new(host, port, tls);
                        t.verify = match verify {
                            TlsVerify::Full => Verify::Full,
                            TlsVerify::AcceptInvalid => Verify::AcceptInvalid,
                            TlsVerify::Pinned(fp) => Verify::Pinned(fp),
                        };
                        self.net.connect(conn, t);
                    }
                    Effect::Send { conn, line } => {
                        let enc = self.client.conn_encoding(conn);
                        self.net.send(conn, enc.encode(&line).into_owned());
                    }
                    Effect::Close { conn } => self.net.close(conn),
                    _ => {}
                }
            }
            if done(&self.client) {
                return;
            }
            assert!(Instant::now() < deadline, "AlpIRC : {what} attendu ; fenêtres : {:#?}", self.dump());
            if let Ok(ev) = self.rx.recv_timeout(Duration::from_millis(20)) {
                let now = now_ms();
                match ev {
                    NetEvent::Connected { conn, .. } => self.client.on_connected(conn, now),
                    NetEvent::Line { conn, bytes } => self.client.on_line(conn, &bytes, now),
                    NetEvent::Closed { conn, error } => self.client.on_closed(conn, error.as_deref(), now),
                    NetEvent::Failed { conn, error, .. } => self.client.on_connect_failed(conn, &error, now),
                    _ => {}
                }
            }
            self.client.tick(now_ms());
        }
    }

    fn pump(&mut self, d: Duration) {
        let end = Instant::now() + d;
        self.pump_until("le temps", |_| Instant::now() >= end);
    }

    fn dump(&self) -> Vec<(String, Vec<String>)> {
        self.client
            .windows()
            .iter()
            .map(|w| (w.name.clone(), w.lines.iter().map(|l| l.text.clone()).collect()))
            .collect()
    }

    /// Connexion liée au réseau du relais `netid`.
    fn bound_conn(c: &Client, netid: &str) -> Option<ConnId> {
        c.conns().into_iter().find(|id| c.conn_info(*id).is_some_and(|i| i.bouncer_network.as_deref() == Some(netid)))
    }

    fn chan_win(c: &Client, netid: &str, chan: &str) -> Option<WinId> {
        let conn = App::bound_conn(c, netid)?;
        c.find_window(Some(conn), chan)
    }

    fn has_line(c: &Client, netid: &str, chan: &str, text: &str) -> bool {
        App::chan_win(c, netid, chan)
            .and_then(|w| c.window(w))
            .is_some_and(|w| w.lines.iter().any(|l| l.text.contains(text)))
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.net.shutdown();
    }
}

#[test]
fn alpirc_client_uses_the_relay_as_a_bouncer() {
    let env = Env::start("interop");
    let bob = env.bob("Bob");
    bob.send("PRIVMSG #test :message d'avant");
    std::thread::sleep(Duration::from_millis(200));

    let home = temp_dir("interop-home");
    let mut app = App::new(&env, home.join("a"));
    // Le parent (non lié) liste les réseaux, une fenêtre Status s'ouvre pour « Fake » (identifiant 1), liée
    // par BOUNCER BIND, avec le canal #test et son historique.
    app.pump_until("connexion liée au réseau 1 et fenêtre #test", |c| App::chan_win(c, "1", "#test").is_some());
    let conn = App::bound_conn(&app.client, "1").unwrap();
    let info = app.client.conn_info(conn).unwrap();
    assert_eq!(info.nick, "Gui");
    assert!(info.caps.iter().any(|c| c == "draft/chathistory"), "{:?}", info.caps);
    assert!(info.caps.iter().any(|c| c == "echo-message"));
    assert_eq!(info.network, "Fake");
    app.pump_until("historique de #test", |c| App::has_line(c, "1", "#test", "message d'avant"));
    let w = App::chan_win(&app.client, "1", "#test").unwrap();
    let hist = app.client.window(w).unwrap().lines.iter().find(|l| l.text.contains("message d'avant")).unwrap().clone();
    assert!(hist.from_history, "ligne venue de CHATHISTORY");
    assert!(hist.msgid.is_some());
    let nicks: Vec<String> = app.client.nicklist(w).into_iter().map(|n| n.nick).collect();
    assert!(nicks.contains(&"Bob".to_string()) && nicks.contains(&"Gui".to_string()), "{nicks:?}");

    // Message du réseau en direct.
    bob.send("PRIVMSG #test :coucou AlpIRC");
    app.pump_until("message de Bob", |c| App::has_line(c, "1", "#test", "<Bob> coucou AlpIRC"));

    // Un second appareil (client ligne à ligne) sur la même session.
    let mut phone = env.client();
    phone.register("gui/Fake:secret", "Gui", &["server-time", "message-tags", "draft/read-marker"], None);
    phone.expect("366", |m| m.command == "366");

    // Tapé dans AlpIRC : le réseau et le téléphone le voient ; AlpIRC l'affiche (écho du relais).
    app.client.input(w, "tapé sur le Mac", now_ms());
    app.pump_until("notre ligne (écho)", |c| App::has_line(c, "1", "#test", "<Gui> tapé sur le Mac"));
    phone.expect("message du Mac sur le téléphone", |m| {
        is_privmsg(m, "#test", "tapé sur le Mac") && m.nick() == "Gui"
    });
    env.ircd.wait_line("Gui", |l| l == "PRIVMSG #test :tapé sur le Mac");

    // Tapé sur le téléphone : AlpIRC l'affiche comme notre ligne.
    phone.send("PRIVMSG #test :tapé sur le téléphone");
    app.pump_until("ligne du téléphone", |c| App::has_line(c, "1", "#test", "<Gui> tapé sur le téléphone"));

    // Marque de lecture posée par le téléphone : AlpIRC la reçoit.
    let t = alpirc_relay::ids::format_time(now_ms());
    phone.send(&format!("MARKREAD #test timestamp={t}"));
    app.pump_until("marque de lecture", |c| {
        App::chan_win(c, "1", "#test").and_then(|w| c.window(w)).is_some_and(|w| w.read_marker_ms.is_some())
    });

    // Un second AlpIRC (autre ordinateur, fenêtres vides) : il rattrape la conversation.
    let mut other = App::new(&env, home.join("b"));
    other.pump_until("rattrapage", |c| {
        App::has_line(c, "1", "#test", "tapé sur le Mac") && App::has_line(c, "1", "#test", "tapé sur le téléphone")
    });
    let w2 = App::chan_win(&other.client, "1", "#test").unwrap();
    let lines: Vec<String> = other.client.window(w2).unwrap().lines.iter().map(|l| l.text.clone()).collect();
    let pos = |s: &str| lines.iter().position(|l| l.contains(s)).unwrap_or_else(|| panic!("{s} absent de {lines:?}"));
    assert!(pos("message d'avant") < pos("coucou AlpIRC"));
    assert!(pos("coucou AlpIRC") < pos("tapé sur le Mac"));
    assert!(pos("tapé sur le Mac") < pos("tapé sur le téléphone"));

    // Les deux AlpIRC : un message de Bob arrive dans les deux.
    bob.send("PRIVMSG #test :pour tout le monde");
    app.pump_until("message pour tous (1)", |c| App::has_line(c, "1", "#test", "pour tout le monde"));
    other.pump_until("message pour tous (2)", |c| App::has_line(c, "1", "#test", "pour tout le monde"));

    if std::env::var_os("RELAY_TEST_DUMP").is_some() {
        eprintln!("{:#?}", app.dump());
    }

    // Fermer AlpIRC (QUIT de ses connexions) : le relais reste sur le réseau.
    let st = app
        .client
        .windows()
        .iter()
        .find(|w| w.conn == Some(conn) && w.kind == alpirc_core::WinKind::Status)
        .map(|w| w.id)
        .unwrap();
    app.client.disconnect(st, Some("bye"), now_ms());
    app.pump(Duration::from_millis(300));
    assert!(env.ircd.is_connected("Gui"));
    assert!(!env.ircd.lines_from("Gui").iter().any(|l| l.starts_with("QUIT")));
    drop(app);
    drop(other);
    let _ = std::fs::remove_dir_all(&home);
}
