//! Les boîtes de dialogue de l'application : un contrôleur par boîte, qui décrit ses contrôles
//! ([`DialogSpec`]), les remplit depuis le moteur et réagit aux messages de la boîte.
//!
//! Les grandes boîtes (Options, carnet d'adresses, dossier des canaux, Channel Central, éditeur de scripts,
//! couleurs) vivent chacune dans leur module ; ici, le cadre commun et les petites boîtes (message, collage
//! de plusieurs lignes, rejoindre un canal, rechercher, à propos, quitter) et le pont avec les dialogues des
//! scripts mSL.

pub mod script;

/// Textes des boîtes de dialogue (voir [`crate::tr`]).
pub const TR: crate::tr::Table = &[];

/// Construit la boîte de l'interface demandée, si elle existe déjà dans cette version.
pub fn build(d: UiDialog, client: &Client) -> Option<Box<dyn DialogCtl>> {
    let _ = (d, client);
    None
}

use alpirc_core::i18n::Lang;
use alpirc_core::{Client, UiDialog, WinId};
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogUnits, DialogView};

use crate::tr::{plain, t, tf};

/// Que faire de la boîte après un message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Keep,
    Close,
}

/// Ce qu'une boîte demande à l'application.
pub enum Action {
    /// Ouvrir une autre boîte.
    Open(Box<dyn DialogCtl>),
    /// Ouvrir une boîte de l'interface demandée par son nom.
    OpenUi(UiDialog),
    /// Exécuter une ligne de commande dans une fenêtre (`/join #x`).
    Command(WinId, String),
    /// Envoyer du texte dans une fenêtre, comme tapé (une ligne par élément).
    Input(WinId, Vec<String>),
    /// Rechercher dans une fenêtre.
    Find { win: WinId, needle: String, forward: bool },
    /// Les réglages ont changé (thème, polices, barres…).
    SettingsChanged,
    OpenUrl(String),
    /// Quitter vraiment l'application.
    Exit,
}

/// Contexte donné aux contrôleurs.
pub struct Cx<'a> {
    pub client: &'a mut Client,
    pub now: i64,
    pub lang: Lang,
    pub actions: Vec<Action>,
}

impl Cx<'_> {
    pub fn t(&self, key: &str) -> String {
        t(self.lang, key)
    }
}

/// Un contrôleur de boîte de dialogue.
pub trait DialogCtl {
    /// Genre de la boîte : une seule boîte d'un genre donné est ouverte à la fois (la seconde demande ramène
    /// la première au premier plan). `None` : pas de limite.
    fn kind(&self) -> Option<String> {
        None
    }
    /// Description de la boîte.
    fn spec(&self, lang: Lang) -> DialogSpec;
    /// Remplissage initial.
    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let _ = (view, cx);
    }
    /// Message de la boîte.
    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome;
    /// Appelé après chaque événement du réseau ou de l'utilisateur, pour suivre l'état du moteur.
    fn refresh(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let _ = (view, cx);
    }
    /// Nom du dialogue mSL, pour les boîtes ouvertes par un script.
    fn script_name(&self) -> Option<&str> {
        None
    }
}

/// Spécification rapide d'une boîte en pixels.
pub fn spec(title: &str, w: i32, h: i32, controls: Vec<ControlSpec>) -> DialogSpec {
    DialogSpec { title: plain(title), width: w, height: h, units: DialogUnits::Px, controls }
}

pub fn text(id: u32, r: (i32, i32, i32, i32), s: &str) -> ControlSpec {
    ControlSpec::new(id, ControlKind::Text, r, s)
}

pub fn button(id: u32, r: (i32, i32, i32, i32), s: &str) -> ControlSpec {
    ControlSpec::new(id, ControlKind::Button, r, s)
}

pub fn edit(id: u32, r: (i32, i32, i32, i32), s: &str) -> ControlSpec {
    ControlSpec::new(id, ControlKind::Edit, r, s)
}

