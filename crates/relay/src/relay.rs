//! Le cœur du relais : un seul fil qui tient tout l'état (utilisateurs, connexions aux réseaux, appareils,
//! historique, marques de lecture) et reçoit les événements d'`alpirc-net` par un canal. Rien n'est partagé
//! entre fils : pas de verrou, un ordre des événements déterministe.
//!
//! Côté réseau, chaque (utilisateur, réseau) a sa connexion permanente ([`Upstream`]) ; côté appareils, chaque
//! connexion ([`Downstream`]) s'identifie (`PASS` ou SASL PLAIN), se lie à un réseau (suffixe `/réseau` ou
//! `BOUNCER BIND`) et reçoit alors l'état du réseau (canaux, sujets, membres) puis tout ce qui s'y passe. Ce
//! qu'un appareil envoie part vers le réseau ; ses propres messages sont renvoyés aux autres appareils (et à
//! lui-même s'il a demandé `echo-message`) : la synchronisation entre appareils (ADR 0009).

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use alpirc_core::session::{ChannelState, SessionEvent, parse_attrs};
use alpirc_core::settings::Verify as SettingsVerify;
use alpirc_net::{Net, NetEvent, ServerTls, Target, Verify};
use alpirc_proto::encoding::Encoding;
use alpirc_proto::{Message, Source, Tags};
use base64::Engine as _;

use crate::auth::{self, Login, Throttle};
use crate::config::{Config, UserConfig, parse_listen};
use crate::downstream::{Downstream, MAX_PREREG_LINES, OFFERED_CAPS, SaslState, cap_value};
use crate::history::{HistoryError, HistoryOptions, HistoryStore, Query, Ref};
use crate::ids::{format_time, now_ms, parse_time};
use crate::markers::{MarkResult, ReadMarkers};
use crate::networks::{BouncerError, NetworkStore};
use crate::routing::{self, Route};
use crate::upstream::{UpState, Upstream, join_lines, perform_line, session_config};

/// Version du relais.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Messages rendus au plus par une commande `CHATHISTORY` (annoncé par `CHATHISTORY=` dans ISUPPORT).
pub const CHATHISTORY_MAX: usize = 1000;
/// Délai pour s'enregistrer, côté appareil.
const CLIENT_REGISTRATION_TIMEOUT_MS: i64 = 60_000;
/// Silence d'un appareil avant un PING, puis avant de le tenir pour parti.
const CLIENT_PING_MS: i64 = 120_000;
const CLIENT_TIMEOUT_MS: i64 = 300_000;
/// Délai d'enregistrement auprès d'un réseau.
const UPSTREAM_REGISTRATION_TIMEOUT_MS: i64 = 90_000;
/// Numériques de l'enregistrement auprès du réseau : remplacés par ceux du relais, pas transmis.
const REG_NUMERICS: &[&str] = &[
    "001", "002", "003", "004", "005", "042", "221", "250", "251", "252", "253", "254", "255", "265", "266", "372",
    "375", "376", "396", "422",
];

/// Un réseau d'un utilisateur.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NetKey {
    pub user: String,
    pub net: String,
}

impl NetKey {
    pub fn new(user: &str, net: &str) -> NetKey {
        NetKey { user: user.to_string(), net: net.to_string() }
    }
}

enum Event {
    Net(NetEvent),
    Shutdown(String),
}

/// Pour arrêter le relais depuis un autre fil (signal).
#[derive(Clone)]
pub struct Stopper(Sender<Event>);

impl Stopper {
    pub fn stop(&self, reason: &str) {
        let _ = self.0.send(Event::Shutdown(reason.to_string()));
    }
}

/// Le relais lancé.
pub struct RelayHandle {
    tx: Sender<Event>,
    thread: Option<JoinHandle<()>>,
    listeners: Vec<(SocketAddr, bool)>,
    fingerprint: String,
}

impl RelayHandle {
    /// Adresses d'écoute effectives (et TLS).
    pub fn listeners(&self) -> &[(SocketAddr, bool)] {
        &self.listeners
    }

    /// Empreinte SHA-256 du certificat (hexadécimal minuscule).
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn stopper(&self) -> Stopper {
        Stopper(self.tx.clone())
    }

    /// Attend la fin du relais (après [`Stopper::stop`]).
    pub fn wait(mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    /// Arrête le relais (QUIT aux réseaux, ERROR aux appareils) et attend sa fin.
    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        if let Some(t) = self.thread.take() {
            let _ = self.tx.send(Event::Shutdown("Relay shutting down".into()));
            let _ = t.join();
        }
    }
}

impl Drop for RelayHandle {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

/// Lit (ou crée) le certificat et la clé du relais. Rend (certificat PEM, clé PEM, empreinte, créé).
pub fn ensure_certificate(cfg: &Config, data: &Path) -> Result<(Vec<u8>, Vec<u8>, String, bool), String> {
    let (cp, kp) = (cfg.cert_path(data), cfg.key_path(data));
    match (cp.exists(), kp.exists()) {
        (true, true) => {
            let cert = std::fs::read(&cp).map_err(|e| format!("{} : {e}", cp.display()))?;
            let key = std::fs::read(&kp).map_err(|e| format!("{} : {e}", kp.display()))?;
            let fp = fingerprint_of_pem(&cert)?;
            Ok((cert, key, fp, false))
        }
        (false, false) => {
            let names = if cfg.cert_names.is_empty() { vec![cfg.server_name.clone()] } else { cfg.cert_names.clone() };
            let (cert, key, fp) = alpirc_net::generate_self_signed(&names)?;
            for p in [&cp, &kp] {
                if let Some(dir) = p.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
                }
            }
            crate::config::write_private(&kp, &String::from_utf8_lossy(&key))
                .map_err(|e| format!("{} : {e}", kp.display()))?;
            std::fs::write(&cp, &cert).map_err(|e| format!("{} : {e}", cp.display()))?;
            Ok((cert, key, fp, true))
        }
        (true, false) => Err(format!("certificat sans clé : {} manque", kp.display())),
        (false, true) => Err(format!("clé sans certificat : {} manque", cp.display())),
    }
}

/// Empreinte SHA-256 du premier certificat d'un fichier PEM.
pub fn fingerprint_of_pem(pem: &[u8]) -> Result<String, String> {
    let text = String::from_utf8_lossy(pem);
    let start = text.find("-----BEGIN CERTIFICATE-----").ok_or("pas de certificat PEM")?;
    let body = &text[start + "-----BEGIN CERTIFICATE-----".len()..];
    let end = body.find("-----END CERTIFICATE-----").ok_or("certificat PEM incomplet")?;
    let b64: String = body[..end].chars().filter(|c| !c.is_whitespace()).collect();
    let der =
        base64::engine::general_purpose::STANDARD.decode(b64).map_err(|e| format!("certificat PEM illisible : {e}"))?;
    Ok(alpirc_net::fingerprint(&der))
}

/// `AB:CD:…` (forme lisible d'une empreinte).
pub fn fingerprint_colons(fp: &str) -> String {
    fp.as_bytes().chunks(2).map(|c| String::from_utf8_lossy(c).to_ascii_uppercase()).collect::<Vec<_>>().join(":")
}

/// Lance le relais : certificat, écoutes, connexions aux réseaux, fil principal.
pub fn start(cfg: Config, data: &Path) -> Result<RelayHandle, String> {
    cfg.validate()?;
    std::fs::create_dir_all(data).map_err(|e| format!("{} : {e}", data.display()))?;
    let (cert, key, fingerprint, created) = ensure_certificate(&cfg, data)?;
    if created {
        log::info!(
            "certificat créé : {} (SHA-256 {})",
            cfg.cert_path(data).display(),
            fingerprint_colons(&fingerprint)
        );
    }
    let (tx, rx) = mpsc::channel();
    let tx_net = tx.clone();
    let net = Net::new(Box::new(move |ev| {
        let _ = tx_net.send(Event::Net(ev));
    }));
    let mut listeners = Vec::new();
    for (i, l) in cfg.listen.iter().enumerate() {
        let addr = parse_listen(&l.address)?;
        let tls = l.tls.then(|| ServerTls { cert_chain_pem: cert.clone(), key_pem: key.clone() });
        let bound = net.listen(i as u32 + 1, addr, tls)?;
        log::info!("écoute sur {bound}{}", if l.tls { " (TLS)" } else { " (en clair)" });
        listeners.push((bound, l.tls));
    }
    let core = Core::new(cfg, data.to_path_buf(), net);
    let thread = std::thread::Builder::new()
        .name("alpirc-relay".into())
        .spawn(move || core.run(rx))
        .map_err(|e| format!("fil du relais : {e}"))?;
    Ok(RelayHandle { tx, thread: Some(thread), listeners, fingerprint })
}

struct UserRt {
    cfg: UserConfig,
    store: NetworkStore,
}

struct Core {
    cfg: Config,
    net: Net,
    now: i64,
    started: i64,
    users: BTreeMap<String, UserRt>,
    ups: BTreeMap<NetKey, Upstream>,
    up_conns: HashMap<u32, NetKey>,
    downs: HashMap<u32, Downstream>,
    history: HistoryStore,
    markers: ReadMarkers,
    throttle: Throttle,
    next_conn: u32,
    batch_seq: u64,
    last_cleanup: i64,
}

/// `:serveur` + numérique ou commande.
fn srv(server: &str, cmd: &str, params: Vec<String>) -> Message {
    Message::new(cmd, params).with_source(server)
}

fn s(x: &str) -> String {
    x.to_string()
}

/// Étiquettes des clients (`+…`).
fn client_tags(tags: &Tags) -> BTreeMap<String, String> {
    tags.0.iter().filter(|(k, _)| k.starts_with('+')).map(|(k, v)| (k.clone(), v.clone().unwrap_or_default())).collect()
}

fn tags_of(map: &BTreeMap<String, String>) -> Tags {
    Tags(map.iter().map(|(k, v)| (k.clone(), (!v.is_empty()).then(|| v.clone()))).collect())
}

/// Commande CTCP d'un texte (`\x01VERSION\x01` → `VERSION`).
fn ctcp_command(text: &str) -> Option<String> {
    let r = text.strip_prefix('\x01')?;
    Some(r.trim_end_matches('\x01').split(' ').next().unwrap_or("").to_ascii_uppercase())
}

/// Texte à garder dans l'historique (pas les requêtes et réponses CTCP, sauf ACTION).
fn storable_text(text: &str) -> bool {
    ctcp_command(text).is_none_or(|c| c == "ACTION")
}

/// TAGMSG qui mérite l'historique (réactions, réponses…), pas les seules indications de frappe.
fn storable_tagmsg(tags: &BTreeMap<String, String>) -> bool {
    tags.keys().any(|k| k != "+typing" && k != "+draft/typing")
}

/// Heure d'un message du réseau : son étiquette `time` si elle est plausible, sinon maintenant.
fn msg_time(msg: &Message, now: i64) -> i64 {
    msg.tags.get("time").and_then(parse_time).filter(|t| *t <= now + 60_000).unwrap_or(now)
}

fn isupport_escape(v: &str) -> String {
    v.replace('\\', "\\x5C").replace(' ', "\\x20")
}

