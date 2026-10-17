//! `alpirc-shell` : l'application AlpIRC sans plateforme.
//!
//! Le [`Shell`] relie le moteur client (`alpirc-core`), le réseau (`alpirc-net`) et la boîte à outils
//! (`alpirc-ui`) : il tient les vues de chaque fenêtre, la disposition façon mIRC (barre de menus, barre
//! d'outils, switchbar, treebar, fenêtres internes), les menus surgissants, les boîtes de dialogue, et applique
//! les effets du moteur. L'application de bureau (winit) et celle d'Android ne fournissent que la fenêtre du
//! système, les événements et quelques services ([`ShellPlatform`]).

pub mod assets;
pub mod dcc;
pub mod dialogs;
mod layout;
pub mod menus;
mod mobile;
pub mod tr;
pub mod views;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use alpirc_core::i18n::Lang;
use alpirc_core::{Activity as CoreActivity, Client, ConnState, Effect, Settings, UiDialog, WinId, WinKind};
use alpirc_net::{Net, NetEvent, Target, Verify};
use alpirc_ui::{
    Activity, DialogMsg, DialogView, MdiArea, MdiChild, MdiResponse, Menu, MenuBar, MenuItem,
    ModalStack, Platform, Point, Rect, Res, Size, Switchbar, Theme, ToolItem, Toolbar, Treebar,
};

pub use dialogs::{Action, DialogCtl, Outcome};
pub use layout::Layout;
use menus::ids;
use tr::t;
use views::{WinView, icon, short_label};

/// Services de la plateforme dont l'application a besoin, en plus de ceux des widgets.
///
/// `now_ms` (hérité de [`Platform`]) doit être l'heure réelle en millisecondes depuis 1970.
pub trait ShellPlatform: Platform {
    /// Notification du système.
    fn notify(&mut self, title: &str, body: &str);
    fn beep(&mut self);
    fn play_sound(&mut self, path: &Path);
    /// Attirer l'attention (icône qui rebondit, fenêtre qui clignote).
    fn flash(&mut self);
    fn open_url(&mut self, url: &str);
    /// Titre de la fenêtre du système.
    fn set_title(&mut self, title: &str);
    /// Quitter l'application.
    fn exit(&mut self);
    /// Vrai si la plateforme affiche la barre de menus elle-même (macOS) : l'application ne la dessine pas et
    /// fournit son contenu par [`Shell::menu_model`].
    fn native_menubar(&self) -> bool {
        false
    }
    /// Le contenu des menus a changé (pour une barre de menus native).
    fn menus_changed(&mut self) {}
    /// Demande à l'utilisateur de choisir un fichier (envoi DCC, chargement d'un script…) ; la plateforme
    /// répond plus tard par [`Shell::file_picked`] avec le même `request`. Par défaut : aucun choix.
    fn pick_file(&mut self, request: u64, save: bool, suggested_name: Option<&str>) {
        let _ = (request, save, suggested_name);
    }
    /// Dossier des fichiers reçus (DCC). Par défaut : `Téléchargements/AlpIRC` s'il existe, sinon aucun.
    fn downloads_dir(&self) -> Option<PathBuf> {
        None
    }
}

/// Disposition générale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutKind {
    /// Ordinateur : fenêtres internes, barre d'outils, switchbar (ADR 0005).
    Desktop,
    /// Téléphone : une fenêtre à la fois, tiroirs (ADR 0006).
    Mobile,
}

/// Origine d'un menu surgissant.
#[derive(Clone, Debug)]
enum PopupSrc {
    /// Menu d'un script : sorte, fenêtre, pseudos sélectionnés, arbre tel qu'affiché.
    Script { kind: alpirc_script::PopupKind, win: WinId, selected: Vec<String>, tree: Vec<alpirc_script::MenuItem> },
    /// Menu d'une fenêtre (switchbar, icône de la fenêtre).
    Window(WinId),
}

/// Ids des entrées du menu d'une fenêtre (switchbar, icône).
mod win_menu {
    pub const CLOSE: u32 = 1;
    pub const MINIMIZE: u32 = 2;
    pub const MAXIMIZE: u32 = 3;
    pub const RESTORE: u32 = 4;
    pub const CLEAR: u32 = 5;
    pub const LOG: u32 = 6;
}

/// L'application.
pub struct Shell {
    pub client: Client,
    pub res: Res,
    net: Net,
    settings_path: PathBuf,
    pub layout_kind: LayoutKind,
    /// Taille de la zone de dessin, en pixels logiques.
    viewport: Size,
    views: HashMap<WinId, WinView>,
    mdi: MdiArea,
    switchbar: Switchbar,
    treebar: Treebar,
    toolbar: Toolbar,
    menubar: MenuBar,
    menubar_dirty: bool,
    popup: Option<(Menu, PopupSrc)>,
    modals: ModalStack,
    ctls: HashMap<u64, Box<dyn DialogCtl>>,
    next_key: u64,
    utc_offset: i32,
    app_focused: bool,
    title: String,
    native_menus: bool,
    /// Rang dans la complétion par Tab en cours (fenêtre, rang).
    tab_cycle: (WinId, usize),
    /// Dernière version des fenêtres vue (pour savoir s'il faut tout resynchroniser).
    windows_seen: Vec<(WinId, u64)>,
    pub(crate) mobile: mobile::MobileState,
    /// Thème choisi la dernière fois (pour ne recharger qu'en cas de changement).
    theme_name: String,
    /// Menu « Commandes » des scripts tel qu'affiché dans la barre de menus.
    commands_tree: Vec<alpirc_script::MenuItem>,
    /// Canaux à rejoindre dès qu'une connexion est prête (liens `irc://`).
    pending_joins: Vec<(alpirc_core::ConnId, Vec<String>)>,
}

