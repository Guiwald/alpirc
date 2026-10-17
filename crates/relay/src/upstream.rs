//! La connexion permanente d'un utilisateur à un de ses réseaux : la session IRC d'`alpirc-core`
//! ([`Session`]), l'état de la connexion, la file anti-flood ([`Bucket`]), la reconnexion avec délai croissant
//! et rotation des serveurs, l'absence automatique.

use std::collections::{HashMap, VecDeque};

use alpirc_core::session::{DEFAULT_CAPS, SaslConfig, Session, SessionConfig, undernet_loc_password};
use alpirc_core::settings::{Network, Server};
use alpirc_proto::encoding::Encoding;
use alpirc_proto::sasl::Mechanism;

use crate::networks::{EffectiveNetwork, SavedChannel, nick_or};
use crate::routing::Pending;

/// Capacités demandées aux réseaux : celles qu'`alpirc-core` sait lire, sans celles que le relais assure
/// lui-même pour ses appareils (`echo-message`, `labeled-response`, historique, marques de lecture,
/// extensions de bouncer).
pub fn upstream_caps() -> Vec<String> {
    const OWN: &[&str] = &[
        "echo-message",
        "labeled-response",
        "draft/chathistory",
        "draft/read-marker",
        "draft/event-playback",
        "soju.im/bouncer-networks",
        "soju.im/bouncer-networks-notify",
    ];
    DEFAULT_CAPS.iter().filter(|c| !OWN.contains(c)).map(|c| c.to_string()).collect()
}

/// Seau à jetons : une rafale de `burst` lignes, puis une toutes les `interval_ms`.
#[derive(Debug, Clone)]
pub struct Bucket {
    tokens: f64,
    last: i64,
    burst: u32,
    interval_ms: u64,
    queue: VecDeque<Vec<u8>>,
}

impl Bucket {
    pub fn new(burst: u32, interval_ms: u64, now: i64) -> Bucket {
        Bucket {
            tokens: burst.max(1) as f64,
            last: now,
            burst: burst.max(1),
            interval_ms: interval_ms.max(1),
            queue: VecDeque::new(),
        }
    }

    fn refill(&mut self, now: i64) {
        let elapsed = (now - self.last).max(0) as f64;
        self.tokens = (self.tokens + elapsed / self.interval_ms as f64).min(self.burst as f64);
        self.last = now;
    }

    /// Prend un jeton tout de suite si possible (anti-flood à l'entrée).
    pub fn take(&mut self, now: i64) -> bool {
        self.refill(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self, now: i64) {
        self.queue.clear();
        self.tokens = self.burst as f64;
        self.last = now;
    }

    pub fn push(&mut self, line: Vec<u8>) {
        self.queue.push_back(line);
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Lignes qui peuvent partir maintenant.
    pub fn drain(&mut self, now: i64) -> Vec<Vec<u8>> {
        self.refill(now);
        let mut out = Vec::new();
        while self.tokens >= 1.0 {
            let Some(l) = self.queue.pop_front() else { break };
            self.tokens -= 1.0;
            out.push(l);
        }
        out
    }

    /// Heure où la prochaine ligne en file pourra partir.
    pub fn next_due(&self) -> Option<i64> {
        if self.queue.is_empty() {
            return None;
        }
        let missing = (1.0 - self.tokens).max(0.0);
        Some(self.last + (missing * self.interval_ms as f64).ceil() as i64)
    }
}

/// État de la connexion à un réseau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpState {
    Disconnected,
    /// Connexion TCP/TLS en cours.
    Connecting,
    /// Connecté, enregistrement en cours (jusqu'à la fin du MOTD).
    Registering,
    /// Prêt (fin du MOTD reçue).
    Ready,
}

impl UpState {
    /// Valeur de l'attribut `state` de `soju.im/bouncer-networks`.
    pub fn attr(self) -> &'static str {
        match self {
            UpState::Disconnected => "disconnected",
            UpState::Connecting | UpState::Registering => "connecting",
            UpState::Ready => "connected",
        }
    }
}

