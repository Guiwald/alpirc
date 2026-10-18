//! Connexions : choix du serveur (rotation, STS), ouverture, enregistrement, déconnexion, reconnexion avec
//! délai croissant (10 s → 60 s), PING de latence et délai de ping, Notify par ISON, retour sur les canaux.

use alpirc_proto::encoding::Encoding;
use alpirc_proto::sasl::Mechanism;
use alpirc_script::EventKind;

use super::state::{Conn, Pres, State};
use super::window::{LineKind, WinKind};
use super::{ConnId, ConnState, Effect, TlsVerify, WinId};
use crate::i18n::Key;
use crate::session::{DEFAULT_CAPS, SaslConfig, Session, SessionConfig, undernet_loc_password};
use crate::settings::{Server, Verify};

/// Intervalle des ISON de la liste Notify (sans MONITOR).
pub(crate) const ISON_INTERVAL_MS: i64 = 30_000;

impl State {
    /// Nouvelle fenêtre Status, avec sa connexion (pas encore ouverte).
    pub(crate) fn new_status_window(&mut self) -> WinId {
        let id = self.next_conn;
        self.next_conn += 1;
        let win = self.create_window(WinKind::Status, Some(id), "Status");
        let c = Conn::new(id, win, &self.settings.identity.nick, self.now, self.settings.flood.burst);
        self.conns.insert(id, c);
        self.update_titles(id);
        win
    }

    /// Connexion d'une fenêtre Status.
    pub(crate) fn conn_of_status(&self, status: WinId) -> Option<ConnId> {
        let w = self.win(status)?;
        w.conn.filter(|c| self.conns.get(c).is_some_and(|x| x.status == status)).or(w.conn)
    }

    /// Ferme proprement la connexion en cours avant d'en ouvrir une autre (sans reconnexion).
    fn drop_current(&mut self, conn: ConnId, reason: &str) {
        let Some(c) = self.conns.get(&conn) else { return };
        if c.state == ConnState::Disconnected {
            return;
        }
        if c.connected() {
            self.send_now(conn, &alpirc_proto::Message::new("QUIT", [reason]).to_line());
            self.flush_sends();
        }
        self.close_effect(conn);
        if let Some(c) = self.conns.get_mut(&conn) {
            c.user_quit = true;
        }
        self.conn_closed(conn, None);
    }

    /// Se connecte à un réseau des réglages (ou, s'il n'y en a pas de ce nom, à l'hôte `nom[:port]`).
    pub(crate) fn connect_network(&mut self, status: WinId, network: &str, server: Option<usize>) {
        let Some(conn) = self.conn_of_status(status) else { return };
        let net = self.settings.network(network).cloned();
        let (net_name, display, servers, encoding) = match net {
            Some(n) => {
                if n.servers.is_empty() {
                    let t = self.t(Key::NoServer, &[("network", &n.name)]);
                    self.print(status, LineKind::Error, t);
                    return;
                }
                (Some(n.name.clone()), n.name.clone(), n.servers.clone(), Encoding::from_name(&n.encoding))
            }
            None => {
                let (host, port, tls) = parse_host_port(network, None);
                let n = self.settings.network_for_host(&host).cloned();
                let enc = n.as_ref().map(|n| Encoding::from_name(&n.encoding)).unwrap_or_default();
                let mut s = Server::new(&host, port, tls);
                if let Some(known) = n.as_ref().and_then(|n| n.servers.iter().find(|x| x.host.eq_ignore_ascii_case(&host))) {
                    s.verify = known.verify;
                    s.fingerprint = known.fingerprint.clone();
                    s.password = known.password.clone();
                }
                (n.as_ref().map(|n| n.name.clone()), n.map(|n| n.name).unwrap_or_else(|| host.clone()), vec![s], enc)
            }
        };
        self.connect_with(conn, net_name, display, servers, server.unwrap_or(0), encoding);
    }

