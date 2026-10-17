//! La boîte Options, à la manière de mIRC 6 : l'arbre des catégories à gauche, les réglages de la catégorie
//! choisie à droite, OK, Annuler et Appliquer en bas.
//!
//! Chaque page a ses contrôles numérotés `page * 100 + n` ; changer de catégorie montre ceux de la page et cache
//! les autres. La boîte lit les réglages à l'ouverture et ne les écrit qu'à OK ou Appliquer, et seulement les
//! parties qu'elle montre (le reste — couleurs, DCC, politiques STS apprises… — n'est pas touché). Les réseaux
//! et les mots surlignés sont modifiés par des boîtes filles ([`NetworkEditor`], [`Form`]) à travers un état
//! partagé.

use std::cell::RefCell;
use std::rc::Rc;

use alpirc_core::i18n::Lang;
use alpirc_core::settings::{DccChatAction, DccExists, DccSendAction, HighlightWord, Network, Server, Settings};
use alpirc_ui::dialog::Widget;
use alpirc_ui::widgets::TreeNode;
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::form::{Field, Form, Val};
use super::network::NetworkEditor;
use super::*;

const W: i32 = 640;
const H: i32 = 460;
/// Début et largeur de la zone des pages.
const PX: i32 = 180;
const PW: i32 = W - PX - MARGIN;

const TREE: u32 = 1;
const OK: u32 = 2;
const CANCEL: u32 = 3;
const APPLY: u32 = 4;

/// Les pages (identifiant dans l'arbre = numéro de page).
const P_CONNECT: u32 = 1;
const P_SERVERS: u32 = 2;
const P_CONNOPT: u32 = 3;
const P_IRC: u32 = 4;
const P_HIGHLIGHT: u32 = 5;
const P_MESSAGES: u32 = 6;
const P_FLOOD: u32 = 7;
const P_LOGGING: u32 = 8;
const P_CATCHER: u32 = 9;
const P_DISPLAY: u32 = 10;
const P_NOTIF: u32 = 11;
const P_SCRIPTS: u32 = 12;
const P_DCC: u32 = 13;

// Connexion.
const NET: u32 = 103;
const SRV: u32 = 105;
const REALNAME: u32 = 108;
const USERNAME: u32 = 110;
const NICK: u32 = 112;
const ALTNICK: u32 = 114;
const CONNECT_BTN: u32 = 115;
const NEWWIN: u32 = 116;
// Serveurs.
const NETS: u32 = 202;
const NADD: u32 = 203;
const NEDIT: u32 = 204;
const NDEL: u32 = 205;
const NUP: u32 = 206;
const NDOWN: u32 = 207;
// Connexion › Options.
const STARTNETS: u32 = 315;
// IRC.
const TSFMT: u32 = 422;
// Surlignage.
const WORDS: u32 = 504;
const WADD: u32 = 505;
const WEDIT: u32 = 506;
const WDEL: u32 = 507;
// Messages.
const VERSION: u32 = 610;
const FINGER: u32 = 612;
const USERINFO: u32 = 614;
const AWAY: u32 = 617;
const QUITMSG: u32 = 621;
// Journaux.
const FOLDER: u32 = 807;
// Affichage.
const CLASSIC: u32 = 1002;
const DARK: u32 = 1003;
const COLORS_BTN: u32 = 1004;
const F16: u32 = 1006;
const F32: u32 = 1007;
const F48: u32 = 1008;
const SWB: u32 = 1011;
const LANG: u32 = 1016;
// Scripts.
const EDITOR_BTN: u32 = 1204;
// DCC.
const SEND_ASK: u32 = 1302;
const SEND_AUTO: u32 = 1303;
const SEND_IGNORE: u32 = 1304;
const CHAT_ASK: u32 = 1306;
const CHAT_AUTO: u32 = 1307;
const CHAT_IGNORE: u32 = 1308;
const EXISTS: u32 = 1313;
const OWN_IP: u32 = 1327;
const DL_FOLDER: u32 = 1330;

/// Une case liée à un réglage.
struct CheckDef {
    id: u32,
    key: &'static str,
    get: fn(&Settings) -> bool,
    set: fn(&mut Settings, bool),
}

/// Un nombre lié à un réglage.
struct NumDef {
    id: u32,
    get: fn(&Settings) -> u64,
    set: fn(&mut Settings, u64),
}

macro_rules! ck {
    ($id:expr, $key:expr, $($f:ident).+) => {
        CheckDef { id: $id, key: $key, get: |s| s.$($f).+, set: |s, v| s.$($f).+ = v }
    };
}

macro_rules! nm {
    ($id:expr, $ty:ty, $($f:ident).+) => {
        NumDef { id: $id, get: |s| s.$($f).+ as u64, set: |s, v| s.$($f).+ = v.min(<$ty>::MAX as u64) as $ty }
    };
}

const CHECKS: &[CheckDef] = &[
    // Connexion › Options.
    ck!(302, "opt.reconnect", connect.reconnect),
    ck!(312, "opt.rejoin", connect.rejoin_channels),
    ck!(314, "opt.connect_startup", connect.connect_on_startup),
    ck!(316, "opt.new_window_server", connect.new_window_for_server),
    // IRC (deux colonnes).
    ck!(402, "opt.irc.joins", irc.show_joins_parts),
    ck!(403, "opt.irc.quits", irc.show_quits),
    ck!(404, "opt.irc.nicks", irc.show_nick_changes),
    ck!(405, "opt.irc.prefix", irc.show_mode_prefix),
    ck!(406, "opt.irc.nick_colors", irc.nick_colors),
    ck!(407, "opt.irc.strip", irc.strip_codes),
    ck!(408, "opt.irc.timestamps", irc.timestamps),
    ck!(409, "opt.irc.notices_active", irc.notices_in_active),
    ck!(410, "opt.irc.query", irc.query_on_private),
    ck!(411, "opt.irc.whois_active", irc.whois_in_active),
    ck!(412, "opt.irc.keep_open", irc.keep_channels_open),
    ck!(413, "opt.irc.rejoin_kick", irc.rejoin_on_kick),
    ck!(414, "opt.irc.join_invite", irc.join_on_invite),
    ck!(415, "opt.irc.who_join", irc.who_on_join),
    // Surlignage.
    ck!(501, "opt.hl.enabled", highlight.enabled),
    ck!(502, "opt.hl.nick", highlight.nick),
    ck!(509, "opt.hl.notify", highlight.notify_on_highlight),
    ck!(510, "opt.hl.beep", highlight.beep),
    ck!(511, "opt.hl.flash", highlight.flash),
    ck!(512, "opt.hl.notify_query", highlight.notify_on_query),
    ck!(513, "opt.hl.beep_query", highlight.beep_on_query),
    // Messages et CTCP.
    ck!(602, "opt.ctcp.version", ctcp.reply_version),
    ck!(603, "opt.ctcp.ping", ctcp.reply_ping),
    ck!(604, "opt.ctcp.time", ctcp.reply_time),
    ck!(605, "opt.ctcp.finger", ctcp.reply_finger),
    ck!(606, "opt.ctcp.clientinfo", ctcp.reply_clientinfo),
    ck!(607, "opt.ctcp.userinfo", ctcp.reply_userinfo),
    ck!(608, "opt.ctcp.source", ctcp.reply_source),
    ck!(618, "opt.away.queries", away.show_in_queries),
    // Anti-flood.
    ck!(702, "opt.flood.enabled", flood.enabled),
    // Journaux.
    ck!(802, "opt.log.enabled", logging.enabled),
    ck!(803, "opt.log.channels", logging.channels),
    ck!(804, "opt.log.queries", logging.queries),
    ck!(805, "opt.log.status", logging.status),
    ck!(809, "opt.log.timestamps", logging.timestamps),
    ck!(810, "opt.log.strip", logging.strip_codes),
    // URL catcher.
    ck!(902, "opt.catch.enabled", catcher.enabled),
    // Affichage.
    ck!(1012, "opt.disp.treebar", display.treebar),
    ck!(1013, "opt.disp.toolbar", display.toolbar),
    ck!(1014, "opt.disp.nicklist", display.nicklist),
    // Notifications.
    ck!(1102, "opt.notif.enabled", notifications.enabled),
    ck!(1103, "opt.notif.inactive", notifications.only_when_inactive),
    // Scripts.
    ck!(1202, "opt.scripts.enabled", scripts.enabled),
    ck!(1203, "opt.scripts.warn", scripts.warn_on_load),
    // DCC.
    ck!(1310, "opt.dcc.passive", dcc.passive),
    ck!(1311, "opt.dcc.passive_chat", dcc.passive_chat),
];