/// Nombre de lignes de texte qu'occupe `s` sur `w` pixels (estimation : 6 px par caractère).
fn text_rows(s: &str, w: i32) -> i32 {
    let per = (w / 6).max(10) as usize;
    s.lines().map(|l| (l.chars().count().max(1)).div_ceil(per) as i32).sum::<i32>().max(1)
}

// ---------------------------------------------------------------------------------------------------------------
// Boîte de message.

/// Boîte de message : un texte et un ou deux boutons ; l'action du bouton « oui » est faite à la validation.
pub struct MessageBox {
    pub title: String,
    pub text: String,
    pub yes: String,
    pub no: Option<String>,
    pub on_yes: Vec<Action>,
}

impl MessageBox {
    pub fn info(title: &str, text: &str, lang: Lang) -> MessageBox {
        MessageBox { title: title.into(), text: text.into(), yes: t(lang, "dlg.ok"), no: None, on_yes: Vec::new() }
    }
}

impl DialogCtl for MessageBox {
    fn spec(&self, _lang: Lang) -> DialogSpec {
        let w = 340;
        let rows = text_rows(&self.text, w - 24);
        let th = rows * 16;
        let by = 12 + th + 12;
        let mut c = vec![text(1, (12, 12, w - 24, th), &self.text)];
        match &self.no {
            Some(no) => {
                c.push(button(2, (w / 2 - 92, by, 84, 24), &self.yes).style("default ok"));
                c.push(button(3, (w / 2 + 8, by, 84, 24), no).style("cancel"));
            }
            None => c.push(button(2, (w / 2 - 42, by, 84, 24), &self.yes).style("default ok")),
        }
        spec(&self.title, w, by + 24 + 12, c)
    }

