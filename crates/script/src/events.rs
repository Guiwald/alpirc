//! Événements : correspondance des gestionnaires (`on`, `ctcp`, `raw`) avec un [`EventInfo`], déclenchement dans
//! chaque remote chargé (le premier gestionnaire qui correspond dans chaque fichier, comme dans mIRC),
//! identifiants de l'événement en cours (`$nick`, `$chan`, `$knick`…), et file d'attente des `on LOAD`,
//! `on START` et `on UNLOAD` (le chargement n'a pas d'hôte sous la main).
//!
//! Le client déclenche un événement avant d'afficher son texte habituel : un `/halt` ou `/haltdef` dans un
//! gestionnaire le lui signale ([`FireResult::haltdef`]). Le préfixe `^` est accepté et ne change rien.

use std::sync::Arc;

use crate::ast::Block;
use crate::exec::{Abort, Frame, R, words};
use crate::idents::push_regml;
use crate::source::{Handler, event_name};
use crate::wild::wildmatch;
use crate::{Ctx, Engine, EventInfo, EventKind, FireResult, Host, Msg};

/// Exécution différée faute d'hôte.
#[derive(Debug, Clone)]
pub enum Pending {
    /// Déclencher ces événements dans un seul script (`on LOAD`, `on START`).
    Events { script: String, kinds: Vec<EventKind> },
    /// Exécuter un corps (gestionnaire `on UNLOAD` d'un script déjà retiré).
    Run { script: String, body: Arc<Block>, event: EventInfo },
}

/// `$1-` d'un événement.
pub fn event_args(ev: &EventInfo) -> Vec<String> {
    match ev.kind {
        Some(EventKind::Ctcp) | Some(EventKind::CtcpReply) => words(&format!("{} {}", ev.what, ev.text)),
        Some(EventKind::Mode) | Some(EventKind::RawMode) => {
            if ev.text.is_empty() {
                words(&ev.what)
            } else if ev.what.is_empty() || ev.text.starts_with(&ev.what) {
                words(&ev.text)
            } else {
                words(&format!("{} {}", ev.what, ev.text))
            }
        }
        _ => words(&ev.text),
    }
}

/// `1,2,5-7` ou `*`.
fn id_matches(field: &str, id: &str) -> bool {
    let f = field.trim();
    if f.is_empty() || f == "*" {
        return true;
    }
    let Ok(id) = id.trim().parse::<i64>() else {
        return false;
    };
    f.split(',').any(|p| match p.split_once('-') {
        Some((a, b)) => {
            let a = a.trim().parse::<i64>().unwrap_or(i64::MIN);
            let b = b.trim().parse::<i64>().unwrap_or(i64::MAX);
            id >= a && id <= b
        }
        None => p.trim().parse::<i64>().ok() == Some(id),
    })
}

impl Engine {
    /// Valeur d'une clé de [`EventInfo::extra`] pour l'événement en cours.
    pub(crate) fn event_extra(&self, ctx: &Ctx, key: &str) -> Option<String> {
        ctx.event.as_ref().and_then(|e| e.get(key)).map(|s| s.to_string())
    }

