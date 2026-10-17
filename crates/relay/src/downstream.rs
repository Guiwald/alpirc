//! Une connexion d'appareil (client IRC) au relais : enregistrement, capacités négociées, et mise en forme des
//! lignes selon ce que l'appareil a négocié ([`Downstream::render`]) : étiquettes retirées, `JOIN` étendu
//! ramené à la forme simple, préfixes multiples et adresses retirés des `NAMES`, commandes inconnues de lui
//! écartées.

use std::collections::BTreeSet;
use std::net::SocketAddr;

use alpirc_proto::{ISupport, Message, Tags};

use crate::upstream::Bucket;

/// Capacités offertes aux appareils (`CAP LS 302`).
pub const OFFERED_CAPS: &[&str] = &[
    "account-notify",
    "account-tag",
    "away-notify",
    "batch",
    "cap-notify",
    "chghost",
    "draft/chathistory",
    "draft/read-marker",
    "echo-message",
    "extended-join",
    "invite-notify",
    "message-tags",
    "multi-prefix",
    "sasl",
    "server-time",
    "setname",
    "soju.im/bouncer-networks",
    "soju.im/bouncer-networks-notify",
    "userhost-in-names",
];

/// Valeur d'une capacité en `CAP LS 302`.
pub fn cap_value(cap: &str) -> Option<&'static str> {
    match cap {
        "sasl" => Some("PLAIN"),
        _ => None,
    }
}

/// Lignes acceptées d'un appareil avant la fin de son enregistrement.
pub const MAX_PREREG_LINES: u32 = 64;

/// État d'un échange SASL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaslState {
    Idle,
    /// `AUTHENTICATE PLAIN` reçu : morceaux de la réponse en cours.
    Plain(String),
}

/// Un appareil connecté.
pub struct Downstream {
    pub id: u32,
    pub peer: SocketAddr,
    pub tls: bool,
    pub connected_at: i64,
    pub registered: bool,
    /// `CAP LS`/`REQ` reçu avant l'enregistrement : il attend `CAP END`.
    pub cap_lock: bool,
    pub cap_version: u32,
    pub caps: BTreeSet<String>,
    /// Pseudo connu de l'appareil (celui de la connexion au réseau une fois lié).
    pub nick: String,
    pub username: Option<String>,
    pub realname: String,
    pub pass: Option<String>,
    pub sasl: SaslState,
    /// Identifiants acceptés par SASL : (utilisateur, réseau demandé, appareil).
    pub sasl_login: Option<crate::auth::Login>,
    /// Utilisateur une fois identifié.
    pub user: Option<String>,
    pub client_name: Option<String>,
    /// Réseau demandé (`BOUNCER BIND`).
    pub bind: Option<String>,
    /// Réseau lié (identifiant).
    pub net: Option<String>,
    pub prereg_lines: u32,
    /// Anti-flood à l'entrée.
    pub inbound: Bucket,
    pub dropped: u32,
    pub last_rx: i64,
    pub ping_sent: Option<i64>,
    pub closing: bool,
}

impl Downstream {
    pub fn new(id: u32, peer: SocketAddr, tls: bool, now: i64) -> Downstream {
        Downstream {
            id,
            peer,
            tls,
            connected_at: now,
            registered: false,
            cap_lock: false,
            cap_version: 0,
            caps: BTreeSet::new(),
            nick: "*".into(),
            username: None,
            realname: String::new(),
            pass: None,
            sasl: SaslState::Idle,
            sasl_login: None,
            user: None,
            client_name: None,
            bind: None,
            net: None,
            prereg_lines: 0,
            inbound: Bucket::new(INBOUND_BURST, INBOUND_INTERVAL_MS, now),
            dropped: 0,
            last_rx: now,
            ping_sent: None,
            closing: false,
        }
    }

    pub fn cap(&self, name: &str) -> bool {
        self.caps.contains(name)
    }

