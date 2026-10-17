//! Le client à la manière de mIRC, au-dessus des sessions : réglages, connexions, fenêtres et lignes, commandes,
//! carnet d'adresses (Notify, Ignore, Control), mots surlignés, journaux, URL catcher, `/list`, file d'envoi
//! protégée contre le flood, réponses CTCP, absence, reconnexion, perform, favoris, notifications, scripts mSL,
//! bouncers (soju), historique (`chathistory`) et marqueurs de lecture.
//!
//! Le client ne touche ni au réseau ni à l'interface : l'hôte (appli de bureau, Android) lui passe ce qui arrive
//! ([`Client::on_line`], [`Client::input`]…) et applique les [`Effect`] qu'il rend ([`Client::take_effects`]).
//! Tout ce qui dépend de l'heure reçoit `now_ms`.
//!
//! Organisation : `state` (l'état, sauf le moteur), `irc` (lignes du serveur → affichage et automatismes),
//! `conn` (connexions), `commands` (commandes intégrées), `host` (l'état vu par le moteur de scripts), `window`,
//! `flood`, `logging`, `util`, `defaults` (scripts du premier démarrage).

mod commands;
mod conn;
mod dcc;
mod defaults;
mod flood;
mod host;
mod irc;
pub mod logging;
mod state;
mod util;
mod window;

use std::path::{Path, PathBuf};

use alpirc_proto::Message;
use alpirc_proto::encoding::Encoding;
use alpirc_script::{Ctx, DialogEvent, Engine, EventInfo, EventKind, MenuItem, PopupKind, ScriptKind};

pub use self::commands::BUILTINS;
pub use self::dcc::{DccAnswer, DccId, DccPrompt, DccType, DccUi, Transfer, TransferState, state_label as dcc_state_label};
pub use self::irc::{HOMEPAGE, VERSION, version_reply};
pub use self::logging::{FileSink, LogSink};
pub use self::util::{contains_word, find_urls};
pub use self::window::{Activity, Line, LineKind, NickEntry, WinKind, Window};
use self::state::{Deferred, State};
use crate::i18n::{Key, Lang};
use crate::session::ChannelState;
use crate::settings::Settings;

/// Numéro d'une connexion (une par fenêtre Status, `$cid`).
pub type ConnId = u32;
/// Numéro d'une fenêtre.
pub type WinId = u32;

/// Vérification du certificat TLS demandée à l'hôte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsVerify {
    Full,
    AcceptInvalid,
    /// Empreinte SHA-256 (hexadécimal) du certificat attendu.
    Pinned(String),
}

/// Boîtes de dialogue de l'interface que le moteur demande d'ouvrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiDialog {
    /// Options, éventuellement sur une page (`display`…).
    Options(Option<String>),
    AddressBook,
    ChannelsFolder,
    ChannelCentral(WinId),
    ScriptEditor,
    Colors,
    About,
    Find(WinId),
    Connect,
}

/// Ce que l'hôte doit faire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Ouvrir une connexion TCP (+TLS) pour `conn` ; l'hôte répond par [`Client::on_connected`] ou
    /// [`Client::on_connect_failed`]. Une connexion déjà ouverte pour `conn` a été fermée avant (`Close`).
    Connect { conn: ConnId, host: String, port: u16, tls: bool, verify: TlsVerify, client_cert: Option<PathBuf> },
    /// Envoyer une ligne (sans fin de ligne), déjà cadencée par la file anti-flood : à envoyer tout de suite,
    /// encodée selon [`Client::conn_encoding`].
    Send { conn: ConnId, line: String },
    /// Fermer la connexion après avoir envoyé les lignes qui précèdent. Le moteur la tient déjà pour fermée :
    /// l'hôte signale sa fin comme d'habitude ([`Client::on_closed`] ou [`Client::on_connect_failed`], comme le
    /// fait `alpirc-net` pour toute tentative) et le moteur l'ignore, ainsi que les lignes de l'ancienne
    /// connexion qui arriveraient encore.
    Close { conn: ConnId },
    /// Notification du système (mot surligné, privé), seulement si les réglages le permettent.
    Notify { win: WinId, title: String, body: String },
    Beep,
    PlaySound(PathBuf),
    /// Faire clignoter la fenêtre ou l'icône (`/flash`, mot surligné).
    Flash { win: WinId },
    OpenUrl(String),
    /// Les réglages ont changé : l'hôte appelle `Settings::save`.
    SaveSettings,
    /// Scripts ou variables changés : l'hôte appelle [`Client::save_scripts`].
    SaveScripts,
    /// Une fenêtre devient l'active (`/query`, `/join` d'un nouveau canal, `/window -a`).
    Activate(WinId),
    /// Ouvrir une boîte de l'interface (`/options`, `/channel`, `/abook`…).
    OpenDialog(UiDialog),
    /// Quitter l'application (`/exit`).
    Exit,
    /// Les dialogues des scripts ont changé : l'interface les redessine.
    ScriptDialogsChanged,
    /// DCC : écouter pour `id` (port pris dans `ports`, `(0, 0)` : choisi par le système), puis ouvrir le flux
    /// `io` avec le premier pair qui se connecte (dans `timeout_secs`). L'hôte rend le port par
    /// [`Client::on_dcc_listening`], ou l'échec par [`Client::on_dcc_ended`].
    DccListen { id: DccId, conn: ConnId, ports: (u16, u16), io: crate::dcc::DccIo, timeout_secs: u64 },
    /// DCC : se connecter à `ip:port` puis ouvrir le flux `io` ; l'hôte répond par [`Client::on_dcc_connected`]
    /// ou [`Client::on_dcc_ended`].
    DccConnect { id: DccId, ip: std::net::IpAddr, port: u16, io: crate::dcc::DccIo, timeout_secs: u64 },
    /// DCC : envoi en écoute, pas encore connecté : le reprendre à `offset` (DCC RESUME accepté).
    DccResume { id: DccId, offset: u64 },
    /// DCC CHAT : envoyer une ligne (UTF-8, sans fin de ligne).
    DccChatSend { id: DccId, line: String },
    /// DCC : fermer (annuler) la connexion directe ou l'écoute `id` ; sa fin, signalée ensuite, est ignorée.
    DccClose { id: DccId },
    /// DCC : demander à l'utilisateur s'il accepte une offre reçue ; réponse par [`Client::dcc_accept`] ou
    /// [`Client::dcc_reject`].
    DccPrompt(DccPrompt),
    /// DCC : faire choisir un fichier à envoyer à `nick` ; réponse par [`Client::dcc_send_file`].
    DccPickFile { conn: ConnId, nick: String },
    /// DCC : ouvrir une fenêtre de l'interface (boîte d'envoi, de discussion, transferts).
    DccWindow(DccUi),
}

