//! État du client (tout sauf le moteur de scripts) : réglages, connexions, fenêtres, files d'attente. Le moteur
//! le voit comme son hôte (`alpirc_script::Host`, voir `host.rs`).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use alpirc_proto::Message;
use alpirc_proto::encoding::Encoding;
use alpirc_script::{Ctx, EventInfo, EventKind};

use super::flood::FloodQueue;
use super::logging::Logger;
use super::window::{Activity, Line, LineKind, NickEntry, WinKind, Window};
use super::{CaughtUrl, ChanList, ConnId, ConnState, Effect, TlsVerify, WinId};
use crate::i18n::{self, Key, Lang};
use crate::session::{Session, SessionConfig};
use crate::settings::{Server, Settings};
use crate::timeutil;

/// Travail remis à la fin du traitement en cours (le moteur de scripts doit être libre).
pub(crate) enum Deferred {
    /// Événements à déclencher, puis sorties à appliquer si aucun n'a fait `/haltdef`.
    Pres(Pres),
    /// Ligne à exécuter par le moteur (perform, `/timer`…).
    Script(Box<Ctx>, String),
    /// Rejoindre les canaux (après le perform).
    AutoJoin(ConnId),
}

/// Une « présentation » : des événements de script et l'affichage par défaut qui en dépend.
#[derive(Default)]
pub(crate) struct Pres {
    pub events: Vec<(Ctx, EventInfo)>,
    pub outs: Vec<Out>,
}

impl Pres {
    pub fn new() -> Pres {
        Pres::default()
    }
    pub fn event(mut self, ctx: Ctx, ev: EventInfo) -> Pres {
        self.events.push((ctx, ev));
        self
    }
    pub fn line(mut self, l: PendingLine) -> Pres {
        self.outs.push(Out::Line(l));
        self
    }
    pub fn effect(mut self, e: Effect) -> Pres {
        self.outs.push(Out::Effect(e));
        self
    }
    pub fn send(mut self, conn: ConnId, line: String) -> Pres {
        self.outs.push(Out::Send(conn, line));
        self
    }
}

pub(crate) enum Out {
    Line(PendingLine),
    Effect(Effect),
    Send(ConnId, String),
    /// CTCP DCC reçu, traité si aucun script ne l'a arrêté (voir `dcc.rs`).
    Dcc(Box<super::dcc::DccIncoming>),
}

/// Une ligne à ajouter à une fenêtre.
#[derive(Debug, Clone)]
pub(crate) struct PendingLine {
    pub win: WinId,
    pub kind: LineKind,
    /// Texte sans l'horodatage.
    pub text: String,
    pub nick: Option<String>,
    pub highlight: bool,
    pub time_ms: i64,
    pub from_history: bool,
    pub msgid: Option<String>,
    /// Ce que la ligne fait à la switchbar si la fenêtre n'est pas active.
    pub activity: Activity,
    pub color: Option<u8>,
    /// Ne pas écrire dans le journal (`/echo -g`).
    pub nolog: bool,
    /// Sans horodatage (`/echo` sans `-t`, lignes des fenêtres `@`).
    pub no_stamp: bool,
}

/// Un ban qui attend l'adresse du pseudo (USERHOST).
#[derive(Debug, Clone)]
pub(crate) struct PendingBan {
    pub chan: String,
    pub nick: String,
    pub mask_type: u8,
    pub kick: Option<String>,
    pub unban_secs: Option<u64>,
}

/// Un auto-op/auto-voice différé.
#[derive(Debug, Clone)]
pub(crate) struct PendingMode {
    pub due: i64,
    pub chan: String,
    pub mode: char,
    pub nick: String,
}

