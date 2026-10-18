//! Essais de l'application entière (sans écran ni réseau) : premier lancement, boutons « Parcourir… » branchés
//! sur la plateforme, bouton et menu « Absent », canaux favoris rejoints une fois la connexion prête, Liste des
//! canaux, Journaux, thèmes et tailles de police du menu Affichage, boîtes gardées dans la fenêtre, `/splay stop` ;
//! et, pour toutes les boîtes et tous les menus dans les sept langues, des raccourcis & sans doublon et des textes
//! statiques assez hauts.

use std::path::{Path, PathBuf};

use alpirc_core::settings::{Favorite, Network, Server};
use alpirc_net::{Net, NetEvent};
use alpirc_ui::{Cursor, DialogMsg, Platform, Rect, ToolItem};

use crate::menus::ids;
use crate::{LayoutKind, Shell, ShellConfig, ShellPlatform, dialogs};

/// Plateforme d'essai : note les fichiers demandés et les dossiers ouverts.
#[derive(Default)]
struct P {
    now: i64,
    picks: Vec<(u64, bool)>,
    opened: Vec<PathBuf>,
    /// `/splay stop` reçus.
    stops: usize,
    /// Titres des notifications envoyées.
    notes: Vec<String>,
}

impl Platform for P {
    fn clipboard_get(&mut self) -> Option<String> {
        None
    }
    fn clipboard_set(&mut self, _text: &str) {}
    fn now_ms(&self) -> i64 {
        self.now
    }
    fn set_cursor(&mut self, _c: Cursor) {}
    fn set_ime_area(&mut self, _rect: Rect) {}
    fn request_repaint(&mut self) {}
}

