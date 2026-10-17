//! Couche protocole d'UNE connexion IRC, sans fenêtres ni entrées-sorties : enregistrement (CAP 302, PASS,
//! NICK/USER, pseudos de secours), capacités IRCv3, SASL, ISUPPORT, notre pseudo et notre adresse, modes
//! utilisateur, canaux (sujet, modes, membres, listes), IAL, lots (`BATCH`), MONITOR, STS, extensions de soju.
//!
//! La session lit des [`Message`] ([`Session::handle`]) et rend des [`SessionEvent`] typés ; ce qu'elle doit
//! envoyer elle-même (PONG, CAP, AUTHENTICATE, NICK de secours, MODE/WHO à l'arrivée sur un canal) s'accumule dans
//! une file ([`Session::take_outgoing`]). Elle ne connaît ni les fenêtres ni l'heure du système : tout ce qui
//! dépend du temps reçoit `now_ms`. Elle sert au client (`crate::client`) et servira au relais (`alpirc-relay`).
//!
//! Les messages d'un lot `chathistory` ne changent pas l'état : ils sont gardés jusqu'à la fin du lot puis rendus
//! d'un bloc ([`SessionEvent::HistoryBatch`]).

mod state;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use alpirc_proto::modes::{self, ModeChange};
use alpirc_proto::sasl::{self, Mechanism, Reassembler, Scram};
use alpirc_proto::{CaseMapping, ISupport, Message};

pub use state::{ChannelState, ListEntry, Member, UserInfo};

/// Capacités demandées par défaut quand le serveur les propose (`sasl` en plus si SASL est configuré ; `sts` est
/// lue sans être demandée).
pub const DEFAULT_CAPS: &[&str] = &[
    "multi-prefix",
    "extended-join",
    "account-notify",
    "away-notify",
    "chghost",
    "setname",
    "userhost-in-names",
    "invite-notify",
    "server-time",
    "message-tags",
    "echo-message",
    "batch",
    "labeled-response",
    "cap-notify",
    "draft/chathistory",
    "draft/read-marker",
    "soju.im/bouncer-networks",
    "soju.im/bouncer-networks-notify",
    "draft/event-playback",
    "standard-replies",
    "account-tag",
];

/// Jeton de nos requêtes WHOX.
const WHOX_TOKEN: &str = "152";

/// Configuration SASL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaslConfig {
    pub mechanism: Mechanism,
    pub account: String,
    pub password: String,
}

/// Ce qu'il faut à la session pour s'enregistrer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    pub nick: String,
    pub alt_nick: String,
    pub username: String,
    pub realname: String,
    /// Mot de passe du serveur (`PASS`). Pour Undernet, voir [`undernet_loc_password`].
    pub password: Option<String>,
    pub sasl: Option<SaslConfig>,
    /// `PRIVMSG NickServ :IDENTIFY <mot de passe>` après 001 (ou quand NickServ le demande), sauf si SASL a réussi.
    pub nickserv_password: Option<String>,
    /// Capacités à demander quand elles sont proposées.
    pub request_caps: Vec<String>,
    /// Réseau du bouncer (soju) auquel lier cette connexion : `BOUNCER BIND <id>` avant `CAP END`.
    pub bouncer_bind: Option<String>,
    /// La connexion est chiffrée (pour STS).
    pub tls: bool,
    /// Demander WHO à l'arrivée sur un canal (toujours fait si `away-notify` est actif).
    pub who_on_join: bool,
    /// Nonce SCRAM imposé (tests) ; tiré au hasard sinon.
    pub scram_nonce: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> SessionConfig {
        SessionConfig {
            nick: "Guiwald".into(),
            alt_nick: "Guiwald_".into(),
            username: "alpirc".into(),
            realname: "AlpIRC user".into(),
            password: None,
            sasl: None,
            nickserv_password: None,
            request_caps: DEFAULT_CAPS.iter().map(|s| s.to_string()).collect(),
            bouncer_bind: None,
            tls: false,
            who_on_join: false,
            scram_nonce: None,
        }
    }
}

/// Mot de passe de serveur pour le « Login on Connect » d'Undernet (<https://www.undernet.org/loc/>) :
/// `<mode> <compte> <mot de passe>`. Le mode suit la page d'Undernet : `+x!` (connexion seulement si X
/// authentifie, adresse masquée), `-!+x` (connexion toujours, masquée si X authentifie), `-x!` (connexion même si
/// X est absent, adresse visible). `+!-x` (X exigé, adresse visible) n'est pas décrit par Undernet : il est déduit
/// du sens des drapeaux (`!` exige, `x` masque).
pub fn undernet_loc_password(username: &str, password: &str, require_x: bool, hide_host: bool) -> String {
    let mode = match (require_x, hide_host) {
        (true, true) => "+x!",
        (false, true) => "-!+x",
        (false, false) => "-x!",
        (true, false) => "+!-x",
    };
    format!("{mode} {username} {password}")
}

/// Politique STS annoncée par le serveur (capacité `sts`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StsInfo {
    pub port: Option<u16>,
    pub duration_secs: Option<u64>,
    pub preload: bool,
}

/// Lit la valeur de la capacité `sts` (`port=6697,duration=2592000,preload`).
pub fn parse_sts(value: &str) -> StsInfo {
    let mut info = StsInfo { port: None, duration_secs: None, preload: false };
    for part in value.split(',') {
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        match k.trim() {
            "port" => info.port = v.trim().parse().ok(),
            "duration" => info.duration_secs = v.trim().parse().ok(),
            "preload" => info.preload = true,
            _ => {}
        }
    }
    info
}

/// PRIVMSG ou NOTICE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgKind {
    Privmsg,
    Notice,
}

/// Réponses normalisées (`FAIL`, `WARN`, `NOTE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyKind {
    Fail,
    Warn,
    Note,
}

/// Ce qu'une ligne du serveur a produit, une fois l'état mis à jour.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    /// Le pseudo essayé pendant l'enregistrement est refusé (433, 432, 437) ; on essaie `next`.
    NickRetry { failed: String, next: String },
    /// 001 : nous voici enregistrés sous ce pseudo.
    Registered { nick: String },
    /// 005 lu (PREFIX, CHANMODES… peuvent avoir changé).
    IsupportChanged,
    /// Fin du MOTD (376 ou 422), la première fois : la connexion est prête.
    MotdEnd,
    /// Capacités activées ou retirées (ACK, DEL).
    CapsChanged { added: Vec<String>, removed: Vec<String> },
    /// Fin de la négociation des capacités (`CAP END` envoyé).
    CapEnd,
    /// Le serveur annonce une politique STS (CAP LS ou NEW).
    Sts(StsInfo),
    SaslSuccess,
    SaslFailed { reason: String },
    LoggedIn { account: String },
    LoggedOut,
    /// Notre adresse a changé (396, CHGHOST, 900).
    OwnHost { user: Option<String>, host: String },
    UserMode { setter: String, modes: String, changes: Vec<ModeChange> },
    Join { channel: String, nick: String, address: Option<String>, account: Option<String>, realname: Option<String>, own: bool },
    Part { channel: String, nick: String, address: Option<String>, reason: Option<String>, own: bool },
    Kick { channel: String, by: String, by_address: Option<String>, target: String, reason: Option<String>, own: bool },
    /// `channels` : canaux que l'on partageait avec lui (avant son départ).
    Quit { nick: String, address: Option<String>, reason: Option<String>, channels: Vec<String> },
    Nick { old: String, new: String, address: Option<String>, channels: Vec<String>, own: bool },
    ChannelMode { channel: String, setter: String, setter_address: Option<String>, modes: String, changes: Vec<ModeChange> },
    Topic { channel: String, setter: String, address: Option<String>, topic: String },
    /// 332.
    TopicIs { channel: String, topic: String },
    /// 333.
    TopicWhoTime { channel: String, setter: String, time: Option<i64> },
    /// 331.
    NoTopic { channel: String },
    /// 324 ; `auto` : réponse au MODE envoyé à l'arrivée.
    ChannelModeIs { channel: String, modes: String, auto: bool },
    /// 329.
    CreationTime { channel: String, time: i64, auto: bool },
    /// 353 ; `initial` : liste reçue à l'arrivée sur le canal.
    Names { channel: String, names: Vec<String>, initial: bool },
    /// 366.
    NamesEnd { channel: String, initial: bool },
    /// 367, 348, 346 ; `quiet` : liste demandée par [`Session::request_list`] avec `quiet`.
    ListEntry { channel: String, mode: char, entry: ListEntry, quiet: bool },
    /// 368, 349, 347.
    ListEnd { channel: String, mode: char, quiet: bool },
    /// 352/354 ; `auto` : WHO envoyé par la session ou demandé avec `auto`.
    WhoReply { nick: String, channel: Option<String>, auto: bool },
    /// 315.
    WhoEnd { mask: String, auto: bool },
    /// 302 : (pseudo, ident, hôte).
    Userhost { entries: Vec<(String, String, String)>, auto: bool },
    /// 303.
    Ison { nicks: Vec<String>, auto: bool },
    /// PRIVMSG ou NOTICE.
    Message {
        kind: MsgKind,
        from: String,
        address: Option<String>,
        /// Cible telle qu'envoyée (canal, `@#canal`, notre pseudo, ou l'autre pseudo pour un écho).
        target: String,
        text: String,
        /// Canal concerné (sans le préfixe STATUSMSG).
        channel: Option<String>,
        statusmsg: Option<char>,
        /// Notre propre message renvoyé (echo-message, ou bouncer).
        own: bool,
        /// Message d'un serveur (ou avant l'enregistrement).
        server: bool,
    },
    Invite { from: String, address: Option<String>, target: String, channel: String, own: bool },
    /// away-notify ; `message` vide : revenu.
    Away { nick: String, message: Option<String> },
    /// 305/306.
    OwnAway { away: bool },
    Account { nick: String, account: Option<String> },
    Chghost { nick: String, user: String, host: String, own: bool },
    Setname { nick: String, realname: String },
    Ping { token: String },
    Pong { server: String, token: String },
    Error { message: String },
    Wallops { from: String, text: String },
    /// 730 : (pseudo, adresse).
    MonitorOnline(Vec<(String, Option<String>)>),
    /// 731.
    MonitorOffline(Vec<String>),
    /// `BOUNCER NETWORK <id> <attributs>` ; `deleted` pour `*`.
    BouncerNetwork { id: String, attrs: Vec<(String, String)>, deleted: bool },
    /// `MARKREAD <cible> timestamp=…` (ou `*`).
    ReadMarker { target: String, timestamp: Option<i64> },
    StandardReply { kind: ReplyKind, command: String, code: String, context: Vec<String>, description: String },
    BatchStart { id: String, kind: String, params: Vec<String> },
    BatchEnd { id: String, kind: String, params: Vec<String> },
    /// Fin d'un lot `chathistory` : ses messages, dans l'ordre reçu.
    HistoryBatch { target: String, messages: Vec<Message> },
}

