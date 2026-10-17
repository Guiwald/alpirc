//! Aiguillage des réponses : quand un appareil pose une question au réseau (`WHOIS`, `WHO`, `LIST`, `MODE #canal
//! +b`…), seul cet appareil doit en voir la réponse, pas les autres appareils attachés au même réseau.
//!
//! Le relais retient, par réseau, les questions en attente (appareil, sorte, cible) dans l'ordre d'envoi. Une
//! réponse numérique est rendue à l'appareil de la plus ancienne question de sa sorte (et de même cible quand
//! des réponses de cette sorte peuvent aussi venir toutes seules, comme `353` après un `JOIN`) ; sa réponse de
//! fin retire la question. Sans question en attente, la réponse va à tous. Une question sans réponse est
//! oubliée au bout de [`PENDING_TTL_MS`].

use std::collections::VecDeque;

use alpirc_proto::{CaseMapping, Message};

/// Durée de vie d'une question sans réponse.
pub const PENDING_TTL_MS: i64 = 60_000;

/// Une sorte de question et ses réponses.
struct Family {
    kind: &'static str,
    replies: &'static [&'static str],
    ends: &'static [&'static str],
    /// Indice du paramètre qui porte la cible dans les réponses, quand il faut la comparer.
    target_param: Option<fn(&str) -> usize>,
}

fn names_target(numeric: &str) -> usize {
    if numeric == "353" { 2 } else { 1 }
}

fn second(_: &str) -> usize {
    1
}

const FAMILIES: &[Family] = &[
    Family {
        kind: "WHOIS",
        replies: &[
            "276", "301", "307", "310", "311", "312", "313", "317", "318", "319", "320", "330", "335", "338", "378",
            "379", "401", "402", "431", "671",
        ],
        ends: &["318"],
        target_param: None,
    },
    Family { kind: "WHOWAS", replies: &["312", "314", "369", "406"], ends: &["369"], target_param: None },
    Family { kind: "WHO", replies: &["352", "354", "315"], ends: &["315"], target_param: None },
    Family { kind: "NAMES", replies: &["353", "366", "403"], ends: &["366", "403"], target_param: Some(names_target) },
    Family { kind: "LIST", replies: &["321", "322", "323"], ends: &["323"], target_param: None },
    Family {
        kind: "MODE",
        replies: &["324", "329", "403", "442"],
        ends: &["329", "403", "442"],
        target_param: Some(second),
    },
    Family { kind: "MODE b", replies: &["367", "368", "482"], ends: &["368", "482"], target_param: Some(second) },
    Family { kind: "MODE e", replies: &["348", "349", "482"], ends: &["349", "482"], target_param: Some(second) },
    Family { kind: "MODE I", replies: &["346", "347", "482"], ends: &["347", "482"], target_param: Some(second) },
    Family { kind: "UMODE", replies: &["221"], ends: &["221"], target_param: None },
    Family {
        kind: "TOPIC",
        replies: &["331", "332", "333", "403", "442"],
        ends: &["331", "333", "403", "442"],
        target_param: Some(second),
    },
    Family { kind: "USERHOST", replies: &["302"], ends: &["302"], target_param: None },
    Family { kind: "ISON", replies: &["303"], ends: &["303"], target_param: None },
    Family { kind: "MOTD", replies: &["375", "372", "376", "422"], ends: &["376", "422"], target_param: None },
    Family {
        kind: "LUSERS",
        replies: &["251", "252", "253", "254", "255", "265", "266"],
        ends: &["266"],
        target_param: None,
    },
    Family { kind: "VERSION", replies: &["351"], ends: &["351"], target_param: None },
    Family { kind: "TIME", replies: &["391"], ends: &["391"], target_param: None },
    Family { kind: "ADMIN", replies: &["256", "257", "258", "259"], ends: &["259"], target_param: None },
    Family { kind: "INFO", replies: &["371", "374"], ends: &["374"], target_param: None },
    Family { kind: "LINKS", replies: &["364", "365"], ends: &["365"], target_param: None },
    Family { kind: "MONITOR L", replies: &["732", "733"], ends: &["733"], target_param: None },
    Family {
        kind: "NICK",
        replies: &["431", "432", "433", "436", "437", "484"],
        ends: &["431", "432", "433", "436", "437", "484"],
        target_param: None,
    },
    Family { kind: "INVITE", replies: &["341", "443"], ends: &["341", "443"], target_param: None },
];

fn family(kind: &str) -> Option<&'static Family> {
    FAMILIES.iter().find(|f| f.kind == kind)
}