impl ShellPlatform for P {
    fn notify(&mut self, title: &str, _body: &str) {
        self.notes.push(title.to_string());
    }
    fn beep(&mut self) {}
    fn play_sound(&mut self, _path: &Path) {}
    fn stop_sound(&mut self) {
        self.stops += 1;
    }
    fn flash(&mut self) {}
    fn open_url(&mut self, _url: &str) {}
    fn set_title(&mut self, _title: &str) {}
    fn exit(&mut self) {}
    fn pick_file(&mut self, request: u64, save: bool, _suggested_name: Option<&str>) {
        self.picks.push((request, save));
    }
    fn open_path(&mut self, path: &Path) {
        self.opened.push(path.to_path_buf());
    }
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("alpirc-shell-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn shell(dir: &Path, p: &mut P) -> Shell {
    p.now = 1_792_260_000_000;
    let cfg = ShellConfig {
        settings_path: dir.join("settings.toml"),
        data_dir: dir.to_path_buf(),
        layout: LayoutKind::Desktop,
        utc_offset: 7200,
        locale: Some("en-US".into()),
        now_ms: p.now,
    };
    let mut s = Shell::new(cfg, Net::new(Box::new(|_e: NetEvent| {})));
    s.set_viewport(alpirc_ui::Size::new(1024, 700));
    s
}

/// Clé de la boîte ouverte du genre `kind`.
fn dialog(s: &Shell, kind: &str) -> Option<u64> {
    s.ctls.iter().find(|(_, c)| c.kind().as_deref() == Some(kind)).map(|(k, _)| *k)
}

/// Connexion simulée à un réseau local (127.0.0.1:1, refusé tout de suite par le système) ; rend la connexion.
fn connect(s: &mut Shell, p: &mut P) -> u32 {
    s.client.settings_mut().networks.insert(0, Network::new("Local", vec![Server::new("127.0.0.1", 1, false)]));
    let st = s.client.windows()[0].id;
    s.client.connect(st, "Local", None, p.now);
    s.tick(p);
    s.client.window(st).and_then(|w| w.conn).unwrap()
}

fn up(s: &mut Shell, p: &mut P, conn: u32) {
    let a: std::net::SocketAddr = "127.0.0.1:1".parse().unwrap();
    s.on_net(NetEvent::Connected { conn, peer: a, local: a, tls: None }, p);
    feed(s, p, conn, ":srv 001 Guiwald :Welcome");
    feed(s, p, conn, ":srv 376 Guiwald :End of MOTD");
}

fn feed(s: &mut Shell, p: &mut P, conn: u32, line: &str) {
    s.on_net(NetEvent::Line { conn, bytes: line.as_bytes().to_vec() }, p);
}

#[test]
fn first_run_opens_the_welcome_box_once() {
    let d = dir("first");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.start(&mut p);
    assert!(dialog(&s, "welcome").is_some(), "boîte du premier lancement");
    // « Plus tard » : réglages enregistrés, plus de question au lancement suivant.
    let key = dialog(&s, "welcome").unwrap();
    s.dialog_msg(key, DialogMsg::Cancel, &mut p);
    assert!(d.join("settings.toml").exists());
    let mut s2 = shell(&d, &mut p);
    s2.start(&mut p);
    assert!(dialog(&s2, "welcome").is_none());
}

#[test]
fn dialogs_stay_inside_a_window_made_smaller() {
    let d = dir("resize");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.start(&mut p);
    let key = dialog(&s, "welcome").unwrap();
    // Fenêtre agrandie puis réduite (Retina : tailles logiques) : la boîte reste centrée, entière.
    s.set_viewport(alpirc_ui::Size::new(1600, 1000));
    s.set_viewport(alpirc_ui::Size::new(520, 380));
    let r = s.modals.get_mut(key).unwrap().rect;
    let b = Rect::new(0, 0, 520, 380);
    assert_eq!(b.intersect(&r), r, "{r:?} dépasse");
    assert_eq!(r, b.center_size(r.size()));
    // Plus petite que la boîte : en haut à gauche, la barre de titre en vue.
    s.set_viewport(alpirc_ui::Size::new(300, 200));
    let r = s.modals.get_mut(key).unwrap().rect;
    assert_eq!((r.x, r.y), (0, 0));
}

#[test]
fn browse_buttons_ask_the_platform_and_fill_the_field() {
    let d = dir("browse");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.client.settings_changed(p.now);
    // Charger un script.
    s.open_dialog(Box::new(dialogs::editor::LoadScript { dir: d.clone(), path: None }), &mut p);
    let key = dialog(&s, "load-script").unwrap();
    s.dialog_msg(key, DialogMsg::Clicked(dialogs::editor::L_BROWSE), &mut p);
    assert_eq!(p.picks, vec![(dialogs::pick_request("load-script"), false)]);
    let file = d.join("away.mrc");
    s.file_picked(p.picks[0].0, Some(file.clone()), &mut p);
    assert_eq!(s.modals.get_mut(key).unwrap().text(dialogs::editor::L_PATH).unwrap(), file.display().to_string());
    s.dialog_msg(key, DialogMsg::Cancel, &mut p);
    // Certificat de SASL EXTERNAL dans l'éditeur de réseau.
    let net = Network::new("Local", vec![Server::new("127.0.0.1", 6697, true)]);
    s.open_dialog(Box::new(dialogs::network::NetworkEditor::new(net, false, Box::new(|_, _| Ok(())))), &mut p);
    let key = dialog(&s, "network").unwrap();
    let v = s.modals.get_mut(key).unwrap();
    assert!(!v.enabled(dialogs::network::CERT_BROWSE), "seulement pour EXTERNAL");
    v.set_selection(dialogs::network::MECH, &[3]);
    s.dialog_msg(key, DialogMsg::SelectionChanged(dialogs::network::MECH), &mut p);
    assert!(s.modals.get_mut(key).unwrap().enabled(dialogs::network::CERT_BROWSE));
    s.dialog_msg(key, DialogMsg::Clicked(dialogs::network::CERT_BROWSE), &mut p);
    let req = p.picks.last().unwrap().0;
    assert_eq!(req, dialogs::pick_request("network"));
    // Choix annulé : rien ne change ; choisi : le chemin dans le champ.
    s.file_picked(req, None, &mut p);
    assert_eq!(s.modals.get_mut(key).unwrap().text(dialogs::network::CERT).unwrap(), "");
    s.file_picked(req, Some(d.join("moi.pem")), &mut p);
    assert_eq!(s.modals.get_mut(key).unwrap().text(dialogs::network::CERT).unwrap(), d.join("moi.pem").display().to_string());
}

fn away_checked(s: &mut Shell) -> (bool, bool) {
    let tool = s.toolbar.items.iter().any(|i| matches!(i, ToolItem::Button { id, checked: true, .. } if *id == ids::AWAY));
    let model = s.menu_model();
    let menu = model[0].1.iter().find(|m| m.id == ids::AWAY).map(|m| (m.checked, m.enabled)).unwrap();
    assert!(menu.1, "connecté : « Absent » permis");
    (tool, menu.0)
}

#[test]
fn away_button_and_menu_follow_the_server() {
    let d = dir("away");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.client.settings_changed(p.now);
    s.start(&mut p);
    let model = s.menu_model();
    assert!(!model[0].1.iter().find(|m| m.id == ids::AWAY).unwrap().enabled, "pas connecté : grisé");
    let conn = connect(&mut s, &mut p);
    up(&mut s, &mut p, conn);
    assert_eq!(away_checked(&mut s), (false, false));
    s.menu_action(ids::AWAY, &mut p);
    // Le serveur confirme : bouton enfoncé, menu coché.
    feed(&mut s, &mut p, conn, ":srv 306 Guiwald :You have been marked as being away");
    assert_eq!(away_checked(&mut s), (true, true));
    s.menu_action(ids::AWAY, &mut p);
    feed(&mut s, &mut p, conn, ":srv 305 Guiwald :You are no longer marked as being away");
    assert_eq!(away_checked(&mut s), (false, false));
}

#[test]
fn favorites_wait_for_the_connection() {
    let d = dir("fav");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.client.settings_changed(p.now);
    s.start(&mut p);
    let conn = connect(&mut s, &mut p);
    let f = Favorite { channel: "#lane-b".into(), network: "Local".into(), key: None, description: String::new(), join_on_connect: false };
    s.join_favorite(&f, p.now);
    assert_eq!(s.pending_joins, vec![(conn, vec!["#lane-b".to_string()])]);
    let st = s.client.windows()[0].id;
    assert!(!s.client.window(st).unwrap().lines.iter().any(|l| l.text.contains("Not connected")));
    up(&mut s, &mut p, conn);
    assert!(s.pending_joins.is_empty(), "rejoint une fois enregistré");
}

#[test]
fn channels_list_dialog_then_sorted_list() {
    let d = dir("list");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.client.settings_changed(p.now);
    s.start(&mut p);
    let conn = connect(&mut s, &mut p);
    up(&mut s, &mut p, conn);
    s.menu_action(ids::CHANLIST, &mut p);
    let key = dialog(&s, "list-channels").expect("boîte « Liste des canaux »");
    s.dialog_msg(key, DialogMsg::Ok(1), &mut p);
    for l in [
        ":srv 321 Guiwald Channel :Users Name",
        ":srv 322 Guiwald #petit 2 :tout petit",
        ":srv 322 Guiwald #lyon 42 :Lyon",
        ":srv 322 Guiwald #grenoble 12 :Grenoble",
        ":srv 323 Guiwald :End of /LIST",
    ] {
        feed(&mut s, &mut p, conn, l);
    }
    let lw = s.client.windows().iter().find(|w| w.kind == alpirc_core::WinKind::ChanList).unwrap().id;
    let rows: Vec<String> = s.views[&lw].list.rows.iter().map(|r| r.cells[0].clone()).collect();
    assert_eq!(rows, vec!["#lyon", "#grenoble", "#petit"], "les plus peuplés d'abord");
}

/// Le sous-menu Affichage › Thème : (id, libellé, coché) de chaque entrée (sans les séparateurs).
fn theme_menu(s: &mut Shell) -> Vec<(u32, String, bool)> {
    let model = s.menu_model();
    let theme = model[1].1.iter().find(|m| m.submenu.iter().any(|i| i.id == ids::THEME_CLASSIC));
    let theme = theme.expect("Affichage › Thème");
    theme.submenu.iter().filter(|i| !i.separator).map(|i| (i.id, i.label.clone(), i.checked && i.radio)).collect()
}

#[test]
fn theme_menu_lists_user_themes_and_selects_palettes() {
    use alpirc_core::settings::{DARK_PALETTE, MIRC_PALETTE, UserTheme};
    let d = dir("themes");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.client.settings_changed(p.now);
    let mut nuit =
        UserTheme { name: "Nuit & jour".into(), base: "dark".into(), palette: DARK_PALETTE, ..Default::default() };
    nuit.palette[4] = 0x123456;
    nuit.colors.background = 1;
    s.client.settings_mut().display.store_theme(nuit.clone()).unwrap();
    s.client.settings_mut().display.store_theme(UserTheme { name: "Pastel".into(), ..Default::default() }).unwrap();
    let m = theme_menu(&mut s);
    let ids_seen: Vec<u32> = m.iter().map(|x| x.0).collect();
    assert_eq!(ids_seen, vec![ids::THEME_CLASSIC, ids::THEME_DARK, ids::THEME_BASE, ids::THEME_BASE + 1]);
    assert_eq!(m[2].1, "Nuit && jour", "le & d'un nom n'est pas un raccourci");
    assert_eq!(m.iter().map(|x| x.2).collect::<Vec<_>>(), vec![true, false, false, false]);
    // Un thème de l'utilisateur basé sur Sombre : sa palette, l'allure sombre, et la coche sur lui seul.
    s.menu_action(ids::THEME_BASE, &mut p);
    let disp = &s.client.settings().display;
    assert_eq!((disp.theme.as_str(), disp.palette, disp.colors.background), ("Nuit & jour", nuit.palette, 1));
    assert_eq!(s.res.theme.palette[4], 0x123456);
    assert_eq!(s.res.theme.face, alpirc_ui::Theme::dark().face);
    assert_eq!(theme_menu(&mut s).iter().map(|x| x.2).collect::<Vec<_>>(), vec![false, false, true, false]);
    // Sombre puis Classique : les palettes des thèmes fournis (pas celle du thème précédent).
    s.menu_action(ids::THEME_DARK, &mut p);
    let disp = &s.client.settings().display;
    assert_eq!((disp.theme.as_str(), disp.palette), ("dark", DARK_PALETTE));
    assert_eq!(theme_menu(&mut s).iter().map(|x| x.2).collect::<Vec<_>>(), vec![false, true, false, false]);
    s.menu_action(ids::THEME_CLASSIC, &mut p);
    assert_eq!(s.client.settings().display.palette, MIRC_PALETTE);
    assert_eq!(s.res.theme.face, alpirc_ui::Theme::classic().face);
    // Un ancien nom (« sombre ») coche Sombre.
    s.client.settings_mut().display.theme = "sombre".into();
    assert_eq!(theme_menu(&mut s).iter().map(|x| x.2).collect::<Vec<_>>(), vec![false, true, false, false]);
}

#[test]
fn font_menu_steps_like_the_options_box() {
    let d = dir("fontstep");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.client.settings_changed(p.now);
    let size = |s: &Shell| s.client.settings().display.chat_font_size;
    // Fixedsys Excelsior (pixel, intégrée) : 16, 32, 48, 64.
    let mut seen = vec![size(&s)];
    for _ in 0..4 {
        s.menu_action(ids::FONT_BIGGER, &mut p);
        seen.push(size(&s));
    }
    assert_eq!(seen, vec![16, 32, 48, 64, 64]);
    s.menu_action(ids::FONT_SMALLER, &mut p);
    assert_eq!(size(&s), 48);
    // Une police vectorielle (une police du système à chasse fixe, s'il y en a une) : de 1 ou 2 pixels.
    let fonts = &s.res.fonts;
    let Some(mono) = fonts.monospace_families().into_iter().find(|f| !fonts.is_pixel_family(f)) else { return };
    let disp = &mut s.client.settings_mut().display;
    (disp.chat_font, disp.chat_font_size) = (mono, 13);
    s.settings_changed(&mut p);
    s.menu_action(ids::FONT_BIGGER, &mut p);
    assert_eq!(size(&s), 14);
    s.client.settings_mut().display.chat_font_size = 16;
    s.menu_action(ids::FONT_BIGGER, &mut p);
    assert_eq!(size(&s), 18);
    s.menu_action(ids::FONT_SMALLER, &mut p);
    s.menu_action(ids::FONT_SMALLER, &mut p);
    assert_eq!(size(&s), 15);
}

#[test]
fn typed_list_with_as_many_channels_shows_the_new_ones() {
    let d = dir("relist");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.client.settings_changed(p.now);
    s.start(&mut p);
    let conn = connect(&mut s, &mut p);
    up(&mut s, &mut p, conn);
    let st = s.client.windows()[0].id;
    let rows = |s: &Shell| {
        let lw = s.client.windows().iter().find(|w| w.kind == alpirc_core::WinKind::ChanList).unwrap().id;
        s.views[&lw].list.rows.iter().map(|r| format!("{} {}", r.cells[0], r.cells[1])).collect::<Vec<_>>()
    };
    s.client.command(st, "/list", p.now);
    feed(&mut s, &mut p, conn, ":srv 321 Guiwald Channel :Users Name");
    feed(&mut s, &mut p, conn, ":srv 322 Guiwald #lyon 42 :Lyon");
    assert_eq!(rows(&s), ["#lyon 42"], "la liste se remplit au fil des réponses");
    feed(&mut s, &mut p, conn, ":srv 322 Guiwald #grenoble 12 :Grenoble");
    feed(&mut s, &mut p, conn, ":srv 323 Guiwald :End of /LIST");
    assert_eq!(rows(&s), ["#lyon 42", "#grenoble 12"]);
    // Le même nombre de canaux, mais d'autres.
    s.client.command(st, "/list", p.now);
    s.tick(&mut p);
    assert!(rows(&s).is_empty(), "l'ancienne liste disparaît");
    feed(&mut s, &mut p, conn, ":srv 321 Guiwald Channel :Users Name");
    feed(&mut s, &mut p, conn, ":srv 322 Guiwald #paris 100 :Paris");
    feed(&mut s, &mut p, conn, ":srv 322 Guiwald #lyon 43 :Lyon");
    feed(&mut s, &mut p, conn, ":srv 323 Guiwald :End of /LIST");
    assert_eq!(rows(&s), ["#paris 100", "#lyon 43"]);
}

#[test]
fn url_list_follows_a_full_catcher() {
    let d = dir("urls");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.client.settings_mut().catcher.max = 2;
    s.client.settings_changed(p.now);
    s.start(&mut p);
    let conn = connect(&mut s, &mut p);
    up(&mut s, &mut p, conn);
    feed(&mut s, &mut p, conn, ":Guiwald!u@h JOIN #lane-f2");
    let chan = s.client.windows().iter().find(|w| w.kind == alpirc_core::WinKind::Channel).unwrap().id;
    s.client.command(chan, "/url", p.now);
    s.tick(&mut p);
    let rows = |s: &Shell| {
        let uw = s.client.windows().iter().find(|w| w.kind == alpirc_core::WinKind::Urls).unwrap().id;
        s.views[&uw].list.rows.iter().map(|r| r.cells[0].clone()).collect::<Vec<_>>()
    };
    for u in ["https://a.example", "https://b.example", "https://c.example"] {
        feed(&mut s, &mut p, conn, &format!(":Elisa!e@h PRIVMSG #lane-f2 :vu {u}"));
    }
    assert_eq!(rows(&s), ["https://c.example", "https://b.example"]);
    // Autant d'adresses qu'avant, mais une autre en tête.
    feed(&mut s, &mut p, conn, ":Elisa!e@h PRIVMSG #lane-f2 :encore https://b.example");
    assert_eq!(rows(&s), ["https://b.example", "https://c.example"]);
}

#[test]
fn splay_stop_reaches_the_platform() {
    let d = dir("splay-stop");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.client.settings_changed(p.now);
    let st = s.client.windows()[0].id;
    s.client.command(st, "/splay stop", p.now);
    s.tick(&mut p);
    assert_eq!(p.stops, 1);
}

#[test]
fn log_files_open_the_folder() {
    let d = dir("logs");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    s.client.settings_changed(p.now);
    s.menu_action(ids::LOGS, &mut p);
    let key = dialog(&s, "logs").expect("boîte des journaux");
    s.dialog_msg(key, DialogMsg::Clicked(4), &mut p);
    assert_eq!(p.opened, vec![d.join("logs")]);
}

// ---------------------------------------------------------------------------------------------------------------
// Toutes les boîtes, dans les sept langues : raccourcis & uniques sur chaque page, textes statiques assez hauts.

mod all_dialogs {
    use std::collections::{BTreeMap, HashSet};

