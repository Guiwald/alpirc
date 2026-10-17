//! Timers : `/timer[nom] [-moecpr] [HH:MM] <répétitions> <intervalle> <commande>`, `/timer[nom] off`, `/timers`,
//! `$timer(N|nom)`, `$ltimer`.
//!
//! Comme dans mIRC, la ligne `/timer` est évaluée quand on la tape (ou quand le script l'exécute), puis la
//! commande est évaluée à nouveau à chaque déclenchement : `$!identifiant` retarde l'évaluation. Les timers
//! « en ligne » (sans `-o`) s'arrêtent quand leur connexion se déconnecte.

use crate::calc::{fmt_num, parse_num};
use crate::exec::{Frame, R, split_word, switches};
use crate::wild::wildmatch;
use crate::{Ctx, Engine, Host};

#[derive(Debug, Clone)]
pub struct Timer {
    pub name: String,
    /// Répétitions demandées (0 : sans fin).
    pub reps: u64,
    /// Répétitions restantes.
    pub left: u64,
    /// Intervalle en millisecondes.
    pub interval: i64,
    pub due: i64,
    pub command: String,
    pub offline: bool,
    pub conn: Option<u32>,
    pub window: Option<String>,
    pub script: Option<String>,
    /// En pause : temps restant avant le prochain déclenchement.
    pub paused: Option<i64>,
    pub ms: bool,
    pub catchup: bool,
}

#[derive(Debug, Default)]
pub struct Timers {
    pub list: Vec<Timer>,
}

impl Timers {
    pub fn find(&self, name: &str) -> Option<usize> {
        self.list.iter().position(|t| t.name.eq_ignore_ascii_case(name))
    }

    fn free_number(&self) -> String {
        let mut n = 1u64;
        while self.find(&n.to_string()).is_some() {
            n += 1;
        }
        n.to_string()
    }

    pub fn next_due(&self) -> Option<i64> {
        self.list.iter().filter(|t| t.paused.is_none()).map(|t| t.due).min()
    }

    /// Arrête les timers en ligne d'une connexion.
    pub fn disconnect(&mut self, conn: Option<u32>) {
        self.list.retain(|t| t.offline || t.conn != conn);
    }
}

/// `HH:MM` → prochain instant (ms) où l'heure locale vaut HH:MM.
fn next_clock(h: &dyn Host, now_ms: i64, hh: u32, mm: u32) -> i64 {
    let now = now_ms.div_euclid(1000);
    let off = h.utc_offset(now) as i64;
    let local = now + off;
    let day = local.div_euclid(86_400);
    let mut t = day * 86_400 + (hh as i64) * 3600 + (mm as i64) * 60;
    if t <= local {
        t += 86_400;
    }
    (t - off) * 1000
}

impl Engine {
    fn timer_names_matching(&self, pat: &str) -> Vec<usize> {
        (0..self.timers.list.len())
            .filter(|&i| {
                let n = &self.timers.list[i].name;
                if pat.contains(['*', '?']) { wildmatch(pat, n, false) } else { n.eq_ignore_ascii_case(pat) }
            })
            .collect()
    }

