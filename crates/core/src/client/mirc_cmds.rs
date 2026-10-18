//! Commandes de mIRC 6.x ajoutées par la revue face à mIRC (`docs/ecarts-mirc.md`) : pseudos et nom complet
//! (`/anick`, `/mnick`, `/fullname`), interrupteurs des Options (`/ajinvite`, `/creq`, `/sreq`, `/remote`,
//! `/toolbar`, `/switchbar`, `/treebar`), listes Control (`/aop`, `/avoice`, `/protect`), messages à tous les privés
//! (`/qmsg`, `/qme`), presse-papiers (`/clipboard`), texte d'une fenêtre dans un fichier (`/savebuf`) et adresse
//! d'un hôte ou d'un pseudo (`/dns`), lignes échangées avec le serveur (`/debug`).

use alpirc_proto::{format, mask};

use super::state::State;
use super::util::{split_word, switches};
use super::window::{LineKind, WinKind};
use super::{ConnId, Effect, WinId};
use crate::i18n::Key;
use crate::settings::{ControlEntry, ControlKind, DccChatAction, DccSendAction};

/// Où va la sortie de `/debug`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DebugTarget {
    /// Une fenêtre `@` (son numéro).
    Window(WinId),
    /// Un fichier du dossier des journaux.
    File(std::path::PathBuf),
}

/// Ligne masquée pour `/debug` : les mots de passe (PASS, AUTHENTICATE, IDENTIFY de NickServ, OPER, LOGIN de X) ne
/// sont pas recopiés.
fn debug_mask(line: &str) -> String {
    let upper = line.to_ascii_uppercase();
    let cmd = upper.trim_start_matches(|c: char| c != ' ' && line.starts_with(':')).trim_start();
    let first = cmd.split(' ').next().unwrap_or("");
    let keep = |n: usize| -> String {
        let parts: Vec<&str> = line.splitn(n + 1, ' ').collect();
        if parts.len() > n { format!("{} ***", parts[..n].join(" ")) } else { line.to_string() }
    };
    match first {
        "PASS" => keep(1),
        "AUTHENTICATE" if !matches!(cmd.split(' ').nth(1), Some("PLAIN" | "EXTERNAL" | "SCRAM-SHA-256" | "+" | "*")) => keep(1),
        "OPER" => keep(2),
        "NS" | "NICKSERV" => keep(2),
        _ if (first == "PRIVMSG" || first == "NS") && (upper.contains(" :IDENTIFY ") || upper.contains(" :LOGIN ") || upper.contains("NICKSERV IDENTIFY")) => {
            match line.find(" :") {
                Some(i) => {
                    let after = &line[i + 2..];
                    let w = after.split(' ').next().unwrap_or("");
                    format!("{} :{w} ***", &line[..i])
                }
                None => keep(2),
            }
        }
        _ => line.to_string(),
    }
}