    use alpirc_core::i18n::Lang;
    use alpirc_core::{UiDialog, WinKind};
    use alpirc_ui::text::mnemonic_char;
    use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, ModalStack, TextStyle};

    use super::*;

    /// Une boîte rencontrée : son genre et sa description dans chaque langue.
    struct Seen {
        name: String,
        kind: Option<String>,
        specs: Vec<(Lang, DialogSpec)>,
    }

    /// Ferme toutes les boîtes par Échap (une question d'un script attend sa réponse), puis ce qui resterait.
    fn close_all(s: &mut Shell, p: &mut P) {
        let mut keys: Vec<u64> = s.ctls.keys().copied().collect();
        keys.sort_unstable();
        for k in keys.into_iter().rev() {
            s.dialog_msg(k, DialogMsg::Cancel, p);
        }
        s.modals = ModalStack::new();
        s.ctls.clear();
    }

    /// Note la boîte `key`, décrite dans la langue de l'application (certaines boîtes écrivent leurs textes à leur
    /// création), une fois par genre et par langue.
    fn record(s: &Shell, key: u64, out: &mut Vec<Seen>) {
        let Some(ctl) = s.ctls.get(&key) else { return };
        let lang = s.language();
        let kind = ctl.kind();
        let spec = ctl.spec(lang);
        // Une boîte par genre (l'éditeur de réseau d'Undernet et celui d'un nouveau réseau sont la même).
        let name = kind.clone().unwrap_or_else(|| format!("« {} »", ctl.spec(Lang::En).title));
        match out.iter_mut().find(|x| x.name == name) {
            Some(x) if x.specs.iter().any(|(l, _)| *l == lang) => {}
            Some(x) => x.specs.push((lang, spec)),
            None => out.push(Seen { name, kind, specs: vec![(lang, spec)] }),
        }
    }