impl SessionEvent {
    /// Vrai si l'événement signale une réponse que le client ne doit pas afficher telle quelle.
    pub fn suppresses_numeric(&self) -> bool {
        match self {
            SessionEvent::ChannelModeIs { auto, .. }
            | SessionEvent::CreationTime { auto, .. }
            | SessionEvent::WhoReply { auto, .. }
            | SessionEvent::WhoEnd { auto, .. }
            | SessionEvent::Userhost { auto, .. }
            | SessionEvent::Ison { auto, .. } => *auto,
            SessionEvent::Names { initial, .. } | SessionEvent::NamesEnd { initial, .. } => *initial,
            SessionEvent::ListEntry { quiet, .. } | SessionEvent::ListEnd { quiet, .. } => *quiet,
            SessionEvent::MonitorOnline(_) | SessionEvent::MonitorOffline(_) => true,
            _ => false,
        }
    }
}

enum SaslState {
    Idle,
    /// `AUTHENTICATE <MÉCA>` envoyé.
    Started,
    /// Réponse envoyée (PLAIN, EXTERNAL).
    Sent,
    ScramFirst(Box<Scram>, Reassembler),
    ScramFinal(Box<Scram>, Reassembler),
    Done,
}

#[derive(Debug, Clone)]
struct Batch {
    kind: String,
    params: Vec<String>,
    parent: Option<String>,
}

/// Une session IRC.
pub struct Session {
    cfg: SessionConfig,
    out: Vec<Message>,
    registered: bool,
    cap_negotiating: bool,
    caps_available: BTreeMap<String, String>,
    caps_enabled: BTreeSet<String>,
    caps_pending: BTreeSet<String>,
    sasl: SaslState,
    sasl_ok: bool,
    nickserv_sent: bool,
    nick_attempt: usize,
    nick: String,
    user: Option<String>,
    host: Option<String>,
    account: Option<String>,
    isupport: ISupport,
    user_modes: String,
    channels: BTreeMap<String, ChannelState>,
    users: HashMap<String, UserInfo>,
    batches: HashMap<String, Batch>,
    history: HashMap<String, Vec<Message>>,
    away: Option<String>,
    away_pending: Option<String>,
    motd_done: bool,
    server_name: String,
    monitor: BTreeMap<String, bool>,
    auto_who: Vec<String>,
    auto_mode: HashSet<String>,
    auto_created: HashSet<String>,
    auto_userhost: usize,
    auto_ison: usize,
    quiet_lists: HashSet<(String, char)>,
    label_seq: u64,
}

impl Session {
    pub fn new(cfg: SessionConfig) -> Session {
        let nick = cfg.nick.clone();
        Session {
            cfg,
            out: Vec::new(),
            registered: false,
            cap_negotiating: false,
            caps_available: BTreeMap::new(),
            caps_enabled: BTreeSet::new(),
            caps_pending: BTreeSet::new(),
            sasl: SaslState::Idle,
            sasl_ok: false,
            nickserv_sent: false,
            nick_attempt: 0,
            nick,
            user: None,
            host: None,
            account: None,
            isupport: ISupport::default(),
            user_modes: String::new(),
            channels: BTreeMap::new(),
            users: HashMap::new(),
            batches: HashMap::new(),
            history: HashMap::new(),
            away: None,
            away_pending: None,
            motd_done: false,
            server_name: String::new(),
            monitor: BTreeMap::new(),
            auto_who: Vec::new(),
            auto_mode: HashSet::new(),
            auto_created: HashSet::new(),
            auto_userhost: 0,
            auto_ison: 0,
            quiet_lists: HashSet::new(),
            label_seq: 0,
        }
    }

    pub fn config(&self) -> &SessionConfig {
        &self.cfg
    }

    pub fn config_mut(&mut self) -> &mut SessionConfig {
        &mut self.cfg
    }

    /// Commence l'enregistrement (la connexion vient de s'ouvrir) : `PASS`, `CAP LS 302`, `NICK`, `USER`.
    /// Remet tout l'état à zéro.
    pub fn start(&mut self) {
        let cfg = self.cfg.clone();
        *self = Session::new(cfg);
        if let Some(p) = self.cfg.password.clone().filter(|p| !p.is_empty()) {
            self.push(Message::new("PASS", [p]));
        }
        self.cap_negotiating = true;
        self.push(Message::new("CAP", ["LS", "302"]));
        let nick = self.cfg.nick.clone();
        self.push(Message::new("NICK", [nick]));
        let (u, r) = (self.cfg.username.clone(), self.cfg.realname.clone());
        self.push(Message::new("USER", [u, "0".into(), "*".into(), r]));
    }

