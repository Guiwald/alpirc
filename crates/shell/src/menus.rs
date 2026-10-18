//! La barre de menus d'AlpIRC (dessinée dans la fenêtre sous Linux et Android, native sur macOS) et la
//! conversion des menus des scripts.

use alpirc_core::i18n::Lang;
use alpirc_core::{Client, WinId};
use alpirc_ui::MenuItem;

use crate::tr::t;

/// Identifiants des commandes des menus.
pub mod ids {
    pub const NEW_STATUS: u32 = 1;
    pub const CONNECT: u32 = 2;
    pub const DISCONNECT: u32 = 3;
    pub const SELECT_SERVER: u32 = 4;
    pub const OPTIONS: u32 = 5;
    pub const EXIT: u32 = 6;
    /// Absent / de retour, sur la connexion active.
    pub const AWAY: u32 = 7;
    pub const TOOLBAR: u32 = 10;
    pub const SWITCHBAR: u32 = 11;
    pub const TREEBAR: u32 = 12;
    pub const NICKLIST: u32 = 13;
    pub const THEME_CLASSIC: u32 = 14;
    pub const THEME_DARK: u32 = 15;
    pub const FONT_BIGGER: u32 = 16;
    pub const FONT_SMALLER: u32 = 17;
    pub const FIND: u32 = 18;
    pub const CLEAR: u32 = 19;
    pub const FAV_FOLDER: u32 = 20;
    pub const FAV_ADD: u32 = 21;
    pub const ABOOK: u32 = 30;
    pub const COLORS: u32 = 31;
    pub const SCRIPTS: u32 = 32;
    pub const CHANLIST: u32 = 33;
    pub const URLS: u32 = 34;
    pub const NOTIFY: u32 = 35;
    pub const DCC_SEND: u32 = 36;
    pub const DCC_CHAT: u32 = 37;
    pub const TRANSFERS: u32 = 38;
    pub const LOGS: u32 = 39;
    pub const CASCADE: u32 = 50;
    pub const TILE_H: u32 = 51;
    pub const TILE_V: u32 = 52;
    pub const ARRANGE: u32 = 53;
    pub const NEXT: u32 = 54;
    pub const CLOSE: u32 = 55;
    pub const HELP: u32 = 60;
    pub const ABOUT: u32 = 61;
    /// Favoris : `FAV_BASE + index`.
    pub const FAV_BASE: u32 = 1_000;
    /// Fenêtres : `WIN_BASE + id`.
    pub const WIN_BASE: u32 = 10_000;
    /// Menu « Commandes » des scripts : `SCRIPT_BASE + action`.
    pub const SCRIPT_BASE: u32 = 1_000_000;
}

/// Ce qu'il faut savoir de l'état de l'interface pour cocher les menus.
pub struct MenuState {
    pub toolbar: bool,
    pub switchbar: bool,
    pub treebar: bool,
    pub nicklist: bool,
    pub dark: bool,
    pub connected: bool,
    /// La connexion active est marquée absente.
    pub away: bool,
    pub native: bool,
}

/// Raccourci affiché : Cmd sur Mac, Ctrl ailleurs ; Alt reste Alt (Option sur Mac).
fn sc(native_mac: bool, s: &str) -> String {
    if native_mac { s.replace("Ctrl+", "⌘").replace("Alt+", "⌥") } else { s.to_string() }
}

/// Convertit les menus d'un script en menus de la boîte à outils ; `base` est ajouté aux numéros d'action.
pub fn from_script(items: &[alpirc_script::MenuItem], base: u32) -> Vec<MenuItem> {
    items
        .iter()
        .map(|it| {
            if it.label.trim() == "-" {
                return MenuItem::separator();
            }
            if !it.children.is_empty() {
                let mut m = MenuItem::submenu(&it.label, from_script(&it.children, base));
                m.enabled = it.enabled;
                return m;
            }
            let mut m = MenuItem::new(it.action.map(|a| base + a).unwrap_or(0), &it.label).checked(it.checked);
            m.enabled = it.enabled && it.action.is_some();
            m
        })
        .collect()
}

/// La barre de menus complète.
/// Chemin (index à chaque niveau) de l'entrée d'action `action`.
pub fn path_of(items: &[alpirc_script::MenuItem], action: u32) -> Option<Vec<usize>> {
    for (i, it) in items.iter().enumerate() {
        if it.action == Some(action) {
            return Some(vec![i]);
        }
        if let Some(mut p) = path_of(&it.children, action) {
            p.insert(0, i);
            return Some(p);
        }
    }
    None
}

/// Action de l'entrée au chemin donné.
pub fn action_at(items: &[alpirc_script::MenuItem], path: &[usize]) -> Option<u32> {
    let (first, rest) = path.split_first()?;
    let it = items.get(*first)?;
    if rest.is_empty() { it.action } else { action_at(&it.children, rest) }
}

