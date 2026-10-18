//! Scripts chargés : chargement (fichier ou texte), remplacement, déchargement, groupes `#nom on|off`,
//! commandes `/load`, `/unload`, `/reload`, `/alias`, `/enable`, `/disable`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::ast::parse_body;
use crate::exec::{R, split_word, switches};
use crate::files::{decode, take_filename};
use crate::source::{self, Alias, Parsed};
use crate::wild::wildmatch;
use crate::{Ctx, Engine, EventInfo, EventKind, Host, LoadedScript, ScriptError, ScriptKind};

/// Un script chargé, avec son texte et ce qu'on en a lu.
#[derive(Debug)]
pub struct Script {
    pub name: String,
    pub path: Option<PathBuf>,
    pub kind: ScriptKind,
    pub text: String,
    pub p: Parsed,
}

impl Script {
    /// Le groupe (s'il y en a un) est-il actif ?
    pub fn group_on(&self, g: &Option<String>) -> bool {
        match g {
            None => true,
            Some(n) => self.p.groups.iter().find(|(x, _)| x == n).map(|(_, on)| *on).unwrap_or(true),
        }
    }

    fn has_event(&self, k: EventKind) -> bool {
        self.p.handlers.iter().any(|h| h.kind == Some(k))
    }
}

/// Sorte d'un fichier d'après son contenu : `.ini` de mIRC selon ses sections, remote sinon.
fn detect_kind(text: &str) -> ScriptKind {
    if source::is_ini(text) {
        let s = source::ini_sections(text);
        if s.script.is_some() {
            return ScriptKind::Remote;
        }
        if s.aliases.is_some() {
            return ScriptKind::Aliases;
        }
        if !s.popups.is_empty() {
            return ScriptKind::Popups;
        }
    }
    ScriptKind::Remote
}

impl Engine {
    /// Reconstruit l'index des alias après un changement de la liste des scripts.
    pub(crate) fn reindex(&mut self) {
        self.alias_index.clear();
        for (i, s) in self.scripts.iter().enumerate() {
            for a in &s.p.aliases {
                let v = self.alias_index.entry(a.name.clone()).or_default();
                if v.last() != Some(&i) {
                    v.push(i);
                }
            }
        }
    }

    fn install(&mut self, name: &str, path: Option<PathBuf>, kind: ScriptKind, text: &str, events: bool) -> Result<(), ScriptError> {
        let parsed = source::parse_any(text, kind, name)?;
        let script = Script { name: name.to_string(), path: path.clone(), kind, text: text.to_string(), p: parsed };
        let startup = events && (script.has_event(EventKind::Load) || script.has_event(EventKind::Start));
        let ls = LoadedScript { name: name.to_string(), path, kind };
        match self.scripts.iter().position(|s| s.name == name) {
            Some(i) => {
                self.scripts[i] = script;
                self.loaded[i] = ls;
            }
            None => {
                self.scripts.push(script);
                self.loaded.push(ls);
                if startup {
                    self.pending.push(crate::events::Pending::Events {
                        script: name.to_string(),
                        kinds: vec![EventKind::Load, EventKind::Start],
                    });
                }
            }
        }
        self.reindex();
        Ok(())
    }

    /// Charge (ou remplace, s'il porte le même nom) un script donné en texte. Au premier chargement d'un remote,
    /// ses `on LOAD` puis `on START` sont mis en attente : ils s'exécutent au prochain appel qui a un hôte
    /// (ou à [`Engine::flush_pending`]).
    pub fn load_text(&mut self, name: &str, kind: ScriptKind, text: &str) -> Result<(), ScriptError> {
        self.install(name, None, kind, text, true)
    }

    /// Comme [`Engine::load_text`], sans `on LOAD`/`on START` : pour rétablir les scripts au démarrage du
    /// client (qui tire lui-même `on START`).
    pub fn restore_text(&mut self, name: &str, kind: ScriptKind, text: &str) -> Result<(), ScriptError> {
        self.install(name, None, kind, text, false)
    }

    /// Nom d'un script chargé depuis un fichier : chemin relatif au dossier des scripts, ou chemin complet.
    fn name_for(&self, path: &Path) -> String {
        let root = crate::files::normalize(self.root());
        match crate::files::normalize(path).strip_prefix(&root) {
            Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
            Err(_) => path.display().to_string(),
        }
    }

