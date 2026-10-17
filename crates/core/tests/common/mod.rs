//! Banc d'essai : un client avec un dossier de données temporaire, des réglages de test (sans horodatage, sans
//! anti-flood), un réseau `TestNet`, et des aides pour simuler le serveur.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use alpirc_core::settings::{Network, Server};
use alpirc_core::{Client, ConnId, Effect, Settings, WinId, WinKind};

pub const T0: i64 = 1_760_000_000_000;

static N: AtomicUsize = AtomicUsize::new(0);

/// Dossier temporaire neuf.
pub fn tmpdir() -> PathBuf {
    let n = N.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("alpirc-core-test-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Réglages des essais.
pub fn settings() -> Settings {
    let mut s = Settings::default();
    s.irc.timestamps = false;
    s.flood.enabled = false;
    s.language = Some("en".into());
    s.networks.push(Network::new("TestNet", vec![Server::new("irc.test.net", 6667, false)]));
    s
}

pub struct H {
    pub c: Client,
    pub now: i64,
    pub conn: ConnId,
    pub status: WinId,
    pub dir: PathBuf,
    pub fx: Vec<Effect>,
}

impl H {
    pub fn new() -> H {
        H::with(settings())
    }

    pub fn with(s: Settings) -> H {
        H::with_dir(s, tmpdir())
    }

    pub fn with_dir(s: Settings, dir: PathBuf) -> H {
        let mut c = Client::new(s, dir.clone(), T0);
        c.set_utc_offset(Some(0));
        let status = c.windows()[0].id;
        let conn = c.windows()[0].conn.unwrap();
        let mut h = H { c, now: T0, conn, status, dir, fx: Vec::new() };
        h.pull();
        h.fx.clear();
        h
    }

    pub fn pull(&mut self) {
        let e = self.c.take_effects();
        self.fx.extend(e);
    }

    /// Lignes envoyées depuis le dernier appel.
    pub fn sends(&mut self) -> Vec<String> {
        self.pull();
        let mut out = Vec::new();
        self.fx.retain(|e| match e {
            Effect::Send { line, .. } => {
                out.push(line.clone());
                false
            }
            _ => true,
        });
        out
    }

    /// Lignes envoyées par une connexion donnée.
    pub fn sends_for(&mut self, conn: ConnId) -> Vec<String> {
        self.pull();
        let mut out = Vec::new();
        self.fx.retain(|e| match e {
            Effect::Send { conn: c, line } if *c == conn => {
                out.push(line.clone());
                false
            }
            _ => true,
        });
        out
    }

    /// Autres effets depuis le dernier appel.
    pub fn effects(&mut self) -> Vec<Effect> {
        self.pull();
        let v: Vec<Effect> = self.fx.iter().filter(|e| !matches!(e, Effect::Send { .. })).cloned().collect();
        self.fx.retain(|e| matches!(e, Effect::Send { .. }));
        v
    }

    pub fn clear(&mut self) {
        self.pull();
        self.fx.clear();
    }

    /// `connect` vers TestNet puis `on_connected`.
    pub fn connect(&mut self) {
        self.c.connect(self.status, "TestNet", None, self.now);
        self.c.on_connected(self.conn, self.now);
    }

    /// Ligne du serveur.
    pub fn feed(&mut self, line: &str) {
        self.c.on_line(self.conn, line.as_bytes(), self.now);
    }

    pub fn feed_conn(&mut self, conn: ConnId, line: &str) {
        self.c.on_line(conn, line.as_bytes(), self.now);
    }

    /// Enregistrement complet sans capacités (001, 005, fin du MOTD).
    pub fn register(&mut self) {
        self.register_with("PREFIX=(ov)@+ CHANTYPES=# NETWORK=TestNet MODES=3");
    }

    pub fn register_with(&mut self, isupport: &str) {
        self.feed(":irc.test.net 001 Guiwald :Welcome to TestNet Guiwald!alpirc@host.test");
        self.feed(&format!(":irc.test.net 005 Guiwald {isupport} :are supported by this server"));
        self.feed(":irc.test.net 376 Guiwald :End of /MOTD command.");
    }

    /// Connexion et enregistrement, en annonçant ces capacités (toutes acceptées).
    pub fn up_with_caps(&mut self, caps: &str) {
        self.connect();
        self.feed(&format!(":irc.test.net CAP * LS :{caps}"));
        let req: Vec<String> = self.sends().into_iter().filter(|l| l.starts_with("CAP REQ")).collect();
        for r in req {
            let list = r.trim_start_matches("CAP REQ ").trim_start_matches(':').to_string();
            self.feed(&format!(":irc.test.net CAP Guiwald ACK :{list}"));
        }
        self.register();
        self.clear();
    }

    /// Connecté, enregistré, effets vidés.
    pub fn up(&mut self) {
        self.connect();
        self.register();
        self.clear();
    }

    /// Nous rejoignons un canal (avec ces pseudos dans NAMES, nous compris si besoin).
    pub fn join(&mut self, chan: &str, names: &str) -> WinId {
        self.feed(&format!(":Guiwald!alpirc@host.test JOIN {chan}"));
        self.feed(&format!(":irc.test.net 353 Guiwald = {chan} :{names}"));
        self.feed(&format!(":irc.test.net 366 Guiwald {chan} :End of /NAMES list."));
        let w = self.win(chan);
        self.c.set_active(w, self.now);
        self.clear();
        w
    }

    pub fn win(&self, name: &str) -> WinId {
        self.c.find_window(Some(self.conn), name).unwrap_or_else(|| panic!("pas de fenêtre {name}"))
    }

    pub fn has_win(&self, name: &str) -> bool {
        self.c.find_window(Some(self.conn), name).is_some()
    }

    pub fn lines(&self, win: WinId) -> Vec<String> {
        self.c.window(win).map(|w| w.lines.iter().map(|l| l.text.clone()).collect()).unwrap_or_default()
    }

    pub fn last(&self, win: WinId) -> String {
        self.lines(win).last().cloned().unwrap_or_default()
    }

    pub fn has_line(&self, win: WinId, text: &str) -> bool {
        self.lines(win).iter().any(|l| l == text)
    }

    pub fn status_lines(&self) -> Vec<String> {
        self.lines(self.status)
    }

    pub fn input(&mut self, win: WinId, text: &str) {
        self.c.input(win, text, self.now);
    }

    pub fn cmd(&mut self, win: WinId, text: &str) {
        self.c.command(win, text, self.now);
    }

    /// Avance l'horloge et appelle `tick`.
    pub fn advance(&mut self, ms: i64) -> Option<i64> {
        self.now += ms;
        self.c.tick(self.now)
    }

    pub fn nicks(&self, win: WinId) -> Vec<String> {
        self.c.nicklist(win).into_iter().map(|e| format!("{}{}", e.prefixes, e.nick)).collect()
    }

    pub fn kind_of(&self, name: &str) -> WinKind {
        self.c.window(self.win(name)).unwrap().kind
    }

    /// Effets `Connect` depuis le dernier appel : (conn, hôte, port, tls).
    pub fn connects(&mut self) -> Vec<(ConnId, String, u16, bool)> {
        self.effects()
            .into_iter()
            .filter_map(|e| match e {
                Effect::Connect { conn, host, port, tls, .. } => Some((conn, host, port, tls)),
                _ => None,
            })
            .collect()
    }
}

pub fn b64(s: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(s)
}

pub fn unb64(s: &str) -> String {
    use base64::Engine;
    String::from_utf8(base64::engine::general_purpose::STANDARD.decode(s).unwrap()).unwrap()
}