    fn buttons(s: &Shell, key: u64) -> Vec<u32> {
        let Some(ctl) = s.ctls.get(&key) else { return Vec::new() };
        ctl.spec(Lang::En).controls.iter().filter(|c| c.kind == ControlKind::Button).map(|c| c.id).collect()
    }

    fn newest(s: &Shell, before: &HashSet<u64>) -> Vec<u64> {
        let mut v: Vec<u64> = s.ctls.keys().copied().filter(|k| !before.contains(k)).collect();
        v.sort_unstable();
        v
    }

    /// Ouvre une boîte, la note, puis clique chacun de ses boutons (sur une boîte rouverte à chaque fois) pour
    /// noter celles qu'ils ouvrent, et un niveau de plus.
    fn explore(s: &mut Shell, p: &mut P, what: &str, open: &dyn Fn(&mut Shell, &mut P), out: &mut Vec<Seen>) {
        close_all(s, p);
        open(s, p);
        let roots = newest(s, &HashSet::new());
        let Some(&root) = roots.first() else { panic!("{what} : aucune boîte ouverte") };
        for k in &roots {
            record(s, *k, out);
        }
        for b in buttons(s, root) {
            close_all(s, p);
            open(s, p);
            let Some(&root) = newest(s, &HashSet::new()).first() else { continue };
            let before: HashSet<u64> = s.ctls.keys().copied().collect();
            s.dialog_msg(root, DialogMsg::Clicked(b), p);
            for child in newest(s, &before) {
                record(s, child, out);
                for b2 in buttons(s, child) {
                    if !s.ctls.contains_key(&child) {
                        break;
                    }
                    let before2: HashSet<u64> = s.ctls.keys().copied().collect();
                    s.dialog_msg(child, DialogMsg::Clicked(b2), p);
                    for g in newest(s, &before2) {
                        record(s, g, out);
                        s.modals.close(g);
                        s.ctls.remove(&g);
                    }
                }
            }
        }
        close_all(s, p);
    }