    fn read_script_file(&self, path: &Path, trusted: bool) -> Result<(PathBuf, String), ScriptError> {
        let real = if trusted {
            if path.is_absolute() { path.to_path_buf() } else { self.root().join(path) }
        } else {
            match self.sandbox_path(&path.to_string_lossy()) {
                Ok(p) => p,
                Err(_) => return Err(ScriptError::Sandbox(path.display().to_string())),
            }
        };
        let bytes = std::fs::read(&real).map_err(|_| ScriptError::Message(format!("* /load: unable to open '{}'", path.display())))?;
        Ok((real, decode(&bytes)))
    }

    /// Charge un fichier de script (`.mrc`, `.txt`, `.ini` de mIRC…). Sans `trusted`, le chemin doit être dans le
    /// dossier des scripts ; `trusted` : chemin choisi par l'utilisateur dans le client. Rend le nom du script.
    pub fn load_file(&mut self, path: &Path, trusted: bool) -> Result<String, ScriptError> {
        let (real, text) = self.read_script_file(path, trusted)?;
        let name = self.name_for(&real);
        self.install(&name, Some(real), detect_kind(&text), &text, true)?;
        Ok(name)
    }

    /// Comme [`Engine::load_file`], sans `on LOAD`/`on START`.
    pub fn restore_file(&mut self, path: &Path, trusted: bool) -> Result<String, ScriptError> {
        let (real, text) = self.read_script_file(path, trusted)?;
        let name = self.name_for(&real);
        self.install(&name, Some(real), detect_kind(&text), &text, false)?;
        Ok(name)
    }

    /// Comme [`Engine::restore_file`], avec la sorte de fichier retenue (`/load -a`, `/load -p…`) au lieu de la
    /// sorte devinée d'après le texte.
    pub fn restore_file_as(&mut self, path: &Path, trusted: bool, kind: ScriptKind) -> Result<String, ScriptError> {
        let (real, text) = self.read_script_file(path, trusted)?;
        let name = self.name_for(&real);
        self.install(&name, Some(real), kind, &text, false)?;
        Ok(name)
    }

    /// Vrai si un script chargé a un gestionnaire de cet événement (le client s'épargne les autres, comme
    /// `on KEYDOWN` à chaque touche).
    pub fn handles(&self, kind: EventKind) -> bool {
        self.scripts.iter().any(|s| s.has_event(kind))
    }