/// La connexion d'un utilisateur à un réseau.
pub struct Upstream {
    pub user: String,
    pub net: EffectiveNetwork,
    /// Identifiant `alpirc-net` de la tentative en cours.
    pub conn: Option<u32>,
    pub state: UpState,
    pub session: Session,
    pub server_idx: usize,
    pub attempts: u32,
    pub reconnect_at: Option<i64>,
    /// Reconnexion immédiate après la fermeture en cours (réglages changés).
    pub reconnect_now: bool,
    /// Plus de reconnexion (réseau supprimé, arrêt).
    pub stopped: bool,
    pub state_since: i64,
    pub last_rx: i64,
    pub ping_sent: Option<i64>,
    pub bucket: Bucket,
    pub encoding: Encoding,
    /// Dernière erreur (attribut `error`).
    pub error: Option<String>,
    /// Absence posée par le relais (aucun appareil).
    pub auto_away: bool,
    /// Réponses 305/306 dues à l'absence automatique, à ne pas montrer.
    pub auto_away_replies: u32,
    /// Absence posée par un appareil.
    pub user_away: Option<String>,
    /// Clés des canaux demandés par les appareils (`JOIN #a clé`), par canal replié.
    pub pending_keys: HashMap<String, String>,
    pub pending: Pending,
    /// Réponses CTCP récemment transmises (heure, cible repliée, commande) : une seule pour plusieurs appareils.
    pub ctcp_replies: VecDeque<(i64, String, String)>,
    /// Hôte, port et TLS de la tentative en cours.
    pub current: Option<(String, u16, bool)>,
    /// Raison de la fermeture en cours (ERROR du serveur, délai de ping…).
    pub close_reason: Option<String>,
}

impl Upstream {
    pub fn new(user: &str, net: EffectiveNetwork, burst: u32, interval_ms: u64, now: i64) -> Upstream {
        let encoding = Encoding::from_name(&net.settings.encoding);
        let cfg = session_config(user, &net.settings, net.settings.servers.first(), false);
        Upstream {
            user: user.to_string(),
            net,
            conn: None,
            state: UpState::Disconnected,
            session: Session::new(cfg),
            server_idx: 0,
            attempts: 0,
            reconnect_at: None,
            reconnect_now: false,
            stopped: false,
            state_since: now,
            last_rx: now,
            ping_sent: None,
            bucket: Bucket::new(burst, interval_ms, now),
            encoding,
            error: None,
            auto_away: false,
            auto_away_replies: 0,
            user_away: None,
            pending_keys: HashMap::new(),
            pending: Pending::default(),
            ctcp_replies: VecDeque::new(),
            current: None,
            close_reason: None,
        }
    }

    pub fn netid(&self) -> &str {
        &self.net.id
    }

    pub fn registered(&self) -> bool {
        matches!(self.state, UpState::Registering | UpState::Ready) && self.session.is_registered()
    }

    /// Serveur de la prochaine tentative.
    pub fn server(&self) -> Option<&Server> {
        let s = &self.net.settings.servers;
        if s.is_empty() { None } else { s.get(self.server_idx % s.len()) }
    }

    /// Délai avant le prochain essai (double à chaque échec, entre `min` et `max` secondes).
    pub fn backoff_ms(&self, min_secs: u64, max_secs: u64) -> i64 {
        let base = min_secs.max(1);
        let shift = self.attempts.saturating_sub(1).min(16);
        (base.saturating_mul(1u64 << shift).min(max_secs.max(base)) * 1000) as i64
    }

    /// Une réponse CTCP d'un appareil vers `target` : faut-il la transmettre (pas déjà fait il y a moins de 5 s) ?
    pub fn ctcp_reply_allowed(&mut self, target: &str, command: &str, now: i64) -> bool {
        self.ctcp_replies.retain(|(t, _, _)| now - t < 5_000);
        let key = (self.session.fold(target), command.to_ascii_uppercase());
        if self.ctcp_replies.iter().any(|(_, t, c)| *t == key.0 && *c == key.1) {
            return false;
        }
        self.ctcp_replies.push_back((now, key.0, key.1));
        true
    }
}