    /// `/timer…` ; `name` : ce qui suit `timer` dans le nom de la commande.
    pub(crate) fn cmd_timer(&mut self, h: &mut dyn Host, ctx: &Ctx, name: &str, args: &str, quiet: bool) -> R<()> {
        let (sw, rest) = switches(args);
        let rest = rest.trim();
        let now = h.now_ms();
        if rest.eq_ignore_ascii_case("off") {
            let pat = if name.is_empty() { "*" } else { name };
            let idx = self.timer_names_matching(pat);
            if idx.is_empty() && !quiet {
                self.info(h, ctx, &format!("* Timer {name} not active"));
            }
            for i in idx.into_iter().rev() {
                let t = self.timers.list.remove(i);
                if !quiet {
                    self.info(h, ctx, &format!("* Timer {} halted", t.name));
                }
            }
            return Ok(());
        }
        if rest.is_empty() {
            let idx = self.timer_names_matching(if name.is_empty() { "*" } else { name });
            if sw.contains('p') || sw.contains('r') || sw.contains('e') {
                for i in idx {
                    if sw.contains('p') {
                        let t = &mut self.timers.list[i];
                        if t.paused.is_none() {
                            t.paused = Some((t.due - now).max(0));
                        }
                    } else if sw.contains('r') {
                        let t = &mut self.timers.list[i];
                        if let Some(left) = t.paused.take() {
                            t.due = now + left;
                        }
                    } else {
                        let t = self.timers.list[i].clone();
                        self.run_timer_command(h, &t)?;
                    }
                }
                return Ok(());
            }
            if idx.is_empty() {
                self.info(h, ctx, &format!("* Timer {name} not active"));
            } else {
                for i in idx {
                    let line = self.timer_line(&self.timers.list[i].clone(), now);
                    self.info(h, ctx, &line);
                }
            }
            return Ok(());
        }
        // [HH:MM] répétitions intervalle commande
        let (mut first, mut r) = split_word(rest);
        let mut at = None;
        if let Some((hh, mm)) = first.split_once(':')
            && let (Ok(hh), Ok(mm)) = (hh.parse::<u32>(), mm.parse::<u32>())
            && hh < 24
            && mm < 60
        {
            at = Some(next_clock(h, now, hh, mm));
            let (f2, r2) = split_word(r);
            first = f2;
            r = r2;
        }
        let (iv, cmd) = split_word(r);
        let (Some(reps), Some(iv)) = (first.parse::<u64>().ok(), parse_num(iv)) else {
            return Err(self.err("/timer: invalid parameters"));
        };
        if cmd.trim().is_empty() {
            return Err(self.err("/timer: insufficient parameters"));
        }
        let ms = sw.contains('m') || sw.contains('h');
        let interval = if ms { iv as i64 } else { (iv * 1000.0) as i64 }.max(0);
        let tname = if name.is_empty() { self.timers.free_number() } else { name.to_string() };
        let c = self.eff_ctx(ctx).into_owned();
        let t = Timer {
            name: tname.clone(),
            reps,
            left: reps,
            interval,
            due: at.unwrap_or(now + interval),
            command: cmd.to_string(),
            offline: sw.contains('o'),
            conn: c.conn,
            window: c.window.clone(),
            script: self.frame().script.clone(),
            paused: None,
            ms,
            catchup: sw.contains('c'),
        };
        match self.timers.find(&tname) {
            Some(i) => self.timers.list[i] = t,
            None => self.timers.list.push(t),
        }
        self.rt.ltimer = tname.clone();
        if !quiet {
            self.info(h, ctx, &format!("* Timer {tname} activated"));
        }
        Ok(())
    }

    fn timer_line(&self, t: &Timer, _now: i64) -> String {
        let delay = if t.ms { format!("{}ms", t.interval) } else { format!("{}s", fmt_num(t.interval as f64 / 1000.0)) };
        let reps = if t.reps == 0 { String::new() } else { format!(" ({} left)", t.left) };
        format!("* Timer {} {delay} delay {}{reps}", t.name, t.command)
    }

    /// `/timers [off]`.
    pub(crate) fn cmd_timers(&mut self, h: &mut dyn Host, ctx: &Ctx, args: &str, quiet: bool) -> R<()> {
        if args.trim().eq_ignore_ascii_case("off") {
            self.timers.list.clear();
            if !quiet {
                self.info(h, ctx, "* All timers halted");
            }
            return Ok(());
        }
        if self.timers.list.is_empty() {
            self.info(h, ctx, "* No active timers");
            return Ok(());
        }
        self.info(h, ctx, "* Active timers:");
        let now = h.now_ms();
        for t in self.timers.list.clone() {
            let line = self.timer_line(&t, now);
            self.info(h, ctx, &line);
        }
        Ok(())
    }

