//! Le DCC dans le client (ADR 0004) : envois et réceptions de fichiers (SEND/GET, reprise par RESUME/ACCEPT,
//! DCC passif) et discussions directes (CHAT, fenêtres `=pseudo`).
//!
//! Le moteur tient l'état des transferts ([`Transfer`], vus par [`Client::transfers`]) et parle le protocole
//! (CTCP `DCC`, voir [`crate::dcc`]) ; l'hôte ouvre les connexions directes ([`Effect::DccListen`],
//! [`Effect::DccConnect`]…) et rend compte par les `Client::on_dcc_*`.
//!
//! Bac à sable (ADR 0007) : un script ne peut ni accepter un DCC (`/dcc get`, `/dcc chat` en réponse à une
//! offre) ni envoyer un fichier qu'il nomme lui-même (`/dcc send pseudo fichier`) ; seules les commandes tapées,
//! les menus et les boutons le peuvent. L'acceptation automatique (réglages) ne vaut jamais pour un programme
//! ou un script ([`crate::dcc::is_dangerous`]).
//!
//! Événements de script : `CHAT` (texte reçu), `OPEN` (fenêtre `=pseudo` ouverte), `FILESENT`, `FILERCVD`,
//! `SENDFAIL`, `GETFAIL` (`$nick`, `$filename`).

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use alpirc_proto::ctcp;
use alpirc_proto::encoding::Encoding;
use alpirc_script::EventKind;

use super::state::{Out, PendingLine, Pres, State};
use super::util::{split_word, switches};
use super::window::{Activity, LineKind, WinKind};
use super::{Client, ConnId, Effect, WinId};
use crate::dcc::{self as proto, DccIo, DccKind, DccOffer};
use crate::i18n::{Key, Lang, fill};
use crate::settings::{DccChatAction, DccExists, DccSendAction};

/// Numéro d'un transfert ou d'une discussion DCC (identifiant des connexions directes chez l'hôte).
pub type DccId = u32;

/// Sorte d'entrée DCC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DccType {
    /// Nous envoyons un fichier.
    Send,
    /// Nous recevons un fichier.
    Get,
    /// Discussion directe.
    Chat,
}

/// État d'un transfert ou d'une discussion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransferState {
    /// Offre reçue : en attente de la décision de l'utilisateur.
    Offered,
    /// En attente de l'autre : réponse à notre offre, acceptation de la reprise, ou connexion à notre écoute.
    Waiting,
    /// Connexion directe en cours d'ouverture.
    Connecting,
    /// Connecté : le fichier passe (ou la discussion est ouverte).
    Active,
    Done,
    Failed,
    /// Annulé, refusé ou ignoré.
    Cancelled,
}

impl TransferState {
    pub fn finished(self) -> bool {
        matches!(self, TransferState::Done | TransferState::Failed | TransferState::Cancelled)
    }
}

/// Un transfert (ou une discussion), pour l'interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    pub id: DccId,
    /// Connexion IRC par laquelle l'offre est passée.
    pub conn: ConnId,
    pub kind: DccType,
    /// Offre reçue (et non faite par nous).
    pub incoming: bool,
    pub nick: String,
    /// Nom affiché (pour une réception : le nom assaini).
    pub file: String,
    /// Fichier local (envoyé, ou où l'on reçoit).
    pub path: Option<PathBuf>,
    pub size: Option<u64>,
    /// Position de départ (reprise).
    pub start: u64,
    /// Position atteinte (octets confirmés ou reçus, départ compris).
    pub pos: u64,
    /// Débit moyen depuis la connexion (octets par seconde).
    pub speed: u64,
    pub state: TransferState,
    /// DCC passif (c'est le destinataire qui écoute).
    pub passive: bool,
    pub error: Option<String>,
    pub created_ms: i64,
    pub started_ms: Option<i64>,
    pub ended_ms: Option<i64>,
    /// Programme ou script : jamais accepté tout seul.
    pub dangerous: bool,
    /// Discussion : sa fenêtre `=pseudo`.
    pub win: Option<WinId>,
}

impl Transfer {
    /// Avancement en pour cent, si la taille est connue.
    pub fn percent(&self) -> Option<u8> {
        let s = self.size?;
        if s == 0 {
            return Some(if self.state == TransferState::Done { 100 } else { 0 });
        }
        Some(((self.pos.min(s) as u128 * 100) / s as u128) as u8)
    }

    /// Temps restant estimé (secondes).
    pub fn eta_secs(&self) -> Option<u64> {
        let s = self.size?;
        if self.speed == 0 || self.state != TransferState::Active {
            return None;
        }
        Some(s.saturating_sub(self.pos) / self.speed)
    }
}

/// Ce que l'interface demande à l'utilisateur pour une offre reçue (« Pat veut vous envoyer x.zip »).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DccPrompt {
    pub id: DccId,
    pub conn: ConnId,
    pub network: String,
    pub nick: String,
    /// Demande de discussion (sinon : envoi de fichier).
    pub chat: bool,
    pub file: String,
    pub size: Option<u64>,
    /// Un fichier de ce nom existe déjà : sa taille (reprise possible si elle est plus petite).
    pub existing: Option<u64>,
    pub dangerous: bool,
    /// Où le fichier sera écrit.
    pub path: Option<PathBuf>,
}

impl DccPrompt {
    /// Reprise possible : un fichier partiel plus petit que l'offre.
    pub fn can_resume(&self) -> bool {
        match (self.existing, self.size) {
            (Some(e), Some(s)) => e > 0 && e < s,
            (Some(e), None) => e > 0,
            _ => false,
        }
    }
}

/// Réponse de l'utilisateur à une offre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DccAnswer {
    /// Accepter (fichier existant : selon les réglages, renommé si on ne sait pas).
    Accept,
    /// Reprendre le fichier partiel.
    Resume,
    /// Écraser le fichier existant.
    Overwrite,
    /// Garder les deux (`nom (1).ext`).
    Rename,
}

/// Fenêtres DCC de l'interface que le moteur demande d'ouvrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DccUi {
    /// La boîte « Envoyer un fichier » (`/dcc send` sans pseudo).
    SendDialog,
    /// La boîte « Discussion DCC » (`/dcc chat` sans pseudo).
    ChatDialog,
    /// La fenêtre des transferts (`/dcc`).
    Transfers,
}

/// Un CTCP DCC reçu, traité après les événements de script (s'ils ne l'ont pas arrêté par `/halt`).
#[derive(Debug, Clone)]
pub(crate) struct DccIncoming {
    pub conn: ConnId,
    pub offer: DccOffer,
}

/// Une entrée DCC et ce que le protocole en retient.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub t: Transfer,
    /// Nom tel qu'il passe dans le protocole.
    wire: String,
    peer_ip: Option<IpAddr>,
    peer_port: u16,
    token: Option<String>,
    /// Port de notre écoute (offre active) : sert à reconnaître RESUME.
    our_port: u16,
    /// Reprise demandée à cette position, en attente d'ACCEPT.
    resume_at: Option<u64>,
    /// Échéance de l'attente en cours (offre, réponse).
    deadline: Option<i64>,
    /// Retirer de la liste une fois terminé (`/dcc send -c`).
    auto_clear: bool,
    /// L'hôte a une écoute ou une connexion ouverte pour cette entrée.
    open: bool,
}

/// L'état DCC du client.
#[derive(Debug, Default)]
pub(crate) struct DccState {
    pub entries: Vec<Entry>,
    pub next_id: DccId,
    /// La commande en cours vient de l'utilisateur (saisie, menu, bouton), pas d'un script ni d'un timer.
    pub user: bool,
    /// Adresse locale de chaque connexion IRC (donnée par l'hôte).
    pub local_ip: HashMap<ConnId, IpAddr>,
    /// Dossier des téléchargements du système (`Téléchargements/AlpIRC`), donné par l'hôte.
    pub downloads: Option<PathBuf>,
    /// Augmente à chaque changement (l'interface redessine la fenêtre des transferts).
    pub version: u64,
    token_seq: u32,
}

