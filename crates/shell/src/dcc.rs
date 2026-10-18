//! DCC dans l'interface : la fenêtre des transferts (« DCC Transfers » de mIRC : fichier, pseudo, taille,
//! avancement, débit, état ; ouvrir le dossier, annuler, renvoyer), les boîtes « Envoyer un fichier » et
//! « Discussion DCC », la question posée pour chaque offre reçue (accepter, reprendre, écraser, renommer,
//! ignorer), et le pont entre les effets DCC du moteur et les connexions directes d'`alpirc-net`.
//!
//! Le moteur décide (protocole, réglages, bac à sable) ; ici on ouvre les écoutes et les connexions, on rend
//! compte au moteur (`Client::on_dcc_*`) et on montre l'état.

use std::cell::RefCell;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use alpirc_core::dcc::{DccIo, human_size};
use alpirc_core::i18n::Lang;
use alpirc_core::{ConnId, DccAnswer, DccId, DccPrompt, DccType, DccUi, Effect, Transfer, TransferState, WinKind};
use alpirc_net::{DccJob, NetEvent};
use alpirc_ui::dialog::Widget;
use alpirc_ui::widgets::listview::ListRow;
use alpirc_ui::{Align, ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogView};

use crate::dialogs::{Cx, DialogCtl, MessageBox, Outcome, button, edit, spec, text};
use crate::tr::{t, tf};
use crate::{Shell, ShellPlatform};