    fn run_timer_command(&mut self, h: &mut dyn Host, t: &Timer) -> R<()> {
        let ctx = Ctx { conn: t.conn, window: t.window.clone(), ..Ctx::default() };
        let block = crate::ast::parse_body(&t.command, 0);
        let frame = Frame { script: t.script.clone(), ..Frame::default() };
        match self.run_top(h, &ctx, frame, &block, false) {
            Ok(()) => Ok(()),
            Err(e) => {
                h.script_error(&ctx, &match e {
                    crate::ScriptError::Sandbox(p) => format!("* {}", crate::ScriptError::Sandbox(p)),
                    other => other.to_string(),
                });
                Ok(())
            }
        }
    }

    /// Déclenche les timers échus, efface les variables et éléments de table expirés. Le client l'appelle
    /// régulièrement (au plus tard à [`Engine::next_timer_due`]).
    pub fn tick(&mut self, host: &mut dyn Host, now_ms: i64) {
        self.flush_pending(host);
        self.vars.tick(now_ms);
        self.tables.tick(now_ms);
        let mut fired: Vec<String> = Vec::new();
        for _ in 0..10_000 {
            let next = self
                .timers
                .list
                .iter()
                .enumerate()
                .filter(|(_, t)| t.paused.is_none() && t.due <= now_ms && !fired.iter().any(|f| f.eq_ignore_ascii_case(&t.name)))
                .min_by_key(|(_, t)| t.due)
                .map(|(i, _)| i);
            let Some(i) = next else { break };
            let t = self.timers.list[i].clone();
            fired.push(t.name.clone());
            // Mise à jour avant l'exécution : la commande peut arrêter ou relancer son propre timer.
            if t.reps > 0 && t.left <= 1 {
                self.timers.list.remove(i);
            } else {
                let tm = &mut self.timers.list[i];
                if tm.reps > 0 {
                    tm.left -= 1;
                }
                let mut nd = tm.due + tm.interval;
                if nd <= now_ms && !tm.catchup {
                    nd = now_ms + tm.interval;
                }
                tm.due = nd;
            }
            let _ = self.run_timer_command(host, &t);
        }
    }

    /// Prochaine échéance (timer, variable `-u`, élément de table `-u`), en ms depuis 1970 ; `Some(0)` s'il reste
    /// des événements `on LOAD`/`on START` en attente.
    pub fn next_timer_due(&self) -> Option<i64> {
        if !self.pending.is_empty() {
            return Some(0);
        }
        [self.timers.next_due(), self.vars.next_due(), self.tables.next_due()].into_iter().flatten().min()
    }

    /// `$timer(N|nom).secs|reps|delay|com|type|ms|name`, `$timer(0)`.
    pub(crate) fn timer_ident(&mut self, h: &mut dyn Host, args: &[String], prop: Option<&str>) -> String {
        let a = args.first().map(|s| s.as_str()).unwrap_or("");
        let t = match a.parse::<usize>() {
            Ok(0) if self.timers.find("0").is_none() => return self.timers.list.len().to_string(),
            Ok(k) if self.timers.find(a).is_none() => self.timers.list.get(k - 1),
            _ => self.timers.find(a).map(|i| &self.timers.list[i]),
        };
        let Some(t) = t else { return String::new() };
        let now = h.now_ms();
        match prop {
            Some("secs") => {
                let left = t.paused.unwrap_or(t.due - now).max(0);
                ((left + 999) / 1000).to_string()
            }
            Some("ms") | Some("mmt") => t.paused.unwrap_or(t.due - now).max(0).to_string(),
            Some("reps") => if t.reps == 0 { "0".into() } else { t.left.to_string() },
            Some("delay") => {
                if t.ms {
                    t.interval.to_string()
                } else {
                    fmt_num(t.interval as f64 / 1000.0)
                }
            }
            Some("com") => t.command.clone(),
            Some("type") => (if t.offline { "offline" } else { "online" }).into(),
            Some("pause") => crate::idents::tf(t.paused.is_some()),
            Some("cid") => t.conn.map(|c| c.to_string()).unwrap_or_default(),
            _ => t.name.clone(),
        }
    }
}