const NUMS: &[NumDef] = &[
    nm!(304, u64, connect.retry_delay_secs),
    nm!(307, u64, connect.max_retry_delay_secs),
    nm!(310, u32, connect.max_retries),
    nm!(319, u64, connect.ping_timeout_secs),
    nm!(322, u64, connect.lag_interval_secs),
    nm!(424, u32, irc.confirm_paste_lines),
    nm!(427, usize, irc.max_lines),
    nm!(429, u64, irc.auto_op_delay_ms),
    nm!(704, u32, flood.burst),
    nm!(707, u64, flood.rate_ms),
    nm!(711, u32, flood.ctcp_flood_limit),
    nm!(713, u64, flood.ctcp_flood_secs),
    nm!(812, usize, logging.reload_lines),
    nm!(904, usize, catcher.max),
    nm!(1316, u64, dcc.timeout_secs),
    nm!(1319, u32, dcc.packet_size),
    nm!(1322, u16, dcc.port_min),
    nm!(1324, u16, dcc.port_max),
];

/// Langues proposées (`None` : celle du système).
const LANGS: [(Option<&str>, &str); 8] = [
    (None, ""),
    (Some("en"), "English"),
    (Some("fr"), "Français"),
    (Some("es"), "Español"),
    (Some("de"), "Deutsch"),
    (Some("it"), "Italiano"),
    (Some("va"), "Valencià"),
    (Some("ga"), "Gaeilge"),
];

/// Positions de la switchbar.
const SWITCHBAR: [&str; 5] = ["top", "bottom", "left", "right", "off"];

/// Un réseau en cours de modification, avec le nom qu'il avait dans les réglages.
#[derive(Clone, Debug)]
struct NetEntry {
    origin: Option<String>,
    net: Network,
}

/// Ce que les boîtes filles modifient.
#[derive(Default)]
struct Shared {
    networks: Vec<NetEntry>,
    words: Vec<HighlightWord>,
    version: u64,
}

/// La boîte Options.
pub struct Options {
    page: u32,
    st: Rc<RefCell<Shared>>,
    seen: u64,
    /// Réglages à l'ouverture (pour remplir les pages).
    base: Settings,
    /// Thème vu dans les réglages (la boîte Couleurs peut le changer pendant que celle-ci est ouverte).
    seen_theme: String,
}

/// Page d'après son nom (`/options display`).
fn page_of(name: &str) -> u32 {
    match name.trim().to_ascii_lowercase().as_str() {
        "servers" | "server" | "networks" => P_SERVERS,
        "connect-options" | "reconnect" | "options" => P_CONNOPT,
        "irc" => P_IRC,
        "highlight" => P_HIGHLIGHT,
        "messages" | "ctcp" | "away" => P_MESSAGES,
        "flood" => P_FLOOD,
        "logging" | "log" | "logs" => P_LOGGING,
        "catcher" | "url" | "urls" => P_CATCHER,
        "display" | "font" | "theme" | "language" => P_DISPLAY,
        "notifications" | "notify" => P_NOTIF,
        "scripts" | "script" => P_SCRIPTS,
        "dcc" => P_DCC,
        _ => P_CONNECT,
    }
}

/// Nom d'une couleur de la palette (« 4 Rouge ») ou « Par défaut ».
pub fn color_name(lang: Lang, c: Option<u8>) -> String {
    match c {
        Some(i) if i < 16 => format!("{i} {}", t(lang, &format!("col.c{i}"))),
        Some(i) => i.to_string(),
        None => t(lang, "opt.hl.default_color"),
    }
}

/// Adresse d'un serveur pour une liste (`hôte:+6697` en TLS, comme mIRC).
fn server_label(s: &Server) -> String {
    format!("{}:{}{}", s.host, if s.tls { "+" } else { "" }, s.port)
}

impl Options {
    pub fn new(client: &Client, page: Option<&str>) -> Options {
        let base = client.settings().clone();
        let networks = base.networks.iter().map(|n| NetEntry { origin: Some(n.name.clone()), net: n.clone() }).collect();
        let st = Shared { networks, words: base.highlight.words.clone(), version: 0 };
        Options { page: page.map(page_of).unwrap_or(P_CONNECT), st: Rc::new(RefCell::new(st)), seen: 0, seen_theme: base.display.theme.clone(), base }
    }

    fn show_page(&mut self, view: &mut DialogView, page: u32) {
        self.page = page;
        let ids: Vec<u32> = view.controls.iter().map(|c| c.id).filter(|&id| id >= 100).collect();
        for id in ids {
            view.set_visible(id, id / 100 == page);
        }
        if view.selection(TREE).first().copied() != Some(page as usize) {
            view.set_selection(TREE, &[page as usize]);
        }
    }

    // --- Connexion ---

    fn fill_networks_combo(&self, view: &mut DialogView, keep: Option<String>) {
        let st = self.st.borrow();
        let names: Vec<String> = st.networks.iter().map(|e| e.net.name.clone()).collect();
        view.set_items(NET, &names);
        let i = keep.and_then(|k| names.iter().position(|n| n.eq_ignore_ascii_case(&k))).or(if names.is_empty() { None } else { Some(0) });
        select_row(view, NET, i);
        drop(st);
        self.fill_servers_combo(view);
    }

    fn fill_servers_combo(&self, view: &mut DialogView) {
        let st = self.st.borrow();
        let servers: Vec<String> =
            sel(view, NET).and_then(|i| st.networks.get(i)).map(|e| e.net.servers.iter().map(server_label).collect()).unwrap_or_default();
        view.set_items(SRV, &servers);
        select_row(view, SRV, if servers.is_empty() { None } else { Some(0) });
    }

    // --- Serveurs ---

    fn fill_networks_list(&self, view: &mut DialogView, lang: Lang, select: Option<usize>) {
        let st = self.st.borrow();
        let rows: Vec<String> = st
            .networks
            .iter()
            .map(|e| {
                let n = &e.net;
                let first = n.servers.first().map(server_label).unwrap_or_default();
                let servers = if n.servers.len() > 1 { format!("{first} (+{})", n.servers.len() - 1) } else { first };
                row(&[n.name.clone(), servers, if n.auto_connect { t(lang, "net.yes") } else { String::new() }])
            })
            .collect();
        let n = rows.len();
        view.set_items(NETS, &rows);
        select_row(view, NETS, select.filter(|&i| i < n));
        drop(st);
        self.update_net_buttons(view);
    }

    fn update_net_buttons(&self, view: &mut DialogView) {
        let n = self.st.borrow().networks.len();
        let s = sel(view, NETS);
        view.set_enabled(NEDIT, s.is_some());
        view.set_enabled(NDEL, s.is_some());
        view.set_enabled(NUP, s.is_some_and(|i| i > 0));
        view.set_enabled(NDOWN, s.is_some_and(|i| i + 1 < n));
    }