/// Textes du DCC (anglais, français ; les autres langues retombent sur l'anglais).
const TEXTS: &[(&str, &str, &str)] = &[
    ("send.offer", "* DCC Send: offering {file} ({size}) to {nick}", "* DCC Send : {file} ({size}) proposé à {nick}"),
    ("send.start", "* DCC Send: sending {file} to {nick}", "* DCC Send : envoi de {file} à {nick}"),
    ("send.resume", "* DCC Send: {nick} resumes {file} at {pos}", "* DCC Send : {nick} reprend {file} à {pos}"),
    (
        "send.done",
        "* DCC Send of {file} to {nick} completed ({size}, {speed}/s)",
        "* DCC Send : {file} envoyé à {nick} ({size}, {speed}/s)",
    ),
    ("send.failed", "* DCC Send of {file} to {nick} failed: {error}", "* DCC Send : échec de l'envoi de {file} à {nick} : {error}"),
    ("get.start", "* DCC Get: receiving {file} from {nick}", "* DCC Get : réception de {file} de {nick}"),
    ("get.resume", "* DCC Get: asking {nick} to resume {file} at {pos}", "* DCC Get : demande à {nick} de reprendre {file} à {pos}"),
    (
        "get.done",
        "* DCC Get of {file} from {nick} completed ({size}, {speed}/s): {path}",
        "* DCC Get : {file} reçu de {nick} ({size}, {speed}/s) : {path}",
    ),
    ("get.failed", "* DCC Get of {file} from {nick} failed: {error}", "* DCC Get : échec de la réception de {file} de {nick} : {error}"),
    ("get.auto", "* DCC Get: accepting {file} from {nick} automatically", "* DCC Get : {file} de {nick} accepté automatiquement"),
    (
        "get.dangerous",
        "* DCC: {file} is a program or a script: it is never accepted automatically",
        "* DCC : {file} est un programme ou un script : jamais accepté automatiquement",
    ),
    ("offer.ignored", "* DCC: offer from {nick} ignored ({file})", "* DCC : offre de {nick} ignorée ({file})"),
    ("offer.rejected", "* DCC: offer from {nick} refused ({file})", "* DCC : offre de {nick} refusée ({file})"),
    ("offer.expired", "* DCC: offer from {nick} expired ({file})", "* DCC : l'offre de {nick} a expiré ({file})"),
    ("offer.invalid", "* DCC: invalid offer from {nick}", "* DCC : offre de {nick} invalide"),
    ("chat.offer.sent", "* DCC Chat: waiting for {nick} to accept…", "* DCC Chat : en attente de l'acceptation de {nick}…"),
    ("chat.connecting", "* DCC Chat: connecting to {nick}…", "* DCC Chat : connexion à {nick}…"),
    ("chat.open", "* DCC Chat connection established with {nick}", "* DCC Chat : connexion établie avec {nick}"),
    ("chat.closed", "* DCC Chat session with {nick} closed", "* DCC Chat : discussion avec {nick} terminée"),
    ("chat.failed", "* DCC Chat with {nick} failed: {error}", "* DCC Chat : échec de la discussion avec {nick} : {error}"),
    ("chat.notconn", "* DCC Chat with {nick} is not connected", "* DCC Chat : la discussion avec {nick} n'est pas connectée"),
    ("chat.auto", "* DCC Chat: accepting {nick} automatically", "* DCC Chat : {nick} accepté automatiquement"),
    ("cancelled", "* DCC {what} of {file} with {nick} cancelled", "* DCC {what} : {file} avec {nick} annulé"),
    ("err.timeout", "timed out", "délai dépassé"),
    ("err.cancelled", "cancelled", "annulé"),
    ("err.incomplete", "incomplete ({pos} of {size})", "incomplet ({pos} sur {size})"),
    ("err.nofile", "* /dcc send: no such file: {file}", "* /dcc send : fichier introuvable : {file}"),
    ("err.script", "* /dcc {cmd}: not allowed from a script (sandbox)", "* /dcc {cmd} : interdit à un script (bac à sable)"),
    ("err.nooffer", "* /dcc {cmd}: no DCC offer from {nick}", "* /dcc {cmd} : aucune offre DCC de {nick}"),
    ("err.nomatch", "* /dcc close: no such DCC {what} with {nick}", "* /dcc close : aucun DCC {what} avec {nick}"),
    (
        "err.usage",
        "* /dcc: usage: /dcc send [-c] nick [file] | chat nick | get nick [file] | close send|get|chat nick [file] | list",
        "* /dcc : usage : /dcc send [-c] pseudo [fichier] | chat pseudo | get pseudo [fichier] | close send|get|chat pseudo [fichier] | list",
    ),
    ("err.folder", "cannot create the folder {path}: {error}", "impossible de créer le dossier {path} : {error}"),
    ("list.empty", "* No DCC transfers", "* Aucun transfert DCC"),
    ("list.head", "* DCC transfers:", "* Transferts DCC :"),
    ("st.offered", "offered", "proposé"),
    ("st.waiting", "waiting", "en attente"),
    ("st.connecting", "connecting", "connexion"),
    ("st.active", "active", "en cours"),
    ("st.done", "done", "terminé"),
    ("st.failed", "failed", "échec"),
    ("st.cancelled", "cancelled", "annulé"),
    ("what.send", "Send", "Send"),
    ("what.get", "Get", "Get"),
    ("what.chat", "Chat", "Chat"),
];

fn text(lang: Lang, key: &str) -> &'static str {
    match TEXTS.iter().find(|(k, _, _)| *k == key) {
        Some((_, en, fr)) => {
            if lang == Lang::Fr {
                fr
            } else {
                en
            }
        }
        None => "",
    }
}

/// Libellé d'un état (liste des transferts, `/dcc list`).
pub fn state_label(lang: Lang, s: TransferState) -> &'static str {
    text(
        lang,
        match s {
            TransferState::Offered => "st.offered",
            TransferState::Waiting => "st.waiting",
            TransferState::Connecting => "st.connecting",
            TransferState::Active => "st.active",
            TransferState::Done => "st.done",
            TransferState::Failed => "st.failed",
            TransferState::Cancelled => "st.cancelled",
        },
    )
}

/// Adresse publique (ni privée, ni locale, ni de bouclage) ?
fn is_public(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_broadcast()
                || v.octets()[0] == 100 && (v.octets()[1] & 0xC0) == 64)
        }
        IpAddr::V6(v) => {
            !(v.is_loopback() || v.is_unspecified() || (v.segments()[0] & 0xFE00) == 0xFC00 || (v.segments()[0] & 0xFFC0) == 0xFE80)
        }
    }
}

/// Chemin donné à `/dcc send` : `~/` développé, chemin relatif pris dans le dossier DCC.
fn expand_path(s: &str, base: &Path) -> PathBuf {
    let s = s.trim().trim_matches('"');
    if let Some(rest) = s.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    let p = PathBuf::from(s);
    if p.is_absolute() { p } else { base.join(p) }
}

