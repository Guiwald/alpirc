//! La fenêtre Notify (liste des amis et leur état, tenue à jour par le moteur) : double-clic sur un pseudo pour
//! ouvrir un privé, comme dans mIRC ; clic droit pour Whois, Privé ou Retirer de la liste. Les commandes partent
//! sur la connexion où le pseudo est en ligne (sinon la première connectée), leurs réponses dans sa fenêtre
//! Status : la fenêtre Notify n'a pas de connexion à elle.

use alpirc_core::{ConnId, ConnState, WinId};
use alpirc_ui::{Menu, MenuItem, Point};

use crate::tr::t;
use crate::{PopupSrc, Shell, ShellPlatform};

/// Entrées du menu de la fenêtre Notify.
pub(crate) mod ids {
    pub const WHOIS: u32 = 1;
    pub const QUERY: u32 = 2;
    pub const REMOVE: u32 = 3;
}

/// Textes de la fenêtre Notify (voir [`crate::tr`]).
pub const TR: crate::tr::Table = &[
    ("notify.menu.whois", ["&Whois", "&Whois", "&Whois", "&Whois", "&Whois", "&Whois", "&Whois"]),
    ("notify.menu.query", ["&Query", "&Privé", "&Privado", "&Query", "&Query", "&Privat", "&Comhrá"]),
    (
        "notify.menu.remove",
        [
            "&Remove from list",
            "&Retirer de la liste",
            "&Quitar de la lista",
            "Aus der Liste &entfernen",
            "&Rimuovi dalla lista",
            "&Llevar de la llista",
            "&Bain den liosta",
        ],
    ),
];

impl Shell {
    /// Fenêtre Status d'où lancer une commande pour un pseudo de la liste Notify.
    fn notify_status_for(&self, nick: &str) -> Option<WinId> {
        let list = self.client.notify_list();
        let online = list
            .iter()
            .find(|n| n.nick.eq_ignore_ascii_case(nick))
            .and_then(|e| e.online.iter().find(|(_, o)| *o == Some(true)).map(|(c, _)| *c));
        let connected = |c: &ConnId| self.client.conn_info(*c).is_some_and(|i| i.state == ConnState::Connected);
        let conn = online.or_else(|| self.client.conns().into_iter().find(connected));
        conn.and_then(|c| self.client.conn_info(c)).map(|i| i.status_window)
    }

    /// Lance `line` pour `nick` depuis la bonne fenêtre Status (la fenêtre Notify si aucune connexion).
    fn notify_command(&mut self, notify_win: WinId, nick: &str, line: &str, now: i64) {
        let target = self.notify_status_for(nick).unwrap_or(notify_win);
        self.client.command(target, line, now);
    }

    /// Double-clic sur un pseudo : un privé avec lui.
    pub(crate) fn notify_double_click(&mut self, win: WinId, nick: &str, now: i64) {
        self.notify_command(win, nick, &format!("/query {nick}"), now);
    }

    /// Les entrées du menu (aussi pour le menu au toucher du téléphone).
    pub(crate) fn notify_menu_items(&self) -> Vec<MenuItem> {
        let lang = self.language();
        vec![
            MenuItem::new(ids::WHOIS, &t(lang, "notify.menu.whois")),
            MenuItem::new(ids::QUERY, &t(lang, "notify.menu.query")),
            MenuItem::separator(),
            MenuItem::new(ids::REMOVE, &t(lang, "notify.menu.remove")),
        ]
    }

    /// Clic droit sur un pseudo de la fenêtre Notify.
    pub(crate) fn open_notify_popup(&mut self, win: WinId, nick: String, at: Point) {
        let menu = Menu::popup(self.notify_menu_items(), at, self.dialog_bounds(), &self.res);
        self.popup = Some((menu, PopupSrc::Notify { win, nick }));
    }

    /// Une entrée du menu choisie.
    pub(crate) fn notify_popup_run(&mut self, id: u32, win: WinId, nick: &str, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        let line = match id {
            ids::WHOIS => format!("/whois {nick}"),
            ids::QUERY => format!("/query {nick}"),
            ids::REMOVE => format!("/notify -r {nick}"),
            _ => return,
        };
        self.notify_command(win, nick, &line, now);
    }
}