/// Attributs au format des étiquettes (`name=Libera\sChat;state=connected`).
pub fn encode_attrs(attrs: &[(String, String)]) -> String {
    let mut t = Tags::default();
    for (k, v) in attrs {
        t.set(k, (!v.is_empty()).then_some(v.as_str()));
    }
    let line = Message { tags: t, source: None, command: "X".into(), params: Vec::new() }.to_line();
    line.strip_prefix('@').and_then(|l| l.strip_suffix(" X")).unwrap_or("").to_string()
}

impl Core {
    fn new(cfg: Config, data: PathBuf, net: Net) -> Core {
        let now = now_ms();
        let h = &cfg.history;
        let history = HistoryStore::new(
            data.join("history"),
            HistoryOptions {
                enabled: h.enabled,
                retention_days: h.retention_days,
                max_lines_per_target: h.max_lines_per_target,
            },
        );
        let mut core = Core {
            net,
            now,
            started: now,
            users: BTreeMap::new(),
            ups: BTreeMap::new(),
            up_conns: HashMap::new(),
            downs: HashMap::new(),
            history,
            markers: ReadMarkers::new(data.join("users")),
            throttle: Throttle::default(),
            next_conn: 0,
            batch_seq: 0,
            last_cleanup: 0,
            cfg,
        };
        for w in core.cfg.warnings() {
            log::warn!("{w}");
        }
        for u in core.cfg.users.clone() {
            let path = data.join("users").join(&u.name).join("networks.json");
            let store = NetworkStore::open(path, &u.name, u.networks.clone());
            let (burst, interval) = (core.cfg.upstream.flood_burst, core.cfg.upstream.flood_interval_ms);
            for n in store.list() {
                let key = NetKey::new(&u.name, &n.id);
                let enabled = n.enabled;
                let mut up = Upstream::new(&u.name, n, burst, interval, now);
                up.reconnect_at = enabled.then_some(now);
                core.ups.insert(key, up);
            }
            core.users.insert(u.name.to_ascii_lowercase(), UserRt { cfg: u, store });
        }
        core
    }

    fn run(mut self, rx: Receiver<Event>) {
        log::info!("relais prêt ({} utilisateur(s), {} réseau(x))", self.users.len(), self.ups.len());
        loop {
            self.now = now_ms();
            let wait = self.next_wake().map(|t| (t - self.now).clamp(5, 1000)).unwrap_or(1000);
            let first = match rx.recv_timeout(Duration::from_millis(wait as u64)) {
                Ok(e) => Some(e),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => {
                    self.shutdown("Relay shutting down");
                    return;
                }
            };
            let mut evs: Vec<Event> = first.into_iter().collect();
            while evs.len() < 1000 {
                match rx.try_recv() {
                    Ok(e) => evs.push(e),
                    Err(_) => break,
                }
            }
            for ev in evs {
                self.now = now_ms();
                match ev {
                    Event::Net(e) => self.on_net(e),
                    Event::Shutdown(reason) => {
                        self.shutdown(&reason);
                        return;
                    }
                }
            }
            self.now = now_ms();
            self.tick();
            self.check_send_queues();
        }
    }

    /// Coupe les appareils dont la file d'envoi dépasse la limite (`[clients] max_send_queue_kib`).
    fn check_send_queues(&mut self) {
        let limit = self.cfg.clients.max_send_queue_kib.saturating_mul(1024);
        if limit == 0 {
            return;
        }
        let late: Vec<(u32, usize)> = self
            .downs
            .values()
            .filter(|d| !d.closing)
            .map(|d| (d.id, self.net.pending_bytes(d.id)))
            .filter(|(_, n)| *n > limit)
            .collect();
        for (id, n) in late {
            let peer = self.downs.get(&id).map(|d| d.peer.to_string()).unwrap_or_default();
            log::warn!(
                "appareil {id} ({peer}) déconnecté : {} Kio en attente d'envoi (limite {} Kio)",
                n / 1024,
                limit / 1024
            );
            self.close_client(id, "Send queue exceeded (device too slow)");
        }
    }

    fn shutdown(&mut self, reason: &str) {
        log::info!("arrêt du relais : {reason}");
        for up in self.ups.values_mut() {
            up.stopped = true;
            if let Some(conn) = up.conn {
                if up.session.is_registered() {
                    let q = Message::new("QUIT", [reason]).to_line();
                    self.net.send(conn, up.encoding.encode(&q).into_owned());
                }
                self.net.close(conn);
            }
        }
        for (id, d) in &self.downs {
            if !d.closing {
                self.net.send(*id, format!("ERROR :Closing link: {reason}").into_bytes());
                self.net.close(*id);
            }
        }
        self.net.shutdown();
    }

    fn next_wake(&self) -> Option<i64> {
        let mut v = Vec::new();
        for up in self.ups.values() {
            if up.state == UpState::Disconnected
                && let Some(t) = up.reconnect_at
            {
                v.push(t);
            }
            if let Some(t) = up.bucket.next_due() {
                v.push(t);
            }
        }
        v.into_iter().min()
    }

    fn alloc_conn(&mut self) -> u32 {
        loop {
            self.next_conn = if self.next_conn + 1 >= alpirc_net::FIRST_ACCEPTED_ID { 1 } else { self.next_conn + 1 };
            if !self.up_conns.contains_key(&self.next_conn) {
                return self.next_conn;
            }
        }
    }

    fn new_batch_id(&mut self) -> String {
        self.batch_seq += 1;
        format!("alpirc{}", self.batch_seq)
    }

    fn server_name(&self) -> String {
        self.cfg.server_name.clone()
    }

    // -----------------------------------------------------------------------------------------------------------
    // Événements du réseau
    // -----------------------------------------------------------------------------------------------------------

