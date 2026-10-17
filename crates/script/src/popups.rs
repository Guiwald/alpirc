//! Popups : menus des fenêtres (`menu status,channel { … }`, fichiers de popups, sections `.ini`), construits à
//! chaque ouverture avec leurs libellés évalués : `$iif(…)` (un libellé vide cache l'entrée et son sous-menu),
//! `$style(1)` coché, `$style(2)` grisé, `$style(3)` les deux, sous-menus par les points en tête (`.Sous`, `..Sous`),
//! séparateurs `-`, et sous-menus dynamiques `$submenu($alias($1))` (alias appelé avec `begin`, 1, 2… jusqu'à un
//! résultat vide, puis `end`).
//!
//! Dans les menus des fenêtres `@`, les entrées d'événements de souris (`sclick`, `dclick`, `mouse`…) ne sont pas
//! montrées.

use std::sync::Arc;

use crate::ast::{Block, matching_paren};
use crate::exec::Frame;
use crate::source::{PopupLine, parse_popup_lines};
use crate::wild::wildmatch;
use crate::{Ctx, Engine, Host, MenuItem, PopupKind, ScriptError};

/// Une entrée cliquable du dernier menu construit.
#[derive(Debug, Clone)]
pub struct Action {
    pub script: Option<String>,
    pub body: Arc<Block>,
    pub menu: String,
}

const MOUSE_ITEMS: &[&str] = &["sclick", "dclick", "lbclick", "rclick", "uclick", "mouse", "leave", "drop"];

fn kind_name(kind: PopupKind) -> &'static str {
    match kind {
        PopupKind::Status => "status",
        PopupKind::Channel => "channel",
        PopupKind::Query => "query",
        PopupKind::Nicklist => "nicklist",
        PopupKind::Menubar => "menubar",
        PopupKind::Custom => "@",
    }
}

fn target_matches(t: &str, kind: PopupKind, window: &str) -> bool {
    match kind {
        PopupKind::Custom => t == "*" || (t.starts_with('@') && wildmatch(t, window, false)),
        PopupKind::Menubar => t == "menubar",
        k => t == "*" || t == kind_name(k),
    }
}

/// Insère une entrée à la profondeur voulue ; `path[d]` : indice de la dernière entrée de profondeur `d`.
fn insert(root: &mut Vec<MenuItem>, path: &mut Vec<usize>, depth: usize, item: MenuItem) {
    let d = depth.min(path.len());
    let mut cur = root;
    for &k in &path[..d] {
        cur = &mut cur[k].children;
    }
    cur.push(item);
    path.truncate(d);
    path.push(cur.len() - 1);
}

/// Les entrées qui ont un sous-menu n'ont pas d'action.
fn fix_actions(items: &mut [MenuItem]) {
    for it in items {
        if !it.children.is_empty() {
            it.action = None;
            fix_actions(&mut it.children);
        }
    }
}

