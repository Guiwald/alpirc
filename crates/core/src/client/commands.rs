//! Commandes intégrées (`/join`, `/msg`, `/ban`, `/window`…). Le moteur de scripts passe d'abord par les alias ;
//! ce qui reste arrive ici ([`State::run_builtin`]). Une commande inconnue est envoyée telle quelle au serveur,
//! comme dans mIRC (`/knock #canal` → `KNOCK #canal`).

use alpirc_proto::modes::{self, ModeChange};
use alpirc_proto::{Message, ctcp, format, mask, split};
use alpirc_script::{Ctx, EventKind};

use super::state::{PendingBan, PendingLine, Pres, State};
use super::util::{split_word, switch_num, switches};
use super::window::{Activity, Line, LineKind, WinKind};
use super::{ConnId, Effect, UiDialog, WinId};
use crate::i18n::Key;
use crate::session::MsgKind;
use crate::settings::{IgnoreEntry, NotifyEntry, Server};

/// Noms des commandes intégrées (complétion Tab).
pub const BUILTINS: &[&str] = &[
    "abook", "about", "action", "admin", "aline", "ame", "amsg", "away", "back", "ban", "beep", "channel", "clear",
    "clearall", "cline", "close", "closemsg", "colors", "ctcp", "ctcpreply", "cycle", "dcc", "dehalfop", "deop",
    "describe", "devoice", "disconnect", "dline", "dns", "echo", "exit", "flash", "folder", "font", "halfop", "help",
    "hop", "ial", "ignore", "iline", "info", "invite", "join", "kick", "lag", "linesep", "links", "list", "log",
    "lusers", "me", "mode", "motd", "msg", "names", "nick", "notice", "notify", "omsg", "onotice", "op", "options",
    "part", "partall", "ping", "query", "quit", "quote", "raw", "rline", "run", "say", "scripts", "server", "setname",
    "sline", "sound", "splay", "stats", "time", "timestamp", "topic", "url", "version", "voice", "who", "whois",
    "whowas", "window",
];

/// `PRIVMSG cible :texte` : le texte toujours en dernier paramètre, comme le font les clients IRC.
pub(crate) fn text_line(cmd: &str, target: &str, text: &str) -> String {
    let target: String = target.chars().filter(|c| !c.is_whitespace()).collect();
    format!("{cmd} {target} :{text}")
}

/// `KICK #canal pseudo [:raison]`.
pub(crate) fn kick_msg(chan: &str, nick: &str, reason: &str) -> Message {
    if reason.is_empty() {
        Message::new("KICK", [chan, nick])
    } else {
        Message::new("KICK", [chan, nick, reason])
    }
}

/// Sorte de ligne d'après un nom de couleur d'événement (`/echo -c info …`).
fn kind_from_name(n: &str) -> Option<LineKind> {
    Some(match n.to_ascii_lowercase().as_str() {
        "action" => LineKind::Action,
        "ctcp" => LineKind::Ctcp,
        "highlight" => LineKind::Highlight,
        "info" => LineKind::Info,
        "info2" => LineKind::Info2,
        "invite" => LineKind::Invite,
        "join" => LineKind::Join,
        "kick" => LineKind::Kick,
        "mode" => LineKind::Mode,
        "nick" => LineKind::Nick,
        "normal" => LineKind::Normal,
        "notice" => LineKind::Notice,
        "notify" => LineKind::Notify,
        "other" => LineKind::Other,
        "own" => LineKind::Own,
        "part" => LineKind::Part,
        "quit" => LineKind::Quit,
        "topic" => LineKind::Topic,
        "wallops" => LineKind::Wallops,
        "whois" => LineKind::Whois,
        "error" => LineKind::Error,
        _ => return None,
    })
}

impl State {
    /// Connexion et fenêtre d'un contexte de commande.
    pub(crate) fn resolve(&self, ctx: &Ctx) -> (Option<ConnId>, WinId) {
        let active = self.active.and_then(|a| self.win(a));
        let conn = ctx.conn.filter(|c| self.conns.contains_key(c)).or_else(|| active.and_then(|w| w.conn));
        let win = ctx
            .window
            .as_deref()
            .and_then(|n| self.find_win(conn, n))
            .or_else(|| active.filter(|w| conn.is_none() || w.conn == conn || w.conn.is_none()).map(|w| w.id))
            .or_else(|| conn.and_then(|c| self.status_of(c)))
            .or(self.active)
            .unwrap_or(0);
        (conn, win)
    }

    fn insufficient(&self, cmd: &str) -> String {
        self.t(Key::InsufficientParams, &[("cmd", cmd)])
    }

    fn need_conn(&self, conn: Option<ConnId>) -> Result<ConnId, String> {
        match conn.and_then(|c| self.conns.get(&c)) {
            Some(c) if c.connected() => Ok(c.id),
            _ => Err(self.t(Key::NotConnected, &[])),
        }
    }

    fn is_chan(&self, conn: Option<ConnId>, name: &str) -> bool {
        match conn.and_then(|c| self.conns.get(&c)) {
            Some(c) => c.session.is_channel(name),
            None => name.starts_with(['#', '&']),
        }
    }

    /// Canal de la fenêtre courante.
    fn win_channel(&self, win: WinId) -> Option<String> {
        self.win(win).filter(|w| w.kind == WinKind::Channel).map(|w| w.name.clone())
    }

