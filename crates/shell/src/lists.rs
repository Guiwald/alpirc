//! Menus du clic droit (appui long sur le téléphone) des fenêtres à liste de mIRC : la liste des canaux (rejoindre,
//! ajouter aux favoris, copier le nom, refaire la liste) et le capteur d'URL (ouvrir, copier, retirer, tout effacer).
//! Et l'enregistrement du texte d'une fenêtre (menu de la fenêtre, comme Buffer › Save de mIRC).

use alpirc_core::{WinId, WinKind};
use alpirc_ui::{Menu, MenuItem, Point};

use crate::tr::t;
use crate::{PopupSrc, Shell, ShellPlatform};

/// Entrées des menus.
pub(crate) mod ids {
    pub const JOIN: u32 = 1;
    pub const FAVORITE: u32 = 2;
    pub const COPY: u32 = 3;
    pub const REFRESH: u32 = 4;
    pub const OPEN: u32 = 11;
    pub const REMOVE: u32 = 13;
    pub const CLEAR: u32 = 14;
}

/// Bit des demandes de fichier « enregistrer le texte d'une fenêtre » (les boîtes ont le bit 62, le DCC le 63).
const BUFFER_BIT: u64 = 1 << 61;

/// Numéro de demande de fichier ([`ShellPlatform::pick_file`]) pour enregistrer le texte de `win`.
pub(crate) fn buffer_request(win: WinId) -> u64 {
    BUFFER_BIT | win as u64
}

/// La fenêtre d'une demande « enregistrer le texte », si c'en est une.
pub(crate) fn buffer_window(request: u64) -> Option<WinId> {
    (request >> 61 == 1).then_some((request & (BUFFER_BIT - 1)) as WinId)
}

/// Textes des menus des listes (voir [`crate::tr`]).
pub const TR: crate::tr::Table = &[
    ("list.join", ["&Join", "&Rejoindre", "&Entrar", "&Betreten", "&Entra", "&Entrar", "&Téigh isteach"]),
    (
        "list.favorite",
        [
            "Add to &Favorites",
            "Ajouter aux &favoris",
            "Añadir a &favoritos",
            "Zu &Favoriten hinzufügen",
            "Aggiungi ai &preferiti",
            "Afegir als &preferits",
            "Cuir le &ceanáin",
        ],
    ),
    ("list.copy", ["&Copy", "&Copier", "&Copiar", "&Kopieren", "&Copia", "&Copiar", "Cói&peáil"]),
    (
        "list.refresh",
        [
            "&List channels...",
            "&Lister les canaux...",
            "&Listar canales...",
            "Channels &auflisten...",
            "Elenca i ca&nali...",
            "&Llistar canals...",
            "&Liostaigh cainéil...",
        ],
    ),
    ("list.open", ["&Open", "&Ouvrir", "&Abrir", "Ö&ffnen", "&Apri", "&Obrir", "&Oscail"]),
    ("list.remove", ["&Remove", "&Retirer", "&Quitar", "&Entfernen", "&Rimuovi", "&Llevar", "&Bain"]),
    (
        "list.clear",
        ["C&lear list", "&Tout effacer", "&Vaciar la lista", "Liste &leeren", "&Svuota la lista", "&Buidar la llista", "Fol&mhaigh an liosta"],
    ),
];

impl Shell {
    /// Les entrées du menu d'une fenêtre à liste (aussi pour le menu au toucher du téléphone) ; `on_row` : une
    /// ligne est sous le pointeur.
    pub(crate) fn list_menu_items(&self, kind: WinKind, on_row: bool) -> Vec<MenuItem> {
        let lang = self.language();
        let row = |id: u32, k: &str| {
            let mut m = MenuItem::new(id, &t(lang, k));
            m.enabled = on_row;
            m
        };
        match kind {
            WinKind::ChanList => vec![
                row(ids::JOIN, "list.join"),
                row(ids::FAVORITE, "list.favorite"),
                row(ids::COPY, "list.copy"),
                MenuItem::separator(),
                MenuItem::new(ids::REFRESH, &t(lang, "list.refresh")),
            ],
            WinKind::Urls => vec![
                row(ids::OPEN, "list.open"),
                row(ids::COPY, "list.copy"),
                MenuItem::separator(),
                row(ids::REMOVE, "list.remove"),
                MenuItem::new(ids::CLEAR, &t(lang, "list.clear")),
            ],
            _ => Vec::new(),
        }
    }

    /// Clic droit dans la liste des canaux ou le capteur d'URL ; `value` : la première colonne de la ligne visée.
    pub(crate) fn open_list_popup(&mut self, win: WinId, kind: WinKind, value: Option<String>, at: Point) {
        let items = self.list_menu_items(kind, value.is_some());
        if items.is_empty() {
            return;
        }
        let menu = Menu::popup(items, at, self.dialog_bounds(), &self.res);
        self.popup = Some((menu, PopupSrc::List { win, kind, value }));
    }

    /// Une entrée du menu choisie.
    pub(crate) fn list_popup_run(&mut self, id: u32, win: WinId, value: Option<String>, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        match (id, value) {
            (ids::JOIN, Some(chan)) => self.client.command(win, &format!("/join {chan}"), now),
            (ids::FAVORITE, Some(chan)) => {
                let net = self.client.window(win).and_then(|w| w.conn).and_then(|c| self.client.conn_info(c)).map(|c| c.network).unwrap_or_default();
                let favs = &mut self.client.settings_mut().favorites;
                if !favs.iter().any(|f| f.channel.eq_ignore_ascii_case(&chan) && f.network.eq_ignore_ascii_case(&net)) {
                    favs.push(alpirc_core::settings::Favorite { channel: chan, network: net, key: None, description: String::new(), join_on_connect: false });
                    self.settings_changed(p);
                }
            }
            (ids::COPY, Some(v)) => p.clipboard_set(&v),
            (ids::REFRESH, _) => self.menu_action(crate::menus::ids::CHANLIST, p),
            (ids::OPEN, Some(url)) => p.open_url(&url),
            (ids::REMOVE, Some(url)) => self.client.remove_url(Some(&url)),
            (ids::CLEAR, _) => self.client.remove_url(None),
            _ => {}
        }
    }
}