    /// Identifiants sans paramètres de l'événement en cours ; `None` : à demander à l'hôte.
    pub(crate) fn event_ident(&mut self, h: &mut dyn Host, ctx: &Ctx, name: &str) -> Option<String> {
        let ev = ctx.event.as_ref();
        let nonempty = |s: &str| if s.is_empty() { None } else { Some(s.to_string()) };
        match name {
            "nick" => ev.and_then(|e| nonempty(&e.nick)),
            "address" => ev.and_then(|e| nonempty(&e.address)),
            "fulladdress" => ev.filter(|e| !e.nick.is_empty()).map(|e| {
                if e.address.is_empty() { e.nick.clone() } else { format!("{}!{}", e.nick, e.address) }
            }),
            "site" => ev.and_then(|e| e.address.split_once('@').map(|(_, s)| s.to_string())),
            "wildsite" => ev.and_then(|e| e.address.split_once('@').map(|(_, s)| format!("*!*@{s}"))),
            "chan" => {
                if let Some(c) = ev.and_then(|e| e.chan.clone()) {
                    return Some(c);
                }
                let w = ctx.window.clone()?;
                if h.is_channel(ctx, &w) { Some(w) } else { None }
            }
            "target" => ev.and_then(|e| e.target.clone()),
            "event" => ev.and_then(|e| e.kind).map(|k| event_name(k).to_string()),
            "knick" | "newnick" | "opnick" | "vnick" | "hnick" | "bnick" | "banmask" | "rawmsg" | "msgtags"
            | "keyval" | "keychar" | "filename" | "dname" | "devent" => {
                let v = ev.and_then(|e| e.get(name)).map(|s| s.to_string());
                if v.is_some() {
                    return v;
                }
                match (name, ev) {
                    ("devent", Some(e)) if e.kind == Some(EventKind::Dialog) => Some(e.what.clone()),
                    _ => None,
                }
            }
            "numeric" => {
                let e = ev?;
                e.get("numeric").map(|s| s.to_string()).or_else(|| {
                    (e.kind == Some(EventKind::Raw) && e.what.chars().all(|c| c.is_ascii_digit()) && !e.what.is_empty())
                        .then(|| e.what.clone())
                })
            }
            "signal" => ev.filter(|e| e.kind == Some(EventKind::Signal)).map(|e| e.what.clone()),
            "parms" => ev.map(|e| e.text.clone()),
            "ctcp" => ev.filter(|e| matches!(e.kind, Some(EventKind::Ctcp) | Some(EventKind::CtcpReply))).map(|e| e.what.clone()),
            _ => None,
        }
    }

    /// Sorte d'une fenêtre : `#`, `?`, `=`, `@`, ou `s` pour Status.
    fn window_type(&mut self, h: &mut dyn Host, ctx: &Ctx, w: &str) -> char {
        if w.starts_with('@') {
            '@'
        } else if w.starts_with('=') {
            '='
        } else if w.eq_ignore_ascii_case("status window") || w.eq_ignore_ascii_case("status") || w.is_empty() {
            's'
        } else if h.is_channel(ctx, w) {
            '#'
        } else {
            '?'
        }
    }

    /// Évalue un champ s'il contient des identifiants ou variables (`on *:TEXT:%trigger:#:`).
    fn field(&mut self, h: &mut dyn Host, ctx: &Ctx, f: &str) -> R<String> {
        if f.contains(['$', '%']) { self.eval_text(h, ctx, f) } else { Ok(f.to_string()) }
    }

    fn match_text(&mut self, h: &mut dyn Host, ctx: &Ctx, hd: &Handler, pat: &str, text: &str) -> R<bool> {
        if hd.pfx.regex {
            let p = self.field(h, ctx, pat)?;
            let Ok(re) = crate::regexid::compile(&p) else { return Ok(false) };
            let t = if re.strip { alpirc_proto::format::strip(text) } else { text.to_string() };
            let found = crate::regexid::find_all(&re, &t);
            if found.is_empty() {
                return Ok(false);
            }
            let mut ml = Vec::new();
            for f in &found {
                push_regml(&mut ml, f);
            }
            self.rt.regml.insert(String::new(), ml);
            return Ok(true);
        }
        let p = self.field(h, ctx, pat)?;
        Ok(p.is_empty() || wildmatch(&p, text, false))
    }

    /// Cible des événements de texte : `#`, `?`, `*`, `=`, `#a,#b`.
    fn match_target(&mut self, h: &mut dyn Host, ctx: &Ctx, field: &str, ev: &EventInfo) -> R<bool> {
        let f = self.field(h, ctx, field)?;
        let is_chat = ev.target.as_deref().is_some_and(|t| t.starts_with('=')) || ev.kind == Some(EventKind::Chat);
        Ok(f.split(',').map(|t| t.trim()).any(|t| match t {
            "" | "*" => true,
            "#" => ev.chan.is_some(),
            "?" => ev.chan.is_none() && !is_chat,
            "=" => is_chat,
            "@" => ev.target.as_deref().is_some_and(|x| x.starts_with('@')),
            other => match &ev.chan {
                Some(c) => wildmatch(other, c, false),
                None => ev.target.as_deref().is_some_and(|x| wildmatch(other, x, false)),
            },
        }))
    }