    /// `[#canal] reste` : le canal donné, ou celui de la fenêtre.
    fn chan_arg<'a>(&self, conn: Option<ConnId>, win: WinId, args: &'a str) -> (Option<String>, &'a str) {
        let (w, r) = split_word(args);
        if !w.is_empty() && self.is_chan(conn, w) {
            (Some(w.to_string()), r)
        } else {
            (self.win_channel(win), args.trim())
        }
    }

    /// Envoie un PRIVMSG/NOTICE (découpé selon la place disponible) et l'affiche si echo-message n'est pas là.
    pub(crate) fn say(&mut self, conn: ConnId, target: &str, text: &str, notice: bool, action: bool, quiet: bool) {
        let Some(c) = self.conns.get(&conn) else { return };
        let echo = c.session.cap("echo-message");
        let cmd = if notice { "NOTICE" } else { "PRIVMSG" };
        let mut budget = split::text_budget(cmd, target, c.session.source_len());
        if action {
            budget = budget.saturating_sub(9).max(32);
        }
        let now = self.now;
        for raw_line in text.split(['\n', '\r']).filter(|l| !l.is_empty()) {
            for chunk in split::split_text(raw_line, budget) {
                if chunk.is_empty() {
                    continue;
                }
                let body = if action { ctcp::encode("ACTION", &chunk) } else { chunk };
                self.send_line(conn, &text_line(cmd, target, &body));
                if !echo && !quiet {
                    let kind = if notice { MsgKind::Notice } else { MsgKind::Privmsg };
                    for l in self.own_lines(conn, kind, target, &body, now) {
                        self.add_line(l);
                    }
                }
            }
        }
    }

    /// Texte simple tapé dans une fenêtre.
    pub(crate) fn send_plain(&mut self, win: WinId, text: &str) {
        let Some(w) = self.win(win) else { return };
        let (kind, conn, name) = (w.kind, w.conn, w.name.clone());
        match kind {
            WinKind::Channel | WinKind::Query => match self.need_conn(conn) {
                Ok(c) => self.say(c, &name, text, false, false, false),
                Err(e) => self.print(win, LineKind::Error, e),
            },
            WinKind::Status => {
                let t = self.t(Key::NotOnChannel, &[]);
                self.print(win, LineKind::Error, t);
            }
            WinKind::DccChat => self.dcc_chat_say(win, text),
            _ => {}
        }
    }

    fn raw_to_server(&mut self, conn: Option<ConnId>, line: &str) -> Result<(), String> {
        let c = self.need_conn(conn)?;
        self.send_line(c, line);
        Ok(())
    }

    /// Ouvre (ou montre) un privé.
    pub(crate) fn open_query(&mut self, conn: ConnId, nick: &str, activate: bool) -> WinId {
        let w = match self.find_kind(conn, WinKind::Query, nick) {
            Some(w) => w,
            None => {
                let w = self.create_window(WinKind::Query, Some(conn), nick);
                self.update_titles(conn);
                if self.conns.get(&conn).is_some_and(|c| c.connected()) {
                    self.request_history(conn, nick, None);
                }
                w
            }
        };
        if activate {
            self.activate(w);
        }
        w
    }

    /// Ferme une fenêtre (événement CLOSE pour les privés et les fenêtres `@`).
    pub(crate) fn close_win(&mut self, win: WinId) {
        let Some(w) = self.win(win) else { return };
        let (kind, conn, name, joined) = (w.kind, w.conn, w.name.clone(), w.joined);
        match kind {
            WinKind::Channel => {
                if let Some(c) = conn
                    && joined
                    && self.conns.get(&c).is_some_and(|x| x.connected())
                {
                    self.send_msg(c, Message::new("PART", [name]));
                    // La fenêtre se ferme à l'écho du PART.
                } else {
                    self.remove_window(win);
                }
            }
            WinKind::Status => {
                let Some(c) = conn else { return };
                let statuses = self.windows.iter().filter(|w| w.kind == WinKind::Status).count();
                let reason = self.settings.identity.quit_message.clone();
                self.user_disconnect(c, &reason);
                if statuses > 1 {
                    let children: Vec<WinId> = self.windows.iter().filter(|w| w.conn == Some(c)).map(|w| w.id).collect();
                    for ch in children {
                        self.remove_window(ch);
                    }
                    self.conns.remove(&c);
                    for p in self.conns.values_mut() {
                        p.bouncer_children.retain(|_, v| *v != c);
                    }
                }
            }
            _ => {
                let mut ev = self.ev(EventKind::Close, conn.unwrap_or(0));
                ev.conn = conn;
                ev.target = Some(name.clone());
                ev.nick = if kind == WinKind::Query { name.clone() } else { String::new() };
                let ctx = self.ctx(conn, Some(win));
                self.push_pres(Pres::new().event(ctx, ev));
                self.remove_window(win);
            }
        }
    }

    /// Demande les listes de bans, d'exceptions et d'invitations d'un canal (Channel Central), sans affichage.
    pub(crate) fn request_lists(&mut self, win: WinId) {
        let Some(w) = self.win(win) else { return };
        let (Some(conn), name) = (w.conn, w.name.clone()) else { return };
        if w.kind != WinKind::Channel {
            return;
        }
        let Some(c) = self.conns.get_mut(&conn) else { return };
        if !c.connected() {
            return;
        }
        let (e, i) = (c.session.isupport().excepts, c.session.isupport().invex);
        c.session.request_list(&name, 'b', true);
        if let Some(e) = e {
            c.session.request_list(&name, e, true);
        }
        if let Some(i) = i {
            c.session.request_list(&name, i, true);
        }
        self.drain_session(conn);
    }

    /// Exécute une commande intégrée. `args` est déjà évalué par le moteur.
    pub(crate) fn run_builtin(&mut self, ctx: &Ctx, name: &str, args: &str, quiet: bool) -> Result<(), String> {
        let (conn, win) = self.resolve(ctx);
        let args = args.trim();
        let name = name.to_ascii_lowercase();
        match name.as_str() {
            "echo" => self.cmd_echo(conn, win, args),
            "say" => {
                if args.is_empty() {
                    return Err(self.insufficient(&name));
                }
                self.send_plain(win, args);
            }
            "msg" | "privmsg" => {
                let (target, text) = split_word(args);
                if target.is_empty() || text.is_empty() {
                    return Err(self.insufficient(&name));
                }
                if target.starts_with('=') {
                    // `/msg =pseudo texte` : discussion DCC.
                    return self.dcc_msg(conn, target, text);
                }
                let c = self.need_conn(conn)?;
                for t in target.split(',').filter(|t| !t.is_empty()) {
                    self.say(c, t, text, false, false, quiet);
                }
            }
            "notice" => {
                let (target, text) = split_word(args);
                if target.is_empty() || text.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                for t in target.split(',').filter(|t| !t.is_empty()) {
                    self.say(c, t, text, true, false, quiet);
                }
            }
            "me" | "action" => {
                if args.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let w = self.win(win).cloned();
                match w {
                    Some(w) if matches!(w.kind, WinKind::Channel | WinKind::Query) => {
                        let c = self.need_conn(w.conn)?;
                        self.say(c, &w.name, args, false, true, quiet);
                    }
                    Some(w) if w.kind == WinKind::DccChat => self.dcc_chat_say(w.id, &ctcp::encode("ACTION", args)),
                    _ => return Err(self.t(Key::NotOnChannel, &[])),
                }
            }
            "describe" => {
                let (target, text) = split_word(args);
                if target.is_empty() || text.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                self.say(c, target, text, false, true, quiet);
            }
            "amsg" | "ame" => {
                if args.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                let chans: Vec<String> = self.conns[&c].session.channels().filter(|ch| ch.joined).map(|ch| ch.name.clone()).collect();
                for ch in chans {
                    self.say(c, &ch, args, false, name == "ame", quiet);
                }
            }
            "omsg" | "onotice" => {
                let (chan, text) = self.chan_arg(conn, win, args);
                let Some(chan) = chan else { return Err(self.insufficient(&name)) };
                if text.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                let sm = self.conns[&c].session.isupport().statusmsg.clone();
                let prefix = if sm.contains('@') || sm.is_empty() { '@' } else { sm.chars().next().unwrap_or('@') };
                self.say(c, &format!("{prefix}{chan}"), text, name == "onotice", false, quiet);
            }
            "ctcp" => {
                let (target, rest) = split_word(args);
                let (cmd, params) = split_word(rest);
                if target.is_empty() || cmd.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                let cmd = cmd.to_ascii_uppercase();
                let params = if cmd == "PING" && params.is_empty() { self.now.to_string() } else { params.to_string() };
                let body = ctcp::encode(&cmd, &params);
                self.send_line(c, &text_line("PRIVMSG", target, &body));
                let echo = self.conns[&c].session.cap("echo-message");
                if !echo && !quiet {
                    for l in self.own_lines(c, MsgKind::Privmsg, target, &body, self.now) {
                        self.add_line(l);
                    }
                }
            }
            "ctcpreply" => {
                let (target, rest) = split_word(args);
                let (cmd, params) = split_word(rest);
                if target.is_empty() || cmd.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                self.send_line(c, &text_line("NOTICE", target, &ctcp::encode(&cmd.to_ascii_uppercase(), params)));
            }
            "query" => {
                let (nick, text) = split_word(args);
                if nick.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let Some(c) = conn else { return Err(self.t(Key::NotConnected, &[])) };
                self.open_query(c, nick, true);
                if !text.is_empty() {
                    let c = self.need_conn(Some(c))?;
                    self.say(c, nick, text, false, false, quiet);
                }
            }
            "join" => {
                let (_sw, rest) = switches(args);
                let (chans, keys) = split_word(rest);
                if chans.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                if chans == "0" {
                    self.send_line(c, "JOIN 0");
                    return Ok(());
                }
                let fixed: Vec<String> = chans
                    .split(',')
                    .filter(|x| !x.is_empty())
                    .map(|x| if self.is_chan(Some(c), x) { x.to_string() } else { format!("#{x}") })
                    .collect();
                // Déjà sur le canal : on montre sa fenêtre.
                if fixed.len() == 1
                    && let Some(w) = self.find_kind(c, WinKind::Channel, &fixed[0])
                    && self.win(w).is_some_and(|x| x.joined)
                {
                    self.activate(w);
                    return Ok(());
                }
                let mut line = format!("JOIN {}", fixed.join(","));
                if !keys.is_empty() {
                    line.push(' ');
                    line.push_str(keys.split_whitespace().next().unwrap_or(""));
                }
                self.send_line(c, &line);
            }
            "part" => {
                let (chan, reason) = self.chan_arg(conn, win, args);
                let Some(chan) = chan else { return Err(self.insufficient(&name)) };
                let joined = conn.and_then(|c| self.find_kind(c, WinKind::Channel, &chan)).and_then(|w| self.win(w)).map(|w| (w.id, w.joined));
                if let Some((w, false)) = joined {
                    self.remove_window(w);
                    return Ok(());
                }
                let c = self.need_conn(conn)?;
                let m = if reason.is_empty() { Message::new("PART", [chan]) } else { Message::new("PART", [chan, reason.to_string()]) };
                self.send_msg(c, m);
            }
            "partall" => {
                let c = self.need_conn(conn)?;
                let chans: Vec<String> = self.conns[&c].session.channels().filter(|ch| ch.joined).map(|ch| ch.name.clone()).collect();
                for ch in chans {
                    let m = if args.is_empty() { Message::new("PART", [ch]) } else { Message::new("PART", [ch, args.to_string()]) };
                    self.send_msg(c, m);
                }
            }
            "hop" | "cycle" => {
                let (chan, _) = self.chan_arg(conn, win, args);
                let Some(chan) = chan else { return Err(self.insufficient(&name)) };
                let c = self.need_conn(conn)?;
                let key = self.conns[&c].session.channel(&chan).and_then(|ch| ch.key().map(str::to_string));
                self.send_msg(c, Message::new("PART", [chan.clone()]));
                match key {
                    Some(k) => self.send_msg(c, Message::new("JOIN", [chan, k])),
                    None => self.send_msg(c, Message::new("JOIN", [chan])),
                }
            }
            "nick" => {
                let (nick, _) = split_word(args);
                if nick.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let Some(c) = conn else { return Err(self.t(Key::NotConnected, &[])) };
                if self.conns.get(&c).is_some_and(|x| x.connected()) {
                    self.send_msg(c, Message::new("NICK", [nick]));
                } else {
                    let st = self.status_of(c).unwrap_or(win);
                    if let Some(x) = self.conns.get_mut(&c) {
                        x.nick_override = Some(nick.to_string());
                        x.session.config_mut().nick = nick.to_string();
                        let cfg = x.session.config().clone();
                        x.session = crate::session::Session::new(cfg);
                    }
                    let t = self.t(Key::OwnNick, &[("new", nick)]);
                    self.print(st, LineKind::Nick, t);
                    self.update_titles(c);
                }
            }
            "quit" => {
                let Some(c) = conn else { return Ok(()) };
                let reason = if args.is_empty() { self.settings.identity.quit_message.clone() } else { args.to_string() };
                self.user_disconnect(c, &reason);
            }
            "disconnect" => {
                let Some(c) = conn else { return Ok(()) };
                let reason = if args.is_empty() { self.settings.identity.quit_message.clone() } else { args.to_string() };
                self.user_disconnect(c, &reason);
            }
            "server" => self.cmd_server(conn, win, args)?,
            "exit" => {
                let ev = alpirc_script::EventInfo { kind: Some(EventKind::Exit), ..alpirc_script::EventInfo::default() };
                self.push_pres(Pres::new().event(Ctx::default(), ev));
                let reason = self.settings.identity.quit_message.clone();
                let ids: Vec<ConnId> = self.conns.keys().copied().collect();
                for id in ids {
                    self.user_disconnect(id, &reason);
                }
                self.effects.push(Effect::Exit);
            }
            "away" | "back" => {
                let c = self.need_conn(conn)?;
                let msg = if name == "back" { None } else { Some(args).filter(|a| !a.is_empty()) };
                let co = self.conns.get_mut(&c).unwrap();
                co.session.set_away(msg);
                self.drain_session(c);
                // Une absence choisie (ou un retour) n'est plus l'affaire de l'absence automatique.
                self.auto_away.retain(|x| *x != c);
            }
            "whois" | "uwho" => {
                if args.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                let target = if self.settings.irc.whois_in_active { win } else { self.status_of(c).unwrap_or(win) };
                if let Some(x) = self.conns.get_mut(&c) {
                    x.whois_win = Some(target);
                }
                self.send_line(c, &format!("WHOIS {args}"));
            }
            "names" => {
                let (chan, _) = self.chan_arg(conn, win, args);
                let c = self.need_conn(conn)?;
                match chan {
                    Some(ch) => self.send_line(c, &format!("NAMES {ch}")),
                    None => self.send_line(c, "NAMES"),
                }
            }
            "topic" => {
                let (chan, text) = self.chan_arg(conn, win, args);
                let Some(chan) = chan else { return Err(self.insufficient(&name)) };
                let c = self.need_conn(conn)?;
                if text.is_empty() {
                    self.send_msg(c, Message::new("TOPIC", [chan]));
                } else {
                    self.send_msg(c, Message::new("TOPIC", [chan, text.to_string()]));
                }
            }
            "mode" => {
                let c = self.need_conn(conn)?;
                if args.is_empty() {
                    match self.win_channel(win) {
                        Some(ch) => self.send_line(c, &format!("MODE {ch}")),
                        None => return Err(self.insufficient(&name)),
                    }
                } else {
                    self.send_line(c, &format!("MODE {args}"));
                }
            }
            "op" | "deop" | "voice" | "devoice" | "halfop" | "dehalfop" | "dehop" => {
                let (chan, nicks) = self.chan_arg(conn, win, args);
                let Some(chan) = chan else { return Err(self.insufficient(&name)) };
                if nicks.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                let (add, mode) = match name.as_str() {
                    "op" => (true, 'o'),
                    "deop" => (false, 'o'),
                    "voice" => (true, 'v'),
                    "devoice" => (false, 'v'),
                    "halfop" => (true, 'h'),
                    _ => (false, 'h'),
                };
                let ch: Vec<ModeChange> = nicks
                    .split([' ', ','])
                    .filter(|n| !n.is_empty())
                    .map(|n| ModeChange { add, mode, arg: Some(n.to_string()) })
                    .collect();
                let per = self.conns[&c].session.isupport().modes_per_line;
                for (m, a) in modes::build(&ch, per) {
                    self.send_line(c, &format!("MODE {chan} {m} {}", a.join(" ")));
                }
            }
            "kick" => {
                let (chan, rest) = self.chan_arg(conn, win, args);
                let Some(chan) = chan else { return Err(self.insufficient(&name)) };
                let (nick, reason) = split_word(rest);
                if nick.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                let m = if reason.is_empty() {
                    Message::new("KICK", [chan, nick.to_string()])
                } else {
                    Message::new("KICK", [chan, nick.to_string(), reason.to_string()])
                };
                self.send_msg(c, m);
            }
            "ban" => self.cmd_ban(conn, win, args)?,
            "invite" => {
                let (nick, rest) = split_word(args);
                if nick.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let chan = if rest.is_empty() { self.win_channel(win) } else { Some(split_word(rest).0.to_string()) };
                let Some(chan) = chan else { return Err(self.insufficient(&name)) };
                let c = self.need_conn(conn)?;
                self.send_msg(c, Message::new("INVITE", [nick.to_string(), chan]));
            }
            "list" => {
                let c = self.need_conn(conn)?;
                let mut words: Vec<&str> = args.split_whitespace().collect();
                let mut take_num = |flag: &str| -> Option<u32> {
                    let i = words.iter().position(|w| w.eq_ignore_ascii_case(flag))?;
                    let v = words.get(i + 1).and_then(|w| w.parse().ok());
                    words.drain(i..(i + 2).min(words.len()));
                    v
                };
                let min = take_num("-min");
                let max = take_num("-max");
                let filter = words.first().map(|s| s.to_string());
                let w = self.chanlist_prepare(c, filter.clone(), min, max);
                self.activate(w);
                let t = self.t(Key::ListStart, &[]);
                self.print(w, LineKind::Info, t);
                match filter.filter(|f| !f.contains(['*', '?']) && self.is_chan(Some(c), f)) {
                    Some(f) => self.send_line(c, &format!("LIST {f}")),
                    None => self.send_line(c, "LIST"),
                }
            }
            "quote" | "raw" => {
                let (sw, line) = switches(args);
                if line.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                self.send_line(c, line);
                if !sw.contains('q') && !quiet {
                    let st = self.status_of(c).unwrap_or(win);
                    let srv = self.conns[&c].host.clone();
                    self.print(st, LineKind::Info, format!("-> {srv} {line}"));
                }
            }
            "links" | "lusers" | "motd" | "admin" | "info" | "time" | "version" | "stats" | "who" | "whowas" | "ping" | "monitor"
            | "map" | "knock" | "silence" => {
                let line = if args.is_empty() { name.to_ascii_uppercase() } else { format!("{} {args}", name.to_ascii_uppercase()) };
                self.raw_to_server(conn, &line)?;
            }
            "setname" => {
                if args.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let c = self.need_conn(conn)?;
                self.send_msg(c, Message::new("SETNAME", [args]));
            }
            "lag" => {
                let c = self.need_conn(conn)?;
                let now = self.now;
                self.send_now(c, &format!("PING :ALPIRC{now}"));
                if let Some(x) = self.conns.get_mut(&c) {
                    x.lag_requested = Some(win);
                    x.lag_sent = Some(now);
                }
            }
            "clear" => {
                let (sw, rest) = switches(args);
                let target = if sw.contains('s') {
                    conn.and_then(|c| self.status_of(c))
                } else if sw.contains('a') || rest.is_empty() {
                    Some(win)
                } else {
                    self.find_win(conn, rest)
                };
                let Some(t) = target else { return Err(self.t(Key::NoSuchWindow, &[("cmd", &name), ("name", rest)])) };
                if let Some(w) = self.win_mut(t) {
                    w.lines.clear();
                    w.unread_marker = None;
                    w.touch();
                }
            }
            "clearall" => {
                for w in self.windows.iter_mut() {
                    w.lines.clear();
                    w.unread_marker = None;
                    w.touch();
                }
            }
            "close" => self.cmd_close(conn, win, args),
            "window" => self.cmd_window(conn, args)?,
            "aline" | "dline" | "rline" | "iline" | "sline" | "cline" => self.cmd_lines(conn, &name, args)?,
            "ignore" => self.cmd_ignore(conn, win, args)?,
            "notify" => self.cmd_notify(conn, win, args)?,
            // Demande de son (CTCP SOUND) : voir `sounds.rs`.
            "sound" => self.cmd_sound(conn, win, args, quiet)?,
            "splay" => {
                let (_, file) = switches(args);
                if file.is_empty() {
                    return Err(self.insufficient(&name));
                }
                let p = std::path::Path::new(file);
                let path = if p.is_absolute() { p.to_path_buf() } else { self.data_dir.join("sounds").join(p) };
                // Bac à sable : pas de remontée hors du dossier des sons.
                if !p.is_absolute() && p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                    return Err(self.t(Key::SoundNotFound, &[("file", file)]));
                }
                if !path.is_file() {
                    return Err(self.t(Key::SoundNotFound, &[("file", file)]));
                }
                self.effects.push(Effect::PlaySound(path));
            }
            "beep" => {
                let n: usize = split_word(args).0.parse().unwrap_or(1).clamp(1, 10);
                for _ in 0..n {
                    self.effects.push(Effect::Beep);
                }
            }
            "flash" => {
                let (_, rest) = switches(args);
                let (w0, _) = split_word(rest);
                let target = if w0.is_empty() { win } else { self.find_win(conn, w0).unwrap_or(win) };
                self.effects.push(Effect::Flash { win: target });
            }
            "url" => {
                let (_, rest) = switches(args);
                if rest.is_empty() {
                    let w = match self.windows.iter().find(|w| w.kind == WinKind::Urls) {
                        Some(w) => w.id,
                        None => self.create_window(WinKind::Urls, None, "URLs"),
                    };
                    self.activate(w);
                } else {
                    let (u, _) = split_word(rest);
                    self.effects.push(Effect::OpenUrl(u.to_string()));
                }
            }
            "run" => return Err(self.t(Key::RunDisabled, &[])),
            "dns" => return Err(self.t(Key::DnsUnsupported, &[])),
            "dcc" => self.cmd_dcc(conn, win, args)?,
            "log" => {
                let (onoff, rest) = split_word(args);
                let target = if rest.is_empty() { win } else { self.find_win(conn, rest).unwrap_or(win) };
                let wname = self.win(target).map(|w| w.name.clone()).unwrap_or_default();
                match onoff.to_ascii_lowercase().as_str() {
                    "on" => {
                        self.log_off.remove(&target);
                        if !self.settings.logging.enabled {
                            self.settings.logging.enabled = true;
                            self.effects.push(Effect::SaveSettings);
                        }
                        let t = self.t(Key::LoggingOn, &[("win", &wname)]);
                        self.print(target, LineKind::Info, t);
                    }
                    "off" => {
                        self.log_off.insert(target);
                        if let Some(p) = self.log_path(target) {
                            let d = self.asctime(self.now);
                            self.logger.close(&p, &d);
                        }
                        let t = self.t(Key::LoggingOff, &[("win", &wname)]);
                        self.print(target, LineKind::Info, t);
                    }
                    _ => return Err(self.insufficient(&name)),
                }
            }
            "channel" => {
                let target = match split_word(args).0 {
                    "" => win,
                    ch => conn.and_then(|c| self.find_kind(c, WinKind::Channel, ch)).unwrap_or(win),
                };
                if self.win(target).is_some_and(|w| w.kind == WinKind::Channel) {
                    self.request_lists(target);
                    self.effects.push(Effect::OpenDialog(UiDialog::ChannelCentral(target)));
                } else {
                    return Err(self.t(Key::NotOnChannel, &[]));
                }
            }
            "options" | "font" => {
                let page = if name == "font" { Some("display".to_string()) } else { Some(args.to_string()).filter(|a| !a.is_empty()) };
                self.effects.push(Effect::OpenDialog(UiDialog::Options(page)));
            }
            "abook" => self.effects.push(Effect::OpenDialog(UiDialog::AddressBook)),
            "folder" => self.effects.push(Effect::OpenDialog(UiDialog::ChannelsFolder)),
            "colors" => self.effects.push(Effect::OpenDialog(UiDialog::Colors)),
            "about" => self.effects.push(Effect::OpenDialog(UiDialog::About)),
            "scripts" => self.effects.push(Effect::OpenDialog(UiDialog::ScriptEditor)),
            "find" => self.effects.push(Effect::OpenDialog(UiDialog::Find(win))),
            "help" => self.cmd_help(win, args),
            "linesep" => {
                let target = if args.is_empty() { win } else { self.find_win(conn, args).unwrap_or(win) };
                let mut l = self.pl(target, LineKind::Info, "-");
                l.no_stamp = true;
                self.add_line(l);
            }
            "timestamp" => self.cmd_timestamp(win, args),
            "ial" => {
                let t = self.t(Key::IalAlwaysOn, &[]);
                self.print(win, LineKind::Info, t);
            }
            "closemsg" => {
                // `/closemsg [pseudo]` : comme `/close -m [pseudo]`.
                let names = if args.is_empty() { String::new() } else { format!(" {args}") };
                self.cmd_close(conn, win, &format!("-m{names}"));
            }
            "flush" => {}
            _ => {
                // Commande inconnue : envoyée au serveur telle quelle, comme dans mIRC.
                let line = if args.is_empty() { name.to_ascii_uppercase() } else { format!("{} {args}", name.to_ascii_uppercase()) };
                self.raw_to_server(conn, &line).map_err(|_| self.t(Key::UnknownCommand, &[("cmd", &name)]))?;
            }
        }
        Ok(())
    }

    // --- /echo ---

    fn cmd_echo(&mut self, conn: Option<ConnId>, win: WinId, args: &str) {
        let mut rest = args;
        let mut color = None;
        let (w0, r0) = split_word(rest);
        if !r0.is_empty() && !w0.is_empty() && w0.chars().all(|c| c.is_ascii_digit()) {
            color = w0.parse::<u8>().ok();
            rest = r0;
        }
        let (sw, r) = switches(rest);
        rest = r;
        let mut kind = LineKind::Normal;
        if sw.contains('c') {
            let (cn, r) = split_word(rest);
            if let Some(k) = kind_from_name(cn) {
                kind = k;
            }
            rest = r;
        }
        let target = if sw.contains('a') {
            self.active.unwrap_or(win)
        } else if sw.contains('s') || sw.contains('d') {
            conn.and_then(|c| self.status_of(c)).unwrap_or(win)
        } else {
            let (w, r) = split_word(rest);
            match self.find_win(conn, w.trim_start_matches('=')) {
                Some(x) if !w.is_empty() && (!r.is_empty() || w.starts_with(['@', '#'])) => {
                    rest = r;
                    x
                }
                _ => win,
            }
        };
        let mut l = self.pl(target, kind, rest.to_string());
        l.color = color;
        l.no_stamp = !sw.contains('t');
        l.nolog = sw.contains('g');
        if sw.contains('e') {
            let mut sep = l.clone();
            sep.text = "-".into();
            self.add_line(sep.clone());
            self.add_line(l);
            self.add_line(sep);
        } else {
            self.add_line(l);
        }
    }

    // --- /server ---

    fn cmd_server(&mut self, conn: Option<ConnId>, win: WinId, args: &str) -> Result<(), String> {
        let (sw, rest) = switches(args);
        let words: Vec<&str> = rest.split_whitespace().collect();
        let status = if sw.contains('m') {
            self.new_status_window()
        } else {
            conn.and_then(|c| self.status_of(c)).or_else(|| self.windows.iter().find(|w| w.kind == WinKind::Status).map(|w| w.id)).unwrap_or(win)
        };
        let Some(c) = self.conn_of_status(status) else { return Ok(()) };
        if sw.contains('m') {
            self.activate(status);
        }
        if words.is_empty() {
            // Reconnexion au même serveur.
            let has = self.conns.get(&c).is_some_and(|x| !x.servers.is_empty());
            if !has {
                self.effects.push(Effect::OpenDialog(UiDialog::Connect));
                return Ok(());
            }
            let quit = self.settings.identity.quit_message.clone();
            if self.conns.get(&c).is_some_and(|x| x.state != super::ConnState::Disconnected) {
                self.drop_and_keep(c, &quit);
            }
            if let Some(x) = self.conns.get_mut(&c) {
                x.user_quit = false;
                x.attempts = 0;
            }
            self.do_connect(c);
            return Ok(());
        }
        let first = words[0];
        if self.settings.network(first).is_some() && !first.contains('.') || self.settings.network(first).is_some() && words.len() == 1 {
            self.connect_network(status, first, None);
            return Ok(());
        }
        let port_word = words.get(1).copied().filter(|w| w.trim_start_matches('+').chars().all(|c| c.is_ascii_digit()) && !w.is_empty());
        let (host, port, mut tls) = super::conn::parse_host_port(first, port_word);
        if sw.contains('e') {
            tls = true;
        }
        let pass_start = if port_word.is_some() { 2 } else { 1 };
        let password = if words.len() > pass_start { Some(words[pass_start..].join(" ")) } else { None };
        let mut s = Server::new(&host, port, tls);
        s.password = password;
        self.connect_adhoc(c, s);
        Ok(())
    }

    /// Ferme la connexion en cours sans perdre la liste des canaux à rejoindre.
    fn drop_and_keep(&mut self, conn: ConnId, reason: &str) {
        if self.conns.get(&conn).is_some_and(|c| c.connected()) {
            self.send_now(conn, &Message::new("QUIT", [reason]).to_line());
            self.flush_sends();
        }
        self.close_effect(conn);
        if let Some(c) = self.conns.get_mut(&conn) {
            c.user_quit = true;
        }
        self.conn_closed(conn, None);
    }

    // --- /ban ---

    fn cmd_ban(&mut self, conn: Option<ConnId>, win: WinId, args: &str) -> Result<(), String> {
        let (sw, rest) = switches(args);
        let (chan, rest) = self.chan_arg(conn, win, rest);
        let Some(chan) = chan else { return Err(self.insufficient("ban")) };
        let (who, rest) = split_word(rest);
        if who.is_empty() {
            return Err(self.insufficient("ban"));
        }
        let (t, after) = split_word(rest);
        let (mask_type, reason) = match t.parse::<u8>() {
            Ok(n) if n <= 19 => (Some(n), after),
            _ => (None, rest),
        };
        let c = self.need_conn(conn)?;
        let remove = sw.contains('r');
        let kick = sw.contains('k').then(|| reason.to_string());
        let unban = switch_num(&sw, 'u').filter(|n| *n > 0);
        let sign = if remove { '-' } else { '+' };
        let is_mask = who.contains(['!', '@', '*']);
        let resolved: Option<String> = if is_mask {
            Some(who.to_string())
        } else {
            match mask_type {
                None => Some(format!("{who}!*@*")),
                Some(ty) => self.conns[&c]
                    .session
                    .user_info(who)
                    .and_then(|u| Some(mask::address_mask(&u.nick, u.user.as_deref()?, u.host.as_deref()?, ty))),
            }
        };
        match resolved {
            Some(m) => {
                self.send_line(c, &format!("MODE {chan} {sign}b {m}"));
                if let Some(r) = kick
                    && !is_mask
                {
                    self.send_msg(c, kick_msg(&chan, who, &r));
                }
                if let Some(s) = unban
                    && !remove
                {
                    let due = self.now + s as i64 * 1000;
                    self.conns.get_mut(&c).unwrap().timed_unbans.push((due, chan, m));
                }
            }
            None => {
                // Adresse inconnue : USERHOST d'abord, le ban suit la réponse.
                let co = self.conns.get_mut(&c).unwrap();
                co.pending_bans.push(PendingBan { chan, nick: who.to_string(), mask_type: mask_type.unwrap_or(2), kick, unban_secs: unban });
                co.session.request_userhost(&[who.to_string()], true);
                self.drain_session(c);
            }
        }
        Ok(())
    }

    // --- /close ---

    fn cmd_close(&mut self, conn: Option<ConnId>, win: WinId, args: &str) {
        let (sw, rest) = switches(args);
        if sw.is_empty() {
            let target = if rest.is_empty() { Some(win) } else { self.find_win(conn, rest) };
            if let Some(t) = target {
                self.close_win(t);
            }
            return;
        }
        let names: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
        let kinds: Vec<WinKind> = sw
            .chars()
            .filter_map(|c| match c {
                'm' => Some(WinKind::Query),
                '@' => Some(WinKind::Custom),
                'c' => Some(WinKind::DccChat),
                _ => None,
            })
            .collect();
        let ids: Vec<WinId> = self
            .windows
            .iter()
            .filter(|w| kinds.contains(&w.kind))
            .filter(|w| w.kind == WinKind::Custom || conn.is_none() || w.conn == conn)
            .filter(|w| names.is_empty() || names.iter().any(|n| self.names_match(w.conn, &w.name, n)))
            .map(|w| w.id)
            .collect();
        for id in ids {
            self.close_win(id);
        }
    }

    fn names_match(&self, conn: Option<ConnId>, a: &str, pat: &str) -> bool {
        let cm = conn.and_then(|c| self.conns.get(&c)).map(|c| c.session.casemapping()).unwrap_or_default();
        mask::wildcard_match(pat, a, cm)
    }

    // --- fenêtres @ ---

    /// `/window [-acehlnrx] nom` : fenêtres `@` (créées au besoin) et toutes les autres (canal, privé, Status,
    /// `=discussion`, Notify…) : `-a` activer, `-c` fermer, `-n` réduire, `-x` agrandir, `-r` restaurer.
    fn cmd_window(&mut self, conn: Option<ConnId>, args: &str) -> Result<(), String> {
        let (sw, rest) = switches(args);
        let (name, _) = split_word(rest);
        let custom = name.starts_with('@');
        if name.is_empty() || custom && name.len() < 2 {
            return Err(self.insufficient("window"));
        }
        let existing = if custom {
            self.find_win(None, name)
        } else {
            self.find_win(conn, name)
                .or_else(|| self.windows.iter().find(|w| w.name.eq_ignore_ascii_case(name)).map(|w| w.id))
        };
        if !custom && existing.is_none() {
            return Err(self.t(Key::NoSuchWindow, &[("cmd", "window"), ("name", name)]));
        }
        if sw.contains('c') {
            if let Some(w) = existing {
                self.close_win(w);
            }
            return Ok(());
        }
        let w = match existing {
            Some(w) => w,
            None => self.create_window(WinKind::Custom, conn, name),
        };
        if let Some(x) = self.win_mut(w) {
            if custom && sw.contains('l') {
                x.listbox = true;
            }
            if custom && sw.contains('e') {
                x.editbox = true;
            }
            x.touch();
        }
        if sw.contains('a') || existing.is_none() && !sw.contains('n') && !sw.contains('h') {
            self.activate(w);
        }
        let state = if sw.contains('n') {
            Some(super::WinState::Minimize)
        } else if sw.contains('x') {
            Some(super::WinState::Maximize)
        } else if sw.contains('r') {
            Some(super::WinState::Restore)
        } else {
            None
        };
        if let Some(state) = state {
            self.effects.push(Effect::WindowState { win: w, state });
        }
        Ok(())
    }

    fn cmd_lines(&mut self, _conn: Option<ConnId>, name: &str, args: &str) -> Result<(), String> {
        let (sw, mut rest) = switches(args);
        let listbox = sw.contains('l');
        let mut color = None;
        if name != "dline" && name != "sline" {
            let (w0, r0) = split_word(rest);
            if w0.chars().all(|c| c.is_ascii_digit()) && !w0.is_empty() {
                color = w0.parse::<u8>().ok();
                rest = r0;
            }
        }
        let (wname, rest) = split_word(rest);
        let Some(win) = self.find_win(None, wname) else {
            return Err(self.t(Key::NoSuchWindow, &[("cmd", name), ("name", wname)]));
        };
        let now = self.now;
        match name {
            "aline" => {
                if listbox {
                    if let Some(w) = self.win_mut(win) {
                        w.list_lines.push(rest.to_string());
                        w.touch();
                    }
                } else {
                    let mut l: PendingLine = self.pl(win, LineKind::Normal, rest.to_string());
                    l.color = color;
                    l.no_stamp = true;
                    l.nolog = true;
                    l.activity = if sw.contains('h') { Activity::Event } else { Activity::None };
                    self.add_line(l);
                }
            }
            "dline" => {
                let (spec, _) = split_word(rest);
                let (a, b) = match spec.split_once('-') {
                    Some((a, b)) => (a.parse::<usize>().unwrap_or(0), b.parse::<usize>().unwrap_or(usize::MAX)),
                    None => {
                        let n = spec.parse::<usize>().unwrap_or(0);
                        (n, n)
                    }
                };
                if a == 0 {
                    return Err(self.insufficient(name));
                }
                if let Some(w) = self.win_mut(win) {
                    if listbox {
                        let end = b.min(w.list_lines.len());
                        if a <= end {
                            w.list_lines.drain(a - 1..end);
                        }
                    } else {
                        let end = b.min(w.lines.len());
                        if a <= end {
                            w.lines.drain(a - 1..end);
                        }
                    }
                    w.touch();
                }
            }
            "rline" | "iline" => {
                let (n, text) = split_word(rest);
                let Ok(n) = n.parse::<usize>() else { return Err(self.insufficient(name)) };
                if n == 0 {
                    return Err(self.insufficient(name));
                }
                let seq = self.next_seq;
                self.next_seq += 1;
                if let Some(w) = self.win_mut(win) {
                    if listbox {
                        if name == "rline" {
                            if let Some(x) = w.list_lines.get_mut(n - 1) {
                                *x = text.to_string();
                            }
                        } else {
                            let i = (n - 1).min(w.list_lines.len());
                            w.list_lines.insert(i, text.to_string());
                        }
                    } else {
                        let line = Line {
                            seq,
                            time_ms: now,
                            kind: LineKind::Normal,
                            text: text.to_string(),
                            nick: None,
                            highlight: false,
                            from_history: false,
                            msgid: None,
                            color,
                        };
                        if name == "rline" {
                            if let Some(x) = w.lines.get_mut(n - 1) {
                                *x = line;
                            }
                        } else {
                            let i = (n - 1).min(w.lines.len());
                            w.lines.insert(i, line);
                        }
                    }
                    w.touch();
                }
            }
            "sline" => {
                let (n, _) = split_word(rest);
                if let Some(w) = self.win_mut(win) {
                    let idx = n.parse::<usize>().ok().or_else(|| w.list_lines.iter().position(|l| l == n).map(|i| i + 1));
                    if let Some(i) = idx {
                        if sw.contains('r') {
                            w.sel.retain(|x| *x != i);
                        } else {
                            if !sw.contains('a') {
                                w.sel.clear();
                            }
                            if !w.sel.contains(&i) {
                                w.sel.push(i);
                            }
                        }
                    }
                    w.touch();
                }
            }
            "cline" => {
                let (n, _) = split_word(rest);
                let Ok(n) = n.parse::<usize>() else { return Err(self.insufficient(name)) };
                if let Some(w) = self.win_mut(win)
                    && let Some(l) = n.checked_sub(1).and_then(|i| w.lines.get_mut(i))
                {
                    l.color = color;
                    w.touch();
                }
            }
            _ => {}
        }
        Ok(())
    }

    // --- carnet d'adresses ---

    fn cmd_ignore(&mut self, conn: Option<ConnId>, win: WinId, args: &str) -> Result<(), String> {
        let (sw, rest) = switches(args);
        let (who, rest) = split_word(rest);
        if who.is_empty() || sw.contains('l') {
            let list: Vec<String> = self.settings.address_book.ignore.iter().map(|e| e.mask.clone()).collect();
            let t = if list.is_empty() { self.t(Key::ListEmpty, &[]) } else { self.t(Key::IgnoreList, &[("list", &list.join(", "))]) };
            self.print(win, LineKind::Info, t);
            return Ok(());
        }
        let (t, rest) = split_word(rest);
        let (ty, network) = match t.parse::<u8>() {
            Ok(n) if n <= 19 => (Some(n), split_word(rest).0),
            _ => (None, t),
        };
        let mask = if who.contains(['!', '@']) {
            who.to_string()
        } else {
            match ty {
                Some(ty) => conn
                    .and_then(|c| self.conns.get(&c))
                    .and_then(|c| c.session.user_info(who))
                    .and_then(|u| Some(mask::address_mask(&u.nick, u.user.as_deref()?, u.host.as_deref()?, ty)))
                    .unwrap_or_else(|| format!("{who}!*@*")),
                None => format!("{who}!*@*"),
            }
        };
        let network = if network.is_empty() { None } else { Some(network.to_string()) };
        if sw.contains('r') {
            let before = self.settings.address_book.ignore.len();
            self.settings.address_book.ignore.retain(|e| !e.mask.eq_ignore_ascii_case(&mask) && !e.mask.eq_ignore_ascii_case(who));
            if self.settings.address_book.ignore.len() != before {
                self.effects.push(Effect::SaveSettings);
            }
            let t = self.t(Key::IgnoreRemoved, &[("mask", &mask)]);
            self.print(win, LineKind::Info, t);
            return Ok(());
        }
        let types: String = sw.chars().filter(|c| "pcntikd".contains(*c)).collect();
        let all = types.is_empty();
        let e = IgnoreEntry {
            mask: mask.clone(),
            private: all || types.contains('p'),
            channel: all || types.contains('c'),
            notice: all || types.contains('n'),
            ctcp: all || types.contains('t'),
            invite: all || types.contains('i'),
            codes: types.contains('k'),
            dcc: all || types.contains('d'),
            network,
            until_ms: switch_num(&sw, 'u').filter(|n| *n > 0).map(|s| self.now + s as i64 * 1000),
        };
        self.settings.address_book.ignore.retain(|x| !x.mask.eq_ignore_ascii_case(&mask));
        self.settings.address_book.ignore.push(e);
        self.effects.push(Effect::SaveSettings);
        let t = self.t(Key::IgnoreAdded, &[("mask", &mask)]);
        self.print(win, LineKind::Info, t);
        Ok(())
    }

    fn cmd_notify(&mut self, conn: Option<ConnId>, win: WinId, args: &str) -> Result<(), String> {
        let (sw, rest) = switches(args);
        let (nick, note) = split_word(rest);
        if nick.is_empty() && !sw.contains('s') {
            // Sans argument : la fenêtre Notify (pseudos et leur état, tenue à jour).
            let w = self.notify_window();
            self.activate(w);
            return Ok(());
        }
        if nick.is_empty() {
            let list: Vec<String> = self.settings.address_book.notify.iter().map(|e| e.nick.clone()).collect();
            let t = if list.is_empty() { self.t(Key::ListEmpty, &[]) } else { self.t(Key::NotifyList, &[("list", &list.join(", "))]) };
            self.print(win, LineKind::Info, t);
            return Ok(());
        }
        let ids: Vec<ConnId> = self.conns.iter().filter(|(_, c)| c.ready).map(|(id, _)| *id).collect();
        if sw.contains('r') {
            self.settings.address_book.notify.retain(|e| !e.nick.eq_ignore_ascii_case(nick));
            self.effects.push(Effect::SaveSettings);
            for id in ids {
                if self.conns[&id].monitor_sent {
                    self.send_line(id, &format!("MONITOR - {nick}"));
                }
                let c = self.conns.get_mut(&id).unwrap();
                let k = c.session.fold(nick);
                c.notify_online.remove(&k);
            }
            let t = self.t(Key::NotifyRemoved, &[("nick", nick)]);
            self.print(win, LineKind::Info, t);
            return Ok(());
        }
        if !self.settings.address_book.notify.iter().any(|e| e.nick.eq_ignore_ascii_case(nick)) {
            self.settings.address_book.notify.push(NotifyEntry { nick: nick.to_string(), note: note.to_string(), network: None });
            self.effects.push(Effect::SaveSettings);
        }
        let now = self.now;
        for id in ids {
            if self.conns[&id].monitor_sent {
                self.send_line(id, &format!("MONITOR + {nick}"));
            } else if let Some(c) = self.conns.get_mut(&id) {
                c.next_ison = Some(now);
            }
        }
        let _ = conn;
        let t = self.t(Key::NotifyAdded, &[("nick", nick)]);
        self.print(win, LineKind::Info, t);
        Ok(())
    }

    /// Texte d'une ligne, codes retirés.
    pub(crate) fn plain(text: &str) -> String {
        format::strip(text)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Fenêtre Notify, aide, horodatage, absence automatique
// ---------------------------------------------------------------------------------------------------------------

impl State {
    /// La fenêtre Notify (créée au besoin) : la liste Notify et l'état de chacun, tenue à jour par l'interface.
    pub(crate) fn notify_window(&mut self) -> WinId {
        if let Some(w) = self.windows.iter().find(|w| w.kind == WinKind::Notify) {
            return w.id;
        }
        let w = self.create_window(WinKind::Notify, None, "Notify");
        let title = self.t(Key::TitleNotify, &[]);
        if let Some(x) = self.win_mut(w) {
            x.title = title;
            x.editbox = false;
            x.touch();
        }
        w
    }

    /// `/help` (liste des commandes) et `/help commande` (usage et rôle).
    fn cmd_help(&mut self, win: WinId, args: &str) {
        let (cmd, _) = split_word(args);
        if !cmd.is_empty() {
            let t = match super::help::help_for(self.lang, cmd) {
                Some(h) => format!("* {h}"),
                None => self.t(Key::HelpNone, &[("cmd", cmd.trim_start_matches('/'))]),
            };
            self.print(win, LineKind::Info, t);
            return;
        }
        let head = self.t(Key::HelpHead, &[]);
        self.print(win, LineKind::Info, head);
        let mut line = String::from("*");
        for (c, _) in super::help::HELP {
            if line.len() + c.len() + 2 > 100 {
                self.print(win, LineKind::Info, std::mem::replace(&mut line, String::from("*")));
            }
            line.push_str(" /");
            line.push_str(c);
        }
        if line.len() > 1 {
            self.print(win, LineKind::Info, line);
        }
        let url = format!("{}#readme", super::irc::HOMEPAGE);
        let more = self.t(Key::HelpMore, &[("url", &url)]);
        self.print(win, LineKind::Info, more);
    }

    /// `/timestamp [on|off]` : horodatage de toutes les fenêtres (sans argument : dit où il en est).
    fn cmd_timestamp(&mut self, win: WinId, args: &str) {
        let (w, _) = split_word(args);
        let on = match w.to_ascii_lowercase().as_str() {
            "on" => Some(true),
            "off" => Some(false),
            _ => None,
        };
        if let Some(on) = on
            && on != self.settings.irc.timestamps
        {
            self.settings.irc.timestamps = on;
            self.effects.push(Effect::SaveSettings);
        }
        let key = if self.settings.irc.timestamps { Key::TimestampOn } else { Key::TimestampOff };
        let t = self.t(key, &[]);
        self.print(win, LineKind::Info, t);
    }

    /// La liste Notify a changé (carnet d'adresses) : MONITOR ou ISON remis à jour sur les connexions prêtes, sans
    /// ligne « ajouté »/« retiré » ; seuls les pseudos ajoutés déjà en ligne sont annoncés, comme d'habitude.
    pub(crate) fn notify_resync(&mut self) {
        let ids: Vec<ConnId> = self.conns.iter().filter(|(_, c)| c.ready && c.connected()).map(|(id, _)| *id).collect();
        for id in ids {
            let nicks = self.notify_nicks(id);
            let Some(c) = self.conns.get_mut(&id) else { continue };
            let keep: std::collections::HashSet<String> = nicks.iter().map(|n| c.session.fold(n)).collect();
            c.notify_online.retain(|k, _| keep.contains(k));
            if c.monitor_sent {
                // Tout est redemandé : le serveur répond pour chaque pseudo, et seuls les changements s'affichent.
                let limit = c.session.isupport().monitor.unwrap_or(100).clamp(1, 100);
                self.send_line(id, "MONITOR C");
                for chunk in nicks.chunks(limit) {
                    self.send_line(id, &format!("MONITOR + {}", chunk.join(",")));
                }
            } else {
                c.next_ison = if nicks.is_empty() { None } else { Some(self.now) };
            }
        }
    }

    /// Absence automatique : après `away.auto_away_minutes` sans saisie, AWAY sur les connexions présentes.
    pub(crate) fn auto_away_tick(&mut self) {
        let mins = self.settings.away.auto_away_minutes;
        if mins == 0 || self.now - self.last_input < mins as i64 * 60_000 {
            return;
        }
        let msg = match self.settings.away.default_message.trim() {
            "" => "Away".to_string(),
            m => m.to_string(),
        };
        let ids: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(id, c)| c.ready && c.connected() && c.session.away().is_none() && !self.is_bouncer_parent(**id))
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if let Some(c) = self.conns.get_mut(&id) {
                c.session.set_away(Some(&msg));
            }
            self.drain_session(id);
            if !self.auto_away.contains(&id) {
                self.auto_away.push(id);
            }
        }
    }

    /// Prochaine échéance de l'absence automatique.
    pub(crate) fn auto_away_next(&self) -> Option<i64> {
        let mins = self.settings.away.auto_away_minutes;
        let due = self.last_input + mins as i64 * 60_000;
        (mins > 0 && due > self.now).then_some(due)
    }

    /// Saisie après une absence automatique : de retour sur les connexions mises absentes par elle.
    pub(crate) fn auto_away_back(&mut self) {
        for id in std::mem::take(&mut self.auto_away) {
            let Some(c) = self.conns.get_mut(&id) else { continue };
            if c.connected() && c.session.away().is_some() {
                c.session.set_away(None);
                self.drain_session(id);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Ce que l'hôte peut demander en plus des lignes tapées
// ---------------------------------------------------------------------------------------------------------------

impl super::Client {
    /// Exécute une commande intégrée (`topic`, `msg`, `raw`…) avec ses arguments tels quels : sans passer par le
    /// moteur de scripts (pas d'alias, pas d'évaluation, `|`, `{` et `}` gardés), mais avec la file anti-flood et
    /// le même écho que la commande tapée. Pour les boîtes de l'interface qui envoient un texte libre (sujet…).
    /// L'erreur éventuelle est aussi affichée dans la fenêtre.
    pub fn builtin(&mut self, win: WinId, name: &str, args: &str, now_ms: i64) -> Result<(), String> {
        self.st.now = now_ms;
        let conn = self.st.win(win).and_then(|w| w.conn);
        let ctx = self.st.ctx(conn, Some(win));
        let name = name.trim_start_matches('/').to_ascii_lowercase();
        self.st.dcc.user = true;
        let r = self.st.run_builtin(&ctx, &name, args, false);
        self.st.dcc.user = false;
        if let Err(e) = &r {
            let target = if self.st.win(win).is_some() { win } else { self.st.active.unwrap_or(win) };
            self.st.print(target, LineKind::Error, if e.starts_with("* ") { e.clone() } else { format!("* {e}") });
        }
        self.finish();
        r
    }

    /// Envoie une ligne brute au serveur de `conn`, telle quelle, par la file anti-flood, sans écho.
    pub fn raw(&mut self, conn: ConnId, line: &str, now_ms: i64) {
        self.st.now = now_ms;
        self.st.send_line(conn, line);
        self.finish();
    }

    /// La liste Notify des réglages a changé (carnet d'adresses) : MONITOR/ISON remis à jour sans rien afficher.
    pub fn notify_changed(&mut self, now_ms: i64) {
        self.st.now = now_ms;
        self.st.notify_resync();
        self.finish();
    }

    /// Touche appuyée (`up` faux) ou relâchée dans une fenêtre : `on KEYDOWN` / `on KEYUP` des scripts, avec
    /// `$keyval` (code de touche à la manière de Windows : `65` pour A, `13` pour Entrée…) et `$keychar`. Rend vrai
    /// si un script a fait `/haltdef` : l'interface ne traite alors pas la touche.
    pub fn key_event(&mut self, win: WinId, keyval: u32, keychar: Option<char>, up: bool, now_ms: i64) -> bool {
        self.st.now = now_ms;
        let kind = if up { EventKind::KeyUp } else { EventKind::KeyDown };
        if !self.st.settings.scripts.enabled || !self.engine.handles(kind) {
            return false;
        }
        let Some(w) = self.st.win(win) else { return false };
        let (conn, name) = (w.conn, w.name.clone());
        let ev = alpirc_script::EventInfo {
            kind: Some(kind),
            conn,
            target: Some(name),
            extra: vec![
                ("keyval".into(), keyval.to_string()),
                ("keychar".into(), keychar.map(String::from).unwrap_or_default()),
            ],
            ..alpirc_script::EventInfo::default()
        };
        let ctx = self.st.ctx(conn, Some(win));
        let super::Client { st, engine } = self;
        let halted = engine.fire(st, &ctx, &ev).haltdef;
        self.finish();
        halted
    }
}

/// Nom retenu dans `order.txt` pour une sorte de fichier qui ne se devine pas d'après son texte.
pub(crate) fn script_kind_name(k: alpirc_script::ScriptKind) -> Option<&'static str> {
    match k {
        alpirc_script::ScriptKind::Aliases => Some("aliases"),
        alpirc_script::ScriptKind::Popups => Some("popups"),
        alpirc_script::ScriptKind::Remote => None,
    }
}

/// Sorte d'un fichier d'après son nom dans `order.txt`.
pub(crate) fn script_kind_from_name(s: &str) -> Option<alpirc_script::ScriptKind> {
    match s.to_ascii_lowercase().as_str() {
        "aliases" => Some(alpirc_script::ScriptKind::Aliases),
        "popups" => Some(alpirc_script::ScriptKind::Popups),
        "remote" => Some(alpirc_script::ScriptKind::Remote),
        _ => None,
    }
}