/// Fichiers nommés par `/dcc send pseudo …` : le texte entier s'il existe, sinon chaque mot (ou chaque nom
/// entre guillemets).
fn parse_files(s: &str, base: &Path) -> Result<Vec<PathBuf>, String> {
    let whole = expand_path(s, base);
    if whole.is_file() {
        return Ok(vec![whole]);
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut words = Vec::new();
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            ' ' if !quoted => {
                if !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    for w in words {
        let p = expand_path(&w, base);
        if !p.is_file() {
            // Sans guillemets, c'était sans doute un seul nom à espaces : on le cite en entier.
            return Err(if s.contains('"') { w } else { s.trim().to_string() });
        }
        out.push(p);
    }
    if out.is_empty() { Err(s.to_string()) } else { Ok(out) }
}

/// Offre inutilisable : ni port ni jeton, ou port sans adresse utilisable.
fn invalid_offer(o: &DccOffer) -> bool {
    (o.port == 0 && o.token.is_none()) || (o.port != 0 && o.ip.is_none_or(|ip| ip.is_unspecified()))
}

impl Pres {
    /// Un CTCP DCC à traiter si aucun script ne l'arrête.
    pub(crate) fn dcc(mut self, d: DccIncoming) -> Pres {
        self.outs.push(Out::Dcc(Box::new(d)));
        self
    }
}

impl State {
    fn dt(&self, key: &str, args: &[(&str, &str)]) -> String {
        fill(text(self.lang, key), args)
    }

    fn entry(&self, id: DccId) -> Option<&Entry> {
        self.dcc.entries.iter().find(|e| e.t.id == id)
    }

    fn entry_mut(&mut self, id: DccId) -> Option<&mut Entry> {
        self.dcc.version += 1;
        self.dcc.entries.iter_mut().find(|e| e.t.id == id)
    }

    fn nick_eq(&self, conn: ConnId, a: &str, b: &str) -> bool {
        match self.conns.get(&conn) {
            Some(c) => c.session.eq(a, b),
            None => alpirc_proto::CaseMapping::Rfc1459.eq(a, b),
        }
    }

    fn dcc_timeout_ms(&self) -> i64 {
        self.settings.dcc.timeout_secs.max(5) as i64 * 1000
    }

    fn dcc_ports(&self) -> (u16, u16) {
        let d = &self.settings.dcc;
        if d.port_min == 0 { (0, 0) } else { (d.port_min, d.port_max.max(d.port_min)) }
    }

    fn dcc_packet(&self) -> u32 {
        self.settings.dcc.packet_size.clamp(512, 1 << 20)
    }

    /// Fenêtre où vont les messages DCC d'une connexion.
    fn dcc_info_win(&self, conn: ConnId) -> WinId {
        self.active_for(conn)
    }

    fn dcc_print(&mut self, conn: ConnId, kind: LineKind, text: String) {
        let w = self.dcc_info_win(conn);
        self.print(w, kind, text);
    }

    /// Dossier des fichiers reçus : réglage, sinon celui du système, sinon `<données>/downloads`.
    pub(crate) fn dcc_folder(&self) -> PathBuf {
        self.settings.dcc.folder.clone().or_else(|| self.dcc.downloads.clone()).unwrap_or_else(|| self.data_dir.join("downloads"))
    }

    /// Adresse annoncée dans nos offres.
    fn announce_ip(&self, conn: ConnId) -> IpAddr {
        if let Some(ip) = self.settings.dcc.own_ip.as_deref().and_then(|s| s.trim().parse::<IpAddr>().ok()) {
            return ip;
        }
        if let Some(ip) = self.conns.get(&conn).and_then(|c| c.session.host()).and_then(|h| h.parse::<IpAddr>().ok()).filter(is_public) {
            return ip;
        }
        self.dcc.local_ip.get(&conn).copied().unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
    }

    fn new_token(&mut self) -> String {
        self.dcc.token_seq = self.dcc.token_seq.wrapping_add(1);
        let base = (self.now.unsigned_abs() / 3) % 1_000_000;
        format!("{}", base * 1000 + (self.dcc.token_seq % 1000) as u64 + 1)
    }

    fn ctcp_dcc(&mut self, conn: ConnId, nick: &str, params: &str) {
        let body = ctcp::encode("DCC", params);
        self.send_line(conn, &super::commands::text_line("PRIVMSG", nick, &body));
    }

    fn new_entry(&mut self, conn: ConnId, kind: DccType, incoming: bool, nick: &str, file: &str) -> DccId {
        self.dcc.next_id += 1;
        let id = self.dcc.next_id;
        let t = Transfer {
            id,
            conn,
            kind,
            incoming,
            nick: nick.to_string(),
            file: file.to_string(),
            path: None,
            size: None,
            start: 0,
            pos: 0,
            speed: 0,
            state: TransferState::Waiting,
            passive: false,
            error: None,
            created_ms: self.now,
            started_ms: None,
            ended_ms: None,
            dangerous: false,
            win: None,
        };
        self.dcc.entries.push(Entry {
            t,
            wire: file.to_string(),
            peer_ip: None,
            peer_port: 0,
            token: None,
            our_port: 0,
            resume_at: None,
            deadline: None,
            auto_clear: false,
            open: false,
        });
        self.dcc.version += 1;
        id
    }

    fn conn_ready(&self, conn: Option<ConnId>) -> Result<ConnId, String> {
        match conn.and_then(|c| self.conns.get(&c)) {
            Some(c) if c.connected() => Ok(c.id),
            _ => Err(self.t(Key::NotConnected, &[])),
        }
    }

    // ---------------------------------------------------------------------------------------------------------
    // Commandes
    // ---------------------------------------------------------------------------------------------------------

    /// `/dcc send|chat|get|close|list …`.
    pub(crate) fn cmd_dcc(&mut self, conn: Option<ConnId>, win: WinId, args: &str) -> Result<(), String> {
        let (sub, rest) = split_word(args);
        let sub = sub.to_ascii_lowercase();
        match sub.as_str() {
            "" | "transfers" => self.effects.push(Effect::DccWindow(DccUi::Transfers)),
            "send" => {
                let (sw, rest) = switches(rest);
                let (nick, files) = split_word(rest);
                if nick.is_empty() {
                    self.effects.push(Effect::DccWindow(DccUi::SendDialog));
                    return Ok(());
                }
                let c = self.conn_ready(conn)?;
                if files.is_empty() {
                    // Le fichier est choisi par l'utilisateur dans l'interface.
                    self.effects.push(Effect::DccPickFile { conn: c, nick: nick.to_string() });
                    return Ok(());
                }
                if !self.dcc.user {
                    return Err(self.dt("err.script", &[("cmd", "send")]));
                }
                let base = self.dcc_folder();
                let paths = parse_files(files, &base).map_err(|f| self.dt("err.nofile", &[("file", &f)]))?;
                for p in paths {
                    self.dcc_offer_file(c, nick, &p, sw.contains('c'))?;
                }
            }
            "chat" => {
                let (nick, _) = split_word(rest);
                if nick.is_empty() {
                    self.effects.push(Effect::DccWindow(DccUi::ChatDialog));
                    return Ok(());
                }
                let c = conn.filter(|c| self.conns.contains_key(c)).ok_or_else(|| self.t(Key::NotConnected, &[]))?;
                self.dcc_chat_start(c, nick)?;
            }
            "get" | "accept" => {
                let (nick, file) = split_word(rest);
                if nick.is_empty() {
                    return Err(self.dt("err.usage", &[]));
                }
                if !self.dcc.user {
                    return Err(self.dt("err.script", &[("cmd", &sub)]));
                }
                let id = self
                    .find_offer(conn, nick, file, DccType::Get)
                    .ok_or_else(|| self.dt("err.nooffer", &[("cmd", &sub), ("nick", nick)]))?;
                self.dcc_accept(id, DccAnswer::Accept);
            }
            "reject" | "ignore" => {
                let (nick, file) = split_word(rest);
                let ids: Vec<DccId> = self
                    .dcc
                    .entries
                    .iter()
                    .filter(|e| e.t.state == TransferState::Offered && (nick.is_empty() || self.nick_eq(e.t.conn, &e.t.nick, nick)))
                    .filter(|e| file.is_empty() || alpirc_proto::mask::wildcard_match(file, &e.t.file, Default::default()))
                    .map(|e| e.t.id)
                    .collect();
                if ids.is_empty() {
                    return Err(self.dt("err.nooffer", &[("cmd", &sub), ("nick", nick)]));
                }
                for id in ids {
                    self.dcc_reject(id);
                }
            }
            "close" | "cancel" => {
                let (what, rest) = split_word(rest);
                let (nick, file) = split_word(rest);
                let kind = match what.to_ascii_lowercase().as_str() {
                    "send" => DccType::Send,
                    "get" => DccType::Get,
                    "chat" => DccType::Chat,
                    _ => return Err(self.dt("err.usage", &[])),
                };
                if nick.is_empty() {
                    return Err(self.dt("err.usage", &[]));
                }
                let ids: Vec<DccId> = self
                    .dcc
                    .entries
                    .iter()
                    .filter(|e| {
                        e.t.kind == kind && !e.t.state.finished() && self.nick_eq(e.t.conn, &e.t.nick, nick.trim_start_matches('='))
                    })
                    .filter(|e| file.is_empty() || alpirc_proto::mask::wildcard_match(file, &e.t.file, Default::default()))
                    .map(|e| e.t.id)
                    .collect();
                if ids.is_empty() {
                    return Err(self.dt("err.nomatch", &[("what", what), ("nick", nick)]));
                }
                for id in ids {
                    self.dcc_cancel(id);
                }
            }
            "list" => self.dcc_list(win),
            _ => return Err(self.dt("err.usage", &[])),
        }
        Ok(())
    }

    fn find_offer(&self, conn: Option<ConnId>, nick: &str, file: &str, kind: DccType) -> Option<DccId> {
        self.dcc
            .entries
            .iter()
            .rev()
            .filter(|e| e.t.kind == kind && e.t.state == TransferState::Offered)
            .filter(|e| conn.is_none_or(|c| c == e.t.conn) || !self.conns.contains_key(&e.t.conn))
            .filter(|e| self.nick_eq(e.t.conn, &e.t.nick, nick))
            .find(|e| file.is_empty() || alpirc_proto::mask::wildcard_match(file, &e.t.file, Default::default()) || e.wire == file)
            .map(|e| e.t.id)
    }

    fn dcc_list(&mut self, win: WinId) {
        let lang = self.lang;
        let lines: Vec<String> = self
            .dcc
            .entries
            .iter()
            .map(|e| {
                let what = match e.t.kind {
                    DccType::Send => text(lang, "what.send"),
                    DccType::Get => text(lang, "what.get"),
                    DccType::Chat => text(lang, "what.chat"),
                };
                let mut s = format!("*  {} {} {}", e.t.id, what, e.t.nick);
                if e.t.kind != DccType::Chat {
                    s.push_str(&format!(" {}", e.t.file));
                    if let Some(sz) = e.t.size {
                        s.push_str(&format!(" {}/{}", proto::human_size(e.t.pos), proto::human_size(sz)));
                    }
                    if let Some(p) = e.t.percent() {
                        s.push_str(&format!(" {p}%"));
                    }
                    if e.t.state == TransferState::Active {
                        s.push_str(&format!(" {}/s", proto::human_size(e.t.speed)));
                    }
                }
                s.push_str(&format!(" [{}]", state_label(lang, e.t.state)));
                s
            })
            .collect();
        if lines.is_empty() {
            let t = self.dt("list.empty", &[]);
            self.print(win, LineKind::Info, t);
            return;
        }
        let t = self.dt("list.head", &[]);
        self.print(win, LineKind::Info, t);
        for l in lines {
            self.print(win, LineKind::Info, l);
        }
    }

    // ---------------------------------------------------------------------------------------------------------
    // Envois
    // ---------------------------------------------------------------------------------------------------------

    /// Propose un fichier à `nick` (offre passive ou écoute selon les réglages).
    pub(crate) fn dcc_offer_file(&mut self, conn: ConnId, nick: &str, path: &Path, auto_clear: bool) -> Result<DccId, String> {
        let size = match std::fs::metadata(path) {
            Ok(m) if m.is_file() => m.len(),
            _ => return Err(self.dt("err.nofile", &[("file", &path.display().to_string())])),
        };
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
        let passive = self.settings.dcc.passive;
        let id = self.new_entry(conn, DccType::Send, false, nick, &name);
        let timeout = self.dcc_timeout_ms();
        let token = if passive { Some(self.new_token()) } else { None };
        let (ports, packet, secs) = (self.dcc_ports(), self.dcc_packet(), self.settings.dcc.timeout_secs);
        if let Some(e) = self.entry_mut(id) {
            e.t.path = Some(path.to_path_buf());
            e.t.size = Some(size);
            e.t.passive = passive;
            e.auto_clear = auto_clear;
            e.token = token.clone();
        }
        if passive {
            let ip = self.announce_ip(conn);
            self.ctcp_dcc(conn, nick, &proto::encode_send(&name, ip, 0, size, token.as_deref()));
            if let Some(e) = self.entry_mut(id) {
                e.deadline = Some(e.t.created_ms + timeout);
            }
            let t = self.dt("send.offer", &[("file", &name), ("size", &proto::human_size(size)), ("nick", nick)]);
            self.dcc_print(conn, LineKind::Ctcp, t);
        } else {
            if let Some(e) = self.entry_mut(id) {
                e.open = true;
            }
            let io = DccIo::Send { path: path.to_path_buf(), offset: 0, size, packet };
            self.effects.push(Effect::DccListen { id, conn, ports, io, timeout_secs: secs });
        }
        Ok(id)
    }

    // ---------------------------------------------------------------------------------------------------------
    // Discussions
    // ---------------------------------------------------------------------------------------------------------

    /// Ouvre (ou montre) la fenêtre `=nick`.
    fn chat_window(&mut self, conn: ConnId, nick: &str) -> WinId {
        let name = format!("={nick}");
        match self.find_kind(conn, WinKind::DccChat, &name) {
            Some(w) => w,
            None => {
                let w = self.create_window(WinKind::DccChat, Some(conn), &name);
                let mut ev = self.ev(EventKind::Open, conn);
                ev.nick = nick.to_string();
                ev.target = Some(name);
                let ctx = self.ctx(Some(conn), Some(w));
                self.push_pres(Pres::new().event(ctx, ev));
                w
            }
        }
    }

    /// `/dcc chat nick` : accepte son offre s'il en a fait une (utilisateur seulement), sinon propose une
    /// discussion.
    pub(crate) fn dcc_chat_start(&mut self, conn: ConnId, nick: &str) -> Result<(), String> {
        if let Some(id) = self.find_offer(Some(conn), nick, "", DccType::Chat) {
            if !self.dcc.user {
                return Err(self.dt("err.script", &[("cmd", "chat")]));
            }
            self.dcc_accept(id, DccAnswer::Accept);
            return Ok(());
        }
        // Discussion déjà ouverte (ou en attente) avec lui : on montre sa fenêtre.
        if let Some(w) = self
            .dcc
            .entries
            .iter()
            .find(|e| e.t.kind == DccType::Chat && e.t.conn == conn && !e.t.state.finished() && self.nick_eq(conn, &e.t.nick, nick))
            .map(|e| e.t.win)
        {
            if let Some(w) = w {
                self.activate(w);
            }
            return Ok(());
        }
        let c = self.conn_ready(Some(conn))?;
        let passive = self.settings.dcc.passive_chat;
        let id = self.new_entry(c, DccType::Chat, false, nick, "");
        let w = self.chat_window(c, nick);
        self.activate(w);
        let token = if passive { Some(self.new_token()) } else { None };
        let (timeout, ports, secs) = (self.dcc_timeout_ms(), self.dcc_ports(), self.settings.dcc.timeout_secs);
        if let Some(e) = self.entry_mut(id) {
            e.t.win = Some(w);
            e.t.passive = passive;
            e.token = token.clone();
        }
        if passive {
            let ip = self.announce_ip(c);
            self.ctcp_dcc(c, nick, &proto::encode_chat(ip, 0, token.as_deref()));
            if let Some(e) = self.entry_mut(id) {
                e.deadline = Some(e.t.created_ms + timeout);
            }
            let t = self.dt("chat.offer.sent", &[("nick", nick)]);
            self.print(w, LineKind::Info, t);
        } else {
            if let Some(e) = self.entry_mut(id) {
                e.open = true;
            }
            self.effects.push(Effect::DccListen { id, conn: c, ports, io: DccIo::Chat, timeout_secs: secs });
        }
        Ok(())
    }

    /// Texte tapé dans une fenêtre `=nick`.
    pub(crate) fn dcc_chat_say(&mut self, win: WinId, text: &str) {
        let Some((id, nick, conn, active)) = self
            .dcc
            .entries
            .iter()
            .rev()
            .find(|e| e.t.kind == DccType::Chat && e.t.win == Some(win))
            .map(|e| (e.t.id, e.t.nick.clone(), e.t.conn, e.t.state == TransferState::Active))
        else {
            let nick = self.win(win).map(|w| w.name.trim_start_matches('=').to_string()).unwrap_or_default();
            let t = self.dt("chat.notconn", &[("nick", &nick)]);
            self.print(win, LineKind::Error, t);
            return;
        };
        if !active {
            let t = self.dt("chat.notconn", &[("nick", &nick)]);
            self.print(win, LineKind::Error, t);
            return;
        }
        let me = self.me(conn);
        for line in text.split(['\n', '\r']).filter(|l| !l.is_empty()) {
            self.effects.push(Effect::DccChatSend { id, line: line.to_string() });
            let shown = match ctcp::parse(line) {
                Some(c) if c.command == "ACTION" => format!("* {me} {}", c.params),
                Some(_) => continue,
                None => format!("<{me}> {line}"),
            };
            let kind = if shown.starts_with("* ") { LineKind::Action } else { LineKind::Own };
            let mut l = self.pl(win, kind, shown);
            l.nick = Some(me.clone());
            l.activity = Activity::None;
            self.add_line(l);
        }
    }

    /// `/msg =nick texte`.
    pub(crate) fn dcc_msg(&mut self, conn: Option<ConnId>, target: &str, text: &str) -> Result<(), String> {
        let nick = target.trim_start_matches('=');
        let win = self
            .dcc
            .entries
            .iter()
            .rev()
            .filter(|e| e.t.kind == DccType::Chat && conn.is_none_or(|c| c == e.t.conn) && self.nick_eq(e.t.conn, &e.t.nick, nick))
            .find_map(|e| e.t.win);
        match win {
            Some(w) => {
                self.dcc_chat_say(w, text);
                Ok(())
            }
            None => Err(self.dt("chat.notconn", &[("nick", nick)])),
        }
    }

    /// La fenêtre `=nick` se ferme : la discussion aussi.
    pub(crate) fn dcc_window_removed(&mut self, win: WinId) {
        let ids: Vec<DccId> =
            self.dcc.entries.iter().filter(|e| e.t.kind == DccType::Chat && e.t.win == Some(win)).map(|e| e.t.id).collect();
        for id in ids {
            if let Some(e) = self.entry_mut(id) {
                e.t.win = None;
            }
            if self.entry(id).is_some_and(|e| !e.t.state.finished()) {
                self.dcc_cancel(id);
            }
        }
    }

    // ---------------------------------------------------------------------------------------------------------
    // CTCP reçus
    // ---------------------------------------------------------------------------------------------------------

    /// Notre offre passive (envoi ou discussion) à laquelle répond ce CTCP (port non nul, même jeton).
    fn passive_reply_to(&self, conn: ConnId, offer: &DccOffer, kind: DccType) -> Option<DccId> {
        if offer.port == 0 {
            return None;
        }
        let token = offer.token.as_deref()?;
        self.dcc
            .entries
            .iter()
            .find(|e| {
                e.t.kind == kind
                    && !e.t.incoming
                    && e.t.passive
                    && e.t.state == TransferState::Waiting
                    && e.token.as_deref() == Some(token)
                    && self.nick_eq(conn, &e.t.nick, &offer.from)
            })
            .map(|e| e.t.id)
    }

    /// Présentation d'un CTCP DCC reçu (ligne affichée, traitement remis après les scripts).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dcc_ctcp(&mut self, conn: ConnId, from: &str, _addr: &str, params: &str, win: WinId, time: i64, mut p: Pres) -> Pres {
        let line = |s: &State, text: String| -> PendingLine {
            let mut l = s.pl(win, LineKind::Ctcp, text);
            l.time_ms = time;
            l
        };
        let Some(offer) = proto::parse_offer(from, params) else {
            return p.line(line(self, format!("[{from} DCC {params}]")));
        };
        match &offer.kind {
            DccKind::Send { file, size } if self.passive_reply_to(conn, &offer, DccType::Send).is_none() => {
                let sz = size.map(proto::human_size).unwrap_or_else(|| "?".into());
                let t = self.t(Key::DccSendOffer, &[("nick", from), ("file", &proto::sanitize_filename(file)), ("size", &sz)]);
                p = p.line(line(self, t));
            }
            DccKind::Chat if self.passive_reply_to(conn, &offer, DccType::Chat).is_none() => {
                let t = self.t(Key::DccChatOffer, &[("nick", from)]);
                p = p.line(line(self, t));
            }
            DccKind::Other(_) => return p.line(line(self, format!("[{from} DCC {params}]"))),
            _ => {}
        }
        p.dcc(DccIncoming { conn, offer })
    }

    /// Traite un CTCP DCC reçu (après les événements de script).
    pub(crate) fn dcc_incoming(&mut self, d: DccIncoming) {
        let DccIncoming { conn, offer } = d;
        match offer.kind.clone() {
            DccKind::Send { file, size } => {
                if let Some(id) = self.passive_reply_to(conn, &offer, DccType::Send) {
                    self.dcc_connect_reply(id, &offer);
                    return;
                }
                if invalid_offer(&offer) {
                    let t = self.dt("offer.invalid", &[("nick", &offer.from)]);
                    self.dcc_print(conn, LineKind::Error, t);
                    return;
                }
                self.dcc_new_get(conn, &offer, &file, size);
            }
            DccKind::Chat => {
                if let Some(id) = self.passive_reply_to(conn, &offer, DccType::Chat) {
                    self.dcc_connect_reply(id, &offer);
                    return;
                }
                if invalid_offer(&offer) {
                    let t = self.dt("offer.invalid", &[("nick", &offer.from)]);
                    self.dcc_print(conn, LineKind::Error, t);
                    return;
                }
                self.dcc_new_chat(conn, &offer);
            }
            DccKind::Resume { position, .. } => self.dcc_on_resume(conn, &offer, position),
            DccKind::Accept { position, .. } => self.dcc_on_accept(conn, &offer, position),
            DccKind::Other(_) => {}
        }
    }

    /// Réponse à notre offre passive : on se connecte à l'écoute de l'autre.
    fn dcc_connect_reply(&mut self, id: DccId, offer: &DccOffer) {
        let Some(ip) = offer.ip else { return };
        let (secs, packet) = (self.settings.dcc.timeout_secs, self.dcc_packet());
        let Some(e) = self.entry_mut(id) else { return };
        e.peer_ip = Some(ip);
        e.peer_port = offer.port;
        e.deadline = None;
        e.open = true;
        e.t.state = TransferState::Connecting;
        let io = match e.t.kind {
            DccType::Send => {
                DccIo::Send { path: e.t.path.clone().unwrap_or_default(), offset: e.t.start, size: e.t.size.unwrap_or(0), packet }
            }
            _ => DccIo::Chat,
        };
        self.effects.push(Effect::DccConnect { id, ip, port: offer.port, io, timeout_secs: secs });
    }

    fn dcc_new_get(&mut self, conn: ConnId, offer: &DccOffer, wire: &str, size: Option<u64>) {
        let name = proto::sanitize_filename(wire);
        let dangerous = proto::is_dangerous(wire);
        let path = self.dcc_folder().join(&name);
        let existing = std::fs::metadata(&path).ok().filter(|m| m.is_file()).map(|m| m.len());
        let id = self.new_entry(conn, DccType::Get, true, &offer.from, &name);
        let deadline = self.now + self.dcc_timeout_ms();
        if let Some(e) = self.entry_mut(id) {
            e.wire = wire.to_string();
            e.t.path = Some(path);
            e.t.size = size;
            e.t.passive = offer.is_passive();
            e.t.dangerous = dangerous;
            e.t.state = TransferState::Offered;
            e.peer_ip = offer.ip;
            e.peer_port = offer.port;
            e.token = offer.token.clone();
            e.deadline = Some(deadline);
        }
        match self.settings.dcc.on_send {
            DccSendAction::Ignore => {
                if let Some(e) = self.entry_mut(id) {
                    e.t.state = TransferState::Cancelled;
                    e.t.ended_ms = Some(e.t.created_ms);
                    e.deadline = None;
                }
                let t = self.dt("offer.ignored", &[("nick", &offer.from), ("file", &name)]);
                self.dcc_print(conn, LineKind::Ctcp, t);
            }
            DccSendAction::AutoGet if !dangerous => {
                let answer = match (existing, self.settings.dcc.on_exists) {
                    (None, _) => Some(DccAnswer::Accept),
                    (Some(_), DccExists::Ask) => None,
                    (Some(_), DccExists::Resume) => Some(DccAnswer::Resume),
                    (Some(_), DccExists::Overwrite) => Some(DccAnswer::Overwrite),
                    (Some(_), DccExists::Rename) => Some(DccAnswer::Rename),
                };
                match answer {
                    Some(a) => {
                        let t = self.dt("get.auto", &[("nick", &offer.from), ("file", &name)]);
                        self.dcc_print(conn, LineKind::Ctcp, t);
                        self.dcc_accept(id, a);
                    }
                    None => self.dcc_ask(id),
                }
            }
            DccSendAction::AutoGet => {
                let t = self.dt("get.dangerous", &[("file", &name)]);
                self.dcc_print(conn, LineKind::Ctcp, t);
                self.dcc_ask(id);
            }
            DccSendAction::Ask => self.dcc_ask(id),
        }
    }

    fn dcc_new_chat(&mut self, conn: ConnId, offer: &DccOffer) {
        let id = self.new_entry(conn, DccType::Chat, true, &offer.from, "");
        let deadline = self.now + self.dcc_timeout_ms();
        if let Some(e) = self.entry_mut(id) {
            e.t.passive = offer.is_passive();
            e.t.state = TransferState::Offered;
            e.peer_ip = offer.ip;
            e.peer_port = offer.port;
            e.token = offer.token.clone();
            e.deadline = Some(deadline);
        }
        match self.settings.dcc.on_chat {
            DccChatAction::Ignore => {
                if let Some(e) = self.entry_mut(id) {
                    e.t.state = TransferState::Cancelled;
                    e.t.ended_ms = Some(e.t.created_ms);
                    e.deadline = None;
                }
                let t = self.dt("offer.ignored", &[("nick", &offer.from), ("file", "chat")]);
                self.dcc_print(conn, LineKind::Ctcp, t);
            }
            DccChatAction::AutoAccept => {
                let t = self.dt("chat.auto", &[("nick", &offer.from)]);
                self.dcc_print(conn, LineKind::Ctcp, t);
                self.dcc_accept(id, DccAnswer::Accept);
            }
            DccChatAction::Ask => self.dcc_ask(id),
        }
    }

    /// Ce que l'interface montre pour une offre reçue.
    pub(crate) fn dcc_prompt(&self, id: DccId) -> Option<DccPrompt> {
        let e = self.entry(id)?;
        let existing = e.t.path.as_ref().and_then(|p| std::fs::metadata(p).ok()).filter(|m| m.is_file()).map(|m| m.len());
        Some(DccPrompt {
            id,
            conn: e.t.conn,
            network: self
                .conns
                .get(&e.t.conn)
                .map(|c| if c.network.is_empty() { c.host.clone() } else { c.network.clone() })
                .unwrap_or_default(),
            nick: e.t.nick.clone(),
            chat: e.t.kind == DccType::Chat,
            file: e.t.file.clone(),
            size: e.t.size,
            existing: if e.t.kind == DccType::Get { existing } else { None },
            dangerous: e.t.dangerous,
            path: e.t.path.clone(),
        })
    }

    /// Demande à l'utilisateur (boîte de l'interface, notification si l'appli n'est pas au premier plan).
    fn dcc_ask(&mut self, id: DccId) {
        let Some(p) = self.dcc_prompt(id) else { return };
        let win = self.dcc_info_win(p.conn);
        let n = &self.settings.notifications;
        if n.enabled && (!self.app_focused || !n.only_when_inactive) {
            let body = if p.chat {
                self.t(Key::DccChatOffer, &[("nick", &p.nick)])
            } else {
                let sz = p.size.map(proto::human_size).unwrap_or_else(|| "?".into());
                self.t(Key::DccSendOffer, &[("nick", &p.nick), ("file", &p.file), ("size", &sz)])
            };
            self.effects.push(Effect::Notify { win, title: "DCC".into(), body: body.trim_start_matches("* ").to_string() });
        }
        self.effects.push(Effect::DccPrompt(p));
    }

    /// Le destinataire demande à reprendre notre envoi à `position`.
    fn dcc_on_resume(&mut self, conn: ConnId, offer: &DccOffer, position: u64) {
        let found = self.dcc.entries.iter().find(|e| {
            e.t.kind == DccType::Send
                && e.t.state == TransferState::Waiting
                && self.nick_eq(conn, &e.t.nick, &offer.from)
                && if offer.port == 0 {
                    e.t.passive && e.token.is_some() && e.token == offer.token
                } else {
                    !e.t.passive && e.our_port == offer.port
                }
        });
        let Some(e) = found else { return };
        let (id, size, passive, wire, token, nick) =
            (e.t.id, e.t.size.unwrap_or(0), e.t.passive, e.wire.clone(), e.token.clone(), e.t.nick.clone());
        if position == 0 || position >= size {
            return;
        }
        let deadline = self.now + self.dcc_timeout_ms();
        if let Some(e) = self.entry_mut(id) {
            e.t.start = position;
            e.t.pos = position;
            if passive {
                e.deadline = Some(deadline);
            }
        }
        if !passive {
            self.effects.push(Effect::DccResume { id, offset: position });
        }
        self.ctcp_dcc(conn, &nick, &proto::encode_accept(&wire, offer.port, position, if passive { token.as_deref() } else { None }));
        let t = self.dt("send.resume", &[("nick", &nick), ("file", &wire), ("pos", &proto::human_size(position))]);
        self.dcc_print(conn, LineKind::Ctcp, t);
    }

    /// L'envoyeur accepte notre demande de reprise.
    fn dcc_on_accept(&mut self, conn: ConnId, offer: &DccOffer, position: u64) {
        let found = self
            .dcc
            .entries
            .iter()
            .find(|e| {
                e.t.kind == DccType::Get
                    && e.t.state == TransferState::Waiting
                    && e.resume_at == Some(position)
                    && self.nick_eq(conn, &e.t.nick, &offer.from)
                    && if offer.port == 0 { e.token.is_some() && e.token == offer.token } else { e.peer_port == offer.port }
            })
            .map(|e| e.t.id);
        if let Some(id) = found {
            if let Some(e) = self.entry_mut(id) {
                e.resume_at = None;
                e.deadline = None;
            }
            self.dcc_start_get(id, position);
        }
    }

    // ---------------------------------------------------------------------------------------------------------
    // Décisions de l'utilisateur
    // ---------------------------------------------------------------------------------------------------------

    /// Accepte une offre reçue.
    pub(crate) fn dcc_accept(&mut self, id: DccId, answer: DccAnswer) {
        let Some(e) = self.entry(id) else { return };
        if e.t.state != TransferState::Offered {
            return;
        }
        match e.t.kind {
            DccType::Chat => self.dcc_accept_chat(id),
            DccType::Get => self.dcc_accept_get(id, answer),
            DccType::Send => {}
        }
    }

    fn dcc_accept_chat(&mut self, id: DccId) {
        let Some(e) = self.entry(id) else { return };
        let (conn, nick, passive, ip, port) = (e.t.conn, e.t.nick.clone(), e.t.passive, e.peer_ip, e.peer_port);
        let w = self.chat_window(conn, &nick);
        self.activate(w);
        let (ports, secs) = (self.dcc_ports(), self.settings.dcc.timeout_secs);
        if let Some(e) = self.entry_mut(id) {
            e.t.win = Some(w);
            e.deadline = None;
            e.open = true;
            e.t.state = if passive { TransferState::Waiting } else { TransferState::Connecting };
        }
        if passive {
            self.effects.push(Effect::DccListen { id, conn, ports, io: DccIo::Chat, timeout_secs: secs });
        } else if let Some(ip) = ip {
            self.effects.push(Effect::DccConnect { id, ip, port, io: DccIo::Chat, timeout_secs: secs });
            let t = self.dt("chat.connecting", &[("nick", &nick)]);
            self.print(w, LineKind::Info, t);
        }
    }

    fn dcc_accept_get(&mut self, id: DccId, answer: DccAnswer) {
        let Some(e) = self.entry(id) else { return };
        let Some(path) = e.t.path.clone() else { return };
        let (conn, size, nick, file) = (e.t.conn, e.t.size, e.t.nick.clone(), e.t.file.clone());
        let existing = std::fs::metadata(&path).ok().filter(|m| m.is_file()).map(|m| m.len());
        let partial = |len: u64| len > 0 && size.is_none_or(|s| len < s);
        let rename = |p: &Path| -> PathBuf {
            let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            dir.join(proto::unique_name(&name, |n| dir.join(n).exists()))
        };
        let (path, offset) = match (answer, existing) {
            (_, None) => (path, 0),
            (DccAnswer::Overwrite, Some(_)) => (path, 0),
            (DccAnswer::Resume, Some(len)) if partial(len) => (path, len),
            (DccAnswer::Accept, Some(len)) => match self.settings.dcc.on_exists {
                DccExists::Resume if partial(len) => (path, len),
                DccExists::Overwrite => (path, 0),
                _ => (rename(&path), 0),
            },
            _ => (rename(&path), 0),
        };
        if let Some(dir) = path.parent()
            && let Err(err) = std::fs::create_dir_all(dir)
        {
            let error = self.dt("err.folder", &[("path", &dir.display().to_string()), ("error", &err.to_string())]);
            self.dcc_fail(id, &error);
            return;
        }
        let file_now = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(file);
        let timeout = self.dcc_timeout_ms();
        let now = self.now;
        if let Some(e) = self.entry_mut(id) {
            e.t.path = Some(path);
            e.t.file = file_now.clone();
            e.deadline = None;
        }
        if offset > 0 {
            let Some(e) = self.entry_mut(id) else { return };
            e.resume_at = Some(offset);
            e.t.state = TransferState::Waiting;
            e.deadline = Some(now + timeout);
            let (wire, port, token) = (e.wire.clone(), e.peer_port, e.token.clone());
            self.ctcp_dcc(conn, &nick, &proto::encode_resume(&wire, port, offset, if port == 0 { token.as_deref() } else { None }));
            let t = self.dt("get.resume", &[("nick", &nick), ("file", &file_now), ("pos", &proto::human_size(offset))]);
            self.dcc_print(conn, LineKind::Ctcp, t);
        } else {
            self.dcc_start_get(id, 0);
        }
    }

    /// Ouvre la réception (connexion à l'envoyeur, ou écoute pour une offre passive).
    fn dcc_start_get(&mut self, id: DccId, offset: u64) {
        let (ports, secs) = (self.dcc_ports(), self.settings.dcc.timeout_secs);
        let Some(e) = self.entry_mut(id) else { return };
        e.t.start = offset;
        e.t.pos = offset;
        e.open = true;
        let io = DccIo::Receive { path: e.t.path.clone().unwrap_or_default(), offset, size: e.t.size };
        let (conn, passive, ip, port) = (e.t.conn, e.t.passive, e.peer_ip, e.peer_port);
        if passive {
            e.t.state = TransferState::Waiting;
            self.effects.push(Effect::DccListen { id, conn, ports, io, timeout_secs: secs });
        } else if let Some(ip) = ip {
            e.t.state = TransferState::Connecting;
            self.effects.push(Effect::DccConnect { id, ip, port, io, timeout_secs: secs });
        }
    }

    /// Refuse une offre reçue.
    pub(crate) fn dcc_reject(&mut self, id: DccId) {
        let Some(e) = self.entry(id) else { return };
        if e.t.state != TransferState::Offered {
            return;
        }
        let (conn, nick, file) =
            (e.t.conn, e.t.nick.clone(), if e.t.kind == DccType::Chat { "chat".to_string() } else { e.t.file.clone() });
        let now = self.now;
        if let Some(e) = self.entry_mut(id) {
            e.t.state = TransferState::Cancelled;
            e.t.ended_ms = Some(now);
            e.deadline = None;
        }
        let t = self.dt("offer.rejected", &[("nick", &nick), ("file", &file)]);
        self.dcc_print(conn, LineKind::Ctcp, t);
    }

    /// Annule un transfert ou ferme une discussion.
    pub(crate) fn dcc_cancel(&mut self, id: DccId) {
        let Some(e) = self.entry(id) else { return };
        match e.t.state {
            TransferState::Offered => return self.dcc_reject(id),
            s if s.finished() => return,
            _ => {}
        }
        let (open, kind, conn, nick, file, win, path, active) =
            (e.open, e.t.kind, e.t.conn, e.t.nick.clone(), e.t.file.clone(), e.t.win, e.t.path.clone(), e.t.state == TransferState::Active);
        if open {
            self.effects.push(Effect::DccClose { id });
        }
        let now = self.now;
        let err = self.dt("err.cancelled", &[]);
        if let Some(e) = self.entry_mut(id) {
            e.open = false;
            e.deadline = None;
            e.t.state = TransferState::Cancelled;
            e.t.ended_ms = Some(now);
            e.t.error = Some(err);
        }
        match kind {
            DccType::Chat => {
                if let Some(w) = win.filter(|w| self.win(*w).is_some())
                    && active
                {
                    let t = self.dt("chat.closed", &[("nick", &nick)]);
                    self.print(w, LineKind::Info, t);
                }
            }
            _ => {
                let what = text(self.lang, if kind == DccType::Send { "what.send" } else { "what.get" });
                let t = self.dt("cancelled", &[("what", what), ("file", &file), ("nick", &nick)]);
                let ev_kind = if kind == DccType::Send { EventKind::SendFail } else { EventKind::GetFail };
                self.dcc_event(conn, ev_kind, &nick, path.as_deref(), t, LineKind::Ctcp);
            }
        }
    }

    /// Relance un envoi terminé (ou échoué) : nouvelle offre du même fichier.
    pub(crate) fn dcc_resend(&mut self, id: DccId) -> Result<(), String> {
        let Some(e) = self.entry(id) else { return Ok(()) };
        if e.t.kind != DccType::Send || !e.t.state.finished() {
            return Ok(());
        }
        let (conn, nick, path) = (e.t.conn, e.t.nick.clone(), e.t.path.clone().unwrap_or_default());
        let c = self.conn_ready(Some(conn))?;
        self.dcc_offer_file(c, &nick, &path, false).map(|_| ())
    }

    /// Retire de la liste les transferts terminés.
    pub(crate) fn dcc_clear_finished(&mut self) {
        self.dcc.entries.retain(|e| !(e.t.state.finished() && e.t.kind != DccType::Chat));
        self.dcc.version += 1;
    }

    /// Échec d'une entrée (avec l'événement de script et la ligne).
    fn dcc_fail(&mut self, id: DccId, error: &str) {
        let Some(e) = self.entry(id) else { return };
        if e.t.state.finished() {
            return;
        }
        let (kind, conn, nick, file, win, path) = (e.t.kind, e.t.conn, e.t.nick.clone(), e.t.file.clone(), e.t.win, e.t.path.clone());
        let now = self.now;
        if let Some(e) = self.entry_mut(id) {
            e.t.state = TransferState::Failed;
            e.t.error = Some(error.to_string());
            e.t.ended_ms = Some(now);
            e.deadline = None;
            e.open = false;
        }
        match kind {
            DccType::Chat => {
                let t = self.dt("chat.failed", &[("nick", &nick), ("error", error)]);
                match win.filter(|w| self.win(*w).is_some()) {
                    Some(w) => self.print(w, LineKind::Error, t),
                    None => self.dcc_print(conn, LineKind::Error, t),
                }
            }
            DccType::Send => {
                let t = self.dt("send.failed", &[("file", &file), ("nick", &nick), ("error", error)]);
                self.dcc_event(conn, EventKind::SendFail, &nick, path.as_deref(), t, LineKind::Error);
            }
            DccType::Get => {
                let t = self.dt("get.failed", &[("file", &file), ("nick", &nick), ("error", error)]);
                self.dcc_event(conn, EventKind::GetFail, &nick, path.as_deref(), t, LineKind::Error);
            }
        }
    }

    /// Événement de script FILESENT, FILERCVD, SENDFAIL ou GETFAIL, et sa ligne (affichée si le script ne
    /// l'arrête pas).
    fn dcc_event(&mut self, conn: ConnId, kind: EventKind, nick: &str, path: Option<&Path>, text: String, lkind: LineKind) {
        let win = self.dcc_info_win(conn);
        let file = path.map(|p| p.display().to_string()).unwrap_or_default();
        let mut ev = self.ev(kind, conn);
        ev.nick = nick.to_string();
        ev.text = file.clone();
        ev.extra.push(("filename".into(), file));
        let ctx = self.ctx(Some(conn), Some(win));
        let l = self.pl(win, lkind, text);
        self.push_pres(Pres::new().event(ctx, ev).line(l));
    }

    // ---------------------------------------------------------------------------------------------------------
    // Ce que dit l'hôte
    // ---------------------------------------------------------------------------------------------------------

    pub(crate) fn dcc_listening(&mut self, id: DccId, port: u16) {
        let Some(e) = self.entry(id) else { return };
        if e.t.state.finished() {
            return;
        }
        let (conn, kind, incoming, nick, wire, size, token, passive, win) =
            (e.t.conn, e.t.kind, e.t.incoming, e.t.nick.clone(), e.wire.clone(), e.t.size, e.token.clone(), e.t.passive, e.t.win);
        let ip = self.announce_ip(conn);
        if let Some(e) = self.entry_mut(id) {
            e.our_port = port;
            e.t.state = TransferState::Waiting;
        }
        match (kind, incoming) {
            (DccType::Send, false) => {
                self.ctcp_dcc(conn, &nick, &proto::encode_send(&wire, ip, port, size.unwrap_or(0), None));
                let t = self.dt("send.offer", &[("file", &wire), ("size", &proto::human_size(size.unwrap_or(0))), ("nick", &nick)]);
                self.dcc_print(conn, LineKind::Ctcp, t);
            }
            (DccType::Chat, false) => {
                self.ctcp_dcc(conn, &nick, &proto::encode_chat(ip, port, None));
                if let Some(w) = win {
                    let t = self.dt("chat.offer.sent", &[("nick", &nick)]);
                    self.print(w, LineKind::Info, t);
                }
            }
            // Offres passives reçues : on répond avec notre adresse, notre port et le même jeton.
            (DccType::Get, true) if passive => {
                self.ctcp_dcc(conn, &nick, &proto::encode_send(&wire, ip, port, size.unwrap_or(0), token.as_deref()));
            }
            (DccType::Chat, true) if passive => {
                self.ctcp_dcc(conn, &nick, &proto::encode_chat(ip, port, token.as_deref()));
                if let Some(w) = win {
                    let t = self.dt("chat.connecting", &[("nick", &nick)]);
                    self.print(w, LineKind::Info, t);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn dcc_connected(&mut self, id: DccId) {
        let now = self.now;
        let Some(e) = self.entry_mut(id) else { return };
        if e.t.state.finished() {
            return;
        }
        e.t.state = TransferState::Active;
        e.t.started_ms = Some(now);
        e.deadline = None;
        let (kind, conn, nick, file, win) = (e.t.kind, e.t.conn, e.t.nick.clone(), e.t.file.clone(), e.t.win);
        match kind {
            DccType::Chat => {
                let t = self.dt("chat.open", &[("nick", &nick)]);
                match win.filter(|w| self.win(*w).is_some()) {
                    Some(w) => self.print(w, LineKind::Info, t),
                    None => self.dcc_print(conn, LineKind::Info, t),
                }
            }
            DccType::Send => {
                let t = self.dt("send.start", &[("file", &file), ("nick", &nick)]);
                self.dcc_print(conn, LineKind::Ctcp, t);
            }
            DccType::Get => {
                let t = self.dt("get.start", &[("file", &file), ("nick", &nick)]);
                self.dcc_print(conn, LineKind::Ctcp, t);
            }
        }
    }

    pub(crate) fn dcc_progress(&mut self, id: DccId, bytes: u64) {
        let now = self.now;
        let Some(e) = self.entry_mut(id) else { return };
        if e.t.state.finished() {
            return;
        }
        e.t.pos = bytes.max(e.t.start);
        if let Some(t0) = e.t.started_ms {
            let ms = (now - t0).max(1) as u64;
            e.t.speed = (e.t.pos.saturating_sub(e.t.start) as u128 * 1000 / ms as u128) as u64;
        }
    }

    pub(crate) fn dcc_chat_line(&mut self, id: DccId, bytes: &[u8]) {
        let Some(e) = self.entry(id) else { return };
        if e.t.kind != DccType::Chat || e.t.state != TransferState::Active {
            return;
        }
        let (conn, nick) = (e.t.conn, e.t.nick.clone());
        let win = match e.t.win.filter(|w| self.win(*w).is_some()) {
            Some(w) => w,
            None => {
                let w = self.chat_window(conn, &nick);
                if let Some(e) = self.entry_mut(id) {
                    e.t.win = Some(w);
                }
                w
            }
        };
        let text = Encoding::Auto.decode(bytes).trim_end_matches(['\r', '\n']).to_string();
        if text.is_empty() {
            return;
        }
        let (shown, body, lkind) = match ctcp::parse(&text) {
            Some(c) if c.command == "ACTION" => (format!("* {nick} {}", c.params), c.params.to_string(), LineKind::Action),
            Some(_) => return,
            None => (format!("<{nick}> {text}"), text.clone(), LineKind::Normal),
        };
        let shown = if self.settings.irc.strip_codes { alpirc_proto::format::strip(&shown) } else { shown };
        let mut ev = self.ev(EventKind::Chat, conn);
        ev.nick = nick.clone();
        ev.target = Some(format!("={nick}"));
        ev.text = body.clone();
        let ctx = self.ctx(Some(conn), Some(win));
        let mut l = self.pl(win, lkind, shown);
        l.nick = Some(nick.clone());
        l.activity = Activity::Message;
        let mut p = Pres::new().event(ctx, ev);
        if let Some(h) = self.highlight(conn, &body) {
            l.highlight = true;
            l.activity = Activity::Highlight;
            l.color = h.color;
        }
        p = p.line(l);
        let hl = &self.settings.highlight;
        let unseen = self.unseen(win);
        if unseen && hl.flash {
            p = p.effect(Effect::Flash { win });
        }
        if unseen && hl.beep_on_query {
            p = p.effect(Effect::Beep);
        }
        let n = &self.settings.notifications;
        if hl.notify_on_query && n.enabled && (unseen || !n.only_when_inactive) {
            let title = self.t(Key::NotifQuery, &[("nick", &format!("={nick}"))]);
            p = p.effect(Effect::Notify { win, title, body: alpirc_proto::format::strip(&body) });
        }
        self.push_pres(p);
    }

    pub(crate) fn dcc_ended(&mut self, id: DccId, bytes: u64, error: Option<&str>) {
        let now = self.now;
        let Some(e) = self.entry_mut(id) else { return };
        e.open = false;
        if e.t.state.finished() {
            return;
        }
        let was_active = e.t.state == TransferState::Active;
        e.t.pos = e.t.pos.max(bytes);
        if let Some(t0) = e.t.started_ms {
            let ms = (now - t0).max(1) as u64;
            e.t.speed = (e.t.pos.saturating_sub(e.t.start) as u128 * 1000 / ms as u128) as u64;
        }
        let (kind, conn, nick, file, win, path, size, pos, speed, auto_clear) =
            (e.t.kind, e.t.conn, e.t.nick.clone(), e.t.file.clone(), e.t.win, e.t.path.clone(), e.t.size, e.t.pos, e.t.speed, e.auto_clear);
        let incomplete = kind == DccType::Get && size.is_some_and(|s| pos < s);
        let error = match error {
            Some(err) => Some(err.to_string()),
            None if incomplete => Some(self.dt("err.incomplete", &[("pos", &pos.to_string()), ("size", &size.unwrap_or(0).to_string())])),
            None if kind == DccType::Chat && !was_active => Some(self.dt("err.timeout", &[])),
            None => None,
        };
        if let Some(err) = error {
            if kind == DccType::Chat && was_active {
                if let Some(e) = self.entry_mut(id) {
                    e.t.state = TransferState::Done;
                    e.t.ended_ms = Some(now);
                    e.t.error = Some(err.clone());
                }
                let t = format!("{} ({err})", self.dt("chat.closed", &[("nick", &nick)]));
                if let Some(w) = win.filter(|w| self.win(*w).is_some()) {
                    self.print(w, LineKind::Info, t);
                }
                return;
            }
            self.dcc_fail(id, &err);
            return;
        }
        if let Some(e) = self.entry_mut(id) {
            e.t.state = TransferState::Done;
            e.t.ended_ms = Some(now);
        }
        let (size_s, speed_s) = (proto::human_size(pos), proto::human_size(speed));
        match kind {
            DccType::Chat => {
                let t = self.dt("chat.closed", &[("nick", &nick)]);
                match win.filter(|w| self.win(*w).is_some()) {
                    Some(w) => self.print(w, LineKind::Info, t),
                    None => self.dcc_print(conn, LineKind::Info, t),
                }
            }
            DccType::Send => {
                let t = self.dt("send.done", &[("file", &file), ("nick", &nick), ("size", &size_s), ("speed", &speed_s)]);
                self.dcc_event(conn, EventKind::FileSent, &nick, path.as_deref(), t, LineKind::Ctcp);
            }
            DccType::Get => {
                let p = path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                let t = self.dt("get.done", &[("file", &file), ("nick", &nick), ("size", &size_s), ("speed", &speed_s), ("path", &p)]);
                self.dcc_event(conn, EventKind::FileRcvd, &nick, path.as_deref(), t, LineKind::Ctcp);
            }
        }
        if auto_clear {
            self.dcc.entries.retain(|e| e.t.id != id);
        }
    }

    /// Échéances : offres reçues sans réponse, réponses attendues qui ne viennent pas.
    pub(crate) fn dcc_tick(&mut self) {
        let now = self.now;
        let due: Vec<(DccId, TransferState)> = self
            .dcc
            .entries
            .iter()
            .filter(|e| e.deadline.is_some_and(|d| d <= now) && !e.t.state.finished())
            .map(|e| (e.t.id, e.t.state))
            .collect();
        for (id, st) in due {
            if st == TransferState::Offered {
                let Some(e) = self.entry_mut(id) else { continue };
                e.t.state = TransferState::Cancelled;
                e.t.ended_ms = Some(now);
                e.deadline = None;
                let (conn, nick, file) =
                    (e.t.conn, e.t.nick.clone(), if e.t.kind == DccType::Chat { "chat".to_string() } else { e.t.file.clone() });
                let t = self.dt("offer.expired", &[("nick", &nick), ("file", &file)]);
                self.dcc_print(conn, LineKind::Ctcp, t);
            } else {
                if self.entry(id).is_some_and(|e| e.open) {
                    self.effects.push(Effect::DccClose { id });
                }
                let err = self.dt("err.timeout", &[]);
                self.dcc_fail(id, &err);
            }
        }
    }

    pub(crate) fn dcc_next_wake(&self) -> Option<i64> {
        self.dcc.entries.iter().filter(|e| !e.t.state.finished()).filter_map(|e| e.deadline).min()
    }
}

impl Client {
    /// Transferts de fichiers (envois et réceptions), du plus ancien au plus récent.
    pub fn transfers(&self) -> Vec<Transfer> {
        self.st.dcc.entries.iter().filter(|e| e.t.kind != DccType::Chat).map(|e| e.t.clone()).collect()
    }

    /// Un transfert ou une discussion.
    pub fn dcc_entry(&self, id: DccId) -> Option<Transfer> {
        self.st.entry(id).map(|e| e.t.clone())
    }

    /// Augmente à chaque changement des transferts (pour redessiner).
    pub fn dcc_version(&self) -> u64 {
        self.st.dcc.version
    }

    /// Ce que l'interface montre pour une offre reçue (toujours en attente : [`TransferState::Offered`]).
    pub fn dcc_prompt(&self, id: DccId) -> Option<DccPrompt> {
        self.st.entry(id).filter(|e| e.t.state == TransferState::Offered)?;
        self.st.dcc_prompt(id)
    }

    /// Adresse locale de la connexion IRC `conn` (donnée par l'hôte à la connexion) : annoncée dans nos offres
    /// DCC, sauf réglage `dcc.own_ip` ou adresse publique vue par le serveur.
    pub fn set_local_ip(&mut self, conn: ConnId, ip: IpAddr) {
        self.st.dcc.local_ip.insert(conn, ip);
    }

    /// Dossier des téléchargements du système (`Téléchargements/AlpIRC`), utilisé si les réglages n'en
    /// donnent pas.
    pub fn set_downloads_dir(&mut self, dir: Option<PathBuf>) {
        self.st.dcc.downloads = dir;
    }

    /// Dossier où arrivent les fichiers reçus.
    pub fn dcc_folder(&self) -> PathBuf {
        self.st.dcc_folder()
    }

    /// Envoie un fichier choisi dans l'interface (boîte « Envoyer », sélecteur de fichiers, glisser-déposer).
    pub fn dcc_send_file(&mut self, conn: ConnId, nick: &str, path: &Path, now_ms: i64) -> Result<DccId, String> {
        self.st.now = now_ms;
        let r = match self.st.conn_ready(Some(conn)) {
            Ok(c) => self.st.dcc_offer_file(c, nick, path, false),
            Err(e) => Err(e),
        };
        self.finish();
        r
    }

    /// Propose une discussion (ou accepte celle que `nick` propose) : boîte « Discussion DCC ».
    pub fn dcc_chat(&mut self, conn: ConnId, nick: &str, now_ms: i64) -> Result<(), String> {
        self.st.now = now_ms;
        let user = std::mem::replace(&mut self.st.dcc.user, true);
        let r = self.st.dcc_chat_start(conn, nick);
        self.st.dcc.user = user;
        self.finish();
        r
    }

    /// L'utilisateur accepte une offre reçue ([`Effect::DccPrompt`]).
    pub fn dcc_accept(&mut self, id: DccId, answer: DccAnswer, now_ms: i64) {
        self.st.now = now_ms;
        self.st.dcc_accept(id, answer);
        self.finish();
    }

    /// L'utilisateur refuse une offre reçue.
    pub fn dcc_reject(&mut self, id: DccId, now_ms: i64) {
        self.st.now = now_ms;
        self.st.dcc_reject(id);
        self.finish();
    }

    /// Annule un transfert (ou ferme une discussion).
    pub fn dcc_cancel(&mut self, id: DccId, now_ms: i64) {
        self.st.now = now_ms;
        self.st.dcc_cancel(id);
        self.finish();
    }

    /// Propose à nouveau un fichier déjà envoyé (ou dont l'envoi a échoué).
    pub fn dcc_resend(&mut self, id: DccId, now_ms: i64) -> Result<(), String> {
        self.st.now = now_ms;
        let r = self.st.dcc_resend(id);
        self.finish();
        r
    }

    /// Retire de la liste les transferts terminés.
    pub fn dcc_clear_finished(&mut self) {
        self.st.dcc_clear_finished();
    }

    /// L'hôte écoute pour `id` ([`Effect::DccListen`]) sur ce port.
    pub fn on_dcc_listening(&mut self, id: DccId, port: u16, now_ms: i64) {
        self.st.now = now_ms;
        self.st.dcc_listening(id, port);
        self.finish();
    }

    /// La connexion directe `id` est établie.
    pub fn on_dcc_connected(&mut self, id: DccId, now_ms: i64) {
        self.st.now = now_ms;
        self.st.dcc_connected(id);
        self.finish();
    }

    /// Avancement d'un transfert : position atteinte (départ compris).
    pub fn on_dcc_progress(&mut self, id: DccId, bytes: u64, now_ms: i64) {
        self.st.now = now_ms;
        self.st.dcc_progress(id, bytes);
    }

    /// Une ligne reçue dans une discussion DCC.
    pub fn on_dcc_chat_line(&mut self, id: DccId, bytes: &[u8], now_ms: i64) {
        self.st.now = now_ms;
        self.st.dcc_chat_line(id, bytes);
        self.finish();
    }

    /// Fin d'une connexion directe (ou échec de l'écoute, de la connexion) : `error` vaut `None` pour un
    /// transfert complet ou une discussion terminée normalement.
    pub fn on_dcc_ended(&mut self, id: DccId, bytes: u64, error: Option<&str>, now_ms: i64) {
        self.st.now = now_ms;
        self.st.dcc_ended(id, bytes, error);
        self.finish();
    }
}
