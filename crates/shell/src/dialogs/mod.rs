//! Les boîtes de dialogue de l'application : un contrôleur par boîte, qui décrit ses contrôles
//! ([`DialogSpec`]), les remplit depuis le moteur et réagit aux messages de la boîte.
//!
//! Les grandes boîtes vivent chacune dans leur module : Options ([`options`]), éditeur de réseau
//! ([`network`]), choix du serveur ([`connect`]), carnet d'adresses ([`abook`]), dossier des canaux
//! ([`folder`]), Channel Central ([`central`]), User Central ([`user`]), éditeur de scripts ([`editor`]),
//! couleurs ([`colors`]), certificat refusé ([`cert`]) et la petite boîte de saisie commune ([`form`]). Ici : le cadre commun (mesures
//! façon Windows 98, raccourcis de construction), les petites boîtes (message, collage de plusieurs lignes,
//! rejoindre un canal, rechercher, à propos, quitter) ; le pont avec les dialogues des scripts mSL est dans
//! [`script`].
//!
//! Une boîte qui en ouvre une autre (l'éditeur de réseau depuis Options, une entrée du carnet d'adresses…)
//! partage avec elle un état `Rc<RefCell<…>>` : la petite boîte y écrit à sa validation et en change la
//! version ; la grande le voit dans [`DialogCtl::refresh`] et se remet à jour.

pub mod abook;
pub mod central;
pub mod cert;
pub mod chanlist;
pub mod colors;
pub mod connect;
pub mod editor;
pub mod folder;
pub mod form;
pub mod input;
pub mod logs;
pub mod network;
pub mod options;
pub mod script;
mod text;
pub mod user;
pub mod welcome;

/// Textes des boîtes de dialogue (voir [`crate::tr`]).
pub const TR: crate::tr::Table = text::TR;

/// Construit la boîte de l'interface demandée.
pub fn build(d: UiDialog, client: &Client) -> Box<dyn DialogCtl> {
    match d {
        UiDialog::Options(page) => Box::new(options::Options::new(client, page.as_deref())),
        UiDialog::AddressBook => Box::new(abook::AddressBook::new(client)),
        UiDialog::ChannelsFolder => Box::new(folder::ChannelsFolder::new(client)),
        UiDialog::ChannelCentral(w) => match central::ChannelCentral::new(client, w) {
            Some(c) => Box::new(c),
            None => {
                let lang = client.language();
                Box::new(MessageBox::info("Channel Central", &t(lang, "cc.not_channel"), lang))
            }
        },
        UiDialog::UserCentral { conn, nick } => Box::new(user::UserCentral::new(conn, &nick)),
        UiDialog::ScriptEditor => Box::new(editor::ScriptEditor::new(client)),
        UiDialog::Colors => Box::new(colors::Colors::new(client)),
        UiDialog::Connect => Box::new(connect::SelectServer::new(client)),
        UiDialog::About => Box::new(About),
        UiDialog::Find(w) => Box::new(FindDialog { win: w }),
    }
}

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use alpirc_core::i18n::Lang;
use alpirc_core::{Client, UiDialog, WinId};
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogUnits, DialogView, FontKind, Fonts, TextStyle};

use crate::ShellPlatform;
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
    Find {
        win: WinId,
        needle: String,
        forward: bool,
    },
    /// Les réglages ont changé (thème, polices, barres…).
    SettingsChanged,
    OpenUrl(String),
    /// Quitter vraiment l'application.
    Exit,
    /// Rejoindre un canal du Channels Folder, sur une connexion à son réseau (ouverte au besoin).
    JoinFavorite(alpirc_core::settings::Favorite),
    /// Demander un fichier à la plateforme pour la boîte de genre `kind` ; la réponse revient par
    /// [`DialogCtl::file_picked`].
    PickFile {
        kind: String,
        save: bool,
        suggested: Option<String>,
    },
    /// Ouvrir un dossier (ou un fichier) dans le gestionnaire de fichiers du système.
    OpenPath(PathBuf),
    /// `/list` sur la connexion active, avec les choix de la boîte « Liste des canaux ».
    ListChannels(chanlist::ListQuery),
    /// Copier un texte dans le presse-papiers.
    CopyText(String),
}