    fn on_net(&mut self, ev: NetEvent) {
        match ev {
            NetEvent::Accepted { conn, peer, tls, .. } => {
                log::debug!("appareil {conn} connecté depuis {peer}");
                self.downs.insert(conn, Downstream::new(conn, peer, tls, self.now));
            }
            NetEvent::Connected { conn, tls, .. } => {
                if let Some(key) = self.up_conns.get(&conn).cloned() {
                    self.up_connected(&key, tls.is_some());
                }
            }
            NetEvent::Line { conn, bytes } => {
                if let Some(key) = self.up_conns.get(&conn).cloned() {
                    self.up_line(&key, &bytes);
                } else if self.downs.contains_key(&conn) {
                    self.client_line(conn, &bytes);
                }
            }
            NetEvent::Closed { conn, error } => {
                if let Some(key) = self.up_conns.remove(&conn) {
                    // Texte anglais de l'erreur (`Display`), comme les avis envoyés aux appareils.
                    self.up_closed(&key, conn, error.map(|e| e.to_string()));
                } else if self.downs.contains_key(&conn) {
                    self.client_closed(conn);
                }
            }
            NetEvent::Failed { conn, error, cert_sha256 } => {
                if let Some(key) = self.up_conns.remove(&conn) {
                    let e = match cert_sha256 {
                        Some(fp) => format!("{error} (SHA-256 {fp})"),
                        None => error.to_string(),
                    };
                    self.up_closed(&key, conn, Some(e));
                }
            }
            NetEvent::ListenerClosed { listener, error: Some(e) } => log::error!("écoute {listener} arrêtée : {e}"),
            // Le relais ne fait pas de DCC.
            _ => {}
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Envois aux appareils
    // -----------------------------------------------------------------------------------------------------------

    fn key_of(&self, id: u32) -> Option<NetKey> {
        let d = self.downs.get(&id)?;
        if !d.registered {
            return None;
        }
        Some(NetKey::new(d.user.as_deref()?, d.net.as_deref()?))
    }

    /// Envoie une ligne à un appareil, mise en forme selon ses capacités.
    fn send_to(&self, id: u32, msg: &Message) {
        let Some(d) = self.downs.get(&id) else { return };
        if d.closing {
            return;
        }
        let up = self.key_of(id).and_then(|k| self.ups.get(&k));
        let is = up.filter(|u| u.session.is_registered()).map(|u| u.session.isupport());
        let own = up.map(|u| u.session.nick().to_string()).unwrap_or_else(|| d.nick.clone());
        if let Some(line) = d.render(msg, is, &own) {
            self.net.send(id, line.into_bytes());
        }
    }

    /// Numérique du relais à un appareil (`:relais NNN pseudo …`).
    fn reply(&self, id: u32, num: &str, rest: &[&str]) {
        let Some(d) = self.downs.get(&id) else { return };
        let mut params = vec![d.nick.clone()];
        params.extend(rest.iter().map(|x| x.to_string()));
        self.send_to(id, &srv(&self.cfg.server_name, num, params));
    }

    fn notice(&self, id: u32, text: &str) {
        let Some(d) = self.downs.get(&id) else { return };
        self.send_to(id, &srv(&self.cfg.server_name, "NOTICE", vec![d.nick.clone(), text.to_string()]));
    }

    fn fail(&self, id: u32, params: &[&str]) {
        self.send_to(id, &srv(&self.cfg.server_name, "FAIL", params.iter().map(|x| x.to_string()).collect()));
    }

    /// Appareils liés à un réseau.
    fn clients_of(&self, key: &NetKey) -> Vec<u32> {
        let mut v: Vec<u32> =
            self.downs.values().filter(|d| !d.closing && d.bound_to(&key.user, &key.net)).map(|d| d.id).collect();
        v.sort_unstable();
        v
    }

    fn broadcast(&self, key: &NetKey, msg: &Message) {
        for id in self.clients_of(key) {
            self.send_to(id, msg);
        }
    }

    fn close_client(&mut self, id: u32, reason: &str) {
        if let Some(d) = self.downs.get_mut(&id) {
            if d.closing {
                return;
            }
            d.closing = true;
            self.net.send(id, format!("ERROR :Closing link: {reason}").into_bytes());
            self.net.close(id);
        }
    }

    fn client_closed(&mut self, id: u32) {
        let key = self.key_of(id);
        if let Some(d) = self.downs.remove(&id)
            && d.registered
        {
            log::info!("appareil {id} ({}) détaché", d.peer);
        }
        if let Some(key) = key {
            if let Some(up) = self.ups.get_mut(&key) {
                up.pending.forget_client(id);
            }
            self.update_auto_away(&key);
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Connexions aux réseaux
    // -----------------------------------------------------------------------------------------------------------

    fn up_connect(&mut self, key: &NetKey) {
        let now = self.now;
        let id = self.alloc_conn();
        let Some(up) = self.ups.get_mut(key) else { return };
        if up.stopped || !up.net.enabled || up.conn.is_some() {
            up.reconnect_at = None;
            return;
        }
        let Some(server) = up.server().cloned() else {
            up.error = Some("No server configured".into());
            up.reconnect_at = None;
            return;
        };
        let mut target = Target::new(server.host.clone(), server.port, server.tls);
        target.verify = match server.verify {
            SettingsVerify::Full => Verify::Full,
            SettingsVerify::AcceptInvalid => Verify::AcceptInvalid,
            SettingsVerify::Pinned => Verify::Pinned(server.fingerprint.clone().unwrap_or_default()),
        };
        if let Some(path) = up
            .net
            .settings
            .sasl
            .as_ref()
            .filter(|s| s.mechanism.eq_ignore_ascii_case("EXTERNAL"))
            .and_then(|s| s.client_cert.clone())
        {
            match std::fs::read(&path) {
                Ok(pem) => target.client_cert = Some((pem.clone(), pem)),
                Err(e) => log::warn!("certificat client {} : {e}", path.display()),
            }
        }
        up.session =
            alpirc_core::session::Session::new(session_config(&up.user, &up.net.settings, Some(&server), server.tls));
        up.encoding = Encoding::from_name(&up.net.settings.encoding);
        up.conn = Some(id);
        up.state = UpState::Connecting;
        up.state_since = now;
        up.reconnect_at = None;
        up.current = Some((server.host.clone(), server.port, server.tls));
        up.bucket.reset(now);
        up.pending.clear();
        up.auto_away = false;
        up.auto_away_replies = 0;
        up.ping_sent = None;
        up.last_rx = now;
        up.close_reason = None;
        log::info!(
            "{}/{} : connexion à {}:{}{}",
            key.user,
            up.net.name(),
            server.host,
            server.port,
            if server.tls { " (TLS)" } else { "" }
        );
        self.up_conns.insert(id, key.clone());
        self.net.connect(id, target);
        self.notify_state(key);
    }

    fn up_connected(&mut self, key: &NetKey, tls: bool) {
        let now = self.now;
        let Some(up) = self.ups.get_mut(key) else { return };
        up.state = UpState::Registering;
        up.state_since = now;
        up.last_rx = now;
        up.session.config_mut().tls = tls;
        up.session.start();
        self.flush_session(key);
    }

    /// Envoie au réseau ce que la session a préparé (PONG tout de suite, le reste par la file anti-flood).
    fn flush_session(&mut self, key: &NetKey) {
        let Some(up) = self.ups.get_mut(key) else { return };
        let out = up.session.take_outgoing();
        for m in out {
            self.up_send(key, &m);
        }
    }

    /// Une ligne vers le réseau.
    fn up_send(&mut self, key: &NetKey, msg: &Message) {
        let now = self.now;
        let Some(up) = self.ups.get_mut(key) else { return };
        let Some(conn) = up.conn else { return };
        let line = msg.to_line();
        let bytes = up.encoding.encode(&line).into_owned();
        if matches!(msg.command.as_str(), "PONG" | "QUIT") {
            self.net.send(conn, bytes);
        } else {
            up.bucket.push(bytes);
        }
        for l in up.bucket.drain(now) {
            self.net.send(conn, l);
        }
    }

    fn up_line(&mut self, key: &NetKey, bytes: &[u8]) {
        let now = self.now;
        let Some(up) = self.ups.get_mut(key) else { return };
        let text = up.encoding.decode(bytes).into_owned();
        let Ok(msg) = Message::parse(&text) else { return };
        up.last_rx = now;
        up.ping_sent = None;
        let was_registered = up.session.is_registered();
        let was_motd = up.session.motd_done();
        let evs = up.session.handle(&msg, now);
        self.flush_session(key);
        for ev in &evs {
            self.up_event(key, ev);
        }
        self.forward_upstream(key, &msg, &evs, was_registered, was_motd);
    }

    fn up_event(&mut self, key: &NetKey, ev: &SessionEvent) {
        match ev {
            SessionEvent::MotdEnd => self.up_ready(key),
            SessionEvent::Join { channel, own: true, .. } => {
                let Some(up) = self.ups.get_mut(key) else { return };
                let k = up.pending_keys.remove(&up.session.fold(channel));
                let chan_key = k.or_else(|| up.session.channel(channel).and_then(|c| c.key().map(str::to_string)));
                if let Some(u) = self.users.get_mut(&key.user.to_ascii_lowercase()) {
                    u.store.add_channel(&key.net, channel, chan_key.as_deref());
                }
            }
            SessionEvent::Part { channel, own: true, .. } | SessionEvent::Kick { channel, own: true, .. } => {
                if let Some(u) = self.users.get_mut(&key.user.to_ascii_lowercase()) {
                    u.store.remove_channel(&key.net, channel);
                }
            }
            SessionEvent::Nick { new, own: true, .. } => {
                for id in self.clients_of(key) {
                    if let Some(d) = self.downs.get_mut(&id) {
                        d.nick = new.clone();
                    }
                }
                let asked = self.ups.get_mut(key).and_then(|u| u.pending.nick_done()).is_some();
                if asked {
                    if let Some(u) = self.users.get_mut(&key.user.to_ascii_lowercase()) {
                        u.store.set_nick(&key.net, new);
                    }
                    self.notify_network(&key.user, &key.net, Some(vec![(s("nickname"), new.clone())]));
                }
            }
            SessionEvent::Error { message } => {
                if let Some(up) = self.ups.get_mut(key) {
                    up.close_reason.get_or_insert_with(|| message.clone());
                }
            }
            SessionEvent::SaslFailed { reason } => {
                log::warn!("{}/{} : SASL refusé : {reason}", key.user, key.net);
            }
            _ => {}
        }
    }

    /// Fin du MOTD : perform, canaux, absence, et mise à jour des appareils déjà là.
    fn up_ready(&mut self, key: &NetKey) {
        let now = self.now;
        let Some(up) = self.ups.get_mut(key) else { return };
        up.state = UpState::Ready;
        up.state_since = now;
        up.attempts = 0;
        up.error = None;
        log::info!("{}/{} : connecté sous {}", key.user, up.net.name(), up.session.nick());
        let perform: Vec<String> = up.net.settings.perform.iter().filter_map(|l| perform_line(l)).collect();
        let user_away = up.user_away.clone();
        let chans = self
            .users
            .get(&key.user.to_ascii_lowercase())
            .and_then(|u| u.store.get(&key.net))
            .map(|n| n.channels)
            .unwrap_or_default();
        for l in perform {
            if let Ok(m) = Message::parse(&l) {
                self.up_send(key, &m);
            }
        }
        for l in join_lines(&chans) {
            if let Ok(m) = Message::parse(&l) {
                self.up_send(key, &m);
            }
        }
        if let Some(m) = user_away
            && let Some(up) = self.ups.get_mut(key)
        {
            up.session.set_away(Some(&m));
            self.flush_session(key);
        }
        self.update_auto_away(key);
        // Appareils restés là pendant la coupure : pseudo et ISUPPORT du réseau.
        let Some(up) = self.ups.get(key) else { return };
        let nick = up.session.nick().to_string();
        let netname = up.net.name().to_string();
        let tokens = self.isupport_tokens(Some(key));
        for id in self.clients_of(key) {
            let old = self.downs.get(&id).map(|d| d.nick.clone()).unwrap_or_default();
            if old != nick {
                self.send_to(id, &Message::new("NICK", [nick.clone()]).with_source(&old));
                if let Some(d) = self.downs.get_mut(&id) {
                    d.nick = nick.clone();
                }
            }
            for m in self.isupport_messages(&nick, &tokens) {
                self.send_to(id, &m);
            }
            self.notice(id, &format!("Connected to {netname}"));
        }
        self.notify_state(key);
    }

    fn up_closed(&mut self, key: &NetKey, conn: u32, error: Option<String>) {
        let now = self.now;
        let (min, max) = (self.cfg.upstream.reconnect_min_secs, self.cfg.upstream.reconnect_max_secs);
        let Some(up) = self.ups.get_mut(key) else { return };
        if up.conn != Some(conn) {
            return;
        }
        let reason = error.or_else(|| up.close_reason.take()).unwrap_or_else(|| "Connection closed".into());
        up.conn = None;
        up.state = UpState::Disconnected;
        up.state_since = now;
        up.current = None;
        up.pending.clear();
        up.bucket.reset(now);
        up.error = Some(reason.clone());
        let mut delay = None;
        if !up.stopped {
            if up.reconnect_now {
                up.reconnect_now = false;
                up.attempts = 0;
                up.reconnect_at = Some(now);
            } else {
                up.attempts += 1;
                if up.net.settings.servers.len() > 1 {
                    up.server_idx = (up.server_idx + 1) % up.net.settings.servers.len();
                }
                let d = up.backoff_ms(min, max);
                up.reconnect_at = Some(now + d);
                delay = Some(d / 1000);
            }
        }
        log::warn!("{}/{} : déconnecté ({reason})", key.user, up.net.name());
        // Pas de PART : les appareils gardent leurs canaux (un PART fermerait les fenêtres d'AlpIRC) ; au retour,
        // les JOIN et NAMES du réseau remettent tout à jour.
        let netname = up.net.name().to_string();
        for id in self.clients_of(key) {
            let t = match delay {
                Some(secs) => format!("Disconnected from {netname} ({reason}); reconnecting in {secs} s"),
                None => format!("Disconnected from {netname} ({reason})"),
            };
            self.notice(id, &t);
        }
        self.notify_state(key);
    }

    /// Lignes du réseau vers les appareils.
    fn forward_upstream(
        &mut self,
        key: &NetKey,
        msg: &Message,
        evs: &[SessionEvent],
        was_registered: bool,
        was_motd: bool,
    ) {
        let now = self.now;
        let Some(up) = self.ups.get_mut(key) else { return };
        if !up.session.is_registered() || !was_registered {
            return;
        }
        let cmd = msg.command.as_str();
        if matches!(cmd, "PING" | "PONG" | "CAP" | "AUTHENTICATE" | "ERROR") {
            return;
        }
        let with_time = |m: &Message| {
            let mut o = m.clone();
            if !o.tags.has("time") {
                o.tags.set("time", Some(&format_time(now)));
            }
            o
        };
        if alpirc_proto::numeric::is_numeric(cmd) {
            if !was_motd && REG_NUMERICS.contains(&cmd) {
                return;
            }
            if cmd.parse::<u16>().is_ok_and(|n| (900..=908).contains(&n)) {
                return;
            }
            if evs.iter().any(|e| {
                e.suppresses_numeric() && !matches!(e, SessionEvent::Names { .. } | SessionEvent::NamesEnd { .. })
            }) {
                return;
            }
            if (cmd == "305" || cmd == "306") && up.auto_away_replies > 0 {
                up.auto_away_replies -= 1;
                return;
            }
            let cm = up.session.casemapping();
            let route = up.pending.route(msg, cm, now);
            let out = with_time(msg);
            match route {
                Route::To(c) => {
                    if self.downs.get(&c).is_some_and(|d| d.bound_to(&key.user, &key.net)) {
                        self.send_to(c, &out);
                    }
                }
                Route::All => self.broadcast(key, &out),
            }
            return;
        }
        match cmd {
            "PRIVMSG" | "NOTICE" | "TAGMSG" => self.upstream_message(key, msg, evs),
            _ => {
                let out = with_time(msg);
                self.broadcast(key, &out);
                // Après notre JOIN : la marque de lecture du canal (draft/read-marker).
                if let Some(SessionEvent::Join { channel, own: true, .. }) = evs.first() {
                    for id in self.clients_of(key) {
                        if self.downs.get(&id).is_some_and(|d| d.cap("draft/read-marker")) {
                            self.send_marker(id, key, channel);
                        }
                    }
                }
            }
        }
    }

    /// Cible d'un message pour l'historique : le canal, ou l'autre personne du privé.
    fn buffer_of(up: &Upstream, msg: &Message) -> String {
        let s = &up.session;
        let target = msg.param(0);
        let stripped = target.trim_start_matches(|c| s.isupport().statusmsg.contains(c));
        if s.is_channel(stripped) {
            stripped.to_string()
        } else if s.is_me(msg.nick()) {
            target.to_string()
        } else {
            msg.nick().to_string()
        }
    }

    /// PRIVMSG, NOTICE, TAGMSG reçus du réseau : historique puis appareils.
    fn upstream_message(&mut self, key: &NetKey, msg: &Message, evs: &[SessionEvent]) {
        let now = self.now;
        let Some(up) = self.ups.get(key) else { return };
        let cmd = msg.command.as_str();
        let server = match evs.first() {
            Some(SessionEvent::Message { server, .. }) => *server,
            _ => msg.source.as_ref().is_none_or(|s| s.is_server()),
        };
        let tags = client_tags(&msg.tags);
        let storable = !server
            && match cmd {
                "TAGMSG" => storable_tagmsg(&tags),
                _ => storable_text(msg.param(1)),
            };
        let t = msg_time(msg, now);
        let (id, t) = if storable {
            let buf = Self::buffer_of(up, msg);
            let fold = up.session.fold(&buf);
            let src = msg.source.as_ref().map(|s| s.raw.clone()).unwrap_or_default();
            self.history.append(&key.user, &key.net, &fold, &buf, t, &src, cmd, &msg.params, tags)
        } else {
            (self.history.new_id(t), t)
        };
        let mut out = msg.clone();
        out.tags.set("time", Some(&format_time(t)));
        out.tags.set("msgid", Some(&id));
        self.broadcast(key, &out);
    }

    // -----------------------------------------------------------------------------------------------------------
    // Absence automatique
    // -----------------------------------------------------------------------------------------------------------

    fn update_auto_away(&mut self, key: &NetKey) {
        if !self.cfg.away.auto_away {
            return;
        }
        let attached = !self.clients_of(key).is_empty();
        let message = self.cfg.away.message.clone();
        let Some(up) = self.ups.get_mut(key) else { return };
        if up.state != UpState::Ready || up.user_away.is_some() {
            return;
        }
        if !attached && !up.auto_away {
            up.auto_away = true;
            up.auto_away_replies += 1;
            up.session.set_away(Some(&message));
            self.flush_session(key);
        } else if attached && up.auto_away {
            up.auto_away = false;
            up.auto_away_replies += 1;
            up.session.set_away(None);
            self.flush_session(key);
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Lignes des appareils
    // -----------------------------------------------------------------------------------------------------------

    fn client_line(&mut self, id: u32, bytes: &[u8]) {
        let now = self.now;
        // Un appareil qui ne lit plus ne fait pas grossir sa file d'une réponse de plus.
        let limit = self.cfg.clients.max_send_queue_kib.saturating_mul(1024);
        if limit > 0 && self.net.pending_bytes(id) > limit {
            self.check_send_queues();
            return;
        }
        let Some(d) = self.downs.get_mut(&id) else { return };
        if d.closing {
            return;
        }
        d.last_rx = now;
        d.ping_sent = None;
        if !d.inbound.take(now) {
            d.dropped += 1;
            let dropped = d.dropped;
            if dropped == 1 {
                self.notice(id, "Too many lines: some were dropped");
            }
            if dropped >= 50 {
                self.close_client(id, "Excess flood");
            }
            return;
        }
        let text = Encoding::Auto.decode(bytes).into_owned();
        let (tags_len, body_len) = match text.strip_prefix('@') {
            Some(r) => match r.split_once(' ') {
                Some((t, b)) => (t.len() + 2, b.len()),
                None => (r.len() + 1, 0),
            },
            None => (0, text.len()),
        };
        if body_len > 510 || tags_len > 8191 {
            self.reply(id, "417", &["Input line was too long"]);
            return;
        }
        let Ok(msg) = Message::parse(&text) else { return };
        if self.downs.get(&id).is_some_and(|d| d.registered) {
            self.client_cmd(id, msg);
        } else {
            self.prereg(id, msg);
        }
    }

    fn prereg(&mut self, id: u32, msg: Message) {
        let Some(d) = self.downs.get_mut(&id) else { return };
        d.prereg_lines += 1;
        if d.prereg_lines > MAX_PREREG_LINES {
            self.close_client(id, "Too many lines before registration");
            return;
        }
        match msg.command.as_str() {
            "CAP" => self.cap_cmd(id, &msg),
            "PASS" => {
                if msg.params.is_empty() {
                    self.reply(id, "461", &["PASS", "Not enough parameters"]);
                } else {
                    d.pass = Some(msg.params[0].clone());
                }
            }
            "NICK" => {
                let n = msg.param(0).to_string();
                if n.is_empty() || n == "*" || n.contains([' ', ',', '!', '@']) {
                    self.reply(id, "432", &[&n, "Erroneous nickname"]);
                    return;
                }
                d.nick = n;
                self.try_register(id);
            }
            "USER" => {
                if msg.params.len() < 4 {
                    self.reply(id, "461", &["USER", "Not enough parameters"]);
                    return;
                }
                d.username = Some(msg.params[0].clone());
                d.realname = msg.params[3].clone();
                self.try_register(id);
            }
            "AUTHENTICATE" => self.sasl_cmd(id, &msg),
            "BOUNCER" => {
                if msg.param(0).eq_ignore_ascii_case("BIND") {
                    let netid = msg.param(1);
                    if netid.is_empty() {
                        self.fail(id, &["BOUNCER", "INVALID_NETID", "BIND", "*", "Missing network ID"]);
                    } else {
                        d.bind = Some(netid.to_string());
                    }
                } else {
                    self.reply(id, "451", &["You have not registered"]);
                }
            }
            "PING" => {
                let token = msg.param(0).to_string();
                let sn = self.server_name();
                self.send_to(id, &srv(&sn, "PONG", vec![sn.clone(), token]));
            }
            "QUIT" => self.close_client(id, "Bye"),
            _ => self.reply(id, "451", &["You have not registered"]),
        }
    }

    fn cap_cmd(&mut self, id: u32, msg: &Message) {
        let Some(d) = self.downs.get_mut(&id) else { return };
        let sub = msg.param(0).to_ascii_uppercase();
        let nick = d.nick.clone();
        let sn = self.cfg.server_name.clone();
        match sub.as_str() {
            "LS" => {
                if !d.registered {
                    d.cap_lock = true;
                }
                let v: u32 = msg.param(1).parse().unwrap_or(0);
                d.cap_version = d.cap_version.max(v);
                if v >= 302 {
                    d.caps.insert("cap-notify".into());
                }
                let list: Vec<String> = OFFERED_CAPS
                    .iter()
                    .map(|c| match (v >= 302, cap_value(c)) {
                        (true, Some(val)) => format!("{c}={val}"),
                        _ => c.to_string(),
                    })
                    .collect();
                // Plusieurs lignes au besoin (302 : `*` sur les lignes non finales).
                let mut chunks: Vec<String> = Vec::new();
                let mut cur = String::new();
                for c in list {
                    if !cur.is_empty() && cur.len() + c.len() + 1 > 380 {
                        chunks.push(std::mem::take(&mut cur));
                    }
                    if !cur.is_empty() {
                        cur.push(' ');
                    }
                    cur.push_str(&c);
                }
                chunks.push(cur);
                let n = chunks.len();
                for (i, c) in chunks.into_iter().enumerate() {
                    let params = if i + 1 < n && v >= 302 {
                        vec![nick.clone(), s("LS"), s("*"), c]
                    } else {
                        vec![nick.clone(), s("LS"), c]
                    };
                    self.send_to(id, &srv(&sn, "CAP", params));
                }
            }
            "LIST" => {
                let caps = d.caps.iter().cloned().collect::<Vec<_>>().join(" ");
                self.send_to(id, &srv(&sn, "CAP", vec![nick, s("LIST"), caps]));
            }
            "REQ" => {
                if !d.registered {
                    d.cap_lock = true;
                }
                let req = msg.param(1).to_string();
                let ok = !req.trim().is_empty()
                    && req.split_whitespace().all(|c| OFFERED_CAPS.contains(&c.trim_start_matches('-')));
                if ok {
                    for c in req.split_whitespace() {
                        match c.strip_prefix('-') {
                            Some(n) => {
                                d.caps.remove(n);
                            }
                            None => {
                                d.caps.insert(c.to_string());
                            }
                        }
                    }
                }
                let verb = if ok { "ACK" } else { "NAK" };
                self.send_to(id, &srv(&sn, "CAP", vec![nick, s(verb), req]));
            }
            "END" => {
                if !d.registered {
                    d.cap_lock = false;
                    self.try_register(id);
                }
            }
            _ => self.reply(id, "410", &[&sub, "Invalid CAP command"]),
        }
    }

    /// Vérifie un utilisateur et son mot de passe (avec frein aux essais) ; rend le nom du réglage.
    fn check_credentials(&mut self, id: u32, user: &str, password: &str) -> Option<String> {
        let ip = self.downs.get(&id)?.peer.ip();
        if !self.throttle.allowed(ip, self.now) {
            log::warn!("trop d'essais depuis {ip}");
            return None;
        }
        let ok = self
            .users
            .get(&user.to_ascii_lowercase())
            .filter(|u| !u.cfg.password_hash.is_empty())
            .map(|u| (auth::verify_password(&u.cfg.password_hash, password), u.cfg.name.clone()));
        match ok {
            Some((true, name)) => {
                self.throttle.succeeded(ip);
                Some(name)
            }
            _ => {
                // Même travail pour un utilisateur inconnu (pas d'indice par la durée).
                if ok.is_none() {
                    let _ = auth::verify_password(
                        "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                        password,
                    );
                }
                self.throttle.failed(ip, self.now);
                log::warn!("identification refusée pour « {user} » depuis {ip}");
                None
            }
        }
    }

    fn sasl_cmd(&mut self, id: u32, msg: &Message) {
        let Some(d) = self.downs.get_mut(&id) else { return };
        if d.registered || d.user.is_some() {
            self.reply(id, "907", &["You have already authenticated using SASL"]);
            return;
        }
        if !d.cap("sasl") {
            self.reply(id, "904", &["SASL authentication failed"]);
            return;
        }
        let p = msg.param(0).to_string();
        match std::mem::replace(&mut d.sasl, SaslState::Idle) {
            SaslState::Idle => {
                if p.eq_ignore_ascii_case("PLAIN") {
                    d.sasl = SaslState::Plain(String::new());
                    self.send_to(id, &Message::new("AUTHENTICATE", ["+"]));
                } else if p == "*" {
                    self.reply(id, "906", &["SASL authentication aborted"]);
                } else {
                    self.reply(id, "908", &["PLAIN", "are available SASL mechanisms"]);
                    self.reply(id, "904", &["SASL authentication failed"]);
                }
            }
            SaslState::Plain(mut buf) => {
                if p == "*" {
                    self.reply(id, "906", &["SASL authentication aborted"]);
                    self.try_register(id);
                    return;
                }
                if p != "+" {
                    buf.push_str(&p);
                }
                if buf.len() > 4096 {
                    self.reply(id, "905", &["SASL message too long"]);
                    return;
                }
                if p.len() == 400 {
                    d.sasl = SaslState::Plain(buf);
                    return;
                }
                let result = auth::decode_sasl_plain(&buf).and_then(|(authz, authc, pw)| {
                    let login = auth::parse_login(&authc);
                    if !authz.is_empty()
                        && !authz.eq_ignore_ascii_case(&login.user)
                        && auth::parse_login(&authz).user != login.user
                    {
                        return None;
                    }
                    Some((login, pw))
                });
                let Some((login, pw)) = result else {
                    self.reply(id, "904", &["SASL authentication failed"]);
                    self.try_register(id);
                    return;
                };
                match self.check_credentials(id, &login.user, &pw) {
                    Some(name) => {
                        let nick = self.downs.get(&id).map(|d| d.nick.clone()).unwrap_or_default();
                        if let Some(d) = self.downs.get_mut(&id) {
                            d.user = Some(name.clone());
                            d.sasl_login = Some(Login { user: name.clone(), ..login });
                        }
                        let mask = format!("{nick}!{name}@{}", self.cfg.server_name);
                        self.reply(id, "900", &[&mask, &name, &format!("You are now logged in as {name}")]);
                        self.reply(id, "903", &["SASL authentication successful"]);
                    }
                    None => self.reply(id, "904", &["SASL authentication failed"]),
                }
                self.try_register(id);
            }
        }
    }

    fn try_register(&mut self, id: u32) {
        let Some(d) = self.downs.get(&id) else { return };
        if d.registered
            || d.closing
            || d.cap_lock
            || matches!(d.sasl, SaslState::Plain(_))
            || d.nick == "*"
            || d.username.is_none()
        {
            return;
        }
        self.complete_registration(id);
    }

    fn reject(&mut self, id: u32, reason: &str) {
        self.reply(id, "464", &[reason]);
        self.close_client(id, reason);
    }

    fn complete_registration(&mut self, id: u32) {
        let Some(d) = self.downs.get(&id) else { return };
        let (login, user) = if let (Some(l), Some(u)) = (d.sasl_login.clone(), d.user.clone()) {
            (l, u)
        } else {
            let (login, password) = match d.pass.as_deref() {
                Some(p) => match auth::parse_pass(p) {
                    Some(x) => x,
                    None => (auth::parse_login(d.username.as_deref().unwrap_or("")), p.to_string()),
                },
                None => {
                    self.reject(id, "Password required (PASS user:password, or SASL PLAIN)");
                    return;
                }
            };
            match self.check_credentials(id, &login.user, &password) {
                Some(u) => (login, u),
                None => {
                    self.reject(id, "Invalid username or password");
                    return;
                }
            }
        };
        let Some(d) = self.downs.get(&id) else { return };
        let user_rt = self.users.get(&user.to_ascii_lowercase());
        let mut want = d.bind.clone().or(login.network.clone());
        // Un client qui ne connaît pas bouncer-networks, avec un seul réseau : lié d'office.
        if want.is_none()
            && !d.cap("soju.im/bouncer-networks")
            && let Some(u) = user_rt
        {
            let nets = u.store.list();
            if nets.len() == 1 {
                want = Some(nets[0].id.clone());
            }
        }
        let netid = match want {
            Some(w) => match user_rt.and_then(|u| u.store.find(&w)) {
                Some(n) => Some(n),
                None => {
                    self.fail(id, &["BOUNCER", "INVALID_NETID", "BIND", &w, "Network not found"]);
                    self.reject(id, &format!("Unknown network: {w}"));
                    return;
                }
            },
            None => None,
        };
        let key = netid.as_ref().map(|n| NetKey::new(&user, n));
        let up_nick = key.as_ref().and_then(|k| self.ups.get(k)).map(|u| u.session.nick().to_string());
        let Some(d) = self.downs.get_mut(&id) else { return };
        d.user = Some(user.clone());
        d.client_name = login.client.clone();
        d.net = netid.clone();
        d.registered = true;
        if let Some(n) = up_nick {
            d.nick = n;
        }
        log::info!(
            "appareil {id} ({}) : {user}{}{}",
            d.peer,
            netid.as_deref().map(|n| format!(" lié au réseau {n}")).unwrap_or_default(),
            login.client.as_deref().map(|c| format!(", {c}")).unwrap_or_default()
        );
        let notify = d.cap("soju.im/bouncer-networks-notify");
        self.welcome(id);
        if let Some(key) = &key {
            self.attach_burst(id, key);
            self.update_auto_away(key);
        }
        if notify {
            self.send_network_list(id);
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Accueil d'un appareil
    // -----------------------------------------------------------------------------------------------------------

    fn own_prefix(up: &Upstream) -> String {
        let s = &up.session;
        match (s.user(), s.host()) {
            (Some(u), Some(h)) => format!("{}!{u}@{h}", s.nick()),
            _ => s.nick().to_string(),
        }
    }

    fn isupport_tokens(&self, key: Option<&NetKey>) -> Vec<String> {
        let up = key.and_then(|k| self.ups.get(k));
        let mut map: BTreeMap<String, String> = match up {
            Some(u) if u.session.is_registered() => u.session.isupport().raw.clone(),
            _ => BTreeMap::new(),
        };
        let defaults =
            [("CASEMAPPING", "rfc1459"), ("CHANTYPES", "#&"), ("PREFIX", "(ov)@+"), ("CHANMODES", "b,k,l,imnpst")];
        for (k, v) in defaults {
            map.entry(k.to_string()).or_insert_with(|| v.to_string());
        }
        map.remove("BOUNCER_NETID");
        if self.history.options().enabled {
            map.insert("CHATHISTORY".into(), CHATHISTORY_MAX.to_string());
            map.insert("MSGREFTYPES".into(), "timestamp,msgid".into());
        } else {
            map.remove("CHATHISTORY");
            map.remove("MSGREFTYPES");
        }
        if let (Some(k), Some(u)) = (key, up) {
            map.insert("BOUNCER_NETID".into(), k.net.clone());
            map.entry("NETWORK".into()).or_insert_with(|| u.net.name().to_string());
        } else {
            map.remove("NETWORK");
        }
        map.into_iter().map(|(k, v)| if v.is_empty() { k } else { format!("{k}={}", isupport_escape(&v)) }).collect()
    }

    fn isupport_messages(&self, nick: &str, tokens: &[String]) -> Vec<Message> {
        let mut out = Vec::new();
        let mut cur: Vec<String> = Vec::new();
        let mut len = 0;
        for t in tokens {
            if !cur.is_empty() && (cur.len() >= 12 || len + t.len() + 1 > 350) {
                out.push(std::mem::take(&mut cur));
                len = 0;
            }
            len += t.len() + 1;
            cur.push(t.clone());
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out.into_iter()
            .map(|toks| {
                let mut p = vec![nick.to_string()];
                p.extend(toks);
                p.push("are supported by this server".into());
                srv(&self.cfg.server_name, "005", p)
            })
            .collect()
    }

    fn welcome(&mut self, id: u32) {
        let Some(d) = self.downs.get(&id) else { return };
        let nick = d.nick.clone();
        let key = self.key_of(id);
        let up = key.as_ref().and_then(|k| self.ups.get(k));
        let sn = self.server_name();
        let full = up.filter(|u| u.session.is_registered()).map(Self::own_prefix).unwrap_or_else(|| nick.clone());
        let welcome = match up {
            Some(u) => format!("Welcome to the {} IRC network via the AlpIRC relay, {full}", u.net.name()),
            None => format!("Welcome to the AlpIRC relay, {full}"),
        };
        let version = format!("alpirc-relay-{VERSION}");
        self.reply(id, "001", &[&welcome]);
        self.reply(id, "002", &[&format!("Your host is {sn}, running {version}")]);
        self.reply(id, "003", &[&format!("This server was created {}", format_time(self.started))]);
        self.reply(id, "004", &[&sn, &version, "iowx", "biklmnopstv"]);
        let tokens = self.isupport_tokens(key.as_ref());
        for m in self.isupport_messages(&nick, &tokens) {
            self.send_to(id, &m);
        }
        // MOTD.
        self.reply(id, "375", &[&format!("- {sn} Message of the day -")]);
        let mut lines: Vec<String> =
            self.cfg.motd.as_deref().map(|m| m.lines().map(str::to_string).collect()).unwrap_or_default();
        match up {
            Some(u) => lines.push(format!("Network {} ({}): {}", u.net.name(), u.net.id, u.state.attr())),
            None => {
                let user = self.downs.get(&id).and_then(|d| d.user.clone()).unwrap_or_default();
                let nets: Vec<String> = self
                    .users
                    .get(&user.to_ascii_lowercase())
                    .map(|u| u.store.list().iter().map(|n| format!("{} ({})", n.name(), n.id)).collect())
                    .unwrap_or_default();
                lines.push("This connection is not bound to a network.".into());
                if !nets.is_empty() {
                    lines.push(format!("Your networks: {}", nets.join(", ")));
                }
                lines.push(format!("Bind with BOUNCER BIND <id>, or log in as {user}/<network>."));
            }
        }
        for l in lines {
            self.reply(id, "372", &[&format!("- {l}")]);
        }
        self.reply(id, "376", &["End of /MOTD command."]);
        if let Some(u) = up.filter(|u| u.session.is_registered()) {
            let modes = u.session.user_modes().to_string();
            let away = u.session.away().is_some() && !u.auto_away;
            if !modes.is_empty() {
                self.reply(id, "221", &[&format!("+{modes}")]);
            }
            if away {
                self.reply(id, "306", &["You have been marked as being away"]);
            }
        }
    }

    /// L'état du réseau pour un appareil qui s'y lie : canaux, sujets, membres, marques de lecture.
    fn attach_burst(&mut self, id: u32, key: &NetKey) {
        let Some(up) = self.ups.get(key) else { return };
        let Some(d) = self.downs.get(&id) else { return };
        let ext = d.cap("extended-join");
        let marker = d.cap("draft/read-marker");
        let prefix = Self::own_prefix(up);
        let account = up.session.account().unwrap_or("*").to_string();
        let realname = up.session.config().realname.clone();
        let channels: Vec<ChannelState> = if up.session.is_registered() {
            up.session.channels().filter(|c| c.joined).cloned().collect()
        } else {
            Vec::new()
        };
        let ready = up.state == UpState::Ready;
        let state_text = format!(
            "{} is {}{}",
            up.net.name(),
            up.state.attr(),
            up.error.as_deref().map(|e| format!(" ({e})")).unwrap_or_default()
        );
        for ch in channels {
            let join = if ext {
                Message::new("JOIN", [ch.name.clone(), account.clone(), realname.clone()])
            } else {
                Message::new("JOIN", [ch.name.clone()])
            };
            self.send_to(id, &join.with_source(&prefix));
            self.send_topic(id, key, &ch, false);
            for m in self.names_messages(id, key, &ch) {
                self.send_to(id, &m);
            }
            if marker {
                self.send_marker(id, key, &ch.name);
            }
            self.reply(id, "366", &[&ch.name, "End of /NAMES list."]);
        }
        if !ready {
            self.notice(id, &state_text);
        }
    }

    fn send_topic(&self, id: u32, key: &NetKey, ch: &ChannelState, explicit: bool) {
        if !self.ups.contains_key(key) {
            return;
        }
        match &ch.topic {
            Some(t) => {
                self.reply(id, "332", &[&ch.name, t]);
                if let (Some(by), Some(at)) = (&ch.topic_setter, ch.topic_time) {
                    self.reply(id, "333", &[&ch.name, by, &at.to_string()]);
                }
            }
            None if explicit => self.reply(id, "331", &[&ch.name, "No topic is set"]),
            None => {}
        }
    }

    /// Lignes 353 d'un canal (tous les préfixes et les adresses ; [`Downstream::render`] retire ce que
    /// l'appareil n'a pas demandé).
    fn names_messages(&self, id: u32, key: &NetKey, ch: &ChannelState) -> Vec<Message> {
        let (Some(up), Some(d)) = (self.ups.get(key), self.downs.get(&id)) else { return Vec::new() };
        let is = up.session.isupport();
        let sym = if ch.has_mode('s') {
            "@"
        } else if ch.has_mode('p') {
            "*"
        } else {
            "="
        };
        let mut entries = Vec::new();
        for m in ch.members.values() {
            let mut e: String = m.modes.chars().filter_map(|c| is.symbol_for_mode(c)).collect();
            e.push_str(&m.nick);
            if let Some(u) = up.session.user_info(&m.nick)
                && let (Some(user), Some(host)) = (&u.user, &u.host)
            {
                e.push_str(&format!("!{user}@{host}"));
            }
            entries.push(e);
        }
        let base = self.cfg.server_name.len() + d.nick.len() + ch.name.len() + 16;
        let mut out = Vec::new();
        let mut cur = String::new();
        for e in entries {
            if !cur.is_empty() && base + cur.len() + e.len() + 1 > 500 {
                out.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(&e);
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out.into_iter()
            .map(|l| srv(&self.cfg.server_name, "353", vec![d.nick.clone(), sym.to_string(), ch.name.clone(), l]))
            .collect()
    }

    fn send_marker(&mut self, id: u32, key: &NetKey, target: &str) {
        let fold = self.ups.get(key).map(|u| u.session.fold(target)).unwrap_or_else(|| target.to_ascii_lowercase());
        let ts = self.markers.get(&key.user, &key.net, &fold);
        let v = ts.map(|t| format!("timestamp={}", format_time(t))).unwrap_or_else(|| "*".into());
        self.send_to(id, &srv(&self.cfg.server_name, "MARKREAD", vec![target.to_string(), v]));
    }

    // -----------------------------------------------------------------------------------------------------------
    // Commandes des appareils enregistrés
    // -----------------------------------------------------------------------------------------------------------

    fn client_cmd(&mut self, id: u32, msg: Message) {
        let cmd = msg.command.clone();
        match cmd.as_str() {
            "PING" => {
                let sn = self.server_name();
                self.send_to(id, &srv(&sn, "PONG", vec![sn.clone(), msg.param(0).to_string()]));
                return;
            }
            "PONG" => return,
            "QUIT" => {
                self.close_client(id, "Detached (the relay stays connected)");
                return;
            }
            "CAP" => {
                self.cap_cmd(id, &msg);
                return;
            }
            "PASS" | "USER" => {
                self.reply(id, "462", &["You may not reregister"]);
                return;
            }
            "AUTHENTICATE" => {
                self.reply(id, "907", &["You have already authenticated"]);
                return;
            }
            "BOUNCER" => {
                self.bouncer_cmd(id, &msg);
                return;
            }
            "CHATHISTORY" => {
                self.chathistory_cmd(id, &msg);
                return;
            }
            "MARKREAD" => {
                self.markread_cmd(id, &msg);
                return;
            }
            _ => {}
        }
        let Some(key) = self.key_of(id) else {
            self.unbound_cmd(id, &msg);
            return;
        };
        let Some(up) = self.ups.get(&key) else { return };
        if !up.registered() {
            let t = format!(
                "Not connected to {} ({}){}",
                up.net.name(),
                up.state.attr(),
                up.error.as_deref().map(|e| format!(": {e}")).unwrap_or_default()
            );
            self.notice(id, &t);
            return;
        }
        match cmd.as_str() {
            "PRIVMSG" | "NOTICE" | "TAGMSG" => self.client_message(id, &key, &msg),
            "JOIN" => {
                if let Some(up) = self.ups.get_mut(&key) {
                    let names: Vec<&str> = msg.param(0).split(',').collect();
                    let keys: Vec<&str> = msg.param(1).split(',').collect();
                    for (i, n) in names.iter().enumerate() {
                        if let Some(k) = keys.get(i).filter(|k| !k.is_empty()) {
                            let f = up.session.fold(n);
                            up.pending_keys.insert(f, k.to_string());
                        }
                    }
                }
                self.forward(id, &key, &msg);
            }
            "MODE" | "NAMES" | "TOPIC" | "WHO" if self.local_reply(id, &key, &msg) => {}
            "AWAY" => {
                if let Some(up) = self.ups.get_mut(&key) {
                    let m = msg.params.first().filter(|m| !m.is_empty()).cloned();
                    up.user_away = m.clone();
                    up.auto_away = false;
                    up.session.set_away(m.as_deref());
                }
                self.flush_session(&key);
            }
            _ => self.forward(id, &key, &msg),
        }
    }

    /// Une commande d'un appareil vers le réseau (étiquettes des clients gardées si le réseau les accepte).
    fn forward(&mut self, id: u32, key: &NetKey, msg: &Message) {
        let now = self.now;
        let Some(up) = self.ups.get_mut(key) else { return };
        let tags = if up.session.cap("message-tags") { tags_of(&client_tags(&msg.tags)) } else { Tags::default() };
        let out = Message { tags, source: None, command: msg.command.clone(), params: msg.params.clone() };
        let line = out.to_line();
        let body = match line.strip_prefix('@') {
            Some(r) => r.split_once(' ').map(|(_, b)| b.len()).unwrap_or(0),
            None => line.len(),
        };
        if body > 510 {
            self.reply(id, "417", &["Input line was too long"]);
            return;
        }
        if let Some((kind, target)) = routing::classify(msg, up.session.casemapping()) {
            up.pending.push(id, kind, target, now);
        }
        self.up_send(key, &out);
    }

    /// PRIVMSG/NOTICE/TAGMSG d'un appareil : au réseau, à l'historique, et aux autres appareils.
    fn client_message(&mut self, id: u32, key: &NetKey, msg: &Message) {
        let now = self.now;
        let cmd = msg.command.as_str();
        if msg.params.is_empty() || msg.param(0).is_empty() {
            self.reply(id, "411", &[&format!("No recipient given ({cmd})")]);
            return;
        }
        if cmd != "TAGMSG" && msg.params.len() < 2 {
            self.reply(id, "412", &["No text to send"]);
            return;
        }
        let text = msg.param(1).to_string();
        let tags = client_tags(&msg.tags);
        let echo_self = self.downs.get(&id).is_some_and(|d| d.cap("echo-message"));
        for target in msg.param(0).split(',').filter(|t| !t.is_empty()) {
            let Some(up) = self.ups.get_mut(key) else { return };
            let up_tags = up.session.cap("message-tags");
            if cmd == "TAGMSG" && !up_tags {
                continue;
            }
            if cmd == "NOTICE"
                && let Some(c) = ctcp_command(&text)
                && !up.ctcp_reply_allowed(target, &c, now)
            {
                // Plusieurs appareils répondent au même CTCP : une seule réponse part.
                continue;
            }
            let mut params = vec![target.to_string()];
            if cmd != "TAGMSG" {
                params.push(text.clone());
            }
            let out = Message {
                tags: if up_tags { tags_of(&tags) } else { Tags::default() },
                source: None,
                command: cmd.to_string(),
                params: params.clone(),
            };
            let line = out.to_line();
            let body = match line.strip_prefix('@') {
                Some(r) => r.split_once(' ').map(|(_, b)| b.len()).unwrap_or(0),
                None => line.len(),
            };
            if body > 510 {
                self.reply(id, "417", &["Input line was too long"]);
                return;
            }
            self.up_send(key, &out);
            // Écho : historique et autres appareils.
            let Some(up) = self.ups.get(key) else { return };
            let prefix = Self::own_prefix(up);
            let stripped = target.trim_start_matches(|c| up.session.isupport().statusmsg.contains(c));
            let buf = if up.session.is_channel(stripped) { stripped.to_string() } else { target.to_string() };
            let fold = up.session.fold(&buf);
            let storable = match cmd {
                "TAGMSG" => storable_tagmsg(&tags),
                _ => storable_text(&text),
            };
            let (mid, t) = if storable {
                self.history.append(&key.user, &key.net, &fold, &buf, now, &prefix, cmd, &params, tags.clone())
            } else {
                (self.history.new_id(now), now)
            };
            let mut echo_tags = tags_of(&tags);
            echo_tags.set("time", Some(&format_time(t)));
            echo_tags.set("msgid", Some(&mid));
            let echo = Message { tags: echo_tags, source: Some(Source::new(prefix)), command: cmd.to_string(), params };
            for c in self.clients_of(key) {
                if c != id || echo_self {
                    self.send_to(c, &echo);
                }
            }
        }
    }

    /// Réponses tirées de l'état connu (MODE, NAMES, TOPIC, WHO d'un canal rejoint), sans déranger le réseau :
    /// un appareil qui se lie en demande souvent pour chaque canal.
    fn local_reply(&mut self, id: u32, key: &NetKey, msg: &Message) -> bool {
        let Some(up) = self.ups.get(key) else { return false };
        let sess = &up.session;
        let target = msg.param(0);
        let Some(ch) = sess.channel(target).filter(|c| c.joined && c.synced).cloned() else { return false };
        match msg.command.as_str() {
            "MODE" if msg.params.len() == 1 => {
                if ch.modes.is_empty() && ch.created.is_none() {
                    return false;
                }
                let modes = if ch.modes.is_empty() { "+".to_string() } else { ch.mode_string(true) };
                let mut rest: Vec<String> = vec![ch.name.clone()];
                rest.extend(modes.split(' ').map(str::to_string));
                let refs: Vec<&str> = rest.iter().map(String::as_str).collect();
                self.reply(id, "324", &refs);
                if let Some(c) = ch.created {
                    self.reply(id, "329", &[&ch.name, &c.to_string()]);
                }
                true
            }
            "NAMES" if msg.params.len() == 1 && !target.contains(',') => {
                for m in self.names_messages(id, key, &ch) {
                    self.send_to(id, &m);
                }
                self.reply(id, "366", &[&ch.name, "End of /NAMES list."]);
                true
            }
            "TOPIC" if msg.params.len() == 1 => {
                self.send_topic(id, key, &ch, true);
                true
            }
            "WHO" => {
                let fields = msg.params.get(1).map(String::as_str);
                let whox = match fields {
                    None => None,
                    Some(f) if f.starts_with('%') => Some(f.trim_start_matches('%').to_string()),
                    Some(_) => return false,
                };
                let is = sess.isupport();
                let server =
                    if sess.server_name().is_empty() { "*".to_string() } else { sess.server_name().to_string() };
                let mut rows = Vec::new();
                for m in ch.members.values() {
                    let Some(u) = sess.user_info(&m.nick) else { return false };
                    let (Some(user), Some(host), Some(real)) = (&u.user, &u.host, &u.realname) else { return false };
                    let mut flags = if u.away { "G".to_string() } else { "H".to_string() };
                    flags.extend(m.modes.chars().filter_map(|c| is.symbol_for_mode(c)));
                    rows.push((m.nick.clone(), user.clone(), host.clone(), real.clone(), u.account.clone(), flags));
                }
                let sn = self.server_name();
                let nick = self.downs.get(&id).map(|d| d.nick.clone()).unwrap_or_default();
                for (n, user, host, real, account, flags) in rows {
                    let m = match &whox {
                        None => srv(
                            &sn,
                            "352",
                            vec![
                                nick.clone(),
                                ch.name.clone(),
                                user,
                                host,
                                server.clone(),
                                n,
                                flags,
                                format!("0 {real}"),
                            ],
                        ),
                        Some(spec) => {
                            let (letters, token) = spec.split_once(',').unwrap_or((spec.as_str(), "0"));
                            let mut p = vec![nick.clone()];
                            for f in "tcuihsnfdlaor".chars().filter(|c| letters.contains(*c)) {
                                p.push(match f {
                                    't' => token.to_string(),
                                    'c' => ch.name.clone(),
                                    'u' => user.clone(),
                                    'i' => "255.255.255.255".into(),
                                    'h' => host.clone(),
                                    's' => server.clone(),
                                    'n' => n.clone(),
                                    'f' => flags.clone(),
                                    'd' | 'l' => "0".into(),
                                    'a' => account.clone().unwrap_or_else(|| "0".into()),
                                    'o' => "n/a".into(),
                                    _ => real.clone(),
                                });
                            }
                            srv(&sn, "354", p)
                        }
                    };
                    self.send_to(id, &m);
                }
                self.reply(id, "315", &[target, "End of /WHO list."]);
                true
            }
            _ => false,
        }
    }

    /// Commandes d'un appareil qui n'est lié à aucun réseau.
    fn unbound_cmd(&mut self, id: u32, msg: &Message) {
        let Some(d) = self.downs.get_mut(&id) else { return };
        let nick = d.nick.clone();
        match msg.command.as_str() {
            "NICK" => {
                let n = msg.param(0).to_string();
                if !n.is_empty() && !n.contains([' ', ',', '!', '@']) {
                    d.nick = n.clone();
                    self.send_to(id, &Message::new("NICK", [n]).with_source(&nick));
                }
            }
            "JOIN" => {
                for c in msg.param(0).split(',').filter(|c| !c.is_empty()) {
                    self.reply(id, "403", &[c, "This connection is not bound to a network (BOUNCER BIND)"]);
                }
            }
            "PRIVMSG" | "TAGMSG" => self.reply(id, "401", &[msg.param(0), "This connection is not bound to a network"]),
            "NOTICE" => {}
            "ISON" => self.reply(id, "303", &[""]),
            "USERHOST" => self.reply(id, "302", &[""]),
            "WHO" => self.reply(id, "315", &[msg.param(0), "End of /WHO list."]),
            "MODE" if msg.params.len() == 1 && !msg.param(0).starts_with(['#', '&']) => self.reply(id, "221", &["+"]),
            "MODE" => {}
            "AWAY" => {
                if msg.params.first().is_some_and(|m| !m.is_empty()) {
                    self.reply(id, "306", &["You have been marked as being away"]);
                } else {
                    self.reply(id, "305", &["You are no longer marked as being away"]);
                }
            }
            "LIST" => {
                self.reply(id, "321", &["Channel", "Users  Name"]);
                self.reply(id, "323", &["End of /LIST"]);
            }
            "MONITOR" => {}
            other => self.reply(id, "421", &[other, "Not bound to a network"]),
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // soju.im/bouncer-networks
    // -----------------------------------------------------------------------------------------------------------

    /// Attributs complets d'un réseau (avec `state`, et `error` le cas échéant).
    fn network_attrs(&self, user: &str, netid: &str) -> Vec<(String, String)> {
        let Some(u) = self.users.get(&user.to_ascii_lowercase()) else { return Vec::new() };
        let Some(n) = u.store.get(netid) else { return Vec::new() };
        let mut a = n.attributes(&u.cfg.name);
        let up = self.ups.get(&NetKey::new(&u.cfg.name, netid));
        a.push((s("state"), up.map(|x| x.state.attr()).unwrap_or("disconnected").to_string()));
        if let Some(e) = up.filter(|x| x.state != UpState::Ready).and_then(|x| x.error.clone()) {
            a.push((s("error"), e));
        }
        a
    }

    /// `BOUNCER NETWORK` à tous les appareils de l'utilisateur qui ont demandé les notifications.
    fn notify_network(&self, user: &str, netid: &str, attrs: Option<Vec<(String, String)>>) {
        let value = match &attrs {
            Some(a) => encode_attrs(a),
            None => "*".to_string(),
        };
        let m = srv(&self.cfg.server_name, "BOUNCER", vec![s("NETWORK"), netid.to_string(), value]);
        let mut ids: Vec<u32> = self
            .downs
            .values()
            .filter(|d| {
                d.registered
                    && !d.closing
                    && d.user.as_deref() == Some(user)
                    && d.cap("soju.im/bouncer-networks-notify")
            })
            .map(|d| d.id)
            .collect();
        ids.sort_unstable();
        for id in ids {
            self.send_to(id, &m);
        }
    }

    /// Changement d'état d'un réseau : seulement ce qui change (`state`, `error` posée à la coupure et retirée au
    /// retour), comme le veut la spécification ; les clients fusionnent avec ce qu'ils savent déjà.
    fn notify_state(&self, key: &NetKey) {
        let Some(up) = self.ups.get(key) else { return };
        let mut a = vec![(s("state"), up.state.attr().to_string())];
        match up.state {
            UpState::Disconnected => {
                if let Some(e) = &up.error {
                    a.push((s("error"), e.clone()));
                }
            }
            UpState::Ready => a.push((s("error"), String::new())),
            UpState::Connecting | UpState::Registering => {}
        }
        self.notify_network(&key.user, &key.net, Some(a));
    }

    fn send_network_list(&mut self, id: u32) {
        let Some(user) = self.downs.get(&id).and_then(|d| d.user.clone()) else { return };
        let batch = self.downs.get(&id).is_some_and(|d| d.cap("batch"));
        let nets: Vec<String> = self
            .users
            .get(&user.to_ascii_lowercase())
            .map(|u| u.store.list().into_iter().map(|n| n.id).collect())
            .unwrap_or_default();
        let bid = self.new_batch_id();
        let sn = self.server_name();
        if batch {
            self.send_to(id, &srv(&sn, "BATCH", vec![format!("+{bid}"), s("soju.im/bouncer-networks")]));
        }
        for n in nets {
            let attrs = self.network_attrs(&user, &n);
            let mut m = srv(&sn, "BOUNCER", vec![s("NETWORK"), n, encode_attrs(&attrs)]);
            if batch {
                m.tags.set("batch", Some(&bid));
            }
            self.send_to(id, &m);
        }
        if batch {
            self.send_to(id, &srv(&sn, "BATCH", vec![format!("-{bid}")]));
        }
    }

    fn bouncer_fail(&self, id: u32, sub: &str, netid: &str, e: &BouncerError) {
        let attr = e.attribute.clone().unwrap_or_else(|| "*".into());
        let mut p: Vec<&str> = vec!["BOUNCER", e.code, sub];
        match e.code {
            "NEED_ATTRIBUTE" => p.push(&attr),
            "INVALID_ATTRIBUTE" | "READ_ONLY_ATTRIBUTE" | "UNKNOWN_ATTRIBUTE" => {
                p.push(netid);
                p.push(&attr);
            }
            _ => p.push(netid),
        }
        p.push(&e.description);
        self.fail(id, &p);
    }

    fn bouncer_cmd(&mut self, id: u32, msg: &Message) {
        let sub = msg.param(0).to_ascii_uppercase();
        let Some(user) = self.downs.get(&id).and_then(|d| d.user.clone()) else { return };
        let sn = self.server_name();
        let ok = |core: &Core, verb: &str, netid: &str| {
            core.send_to(id, &srv(&sn, "BOUNCER", vec![verb.to_string(), netid.to_string()]))
        };
        match sub.as_str() {
            "BIND" => self.fail(
                id,
                &["BOUNCER", "REGISTRATION_IS_COMPLETED", "BIND", "Cannot bind to a network after registration"],
            ),
            "LISTNETWORKS" => self.send_network_list(id),
            "ADDNETWORK" => {
                let attrs = parse_attrs(msg.param(1));
                let lu = user.to_ascii_lowercase();
                let Some(u) = self.users.get_mut(&lu) else { return };
                match u.store.add(&attrs) {
                    Ok(netid) => {
                        let net = u.store.get(&netid);
                        let name = u.cfg.name.clone();
                        if let Some(net) = net {
                            let key = NetKey::new(&name, &netid);
                            let mut up = Upstream::new(
                                &name,
                                net,
                                self.cfg.upstream.flood_burst,
                                self.cfg.upstream.flood_interval_ms,
                                self.now,
                            );
                            // Connexion au prochain battement, après les réponses (ordre de l'exemple de soju).
                            up.reconnect_at = Some(self.now);
                            self.ups.insert(key, up);
                            log::info!("{name} : réseau {netid} ajouté depuis l'appareil {id}");
                        }
                        let attrs = self.network_attrs(&name, &netid);
                        self.notify_network(&name, &netid, Some(attrs));
                        ok(self, "ADDNETWORK", &netid);
                    }
                    Err(e) => self.bouncer_fail(id, "ADDNETWORK", "*", &e),
                }
            }
            "CHANGENETWORK" => {
                let netid = msg.param(1).to_string();
                let attrs = parse_attrs(msg.param(2));
                let lu = user.to_ascii_lowercase();
                let Some(u) = self.users.get_mut(&lu) else { return };
                match u.store.change(&netid, &attrs) {
                    Ok(change) => {
                        let name = u.cfg.name.clone();
                        let net = u.store.get(&netid);
                        let key = NetKey::new(&name, &netid);
                        if let (Some(up), Some(net)) = (self.ups.get_mut(&key), net) {
                            up.net = net;
                            if change.reconnect {
                                match up.conn {
                                    Some(conn) => {
                                        up.reconnect_now = true;
                                        if up.session.is_registered() {
                                            let q = Message::new("QUIT", ["Changing network settings"]).to_line();
                                            self.net.send(conn, up.encoding.encode(&q).into_owned());
                                        }
                                        self.net.close(conn);
                                    }
                                    None => {
                                        up.attempts = 0;
                                        up.reconnect_at = Some(self.now);
                                    }
                                }
                            } else if let Some(n) = change.nick.clone()
                                && up.registered()
                            {
                                self.up_send(&key, &Message::new("NICK", [n]));
                            }
                        }
                        self.notify_network(&name, &netid, Some(change.attributes));
                        ok(self, "CHANGENETWORK", &netid);
                    }
                    Err(e) => self.bouncer_fail(id, "CHANGENETWORK", &netid, &e),
                }
            }
            "DELNETWORK" => {
                let netid = msg.param(1).to_string();
                let lu = user.to_ascii_lowercase();
                let Some(u) = self.users.get_mut(&lu) else { return };
                match u.store.delete(&netid) {
                    Ok(()) => {
                        let name = u.cfg.name.clone();
                        let key = NetKey::new(&name, &netid);
                        if let Some(mut up) = self.ups.remove(&key) {
                            up.stopped = true;
                            if let Some(conn) = up.conn {
                                self.up_conns.remove(&conn);
                                if up.session.is_registered() {
                                    let q = Message::new("QUIT", ["Network removed"]).to_line();
                                    self.net.send(conn, up.encoding.encode(&q).into_owned());
                                }
                                self.net.close(conn);
                            }
                        }
                        for c in self.clients_of(&key) {
                            self.close_client(c, "Network removed");
                        }
                        self.markers.forget_network(&name, &netid);
                        self.notify_network(&name, &netid, None);
                        ok(self, "DELNETWORK", &netid);
                    }
                    Err(e) => self.bouncer_fail(id, "DELNETWORK", &netid, &e),
                }
            }
            _ => self.fail(id, &["BOUNCER", "UNKNOWN_COMMAND", &sub, "Unknown subcommand"]),
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // draft/chathistory
    // -----------------------------------------------------------------------------------------------------------

    fn chathistory_cmd(&mut self, id: u32, msg: &Message) {
        let sub = msg.param(0).to_ascii_uppercase();
        let Some(key) = self.key_of(id) else {
            self.fail(
                id,
                &["CHATHISTORY", "INVALID_TARGET", &sub, msg.param(1), "This connection is not bound to a network"],
            );
            return;
        };
        let invalid = |core: &Core, why: &str| core.fail(id, &["CHATHISTORY", "INVALID_PARAMS", &sub, why]);
        let limit_at = |i: usize| {
            msg.params.get(i).and_then(|l| l.parse::<usize>().ok()).filter(|l| *l > 0).map(|l| l.min(CHATHISTORY_MAX))
        };
        let parse_ref = |i: usize| msg.params.get(i).and_then(|r| Ref::parse(r));
        let cm = self.ups.get(&key).map(|u| u.session.casemapping()).unwrap_or_default();
        if sub == "TARGETS" {
            let (Some(Ref::Time(a)), Some(Ref::Time(b)), Some(limit)) = (parse_ref(1), parse_ref(2), limit_at(3))
            else {
                invalid(self, "Invalid parameters");
                return;
            };
            let list = self.history.targets(&key.user, &key.net, a, b, limit);
            let bid = self.new_batch_id();
            let sn = self.server_name();
            let batch = self.downs.get(&id).is_some_and(|d| d.cap("batch"));
            if batch {
                self.send_to(id, &srv(&sn, "BATCH", vec![format!("+{bid}"), s("draft/chathistory-targets")]));
            }
            for t in list {
                let mut m = srv(&sn, "CHATHISTORY", vec![s("TARGETS"), t.name, format_time(t.last)]);
                if batch {
                    m.tags.set("batch", Some(&bid));
                }
                self.send_to(id, &m);
            }
            if batch {
                self.send_to(id, &srv(&sn, "BATCH", vec![format!("-{bid}")]));
            }
            return;
        }
        let target = msg.param(1).to_string();
        if target.is_empty() || target == "*" {
            invalid(self, "Missing target");
            return;
        }
        let q = match sub.as_str() {
            "LATEST" => {
                let r = match msg.param(2) {
                    "*" => None,
                    other => match Ref::parse(other) {
                        Some(r) => Some(r),
                        None => {
                            invalid(self, "Invalid message reference");
                            return;
                        }
                    },
                };
                (Query::Latest(r), limit_at(3))
            }
            "BEFORE" | "AFTER" | "AROUND" => {
                let Some(r) = parse_ref(2) else {
                    invalid(self, "Invalid message reference");
                    return;
                };
                let q = match sub.as_str() {
                    "BEFORE" => Query::Before(r),
                    "AFTER" => Query::After(r),
                    _ => Query::Around(r),
                };
                (q, limit_at(3))
            }
            "BETWEEN" => {
                let (Some(a), Some(b)) = (parse_ref(2), parse_ref(3)) else {
                    invalid(self, "Invalid message reference");
                    return;
                };
                (Query::Between(a, b), limit_at(4))
            }
            _ => {
                invalid(self, "Unknown subcommand");
                return;
            }
        };
        let (q, Some(limit)) = q else {
            invalid(self, "Invalid limit");
            return;
        };
        let fold = cm.fold(&target);
        let entries = match self.history.query(&key.user, &key.net, &fold, &q, limit) {
            Ok(e) => e,
            Err(HistoryError::UnknownMsgid) => {
                self.fail(id, &["CHATHISTORY", "INVALID_PARAMS", &sub, &target, "Unknown message ID"]);
                return;
            }
        };
        let name = self.history.target_name(&key.user, &key.net, &fold).unwrap_or(target);
        let (batch, tags_ok) =
            self.downs.get(&id).map(|d| (d.cap("batch"), d.cap("message-tags"))).unwrap_or((false, false));
        let complete = entries.len() < limit;
        let bid = self.new_batch_id();
        let sn = self.server_name();
        if batch {
            let mut b = srv(&sn, "BATCH", vec![format!("+{bid}"), s("chathistory"), name]);
            if complete {
                b.tags.set("draft/chathistory-end", None);
            }
            self.send_to(id, &b);
        }
        for e in entries {
            if e.cmd == "TAGMSG" && !tags_ok {
                continue;
            }
            let mut m = e.to_message();
            if batch {
                m.tags.set("batch", Some(&bid));
            }
            self.send_to(id, &m);
        }
        if batch {
            self.send_to(id, &srv(&sn, "BATCH", vec![format!("-{bid}")]));
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // draft/read-marker
    // -----------------------------------------------------------------------------------------------------------

    fn markread_cmd(&mut self, id: u32, msg: &Message) {
        let target = msg.param(0).to_string();
        if target.is_empty() {
            self.fail(id, &["MARKREAD", "NEED_MORE_PARAMS", "Missing parameters"]);
            return;
        }
        let Some(key) = self.key_of(id) else {
            self.fail(id, &["MARKREAD", "INVALID_PARAMS", &target, "This connection is not bound to a network"]);
            return;
        };
        if msg.params.len() < 2 {
            self.send_marker(id, &key, &target);
            return;
        }
        let ts = msg.param(1).strip_prefix("timestamp=").and_then(parse_time);
        let Some(ts) = ts else {
            self.fail(id, &["MARKREAD", "INVALID_PARAMS", msg.param(1), "Invalid timestamp"]);
            return;
        };
        let fold = self.ups.get(&key).map(|u| u.session.fold(&target)).unwrap_or_else(|| target.to_ascii_lowercase());
        match self.markers.set(&key.user, &key.net, &fold, ts, self.now) {
            MarkResult::Updated(t) => {
                let m = srv(
                    &self.cfg.server_name,
                    "MARKREAD",
                    vec![target.clone(), format!("timestamp={}", format_time(t))],
                );
                for c in self.clients_of(&key) {
                    if self.downs.get(&c).is_some_and(|d| d.cap("draft/read-marker")) {
                        self.send_to(c, &m);
                    }
                }
            }
            MarkResult::Unchanged(_) => self.send_marker(id, &key, &target),
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Minuteries
    // -----------------------------------------------------------------------------------------------------------

    fn tick(&mut self) {
        let now = self.now;
        let (interval, timeout) =
            (self.cfg.upstream.ping_interval_secs as i64 * 1000, self.cfg.upstream.ping_timeout_secs as i64 * 1000);
        let keys: Vec<NetKey> = self.ups.keys().cloned().collect();
        for key in keys {
            let Some(up) = self.ups.get_mut(&key) else { continue };
            if up.state == UpState::Disconnected {
                if up.reconnect_at.is_some_and(|t| t <= now) {
                    self.up_connect(&key);
                }
                continue;
            }
            let Some(conn) = up.conn else { continue };
            for l in up.bucket.drain(now) {
                self.net.send(conn, l);
            }
            let registering = matches!(up.state, UpState::Connecting | UpState::Registering) && !up.session.motd_done();
            if registering && now - up.state_since > UPSTREAM_REGISTRATION_TIMEOUT_MS {
                up.close_reason = Some("Registration timeout".into());
                self.net.close(conn);
                continue;
            }
            if up.state == UpState::Connecting {
                continue;
            }
            let idle = now - up.last_rx;
            if idle > timeout.max(10_000) {
                up.close_reason = Some("Ping timeout".into());
                self.net.close(conn);
            } else if idle > interval.max(5_000) && up.ping_sent.is_none() {
                up.ping_sent = Some(now);
                let l = up.encoding.encode("PING :alpirc-relay").into_owned();
                self.net.send(conn, l);
            }
        }
        let ids: Vec<u32> = self.downs.keys().copied().collect();
        for id in ids {
            let Some(d) = self.downs.get_mut(&id) else { continue };
            if d.closing {
                continue;
            }
            if !d.registered {
                if now - d.connected_at > CLIENT_REGISTRATION_TIMEOUT_MS {
                    self.close_client(id, "Registration timeout");
                }
                continue;
            }
            let idle = now - d.last_rx;
            if idle > CLIENT_TIMEOUT_MS {
                self.close_client(id, "Ping timeout");
            } else if idle > CLIENT_PING_MS && d.ping_sent.is_none() {
                d.ping_sent = Some(now);
                let sn = self.server_name();
                self.net.send(id, format!("PING :{sn}").into_bytes());
            }
        }
        if now - self.last_cleanup > 86_400_000 {
            self.last_cleanup = now;
            let n = self.history.cleanup(now);
            if n > 0 {
                log::info!("historique : {n} fichier(s) retiré(s) ou raccourci(s)");
            }
            self.throttle.prune(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attrs_encoding() {
        let a = vec![(s("name"), s("Libera Chat")), (s("state"), s("connected")), (s("realname"), String::new())];
        let e = encode_attrs(&a);
        assert_eq!(e, "name=Libera\\sChat;state=connected;realname");
        assert_eq!(parse_attrs(&e)[0], (s("name"), s("Libera Chat")));
    }

    #[test]
    fn ctcp_helpers() {
        assert_eq!(ctcp_command("\x01VERSION\x01").as_deref(), Some("VERSION"));
        assert_eq!(ctcp_command("\x01ACTION danse\x01").as_deref(), Some("ACTION"));
        assert_eq!(ctcp_command("salut"), None);
        assert!(storable_text("salut"));
        assert!(storable_text("\x01ACTION danse\x01"));
        assert!(!storable_text("\x01VERSION\x01"));
        let mut t = BTreeMap::new();
        t.insert(s("+typing"), s("active"));
        assert!(!storable_tagmsg(&t));
        t.insert(s("+draft/react"), s("👍"));
        assert!(storable_tagmsg(&t));
    }

    #[test]
    fn times() {
        let now = 1_000_000_000_000;
        let m = Message::parse("@time=2001-09-09T01:46:40.000Z :a PRIVMSG #a x").unwrap();
        assert_eq!(msg_time(&m, now + 5), now);
        let m = Message::parse("@time=2099-01-01T00:00:00.000Z :a PRIVMSG #a x").unwrap();
        assert_eq!(msg_time(&m, now), now, "heure du futur ignorée");
        assert_eq!(fingerprint_colons("ab01"), "AB:01");
        assert_eq!(isupport_escape("Rizon IRC"), "Rizon\\x20IRC");
    }

    #[test]
    fn pem_fingerprint() {
        let (cert, _key, fp) = alpirc_net::generate_self_signed(&["relay.test".to_string()]).unwrap();
        assert_eq!(fingerprint_of_pem(&cert).unwrap(), fp);
        assert!(fingerprint_of_pem(b"rien").is_err());
    }
}
