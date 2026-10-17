//! Vie des connexions : reconnexion avec délai croissant, rotation des serveurs, retour sur les canaux (avec
//! clés), délai de ping, PING de latence, file anti-flood, fermeture des fenêtres, démarrage, bouncer soju.

mod common;

use alpirc_core::settings::{Network, Server};
use alpirc_core::{ConnState, Effect, WinKind};
use common::*;

#[test]
fn reconnect_and_rejoin_with_keys() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "@Guiwald");
    h.feed(":Guiwald!alpirc@host.test MODE #lyon +k clé");
    h.join("#open", "Guiwald");
    h.c.on_closed(h.conn, Some("Connection reset by peer"), h.now);
    assert!(h.has_line(h.status, "* Disconnected (Connection reset by peer)"));
    assert!(h.has_line(h.status, "* Retrying in 10 seconds (attempt 1)..."));
    assert_eq!(h.last(w), "* Disconnected (Connection reset by peer)");
    assert!(!h.c.window(w).unwrap().joined);
    assert_eq!(h.c.conn_info(h.conn).unwrap().state, ConnState::Disconnected);
    let next = h.advance(9_999);
    assert_eq!(next, Some(T0 + 10_000));
    assert!(h.connects().is_empty());
    h.advance(1);
    assert_eq!(h.connects(), vec![(h.conn, "irc.test.net".into(), 6667, false)]);
    h.c.on_connected(h.conn, h.now);
    h.register();
    assert!(h.sends().contains(&"JOIN #lyon,#open clé".to_string()));
    h.feed(":Guiwald!alpirc@host.test JOIN #lyon");
    assert_eq!(h.win("#lyon"), w, "même fenêtre");
    assert!(h.c.window(w).unwrap().joined);
}

#[test]
fn rejoin_disabled() {
    let mut s = settings();
    s.connect.rejoin_channels = false;
    let mut h = H::with(s);
    h.up();
    h.join("#lyon", "Guiwald");
    h.c.on_closed(h.conn, None, h.now);
    h.advance(10_000);
    h.c.on_connected(h.conn, h.now);
    h.register();
    assert!(!h.sends().iter().any(|l| l.starts_with("JOIN")));
}

#[test]
fn backoff_and_server_rotation() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().servers.push(Server::new("irc2.test.net", 6697, true));
    let mut h = H::with(s);
    h.c.connect(h.status, "TestNet", None, h.now);
    assert_eq!(h.connects()[0].1, "irc.test.net");
    h.c.on_connect_failed(h.conn, "Connection refused", h.now);
    assert!(h.has_line(h.status, "* Unable to connect to server (Connection refused)"));
    assert!(h.has_line(h.status, "* Retrying in 10 seconds (attempt 1)..."));
    h.advance(10_000);
    assert_eq!(h.connects(), vec![(h.conn, "irc2.test.net".into(), 6697, true)]);
    h.c.on_connect_failed(h.conn, "timeout", h.now);
    assert!(h.has_line(h.status, "* Retrying in 20 seconds (attempt 2)..."));
    h.advance(20_000);
    assert_eq!(h.connects()[0].1, "irc.test.net");
    h.c.on_connect_failed(h.conn, "x", h.now);
    h.advance(40_000);
    h.c.on_connect_failed(h.conn, "x", h.now);
    assert!(h.has_line(h.status, "* Retrying in 60 seconds (attempt 4)..."));
    h.advance(60_000);
    h.c.on_connect_failed(h.conn, "x", h.now);
    assert_eq!(h.last(h.status), "* Retrying in 60 seconds (attempt 5)...");
    // Une connexion réussie remet le compteur à zéro.
    h.advance(60_000);
    h.c.on_connected(h.conn, h.now);
    h.register();
    h.c.on_closed(h.conn, None, h.now);
    assert!(h.has_line(h.status, "* Retrying in 10 seconds (attempt 1)..."));
}

