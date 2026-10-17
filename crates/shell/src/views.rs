//! Les vues d'une fenêtre du client : discussion, liste des pseudos, ligne de saisie, et les listes des
//! fenêtres spéciales (liste des canaux, adresses attrapées, Notify, fenêtres `@` en liste).

use alpirc_core::{Client, WinId, WinKind, Window};
use alpirc_ui::widgets::{Column, ListView};
use alpirc_ui::widgets::listview::ListRow;
use alpirc_ui::{ChatLine, ChatSource, ChatView, Editbox, IndentMode, NickEntry, NickList, Res};

use crate::tr::t;

/// Les lignes d'une fenêtre du moteur, vues par la vue de discussion.
pub struct WinLines<'a> {
    pub win: &'a Window,
    pub client: &'a Client,
    pub nicks: &'a [NickEntry],
}

impl ChatSource for WinLines<'_> {
    fn len(&self) -> usize {
        self.win.lines.len()
    }

    fn line(&self, i: usize) -> ChatLine<'_> {
        let l = &self.win.lines[i];
        let default_fg = l.color.unwrap_or_else(|| self.client.color_index(l.kind));
        ChatLine { seq: l.seq, text: &l.text, default_fg, highlight: false }
    }

    fn version(&self) -> u64 {
        self.win.version
    }

    fn is_nick(&self, word: &str) -> bool {
        self.nicks.iter().any(|n| n.nick.eq_ignore_ascii_case(word))
    }
}

/// Le composant qui a le focus dans une fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewFocus {
    Editbox,
    Chat,
    Nicks,
    List,
}

/// État d'affichage d'une fenêtre.
pub struct WinView {
    pub id: WinId,
    pub kind: WinKind,
    pub chat: ChatView,
    pub nicks: NickList,
    pub editbox: Editbox,
    pub list: ListView,
    /// Largeur de la liste des pseudos (pixels logiques).
    pub nick_width: i32,
    pub focus: ViewFocus,
    /// Version de la fenêtre lors de la dernière mise à jour de la liste des pseudos / de la liste.
    synced: u64,
    /// Glisser de la séparation discussion / pseudos en cours (x de départ, largeur de départ).
    pub split_drag: Option<(i32, i32)>,
}

impl WinView {
    pub fn new(win: &Window, client: &Client) -> WinView {
        let lang = client.language();
        let mut chat = ChatView::new();
        chat.indent = IndentMode::FirstWord;
        let mut editbox = Editbox::new();
        editbox.focused = true;
        editbox.history = win.history.clone();
        let list = match win.kind {
            WinKind::ChanList => ListView::new(vec![
                Column::new(&t(lang, "chanlist.name"), 160),
                Column::new(&t(lang, "chanlist.users"), 50),
                Column::new(&t(lang, "chanlist.topic"), 600),
            ]),
            WinKind::Urls => ListView::new(vec![
                Column::new(&t(lang, "urls.url"), 360),
                Column::new(&t(lang, "urls.nick"), 100),
                Column::new(&t(lang, "urls.where"), 110),
                Column::new(&t(lang, "urls.time"), 120),
            ]),
            WinKind::Notify => ListView::new(vec![
                Column::new(&t(lang, "notify.nick"), 140),
                Column::new(&t(lang, "notify.state"), 90),
                Column::new(&t(lang, "notify.note"), 300),
            ]),
            _ => ListView::new(vec![Column::new("", 2000)]),
        };
        let focus = if has_list(win) { ViewFocus::List } else { ViewFocus::Editbox };
        WinView {
            id: win.id,
            kind: win.kind,
            chat,
            nicks: NickList::new(),
            editbox,
            list,
            nick_width: 130,
            focus,
            synced: u64::MAX,
            split_drag: None,
        }
    }

