//! L'état du client vu par le moteur de scripts : commandes intégrées, identifiants qui dépendent de l'état
//! (`$me`, `$chan`, `$nick(#,N)`, `$ial`, `$window`, `$line`…), modes des membres, erreurs de script.

use alpirc_proto::mask;
use alpirc_script::{Ctx, Host};

use super::state::State;
use super::window::{LineKind, WinKind};
use super::{ConnId, ConnState, WinId};

const TRUE: &str = "$true";
const FALSE: &str = "$false";

fn tf(b: bool) -> String {
    (if b { TRUE } else { FALSE }).to_string()
}

/// Nom du système, pour `$os`.
pub fn os_name() -> &'static str {
    if cfg!(target_os = "android") {
        "Android"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "ios") {
        "iOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Linux"
    }
}

fn win_type(k: WinKind) -> &'static str {
    match k {
        WinKind::Status => "status",
        WinKind::Channel => "channel",
        WinKind::Query => "query",
        WinKind::DccChat => "chat",
        WinKind::Custom => "custom",
        WinKind::ChanList => "list",
        WinKind::Notify => "notify",
        WinKind::Urls => "urls",
    }
}

impl State {
    fn ctx_conn(&self, ctx: &Ctx) -> Option<ConnId> {
        self.resolve(ctx).0
    }

    /// N-ième élément (à partir de 1) ou nombre d'éléments pour 0 ; recherche par nom sinon.
    fn nth<T: Clone>(list: &[T], arg: &str, by_name: impl Fn(&T) -> bool) -> Result<Option<T>, usize> {
        match arg.parse::<usize>() {
            Ok(0) => Err(list.len()),
            Ok(n) => Ok(list.get(n - 1).cloned()),
            Err(_) => Ok(list.iter().find(|x| by_name(x)).cloned()),
        }
    }

    fn window_ident(&self, ctx: &Ctx, args: &[String], prop: Option<&str>) -> Option<String> {
        let conn = self.ctx_conn(ctx);
        let a = args.first().map(String::as_str).unwrap_or("");
        let w = match a.parse::<usize>() {
            Ok(0) => {
                let n = self.windows.iter().filter(|w| w.kind == WinKind::Custom).count();
                return Some(n.to_string());
            }
            Ok(n) => self.windows.iter().filter(|w| w.kind == WinKind::Custom).nth(n - 1),
            Err(_) => {
                let id = if a.eq_ignore_ascii_case("-1") || a.is_empty() { self.active } else { self.find_win(conn, a) };
                id.and_then(|i| self.win(i))
            }
        }?;
        Some(match prop {
            Some("wid") => w.id.to_string(),
            Some("cid") => w.conn.map(|c| c.to_string()).unwrap_or_default(),
            Some("type") => win_type(w.kind).to_string(),
            Some("title") => w.title.clone(),
            Some("state") => "normal".into(),
            Some("lines") => w.lines.len().to_string(),
            Some("activity") => format!("{:?}", w.activity).to_lowercase(),
            _ => w.name.clone(),
        })
    }

    fn find_any_win(&self, ctx: &Ctx, name: &str) -> Option<WinId> {
        let conn = self.ctx_conn(ctx);
        self.find_win(conn, name).or_else(|| if name.eq_ignore_ascii_case("status window") { conn.and_then(|c| self.status_of(c)) } else { None })
    }

    fn sorted_nicks(&self, conn: ConnId, chan: &str, filter: &str) -> Vec<(String, String)> {
        let Some(w) = self.find_kind(conn, WinKind::Channel, chan) else { return Vec::new() };
        let c = &self.conns[&conn];
        self.nicklist(w)
            .into_iter()
            .filter_map(|e| {
                let modes = c.session.member_modes(chan, &e.nick).unwrap_or("").to_string();
                let keep = filter.is_empty()
                    || filter.chars().any(|f| match f {
                        'a' => true,
                        'r' => modes.is_empty(),
                        m => modes.contains(m),
                    });
                keep.then_some((e.nick, modes))
            })
            .collect()
    }