/// `on` / `off` (sans tenir compte de la casse) ; `None` pour autre chose.
fn on_off(w: &str) -> Option<bool> {
    match w.to_ascii_lowercase().as_str() {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

impl State {
    /// Les commandes de ce module ; `None` : ce n'en est pas une. `quiet` : `/.commande`, sans message.
    pub(crate) fn mirc_command(&mut self, name: &str, conn: Option<ConnId>, win: WinId, args: &str, quiet: bool) -> Option<Result<(), String>> {
        let r = match name {
            "anick" | "mnick" | "fullname" => self.cmd_identity(name, win, args, quiet),
            "ajinvite" => {
                if let Some(on) = on_off(split_word(args).0) {
                    self.settings.irc.join_on_invite = on;
                    self.effects.push(Effect::SaveSettings);
                }
                let k = if self.settings.irc.join_on_invite { Key::AjinviteOn } else { Key::AjinviteOff };
                self.say_info(win, k, &[], quiet);
                Ok(())
            }
            "creq" | "sreq" => self.cmd_dcc_request(name == "creq", win, args, quiet),
            "remote" => {
                if let Some(on) = on_off(split_word(args).0) {
                    self.settings.scripts.enabled = on;
                    self.effects.push(Effect::SaveSettings);
                }
                let k = if self.settings.scripts.enabled { Key::RemoteOn } else { Key::RemoteOff };
                self.say_info(win, k, &[], quiet);
                Ok(())
            }
            "toolbar" | "switchbar" | "treebar" => {
                let Some(on) = on_off(split_word(args).0) else { return Some(Err(self.insufficient(name))) };
                let d = &mut self.settings.display;
                match name {
                    "toolbar" => d.toolbar = on,
                    "treebar" => d.treebar = on,
                    _ if !on => d.switchbar = "off".into(),
                    _ if d.switchbar == "off" => d.switchbar = "top".into(),
                    _ => {}
                }
                self.effects.push(Effect::SaveSettings);
                Ok(())
            }
            "aop" => self.cmd_control(ControlKind::Autoop, conn, win, args, quiet),
            "avoice" => self.cmd_control(ControlKind::Autovoice, conn, win, args, quiet),
            "protect" => self.cmd_control(ControlKind::Protect, conn, win, args, quiet),
            "qmsg" | "qme" => {
                if args.is_empty() {
                    return Some(Err(self.insufficient(name)));
                }
                let c = match self.need_conn(conn) {
                    Ok(c) => c,
                    Err(e) => return Some(Err(e)),
                };
                let queries: Vec<String> = self.windows.iter().filter(|w| w.kind == WinKind::Query && w.conn == Some(c)).map(|w| w.name.clone()).collect();
                for q in queries {
                    self.say(c, &q, args, false, name == "qme", quiet);
                }
                Ok(())
            }
            "clipboard" => {
                let (sw, text) = switches(args);
                let text = if sw.contains('n') { format!("{text}\r\n") } else { text.to_string() };
                self.effects.push(Effect::Clipboard { text, append: sw.contains('a') });
                Ok(())
            }
            "savebuf" => self.cmd_savebuf(conn, win, args, quiet),
            _ => return None,
        };
        Some(r)
    }

    /// Message d'information dans la fenêtre, sauf pour `/.commande`.
    fn say_info(&mut self, win: WinId, key: Key, args: &[(&str, &str)], quiet: bool) {
        if !quiet {
            let t = self.t(key, args);
            self.print(win, LineKind::Info, t);
        }
    }

    /// `/anick`, `/mnick`, `/fullname` : pseudo de secours, pseudo principal (sans changer l'actuel, au contraire de
    /// `/nick`), nom complet ; pris en compte à la prochaine connexion.
    fn cmd_identity(&mut self, name: &str, win: WinId, args: &str, quiet: bool) -> Result<(), String> {
        let value = if name == "fullname" { args.trim() } else { split_word(args).0 };
        if value.is_empty() {
            return Err(self.insufficient(name));
        }
        let id = &mut self.settings.identity;
        let (key, mark) = match name {
            "anick" => {
                id.alt_nick = value.to_string();
                (Key::AltNickSet, "nick")
            }
            "mnick" => {
                id.nick = value.to_string();
                (Key::MainNickSet, "nick")
            }
            _ => {
                id.realname = value.to_string();
                (Key::FullnameSet, "name")
            }
        };
        self.effects.push(Effect::SaveSettings);
        self.say_info(win, key, &[(mark, value)], quiet);
        Ok(())
    }

    /// `/creq` (discussions) ou `/sreq` (envois de fichiers) `[+m|-m] [ask|auto|ignore]`, comme les boutons
    /// d'Options › DCC. Le réglage de réduction (`+m`) n'existe pas dans AlpIRC : ignoré.
    fn cmd_dcc_request(&mut self, chat: bool, win: WinId, args: &str, quiet: bool) -> Result<(), String> {
        for w in args.split_whitespace() {
            match (chat, w.to_ascii_lowercase().as_str()) {
                (_, "+m" | "-m") => {}
                (true, "ask") => self.settings.dcc.on_chat = DccChatAction::Ask,
                (true, "auto") => self.settings.dcc.on_chat = DccChatAction::AutoAccept,
                (true, "ignore") => self.settings.dcc.on_chat = DccChatAction::Ignore,
                (false, "ask") => self.settings.dcc.on_send = DccSendAction::Ask,
                (false, "auto") => self.settings.dcc.on_send = DccSendAction::AutoGet,
                (false, "ignore") => self.settings.dcc.on_send = DccSendAction::Ignore,
                _ => return Err(self.insufficient(if chat { "creq" } else { "sreq" })),
            }
            self.effects.push(Effect::SaveSettings);
        }
        let key = if chat {
            match self.settings.dcc.on_chat {
                DccChatAction::Ask => Key::CreqAsk,
                DccChatAction::AutoAccept => Key::CreqAuto,
                DccChatAction::Ignore => Key::CreqIgnore,
            }
        } else {
            match self.settings.dcc.on_send {
                DccSendAction::Ask => Key::SreqAsk,
                DccSendAction::AutoGet => Key::SreqAuto,
                DccSendAction::Ignore => Key::SreqIgnore,
            }
        };
        self.say_info(win, key, &[], quiet);
        Ok(())
    }

    /// `/aop`, `/avoice`, `/protect` `[-lrw] <on|off|pseudo|adresse> [#canal1,#canal2…] [type] [réseau]`, comme
    /// dans mIRC : `on`/`off` allume ou éteint la liste ; `-l` la montre ; `-r` retire ; sans `-w`, l'entrée vaut
    /// pour le réseau de la connexion ; un type (0 à 19) fait un masque de l'adresse connue du pseudo.
    fn cmd_control(&mut self, kind: ControlKind, conn: Option<ConnId>, win: WinId, args: &str, quiet: bool) -> Result<(), String> {
        let list = self.t(
            match kind {
                ControlKind::Autoop => Key::ListAutoOp,
                ControlKind::Autovoice => Key::ListAutoVoice,
                ControlKind::Protect => Key::ListProtect,
            },
            &[],
        );
        let (sw, rest) = switches(args);
        let (target, rest) = split_word(rest);
        if sw.contains('l') || target.is_empty() {
            let state = self.t(if self.settings.address_book.control_on(kind) { Key::StateOn } else { Key::StateOff }, &[]);
            let head = self.t(Key::ControlListHead, &[("list", &list), ("state", &state)]);
            self.print(win, LineKind::Info, head);
            let all_c = self.t(Key::AllChannels, &[]);
            let all_n = self.t(Key::AllNetworks, &[]);
            let rows: Vec<String> = self
                .settings
                .address_book
                .control
                .iter()
                .filter(|e| e.kind == kind)
                .map(|e| {
                    let chans = if e.channels.is_empty() { all_c.clone() } else { e.channels.join(",") };
                    let net = e.network.clone().unwrap_or_else(|| all_n.clone());
                    self.t(Key::ControlListLine, &[("mask", &e.mask), ("channels", &chans), ("network", &net)])
                })
                .collect();
            for r in rows {
                self.print(win, LineKind::Info, r);
            }
            return Ok(());
        }
        if let Some(on) = on_off(target) {
            self.settings.address_book.set_control_on(kind, on);
            self.effects.push(Effect::SaveSettings);
            self.say_info(win, if on { Key::ControlOn } else { Key::ControlOff }, &[("list", &list)], quiet);
            return Ok(());
        }
        // Canaux, type, réseau : dans cet ordre, chacun facultatif.
        let mut words = rest.split_whitespace().peekable();
        let channels: Vec<String> = match words.peek() {
            Some(w) if w.starts_with(['#', '&', '+', '!']) => words.next().unwrap().split(',').filter(|c| !c.is_empty()).map(str::to_string).collect(),
            _ => Vec::new(),
        };
        let mask_type = match words.peek().and_then(|w| w.parse::<u8>().ok()).filter(|t| *t < 20) {
            Some(t) => {
                words.next();
                Some(t)
            }
            None => None,
        };
        let net_word = words.next().map(str::to_string);
        let mask = if target.contains(['!', '@']) {
            target.to_string()
        } else {
            let known = conn.and_then(|c| self.conns.get(&c)).and_then(|c| c.session.user_info(target)).and_then(|u| Some((u.user.clone()?, u.host.clone()?)));
            match (mask_type, known) {
                (Some(t), Some((user, host))) => mask::address_mask(target, &user, &host, t),
                _ => target.to_string(),
            }
        };
        let network = if sw.contains('w') {
            None
        } else {
            net_word.or_else(|| conn.and_then(|c| self.conns.get(&c)).map(|c| c.net_name.clone().unwrap_or_else(|| c.network.clone())).filter(|n| !n.is_empty()))
        };
        let book = &mut self.settings.address_book;
        let same = |e: &ControlEntry| e.kind == kind && e.mask.eq_ignore_ascii_case(&mask);
        if sw.contains('r') {
            let before = book.control.len();
            book.control.retain(|e| !same(e));
            if book.control.len() == before {
                return Err(self.t(Key::ControlNotFound, &[("mask", &mask), ("list", &list)]));
            }
            self.effects.push(Effect::SaveSettings);
            self.say_info(win, Key::ControlRemoved, &[("mask", &mask), ("list", &list)], quiet);
            return Ok(());
        }
        let e = ControlEntry { kind, mask: mask.clone(), channels, network };
        match book.control.iter().position(same) {
            Some(i) => book.control[i] = e,
            None => book.control.push(e),
        }
        self.effects.push(Effect::SaveSettings);
        self.say_info(win, Key::ControlAdded, &[("mask", &mask), ("list", &list)], quiet);
        Ok(())
    }

    /// `/savebuf [-as] [lignes|N-M] <fenêtre> <fichier>` : le texte d'une fenêtre (sans les codes de couleur) dans un
    /// fichier du dossier des scripts (le bac à sable de `/write`). `-a` : à la suite du fichier ; `-s` : la fenêtre
    /// Status ; `lignes` : les N dernières ; `N-M` : de la ligne N à la ligne M.
    fn cmd_savebuf(&mut self, conn: Option<ConnId>, win: WinId, args: &str, quiet: bool) -> Result<(), String> {
        let (sw, mut rest) = switches(args);
        let mut range: Option<(usize, Option<usize>)> = None;
        let (first, r) = split_word(rest);
        if let Some((a, b)) = first.split_once('-').and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))) {
            range = Some((a.max(1), Some(b)));
            rest = r;
        } else if let Ok(n) = first.parse::<usize>() {
            range = Some((0, Some(n)));
            rest = r;
        }
        let (target, file) = if sw.contains('s') {
            (conn.and_then(|c| self.status_of(c)).or(Some(win)), rest.trim())
        } else {
            let (w, f) = split_word(rest);
            (self.find_win(conn, w), f.trim())
        };
        let file = file.trim_matches('"');
        let Some(target) = target else { return Err(self.t(Key::NoSuchWindow, &[("cmd", "savebuf"), ("name", split_word(rest).0)])) };
        if file.is_empty() {
            return Err(self.insufficient("savebuf"));
        }
        let cant = |s: &Self| s.t(Key::ScrCannotWrite, &[("cmd", "savebuf"), ("file", file)]);
        // Le bac à sable des scripts : un chemin relatif, sans « .. ».
        let rel = std::path::Path::new(file);
        if rel.is_absolute() || rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) || file.contains(':') {
            return Err(cant(self));
        }
        let path = self.data_dir.join("scripts").join(rel);
        let lines: Vec<String> = self.win(target).map(|w| w.lines.iter().map(|l| format::strip(&l.text)).collect()).unwrap_or_default();
        let picked: Vec<String> = match range {
            Some((0, Some(n))) => lines[lines.len().saturating_sub(n)..].to_vec(),
            Some((a, Some(b))) => lines.iter().skip(a - 1).take(b.saturating_sub(a - 1)).cloned().collect(),
            _ => lines,
        };
        let mut text = picked.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let ok = if sw.contains('a') {
            use std::io::Write;
            std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| f.write_all(text.as_bytes())).is_ok()
        } else {
            std::fs::write(&path, &text).is_ok()
        };
        if !ok {
            return Err(cant(self));
        }
        self.say_info(win, Key::BufferSaved, &[("n", &picked.len().to_string()), ("file", file)], quiet);
        Ok(())
    }

    /// `/dns [-c] <hôte|pseudo>` : l'adresse IP d'un hôte (recherche faite par l'hôte de l'application) ou d'un
    /// pseudo (son hôte est pris dans la liste interne des adresses, sinon demandé par USERHOST). `-c` vide la file.
    /// Retrouver le nom d'une adresse IP n'est pas possible (pas de résolution inverse sans bibliothèque C).
    pub(crate) fn cmd_dns(&mut self, conn: Option<ConnId>, win: WinId, args: &str, quiet: bool) -> Result<(), String> {
        let (sw, rest) = switches(args);
        if sw.contains('c') {
            self.dns.clear();
            for c in self.conns.values_mut() {
                c.pending_dns.clear();
            }
            if rest.is_empty() {
                return Ok(());
            }
        }
        let target = split_word(rest).0;
        if target.is_empty() {
            return Err(self.insufficient("dns"));
        }
        if let Ok(ip) = target.parse::<std::net::IpAddr>() {
            self.say_info(win, Key::DnsNoReverse, &[("ip", &ip.to_string())], false);
            return Ok(());
        }
        if target.contains('.') {
            self.start_dns(win, target, quiet);
            return Ok(());
        }
        let c = self.need_conn(conn)?;
        let host = self.conns.get(&c).and_then(|x| x.session.user_info(target)).and_then(|u| u.host.clone());
        match host {
            Some(h) => self.start_dns(win, &h, quiet),
            None => {
                let x = self.conns.get_mut(&c).unwrap();
                if x.pending_dns.len() >= 8 {
                    x.pending_dns.remove(0);
                }
                x.pending_dns.push((target.to_string(), win));
                x.session.request_userhost(&[target.to_string()], true);
                self.drain_session(c);
            }
        }
        Ok(())
    }

    fn start_dns(&mut self, win: WinId, host: &str, quiet: bool) {
        let id = self.next_dns;
        self.next_dns = self.next_dns.wrapping_add(1).max(1);
        self.dns.insert(id, (win, host.to_string()));
        self.say_info(win, Key::DnsResolving, &[("host", host)], quiet);
        self.effects.push(Effect::Resolve { id, host: host.to_string() });
    }

    /// Réponse USERHOST : les `/dns pseudo` en attente de cette adresse partent.
    pub(crate) fn dns_userhost(&mut self, conn: ConnId, entries: &[(String, String, String)]) {
        let Some(c) = self.conns.get_mut(&conn) else { return };
        if c.pending_dns.is_empty() {
            return;
        }
        let mut go = Vec::new();
        let pending = std::mem::take(&mut c.pending_dns);
        for (nick, win) in pending {
            match entries.iter().find(|(n, _, _)| c.session.eq(n, &nick)) {
                Some((_, _, host)) => go.push((win, host.clone())),
                None => c.pending_dns.push((nick, win)),
            }
        }
        for (win, host) in go {
            let win = if self.win(win).is_some() { win } else { self.status_of(conn).unwrap_or(win) };
            self.start_dns(win, &host, false);
        }
    }

    /// Réponse de l'hôte à une recherche `/dns` : les adresses (séparées par des virgules) ou l'échec.
    pub(crate) fn dns_answer(&mut self, id: u32, result: Result<String, &crate::NetError>) {
        let Some((win, host)) = self.dns.remove(&id) else { return };
        let win = if self.win(win).is_some() { win } else { self.active.unwrap_or(win) };
        match result {
            Ok(addr) => self.say_info(win, Key::DnsResolved, &[("host", &host), ("addr", &addr)], false),
            Err(_) => self.say_info(win, Key::DnsFailed, &[("host", &host)], false),
        }
    }

    /// `/debug [-c] [on|off|@fenêtre|fichier]` : les lignes reçues (`<-`) et envoyées (`->`) de la connexion, dans
    /// une fenêtre `@` ou un fichier du dossier des journaux (`on` : `debug.log`). Les mots de passe sont masqués.
    /// `off` arrête (`-c` ferme aussi la fenêtre) ; sans paramètre : où va la sortie.
    pub(crate) fn cmd_debug(&mut self, conn: Option<ConnId>, win: WinId, args: &str, quiet: bool) -> Result<(), String> {
        let (sw, rest) = switches(args);
        let c = self.need_conn(conn)?;
        let what = split_word(rest).0;
        let target = match what.to_ascii_lowercase().as_str() {
            "" => {
                let cur = self.conns.get(&c).and_then(|x| x.debug.clone());
                match cur {
                    Some(t) => {
                        let name = self.debug_target_name(&t);
                        self.say_info(win, Key::DebugOn, &[("target", &name)], false);
                    }
                    None => self.say_info(win, Key::DebugOff, &[], false),
                }
                return Ok(());
            }
            "off" => {
                let old = self.conns.get_mut(&c).and_then(|x| x.debug.take());
                if let (true, Some(DebugTarget::Window(w))) = (sw.contains('c'), old) {
                    self.close_win(w);
                }
                self.say_info(win, Key::DebugOff, &[], quiet);
                return Ok(());
            }
            "on" => DebugTarget::File(self.log_folder().join("debug.log")),
            w if w.starts_with('@') => {
                let w = what.to_string();
                let id = match self.find_kind(c, WinKind::Custom, &w).or_else(|| self.find_win(Some(c), &w)) {
                    Some(id) => id,
                    None => self.create_window(WinKind::Custom, Some(c), &w),
                };
                DebugTarget::Window(id)
            }
            _ => {
                let rel = std::path::Path::new(what.trim_matches('"'));
                if rel.is_absolute() || rel.components().any(|x| !matches!(x, std::path::Component::Normal(_))) || what.contains(':') {
                    return Err(self.t(Key::ScrCannotWrite, &[("cmd", "debug"), ("file", what)]));
                }
                DebugTarget::File(self.log_folder().join(rel))
            }
        };
        let name = self.debug_target_name(&target);
        if let Some(x) = self.conns.get_mut(&c) {
            x.debug = Some(target);
        }
        self.say_info(win, Key::DebugOn, &[("target", &name)], quiet);
        Ok(())
    }

    fn debug_target_name(&self, t: &DebugTarget) -> String {
        match t {
            DebugTarget::Window(w) => self.win(*w).map(|x| x.name.clone()).unwrap_or_default(),
            DebugTarget::File(p) => p.display().to_string(),
        }
    }

    /// Une ligne reçue (`incoming`) ou envoyée, recopiée par `/debug` s'il est actif sur cette connexion.
    pub(crate) fn debug_line(&mut self, conn: ConnId, incoming: bool, line: &str) {
        let Some(t) = self.conns.get(&conn).and_then(|c| c.debug.clone()) else { return };
        let text = format!("{} {}", if incoming { "<-" } else { "->" }, debug_mask(line));
        match t {
            DebugTarget::Window(w) if self.win(w).is_some() => self.print(w, LineKind::Normal, text),
            DebugTarget::Window(_) => {
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.debug = None;
                }
            }
            DebugTarget::File(p) => {
                use std::io::Write;
                if let Some(d) = p.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
                    let _ = writeln!(f, "{text}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::debug_mask;

    #[test]
    fn debug_hides_passwords() {
        assert_eq!(debug_mask("PASS secret"), "PASS ***");
        assert_eq!(debug_mask("AUTHENTICATE PLAIN"), "AUTHENTICATE PLAIN");
        assert_eq!(debug_mask("AUTHENTICATE Z3VpAGd1aQBzZWNyZXQ="), "AUTHENTICATE ***");
        assert_eq!(debug_mask("OPER gui secret"), "OPER gui ***");
        assert_eq!(debug_mask("PRIVMSG NickServ :IDENTIFY secret"), "PRIVMSG NickServ :IDENTIFY ***");
        assert_eq!(debug_mask("PRIVMSG x@channels.undernet.org :LOGIN gui secret"), "PRIVMSG x@channels.undernet.org :LOGIN ***");
        assert_eq!(debug_mask("PRIVMSG #lyon :salut"), "PRIVMSG #lyon :salut");
        assert_eq!(debug_mask("NICKSERV IDENTIFY secret"), "NICKSERV IDENTIFY ***");
        assert_eq!(debug_mask(":irc.test.net 001 Guiwald :Welcome"), ":irc.test.net 001 Guiwald :Welcome");
    }
}