/// Paramètres de démarrage.
pub struct ShellConfig {
    pub settings_path: PathBuf,
    pub data_dir: PathBuf,
    pub layout: LayoutKind,
    /// Décalage de l'heure locale (secondes), calculé par la plateforme avant le démarrage des fils.
    pub utc_offset: i32,
    /// Langue du système (`fr-FR`…).
    pub locale: Option<String>,
    pub now_ms: i64,
}

impl Shell {
    /// Crée l'application. `net` est créé par la plateforme, dont la fonction de rappel ramène chaque
    /// [`NetEvent`] dans [`Shell::on_net`] (dans le fil de l'interface).
    pub fn new(cfg: ShellConfig, net: Net) -> Shell {
        let settings = Settings::load(&cfg.settings_path);
        let mut client = Client::new(settings, cfg.data_dir.clone(), cfg.now_ms);
        client.set_utc_offset(Some(cfg.utc_offset));
        if let Some(l) = &cfg.locale {
            client.set_system_locale(l);
        }
        let fonts = assets::fonts();
        fonts.load_system_fallbacks();
        let mut res = Res::new(fonts);
        assets::register_icons(&mut res.icons);
        let mut shell = Shell {
            client,
            res,
            net,
            settings_path: cfg.settings_path,
            layout_kind: cfg.layout,
            viewport: Size::new(800, 600),
            views: HashMap::new(),
            mdi: MdiArea::new(),
            switchbar: Switchbar::new(),
            treebar: Treebar::new(),
            toolbar: Toolbar::new(Vec::new()),
            menubar: MenuBar::new(Vec::new()),
            menubar_dirty: true,
            popup: None,
            modals: ModalStack::new(),
            ctls: HashMap::new(),
            next_key: 1,
            utc_offset: cfg.utc_offset,
            app_focused: true,
            title: String::new(),
            native_menus: false,
            tab_cycle: (0, 0),
            windows_seen: Vec::new(),
            mobile: mobile::MobileState::default(),
            theme_name: String::new(),
            pending_joins: Vec::new(),
            commands_tree: Vec::new(),
        };
        shell.apply_settings();
        shell.build_toolbar();
        shell
    }

    /// À appeler une fois la plateforme prête : scripts de démarrage, connexions automatiques.
    pub fn start(&mut self, p: &mut dyn ShellPlatform) {
        self.native_menus = p.native_menubar();
        let now = p.now_ms();
        self.client.run_pending_scripts(now);
        self.client.connect_startup(now);
        self.after(p);
    }

    pub fn language(&self) -> Lang {
        self.client.language()
    }

    /// Change la taille de la zone de dessin (pixels logiques).
    pub fn set_viewport(&mut self, size: Size) {
        self.viewport = size;
    }

    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// L'application a (ou n'a plus) le focus du système.
    pub fn set_app_focused(&mut self, focused: bool, p: &mut dyn ShellPlatform) {
        self.app_focused = focused;
        self.mdi.app_focused = focused;
        self.client.set_app_focused(focused);
        p.request_repaint();
    }

    // -----------------------------------------------------------------------------------------------------------
    // Réglages, thème, barre d'outils.

    fn apply_settings(&mut self) {
        let s = self.client.settings();
        let name = s.display.theme.clone();
        if name != self.theme_name {
            self.theme_name = name.clone();
            self.res.theme = Theme::named(&name).unwrap_or_else(Theme::classic);
        }
        if self.res.theme.name == "classic" {
            self.res.theme.palette = s.display.palette;
        }
        let zoom = (s.display.chat_font_size / 16).clamp(1, 4);
        self.res.fonts.set_chat_zoom(zoom);
        self.treebar.width = self.treebar.width.max(120);
        self.menubar_dirty = true;
    }

    fn build_toolbar(&mut self) {
        let lang = self.language();
        let res = &self.res;
        let b = |id: u32, name: &str, tip: &str| ToolItem::button(id, icon(res, name), &t(lang, tip));
        self.toolbar = Toolbar::new(vec![
            b(ids::CONNECT, "toolbar/connect", "tb.connect"),
            b(ids::DISCONNECT, "toolbar/disconnect", "tb.disconnect"),
            ToolItem::separator(),
            b(ids::OPTIONS, "toolbar/options", "tb.options"),
            b(ids::FAV_FOLDER, "toolbar/favorites", "tb.favorites"),
            b(ids::CHANLIST, "toolbar/chanlist", "tb.chanlist"),
            b(ids::SCRIPTS, "toolbar/scripts", "tb.scripts"),
            b(ids::ABOOK, "toolbar/abook", "tb.abook"),
            ToolItem::separator(),
            b(ids::DCC_SEND, "toolbar/dcc_send", "tb.dcc_send"),
            b(ids::DCC_CHAT, "toolbar/dcc_chat", "tb.dcc_chat"),
            b(ids::TRANSFERS, "toolbar/transfers", "tb.transfers"),
            ToolItem::separator(),
            b(ids::NOTIFY, "toolbar/notify", "tb.notify"),
            b(ids::URLS, "toolbar/urls", "tb.urls"),
            ToolItem::separator(),
            b(ids::CASCADE, "toolbar/cascade", "tb.cascade"),
            b(ids::TILE_H, "toolbar/tile_h", "tb.tile_h"),
            b(ids::TILE_V, "toolbar/tile_v", "tb.tile_v"),
            ToolItem::separator(),
            b(ids::COLORS, "toolbar/colors", "tb.colors"),
            b(ids::HELP, "toolbar/help", "tb.help"),
        ]);
    }

    fn save_settings(&self) {
        if let Err(e) = self.client.settings().save(&self.settings_path) {
            log::warn!("réglages non enregistrés : {e}");
        }
    }

    /// Les réglages ont été modifiés (boîte Options, menus) : on les applique et on les enregistre.
    pub fn settings_changed(&mut self, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        self.client.settings_changed(now);
        self.apply_settings();
        self.build_toolbar();
        self.after(p);
    }

    // -----------------------------------------------------------------------------------------------------------
    // Réseau et effets du moteur.