/// État d'une connexion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    Disconnected,
    Connecting,
    /// Connectée, enregistrement en cours (avant 001).
    Registering,
    Connected,
}

/// Résumé d'une connexion pour l'interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnInfo {
    pub id: ConnId,
    pub status_window: WinId,
    pub network: String,
    pub server: String,
    pub port: u16,
    pub tls: bool,
    pub state: ConnState,
    pub nick: String,
    pub lag_ms: Option<i64>,
    pub away: Option<String>,
    pub caps: Vec<String>,
    /// Réseau du bouncer auquel la connexion est liée.
    pub bouncer_network: Option<String>,
}

/// Un canal de la liste `/list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChanEntry {
    pub name: String,
    pub users: u32,
    pub topic: String,
}

/// Résultats de `/list` (l'interface trie comme elle veut).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChanList {
    pub entries: Vec<ChanEntry>,
    pub complete: bool,
    pub filter: Option<String>,
    pub min_users: Option<u32>,
    pub max_users: Option<u32>,
}

/// Une entrée de la liste Notify avec son état.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotifyStatus {
    pub nick: String,
    pub note: String,
    pub network: Option<String>,
    /// En ligne (`Some(true)`), absent (`Some(false)`) ou inconnu, pour chaque connexion prête.
    pub online: Vec<(ConnId, Option<bool>)>,
    pub last_seen_ms: Option<i64>,
}

/// Une adresse attrapée par l'URL catcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaughtUrl {
    pub url: String,
    pub nick: String,
    pub window: String,
    pub conn: Option<ConnId>,
    pub time_ms: i64,
}

/// Le client.
pub struct Client {
    st: State,
    engine: Engine,
}

impl Client {
    /// Crée le client : charge les scripts de `<data_dir>/scripts` (écrits par défaut au premier démarrage),
    /// leurs variables et la liste des utilisateurs, ouvre une première fenêtre Status (non connectée) et
    /// déclenche `on START`. Rien ne se connecte ici : voir [`Client::connect_startup`].
    pub fn new(settings: Settings, data_dir: PathBuf, now_ms: i64) -> Client {
        let engine = Engine::new(data_dir.join("scripts"));
        let burst = settings.flood.burst;
        let _ = burst;
        let st = State {
            settings,
            data_dir,
            now: now_ms,
            lang: Lang::En,
            system_locale: None,
            utc_offset: None,
            windows: Vec::new(),
            active: None,
            next_win: 1,
            next_seq: 1,
            conns: Default::default(),
            next_conn: 1,
            effects: Vec::new(),
            deferred: Default::default(),
            urls: Vec::new(),
            chanlists: Default::default(),
            logger: logging::Logger::new(Box::new(FileSink)),
            app_focused: true,
            notify_seen: Default::default(),
            dialogs_version: 0,
            log_off: Default::default(),
            last_input: now_ms,
            cur_halted: false,
            capture: None,
            scripts_fp: 0,
            scram_nonce: None,
            dcc: Default::default(),
        };
        let mut c = Client { st, engine };
        c.st.apply_language();
        c.st.new_status_window();
        c.load_scripts();
        if c.st.settings.scripts.enabled {
            let Client { st, engine } = &mut c;
            engine.flush_pending(st);
            let ev = EventInfo { kind: Some(EventKind::Start), ..EventInfo::default() };
            engine.fire(st, &Ctx::default(), &ev);
        }
        c.st.dialogs_version = c.engine.dialogs_version();
        c.st.scripts_fp = c.scripts_fingerprint();
        c.pump();
        c
    }