    /// Met à jour la liste des pseudos et les listes depuis le moteur, si la fenêtre a changé.
    pub fn sync(&mut self, win: &Window, client: &Client, utc_offset: i32) {
        if self.synced == win.version && !matches!(win.kind, WinKind::Urls | WinKind::Notify | WinKind::ChanList) {
            return;
        }
        self.synced = win.version;
        match win.kind {
            WinKind::Channel => {
                let entries: Vec<NickEntry> = client
                    .nicklist(win.id)
                    .into_iter()
                    .map(|n| {
                        let mut e = NickEntry::new(&n.prefixes.chars().next().map(String::from).unwrap_or_default(), &n.nick);
                        e.away = n.away;
                        e
                    })
                    .collect();
                if entries.len() != self.nicks.entries().len()
                    || entries.iter().zip(self.nicks.entries()).any(|(a, b)| a.nick != b.nick || a.prefix != b.prefix || a.away != b.away)
                {
                    self.nicks.set_entries(entries);
                }
            }
            WinKind::ChanList => {
                if let Some(cl) = client.chanlist(win.id) {
                    if cl.entries.len() != self.list.rows.len() {
                        let rows: Vec<ListRow> = cl
                            .entries
                            .iter()
                            .map(|e| ListRow::new(&[e.name.clone(), e.users.to_string(), alpirc_proto::format::strip(&e.topic)]))
                            .collect();
                        self.list.set_rows(rows);
                        if self.list.sort.is_some() {
                            self.list.sort_rows();
                        }
                    }
                }
            }
            WinKind::Urls => {
                let urls = client.urls();
                if urls.len() != self.list.rows.len() {
                    let rows: Vec<ListRow> = urls
                        .iter()
                        .rev()
                        .map(|u| ListRow::new(&[u.url.clone(), u.nick.clone(), u.window.clone(), alpirc_core::timeutil::format_time(u.time_ms, utc_offset, "dd/mm HH:nn")]))
                        .collect();
                    self.list.set_rows(rows);
                }
            }
            WinKind::Notify => {
                let lang = client.language();
                let rows: Vec<ListRow> = client
                    .notify_list()
                    .iter()
                    .map(|n| {
                        let on = n.online.iter().any(|(_, o)| *o == Some(true));
                        let state = if on { t(lang, "notify.online") } else { t(lang, "notify.offline") };
                        ListRow::new(&[n.nick.clone(), state, n.note.clone()])
                    })
                    .collect();
                if rows.len() != self.list.rows.len() || rows.iter().zip(&self.list.rows).any(|(a, b)| a.cells != b.cells) {
                    self.list.set_rows(rows);
                }
            }
            WinKind::Custom if win.listbox => {
                if win.list_lines.len() != self.list.rows.len()
                    || win.list_lines.iter().zip(&self.list.rows).any(|(a, b)| b.cells.first() != Some(a))
                {
                    let rows: Vec<ListRow> = win.list_lines.iter().map(|l| ListRow::new(&[alpirc_proto::format::strip(l)])).collect();
                    self.list.set_rows(rows);
                }
            }
            _ => {}
        }
    }

    /// Vrai si la fenêtre a une liste des pseudos visible.
    pub fn has_nicklist(&self, show: bool) -> bool {
        self.kind == WinKind::Channel && show
    }

    pub fn set_focus(&mut self, f: ViewFocus) {
        self.focus = f;
        self.editbox.focused = f == ViewFocus::Editbox;
        self.list.focused = f == ViewFocus::List;
    }
}

/// Vrai si la fenêtre affiche une liste plutôt qu'une discussion.
pub fn has_list(win: &Window) -> bool {
    matches!(win.kind, WinKind::ChanList | WinKind::Urls | WinKind::Notify) || (win.kind == WinKind::Custom && win.listbox)
}

/// Vrai si la fenêtre a une ligne de saisie.
pub fn has_editbox(win: &Window) -> bool {
    match win.kind {
        WinKind::Status | WinKind::Channel | WinKind::Query | WinKind::DccChat => true,
        WinKind::Custom => win.editbox,
        _ => false,
    }
}

/// Nom de l'icône (dans le registre) d'une sorte de fenêtre.
pub fn icon_name(win: &Window, connected: bool) -> &'static str {
    match win.kind {
        WinKind::Status if connected => "window/network_connected",
        WinKind::Status => "window/network_disconnected",
        WinKind::Channel => "window/channel",
        WinKind::Query => "window/query",
        WinKind::DccChat => "window/dcc_chat",
        WinKind::Custom => "window/custom",
        WinKind::ChanList => "window/chanlist",
        WinKind::Notify => "window/notify",
        WinKind::Urls => "window/urls",
    }
}

/// Libellé court (switchbar, treebar) d'une fenêtre.
pub fn short_label(win: &Window, client: &Client) -> String {
    let lang = client.language();
    match win.kind {
        WinKind::Status => match win.conn.and_then(|c| client.conn_info(c)) {
            Some(ci) if !ci.network.is_empty() && ci.state != alpirc_core::ConnState::Disconnected => ci.network.clone(),
            _ => "Status".to_string(),
        },
        WinKind::ChanList => t(lang, "win.chanlist"),
        WinKind::Urls => t(lang, "win.urls"),
        WinKind::Notify => t(lang, "win.notify"),
        _ => win.name.clone(),
    }
}

/// Résolution des icônes avec repli.
pub fn icon(res: &Res, name: &str) -> Option<alpirc_ui::IconId> {
    res.icons.get(name).or_else(|| {
        let fallback = name.rsplit('/').next().unwrap_or(name);
        res.icons.get(fallback)
    })
}
