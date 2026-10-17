//! Fenêtres et lignes, comme dans mIRC : Status, canaux, privés, fenêtres `@` des scripts, liste des canaux.

use std::collections::VecDeque;

use super::{ConnId, WinId};

/// Sorte de fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WinKind {
    Status,
    Channel,
    Query,
    /// DCC CHAT (jalon DCC).
    DccChat,
    /// Fenêtre `@nom` d'un script.
    Custom,
    /// Résultats de `/list`.
    ChanList,
    Notify,
    Urls,
}

/// Couleur de la fenêtre dans la switchbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Activity {
    #[default]
    None,
    /// Événement (arrivée, départ, mode…).
    Event,
    /// Message.
    Message,
    /// On nous a nommé (ou un mot surligné).
    Highlight,
}

/// Sorte de ligne : donne sa couleur (voir `Settings::color_index`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineKind {
    Normal,
    Action,
    Ctcp,
    Highlight,
    Info,
    Info2,
    Invite,
    Join,
    Kick,
    Mode,
    Nick,
    Notice,
    Notify,
    Other,
    Own,
    Part,
    Quit,
    Topic,
    Wallops,
    Whois,
    Error,
}

/// Une ligne affichée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Numéro unique, croissant dans l'ordre d'arrivée (les lignes d'historique, insérées à leur place selon
    /// l'heure, ont aussi un numéro neuf).
    pub seq: u64,
    /// Heure (ms depuis 1970) : celle du serveur (`server-time`) si elle est connue.
    pub time_ms: i64,
    pub kind: LineKind,
    /// Texte affiché tel quel : horodatage compris (si l'option est active), décorations (`<Guiwald> `),
    /// codes de mise en forme de mIRC gardés.
    pub text: String,
    /// Auteur, pour les messages.
    pub nick: Option<String>,
    pub highlight: bool,
    /// Vient d'un lot `chathistory`.
    pub from_history: bool,
    /// `msgid` du serveur, s'il y en a un (pour ne pas doubler l'historique).
    pub msgid: Option<String>,
    /// Couleur imposée (mot surligné avec sa couleur, `/echo 4 …`) ; sinon celle de `kind`.
    pub color: Option<u8>,
}

/// Une fenêtre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub id: WinId,
    pub kind: WinKind,
    pub conn: Option<ConnId>,
    /// `Status`, `#lyon`, `Guiwald`, `@pic`…
    pub name: String,
    /// Titre à la manière de mIRC.
    pub title: String,
    pub lines: VecDeque<Line>,
    pub activity: Activity,
    /// Numéro de la première ligne non lue (barre de lecture).
    pub unread_marker: Option<u64>,
    /// Lignes tapées (la plus récente à la fin).
    pub history: Vec<String>,
    /// Augmente à chaque changement (lignes, titre, liste des pseudos…).
    pub version: u64,
    /// Fenêtre `@` avec une liste à côté (`/window -l`).
    pub listbox: bool,
    /// Lignes de la liste d'une fenêtre `@` à liste.
    pub list_lines: Vec<String>,
    /// Lignes sélectionnées de la liste (numéros à partir de 1).
    pub sel: Vec<usize>,
    /// Canal : nous y sommes.
    pub joined: bool,
    /// Fenêtre `@` avec une ligne de saisie (`/window -e`).
    pub editbox: bool,
    /// Dernier marqueur de lecture connu (`MARKREAD`), en ms.
    pub read_marker_ms: Option<i64>,
}

impl Window {
    pub(crate) fn new(id: WinId, kind: WinKind, conn: Option<ConnId>, name: &str) -> Window {
        Window {
            id,
            kind,
            conn,
            name: name.to_string(),
            title: name.to_string(),
            lines: VecDeque::new(),
            activity: Activity::None,
            unread_marker: None,
            history: Vec::new(),
            version: 0,
            listbox: false,
            list_lines: Vec::new(),
            sel: Vec::new(),
            joined: false,
            editbox: true,
            read_marker_ms: None,
        }
    }

    /// Heure de la dernière ligne.
    pub fn last_time(&self) -> Option<i64> {
        self.lines.back().map(|l| l.time_ms)
    }

    pub(crate) fn touch(&mut self) {
        self.version += 1;
    }
}

/// Une entrée de la liste des pseudos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NickEntry {
    pub nick: String,
    /// Symboles de préfixe, du plus fort au plus faible (`@+`).
    pub prefixes: String,
    pub away: bool,
    pub account: Option<String>,
}
