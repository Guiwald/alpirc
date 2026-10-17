//! Pont entre les dialogues des scripts mSL (`/dialog -m nom table`) et les boîtes de la boîte à outils.
//!
//! Le moteur de scripts tient l'état des contrôles ; la boîte affichée le reflète. Quand l'utilisateur agit,
//! la boîte recopie l'état dans le moteur puis lui envoie l'événement (`on DIALOG`). Quand un script change
//! un contrôle (`/did`), la boîte se remet à jour.

use alpirc_core::i18n::Lang;
use alpirc_script::{ControlKind as SK, DialogEvent};
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogUnits, DialogView};

use super::{Cx, DialogCtl, Outcome};

pub struct ScriptDialog {
    pub name: String,
    synced_version: u64,
    spec: DialogSpec,
}

fn kind(k: SK) -> ControlKind {
    match k {
        SK::Text => ControlKind::Text,
        SK::Edit => ControlKind::Edit,
        SK::Button => ControlKind::Button,
        SK::Check => ControlKind::Check,
        SK::Radio => ControlKind::Radio,
        SK::Box => ControlKind::Box,
        SK::List => ControlKind::List,
        SK::Combo => ControlKind::Combo,
        SK::Icon => ControlKind::Icon,
        SK::Link => ControlKind::Link,
        SK::Tab => ControlKind::Tab,
        SK::Scroll => ControlKind::Scroll,
    }
}

impl ScriptDialog {
    /// Crée la boîte d'un dialogue ouvert dans le moteur ; `None` s'il n'existe pas.
    pub fn new(name: &str, engine: &alpirc_script::Engine) -> Option<ScriptDialog> {
        let d = engine.dialogs().iter().find(|d| d.name.eq_ignore_ascii_case(name))?;
        let controls = d
            .controls
            .iter()
            .map(|c| {
                let mut cs = ControlSpec::new(c.id, kind(c.kind), c.rect, &c.text);
                cs.items = c.items.clone();
                cs.styles = c.styles.clone();
                if !c.enabled && !cs.styles.iter().any(|s| s == "disable") {
                    cs.styles.push("disable".into());
                }
                if !c.visible && !cs.styles.iter().any(|s| s == "hide") {
                    cs.styles.push("hide".into());
                }
                cs.tab = c.tab;
                cs
            })
            .collect();
        let (w, h) = (d.rect.2.max(40), d.rect.3.max(30));
        let spec = DialogSpec {
            title: d.title.clone(),
            width: w,
            height: h,
            units: if d.dbu { DialogUnits::Dbu } else { DialogUnits::Px },
            controls,
        };
        Some(ScriptDialog { name: d.name.clone(), synced_version: engine.dialogs_version(), spec })
    }

    /// Recopie l'état du moteur dans la boîte.
    fn pull(&mut self, view: &mut DialogView, engine: &alpirc_script::Engine) {
        let Some(d) = engine.dialogs().iter().find(|d| d.name.eq_ignore_ascii_case(&self.name)) else { return };
        if view.title != d.title {
            view.title = d.title.clone();
        }
        for c in &d.controls {
            match c.kind {
                SK::List | SK::Combo => {
                    if view.items(c.id) != c.items {
                        view.set_items(c.id, &c.items);
                    }
                    let sel: Vec<usize> = c.selected.iter().filter(|&&n| n > 0).map(|n| n - 1).collect();
                    if view.selection(c.id) != sel {
                        view.set_selection(c.id, &sel);
                    }
                    if c.kind == SK::Combo && view.text(c.id).as_deref() != Some(c.text.as_str()) && c.has_style("edit") {
                        view.set_text(c.id, &c.text);
                    }
                }
                SK::Check | SK::Radio => {
                    if view.checked(c.id) != c.checked {
                        view.set_checked(c.id, c.checked);
                    }
                    if view.text(c.id).as_deref() != Some(c.text.as_str()) {
                        view.set_text(c.id, &c.text);
                    }
                }
                SK::Edit if c.has_style("multi") => {
                    let text = if c.items.is_empty() { c.text.clone() } else { c.items.join("\n") };
                    if view.text(c.id).as_deref() != Some(text.as_str()) {
                        view.set_text(c.id, &text);
                    }
                }
                _ => {
                    if view.text(c.id).as_deref() != Some(c.text.as_str()) {
                        view.set_text(c.id, &c.text);
                    }
                }
            }
            if view.enabled(c.id) != c.enabled {
                view.set_enabled(c.id, c.enabled);
            }
            if view.visible(c.id) != c.visible {
                view.set_visible(c.id, c.visible);
            }
        }
    }