/// Une connexion (une fenêtre Status).
pub(crate) struct Conn {
    pub id: ConnId,
    pub status: WinId,
    /// Nom affiché du réseau.
    pub network: String,
    /// Réseau des réglages, s'il y en a un.
    pub net_name: Option<String>,
    pub servers: Vec<Server>,
    pub server_idx: usize,
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub verify: TlsVerify,
    pub client_cert: Option<PathBuf>,
    pub state: ConnState,
    pub session: Session,
    pub flood: FloodQueue,
    pub encoding: Encoding,
    pub last_rx: i64,
    pub next_lag_ping: i64,
    pub lag_sent: Option<i64>,
    pub lag_ms: Option<i64>,
    /// `/lag` en attente d'affichage.
    pub lag_requested: Option<WinId>,
    /// Déconnexion voulue (pas de reconnexion).
    pub user_quit: bool,
    pub reconnect_at: Option<i64>,
    pub attempts: u32,
    pub rejoin: Vec<(String, Option<String>)>,
    /// Pseudos dont le message d'absence a déjà été montré (pseudo replié → message).
    pub away_shown: HashMap<String, String>,
    pub ctcp_times: VecDeque<i64>,
    pub notify_online: HashMap<String, bool>,
    pub next_ison: Option<i64>,
    pub monitor_sent: bool,
    pub bouncer_parent: Option<ConnId>,
    pub bouncer_netid: Option<String>,
    pub bouncer_children: BTreeMap<String, ConnId>,
    pub bouncer_names: BTreeMap<String, String>,
    pub bouncer_batch: bool,
    pub pending_bans: Vec<PendingBan>,
    pub pending_modes: Vec<PendingMode>,
    pub timed_unbans: Vec<(i64, String, String)>,
    /// Fenêtre où va la réponse WHOIS en cours.
    pub whois_win: Option<WinId>,
    pub list_win: Option<WinId>,
    /// Réponse STS à appliquer : reconnexion chiffrée sur ce port.
    pub sts_upgrade: Option<u16>,
    pub ready: bool,
    /// Pseudo choisi par `/nick` hors connexion.
    pub nick_override: Option<String>,
    /// Fins de connexion encore à venir pour des connexions que nous avons fermées nous-mêmes : l'hôte les
    /// signale (ou des lignes de l'ancienne connexion arrivent encore) ; on les ignore.
    pub stale_events: u32,
}

impl Conn {
    pub fn new(id: ConnId, status: WinId, nick: &str, now: i64, burst: u32) -> Conn {
        let cfg = SessionConfig { nick: nick.to_string(), ..SessionConfig::default() };
        Conn {
            id,
            status,
            network: String::new(),
            net_name: None,
            servers: Vec::new(),
            server_idx: 0,
            host: String::new(),
            port: 6667,
            tls: false,
            verify: TlsVerify::Full,
            client_cert: None,
            state: ConnState::Disconnected,
            session: Session::new(cfg),
            flood: FloodQueue::new(burst, now),
            encoding: Encoding::Auto,
            last_rx: now,
            next_lag_ping: now,
            lag_sent: None,
            lag_ms: None,
            lag_requested: None,
            user_quit: false,
            reconnect_at: None,
            attempts: 0,
            rejoin: Vec::new(),
            away_shown: HashMap::new(),
            ctcp_times: VecDeque::new(),
            notify_online: HashMap::new(),
            next_ison: None,
            monitor_sent: false,
            bouncer_parent: None,
            bouncer_netid: None,
            bouncer_children: BTreeMap::new(),
            bouncer_names: BTreeMap::new(),
            bouncer_batch: false,
            pending_bans: Vec::new(),
            pending_modes: Vec::new(),
            timed_unbans: Vec::new(),
            whois_win: None,
            list_win: None,
            sts_upgrade: None,
            ready: false,
            nick_override: None,
            stale_events: 0,
        }
    }

    pub fn connected(&self) -> bool {
        matches!(self.state, ConnState::Registering | ConnState::Connected)
    }
}

