//! Fiches whois gardées pour la boîte User Central (`/uwho`, entrée « Infos » des menus des pseudos), comme dans
//! mIRC : les réponses du serveur à un WHOIS remplissent une fiche par pseudo (adresse, nom, serveur, canaux,
//! compte, inactivité, absence…). Une recherche lancée par `/uwho` est « discrète » : ses réponses ne
//! s'affichent dans aucune fenêtre, la boîte les montre. Un `/whois` ordinaire s'affiche comme avant, et met aussi
//! la fiche à jour.

use alpirc_proto::{Message, numeric as n};

use super::state::State;
use super::{ConnId, Effect, UiDialog};

/// Nombre de fiches gardées par connexion (les plus anciennes partent).
const KEEP: usize = 32;

/// Où en est la recherche d'une fiche.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WhoisState {
    /// Demandée, réponses en cours.
    #[default]
    Pending,
    /// Fin de la réponse (318) reçue.
    Done,
    /// Le serveur ne connaît pas ce pseudo (401).
    NoSuchNick,
}

/// La fiche whois d'un utilisateur.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WhoisInfo {
    /// Pseudo tel que le serveur l'écrit.
    pub nick: String,
    pub user: String,
    pub host: String,
    pub realname: String,
    pub server: String,
    pub server_info: String,
    /// Canaux, avec leur préfixe de rang (`@#lyon +#grenoble`).
    pub channels: String,
    pub account: Option<String>,
    /// Inactivité (secondes) et heure de connexion (secondes depuis 1970).
    pub idle_secs: Option<u64>,
    pub signon: Option<i64>,
    /// Message d'absence.
    pub away: Option<String>,
    /// Opérateur IRC (texte du serveur).
    pub operator: Option<String>,
    /// Connexion chiffrée (671).
    pub secure: bool,
    /// Autres lignes du serveur (adresse réelle, pseudo enregistré…), dans l'ordre.
    pub extra: Vec<String>,
    pub state: WhoisState,
    /// Dernière mise à jour (ms depuis 1970).
    pub updated_ms: i64,
    /// Recherche lancée par `/uwho` : rien ne s'affiche dans les fenêtres.
    pub(crate) quiet: bool,
    /// Une réponse 311 est arrivée pour cette recherche (les champs de la précédente sont effacés à ce moment).
    pub(crate) fresh: bool,
}

impl WhoisInfo {
    /// `ident@hôte`, vide si on ne le connaît pas.
    pub fn address(&self) -> String {
        if self.user.is_empty() && self.host.is_empty() { String::new() } else { format!("{}@{}", self.user, self.host) }
    }

    /// Lignes de la fiche, à la manière de la réponse affichée par `/whois` (pour « Copier tout »).
    pub fn lines(&self) -> Vec<String> {
        let mut v = Vec::new();
        if !self.address().is_empty() {
            v.push(format!("{} {} * {}", self.nick, self.address(), self.realname));
        }
        if !self.channels.is_empty() {
            v.push(format!("{} {}", self.nick, self.channels));
        }
        if !self.server.is_empty() {
            v.push(format!("{} {} {}", self.nick, self.server, self.server_info).trim_end().to_string());
        }
        if let Some(a) = &self.account {
            v.push(format!("{} {a}", self.nick));
        }
        if let Some(a) = &self.away {
            v.push(format!("{} {a}", self.nick));
        }
        if let Some(o) = &self.operator {
            v.push(format!("{} {o}", self.nick));
        }
        v.extend(self.extra.iter().map(|e| format!("{} {e}", self.nick)));
        v
    }
}

impl State {
    /// `/uwho pseudo [pseudo]` : demande la fiche sans rien afficher, et ouvre la boîte User Central.
    pub(crate) fn uwho(&mut self, conn: ConnId, args: &str) {
        let nick = args.split_whitespace().last().unwrap_or("").to_string();
        let now = self.now;
        let Some(c) = self.conns.get_mut(&conn) else { return };
        let k = c.session.fold(&nick);
        let e = c.whois.entry(k).or_default();
        if e.nick.is_empty() {
            e.nick = nick.clone();
        }
        e.state = WhoisState::Pending;
        e.quiet = true;
        e.fresh = false;
        e.updated_ms = now;
        trim(&mut c.whois);
        self.send_line(conn, &format!("WHOIS {args}"));
        self.effects.push(Effect::OpenDialog(UiDialog::UserCentral { conn, nick }));
    }