    /// Connexion à un serveur donné à la main (`/server hôte port mot-de-passe`).
    pub(crate) fn connect_adhoc(&mut self, conn: ConnId, mut server: Server) {
        let n = self.settings.network_for_host(&server.host).cloned();
        if let Some(known) = n.as_ref().and_then(|n| n.servers.iter().find(|x| x.host.eq_ignore_ascii_case(&server.host))) {
            server.verify = known.verify;
            server.fingerprint = known.fingerprint.clone();
            if server.password.is_none() {
                server.password = known.password.clone();
            }
        }
        let enc = n.as_ref().map(|n| Encoding::from_name(&n.encoding)).unwrap_or_default();
        let display = n.as_ref().map(|n| n.name.clone()).unwrap_or_else(|| server.host.clone());
        self.connect_with(conn, n.map(|n| n.name), display, vec![server], 0, enc);
    }

    fn connect_with(&mut self, conn: ConnId, net_name: Option<String>, display: String, servers: Vec<Server>, idx: usize, enc: Encoding) {
        let same = self.conns.get(&conn).is_some_and(|c| c.net_name.is_some() && c.net_name == net_name);
        let quit = self.settings.identity.quit_message.clone();
        self.drop_current(conn, &quit);
        let Some(c) = self.conns.get_mut(&conn) else { return };
        if !same {
            c.rejoin.clear();
        }
        c.net_name = net_name;
        c.network = display;
        c.server_idx = idx % servers.len().max(1);
        c.servers = servers;
        c.encoding = enc;
        c.user_quit = false;
        c.attempts = 0;
        c.reconnect_at = None;
        self.do_connect(conn);
    }