    /// Toutes les boîtes qu'on peut ouvrir : celles des menus, de la connexion (certificat, Channel Central, DCC,
    /// questions des scripts) et celles qu'ouvrent leurs boutons.
    fn every_dialog() -> Vec<Seen> {
        let mut out = Vec::new();
        for lang in Lang::ALL {
            every_dialog_in(lang, &mut out);
        }
        out
    }

    /// Ce qui ouvre une boîte.
    type Opener = Box<dyn Fn(&mut Shell, &mut P)>;

    fn every_dialog_in(lang: Lang, out: &mut Vec<Seen>) {
        let d = dir(&format!("every-dialog-{}", lang.code()));
        let mut p = P::default();
        let mut s = shell(&d, &mut p);
        s.client.settings_mut().language = Some(lang.code().to_string());
        s.settings_changed(&mut p);
        assert_eq!(s.language(), lang);
        // Aucun vrai serveur : tous les réseaux vers un port fermé de la machine.
        for n in &mut s.client.settings_mut().networks {
            for srv in &mut n.servers {
                (srv.host, srv.port, srv.tls) = ("127.0.0.1".into(), 1, false);
            }
        }
        std::fs::create_dir_all(d.join("logs/Local")).unwrap();
        std::fs::write(d.join("logs/Local/#lane-f2.log"), "[10:00] <Guiwald> salut\n").unwrap();
        s.start(&mut p);
        let welcome = |s: &mut Shell, p: &mut P| s.open_dialog(Box::new(dialogs::welcome::Welcome::new(&s.client)), p);
        explore(&mut s, &mut p, "bienvenue", &welcome, out);
        // Connecté à un réseau local, dans un canal.
        s.client.settings_changed(p.now);
        let conn = connect(&mut s, &mut p);
        up(&mut s, &mut p, conn);
        feed(&mut s, &mut p, conn, ":srv 005 Guiwald PREFIX=(ov)@+ CHANMODES=beI,k,l,imnpst EXCEPTS INVEX :are supported");
        feed(&mut s, &mut p, conn, ":Guiwald!u@h JOIN #lane-f2");
        feed(&mut s, &mut p, conn, ":srv 353 Guiwald = #lane-f2 :@Guiwald Elisa");
        feed(&mut s, &mut p, conn, ":srv 366 Guiwald #lane-f2 :End of /NAMES list.");
        let chan = s.client.windows().iter().find(|w| w.kind == WinKind::Channel).map(|w| w.id).unwrap();
        s.activate(chan, p.now);
        let ui = |d: UiDialog| move |s: &mut Shell, p: &mut P| s.open_ui_dialog(d.clone(), p);
        let openers: Vec<Opener> = vec![
            // D'abord ce qui demande la connexion (les boutons des autres boîtes peuvent la fermer).
            Box::new(move |s: &mut Shell, p: &mut P| {
                feed(s, p, conn, ":Elisa!e@127.0.0.1 PRIVMSG Guiwald :\u{1}DCC SEND photo.jpg 3232235777 5000 1234\u{1}");
                feed(s, p, conn, ":Elisa!e@127.0.0.1 PRIVMSG Guiwald :\u{1}DCC CHAT chat 3232235777 5001\u{1}");
            }),
            Box::new(move |s: &mut Shell, p: &mut P| s.cert_failed(conn, "UnknownIssuer", &"ab".repeat(32), p)),
            Box::new(ui(UiDialog::Options(None))),
            Box::new(ui(UiDialog::AddressBook)),
            Box::new(ui(UiDialog::ChannelsFolder)),
            Box::new(ui(UiDialog::ScriptEditor)),
            Box::new(ui(UiDialog::Colors)),
            Box::new(ui(UiDialog::Connect)),
            Box::new(ui(UiDialog::About)),
            Box::new(ui(UiDialog::Find(chan))),
            Box::new(ui(UiDialog::ChannelCentral(chan))),
            Box::new(|s: &mut Shell, p: &mut P| s.menu_action(ids::CHANLIST, p)),
            Box::new(|s: &mut Shell, p: &mut P| s.menu_action(ids::LOGS, p)),
            Box::new(|s: &mut Shell, p: &mut P| s.menu_action(ids::DCC_SEND, p)),
            Box::new(|s: &mut Shell, p: &mut P| s.menu_action(ids::DCC_CHAT, p)),
            Box::new(|s: &mut Shell, p: &mut P| s.menu_action(ids::TRANSFERS, p)),
            Box::new(move |s: &mut Shell, p: &mut P| s.open_dialog(Box::new(dialogs::JoinPrompt { win: chan }), p)),
            Box::new(move |s: &mut Shell, p: &mut P| {
                let lines = vec!["un".to_string(), "deux".to_string()];
                s.open_dialog(Box::new(dialogs::PasteConfirm { win: chan, win_name: "#lane-f2".into(), lines }), p)
            }),
            Box::new(move |s: &mut Shell, p: &mut P| {
                let dir = s.client.data_dir().to_path_buf();
                s.open_dialog(Box::new(dialogs::editor::LoadScript { dir, path: None }), p)
            }),
            Box::new(move |s: &mut Shell, p: &mut P| {
                let path = s.client.data_dir().join("away.mrc");
                s.open_dialog(Box::new(dialogs::editor::ConfirmScript { path, name: "away.mrc".into() }), p)
            }),
            Box::new(move |s: &mut Shell, p: &mut P| {
                s.client.command(chan, "//echo -a $?=\"Enter channel name:\"", p.now);
                s.tick(p);
            }),
            Box::new(move |s: &mut Shell, p: &mut P| {
                s.client.command(chan, "//echo -a $input(Really?,qy,-1,Title)", p.now);
                s.tick(p);
            }),
            Box::new(move |s: &mut Shell, p: &mut P| {
                s.client.command(chan, "//echo -a $input(Password,pw,-1,Title)", p.now);
                s.tick(p);
            }),
            // Quitter (son bouton Oui ferme les connexions) : en dernier.
            Box::new(|s: &mut Shell, p: &mut P| {
                let lang = s.language();
                s.open_dialog(Box::new(dialogs::quit_confirm(lang, 2)), p)
            }),
        ];
        for (i, open) in openers.iter().enumerate() {
            explore(&mut s, &mut p, &format!("ouverture n° {i}"), open.as_ref(), out);
        }
    }