    fn ident(&mut self, ctx: &Ctx, name: &str, args: &[String], prop: Option<&str>) -> Option<String> {
        let conn = self.ctx_conn(ctx);
        let c = conn.and_then(|id| self.conns.get(&id));
        let a = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        let v = match name {
            "me" => self.me(conn.unwrap_or(0)),
            "mnick" => self.settings.identity.nick.clone(),
            "anick" => self.settings.identity.alt_nick.clone(),
            "fullname" => self.settings.identity.realname.clone(),
            "email" | "username" => self.settings.identity.username.clone(),
            "server" => match c {
                Some(c) if c.connected() => {
                    if c.session.server_name().is_empty() { c.host.clone() } else { c.session.server_name().to_string() }
                }
                _ => String::new(),
            },
            "serverip" | "ip" => String::new(),
            "host" => c.and_then(|c| c.session.host().map(str::to_string)).unwrap_or_default(),
            "port" => c.map(|c| c.port.to_string()).unwrap_or_default(),
            "ssl" => tf(c.is_some_and(|c| c.tls)),
            "network" => c.map(|c| c.network.clone()).unwrap_or_default(),
            "cid" => conn.map(|c| c.to_string()).unwrap_or_default(),
            "status" => match c.map(|c| c.state) {
                Some(ConnState::Connected) => "connected".into(),
                Some(ConnState::Registering) => "logging on".into(),
                Some(ConnState::Connecting) => "connecting".into(),
                _ => "disconnected".into(),
            },
            "active" => self.active.and_then(|w| self.win(w)).map(|w| w.name.clone()).unwrap_or_default(),
            "activecid" => self.active.and_then(|w| self.win(w)).and_then(|w| w.conn).map(|c| c.to_string()).unwrap_or_default(),
            "activewid" => self.active.map(|w| w.to_string()).unwrap_or_default(),
            "appactive" => tf(self.app_focused),
            "window" => return self.window_ident(ctx, args, prop),
            "version" => format!("AlpIRC {}", super::irc::VERSION),
            "os" => os_name().into(),
            "usermode" => c.map(|c| if c.session.user_modes().is_empty() { String::new() } else { format!("+{}", c.session.user_modes()) }).unwrap_or_default(),
            "away" => tf(c.is_some_and(|c| c.session.away().is_some())),
            "awaymsg" => c.and_then(|c| c.session.away().map(str::to_string)).unwrap_or_default(),
            "idle" => ((self.now - self.last_input) / 1000).max(0).to_string(),
            "lag" => c.and_then(|c| c.lag_ms).map(|l| l.to_string()).unwrap_or_default(),
            "timestamp" => self.stamp(self.now),
            "color" | "colour" => {
                let k = match a(0).to_ascii_lowercase().as_str() {
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
                    "notice" => LineKind::Notice,
                    "notify" => LineKind::Notify,
                    "other" => LineKind::Other,
                    "own" => LineKind::Own,
                    "part" => LineKind::Part,
                    "quit" => LineKind::Quit,
                    "topic" => LineKind::Topic,
                    "wallops" => LineKind::Wallops,
                    "whois" => LineKind::Whois,
                    _ => LineKind::Normal,
                };
                self.settings.color_index(k).to_string()
            }
            "scon" => {
                let ids: Vec<ConnId> = self.conns.keys().copied().collect();
                match a(0).parse::<usize>() {
                    Ok(0) => ids.len().to_string(),
                    Ok(n) => ids.get(n - 1).map(|i| i.to_string()).unwrap_or_default(),
                    Err(_) => String::new(),
                }
            }
            "chan" => {
                let c = c?;
                let chans: Vec<String> = c.session.channels().filter(|x| x.joined).map(|x| x.name.clone()).collect();
                let ch = match Self::nth(&chans, a(0), |x| c.session.eq(x, a(0))) {
                    Err(n) => return Some(n.to_string()),
                    Ok(v) => v?,
                };
                let st = c.session.channel(&ch)?;
                match prop {
                    Some("topic") => st.topic.clone().unwrap_or_default(),
                    Some("mode") => st.mode_string(true),
                    Some("key") => st.key().unwrap_or("").to_string(),
                    Some("limit") => st.limit().map(|l| l.to_string()).unwrap_or_default(),
                    Some("users") => st.members.len().to_string(),
                    _ => ch,
                }
            }
            "nick" if !args.is_empty() => {
                let id = conn?;
                let list = self.sorted_nicks(id, a(0), a(2));
                let cc = &self.conns[&id];
                if a(1).is_empty() || a(1).parse::<usize>().is_err() {
                    // `$nick(#, pseudo)` : sa position.
                    let pos = list.iter().position(|(n, _)| cc.session.eq(n, a(1)))?;
                    return Some((pos + 1).to_string());
                }
                let (nick, modes) = match Self::nth(&list, a(1), |_| false) {
                    Err(n) => return Some(n.to_string()),
                    Ok(v) => v?,
                };
                match prop {
                    Some("pnick") => {
                        let sym: String = modes.chars().take(1).filter_map(|m| cc.session.isupport().symbol_for_mode(m)).collect();
                        format!("{sym}{nick}")
                    }
                    Some("idle") => {
                        let last = cc.session.channel(a(0)).and_then(|ch| ch.members.get(&cc.session.fold(&nick))).map(|m| m.last_spoke).unwrap_or(0);
                        if last == 0 { "0".into() } else { ((self.now - last) / 1000).to_string() }
                    }
                    Some("color") => super::util::nick_color(&nick).to_string(),
                    _ => nick,
                }
            }
            "opnick" | "vnick" | "hnick" | "nopnick" | "nvnick" => {
                let id = conn?;
                let filter = match name {
                    "opnick" => "o",
                    "vnick" => "v",
                    "hnick" => "h",
                    _ => "",
                };
                let mut list = self.sorted_nicks(id, a(0), filter);
                if name == "nopnick" {
                    list.retain(|(_, m)| !m.contains('o'));
                } else if name == "nvnick" {
                    list.retain(|(_, m)| !m.contains('v'));
                }
                match Self::nth(&list, a(1), |_| false) {
                    Err(n) => n.to_string(),
                    Ok(v) => v?.0,
                }
            }
            "ial" => {
                let c = c?;
                let cm = c.session.casemapping();
                let pat = if a(0).is_empty() { "*" } else { a(0) };
                let mut list: Vec<(String, String)> = c
                    .session
                    .users()
                    .filter(|u| mask::wildcard_match(pat, &u.full_mask(), cm))
                    .map(|u| (u.nick.clone(), u.full_mask()))
                    .collect();
                list.sort();
                let n = if a(1).is_empty() { "1" } else { a(1) };
                let (nick, full) = match Self::nth(&list, n, |_| false) {
                    Err(n) => return Some(n.to_string()),
                    Ok(v) => v?,
                };
                match prop {
                    Some("nick") => nick,
                    Some("addr") => full.split_once('!').map(|(_, x)| x.to_string()).unwrap_or_default(),
                    Some("user") => full.split_once('!').and_then(|(_, x)| x.split_once('@')).map(|(u, _)| u.to_string()).unwrap_or_default(),
                    Some("host") => full.split_once('@').map(|(_, h)| h.to_string()).unwrap_or_default(),
                    _ => full,
                }
            }
            "address" if !args.is_empty() => {
                let c = c?;
                let u = c.session.user_info(a(0))?;
                let (user, host) = (u.user.clone()?, u.host.clone()?);
                if a(1).is_empty() {
                    format!("{user}@{host}")
                } else {
                    let ty: u8 = a(1).parse().unwrap_or(5).min(19);
                    mask::address_mask(&u.nick, &user, &host, ty)
                }
            }
            "comchan" => {
                let c = c?;
                let chans = c.session.common_channels(a(0));
                match Self::nth(&chans, if a(1).is_empty() { "0" } else { a(1) }, |_| false) {
                    Err(n) => n.to_string(),
                    Ok(v) => v?,
                }
            }
            "query" => {
                let list: Vec<String> =
                    self.windows.iter().filter(|w| w.kind == WinKind::Query && (conn.is_none() || w.conn == conn)).map(|w| w.name.clone()).collect();
                match Self::nth(&list, a(0), |x| x.eq_ignore_ascii_case(a(0))) {
                    Err(n) => n.to_string(),
                    Ok(v) => v?,
                }
            }
            "chat" => {
                let list: Vec<String> = self.windows.iter().filter(|w| w.kind == WinKind::DccChat).map(|w| w.name.clone()).collect();
                match Self::nth(&list, a(0), |x| x.eq_ignore_ascii_case(a(0))) {
                    Err(n) => n.to_string(),
                    Ok(v) => v?,
                }
            }
            "notify" => {
                let list = self.settings.address_book.notify.clone();
                let e = match Self::nth(&list, a(0), |x| x.nick.eq_ignore_ascii_case(a(0))) {
                    Err(n) => return Some(n.to_string()),
                    Ok(v) => v?,
                };
                match prop {
                    Some("note") => e.note,
                    Some("ison") => {
                        let on = c.is_some_and(|c| c.notify_online.get(&c.session.fold(&e.nick)).copied().unwrap_or(false));
                        tf(on)
                    }
                    _ => e.nick,
                }
            }
            "ignore" => {
                let list = self.settings.address_book.ignore.clone();
                match Self::nth(&list, a(0), |x| x.mask.eq_ignore_ascii_case(a(0))) {
                    Err(n) => n.to_string(),
                    Ok(v) => v?.mask,
                }
            }
            "line" => {
                let w = self.find_any_win(ctx, a(0)).and_then(|w| self.win(w))?;
                let listbox = a(2) == "1";
                let n: usize = a(1).parse().ok()?;
                if listbox {
                    if n == 0 { w.list_lines.len().to_string() } else { w.list_lines.get(n - 1)?.clone() }
                } else if n == 0 {
                    w.lines.len().to_string()
                } else {
                    w.lines.get(n - 1)?.text.clone()
                }
            }
            "sline" => {
                let w = self.find_any_win(ctx, a(0)).and_then(|w| self.win(w))?;
                let n: usize = a(1).parse().ok()?;
                if n == 0 {
                    return Some(w.sel.len().to_string());
                }
                let ln = *w.sel.get(n - 1)?;
                match prop {
                    Some("ln") => ln.to_string(),
                    _ => w.list_lines.get(ln - 1).cloned().or_else(|| w.lines.get(ln - 1).map(|l| l.text.clone()))?,
                }
            }
            "isban" | "isexcept" | "isinvite" => {
                let c = c?;
                let ch = c.session.channel(a(1))?;
                let is = c.session.isupport();
                let mode = match name {
                    "isban" => 'b',
                    "isexcept" => is.excepts.unwrap_or('e'),
                    _ => is.invex.unwrap_or('I'),
                };
                let list = ch.list(mode, is)?;
                tf(list.iter().any(|e| c.session.eq(&e.mask, a(0))))
            }
            "isignore" => {
                let (nick, addr) = match a(0).split_once('!') {
                    Some((n, r)) => (n.to_string(), r.to_string()),
                    None => {
                        let addr = c.and_then(|c| c.session.user_info(a(0))).and_then(|u| u.address()).unwrap_or_else(|| "*@*".into());
                        (a(0).to_string(), addr)
                    }
                };
                let id = conn?;
                tf(self.ignored(id, &nick, &addr, super::irc::IgnoreType::Private) == super::irc::Ignored::Drop
                    || self.ignored(id, &nick, &addr, super::irc::IgnoreType::Channel) == super::irc::Ignored::Drop)
            }
            "isnotify" => tf(self.settings.address_book.notify.iter().any(|e| e.nick.eq_ignore_ascii_case(a(0)))),
            _ => return None,
        };
        Some(v)
    }
}