/// Textes du DCC (voir [`crate::tr`]).
pub const TR: crate::tr::Table = &[
    (
        "dcc.transfers.title",
        [
            "DCC Transfers",
            "Transferts DCC",
            "Transferencias DCC",
            "DCC-Übertragungen",
            "Trasferimenti DCC",
            "Transferències DCC",
            "Aistrithe DCC",
        ],
    ),
    ("dcc.col.file", ["File", "Fichier", "Archivo", "Datei", "File", "Fitxer", "Comhad"]),
    ("dcc.col.nick", ["Nick", "Pseudo", "Nick", "Nick", "Nick", "Sobrenom", "Leasainm"]),
    ("dcc.col.type", ["Type", "Type", "Tipo", "Typ", "Tipo", "Tipus", "Cineál"]),
    ("dcc.col.size", ["Size", "Taille", "Tamaño", "Größe", "Dim.", "Mida", "Méid"]),
    ("dcc.col.progress", ["%", "%", "%", "%", "%", "%", "%"]),
    ("dcc.col.speed", ["Speed", "Débit", "Velocidad", "Rate", "Velocità", "Velocitat", "Luas"]),
    ("dcc.col.status", ["Status", "État", "Estado", "Status", "Stato", "Estat", "Staid"]),
    ("dcc.type.send", ["Send", "Envoi", "Envío", "Senden", "Invio", "Enviament", "Seoladh"]),
    ("dcc.type.get", ["Get", "Réception", "Recepción", "Empfang", "Ricezione", "Recepció", "Fáil"]),
    (
        "dcc.btn.open_folder",
        [
            "Open &Folder",
            "Ouvrir le &dossier",
            "Abrir &carpeta",
            "&Ordner öffnen",
            "Apri &cartella",
            "Obrir &carpeta",
            "Oscail &fillteán",
        ],
    ),
    (
        "dcc.btn.cancel",
        ["&Cancel Transfer", "&Annuler", "Ca&ncelar", "&Abbrechen", "&Annulla", "Ca&ncel·lar", "&Cealaigh"],
    ),
    ("dcc.btn.resend", ["&Resend", "&Renvoyer", "&Reenviar", "&Erneut senden", "&Reinvia", "&Reenviar", "&Athsheol"]),
    (
        "dcc.btn.clear",
        [
            "C&lear Finished",
            "&Vider les terminés",
            "&Quitar acabadas",
            "&Fertige entfernen",
            "&Togli completati",
            "Ne&tejar acabats",
            "&Glan críochnaithe",
        ],
    ),
    (
        "dcc.none",
        [
            "No transfers yet. Send a file with /dcc send nick, or from a query's menu.",
            "Aucun transfert. Envoyez un fichier par /dcc send pseudo, ou par le menu d'un privé.",
            "Aún no hay transferencias. Envía un archivo con /dcc send nick, o desde el menú de un privado.",
            "Noch keine Übertragungen. Senden Sie eine Datei mit /dcc send Nick oder über das Menü einer Query.",
            "Ancora nessun trasferimento. Invia un file con /dcc send nick, o dal menu di una query.",
            "Encara no hi ha cap transferència. Envieu un fitxer amb /dcc send sobrenom, o des del menú d'un privat.",
            "Níl aistrithe ann fós. Seol comhad le /dcc send leasainm, nó ó roghchlár comhrá.",
        ],
    ),
    (
        "dcc.detail",
        [
            "{done} of {size} — {speed}/s{eta} — {path}",
            "{done} sur {size} — {speed}/s{eta} — {path}",
            "{done} de {size} — {speed}/s{eta} — {path}",
            "{done} von {size} — {speed}/s{eta} — {path}",
            "{done} di {size} — {speed}/s{eta} — {path}",
            "{done} de {size} — {speed}/s{eta} — {path}",
            "{done} as {size} — {speed}/s{eta} — {path}",
        ],
    ),
    (
        "dcc.detail.eta",
        [
            ", {eta} left",
            ", reste {eta}",
            ", quedan {eta}",
            ", noch {eta}",
            ", mancano {eta}",
            ", queden {eta}",
            ", {eta} fágtha",
        ],
    ),
    (
        "dcc.detail.error",
        [
            "{error} — {path}",
            "{error} — {path}",
            "{error} — {path}",
            "{error} — {path}",
            "{error} — {path}",
            "{error} — {path}",
            "{error} — {path}",
        ],
    ),
    (
        "dcc.send.title",
        [
            "DCC Send",
            "Envoyer un fichier (DCC)",
            "Enviar archivo (DCC)",
            "Datei senden (DCC)",
            "Invia file (DCC)",
            "Enviar fitxer (DCC)",
            "Seol comhad (DCC)",
        ],
    ),
    ("dcc.send.nick", ["&Nickname:", "&Pseudo :", "&Nick:", "&Nickname:", "&Nick:", "&Sobrenom:", "&Leasainm:"]),
    ("dcc.send.file", ["&File:", "&Fichier :", "&Archivo:", "&Datei:", "&File:", "&Fitxer:", "&Comhad:"]),
    (
        "dcc.send.browse",
        [
            "&Browse...",
            "Pa&rcourir...",
            "&Examinar...",
            "D&urchsuchen...",
            "Sfogl&ia...",
            "E&xplorar...",
            "&Brabhsáil...",
        ],
    ),
    ("dcc.send.send", ["&Send", "&Envoyer", "En&viar", "&Senden", "In&via", "En&viar", "&Seol"]),
    (
        "dcc.send.need",
        [
            "Give a nickname and a file.",
            "Indiquez un pseudo et un fichier.",
            "Indica un nick y un archivo.",
            "Geben Sie einen Nick und eine Datei an.",
            "Indica un nick e un file.",
            "Indiqueu un sobrenom i un fitxer.",
            "Tabhair leasainm agus comhad.",
        ],
    ),
    ("dcc.chat.title", ["DCC Chat", "Discussion DCC", "Chat DCC", "DCC-Chat", "Chat DCC", "Xat DCC", "Comhrá DCC"]),
    ("dcc.chat.chat", ["&Chat", "&Discuter", "&Chatear", "&Chatten", "&Chatta", "&Xatejar", "&Comhrá"]),
    (
        "dcc.chat.need",
        [
            "Give a nickname.",
            "Indiquez un pseudo.",
            "Indica un nick.",
            "Geben Sie einen Nick an.",
            "Indica un nick.",
            "Indiqueu un sobrenom.",
            "Tabhair leasainm.",
        ],
    ),
    (
        "dcc.noconn",
        [
            "Not connected to a server: connect first.",
            "Pas connecté à un serveur : connectez-vous d'abord.",
            "No estás conectado a ningún servidor: conéctate primero.",
            "Nicht mit einem Server verbunden: Verbinden Sie sich zuerst.",
            "Non sei connesso a un server: connettiti prima.",
            "No esteu connectat a cap servidor: connecteu-vos primer.",
            "Gan cheangal le freastalaí: ceangail ar dtús.",
        ],
    ),
    (
        "dcc.offer.title",
        [
            "DCC Send from {nick}",
            "Envoi DCC de {nick}",
            "Envío DCC de {nick}",
            "DCC-Send von {nick}",
            "Invio DCC da {nick}",
            "Enviament DCC de {nick}",
            "Seoladh DCC ó {nick}",
        ],
    ),
    (
        "dcc.offer.chat_title",
        [
            "DCC Chat from {nick}",
            "Discussion DCC de {nick}",
            "Chat DCC de {nick}",
            "DCC-Chat von {nick}",
            "Chat DCC da {nick}",
            "Xat DCC de {nick}",
            "Comhrá DCC ó {nick}",
        ],
    ),
    (
        "dcc.offer.text",
        [
            "{nick} ({network}) wants to send you {file} ({size}).",
            "{nick} ({network}) veut vous envoyer {file} ({size}).",
            "{nick} ({network}) quiere enviarte {file} ({size}).",
            "{nick} ({network}) möchte Ihnen {file} ({size}) senden.",
            "{nick} ({network}) vuole inviarti {file} ({size}).",
            "{nick} ({network}) vol enviar-vos {file} ({size}).",
            "Ba mhaith le {nick} ({network}) {file} ({size}) a sheoladh chugat.",
        ],
    ),
    (
        "dcc.offer.chat_text",
        [
            "{nick} ({network}) wants to chat with you directly (DCC Chat). Your IP address will be visible to them.",
            "{nick} ({network}) veut discuter avec vous en direct (DCC Chat). Votre adresse IP lui sera visible.",
            "{nick} ({network}) quiere chatear contigo directamente (DCC Chat). Podrá ver tu dirección IP.",
            "{nick} ({network}) möchte direkt mit Ihnen chatten (DCC Chat). Ihre IP-Adresse wird dabei sichtbar.",
            "{nick} ({network}) vuole chattare con te direttamente (DCC Chat). Potrà vedere il tuo indirizzo IP.",
            "{nick} ({network}) vol xatejar directament amb vós (DCC Chat). Podrà veure la vostra adreça IP.",
            "Ba mhaith le {nick} ({network}) comhrá díreach a dhéanamh leat (DCC Chat). Feicfidh sé/sí do sheoladh IP.",
        ],
    ),
    (
        "dcc.offer.danger",
        [
            "Warning: this file is a program or a script. Open it only if you trust {nick}.",
            "Attention : ce fichier est un programme ou un script. Ne l'ouvrez que si vous avez confiance en {nick}.",
            "Atención: este archivo es un programa o un script. Ábrelo solo si confías en {nick}.",
            "Achtung: Diese Datei ist ein Programm oder ein Script. Öffnen Sie sie nur, wenn Sie {nick} vertrauen.",
            "Attenzione: questo file è un programma o uno script. Aprilo solo se ti fidi di {nick}.",
            "Atenció: este fitxer és un programa o un script. Obriu-lo només si confieu en {nick}.",
            "Aire: is ríomhchlár nó script é an comhad seo. Ná hoscail é ach amháin má tá muinín agat as {nick}.",
        ],
    ),
    (
        "dcc.offer.exists",
        [
            "A file with this name already exists ({size}).",
            "Un fichier de ce nom existe déjà ({size}).",
            "Ya existe un archivo con este nombre ({size}).",
            "Eine Datei mit diesem Namen existiert bereits ({size}).",
            "Esiste già un file con questo nome ({size}).",
            "Ja existix un fitxer amb este nom ({size}).",
            "Tá comhad leis an ainm seo ann cheana ({size}).",
        ],
    ),
    (
        "dcc.offer.saveto",
        [
            "Saved in: {path}",
            "Enregistré dans : {path}",
            "Se guardará en: {path}",
            "Gespeichert in: {path}",
            "Salvato in: {path}",
            "Es guardarà en: {path}",
            "Sábháilfear in: {path}",
        ],
    ),
    ("dcc.offer.accept", ["&Accept", "&Accepter", "&Aceptar", "&Annehmen", "&Accetta", "&Acceptar", "&Glac leis"]),
    ("dcc.offer.resume", ["&Resume", "&Reprendre", "&Reanudar", "&Fortsetzen", "&Riprendi", "&Reprendre", "A&tosaigh"]),
    (
        "dcc.offer.overwrite",
        ["&Overwrite", "É&craser", "Reem&plazar", "&Ersetzen", "&Sovrascrivi", "&Substituir", "&Forscríobh"],
    ),
    (
        "dcc.offer.rename",
        ["Re&name", "Re&nommer", "Re&nombrar", "&Umbenennen", "Ri&nomina", "Rea&nomenar", "Athai&nmnigh"],
    ),
    ("dcc.offer.ignore", ["&Ignore", "&Ignorer", "&Ignorar", "&Ignorieren", "&Ignora", "Ig&norar", "Neamh&aird"]),
    ("dcc.error.title", ["DCC", "DCC", "DCC", "DCC", "DCC", "DCC", "DCC"]),
];