/// Configuration de la session pour un serveur d'un réseau.
pub fn session_config(user: &str, net: &Network, server: Option<&Server>, tls: bool) -> SessionConfig {
    let nick = nick_or(net, user);
    let mut password = server.and_then(|s| s.password.clone()).filter(|p| !p.is_empty());
    if let Some(x) = net.undernet_x.as_ref().filter(|x| !x.username.is_empty()) {
        password = Some(undernet_loc_password(&x.username, &x.password, x.require_x, x.hide_host));
    }
    let sasl = net.sasl.as_ref().and_then(|s| {
        let mech = Mechanism::from_name(&s.mechanism).unwrap_or(Mechanism::Plain);
        (mech == Mechanism::External || !s.account.is_empty()).then(|| SaslConfig {
            mechanism: mech,
            account: s.account.clone(),
            password: s.password.clone(),
        })
    });
    SessionConfig {
        alt_nick: net.alt_nick.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("{nick}_")),
        username: net.username.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| user.to_string()),
        realname: net.realname.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| nick.clone()),
        nick,
        password,
        sasl,
        nickserv_password: net.nickserv_password.clone().filter(|p| !p.is_empty()),
        request_caps: upstream_caps(),
        bouncer_bind: None,
        tls,
        who_on_join: true,
        scram_nonce: None,
    }
}

/// Une ligne de perform : commande IRC brute, `/` de tête toléré ; `/msg`, `/notice`, `/quote` et `/raw`
/// traduits.
pub fn perform_line(line: &str) -> Option<String> {
    let l = line.trim();
    if l.is_empty() {
        return None;
    }
    let Some(rest) = l.strip_prefix('/') else { return Some(l.to_string()) };
    let rest = rest.trim_start_matches('/');
    let (cmd, args) = rest.split_once(' ').unwrap_or((rest, ""));
    let args = args.trim();
    Some(match cmd.to_ascii_lowercase().as_str() {
        "msg" | "privmsg" => {
            let (t, text) = args.split_once(' ')?;
            format!("PRIVMSG {t} :{}", text.trim_start())
        }
        "notice" => {
            let (t, text) = args.split_once(' ')?;
            format!("NOTICE {t} :{}", text.trim_start())
        }
        "quote" | "raw" => args.to_string(),
        "j" => format!("JOIN {args}"),
        _ => {
            if args.is_empty() {
                cmd.to_ascii_uppercase()
            } else {
                format!("{} {args}", cmd.to_ascii_uppercase())
            }
        }
    })
    .filter(|s| !s.trim().is_empty())
}