    /// Un événement du réseau.
    pub fn on_net(&mut self, ev: NetEvent, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        match ev {
            NetEvent::Connected { conn, .. } => self.client.on_connected(conn, now),
            NetEvent::Line { conn, bytes } => self.client.on_line(conn, &bytes, now),
            NetEvent::Closed { conn, error } => self.client.on_closed(conn, error.as_deref(), now),
            NetEvent::Failed { conn, error, cert_sha256 } => {
                let msg = match cert_sha256 {
                    Some(fp) => format!("{error} [SHA-256 {fp}]"),
                    None => error,
                };
                self.client.on_connect_failed(conn, &msg, now)
            }
            NetEvent::Accepted { .. } | NetEvent::ListenerClosed { .. } => {}
        }
        self.after(p);
    }

    /// Battement d'horloge du moteur (timers, file anti-flood, reconnexions…). Rend l'heure du prochain.
    pub fn tick(&mut self, p: &mut dyn ShellPlatform) -> Option<i64> {
        let now = p.now_ms();
        let next = self.client.tick(now);
        self.after(p);
        next
    }

    /// Après toute action : effets du moteur, fenêtres, boîtes, titre.
    fn after(&mut self, p: &mut dyn ShellPlatform) {
        for _ in 0..8 {
            let effects = self.client.take_effects();
            if effects.is_empty() {
                break;
            }
            for e in effects {
                self.effect(e, p);
            }
        }
        self.flush_pending_joins();
        self.refresh_dialogs(p);
        self.sync_windows();
        self.update_title(p);
        if self.menubar_dirty {
            self.menubar_dirty = false;
            if self.native_menus {
                p.menus_changed();
            } else {
                self.rebuild_menubar();
            }
        }
        p.request_repaint();
    }

    fn effect(&mut self, e: Effect, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        match e {
            Effect::Connect { conn, host, port, tls, verify, client_cert } => {
                let mut target = Target::new(host, port, tls);
                target.verify = match verify {
                    alpirc_core::TlsVerify::Full => Verify::Full,
                    alpirc_core::TlsVerify::AcceptInvalid => Verify::AcceptInvalid,
                    alpirc_core::TlsVerify::Pinned(fp) => Verify::Pinned(fp),
                };
                if let Some(path) = client_cert {
                    if let Ok(pem) = std::fs::read(&path) {
                        // Un seul fichier PEM avec le certificat et la clé.
                        target.client_cert = Some((pem.clone(), pem));
                    }
                }
                self.net.connect(conn, target);
            }
            Effect::Send { conn, line } => {
                let enc = self.client.conn_encoding(conn);
                self.net.send(conn, enc.encode(&line).into_owned());
            }
            Effect::Close { conn } => self.net.close(conn),
            Effect::Notify { win, title, body } => {
                if !self.app_focused || self.client.active() != Some(win) {
                    p.notify(&title, &body);
                }
            }
            Effect::Beep => p.beep(),
            Effect::PlaySound(path) => p.play_sound(&path),
            Effect::Flash { .. } => {
                if !self.app_focused {
                    p.flash();
                }
            }
            Effect::OpenUrl(u) => p.open_url(&u),
            Effect::SaveSettings => {
                self.save_settings();
                self.apply_settings();
            }
            Effect::SaveScripts => {
                if let Err(e) = self.client.save_scripts() {
                    log::warn!("scripts non enregistrés : {e}");
                }
            }
            Effect::Activate(w) => {
                self.sync_windows();
                self.activate(w, now);
            }
            Effect::OpenDialog(d) => self.open_ui_dialog(d, p),
            Effect::Exit => self.exit_now(p),
            Effect::ScriptDialogsChanged => self.sync_script_dialogs(p),
        }
        self.menubar_dirty = true;
    }

    /// Rend une fenêtre active (moteur et interface).
    fn activate(&mut self, w: WinId, now: i64) {
        if self.client.window(w).is_none() {
            return;
        }
        if self.mdi.active() != Some(w as u64) {
            self.mdi.activate(w as u64);
        }
        if self.client.active() != Some(w) {
            self.client.set_active(w, now);
        }
        self.mobile.active = Some(w);
        self.menubar_dirty = true;
    }

    // -----------------------------------------------------------------------------------------------------------
    // Fenêtres : vues, switchbar, treebar, MDI.

    /// Fenêtres dans l'ordre de la switchbar : chaque Status suivi de ses canaux puis de ses privés, puis les
    /// autres.
    pub fn ordered_windows(&self) -> Vec<WinId> {
        let wins = self.client.windows();
        let mut out = Vec::new();
        for st in wins.iter().filter(|w| w.kind == WinKind::Status) {
            out.push(st.id);
            for kind in [WinKind::Channel, WinKind::Query, WinKind::DccChat, WinKind::ChanList, WinKind::Custom] {
                for w in wins.iter().filter(|w| w.kind == kind && w.conn.is_some() && w.conn == st.conn) {
                    out.push(w.id);
                }
            }
        }
        for w in wins {
            if !out.contains(&w.id) {
                out.push(w.id);
            }
        }
        out
    }