    /// Une réponse numérique : met à jour la fiche du pseudo concerné. Rend vrai si elle appartient à une recherche
    /// discrète (`/uwho`) et ne doit donc pas s'afficher.
    pub(crate) fn record_whois(&mut self, conn: ConnId, msg: &Message) -> bool {
        let cmd = msg.command.as_str();
        let related = matches!(
            cmd,
            n::RPL_WHOISUSER
                | n::RPL_WHOISSERVER
                | n::RPL_WHOISOPERATOR
                | n::RPL_WHOISIDLE
                | n::RPL_ENDOFWHOIS
                | n::RPL_WHOISCHANNELS
                | n::RPL_WHOISACCOUNT
                | n::RPL_WHOISSECURE
                | n::RPL_WHOISREGNICK
                | n::RPL_WHOISSPECIAL
                | n::RPL_WHOISHOST
                | n::RPL_WHOISMODES
                | n::RPL_WHOISACTUALLY
                | n::RPL_AWAY
                | n::ERR_NOSUCHNICK
                | "276"
                | "335"
                | "310"
        );
        if !related {
            return false;
        }
        let now = self.now;
        let Some(c) = self.conns.get_mut(&conn) else { return false };
        let nick = msg.param(1).to_string();
        let k = c.session.fold(&nick);
        let in_progress = c.whois.get(&k).is_some_and(|e| e.state == WhoisState::Pending);
        // 301 et 401 viennent aussi hors d'un whois (message à un absent, à un pseudo inconnu) : seulement pendant
        // une recherche. Un /whois ordinaire crée sa fiche à sa première réponse.
        if matches!(cmd, n::RPL_AWAY | n::ERR_NOSUCHNICK) && !in_progress {
            return false;
        }
        if !in_progress && cmd != n::RPL_WHOISUSER {
            // Une réponse d'un /whois dont on n'a pas vu le début : pas de fiche à compléter.
            return false;
        }
        let e = c.whois.entry(k).or_default();
        if !in_progress {
            // Début d'un /whois ordinaire.
            e.state = WhoisState::Pending;
            e.quiet = false;
            e.fresh = false;
        }
        if cmd == n::RPL_WHOISUSER && !e.fresh {
            let quiet = e.quiet;
            *e = WhoisInfo { quiet, fresh: true, ..WhoisInfo::default() };
        }
        e.updated_ms = now;
        if !nick.is_empty() && cmd != n::ERR_NOSUCHNICK {
            e.nick = nick;
        }
        let trailing = msg.trailing().to_string();
        let rest = || msg.params.iter().skip(2).cloned().collect::<Vec<_>>().join(" ");
        match cmd {
            n::RPL_WHOISUSER => {
                e.user = msg.param(2).to_string();
                e.host = msg.param(3).to_string();
                e.realname = trailing;
            }
            n::RPL_WHOISSERVER => {
                e.server = msg.param(2).to_string();
                e.server_info = trailing;
            }
            n::RPL_WHOISCHANNELS => {
                let add = trailing.trim();
                if e.channels.is_empty() {
                    e.channels = add.to_string();
                } else if !add.is_empty() {
                    e.channels = format!("{} {add}", e.channels);
                }
            }
            n::RPL_WHOISIDLE => {
                e.idle_secs = msg.param(2).parse().ok();
                e.signon = msg.param(3).parse().ok().filter(|&s: &i64| s > 0);
            }
            n::RPL_WHOISACCOUNT => e.account = Some(msg.param(2).to_string()),
            n::RPL_WHOISSECURE => e.secure = true,
            n::RPL_WHOISOPERATOR => e.operator = Some(trailing),
            n::RPL_AWAY => e.away = Some(trailing),
            n::ERR_NOSUCHNICK => {
                e.state = WhoisState::NoSuchNick;
            }
            n::RPL_ENDOFWHOIS => {
                if e.state == WhoisState::Pending {
                    e.state = if e.fresh { WhoisState::Done } else { WhoisState::NoSuchNick };
                }
            }
            _ => {
                let r = rest();
                if !r.is_empty() && !e.extra.contains(&r) {
                    e.extra.push(r);
                }
            }
        }
        let quiet = e.quiet;
        if cmd == n::RPL_ENDOFWHOIS {
            e.quiet = false;
        }
        trim(&mut c.whois);
        quiet
    }
}

/// Garde les [`KEEP`] fiches les plus récentes.
fn trim(map: &mut std::collections::HashMap<String, WhoisInfo>) {
    while map.len() > KEEP {
        let Some(old) = map.iter().min_by_key(|(_, e)| e.updated_ms).map(|(k, _)| k.clone()) else { break };
        map.remove(&old);
    }
}