    /// Recopie l'état d'un contrôle de la boîte dans le moteur.
    fn push(&self, id: u32, view: &DialogView, engine: &mut alpirc_script::Engine) {
        let Some(d) = engine.dialog_mut(&self.name) else { return };
        let Some(c) = d.controls.iter_mut().find(|c| c.id == id) else { return };
        match c.kind {
            SK::List | SK::Combo => {
                c.selected = view.selection(id).into_iter().map(|i| i + 1).collect();
                if c.kind == SK::Combo {
                    if let Some(t) = view.text(id) {
                        c.text = t;
                    }
                }
            }
            SK::Check | SK::Radio => {
                c.checked = view.checked(id);
                // Un bouton radio coché décoche les autres de son groupe : on relit tout le groupe.
                if c.kind == SK::Radio {
                    let ids: Vec<u32> = d.controls.iter().filter(|x| x.kind == SK::Radio).map(|x| x.id).collect();
                    for rid in ids {
                        if let Some(rc) = d.controls.iter_mut().find(|x| x.id == rid) {
                            rc.checked = view.checked(rid);
                        }
                    }
                }
            }
            SK::Edit => {
                let text = view.text(id).unwrap_or_default();
                if c.has_style("multi") {
                    c.items = text.lines().map(str::to_string).collect();
                }
                c.text = text;
            }
            SK::Scroll => {
                c.text = view.value(id).to_string();
            }
            _ => {}
        }
    }
}

impl DialogCtl for ScriptDialog {
    fn kind(&self) -> Option<String> {
        Some(format!("script:{}", self.name.to_lowercase()))
    }

    fn spec(&self, _lang: Lang) -> DialogSpec {
        self.spec.clone()
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        self.pull(view, cx.client.engine());
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        let now = cx.now;
        let ev = match msg {
            DialogMsg::Clicked(id) => {
                self.push(id, view, cx.client.engine_mut());
                DialogEvent::Click(id)
            }
            DialogMsg::Changed(id) => {
                self.push(id, view, cx.client.engine_mut());
                DialogEvent::Edit(id)
            }
            DialogMsg::SelectionChanged(id) => {
                self.push(id, view, cx.client.engine_mut());
                DialogEvent::Select(id)
            }
            DialogMsg::DoubleClicked(id) => {
                self.push(id, view, cx.client.engine_mut());
                DialogEvent::DoubleClick(id)
            }
            DialogMsg::Ok(id) => {
                // Les champs n'envoient pas toujours « Changed » : on recopie tout avant de valider.
                let ids: Vec<u32> = cx.client.engine().dialogs().iter().find(|d| d.name == self.name).map(|d| d.controls.iter().map(|c| c.id).collect()).unwrap_or_default();
                for cid in ids {
                    self.push(cid, view, cx.client.engine_mut());
                }
                DialogEvent::Ok(id)
            }
            DialogMsg::Cancel => DialogEvent::Close,
        };
        cx.client.dialog_event(&self.name, ev, now);
        let still_open = cx.client.engine().dialogs().iter().any(|d| d.name.eq_ignore_ascii_case(&self.name));
        if still_open { Outcome::Keep } else { Outcome::Close }
    }

    fn refresh(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let v = cx.client.engine().dialogs_version();
        if v != self.synced_version {
            self.synced_version = v;
            self.pull(view, cx.client.engine());
        }
    }

    fn script_name(&self) -> Option<&str> {
        Some(&self.name)
    }
}