pub(crate) struct State {
    pub settings: Settings,
    pub data_dir: PathBuf,
    pub now: i64,
    pub lang: Lang,
    pub system_locale: Option<String>,
    /// Décalage horaire imposé (tests) ; sinon celui du système.
    pub utc_offset: Option<i32>,
    pub windows: Vec<Window>,
    pub active: Option<WinId>,
    pub next_win: WinId,
    pub next_seq: u64,
    pub conns: BTreeMap<ConnId, Conn>,
    pub next_conn: ConnId,
    pub effects: Vec<Effect>,
    pub deferred: VecDeque<Deferred>,
    pub urls: Vec<CaughtUrl>,
    pub chanlists: HashMap<WinId, ChanList>,
    pub logger: Logger,
    pub app_focused: bool,
    pub notify_seen: HashMap<String, i64>,
    pub dialogs_version: u64,
    pub log_off: HashSet<WinId>,
    pub last_input: i64,
    /// Sorties de la ligne en cours supprimées (`raw` avec `/haltdef`).
    pub cur_halted: bool,
    /// Lignes écrites par les scripts pendant un événement : ajoutées après l'affichage par défaut de
    /// l'événement (comme les gestionnaires sans `^` de mIRC, qui s'exécutent après l'affichage).
    pub capture: Option<Vec<PendingLine>>,
    /// Empreinte des scripts, variables et utilisateurs déjà enregistrés (pour `Effect::SaveScripts`).
    pub scripts_fp: u64,
    /// Nonce SCRAM imposé (tests).
    pub scram_nonce: Option<String>,
    /// Transferts et discussions DCC (voir `dcc.rs`).
    pub dcc: super::dcc::DccState,
}

impl State {
    // ---------------------------------------------------------------------------------------------------------
    // Langue, heure
    // ---------------------------------------------------------------------------------------------------------

    pub fn apply_language(&mut self) {
        self.lang = self
            .settings
            .language
            .as_deref()
            .and_then(Lang::from_locale)
            .or_else(|| self.system_locale.as_deref().and_then(Lang::from_locale))
            .unwrap_or(Lang::En);
    }

    pub fn t(&self, key: Key, args: &[(&str, &str)]) -> String {
        i18n::trf(self.lang, key, args)
    }

    pub fn offset(&self, ms: i64) -> i32 {
        self.utc_offset.unwrap_or_else(|| timeutil::local_offset(ms))
    }

    pub fn asctime(&self, ms: i64) -> String {
        timeutil::asctime(ms, self.offset(ms))
    }

    pub fn stamp(&self, ms: i64) -> String {
        timeutil::format_time(ms, self.offset(ms), &self.settings.irc.timestamp_format)
    }

    // ---------------------------------------------------------------------------------------------------------
    // Fenêtres
    // ---------------------------------------------------------------------------------------------------------

    pub fn win(&self, id: WinId) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    pub fn win_mut(&mut self, id: WinId) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    fn names_eq(&self, conn: Option<ConnId>, a: &str, b: &str) -> bool {
        match conn.and_then(|c| self.conns.get(&c)) {
            Some(c) => c.session.eq(a, b),
            None => alpirc_proto::CaseMapping::Rfc1459.eq(a, b),
        }
    }

    /// Fenêtre d'un nom pour une connexion (les fenêtres `@` sont communes à toutes).
    pub fn find_win(&self, conn: Option<ConnId>, name: &str) -> Option<WinId> {
        if name.starts_with('@') {
            return self.windows.iter().find(|w| w.kind == WinKind::Custom && w.name.eq_ignore_ascii_case(name)).map(|w| w.id);
        }
        if (name.eq_ignore_ascii_case("status window") || name.eq_ignore_ascii_case("status"))
            && let Some(c) = conn.and_then(|c| self.conns.get(&c))
        {
            return Some(c.status);
        }
        self.windows
            .iter()
            .find(|w| w.conn == conn && w.kind != WinKind::Status && self.names_eq(conn, &w.name, name))
            .or_else(|| self.windows.iter().find(|w| w.conn == conn && self.names_eq(conn, &w.name, name)))
            .map(|w| w.id)
    }

    pub fn find_kind(&self, conn: ConnId, kind: WinKind, name: &str) -> Option<WinId> {
        self.windows.iter().find(|w| w.conn == Some(conn) && w.kind == kind && self.names_eq(Some(conn), &w.name, name)).map(|w| w.id)
    }