    /// Canal des événements `JOIN`, `PART`, `KICK`… : `#`, `*`, `#a,#b`.
    fn match_chan(&mut self, h: &mut dyn Host, ctx: &Ctx, field: &str, ev: &EventInfo) -> R<bool> {
        let f = self.field(h, ctx, field)?;
        let chan = ev.chan.clone().or_else(|| ev.target.clone());
        Ok(f.split(',').map(|t| t.trim()).any(|t| match (t, &chan) {
            ("" | "*", _) => true,
            ("#", Some(_)) => true,
            (_, Some(c)) => wildmatch(t, c, false),
            _ => false,
        }))
    }

    /// Fenêtre des événements `INPUT`, `OPEN`, `CLOSE`, `ACTIVE`, `KEYDOWN` : `*`, `#`, `?`, `=`, `!`, `@`,
    /// `@nom`, `#canal`.
    fn match_window(&mut self, h: &mut dyn Host, ctx: &Ctx, field: &str, ev: &EventInfo) -> R<bool> {
        let f = self.field(h, ctx, field)?;
        let w = ev.target.clone().or_else(|| ctx.window.clone()).unwrap_or_default();
        let ty = self.window_type(h, ctx, &w);
        Ok(f.split(',').map(|t| t.trim()).any(|t| match t {
            "" | "*" => true,
            "#" => ty == '#',
            "?" => ty == '?',
            "=" => ty == '=',
            "@" => ty == '@',
            "!" => w.starts_with('!'),
            other => wildmatch(other, &w, false),
        }))
    }

    /// Le gestionnaire correspond-il à l'événement ? Rend `$ulevel`.
    fn handler_matches(&mut self, h: &mut dyn Host, ctx: &Ctx, hd: &Handler, ev: &EventInfo) -> R<Option<String>> {
        use EventKind::*;
        let Some(kind) = ev.kind else { return Ok(None) };
        if hd.kind != Some(kind) {
            return Ok(None);
        }
        let me = h.identifier(ctx, "me", &[], None).unwrap_or_default();
        let is_me = !ev.nick.is_empty() && crate::wild::fold_eq(&ev.nick, &me);
        if (hd.pfx.not_me && is_me) || (hd.pfx.me_only && !is_me) {
            return Ok(None);
        }
        let ulevel = if kind == Raw {
            "*".to_string()
        } else {
            let addr = if ev.address.is_empty() { format!("{}!*@*", ev.nick) } else { format!("{}!{}", ev.nick, ev.address) };
            match self.users.event_level(&hd.level, hd.pfx.exact, &addr) {
                Some(l) => l,
                None => return Ok(None),
            }
        };
        if hd.pfx.op {
            let Some(chan) = ev.chan.clone() else { return Ok(None) };
            if !h.member_modes(ctx, &chan, &me).is_some_and(|m| m.contains('o')) {
                return Ok(None);
            }
        }
        let f = |i: usize| hd.fields.get(i).map(|s| s.as_str()).unwrap_or("*");
        let ok = match kind {
            Text | Action | Notice => self.match_text(h, ctx, hd, f(0), &ev.text)? && self.match_target(h, ctx, f(1), ev)?,
            Ctcp | CtcpReply => {
                let full = if ev.text.is_empty() { ev.what.clone() } else { format!("{} {}", ev.what, ev.text) };
                let m = self.match_text(h, ctx, hd, f(0), &full)? || self.match_text(h, ctx, hd, f(0), &ev.what)?;
                m && (kind == CtcpReply || self.match_target(h, ctx, f(1), ev)?)
            }
            Join | Part | Kick | Topic | Invite | Mode | RawMode | Op | Deop | Help | Dehelp | Voice | Devoice | Ban
            | Unban => self.match_chan(h, ctx, f(0), ev)?,
            Logon | Snotice | Wallops | Error | Chat => self.match_text(h, ctx, hd, f(0), &ev.text)?,
            Input | Close | Active => self.match_window(h, ctx, f(0), ev)?,
            Open => self.match_window(h, ctx, f(0), ev)? && self.match_text(h, ctx, hd, f(1), &ev.text)?,
            FileSent | FileRcvd | SendFail | GetFail => {
                let p = self.field(h, ctx, f(0))?;
                let file = ev.get("filename").unwrap_or(&ev.text);
                let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
                wildmatch(&p, file, false) || wildmatch(&p, base, false)
            }
            Signal => {
                let p = self.field(h, ctx, f(0))?;
                wildmatch(&p, &ev.what, false)
            }
            Dialog => {
                let p = self.field(h, ctx, f(0))?;
                let dname = ev.get("dname").unwrap_or("");
                wildmatch(&p, dname, false)
                    && (f(1) == "*" || f(1).eq_ignore_ascii_case(&ev.what))
                    && id_matches(f(2), ev.get("did").unwrap_or("0"))
            }
            KeyDown | KeyUp => {
                let key = ev.get("keyval").unwrap_or("");
                self.match_window(h, ctx, f(0), ev)? && (f(1) == "*" || f(1).split(',').any(|k| k.trim() == key))
            }
            Raw => {
                let w = f(0);
                (w == "*" || w.eq_ignore_ascii_case(&ev.what)) && self.match_text(h, ctx, hd, f(1), &ev.text)?
            }
            _ => true,
        };
        Ok(ok.then_some(ulevel))
    }