/// La barre de menus complète, et l'arbre du menu « Commandes » des scripts (pour retrouver ses entrées).
pub fn menubar(client: &mut Client, active: Option<WinId>, st: &MenuState) -> (Vec<(String, Vec<MenuItem>)>, Vec<alpirc_script::MenuItem>) {
    use ids::*;
    let lang: Lang = client.language();
    let tr = |k: &str| t(lang, k);
    let m = |id: u32, k: &str| MenuItem::new(id, &t(lang, k));
    let mac = st.native;

    let mut file = vec![
        m(NEW_STATUS, "file.new_status").shortcut(&sc(mac, "Ctrl+N")),
        MenuItem::separator(),
        m(CONNECT, "file.connect"),
        m(DISCONNECT, "file.disconnect"),
        m(SELECT_SERVER, "file.select_server"),
        MenuItem::separator(),
        m(AWAY, "file.away").checked(st.away),
    ];
    if !mac {
        file.push(MenuItem::separator());
        file.push(m(OPTIONS, "file.options").shortcut("Alt+O"));
        file.push(MenuItem::separator());
        file.push(m(EXIT, "file.exit"));
    }
    if st.connected {
        file[3].enabled = true;
    } else {
        file[3].enabled = false;
    }
    file[6].enabled = st.connected;

    let view = vec![
        m(TOOLBAR, "view.toolbar").checked(st.toolbar),
        m(SWITCHBAR, "view.switchbar").checked(st.switchbar),
        m(TREEBAR, "view.treebar").checked(st.treebar),
        m(NICKLIST, "view.nicklist").checked(st.nicklist),
        MenuItem::separator(),
        MenuItem::submenu(&tr("view.theme"), vec![m(THEME_CLASSIC, "view.theme.classic").radio(!st.dark), m(THEME_DARK, "view.theme.dark").radio(st.dark)]),
        m(FONT_BIGGER, "view.font_bigger").shortcut(&sc(mac, "Ctrl++")),
        m(FONT_SMALLER, "view.font_smaller").shortcut(&sc(mac, "Ctrl+-")),
        MenuItem::separator(),
        m(FIND, "view.find").shortcut(&sc(mac, "Ctrl+F")),
        m(CLEAR, "view.clear"),
    ];

    let mut fav = vec![m(FAV_FOLDER, "fav.folder").shortcut("Alt+J"), m(FAV_ADD, "fav.add"), MenuItem::separator()];
    let favs = &client.settings().favorites;
    if favs.is_empty() {
        let mut e = MenuItem::new(0, &tr("menu.empty"));
        e.enabled = false;
        fav.push(e);
    }
    for (i, f) in favs.iter().enumerate().take(40) {
        let label = if f.network.is_empty() { f.channel.clone() } else { format!("{}  ({})", f.channel, f.network) };
        fav.push(MenuItem::new(FAV_BASE + i as u32, &label.replace('&', "&&")));
    }

    let tools = vec![
        m(ABOOK, "tools.abook").shortcut("Alt+B"),
        m(COLORS, "tools.colors").shortcut("Alt+K"),
        m(SCRIPTS, "tools.scripts").shortcut("Alt+R"),
        MenuItem::separator(),
        m(CHANLIST, "tools.chanlist"),
        m(URLS, "tools.urls"),
        m(NOTIFY, "tools.notify"),
        MenuItem::separator(),
        m(DCC_SEND, "tools.dcc_send"),
        m(DCC_CHAT, "tools.dcc_chat"),
        m(TRANSFERS, "tools.transfers"),
        MenuItem::separator(),
        m(LOGS, "tools.logs"),
    ];

    let commands_tree = match active {
        Some(w) => client.popup(alpirc_script::PopupKind::Menubar, w, &[]),
        None => Vec::new(),
    };
    let commands = from_script(&commands_tree, SCRIPT_BASE);
    let commands = if commands.is_empty() {
        let mut e = MenuItem::new(0, &tr("menu.empty"));
        e.enabled = false;
        vec![e]
    } else {
        commands
    };

    let mut window = vec![
        m(CASCADE, "win.cascade"),
        m(TILE_H, "win.tile_h"),
        m(TILE_V, "win.tile_v"),
        m(ARRANGE, "win.arrange"),
        MenuItem::separator(),
        m(NEXT, "win.next").shortcut(&sc(mac, "Ctrl+Tab")),
        m(CLOSE, "win.close").shortcut(&sc(mac, "Ctrl+W")),
        MenuItem::separator(),
    ];
    for w in client.windows().iter().take(60) {
        let label = crate::views::short_label(w, client);
        window.push(MenuItem::new(WIN_BASE + w.id, &label.replace('&', "&&")).radio(Some(w.id) == active));
    }

    let mut help = vec![m(HELP, "help.readme").shortcut("F1")];
    if !mac {
        help.push(MenuItem::separator());
        help.push(m(ABOUT, "help.about"));
    }

    (vec![
        (tr("menu.file"), file),
        (tr("menu.view"), view),
        (tr("menu.favorites"), fav),
        (tr("menu.tools"), tools),
        (tr("menu.commands"), commands),
        (tr("menu.window"), window),
        (tr("menu.help"), help),
    ], commands_tree)
}