impl Engine {
    /// Construit le menu d'une sorte de fenêtre (`custom_window` : nom de la fenêtre `@` pour
    /// [`PopupKind::Custom`]). `ctx.selected` donne `$1-` (pseudos choisis dans la liste) et `$snicks`.
    pub fn popup(&mut self, host: &mut dyn Host, ctx: &Ctx, kind: PopupKind, custom_window: Option<&str>) -> Vec<MenuItem> {
        self.flush_pending(host);
        self.popup_actions.clear();
        let window = custom_window.unwrap_or("");
        let menu_name = if kind == PopupKind::Custom { window.to_string() } else { kind_name(kind).to_string() };
        let sources: Vec<(String, Arc<Vec<PopupLine>>)> = self
            .scripts
            .iter()
            .flat_map(|s| {
                s.p.menus
                    .iter()
                    .filter(|m| s.group_on(&m.group) && m.targets.iter().any(|t| target_matches(t, kind, window)))
                    .map(|m| (s.name.clone(), m.items.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut out = Vec::new();
        for (sname, lines) in sources {
            let mut built = self.build_menu(host, ctx, &sname, &lines, &menu_name, kind == PopupKind::Custom);
            out.append(&mut built);
        }
        fix_actions(&mut out);
        self.rt.menu.clear();
        out
    }

    fn build_menu(
        &mut self,
        h: &mut dyn Host,
        ctx: &Ctx,
        script: &str,
        lines: &[PopupLine],
        menu: &str,
        custom: bool,
    ) -> Vec<MenuItem> {
        let mut root = Vec::new();
        let mut path = Vec::new();
        let mut hidden: Option<usize> = None;
        for line in lines {
            if let Some(hd) = hidden {
                if line.depth > hd {
                    continue;
                }
                hidden = None;
            }
            if custom && line.depth == 0 && MOUSE_ITEMS.iter().any(|m| m.eq_ignore_ascii_case(&line.label)) {
                continue;
            }
            if line.label == "-" {
                insert(&mut root, &mut path, line.depth, MenuItem { label: "-".into(), enabled: true, checked: false, children: vec![], action: None });
                continue;
            }
            if line.label.get(..9).is_some_and(|p| p.eq_ignore_ascii_case("$submenu(")) {
                for (d, item) in self.dynamic_submenu(h, ctx, script, &line.label, menu) {
                    insert(&mut root, &mut path, line.depth + d, item);
                }
                continue;
            }
            match self.popup_item(h, ctx, script, &line.label, line.body.clone(), menu, line.line) {
                Some(item) => insert(&mut root, &mut path, line.depth, item),
                None => hidden = Some(line.depth),
            }
        }
        root
    }

    /// Évalue un libellé ; `None` s'il est vide (entrée cachée).
    #[allow(clippy::too_many_arguments)]
    fn popup_item(
        &mut self,
        h: &mut dyn Host,
        ctx: &Ctx,
        script: &str,
        label: &str,
        body: Option<Arc<Block>>,
        menu: &str,
        line: usize,
    ) -> Option<MenuItem> {
        self.rt.style = (false, false);
        self.rt.menu = menu.to_string();
        let frame = Frame { script: Some(script.to_string()), args: ctx.selected.clone(), line, ..Frame::default() };
        let r = self.top(h, |e, h| {
            e.rt.frames.push(frame);
            let r = e.eval_text(h, ctx, label);
            e.rt.frames.pop();
            r
        });
        let text = match r {
            Ok(t) => t,
            Err(e) => {
                if let Some(m) = e.message() {
                    h.script_error(ctx, &m);
                }
                return None;
            }
        };
        let text = text.trim().to_string();
        if text.is_empty() {
            return None;
        }
        let (checked, disabled) = self.rt.style;
        let action = body.map(|b| {
            self.popup_actions.push(Action { script: Some(script.to_string()), body: b, menu: menu.to_string() });
            (self.popup_actions.len() - 1) as u32
        });
        Some(MenuItem { label: text, enabled: !disabled, checked, children: vec![], action })
    }

    /// `$submenu($alias($1))` : entrées (profondeur relative, entrée) produites par l'alias.
    fn dynamic_submenu(&mut self, h: &mut dyn Host, ctx: &Ctx, script: &str, label: &str, menu: &str) -> Vec<(usize, MenuItem)> {
        let open = 8; // position de `(` dans `$submenu(`
        let Some(close) = matching_paren(&label[open..]) else { return vec![] };
        let inner = label[open + 1..open + close].to_string();
        let mut out = Vec::new();
        let mut keys: Vec<String> = vec!["begin".into()];
        keys.extend((1..=200).map(|i| i.to_string()));
        keys.push("end".into());
        for k in keys {
            let frame = Frame { script: Some(script.to_string()), args: vec![k.clone()], ..Frame::default() };
            let r = self.top(h, |e, h| {
                e.rt.frames.push(frame);
                let r = e.eval_text(h, ctx, &inner);
                e.rt.frames.pop();
                r
            });
            let v = r.unwrap_or_default();
            if v.trim().is_empty() {
                if k.chars().all(|c| c.is_ascii_digit()) {
                    // Fin de la liste : on passe à `end`.
                    let frame = Frame { script: Some(script.to_string()), args: vec!["end".into()], ..Frame::default() };
                    let r = self.top(h, |e, h| {
                        e.rt.frames.push(frame);
                        let r = e.eval_text(h, ctx, &inner);
                        e.rt.frames.pop();
                        r
                    });
                    if let Ok(v) = r
                        && !v.trim().is_empty()
                    {
                        out.extend(self.submenu_lines(h, ctx, script, &v, menu));
                    }
                    break;
                }
                continue;
            }
            out.extend(self.submenu_lines(h, ctx, script, &v, menu));
            if k == "end" {
                break;
            }
        }
        out
    }

    fn submenu_lines(&mut self, h: &mut dyn Host, ctx: &Ctx, script: &str, text: &str, menu: &str) -> Vec<(usize, MenuItem)> {
        let Ok(lines) = parse_popup_lines(text, 0, script) else { return vec![] };
        let mut out = Vec::new();
        for l in lines {
            if l.label == "-" {
                out.push((l.depth, MenuItem { label: "-".into(), enabled: true, checked: false, children: vec![], action: None }));
            } else if let Some(it) = self.popup_item(h, ctx, script, &l.label, l.body.clone(), menu, 0) {
                out.push((l.depth, it));
            }
        }
        out
    }

    /// Exécute l'entrée `action` du dernier menu construit par [`Engine::popup`]. `$1-` = `ctx.selected`.
    pub fn popup_run(&mut self, host: &mut dyn Host, ctx: &Ctx, action: u32) -> Result<(), ScriptError> {
        self.flush_pending(host);
        let Some(a) = self.popup_actions.get(action as usize).cloned() else {
            return Err(ScriptError::Message("* popup: no such menu item".into()));
        };
        self.rt.menu = a.menu.clone();
        let frame = Frame { script: a.script.clone(), args: Self::default_args(ctx), ..Frame::default() };
        let r = self.run_top(host, ctx, frame, &a.body, false);
        self.rt.menu.clear();
        r
    }
}