    fn load_scripts(&mut self) {
        let dir = self.engine.root().to_path_buf();
        let _ = std::fs::create_dir_all(&dir);
        let order_file = dir.join("order.txt");
        let first = !order_file.exists() && defaults::FILES.iter().all(|(n, _)| !dir.join(n).exists());
        if first {
            for (n, text) in defaults::FILES {
                let _ = std::fs::write(dir.join(n), text);
            }
        }
        let order: Vec<String> = match std::fs::read_to_string(&order_file) {
            Ok(t) => t.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect(),
            Err(_) => {
                let mut v: Vec<String> = defaults::FILES.iter().map(|(n, _)| n.to_string()).collect();
                let mut others: Vec<String> = std::fs::read_dir(&dir)
                    .map(|rd| {
                        rd.filter_map(|e| e.ok())
                            .map(|e| e.file_name().to_string_lossy().into_owned())
                            .filter(|n| (n.ends_with(".mrc") || n.ends_with(".ini")) && !v.contains(n))
                            .collect()
                    })
                    .unwrap_or_default();
                others.sort();
                v.extend(others);
                v
            }
        };
        let status = self.st.windows.first().map(|w| w.id).unwrap_or(0);
        for name in order {
            let path = dir.join(&name);
            if !path.is_file() {
                continue;
            }
            if let Err(e) = self.engine.restore_file(&path, true) {
                let t = self.st.t(Key::ScriptLoadError, &[("file", &name), ("error", &e.to_string())]);
                self.st.print(status, LineKind::Error, t);
            }
        }
        if let Ok(t) = std::fs::read_to_string(dir.join("vars.txt")) {
            self.engine.set_vars_text(&t);
        }
        if let Ok(t) = std::fs::read_to_string(dir.join("users.txt")) {
            self.engine.set_users_text(&t);
        }
    }