    /// Déclenche un événement dans les remotes (tous, ou un seul). Rend vrai si un gestionnaire a fait `/halt`
    /// ou `/haltdef`.
    pub(crate) fn fire_scripts(&mut self, h: &mut dyn Host, ctx: &Ctx, ev: &EventInfo, only: Option<&str>) -> bool {
        let snapshot: Vec<(String, Vec<Arc<Handler>>)> = self
            .scripts
            .iter()
            .filter(|s| only.is_none_or(|o| s.name == o))
            .map(|s| (s.name.clone(), s.p.handlers.iter().filter(|hd| hd.kind == ev.kind && ev.kind.is_some()).cloned().collect()))
            .filter(|(_, v): &(String, Vec<_>)| !v.is_empty())
            .collect();
        let mut haltdef = false;
        for (sname, handlers) in snapshot {
            for hd in handlers {
                let Some(s) = self.scripts.iter().find(|s| s.name == sname) else { break };
                if !s.group_on(&hd.group) {
                    continue;
                }
                if hd.pfx.amp && haltdef {
                    continue;
                }
                let frame = Frame { script: Some(sname.clone()), args: event_args(ev), line: hd.line, ..Frame::default() };
                let m = self.top(h, |e, h| {
                    e.rt.frames.push(frame);
                    let r = e.handler_matches(h, ctx, &hd, ev);
                    e.rt.frames.pop();
                    r
                });
                let ulevel = match m {
                    Ok(Some(l)) => l,
                    Ok(None) => continue,
                    Err(e) => {
                        if let Some(msg) = e.message() {
                            crate::exec::report(h, ctx, &msg);
                        }
                        continue;
                    }
                };
                let saved = self.rt.haltdef;
                let saved_halted = self.rt.halted;
                self.rt.haltdef = false;
                self.rt.halted = haltdef;
                let frame = Frame { args: event_args(ev), script: Some(sname.clone()), ulevel, line: hd.line, ..Frame::default() };
                let body = hd.body.clone();
                let r = self.top(h, |e, h| e.run_routine(h, ctx, frame, &body));
                let flagged = self.rt.haltdef;
                self.rt.haltdef = saved;
                self.rt.halted = saved_halted;
                match r {
                    Ok(_) => haltdef |= flagged,
                    Err(Abort::Halt) => haltdef = true,
                    Err(e) => {
                        haltdef |= flagged;
                        if let Some(msg) = e.message() {
                            crate::exec::report(h, ctx, &msg);
                        }
                    }
                }
                break;
            }
        }
        haltdef
    }