/// Les demandes d'un fichier à la plateforme portent ce préfixe (bits hauts de `request`).
const PICK_TAG: u64 = 0xDCC0;

/// Ce que les boîtes DCC demandent à l'application (elles n'ont pas accès à la plateforme).
pub(crate) enum UiReq {
    /// Choisir un fichier pour la boîte « Envoyer un fichier ».
    PickForDialog,
    OpenFolder(PathBuf),
}

/// Pour qui un fichier est choisi.
enum Pick {
    /// La boîte « Envoyer un fichier » (le nom va dans son champ).
    Dialog,
    /// `/dcc send pseudo` sans fichier : envoi direct.
    SendTo { conn: ConnId, nick: String },
}

/// L'état DCC de l'interface.
#[derive(Default)]
pub(crate) struct DccShell {
    /// Adresse locale de chaque connexion IRC (famille de l'écoute).
    local: HashMap<ConnId, IpAddr>,
    reqs: Rc<RefCell<Vec<UiReq>>>,
    /// Fichier choisi pour la boîte d'envoi, en attente qu'elle le prenne.
    picked: Rc<RefCell<Option<PathBuf>>>,
    picks: HashMap<u64, Pick>,
    next_pick: u64,
}

impl DccShell {
    fn new_pick(&mut self, what: Pick) -> u64 {
        self.next_pick += 1;
        let r = (PICK_TAG << 48) | (self.next_pick & 0xFFFF_FFFF_FFFF);
        self.picks.insert(r, what);
        r
    }
}

/// La demande de fichier vient-elle du DCC ?
pub(crate) fn is_dcc_pick(request: u64) -> bool {
    request >> 48 == PICK_TAG
}

fn job(io: DccIo) -> DccJob {
    match io {
        DccIo::Chat => DccJob::Chat,
        DccIo::Send { path, offset, size, packet } => DccJob::Send { path, offset, size, packet: packet as usize },
        DccIo::Receive { path, offset, size } => DccJob::Receive { path, offset, size },
    }
}

