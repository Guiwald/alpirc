//! Exécution : instructions, lignes de commande, appels d'alias, aiguillage vers les commandes du moteur ou de
//! l'hôte, garde-fous (profondeur, boucles infinies).

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::ast::{Block, Stmt, parse_body};
use crate::source::Alias;
use crate::{Ctx, Engine, Host, ScriptError};

/// Message des garde-fous.
pub const TOO_DEEP: &str = "Error: script too deep / infinite loop";

/// Arrêt d'un script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Abort {
    /// `/halt` ou `$$1` vide : arrêt silencieux.
    Halt,
    /// Erreur affichée (déjà au format de mIRC : `* /msg: insufficient parameters (line 3, x.mrc)`).
    Error(String),
    /// Accès à un fichier hors du dossier des scripts (chemin demandé).
    Sandbox(String),
}

impl Abort {
    /// Texte à montrer à l'utilisateur (`None` pour un arrêt silencieux).
    pub fn message(&self) -> Option<String> {
        match self {
            Abort::Halt => None,
            Abort::Error(m) => Some(m.clone()),
            Abort::Sandbox(p) => Some(format!("* {}", ScriptError::Sandbox(p.clone()))),
        }
    }

    pub fn into_script_error(self) -> Option<ScriptError> {
        match self {
            Abort::Halt => None,
            Abort::Error(m) => Some(ScriptError::Message(m)),
            Abort::Sandbox(p) => Some(ScriptError::Sandbox(p)),
        }
    }
}

pub type R<T> = Result<T, Abort>;

/// Suite à donner après une instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flow {
    Normal,
    Break,
    Continue,
    Return(Option<String>),
    Goto(String),
}

/// Contexte d'une routine en cours (alias, événement, ligne tapée, timer, popup…).
#[derive(Debug, Default, Clone)]
pub struct Frame {
    /// Variables locales (`/var`) : nom en minuscules → (nom d'origine, valeur).
    pub locals: HashMap<String, (String, String)>,
    /// `$1-`.
    pub args: Vec<String>,
    /// Script d'où vient le code (alias locaux, `$script`).
    pub script: Option<String>,
    /// Alias en cours (un alias qui s'appelle lui-même en commande appelle la commande intégrée).
    pub alias: Option<String>,
    pub isid: bool,
    pub prop: String,
    pub line: usize,
    /// Ligne tapée avec un seul `/` : rien n'est évalué.
    pub no_eval: bool,
    /// `/scid N` sans commande : connexion utilisée pour la suite de la routine.
    pub conn: Option<Option<u32>>,
    /// `$ulevel`.
    pub ulevel: String,
}

/// État d'exécution partagé par les routines imbriquées.
#[derive(Debug)]
pub struct Runtime {
    pub frames: Vec<Frame>,
    pub iterations: u64,
    pub max_iterations: u64,
    pub max_depth: usize,
    pub v1: String,
    pub v2: String,
    pub result: String,
    /// `/haltdef` vu pendant le gestionnaire en cours.
    pub haltdef: bool,
    /// Un gestionnaire précédent du même événement a fait `/haltdef` (`$halted`).
    pub halted: bool,
    /// `$regml` : nom → (texte, position à partir de 1).
    pub regml: HashMap<String, Vec<(String, usize)>>,
    pub readn: usize,
    /// Jeton courant de `$*`.
    pub star: Option<String>,
    /// `$style(N)` dans un libellé de popup : (coché, grisé).
    pub style: (bool, bool),
    /// Sorte de menu en cours de construction (`$menu`).
    pub menu: String,
    /// Variables `set -u0` à effacer à la fin du script.
    pub u0: Vec<String>,
    pub ltimer: String,
    /// La routine du bas de la pile vient d'une ligne tapée (les `/set`… affichent un écho).
    pub typed: bool,
}

impl Default for Runtime {
    fn default() -> Self {
        Runtime {
            frames: Vec::new(),
            iterations: 0,
            max_iterations: 1_000_000,
            max_depth: 100,
            v1: String::new(),
            v2: String::new(),
            result: String::new(),
            haltdef: false,
            halted: false,
            regml: HashMap::new(),
            readn: 0,
            star: None,
            style: (false, false),
            menu: String::new(),
            u0: Vec::new(),
            ltimer: String::new(),
            typed: false,
        }
    }
}