    /// Déclenche un événement dans tous les remotes chargés, dans l'ordre de chargement ; dans chaque fichier, seul
    /// le premier gestionnaire qui correspond s'exécute. Tient compte des niveaux (liste des utilisateurs), du texte
    /// (jokers `* ? &`, ou expression régulière avec le préfixe `$`), de la cible, des groupes, et des préfixes
    /// `!` (pas pour nos actions), `@` (seulement si nous sommes opérateur), `&` (seulement si personne n'a fait
    /// `/haltdef`), `me:` (seulement pour nos actions). `on DISCONNECT` arrête aussi les timers en ligne de la
    /// connexion.
    pub fn fire(&mut self, host: &mut dyn Host, ctx: &Ctx, event: &EventInfo) -> FireResult {
        self.flush_pending(host);
        let mut c = ctx.clone();
        c.event = Some(event.clone());
        if event.conn.is_some() {
            c.conn = event.conn;
        }
        if event.kind == Some(EventKind::Disconnect) {
            self.timers.disconnect(c.conn);
        }
        let haltdef = self.fire_scripts(host, &c, event, None);
        FireResult { haltdef }
    }

    /// Exécute les `on LOAD`/`on START`/`on UNLOAD` laissés en attente par [`Engine::load_text`],
    /// [`Engine::load_file`] et [`Engine::unload`]. Les autres appels qui ont un hôte le font d'eux-mêmes ; le
    /// client peut l'appeler juste après un chargement (après avoir averti l'utilisateur, voir
    /// [`Engine::runs_at_startup`]).
    pub fn flush_pending(&mut self, host: &mut dyn Host) {
        if self.pending.is_empty() || !self.rt.frames.is_empty() {
            return;
        }
        let list = std::mem::take(&mut self.pending);
        for p in list {
            match p {
                Pending::Events { script, kinds } => {
                    for k in kinds {
                        let ev = EventInfo { kind: Some(k), ..EventInfo::default() };
                        let ctx = Ctx { event: Some(ev.clone()), ..Ctx::default() };
                        self.fire_scripts(host, &ctx, &ev, Some(&script));
                    }
                }
                Pending::Run { script, body, event } => {
                    let ctx = Ctx { event: Some(event), ..Ctx::default() };
                    let frame = Frame { script: Some(script), ..Frame::default() };
                    if let Err(e) = self.top(host, |e, h| e.run_routine(h, &ctx, frame, &body))
                        && let Some(m) = e.message()
                    {
                        crate::exec::report(host, &ctx, &m);
                    }
                }
            }
        }
    }

    /// Oublie les `on LOAD`/`on START`/`on UNLOAD` en attente (l'utilisateur a refusé le script).
    pub fn discard_pending(&mut self) {
        self.pending.clear();
    }

    /// `/signal [-n] nom [paramètres]` : déclenche `on SIGNAL` tout de suite (avec ou sans `-n`).
    pub(crate) fn cmd_signal(&mut self, h: &mut dyn Host, ctx: &Ctx, args: &str) -> R<()> {
        let (_, rest) = crate::exec::switches(args);
        let (name, params) = crate::exec::split_word(rest);
        if name.is_empty() {
            return Err(self.err(Msg::InsufficientParams { cmd: "signal".into() }));
        }
        let ev = EventInfo { kind: Some(EventKind::Signal), conn: ctx.conn, what: name.to_string(), text: params.to_string(), ..EventInfo::default() };
        let mut c = ctx.clone();
        c.event = Some(ev.clone());
        self.fire_scripts(h, &c, &ev, None);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert!(id_matches("*", "3"));
        assert!(id_matches("1,2,5-7", "6"));
        assert!(!id_matches("1,2,5-7", "4"));
    }
}