    pub fn create_window(&mut self, kind: WinKind, conn: Option<ConnId>, name: &str) -> WinId {
        let id = self.next_win;
        self.next_win += 1;
        let mut w = Window::new(id, kind, conn, name);
        if kind == WinKind::Custom {
            w.editbox = false;
        }
        self.windows.push(w);
        if self.active.is_none() {
            self.active = Some(id);
        }
        self.reload_log(id);
        id
    }

    /// Fenêtre Status d'une connexion.
    pub fn status_of(&self, conn: ConnId) -> Option<WinId> {
        self.conns.get(&conn).map(|c| c.status)
    }

    /// La fenêtre active si elle appartient à la connexion, sinon sa fenêtre Status.
    pub fn active_for(&self, conn: ConnId) -> WinId {
        match self.active.and_then(|a| self.win(a)) {
            Some(w) if w.conn == Some(conn) && !matches!(w.kind, WinKind::ChanList) => w.id,
            _ => self.status_of(conn).or(self.active).unwrap_or(0),
        }
    }

    /// Active une fenêtre de la part du moteur (et le dit à l'interface).
    pub fn activate(&mut self, win: WinId) {
        if self.win(win).is_none() {
            return;
        }
        if let Some(old) = self.active
            && old != win
            && let Some(w) = self.win_mut(old)
        {
            w.unread_marker = None;
            w.touch();
        }
        self.active = Some(win);
        if let Some(w) = self.win_mut(win) {
            w.activity = Activity::None;
            w.touch();
        }
        self.effects.push(Effect::Activate(win));
    }

    /// Retire une fenêtre (journal fermé, fenêtre active déplacée).
    pub fn remove_window(&mut self, win: WinId) {
        let Some(i) = self.windows.iter().position(|w| w.id == win) else { return };
        if let Some(p) = self.log_path(win) {
            let date = self.asctime(self.now);
            self.logger.close(&p, &date);
        }
        let w = self.windows.remove(i);
        if w.kind == WinKind::DccChat {
            self.dcc_window_removed(win);
        }
        self.chanlists.remove(&win);
        self.log_off.remove(&win);
        for c in self.conns.values_mut() {
            if c.whois_win == Some(win) {
                c.whois_win = None;
            }
            if c.list_win == Some(win) {
                c.list_win = None;
            }
            if c.lag_requested == Some(win) {
                c.lag_requested = Some(c.status);
            }
        }
        if self.active == Some(win) {
            let next = w
                .conn
                .and_then(|c| self.status_of(c))
                .filter(|s| self.win(*s).is_some())
                .or_else(|| self.windows.first().map(|w| w.id));
            self.active = next;
            if let Some(n) = next {
                self.effects.push(Effect::Activate(n));
            }
        }
    }

    // ---------------------------------------------------------------------------------------------------------
    // Lignes
    // ---------------------------------------------------------------------------------------------------------

    pub fn pl(&self, win: WinId, kind: LineKind, text: impl Into<String>) -> PendingLine {
        PendingLine {
            win,
            kind,
            text: text.into(),
            nick: None,
            highlight: false,
            time_ms: self.now,
            from_history: false,
            msgid: None,
            activity: Activity::Event,
            color: None,
            nolog: false,
            no_stamp: false,
        }
    }

    /// Écrit tout de suite une ligne d'information.
    pub fn print(&mut self, win: WinId, kind: LineKind, text: impl Into<String>) {
        let l = self.pl(win, kind, text);
        self.add_line(l);
    }