impl Host for State {
    fn command(&mut self, ctx: &Ctx, name: &str, args: &str) -> Result<(), String> {
        self.run_builtin(ctx, name, args, false)
    }

    fn command_quiet(&mut self, ctx: &Ctx, name: &str, args: &str) -> Result<(), String> {
        self.run_builtin(ctx, name, args, true)
    }

    fn identifier(&mut self, ctx: &Ctx, name: &str, args: &[String], prop: Option<&str>) -> Option<String> {
        self.ident(ctx, name, args, prop)
    }

    fn member_modes(&mut self, ctx: &Ctx, chan: &str, nick: &str) -> Option<String> {
        let conn = self.ctx_conn(ctx)?;
        self.conns.get(&conn)?.session.member_modes(chan, nick).map(str::to_string)
    }

    fn is_channel(&mut self, ctx: &Ctx, name: &str) -> bool {
        match self.ctx_conn(ctx).and_then(|c| self.conns.get(&c)) {
            Some(c) => c.session.is_channel(name),
            None => name.starts_with(['#', '&']),
        }
    }

    fn script_error(&mut self, ctx: &Ctx, text: &str) {
        let (_, win) = self.resolve(ctx);
        let target = if self.win(win).is_some() { win } else { self.active.unwrap_or(win) };
        self.print(target, LineKind::Error, text.to_string());
    }

    fn now_ms(&self) -> i64 {
        self.now
    }

    fn utc_offset(&self, unix_secs: i64) -> i32 {
        self.offset(unix_secs * 1000)
    }
}