    fn sync_windows(&mut self) {
        let order = self.ordered_windows();
        let seen: Vec<(WinId, u64)> = order.iter().filter_map(|id| self.client.window(*id).map(|w| (w.id, w.version))).collect();
        // Fenêtres disparues.
        let gone: Vec<WinId> = self.views.keys().copied().filter(|id| !order.contains(id)).collect();
        for id in gone {
            self.views.remove(&id);
            self.mdi.remove(id as u64);
            self.ctls.retain(|_, _| true);
        }
        // Nouvelles fenêtres.
        for id in &order {
            if self.views.contains_key(id) {
                continue;
            }
            let Some(win) = self.client.window(*id) else { continue };
            let view = WinView::new(win, &self.client);
            let connected = win.conn.and_then(|c| self.client.conn_info(c)).is_some_and(|c| c.state != ConnState::Disconnected);
            let ic = icon(&self.res, views::icon_name(win, connected));
            let n = self.views.len() as i32;
            let area = self.layout().mdi;
            let w = (area.w * 3 / 4).max(320);
            let h = (area.h * 3 / 4).max(200);
            let off = (n % 8) * 22;
            let child = MdiChild::new(*id as u64, &win.title, ic, Rect::new(off, off, w, h));
            let first = self.mdi.children.is_empty();
            self.mdi.add(child);
            if first {
                self.mdi.maximize(*id as u64);
            }
            self.views.insert(*id, view);
            self.menubar_dirty = true;
        }
        // Mises à jour.
        let offset = self.utc_offset;
        for id in &order {
            let Some(win) = self.client.window(*id) else { continue };
            if let Some(v) = self.views.get_mut(id) {
                v.sync(win, &self.client, offset);
            }
            let connected = win.conn.and_then(|c| self.client.conn_info(c)).is_some_and(|c| c.state != ConnState::Disconnected);
            let ic = icon(&self.res, views::icon_name(win, connected));
            if let Some(c) = self.mdi.child_mut(*id as u64) {
                if c.title != win.title {
                    c.title = win.title.clone();
                }
                c.icon = ic;
            }
        }
        // L'interface suit la fenêtre active du moteur.
        if let Some(a) = self.client.active() {
            if self.mdi.active() != Some(a as u64) && self.views.contains_key(&a) {
                self.mdi.activate(a as u64);
            }
            self.mobile.active.get_or_insert(a);
        }
        if seen != self.windows_seen {
            self.windows_seen = seen;
            self.rebuild_bars();
        }
    }

    fn activity(a: CoreActivity) -> Activity {
        match a {
            CoreActivity::None => Activity::None,
            CoreActivity::Event => Activity::Event,
            CoreActivity::Message => Activity::Message,
            CoreActivity::Highlight => Activity::Highlight,
        }
    }

    fn rebuild_bars(&mut self) {
        let order = self.ordered_windows();
        let mut items = Vec::new();
        let mut roots: Vec<alpirc_ui::TreebarNode> = Vec::new();
        for id in &order {
            let Some(win) = self.client.window(*id) else { continue };
            let connected = win.conn.and_then(|c| self.client.conn_info(c)).is_some_and(|c| c.state != ConnState::Disconnected);
            let ic = icon(&self.res, views::icon_name(win, connected));
            let label = short_label(win, &self.client);
            let mut item = alpirc_ui::SwitchItem::new(*id as u64, &label, ic);
            item.activity = Self::activity(win.activity);
            items.push(item);
            let mut node = alpirc_ui::TreebarNode::new(*id as u64, &label, ic);
            node.activity = Self::activity(win.activity);
            if win.kind == WinKind::Status {
                roots.push(node);
            } else if let Some(parent) = roots
                .iter_mut()
                .rev()
                .find(|r| self.client.window(r.id as WinId).and_then(|w| w.conn).is_some() && self.client.window(r.id as WinId).and_then(|w| w.conn) == win.conn)
            {
                parent.children.push(node);
            } else {
                roots.push(node);
            }
        }
        // Garder l'état déplié des nœuds.
        for r in roots.iter_mut() {
            if let Some(old) = self.treebar.roots.iter().find(|o| o.id == r.id) {
                r.expanded = old.expanded;
            }
        }
        self.switchbar.items = items;
        self.switchbar.active = self.client.active().map(|a| a as u64);
        self.treebar.roots = roots;
        self.treebar.active = self.switchbar.active;
    }

    fn update_title(&mut self, p: &mut dyn ShellPlatform) {
        let title = match self.client.active().and_then(|a| self.client.window(a)) {
            Some(w) if self.mdi.is_maximized() || self.layout_kind == LayoutKind::Mobile => format!("AlpIRC - [{}]", w.title),
            _ => "AlpIRC".to_string(),
        };
        if title != self.title {
            self.title = title.clone();
            p.set_title(&title);
        }
        self.switchbar.active = self.client.active().map(|a| a as u64);
        self.treebar.active = self.switchbar.active;
    }

    /// Titre courant de la fenêtre du système.
    pub fn title(&self) -> &str {
        &self.title
    }

    // -----------------------------------------------------------------------------------------------------------
    // Menus.

    fn menu_state(&self) -> menus::MenuState {
        let d = &self.client.settings().display;
        menus::MenuState {
            toolbar: d.toolbar,
            switchbar: d.switchbar != "off",
            treebar: d.treebar,
            nicklist: d.nicklist,
            dark: self.res.theme.name == "dark",
            connected: self.client.conns().iter().any(|c| self.client.conn_info(*c).is_some_and(|i| i.state != ConnState::Disconnected)),
            native: self.native_menus,
        }
    }

    /// Le contenu de la barre de menus (pour une barre native).
    pub fn menu_model(&mut self) -> Vec<(String, Vec<MenuItem>)> {
        let st = self.menu_state();
        let active = self.client.active();
        let (model, tree) = menus::menubar(&mut self.client, active, &st);
        self.commands_tree = tree;
        model
    }

    fn rebuild_menubar(&mut self) {
        let open = self.menubar.is_active() || self.popup.is_some();
        if open {
            // Ne pas reconstruire une barre dont un menu est ouvert : on le fera après.
            self.menubar_dirty = true;
            return;
        }
        let model = self.menu_model();
        self.menubar = MenuBar::new(model);
    }