#[test]
fn max_retries() {
    let mut s = settings();
    s.connect.max_retries = 2;
    let mut h = H::with(s);
    h.c.connect(h.status, "TestNet", None, h.now);
    h.c.on_connect_failed(h.conn, "x", h.now);
    h.advance(10_000);
    h.c.on_connect_failed(h.conn, "x", h.now);
    h.advance(20_000);
    h.clear();
    h.c.on_connect_failed(h.conn, "x", h.now);
    h.advance(120_000);
    assert!(h.connects().is_empty());
}

#[test]
fn no_reconnect_when_disabled() {
    let mut s = settings();
    s.connect.reconnect = false;
    let mut h = H::with(s);
    h.up();
    h.c.on_closed(h.conn, None, h.now);
    h.advance(100_000);
    assert!(h.connects().is_empty());
}

#[test]
fn ping_timeout_closes_and_reconnects() {
    let mut h = H::new();
    h.up();
    h.advance(239_000);
    assert!(!h.effects().contains(&Effect::Close { conn: h.conn }));
    h.advance(1_000);
    assert!(h.effects().contains(&Effect::Close { conn: h.conn }));
    assert!(h.has_line(h.status, "* Ping timeout (240 seconds)"));
    assert!(h.has_line(h.status, "* Disconnected (Ping timeout)"));
    // L'hôte signale ensuite la fermeture : ignorée.
    let n = h.status_lines().len();
    h.c.on_closed(h.conn, Some("closed"), h.now);
    assert_eq!(h.status_lines().len(), n);
    h.advance(10_000);
    assert_eq!(h.connects().len(), 1);
}

#[test]
fn lines_from_closed_connection_ignored() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald Pat");
    h.input(h.status, "/server irc.test.net");
    h.clear();
    h.feed(":Pat!p@h PRIVMSG #lyon :trop tard");
    assert_eq!(h.last(w), "* Disconnected");
    h.c.on_closed(h.conn, None, h.now);
    assert_eq!(h.c.conn_info(h.conn).unwrap().state, ConnState::Connecting);
    h.c.on_connected(h.conn, h.now);
    assert_eq!(h.sends()[0], "CAP LS 302");
}

#[test]
fn lag_ping_every_minute() {
    let mut h = H::new();
    h.up();
    h.advance(59_000);
    assert!(h.sends().is_empty());
    h.advance(1_000);
    let t = h.now;
    assert_eq!(h.sends(), vec![format!("PING :ALPIRC{t}")]);
    h.now += 80;
    h.feed(&format!(":irc.test.net PONG irc.test.net :ALPIRC{t}"));
    assert_eq!(h.c.conn_info(h.conn).unwrap().lag_ms, Some(80));
    assert!(!h.status_lines().iter().any(|l| l.contains("Lag")), "mesure silencieuse");
    h.advance(60_000);
    assert_eq!(h.sends().len(), 1);
}

#[test]
fn flood_queue_pacing() {
    let mut s = settings();
    s.flood.enabled = true;
    s.flood.burst = 2;
    s.flood.rate_ms = 1000;
    let mut h = H::with(s);
    h.connect();
    h.feed(":irc.test.net 001 Guiwald :Welcome");
    h.feed(":irc.test.net 376 Guiwald :End");
    let w = h.join("#lyon", "Guiwald");
    h.clear();
    h.advance(10_000);
    h.sends();
    h.input(w, "un\ndeux\ntrois\nquatre");
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :un", "PRIVMSG #lyon :deux"]);
    // PONG passe devant la file.
    h.feed("PING :x");
    assert_eq!(h.sends(), vec!["PONG x"]);
    let next = h.c.tick(h.now).unwrap();
    assert_eq!(next, h.now + 1000);
    h.advance(999);
    assert!(h.sends().is_empty());
    h.advance(1);
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :trois"]);
    h.advance(1000);
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :quatre"]);
    // Les lignes affichées ne dépendent pas de la file.
    assert_eq!(h.lines(w).len(), 5);
}