    pub fn add_line(&mut self, pl: PendingLine) {
        if let Some(c) = self.capture.as_mut() {
            c.push(pl);
            return;
        }
        let ts = if self.settings.irc.timestamps && !pl.no_stamp { format!("{} ", self.stamp(pl.time_ms)) } else { String::new() };
        let max = self.settings.irc.max_lines.max(10);
        let seq = self.next_seq;
        let active = self.active == Some(pl.win);
        let Some(w) = self.windows.iter_mut().find(|w| w.id == pl.win) else { return };
        if pl.from_history
            && w.lines.iter().any(|l| {
                (pl.msgid.is_some() && l.msgid == pl.msgid) || (l.time_ms == pl.time_ms && l.nick == pl.nick && l.text.ends_with(&pl.text))
            })
        {
            return;
        }
        self.next_seq += 1;
        let kind = if pl.highlight { LineKind::Highlight } else { pl.kind };
        let line = Line {
            seq,
            time_ms: pl.time_ms,
            kind,
            text: format!("{ts}{}", pl.text),
            nick: pl.nick.clone(),
            highlight: pl.highlight,
            from_history: pl.from_history,
            msgid: pl.msgid.clone(),
            color: pl.color,
        };
        if pl.from_history {
            let pos = w.lines.iter().position(|l| l.time_ms > pl.time_ms).unwrap_or(w.lines.len());
            w.lines.insert(pos, line);
        } else {
            w.lines.push_back(line);
        }
        while w.lines.len() > max {
            w.lines.pop_front();
        }
        let counts = !pl.from_history || w.read_marker_ms.is_some_and(|m| pl.time_ms > m);
        if !active && counts && pl.activity > w.activity {
            w.activity = pl.activity;
        }
        if !active && counts && pl.activity >= Activity::Message && w.unread_marker.is_none() {
            w.unread_marker = Some(seq);
        }
        w.touch();
        if !pl.from_history && !pl.nolog {
            self.log_line(pl.win, &pl.text, pl.time_ms);
        }
    }

    // ---------------------------------------------------------------------------------------------------------
    // Journaux
    // ---------------------------------------------------------------------------------------------------------

    pub fn log_folder(&self) -> PathBuf {
        self.settings.logging.folder.clone().unwrap_or_else(|| self.data_dir.join("logs"))
    }

    pub fn log_path(&self, win: WinId) -> Option<PathBuf> {
        let w = self.win(win)?;
        let net = match w.conn.and_then(|c| self.conns.get(&c)) {
            Some(c) if !c.network.is_empty() => c.network.clone(),
            Some(c) if !c.host.is_empty() => c.host.clone(),
            _ => "local".to_string(),
        };
        Some(Logger::path(&self.log_folder(), &net, &w.name))
    }

    fn should_log(&self, win: WinId) -> bool {
        let lo = &self.settings.logging;
        if !lo.enabled || self.log_off.contains(&win) {
            return false;
        }
        match self.win(win).map(|w| w.kind) {
            Some(WinKind::Channel) => lo.channels,
            Some(WinKind::Query) | Some(WinKind::DccChat) => lo.queries,
            Some(WinKind::Status) => lo.status,
            _ => false,
        }
    }

    fn log_line(&mut self, win: WinId, text: &str, time: i64) {
        if !self.should_log(win) {
            return;
        }
        let Some(path) = self.log_path(win) else { return };
        let mut t = if self.settings.logging.strip_codes { alpirc_proto::format::strip(text) } else { text.to_string() };
        if self.settings.logging.timestamps {
            t = format!("{} {t}", self.stamp(time));
        }
        let date = self.asctime(self.now);
        self.logger.write(&path, &t, &date);
    }