    /// Une commande des menus (barre, barre d'outils, menu natif).
    pub fn menu_action(&mut self, id: u32, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        let active = self.client.active();
        let lang = self.language();
        let status_of_active = active.and_then(|a| self.client.window(a)).and_then(|w| w.conn).and_then(|c| self.client.conn_info(c)).map(|c| c.status_window);
        match id {
            ids::NEW_STATUS => {
                let w = self.client.new_status(now);
                self.sync_windows();
                self.activate(w, now);
            }
            ids::CONNECT => {
                let st = status_of_active.or_else(|| self.client.windows().iter().find(|w| w.kind == WinKind::Status).map(|w| w.id));
                if let Some(st) = st {
                    let connected = self.client.window(st).and_then(|w| w.conn).and_then(|c| self.client.conn_info(c)).is_some_and(|c| c.state != ConnState::Disconnected);
                    if connected {
                        self.open_ui_dialog(UiDialog::Connect, p);
                    } else {
                        let net = self
                            .client
                            .window(st)
                            .and_then(|w| w.conn)
                            .and_then(|c| self.client.conn_info(c))
                            .map(|c| c.network)
                            .filter(|n| !n.is_empty())
                            .or_else(|| self.client.settings().networks.first().map(|n| n.name.clone()));
                        match net {
                            Some(n) => self.client.connect(st, &n, None, now),
                            None => self.open_ui_dialog(UiDialog::Connect, p),
                        }
                    }
                }
            }
            ids::DISCONNECT => {
                if let Some(st) = status_of_active {
                    self.client.disconnect(st, None, now);
                }
            }
            ids::SELECT_SERVER => self.open_ui_dialog(UiDialog::Connect, p),
            ids::OPTIONS => self.open_ui_dialog(UiDialog::Options(None), p),
            ids::EXIT => self.request_exit(p),
            ids::TOOLBAR => {
                let s = self.client.settings_mut();
                s.display.toolbar = !s.display.toolbar;
                self.settings_changed(p);
            }
            ids::SWITCHBAR => {
                let s = self.client.settings_mut();
                s.display.switchbar = if s.display.switchbar == "off" { "top".into() } else { "off".into() };
                self.settings_changed(p);
            }
            ids::TREEBAR => {
                let s = self.client.settings_mut();
                s.display.treebar = !s.display.treebar;
                self.settings_changed(p);
            }
            ids::NICKLIST => {
                let s = self.client.settings_mut();
                s.display.nicklist = !s.display.nicklist;
                self.settings_changed(p);
            }
            ids::THEME_CLASSIC | ids::THEME_DARK => {
                self.client.settings_mut().display.theme = if id == ids::THEME_DARK { "dark".into() } else { "classic".into() };
                self.settings_changed(p);
            }
            ids::FONT_BIGGER | ids::FONT_SMALLER => {
                let s = self.client.settings_mut();
                let z = (s.display.chat_font_size / 16).clamp(1, 4);
                let z = if id == ids::FONT_BIGGER { (z + 1).min(4) } else { z.saturating_sub(1).max(1) };
                s.display.chat_font_size = z * 16;
                self.settings_changed(p);
            }
            ids::FIND => {
                if let Some(w) = active {
                    self.open_dialog(Box::new(dialogs::FindDialog { win: w }), p);
                }
            }
            ids::CLEAR => {
                if let Some(w) = active {
                    self.client.command(w, "/clear", now);
                }
            }
            ids::FAV_FOLDER => self.open_ui_dialog(UiDialog::ChannelsFolder, p),
            ids::FAV_ADD => {
                if let Some(w) = active.and_then(|a| self.client.window(a)).filter(|w| w.kind == WinKind::Channel) {
                    let net = w.conn.and_then(|c| self.client.conn_info(c)).map(|c| c.network).unwrap_or_default();
                    let chan = w.name.clone();
                    let favs = &mut self.client.settings_mut().favorites;
                    if !favs.iter().any(|f| f.channel.eq_ignore_ascii_case(&chan) && f.network == net) {
                        favs.push(alpirc_core::settings::Favorite { channel: chan, network: net, key: None, description: String::new(), join_on_connect: false });
                        self.settings_changed(p);
                    }
                }
            }
            ids::ABOOK => self.open_ui_dialog(UiDialog::AddressBook, p),
            ids::COLORS => self.open_ui_dialog(UiDialog::Colors, p),
            ids::SCRIPTS => self.open_ui_dialog(UiDialog::ScriptEditor, p),
            ids::CHANLIST => {
                if let Some(w) = active {
                    self.client.command(w, "/list", now);
                }
            }
            ids::URLS => {
                if let Some(w) = active {
                    self.client.command(w, "/url", now);
                }
            }
            ids::NOTIFY => {
                if let Some(w) = active {
                    self.client.command(w, "/notify", now);
                }
            }
            ids::DCC_SEND | ids::DCC_CHAT | ids::TRANSFERS => {
                let mb = dialogs::coming_soon(lang, "DCC");
                self.open_dialog(Box::new(mb), p);
            }
            ids::LOGS => {
                let mb = dialogs::coming_soon(lang, &t(lang, "tools.logs"));
                self.open_dialog(Box::new(mb), p);
            }
            ids::CASCADE => {
                let s = self.layout().mdi.size();
                self.mdi.cascade(s);
            }
            ids::TILE_H => {
                let s = self.layout().mdi.size();
                self.mdi.tile_vertical(s);
            }
            ids::TILE_V => {
                let s = self.layout().mdi.size();
                self.mdi.tile_horizontal(s);
            }
            ids::ARRANGE => {
                let s = self.layout().mdi.size();
                self.mdi.arrange_icons(s);
            }
            ids::NEXT => {
                let r = self.mdi.next();
                self.mdi_responses(r, p);
            }
            ids::CLOSE => {
                if let Some(w) = active {
                    self.client.close_window(w, now);
                }
            }
            ids::HELP => p.open_url("https://github.com/Guiwald/alpirc#readme"),
            ids::ABOUT => self.open_dialog(Box::new(dialogs::About), p),
            id if id >= ids::SCRIPT_BASE => {
                if let Some(w) = active {
                    let tree = std::mem::take(&mut self.commands_tree);
                    self.run_script_menu(alpirc_script::PopupKind::Menubar, w, &[], &tree, id - ids::SCRIPT_BASE, now);
                }
            }
            id if id >= ids::WIN_BASE => self.activate(id - ids::WIN_BASE, now),
            id if id >= ids::FAV_BASE => {
                let i = (id - ids::FAV_BASE) as usize;
                if let Some(f) = self.client.settings().favorites.get(i).cloned() {
                    self.join_favorite(&f, now);
                }
            }
            _ => {}
        }
        self.menubar_dirty = true;
        self.after(p);
    }