    /// Lignes à envoyer, dans l'ordre.
    pub fn take_outgoing(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.out)
    }

    /// Met une ligne dans la file d'envoi de la session.
    pub fn send(&mut self, m: Message) {
        self.out.push(m);
    }

    fn push(&mut self, m: Message) {
        self.out.push(m);
    }

    // ---------------------------------------------------------------------------------------------------------
    // Accesseurs
    // ---------------------------------------------------------------------------------------------------------

    pub fn nick(&self) -> &str {
        &self.nick
    }
    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }
    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }
    pub fn account(&self) -> Option<&str> {
        self.account.as_deref()
    }
    /// Longueur de `pseudo!ident@hôte` tel que les autres le voient (estimation large si inconnu).
    pub fn source_len(&self) -> usize {
        match (&self.user, &self.host) {
            (Some(u), Some(h)) => self.nick.len() + 1 + u.len() + 1 + h.len(),
            _ => self.nick.len() + 1 + 10 + 1 + 63,
        }
    }
    pub fn isupport(&self) -> &ISupport {
        &self.isupport
    }
    pub fn casemapping(&self) -> CaseMapping {
        self.isupport.casemapping
    }
    pub fn fold(&self, s: &str) -> String {
        self.isupport.casemapping.fold(s)
    }
    pub fn eq(&self, a: &str, b: &str) -> bool {
        self.isupport.casemapping.eq(a, b)
    }
    pub fn is_me(&self, nick: &str) -> bool {
        self.eq(nick, &self.nick)
    }
    pub fn is_channel(&self, name: &str) -> bool {
        self.isupport.is_channel(name)
    }
    pub fn is_registered(&self) -> bool {
        self.registered
    }
    pub fn motd_done(&self) -> bool {
        self.motd_done
    }
    pub fn cap_negotiating(&self) -> bool {
        self.cap_negotiating
    }
    pub fn caps_available(&self) -> &BTreeMap<String, String> {
        &self.caps_available
    }
    pub fn caps_enabled(&self) -> &BTreeSet<String> {
        &self.caps_enabled
    }
    pub fn cap(&self, name: &str) -> bool {
        self.caps_enabled.contains(name)
    }
    pub fn sasl_succeeded(&self) -> bool {
        self.sasl_ok
    }
    pub fn user_modes(&self) -> &str {
        &self.user_modes
    }
    pub fn away(&self) -> Option<&str> {
        self.away.as_deref()
    }
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
    pub fn channel(&self, name: &str) -> Option<&ChannelState> {
        self.channels.get(&self.fold(name))
    }
    pub fn channels(&self) -> impl Iterator<Item = &ChannelState> {
        self.channels.values()
    }
    pub fn user_info(&self, nick: &str) -> Option<&UserInfo> {
        self.users.get(&self.fold(nick))
    }
    pub fn users(&self) -> impl Iterator<Item = &UserInfo> {
        self.users.values()
    }
    /// État MONITOR : pseudo → en ligne.
    pub fn monitor_status(&self) -> &BTreeMap<String, bool> {
        &self.monitor
    }
    /// Sorte d'un lot ouvert.
    pub fn batch_kind(&self, id: &str) -> Option<&str> {
        self.batches.get(id).map(|b| b.kind.as_str())
    }
    /// Modes de préfixe d'un pseudo sur un canal, s'il y est.
    pub fn member_modes(&self, chan: &str, nick: &str) -> Option<&str> {
        let c = self.channel(chan)?;
        c.members.get(&self.fold(nick)).map(|m| m.modes.as_str())
    }
    /// Canaux (noms) que l'on partage avec ce pseudo.
    pub fn common_channels(&self, nick: &str) -> Vec<String> {
        let k = self.fold(nick);
        self.channels.values().filter(|c| c.members.contains_key(&k)).map(|c| c.name.clone()).collect()
    }
    /// Sommes-nous opérateur (`o` ou plus fort) sur ce canal ?
    pub fn am_op(&self, chan: &str) -> bool {
        let Some(m) = self.member_modes(chan, &self.nick) else { return false };
        let op_rank = self.isupport.rank('o').unwrap_or(0);
        m.chars().any(|c| self.isupport.rank(c).is_some_and(|r| r <= op_rank))
    }
    /// Sommes-nous au moins demi-opérateur (`h`) ?
    pub fn am_halfop(&self, chan: &str) -> bool {
        if self.am_op(chan) {
            return true;
        }
        self.member_modes(chan, &self.nick).is_some_and(|m| m.contains('h'))
    }

    // ---------------------------------------------------------------------------------------------------------
    // Requêtes
    // ---------------------------------------------------------------------------------------------------------

    /// `WHO <cible>` (WHOX si possible pour avoir les comptes). `auto` : la réponse n'est pas à afficher.
    pub fn request_who(&mut self, target: &str, auto: bool) {
        if auto {
            self.auto_who.push(self.fold(target));
        }
        if self.isupport.whox {
            self.push(Message::new("WHO", [target.to_string(), format!("%tcuhnfar,{WHOX_TOKEN}")]));
        } else {
            self.push(Message::new("WHO", [target]));
        }
    }

    /// `MODE #canal` ; `auto` : réponse non affichée.
    pub fn request_modes(&mut self, chan: &str, auto: bool) {
        if auto {
            self.auto_mode.insert(self.fold(chan));
        }
        self.push(Message::new("MODE", [chan]));
    }

    /// Liste `b`, `e` ou `I` d'un canal ; `quiet` : réponse non affichée (Channel Central).
    pub fn request_list(&mut self, chan: &str, mode: char, quiet: bool) {
        if quiet {
            self.quiet_lists.insert((self.fold(chan), mode));
        }
        self.push(Message::new("MODE", [chan.to_string(), format!("+{mode}")]));
    }

    /// `USERHOST n1 n2…` (5 au plus par ligne).
    pub fn request_userhost(&mut self, nicks: &[String], auto: bool) {
        for chunk in nicks.chunks(5) {
            if auto {
                self.auto_userhost += 1;
            }
            self.push(Message::new("USERHOST", chunk.iter().cloned()));
        }
    }

    /// `ISON n1 n2…`.
    pub fn request_ison(&mut self, nicks: &[String], auto: bool) {
        if nicks.is_empty() {
            return;
        }
        if auto {
            self.auto_ison += 1;
        }
        self.push(Message::new("ISON", [nicks.join(" ")]));
    }

    /// Notre `AWAY` : retient le message pour la réponse 306.
    pub fn set_away(&mut self, message: Option<&str>) {
        match message.filter(|m| !m.is_empty()) {
            Some(m) => {
                self.away_pending = Some(m.to_string());
                self.push(Message::new("AWAY", [m]));
            }
            None => {
                self.away_pending = None;
                self.push(Message::new::<&str>("AWAY", []));
            }
        }
    }

    /// Étiquette `label` pour une requête (labeled-response), si la capacité est active.
    pub fn next_label(&mut self) -> Option<String> {
        if !self.cap("labeled-response") {
            return None;
        }
        self.label_seq += 1;
        Some(format!("a{}", self.label_seq))
    }

    // ---------------------------------------------------------------------------------------------------------
    // Lecture des messages
    // ---------------------------------------------------------------------------------------------------------

    /// Lit une ligne du serveur, met l'état à jour, rend ce qui s'est passé.
    pub fn handle(&mut self, msg: &Message, now_ms: i64) -> Vec<SessionEvent> {
        // Messages d'un lot d'historique : mis de côté, sans toucher à l'état.
        if msg.command != "BATCH"
            && let Some(id) = msg.tags.get("batch")
            && let Some(root) = self.history_root(id)
        {
            self.history.entry(root).or_default().push(msg.clone());
            return Vec::new();
        }
        let mut ev = Vec::new();
        let cmd = msg.command.as_str();
        match cmd {
            "PING" => {
                let token = msg.params.first().cloned().unwrap_or_default();
                self.push(Message::new("PONG", msg.params.iter().cloned()));
                ev.push(SessionEvent::Ping { token });
            }
            "PONG" => ev.push(SessionEvent::Pong { server: msg.param(0).to_string(), token: msg.trailing().to_string() }),
            "CAP" => self.on_cap(msg, &mut ev),
            "AUTHENTICATE" => self.on_authenticate(msg, &mut ev),
            "ERROR" => ev.push(SessionEvent::Error { message: msg.trailing().to_string() }),
            "BATCH" => self.on_batch(msg, &mut ev),
            "JOIN" => self.on_join(msg, &mut ev),
            "PART" => self.on_part(msg, &mut ev),
            "KICK" => self.on_kick(msg, &mut ev),
            "QUIT" => self.on_quit(msg, &mut ev),
            "NICK" => self.on_nick(msg, &mut ev),
            "MODE" => self.on_mode(msg, now_ms, &mut ev),
            "TOPIC" => {
                let chan = msg.param(0).to_string();
                let topic = msg.param(1).to_string();
                let key = self.fold(&chan);
                if let Some(c) = self.channels.get_mut(&key) {
                    c.topic = if topic.is_empty() { None } else { Some(topic.clone()) };
                    c.topic_setter = Some(msg.nick().to_string());
                    c.topic_time = Some(now_ms / 1000);
                }
                ev.push(SessionEvent::Topic {
                    channel: chan,
                    setter: msg.nick().to_string(),
                    address: msg.source.as_ref().and_then(|s| s.address()),
                    topic,
                });
            }
            "PRIVMSG" | "NOTICE" => self.on_message(msg, now_ms, &mut ev),
            "INVITE" => {
                let target = msg.param(0).to_string();
                ev.push(SessionEvent::Invite {
                    from: msg.nick().to_string(),
                    address: msg.source.as_ref().and_then(|s| s.address()),
                    own: self.is_me(&target),
                    target,
                    channel: msg.param(1).to_string(),
                });
            }
            "AWAY" => {
                let nick = msg.nick().to_string();
                let message = msg.params.first().filter(|m| !m.is_empty()).cloned();
                if let Some(u) = self.users.get_mut(&self.isupport.casemapping.fold(&nick)) {
                    u.away = message.is_some();
                    u.away_message = message.clone();
                }
                if self.is_me(&nick) {
                    self.away = message.clone();
                }
                ev.push(SessionEvent::Away { nick, message });
            }
            "ACCOUNT" => {
                let nick = msg.nick().to_string();
                let acc = msg.param(0);
                let account = if acc == "*" || acc.is_empty() { None } else { Some(acc.to_string()) };
                if let Some(u) = self.users.get_mut(&self.isupport.casemapping.fold(&nick)) {
                    u.account = account.clone();
                }
                if self.is_me(&nick) {
                    self.account = account.clone();
                }
                ev.push(SessionEvent::Account { nick, account });
            }
            "CHGHOST" => {
                let nick = msg.nick().to_string();
                let (user, host) = (msg.param(0).to_string(), msg.param(1).to_string());
                if let Some(u) = self.users.get_mut(&self.isupport.casemapping.fold(&nick)) {
                    u.user = Some(user.clone());
                    u.host = Some(host.clone());
                }
                let own = self.is_me(&nick);
                if own {
                    self.user = Some(user.clone());
                    self.host = Some(host.clone());
                    ev.push(SessionEvent::OwnHost { user: Some(user.clone()), host: host.clone() });
                }
                ev.push(SessionEvent::Chghost { nick, user, host, own });
            }
            "SETNAME" => {
                let nick = msg.nick().to_string();
                let realname = msg.param(0).to_string();
                if let Some(u) = self.users.get_mut(&self.isupport.casemapping.fold(&nick)) {
                    u.realname = Some(realname.clone());
                }
                if self.is_me(&nick) {
                    self.cfg.realname = realname.clone();
                }
                ev.push(SessionEvent::Setname { nick, realname });
            }
            "WALLOPS" => ev.push(SessionEvent::Wallops { from: msg.nick().to_string(), text: msg.trailing().to_string() }),
            "BOUNCER" => {
                if msg.param(0).eq_ignore_ascii_case("NETWORK") {
                    let id = msg.param(1).to_string();
                    let raw = msg.param(2);
                    let deleted = raw == "*";
                    let attrs = if deleted { Vec::new() } else { parse_attrs(raw) };
                    ev.push(SessionEvent::BouncerNetwork { id, attrs, deleted });
                }
            }
            "MARKREAD" => {
                let target = msg.param(0).to_string();
                let ts = msg.param(1);
                let timestamp = ts.strip_prefix("timestamp=").and_then(crate::timeutil::parse_iso8601);
                ev.push(SessionEvent::ReadMarker { target, timestamp });
            }
            "FAIL" | "WARN" | "NOTE" => {
                let kind = match cmd {
                    "FAIL" => ReplyKind::Fail,
                    "WARN" => ReplyKind::Warn,
                    _ => ReplyKind::Note,
                };
                let n = msg.params.len();
                let context = if n > 3 { msg.params[2..n - 1].to_vec() } else { Vec::new() };
                ev.push(SessionEvent::StandardReply {
                    kind,
                    command: msg.param(0).to_string(),
                    code: msg.param(1).to_string(),
                    context,
                    description: msg.trailing().to_string(),
                });
            }
            _ if alpirc_proto::numeric::is_numeric(cmd) => self.on_numeric(msg, now_ms, &mut ev),
            _ => {}
        }
        ev
    }

    /// Lot d'historique (ou lot contenu dans un lot d'historique) auquel appartient `id`.
    fn history_root(&self, id: &str) -> Option<String> {
        let mut cur = id.to_string();
        let mut found = None;
        for _ in 0..16 {
            let Some(b) = self.batches.get(&cur) else { break };
            if b.kind == "chathistory" {
                found = Some(cur.clone());
            }
            match &b.parent {
                Some(p) => cur = p.clone(),
                None => break,
            }
        }
        found
    }

    fn on_batch(&mut self, msg: &Message, ev: &mut Vec<SessionEvent>) {
        let reference = msg.param(0);
        if let Some(id) = reference.strip_prefix('+') {
            let kind = msg.param(1).to_string();
            let params: Vec<String> = msg.params.iter().skip(2).cloned().collect();
            let parent = msg.tags.get("batch").map(str::to_string);
            self.batches.insert(id.to_string(), Batch { kind: kind.clone(), params: params.clone(), parent });
            ev.push(SessionEvent::BatchStart { id: id.to_string(), kind, params });
        } else if let Some(id) = reference.strip_prefix('-') {
            let root_before = self.history_root(id);
            let Some(b) = self.batches.remove(id) else { return };
            if b.kind == "chathistory" && root_before.as_deref() == Some(id) {
                let messages = self.history.remove(id).unwrap_or_default();
                // Un lot d'historique dans un autre : ses messages rejoignent ceux du parent.
                if let Some(parent_root) = b.parent.as_deref().and_then(|p| self.history_root(p)) {
                    self.history.entry(parent_root).or_default().extend(messages);
                } else {
                    let target = b.params.first().cloned().unwrap_or_default();
                    ev.push(SessionEvent::HistoryBatch { target, messages });
                }
            }
            ev.push(SessionEvent::BatchEnd { id: id.to_string(), kind: b.kind, params: b.params });
        }
    }

    // --- capacités ---

    fn wanted(&self, name: &str, value: &str) -> bool {
        if self.caps_enabled.contains(name) {
            return false;
        }
        if name == "sasl" {
            let Some(s) = &self.cfg.sasl else { return false };
            return value.is_empty() || value.split(',').any(|m| m.eq_ignore_ascii_case(s.mechanism.name()));
        }
        self.cfg.request_caps.iter().any(|c| c == name)
    }

    fn request_caps(&mut self, names: Vec<String>) {
        let mut line = String::new();
        let mut lines = Vec::new();
        for n in names {
            if !line.is_empty() && line.len() + n.len() + 1 > 400 {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(&n);
            self.caps_pending.insert(n);
        }
        if !line.is_empty() {
            lines.push(line);
        }
        for l in lines {
            self.push(Message::new("CAP", ["REQ".to_string(), l]));
        }
    }

    fn on_cap(&mut self, msg: &Message, ev: &mut Vec<SessionEvent>) {
        let sub = msg.param(1).to_ascii_uppercase();
        let caps_param = msg.trailing();
        let parse = |s: &str| -> Vec<(String, String)> {
            s.split_whitespace()
                .map(|c| match c.split_once('=') {
                    Some((k, v)) => (k.to_string(), v.to_string()),
                    None => (c.to_string(), String::new()),
                })
                .collect()
        };
        match sub.as_str() {
            "LS" | "NEW" => {
                let multi = sub == "LS" && msg.params.len() >= 4 && msg.param(2) == "*";
                let list = parse(caps_param);
                let mut want = Vec::new();
                for (k, v) in &list {
                    self.caps_available.insert(k.clone(), v.clone());
                    if k == "sts" {
                        ev.push(SessionEvent::Sts(parse_sts(v)));
                    }
                }
                if multi {
                    return;
                }
                let names: Vec<(String, String)> = if sub == "LS" {
                    self.caps_available.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                } else {
                    list
                };
                for (k, v) in names {
                    if self.wanted(&k, &v) && !self.caps_pending.contains(&k) {
                        want.push(k);
                    }
                }
                if want.is_empty() {
                    if sub == "LS" {
                        self.end_cap(ev);
                    }
                } else {
                    self.request_caps(want);
                }
            }
            "ACK" => {
                let mut added = Vec::new();
                let mut removed = Vec::new();
                for (k, _) in parse(caps_param) {
                    if let Some(n) = k.strip_prefix('-') {
                        self.caps_enabled.remove(n);
                        self.caps_pending.remove(n);
                        removed.push(n.to_string());
                    } else {
                        self.caps_pending.remove(&k);
                        self.caps_enabled.insert(k.clone());
                        added.push(k);
                    }
                }
                ev.push(SessionEvent::CapsChanged { added, removed });
                self.cap_continue(ev);
            }
            "NAK" => {
                for (k, _) in parse(caps_param) {
                    self.caps_pending.remove(&k);
                }
                self.cap_continue(ev);
            }
            "DEL" => {
                let mut removed = Vec::new();
                for (k, _) in parse(caps_param) {
                    self.caps_available.remove(&k);
                    if self.caps_enabled.remove(&k) {
                        removed.push(k);
                    }
                }
                ev.push(SessionEvent::CapsChanged { added: Vec::new(), removed });
            }
            _ => {}
        }
    }

    fn cap_continue(&mut self, ev: &mut Vec<SessionEvent>) {
        if !self.caps_pending.is_empty() || !self.cap_negotiating {
            return;
        }
        if self.cfg.sasl.is_some() && self.caps_enabled.contains("sasl") && matches!(self.sasl, SaslState::Idle) {
            let mech = self.cfg.sasl.as_ref().map(|s| s.mechanism).unwrap_or(Mechanism::Plain);
            self.sasl = SaslState::Started;
            self.push(Message::new("AUTHENTICATE", [mech.name()]));
            return;
        }
        if matches!(self.sasl, SaslState::Idle | SaslState::Done) {
            self.end_cap(ev);
        }
    }

    fn end_cap(&mut self, ev: &mut Vec<SessionEvent>) {
        if !self.cap_negotiating {
            return;
        }
        self.cap_negotiating = false;
        if let Some(id) = self.cfg.bouncer_bind.clone() {
            self.push(Message::new("BOUNCER", ["BIND".to_string(), id]));
        }
        self.push(Message::new("CAP", ["END"]));
        ev.push(SessionEvent::CapEnd);
    }

    fn send_auth(&mut self, b64: &str) {
        for c in sasl::chunks(b64) {
            self.push(Message::new("AUTHENTICATE", [c]));
        }
    }

    fn sasl_abort(&mut self, reason: &str, ev: &mut Vec<SessionEvent>) {
        self.push(Message::new("AUTHENTICATE", ["*"]));
        self.sasl = SaslState::Done;
        ev.push(SessionEvent::SaslFailed { reason: reason.to_string() });
        self.end_cap(ev);
    }

    fn on_authenticate(&mut self, msg: &Message, ev: &mut Vec<SessionEvent>) {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD;
        let param = msg.param(0).to_string();
        let Some(cfg) = self.cfg.sasl.clone() else { return };
        let state = std::mem::replace(&mut self.sasl, SaslState::Idle);
        self.sasl = match state {
            SaslState::Started => match cfg.mechanism {
                Mechanism::Plain => {
                    self.send_auth(&sasl::plain(&cfg.account, &cfg.password));
                    SaslState::Sent
                }
                Mechanism::External => {
                    self.send_auth("");
                    SaslState::Sent
                }
                Mechanism::ScramSha256 => {
                    let scram = match &self.cfg.scram_nonce {
                        Some(n) => Scram::with_nonce(&cfg.account, &cfg.password, n),
                        None => Scram::new(&cfg.account, &cfg.password),
                    };
                    self.send_auth(&b64.encode(scram.client_first()));
                    SaslState::ScramFirst(Box::new(scram), Reassembler::default())
                }
            },
            SaslState::ScramFirst(mut scram, mut re) => match re.push(&param) {
                None => SaslState::ScramFirst(scram, re),
                Some(data) => {
                    let server_first = String::from_utf8_lossy(&data).into_owned();
                    match scram.client_final(&server_first) {
                        Ok(fin) => {
                            self.send_auth(&b64.encode(fin));
                            SaslState::ScramFinal(scram, Reassembler::default())
                        }
                        Err(e) => {
                            self.sasl_abort(e, ev);
                            return;
                        }
                    }
                }
            },
            SaslState::ScramFinal(scram, mut re) => match re.push(&param) {
                None => SaslState::ScramFinal(scram, re),
                Some(data) => {
                    let server_final = String::from_utf8_lossy(&data).into_owned();
                    if scram.verify(&server_final) {
                        self.send_auth("");
                        SaslState::Sent
                    } else {
                        self.sasl_abort("signature du serveur invalide", ev);
                        return;
                    }
                }
            },
            other => other,
        };
    }

    // --- pseudos de secours ---

    fn nick_candidate(&self, attempt: usize) -> String {
        let base = &self.cfg.nick;
        match attempt {
            0 => base.clone(),
            1 if !self.cfg.alt_nick.is_empty() && self.cfg.alt_nick != *base => self.cfg.alt_nick.clone(),
            1 | 2 => format!("{base}_"),
            n => {
                let suffix = (n - 2).to_string();
                let max = self.isupport.nicklen.max(suffix.len() + 1);
                let keep: String = base.chars().take(max.saturating_sub(suffix.len())).collect();
                format!("{keep}{suffix}")
            }
        }
    }

    // --- enregistrement et numériques ---

    fn on_numeric(&mut self, msg: &Message, now_ms: i64, ev: &mut Vec<SessionEvent>) {
        use alpirc_proto::numeric as n;
        let cmd = msg.command.as_str();
        let p = |i: usize| msg.param(i).to_string();
        match cmd {
            n::RPL_WELCOME => {
                self.registered = true;
                self.cap_negotiating = false;
                self.nick = p(0);
                if let Some(src) = &msg.source {
                    self.server_name = src.raw.clone();
                }
                if let Some(last) = msg.trailing().split_whitespace().last()
                    && let Some((nick, rest)) = last.split_once('!')
                    && let Some((u, h)) = rest.split_once('@')
                    && self.eq(nick, &self.nick)
                {
                    self.user = Some(u.to_string());
                    self.host = Some(h.to_string());
                }
                ev.push(SessionEvent::Registered { nick: self.nick.clone() });
                self.maybe_nickserv();
            }
            n::RPL_ISUPPORT => {
                let k = msg.params.len();
                if k >= 3 {
                    self.isupport.apply(&msg.params[1..k - 1]);
                } else if k == 2 {
                    self.isupport.apply(&msg.params[1..]);
                }
                ev.push(SessionEvent::IsupportChanged);
            }
            n::RPL_ENDOFMOTD | n::ERR_NOMOTD => {
                if !self.motd_done {
                    self.motd_done = true;
                    ev.push(SessionEvent::MotdEnd);
                }
            }
            n::RPL_UMODEIS => {
                let modes = p(1);
                self.user_modes = modes.trim_start_matches('+').to_string();
            }
            n::RPL_HOSTHIDDEN => {
                let h = p(1);
                let (user, host) = match h.split_once('@') {
                    Some((u, h)) => (Some(u.to_string()), h.to_string()),
                    None => (None, h),
                };
                if user.is_some() {
                    self.user = user.clone();
                }
                self.host = Some(host.clone());
                ev.push(SessionEvent::OwnHost { user, host });
            }
            n::ERR_NICKNAMEINUSE | n::ERR_ERRONEUSNICKNAME | n::ERR_UNAVAILRESOURCE | n::ERR_NICKCOLLISION => {
                if !self.registered {
                    let failed = p(1);
                    if cmd == n::ERR_UNAVAILRESOURCE && self.is_channel(&failed) {
                        return;
                    }
                    self.nick_attempt += 1;
                    let next = self.nick_candidate(self.nick_attempt);
                    self.nick = next.clone();
                    self.push(Message::new("NICK", [next.clone()]));
                    ev.push(SessionEvent::NickRetry { failed, next });
                }
            }
            n::RPL_LOGGEDIN => {
                let account = p(2);
                if let Some((_, rest)) = msg.param(1).split_once('!')
                    && let Some((u, h)) = rest.split_once('@')
                {
                    self.user = Some(u.to_string());
                    self.host = Some(h.to_string());
                }
                self.account = Some(account.clone());
                ev.push(SessionEvent::LoggedIn { account });
            }
            n::RPL_LOGGEDOUT => {
                self.account = None;
                ev.push(SessionEvent::LoggedOut);
            }
            n::RPL_SASLSUCCESS => {
                self.sasl = SaslState::Done;
                self.sasl_ok = true;
                ev.push(SessionEvent::SaslSuccess);
                self.end_cap(ev);
            }
            n::ERR_SASLFAIL | n::ERR_SASLTOOLONG | n::ERR_SASLABORTED | n::ERR_NICKLOCKED => {
                if !matches!(self.sasl, SaslState::Idle) || self.cap_negotiating {
                    self.sasl = SaslState::Done;
                    ev.push(SessionEvent::SaslFailed { reason: msg.trailing().to_string() });
                    self.end_cap(ev);
                }
            }
            n::ERR_SASLALREADY => {
                self.sasl = SaslState::Done;
                self.end_cap(ev);
            }
            n::RPL_NOWAWAY => {
                self.away = Some(self.away_pending.clone().unwrap_or_else(|| "Away".into()));
                ev.push(SessionEvent::OwnAway { away: true });
            }
            n::RPL_UNAWAY => {
                self.away = None;
                ev.push(SessionEvent::OwnAway { away: false });
            }
            n::RPL_CHANNELMODEIS => {
                let chan = p(1);
                let key = self.fold(&chan);
                let auto = self.auto_mode.remove(&key);
                if auto {
                    self.auto_created.insert(key.clone());
                }
                let modes: String = msg.params.iter().skip(2).cloned().collect::<Vec<_>>().join(" ");
                let changes = modes::parse_channel(msg.param(2), msg.params.get(3..).unwrap_or(&[]), &self.isupport);
                if let Some(c) = self.channels.get_mut(&key) {
                    // On garde les listes et les préfixes ; on remplace les modes simples et à paramètre.
                    c.modes.clear();
                    c.mode_params.clear();
                    for ch in changes {
                        if ch.add && self.isupport.chanmode_type(ch.mode).is_some_and(|t| t >= 1) && self.isupport.rank(ch.mode).is_none() {
                            c.set_mode(ch.mode, ch.arg);
                        } else if ch.add && self.isupport.chanmode_type(ch.mode).is_none() {
                            c.set_mode(ch.mode, None);
                        }
                    }
                }
                ev.push(SessionEvent::ChannelModeIs { channel: chan, modes, auto });
            }
            n::RPL_CREATIONTIME => {
                let chan = p(1);
                let key = self.fold(&chan);
                let time = msg.param(2).parse().unwrap_or(0);
                if let Some(c) = self.channels.get_mut(&key) {
                    c.created = Some(time);
                }
                let auto = self.auto_created.remove(&key);
                ev.push(SessionEvent::CreationTime { channel: chan, time, auto });
            }
            n::RPL_TOPIC => {
                let chan = p(1);
                let topic = msg.trailing().to_string();
                let key = self.fold(&chan);
                if let Some(c) = self.channels.get_mut(&key) {
                    c.topic = Some(topic.clone());
                }
                ev.push(SessionEvent::TopicIs { channel: chan, topic });
            }
            n::RPL_NOTOPIC => {
                let chan = p(1);
                let key = self.fold(&chan);
                if let Some(c) = self.channels.get_mut(&key) {
                    c.topic = None;
                }
                ev.push(SessionEvent::NoTopic { channel: chan });
            }
            n::RPL_TOPICWHOTIME => {
                let chan = p(1);
                let setter = p(2);
                let time = msg.param(3).parse().ok();
                let key = self.fold(&chan);
                if let Some(c) = self.channels.get_mut(&key) {
                    c.topic_setter = Some(setter.clone());
                    c.topic_time = time;
                }
                ev.push(SessionEvent::TopicWhoTime { channel: chan, setter, time });
            }
            n::RPL_NAMREPLY => self.on_names(msg, ev),
            n::RPL_ENDOFNAMES => {
                let chan = p(1);
                let key = self.fold(&chan);
                let mut initial = false;
                if let Some(c) = self.channels.get_mut(&key) {
                    initial = !c.synced;
                    c.synced = true;
                    c.names_in_progress = false;
                }
                ev.push(SessionEvent::NamesEnd { channel: chan, initial });
            }
            n::RPL_BANLIST | n::RPL_EXCEPTLIST | n::RPL_INVITELIST => {
                let mode = match cmd {
                    n::RPL_BANLIST => 'b',
                    n::RPL_EXCEPTLIST => self.isupport.excepts.unwrap_or('e'),
                    _ => self.isupport.invex.unwrap_or('I'),
                };
                let chan = p(1);
                let key = self.fold(&chan);
                let entry = ListEntry {
                    mask: p(2),
                    setter: msg.params.get(3).cloned(),
                    time: msg.params.get(4).and_then(|t| t.parse().ok()),
                };
                let is = self.isupport.clone();
                if let Some(c) = self.channels.get_mut(&key) {
                    if !c.lists_in_progress.contains(&mode) {
                        c.lists_in_progress.push(mode);
                        if let Some(l) = c.list_mut(mode, &is) {
                            l.clear();
                        }
                    }
                    if let Some(l) = c.list_mut(mode, &is)
                        && !l.iter().any(|e| e.mask == entry.mask)
                    {
                        l.push(entry.clone());
                    }
                }
                let quiet = self.quiet_lists.contains(&(key, mode));
                ev.push(SessionEvent::ListEntry { channel: chan, mode, entry, quiet });
            }
            n::RPL_ENDOFBANLIST | n::RPL_ENDOFEXCEPTLIST | n::RPL_ENDOFINVITELIST => {
                let mode = match cmd {
                    n::RPL_ENDOFBANLIST => 'b',
                    n::RPL_ENDOFEXCEPTLIST => self.isupport.excepts.unwrap_or('e'),
                    _ => self.isupport.invex.unwrap_or('I'),
                };
                let chan = p(1);
                let key = self.fold(&chan);
                let is = self.isupport.clone();
                if let Some(c) = self.channels.get_mut(&key) {
                    if !c.lists_in_progress.contains(&mode)
                        && let Some(l) = c.list_mut(mode, &is)
                    {
                        // Liste vide.
                        l.clear();
                    }
                    c.lists_in_progress.retain(|m| *m != mode);
                }
                let quiet = self.quiet_lists.remove(&(key, mode));
                ev.push(SessionEvent::ListEnd { channel: chan, mode, quiet });
            }
            n::RPL_WHOREPLY => {
                // me #chan user host server nick flags :hops realname
                let chan = p(1);
                let nick = p(5);
                let flags = p(6);
                let realname = msg.param(7).split_once(' ').map(|(_, r)| r.to_string()).unwrap_or_default();
                self.who_update(&nick, Some(p(2)), Some(p(3)), &flags, None, Some(realname), &chan);
                let auto = self.who_is_auto(&chan);
                ev.push(SessionEvent::WhoReply { nick, channel: self.is_channel(&chan).then_some(chan), auto });
            }
            n::RPL_WHOSPCRPL => {
                // me token chan user host nick flags account :realname
                if msg.param(1) == WHOX_TOKEN && msg.params.len() >= 9 {
                    let chan = p(2);
                    let nick = p(5);
                    let acc = p(7);
                    let account = if acc == "0" || acc == "*" { Some(None) } else { Some(Some(acc)) };
                    self.who_update(&nick, Some(p(3)), Some(p(4)), &p(6), account, Some(p(8)), &chan);
                    let auto = self.who_is_auto(&chan);
                    ev.push(SessionEvent::WhoReply { nick, channel: self.is_channel(&chan).then_some(chan), auto });
                }
            }
            n::RPL_ENDOFWHO => {
                let mask = p(1);
                let key = self.fold(&mask);
                let auto = match self.auto_who.iter().position(|m| *m == key) {
                    Some(i) => {
                        self.auto_who.remove(i);
                        true
                    }
                    None => false,
                };
                ev.push(SessionEvent::WhoEnd { mask, auto });
            }
            n::RPL_WHOISUSER => {
                let nick = p(1);
                let key = self.fold(&nick);
                let u = self.users.entry(key).or_insert_with(|| UserInfo { nick: nick.clone(), ..UserInfo::default() });
                u.user = Some(p(2));
                u.host = Some(p(3));
                u.realname = Some(msg.trailing().to_string());
            }
            n::RPL_WHOISACCOUNT => {
                let key = self.fold(msg.param(1));
                if let Some(u) = self.users.get_mut(&key) {
                    u.account = Some(p(2));
                }
            }
            n::RPL_AWAY => {
                let key = self.fold(msg.param(1));
                if let Some(u) = self.users.get_mut(&key) {
                    u.away = true;
                    u.away_message = Some(msg.trailing().to_string());
                }
            }
            n::RPL_USERHOST => {
                let mut entries = Vec::new();
                for item in msg.trailing().split_whitespace() {
                    let Some((nick, rest)) = item.split_once('=') else { continue };
                    let nick = nick.trim_end_matches('*');
                    let away = rest.starts_with('-');
                    let rest = rest.trim_start_matches(['+', '-']);
                    let Some((u, h)) = rest.split_once('@') else { continue };
                    let key = self.fold(nick);
                    let e = self.users.entry(key).or_insert_with(|| UserInfo { nick: nick.to_string(), ..UserInfo::default() });
                    e.user = Some(u.to_string());
                    e.host = Some(h.to_string());
                    e.away = away;
                    entries.push((nick.to_string(), u.to_string(), h.to_string()));
                }
                let auto = if self.auto_userhost > 0 {
                    self.auto_userhost -= 1;
                    true
                } else {
                    false
                };
                ev.push(SessionEvent::Userhost { entries, auto });
            }
            n::RPL_ISON => {
                let nicks: Vec<String> = msg.trailing().split_whitespace().map(str::to_string).collect();
                let auto = if self.auto_ison > 0 {
                    self.auto_ison -= 1;
                    true
                } else {
                    false
                };
                ev.push(SessionEvent::Ison { nicks, auto });
            }
            n::RPL_MONONLINE => {
                let mut list = Vec::new();
                for t in msg.trailing().split(',').filter(|t| !t.is_empty()) {
                    let (nick, addr) = match t.split_once('!') {
                        Some((n, a)) => (n.to_string(), Some(a.to_string())),
                        None => (t.to_string(), None),
                    };
                    self.monitor.insert(self.fold(&nick), true);
                    list.push((nick, addr));
                }
                ev.push(SessionEvent::MonitorOnline(list));
            }
            n::RPL_MONOFFLINE => {
                let mut list = Vec::new();
                for t in msg.trailing().split(',').filter(|t| !t.is_empty()) {
                    let nick = t.split('!').next().unwrap_or(t).to_string();
                    self.monitor.insert(self.fold(&nick), false);
                    list.push(nick);
                }
                ev.push(SessionEvent::MonitorOffline(list));
            }
            "421" if msg.param(1).eq_ignore_ascii_case("CAP") => {
                // Serveur sans CAP : l'enregistrement continue sans négociation.
                self.cap_negotiating = false;
            }
            _ => {}
        }
        let _ = now_ms;
    }

    fn who_is_auto(&self, target: &str) -> bool {
        let k = self.fold(target);
        self.auto_who.iter().any(|m| *m == k)
    }

    #[allow(clippy::too_many_arguments)]
    fn who_update(
        &mut self,
        nick: &str,
        user: Option<String>,
        host: Option<String>,
        flags: &str,
        account: Option<Option<String>>,
        realname: Option<String>,
        chan: &str,
    ) {
        let key = self.fold(nick);
        let u = self.users.entry(key.clone()).or_insert_with(|| UserInfo { nick: nick.to_string(), ..UserInfo::default() });
        u.nick = nick.to_string();
        if user.is_some() {
            u.user = user;
        }
        if host.is_some() {
            u.host = host;
        }
        u.away = flags.starts_with('G');
        if !u.away {
            u.away_message = None;
        }
        if let Some(a) = account {
            u.account = a;
        }
        if realname.is_some() {
            u.realname = realname;
        }
        // Préfixes du canal dans les drapeaux (`H@+`) : mise à jour des modes du membre.
        let is = self.isupport.clone();
        let ckey = self.fold(chan);
        if let Some(c) = self.channels.get_mut(&ckey)
            && let Some(m) = c.members.get_mut(&key)
        {
            let mut modes = String::new();
            for ch in flags.chars() {
                if let Some(md) = is.mode_for_symbol(ch) {
                    state::add_prefix_mode(&mut modes, md, &is);
                }
            }
            if !modes.is_empty() || !flags.chars().any(|c| is.mode_for_symbol(c).is_some()) {
                m.modes = modes;
            }
        }
        if self.eq(nick, &self.nick.clone()) {
            if let Some(u) = self.users.get(&key) {
                self.user = u.user.clone().or(self.user.take());
                self.host = u.host.clone().or(self.host.take());
            }
        }
    }

    fn maybe_nickserv(&mut self) {
        if self.nickserv_sent || self.sasl_ok {
            return;
        }
        if let Some(pw) = self.cfg.nickserv_password.clone().filter(|p| !p.is_empty()) {
            self.nickserv_sent = true;
            self.push(Message::new("PRIVMSG", ["NickServ".to_string(), format!("IDENTIFY {pw}")]));
        }
    }

    // --- canaux ---

    fn ial_touch(&mut self, nick: &str, user: Option<&str>, host: Option<&str>) {
        let key = self.fold(nick);
        let u = self.users.entry(key).or_insert_with(|| UserInfo { nick: nick.to_string(), ..UserInfo::default() });
        u.nick = nick.to_string();
        if let Some(x) = user {
            u.user = Some(x.to_string());
        }
        if let Some(x) = host {
            u.host = Some(x.to_string());
        }
    }

    fn ial_forget_if_alone(&mut self, nick: &str) {
        if self.is_me(nick) {
            return;
        }
        let key = self.fold(nick);
        if !self.channels.values().any(|c| c.members.contains_key(&key)) {
            self.users.remove(&key);
        }
    }

    fn on_join(&mut self, msg: &Message, ev: &mut Vec<SessionEvent>) {
        let Some(src) = msg.source.clone() else { return };
        let nick = src.nick().to_string();
        let channel = msg.param(0).to_string();
        let (account, realname) = if msg.params.len() >= 3 {
            let a = msg.param(1);
            (if a == "*" { None } else { Some(a.to_string()) }, Some(msg.param(2).to_string()))
        } else {
            (None, None)
        };
        let key = self.fold(&channel);
        let own = self.is_me(&nick);
        let nkey = self.fold(&nick);
        if own {
            let mut c = ChannelState::new(&channel);
            c.joined = true;
            self.channels.insert(key.clone(), c);
            if let Some(u) = src.user() {
                self.user = Some(u.to_string());
            }
            if let Some(h) = src.host() {
                self.host = Some(h.to_string());
            }
            self.request_modes(&channel, true);
            if self.cfg.who_on_join || self.cap("away-notify") {
                self.request_who(&channel, true);
            }
        }
        if let Some(c) = self.channels.get_mut(&key) {
            c.members.insert(nkey.clone(), Member { nick: nick.clone(), modes: String::new(), last_spoke: 0 });
        }
        self.ial_touch(&nick, src.user(), src.host());
        if let Some(u) = self.users.get_mut(&nkey) {
            if account.is_some() || msg.params.len() >= 3 {
                u.account = account.clone();
            }
            if realname.is_some() {
                u.realname = realname.clone();
            }
            if let Some(a) = msg.tags.get("account") {
                u.account = Some(a.to_string());
            }
        }
        ev.push(SessionEvent::Join { channel, nick, address: src.address(), account, realname, own });
    }

    fn on_part(&mut self, msg: &Message, ev: &mut Vec<SessionEvent>) {
        let nick = msg.nick().to_string();
        let own = self.is_me(&nick);
        let reason = msg.params.get(1).filter(|r| !r.is_empty()).cloned();
        let address = msg.source.as_ref().and_then(|s| s.address());
        for channel in msg.param(0).split(',').filter(|c| !c.is_empty()) {
            let key = self.fold(channel);
            if own {
                self.channels.remove(&key);
                let others: Vec<String> = self.users.values().map(|u| u.nick.clone()).collect();
                for o in others {
                    self.ial_forget_if_alone(&o);
                }
            } else {
                if let Some(c) = self.channels.get_mut(&key) {
                    c.members.remove(&self.isupport.casemapping.fold(&nick));
                }
                self.ial_forget_if_alone(&nick);
            }
            ev.push(SessionEvent::Part { channel: channel.to_string(), nick: nick.clone(), address: address.clone(), reason: reason.clone(), own });
        }
    }

    fn on_kick(&mut self, msg: &Message, ev: &mut Vec<SessionEvent>) {
        let channel = msg.param(0).to_string();
        let target = msg.param(1).to_string();
        let reason = msg.params.get(2).filter(|r| !r.is_empty()).cloned();
        let key = self.fold(&channel);
        let own = self.is_me(&target);
        if own {
            if let Some(c) = self.channels.get_mut(&key) {
                c.joined = false;
                c.members.clear();
                c.synced = false;
            }
            let others: Vec<String> = self.users.values().map(|u| u.nick.clone()).collect();
            for o in others {
                self.ial_forget_if_alone(&o);
            }
        } else {
            let t = self.fold(&target);
            if let Some(c) = self.channels.get_mut(&key) {
                c.members.remove(&t);
            }
            self.ial_forget_if_alone(&target);
        }
        ev.push(SessionEvent::Kick {
            channel,
            by: msg.nick().to_string(),
            by_address: msg.source.as_ref().and_then(|s| s.address()),
            target,
            reason,
            own,
        });
    }

    fn on_quit(&mut self, msg: &Message, ev: &mut Vec<SessionEvent>) {
        let nick = msg.nick().to_string();
        let key = self.fold(&nick);
        let mut channels = Vec::new();
        for c in self.channels.values_mut() {
            if c.members.remove(&key).is_some() {
                channels.push(c.name.clone());
            }
        }
        if !self.is_me(&nick) {
            self.users.remove(&key);
        }
        ev.push(SessionEvent::Quit {
            nick,
            address: msg.source.as_ref().and_then(|s| s.address()),
            reason: msg.params.first().filter(|r| !r.is_empty()).cloned(),
            channels,
        });
    }

    fn on_nick(&mut self, msg: &Message, ev: &mut Vec<SessionEvent>) {
        let old = msg.nick().to_string();
        let new = msg.param(0).to_string();
        let ok = self.fold(&old);
        let nk = self.fold(&new);
        let mut channels = Vec::new();
        for c in self.channels.values_mut() {
            if let Some(mut m) = c.members.remove(&ok) {
                m.nick = new.clone();
                c.members.insert(nk.clone(), m);
                channels.push(c.name.clone());
            }
        }
        if let Some(mut u) = self.users.remove(&ok) {
            u.nick = new.clone();
            self.users.insert(nk.clone(), u);
        }
        let own = self.is_me(&old);
        if own {
            self.nick = new.clone();
        }
        if let Some(v) = self.monitor.remove(&ok) {
            self.monitor.insert(ok, v);
        }
        ev.push(SessionEvent::Nick { old, new, address: msg.source.as_ref().and_then(|s| s.address()), channels, own });
    }

    fn on_mode(&mut self, msg: &Message, now_ms: i64, ev: &mut Vec<SessionEvent>) {
        let target = msg.param(0).to_string();
        let setter = msg.nick().to_string();
        let modes_str: String = msg.params.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
        if self.is_channel(&target) {
            let changes = modes::parse_channel(msg.param(1), msg.params.get(2..).unwrap_or(&[]), &self.isupport);
            let key = self.fold(&target);
            let is = self.isupport.clone();
            if let Some(c) = self.channels.get_mut(&key) {
                for ch in &changes {
                    if is.rank(ch.mode).is_some() {
                        if let Some(arg) = &ch.arg {
                            let nk = is.casemapping.fold(arg);
                            if let Some(m) = c.members.get_mut(&nk) {
                                if ch.add {
                                    state::add_prefix_mode(&mut m.modes, ch.mode, &is);
                                } else {
                                    m.modes.retain(|x| x != ch.mode);
                                }
                            }
                        }
                        continue;
                    }
                    match is.chanmode_type(ch.mode) {
                        Some(0) => {
                            if let (Some(arg), Some(l)) = (&ch.arg, c.list_mut(ch.mode, &is)) {
                                if ch.add {
                                    if !l.iter().any(|e| e.mask == *arg) {
                                        l.push(ListEntry { mask: arg.clone(), setter: Some(setter.clone()), time: Some(now_ms / 1000) });
                                    }
                                } else {
                                    l.retain(|e| !is.casemapping.eq(&e.mask, arg));
                                }
                            }
                        }
                        Some(1) | Some(2) => {
                            if ch.add {
                                c.set_mode(ch.mode, ch.arg.clone());
                            } else {
                                c.unset_mode(ch.mode);
                            }
                        }
                        _ => {
                            if ch.add {
                                c.set_mode(ch.mode, None);
                            } else {
                                c.unset_mode(ch.mode);
                            }
                        }
                    }
                }
            }
            ev.push(SessionEvent::ChannelMode {
                channel: target,
                setter,
                setter_address: msg.source.as_ref().and_then(|s| s.address()),
                modes: modes_str,
                changes,
            });
        } else {
            let changes = modes::parse_user(msg.param(1));
            if self.is_me(&target) {
                for ch in &changes {
                    if ch.add {
                        if !self.user_modes.contains(ch.mode) {
                            self.user_modes.push(ch.mode);
                        }
                    } else {
                        self.user_modes.retain(|c| c != ch.mode);
                    }
                }
            }
            ev.push(SessionEvent::UserMode { setter, modes: modes_str, changes });
        }
    }

    fn on_names(&mut self, msg: &Message, ev: &mut Vec<SessionEvent>) {
        // me = #chan :names
        let chan = msg.param(2).to_string();
        let key = self.fold(&chan);
        let is = self.isupport.clone();
        let names: Vec<String> = msg.trailing().split_whitespace().map(str::to_string).collect();
        let mut initial = false;
        let mut ial = Vec::new();
        if let Some(c) = self.channels.get_mut(&key) {
            initial = !c.synced;
            if !c.names_in_progress {
                c.names_in_progress = true;
                c.members.clear();
            }
            for n in &names {
                let (pmodes, rest) = is.split_prefixes(n);
                let (nick, user, host) = match rest.split_once('!') {
                    Some((nk, uh)) => match uh.split_once('@') {
                        Some((u, h)) => (nk, Some(u), Some(h)),
                        None => (nk, None, None),
                    },
                    None => (rest, None, None),
                };
                let mut modes = String::new();
                for m in pmodes.chars() {
                    state::add_prefix_mode(&mut modes, m, &is);
                }
                c.members.insert(is.casemapping.fold(nick), Member { nick: nick.to_string(), modes, last_spoke: 0 });
                ial.push((nick.to_string(), user.map(str::to_string), host.map(str::to_string)));
            }
        }
        for (n, u, h) in ial {
            self.ial_touch(&n, u.as_deref(), h.as_deref());
        }
        ev.push(SessionEvent::Names { channel: chan, names, initial });
    }

    fn on_message(&mut self, msg: &Message, now_ms: i64, ev: &mut Vec<SessionEvent>) {
        let kind = if msg.command == "NOTICE" { MsgKind::Notice } else { MsgKind::Privmsg };
        let target = msg.param(0).to_string();
        let text = msg.param(1).to_string();
        let (from, address, server) = match &msg.source {
            Some(s) => (s.nick().to_string(), s.address(), s.is_server() || (!self.registered && s.user().is_none())),
            None => (self.server_name.clone(), None, true),
        };
        let mut statusmsg = None;
        let mut chan_name = target.as_str();
        if let Some(c) = target.chars().next()
            && self.isupport.statusmsg.contains(c)
            && self.is_channel(&target[c.len_utf8()..])
        {
            statusmsg = Some(c);
            chan_name = &target[c.len_utf8()..];
        }
        let channel = self.is_channel(chan_name).then(|| chan_name.to_string());
        let own = !server && self.is_me(&from);
        if !server {
            if let Some(src) = &msg.source
                && self.users.contains_key(&self.fold(&from))
            {
                let (u, h) = (src.user().map(str::to_string), src.host().map(str::to_string));
                self.ial_touch(&from, u.as_deref(), h.as_deref());
            }
            if let Some(acc) = msg.tags.get("account")
                && let Some(u) = self.users.get_mut(&self.isupport.casemapping.fold(&from))
            {
                u.account = Some(acc.to_string());
            }
            if let Some(ch) = &channel {
                let (ck, nk) = (self.fold(ch), self.fold(&from));
                if let Some(m) = self.channels.get_mut(&ck).and_then(|c| c.members.get_mut(&nk)) {
                    m.last_spoke = now_ms;
                }
            }
            if kind == MsgKind::Notice && from.eq_ignore_ascii_case("NickServ") {
                let l = text.to_ascii_lowercase();
                if l.contains("identify") && (l.contains("registered") || l.contains("protected") || l.contains("please")) {
                    self.maybe_nickserv();
                }
            }
        }
        ev.push(SessionEvent::Message { kind, from, address, target, text, channel, statusmsg, own, server });
    }
}