    /// Ouvre l'éditeur sur le réseau `i` (ou un nouveau).
    fn edit_network(&self, i: Option<usize>, cx: &mut Cx) {
        let st = self.st.clone();
        let net = match i {
            Some(i) => match self.st.borrow().networks.get(i) {
                Some(e) => e.net.clone(),
                None => return,
            },
            None => Network::new("", vec![Server::new("", 6697, true)]),
        };
        let lang = cx.lang;
        let ed = NetworkEditor::new(
            net,
            i.is_none(),
            Box::new(move |n, _cx| {
                let mut s = st.borrow_mut();
                if s.networks.iter().enumerate().any(|(k, e)| Some(k) != i && e.net.name.eq_ignore_ascii_case(&n.name)) {
                    return Err(tf(lang, "opt.err_net_exists", &[("name", &n.name)]));
                }
                match i {
                    Some(i) => s.networks[i].net = n,
                    None => s.networks.push(NetEntry { origin: None, net: n }),
                }
                s.version += 1;
                Ok(())
            }),
        );
        cx.actions.push(Action::Open(Box::new(ed)));
    }

    // --- Surlignage ---

    fn fill_words(&self, view: &mut DialogView, lang: Lang, select: Option<usize>) {
        let st = self.st.borrow();
        let yes = |b: bool| if b { t(lang, "net.yes") } else { String::new() };
        let rows: Vec<String> = st.words.iter().map(|w| row(&[w.text.clone(), color_name(lang, w.color), yes(w.beep), yes(w.flash), yes(w.notify)])).collect();
        let n = rows.len();
        view.set_items(WORDS, &rows);
        select_row(view, WORDS, select.filter(|&i| i < n));
        drop(st);
        let s = sel(view, WORDS).is_some();
        view.set_enabled(WEDIT, s);
        view.set_enabled(WDEL, s);
    }

    fn edit_word(&self, i: Option<usize>, cx: &mut Cx) {
        let lang = cx.lang;
        let w = match i {
            Some(i) => match self.st.borrow().words.get(i) {
                Some(w) => w.clone(),
                None => return,
            },
            None => HighlightWord { notify: true, flash: true, ..Default::default() },
        };
        let mut colors = vec![t(lang, "opt.hl.default_color")];
        colors.extend((0..16).map(|c| color_name(lang, Some(c))));
        let fields = vec![
            Field::text(&t(lang, "opt.hl.word"), &w.text),
            Field::choice(&t(lang, "opt.hl.color"), colors, w.color.map(|c| c as usize + 1).unwrap_or(0)),
            Field::check(&t(lang, "opt.hl.word_beep"), w.beep),
            Field::check(&t(lang, "opt.hl.word_flash"), w.flash),
            Field::check(&t(lang, "opt.hl.word_notify"), w.notify),
        ];
        let st = self.st.clone();
        let title = t(lang, if i.is_some() { "opt.hl.edit_title" } else { "opt.hl.add_title" });
        let form = Form::new(
            &title,
            "highlight",
            fields,
            Box::new(move |v: &[Val], _cx| {
                let text = v[0].text();
                if text.is_empty() {
                    return Err(t(lang, "opt.hl.err_empty"));
                }
                let k = v[1].index();
                let word = HighlightWord { text, color: (k > 0).then(|| (k - 1) as u8), beep: v[2].on(), flash: v[3].on(), notify: v[4].on() };
                let mut s = st.borrow_mut();
                match i {
                    Some(i) if i < s.words.len() => s.words[i] = word,
                    _ => s.words.push(word),
                }
                s.version += 1;
                Ok(())
            }),
        );
        cx.actions.push(Action::Open(Box::new(form)));
    }

    /// Contrôles qui dépendent d'une case : grisés quand elle est décochée, comme dans mIRC.
    fn update_deps(&self, view: &mut DialogView) {
        const DEPS: &[(u32, &[u32])] = &[
            (302, &[303, 304, 305, 306, 307, 308, 309, 310, 311, 312]),
            (314, &[STARTNETS]),
            (408, &[421, TSFMT]),
            (501, &[502, 503, WORDS, WADD, WEDIT, WDEL]),
            (702, &[703, 704, 705, 706, 707, 708]),
            (802, &[803, 804, 805, 806, FOLDER, 808, 809, 810, 811, 812, 813]),
            (902, &[903, 904, 905]),
            (1102, &[1103]),
            (1202, &[1203]),
        ];
        for (check_id, ids) in DEPS {
            let on = view.checked(*check_id);
            for id in *ids {
                view.set_enabled(*id, on);
            }
        }
        // Modifier et Retirer demandent aussi un mot choisi.
        if view.checked(501) {
            let s = sel(view, WORDS).is_some();
            view.set_enabled(WEDIT, s);
            view.set_enabled(WDEL, s);
        }
    }

    // --- Lecture et écriture des réglages ---