    /// Lié à ce réseau de cet utilisateur.
    pub fn bound_to(&self, user: &str, net: &str) -> bool {
        self.registered && self.user.as_deref() == Some(user) && self.net.as_deref() == Some(net)
    }

    /// La ligne telle que cet appareil doit la recevoir, ou `None` s'il ne doit pas la voir.
    pub fn render(&self, msg: &Message, is: Option<&ISupport>, own_nick: &str) -> Option<String> {
        let cmd = msg.command.as_str();
        let needs = |cap: &str| self.cap(cap);
        let allowed = match cmd {
            "AWAY" => needs("away-notify"),
            "ACCOUNT" => needs("account-notify"),
            "CHGHOST" => needs("chghost"),
            "SETNAME" => needs("setname"),
            "TAGMSG" => needs("message-tags"),
            "BATCH" => needs("batch"),
            "MARKREAD" => needs("draft/read-marker"),
            "INVITE" => {
                needs("invite-notify") || is.map(|i| i.casemapping).unwrap_or_default().eq(msg.param(0), own_nick)
            }
            _ => true,
        };
        if !allowed {
            return None;
        }
        let mut out = msg.clone();
        match cmd {
            "JOIN" if out.params.len() >= 3 && !needs("extended-join") => out.params.truncate(1),
            "353" if !needs("multi-prefix") || !needs("userhost-in-names") => {
                if let Some(last) = out.params.last_mut() {
                    *last = filter_names(last, is, needs("multi-prefix"), needs("userhost-in-names"));
                }
            }
            "352" if !needs("multi-prefix") && out.params.len() > 6 => {
                out.params[6] = filter_who_flags(&out.params[6], is);
            }
            _ => {}
        }
        out.tags = self.filter_tags(&msg.tags);
        Some(out.to_line())
    }

    /// Étiquettes que l'appareil a négociées.
    pub fn filter_tags(&self, tags: &Tags) -> Tags {
        let mt = self.cap("message-tags");
        Tags(
            tags.0
                .iter()
                .filter(|(k, _)| match k.as_str() {
                    "time" => self.cap("server-time"),
                    "batch" => self.cap("batch"),
                    "account" => self.cap("account-tag"),
                    "label" => false,
                    "draft/chathistory-end" => self.cap("draft/chathistory"),
                    _ => mt,
                })
                .cloned()
                .collect(),
        )
    }
}

/// Anti-flood à l'entrée : 500 lignes d'un coup (un appareil qui se lie envoie plusieurs lignes par canal),
/// puis 25 par seconde.
pub const INBOUND_BURST: u32 = 500;
pub const INBOUND_INTERVAL_MS: u64 = 40;