/// Attributs d'un réseau du bouncer (`name=Libera;state=connected`), valeurs déséchappées comme les étiquettes.
pub fn parse_attrs(raw: &str) -> Vec<(String, String)> {
    let line = format!("@{raw} X");
    match Message::parse(&line) {
        Ok(m) => m.tags.0.into_iter().map(|(k, v)| (k, v.unwrap_or_default())).collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(s: &mut Session, line: &str) -> Vec<SessionEvent> {
        s.handle(&Message::parse(line).unwrap(), 1_000)
    }

    fn sent(s: &mut Session) -> Vec<String> {
        s.take_outgoing().iter().map(|m| m.to_line()).collect()
    }

    #[test]
    fn registration_without_cap() {
        let mut s = Session::new(SessionConfig::default());
        s.start();
        assert_eq!(sent(&mut s), vec!["CAP LS 302", "NICK Guiwald", "USER alpirc 0 * :AlpIRC user"]);
        feed(&mut s, ":srv 421 * CAP :Unknown command");
        assert!(!s.cap_negotiating());
        let ev = feed(&mut s, ":srv 001 Guiwald :Welcome Guiwald!gui@host.fr");
        assert_eq!(ev, vec![SessionEvent::Registered { nick: "Guiwald".into() }]);
        assert_eq!(s.host(), Some("host.fr"));
    }

    #[test]
    fn loc_passwords() {
        assert_eq!(undernet_loc_password("gui", "pw", true, true), "+x! gui pw");
        assert_eq!(undernet_loc_password("gui", "pw", false, true), "-!+x gui pw");
        assert_eq!(undernet_loc_password("gui", "pw", false, false), "-x! gui pw");
    }

    #[test]
    fn cap_multiline_and_sts() {
        let mut s = Session::new(SessionConfig::default());
        s.start();
        sent(&mut s);
        assert!(feed(&mut s, ":srv CAP * LS * :multi-prefix sts=port=6697,duration=300").iter().any(|e| matches!(e, SessionEvent::Sts(_))));
        assert!(sent(&mut s).is_empty());
        feed(&mut s, ":srv CAP * LS :server-time foo");
        assert_eq!(sent(&mut s), vec!["CAP REQ :multi-prefix server-time"]);
        feed(&mut s, ":srv CAP * ACK :multi-prefix server-time");
        assert_eq!(sent(&mut s), vec!["CAP END"]);
        assert!(s.cap("server-time"));
        feed(&mut s, ":srv CAP Guiwald DEL :server-time");
        assert!(!s.cap("server-time"));
    }

    #[test]
    fn nick_candidates() {
        let mut s = Session::new(SessionConfig::default());
        assert_eq!(s.nick_candidate(1), "Guiwald_");
        assert_eq!(s.nick_candidate(3), "Guiwald1");
        s.cfg.alt_nick = String::new();
        assert_eq!(s.nick_candidate(1), "Guiwald_");
        s.cfg.nick = "VeryLongNick".into();
        assert_eq!(s.nick_candidate(4), "VeryLong2");
    }

    #[test]
    fn attrs() {
        assert_eq!(parse_attrs("name=Libera\\sChat;state=connected"), vec![("name".into(), "Libera Chat".into()), ("state".into(), "connected".into())]);
    }
}