    /// Les pages d'une boîte : (nom, contrôles montrés ensemble). Options : une page par centaine d'identifiants ;
    /// boîte à onglets : une page par onglet (la bande d'onglets et les contrôles hors onglet sont sur toutes).
    fn pages<'a>(kind: Option<&str>, spec: &'a DialogSpec) -> Vec<(String, Vec<&'a ControlSpec>)> {
        let c = &spec.controls;
        let tabs: Vec<u32> = c.iter().filter(|x| x.kind == ControlKind::Tab).map(|x| x.id).collect();
        if !tabs.is_empty() {
            return tabs
                .iter()
                .map(|t| {
                    let title = c.iter().find(|x| x.id == *t).map(|x| x.text.clone()).unwrap_or_default();
                    (title, c.iter().filter(|x| x.tab.is_none_or(|p| p == *t)).collect())
                })
                .collect();
        }
        if kind == Some("options") {
            let mut by: BTreeMap<u32, Vec<&ControlSpec>> = BTreeMap::new();
            for x in c.iter().filter(|x| x.id >= 100) {
                by.entry(x.id / 100).or_default().push(x);
            }
            return by
                .into_iter()
                .map(|(page, mut v)| {
                    v.extend(c.iter().filter(|x| x.id < 100));
                    (format!("page {page}"), v)
                })
                .collect();
        }
        vec![(String::new(), c.iter().collect())]
    }

    /// Lettre de raccourci des contrôles qui en ont une (étiquette, cadre, bouton, case, radio, onglet).
    fn mnemonic(c: &ControlSpec) -> Option<char> {
        use ControlKind::*;
        matches!(c.kind, Text | Box | Button | Check | Radio | Tab).then(|| mnemonic_char(&c.text)).flatten()
    }

    /// Clés des tables de textes qui donnent ce texte dans cette langue (pour retrouver un raccourci en double).
    fn keys_of(lang: Lang, text: &str) -> Vec<&'static str> {
        let col = Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0);
        crate::tr::TABLES.iter().flat_map(|t| t.iter()).filter(|(_, v)| v[col] == text).map(|(k, _)| *k).collect()
    }

    /// Outil : toutes les pages et leurs raccourcis (`cargo test -p alpirc-shell dump_mnemonics -- --ignored
    /// --nocapture`).
    #[test]
    #[ignore]
    fn dump_mnemonics() {
        for seen in every_dialog() {
            for (lang, spec) in &seen.specs {
                for (page, ctls) in pages(seen.kind.as_deref(), spec) {
                    let mut v: Vec<String> = ctls
                        .iter()
                        .filter_map(|c| mnemonic(c).map(|m| format!("{m} {} {:?}", c.text, keys_of(*lang, &c.text))))
                        .collect();
                    v.sort();
                    println!("== {lang:?} {} {page}\n  {}", seen.name, v.join("\n  "));
                }
            }
        }
        for (lang, path, items) in every_menu() {
            let mut v: Vec<String> = items
                .iter()
                .filter_map(|it| {
                    let m = mnemonic_char(&it.label)?;
                    Some(format!("{m} {} {:?}", it.label, keys_of(lang, &it.label)))
                })
                .collect();
            v.sort();
            println!("== {lang:?} menu {path}\n  {}", v.join("\n  "));
        }
    }

    /// Menus vus : (langue, chemin, entrées sans les séparateurs).
    type Menus = Vec<(Lang, String, Vec<alpirc_ui::MenuItem>)>;

    /// Tous les menus (et sous-menus) de l'application dans chaque langue : barre de menus, menu d'une fenêtre,
    /// menu de la fenêtre Notify.
    fn every_menu() -> Menus {
        fn walk(lang: Lang, path: &str, items: &[alpirc_ui::MenuItem], out: &mut Menus) {
            out.push((lang, path.to_string(), items.iter().filter(|i| !i.separator).cloned().collect()));
            for it in items.iter().filter(|i| i.is_submenu()) {
                walk(lang, &format!("{path} › {}", it.label), &it.submenu, out);
            }
        }
        let d = dir("menu-mnemonics");
        let mut p = P::default();
        let mut s = shell(&d, &mut p);
        s.client.settings_changed(p.now);
        // Des thèmes de l'utilisateur et une fenêtre de canal (menus de la fenêtre, du script).
        use alpirc_core::settings::UserTheme;
        s.client.settings_mut().display.store_theme(UserTheme { name: "Atelier".into(), ..Default::default() }).unwrap();
        let conn = connect(&mut s, &mut p);
        up(&mut s, &mut p, conn);
        feed(&mut s, &mut p, conn, ":Guiwald!u@h JOIN #lane-f2");
        let chan = s.client.windows().iter().find(|w| w.kind == WinKind::Channel).map(|w| w.id).unwrap();
        let mut out = Vec::new();
        for lang in Lang::ALL {
            s.client.settings_mut().language = Some(lang.code().to_string());
            s.settings_changed(&mut p);
            assert_eq!(s.language(), lang);
            let bar = s.menu_model();
            let titles: Vec<alpirc_ui::MenuItem> = bar.iter().map(|(t, items)| alpirc_ui::MenuItem::submenu(t, items.clone())).collect();
            walk(lang, "barre de menus", &titles, &mut out);
            walk(lang, "menu de la fenêtre", &s.desktop_window_menu_items(chan), &mut out);
            walk(lang, "menu Notify", &s.notify_menu_items(), &mut out);
        }
        out
    }

    #[test]
    fn mnemonics_unique_on_each_dialog_page() {
        let all = every_dialog();
        assert!(all.len() >= 40, "{} boîtes seulement", all.len());
        let mut clashes = Vec::new();
        for seen in &all {
            for (lang, spec) in &seen.specs {
                for (page, ctls) in pages(seen.kind.as_deref(), spec) {
                    let mut by: BTreeMap<char, Vec<&str>> = BTreeMap::new();
                    for c in ctls {
                        if let Some(m) = mnemonic(c) {
                            by.entry(m).or_default().push(&c.text);
                        }
                    }
                    for (m, labels) in by.into_iter().filter(|(_, l)| l.len() > 1) {
                        let keys: Vec<String> = labels.iter().map(|l| format!("{l} {:?}", keys_of(*lang, l))).collect();
                        clashes.push(format!("{:?} {} {page} : '{m}' ← {}", lang, seen.name, keys.join(" | ")));
                    }
                }
            }
        }
        assert!(clashes.is_empty(), "{} raccourcis en double :\n{}", clashes.len(), clashes.join("\n"));
    }

    #[test]
    fn mnemonics_unique_in_each_menu() {
        let mut clashes = Vec::new();
        for (lang, path, items) in every_menu() {
            let mut by: BTreeMap<char, Vec<&str>> = BTreeMap::new();
            for it in &items {
                if let Some(m) = mnemonic_char(&it.label) {
                    by.entry(m).or_default().push(&it.label);
                }
            }
            for (m, labels) in by.into_iter().filter(|(_, l)| l.len() > 1) {
                let keys: Vec<String> = labels.iter().map(|l| format!("{l} {:?}", keys_of(lang, l))).collect();
                clashes.push(format!("{lang:?} {path} : '{m}' ← {}", keys.join(" | ")));
            }
        }
        assert!(clashes.is_empty(), "{} raccourcis en double :\n{}", clashes.len(), clashes.join("\n"));
    }

    #[test]
    fn static_texts_are_tall_enough() {
        let all = every_dialog();
        let fonts = crate::assets::fonts();
        let lh = fonts.line_height(alpirc_ui::FontKind::Ui);
        let mut short = Vec::new();
        for seen in &all {
            for (lang, spec) in &seen.specs {
                for c in spec.controls.iter().filter(|c| c.kind == ControlKind::Text && !c.text.is_empty()) {
                    let (_, _, w, h) = c.rect;
                    let nowrap = c.styles.iter().any(|s| s == "nowrap");
                    let need = if nowrap { lh } else { fonts.label_rows(&c.text, TextStyle::UI, w) as i32 * lh };
                    if need > h.max(lh) {
                        short.push(format!("{lang:?} {} #{} : {need} px pour {h} « {} »", seen.name, c.id, c.text.replace('\n', "⏎")));
                    }
                }
            }
        }
        assert!(short.is_empty(), "{} textes coupés :\n{}", short.len(), short.join("\n"));
    }
}