/// Lignes `JOIN #a,#b clé` de moins de 400 octets (canaux à clé d'abord).
pub fn join_lines(chans: &[SavedChannel]) -> Vec<String> {
    let mut sorted: Vec<&SavedChannel> = chans.iter().collect();
    sorted.sort_by_key(|c| c.key.is_none());
    let mut out = Vec::new();
    let (mut names, mut keys): (Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new());
    let mut len = 0;
    let flush = |names: &mut Vec<&str>, keys: &mut Vec<&str>, out: &mut Vec<String>| {
        if names.is_empty() {
            return;
        }
        let mut l = format!("JOIN {}", names.join(","));
        if !keys.is_empty() {
            l.push(' ');
            l.push_str(&keys.join(","));
        }
        out.push(l);
        names.clear();
        keys.clear();
    };
    for c in sorted {
        let add = c.name.len() + 1 + c.key.as_ref().map(|k| k.len() + 1).unwrap_or(0);
        if len + add > 400 {
            flush(&mut names, &mut keys, &mut out);
            len = 0;
        }
        names.push(&c.name);
        if let Some(k) = &c.key {
            keys.push(k);
        }
        len += add;
    }
    flush(&mut names, &mut keys, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alpirc_core::settings::{SaslSettings, UndernetX};

    #[test]
    fn bucket() {
        let mut b = Bucket::new(2, 1000, 0);
        for i in 0..4 {
            b.push(vec![i]);
        }
        assert_eq!(b.drain(0), vec![vec![0], vec![1]]);
        assert_eq!(b.next_due(), Some(1000));
        assert!(b.drain(500).is_empty());
        assert_eq!(b.drain(1000), vec![vec![2]]);
        assert_eq!(b.drain(5000), vec![vec![3]]);
        assert_eq!(b.next_due(), None);
        let mut i = Bucket::new(2, 100, 0);
        assert!(i.take(0) && i.take(0) && !i.take(0));
        assert!(i.take(100));
    }

    #[test]
    fn caps_requested_upstream() {
        let c = upstream_caps();
        assert!(c.contains(&"server-time".to_string()));
        assert!(c.contains(&"multi-prefix".to_string()));
        assert!(!c.contains(&"echo-message".to_string()));
        assert!(!c.contains(&"draft/chathistory".to_string()));
    }

    #[test]
    fn session_configs() {
        let mut n = Network::new("Undernet", vec![Server::new("irc.undernet.org", 6667, false)]);
        let c = session_config("gui", &n, n.servers.first(), false);
        assert_eq!((c.nick.as_str(), c.alt_nick.as_str(), c.username.as_str()), ("gui", "gui_", "gui"));
        assert!(c.who_on_join);
        n.nick = Some("Guiwald".into());
        n.undernet_x = Some(UndernetX { username: "guiwald".into(), password: "pw".into(), ..UndernetX::default() });
        let c = session_config("gui", &n, n.servers.first(), false);
        assert_eq!(c.password.as_deref(), Some("+x! guiwald pw"));
        assert_eq!(c.realname, "Guiwald");
        n.sasl = Some(SaslSettings { account: "a".into(), password: "p".into(), ..SaslSettings::default() });
        let c = session_config("gui", &n, n.servers.first(), true);
        assert_eq!(c.sasl.unwrap().mechanism, Mechanism::Plain);
        assert!(c.tls);
    }

    #[test]
    fn perform_lines() {
        assert_eq!(perform_line("MODE Gui +x").as_deref(), Some("MODE Gui +x"));
        assert_eq!(perform_line("/mode Gui +x").as_deref(), Some("MODE Gui +x"));
        assert_eq!(perform_line("/msg X login gui pw").as_deref(), Some("PRIVMSG X :login gui pw"));
        assert_eq!(perform_line("/notice Bob hi").as_deref(), Some("NOTICE Bob :hi"));
        assert_eq!(perform_line("/quote OPER a b").as_deref(), Some("OPER a b"));
        assert_eq!(perform_line("/j #a").as_deref(), Some("JOIN #a"));
        assert_eq!(perform_line("   "), None);
        assert_eq!(perform_line("/msg X"), None);
    }

    #[test]
    fn joins() {
        let ch = |n: &str, k: Option<&str>| SavedChannel { name: n.into(), key: k.map(str::to_string) };
        assert_eq!(join_lines(&[ch("#a", None), ch("#b", Some("k"))]), vec!["JOIN #b,#a k"]);
        let many: Vec<SavedChannel> = (0..100).map(|i| ch(&format!("#canal-numero-{i}"), None)).collect();
        let lines = join_lines(&many);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| l.len() < 420));
    }

    #[test]
    fn backoff() {
        let n = EffectiveNetwork {
            id: "1".into(),
            settings: Network::new("N", vec![Server::new("a", 1, false), Server::new("b", 2, false)]),
            channels: Vec::new(),
            from_config: true,
            enabled: true,
        };
        let mut u = Upstream::new("gui", n, 10, 1000, 0);
        u.attempts = 1;
        assert_eq!(u.backoff_ms(10, 300), 10_000);
        u.attempts = 3;
        assert_eq!(u.backoff_ms(10, 300), 40_000);
        u.attempts = 30;
        assert_eq!(u.backoff_ms(10, 300), 300_000);
        u.server_idx = 3;
        assert_eq!(u.server().unwrap().host, "b");
        assert!(u.ctcp_reply_allowed("Bob", "version", 0));
        assert!(!u.ctcp_reply_allowed("bob", "VERSION", 1000));
        assert!(u.ctcp_reply_allowed("bob", "VERSION", 6000));
    }
}