/// Retire d'une liste `NAMES` les préfixes en trop (`multi-prefix`) et les adresses (`userhost-in-names`).
pub fn filter_names(list: &str, is: Option<&ISupport>, multi: bool, uhnames: bool) -> String {
    let default = ISupport::default();
    let is = is.unwrap_or(&default);
    list.split(' ')
        .filter(|n| !n.is_empty())
        .map(|n| {
            let (modes, rest) = is.split_prefixes(n);
            let mut prefix = String::new();
            for (i, m) in modes.chars().enumerate() {
                if i > 0 && !multi {
                    break;
                }
                if let Some(s) = is.symbol_for_mode(m) {
                    prefix.push(s);
                }
            }
            let nick = if uhnames { rest } else { rest.split('!').next().unwrap_or(rest) };
            format!("{prefix}{nick}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Drapeaux d'une réponse WHO (`H@+`) ramenés au préfixe le plus fort.
pub fn filter_who_flags(flags: &str, is: Option<&ISupport>) -> String {
    let default = ISupport::default();
    let is = is.unwrap_or(&default);
    let mut out = String::new();
    let mut seen_prefix = false;
    for c in flags.chars() {
        if is.mode_for_symbol(c).is_some() {
            if seen_prefix {
                continue;
            }
            seen_prefix = true;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(caps: &[&str]) -> Downstream {
        let mut d = Downstream::new(1_000_000, "127.0.0.1:1".parse().unwrap(), true, 0);
        d.caps = caps.iter().map(|s| s.to_string()).collect();
        d
    }

    fn m(s: &str) -> Message {
        Message::parse(s).unwrap()
    }

    #[test]
    fn tags_by_caps() {
        let line =
            m("@time=2026-10-17T10:00:00.000Z;msgid=abc;+draft/reply=x;account=gui;label=1 :a!b@c PRIVMSG #x :hi");
        assert_eq!(d(&[]).render(&line, None, "me").unwrap(), ":a!b@c PRIVMSG #x hi");
        assert_eq!(
            d(&["server-time"]).render(&line, None, "me").unwrap(),
            "@time=2026-10-17T10:00:00.000Z :a!b@c PRIVMSG #x hi"
        );
        assert_eq!(
            d(&["message-tags"]).render(&line, None, "me").unwrap(),
            "@msgid=abc;+draft/reply=x :a!b@c PRIVMSG #x hi"
        );
        assert_eq!(d(&["account-tag"]).render(&line, None, "me").unwrap(), "@account=gui :a!b@c PRIVMSG #x hi");
    }

    #[test]
    fn commands_by_caps() {
        let away = m(":a!b@c AWAY :parti");
        assert!(d(&[]).render(&away, None, "me").is_none());
        assert!(d(&["away-notify"]).render(&away, None, "me").is_some());
        let tag = m("@+typing=active :a!b@c TAGMSG #x");
        assert!(d(&["server-time"]).render(&tag, None, "me").is_none());
        assert_eq!(d(&["message-tags"]).render(&tag, None, "me").unwrap(), "@+typing=active :a!b@c TAGMSG #x");
        let inv = m(":a!b@c INVITE Other #x");
        assert!(d(&[]).render(&inv, None, "me").is_none());
        assert!(d(&["invite-notify"]).render(&inv, None, "me").is_some());
        let inv_me = m(":a!b@c INVITE Me #x");
        assert!(d(&[]).render(&inv_me, None, "me").is_some());
        assert!(d(&[]).render(&m(":r BATCH +1 netsplit a b"), None, "me").is_none());
        assert!(d(&[]).render(&m(":r MARKREAD #a *"), None, "me").is_none());
    }

    #[test]
    fn extended_join() {
        let j = m(":a!b@c JOIN #x gui :Guillaume");
        assert_eq!(d(&[]).render(&j, None, "me").unwrap(), ":a!b@c JOIN #x");
        assert_eq!(d(&["extended-join"]).render(&j, None, "me").unwrap(), ":a!b@c JOIN #x gui Guillaume");
    }

    #[test]
    fn names_and_who() {
        let mut is = ISupport::default();
        is.apply(&["PREFIX=(ohv)@%+".to_string()]);
        let n = m(":s 353 me = #x :@+a!u@h %b c!u2@h2");
        assert_eq!(d(&[]).render(&n, Some(&is), "me").unwrap(), ":s 353 me = #x :@a %b c");
        assert_eq!(d(&["multi-prefix"]).render(&n, Some(&is), "me").unwrap(), ":s 353 me = #x :@+a %b c");
        assert_eq!(d(&["userhost-in-names"]).render(&n, Some(&is), "me").unwrap(), ":s 353 me = #x :@a!u@h %b c!u2@h2");
        assert_eq!(
            d(&["userhost-in-names", "multi-prefix"]).render(&n, Some(&is), "me").unwrap(),
            ":s 353 me = #x :@+a!u@h %b c!u2@h2"
        );
        let w = m(":s 352 me #x u h s a H@+ :0 Real");
        assert_eq!(d(&[]).render(&w, Some(&is), "me").unwrap(), ":s 352 me #x u h s a H@ :0 Real");
        assert_eq!(d(&["multi-prefix"]).render(&w, Some(&is), "me").unwrap(), ":s 352 me #x u h s a H@+ :0 Real");
        assert_eq!(filter_who_flags("G*%+", Some(&is)), "G*%");
    }
}