#[test]
fn toucher_une_notification_ouvre_sa_fenetre() {
    let d = dir("notif");
    let mut p = P::default();
    let mut s = shell(&d, &mut p);
    let conn = connect(&mut s, &mut p);
    up(&mut s, &mut p, conn);
    feed(&mut s, &mut p, conn, ":Guiwald!u@h JOIN #a");
    feed(&mut s, &mut p, conn, ":Guiwald!u@h JOIN #b");
    let a = s.client.find_window(Some(conn), "#a").unwrap();
    let b = s.client.find_window(Some(conn), "#b").unwrap();
    assert_eq!(s.client.active(), Some(b));
    // Appli derrière : on nous nomme dans #a.
    s.set_app_focused(false, &mut p);
    feed(&mut s, &mut p, conn, ":bob!u@h PRIVMSG #a :Guiwald: coucou");
    let title = p.notes.last().cloned().expect("notification");
    // On la touche : #a passe devant.
    s.set_app_focused(true, &mut p);
    s.mobile_show_notified(&title, &mut p);
    assert_eq!(s.client.active(), Some(a));
    // Un titre inconnu ne change rien.
    s.mobile_show_notified("rien", &mut p);
    assert_eq!(s.client.active(), Some(a));
    let _ = std::fs::remove_dir_all(&d);
}