    /// Noms des alias visibles de partout (scripts chargés et `/alias`), triés, pour la complétion par Tab.
    pub fn alias_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.dyn_aliases.iter().filter(|a| !a.local).map(|a| a.name.clone()).collect();
        for s in &self.scripts {
            for a in &s.p.aliases {
                if !a.local && s.group_on(&a.group) {
                    v.push(a.name.clone());
                }
            }
        }
        v.sort();
        v.dedup();
        v
    }

    /// Retire un script sans `on UNLOAD` ni rien exécuter (chargement refusé par l'hôte).
    fn forget(&mut self, name: &str) {
        if let Some(i) = self.scripts.iter().position(|s| s.name == name) {
            self.scripts.remove(i);
            self.loaded.remove(i);
            self.reindex();
        }
        self.pending.retain(|p| !matches!(p, crate::events::Pending::Events { script, .. } if script == name));
    }

    /// Décharge un script ; son `on UNLOAD` est mis en attente (voir [`Engine::flush_pending`]).
    pub fn unload(&mut self, name: &str) -> bool {
        let Some(i) = self.find_script(name) else { return false };
        let s = self.scripts.remove(i);
        self.loaded.remove(i);
        self.reindex();
        if let Some(h) = s.p.handlers.iter().find(|h| h.kind == Some(EventKind::Unload) && s.group_on(&h.group)) {
            self.pending.push(crate::events::Pending::Run {
                script: s.name.clone(),
                body: h.body.clone(),
                event: EventInfo { kind: Some(EventKind::Unload), ..EventInfo::default() },
            });
        }
        true
    }

    fn find_script(&self, name: &str) -> Option<usize> {
        let n = name.replace('\\', "/");
        self.scripts.iter().position(|s| s.name.eq_ignore_ascii_case(&n)).or_else(|| {
            self.scripts.iter().position(|s| {
                s.path.as_ref().is_some_and(|p| p.to_string_lossy().replace('\\', "/").ends_with(&n))
                    || s.name.rsplit('/').next().is_some_and(|b| b.eq_ignore_ascii_case(&n))
            })
        })
    }

    /// Scripts chargés, dans l'ordre de chargement.
    pub fn scripts(&self) -> &[LoadedScript] {
        &self.loaded
    }

    /// Texte source d'un script (pour l'éditeur) ; l'état des groupes changé par `/enable` y est reporté.
    pub fn script_text(&self, name: &str) -> Option<&str> {
        self.find_script(name).map(|i| self.scripts[i].text.as_str())
    }

    /// Vrai si un alias de ce nom est visible de partout (alias `-l` exclus).
    pub fn has_alias(&self, name: &str) -> bool {
        let n = name.trim_start_matches('/').to_ascii_lowercase();
        self.dyn_aliases.iter().any(|a| a.name == n)
            || self.alias_index.get(&n).is_some_and(|idx| {
                idx.iter().any(|&i| {
                    let s = &self.scripts[i];
                    s.p.aliases.iter().any(|a| a.name == n && !a.local && s.group_on(&a.group))
                })
            })
    }

    /// Vrai si le script s'exécute dès son chargement ou au démarrage (`on LOAD`, `on START`) : le client avertit
    /// l'utilisateur (ADR 0007) avant [`Engine::flush_pending`], ou annule avec [`Engine::discard_pending`].
    pub fn runs_at_startup(&self, name: &str) -> bool {
        self.find_script(name).is_some_and(|i| {
            let s = &self.scripts[i];
            s.has_event(EventKind::Load) || s.has_event(EventKind::Start)
        })
    }

    /// `/load -rs|-a|-pc|-pq|-pn|-ps|-pm|-ru|-rv fichier` (dans le dossier des scripts).
    pub(crate) fn cmd_load(&mut self, h: &mut dyn Host, _ctx: &Ctx, args: &str) -> R<()> {
        let (sw, rest) = switches(args);
        let (file, _) = take_filename(rest);
        if file.is_empty() {
            return Err(self.err("/load: insufficient parameters"));
        }
        let path = self.sandbox_path(&file)?;
        let bytes = std::fs::read(&path).map_err(|_| self.err(format!("/load: unable to open '{file}'")))?;
        let mut text = decode(&bytes);
        let name = self.name_for(&path);
        let r = if sw.contains('r') && sw.contains('u') {
            self.merge_users_text(&text);
            Ok(())
        } else if sw.contains('r') && sw.contains('v') {
            self.merge_vars_text(&text);
            Ok(())
        } else if sw.contains('a') {
            self.install(&name, Some(path), ScriptKind::Aliases, &text, false)
        } else if sw.contains('p') {
            let target = if sw.contains('c') {
                "channel"
            } else if sw.contains('q') {
                "query"
            } else if sw.contains('n') {
                "nicklist"
            } else if sw.contains('m') {
                "menubar"
            } else {
                "status"
            };
            if !source::is_ini(&text) && !text.trim_start().starts_with('[') {
                text = format!("[{target}]\n{text}");
            }
            self.install(&name, Some(path), ScriptKind::Popups, &text, false)
        } else {
            let k = detect_kind(&text);
            let fresh = self.find_script(&name).is_none();
            let r = self.install(&name, Some(path.clone()), k, &text, true);
            // Nouveau script qui s'exécute dès son chargement : l'hôte peut demander d'abord (ADR 0007).
            if r.is_ok() && fresh && self.runs_at_startup(&name) && !h.confirm_load(&name, &path) {
                self.forget(&name);
                return Ok(());
            }
            r
        };
        r.map_err(|e| self.err(e.to_string()))?;
        self.flush_pending_nested(h);
        Ok(())
    }

    /// Les `on LOAD` d'un `/load` tapé ou lancé par un script s'exécutent tout de suite.
    fn flush_pending_nested(&mut self, h: &mut dyn Host) {
        let list = std::mem::take(&mut self.pending);
        for p in list {
            if let crate::events::Pending::Events { script, kinds } = p {
                for k in kinds {
                    let ev = EventInfo { kind: Some(k), ..EventInfo::default() };
                    let ctx = Ctx { event: Some(ev.clone()), ..Ctx::default() };
                    self.fire_scripts(h, &ctx, &ev, Some(&script));
                }
            } else {
                self.pending.push(p);
            }
        }
    }

    /// `/unload -rs|-a fichier`.
    pub(crate) fn cmd_unload(&mut self, h: &mut dyn Host, _ctx: &Ctx, args: &str) -> R<()> {
        let (_, rest) = switches(args);
        let (file, _) = take_filename(rest);
        if !self.unload(&file) {
            return Err(self.err(format!("/unload: no such script '{file}'")));
        }
        // `on UNLOAD` tout de suite (l'hôte est là).
        let list = std::mem::take(&mut self.pending);
        for p in list {
            if let crate::events::Pending::Run { script, body, event } = p {
                let ctx = Ctx { event: Some(event), ..Ctx::default() };
                let frame = crate::exec::Frame { script: Some(script), ..crate::exec::Frame::default() };
                if let Err(e) = self.run_routine(h, &ctx, frame, &body)
                    && let Some(m) = e.message()
                {
                    h.script_error(&ctx, &m);
                }
            } else {
                self.pending.push(p);
            }
        }
        Ok(())
    }

    /// `/reload -rs fichier` : relit le fichier sans `on LOAD`/`on START`.
    pub(crate) fn cmd_reload(&mut self, args: &str) -> R<()> {
        let (_, rest) = switches(args);
        let (file, _) = take_filename(rest);
        let path = match self.find_script(&file).and_then(|i| self.scripts[i].path.clone()) {
            Some(p) => p,
            None => self.sandbox_path(&file)?,
        };
        let bytes = std::fs::read(&path).map_err(|_| self.err(format!("/reload: unable to open '{file}'")))?;
        let text = decode(&bytes);
        let name = self.name_for(&path);
        let kind = self.find_script(&name).map(|i| self.scripts[i].kind).unwrap_or_else(|| detect_kind(&text));
        self.install(&name, Some(path), kind, &text, false).map_err(|e| self.err(e.to_string()))
    }

    /// `/alias [-l] nom commandes` (sans commandes : retire l'alias).
    pub(crate) fn cmd_alias(&mut self, args: &str) -> R<()> {
        let (sw, rest) = switches(args);
        let (name, cmds) = split_word(rest);
        let name = name.trim_start_matches('/').to_ascii_lowercase();
        if name.is_empty() {
            return Err(self.err("/alias: insufficient parameters"));
        }
        self.dyn_aliases.retain(|a| a.name != name);
        if !cmds.trim().is_empty() {
            let body = parse_body(cmds, 0);
            self.dyn_aliases.push(Arc::new(Alias { name, local: sw.contains('l'), body: Arc::new(body), group: None, line: 0 }));
        }
        Ok(())
    }

    /// `/enable #groupe…` et `/disable #groupe…` (jokers permis). L'état est reporté dans le texte du script.
    pub(crate) fn cmd_enable(&mut self, args: &str, on: bool) {
        for g in args.split_whitespace() {
            let pat = g.to_ascii_lowercase();
            for s in self.scripts.iter_mut() {
                let mut changed = Vec::new();
                for (name, state) in s.p.groups.iter_mut() {
                    if wildmatch(&pat, name, false) && *state != on {
                        *state = on;
                        changed.push(name.clone());
                    }
                }
                for name in changed {
                    s.text = rewrite_group_line(&s.text, &name, on);
                }
            }
        }
    }
}