    fn load(&self, view: &mut DialogView, s: &Settings) {
        for c in CHECKS {
            view.set_checked(c.id, (c.get)(s));
        }
        for n in NUMS {
            view.set_text(n.id, &(n.get)(s).to_string());
        }
        view.set_text(REALNAME, &s.identity.realname);
        view.set_text(USERNAME, &s.identity.username);
        view.set_text(NICK, &s.identity.nick);
        view.set_text(ALTNICK, &s.identity.alt_nick);
        view.set_checked(NEWWIN, false);
        view.set_text(STARTNETS, &s.connect.startup_networks.join(", "));
        view.set_text(TSFMT, &s.irc.timestamp_format);
        view.set_text(VERSION, s.ctcp.version_reply.as_deref().unwrap_or(""));
        view.set_text(FINGER, s.ctcp.finger_text.as_deref().unwrap_or(""));
        view.set_text(USERINFO, s.ctcp.userinfo_text.as_deref().unwrap_or(""));
        view.set_text(AWAY, &s.away.default_message);
        view.set_text(QUITMSG, &s.identity.quit_message);
        view.set_text(FOLDER, &s.logging.folder.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
        view.set_checked(if s.display.theme == "dark" { DARK } else { CLASSIC }, true);
        let fs = s.display.chat_font_size;
        view.set_checked(
            if fs >= 48 {
                F48
            } else if fs >= 32 {
                F32
            } else {
                F16
            },
            true,
        );
        view.set_selection(SWB, &[SWITCHBAR.iter().position(|x| *x == crate::switchbar_pos(&s.display.switchbar)).unwrap_or(0)]);
        let li = LANGS.iter().position(|(c, _)| c.is_some_and(|c| s.language.as_deref().is_some_and(|l| l.eq_ignore_ascii_case(c)))).unwrap_or(0);
        view.set_selection(LANG, &[li]);
        let d = &s.dcc;
        view.set_checked(
            match d.on_send {
                DccSendAction::Ask => SEND_ASK,
                DccSendAction::AutoGet => SEND_AUTO,
                DccSendAction::Ignore => SEND_IGNORE,
            },
            true,
        );
        view.set_checked(
            match d.on_chat {
                DccChatAction::Ask => CHAT_ASK,
                DccChatAction::AutoAccept => CHAT_AUTO,
                DccChatAction::Ignore => CHAT_IGNORE,
            },
            true,
        );
        let ex = match d.on_exists {
            DccExists::Ask => 0,
            DccExists::Resume => 1,
            DccExists::Overwrite => 2,
            DccExists::Rename => 3,
        };
        view.set_selection(EXISTS, &[ex]);
        view.set_text(OWN_IP, d.own_ip.as_deref().unwrap_or(""));
        view.set_text(DL_FOLDER, &d.folder.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
    }

    /// Les réglages tels que la boîte les montre (à partir des réglages en vigueur).
    fn collect(&self, view: &DialogView, live: &Settings) -> Settings {
        let mut s = live.clone();
        for c in CHECKS {
            (c.set)(&mut s, view.checked(c.id));
        }
        for n in NUMS {
            let v = num(view, n.id, (n.get)(&s));
            (n.set)(&mut s, v);
        }
        let keep = |v: Option<String>, old: &str| v.unwrap_or_else(|| old.to_string());
        s.identity.realname = keep(opt_text(view, REALNAME), &s.identity.realname);
        s.identity.username = keep(opt_text(view, USERNAME), &s.identity.username);
        s.identity.nick = keep(opt_text(view, NICK), &s.identity.nick);
        s.identity.alt_nick = keep(opt_text(view, ALTNICK), &s.identity.alt_nick);
        s.identity.quit_message = text_of(view, QUITMSG);
        s.connect.startup_networks = text_of(view, STARTNETS).split(',').map(str::trim).filter(|x| !x.is_empty()).map(str::to_string).collect();
        s.irc.timestamp_format = keep(opt_text(view, TSFMT), &s.irc.timestamp_format);
        s.ctcp.version_reply = opt_text(view, VERSION);
        s.ctcp.finger_text = opt_text(view, FINGER);
        s.ctcp.userinfo_text = opt_text(view, USERINFO);
        s.away.default_message = keep(opt_text(view, AWAY), &s.away.default_message);
        s.logging.folder = opt_text(view, FOLDER).map(PathBuf::from);
        s.display.theme = if view.checked(DARK) { "dark".into() } else { "classic".into() };
        // La taille n'est changée que si l'on a choisi un autre bouton que celui de départ (le menu permet 64).
        let fs = if view.checked(F48) {
            48
        } else if view.checked(F32) {
            32
        } else {
            16
        };
        let old = s.display.chat_font_size;
        let old_bucket = if old >= 48 {
            48
        } else if old >= 32 {
            32
        } else {
            16
        };
        if fs != old_bucket {
            s.display.chat_font_size = fs;
        }
        s.display.switchbar = SWITCHBAR[sel(view, SWB).unwrap_or(0).min(4)].to_string();
        s.language = LANGS[sel(view, LANG).unwrap_or(0).min(LANGS.len() - 1)].0.map(str::to_string);
        s.dcc.on_send = if view.checked(SEND_AUTO) {
            DccSendAction::AutoGet
        } else if view.checked(SEND_IGNORE) {
            DccSendAction::Ignore
        } else {
            DccSendAction::Ask
        };
        s.dcc.on_chat = if view.checked(CHAT_AUTO) {
            DccChatAction::AutoAccept
        } else if view.checked(CHAT_IGNORE) {
            DccChatAction::Ignore
        } else {
            DccChatAction::Ask
        };
        s.dcc.on_exists = match sel(view, EXISTS) {
            Some(1) => DccExists::Resume,
            Some(2) => DccExists::Overwrite,
            Some(3) => DccExists::Rename,
            _ => DccExists::Ask,
        };
        // Une adresse IP mal écrite garde l'ancienne.
        s.dcc.own_ip = match opt_text(view, OWN_IP) {
            Some(ip) if ip.parse::<std::net::IpAddr>().is_err() => live.dcc.own_ip.clone(),
            v => v,
        };
        s.dcc.folder = opt_text(view, DL_FOLDER).map(PathBuf::from);
        if s.dcc.packet_size == 0 {
            s.dcc.packet_size = live.dcc.packet_size.max(512);
        }
        // Réseaux : on garde ce que le moteur a appris (STS, réseaux du bouncer).
        let st = self.st.borrow();
        s.networks = st
            .networks
            .iter()
            .map(|e| {
                let mut n = e.net.clone();
                if let Some(old) = e.origin.as_deref().and_then(|o| live.network(o)) {
                    n.sts = old.sts.clone();
                    n.bouncer_networks = old.bouncer_networks.clone();
                }
                n
            })
            .collect();
        s.highlight.words = st.words.clone();
        s
    }

    /// OK ou Appliquer : écrit les réglages.
    fn apply(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let s = self.collect(view, cx.client.settings());
        if &s != cx.client.settings() {
            *cx.client.settings_mut() = s;
            cx.actions.push(Action::SettingsChanged);
        }
        let mut st = self.st.borrow_mut();
        for e in st.networks.iter_mut() {
            e.origin = Some(e.net.name.clone());
        }
        self.seen_theme = cx.client.settings().display.theme.clone();
    }

    /// Bouton « Se connecter » de la page Connexion.
    fn connect(&mut self, view: &mut DialogView, cx: &mut Cx) {
        self.apply(view, cx);
        let Some(i) = sel(view, NET) else { return };
        let Some(name) = self.st.borrow().networks.get(i).map(|e| e.net.name.clone()) else { return };
        let server = sel(view, SRV);
        let status = if view.checked(NEWWIN) {
            let w = cx.client.new_status(cx.now);
            cx.client.set_active(w, cx.now);
            Some(w)
        } else {
            status_window(cx.client)
        };
        if let Some(st) = status {
            // Les réglages doivent être en place avant la connexion.
            cx.client.settings_changed(cx.now);
            cx.client.connect(st, &name, server, cx.now);
        }
    }
}

impl DialogCtl for Options {
    fn kind(&self) -> Option<String> {
        Some("options".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tr = |k: &str| t(lang, k);
        let mut c: Vec<ControlSpec> = Vec::new();
        let tree_h = H - 2 * MARGIN - BTN_H - 8;
        c.push(ControlSpec::new(TREE, ControlKind::Tree, (MARGIN, MARGIN, PX - MARGIN - 8, tree_h), ""));
        let (gx, ix) = (PX, PX + 12);
        let cw = PW - 24;
        // Un nombre : étiquette, champ, unité.
        let num_row = |c: &mut Vec<ControlSpec>, lid: u32, id: u32, y: i32, label: &str, unit: Option<&str>, lw: i32| {
            c.push(field_label(lid, ix, y, lw, label));
            c.push(edit(id, (ix + lw + 6, y, 70, EDIT_H), "").style("limit 9"));
            if let Some(u) = unit {
                c.push(field_label(id + 1, ix + lw + 6 + 70 + 6, y, cw - lw - 82, u));
            }
        };
        let ck = |id: u32, r: (i32, i32, i32, i32)| -> ControlSpec {
            let key = CHECKS.iter().find(|d| d.id == id).map(|d| d.key).unwrap_or("");
            check(id, r, &t(lang, key))
        };

        // 1. Connexion.
        c.push(group(101, (gx, MARGIN, PW, 92), &tr("opt.connect.server_group")));
        c.push(field_label(102, ix, 28, 134, &tr("opt.network")));
        c.push(drop_list(NET, (ix + 140, 28, cw - 140, EDIT_H + 10 * 16), &[] as &[&str]));
        c.push(field_label(104, ix, 58, 134, &tr("opt.server")));
        c.push(drop_list(SRV, (ix + 140, 58, cw - 140, EDIT_H + 10 * 16), &[] as &[&str]));
        c.push(group(106, (gx, 108, PW, 150), &tr("opt.connect.identity")));
        for (k, (lid, id, key)) in
            [(107, REALNAME, "opt.realname"), (109, USERNAME, "opt.username"), (111, NICK, "opt.nick"), (113, ALTNICK, "opt.alt")].into_iter().enumerate()
        {
            let y = 128 + k as i32 * ROW;
            c.push(field_label(lid, ix, y, 134, &tr(key)));
            c.push(edit(id, (ix + 140, y, cw - 140, EDIT_H), ""));
        }
        c.push(button(CONNECT_BTN, (gx, 270, 150, BTN_H), &tr("opt.connect_btn")));
        c.push(check(NEWWIN, (gx + 162, 272, PW - 162, CHECK_H), &tr("opt.new_window")));
        c.push(text(117, (gx, 306, PW, 34), &tr("opt.connect.note")));

        // 2. Serveurs.
        c.push(text(201, (gx, MARGIN, PW, LABEL_H), &tr("opt.servers.label")));
        let cols = [tr("opt.servers.col.name"), tr("opt.servers.col.servers"), tr("opt.servers.col.auto")];
        c.push(list_view(NETS, (gx, 26, PW - 104, 300), &cols));
        let bx = gx + PW - 96;
        for (k, (id, key)) in
            [(NADD, "dlg.add_dots"), (NEDIT, "dlg.edit_dots"), (NDEL, "dlg.delete"), (NUP, "dlg.up"), (NDOWN, "dlg.down")].into_iter().enumerate()
        {
            c.push(button(id, (bx, 26 + k as i32 * 27, 96, BTN_H), &tr(key)));
        }
        c.push(text(208, (gx, 334, PW, 50), &tr("opt.servers.note")));

        // 3. Connexion › Options.
        c.push(group(301, (gx, MARGIN, PW, 150), &tr("opt.connopt.reconnect_group")));
        c.push(ck(302, (ix, 28, cw, CHECK_H)));
        num_row(&mut c, 303, 304, 50, &tr("opt.retry_delay"), Some(&tr("opt.unit.seconds")), 150);
        num_row(&mut c, 306, 307, 76, &tr("opt.max_delay"), Some(&tr("opt.unit.seconds")), 150);
        num_row(&mut c, 309, 310, 102, &tr("opt.max_retries"), Some(&tr("opt.unit.no_limit")), 150);
        c.push(ck(312, (ix, 130, cw, CHECK_H)));
        c.push(group(313, (gx, 166, PW, 98), &tr("opt.connopt.startup_group")));
        c.push(ck(314, (ix, 186, cw, CHECK_H)));
        c.push(edit(STARTNETS, (ix + 18, 208, cw - 18, EDIT_H), ""));
        c.push(ck(316, (ix, 238, cw, CHECK_H)));
        c.push(group(317, (gx, 272, PW, 88), &tr("opt.connopt.server_group")));
        num_row(&mut c, 318, 319, 292, &tr("opt.ping_timeout"), Some(&tr("opt.unit.seconds")), 150);
        num_row(&mut c, 321, 322, 320, &tr("opt.lag_interval"), Some(&tr("opt.unit.seconds")), 150);

        // 4. IRC.
        c.push(group(401, (gx, MARGIN, PW, 206), &tr("opt.irc.group")));
        let colw = (cw - 8) / 2;
        for (k, id) in (402..=409).enumerate() {
            c.push(ck(id, (ix, 28 + k as i32 * 22, colw, CHECK_H)));
        }
        for (k, id) in (410..=415).enumerate() {
            c.push(ck(id, (ix + colw + 8, 28 + k as i32 * 22, colw, CHECK_H)));
        }
        c.push(group(420, (gx, 222, PW, 140), &tr("opt.irc.options_group")));
        c.push(field_label(421, ix, 242, 190, &tr("opt.irc.ts_format")));
        c.push(edit(TSFMT, (ix + 196, 242, 110, EDIT_H), ""));
        num_row(&mut c, 423, 424, 270, &tr("opt.irc.paste"), Some(&tr("opt.unit.lines")), 190);
        num_row(&mut c, 426, 427, 298, &tr("opt.irc.max_lines"), None, 190);
        num_row(&mut c, 428, 429, 326, &tr("opt.irc.auto_op"), Some(&tr("opt.unit.ms")), 190);

        // 5. Surlignage.
        c.push(ck(501, (gx, MARGIN, PW / 2 - 4, CHECK_H)));
        c.push(ck(502, (gx + PW / 2, MARGIN, PW / 2, CHECK_H)));
        c.push(text(503, (gx, 34, PW, LABEL_H), &tr("opt.hl.words")));
        let cols = [tr("opt.hl.col.word"), tr("opt.hl.col.color"), tr("opt.hl.col.beep"), tr("opt.hl.col.flash"), tr("opt.hl.col.notify")];
        c.push(list_view(WORDS, (gx, 52, PW - 104, 170), &cols));
        for (k, (id, key)) in [(WADD, "dlg.add_dots"), (WEDIT, "dlg.edit_dots"), (WDEL, "dlg.remove")].into_iter().enumerate() {
            c.push(button(id, (bx, 52 + k as i32 * 27, 96, BTN_H), &tr(key)));
        }
        c.push(group(508, (gx, 232, PW, 98), &tr("opt.hl.alerts")));
        c.push(ck(509, (ix, 252, colw, CHECK_H)));
        c.push(ck(510, (ix, 274, colw, CHECK_H)));
        c.push(ck(511, (ix, 296, colw, CHECK_H)));
        c.push(ck(512, (ix + colw + 8, 252, colw, CHECK_H)));
        c.push(ck(513, (ix + colw + 8, 274, colw, CHECK_H)));

        // 6. Messages et CTCP.
        c.push(group(601, (gx, MARGIN, PW, 200), &tr("opt.ctcp.group")));
        for (k, id) in (602..=605).enumerate() {
            c.push(ck(id, (ix, 28 + k as i32 * 22, colw, CHECK_H)));
        }
        for (k, id) in (606..=608).enumerate() {
            c.push(ck(id, (ix + colw + 8, 28 + k as i32 * 22, colw, CHECK_H)));
        }
        for (k, (lid, id, key)) in
            [(609, VERSION, "opt.ctcp.version_text"), (611, FINGER, "opt.ctcp.finger_text"), (613, USERINFO, "opt.ctcp.userinfo_text")].into_iter().enumerate()
        {
            let y = 120 + k as i32 * ROW;
            c.push(field_label(lid, ix, y, 130, &tr(key)));
            c.push(edit(id, (ix + 136, y, cw - 136, EDIT_H), ""));
        }
        c.push(group(615, (gx, 216, PW, 80), &tr("opt.away.group")));
        c.push(field_label(616, ix, 236, 130, &tr("opt.away.message")));
        c.push(edit(AWAY, (ix + 136, 236, cw - 136, EDIT_H), ""));
        c.push(ck(618, (ix, 266, cw, CHECK_H)));
        c.push(group(619, (gx, 304, PW, 56), &tr("opt.quit.group")));
        c.push(field_label(620, ix, 324, 130, &tr("opt.quit.message")));
        c.push(edit(QUITMSG, (ix + 136, 324, cw - 136, EDIT_H), ""));

        // 7. Anti-flood.
        c.push(group(701, (gx, MARGIN, PW, 112), &tr("opt.flood.group")));
        c.push(ck(702, (ix, 28, cw, CHECK_H)));
        num_row(&mut c, 703, 704, 52, &tr("opt.flood.burst"), Some(&tr("opt.unit.lines")), 190);
        num_row(&mut c, 706, 707, 80, &tr("opt.flood.rate"), Some(&tr("opt.unit.ms")), 190);
        c.push(group(709, (gx, 128, PW, 88), &tr("opt.flood.ctcp_group")));
        num_row(&mut c, 710, 711, 148, &tr("opt.flood.ctcp_limit"), Some(&tr("opt.unit.replies")), 190);
        num_row(&mut c, 712, 713, 176, &tr("opt.flood.ctcp_secs"), Some(&tr("opt.unit.seconds")), 190);

        // 8. Journaux.
        c.push(group(801, (gx, MARGIN, PW, 262), &tr("opt.log.group")));
        c.push(ck(802, (ix, 28, cw, CHECK_H)));
        c.push(ck(803, (ix + 18, 50, cw - 18, CHECK_H)));
        c.push(ck(804, (ix + 18, 72, cw - 18, CHECK_H)));
        c.push(ck(805, (ix + 18, 94, cw - 18, CHECK_H)));
        c.push(field_label(806, ix, 122, 80, &tr("opt.log.folder")));
        c.push(edit(FOLDER, (ix + 86, 122, cw - 86, EDIT_H), ""));
        c.push(text(808, (ix + 86, 146, cw - 86, LABEL_H), &tr("opt.log.folder_note")));
        c.push(ck(809, (ix, 172, cw, CHECK_H)));
        c.push(ck(810, (ix, 194, cw, CHECK_H)));
        num_row(&mut c, 811, 812, 222, &tr("opt.log.reload"), Some(&tr("opt.unit.lines")), 250);

        // 9. URL catcher.
        c.push(group(901, (gx, MARGIN, PW, 84), &tr("opt.catch.group")));
        c.push(ck(902, (ix, 28, cw, CHECK_H)));
        num_row(&mut c, 903, 904, 52, &tr("opt.catch.max"), Some(&tr("opt.unit.addresses")), 150);

        // 10. Affichage.
        let half = (PW - 8) / 2;
        c.push(group(1001, (gx, MARGIN, half, 96), &tr("opt.disp.theme")));
        c.push(radio(CLASSIC, (ix, 28, half - 24, CHECK_H), &tr("opt.disp.classic")));
        c.push(radio(DARK, (ix, 50, half - 24, CHECK_H), &tr("opt.disp.dark")));
        c.push(button(COLORS_BTN, (ix + 110, 36, half - 134, BTN_H), &tr("opt.disp.colors")));
        c.push(group(1005, (gx + half + 8, MARGIN, half, 96), &tr("opt.disp.font")));
        c.push(radio(F16, (gx + half + 20, 28, half - 24, CHECK_H), &tr("opt.disp.font_normal")));
        c.push(radio(F32, (gx + half + 20, 50, half - 24, CHECK_H), &tr("opt.disp.font_large")));
        c.push(radio(F48, (gx + half + 20, 72, half - 24, CHECK_H), &tr("opt.disp.font_huge")));
        c.push(group(1009, (gx, 112, PW, 124), &tr("opt.disp.window")));
        c.push(field_label(1010, ix, 132, 110, &tr("opt.disp.switchbar")));
        let swb = [tr("opt.disp.swb.top"), tr("opt.disp.swb.bottom"), tr("opt.disp.swb.left"), tr("opt.disp.swb.right"), tr("opt.disp.swb.off")];
        c.push(drop_list(SWB, (ix + 116, 132, 150, EDIT_H + 5 * 16 + 4), &swb));
        c.push(ck(1012, (ix, 162, cw, CHECK_H)));
        c.push(ck(1013, (ix, 184, cw, CHECK_H)));
        c.push(ck(1014, (ix, 206, cw, CHECK_H)));
        c.push(group(1015, (gx, 244, PW, 58), &tr("opt.disp.language")));
        let langs: Vec<String> = LANGS.iter().map(|(code, name)| if code.is_none() { tr("opt.disp.lang_auto") } else { name.to_string() }).collect();
        c.push(drop_list(LANG, (ix, 266, 220, EDIT_H + 8 * 16 + 4), &langs));

        // 11. Notifications.
        c.push(group(1101, (gx, MARGIN, PW, 78), &tr("opt.notif.group")));
        c.push(ck(1102, (ix, 28, cw, CHECK_H)));
        c.push(ck(1103, (ix + 18, 50, cw - 18, CHECK_H)));

        // 12. Scripts.
        c.push(group(1201, (gx, MARGIN, PW, 78), &tr("opt.scripts.group")));
        c.push(ck(1202, (ix, 28, cw, CHECK_H)));
        c.push(ck(1203, (ix, 50, cw, CHECK_H)));
        c.push(button(EDITOR_BTN, (gx, 96, 160, BTN_H), &tr("opt.scripts.editor")));

        // 13. DCC.
        c.push(group(1301, (gx, MARGIN, half, 96), &tr("opt.dcc.send_group")));
        c.push(radio(SEND_ASK, (ix, 28, half - 24, CHECK_H), &tr("opt.dcc.ask")));
        c.push(radio(SEND_AUTO, (ix, 50, half - 24, CHECK_H), &tr("opt.dcc.auto_get")));
        c.push(radio(SEND_IGNORE, (ix, 72, half - 24, CHECK_H), &tr("opt.dcc.ignore")));
        c.push(group(1305, (gx + half + 8, MARGIN, half, 96), &tr("opt.dcc.chat_group")));
        let rx = gx + half + 20;
        c.push(radio(CHAT_ASK, (rx, 28, half - 24, CHECK_H), &tr("opt.dcc.ask")));
        c.push(radio(CHAT_AUTO, (rx, 50, half - 24, CHECK_H), &tr("opt.dcc.auto_accept")));
        c.push(radio(CHAT_IGNORE, (rx, 72, half - 24, CHECK_H), &tr("opt.dcc.ignore")));
        c.push(group(1309, (gx, 112, PW, 284), &tr("opt.dcc.options_group")));
        c.push(ck(1310, (ix, 132, cw, CHECK_H)));
        c.push(ck(1311, (ix, 154, cw, CHECK_H)));
        c.push(field_label(1312, ix, 180, 150, &tr("opt.dcc.exists")));
        let exists = [tr("opt.dcc.exists.ask"), tr("opt.dcc.exists.resume"), tr("opt.dcc.exists.overwrite"), tr("opt.dcc.exists.rename")];
        c.push(drop_list(EXISTS, (ix + 156, 180, 210, EDIT_H + 4 * 16 + 4), &exists));
        num_row(&mut c, 1315, 1316, 208, &tr("opt.dcc.timeout"), Some(&tr("opt.unit.seconds")), 150);
        num_row(&mut c, 1318, 1319, 236, &tr("opt.dcc.packet"), Some(&tr("opt.unit.bytes")), 150);
        c.push(field_label(1321, ix, 264, 150, &tr("opt.dcc.ports")));
        c.push(edit(1322, (ix + 156, 264, 64, EDIT_H), "").style("limit 5"));
        c.push(field_label(1323, ix + 226, 264, 20, &tr("opt.dcc.ports_to")).style("center"));
        c.push(edit(1324, (ix + 252, 264, 64, EDIT_H), "").style("limit 5"));
        c.push(field_label(1325, ix + 324, 264, cw - 324, &tr("opt.dcc.ports_auto")));
        c.push(field_label(1326, ix, 292, 150, &tr("opt.dcc.own_ip")));
        c.push(edit(OWN_IP, (ix + 156, 292, 160, EDIT_H), "").style("limit 45"));
        c.push(field_label(1328, ix + 324, 292, cw - 324, &tr("opt.dcc.auto")));
        c.push(field_label(1329, ix, 320, 150, &tr("opt.dcc.folder")));
        c.push(edit(DL_FOLDER, (ix + 156, 320, cw - 156, EDIT_H), ""));
        c.push(text(1331, (ix + 156, 346, cw - 156, 36), &tr("opt.dcc.folder_note")));

        // Pages cachées sauf la première montrée.
        let page = self.page;
        let mut c: Vec<ControlSpec> = c.into_iter().map(|cs| if cs.id >= 100 && cs.id / 100 != page { cs.style("hide") } else { cs }).collect();
        c.extend(ok_cancel(lang, W, H, OK, CANCEL, Some(APPLY)));
        spec(&tr("opt.title"), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let lang = cx.lang;
        let node = |p: u32, key: &str| TreeNode::new(p as u64, &plain(&t(lang, key)));
        let roots = vec![
            node(P_CONNECT, "opt.page.connect").with_children(vec![node(P_SERVERS, "opt.page.servers"), node(P_CONNOPT, "opt.page.connopt")]),
            node(P_IRC, "opt.page.irc").with_children(vec![
                node(P_HIGHLIGHT, "opt.page.highlight"),
                node(P_MESSAGES, "opt.page.messages"),
                node(P_FLOOD, "opt.page.flood"),
                node(P_LOGGING, "opt.page.logging"),
                node(P_CATCHER, "opt.page.catcher"),
            ]),
            node(P_DCC, "opt.page.dcc"),
            node(P_DISPLAY, "opt.page.display"),
            node(P_NOTIF, "opt.page.notifications"),
            node(P_SCRIPTS, "opt.page.scripts"),
        ];
        view.set_tree(TREE, roots);
        if let Some(Widget::Tree(t)) = view.widget_mut(TREE) {
            t.show_selection_always = true;
        }
        view.set_columns(NETS, &[(t(lang, "opt.servers.col.name"), 104), (t(lang, "opt.servers.col.servers"), 150), (t(lang, "opt.servers.col.auto"), 86)]);
        view.set_columns(
            WORDS,
            &[
                (t(lang, "opt.hl.col.word"), 100),
                (t(lang, "opt.hl.col.color"), 86),
                (t(lang, "opt.hl.col.beep"), 46),
                (t(lang, "opt.hl.col.flash"), 52),
                (t(lang, "opt.hl.col.notify"), 56),
            ],
        );
        let base = self.base.clone();
        self.load(view, &base);
        // Réseau et serveur : celui de la fenêtre active s'il y en a un, sinon le premier.
        let active_net =
            status_window(cx.client).and_then(|w| cx.client.window(w)).and_then(|w| w.conn).and_then(|c| cx.client.conn_info(c)).map(|c| c.network);
        self.fill_networks_combo(view, active_net);
        self.fill_networks_list(view, lang, Some(0));
        self.fill_words(view, lang, None);
        let page = self.page;
        self.show_page(view, page);
        self.update_deps(view);
        view.set_focus(TREE);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        let lang = cx.lang;
        match msg {
            DialogMsg::Ok(_) => {
                self.apply(view, cx);
                Outcome::Close
            }
            DialogMsg::Cancel => Outcome::Close,
            DialogMsg::Clicked(APPLY) => {
                self.apply(view, cx);
                Outcome::Keep
            }
            DialogMsg::SelectionChanged(TREE) => {
                if let Some(p) = sel(view, TREE) {
                    if p as u32 == P_CONNECT {
                        let keep = sel(view, NET).and_then(|i| self.st.borrow().networks.get(i).map(|e| e.net.name.clone()));
                        self.fill_networks_combo(view, keep);
                    }
                    self.show_page(view, p as u32);
                }
                Outcome::Keep
            }
            DialogMsg::SelectionChanged(NET) => {
                self.fill_servers_combo(view);
                Outcome::Keep
            }
            DialogMsg::Clicked(CONNECT_BTN) => {
                self.connect(view, cx);
                Outcome::Close
            }
            // Réseaux.
            DialogMsg::SelectionChanged(NETS) => {
                self.update_net_buttons(view);
                Outcome::Keep
            }
            DialogMsg::Clicked(NADD) => {
                self.edit_network(None, cx);
                Outcome::Keep
            }
            DialogMsg::Clicked(NEDIT) | DialogMsg::DoubleClicked(NETS) => {
                if let Some(i) = sel(view, NETS) {
                    self.edit_network(Some(i), cx);
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(NDEL) => {
                if let Some(i) = sel(view, NETS) {
                    let mut st = self.st.borrow_mut();
                    if i < st.networks.len() {
                        st.networks.remove(i);
                        st.version += 1;
                    }
                    drop(st);
                    self.fill_networks_list(view, lang, Some(i.saturating_sub(1)));
                    self.seen = self.st.borrow().version;
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(id @ (NUP | NDOWN)) => {
                if let Some(i) = sel(view, NETS) {
                    let mut st = self.st.borrow_mut();
                    let j = if id == NUP { i.checked_sub(1) } else { Some(i + 1).filter(|&j| j < st.networks.len()) };
                    if let Some(j) = j {
                        st.networks.swap(i, j);
                        st.version += 1;
                        drop(st);
                        self.fill_networks_list(view, lang, Some(j));
                        self.seen = self.st.borrow().version;
                    }
                }
                Outcome::Keep
            }
            // Mots surlignés.
            DialogMsg::SelectionChanged(WORDS) => {
                let s = sel(view, WORDS).is_some();
                view.set_enabled(WEDIT, s);
                view.set_enabled(WDEL, s);
                Outcome::Keep
            }
            DialogMsg::Clicked(WADD) => {
                self.edit_word(None, cx);
                Outcome::Keep
            }
            DialogMsg::Clicked(WEDIT) | DialogMsg::DoubleClicked(WORDS) => {
                if let Some(i) = sel(view, WORDS) {
                    self.edit_word(Some(i), cx);
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(WDEL) => {
                if let Some(i) = sel(view, WORDS) {
                    let mut st = self.st.borrow_mut();
                    if i < st.words.len() {
                        st.words.remove(i);
                        st.version += 1;
                    }
                    drop(st);
                    self.fill_words(view, lang, Some(i.saturating_sub(1)));
                    self.seen = self.st.borrow().version;
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(id) if CHECKS.iter().any(|c| c.id == id) => {
                self.update_deps(view);
                Outcome::Keep
            }
            DialogMsg::Clicked(COLORS_BTN) => {
                cx.actions.push(Action::OpenUi(UiDialog::Colors));
                Outcome::Keep
            }
            DialogMsg::Clicked(EDITOR_BTN) => {
                cx.actions.push(Action::OpenUi(UiDialog::ScriptEditor));
                Outcome::Keep
            }
            _ => Outcome::Keep,
        }
    }

    fn refresh(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let v = self.st.borrow().version;
        if v != self.seen {
            self.seen = v;
            let lang = cx.lang;
            // Une entrée ajoutée : on la choisit ; sinon on garde le choix.
            let (n, prev) = (self.st.borrow().networks.len(), view.items(NETS).len());
            let pick = if n > prev { Some(n - 1) } else { sel(view, NETS) };
            self.fill_networks_list(view, lang, pick);
            let (n, prev) = (self.st.borrow().words.len(), view.items(WORDS).len());
            let pick = if n > prev { Some(n - 1) } else { sel(view, WORDS) };
            self.fill_words(view, lang, pick);
            let keep_net = sel(view, NET).and_then(|i| self.st.borrow().networks.get(i).map(|e| e.net.name.clone()));
            self.fill_networks_combo(view, keep_net);
        }
        // Thème changé ailleurs (boîte Couleurs, menu).
        let theme = cx.client.settings().display.theme.clone();
        if theme != self.seen_theme {
            view.set_checked(if theme == "dark" { DARK } else { CLASSIC }, true);
            self.seen_theme = theme;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};

    #[test]
    fn pages_switch_and_settings_are_written() {
        let mut k = Kit::new("options");
        let mut o = Options::new(&k.client, Some("irc"));
        assert_translated(&o);
        let (mut v, _) = k.open(&mut o);
        assert_eq!(o.page, P_IRC);
        assert!(v.visible(402) && !v.visible(NICK));
        // Changer de page.
        v.set_selection(TREE, &[P_CONNECT as usize]);
        k.send(&mut o, &mut v, DialogMsg::SelectionChanged(TREE));
        assert!(v.visible(NICK) && !v.visible(402));
        // Quelques réglages.
        v.set_text(NICK, "Elisa");
        v.set_checked(402, false);
        v.set_text(427, "abc"); // invalide : on garde l'ancienne valeur
        v.set_text(424, "5");
        v.set_checked(DARK, true);
        v.set_selection(SWB, &[1]);
        v.set_selection(LANG, &[2]);
        v.set_checked(F32, true);
        v.set_text(STARTNETS, "Undernet,  Libera.Chat ,");
        let (o2, actions) = k.send(&mut o, &mut v, DialogMsg::Ok(OK));
        assert_eq!(o2, Outcome::Close);
        assert!(actions.iter().any(|a| matches!(a, Action::SettingsChanged)));
        let s = k.client.settings();
        assert_eq!(s.identity.nick, "Elisa");
        assert!(!s.irc.show_joins_parts);
        assert_eq!(s.irc.max_lines, 5000);
        assert_eq!(s.irc.confirm_paste_lines, 5);
        assert_eq!(s.display.theme, "dark");
        assert_eq!(s.display.switchbar, "bottom");
        assert_eq!(s.display.chat_font_size, 32);
        assert_eq!(s.language.as_deref(), Some("fr"));
        assert_eq!(s.connect.startup_networks, vec!["Undernet", "Libera.Chat"]);
    }

    #[test]
    fn cancel_discards_and_untouched_font_size_is_kept() {
        let mut k = Kit::new("options2");
        k.client.settings_mut().display.chat_font_size = 64;
        let mut o = Options::new(&k.client, Some("display"));
        let (mut v, _) = k.open(&mut o);
        assert!(v.checked(F48));
        v.set_text(NICK, "Zaza");
        let (_, a) = k.send(&mut o, &mut v, DialogMsg::Cancel);
        assert!(a.is_empty());
        assert_eq!(k.client.settings().identity.nick, "Guiwald");
        // Appliquer sans toucher à la taille : 64 reste.
        let (_, _) = k.send(&mut o, &mut v, DialogMsg::Clicked(APPLY));
        assert_eq!(k.client.settings().display.chat_font_size, 64);
        assert_eq!(k.client.settings().identity.nick, "Zaza");
    }

    #[test]
    fn networks_through_the_editor() {
        let mut k = Kit::new("options3");
        let mut o = Options::new(&k.client, Some("servers"));
        let (mut v, _) = k.open(&mut o);
        let n0 = k.client.settings().networks.len();
        // Ajouter : l'éditeur s'ouvre ; on le valide comme le ferait l'utilisateur.
        let (_, mut actions) = k.send(&mut o, &mut v, DialogMsg::Clicked(NADD));
        let Some(Action::Open(mut ed)) = actions.pop() else { panic!("éditeur attendu") };
        let (mut ev, _) = k.open(ed.as_mut());
        ev.set_text(101, "TestNet");
        ev.set_text(110, "127.0.0.1");
        k.send(ed.as_mut(), &mut ev, DialogMsg::Changed(110));
        ev.set_checked(113, false);
        k.send(ed.as_mut(), &mut ev, DialogMsg::Clicked(113));
        let (out, _) = k.send(ed.as_mut(), &mut ev, DialogMsg::Ok(1));
        assert_eq!(out, Outcome::Close);
        k.refresh(&mut o, &mut v);
        assert_eq!(v.items(NETS).len(), n0 + 1);
        // Un nom déjà pris est refusé.
        let (_, mut actions) = k.send(&mut o, &mut v, DialogMsg::Clicked(NADD));
        let Some(Action::Open(mut ed)) = actions.pop() else { panic!() };
        let (mut ev, _) = k.open(ed.as_mut());
        ev.set_text(101, "undernet");
        ev.set_text(110, "x.org");
        k.send(ed.as_mut(), &mut ev, DialogMsg::Changed(110));
        let (out, _) = k.send(ed.as_mut(), &mut ev, DialogMsg::Ok(1));
        assert_eq!(out, Outcome::Keep);
        // Supprimer le premier, valider.
        v.set_selection(NETS, &[0]);
        k.send(&mut o, &mut v, DialogMsg::Clicked(NDEL));
        k.send(&mut o, &mut v, DialogMsg::Ok(OK));
        let s = k.client.settings();
        assert_eq!(s.networks.len(), n0);
        assert!(s.network("Undernet").is_none());
        let t = s.network("TestNet").unwrap();
        assert_eq!((t.servers[0].host.as_str(), t.servers[0].port, t.servers[0].tls), ("127.0.0.1", 6667, false));
    }

    #[test]
    fn connect_button_applies_and_connects() {
        let mut k = Kit::new("options5");
        k.client.settings_mut().networks.insert(0, Network::new("Local", vec![Server::new("127.0.0.1", 16667, false), Server::new("localhost", 16667, false)]));
        let mut o = Options::new(&k.client, None);
        let (mut v, _) = k.open(&mut o);
        assert_eq!(v.items(NET)[0], "Local");
        v.set_selection(NET, &[0]);
        k.send(&mut o, &mut v, DialogMsg::SelectionChanged(NET));
        assert_eq!(v.items(SRV), vec!["127.0.0.1:16667", "localhost:16667"]);
        v.set_selection(SRV, &[1]);
        v.set_text(NICK, "Testeur");
        v.set_checked(NEWWIN, true);
        let n0 = k.client.windows().len();
        let (out, _) = k.send(&mut o, &mut v, DialogMsg::Clicked(CONNECT_BTN));
        assert_eq!(out, Outcome::Close);
        assert_eq!(k.client.settings().identity.nick, "Testeur");
        assert_eq!(k.client.windows().len(), n0 + 1, "nouvelle fenêtre Status");
        let fx = k.client.take_effects();
        assert!(fx.iter().any(|e| matches!(e, alpirc_core::Effect::Connect { host, .. } if host == "localhost")), "{fx:?}");
    }

    #[test]
    fn dcc_page() {
        use alpirc_core::settings::{DccChatAction, DccExists, DccSendAction};
        let mut k = Kit::new("options6");
        let mut o = Options::new(&k.client, Some("dcc"));
        let (mut v, _) = k.open(&mut o);
        assert_eq!(o.page, P_DCC);
        assert!(v.visible(SEND_ASK) && v.checked(SEND_ASK) && v.checked(1310));
        v.set_checked(SEND_AUTO, true);
        v.set_checked(CHAT_IGNORE, true);
        v.set_checked(1310, false);
        v.set_selection(EXISTS, &[1]);
        v.set_text(1316, "60");
        v.set_text(1322, "50000");
        v.set_text(1324, "50010");
        v.set_text(OWN_IP, "pas une IP");
        v.set_text(DL_FOLDER, "/tmp/recus");
        k.send(&mut o, &mut v, DialogMsg::Ok(OK));
        let d = &k.client.settings().dcc;
        assert_eq!((d.on_send, d.on_chat, d.on_exists), (DccSendAction::AutoGet, DccChatAction::Ignore, DccExists::Resume));
        assert!(!d.passive);
        assert_eq!((d.timeout_secs, d.port_min, d.port_max), (60, 50000, 50010));
        assert_eq!(d.own_ip, None, "adresse invalide : l'ancienne reste");
        assert_eq!(d.folder.as_deref(), Some(std::path::Path::new("/tmp/recus")));
    }

    #[test]
    fn highlight_words() {
        let mut k = Kit::new("options4");
        let mut o = Options::new(&k.client, Some("highlight"));
        let (mut v, _) = k.open(&mut o);
        let (_, mut actions) = k.send(&mut o, &mut v, DialogMsg::Clicked(WADD));
        let Some(Action::Open(mut f)) = actions.pop() else { panic!() };
        let (mut fv, _) = k.open(f.as_mut());
        fv.set_text(11, "alpirc");
        fv.set_selection(13, &[5]);
        k.send(f.as_mut(), &mut fv, DialogMsg::Ok(1));
        k.refresh(&mut o, &mut v);
        assert_eq!(v.items(WORDS).len(), 1);
        k.send(&mut o, &mut v, DialogMsg::Ok(OK));
        let w = &k.client.settings().highlight.words[0];
        assert_eq!((w.text.as_str(), w.color), ("alpirc", Some(4)));
    }
}