    /// Configuration de la session pour la prochaine connexion.
    pub(crate) fn session_config(&self, conn: ConnId) -> SessionConfig {
        let c = &self.conns[&conn];
        let net = c.net_name.as_deref().and_then(|n| self.settings.network(n));
        let id = &self.settings.identity;
        let server = c.servers.get(c.server_idx);
        let mut password = server.and_then(|s| s.password.clone()).filter(|p| !p.is_empty());
        // Login on Connect d'Undernet : prime sur le mot de passe du serveur.
        if let Some(x) = net.and_then(|n| n.undernet_x.as_ref()).filter(|x| !x.username.is_empty()) {
            password = Some(undernet_loc_password(&x.username, &x.password, x.require_x, x.hide_host));
        }
        let sasl = net.and_then(|n| n.sasl.as_ref()).and_then(|s| {
            let mech = Mechanism::from_name(&s.mechanism).unwrap_or(Mechanism::Plain);
            (mech == Mechanism::External || !s.account.is_empty())
                .then(|| SaslConfig { mechanism: mech, account: s.account.clone(), password: s.password.clone() })
        });
        let nick = c
            .nick_override
            .clone()
            .or_else(|| net.and_then(|n| n.nick.clone()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| id.nick.clone());
        SessionConfig {
            nick,
            alt_nick: net.and_then(|n| n.alt_nick.clone()).unwrap_or_else(|| id.alt_nick.clone()),
            username: net.and_then(|n| n.username.clone()).unwrap_or_else(|| id.username.clone()),
            realname: net.and_then(|n| n.realname.clone()).unwrap_or_else(|| id.realname.clone()),
            password,
            sasl,
            nickserv_password: net.and_then(|n| n.nickserv_password.clone()),
            request_caps: DEFAULT_CAPS.iter().map(|s| s.to_string()).collect(),
            bouncer_bind: c.bouncer_netid.clone(),
            tls: c.tls,
            who_on_join: self.settings.irc.who_on_join,
            scram_nonce: self.scram_nonce.clone(),
        }
    }

    /// Ouvre la connexion vers le serveur courant de la liste.
    pub(crate) fn do_connect(&mut self, conn: ConnId) {
        let now = self.now;
        let Some(c) = self.conns.get(&conn) else { return };
        let Some(server) = c.servers.get(c.server_idx).cloned() else { return };
        let (mut tls, mut port) = (server.tls, server.port);
        let net = c.net_name.as_deref().and_then(|n| self.settings.network(n)).cloned();
        if let Some(p) = c.sts_upgrade {
            tls = true;
            port = p;
        } else if let Some(pol) = net.as_ref().and_then(|n| n.sts.as_ref())
            && pol.expires_ms.is_none_or(|e| e > now)
            && !tls
        {
            tls = true;
            port = pol.port;
        }
        let verify = match server.verify {
            Verify::Full => TlsVerify::Full,
            Verify::AcceptInvalid => TlsVerify::AcceptInvalid,
            Verify::Pinned => TlsVerify::Pinned(server.fingerprint.clone().unwrap_or_default()),
        };
        let client_cert = net
            .as_ref()
            .and_then(|n| n.sasl.as_ref())
            .filter(|s| s.mechanism.eq_ignore_ascii_case("EXTERNAL"))
            .and_then(|s| s.client_cert.clone());
        let burst = self.settings.flood.burst;
        {
            let c = self.conns.get_mut(&conn).unwrap();
            c.sts_upgrade = None;
            c.host = server.host.clone();
            c.port = port;
            c.tls = tls;
            c.verify = verify.clone();
            c.client_cert = client_cert.clone();
        }
        let cfg = self.session_config(conn);
        let c = self.conns.get_mut(&conn).unwrap();
        c.session = Session::new(cfg);
        c.state = ConnState::Connecting;
        c.reconnect_at = None;
        c.ready = false;
        c.lag_ms = None;
        c.lag_sent = None;
        c.flood.reset(burst, now);
        c.notify_online.clear();
        c.monitor_sent = false;
        c.next_ison = None;
        c.pending_bans.clear();
        c.pending_modes.clear();
        c.whois_win = None;
        c.last_rx = now;
        let st = c.status;
        let t = self.t(Key::Connecting, &[("host", &server.host), ("port", &port.to_string())]);
        self.print(st, LineKind::Info, t);
        self.effects.push(Effect::Connect { conn, host: server.host, port, tls, verify, client_cert });
        self.update_titles(conn);
    }

    /// La connexion TCP (et TLS) est ouverte : enregistrement.
    pub(crate) fn conn_connected(&mut self, conn: ConnId) {
        let now = self.now;
        let burst = self.settings.flood.burst;
        let Some(c) = self.conns.get_mut(&conn) else { return };
        c.state = ConnState::Registering;
        c.last_rx = now;
        c.flood.reset(burst, now);
        let tls = c.tls;
        c.session.config_mut().tls = tls;
        c.session.start();
        let st = c.status;
        let t = self.t(Key::Connected, &[]);
        self.print(st, LineKind::Info, t);
        self.drain_session(conn);
        let ev = self.ev(EventKind::Logon, conn);
        let ctx = self.ctx(Some(conn), Some(st));
        self.push_pres(Pres::new().event(ctx, ev));
        self.update_titles(conn);
    }

    /// Échec de l'ouverture : serveur suivant, nouvel essai plus tard.
    pub(crate) fn conn_failed(&mut self, conn: ConnId, error: &str) {
        let Some(c) = self.conns.get_mut(&conn) else { return };
        if c.state == ConnState::Disconnected {
            return;
        }
        c.state = ConnState::Disconnected;
        if c.servers.len() > 1 {
            c.server_idx = (c.server_idx + 1) % c.servers.len();
        }
        let st = c.status;
        let user_quit = c.user_quit;
        let t = self.t(Key::ConnectFailed, &[("error", error)]);
        self.print(st, LineKind::Error, t);
        if !user_quit && self.settings.connect.reconnect {
            self.schedule_reconnect(conn);
        }
        self.update_titles(conn);
    }

    /// La connexion est fermée (par le serveur, le réseau, ou par nous).
    pub(crate) fn conn_closed(&mut self, conn: ConnId, error: Option<&str>) {
        let Some(c) = self.conns.get_mut(&conn) else { return };
        if c.state == ConnState::Disconnected {
            return;
        }
        let was = c.state;
        let joined: Vec<(String, Option<String>)> =
            c.session.channels().filter(|ch| ch.joined).map(|ch| (ch.name.clone(), ch.key().map(str::to_string))).collect();
        if !joined.is_empty() {
            c.rejoin = joined;
        }
        c.state = ConnState::Disconnected;
        c.ready = false;
        c.lag_sent = None;
        let st = c.status;
        let sts = c.sts_upgrade.is_some();
        let user_quit = c.user_quit;
        let wins: Vec<WinId> = self.windows.iter().filter(|w| w.conn == Some(conn) && w.kind == WinKind::Channel).map(|w| w.id).collect();
        let t = match error {
            Some(e) if !e.is_empty() => self.t(Key::DisconnectedReason, &[("error", e)]),
            _ => self.t(Key::Disconnected, &[]),
        };
        for w in &wins {
            if let Some(x) = self.win_mut(*w) {
                x.joined = false;
            }
            self.print(*w, LineKind::Info, t.clone());
        }
        self.print(st, LineKind::Info, t);
        if was != ConnState::Connecting {
            let ev = self.ev(EventKind::Disconnect, conn);
            let ctx = self.ctx(Some(conn), Some(st));
            let mut p = Pres::new().event(ctx, ev);
            // Son de la déconnexion (Options › Sons), sauf si on l'a demandée.
            if !user_quit
                && !sts
                && let Some(fx) = self.event_sound(crate::settings::SoundEvent::Disconnect)
            {
                p = p.effect(fx);
            }
            self.push_pres(p);
        }
        if sts {
            self.do_connect(conn);
            return;
        }
        if !user_quit && self.settings.connect.reconnect {
            self.schedule_reconnect(conn);
        }
        self.update_titles(conn);
    }

    /// Prochain essai : 10 s, 20 s, 40 s, puis 60 s (réglages).
    fn schedule_reconnect(&mut self, conn: ConnId) {
        let co = self.settings.connect.clone();
        let now = self.now;
        let Some(c) = self.conns.get_mut(&conn) else { return };
        if c.servers.is_empty() {
            return;
        }
        c.attempts += 1;
        if co.max_retries > 0 && c.attempts > co.max_retries {
            c.reconnect_at = None;
            return;
        }
        let base = co.retry_delay_secs.max(1);
        let shift = (c.attempts - 1).min(16);
        let delay = base.saturating_mul(1u64 << shift).min(co.max_retry_delay_secs.max(base));
        c.reconnect_at = Some(now + delay as i64 * 1000);
        let n = c.attempts;
        let st = c.status;
        let t = self.t(Key::Retrying, &[("secs", &delay.to_string()), ("n", &n.to_string())]);
        self.print(st, LineKind::Info, t);
    }

    /// Déconnexion voulue (`/quit`, `/disconnect`, fermeture de la fenêtre).
    pub(crate) fn user_disconnect(&mut self, conn: ConnId, reason: &str) {
        let Some(c) = self.conns.get_mut(&conn) else { return };
        c.user_quit = true;
        c.reconnect_at = None;
        if c.state == ConnState::Disconnected {
            return;
        }
        if c.connected() {
            self.send_now(conn, &alpirc_proto::Message::new("QUIT", [reason]).to_line());
            self.flush_sends();
        }
        self.close_effect(conn);
        self.conn_closed(conn, None);
    }

    /// Minuteries des connexions : reconnexion, délai de ping, latence, ISON, auto-op, bans temporaires.
    pub(crate) fn tick_conns(&mut self) {
        let now = self.now;
        let ids: Vec<ConnId> = self.conns.keys().copied().collect();
        let co = self.settings.connect.clone();
        for id in ids {
            let Some(c) = self.conns.get(&id) else { continue };
            if c.state == ConnState::Disconnected && c.reconnect_at.is_some_and(|t| t <= now) {
                self.do_connect(id);
                continue;
            }
            if !c.connected() {
                continue;
            }
            let timeout = co.ping_timeout_secs.max(10) as i64 * 1000;
            if now - c.last_rx >= timeout {
                let st = c.status;
                let t = self.t(Key::PingTimeout, &[("secs", &co.ping_timeout_secs.to_string())]);
                self.print(st, LineKind::Error, t);
                self.close_effect(id);
                let reason = self.t(Key::PingTimeoutReason, &[]);
                self.conn_closed(id, Some(&reason));
                continue;
            }
            let registered = c.session.is_registered();
            if registered && now >= c.next_lag_ping {
                self.send_now(id, &format!("PING :ALPIRC{now}"));
                let c = self.conns.get_mut(&id).unwrap();
                c.lag_sent = Some(now);
                c.next_lag_ping = now + co.lag_interval_secs.max(5) as i64 * 1000;
            }
            let c = self.conns.get(&id).unwrap();
            if registered && c.next_ison.is_some_and(|t| t <= now) {
                let nicks = self.notify_nicks(id);
                let c = self.conns.get_mut(&id).unwrap();
                c.next_ison = Some(now + ISON_INTERVAL_MS);
                c.session.request_ison(&nicks, true);
                self.drain_session(id);
            }
            self.run_pending_modes(id);
            let c = self.conns.get_mut(&id).unwrap();
            let (due, rest): (Vec<_>, Vec<_>) = c.timed_unbans.drain(..).partition(|(t, _, _)| *t <= now);
            c.timed_unbans = rest;
            for (_, chan, mask) in due {
                self.send_line(id, &format!("MODE {chan} -b {mask}"));
            }
        }
    }

    /// Prochaine échéance des connexions.
    pub(crate) fn next_wake(&self) -> Option<i64> {
        let mut v: Vec<i64> = Vec::new();
        let co = &self.settings.connect;
        for c in self.conns.values() {
            if let Some(t) = c.reconnect_at {
                v.push(t);
            }
            if !c.connected() {
                continue;
            }
            if let Some(t) = c.flood.next_due(self.settings.flood.rate_ms) {
                v.push(t);
            }
            v.push(c.last_rx + co.ping_timeout_secs.max(10) as i64 * 1000);
            if c.session.is_registered() {
                v.push(c.next_lag_ping);
                if let Some(t) = c.next_ison {
                    v.push(t);
                }
            }
            v.extend(c.pending_modes.iter().map(|m| m.due));
            v.extend(c.timed_unbans.iter().map(|t| t.0));
        }
        v.into_iter().min()
    }

    /// Retour sur les canaux (après une reconnexion) et favoris à rejoindre à la connexion.
    pub(crate) fn auto_join(&mut self, conn: ConnId) {
        let rejoin_opt = self.settings.connect.rejoin_channels;
        let Some(c) = self.conns.get_mut(&conn) else { return };
        let mut chans: Vec<(String, Option<String>)> = if rejoin_opt { std::mem::take(&mut c.rejoin) } else { Vec::new() };
        let net_names: Vec<String> = [c.net_name.clone(), Some(c.network.clone())].into_iter().flatten().collect();
        for f in &self.settings.favorites {
            if f.join_on_connect && net_names.iter().any(|n| n.eq_ignore_ascii_case(&f.network)) {
                chans.push((f.channel.clone(), f.key.clone().filter(|k| !k.is_empty())));
            }
        }
        let c = self.conns.get(&conn).unwrap();
        let mut seen = Vec::new();
        chans.retain(|(ch, _)| {
            let k = c.session.fold(ch);
            if seen.contains(&k) || c.session.channel(ch).is_some_and(|x| x.joined) {
                false
            } else {
                seen.push(k);
                true
            }
        });
        if chans.is_empty() {
            return;
        }
        // Les canaux à clé d'abord (les clés vont dans l'ordre).
        chans.sort_by_key(|(_, k)| k.is_none());
        for l in join_lines(&chans) {
            self.send_line(conn, &l);
        }
    }

    /// Ouvre une connexion par réseau d'un bouncer soju (`BOUNCER BIND`).
    pub(crate) fn spawn_bouncer_child(&mut self, parent: ConnId, netid: &str, name: &str) {
        let Some(p) = self.conns.get(&parent) else { return };
        let (servers, idx, net_name, enc) = (p.servers.clone(), p.server_idx, p.net_name.clone(), p.encoding);
        let host_port = (p.host.clone(), p.port, p.tls);
        let win = self.new_status_window();
        let Some(child) = self.conn_of_status(win) else { return };
        let c = self.conns.get_mut(&child).unwrap();
        c.bouncer_parent = Some(parent);
        c.bouncer_netid = Some(netid.to_string());
        c.network = name.to_string();
        c.net_name = net_name;
        let mut servers = servers;
        // Même serveur que le parent (port et TLS effectifs, STS compris).
        if let Some(s) = servers.get_mut(idx) {
            s.host = host_port.0;
            s.port = host_port.1;
            s.tls = host_port.2;
        }
        c.servers = servers;
        c.server_idx = idx;
        c.encoding = enc;
        if let Some(p) = self.conns.get_mut(&parent) {
            p.bouncer_children.insert(netid.to_string(), child);
        }
        self.do_connect(child);
    }
}

/// Lignes `JOIN #a,#b clé` de moins de 400 octets.
pub(crate) fn join_lines(chans: &[(String, Option<String>)]) -> Vec<String> {
    let mut out = Vec::new();
    let mut names: Vec<&str> = Vec::new();
    let mut keys: Vec<&str> = Vec::new();
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
    for (ch, k) in chans {
        let add = ch.len() + 1 + k.as_ref().map(|k| k.len() + 1).unwrap_or(0);
        if len + add > 400 {
            flush(&mut names, &mut keys, &mut out);
            len = 0;
        }
        names.push(ch);
        if let Some(k) = k {
            keys.push(k);
        }
        len += add;
    }
    flush(&mut names, &mut keys, &mut out);
    out
}

/// `hôte`, `hôte:port`, `hôte:+port` (TLS), avec un port donné à part (`6667`, `+6697`).
pub(crate) fn parse_host_port(spec: &str, port_word: Option<&str>) -> (String, u16, bool) {
    let mut host = spec.to_string();
    let mut port = 6667u16;
    let mut tls = false;
    // IPv6 entre crochets : [::1]:6697.
    if let Some(rest) = spec.strip_prefix('[')
        && let Some((h, after)) = rest.split_once(']')
    {
        host = h.to_string();
        if let Some(p) = after.strip_prefix(':') {
            tls = p.starts_with('+');
            port = p.trim_start_matches('+').parse().unwrap_or(6667);
        }
    } else if spec.matches(':').count() == 1
        && let Some((h, p)) = spec.split_once(':')
    {
        host = h.to_string();
        tls = p.starts_with('+');
        port = p.trim_start_matches('+').parse().unwrap_or(6667);
    }
    if let Some(p) = port_word {
        tls = p.starts_with('+');
        port = p.trim_start_matches('+').parse().unwrap_or(port);
    }
    (host, port, tls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_ports() {
        assert_eq!(parse_host_port("irc.libera.chat:+6697", None), ("irc.libera.chat".into(), 6697, true));
        assert_eq!(parse_host_port("irc.undernet.org", Some("6667")), ("irc.undernet.org".into(), 6667, false));
        assert_eq!(parse_host_port("irc.x.org", Some("+7000")), ("irc.x.org".into(), 7000, true));
        assert_eq!(parse_host_port("[::1]:+6697", None), ("::1".into(), 6697, true));
    }

    #[test]
    fn joins() {
        let v = vec![("#a".to_string(), Some("k".to_string())), ("#b".to_string(), None)];
        assert_eq!(join_lines(&v), vec!["JOIN #a,#b k"]);
    }
}
