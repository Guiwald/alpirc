//! Outils des essais du relais : un faux serveur IRC (en clair, sur `alpirc-net`), des clients ligne à ligne
//! (TLS épinglé vers le relais, ou en clair vers le faux serveur), et le lancement d'un relais d'essai.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use alpirc_core::settings::{Network, Server};
use alpirc_net::{Net, NetEvent, Target, Verify};
use alpirc_proto::Message;
use alpirc_relay::config::{Config, ListenConfig, NetworkConfig, UpstreamConfig, UserConfig};

pub const WAIT: Duration = Duration::from_secs(10);
pub const PASSWORD: &str = "secret";

/// Dossier temporaire vidé.
pub fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("alpirc-relay-it-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ---------------------------------------------------------------------------------------------------------------
// Faux serveur IRC
// ---------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct FakeClient {
    nick: String,
    user: String,
    registered: bool,
    caps: BTreeSet<String>,
    cap_neg: bool,
    away: bool,
}

#[derive(Default)]
struct FakeChan {
    name: String,
    members: BTreeMap<u32, String>,
    topic: Option<String>,
}

#[derive(Default)]
pub struct FakeState {
    clients: HashMap<u32, FakeClient>,
    chans: BTreeMap<String, FakeChan>,
    /// (pseudo au moment de la réception, ligne reçue).
    pub log: Vec<(String, String)>,
    pub closed: Vec<String>,
}

/// Un petit serveur IRC : enregistrement (CAP, 001-005, MOTD), JOIN/PART/NAMES/TOPIC/MODE/WHO/WHOIS,
/// PRIVMSG/NOTICE entre clients, AWAY, QUIT.
pub struct FakeIrcd {
    pub addr: SocketAddr,
    pub state: Arc<Mutex<FakeState>>,
    net: Arc<Net>,
}

const SRV: &str = "irc.fake.test";

impl FakeIrcd {
    pub fn start() -> FakeIrcd {
        let (tx, rx) = mpsc::channel::<NetEvent>();
        let net = Arc::new(Net::new(Box::new(move |ev| {
            let _ = tx.send(ev);
        })));
        let addr = net.listen(1, "127.0.0.1:0".parse().unwrap(), None).unwrap();
        let state = Arc::new(Mutex::new(FakeState::default()));
        let (st, n) = (state.clone(), net.clone());
        std::thread::spawn(move || {
            while let Ok(ev) = rx.recv() {
                let mut s = st.lock().unwrap();
                match ev {
                    NetEvent::Accepted { conn, .. } => {
                        s.clients.insert(conn, FakeClient { nick: "*".into(), ..FakeClient::default() });
                    }
                    NetEvent::Line { conn, bytes } => {
                        let line = String::from_utf8_lossy(&bytes).into_owned();
                        handle(&mut s, &n, conn, &line);
                    }
                    NetEvent::Closed { conn, .. } => {
                        if let Some(c) = s.clients.remove(&conn) {
                            let mask = format!("{}!{}@127.0.0.1", c.nick, c.user);
                            let q = format!(":{mask} QUIT :Connection closed");
                            let peers = peers_of(&s, conn);
                            for p in peers {
                                n.send(p, q.clone().into_bytes());
                            }
                            for ch in s.chans.values_mut() {
                                ch.members.remove(&conn);
                            }
                            s.closed.push(c.nick);
                        }
                    }
                    _ => {}
                }
            }
        });
        FakeIrcd { addr, state, net }
    }