/// Sorte de question d'une commande d'un appareil, et sa cible (repliée), si elle attend une réponse à aiguiller.
pub fn classify(msg: &Message, cm: CaseMapping) -> Option<(&'static str, Option<String>)> {
    let p = |i: usize| msg.params.get(i).map(String::as_str);
    let fold = |s: &str| cm.fold(s);
    let kind: &'static str = match msg.command.as_str() {
        "WHOIS" => "WHOIS",
        "WHOWAS" => "WHOWAS",
        "WHO" => "WHO",
        "NAMES" => {
            let t = p(0)?;
            return Some(("NAMES", Some(fold(t.split(',').next().unwrap_or(t)))));
        }
        "LIST" => "LIST",
        "MODE" => {
            let target = p(0)?;
            let is_chan = target.starts_with(['#', '&', '!', '+']);
            return match (is_chan, msg.params.len()) {
                (true, 1) => Some(("MODE", Some(fold(target)))),
                (true, 2) => {
                    let m = p(1)?.trim_start_matches('+');
                    let kind = match m {
                        "b" => "MODE b",
                        "e" => "MODE e",
                        "I" => "MODE I",
                        _ => return None,
                    };
                    Some((kind, Some(fold(target))))
                }
                (false, 1) => Some(("UMODE", None)),
                _ => None,
            };
        }
        "TOPIC" if msg.params.len() == 1 => return Some(("TOPIC", Some(fold(p(0)?)))),
        "USERHOST" => "USERHOST",
        "ISON" => "ISON",
        "MOTD" => "MOTD",
        "LUSERS" => "LUSERS",
        "VERSION" => "VERSION",
        "TIME" => "TIME",
        "ADMIN" => "ADMIN",
        "INFO" => "INFO",
        "LINKS" => "LINKS",
        "MONITOR" if p(0).is_some_and(|s| s.eq_ignore_ascii_case("L")) => "MONITOR L",
        "NICK" => "NICK",
        "INVITE" if msg.params.len() >= 2 => "INVITE",
        _ => return None,
    };
    Some((kind, None))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingCmd {
    client: u32,
    kind: &'static str,
    target: Option<String>,
    at: i64,
}

/// Où envoyer une réponse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// À l'appareil qui a posé la question.
    To(u32),
    /// À tous les appareils du réseau.
    All,
}

/// Les questions en attente d'un réseau.
#[derive(Debug, Default)]
pub struct Pending {
    list: VecDeque<PendingCmd>,
    /// Dernière question terminée par 324 ou 332 : 329 ou 333 qui suivent vont au même appareil.
    trailing: Option<(&'static str, String, u32, i64)>,
}

impl Pending {
    /// Retient la question d'un appareil.
    pub fn push(&mut self, client: u32, kind: &'static str, target: Option<String>, now: i64) {
        self.expire(now);
        self.list.push_back(PendingCmd { client, kind, target, at: now });
    }

    fn expire(&mut self, now: i64) {
        self.list.retain(|p| now - p.at < PENDING_TTL_MS);
    }

    /// Oublie les questions d'un appareil parti.
    pub fn forget_client(&mut self, client: u32) {
        self.list.retain(|p| p.client != client);
    }

    pub fn clear(&mut self) {
        self.list.clear();
        self.trailing = None;
    }

    /// Questions en attente (essais).
    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Le `NICK` d'un appareil a abouti : rend cet appareil.
    pub fn nick_done(&mut self) -> Option<u32> {
        let i = self.list.iter().position(|p| p.kind == "NICK")?;
        self.list.remove(i).map(|p| p.client)
    }