#[test]
fn registration_bypasses_flood_queue() {
    let mut s = settings();
    s.flood.enabled = true;
    s.flood.burst = 1;
    let mut h = H::with(s);
    h.connect();
    assert_eq!(h.sends().len(), 3);
}

#[test]
fn close_channel_query_and_status() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.c.close_window(w, h.now);
    assert_eq!(h.sends(), vec!["PART #lyon"]);
    assert!(h.has_win("#lyon"), "fermée à l'écho");
    h.feed(":Guiwald!alpirc@host.test PART #lyon");
    assert!(!h.has_win("#lyon"));
    h.feed(":Bob!b@h PRIVMSG Guiwald :hey");
    let q = h.win("Bob");
    h.c.close_window(q, h.now);
    assert!(!h.has_win("Bob"));
    // Dernière fenêtre Status : déconnexion, la fenêtre reste.
    h.c.close_window(h.status, h.now);
    assert_eq!(h.sends(), vec!["QUIT :AlpIRC — https://github.com/Guiwald/alpirc"]);
    assert!(h.c.window(h.status).is_some());
    // Une seconde fenêtre Status se ferme avec ses fenêtres.
    let s2 = h.c.new_status(h.now);
    let c2 = h.c.window(s2).unwrap().conn.unwrap();
    h.c.connect(s2, "TestNet", None, h.now);
    h.c.on_connected(c2, h.now);
    h.feed_conn(c2, ":irc.test.net 001 Guiwald :Welcome");
    h.feed_conn(c2, ":Guiwald!a@h JOIN #deux");
    assert!(h.c.find_window(Some(c2), "#deux").is_some());
    h.c.close_window(s2, h.now);
    assert!(h.c.window(s2).is_none());
    assert!(h.c.find_window(Some(c2), "#deux").is_none());
    assert!(h.c.conn_info(c2).is_none());
}

#[test]
fn close_unjoined_channel_immediately() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "@Pat Guiwald");
    h.feed(":Pat!p@h KICK #lyon Guiwald :out");
    h.c.close_window(w, h.now);
    assert!(!h.has_win("#lyon"));
    assert!(h.sends().is_empty());
}

#[test]
fn connect_on_startup() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().auto_connect = true;
    s.connect.connect_on_startup = true;
    s.connect.startup_networks = vec!["Libera.Chat".into()];
    let mut h = H::with(s);
    h.c.connect_startup(h.now);
    let c = h.connects();
    let hosts: Vec<&str> = c.iter().map(|x| x.1.as_str()).collect();
    assert_eq!(hosts, vec!["irc.libera.chat", "irc.test.net"]);
    assert_eq!(c[0].0, h.conn);
    assert_eq!(h.c.windows().iter().filter(|w| w.kind == WinKind::Status).count(), 2);
}

#[test]
fn next_wake_reports_timers() {
    let mut h = H::new();
    assert_eq!(h.c.tick(h.now), None);
    h.up();
    let n = h.c.tick(h.now).unwrap();
    assert_eq!(n, h.now + 60_000, "PING de latence");
}

fn bouncer_up(h: &mut H) {
    h.connect();
    h.feed(":bnc CAP * LS :batch soju.im/bouncer-networks soju.im/bouncer-networks-notify server-time");
    h.feed(":bnc CAP * ACK :batch soju.im/bouncer-networks soju.im/bouncer-networks-notify server-time");
    h.feed(":bnc 001 Guiwald :Welcome to soju");
    h.feed(":bnc 376 Guiwald :End");
}