    /// Lignes reçues d'un pseudo.
    pub fn lines_from(&self, nick: &str) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .log
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(nick))
            .map(|(_, l)| l.clone())
            .collect()
    }

    /// Attend une ligne reçue d'un pseudo.
    pub fn wait_line(&self, nick: &str, pred: impl Fn(&str) -> bool) -> String {
        let t0 = Instant::now();
        loop {
            if let Some(l) = self.lines_from(nick).into_iter().find(|l| pred(l)) {
                return l;
            }
            assert!(
                t0.elapsed() < WAIT,
                "le faux serveur n'a pas reçu la ligne attendue de {nick} : {:?}",
                self.lines_from(nick)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Membres d'un canal.
    pub fn members(&self, chan: &str) -> Vec<String> {
        let s = self.state.lock().unwrap();
        s.chans.get(&chan.to_ascii_lowercase()).map(|c| c.members.values().cloned().collect()).unwrap_or_default()
    }

    pub fn wait_member(&self, chan: &str, nick: &str) {
        let t0 = Instant::now();
        while !self.members(chan).iter().any(|n| n.trim_start_matches('@').eq_ignore_ascii_case(nick)) {
            assert!(t0.elapsed() < WAIT, "{nick} n'est pas arrivé sur {chan} : {:?}", self.members(chan));
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn is_connected(&self, nick: &str) -> bool {
        self.state.lock().unwrap().clients.values().any(|c| c.nick.eq_ignore_ascii_case(nick) && c.registered)
    }

    /// Ferme la connexion d'un pseudo (coupure du réseau).
    pub fn kill(&self, nick: &str) {
        let s = self.state.lock().unwrap();
        if let Some((id, _)) = s.clients.iter().find(|(_, c)| c.nick.eq_ignore_ascii_case(nick)) {
            self.net.close(*id);
        }
    }
}

fn peers_of(s: &FakeState, conn: u32) -> Vec<u32> {
    let mut v: BTreeSet<u32> = BTreeSet::new();
    for ch in s.chans.values() {
        if ch.members.contains_key(&conn) {
            v.extend(ch.members.keys().copied().filter(|c| *c != conn));
        }
    }
    v.into_iter().collect()
}

fn send(n: &Net, s: &FakeState, conn: u32, line: String) {
    let with_time = s.clients.get(&conn).is_some_and(|c| c.caps.contains("server-time"));
    let line = if with_time && !line.starts_with('@') {
        format!("@time={} {line}", alpirc_relay::ids::format_time(alpirc_relay::ids::now_ms()))
    } else {
        line
    };
    n.send(conn, line.into_bytes());
}

fn handle(s: &mut FakeState, n: &Net, conn: u32, line: &str) {
    let Ok(m) = Message::parse(line) else { return };
    let nick = s.clients.get(&conn).map(|c| c.nick.clone()).unwrap_or_default();
    s.log.push((nick.clone(), line.to_string()));
    let mask = s.clients.get(&conn).map(|c| format!("{}!{}@127.0.0.1", c.nick, c.user)).unwrap_or_default();
    let p = |i: usize| m.param(i).to_string();
    match m.command.as_str() {
        "CAP" => match m.param(0) {
            "LS" => {
                if let Some(c) = s.clients.get_mut(&conn) {
                    c.cap_neg = true;
                }
                send(n, s, conn, format!(":{SRV} CAP * LS :multi-prefix server-time userhost-in-names"));
            }
            "REQ" => {
                let req = p(1);
                let ok =
                    req.split_whitespace().all(|c| ["multi-prefix", "server-time", "userhost-in-names"].contains(&c));
                if ok && let Some(c) = s.clients.get_mut(&conn) {
                    c.caps.extend(req.split_whitespace().map(str::to_string));
                }
                send(n, s, conn, format!(":{SRV} CAP * {} :{req}", if ok { "ACK" } else { "NAK" }));
            }
            "END" => {
                if let Some(c) = s.clients.get_mut(&conn) {
                    c.cap_neg = false;
                }
                try_register(s, n, conn);
            }
            _ => {}
        },
        "NICK" => {
            let new = p(0);
            if s.clients.iter().any(|(id, c)| *id != conn && c.nick.eq_ignore_ascii_case(&new)) {
                send(n, s, conn, format!(":{SRV} 433 {nick} {new} :Nickname is already in use"));
                return;
            }
            let registered = s.clients.get(&conn).is_some_and(|c| c.registered);
            if let Some(c) = s.clients.get_mut(&conn) {
                c.nick = new.clone();
            }
            for ch in s.chans.values_mut() {
                if let Some(v) = ch.members.get_mut(&conn) {
                    *v = new.clone();
                }
            }
            if registered {
                let l = format!(":{mask} NICK :{new}");
                for pid in peers_of(s, conn) {
                    send(n, s, pid, l.clone());
                }
                send(n, s, conn, l);
            } else {
                try_register(s, n, conn);
            }
        }
        "USER" => {
            if let Some(c) = s.clients.get_mut(&conn) {
                c.user = p(0);
            }
            try_register(s, n, conn);
        }
        "PING" => send(n, s, conn, format!(":{SRV} PONG {SRV} :{}", p(0))),
        "JOIN" => {
            for chan in p(0).split(',') {
                let key = chan.to_ascii_lowercase();
                let ch = s
                    .chans
                    .entry(key.clone())
                    .or_insert_with(|| FakeChan { name: chan.to_string(), ..FakeChan::default() });
                let op = ch.members.is_empty();
                ch.members.insert(conn, if op { format!("@{nick}") } else { nick.clone() });
                let members: Vec<u32> = ch.members.keys().copied().collect();
                let names: Vec<String> = ch.members.values().cloned().collect();
                let (cname, topic) = (ch.name.clone(), ch.topic.clone());
                for mid in members {
                    send(n, s, mid, format!(":{mask} JOIN {cname}"));
                }
                if let Some(t) = topic {
                    send(n, s, conn, format!(":{SRV} 332 {nick} {cname} :{t}"));
                    send(n, s, conn, format!(":{SRV} 333 {nick} {cname} Someone 1700000000"));
                }
                send(n, s, conn, format!(":{SRV} 353 {nick} = {cname} :{}", names.join(" ")));
                send(n, s, conn, format!(":{SRV} 366 {nick} {cname} :End of NAMES list"));
            }
        }
        "PART" => {
            let key = p(0).to_ascii_lowercase();
            if let Some(ch) = s.chans.get(&key) {
                let members: Vec<u32> = ch.members.keys().copied().collect();
                let cname = ch.name.clone();
                for mid in members {
                    send(n, s, mid, format!(":{mask} PART {cname} :{}", p(1)));
                }
            }
            if let Some(ch) = s.chans.get_mut(&key) {
                ch.members.remove(&conn);
            }
        }
        "TOPIC" if m.params.len() >= 2 => {
            let key = p(0).to_ascii_lowercase();
            if let Some(ch) = s.chans.get_mut(&key) {
                ch.topic = Some(p(1));
                let members: Vec<u32> = ch.members.keys().copied().collect();
                let cname = ch.name.clone();
                for mid in members {
                    send(n, s, mid, format!(":{mask} TOPIC {cname} :{}", p(1)));
                }
            }
        }
        "MODE" => {
            if p(0).starts_with('#') {
                if m.params.len() == 1 {
                    send(n, s, conn, format!(":{SRV} 324 {nick} {} +nt", p(0)));
                    send(n, s, conn, format!(":{SRV} 329 {nick} {} 1700000000", p(0)));
                } else if p(1) == "b" || p(1) == "+b" {
                    send(n, s, conn, format!(":{SRV} 368 {nick} {} :End of ban list", p(0)));
                }
            } else {
                send(n, s, conn, format!(":{SRV} 221 {nick} +i"));
            }
        }
        "WHO" => {
            let key = p(0).to_ascii_lowercase();
            let rows: Vec<String> = s
                .chans
                .get(&key)
                .map(|ch| {
                    ch.members
                        .iter()
                        .map(|(id, shown)| {
                            let c = &s.clients[id];
                            let pre = if shown.starts_with('@') { "@" } else { "" };
                            format!(
                                ":{SRV} 352 {nick} {} {} 127.0.0.1 {SRV} {} H{pre} :0 {} real",
                                ch.name, c.user, c.nick, c.nick
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            for r in rows {
                send(n, s, conn, r);
            }
            send(n, s, conn, format!(":{SRV} 315 {nick} {} :End of WHO list", p(0)));
        }
        "WHOIS" => {
            let target = p(m.params.len().saturating_sub(1));
            match s.clients.values().find(|c| c.nick.eq_ignore_ascii_case(&target)) {
                Some(c) => {
                    let l = format!(":{SRV} 311 {nick} {} {} 127.0.0.1 * :{} real", c.nick, c.user, c.nick);
                    send(n, s, conn, l);
                }
                None => send(n, s, conn, format!(":{SRV} 401 {nick} {target} :No such nick")),
            }
            send(n, s, conn, format!(":{SRV} 318 {nick} {target} :End of WHOIS list"));
        }
        "AWAY" => {
            let away = m.params.first().is_some_and(|x| !x.is_empty());
            if let Some(c) = s.clients.get_mut(&conn) {
                c.away = away;
            }
            if away {
                send(n, s, conn, format!(":{SRV} 306 {nick} :You have been marked as being away"));
            } else {
                send(n, s, conn, format!(":{SRV} 305 {nick} :You are no longer marked as being away"));
            }
        }
        "PRIVMSG" | "NOTICE" => {
            let target = p(0);
            let l = format!(":{mask} {} {target} :{}", m.command, p(1));
            if target.starts_with('#') {
                let members: Vec<u32> = s
                    .chans
                    .get(&target.to_ascii_lowercase())
                    .map(|c| c.members.keys().copied().filter(|c| *c != conn).collect())
                    .unwrap_or_default();
                for mid in members {
                    send(n, s, mid, l.clone());
                }
            } else {
                match s.clients.iter().find(|(_, c)| c.nick.eq_ignore_ascii_case(&target)).map(|(id, _)| *id) {
                    Some(id) => send(n, s, id, l),
                    None => send(n, s, conn, format!(":{SRV} 401 {nick} {target} :No such nick")),
                }
            }
        }
        "QUIT" => {
            let q = format!(":{mask} QUIT :Quit: {}", p(0));
            for pid in peers_of(s, conn) {
                send(n, s, pid, q.clone());
            }
            n.send(conn, b"ERROR :Closing link".to_vec());
            n.close(conn);
        }
        _ => {
            if s.clients.get(&conn).is_some_and(|c| c.registered) {
                send(n, s, conn, format!(":{SRV} 421 {nick} {} :Unknown command", m.command));
            }
        }
    }
}

fn try_register(s: &mut FakeState, n: &Net, conn: u32) {
    let Some(c) = s.clients.get(&conn) else { return };
    if c.registered || c.cap_neg || c.nick == "*" || c.user.is_empty() {
        return;
    }
    let (nick, user) = (c.nick.clone(), c.user.clone());
    s.clients.get_mut(&conn).unwrap().registered = true;
    for l in [
        format!(":{SRV} 001 {nick} :Welcome to FakeNet {nick}!{user}@127.0.0.1"),
        format!(":{SRV} 002 {nick} :Your host is {SRV}"),
        format!(":{SRV} 003 {nick} :Created today"),
        format!(":{SRV} 004 {nick} {SRV} fake-1 io biklmnopstv"),
        format!(
            ":{SRV} 005 {nick} CHANTYPES=# PREFIX=(ov)@+ NETWORK=FakeNet CASEMAPPING=rfc1459 CHANMODES=b,k,l,imnpst :are supported"
        ),
        format!(":{SRV} 375 {nick} :- MOTD -"),
        format!(":{SRV} 372 {nick} :- Faux serveur"),
        format!(":{SRV} 376 {nick} :End of MOTD"),
    ] {
        send(n, s, conn, l);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Client ligne à ligne
// ---------------------------------------------------------------------------------------------------------------

/// Un client IRC ligne à ligne, sur sa propre couche réseau.
pub struct Raw {
    net: Net,
    rx: Receiver<NetEvent>,
    conn: u32,
    pub closed: bool,
    /// Lignes lues en attendant une autre.
    pub seen: Vec<Message>,
}

impl Raw {
    fn connect(target: Target) -> Raw {
        let (tx, rx) = mpsc::channel();
        let net = Net::new(Box::new(move |ev| {
            let _ = tx.send(ev);
        }));
        net.connect(7, target);
        let r = Raw { net, rx, conn: 7, closed: false, seen: Vec::new() };
        match r.rx.recv_timeout(WAIT) {
            Ok(NetEvent::Connected { .. }) => {}
            other => panic!("connexion attendue, reçu {other:?}"),
        }
        r
    }

    /// Vers le relais, en TLS, certificat épinglé.
    pub fn tls(addr: SocketAddr, fingerprint: &str) -> Raw {
        let mut t = Target::new(addr.ip().to_string(), addr.port(), true);
        t.verify = Verify::Pinned(fingerprint.to_string());
        Raw::connect(t)
    }

    /// En clair (faux serveur).
    pub fn plain(addr: SocketAddr) -> Raw {
        Raw::connect(Target::new(addr.ip().to_string(), addr.port(), false))
    }

    pub fn send(&self, line: &str) {
        self.net.send(self.conn, line.as_bytes().to_vec());
    }

    /// Ligne suivante (ou `None` après `d`, ou à la fermeture).
    pub fn next(&mut self, d: Duration) -> Option<Message> {
        if self.closed {
            return None;
        }
        let deadline = Instant::now() + d;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(NetEvent::Line { bytes, .. }) => {
                    let l = String::from_utf8_lossy(&bytes).into_owned();
                    if let Ok(m) = Message::parse(&l) {
                        return Some(m);
                    }
                }
                Ok(NetEvent::Closed { .. }) | Ok(NetEvent::Failed { .. }) => {
                    self.closed = true;
                    return None;
                }
                Ok(_) => {}
                Err(_) => return None,
            }
        }
    }

    /// Attend une ligne qui convient (les autres vont dans `seen`).
    pub fn expect(&mut self, what: &str, pred: impl Fn(&Message) -> bool) -> Message {
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.next(left) {
                Some(m) if pred(&m) => return m,
                Some(m) => self.seen.push(m),
                None => {
                    panic!("attendu : {what} ; lu : {:#?}", self.seen.iter().map(|m| m.to_line()).collect::<Vec<_>>())
                }
            }
        }
    }

    /// Attend une commande (et éventuellement un premier paramètre).
    pub fn expect_cmd(&mut self, cmd: &str) -> Message {
        self.expect(cmd, |m| m.command == cmd)
    }

    /// Toutes les lignes reçues pendant `d`.
    pub fn drain(&mut self, d: Duration) -> Vec<Message> {
        let deadline = Instant::now() + d;
        let mut v = Vec::new();
        while let Some(m) = self.next(deadline.saturating_duration_since(Instant::now())) {
            v.push(m);
        }
        v
    }

    /// Lignes reçues jusqu'au PONG de `token` (exclu).
    pub fn until_pong(&mut self, token: &str) -> Vec<Message> {
        self.send(&format!("PING :{token}"));
        let mut v = Vec::new();
        loop {
            let m = self.next(WAIT).unwrap_or_else(|| {
                panic!("PONG {token} attendu ; lu : {:?}", v.iter().map(Message::to_line).collect::<Vec<_>>())
            });
            if m.command == "PONG" && m.trailing() == token {
                return v;
            }
            v.push(m);
        }
    }

    /// Attend la fermeture.
    pub fn wait_closed(&mut self) {
        let deadline = Instant::now() + WAIT;
        while !self.closed {
            assert!(Instant::now() < deadline, "fermeture attendue");
            self.next(Duration::from_millis(200));
        }
    }

    /// Enregistrement auprès du relais avec ces capacités ; rend les lignes jusqu'à la fin du MOTD.
    pub fn register(&mut self, pass: &str, nick: &str, caps: &[&str], bind: Option<&str>) -> Vec<Message> {
        self.send("CAP LS 302");
        self.send(&format!("PASS {pass}"));
        self.send(&format!("NICK {nick}"));
        self.send(&format!("USER {nick} 0 * :Essai"));
        self.expect("CAP LS", |m| m.command == "CAP" && m.param(1) == "LS");
        if !caps.is_empty() {
            self.send(&format!("CAP REQ :{}", caps.join(" ")));
            self.expect("CAP ACK", |m| m.command == "CAP" && m.param(1) == "ACK");
        }
        if let Some(b) = bind {
            self.send(&format!("BOUNCER BIND {b}"));
        }
        self.send("CAP END");
        let mut v = Vec::new();
        loop {
            let m = self.next(WAIT).unwrap_or_else(|| {
                panic!("fin du MOTD attendue ; lu : {:?}", v.iter().map(Message::to_line).collect::<Vec<_>>())
            });
            let end = m.command == "376" || m.command == "422";
            v.push(m);
            if end {
                return v;
            }
        }
    }
}

impl Drop for Raw {
    fn drop(&mut self) {
        self.net.shutdown();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Relais d'essai
// ---------------------------------------------------------------------------------------------------------------

pub fn test_config(ircd: SocketAddr, hash: &str) -> Config {
    let mut net = Network::new("Fake", vec![Server::new(&ircd.ip().to_string(), ircd.port(), false)]);
    net.nick = Some("Gui".into());
    net.realname = Some("Guillaume".into());
    Config {
        server_name: "relay.test".into(),
        listen: vec![ListenConfig { address: "127.0.0.1:0".into(), tls: true }],
        upstream: UpstreamConfig {
            flood_burst: 100,
            flood_interval_ms: 50,
            reconnect_min_secs: 1,
            reconnect_max_secs: 2,
            ..UpstreamConfig::default()
        },
        users: vec![UserConfig {
            name: "gui".into(),
            password_hash: hash.to_string(),
            networks: vec![NetworkConfig { network: net, channels: vec!["#test".into()], enabled: true }],
        }],
        ..Config::default()
    }
}

/// Un faux serveur, un relais d'essai connecté à lui (utilisateur `gui`, réseau `Fake` d'identifiant 1, canal
/// `#test`), et le dossier de données.
pub struct Env {
    pub ircd: FakeIrcd,
    pub relay: Option<alpirc_relay::RelayHandle>,
    pub data: PathBuf,
    pub addr: SocketAddr,
    pub fp: String,
    pub cfg: Config,
}

impl Env {
    pub fn start(name: &str) -> Env {
        let ircd = FakeIrcd::start();
        let data = temp_dir(name);
        let hash = alpirc_relay::auth::hash_password(PASSWORD).unwrap();
        let cfg = test_config(ircd.addr, &hash);
        let mut e = Env { ircd, relay: None, data, addr: "127.0.0.1:1".parse().unwrap(), fp: String::new(), cfg };
        e.boot();
        e
    }

    /// (Re)lance le relais et attend qu'il soit sur `#test`.
    pub fn boot(&mut self) {
        let relay = alpirc_relay::start(self.cfg.clone(), &self.data).expect("relais lancé");
        self.addr = relay.listeners()[0].0;
        self.fp = relay.fingerprint().to_string();
        self.relay = Some(relay);
        self.ircd.wait_member("#test", "Gui");
    }

    pub fn stop_relay(&mut self) {
        if let Some(r) = self.relay.take() {
            r.stop();
        }
    }

    pub fn client(&self) -> Raw {
        Raw::tls(self.addr, &self.fp)
    }

    /// Un utilisateur du faux serveur (« Bob ») sur `#test`.
    pub fn bob(&self, nick: &str) -> Raw {
        let mut b = Raw::plain(self.ircd.addr);
        b.send(&format!("NICK {nick}"));
        b.send(&format!("USER {} 0 * :{nick}", nick.to_ascii_lowercase()));
        b.expect_cmd("376");
        b.send("JOIN #test");
        b.expect("366", |m| m.command == "366");
        b
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        self.stop_relay();
        let _ = std::fs::remove_dir_all(&self.data);
    }
}

/// Le texte d'un PRIVMSG.
pub fn is_privmsg(m: &Message, target: &str, text: &str) -> bool {
    m.command == "PRIVMSG" && m.param(0).eq_ignore_ascii_case(target) && m.param(1) == text
}