    /// Enregistre les scripts chargés, leur ordre, les variables et la liste des utilisateurs.
    pub fn save_scripts(&self) -> std::io::Result<()> {
        let root = self.engine.root().to_path_buf();
        std::fs::create_dir_all(&root)?;
        let mut order = Vec::new();
        for s in self.engine.scripts() {
            let path = s.path.clone().unwrap_or_else(|| root.join(&s.name));
            if let Some(text) = self.engine.script_text(&s.name) {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&path, text)?;
            }
            order.push(match path.strip_prefix(&root) {
                Ok(rel) => rel.to_string_lossy().into_owned(),
                Err(_) => path.display().to_string(),
            });
        }
        std::fs::write(root.join("order.txt"), order.join("\n") + "\n")?;
        std::fs::write(root.join("vars.txt"), self.engine.vars_text())?;
        std::fs::write(root.join("users.txt"), self.engine.users_text())?;
        Ok(())
    }

    /// Fait tout ce qui a été remis à plus tard (événements de script, perform, retour sur les canaux).
    fn pump(&mut self) {
        let Client { st, engine } = self;
        let scripts = st.settings.scripts.enabled;
        let mut guard = 0;
        while let Some(d) = st.deferred.pop_front() {
            guard += 1;
            if guard > 100_000 {
                st.deferred.clear();
                break;
            }
            match d {
                Deferred::Pres(p) => {
                    let mut halted = false;
                    if scripts && !p.events.is_empty() {
                        st.capture = Some(Vec::new());
                        for (ctx, ev) in &p.events {
                            halted |= engine.fire(st, ctx, ev).haltdef;
                        }
                    }
                    let captured = st.capture.take().unwrap_or_default();
                    if !halted {
                        for o in p.outs {
                            match o {
                                state::Out::Line(l) => st.add_line(l),
                                state::Out::Effect(e) => st.effects.push(e),
                                state::Out::Send(c, l) => st.send_line(c, &l),
                                state::Out::Dcc(d) => st.dcc_incoming(*d),
                            }
                        }
                    }
                    for l in captured {
                        st.add_line(l);
                    }
                }
                Deferred::Script(ctx, line) => {
                    if let Err(e) = engine.run_line(st, &ctx, &line) {
                        let (_, w) = st.resolve(&ctx);
                        st.print(w, LineKind::Error, e.to_string());
                    }
                }
                Deferred::AutoJoin(c) => st.auto_join(c),
            }
        }
        let v = engine.dialogs_version();
        if v != st.dialogs_version {
            st.dialogs_version = v;
            st.effects.push(Effect::ScriptDialogsChanged);
        }
    }

    fn finish(&mut self) {
        self.pump();
        self.st.flush_sends();
        if self.st.settings.scripts.enabled {
            let fp = self.scripts_fingerprint();
            if fp != self.st.scripts_fp {
                self.st.scripts_fp = fp;
                if !self.st.effects.contains(&Effect::SaveScripts) {
                    self.st.effects.push(Effect::SaveScripts);
                }
            }
        }
    }

    /// Empreinte de ce que [`Client::save_scripts`] enregistre (noms et tailles des scripts, variables,
    /// utilisateurs).
    fn scripts_fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for s in self.engine.scripts() {
            s.name.hash(&mut h);
            self.engine.script_text(&s.name).map(str::len).hash(&mut h);
        }
        self.engine.vars_text().hash(&mut h);
        self.engine.users_text().hash(&mut h);
        h.finish()
    }

    /// Exécute les `on LOAD`/`on START` en attente d'un script chargé par `engine_mut().load_file(…)`, après
    /// l'avertissement de l'interface (`settings.scripts.warn_on_load`, ADR 0007) ; sinon
    /// `engine_mut().discard_pending()`.
    pub fn run_pending_scripts(&mut self, now_ms: i64) {
        self.st.now = now_ms;
        let Client { st, engine } = self;
        engine.flush_pending(st);
        self.finish();
    }

    /// Ce que l'hôte doit faire, dans l'ordre.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.st.effects)
    }

    // ---------------------------------------------------------------------------------------------------------
    // Réseau
    // ---------------------------------------------------------------------------------------------------------

    /// La connexion `conn` est ouverte.
    pub fn on_connected(&mut self, conn: ConnId, now_ms: i64) {
        self.st.now = now_ms;
        if let Some(c) = self.st.conns.get_mut(&conn) {
            // Une nouvelle connexion : les fins des anciennes sont toutes arrivées (ou ne viendront pas).
            c.stale_events = 0;
        }
        self.st.conn_connected(conn);
        self.finish();
    }

    /// L'ouverture de `conn` a échoué : message, puis nouvel essai (serveur suivant) si les réglages le veulent.
    pub fn on_connect_failed(&mut self, conn: ConnId, error: &str, now_ms: i64) {
        self.st.now = now_ms;
        if self.consume_stale(conn) {
            return;
        }
        self.st.conn_failed(conn, error);
        self.finish();
    }

    /// Une ligne reçue (sans ou avec sa fin de ligne), décodée selon l'encodage du réseau.
    pub fn on_line(&mut self, conn: ConnId, bytes: &[u8], now_ms: i64) {
        self.st.now = now_ms;
        let Some(c) = self.st.conns.get_mut(&conn) else { return };
        if c.state == ConnState::Disconnected || c.stale_events > 0 {
            return;
        }
        c.last_rx = now_ms;
        let text = c.encoding.decode(bytes).into_owned();
        let Ok(msg) = Message::parse(&text) else { return };
        let mut halted = false;
        if self.st.settings.scripts.enabled {
            let Client { st, engine } = self;
            let mut ev = st.ev(EventKind::Raw, conn);
            ev.what = msg.command.clone();
            ev.nick = msg.nick().to_string();
            ev.address = msg.source.as_ref().and_then(|s| s.address()).unwrap_or_default();
            ev.text = msg.params.join(" ");
            ev.extra.push(("rawmsg".into(), text.trim_end_matches(['\r', '\n']).to_string()));
            if alpirc_proto::numeric::is_numeric(&msg.command) {
                ev.extra.push(("numeric".into(), msg.command.clone()));
            }
            let status = st.status_of(conn);
            let ctx = st.ctx(Some(conn), status);
            halted = engine.fire(st, &ctx, &ev).haltdef;
        }
        self.st.handle_message(conn, &msg, halted);
        self.finish();
    }

    /// La connexion est fermée par le serveur ou le réseau (`error` : la raison, s'il y en a une).
    pub fn on_closed(&mut self, conn: ConnId, error: Option<&str>, now_ms: i64) {
        self.st.now = now_ms;
        if self.consume_stale(conn) {
            return;
        }
        self.st.conn_closed(conn, error);
        self.finish();
    }

    /// Fin signalée d'une connexion que le moteur a fermée lui-même : à ignorer.
    fn consume_stale(&mut self, conn: ConnId) -> bool {
        match self.st.conns.get_mut(&conn) {
            Some(c) if c.stale_events > 0 => {
                c.stale_events -= 1;
                true
            }
            _ => false,
        }
    }

    /// Minuteries : file anti-flood, PING de latence (60 s), délai de ping (240 s), reconnexion, ISON (30 s),
    /// auto-op, timers des scripts, variables `-u`. Rend la prochaine échéance (ms), s'il y en a une.
    pub fn tick(&mut self, now_ms: i64) -> Option<i64> {
        self.st.now = now_ms;
        self.st.tick_conns();
        self.st.dcc_tick();
        if self.st.settings.scripts.enabled {
            let Client { st, engine } = self;
            engine.tick(st, now_ms);
        }
        self.finish();
        let engine_next = if self.st.settings.scripts.enabled { self.engine.next_timer_due() } else { None };
        [self.st.next_wake(), engine_next, self.st.dcc_next_wake()].into_iter().flatten().min()
    }

    /// Se connecte aux réseaux à ouvrir au démarrage (`connect_on_startup` et `auto_connect`).
    pub fn connect_startup(&mut self, now_ms: i64) {
        self.st.now = now_ms;
        let mut nets: Vec<String> = Vec::new();
        if self.st.settings.connect.connect_on_startup {
            nets.extend(self.st.settings.connect.startup_networks.iter().cloned());
        }
        for n in &self.st.settings.networks {
            if n.auto_connect && !nets.iter().any(|x| x.eq_ignore_ascii_case(&n.name)) {
                nets.push(n.name.clone());
            }
        }
        for (i, n) in nets.iter().enumerate() {
            let status = if i == 0 {
                self.st.windows.iter().find(|w| w.kind == WinKind::Status).map(|w| w.id).unwrap_or(0)
            } else {
                self.st.new_status_window()
            };
            self.st.connect_network(status, n, None);
        }
        self.finish();
    }

    // ---------------------------------------------------------------------------------------------------------
    // Saisie
    // ---------------------------------------------------------------------------------------------------------

    /// Ce que l'utilisateur a tapé dans une fenêtre (plusieurs lignes possibles). `on INPUT` d'abord (`/haltdef`
    /// arrête tout) ; `/commande` (alias d'abord, `//` pour évaluer) ; texte simple : message à la fenêtre.
    pub fn input(&mut self, win: WinId, text: &str, now_ms: i64) {
        self.st.now = now_ms;
        self.st.last_input = now_ms;
        for line in text.split('\n') {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let Some(w) = self.st.win_mut(win) else { return };
            w.history.push(line.to_string());
            if w.history.len() > 100 {
                w.history.remove(0);
            }
            let (conn, name) = (w.conn, w.name.clone());
            let ctx = self.st.ctx(conn, Some(win));
            if self.st.settings.scripts.enabled {
                let ev = EventInfo { kind: Some(EventKind::Input), conn, target: Some(name), text: line.to_string(), ..EventInfo::default() };
                let Client { st, engine } = self;
                if engine.fire(st, &ctx, &ev).haltdef {
                    self.pump();
                    continue;
                }
            }
            if line.starts_with('/') {
                // Commande de l'utilisateur (pas d'un script) : elle peut accepter un DCC.
                self.st.dcc.user = true;
                self.run_line(win, &ctx, line);
                self.st.dcc.user = false;
            } else {
                self.st.send_plain(win, line);
            }
            self.pump();
        }
        self.finish();
    }

    fn run_line(&mut self, win: WinId, ctx: &Ctx, line: &str) {
        let Client { st, engine } = self;
        if st.settings.scripts.enabled {
            if let Err(e) = engine.run_line(st, ctx, line) {
                st.print(win, LineKind::Error, e.to_string());
            }
        } else {
            let body = line.trim_start_matches('/');
            let (name, args) = util::split_word(body);
            if let Err(e) = st.run_builtin(ctx, name.trim_start_matches(['!', '.']), args, name.starts_with('.')) {
                st.print(win, LineKind::Error, if e.starts_with("* ") { e } else { format!("* {e}") });
            }
        }
    }

    /// Exécute une commande (menus, boutons de l'interface), comme une saisie précédée de `/`, sans `on INPUT`.
    pub fn command(&mut self, win: WinId, line: &str, now_ms: i64) {
        self.st.now = now_ms;
        let line = if line.starts_with('/') { line.to_string() } else { format!("/{line}") };
        let conn = self.st.win(win).and_then(|w| w.conn);
        let ctx = self.st.ctx(conn, Some(win));
        self.st.dcc.user = true;
        self.run_line(win, &ctx, &line);
        self.st.dcc.user = false;
        self.finish();
    }

    // ---------------------------------------------------------------------------------------------------------
    // Fenêtres
    // ---------------------------------------------------------------------------------------------------------

    /// Nouvelle fenêtre Status (« nouvelle fenêtre serveur » de mIRC), avec sa connexion.
    pub fn new_status(&mut self, now_ms: i64) -> WinId {
        self.st.now = now_ms;
        let w = self.st.new_status_window();
        self.finish();
        w
    }

    /// Connecte (ou reconnecte) une fenêtre Status à un réseau des réglages (`server` : numéro du serveur dans
    /// sa liste), ou à `hôte[:port]` si aucun réseau ne porte ce nom (`:+6697` : TLS).
    pub fn connect(&mut self, status: WinId, network: &str, server: Option<usize>, now_ms: i64) {
        self.st.now = now_ms;
        self.st.connect_network(status, network, server);
        self.finish();
    }

    /// Déconnexion voulue (pas de reconnexion automatique).
    pub fn disconnect(&mut self, status: WinId, reason: Option<&str>, now_ms: i64) {
        self.st.now = now_ms;
        if let Some(c) = self.st.conn_of_status(status) {
            let r = reason.map(str::to_string).unwrap_or_else(|| self.st.settings.identity.quit_message.clone());
            self.st.user_disconnect(c, &r);
        }
        self.finish();
    }

    /// L'interface montre cette fenêtre : activité effacée, `MARKREAD` (read-marker), `on ACTIVE`.
    pub fn set_active(&mut self, win: WinId, now_ms: i64) {
        self.st.now = now_ms;
        let Some(w) = self.st.win(win) else { return };
        let (conn, kind, name, last) = (w.conn, w.kind, w.name.clone(), w.last_time());
        if let Some(old) = self.st.active
            && old != win
            && let Some(o) = self.st.win_mut(old)
        {
            o.unread_marker = None;
            o.touch();
        }
        self.st.active = Some(win);
        if let Some(w) = self.st.win_mut(win) {
            w.activity = Activity::None;
            w.touch();
        }
        if let (Some(c), Some(t)) = (conn, last)
            && matches!(kind, WinKind::Channel | WinKind::Query)
            && self.st.conns.get(&c).is_some_and(|x| x.connected() && x.session.cap("draft/read-marker"))
            && self.st.win(win).is_some_and(|w| w.read_marker_ms.is_none_or(|m| m < t))
        {
            if let Some(w) = self.st.win_mut(win) {
                w.read_marker_ms = Some(t);
            }
            self.st.send_line(c, &format!("MARKREAD {name} timestamp={}", crate::timeutil::format_iso8601(t)));
        }
        let ev = EventInfo { kind: Some(EventKind::Active), conn, target: Some(name), ..EventInfo::default() };
        let ctx = self.st.ctx(conn, Some(win));
        self.st.push_pres(state::Pres::new().event(ctx, ev));
        self.finish();
    }

    pub fn active(&self) -> Option<WinId> {
        self.st.active
    }

    /// Ferme une fenêtre : canal → PART (la fenêtre se ferme à l'écho, ou tout de suite si on n'y est plus) ;
    /// privé, fenêtre `@` → fermée (`on CLOSE`) ; Status → QUIT et fermeture de ses fenêtres (la dernière Status
    /// reste ouverte).
    pub fn close_window(&mut self, win: WinId, now_ms: i64) {
        self.st.now = now_ms;
        self.st.close_win(win);
        self.finish();
    }

    pub fn windows(&self) -> &[Window] {
        &self.st.windows
    }

    pub fn window(&self, id: WinId) -> Option<&Window> {
        self.st.win(id)
    }

    /// Fenêtre d'un nom (`#lyon`, `Guiwald`, `Status`, `@pic`) pour une connexion.
    pub fn find_window(&self, conn: Option<ConnId>, name: &str) -> Option<WinId> {
        self.st.find_win(conn, name)
    }

    /// Liste des pseudos d'un canal : préfixe le plus fort d'abord (PREFIX), puis par ordre alphabétique.
    pub fn nicklist(&self, win: WinId) -> Vec<NickEntry> {
        self.st.nicklist(win)
    }

    /// État du canal d'une fenêtre (sujet, modes, listes), pour Channel Central.
    pub fn channel_state(&self, win: WinId) -> Option<&ChannelState> {
        let w = self.st.win(win)?;
        if w.kind != WinKind::Channel {
            return None;
        }
        self.st.conns.get(&w.conn?)?.session.channel(&w.name)
    }

    /// Demande les listes de bans, d'exceptions et d'invitations du canal (sans affichage).
    pub fn request_lists(&mut self, win: WinId) {
        self.st.request_lists(win);
        self.finish();
    }

    pub fn conn_info(&self, conn: ConnId) -> Option<ConnInfo> {
        let c = self.st.conns.get(&conn)?;
        Some(ConnInfo {
            id: c.id,
            status_window: c.status,
            network: c.network.clone(),
            server: c.host.clone(),
            port: c.port,
            tls: c.tls,
            state: c.state,
            nick: c.session.nick().to_string(),
            lag_ms: c.lag_ms,
            away: c.session.away().map(str::to_string),
            caps: c.session.caps_enabled().iter().cloned().collect(),
            bouncer_network: c.bouncer_netid.clone(),
        })
    }

    /// Connexions ouvertes ou non (une par fenêtre Status).
    pub fn conns(&self) -> Vec<ConnId> {
        self.st.conns.keys().copied().collect()
    }

    /// Session IRC d'une connexion (lecture de l'état du protocole).
    pub fn session(&self, conn: ConnId) -> Option<&crate::session::Session> {
        self.st.conns.get(&conn).map(|c| &c.session)
    }

    // ---------------------------------------------------------------------------------------------------------
    // Réglages
    // ---------------------------------------------------------------------------------------------------------

    pub fn settings(&self) -> &Settings {
        &self.st.settings
    }

    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.st.settings
    }

    /// Les réglages ont été modifiés : langue, encodages, titres ; demande leur enregistrement.
    pub fn settings_changed(&mut self, now_ms: i64) {
        self.st.now = now_ms;
        self.st.apply_language();
        let ids: Vec<ConnId> = self.st.conns.keys().copied().collect();
        for id in ids {
            let enc = self
                .st
                .conns
                .get(&id)
                .and_then(|c| c.net_name.clone())
                .and_then(|n| self.st.settings.network(&n).map(|n| Encoding::from_name(&n.encoding)));
            if let (Some(e), Some(c)) = (enc, self.st.conns.get_mut(&id)) {
                c.encoding = e;
            }
            self.st.update_titles(id);
        }
        self.st.effects.push(Effect::SaveSettings);
        self.finish();
    }

    /// Couleur (numéro de la palette) d'une sorte de ligne.
    pub fn color_index(&self, kind: LineKind) -> u8 {
        self.st.settings.color_index(kind)
    }

    /// L'application a (ou n'a plus) le focus : les notifications en dépendent.
    pub fn set_app_focused(&mut self, focused: bool) {
        self.st.app_focused = focused;
    }

    /// Langue du système (`fr_FR.UTF-8`…), utilisée si les réglages n'en imposent pas.
    pub fn set_system_locale(&mut self, locale: &str) {
        self.st.system_locale = Some(locale.to_string());
        self.st.apply_language();
        let ids: Vec<ConnId> = self.st.conns.keys().copied().collect();
        for id in ids {
            self.st.update_titles(id);
        }
    }

    /// Langue en usage.
    pub fn language(&self) -> Lang {
        self.st.lang
    }

    /// Décalage horaire imposé (secondes) pour l'horodatage ; `None` : celui du système.
    pub fn set_utc_offset(&mut self, offset_secs: Option<i32>) {
        self.st.utc_offset = offset_secs;
    }

    /// Destination des journaux (par défaut, le disque).
    pub fn set_log_sink(&mut self, sink: Box<dyn LogSink>) {
        self.st.logger = logging::Logger::new(sink);
    }

    /// Nonce SCRAM imposé, pour des essais reproductibles.
    #[doc(hidden)]
    pub fn set_scram_nonce(&mut self, nonce: Option<&str>) {
        self.st.scram_nonce = nonce.map(str::to_string);
    }

    pub fn data_dir(&self) -> &Path {
        &self.st.data_dir
    }

    // ---------------------------------------------------------------------------------------------------------
    // Scripts
    // ---------------------------------------------------------------------------------------------------------

    /// Menu d'une fenêtre ; `selected` : pseudos choisis (liste des pseudos), sinon le pseudo du privé.
    pub fn popup(&mut self, kind: PopupKind, win: WinId, selected: &[String]) -> Vec<MenuItem> {
        let ctx = self.popup_ctx(kind, win, selected);
        let custom = if kind == PopupKind::Custom { self.st.win(win).map(|w| w.name.clone()) } else { None };
        let Client { st, engine } = self;
        let m = engine.popup(st, &ctx, kind, custom.as_deref());
        self.finish();
        m
    }

    fn popup_ctx(&self, kind: PopupKind, win: WinId, selected: &[String]) -> Ctx {
        let conn = self.st.win(win).and_then(|w| w.conn);
        let mut ctx = self.st.ctx(conn, Some(win));
        ctx.selected = selected.to_vec();
        if ctx.selected.is_empty()
            && kind == PopupKind::Query
            && let Some(w) = self.st.win(win)
        {
            ctx.selected = vec![w.name.clone()];
        }
        ctx
    }

    /// Lance l'entrée choisie du dernier menu construit par [`Client::popup`].
    pub fn popup_run(&mut self, win: WinId, selected: &[String], action: u32, now_ms: i64) {
        self.st.now = now_ms;
        let kind = match self.st.win(win).map(|w| w.kind) {
            Some(WinKind::Query) => PopupKind::Query,
            _ => PopupKind::Nicklist,
        };
        let ctx = self.popup_ctx(kind, win, selected);
        let Client { st, engine } = self;
        st.dcc.user = true;
        if let Err(e) = engine.popup_run(st, &ctx, action) {
            st.print(win, LineKind::Error, e.to_string());
        }
        st.dcc.user = false;
        self.finish();
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// Charge un script donné en texte (éditeur de scripts) ; ses `on LOAD`/`on START` s'exécutent tout de suite.
    pub fn load_script_text(&mut self, name: &str, kind: ScriptKind, text: &str, now_ms: i64) -> Result<(), alpirc_script::ScriptError> {
        self.st.now = now_ms;
        self.engine.load_text(name, kind, text)?;
        let Client { st, engine } = self;
        engine.flush_pending(st);
        self.st.effects.push(Effect::SaveScripts);
        self.finish();
        Ok(())
    }

    /// Événement d'un dialogue de script (clic, saisie…).
    pub fn dialog_event(&mut self, name: &str, ev: DialogEvent, now_ms: i64) {
        self.st.now = now_ms;
        let Client { st, engine } = self;
        engine.dialog_event(st, name, ev);
        self.finish();
    }

    // ---------------------------------------------------------------------------------------------------------
    // Listes
    // ---------------------------------------------------------------------------------------------------------

    /// Résultats de `/list` d'une fenêtre de liste des canaux.
    pub fn chanlist(&self, win: WinId) -> Option<&ChanList> {
        self.st.chanlists.get(&win)
    }

    /// La liste Notify et l'état de chaque pseudo.
    pub fn notify_list(&self) -> Vec<NotifyStatus> {
        self.st
            .settings
            .address_book
            .notify
            .iter()
            .map(|e| {
                let online = self
                    .st
                    .conns
                    .values()
                    .filter(|c| c.ready)
                    .map(|c| (c.id, c.notify_online.get(&c.session.fold(&e.nick)).copied()))
                    .collect();
                let key = alpirc_proto::CaseMapping::Rfc1459.fold(&e.nick);
                NotifyStatus {
                    nick: e.nick.clone(),
                    note: e.note.clone(),
                    network: e.network.clone(),
                    online,
                    last_seen_ms: self.st.notify_seen.get(&key).copied(),
                }
            })
            .collect()
    }

    /// Adresses attrapées (la plus récente à la fin).
    pub fn urls(&self) -> &[CaughtUrl] {
        &self.st.urls
    }

    /// Complétion Tab : pseudos du canal (les plus récemment actifs d'abord), canaux (`#`), commandes (`/`).
    /// `cycle_index` : 0 pour la première proposition, 1 pour la suivante…
    pub fn tab_complete(&self, win: WinId, word: &str, cycle_index: usize) -> Option<String> {
        let w = self.st.win(win)?;
        let conn = w.conn.and_then(|c| self.st.conns.get(&c));
        let lower = |s: &str| alpirc_proto::CaseMapping::Rfc1459.fold(s);
        let prefix = lower(word);
        let mut cands: Vec<String> = Vec::new();
        if let Some(cmd) = word.strip_prefix('/') {
            let p = cmd.to_ascii_lowercase();
            cands = BUILTINS.iter().filter(|b| b.starts_with(&p)).map(|b| format!("/{b}")).collect();
        } else if conn.is_some_and(|c| c.session.is_channel(word)) || (conn.is_none() && word.starts_with('#')) {
            let mut v: Vec<String> = self
                .st
                .windows
                .iter()
                .filter(|x| x.kind == WinKind::Channel && x.conn == w.conn && lower(&x.name).starts_with(&prefix))
                .map(|x| x.name.clone())
                .collect();
            v.sort_by_key(|a| lower(a));
            cands = v;
        } else if let Some(c) = conn {
            match w.kind {
                WinKind::Channel => {
                    if let Some(ch) = c.session.channel(&w.name) {
                        let mut v: Vec<(i64, String)> =
                            ch.members.values().filter(|m| lower(&m.nick).starts_with(&prefix)).map(|m| (m.last_spoke, m.nick.clone())).collect();
                        v.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| lower(&a.1).cmp(&lower(&b.1))));
                        cands = v.into_iter().map(|x| x.1).collect();
                    }
                }
                WinKind::Query => {
                    for n in [w.name.clone(), c.session.nick().to_string()] {
                        if lower(&n).starts_with(&prefix) && !cands.contains(&n) {
                            cands.push(n);
                        }
                    }
                }
                _ => {}
            }
        }
        if cands.is_empty() {
            return None;
        }
        Some(cands[cycle_index % cands.len()].clone())
    }

    /// Encodage d'une connexion (pour encoder les lignes envoyées).
    pub fn conn_encoding(&self, conn: ConnId) -> Encoding {
        self.st.conn_encoding(conn)
    }

    /// Lignes d'une fenêtre qui contiennent le texte (codes retirés, sans tenir compte de la casse).
    pub fn search(&self, win: WinId, needle: &str) -> Vec<usize> {
        let Some(w) = self.st.win(win) else { return Vec::new() };
        let n = needle.to_lowercase();
        if n.is_empty() {
            return Vec::new();
        }
        w.lines.iter().enumerate().filter(|(_, l)| State::plain(&l.text).to_lowercase().contains(&n)).map(|(i, _)| i).collect()
    }
}