#[test]
fn bouncer_networks_open_bound_connections() {
    let mut h = H::new();
    bouncer_up(&mut h);
    assert!(h.sends().contains(&"BOUNCER LISTNETWORKS".to_string()));
    h.clear();
    h.feed(":bnc BATCH +b1 soju.im/bouncer-networks");
    h.feed("@batch=b1 :bnc BOUNCER NETWORK 1 name=Libera;state=connected");
    h.feed("@batch=b1 :bnc BOUNCER NETWORK 2 name=OFTC\\sNet");
    h.feed(":bnc BATCH -b1");
    assert_eq!(h.last(h.status), "* Bouncer networks: Libera (1), OFTC Net (2)");
    let fx = h.effects();
    assert!(fx.contains(&Effect::SaveSettings));
    let connects: Vec<(u32, String)> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Connect { conn, host, .. } => Some((*conn, host.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(connects.len(), 2);
    assert!(connects.iter().all(|(c, host)| *c != h.conn && host == "irc.test.net"));
    let saved = &h.c.settings().network("TestNet").unwrap().bouncer_networks;
    assert_eq!(saved.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["Libera", "OFTC Net"]);
    // La connexion enfant se lie à son réseau avant CAP END.
    let child = connects[0].0;
    h.c.on_connected(child, h.now);
    assert_eq!(h.sends_for(child), vec!["CAP LS 302", "NICK Guiwald", "USER alpirc 0 * :AlpIRC user"]);
    h.feed_conn(child, ":bnc CAP * LS :soju.im/bouncer-networks");
    assert_eq!(h.sends_for(child), vec!["CAP REQ soju.im/bouncer-networks"]);
    h.feed_conn(child, ":bnc CAP * ACK :soju.im/bouncer-networks");
    assert_eq!(h.sends_for(child), vec!["BOUNCER BIND 1", "CAP END"]);
    h.feed_conn(child, ":bnc 001 Guiwald :Welcome");
    h.feed_conn(child, ":bnc 005 Guiwald NETWORK=Libera.Chat :are supported");
    h.feed_conn(child, ":bnc 376 Guiwald :End");
    assert!(!h.sends_for(child).contains(&"BOUNCER LISTNETWORKS".to_string()), "connexion liée : pas de sous-réseaux");
    let info = h.c.conn_info(child).unwrap();
    assert_eq!(info.bouncer_network.as_deref(), Some("1"));
    assert_eq!(info.network, "Libera");
    let st = h.c.window(info.status_window).unwrap();
    assert_eq!(st.title, "Status: Guiwald on Libera (irc.test.net:6667)");
}

#[test]
fn bouncer_network_notify_and_delete() {
    let mut h = H::new();
    bouncer_up(&mut h);
    h.clear();
    h.feed(":bnc BOUNCER NETWORK 7 name=EFnet");
    assert_eq!(h.last(h.status), "* Bouncer networks: EFnet (7)");
    assert_eq!(h.connects().len(), 1);
    h.feed(":bnc BOUNCER NETWORK 7 name=EFnet;state=connected");
    assert!(h.connects().is_empty(), "pas de seconde connexion");
    h.feed(":bnc BOUNCER NETWORK 7 *");
    assert!(h.c.settings().network("TestNet").unwrap().bouncer_networks.is_empty());
}

#[test]
fn bouncer_option_off() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().bouncer = false;
    let mut h = H::with(s);
    bouncer_up(&mut h);
    assert!(!h.sends().contains(&"BOUNCER LISTNETWORKS".to_string()));
}

#[test]
fn adhoc_server_uses_known_network_settings() {
    let mut s = settings();
    s.networks.push(Network::new("Other", vec![Server::new("irc.other.org", 6667, false)]));
    s.network_mut("Other").unwrap().nick = Some("AutreNick".into());
    let mut h = H::with(s);
    h.input(h.status, "/server irc.other.org");
    h.c.on_connected(h.conn, h.now);
    assert!(h.sends().contains(&"NICK AutreNick".to_string()));
    assert_eq!(h.c.conn_info(h.conn).unwrap().network, "Other");
}