    /// Rejoint un favori, sur une connexion à son réseau (ouverte au besoin).
    fn join_favorite(&mut self, f: &alpirc_core::settings::Favorite, now: i64) {
        let conn = self.client.conns().into_iter().find(|c| {
            self.client.conn_info(*c).is_some_and(|i| i.state != ConnState::Disconnected && (f.network.is_empty() || i.network.eq_ignore_ascii_case(&f.network)))
        });
        let cmd = match &f.key {
            Some(k) if !k.is_empty() => format!("/join {} {}", f.channel, k),
            _ => format!("/join {}", f.channel),
        };
        match conn.and_then(|c| self.client.conn_info(c)) {
            Some(ci) => self.client.command(ci.status_window, &cmd, now),
            None if !f.network.is_empty() => {
                // Pas encore connecté à ce réseau : nouvelle fenêtre, connexion, puis le canal en Perform implicite.
                let st = self
                    .client
                    .windows()
                    .iter()
                    .find(|w| w.kind == WinKind::Status && w.conn.and_then(|c| self.client.conn_info(c)).is_none_or(|c| c.state == ConnState::Disconnected))
                    .map(|w| w.id)
                    .unwrap_or_else(|| self.client.new_status(now));
                self.client.connect(st, &f.network, None, now);
                self.client.command(st, &cmd, now);
            }
            None => {}
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Boîtes de dialogue.

    /// Ouvre une boîte (ou ramène au premier plan celle du même genre).
    pub fn open_dialog(&mut self, mut ctl: Box<dyn DialogCtl>, p: &mut dyn ShellPlatform) {
        if let Some(kind) = ctl.kind() {
            if let Some((&key, _)) = self.ctls.iter().find(|(_, c)| c.kind().as_deref() == Some(kind.as_str())) {
                if let Some(i) = self.modals.dialogs.iter().position(|(k, _)| *k == key) {
                    let d = self.modals.dialogs.remove(i);
                    self.modals.dialogs.push(d);
                }
                return;
            }
        }
        let lang = self.language();
        let spec = ctl.spec(lang);
        let mut view = DialogView::new(&spec, &self.res);
        let now = p.now_ms();
        let mut cx = dialogs::Cx { client: &mut self.client, now, lang, actions: Vec::new() };
        ctl.init(&mut view, &mut cx);
        let actions = std::mem::take(&mut cx.actions);
        let key = self.next_key;
        self.next_key += 1;
        let bounds = self.dialog_bounds();
        self.modals.push(key, view, bounds);
        self.ctls.insert(key, ctl);
        self.run_actions(actions, p);
        p.request_repaint();
    }

    fn dialog_bounds(&self) -> Rect {
        Rect::new(0, 0, self.viewport.w, self.viewport.h)
    }

    /// Une boîte de l'interface demandée par le moteur ou un menu.
    pub fn open_ui_dialog(&mut self, d: UiDialog, p: &mut dyn ShellPlatform) {
        let lang = self.language();
        let ctl: Box<dyn DialogCtl> = match d {
            UiDialog::About => Box::new(dialogs::About),
            UiDialog::Find(w) => Box::new(dialogs::FindDialog { win: w }),
            other => match dialogs::build(other.clone(), &self.client) {
                Some(c) => c,
                None => {
                    let what = match other {
                        UiDialog::Options(_) => t(lang, "file.options"),
                        UiDialog::AddressBook => t(lang, "tools.abook"),
                        UiDialog::ChannelsFolder => t(lang, "fav.folder"),
                        UiDialog::ChannelCentral(_) => "Channel Central".to_string(),
                        UiDialog::ScriptEditor => t(lang, "tools.scripts"),
                        UiDialog::Colors => t(lang, "tools.colors"),
                        UiDialog::Connect => t(lang, "file.select_server"),
                        _ => String::new(),
                    };
                    Box::new(dialogs::coming_soon(lang, &what))
                }
            },
        };
        self.open_dialog(ctl, p);
    }

    fn dialog_msg(&mut self, key: u64, msg: DialogMsg, p: &mut dyn ShellPlatform) {
        let Some(mut ctl) = self.ctls.remove(&key) else { return };
        let lang = self.language();
        let now = p.now_ms();
        let outcome = {
            let Some(view) = self.modals.get_mut(key) else { return };
            let mut cx = dialogs::Cx { client: &mut self.client, now, lang, actions: Vec::new() };
            let o = ctl.on(msg, view, &mut cx);
            let actions = std::mem::take(&mut cx.actions);
            (o, actions)
        };
        let (o, actions) = outcome;
        if o == Outcome::Close {
            self.modals.close(key);
        } else {
            self.ctls.insert(key, ctl);
        }
        self.run_actions(actions, p);
        self.after(p);
    }

    fn run_actions(&mut self, actions: Vec<Action>, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        for a in actions {
            match a {
                Action::Open(c) => self.open_dialog(c, p),
                Action::OpenUi(d) => self.open_ui_dialog(d, p),
                Action::Command(w, line) => self.client.command(w, &line, now),
                Action::Input(w, lines) => {
                    for l in lines {
                        self.client.input(w, &l, now);
                    }
                }
                Action::Find { win, needle, forward } => self.find(win, &needle, forward),
                Action::SettingsChanged => self.settings_changed(p),
                Action::OpenUrl(u) => p.open_url(&u),
                Action::Exit => self.exit_now(p),
            }
        }
    }

    fn refresh_dialogs(&mut self, _p: &mut dyn ShellPlatform) {
        if self.ctls.is_empty() {
            return;
        }
        let lang = self.language();
        let keys: Vec<u64> = self.ctls.keys().copied().collect();
        let mut close = Vec::new();
        for key in keys {
            let Some(ctl) = self.ctls.get_mut(&key) else { continue };
            if let Some(name) = ctl.script_name() {
                if !self.client.engine().dialogs().iter().any(|d| d.name.eq_ignore_ascii_case(name)) {
                    close.push(key);
                    continue;
                }
            }
            if let Some(view) = self.modals.get_mut(key) {
                let mut cx = dialogs::Cx { client: &mut self.client, now: 0, lang, actions: Vec::new() };
                ctl.refresh(view, &mut cx);
            }
        }
        for k in close {
            self.modals.close(k);
            self.ctls.remove(&k);
        }
    }

    fn sync_script_dialogs(&mut self, p: &mut dyn ShellPlatform) {
        let names: Vec<String> = self.client.engine().dialogs().iter().map(|d| d.name.clone()).collect();
        for n in names {
            let open = self.ctls.values().any(|c| c.script_name().is_some_and(|s| s.eq_ignore_ascii_case(&n)));
            if !open {
                if let Some(d) = dialogs::script::ScriptDialog::new(&n, self.client.engine()) {
                    self.open_dialog(Box::new(d), p);
                }
            }
        }
        self.refresh_dialogs(p);
    }

    // -----------------------------------------------------------------------------------------------------------
    // Fichiers choisis.

    /// Réponse de la plateforme à [`ShellPlatform::pick_file`].
    pub fn file_picked(&mut self, request: u64, path: Option<PathBuf>, p: &mut dyn ShellPlatform) {
        let _ = (request, path);
        self.after(p);
    }

    // -----------------------------------------------------------------------------------------------------------
    // Liens irc://.

    /// Ouvre un lien `irc://hôte[:port][/#canal[,#canal2]]` (ou `ircs://`) : connexion à ce serveur (ou à la
    /// connexion déjà ouverte vers lui), puis les canaux.
    pub fn open_irc_url(&mut self, url: &str, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        let Some((host, port, tls, chans)) = parse_irc_url(url) else { return };
        let existing = self.client.conns().into_iter().find(|c| {
            self.client.conn_info(*c).is_some_and(|i| i.state != ConnState::Disconnected && i.server.eq_ignore_ascii_case(&host))
        });
        match existing.and_then(|c| self.client.conn_info(c)) {
            Some(ci) => {
                for ch in &chans {
                    self.client.command(ci.status_window, &format!("/join {ch}"), now);
                }
            }
            None => {
                let st = self
                    .client
                    .windows()
                    .iter()
                    .find(|w| w.kind == WinKind::Status && w.conn.and_then(|c| self.client.conn_info(c)).is_none_or(|c| c.state == ConnState::Disconnected))
                    .map(|w| w.id)
                    .unwrap_or_else(|| self.client.new_status(now));
                let port_arg = if tls { format!("+{port}") } else { port.to_string() };
                self.client.command(st, &format!("/server {host} {port_arg}"), now);
                if let Some(conn) = self.client.window(st).and_then(|w| w.conn) {
                    if !chans.is_empty() {
                        self.pending_joins.push((conn, chans));
                    }
                }
            }
        }
        self.after(p);
    }

    fn flush_pending_joins(&mut self) {
        if self.pending_joins.is_empty() {
            return;
        }
        let ready: Vec<usize> = (0..self.pending_joins.len())
            .filter(|&i| self.client.conn_info(self.pending_joins[i].0).is_some_and(|c| c.state == ConnState::Connected))
            .collect();
        for i in ready.into_iter().rev() {
            let (conn, chans) = self.pending_joins.remove(i);
            if let Some(ci) = self.client.conn_info(conn) {
                for ch in chans {
                    self.client.command(ci.status_window, &format!("/join {ch}"), 0);
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Quitter.

    /// Quitter, avec confirmation si des connexions sont ouvertes.
    pub fn request_exit(&mut self, p: &mut dyn ShellPlatform) {
        let n = self.client.conns().iter().filter(|c| self.client.conn_info(**c).is_some_and(|i| i.state == ConnState::Connected)).count();
        if n > 0 {
            let lang = self.language();
            self.open_dialog(Box::new(dialogs::quit_confirm(lang, n)), p);
        } else {
            self.exit_now(p);
        }
    }

    /// Quitte : QUIT sur chaque connexion, réglages et scripts enregistrés.
    pub fn exit_now(&mut self, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        let statuses: Vec<WinId> = self.client.windows().iter().filter(|w| w.kind == WinKind::Status).map(|w| w.id).collect();
        for st in statuses {
            self.client.disconnect(st, None, now);
        }
        for e in self.client.take_effects() {
            match e {
                Effect::Send { conn, line } => {
                    let enc = self.client.conn_encoding(conn);
                    self.net.send(conn, enc.encode(&line).into_owned());
                }
                Effect::Close { conn } => self.net.close(conn),
                _ => {}
            }
        }
        self.save_settings();
        let _ = self.client.save_scripts();
        // Laisser partir les QUIT (close() vide la file avant de fermer).
        std::thread::sleep(std::time::Duration::from_millis(300));
        p.exit();
    }

    // -----------------------------------------------------------------------------------------------------------
    // Recherche.

    fn find(&mut self, win: WinId, needle: &str, forward: bool) {
        let rect = self.window_rects(win).map(|r| r.chat).unwrap_or_default();
        let Some(w) = self.client.window(win) else { return };
        let nicks: Vec<alpirc_ui::NickEntry> = Vec::new();
        let src = views::WinLines { win: w, client: &self.client, nicks: &nicks };
        if let Some(v) = self.views.get_mut(&win) {
            v.chat.set_search(Some(needle));
            v.chat.find(forward, &self.res, rect, &src);
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Menus surgissants.

    fn open_script_popup(&mut self, kind: alpirc_script::PopupKind, win: WinId, selected: Vec<String>, at: Point) {
        let tree = self.client.popup(kind, win, &selected);
        let items = menus::from_script(&tree, 0);
        if items.is_empty() {
            return;
        }
        let menu = Menu::popup(items, at, self.dialog_bounds(), &self.res);
        self.popup = Some((menu, PopupSrc::Script { kind, win, selected, tree }));
    }

    /// Exécute l'entrée d'un menu de script choisie dans `tree` (numéro d'action `action`). Les numéros
    /// d'action ne valent que pour le dernier menu demandé au moteur : on redemande le même menu et on y prend
    /// l'entrée au même endroit.
    fn run_script_menu(&mut self, kind: alpirc_script::PopupKind, win: WinId, selected: &[String], tree: &[alpirc_script::MenuItem], action: u32, now: i64) {
        let Some(path) = menus::path_of(tree, action) else { return };
        let fresh = self.client.popup(kind, win, selected);
        if let Some(a) = menus::action_at(&fresh, &path) {
            self.client.popup_run(win, selected, a, now);
        }
    }

    fn open_window_menu(&mut self, win: WinId, at: Point) {
        let lang = self.language();
        let st = self.mdi.child(win as u64).map(|c| c.state);
        let mut items = vec![
            MenuItem::new(win_menu::RESTORE, &t(lang, "swb.restore")),
            MenuItem::new(win_menu::MINIMIZE, &t(lang, "swb.minimize")),
            MenuItem::new(win_menu::MAXIMIZE, &t(lang, "swb.maximize")),
            MenuItem::separator(),
            MenuItem::new(win_menu::CLEAR, &t(lang, "swb.clear")),
            MenuItem::new(win_menu::LOG, &t(lang, "swb.log")),
            MenuItem::separator(),
            MenuItem::new(win_menu::CLOSE, &t(lang, "swb.close")).shortcut("Ctrl+F4"),
        ];
        match st {
            Some(alpirc_ui::WindowState::Normal) => items[0].enabled = false,
            Some(alpirc_ui::WindowState::Minimized) => items[1].enabled = false,
            Some(alpirc_ui::WindowState::Maximized) => items[2].enabled = false,
            None => {}
        }
        let menu = Menu::popup(items, at, self.dialog_bounds(), &self.res);
        self.popup = Some((menu, PopupSrc::Window(win)));
    }

    fn popup_activated(&mut self, id: u32, src: PopupSrc, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        match src {
            PopupSrc::Script { kind, win, selected, tree } => self.run_script_menu(kind, win, &selected, &tree, id, now),
            PopupSrc::Window(win) => {
                let r = match id {
                    win_menu::CLOSE => {
                        self.client.close_window(win, now);
                        Vec::new()
                    }
                    win_menu::MINIMIZE => self.mdi.minimize(win as u64),
                    win_menu::MAXIMIZE => self.mdi.maximize(win as u64),
                    win_menu::RESTORE => self.mdi.restore(win as u64),
                    win_menu::CLEAR => {
                        self.client.command(win, "/clear", now);
                        Vec::new()
                    }
                    win_menu::LOG => {
                        self.client.command(win, "/log", now);
                        Vec::new()
                    }
                    _ => Vec::new(),
                };
                self.mdi_responses(r, p);
            }
        }
        self.after(p);
    }

    fn mdi_responses(&mut self, rs: Vec<MdiResponse>, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        for r in rs {
            match r {
                MdiResponse::Activated(id) => {
                    let w = id as WinId;
                    if self.client.active() != Some(w) {
                        self.client.set_active(w, now);
                    }
                    self.menubar_dirty = true;
                }
                MdiResponse::CloseRequested(id) => self.client.close_window(id as WinId, now),
                MdiResponse::SystemMenu(id, pos) => self.open_window_menu(id as WinId, pos),
                MdiResponse::StateChanged(..) | MdiResponse::Moved(_) => {}
                MdiResponse::Content(id, inp) => {
                    let Some(cr) = self.mdi.content_rect(id, self.layout().mdi) else { continue };
                    let abs = inp.translated(cr.x, cr.y);
                    self.content_event(id as WinId, &abs, p);
                }
            }
        }
    }
}

/// Lit un lien `irc://` ou `ircs://` : (hôte, port, TLS, canaux).
pub fn parse_irc_url(url: &str) -> Option<(String, u16, bool, Vec<String>)> {
    let (tls, rest) = if let Some(r) = url.strip_prefix("ircs://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("irc://") {
        (false, r)
    } else {
        return None;
    };
    let (hostport, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) if !h.contains(']') || h.ends_with(']') => (h.to_string(), p.trim_start_matches('+').parse().ok()?),
        _ => (hostport.to_string(), if tls { 6697 } else { 6667 }),
    };
    if host.is_empty() {
        return None;
    }
    let chans = path
        .split(['?', '&'])
        .next()
        .unwrap_or("")
        .split(',')
        .filter(|c| !c.is_empty() && !c.eq_ignore_ascii_case("isnick") && !c.eq_ignore_ascii_case("needkey"))
        .map(|c| {
            let c = c.replace("%23", "#");
            if c.starts_with(['#', '&']) { c } else { format!("#{c}") }
        })
        .collect();
    Some((host, port, tls, chans))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn irc_urls() {
        assert_eq!(parse_irc_url("irc://irc.undernet.org/lyon"), Some(("irc.undernet.org".into(), 6667, false, vec!["#lyon".into()])));
        assert_eq!(parse_irc_url("ircs://irc.libera.chat:6697/#rust,#alpirc"), Some(("irc.libera.chat".into(), 6697, true, vec!["#rust".into(), "#alpirc".into()])));
        assert_eq!(parse_irc_url("irc://h:+7000"), Some(("h".into(), 7000, false, vec![])));
        assert_eq!(parse_irc_url("http://x"), None);
    }
}

/// Le réglage de la switchbar (`top`, `bottom`, `left`, `right`, `off`).
pub(crate) fn switchbar_pos(s: &str) -> &str {
    match s {
        "bottom" | "left" | "right" | "off" => s,
        _ => "top",
    }
}