    fn on(&mut self, msg: DialogMsg, _view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) | DialogMsg::Clicked(2) => {
                cx.actions.append(&mut self.on_yes);
                Outcome::Close
            }
            DialogMsg::Cancel | DialogMsg::Clicked(3) => Outcome::Close,
            _ => Outcome::Keep,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Collage de plusieurs lignes.

/// Confirmation avant d'envoyer plusieurs lignes collées.
pub struct PasteConfirm {
    pub win: WinId,
    pub win_name: String,
    pub lines: Vec<String>,
}

impl DialogCtl for PasteConfirm {
    fn kind(&self) -> Option<String> {
        Some("paste".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let w = 420;
        let msg = tf(lang, "paste.text", &[("n", &self.lines.len().to_string()), ("win", &self.win_name)]);
        let preview: String = self.lines.iter().take(200).cloned().collect::<Vec<_>>().join("\n");
        spec(
            &t(lang, "paste.title"),
            w,
            12 + 32 + 8 + 160 + 12 + 24 + 12,
            vec![
                text(1, (12, 12, w - 24, 32), &msg),
                ControlSpec::new(4, ControlKind::TextArea, (12, 52, w - 24, 160), &preview).style("read"),
                button(2, (w - 12 - 3 * 92 - 16, 224, 92, 24), &t(lang, "paste.send")).style("default ok"),
                button(5, (w - 12 - 2 * 92 - 8, 224, 92, 24), &t(lang, "paste.edit")),
                button(3, (w - 12 - 92, 224, 92, 24), &t(lang, "dlg.cancel")).style("cancel"),
            ],
        )
    }

    fn on(&mut self, msg: DialogMsg, _view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) | DialogMsg::Clicked(2) => {
                cx.actions.push(Action::Input(self.win, std::mem::take(&mut self.lines)));
                Outcome::Close
            }
            DialogMsg::Clicked(5) => {
                let one = self.lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
                cx.actions.push(Action::Input(self.win, vec![one]));
                Outcome::Close
            }
            DialogMsg::Cancel | DialogMsg::Clicked(3) => Outcome::Close,
            _ => Outcome::Keep,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Rejoindre un canal.

pub struct JoinPrompt {
    pub win: WinId,
}

impl DialogCtl for JoinPrompt {
    fn kind(&self) -> Option<String> {
        Some("join".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let w = 300;
        spec(
            &t(lang, "join.title"),
            w,
            12 + 16 + 6 + 22 + 14 + 24 + 12,
            vec![
                text(1, (12, 12, w - 24, 16), &t(lang, "join.prompt")),
                edit(2, (12, 34, w - 24, 22), "#"),
                button(3, (w - 12 - 2 * 84 - 8, 70, 84, 24), &t(lang, "dlg.ok")).style("default ok"),
                button(4, (w - 12 - 84, 70, 84, 24), &t(lang, "dlg.cancel")).style("cancel"),
            ],
        )
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        view.set_focus(2);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) | DialogMsg::Clicked(3) => {
                let chan = view.text(2).unwrap_or_default();
                let chan = chan.trim();
                if !chan.is_empty() && chan != "#" {
                    cx.actions.push(Action::Command(self.win, format!("/join {chan}")));
                }
                Outcome::Close
            }
            DialogMsg::Cancel | DialogMsg::Clicked(4) => Outcome::Close,
            _ => Outcome::Keep,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Rechercher.

pub struct FindDialog {
    pub win: WinId,
}

impl DialogCtl for FindDialog {
    fn kind(&self) -> Option<String> {
        Some("find".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let w = 340;
        spec(
            &t(lang, "find.title"),
            w,
            12 + 22 + 12 + 24 + 12,
            vec![
                text(1, (12, 15, 80, 16), &t(lang, "find.prompt")),
                edit(2, (92, 12, w - 104, 22), ""),
                button(3, (w - 12 - 3 * 84 - 16, 46, 84, 24), &t(lang, "find.next")).style("default"),
                button(4, (w - 12 - 2 * 84 - 8, 46, 84, 24), &t(lang, "find.prev")),
                button(5, (w - 12 - 84, 46, 84, 24), &t(lang, "dlg.close")).style("cancel"),
            ],
        )
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        view.set_focus(2);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Clicked(3) | DialogMsg::Clicked(4) | DialogMsg::Ok(_) => {
                let needle = view.text(2).unwrap_or_default();
                if !needle.is_empty() {
                    cx.actions.push(Action::Find { win: self.win, needle, forward: !matches!(msg, DialogMsg::Clicked(4)) });
                }
                Outcome::Keep
            }
            DialogMsg::Cancel | DialogMsg::Clicked(5) => Outcome::Close,
            _ => Outcome::Keep,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// À propos.

pub struct About;

impl DialogCtl for About {
    fn kind(&self) -> Option<String> {
        Some("about".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let w = 380;
        let body = tf(lang, "about.text", &[("v", env!("CARGO_PKG_VERSION"))]);
        let th = text_rows(&body, w - 80) * 16;
        let mut icon = ControlSpec::new(9, ControlKind::Icon, (16, 16, 32, 32), "app/alpirc-32");
        icon.styles.push("noborder".into());
        spec(
            &t(lang, "about.title"),
            w,
            16 + th + 16 + 24 + 12,
            vec![
                icon,
                text(1, (64, 16, w - 80, th), &body),
                button(2, (w - 12 - 84, 16 + th + 16, 84, 24), &t(lang, "dlg.ok")).style("default ok"),
            ],
        )
    }

    fn on(&mut self, msg: DialogMsg, _view: &mut DialogView, _cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) | DialogMsg::Cancel | DialogMsg::Clicked(2) => Outcome::Close,
            _ => Outcome::Keep,
        }
    }
}

/// Boîte provisoire pour les fenêtres qui ne sont pas encore faites.
pub fn coming_soon(lang: Lang, what: &str) -> MessageBox {
    MessageBox::info(&format!("{} — {}", plain(what), t(lang, "soon.title")), &t(lang, "soon.text"), lang)
}

/// Confirmation de sortie quand des connexions sont ouvertes.
pub fn quit_confirm(lang: Lang, n: usize) -> MessageBox {
    MessageBox {
        title: t(lang, "quit.title"),
        text: tf(lang, "quit.text", &[("n", &n.to_string())]),
        yes: t(lang, "dlg.yes"),
        no: Some(t(lang, "dlg.no")),
        on_yes: vec![Action::Exit],
    }
}