/// Sépare le premier mot du reste.
pub fn split_word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    match s.find([' ', '\t']) {
        Some(i) => (&s[..i], s[i + 1..].trim_start()),
        None => (s, ""),
    }
}

/// Paramètres d'un alias appelé en commande : les mots de la ligne.
pub fn words(s: &str) -> Vec<String> {
    s.split(' ').filter(|w| !w.is_empty()).map(|w| w.to_string()).collect()
}

/// Lit des switches en tête d'arguments (`-abc`, plusieurs groupes possibles) ; rend (lettres, reste).
pub fn switches(args: &str) -> (String, &str) {
    let mut sw = String::new();
    let mut rest = args.trim_start();
    while rest.starts_with('-') && rest.len() > 1 && !rest[1..].starts_with([' ', '-']) {
        let (w, r) = split_word(rest);
        // `-5` : un nombre négatif n'est pas un switch.
        if w[1..].chars().all(|c| c.is_ascii_digit() || c == '.') {
            break;
        }
        sw.push_str(&w[1..]);
        rest = r;
    }
    (sw, rest)
}

/// Valeur numérique qui suit une lettre de switch (`u10` → 10).
pub fn switch_num(sw: &str, letter: char) -> Option<f64> {
    let i = sw.find(letter)?;
    let digits: String = sw[i + 1..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    digits.parse().ok()
}

impl Engine {
    pub(crate) fn frame(&self) -> &Frame {
        static EMPTY: std::sync::OnceLock<Frame> = std::sync::OnceLock::new();
        self.rt.frames.last().unwrap_or_else(|| EMPTY.get_or_init(Frame::default))
    }

    pub(crate) fn frame_mut(&mut self) -> &mut Frame {
        if self.rt.frames.is_empty() {
            self.rt.frames.push(Frame::default());
        }
        self.rt.frames.last_mut().unwrap()
    }

    /// Erreur au format de mIRC, avec la ligne et le fichier si on est dans un script.
    pub(crate) fn err(&self, msg: impl Into<String>) -> Abort {
        let msg = msg.into();
        let msg = if msg.starts_with("* ") { msg } else { format!("* {msg}") };
        match self.rt.frames.last() {
            Some(f) if f.script.is_some() && f.line > 0 => {
                Abort::Error(format!("{msg} (line {}, {})", f.line, f.script.as_deref().unwrap_or("")))
            }
            _ => Abort::Error(msg),
        }
    }

    /// Contexte effectif (connexion changée par `/scid`).
    pub(crate) fn eff_ctx<'a>(&self, ctx: &'a Ctx) -> Cow<'a, Ctx> {
        match self.rt.frames.last().and_then(|f| f.conn) {
            Some(c) if c != ctx.conn => {
                let mut x = ctx.clone();
                x.conn = c;
                Cow::Owned(x)
            }
            _ => Cow::Borrowed(ctx),
        }
    }

    pub(crate) fn count_iter(&mut self) -> R<()> {
        self.rt.iterations += 1;
        if self.rt.iterations > self.rt.max_iterations { Err(self.err(TOO_DEEP)) } else { Ok(()) }
    }

    /// Exécution « de premier niveau » : remet les compteurs à zéro au début, fait le ménage (`set -u0`) à la fin.
    pub(crate) fn top<T>(&mut self, h: &mut dyn Host, f: impl FnOnce(&mut Self, &mut dyn Host) -> R<T>) -> R<T> {
        let outer = self.rt.frames.is_empty();
        if outer {
            self.rt.iterations = 0;
        }
        let r = f(self, h);
        if outer {
            self.rt.frames.clear();
            self.rt.typed = false;
            let names = std::mem::take(&mut self.rt.u0);
            for n in names {
                self.vars.remove(&n);
            }
        }
        r
    }

    /// Exécute une routine (corps d'alias, d'événement, de popup, de timer…) dans un nouveau contexte.
    pub(crate) fn run_routine(&mut self, h: &mut dyn Host, ctx: &Ctx, frame: Frame, body: &[Stmt]) -> R<Option<String>> {
        if self.rt.frames.len() >= self.rt.max_depth {
            return Err(self.err(TOO_DEEP));
        }
        self.rt.frames.push(frame);
        let r = self.exec_block(h, ctx, body);
        let r = match r {
            Ok(Flow::Goto(l)) => Err(self.err(format!("/goto: '{l}' not found"))),
            Ok(Flow::Return(v)) => Ok(v),
            Ok(_) => Ok(None),
            Err(e) => Err(e),
        };
        self.rt.frames.pop();
        r
    }

    pub(crate) fn exec_block(&mut self, h: &mut dyn Host, ctx: &Ctx, block: &[Stmt]) -> R<Flow> {
        let mut i = 0;
        while i < block.len() {
            match self.exec_stmt(h, ctx, &block[i])? {
                Flow::Normal => i += 1,
                Flow::Goto(label) => match find_label(block, &label) {
                    Some(j) => {
                        self.count_iter()?;
                        i = j + 1;
                    }
                    None => return Ok(Flow::Goto(label)),
                },
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    fn set_line(&mut self, line: usize) {
        if let Some(f) = self.rt.frames.last_mut() {
            f.line = line;
        }
    }

    fn exec_stmt(&mut self, h: &mut dyn Host, ctx: &Ctx, s: &Stmt) -> R<Flow> {
        match s {
            Stmt::Cmd { line, text } => {
                self.set_line(*line);
                self.exec_line(h, ctx, text)
            }
            Stmt::If { line, branches } => {
                self.set_line(*line);
                for (cond, body) in branches {
                    let take = match cond {
                        Some(c) => self.eval_cond(h, ctx, c)?,
                        None => true,
                    };
                    if take {
                        return self.exec_block(h, ctx, body);
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::While { line, cond, body } => {
                loop {
                    self.set_line(*line);
                    self.count_iter()?;
                    if !self.eval_cond(h, ctx, cond)? {
                        break;
                    }
                    match self.exec_block(h, ctx, body)? {
                        Flow::Break => break,
                        Flow::Normal | Flow::Continue => {}
                        other => return Ok(other),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Label { .. } => Ok(Flow::Normal),
            Stmt::Block { body, .. } => self.exec_block(h, ctx, body),
        }
    }

    /// Cherche un alias : ceux de `/alias`, puis ceux du script courant (locaux compris), puis les autres.
    pub(crate) fn find_alias(&self, name: &str) -> Option<(Arc<Alias>, Option<String>)> {
        if let Some(a) = self.dyn_aliases.iter().find(|a| a.name == name) {
            return Some((a.clone(), None));
        }
        let idx = self.alias_index.get(name)?;
        let cur = self.frame().script.as_deref();
        if let Some(cur) = cur {
            for &i in idx {
                let s = &self.scripts[i];
                if s.name == cur
                    && let Some(a) = s.p.aliases.iter().find(|a| a.name == name && s.group_on(&a.group))
                {
                    return Some((a.clone(), Some(s.name.clone())));
                }
            }
        }
        for &i in idx {
            let s = &self.scripts[i];
            if let Some(a) = s.p.aliases.iter().find(|a| a.name == name && !a.local && s.group_on(&a.group)) {
                return Some((a.clone(), Some(s.name.clone())));
            }
        }
        None
    }

    /// Appelle un alias (en commande : `isid` faux ; en identifiant : vrai).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn call_alias(
        &mut self,
        h: &mut dyn Host,
        ctx: &Ctx,
        alias: &Arc<Alias>,
        script: Option<String>,
        args: Vec<String>,
        isid: bool,
        prop: &str,
    ) -> R<Option<String>> {
        let conn = self.rt.frames.last().and_then(|f| f.conn);
        let frame = Frame {
            args,
            script,
            alias: Some(alias.name.clone()),
            isid,
            prop: prop.to_string(),
            line: alias.line,
            conn,
            ..Frame::default()
        };
        let body = alias.body.clone();
        self.run_routine(h, ctx, frame, &body)
    }

    /// Exécute une ligne de commande (une seule commande, déjà séparée des autres).
    pub(crate) fn exec_line(&mut self, h: &mut dyn Host, ctx: &Ctx, text: &str) -> R<Flow> {
        let mut t = text.trim();
        while let Some(r) = t.strip_prefix('/') {
            t = r;
        }
        let mut quiet = false;
        let mut bypass = false;
        loop {
            if let Some(r) = t.strip_prefix('.') {
                quiet = true;
                t = r;
            } else if let Some(r) = t.strip_prefix('!') {
                bypass = true;
                t = r;
            } else {
                break;
            }
        }
        if t.is_empty() {
            return Ok(Flow::Normal);
        }
        let no_eval = self.frame().no_eval;
        let (name0, rest0) = split_word(t);
        let (name, rest): (String, String) = if !no_eval && (name0.starts_with('$') || name0.starts_with('%')) {
            // Le nom de la commande peut venir d'un identifiant (`$iif(%x,msg,notice) $nick …`).
            let ev = self.eval_text(h, ctx, name0)?;
            let ev = ev.trim().trim_start_matches('/').to_string();
            let (n, r) = split_word(&ev);
            let rest = if r.is_empty() { rest0.to_string() } else { format!("{r} {rest0}") };
            (n.to_string(), rest)
        } else {
            (name0.to_string(), rest0.to_string())
        };
        if name.is_empty() {
            return Ok(Flow::Normal);
        }
        let lname = name.to_ascii_lowercase();
        match lname.as_str() {
            "return" | "returnex" => {
                let v = if no_eval { rest.clone() } else { self.eval_text(h, ctx, &rest)? };
                return Ok(Flow::Return(Some(v.trim_matches(' ').to_string())));
            }
            "halt" => return Err(Abort::Halt),
            "haltdef" => {
                self.rt.haltdef = true;
                return Ok(Flow::Normal);
            }
            "break" => return Ok(Flow::Break),
            "continue" => return Ok(Flow::Continue),
            "goto" => {
                let l = if no_eval { rest.clone() } else { self.eval_text(h, ctx, &rest)? };
                return Ok(Flow::Goto(l.trim().to_ascii_lowercase()));
            }
            "var" => {
                self.cmd_var(h, ctx, &rest, quiet)?;
                return Ok(Flow::Normal);
            }
            _ => {}
        }
        // Alias (sauf `!` et sauf l'alias en cours qui appelle la commande du même nom).
        if !bypass && self.frame().alias.as_deref() != Some(lname.as_str())
            && let Some((alias, script)) = self.find_alias(&lname)
        {
            let args = if no_eval { rest.clone() } else { self.eval_text(h, ctx, &rest)? };
            let v = self.call_alias(h, ctx, &alias, script, words(&args), false, "")?;
            self.rt.result = v.unwrap_or_default();
            return Ok(Flow::Normal);
        }
        // Commandes dont le premier argument (nom de variable) n'est pas évalué.
        if matches!(lname.as_str(), "set" | "unset" | "inc" | "dec" | "unsetall") {
            self.cmd_setlike(h, ctx, &lname, &rest, quiet)?;
            return Ok(Flow::Normal);
        }
        // `$*` : la commande est répétée pour chaque jeton de `$1-`.
        if !no_eval && rest.split(' ').any(|w| w == "$*") {
            let toks = self.frame().args.clone();
            for tk in toks {
                self.rt.star = Some(tk);
                let a = self.eval_text(h, ctx, &rest);
                let a = match a {
                    Ok(a) => a,
                    Err(e) => {
                        self.rt.star = None;
                        return Err(e);
                    }
                };
                let r = self.dispatch(h, ctx, &lname, a.trim_matches(' '), quiet);
                if r.is_err() {
                    self.rt.star = None;
                    return r;
                }
            }
            self.rt.star = None;
            return Ok(Flow::Normal);
        }
        let args = if no_eval { rest } else { self.eval_text(h, ctx, &rest)? };
        self.dispatch(h, ctx, &lname, args.trim_matches(' '), quiet)
    }

    /// Commandes du moteur, puis celles de l'hôte. `args` est déjà évalué.
    pub(crate) fn dispatch(&mut self, h: &mut dyn Host, ctx: &Ctx, name: &str, args: &str, quiet: bool) -> R<Flow> {
        match name {
            "timers" => self.cmd_timers(h, ctx, args, quiet)?,
            n if n.starts_with("timer") => self.cmd_timer(h, ctx, &n[5..], args, quiet)?,
            "hmake" => self.cmd_hmake(h, ctx, args, quiet)?,
            "hadd" => self.cmd_hadd(h, ctx, args)?,
            "hdel" => self.cmd_hdel(args)?,
            "hfree" => self.cmd_hfree(args)?,
            "hinc" => self.cmd_hinc(h, args, 1.0)?,
            "hdec" => self.cmd_hinc(h, args, -1.0)?,
            "hload" => self.cmd_hload(args)?,
            "hsave" => self.cmd_hsave(args)?,
            "tokenize" => {
                let (c, text) = split_word(args);
                let Some(c) = crate::strings::sep_char(c) else {
                    return Err(self.err("/tokenize: invalid parameters"));
                };
                let t: Vec<String> = crate::strings::toks(text, c).into_iter().map(|x| x.to_string()).collect();
                self.frame_mut().args = t;
            }
            "noop" => {}
            "write" => self.cmd_write(args)?,
            "writeini" => self.cmd_writeini(args)?,
            "remini" => self.cmd_remini(args)?,
            "flushini" => {}
            "remove" => self.cmd_remove(args)?,
            "rename" => self.cmd_rename(args)?,
            "copy" => self.cmd_copy(args)?,
            "mkdir" => self.cmd_mkdir(args)?,
            "rmdir" => self.cmd_rmdir(args)?,
            "load" => self.cmd_load(h, ctx, args)?,
            "unload" => self.cmd_unload(h, ctx, args)?,
            "reload" => self.cmd_reload(args)?,
            "alias" => self.cmd_alias(args)?,
            "dialog" => self.cmd_dialog(h, ctx, args)?,
            "did" => self.cmd_did(args)?,
            "signal" => self.cmd_signal(h, ctx, args)?,
            "scid" | "scon" => return self.cmd_scid(h, ctx, name == "scon", args),
            "enable" | "disable" => self.cmd_enable(args, name == "enable"),
            "auser" | "ruser" | "guser" | "rlevel" | "flush" => self.cmd_users(h, ctx, name, args)?,
            "run" | "dll" | "comopen" | "comclose" | "comreg" | "sockopen" | "socklisten" | "sockudp" => {
                return Err(self.err(format!("/{name}: not allowed by the script sandbox")));
            }
            _ => {
                let c = self.eff_ctx(ctx).into_owned();
                let r = if quiet { h.command_quiet(&c, name, args) } else { h.command(&c, name, args) };
                if let Err(e) = r {
                    return Err(self.err(e));
                }
            }
        }
        Ok(Flow::Normal)
    }

    /// Écho d'information du moteur (`* Timer 1 activated`…), dans la fenêtre active.
    pub(crate) fn info(&mut self, h: &mut dyn Host, ctx: &Ctx, text: &str) {
        let c = self.eff_ctx(ctx).into_owned();
        let _ = h.command(&c, "echo", &format!("-a {text}"));
    }

    /// `/scid N [commande]`, `/scid -a commande`, `/scid -r`, `/scon N|-a commande`.
    fn cmd_scid(&mut self, h: &mut dyn Host, ctx: &Ctx, scon: bool, args: &str) -> R<Flow> {
        let (sw, rest) = switches(args);
        if sw.contains('r') {
            self.frame_mut().conn = None;
            return Ok(Flow::Normal);
        }
        let cids: Vec<Option<u32>> = if sw.contains('a') {
            let n: usize = h.identifier(ctx, "scon", &["0".to_string()], None).and_then(|v| v.parse().ok()).unwrap_or(0);
            (1..=n)
                .filter_map(|i| h.identifier(ctx, "scon", &[i.to_string()], None).and_then(|v| v.parse().ok()))
                .map(Some)
                .collect()
        } else {
            let (n, _) = split_word(rest);
            let cid = if scon {
                h.identifier(ctx, "scon", &[n.to_string()], None).and_then(|v| v.parse().ok())
            } else {
                n.parse().ok()
            };
            vec![cid]
        };
        let cmd = if sw.contains('a') { rest.to_string() } else { split_word(rest).1.to_string() };
        if cmd.is_empty() {
            if let Some(c) = cids.first() {
                self.frame_mut().conn = Some(*c);
            }
            return Ok(Flow::Normal);
        }
        let block = parse_body(&cmd, self.frame().line);
        for cid in cids {
            let prev = self.frame().conn;
            self.frame_mut().conn = Some(cid);
            let r = self.exec_block(h, ctx, &block);
            self.frame_mut().conn = prev;
            match r? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    /// Exécute un bloc de premier niveau et ramène le résultat à une erreur affichable.
    pub(crate) fn run_top(&mut self, h: &mut dyn Host, ctx: &Ctx, frame: Frame, body: &Block, typed: bool) -> Result<(), ScriptError> {
        let r = self.top(h, |e, h| {
            if typed && e.rt.frames.is_empty() {
                e.rt.typed = true;
            }
            e.run_routine(h, ctx, frame, body)
        });
        match r {
            Ok(_) => Ok(()),
            Err(e) => match e.into_script_error() {
                None => Ok(()),
                Some(se) => Err(se),
            },
        }
    }

    /// `$1-` par défaut d'une ligne lancée par le client : pseudos choisis, ou texte de l'événement.
    pub(crate) fn default_args(ctx: &Ctx) -> Vec<String> {
        if !ctx.selected.is_empty() {
            return ctx.selected.clone();
        }
        match &ctx.event {
            Some(ev) => crate::events::event_args(ev),
            None => Vec::new(),
        }
    }

    /// Ligne tapée par l'utilisateur ou venue d'un popup : `/cmd` (rien n'est évalué), `//cmd` (évalué), plusieurs
    /// commandes séparées par ` | `, blocs `{ }` et `if` compris. Sans `/`, la ligne est traitée comme du code de
    /// script (évaluée) : c'est au client de décider d'envoyer le texte simple sur IRC.
    pub fn run_line(&mut self, host: &mut dyn Host, ctx: &Ctx, line: &str) -> Result<(), ScriptError> {
        self.flush_pending(host);
        let t = line.trim_start();
        let (eval, body) = if let Some(r) = t.strip_prefix("//") {
            (true, r)
        } else if let Some(r) = t.strip_prefix('/') {
            (false, r)
        } else {
            (true, t)
        };
        let block = parse_body(body, 0);
        let frame = Frame { args: Self::default_args(ctx), no_eval: !eval, ..Frame::default() };
        self.run_top(host, ctx, frame, &block, true)
    }

    /// Exécute une commande par son nom, arguments déjà prêts (non évalués) : alias d'abord (sauf si le nom
    /// commence par `!`), puis commandes du moteur (`set`, `timer`, `hadd`, `if`…), puis [`Host::command`].
    pub fn command(&mut self, host: &mut dyn Host, ctx: &Ctx, name: &str, args: &str) -> Result<(), ScriptError> {
        self.flush_pending(host);
        let n = name.trim().trim_start_matches('/');
        let line = if args.is_empty() { n.to_string() } else { format!("{n} {args}") };
        let lname = n.trim_start_matches(['.', '!']).to_ascii_lowercase();
        let block = if matches!(lname.as_str(), "if" | "while") {
            parse_body(&line, 0)
        } else {
            vec![Stmt::Cmd { line: 0, text: line }]
        };
        let frame = Frame { args: Self::default_args(ctx), no_eval: true, ..Frame::default() };
        self.run_top(host, ctx, frame, &block, true)
    }

    /// Évalue un texte (identifiants, variables, `[ ]`, `$+`). En cas d'erreur, elle est signalée à l'hôte et le
    /// résultat est vide.
    pub fn evaluate(&mut self, host: &mut dyn Host, ctx: &Ctx, text: &str) -> String {
        self.flush_pending(host);
        let frame = Frame { args: Self::default_args(ctx), ..Frame::default() };
        let r = self.top(host, |e, h| {
            e.rt.frames.push(frame);
            let r = e.eval_text(h, ctx, text);
            e.rt.frames.pop();
            r
        });
        match r {
            Ok(v) => v,
            Err(e) => {
                if let Some(m) = e.message() {
                    host.script_error(ctx, &m);
                }
                String::new()
            }
        }
    }
}

/// Position d'une étiquette `:nom` dans un bloc.
fn find_label(block: &[Stmt], label: &str) -> Option<usize> {
    block.iter().position(|s| matches!(s, Stmt::Label { name, .. } if name == label))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sw() {
        assert_eq!(switches("-u10 %x 5"), ("u10".to_string(), "%x 5"));
        assert_eq!(switches("-s -n %x"), ("sn".to_string(), "%x"));
        assert_eq!(switches("-5 x"), (String::new(), "-5 x"));
        assert_eq!(switch_num("u10s", 'u'), Some(10.0));
    }
}
