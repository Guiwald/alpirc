//! État d'une connexion : canaux (sujet, modes, membres, listes), carnet des adresses connues (IAL).

use std::collections::BTreeMap;

use alpirc_proto::ISupport;

/// Un membre d'un canal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Member {
    /// Pseudo, avec sa casse.
    pub nick: String,
    /// Modes de préfixe (`o`, `v`…), du plus fort au plus faible selon `PREFIX`.
    pub modes: String,
    /// Dernière fois qu'il a parlé dans le canal (ms ; 0 = jamais), pour la complétion Tab.
    pub last_spoke: i64,
}

/// Une entrée de liste de canal (ban, exception, invitation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEntry {
    pub mask: String,
    pub setter: Option<String>,
    /// Secondes depuis 1970.
    pub time: Option<i64>,
}

/// L'état d'un canal tel que le serveur nous l'a décrit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChannelState {
    pub name: String,
    pub topic: Option<String>,
    pub topic_setter: Option<String>,
    /// Secondes depuis 1970.
    pub topic_time: Option<i64>,
    /// Modes sans paramètre et avec paramètre (lettres seules, dans l'ordre où ils ont été posés).
    pub modes: String,
    /// Paramètres des modes (`k` → clé, `l` → limite…).
    pub mode_params: BTreeMap<char, String>,
    /// Date de création (secondes), si le serveur l'a dite (329).
    pub created: Option<i64>,
    /// Membres, par pseudo replié (CASEMAPPING).
    pub members: BTreeMap<String, Member>,
    pub bans: Vec<ListEntry>,
    pub excepts: Vec<ListEntry>,
    pub invites: Vec<ListEntry>,
    /// Vrai tant que nous sommes sur le canal.
    pub joined: bool,
    /// La première réponse NAMES (à l'arrivée) est complète.
    pub synced: bool,
    /// Une réponse NAMES est en cours de réception.
    pub(crate) names_in_progress: bool,
    /// Listes (`b`, `e`, `I`) en cours de réception.
    pub(crate) lists_in_progress: Vec<char>,
}

impl ChannelState {
    pub fn new(name: &str) -> ChannelState {
        ChannelState { name: name.to_string(), ..ChannelState::default() }
    }

    pub fn key(&self) -> Option<&str> {
        self.mode_params.get(&'k').map(String::as_str)
    }

    pub fn limit(&self) -> Option<u32> {
        self.mode_params.get(&'l').and_then(|v| v.parse().ok())
    }

    pub fn has_mode(&self, m: char) -> bool {
        self.modes.contains(m)
    }

    /// `+ntk` ; avec `params`, `+ntkl clé 50` (les paramètres dans l'ordre des lettres).
    pub fn mode_string(&self, params: bool) -> String {
        if self.modes.is_empty() {
            return String::new();
        }
        let mut s = format!("+{}", self.modes);
        if params {
            for m in self.modes.chars() {
                if let Some(p) = self.mode_params.get(&m) {
                    s.push(' ');
                    s.push_str(p);
                }
            }
        }
        s
    }

    pub(crate) fn set_mode(&mut self, m: char, param: Option<String>) {
        if !self.modes.contains(m) {
            self.modes.push(m);
        }
        match param {
            Some(p) => {
                self.mode_params.insert(m, p);
            }
            None => {
                self.mode_params.remove(&m);
            }
        }
    }

    pub(crate) fn unset_mode(&mut self, m: char) {
        self.modes.retain(|c| c != m);
        self.mode_params.remove(&m);
    }

    /// Liste d'un mode de type A (`b`, l'exception, l'invitation), selon ISUPPORT.
    pub fn list(&self, mode: char, is: &ISupport) -> Option<&Vec<ListEntry>> {
        if mode == 'b' {
            Some(&self.bans)
        } else if mode == is.excepts.unwrap_or('e') {
            Some(&self.excepts)
        } else if mode == is.invex.unwrap_or('I') {
            Some(&self.invites)
        } else {
            None
        }
    }

    pub(crate) fn list_mut(&mut self, mode: char, is: &ISupport) -> Option<&mut Vec<ListEntry>> {
        if mode == 'b' {
            Some(&mut self.bans)
        } else if mode == is.excepts.unwrap_or('e') {
            Some(&mut self.excepts)
        } else if mode == is.invex.unwrap_or('I') {
            Some(&mut self.invites)
        } else {
            None
        }
    }
}

/// Ce qu'on sait d'un utilisateur (Internal Address List de mIRC).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserInfo {
    pub nick: String,
    pub user: Option<String>,
    pub host: Option<String>,
    /// Compte de services (account-notify, extended-join, WHOX, 330).
    pub account: Option<String>,
    pub away: bool,
    pub away_message: Option<String>,
    pub realname: Option<String>,
}

impl UserInfo {
    /// `ident@hôte`, si connus.
    pub fn address(&self) -> Option<String> {
        match (&self.user, &self.host) {
            (Some(u), Some(h)) => Some(format!("{u}@{h}")),
            _ => None,
        }
    }

    /// `pseudo!ident@hôte` (avec `*` pour ce qui manque).
    pub fn full_mask(&self) -> String {
        format!(
            "{}!{}@{}",
            self.nick,
            self.user.as_deref().unwrap_or("*"),
            self.host.as_deref().unwrap_or("*")
        )
    }
}

/// Insère un mode de préfixe dans une chaîne de modes, au rang voulu (`PREFIX`).
pub(crate) fn add_prefix_mode(modes: &mut String, m: char, is: &ISupport) {
    if modes.contains(m) {
        return;
    }
    let mut v: Vec<char> = modes.chars().collect();
    v.push(m);
    v.sort_by_key(|c| is.rank(*c).unwrap_or(usize::MAX));
    *modes = v.into_iter().collect();
}
