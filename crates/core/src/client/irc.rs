//! Ce que le client fait des lignes du serveur : la session met l'état à jour, puis chaque événement devient une
//! « présentation » (événements de script + affichage par défaut, à la manière de mIRC 6), plus les automatismes
//! (réponses CTCP, auto-op, Notify, bouncer, historique, marqueurs de lecture).

use alpirc_proto::{Message, ctcp, format, mask, numeric as n};
use alpirc_script::EventKind;

use super::state::{PendingLine, Pres};
use super::window::{Activity, LineKind, WinKind};
use super::{ChanEntry, ChanList, ConnId, ConnState, Effect, WinId};
use crate::i18n::Key;
use crate::session::{MsgKind, ReplyKind, SessionEvent as SE};
use crate::settings::{ControlKind, SoundEvent};
use crate::timeutil;

/// Sortes d'ignorance (`/ignore -pcntikd`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IgnoreType {
    Private,
    Channel,
    Notice,
    Ctcp,
    Invite,
    Dcc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ignored {
    No,
    StripCodes,
    Drop,
}

/// Mot surligné trouvé.
#[derive(Debug, Clone)]
pub(crate) struct Hit {
    pub color: Option<u8>,
    pub beep: bool,
    pub flash: bool,
    pub notify: bool,
}

/// Version d'AlpIRC.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Adresse du projet.
pub const HOMEPAGE: &str = "https://github.com/Guiwald/alpirc";

/// Réponse CTCP VERSION par défaut.
pub fn version_reply() -> String {
    format!("AlpIRC {VERSION} (Rust) {HOMEPAGE}")
}

impl super::state::State {
    /// Lit une ligne du serveur.
    pub(crate) fn handle_message(&mut self, conn: ConnId, msg: &Message, raw_halted: bool) {
        let now = self.now;
        let time = crate::session::message_time(msg, now);
        let events = match self.conns.get_mut(&conn) {
            Some(c) => c.session.handle(msg, now),
            None => return,
        };
        let suppress = events.iter().any(|e| e.suppresses_numeric() || matches!(e, SE::NickRetry { .. }));
        self.cur_halted = raw_halted;
        for e in events {
            self.on_event(conn, msg, e, time);
        }
        if n::is_numeric(&msg.command) && !suppress {
            self.show_numeric(conn, msg, time);
        }
        self.cur_halted = false;
        self.drain_session(conn);
        self.update_titles(conn);
    }

    fn chan_win(&self, conn: ConnId, chan: &str) -> Option<WinId> {
        self.find_kind(conn, WinKind::Channel, chan)
    }

    fn query_win(&self, conn: ConnId, nick: &str) -> Option<WinId> {
        self.find_kind(conn, WinKind::Query, nick)
    }

    fn status(&self, conn: ConnId) -> WinId {
        self.status_of(conn).unwrap_or(0)
    }

    fn line_at(&self, win: WinId, kind: LineKind, text: String, time: i64) -> PendingLine {
        let mut l = self.pl(win, kind, text);
        l.time_ms = time;
        l
    }

    fn conn_network(&self, conn: ConnId) -> String {
        self.conns.get(&conn).map(|c| if c.network.is_empty() { c.host.clone() } else { c.network.clone() }).unwrap_or_default()
    }

    /// Le message d'un utilisateur est-il ignoré ?
    pub(crate) fn ignored(&self, conn: ConnId, nick: &str, address: &str, ty: IgnoreType) -> Ignored {
        let Some(c) = self.conns.get(&conn) else { return Ignored::No };
        let full = format!("{nick}!{address}");
        let cm = c.session.casemapping();
        let net = self.conn_network(conn);
        let mut out = Ignored::No;
        for e in &self.settings.address_book.ignore {
            if e.until_ms.is_some_and(|u| u <= self.now) {
                continue;
            }
            if let Some(n) = &e.network
                && !n.eq_ignore_ascii_case(&net)
                && c.net_name.as_deref().is_none_or(|x| !x.eq_ignore_ascii_case(n))
            {
                continue;
            }
            let m = if e.mask.contains(['!', '@']) { e.mask.clone() } else { format!("{}!*@*", e.mask) };
            if !mask::wildcard_match(&m, &full, cm) {
                continue;
            }
            let hit = match ty {
                IgnoreType::Private => e.private,
                IgnoreType::Channel => e.channel,
                IgnoreType::Notice => e.notice,
                IgnoreType::Ctcp => e.ctcp,
                IgnoreType::Invite => e.invite,
                IgnoreType::Dcc => e.dcc,
            };
            if hit {
                return Ignored::Drop;
            }
            if e.codes {
                out = Ignored::StripCodes;
            }
        }
        out
    }

    /// Notre pseudo ou un mot surligné dans le texte ?
    pub(crate) fn highlight(&self, conn: ConnId, text: &str) -> Option<Hit> {
        let h = &self.settings.highlight;
        if !h.enabled {
            return None;
        }
        let me = self.me(conn);
        if h.nick && super::util::contains_word(text, &me) {
            return Some(Hit { color: None, beep: h.beep, flash: h.flash, notify: h.notify_on_highlight });
        }
        for w in &h.words {
            if super::util::contains_word(text, &w.text) {
                return Some(Hit { color: w.color, beep: w.beep || h.beep, flash: w.flash || h.flash, notify: w.notify || h.notify_on_highlight });
            }
        }
        None
    }

    /// La fenêtre est-elle hors de vue (pas active, ou appli sans le focus) ?
    pub(crate) fn unseen(&self, win: WinId) -> bool {
        self.active != Some(win) || !self.app_focused
    }

    /// Effets d'une alerte (surlignage, privé) : bip, clignotement, notification. « Tout couper » (Options › Sons)
    /// fait taire le bip.
    fn alert_effects(&self, win: WinId, beep: bool, flash: bool, notify: bool, title: String, body: &str) -> Vec<Effect> {
        let mut v = Vec::new();
        let unseen = self.unseen(win);
        if unseen && beep && !self.settings.sounds.mute {
            v.push(Effect::Beep);
        }
        if unseen && flash {
            v.push(Effect::Flash { win });
        }
        let n = &self.settings.notifications;
        if notify && n.enabled && (unseen || !n.only_when_inactive) {
            v.push(Effect::Notify { win, title, body: format::strip(body) });
        }
        v
    }

    /// `<nick>` (avec le préfixe de mode et la couleur du pseudo si les options le veulent).
    fn nick_tag(&self, conn: ConnId, chan: Option<&str>, nick: &str) -> String {
        let mut prefix = String::new();
        if self.settings.irc.show_mode_prefix
            && let (Some(ch), Some(c)) = (chan, self.conns.get(&conn))
            && let Some(m) = c.session.member_modes(ch, nick)
            && let Some(first) = m.chars().next()
            && let Some(sym) = c.session.isupport().symbol_for_mode(first)
        {
            prefix.push(sym);
        }
        if self.settings.irc.nick_colors {
            format!("{prefix}\x03{:02}{nick}\x03", super::util::nick_color(nick))
        } else {
            format!("{prefix}{nick}")
        }
    }