    /// Remet les dernières lignes du journal dans une fenêtre qui s'ouvre (`reload_lines`).
    fn reload_log(&mut self, win: WinId) {
        let n = self.settings.logging.reload_lines;
        if n == 0 || !self.should_log(win) {
            return;
        }
        let Some(path) = self.log_path(win) else { return };
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty() && !l.starts_with("Session ")).collect();
        let start = lines.len().saturating_sub(n);
        let now = self.now;
        if let Some(w) = self.win_mut(win) {
            for l in &lines[start..] {
                w.lines.push_back(Line {
                    seq: 0,
                    time_ms: now,
                    kind: LineKind::Other,
                    text: l.to_string(),
                    nick: None,
                    highlight: false,
                    from_history: true,
                    msgid: None,
                    color: Some(14),
                });
            }
        }
        for i in 0..self.windows.len() {
            if self.windows[i].id == win {
                for l in self.windows[i].lines.iter_mut() {
                    if l.seq == 0 {
                        l.seq = self.next_seq;
                        self.next_seq += 1;
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------------------------------------------------
    // Envoi
    // ---------------------------------------------------------------------------------------------------------

    /// Met une ligne dans la file d'envoi (protégée contre le flood).
    pub fn send_line(&mut self, conn: ConnId, line: &str) {
        let line: String = line.chars().filter(|c| *c != '\r' && *c != '\n' && *c != '\0').collect();
        if line.trim().is_empty() {
            return;
        }
        let Some(c) = self.conns.get_mut(&conn) else { return };
        let bypass = !c.session.is_registered() || is_priority(&line);
        c.flood.push(line, bypass);
    }

    pub fn send_msg(&mut self, conn: ConnId, m: Message) {
        let l = m.to_line();
        self.send_line(conn, &l);
    }

    /// Ligne envoyée sans attendre (lag PING, QUIT).
    pub fn send_now(&mut self, conn: ConnId, line: &str) {
        if let Some(c) = self.conns.get_mut(&conn) {
            c.flood.push(line.to_string(), true);
        }
    }

    /// Ce que la session veut envoyer.
    pub fn drain_session(&mut self, conn: ConnId) {
        let Some(c) = self.conns.get_mut(&conn) else { return };
        let out = c.session.take_outgoing();
        let registered = c.session.is_registered();
        for m in out {
            let l = m.to_line();
            let bypass = !registered || is_priority(&l);
            c.flood.push(l, bypass);
        }
    }

    /// Demande à l'hôte de fermer la connexion (sa fin, quand elle sera signalée, sera ignorée).
    pub fn close_effect(&mut self, conn: ConnId) {
        self.effects.push(Effect::Close { conn });
        if let Some(c) = self.conns.get_mut(&conn) {
            c.stale_events += 1;
        }
    }

    /// Fait partir ce que la file permet.
    pub fn flush_sends(&mut self) {
        let fl = self.settings.flood.clone();
        let now = self.now;
        let mut sends = Vec::new();
        for c in self.conns.values_mut() {
            if !c.connected() {
                continue;
            }
            for l in c.flood.drain(now, fl.enabled, fl.burst, fl.rate_ms) {
                sends.push(Effect::Send { conn: c.id, line: l });
            }
        }
        self.effects.extend(sends);
    }

    // ---------------------------------------------------------------------------------------------------------
    // Divers
    // ---------------------------------------------------------------------------------------------------------

    pub fn push_pres(&mut self, mut p: Pres) {
        if self.cur_halted {
            p.outs.retain(|o| !matches!(o, Out::Line(_)));
        }
        self.deferred.push_back(Deferred::Pres(p));
    }

    pub fn ctx(&self, conn: Option<ConnId>, win: Option<WinId>) -> Ctx {
        Ctx { conn, window: win.and_then(|w| self.win(w)).map(|w| w.name.clone()), selected: Vec::new(), event: None }
    }

    /// Événement de script, champs courants remplis.
    pub fn ev(&self, kind: EventKind, conn: ConnId) -> EventInfo {
        EventInfo { kind: Some(kind), conn: Some(conn), ..EventInfo::default() }
    }

    pub fn me(&self, conn: ConnId) -> String {
        self.conns.get(&conn).map(|c| c.session.nick().to_string()).unwrap_or_else(|| self.settings.identity.nick.clone())
    }

    /// Liste des pseudos d'une fenêtre de canal, triée comme dans mIRC.
    pub fn nicklist(&self, win: WinId) -> Vec<NickEntry> {
        let Some(w) = self.win(win) else { return Vec::new() };
        let Some(conn) = w.conn.and_then(|c| self.conns.get(&c)) else { return Vec::new() };
        let s = &conn.session;
        let Some(ch) = s.channel(&w.name) else { return Vec::new() };
        let is = s.isupport();
        let mut v: Vec<(usize, String, NickEntry)> = ch
            .members
            .values()
            .map(|m| {
                let rank = m.modes.chars().filter_map(|c| is.rank(c)).min().unwrap_or(usize::MAX);
                let prefixes: String = m.modes.chars().filter_map(|c| is.symbol_for_mode(c)).collect();
                let u = s.user_info(&m.nick);
                (
                    rank,
                    s.fold(&m.nick),
                    NickEntry {
                        nick: m.nick.clone(),
                        prefixes,
                        away: u.is_some_and(|u| u.away),
                        account: u.and_then(|u| u.account.clone()),
                    },
                )
            })
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        v.into_iter().map(|x| x.2).collect()
    }

    /// Recalcule les titres des fenêtres d'une connexion.
    pub fn update_titles(&mut self, conn: ConnId) {
        let Some(c) = self.conns.get(&conn) else { return };
        let mut titles: Vec<(WinId, String)> = Vec::new();
        for w in self.windows.iter().filter(|w| w.conn == Some(conn)) {
            let t = match w.kind {
                WinKind::Status => {
                    if c.connected() {
                        let modes = if c.session.user_modes().is_empty() { String::new() } else { format!("+{}", c.session.user_modes()) };
                        let net = if c.network.is_empty() { c.host.clone() } else { c.network.clone() };
                        self.t(
                            Key::TitleStatus,
                            &[("nick", c.session.nick()), ("modes", &modes), ("network", &net), ("server", &c.host), ("port", &c.port.to_string())],
                        )
                        .replace(" []", "")
                    } else {
                        self.t(Key::TitleStatusOffline, &[("nick", c.session.nick())])
                    }
                }
                WinKind::Channel => match c.session.channel(&w.name) {
                    Some(ch) => {
                        let modes = ch.mode_string(true);
                        let mut t = if modes.is_empty() { w.name.clone() } else { format!("[{modes}] {}", w.name) };
                        if let Some(tp) = &ch.topic {
                            t.push_str(": ");
                            t.push_str(tp);
                        }
                        t
                    }
                    None => w.name.clone(),
                },
                WinKind::Query => match c.session.user_info(&w.name).and_then(|u| u.address()) {
                    Some(a) => format!("{} ({a})", w.name),
                    None => w.name.clone(),
                },
                WinKind::ChanList => {
                    let n = self.chanlists.get(&w.id).map(|l| l.entries.len()).unwrap_or(0);
                    format!("{} ({n})", self.t(Key::TitleChanList, &[]))
                }
                _ => w.name.clone(),
            };
            if t != w.title {
                titles.push((w.id, t));
            }
        }
        for (id, t) in titles {
            if let Some(w) = self.win_mut(id) {
                w.title = t;
                w.touch();
            }
        }
    }

    /// Les fenêtres de canal d'une connexion changent (liste des pseudos) : l'interface redessine.
    pub fn touch_channels(&mut self, conn: ConnId, chans: &[String]) {
        let ids: Vec<WinId> = chans.iter().filter_map(|c| self.find_kind(conn, WinKind::Channel, c)).collect();
        for id in ids {
            if let Some(w) = self.win_mut(id) {
                w.touch();
            }
        }
    }

    pub fn conn_encoding(&self, conn: ConnId) -> Encoding {
        self.conns.get(&conn).map(|c| c.encoding).unwrap_or_default()
    }

    /// URL catcher.
    pub fn catch_urls(&mut self, conn: ConnId, nick: &str, window: &str, text: &str, time: i64) {
        if !self.settings.catcher.enabled {
            return;
        }
        for url in super::util::find_urls(text) {
            self.urls.retain(|u| u.url != url);
            self.urls.push(CaughtUrl { url, nick: nick.to_string(), window: window.to_string(), conn: Some(conn), time_ms: time });
        }
        let max = self.settings.catcher.max.max(1);
        if self.urls.len() > max {
            let extra = self.urls.len() - max;
            self.urls.drain(..extra);
        }
    }
}

/// Lignes qui passent avant la file (réponses au serveur, négociation).
pub(crate) fn is_priority(line: &str) -> bool {
    let cmd = line.split(' ').next().unwrap_or("").to_ascii_uppercase();
    matches!(cmd.as_str(), "PONG" | "CAP" | "AUTHENTICATE" | "PASS" | "BOUNCER")
}