/// Remplace `#nom on|off` par l'état voulu dans le texte source (format `.ini` compris).
fn rewrite_group_line(text: &str, name: &str, on: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let cr = line.ends_with('\r');
        let l = line.trim_end_matches('\r');
        let (prefix, body) = match l.find('=') {
            Some(k) if l.starts_with('n') && l[1..k].chars().all(|c| c.is_ascii_digit()) && k > 1 => (&l[..=k], &l[k + 1..]),
            _ => ("", l),
        };
        let w: Vec<&str> = body.split_whitespace().collect();
        if w.len() == 2 && w[0].eq_ignore_ascii_case(name) && (w[1].eq_ignore_ascii_case("on") || w[1].eq_ignore_ascii_case("off")) {
            let indent = &body[..body.len() - body.trim_start().len()];
            out.push_str(prefix);
            out.push_str(indent);
            out.push_str(w[0]);
            out.push(' ');
            out.push_str(if on { "on" } else { "off" });
        } else {
            out.push_str(l);
        }
        if cr {
            out.push('\r');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_rewrite() {
        assert_eq!(rewrite_group_line("a\n#g off\nb", "#g", true), "a\n#g on\nb");
        assert_eq!(rewrite_group_line("n1=#g on", "#g", false), "n1=#g off");
    }
}