    fn on_event(&mut self, conn: ConnId, msg: &Message, e: SE, time: i64) {
        let now = self.now;
        match e {
            SE::Registered { .. } => {
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.state = ConnState::Connected;
                    c.attempts = 0;
                    c.reconnect_at = None;
                    c.next_lag_ping = now + self.settings.connect.lag_interval_secs as i64 * 1000;
                }
            }
            SE::NickRetry { failed, next } => {
                let st = self.status(conn);
                let t = self.t(Key::NickRetry, &[("nick", &failed), ("next", &next)]);
                self.print(st, LineKind::Info, t);
            }
            SE::IsupportChanged => {
                if let Some(c) = self.conns.get_mut(&conn)
                    && c.bouncer_netid.is_none()
                    && let Some(n) = c.session.isupport().network.clone()
                {
                    c.network = n;
                }
            }
            SE::MotdEnd => self.on_ready(conn),
            SE::Sts(info) => self.on_sts(conn, info),
            SE::SaslFailed { reason } => {
                let st = self.status(conn);
                let t = self.t(Key::SaslFailed, &[("reason", &reason)]);
                self.print(st, LineKind::Error, t);
            }
            SE::UserMode { setter, modes, .. } => {
                let st = self.status(conn);
                let t = self.t(Key::SetsMode, &[("nick", &setter), ("modes", &modes)]);
                let mut ev = self.ev(EventKind::UserMode, conn);
                ev.nick = setter;
                ev.text = modes;
                let p = Pres::new().event(self.ctx(Some(conn), Some(st)), ev).line(self.line_at(st, LineKind::Mode, t, time));
                self.push_pres(p);
            }
            SE::Join { channel, nick, address, own, .. } => self.on_join(conn, msg, channel, nick, address, own, time),
            SE::Part { channel, nick, address, reason, own } => {
                let win = self.chan_win(conn, &channel);
                let mut ev = self.ev(EventKind::Part, conn);
                ev.nick = nick.clone();
                ev.address = address.clone().unwrap_or_default();
                ev.chan = Some(channel.clone());
                ev.target = Some(channel.clone());
                ev.text = reason.clone().unwrap_or_default();
                let ctx = self.ctx(Some(conn), win);
                let mut p = Pres::new().event(ctx, ev);
                if own {
                    if let Some(w) = win {
                        self.remove_window(w);
                    }
                } else if let Some(w) = win {
                    if self.settings.irc.show_joins_parts {
                        let a = address.unwrap_or_default();
                        let t = match &reason {
                            Some(r) => self.t(Key::PartsReason, &[("nick", &nick), ("address", &a), ("reason", r)]),
                            None => self.t(Key::Parts, &[("nick", &nick), ("address", &a)]),
                        };
                        p = p.line(self.line_at(w, LineKind::Part, t, time));
                    }
                    if let Some(x) = self.win_mut(w) {
                        x.touch();
                    }
                }
                self.push_pres(p);
            }
            SE::Kick { channel, by, by_address, target, reason, own } => {
                let win = self.chan_win(conn, &channel);
                let r = reason.clone().unwrap_or_default();
                let mut ev = self.ev(EventKind::Kick, conn);
                ev.nick = by.clone();
                ev.address = by_address.unwrap_or_default();
                ev.chan = Some(channel.clone());
                ev.target = Some(channel.clone());
                ev.text = r.clone();
                ev.extra.push(("knick".into(), target.clone()));
                let ctx = self.ctx(Some(conn), win);
                let mut p = Pres::new().event(ctx, ev);
                if own {
                    let t = self.t(Key::YouKicked, &[("chan", &channel), ("by", &by), ("reason", &r)]);
                    let keep = self.settings.irc.keep_channels_open;
                    match win {
                        Some(w) if keep => {
                            if let Some(x) = self.win_mut(w) {
                                x.joined = false;
                                x.touch();
                            }
                            p = p.line(self.line_at(w, LineKind::Kick, t, time));
                        }
                        _ => {
                            if let Some(w) = win {
                                self.remove_window(w);
                            }
                            let st = self.status(conn);
                            p = p.line(self.line_at(st, LineKind::Kick, t, time));
                        }
                    }
                    if self.settings.irc.rejoin_on_kick {
                        p = p.send(conn, format!("JOIN {channel}"));
                    }
                    if let Some(fx) = self.event_sound(SoundEvent::Kick) {
                        p = p.effect(fx);
                    }
                } else if let Some(w) = win {
                    let t = self.t(Key::Kicked, &[("nick", &target), ("by", &by), ("reason", &r)]);
                    p = p.line(self.line_at(w, LineKind::Kick, t, time));
                    if let Some(x) = self.win_mut(w) {
                        x.touch();
                    }
                }
                self.push_pres(p);
            }
            SE::Quit { nick, address, reason, channels } => {
                let a = address.clone().unwrap_or_default();
                let mut ev = self.ev(EventKind::Quit, conn);
                ev.nick = nick.clone();
                ev.address = a.clone();
                ev.text = reason.clone().unwrap_or_default();
                let st = self.status(conn);
                let mut p = Pres::new().event(self.ctx(Some(conn), Some(st)), ev);
                let t = match &reason {
                    Some(r) => self.t(Key::QuitsReason, &[("nick", &nick), ("address", &a), ("reason", r)]),
                    None => self.t(Key::Quits, &[("nick", &nick), ("address", &a)]),
                };
                let mut wins: Vec<WinId> = channels.iter().filter_map(|c| self.chan_win(conn, c)).collect();
                if let Some(q) = self.query_win(conn, &nick) {
                    wins.push(q);
                }
                if self.settings.irc.show_quits {
                    for w in &wins {
                        p = p.line(self.line_at(*w, LineKind::Quit, t.clone(), time));
                    }
                }
                self.touch_channels(conn, &channels);
                if let Some(c) = self.conns.get_mut(&conn) {
                    let k = c.session.fold(&nick);
                    c.away_shown.remove(&k);
                }
                self.push_pres(p);
            }
            SE::Nick { old, new, address, channels, own } => {
                let mut ev = self.ev(EventKind::Nick, conn);
                ev.nick = old.clone();
                ev.address = address.unwrap_or_default();
                ev.extra.push(("newnick".into(), new.clone()));
                ev.text = new.clone();
                let st = self.status(conn);
                let mut p = Pres::new().event(self.ctx(Some(conn), Some(st)), ev);
                let mut wins: Vec<WinId> = channels.iter().filter_map(|c| self.chan_win(conn, c)).collect();
                if let Some(q) = self.query_win(conn, &old) {
                    if self.query_win(conn, &new).is_none_or(|x| x == q)
                        && let Some(w) = self.win_mut(q)
                    {
                        w.name = new.clone();
                        w.touch();
                    }
                    wins.push(q);
                }
                let t = if own {
                    wins.insert(0, st);
                    self.t(Key::OwnNick, &[("new", &new)])
                } else {
                    self.t(Key::NickChange, &[("nick", &old), ("new", &new)])
                };
                if own || self.settings.irc.show_nick_changes {
                    for w in wins {
                        p = p.line(self.line_at(w, LineKind::Nick, t.clone(), time));
                    }
                }
                self.touch_channels(conn, &channels);
                self.push_pres(p);
            }
            SE::ChannelMode { channel, setter, setter_address, modes, changes } => {
                self.on_channel_mode(conn, channel, setter, setter_address.unwrap_or_default(), modes, changes, time)
            }
            SE::Topic { channel, setter, address, topic } => {
                let win = self.chan_win(conn, &channel).unwrap_or_else(|| self.status(conn));
                let mut ev = self.ev(EventKind::Topic, conn);
                ev.nick = setter.clone();
                ev.address = address.unwrap_or_default();
                ev.chan = Some(channel.clone());
                ev.target = Some(channel.clone());
                ev.text = topic.clone();
                let t = self.t(Key::ChangesTopic, &[("nick", &setter), ("topic", &topic)]);
                let p = Pres::new().event(self.ctx(Some(conn), Some(win)), ev).line(self.line_at(win, LineKind::Topic, t, time));
                self.push_pres(p);
            }
            SE::TopicIs { channel, topic } => {
                let (win, t) = match self.chan_win(conn, &channel) {
                    Some(w) => (w, self.t(Key::TopicIs, &[("topic", &topic)])),
                    None => (self.active_for(conn), format!("{channel}: {topic}")),
                };
                let p = Pres::new().line(self.line_at(win, LineKind::Topic, t, time));
                self.push_pres(p);
            }
            SE::TopicWhoTime { channel, setter, time: when } => {
                if let Some(w) = self.chan_win(conn, &channel) {
                    let date = when.map(|s| self.date_text(s * 1000)).unwrap_or_default();
                    let t = self.t(Key::TopicSetBy, &[("nick", &setter), ("date", &date)]);
                    let p = Pres::new().line(self.line_at(w, LineKind::Topic, t, time));
                    self.push_pres(p);
                }
            }
            SE::NoTopic { channel } => {
                let win = self.chan_win(conn, &channel).unwrap_or_else(|| self.active_for(conn));
                let t = self.t(Key::NoTopic, &[("chan", &channel)]);
                let p = Pres::new().line(self.line_at(win, LineKind::Topic, t, time));
                self.push_pres(p);
            }
            SE::ChannelModeIs { channel, modes, auto } => {
                if !auto {
                    let win = self.chan_win(conn, &channel).unwrap_or_else(|| self.active_for(conn));
                    let t = self.t(Key::ChannelModes, &[("chan", &channel), ("modes", &modes)]);
                    let p = Pres::new().line(self.line_at(win, LineKind::Mode, t, time));
                    self.push_pres(p);
                }
            }
            SE::CreationTime { channel, time: when, auto } => {
                if !auto {
                    let win = self.chan_win(conn, &channel).unwrap_or_else(|| self.active_for(conn));
                    let date = self.date_text(when * 1000);
                    let t = self.t(Key::CreatedOn, &[("chan", &channel), ("date", &date)]);
                    let p = Pres::new().line(self.line_at(win, LineKind::Mode, t, time));
                    self.push_pres(p);
                }
            }
            SE::Names { channel, names, initial } => {
                if !initial {
                    let win = self.chan_win(conn, &channel).unwrap_or_else(|| self.active_for(conn));
                    let t = self.t(Key::Names, &[("chan", &channel), ("names", &names.join(" "))]);
                    let p = Pres::new().line(self.line_at(win, LineKind::Normal, t, time));
                    self.push_pres(p);
                }
            }
            SE::NamesEnd { channel, .. } => self.touch_channels(conn, &[channel]),
            SE::ListEntry { channel, entry, quiet, .. } => {
                if !quiet {
                    let win = self.chan_win(conn, &channel).unwrap_or_else(|| self.active_for(conn));
                    let date = entry.time.map(|s| self.date_text(s * 1000)).unwrap_or_default();
                    let setter = entry.setter.clone().unwrap_or_default();
                    let t = self.t(Key::ListEntryFmt, &[("chan", &channel), ("mask", &entry.mask), ("setter", &setter), ("date", &date)]);
                    let p = Pres::new().line(self.line_at(win, LineKind::Normal, t, time));
                    self.push_pres(p);
                }
            }
            SE::ListEnd { channel, .. } => self.touch_channels(conn, &[channel]),
            SE::WhoEnd { mask, .. } => self.touch_channels(conn, &[mask]),
            SE::WhoReply { .. } => {}
            SE::Userhost { entries, .. } => self.resolve_bans(conn, &entries),
            SE::Ison { nicks, auto } => {
                if auto {
                    self.on_ison(conn, &nicks);
                }
            }
            SE::Message { kind, from, address, target, text, channel, statusmsg, own, server } => {
                self.on_message(conn, msg, kind, from, address.unwrap_or_default(), target, text, channel, statusmsg, own, server, time)
            }
            SE::Invite { from, address, target, channel, own } => {
                let addr = address.unwrap_or_default();
                if own && self.ignored(conn, &from, &addr, IgnoreType::Invite) == Ignored::Drop {
                    return;
                }
                let mut ev = self.ev(EventKind::Invite, conn);
                ev.nick = from.clone();
                ev.address = addr.clone();
                ev.chan = Some(channel.clone());
                ev.target = Some(target.clone());
                let mut p;
                if own {
                    let win = self.active_for(conn);
                    let t = self.t(Key::Invites, &[("nick", &from), ("address", &addr), ("chan", &channel)]);
                    p = Pres::new().event(self.ctx(Some(conn), Some(win)), ev).line(self.line_at(win, LineKind::Invite, t, time));
                    if self.settings.irc.join_on_invite {
                        p = p.send(conn, format!("JOIN {channel}"));
                    }
                    if let Some(fx) = self.event_sound(SoundEvent::Invite) {
                        p = p.effect(fx);
                    }
                } else {
                    let win = self.chan_win(conn, &channel).unwrap_or_else(|| self.status(conn));
                    let t = self.t(Key::InviteNotify, &[("nick", &from), ("target", &target), ("chan", &channel)]);
                    p = Pres::new().line(self.line_at(win, LineKind::Invite, t, time));
                }
                self.push_pres(p);
            }
            SE::Away { nick, message } => {
                let chans = self.conns.get(&conn).map(|c| c.session.common_channels(&nick)).unwrap_or_default();
                if message.is_none()
                    && let Some(c) = self.conns.get_mut(&conn)
                {
                    let k = c.session.fold(&nick);
                    c.away_shown.remove(&k);
                }
                self.touch_channels(conn, &chans);
            }
            SE::Account { nick, .. } | SE::Chghost { nick, .. } | SE::Setname { nick, .. } => {
                let chans = self.conns.get(&conn).map(|c| c.session.common_channels(&nick)).unwrap_or_default();
                self.touch_channels(conn, &chans);
            }
            SE::Ping { token } => {
                let mut ev = self.ev(EventKind::Ping, conn);
                ev.text = token;
                ev.nick = msg.nick().to_string();
                let st = self.status(conn);
                let p = Pres::new().event(self.ctx(Some(conn), Some(st)), ev);
                self.push_pres(p);
            }
            SE::Pong { server, token } => {
                if let Some(ms) = token.strip_prefix("ALPIRC").and_then(|t| t.parse::<i64>().ok()) {
                    let mut show = None;
                    if let Some(c) = self.conns.get_mut(&conn) {
                        c.lag_ms = Some((now - ms).max(0));
                        c.lag_sent = None;
                        show = c.lag_requested.take().map(|w| (w, c.lag_ms.unwrap_or(0)));
                    }
                    if let Some((w, lag)) = show {
                        let t = self.t(Key::Lag, &[("ms", &lag.to_string())]);
                        self.print(w, LineKind::Info, t);
                    }
                    return;
                }
                let mut ev = self.ev(EventKind::Pong, conn);
                ev.nick = server;
                ev.text = token;
                let st = self.status(conn);
                let p = Pres::new().event(self.ctx(Some(conn), Some(st)), ev);
                self.push_pres(p);
            }
            SE::Error { message } => {
                let st = self.status(conn);
                let mut ev = self.ev(EventKind::Error, conn);
                ev.text = message.clone();
                let p = Pres::new().event(self.ctx(Some(conn), Some(st)), ev).line(self.line_at(st, LineKind::Error, format!("ERROR {message}"), time));
                self.push_pres(p);
            }
            SE::Wallops { from, text } => {
                let st = self.status(conn);
                let mut ev = self.ev(EventKind::Wallops, conn);
                ev.nick = from.clone();
                ev.text = text.clone();
                let p = Pres::new().event(self.ctx(Some(conn), Some(st)), ev).line(self.line_at(st, LineKind::Wallops, format!("!{from}! {text}"), time));
                self.push_pres(p);
            }
            SE::MonitorOnline(list) => {
                for (nick, addr) in list {
                    self.notify_change(conn, &nick, true, addr.as_deref().unwrap_or(""));
                }
            }
            SE::MonitorOffline(list) => {
                for nick in list {
                    self.notify_change(conn, &nick, false, "");
                }
            }
            SE::BouncerNetwork { id, attrs, deleted } => self.on_bouncer_network(conn, id, attrs, deleted),
            SE::BatchStart { kind, .. } => {
                if kind == "soju.im/bouncer-networks"
                    && let Some(c) = self.conns.get_mut(&conn)
                {
                    c.bouncer_batch = true;
                }
            }
            SE::BatchEnd { kind, .. } => {
                if kind == "soju.im/bouncer-networks" {
                    if let Some(c) = self.conns.get_mut(&conn) {
                        c.bouncer_batch = false;
                    }
                    self.print_bouncer_networks(conn);
                }
            }
            SE::ReadMarker { target, timestamp } => {
                if let Some(ts) = timestamp
                    && let Some(w) = self.find_win(Some(conn), &target)
                    && let Some(win) = self.win_mut(w)
                {
                    win.read_marker_ms = Some(ts);
                    if win.lines.iter().all(|l| l.time_ms <= ts) {
                        win.activity = Activity::None;
                        win.unread_marker = None;
                    }
                    win.touch();
                }
            }
            SE::StandardReply { kind, command, code, context, description } => {
                let win = self.active_for(conn);
                let k = match kind {
                    ReplyKind::Fail => "FAIL",
                    ReplyKind::Warn => "WARN",
                    ReplyKind::Note => "NOTE",
                };
                let ctx = if context.is_empty() { String::new() } else { format!(" {}", context.join(" ")) };
                let lk = if kind == ReplyKind::Fail { LineKind::Error } else { LineKind::Info };
                let p = Pres::new().line(self.line_at(win, lk, format!("{k} {command} {code}{ctx}: {description}"), time));
                self.push_pres(p);
            }
            SE::HistoryBatch { target, messages } => self.on_history(conn, &target, &messages),
            SE::CapsChanged { .. }
            | SE::CapEnd
            | SE::SaslSuccess
            | SE::LoggedIn { .. }
            | SE::LoggedOut
            | SE::OwnHost { .. }
            | SE::OwnAway { .. } => {}
        }
    }

    // --- arrivée sur un canal ---

    #[allow(clippy::too_many_arguments)]
    fn on_join(&mut self, conn: ConnId, _msg: &Message, channel: String, nick: String, address: Option<String>, own: bool, time: i64) {
        let addr = address.unwrap_or_default();
        let mut ev = self.ev(EventKind::Join, conn);
        ev.nick = nick.clone();
        ev.address = addr.clone();
        ev.chan = Some(channel.clone());
        ev.target = Some(channel.clone());
        if own {
            let (win, created) = match self.chan_win(conn, &channel) {
                Some(w) => (w, false),
                None => (self.create_window(WinKind::Channel, Some(conn), &channel), true),
            };
            let last = self.win(win).and_then(|w| w.last_time());
            if let Some(w) = self.win_mut(win) {
                w.joined = true;
                w.name = channel.clone();
                w.touch();
            }
            if created {
                self.activate(win);
            }
            let t = self.t(Key::NowTalking, &[("chan", &channel)]);
            let p = Pres::new().event(self.ctx(Some(conn), Some(win)), ev).line(self.line_at(win, LineKind::Join, t, time));
            self.push_pres(p);
            self.request_history(conn, &channel, last);
        } else {
            let Some(win) = self.chan_win(conn, &channel) else { return };
            let mut p = Pres::new().event(self.ctx(Some(conn), Some(win)), ev);
            if self.settings.irc.show_joins_parts {
                let t = self.t(Key::Joins, &[("nick", &nick), ("address", &addr)]);
                p = p.line(self.line_at(win, LineKind::Join, t, time));
            }
            if let Some(w) = self.win_mut(win) {
                w.touch();
            }
            self.push_pres(p);
            self.control_on_join(conn, &channel, &nick, &addr);
        }
    }

    /// Historique et marqueur de lecture d'une cible (bouncer).
    pub(crate) fn request_history(&mut self, conn: ConnId, target: &str, last: Option<i64>) {
        let Some(c) = self.conns.get(&conn) else { return };
        let hist = c.session.cap("draft/chathistory");
        let mark = c.session.cap("draft/read-marker");
        if hist {
            let since = match last {
                Some(t) => format!("timestamp={}", timeutil::format_iso8601(t)),
                None => "*".to_string(),
            };
            self.send_line(conn, &format!("CHATHISTORY LATEST {target} {since} 100"));
        }
        if mark {
            self.send_line(conn, &format!("MARKREAD {target}"));
        }
    }

    // --- modes ---

    #[allow(clippy::too_many_arguments)]
    fn on_channel_mode(
        &mut self,
        conn: ConnId,
        channel: String,
        setter: String,
        address: String,
        modes: String,
        changes: Vec<alpirc_proto::modes::ModeChange>,
        time: i64,
    ) {
        let win = self.chan_win(conn, &channel).unwrap_or_else(|| self.status(conn));
        let ctx = self.ctx(Some(conn), Some(win));
        let base = |kind: EventKind, s: &Self| {
            let mut ev = s.ev(kind, conn);
            ev.nick = setter.clone();
            ev.address = address.clone();
            ev.chan = Some(channel.clone());
            ev.target = Some(channel.clone());
            ev
        };
        let mut p = Pres::new();
        let Some(c) = self.conns.get(&conn) else { return };
        let is = c.session.isupport().clone();
        let me = c.session.nick().to_string();
        let mut plain_change = false;
        let mut reops = Vec::new();
        for ch in &changes {
            let sign = if ch.add { '+' } else { '-' };
            let mut raw = base(EventKind::RawMode, self);
            raw.what = format!("{sign}{}", ch.mode);
            raw.text = ch.arg.clone().unwrap_or_default();
            p = p.event(ctx.clone(), raw);
            let arg = ch.arg.clone().unwrap_or_default();
            let special = match (ch.mode, ch.add) {
                ('o', true) => Some((EventKind::Op, "opnick")),
                ('o', false) => Some((EventKind::Deop, "opnick")),
                ('h', true) => Some((EventKind::Help, "hnick")),
                ('h', false) => Some((EventKind::Dehelp, "hnick")),
                ('v', true) => Some((EventKind::Voice, "vnick")),
                ('v', false) => Some((EventKind::Devoice, "vnick")),
                ('b', true) => Some((EventKind::Ban, "banmask")),
                ('b', false) => Some((EventKind::Unban, "banmask")),
                _ => None,
            };
            match special {
                Some((k, key)) => {
                    let mut ev = base(k, self);
                    ev.text = arg.clone();
                    ev.extra.push((key.to_string(), arg.clone()));
                    if ch.mode == 'b' {
                        // `$bnick` : pseudo du canal touché par le masque, s'il y en a un.
                        if let Some(b) = self.ban_victim(conn, &channel, &arg) {
                            ev.extra.push(("bnick".into(), b));
                        }
                    }
                    p = p.event(ctx.clone(), ev);
                }
                None => {
                    if is.rank(ch.mode).is_none() {
                        plain_change = true;
                    }
                }
            }
            // Protection : un utilisateur protégé est déopé par un autre.
            if ch.mode == 'o'
                && !ch.add
                && !setter.eq_ignore_ascii_case(&arg)
                && !setter.eq_ignore_ascii_case(&me)
                && self.is_protected(conn, &channel, &arg)
            {
                reops.push(arg.clone());
            }
        }
        if plain_change {
            let mut ev = base(EventKind::Mode, self);
            ev.what = modes.split(' ').next().unwrap_or("").to_string();
            ev.text = modes.clone();
            p = p.event(ctx.clone(), ev);
        }
        let t = self.t(Key::SetsMode, &[("nick", &setter), ("modes", &modes)]);
        p = p.line(self.line_at(win, LineKind::Mode, t, time));
        if let Some(w) = self.win_mut(win) {
            w.touch();
        }
        self.push_pres(p);
        let am_op = self.conns.get(&conn).is_some_and(|c| c.session.am_op(&channel));
        if am_op && !reops.is_empty() {
            let ch: Vec<_> = reops.iter().map(|n| alpirc_proto::modes::ModeChange { add: true, mode: 'o', arg: Some(n.clone()) }).collect();
            for (m, args) in alpirc_proto::modes::build(&ch, is.modes_per_line) {
                self.send_line(conn, &format!("MODE {channel} {m} {}", args.join(" ")));
            }
        }
    }

    fn ban_victim(&self, conn: ConnId, chan: &str, banmask: &str) -> Option<String> {
        let c = self.conns.get(&conn)?;
        let ch = c.session.channel(chan)?;
        let cm = c.session.casemapping();
        ch.members.values().find_map(|m| {
            let u = c.session.user_info(&m.nick)?;
            mask::wildcard_match(banmask, &u.full_mask(), cm).then(|| m.nick.clone())
        })
    }

    fn control_matches(&self, conn: ConnId, chan: &str, nick: &str, address: &str, kind: ControlKind) -> bool {
        let Some(c) = self.conns.get(&conn) else { return false };
        let cm = c.session.casemapping();
        let full = format!("{nick}!{address}");
        let net = self.conn_network(conn);
        self.settings.address_book.control.iter().any(|e| {
            e.kind == kind
                && e.network.as_deref().is_none_or(|n| n.eq_ignore_ascii_case(&net) || c.net_name.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(n)))
                && (e.channels.is_empty() || e.channels.iter().any(|x| mask::wildcard_match(x, chan, cm)))
                && {
                    let m = if e.mask.contains(['!', '@']) { e.mask.clone() } else { format!("{}!*@*", e.mask) };
                    mask::wildcard_match(&m, &full, cm)
                }
        })
    }

    fn is_protected(&self, conn: ConnId, chan: &str, nick: &str) -> bool {
        let addr = self.conns.get(&conn).and_then(|c| c.session.user_info(nick)).and_then(|u| u.address()).unwrap_or_else(|| "*@*".into());
        self.control_matches(conn, chan, nick, &addr, ControlKind::Protect)
    }

    fn control_on_join(&mut self, conn: ConnId, chan: &str, nick: &str, address: &str) {
        let due = self.now + self.settings.irc.auto_op_delay_ms as i64;
        let mut modes = Vec::new();
        if self.control_matches(conn, chan, nick, address, ControlKind::Autoop) {
            modes.push('o');
        } else if self.control_matches(conn, chan, nick, address, ControlKind::Autovoice) {
            modes.push('v');
        }
        if let Some(c) = self.conns.get_mut(&conn) {
            for m in modes {
                c.pending_modes.push(super::state::PendingMode { due, chan: chan.to_string(), mode: m, nick: nick.to_string() });
            }
        }
    }

    /// Auto-op/auto-voice échus : regroupés par canal, `MODES` par ligne.
    pub(crate) fn run_pending_modes(&mut self, conn: ConnId) {
        let now = self.now;
        let Some(c) = self.conns.get_mut(&conn) else { return };
        let (due, rest): (Vec<_>, Vec<_>) = c.pending_modes.drain(..).partition(|m| m.due <= now);
        c.pending_modes = rest;
        if due.is_empty() {
            return;
        }
        let s = &c.session;
        let per_line = s.isupport().modes_per_line;
        let mut by_chan: Vec<(String, Vec<alpirc_proto::modes::ModeChange>)> = Vec::new();
        for m in due {
            let allowed = if m.mode == 'o' { s.am_op(&m.chan) } else { s.am_halfop(&m.chan) };
            let Some(cur) = s.member_modes(&m.chan, &m.nick) else { continue };
            if !allowed || cur.contains(m.mode) {
                continue;
            }
            let ch = alpirc_proto::modes::ModeChange { add: true, mode: m.mode, arg: Some(m.nick.clone()) };
            match by_chan.iter_mut().find(|(c, _)| s.eq(c, &m.chan)) {
                Some((_, v)) => {
                    if !v.iter().any(|x| x.mode == ch.mode && x.arg == ch.arg) {
                        v.push(ch);
                    }
                }
                None => by_chan.push((m.chan.clone(), vec![ch])),
            }
        }
        let mut lines = Vec::new();
        for (chan, mut v) in by_chan {
            v.sort_by_key(|x| x.mode);
            for (m, args) in alpirc_proto::modes::build(&v, per_line) {
                lines.push(format!("MODE {chan} {m} {}", args.join(" ")));
            }
        }
        for l in lines {
            self.send_line(conn, &l);
        }
    }

    // --- bans en attente d'adresse ---

    fn resolve_bans(&mut self, conn: ConnId, entries: &[(String, String, String)]) {
        let now = self.now;
        let Some(c) = self.conns.get_mut(&conn) else { return };
        let mut lines = Vec::new();
        let pending = std::mem::take(&mut c.pending_bans);
        for b in pending {
            match entries.iter().find(|(n, _, _)| c.session.eq(n, &b.nick)) {
                Some((nick, user, host)) => {
                    let m = mask::address_mask(nick, user, host, b.mask_type);
                    lines.push(format!("MODE {} +b {m}", b.chan));
                    if let Some(r) = &b.kick {
                        lines.push(super::commands::kick_msg(&b.chan, nick, r).to_line());
                    }
                    if let Some(s) = b.unban_secs {
                        c.timed_unbans.push((now + s as i64 * 1000, b.chan.clone(), m));
                    }
                }
                None => c.pending_bans.push(b),
            }
        }
        for l in lines {
            self.send_line(conn, &l);
        }
    }

    // --- Notify ---

    /// Pseudos de la liste Notify qui valent pour cette connexion.
    pub(crate) fn notify_nicks(&self, conn: ConnId) -> Vec<String> {
        // La connexion parente d'un bouncer ne surveille personne : ses connexions liées le font.
        if self.is_bouncer_parent(conn) {
            return Vec::new();
        }
        let net = self.conn_network(conn);
        let net_name = self.conns.get(&conn).and_then(|c| c.net_name.clone());
        self.settings
            .address_book
            .notify
            .iter()
            .filter(|e| {
                e.network
                    .as_deref()
                    .is_none_or(|n| n.eq_ignore_ascii_case(&net) || net_name.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(n)))
            })
            .map(|e| e.nick.clone())
            .collect()
    }

    fn on_ison(&mut self, conn: ConnId, online: &[String]) {
        let nicks = self.notify_nicks(conn);
        let Some(c) = self.conns.get(&conn) else { return };
        let on: Vec<String> = online.iter().map(|n| c.session.fold(n)).collect();
        let changes: Vec<(String, bool)> = nicks
            .iter()
            .map(|n| {
                let k = c.session.fold(n);
                (n.clone(), on.contains(&k))
            })
            .collect();
        for (n, is_on) in changes {
            self.notify_change(conn, &n, is_on, "");
        }
    }

    /// Un pseudo de la liste Notify arrive ou part.
    pub(crate) fn notify_change(&mut self, conn: ConnId, nick: &str, online: bool, address: &str) {
        let now = self.now;
        let Some(c) = self.conns.get_mut(&conn) else { return };
        let k = c.session.fold(nick);
        let was = c.notify_online.insert(k.clone(), online);
        if online {
            self.notify_seen.insert(k.clone(), now);
        }
        let changed = match (was, online) {
            (Some(true), true) => false,
            (_, true) => true,
            (Some(true), false) => true,
            _ => false,
        };
        if !changed {
            return;
        }
        let win = self.active_for(conn);
        let (key, kind) = if online { (Key::NotifyOn, EventKind::Notify) } else { (Key::NotifyOff, EventKind::Unotify) };
        let t = self.t(key, &[("nick", nick)]);
        let mut ev = self.ev(kind, conn);
        ev.nick = nick.to_string();
        ev.address = address.to_string();
        let mut p = Pres::new().event(self.ctx(Some(conn), Some(win)), ev).line(self.pl(win, LineKind::Notify, t));
        if let Some(fx) = self.event_sound(SoundEvent::Notify) {
            p = p.effect(fx);
        }
        self.push_pres(p);
    }

    // --- connexion prête (fin du MOTD) ---

    fn on_ready(&mut self, conn: ConnId) {
        let Some(c) = self.conns.get_mut(&conn) else { return };
        c.ready = true;
        c.state = ConnState::Connected;
        c.attempts = 0;
        let net_name = c.net_name.clone();
        let bound = c.bouncer_netid.is_some();
        let bouncer_cap = c.session.cap("soju.im/bouncer-networks");
        let monitor = c.session.isupport().monitor;
        let st = c.status;
        let ctx = self.ctx(Some(conn), Some(st));
        let ev = self.ev(EventKind::Connect, conn);
        let mut p = Pres::new().event(ctx.clone(), ev);
        if let Some(fx) = self.event_sound(SoundEvent::Connect) {
            p = p.effect(fx);
        }
        self.push_pres(p);
        let net = net_name.as_deref().and_then(|n| self.settings.network(n)).cloned();
        // Connexion parente d'un bouncer (non liée) : elle ne sert qu'à lister les réseaux ; perform, favoris et
        // Notify sont pour les connexions liées.
        let parent = self.is_bouncer_parent(conn);
        if let Some(n) = net.as_ref().filter(|_| !parent) {
            for l in &n.perform {
                let l = l.trim().trim_start_matches('/');
                if !l.is_empty() {
                    self.deferred.push_back(super::state::Deferred::Script(Box::new(ctx.clone()), format!("//{l}")));
                }
            }
        }
        if !parent {
            self.deferred.push_back(super::state::Deferred::AutoJoin(conn));
        }
        // Notify.
        let nicks = self.notify_nicks(conn);
        if let Some(limit) = monitor {
            if !nicks.is_empty() {
                for chunk in nicks.chunks(limit.clamp(1, 100)) {
                    self.send_line(conn, &format!("MONITOR + {}", chunk.join(",")));
                }
            }
            if let Some(c) = self.conns.get_mut(&conn) {
                c.monitor_sent = true;
                c.next_ison = None;
            }
        } else if let Some(c) = self.conns.get_mut(&conn) {
            c.next_ison = if nicks.is_empty() { None } else { Some(self.now) };
        }
        // Bouncer : liste des réseaux (avec bouncer-networks-notify, le bouncer l'envoie de lui-même).
        let notify = self.conns.get(&conn).is_some_and(|c| c.session.cap("soju.im/bouncer-networks-notify"));
        if bouncer_cap && !bound && !notify && net.as_ref().is_none_or(|n| n.bouncer) {
            self.send_line(conn, "BOUNCER LISTNETWORKS");
        }
    }

    /// Connexion parente d'un bouncer soju : `soju.im/bouncer-networks` négocié, pas liée à un réseau, et le
    /// réglage du réseau veut une connexion par réseau du bouncer.
    pub(crate) fn is_bouncer_parent(&self, conn: ConnId) -> bool {
        let Some(c) = self.conns.get(&conn) else { return false };
        c.bouncer_netid.is_none()
            && c.session.cap("soju.im/bouncer-networks")
            && c.net_name.as_deref().and_then(|n| self.settings.network(n)).is_none_or(|n| n.bouncer)
    }

    // --- STS ---

    fn on_sts(&mut self, conn: ConnId, info: crate::session::StsInfo) {
        let Some(c) = self.conns.get(&conn) else { return };
        let tls = c.tls;
        let host = c.host.clone();
        let port = c.port;
        let net_name = c.net_name.clone();
        if !tls {
            let Some(p) = info.port else { return };
            if let Some(n) = net_name.as_deref().and_then(|n| self.settings.network_mut(n)) {
                n.sts = Some(crate::settings::StsPolicy { port: p, expires_ms: None });
                self.effects.push(Effect::SaveSettings);
            }
            let st = self.status(conn);
            let t = self.t(Key::StsUpgrade, &[("port", &p.to_string())]);
            self.print(st, LineKind::Info, t);
            if let Some(c) = self.conns.get_mut(&conn) {
                c.sts_upgrade = Some(p);
                c.state = ConnState::Disconnected;
                c.ready = false;
            }
            self.close_effect(conn);
            self.do_connect(conn);
        } else if let Some(d) = info.duration_secs {
            let expires = self.now + d as i64 * 1000;
            let mut first = false;
            if let Some(n) = net_name.as_deref().and_then(|n| self.settings.network_mut(n)) {
                first = n.sts.as_ref().is_none_or(|s| s.expires_ms.is_none());
                n.sts = if d == 0 { None } else { Some(crate::settings::StsPolicy { port, expires_ms: Some(expires) }) };
                self.effects.push(Effect::SaveSettings);
            }
            if first && d > 0 {
                let st = self.status(conn);
                let t = self.t(Key::StsStored, &[("host", &host)]);
                self.print(st, LineKind::Info, t);
            }
        }
    }

    // --- bouncer ---

    fn on_bouncer_network(&mut self, conn: ConnId, id: String, attrs: Vec<(String, String)>, deleted: bool) {
        let Some(c) = self.conns.get_mut(&conn) else { return };
        if c.bouncer_netid.is_some() {
            return;
        }
        let net_name = c.net_name.clone();
        if deleted {
            c.bouncer_names.remove(&id);
            c.bouncer_attrs.remove(&id);
            if let Some(n) = net_name.as_deref().and_then(|n| self.settings.network_mut(n)) {
                n.bouncer_networks.retain(|b| b.id != id);
                self.effects.push(Effect::SaveSettings);
            }
            return;
        }
        // Une notification peut ne porter que ce qui a changé (`state=…`) : les attributs se fusionnent, et un
        // attribut sans valeur est retiré. Le nom reste celui qu'on connaît s'il n'est pas redonné.
        let known = c.bouncer_names.get(&id).cloned().or_else(|| {
            let n = net_name.as_deref().and_then(|n| self.settings.network(n))?;
            n.bouncer_networks.iter().find(|b| b.id == id).map(|b| b.name.clone())
        });
        let Some(c) = self.conns.get_mut(&conn) else { return };
        let merged = c.bouncer_attrs.entry(id.clone()).or_default();
        for (k, v) in &attrs {
            if v.is_empty() {
                merged.remove(k);
            } else {
                merged.insert(k.clone(), v.clone());
            }
        }
        let name = merged.get("name").cloned().or(known).unwrap_or_else(|| id.clone());
        let renamed = c.bouncer_names.get(&id) != Some(&name);
        c.bouncer_names.insert(id.clone(), name.clone());
        let child = c.bouncer_children.get(&id).copied();
        let in_batch = c.bouncer_batch;
        let has_child = c.bouncer_children.contains_key(&id);
        let mut wanted = true;
        if let Some(n) = net_name.as_deref().and_then(|n| self.settings.network_mut(n)) {
            wanted = n.bouncer;
            match n.bouncer_networks.iter_mut().find(|b| b.id == id) {
                Some(b) => {
                    if b.name != name {
                        b.name = name.clone();
                        self.effects.push(Effect::SaveSettings);
                    }
                }
                None => {
                    n.bouncer_networks.push(crate::settings::BouncerNetwork { id: id.clone(), name: name.clone() });
                    self.effects.push(Effect::SaveSettings);
                }
            }
        }
        // La fenêtre Status de la connexion liée prend le nouveau nom.
        if renamed
            && let Some(ch) = child
            && let Some(cc) = self.conns.get_mut(&ch)
        {
            cc.network = name.clone();
            self.update_titles(ch);
        }
        if !in_batch && renamed {
            self.print_bouncer_networks(conn);
        }
        if wanted && !has_child {
            self.spawn_bouncer_child(conn, &id, &name);
        }
    }

    fn print_bouncer_networks(&mut self, conn: ConnId) {
        let Some(c) = self.conns.get(&conn) else { return };
        if c.bouncer_names.is_empty() {
            return;
        }
        let list: Vec<String> = c.bouncer_names.iter().map(|(id, n)| format!("{n} ({id})")).collect();
        let st = c.status;
        let t = self.t(Key::BouncerNetworks, &[("list", &list.join(", "))]);
        self.print(st, LineKind::Info, t);
    }

    // --- messages ---

    #[allow(clippy::too_many_arguments)]
    fn on_message(
        &mut self,
        conn: ConnId,
        msg: &Message,
        kind: MsgKind,
        from: String,
        addr: String,
        target: String,
        text: String,
        channel: Option<String>,
        statusmsg: Option<char>,
        own: bool,
        server: bool,
        time: i64,
    ) {
        let msgid = msg.tags.get("msgid").map(str::to_string);
        if server {
            let st = self.status(conn);
            let mut ev = self.ev(EventKind::Snotice, conn);
            ev.nick = from.clone();
            ev.text = text.clone();
            let p = Pres::new().event(self.ctx(Some(conn), Some(st)), ev).line(self.line_at(st, LineKind::Notice, format!("-{from}- {text}"), time));
            self.push_pres(p);
            return;
        }
        if own {
            if kind == MsgKind::Privmsg && channel.is_none() {
                self.own_private_elsewhere(conn, &target, &text);
            }
            let lines = self.own_lines(conn, kind, &target, &text, time);
            let mut p = Pres::new();
            for mut l in lines {
                l.msgid = msgid.clone();
                p = p.line(l);
            }
            self.push_pres(p);
            return;
        }
        let ctcp = ctcp::parse(&text);
        let is_action = ctcp.as_ref().is_some_and(|c| c.command == "ACTION");
        // Ignorer.
        let ity = match (&ctcp, kind, &channel) {
            (Some(c), MsgKind::Privmsg, _) if c.command == "DCC" => IgnoreType::Dcc,
            (Some(c), _, _) if c.command != "ACTION" => IgnoreType::Ctcp,
            (_, MsgKind::Notice, _) => IgnoreType::Notice,
            (_, _, Some(_)) => IgnoreType::Channel,
            _ => IgnoreType::Private,
        };
        let ign = self.ignored(conn, &from, &addr, ity);
        if ign == Ignored::Drop {
            return;
        }
        if let Some(c) = &ctcp
            && !is_action
        {
            let (cmd, params) = (c.command.clone(), c.params.to_string());
            if kind == MsgKind::Privmsg {
                self.on_ctcp(conn, &from, &addr, &target, channel.as_deref(), &cmd, &params, time);
            } else {
                self.on_ctcp_reply(conn, &from, &addr, &cmd, &params, time);
            }
            return;
        }
        let body = if is_action { ctcp.as_ref().map(|c| c.params.to_string()).unwrap_or_default() } else { text.clone() };
        let shown = if self.settings.irc.strip_codes || ign == Ignored::StripCodes { format::strip(&body) } else { body.clone() };
        let hit = self.highlight(conn, &body);
        let mut p = Pres::new();
        let ev_kind = match (kind, is_action) {
            (MsgKind::Notice, _) => EventKind::Notice,
            (_, true) => EventKind::Action,
            _ => EventKind::Text,
        };
        let mut ev = self.ev(ev_kind, conn);
        ev.nick = from.clone();
        ev.address = addr.clone();
        ev.chan = channel.clone();
        ev.target = Some(target.clone());
        ev.text = body.clone();
        if !msg.tags.is_empty() {
            let mut t = String::new();
            let tagline = msg.to_line();
            if let Some(rest) = tagline.strip_prefix('@') {
                t = rest.split(' ').next().unwrap_or("").to_string();
            }
            ev.extra.push(("msgtags".into(), t));
        }
        let (win, text_line, lkind) = match (&channel, kind) {
            (Some(ch), MsgKind::Privmsg) => {
                let win = self.chan_win(conn, ch).unwrap_or_else(|| self.status(conn));
                let tag = self.nick_tag(conn, Some(ch), &from);
                let disp = if statusmsg.is_some() { format!("{tag}:{target}") } else { tag };
                let t = if is_action { format!("* {from} {shown}") } else { format!("<{disp}> {shown}") };
                (win, t, if is_action { LineKind::Action } else { LineKind::Normal })
            }
            (Some(ch), MsgKind::Notice) => {
                let win = self.chan_win(conn, ch).unwrap_or_else(|| self.status(conn));
                (win, format!("-{from}:{target}- {shown}"), LineKind::Notice)
            }
            (None, MsgKind::Notice) => {
                let win = if self.settings.irc.notices_in_active { self.active_for(conn) } else { self.status(conn) };
                (win, format!("-{from}- {shown}"), LineKind::Notice)
            }
            (None, MsgKind::Privmsg) => {
                let mut win = self.query_win(conn, &from);
                if win.is_none() && self.settings.irc.query_on_private {
                    let w = self.create_window(WinKind::Query, Some(conn), &from);
                    let mut oev = self.ev(EventKind::Open, conn);
                    oev.nick = from.clone();
                    oev.address = addr.clone();
                    oev.target = Some(from.clone());
                    oev.text = body.clone();
                    self.push_pres(Pres::new().event(self.ctx(Some(conn), Some(w)), oev));
                    self.request_history(conn, &from, None);
                    win = Some(w);
                }
                match win {
                    Some(w) => {
                        let t = if is_action { format!("* {from} {shown}") } else { format!("<{}> {shown}", self.nick_tag(conn, None, &from)) };
                        (w, t, if is_action { LineKind::Action } else { LineKind::Normal })
                    }
                    None => {
                        let w = self.active_for(conn);
                        let t = if is_action { format!("* {from} {shown}") } else { format!("*{from}* {shown}") };
                        (w, t, if is_action { LineKind::Action } else { LineKind::Normal })
                    }
                }
            }
        };
        let window_name = self.win(win).map(|w| w.name.clone()).unwrap_or_default();
        p = p.event(self.ctx(Some(conn), Some(win)), ev);
        let mut l = self.line_at(win, lkind, text_line, time);
        l.nick = Some(from.clone());
        l.msgid = msgid;
        l.activity = Activity::Message;
        if let Some(h) = &hit {
            l.highlight = true;
            l.activity = Activity::Highlight;
            l.color = h.color;
        }
        p = p.line(l);
        // Alertes.
        let private = channel.is_none() && kind == MsgKind::Privmsg;
        // Son (Options › Sons) : on vous nomme, sinon message privé, sinon message d'un canal ; un seul par message,
        // et il remplace le bip des alertes.
        let sound_ev = match (&hit, private, &channel, kind) {
            (Some(_), _, _, _) => Some(SoundEvent::Highlight),
            (None, true, _, _) => Some(SoundEvent::Query),
            (None, false, Some(_), MsgKind::Privmsg) => Some(SoundEvent::ChannelMessage),
            _ => None,
        };
        let sound = sound_ev.and_then(|e| self.message_sound(e, win, time));
        if let Some(fx) = &sound {
            p = p.effect(fx.clone());
        }
        let hl = &self.settings.highlight;
        let (mut beep, mut flash, mut notify) = (false, false, false);
        if let Some(h) = &hit {
            beep |= h.beep;
            flash |= h.flash;
            notify |= h.notify;
        }
        if private {
            beep |= hl.beep_on_query;
            flash |= hl.flash;
            notify |= hl.notify_on_query;
        }
        if beep || flash || notify {
            let title = if private {
                self.t(Key::NotifQuery, &[("nick", &from)])
            } else {
                self.t(Key::NotifHighlight, &[("nick", &from), ("chan", channel.as_deref().unwrap_or(&target))])
            };
            for e in self.alert_effects(win, beep && sound.is_none(), flash, notify, title, &body) {
                p = p.effect(e);
            }
        }
        self.push_pres(p);
        self.catch_urls(conn, &from, &window_name, &body, time);
    }

    /// Notre message privé renvoyé par le serveur. S'il n'a pas été envoyé d'ici (écrit sur un autre appareil et
    /// renvoyé par un bouncer), il va dans le privé de la cible, ouvert au besoin comme pour un message reçu
    /// (sans le rendre actif) ; l'écho d'un `/msg` tapé ici reste à la manière de mIRC (`-> *pseudo*`).
    fn own_private_elsewhere(&mut self, conn: ConnId, target: &str, text: &str) {
        let now = self.now;
        let Some(c) = self.conns.get_mut(&conn) else { return };
        let fold = c.session.fold(target);
        c.own_sent.retain(|(t, _, _)| now - t <= super::state::OWN_ECHO_MS);
        if let Some(i) = c.own_sent.iter().position(|(_, f, x)| *f == fold && x == text) {
            c.own_sent.remove(i);
            return;
        }
        let is_nick = !target.is_empty() && !target.starts_with(['$', '#', '&']) && !target.contains(['*', ',']);
        let ctcp_other = ctcp::parse(text).is_some_and(|x| x.command != "ACTION");
        if !is_nick || ctcp_other || !self.settings.irc.query_on_private || self.query_win(conn, target).is_some() {
            return;
        }
        self.create_window(WinKind::Query, Some(conn), target);
        self.request_history(conn, target, None);
    }

    /// Lignes de nos propres messages (écho du serveur, ou envoi sans echo-message).
    pub(crate) fn own_lines(&self, conn: ConnId, kind: MsgKind, target: &str, text: &str, time: i64) -> Vec<PendingLine> {
        let me = self.me(conn);
        let is_chan = self.conns.get(&conn).is_some_and(|c| c.session.is_channel(target.trim_start_matches(|ch| {
            c.session.isupport().statusmsg.contains(ch)
        })));
        let chan_name = if is_chan {
            let c = &self.conns[&conn];
            target.trim_start_matches(|ch| c.session.isupport().statusmsg.contains(ch)).to_string()
        } else {
            target.to_string()
        };
        let target_win = if is_chan { self.chan_win(conn, &chan_name) } else { self.query_win(conn, target) };
        let mut out = Vec::new();
        let mk = |s: &Self, win: WinId, k: LineKind, t: String| {
            let mut l = s.line_at(win, k, t, time);
            l.nick = Some(me.clone());
            l.activity = Activity::None;
            l
        };
        match ctcp::parse(text) {
            Some(c) if c.command == "ACTION" => match target_win {
                Some(w) => out.push(mk(self, w, LineKind::Action, format!("* {me} {}", c.params))),
                None => {
                    let w = self.active_for(conn);
                    let t = self.t(Key::MsgSent, &[("nick", target), ("text", &format!("* {me} {}", c.params))]);
                    out.push(mk(self, w, LineKind::Action, t));
                }
            },
            // Demande de son (`/sound`) : voir `sounds.rs`.
            Some(c) if c.command == "SOUND" && kind == MsgKind::Privmsg => {
                out.extend(self.own_sound_lines(conn, target, target_win, c.params, time));
            }
            Some(c) => {
                if kind == MsgKind::Privmsg {
                    let w = self.active_for(conn);
                    let cmd = if c.params.is_empty() { c.command.clone() } else { format!("{} {}", c.command, c.params) };
                    let t = self.t(Key::CtcpSent, &[("nick", target), ("cmd", &cmd)]);
                    out.push(mk(self, w, LineKind::Ctcp, t));
                }
            }
            None => match kind {
                MsgKind::Privmsg => match target_win {
                    Some(w) => {
                        let tag = self.nick_tag(conn, if is_chan { Some(&chan_name) } else { None }, &me);
                        let disp = if is_chan && chan_name != target { format!("{tag}:{target}") } else { tag };
                        out.push(mk(self, w, LineKind::Own, format!("<{disp}> {text}")));
                    }
                    None => {
                        let w = self.active_for(conn);
                        let t = self.t(Key::MsgSent, &[("nick", target), ("text", text)]);
                        out.push(mk(self, w, LineKind::Own, t));
                    }
                },
                MsgKind::Notice => {
                    let w = if is_chan { target_win.unwrap_or_else(|| self.active_for(conn)) } else { self.active_for(conn) };
                    let t = self.t(Key::NoticeSent, &[("nick", target), ("text", text)]);
                    out.push(mk(self, w, LineKind::Notice, t));
                }
            },
        }
        out
    }

    // --- CTCP ---

    #[allow(clippy::too_many_arguments)]
    fn on_ctcp(&mut self, conn: ConnId, from: &str, addr: &str, target: &str, chan: Option<&str>, cmd: &str, params: &str, time: i64) {
        let win = self.active_for(conn);
        let mut ev = self.ev(EventKind::Ctcp, conn);
        ev.nick = from.to_string();
        ev.address = addr.to_string();
        ev.chan = chan.map(str::to_string);
        ev.target = Some(target.to_string());
        ev.what = cmd.to_string();
        ev.text = params.to_string();
        let mut p = Pres::new().event(self.ctx(Some(conn), Some(win)), ev);
        if cmd == "DCC" {
            // Offres et réponses DCC : voir `dcc.rs` (traitées après les scripts, sauf `/halt`).
            p = self.dcc_ctcp(conn, from, addr, params, win, time, p);
            self.push_pres(p);
            return;
        }
        if cmd == "SOUND" {
            // Demande de son : voir `sounds.rs`.
            p = self.ctcp_sound(conn, from, chan, params, time, p);
            self.push_pres(p);
            return;
        }
        let shown = match chan {
            Some(c) => format!("[{from}:{c} {cmd}]"),
            None => format!("[{from} {cmd}]"),
        };
        p = p.line(self.line_at(win, LineKind::Ctcp, shown, time));
        if let Some(reply) = self.ctcp_reply_text(cmd, params)
            && self.ctcp_allowed(conn)
        {
            let line = super::commands::text_line("NOTICE", from, &ctcp::encode(cmd, &reply));
            p = p.send(conn, line);
        }
        self.push_pres(p);
    }

    /// Garde-fou anti-flood CTCP : au plus `ctcp_flood_limit` réponses par `ctcp_flood_secs`.
    fn ctcp_allowed(&mut self, conn: ConnId) -> bool {
        let now = self.now;
        let limit = self.settings.flood.ctcp_flood_limit.max(1) as usize;
        let window = self.settings.flood.ctcp_flood_secs as i64 * 1000;
        let Some(c) = self.conns.get_mut(&conn) else { return false };
        while c.ctcp_times.front().is_some_and(|t| now - *t >= window) {
            c.ctcp_times.pop_front();
        }
        if c.ctcp_times.len() >= limit {
            return false;
        }
        c.ctcp_times.push_back(now);
        true
    }

    fn ctcp_reply_text(&self, cmd: &str, params: &str) -> Option<String> {
        let o = &self.settings.ctcp;
        match cmd {
            "VERSION" if o.reply_version => Some(o.version_reply.clone().filter(|v| !v.is_empty()).unwrap_or_else(version_reply)),
            "PING" if o.reply_ping => Some(params.to_string()),
            "TIME" if o.reply_time => Some(self.asctime(self.now)),
            "CLIENTINFO" if o.reply_clientinfo => Some("ACTION CLIENTINFO DCC FINGER PING SOUND SOURCE TIME USERINFO VERSION".into()),
            "FINGER" if o.reply_finger => Some(o.finger_text.clone().unwrap_or_else(|| {
                let idle = ((self.now - self.last_input) / 1000).max(0);
                format!("{} ({}) Idle {idle} seconds", self.settings.identity.realname, self.settings.identity.username)
            })),
            "USERINFO" if o.reply_userinfo => Some(o.userinfo_text.clone().unwrap_or_else(|| self.settings.identity.realname.clone())),
            "SOURCE" if o.reply_source => Some(HOMEPAGE.to_string()),
            _ => None,
        }
    }

    fn on_ctcp_reply(&mut self, conn: ConnId, from: &str, addr: &str, cmd: &str, params: &str, time: i64) {
        let win = self.active_for(conn);
        let mut ev = self.ev(EventKind::CtcpReply, conn);
        ev.nick = from.to_string();
        ev.address = addr.to_string();
        ev.what = cmd.to_string();
        ev.text = params.to_string();
        let text = if cmd == "PING" {
            match params.trim().parse::<i64>() {
                Ok(v) => {
                    let sent_ms = if v > 100_000_000_000 { v } else { v * 1000 };
                    let secs = (self.now - sent_ms).max(0) as f64 / 1000.0;
                    self.t(Key::PingSecs, &[("secs", &format!("{secs:.3}"))])
                }
                Err(_) => params.to_string(),
            }
        } else {
            params.to_string()
        };
        let t = self.t(Key::CtcpReply, &[("nick", from), ("cmd", cmd), ("text", &text)]);
        let p = Pres::new().event(self.ctx(Some(conn), Some(win)), ev).line(self.line_at(win, LineKind::Ctcp, t, time));
        self.push_pres(p);
    }

    // --- historique ---

    fn on_history(&mut self, conn: ConnId, target: &str, messages: &[Message]) {
        let Some(win) = self.find_win(Some(conn), target) else { return };
        let me = self.me(conn);
        for m in messages {
            let time = crate::session::message_time(m, self.now);
            let nick = m.nick().to_string();
            let addr = m.source.as_ref().and_then(|s| s.address()).unwrap_or_default();
            let (kind, text) = match m.command.as_str() {
                "PRIVMSG" | "NOTICE" => {
                    let body = m.param(1);
                    let is_me = self.conns.get(&conn).is_some_and(|c| c.session.eq(&nick, &me));
                    match ctcp::parse(body) {
                        Some(c) if c.command == "ACTION" => (LineKind::Action, format!("* {nick} {}", c.params)),
                        Some(_) => continue,
                        None if m.command == "NOTICE" => (LineKind::Notice, format!("-{nick}- {body}")),
                        None => (if is_me { LineKind::Own } else { LineKind::Normal }, format!("<{nick}> {body}")),
                    }
                }
                "JOIN" => (LineKind::Join, self.t(Key::Joins, &[("nick", &nick), ("address", &addr)])),
                "PART" => (LineKind::Part, self.t(Key::Parts, &[("nick", &nick), ("address", &addr)])),
                "QUIT" => (LineKind::Quit, self.t(Key::Quits, &[("nick", &nick), ("address", &addr)])),
                "NICK" => (LineKind::Nick, self.t(Key::NickChange, &[("nick", &nick), ("new", m.param(0))])),
                "KICK" => (LineKind::Kick, self.t(Key::Kicked, &[("nick", m.param(1)), ("by", &nick), ("reason", m.param(2))])),
                "TOPIC" => (LineKind::Topic, self.t(Key::ChangesTopic, &[("nick", &nick), ("topic", m.param(1))])),
                "MODE" => (LineKind::Mode, self.t(Key::SetsMode, &[("nick", &nick), ("modes", &m.params[1..].join(" "))])),
                _ => continue,
            };
            let mut l = self.line_at(win, kind, text, time);
            l.from_history = true;
            l.msgid = m.tags.get("msgid").map(str::to_string);
            if matches!(m.command.as_str(), "PRIVMSG" | "NOTICE") {
                l.nick = Some(nick.clone());
                let mine = self.conns.get(&conn).is_some_and(|c| c.session.eq(&nick, &me));
                l.activity = if mine { Activity::None } else { Activity::Message };
                if !mine && let Some(h) = self.highlight(conn, m.param(1)) {
                    l.highlight = true;
                    l.color = h.color;
                    l.activity = Activity::Highlight;
                }
            } else {
                l.activity = Activity::Event;
            }
            self.add_line(l);
        }
    }

    // --- numériques ---

    fn show_numeric(&mut self, conn: ConnId, msg: &Message, time: i64) {
        let cmd = msg.command.as_str();
        let p = |i: usize| msg.param(i).to_string();
        let rest = || msg.params.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
        let st = self.status(conn);
        let whois_win = |s: &mut Self| -> WinId {
            if let Some(w) = s.conns.get(&conn).and_then(|c| c.whois_win).filter(|w| s.win(*w).is_some()) {
                return w;
            }
            let w = if s.settings.irc.whois_in_active { s.active_for(conn) } else { s.status(conn) };
            if let Some(c) = s.conns.get_mut(&conn) {
                c.whois_win = Some(w);
            }
            w
        };
        let out: Option<(WinId, LineKind, String)> = match cmd {
            // Gérés ailleurs (événements de la session, liste des canaux).
            n::RPL_TOPIC | n::RPL_TOPICWHOTIME | n::RPL_NOTOPIC | n::RPL_CHANNELMODEIS | n::RPL_CREATIONTIME | n::RPL_NAMREPLY
            | n::RPL_BANLIST | n::RPL_EXCEPTLIST | n::RPL_INVITELIST | n::RPL_MONONLINE | n::RPL_MONOFFLINE => None,
            n::RPL_LISTSTART => {
                self.chanlist_start(conn);
                None
            }
            n::RPL_LIST => {
                self.chanlist_entry(conn, &p(1), msg.param(2).parse().unwrap_or(0), msg.trailing());
                None
            }
            n::RPL_LISTEND => {
                self.chanlist_end(conn);
                None
            }
            n::RPL_MOTDSTART | n::RPL_MOTD | n::RPL_ENDOFMOTD => Some((st, LineKind::Normal, msg.trailing().to_string())),
            n::RPL_WHOISUSER => {
                let w = whois_win(self);
                let t = self.t(Key::WhoisUser, &[("nick", &p(1)), ("address", &format!("{}@{}", p(2), p(3))), ("realname", msg.trailing())]);
                Some((w, LineKind::Whois, t))
            }
            n::RPL_WHOWASUSER => {
                let w = whois_win(self);
                let t = self.t(Key::WhowasUser, &[("nick", &p(1)), ("address", &format!("{}@{}", p(2), p(3))), ("realname", msg.trailing())]);
                Some((w, LineKind::Whois, t))
            }
            n::RPL_WHOISCHANNELS => {
                let w = whois_win(self);
                Some((w, LineKind::Whois, self.t(Key::WhoisChannels, &[("nick", &p(1)), ("chans", msg.trailing().trim())])))
            }
            n::RPL_WHOISSERVER => {
                let w = whois_win(self);
                Some((w, LineKind::Whois, self.t(Key::WhoisServer, &[("nick", &p(1)), ("server", &p(2)), ("info", msg.trailing())])))
            }
            n::RPL_WHOISIDLE => {
                let w = whois_win(self);
                let idle = timeutil::duration(msg.param(2).parse().unwrap_or(0));
                let signon = msg.param(3).parse::<i64>().map(|s| self.date_text(s * 1000)).unwrap_or_default();
                Some((w, LineKind::Whois, self.t(Key::WhoisIdle, &[("nick", &p(1)), ("idle", &idle), ("signon", &signon)])))
            }
            n::RPL_WHOISACCOUNT => {
                let w = whois_win(self);
                Some((w, LineKind::Whois, self.t(Key::WhoisAccount, &[("nick", &p(1)), ("account", &p(2))])))
            }
            n::RPL_WHOISSECURE => {
                let w = whois_win(self);
                Some((w, LineKind::Whois, self.t(Key::WhoisSecure, &[("nick", &p(1))])))
            }
            n::RPL_ENDOFWHOIS | n::RPL_ENDOFWHOWAS => {
                let w = whois_win(self);
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.whois_win = None;
                }
                let t = if cmd == n::RPL_ENDOFWHOIS { self.t(Key::WhoisEnd, &[("nick", &p(1))]) } else { rest() };
                Some((w, LineKind::Whois, t))
            }
            n::RPL_WHOISOPERATOR | n::RPL_WHOISREGNICK | n::RPL_WHOISSPECIAL | n::RPL_WHOISHOST | n::RPL_WHOISMODES
            | n::RPL_WHOISACTUALLY | "276" | "335" | "310" => {
                let w = whois_win(self);
                Some((w, LineKind::Whois, rest()))
            }
            n::RPL_AWAY => {
                let nick = p(1);
                let text = self.t(Key::AwayMsg, &[("nick", &nick), ("msg", msg.trailing())]);
                let in_whois = self.conns.get(&conn).is_some_and(|c| c.whois_win.is_some());
                if in_whois {
                    let w = whois_win(self);
                    Some((w, LineKind::Whois, text))
                } else {
                    let show = self.settings.away.show_in_queries;
                    let c = self.conns.get_mut(&conn).unwrap();
                    let k = c.session.fold(&nick);
                    if c.away_shown.get(&k).map(String::as_str) == Some(msg.trailing()) {
                        None
                    } else {
                        c.away_shown.insert(k, msg.trailing().to_string());
                        match self.query_win(conn, &nick) {
                            Some(q) if show => Some((q, LineKind::Info, text)),
                            _ => Some((self.active_for(conn), LineKind::Info, text)),
                        }
                    }
                }
            }
            n::RPL_UNAWAY | n::RPL_NOWAWAY => Some((self.active_for(conn), LineKind::Info, msg.trailing().to_string())),
            n::RPL_ENDOFNAMES | n::RPL_ENDOFBANLIST | n::RPL_ENDOFEXCEPTLIST | n::RPL_ENDOFINVITELIST => {
                let chan = p(1);
                let w = self.chan_win(conn, &chan).unwrap_or_else(|| self.active_for(conn));
                Some((w, LineKind::Normal, rest()))
            }
            _ if n::is_error(cmd) => Some((self.active_for(conn), LineKind::Error, rest())),
            _ => Some((st, LineKind::Normal, rest())),
        };
        if let Some((w, k, t)) = out {
            let l = self.line_at(w, k, t, time);
            self.push_pres(Pres::new().line(l));
        }
    }

    // --- /list ---

    fn list_window(&mut self, conn: ConnId) -> WinId {
        if let Some(w) = self.conns.get(&conn).and_then(|c| c.list_win).filter(|w| self.win(*w).is_some()) {
            return w;
        }
        let name = "Channels List";
        let w = match self.find_kind(conn, WinKind::ChanList, name) {
            Some(w) => w,
            None => self.create_window(WinKind::ChanList, Some(conn), name),
        };
        if let Some(c) = self.conns.get_mut(&conn) {
            c.list_win = Some(w);
        }
        self.chanlists.entry(w).or_default();
        w
    }

    pub(crate) fn chanlist_start(&mut self, conn: ConnId) {
        let w = self.list_window(conn);
        if let Some(l) = self.chanlists.get_mut(&w) {
            l.entries.clear();
            l.complete = false;
            l.generation += 1;
        }
        if let Some(x) = self.win_mut(w) {
            x.touch();
        }
    }

    fn chanlist_entry(&mut self, conn: ConnId, name: &str, users: u32, topic: &str) {
        let w = self.list_window(conn);
        let cm = self.conns.get(&conn).map(|c| c.session.casemapping()).unwrap_or_default();
        if let Some(l) = self.chanlists.get_mut(&w) {
            let pass_filter = l.filter.as_deref().is_none_or(|f| {
                let pat = if f.contains(['*', '?']) { f.to_string() } else { format!("*{f}*") };
                mask::wildcard_match(&pat, name, cm) || (!l.names_only && mask::wildcard_match(&pat, topic, cm))
            });
            if pass_filter && l.min_users.is_none_or(|m| users >= m) && l.max_users.is_none_or(|m| users <= m) {
                l.entries.push(ChanEntry { name: name.to_string(), users, topic: topic.to_string() });
            }
        }
        if let Some(x) = self.win_mut(w) {
            x.touch();
        }
    }

    fn chanlist_end(&mut self, conn: ConnId) {
        let w = self.list_window(conn);
        let n = self.chanlists.get_mut(&w).map(|l| {
            l.complete = true;
            l.entries.len()
        });
        let t = self.t(Key::ListDone, &[("n", &n.unwrap_or(0).to_string())]);
        self.print(w, LineKind::Info, t);
    }

    /// Prépare la fenêtre de `/list` (filtre côté client).
    pub(crate) fn chanlist_prepare(&mut self, conn: ConnId, filter: Option<String>, min: Option<u32>, max: Option<u32>) -> WinId {
        let w = self.list_window(conn);
        let generation = self.chanlists.get(&w).map_or(0, |l| l.generation) + 1;
        let l = ChanList { filter, min_users: min, max_users: max, generation, ..Default::default() };
        self.chanlists.insert(w, l);
        w
    }
}