fn dcc_kind_label(lang: Lang, k: DccType) -> String {
    match k {
        DccType::Send => t(lang, "dcc.type.send"),
        DccType::Get => t(lang, "dcc.type.get"),
        DccType::Chat => "Chat".into(),
    }
}

/// Première lettre en capitale (`done` → `Done`).
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Durée lisible (`1h02`, `3 min`, `12 s`).
fn human_secs(s: u64) -> String {
    if s >= 3600 {
        format!("{}h{:02}", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{} min", s / 60)
    } else {
        format!("{s} s")
    }
}

impl Shell {
    /// Une connexion IRC est ouverte : son adresse locale sert aux offres DCC.
    pub(crate) fn dcc_conn_local(&mut self, conn: ConnId, local: SocketAddr) {
        let ip = local.ip().to_canonical();
        self.dcc.local.insert(conn, ip);
        self.client.set_local_ip(conn, ip);
    }

    /// Les effets DCC du moteur.
    pub(crate) fn dcc_effect(&mut self, e: Effect, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        match e {
            Effect::DccListen { id, conn, ports, io, timeout_secs } => {
                let bind = match self.dcc.local.get(&conn) {
                    Some(IpAddr::V6(_)) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                    _ => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                };
                match self.net.dcc_listen(id, bind, ports, job(io), Duration::from_secs(timeout_secs.max(1))) {
                    Ok(addr) => self.client.on_dcc_listening(id, addr.port(), now),
                    Err(err) => self.client.on_dcc_ended(id, 0, Some(&err), now),
                }
            }
            Effect::DccConnect { id, ip, port, io, timeout_secs } => {
                self.net.dcc_connect(id, SocketAddr::new(ip, port), job(io), Duration::from_secs(timeout_secs.max(1)));
            }
            Effect::DccResume { id, offset } => self.net.dcc_resume(id, offset),
            Effect::DccChatSend { id, line } => self.net.dcc_send_line(id, line.into_bytes()),
            Effect::DccClose { id } => self.net.dcc_close(id),
            Effect::DccPrompt(prompt) => self.open_dialog(Box::new(OfferDialog { prompt }), p),
            Effect::DccPickFile { conn, nick } => {
                let r = self.dcc.new_pick(Pick::SendTo { conn, nick });
                p.pick_file(r, false, None);
            }
            Effect::DccWindow(ui) => self.dcc_open_ui(ui, None, p),
            _ => {}
        }
    }

    /// Les événements DCC du réseau.
    pub(crate) fn dcc_net(&mut self, ev: NetEvent, now: i64) {
        match ev {
            NetEvent::DccConnected { id, .. } => self.client.on_dcc_connected(id, now),
            NetEvent::DccLine { id, bytes } => self.client.on_dcc_chat_line(id, &bytes, now),
            NetEvent::DccProgress { id, bytes } => self.client.on_dcc_progress(id, bytes, now),
            NetEvent::DccEnded { id, bytes, error } => self.client.on_dcc_ended(id, bytes, error.as_ref(), now),
            _ => {}
        }
    }

    /// Ouvre une fenêtre DCC de l'interface.
    fn dcc_open_ui(&mut self, ui: DccUi, file: Option<PathBuf>, p: &mut dyn ShellPlatform) {
        let active = self.client.active().and_then(|a| self.client.window(a));
        let conn = active.and_then(|w| w.conn);
        let nick = active
            .filter(|w| matches!(w.kind, WinKind::Query | WinKind::DccChat))
            .map(|w| w.name.trim_start_matches('=').to_string())
            .unwrap_or_default();
        let ctl: Box<dyn DialogCtl> = match ui {
            DccUi::Transfers => Box::new(TransfersDialog { version: u64::MAX, reqs: self.dcc.reqs.clone(), ids: Vec::new() }),
            DccUi::SendDialog => Box::new(SendDialog {
                conn,
                nick,
                file: file.map(|f| f.display().to_string()).unwrap_or_default(),
                reqs: self.dcc.reqs.clone(),
                picked: self.dcc.picked.clone(),
            }),
            DccUi::ChatDialog => Box::new(ChatDialog { conn, nick }),
        };
        self.open_dialog(ctl, p);
    }

    /// Entrées DCC des menus et de la barre d'outils.
    pub(crate) fn dcc_menu(&mut self, id: u32, p: &mut dyn ShellPlatform) {
        let ui = match id {
            crate::menus::ids::DCC_SEND => DccUi::SendDialog,
            crate::menus::ids::DCC_CHAT => DccUi::ChatDialog,
            _ => DccUi::Transfers,
        };
        self.dcc_open_ui(ui, None, p);
    }

    /// Après chaque action : demandes des boîtes DCC, questions devenues sans objet.
    pub(crate) fn dcc_after(&mut self, p: &mut dyn ShellPlatform) {
        let reqs: Vec<UiReq> = std::mem::take(&mut *self.dcc.reqs.borrow_mut());
        for r in reqs {
            match r {
                UiReq::PickForDialog => {
                    let req = self.dcc.new_pick(Pick::Dialog);
                    p.pick_file(req, false, None);
                }
                UiReq::OpenFolder(path) => p.open_path(&path),
            }
        }
        // Offres acceptées, refusées ou expirées ailleurs (commande, délai) : leur question se ferme.
        let stale: Vec<u64> = self
            .ctls
            .iter()
            .filter_map(|(k, c)| {
                let kind = c.kind()?;
                let id: DccId = kind.strip_prefix("dcc.offer.")?.parse().ok()?;
                self.client.dcc_prompt(id).is_none().then_some(*k)
            })
            .collect();
        for k in stale {
            self.modals.close(k);
            self.ctls.remove(&k);
        }
    }

    /// Réponse de la plateforme à une demande de fichier du DCC.
    pub(crate) fn dcc_file_picked(&mut self, request: u64, path: Option<PathBuf>, p: &mut dyn ShellPlatform) {
        let Some(what) = self.dcc.picks.remove(&request) else { return };
        let Some(path) = path else { return };
        match what {
            Pick::Dialog => *self.dcc.picked.borrow_mut() = Some(path),
            Pick::SendTo { conn, nick } => {
                let now = p.now_ms();
                if let Err(e) = self.client.dcc_send_file(conn, &nick, &path, now) {
                    self.dcc_error(&e, p);
                }
            }
        }
    }

    fn dcc_error(&mut self, msg: &str, p: &mut dyn ShellPlatform) {
        let lang = self.language();
        let mb = MessageBox::info(&t(lang, "dcc.error.title"), msg.trim_start_matches("* "), lang);
        self.open_dialog(Box::new(mb), p);
    }

    /// Un fichier lâché sur la fenêtre : envoyé au pseudo du privé (ou de la discussion DCC) actif, sinon la
    /// boîte « Envoyer un fichier » s'ouvre avec ce fichier.
    pub fn file_dropped(&mut self, path: PathBuf, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        let target = self
            .client
            .active()
            .and_then(|a| self.client.window(a))
            .filter(|w| matches!(w.kind, WinKind::Query | WinKind::DccChat))
            .and_then(|w| Some((w.conn?, w.name.trim_start_matches('=').to_string())));
        match target {
            Some((conn, nick)) => {
                if let Err(e) = self.client.dcc_send_file(conn, &nick, &path, now) {
                    self.dcc_error(&e, p);
                }
            }
            None => self.dcc_open_ui(DccUi::SendDialog, Some(path), p),
        }
        self.after(p);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Fenêtre des transferts.

const LIST: u32 = 10;
const BAR: u32 = 11;
const DETAIL: u32 = 12;
const B_FOLDER: u32 = 20;
const B_CANCEL: u32 = 21;
const B_RESEND: u32 = 22;
const B_CLEAR: u32 = 23;
const B_CLOSE: u32 = 24;

/// Largeurs des colonnes (fichier, pseudo, type, taille, %, débit, état).
const WIDTHS: [i32; 7] = [184, 84, 64, 70, 52, 74, 92];

struct TransfersDialog {
    /// Version des transferts déjà affichée.
    version: u64,
    reqs: Rc<RefCell<Vec<UiReq>>>,
    /// Identifiants des lignes, dans l'ordre.
    ids: Vec<DccId>,
}

impl TransfersDialog {
    fn selected(&self, view: &DialogView) -> Option<DccId> {
        view.selection(LIST).first().and_then(|&i| self.ids.get(i)).copied()
    }

    fn row(lang: Lang, x: &Transfer) -> ListRow {
        let size = x.size.map(human_size).unwrap_or_else(|| "?".into());
        let pct = x.percent().map(|p| format!("{p}%")).unwrap_or_default();
        let speed = if x.state == TransferState::Active && x.speed > 0 { format!("{}/s", human_size(x.speed)) } else { String::new() };
        let status = capitalize(alpirc_core::client::dcc_state_label(lang, x.state));
        let mut r = ListRow::new(&[x.file.clone(), x.nick.clone(), dcc_kind_label(lang, x.kind), size, pct, speed, status]);
        r.data = x.id as u64;
        r.color = match x.state {
            TransferState::Failed => Some(0xC00000),
            TransferState::Done => Some(0x007F00),
            TransferState::Cancelled => Some(0x7F7F7F),
            _ => None,
        };
        r
    }

    fn fill(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let lang = cx.lang;
        let list = cx.client.transfers();
        let keep = self.selected(view);
        self.ids = list.iter().map(|x| x.id).collect();
        let rows: Vec<ListRow> = list.iter().map(|x| Self::row(lang, x)).collect();
        if let Some(Widget::ListView(lv)) = view.widget_mut(LIST) {
            let top = lv.top;
            let same = lv.rows.len() == rows.len() && lv.rows.iter().zip(&rows).all(|(a, b)| a.data == b.data);
            if same {
                for (a, b) in lv.rows.iter_mut().zip(rows) {
                    *a = b;
                }
            } else {
                lv.set_rows(rows);
                lv.top = top.min(lv.rows.len().saturating_sub(1));
                // La nouvelle offre (en bas) est choisie, sinon celle d'avant reste choisie.
                let want = keep.or_else(|| self.ids.last().copied());
                if let Some(i) = want.and_then(|id| self.ids.iter().position(|x| *x == id)) {
                    lv.sel.set(i, true);
                }
            }
        }
        self.details(view, cx);
    }

    fn details(&self, view: &mut DialogView, cx: &mut Cx) {
        let lang = cx.lang;
        let sel = self.selected(view).and_then(|id| cx.client.dcc_entry(id));
        match &sel {
            None => {
                view.set_value(BAR, 0);
                view.set_text(DETAIL, if self.ids.is_empty() { t(lang, "dcc.none") } else { String::new() }.as_str());
            }
            Some(x) => {
                view.set_value(BAR, x.percent().unwrap_or(0) as i32);
                let path = x.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                let s = match &x.error {
                    Some(e) if x.state != TransferState::Done => tf(lang, "dcc.detail.error", &[("error", e), ("path", &path)]),
                    _ => {
                        let eta = x.eta_secs().map(|s| tf(lang, "dcc.detail.eta", &[("eta", &human_secs(s))])).unwrap_or_default();
                        tf(
                            lang,
                            "dcc.detail",
                            &[
                                ("done", &human_size(x.pos)),
                                ("size", &x.size.map(human_size).unwrap_or_else(|| "?".into())),
                                ("speed", &human_size(x.speed)),
                                ("eta", &eta),
                                ("path", &path),
                            ],
                        )
                    }
                };
                view.set_text(DETAIL, &s);
            }
        }
        view.set_enabled(B_CANCEL, sel.as_ref().is_some_and(|x| !x.state.finished()));
        view.set_enabled(B_RESEND, sel.as_ref().is_some_and(|x| x.kind == DccType::Send && x.state.finished()));
    }
}

impl DialogCtl for TransfersDialog {
    fn kind(&self) -> Option<String> {
        Some("dcc.transfers".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let w = WIDTHS.iter().sum::<i32>() + 4 + 24;
        let cols = ["dcc.col.file", "dcc.col.nick", "dcc.col.type", "dcc.col.size", "dcc.col.progress", "dcc.col.speed", "dcc.col.status"]
            .iter()
            .map(|k| t(lang, k))
            .collect::<Vec<_>>()
            .join("\t");
        let bw = 120;
        let by = 12 + 220 + 8 + 16 + 6 + 34 + 8;
        spec(
            &t(lang, "dcc.transfers.title"),
            w,
            by + 24 + 12,
            vec![
                ControlSpec::new(LIST, ControlKind::ListView, (12, 12, w - 24, 220), &cols),
                ControlSpec::new(BAR, ControlKind::Progress, (12, 12 + 220 + 8, w - 24, 16), "").style("smooth"),
                text(DETAIL, (12, 12 + 220 + 8 + 16 + 6, w - 24, 34), ""),
                button(B_FOLDER, (12, by, bw, 24), &t(lang, "dcc.btn.open_folder")),
                button(B_CANCEL, (12 + (bw + 8), by, bw, 24), &t(lang, "dcc.btn.cancel")),
                button(B_RESEND, (12 + 2 * (bw + 8), by, bw, 24), &t(lang, "dcc.btn.resend")),
                button(B_CLEAR, (12 + 3 * (bw + 8), by, bw, 24), &t(lang, "dcc.btn.clear")),
                button(B_CLOSE, (w - 12 - 90, by, 90, 24), &t(lang, "dlg.close")).style("default cancel"),
            ],
        )
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        if let Some(Widget::ListView(lv)) = view.widget_mut(LIST) {
            for (i, (c, w)) in lv.columns.iter_mut().zip(WIDTHS).enumerate() {
                c.width = w;
                if (3..=5).contains(&i) {
                    c.align = Align::Right;
                }
            }
        }
        self.version = cx.client.dcc_version();
        self.fill(view, cx);
        view.set_focus(LIST);
    }

    fn refresh(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let v = cx.client.dcc_version();
        if v != self.version {
            self.version = v;
            self.fill(view, cx);
        }
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        let sel = self.selected(view);
        match msg {
            DialogMsg::SelectionChanged(LIST) => self.details(view, cx),
            DialogMsg::Clicked(B_FOLDER) | DialogMsg::DoubleClicked(LIST) => {
                let dir = sel
                    .and_then(|id| cx.client.dcc_entry(id))
                    .and_then(|x| x.path)
                    .and_then(|p| p.parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| cx.client.dcc_folder());
                let _ = std::fs::create_dir_all(&dir);
                self.reqs.borrow_mut().push(UiReq::OpenFolder(dir));
            }
            DialogMsg::Clicked(B_CANCEL) => {
                if let Some(id) = sel {
                    cx.client.dcc_cancel(id, cx.now);
                }
            }
            DialogMsg::Clicked(B_RESEND) => {
                if let Some(id) = sel
                    && let Err(e) = cx.client.dcc_resend(id, cx.now)
                {
                    view.set_text(DETAIL, e.trim_start_matches("* "));
                }
            }
            DialogMsg::Clicked(B_CLEAR) => cx.client.dcc_clear_finished(),
            DialogMsg::Clicked(B_CLOSE) | DialogMsg::Cancel | DialogMsg::Ok(_) => return Outcome::Close,
            _ => {}
        }
        self.refresh(view, cx);
        Outcome::Keep
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Envoyer un fichier.

struct SendDialog {
    conn: Option<ConnId>,
    nick: String,
    file: String,
    reqs: Rc<RefCell<Vec<UiReq>>>,
    picked: Rc<RefCell<Option<PathBuf>>>,
}

impl DialogCtl for SendDialog {
    fn kind(&self) -> Option<String> {
        Some("dcc.send".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let w = 440;
        spec(
            &t(lang, "dcc.send.title"),
            w,
            12 + 22 + 10 + 22 + 10 + 32 + 8 + 24 + 12,
            vec![
                text(1, (12, 15, 80, 16), &t(lang, "dcc.send.nick")),
                edit(2, (96, 12, 160, 22), &self.nick),
                text(3, (12, 47, 80, 16), &t(lang, "dcc.send.file")),
                edit(4, (96, 44, w - 96 - 12 - 100, 22), &self.file),
                button(5, (w - 12 - 92, 43, 92, 24), &t(lang, "dcc.send.browse")),
                text(8, (12, 76, w - 24, 32), ""),
                button(6, (w - 12 - 2 * 92 - 8, 116, 92, 24), &t(lang, "dcc.send.send")).style("default"),
                button(7, (w - 12 - 92, 116, 92, 24), &t(lang, "dlg.cancel")).style("cancel"),
            ],
        )
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        view.set_focus(if self.nick.is_empty() { 2 } else { 4 });
    }

    fn refresh(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        if let Some(p) = self.picked.borrow_mut().take() {
            view.set_text(4, &p.display().to_string());
            view.set_focus(6);
        }
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Clicked(5) => self.reqs.borrow_mut().push(UiReq::PickForDialog),
            DialogMsg::Clicked(6) | DialogMsg::Ok(_) => {
                let nick = view.text(2).unwrap_or_default().trim().to_string();
                let file = view.text(4).unwrap_or_default().trim().trim_matches('"').to_string();
                if nick.is_empty() || file.is_empty() {
                    view.set_text(8, &cx.t("dcc.send.need"));
                    return Outcome::Keep;
                }
                let Some(conn) =
                    self.conn.filter(|c| cx.client.conn_info(*c).is_some_and(|i| i.state != alpirc_core::ConnState::Disconnected))
                else {
                    view.set_text(8, &cx.t("dcc.noconn"));
                    return Outcome::Keep;
                };
                let path = match file.strip_prefix("~/").zip(std::env::var_os("HOME")) {
                    Some((rest, home)) => PathBuf::from(home).join(rest),
                    None => PathBuf::from(&file),
                };
                match cx.client.dcc_send_file(conn, &nick, &path, cx.now) {
                    Ok(_) => return Outcome::Close,
                    Err(e) => view.set_text(8, e.trim_start_matches("* ")),
                }
            }
            DialogMsg::Clicked(7) | DialogMsg::Cancel => return Outcome::Close,
            _ => {}
        }
        Outcome::Keep
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Discussion DCC.

struct ChatDialog {
    conn: Option<ConnId>,
    nick: String,
}

impl DialogCtl for ChatDialog {
    fn kind(&self) -> Option<String> {
        Some("dcc.chat".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let w = 320;
        spec(
            &t(lang, "dcc.chat.title"),
            w,
            12 + 22 + 8 + 32 + 8 + 24 + 12,
            vec![
                text(1, (12, 15, 80, 16), &t(lang, "dcc.send.nick")),
                edit(2, (96, 12, w - 96 - 12, 22), &self.nick),
                text(5, (12, 42, w - 24, 32), ""),
                button(3, (w - 12 - 2 * 92 - 8, 82, 92, 24), &t(lang, "dcc.chat.chat")).style("default"),
                button(4, (w - 12 - 92, 82, 92, 24), &t(lang, "dlg.cancel")).style("cancel"),
            ],
        )
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        view.set_focus(2);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Clicked(3) | DialogMsg::Ok(_) => {
                let nick = view.text(2).unwrap_or_default().trim().trim_start_matches('=').to_string();
                if nick.is_empty() {
                    view.set_text(5, &cx.t("dcc.chat.need"));
                    return Outcome::Keep;
                }
                let Some(conn) = self.conn.filter(|c| cx.client.conn_info(*c).is_some()) else {
                    view.set_text(5, &cx.t("dcc.noconn"));
                    return Outcome::Keep;
                };
                match cx.client.dcc_chat(conn, &nick, cx.now) {
                    Ok(()) => return Outcome::Close,
                    Err(e) => view.set_text(5, e.trim_start_matches("* ")),
                }
            }
            DialogMsg::Clicked(4) | DialogMsg::Cancel => return Outcome::Close,
            _ => {}
        }
        Outcome::Keep
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Offre reçue.

const O_ACCEPT: u32 = 2;
const O_RESUME: u32 = 3;
const O_OVERWRITE: u32 = 4;
const O_RENAME: u32 = 5;
const O_IGNORE: u32 = 6;

struct OfferDialog {
    prompt: DccPrompt,
}

impl OfferDialog {
    fn body(&self, lang: Lang) -> String {
        let p = &self.prompt;
        let network = if p.network.is_empty() { "IRC".to_string() } else { p.network.clone() };
        if p.chat {
            return tf(lang, "dcc.offer.chat_text", &[("nick", &p.nick), ("network", &network)]);
        }
        let size = p.size.map(human_size).unwrap_or_else(|| "?".into());
        let mut s = tf(lang, "dcc.offer.text", &[("nick", &p.nick), ("network", &network), ("file", &p.file), ("size", &size)]);
        if p.dangerous {
            s.push_str("\n\n");
            s.push_str(&tf(lang, "dcc.offer.danger", &[("nick", &p.nick)]));
        }
        if let Some(e) = p.existing {
            s.push_str("\n\n");
            s.push_str(&tf(lang, "dcc.offer.exists", &[("size", &human_size(e))]));
        }
        if let Some(dir) = p.path.as_ref().and_then(|x| x.parent()) {
            s.push_str("\n\n");
            s.push_str(&tf(lang, "dcc.offer.saveto", &[("path", &dir.display().to_string())]));
        }
        s
    }
}

impl DialogCtl for OfferDialog {
    fn kind(&self) -> Option<String> {
        Some(format!("dcc.offer.{}", self.prompt.id))
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let w = 440;
        let body = self.body(lang);
        // Lignes de texte (environ 56 caractères par ligne sur cette largeur), avec de la marge.
        let rows: i32 = body.lines().map(|l| (l.chars().count().max(1) as i32 + 55) / 56).sum::<i32>().max(1);
        let th = rows * 17 + 6;
        let by = 16 + th + 14;
        let title = if self.prompt.chat {
            tf(lang, "dcc.offer.chat_title", &[("nick", &self.prompt.nick)])
        } else {
            tf(lang, "dcc.offer.title", &[("nick", &self.prompt.nick)])
        };
        let mut icon = ControlSpec::new(
            9,
            ControlKind::Icon,
            (14, 16, 32, 32),
            if self.prompt.chat { "toolbar/dcc_chat" } else { "toolbar/dcc_send" },
        );
        icon.styles.push("noborder".into());
        let mut c = vec![icon, text(1, (56, 16, w - 56 - 12, th), &body)];
        let mut buttons: Vec<(u32, &str)> = Vec::new();
        if self.prompt.existing.is_some() && !self.prompt.chat {
            if self.prompt.can_resume() {
                buttons.push((O_RESUME, "dcc.offer.resume"));
            }
            buttons.push((O_OVERWRITE, "dcc.offer.overwrite"));
            buttons.push((O_RENAME, "dcc.offer.rename"));
        } else {
            buttons.push((O_ACCEPT, "dcc.offer.accept"));
        }
        buttons.push((O_IGNORE, "dcc.offer.ignore"));
        let bw = 92;
        let n = buttons.len() as i32;
        let mut x = w - 12 - n * bw - (n - 1) * 8;
        for (i, (id, key)) in buttons.iter().enumerate() {
            let mut b = button(*id, (x, by, bw, 24), &t(lang, key));
            // Pas de bouton par défaut pour un programme : Entrée ne l'accepte pas par mégarde.
            if i == 0 && !self.prompt.dangerous {
                b = b.style("default");
            }
            if *id == O_IGNORE {
                b = b.style("cancel");
            }
            c.push(b);
            x += bw + 8;
        }
        spec(&title, w, by + 24 + 12, c)
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        let first = [O_RESUME, O_ACCEPT, O_OVERWRITE].into_iter().find(|id| view.control_rect(*id).is_some());
        if self.prompt.dangerous {
            view.set_focus(O_IGNORE);
        } else if let Some(id) = first {
            view.set_focus(id);
        }
    }

    fn on(&mut self, msg: DialogMsg, _view: &mut DialogView, cx: &mut Cx) -> Outcome {
        let id = self.prompt.id;
        let answer = match msg {
            DialogMsg::Clicked(O_ACCEPT) => DccAnswer::Accept,
            DialogMsg::Clicked(O_RESUME) => DccAnswer::Resume,
            DialogMsg::Clicked(O_OVERWRITE) => DccAnswer::Overwrite,
            DialogMsg::Clicked(O_RENAME) => DccAnswer::Rename,
            DialogMsg::Ok(b) if b != O_IGNORE => DccAnswer::Accept,
            DialogMsg::Clicked(O_IGNORE) | DialogMsg::Cancel | DialogMsg::Ok(_) => {
                cx.client.dcc_reject(id, cx.now);
                return Outcome::Close;
            }
            _ => return Outcome::Keep,
        };
        cx.client.dcc_accept(id, answer, cx.now);
        Outcome::Close
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_tags() {
        let mut d = DccShell::default();
        let r = d.new_pick(Pick::Dialog);
        assert!(is_dcc_pick(r));
        assert!(!is_dcc_pick(1));
        assert!(!is_dcc_pick(42 << 20));
    }

    #[test]
    fn durations() {
        assert_eq!(human_secs(5), "5 s");
        assert_eq!(human_secs(125), "2 min");
        assert_eq!(human_secs(3720), "1h02");
    }
}