    /// Aiguille une réponse numérique.
    pub fn route(&mut self, msg: &Message, cm: CaseMapping, now: i64) -> Route {
        self.expire(now);
        let numeric = msg.command.as_str();
        // 329 après 324, 333 après 332 : même appareil.
        if let Some((kind, target, client, at)) = self.trailing.clone()
            && now - at < 5_000
            && ((kind == "MODE" && numeric == "329") || (kind == "TOPIC" && numeric == "333"))
            && cm.fold(msg.param(1)) == target
        {
            self.trailing = None;
            return Route::To(client);
        }
        for (i, p) in self.list.iter().enumerate() {
            let Some(f) = family(p.kind) else { continue };
            if !f.replies.contains(&numeric) {
                continue;
            }
            if let (Some(tp), Some(want)) = (f.target_param, p.target.as_ref()) {
                let got = cm.fold(msg.param(tp(numeric)));
                if got != *want {
                    continue;
                }
            }
            let client = p.client;
            let ends = f.ends.contains(&numeric)
                || (p.kind == "MODE" && numeric == "324")
                || (p.kind == "TOPIC" && numeric == "332");
            if ends {
                let p = self.list.remove(i).expect("présent");
                if numeric == "324" || numeric == "332" {
                    self.trailing = Some((p.kind, p.target.unwrap_or_default(), client, now));
                }
            }
            return Route::To(client);
        }
        Route::All
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(s: &str) -> Message {
        Message::parse(s).unwrap()
    }

    const CM: CaseMapping = CaseMapping::Rfc1459;

    #[test]
    fn classify_commands() {
        assert_eq!(classify(&m("WHOIS Bob"), CM), Some(("WHOIS", None)));
        assert_eq!(classify(&m("NAMES #Lyon"), CM), Some(("NAMES", Some("#lyon".into()))));
        assert_eq!(classify(&m("MODE #Lyon"), CM), Some(("MODE", Some("#lyon".into()))));
        assert_eq!(classify(&m("MODE #Lyon +b"), CM), Some(("MODE b", Some("#lyon".into()))));
        assert_eq!(classify(&m("MODE #Lyon b"), CM), Some(("MODE b", Some("#lyon".into()))));
        assert_eq!(classify(&m("MODE #Lyon +o Bob"), CM), None);
        assert_eq!(classify(&m("MODE #Lyon +m"), CM), None);
        assert_eq!(classify(&m("MODE Gui"), CM), Some(("UMODE", None)));
        assert_eq!(classify(&m("MODE Gui +i"), CM), None);
        assert_eq!(classify(&m("TOPIC #a"), CM), Some(("TOPIC", Some("#a".into()))));
        assert_eq!(classify(&m("TOPIC #a :nouveau"), CM), None);
        assert_eq!(classify(&m("PRIVMSG #a :x"), CM), None);
        assert_eq!(classify(&m("MONITOR L"), CM), Some(("MONITOR L", None)));
        assert_eq!(classify(&m("MONITOR + Bob"), CM), None);
    }

    #[test]
    fn whois_goes_to_asker_only() {
        let mut p = Pending::default();
        p.push(7, "WHOIS", None, 0);
        p.push(8, "WHOIS", None, 0);
        assert_eq!(p.route(&m(":s 311 me Bob b h * :Bob"), CM, 1), Route::To(7));
        assert_eq!(p.route(&m(":s 318 me Bob :End"), CM, 1), Route::To(7));
        assert_eq!(p.route(&m(":s 311 me Al a h * :Al"), CM, 1), Route::To(8));
        assert_eq!(p.route(&m(":s 318 me Al :End"), CM, 1), Route::To(8));
        assert!(p.is_empty());
        assert_eq!(p.route(&m(":s 311 me X x h * :X"), CM, 1), Route::All);
    }

    #[test]
    fn names_after_join_are_not_captured() {
        let mut p = Pending::default();
        p.push(7, "NAMES", Some("#a".into()), 0);
        // NAMES d'un canal que l'on vient de rejoindre : pour tous.
        assert_eq!(p.route(&m(":s 353 me = #b :x y"), CM, 1), Route::All);
        assert_eq!(p.route(&m(":s 366 me #b :End"), CM, 1), Route::All);
        assert_eq!(p.route(&m(":s 353 me = #A :x y"), CM, 1), Route::To(7));
        assert_eq!(p.route(&m(":s 366 me #a :End"), CM, 1), Route::To(7));
        assert!(p.is_empty());
    }

    #[test]
    fn mode_and_topic_trailers() {
        let mut p = Pending::default();
        p.push(3, "MODE", Some("#a".into()), 0);
        assert_eq!(p.route(&m(":s 324 me #a +nt"), CM, 1), Route::To(3));
        assert!(p.is_empty());
        assert_eq!(p.route(&m(":s 329 me #a 1700000000"), CM, 2), Route::To(3));
        assert_eq!(p.route(&m(":s 329 me #a 1700000000"), CM, 3), Route::All);
        p.push(4, "TOPIC", Some("#a".into()), 10);
        assert_eq!(p.route(&m(":s 332 me #a :sujet"), CM, 11), Route::To(4));
        assert_eq!(p.route(&m(":s 333 me #a x 1"), CM, 12), Route::To(4));
        p.push(4, "TOPIC", Some("#b".into()), 10);
        assert_eq!(p.route(&m(":s 331 me #b :No topic"), CM, 11), Route::To(4));
        assert!(p.is_empty());
    }

    #[test]
    fn expiry_and_forget() {
        let mut p = Pending::default();
        p.push(1, "LIST", None, 0);
        assert_eq!(p.route(&m(":s 322 me #a 3 :x"), CM, PENDING_TTL_MS + 1), Route::All);
        p.push(1, "LIST", None, 0);
        p.push(2, "WHO", None, 0);
        p.forget_client(1);
        assert_eq!(p.len(), 1);
        p.push(5, "NICK", None, 0);
        assert_eq!(p.nick_done(), Some(5));
        assert_eq!(p.nick_done(), None);
        p.clear();
        assert!(p.is_empty());
    }
}