/// Contexte donné aux contrôleurs.
pub struct Cx<'a> {
    pub client: &'a mut Client,
    pub now: i64,
    pub lang: Lang,
    /// Décalage horaire de l'appareil (secondes), pour les dates affichées.
    pub utc_offset: i32,
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
    /// Réponse de la plateforme à [`Action::PickFile`] (`None` : choix annulé).
    fn file_picked(&mut self, path: Option<PathBuf>, view: &mut DialogView, cx: &mut Cx) {
        let _ = (path, view, cx);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Fichiers demandés par les boîtes.

/// Bit qui marque les demandes de fichier faites par une boîte (le reste : empreinte de son genre ; le bit de
/// poids fort reste à 0, celles du DCC l'ont à 1).
const PICK_BIT: u64 = 1 << 62;

/// Numéro de demande de fichier ([`ShellPlatform::pick_file`]) d'une boîte de genre `kind`.
pub fn pick_request(kind: &str) -> u64 {
    // FNV-1a : stable d'une exécution à l'autre.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in kind.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    PICK_BIT | (h & (PICK_BIT - 1))
}

/// Vrai si la demande de fichier vient d'une boîte de dialogue.
pub fn is_pick_request(request: u64) -> bool {
    request & PICK_BIT != 0 && request >> 63 == 0
}

impl crate::Shell {
    /// Fichier choisi pour une boîte (voir [`Action::PickFile`]) : passé à la boîte qui l'a demandé, si elle est
    /// encore ouverte.
    pub(crate) fn dialog_file_picked(&mut self, request: u64, path: Option<PathBuf>, p: &mut dyn ShellPlatform) {
        let key = self.ctls.iter().find(|(_, c)| c.kind().is_some_and(|k| pick_request(&k) == request)).map(|(k, _)| *k);
        let Some(key) = key else { return };
        let Some(mut ctl) = self.ctls.remove(&key) else { return };
        let lang = self.language();
        let now = p.now_ms();
        let mut actions = Vec::new();
        if let Some(view) = self.modals.get_mut(key) {
            let mut cx = Cx { client: &mut self.client, now, lang, utc_offset: self.utc_offset, actions: Vec::new() };
            ctl.file_picked(path, view, &mut cx);
            actions = cx.actions;
        }
        self.ctls.insert(key, ctl);
        self.run_actions(actions, p);
    }

    /// Une connexion TLS a échoué sur le certificat du serveur : boîte « faire confiance à ce certificat ».
    pub(crate) fn cert_failed(&mut self, conn: alpirc_core::ConnId, error: &str, fp: &str, p: &mut dyn ShellPlatform) {
        if let Some(c) = cert::CertPrompt::new(&self.client, conn, error, fp) {
            self.open_dialog(Box::new(c), p);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Mesures et raccourcis de construction.

/// Marge des boîtes (pixels logiques).
pub const MARGIN: i32 = 8;
/// Bouton poussoir : largeur et hauteur.
pub const BTN_W: i32 = 80;
pub const BTN_H: i32 = 23;
/// Champ de saisie d'une ligne.
pub const EDIT_H: i32 = 22;
/// Case à cocher, bouton radio.
pub const CHECK_H: i32 = 18;
/// Pas entre deux lignes de champs.
pub const ROW: i32 = 28;
/// Hauteur d'une étiquette.
pub const LABEL_H: i32 = 16;

pub fn check(id: u32, r: (i32, i32, i32, i32), s: &str) -> ControlSpec {
    ControlSpec::new(id, ControlKind::Check, r, s)
}

pub fn radio(id: u32, r: (i32, i32, i32, i32), s: &str) -> ControlSpec {
    ControlSpec::new(id, ControlKind::Radio, r, s)
}

/// Cadre de groupe.
pub fn group(id: u32, r: (i32, i32, i32, i32), s: &str) -> ControlSpec {
    ControlSpec::new(id, ControlKind::Box, r, s)
}

/// Liste déroulante non modifiable ; la hauteur `r.3` comprend la liste ouverte.
pub fn drop_list<S: AsRef<str>>(id: u32, r: (i32, i32, i32, i32), items: &[S]) -> ControlSpec {
    ControlSpec::new(id, ControlKind::Combo, r, "").items(items).style("drop")
}

/// Liste déroulante modifiable.
pub fn combo_edit<S: AsRef<str>>(id: u32, r: (i32, i32, i32, i32), items: &[S], text: &str) -> ControlSpec {
    ControlSpec::new(id, ControlKind::Combo, r, text).items(items).style("edit")
}

/// Liste à colonnes (titres ; largeurs avec [`DialogView::set_columns`]).
pub fn list_view(id: u32, r: (i32, i32, i32, i32), cols: &[String]) -> ControlSpec {
    ControlSpec::new(id, ControlKind::ListView, r, &cols.join("\t"))
}

pub fn list_box(id: u32, r: (i32, i32, i32, i32)) -> ControlSpec {
    ControlSpec::new(id, ControlKind::List, r, "")
}

pub fn text_area(id: u32, r: (i32, i32, i32, i32), s: &str) -> ControlSpec {
    ControlSpec::new(id, ControlKind::TextArea, r, s)
}

pub fn password(id: u32, r: (i32, i32, i32, i32), s: &str) -> ControlSpec {
    edit(id, r, s).style("password")
}

/// Étiquette d'un champ placé à droite : centrée verticalement sur un champ de hauteur [`EDIT_H`] en `y`.
pub fn field_label(id: u32, x: i32, y: i32, w: i32, s: &str) -> ControlSpec {
    text(id, (x, y + (EDIT_H - LABEL_H) / 2, w, LABEL_H), s).style("nowrap")
}

/// Boutons OK, Annuler (et Appliquer) en bas à droite d'une boîte de `w` sur `h`.
pub fn ok_cancel(lang: Lang, w: i32, h: i32, ok: u32, cancel: u32, apply: Option<u32>) -> Vec<ControlSpec> {
    let y = h - MARGIN - BTN_H;
    let n = if apply.is_some() { 3 } else { 2 };
    let x0 = w - MARGIN - n * BTN_W - (n - 1) * 6;
    let mut v = vec![
        button(ok, (x0, y, BTN_W, BTN_H), &t(lang, "dlg.ok")).style("default ok"),
        button(cancel, (x0 + BTN_W + 6, y, BTN_W, BTN_H), &t(lang, "dlg.cancel")).style("cancel"),
    ];
    if let Some(a) = apply {
        v.push(button(a, (x0 + 2 * (BTN_W + 6), y, BTN_W, BTN_H), &t(lang, "dlg.apply")));
    }
    v
}

/// Une ligne de liste à colonnes (cellules séparées par des tabulations).
pub fn row<S: AsRef<str>>(cells: &[S]) -> String {
    cells.iter().map(|c| c.as_ref().replace(['\t', '\n'], " ")).collect::<Vec<_>>().join("\t")
}

/// Premier élément choisi d'une liste.
pub fn sel(view: &DialogView, id: u32) -> Option<usize> {
    view.selection(id).first().copied()
}

/// Texte d'un champ.
pub fn text_of(view: &DialogView, id: u32) -> String {
    view.text(id).unwrap_or_default()
}

/// Texte d'un champ sans espaces autour ; `None` s'il est vide.
pub fn opt_text(view: &DialogView, id: u32) -> Option<String> {
    let s = text_of(view, id).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Nombre lu dans un champ ; `default` s'il n'est pas valable.
pub fn num<T: std::str::FromStr>(view: &DialogView, id: u32, default: T) -> T {
    text_of(view, id).trim().parse().unwrap_or(default)
}

/// Choisit une ligne d'une liste et la montre.
pub fn select_row(view: &mut DialogView, id: u32, i: Option<usize>) {
    match i {
        Some(i) => view.set_selection(id, &[i]),
        None => view.set_selection(id, &[]),
    }
}

/// Fenêtre Status à utiliser pour une commande de la boîte (celle de la fenêtre active, sinon la première).
pub fn status_window(client: &Client) -> Option<WinId> {
    let active = client.active().and_then(|a| client.window(a));
    active
        .and_then(|w| w.conn)
        .and_then(|c| client.conn_info(c))
        .map(|c| c.status_window)
        .or_else(|| client.windows().iter().find(|w| w.kind == alpirc_core::WinKind::Status).map(|w| w.id))
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

thread_local! {
    /// Polices avec lesquelles les boîtes mesurent leurs textes en se décrivant ([`DialogCtl::spec`] n'a pas les
    /// ressources de dessin) : celles de l'application ([`set_measure_fonts`]), sinon les polices intégrées.
    static MEASURE: RefCell<Option<Rc<Fonts>>> = const { RefCell::new(None) };
}

/// Donne aux boîtes les polices de l'application, pour mesurer leurs textes (voir [`text_height`]).
pub(crate) fn set_measure_fonts(fonts: Rc<Fonts>) {
    MEASURE.with(|m| *m.borrow_mut() = Some(fonts));
}

fn measure_fonts() -> Rc<Fonts> {
    MEASURE.with(|m| m.borrow_mut().get_or_insert_with(|| Rc::new(crate::assets::fonts())).clone())
}

/// Hauteur du texte statique `s` sur `w` pixels : ses lignes, mesurées avec la police de l'interface et coupées
/// comme à l'écran (une adresse trop longue passe à la ligne), à la hauteur de ligne de l'interface.
pub(crate) fn text_height(s: &str, w: i32) -> i32 {
    let f = measure_fonts();
    f.label_rows(s, TextStyle::UI, w) as i32 * f.line_height(FontKind::Ui)
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
        let th = text_height(&self.text, w - 24);
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
        let prompt = t(lang, "join.prompt");
        // La question peut tenir sur deux lignes (allemand).
        let th = text_height(&prompt, w - 24);
        let ey = 12 + th + 6;
        let by = ey + 22 + 14;
        spec(
            &t(lang, "join.title"),
            w,
            by + 24 + 12,
            vec![
                text(1, (12, 12, w - 24, th), &prompt),
                edit(2, (12, ey, w - 24, 22), "#"),
                button(3, (w - 12 - 2 * 84 - 8, by, 84, 24), &t(lang, "dlg.ok")).style("default ok"),
                button(4, (w - 12 - 84, by, 84, 24), &t(lang, "dlg.cancel")).style("cancel"),
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
        let th = text_height(&body, w - 80);
        let mut icon = ControlSpec::new(9, ControlKind::Icon, (16, 16, 32, 32), "app/alpirc-32");
        icon.styles.push("noborder".into());
        spec(
            &t(lang, "about.title"),
            w,
            16 + th + 16 + 24 + 12,
            vec![icon, text(1, (64, 16, w - 80, th), &body), button(2, (w - 12 - 84, 16 + th + 16, 84, 24), &t(lang, "dlg.ok")).style("default ok")],
        )
    }

    fn on(&mut self, msg: DialogMsg, _view: &mut DialogView, _cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) | DialogMsg::Cancel | DialogMsg::Clicked(2) => Outcome::Close,
            _ => Outcome::Keep,
        }
    }
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

// ---------------------------------------------------------------------------------------------------------------
// Banc d'essai des boîtes.

#[cfg(test)]
pub(crate) mod testkit {
    use std::path::PathBuf;

    use alpirc_core::{Client, ConnId, Settings, WinId};
    use alpirc_ui::{DialogMsg, DialogView, Rect, Res};

    use super::{Action, Cx, DialogCtl, Outcome};

    /// Un client sans réseau, dans un dossier temporaire, et de quoi ouvrir des boîtes.
    pub struct Kit {
        pub client: Client,
        pub res: Res,
        pub dir: PathBuf,
        pub now: i64,
        pub lang: alpirc_core::i18n::Lang,
    }

    impl Kit {
        pub fn new(name: &str) -> Kit {
            let dir = std::env::temp_dir().join(format!("alpirc-dlg-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let now = 1_792_260_000_000;
            let client = Client::new(Settings::default(), dir.clone(), now);
            let res = Res::new(crate::assets::fonts());
            super::set_measure_fonts(res.fonts.clone());
            Kit { client, res, dir, now, lang: alpirc_core::i18n::Lang::En }
        }

        /// Construit la boîte et l'initialise.
        pub fn open(&mut self, ctl: &mut dyn DialogCtl) -> (DialogView, Vec<Action>) {
            let spec = ctl.spec(self.lang);
            let mut view = DialogView::new(&spec, &self.res);
            view.center_in(Rect::new(0, 0, 1024, 768));
            let mut cx = Cx { client: &mut self.client, now: self.now, lang: self.lang, utc_offset: 0, actions: Vec::new() };
            ctl.init(&mut view, &mut cx);
            let a = cx.actions;
            (view, a)
        }

        pub fn send(&mut self, ctl: &mut dyn DialogCtl, view: &mut DialogView, msg: DialogMsg) -> (Outcome, Vec<Action>) {
            let mut cx = Cx { client: &mut self.client, now: self.now, lang: self.lang, utc_offset: 0, actions: Vec::new() };
            let o = ctl.on(msg, view, &mut cx);
            let a = cx.actions;
            (o, a)
        }

        pub fn refresh(&mut self, ctl: &mut dyn DialogCtl, view: &mut DialogView) -> Vec<Action> {
            let mut cx = Cx { client: &mut self.client, now: self.now, lang: self.lang, utc_offset: 0, actions: Vec::new() };
            ctl.refresh(view, &mut cx);
            cx.actions
        }

        /// Connexion simulée à Undernet ; rend la connexion et la fenêtre Status.
        pub fn connect(&mut self) -> (ConnId, WinId) {
            let status = self.client.windows()[0].id;
            self.client.connect(status, "Undernet", None, self.now);
            let conn = self.client.conns()[0];
            self.client.on_connected(conn, self.now);
            for l in [
                ":irc.undernet.org 001 Guiwald :Welcome",
                ":irc.undernet.org 005 Guiwald NETWORK=UnderNet PREFIX=(ov)@+ CHANMODES=b,k,l,imnpstrDd :are supported",
                ":irc.undernet.org 376 Guiwald :End of /MOTD command.",
            ] {
                self.line(conn, l);
            }
            self.client.take_effects();
            (conn, status)
        }

        pub fn line(&mut self, conn: ConnId, l: &str) {
            self.client.on_line(conn, l.as_bytes(), self.now);
        }

        /// Lignes envoyées au serveur depuis le dernier appel.
        pub fn sent(&mut self) -> Vec<String> {
            self.client
                .take_effects()
                .into_iter()
                .filter_map(|e| match e {
                    alpirc_core::Effect::Send { line, .. } => Some(line),
                    _ => None,
                })
                .collect()
        }

        /// Exécute les actions qui touchent le moteur (commandes, réglages) comme le ferait l'application.
        pub fn run(&mut self, actions: Vec<Action>) {
            for a in actions {
                match a {
                    Action::Command(w, l) => self.client.command(w, &l, self.now),
                    Action::SettingsChanged => self.client.settings_changed(self.now),
                    _ => {}
                }
            }
        }
    }

    /// Vérifie qu'aucun texte de la boîte n'est une clé de traduction oubliée (`opt.irc.x`).
    pub fn assert_translated(ctl: &dyn DialogCtl) {
        for lang in [alpirc_core::i18n::Lang::En, alpirc_core::i18n::Lang::Fr] {
            let s = ctl.spec(lang);
            let mut texts = vec![s.title.clone()];
            for c in &s.controls {
                texts.push(c.text.clone());
                texts.extend(c.items.iter().cloned());
                texts.extend(c.text.split('\t').map(str::to_string));
            }
            for t in texts {
                const PREFIXES: [&str; 12] = ["opt.", "net.", "ab.", "fold.", "cc.", "uc.", "se.", "col.", "srv.", "cert.", "form.", "dlg."];
                let looks_like_key = PREFIXES.iter().any(|p| t.starts_with(p))
                    && !t.contains(' ')
                    && t.chars().all(|c| c.is_ascii_lowercase() || c == '.' || c == '_' || c.is_ascii_digit());
                assert!(!looks_like_key, "clé sans traduction : {t}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_text_is_measured_with_the_interface_font() {
        let k = testkit::Kit::new("about");
        for lang in Lang::ALL {
            let s = About.spec(lang);
            let body = s.controls.iter().find(|c| c.id == 1).unwrap();
            // Toutes les lignes, coupées comme à l'écran (l'adresse peut passer à la ligne).
            let rows = k.res.fonts.label_rows(&body.text, TextStyle::UI, body.rect.2);
            assert!(rows >= body.text.lines().count());
            assert_eq!(body.rect.3, rows as i32 * k.res.ui_line(), "{lang:?}");
            // Le bouton OK est sous la dernière ligne.
            let ok = s.controls.iter().find(|c| c.id == 2).unwrap();
            assert!(ok.rect.1 >= body.rect.1 + body.rect.3);
        }
    }
}
