//! Essais de l'application entière (sans écran ni réseau) : premier lancement, boutons « Parcourir… » branchés
//! sur la plateforme, bouton et menu « Absent », canaux favoris rejoints une fois la connexion prête, Liste des
//! canaux, Journaux.

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
    fn notify(&mut self, _title: &str, _body: &str) {}
    fn beep(&mut self) {}
    fn play_sound(&mut self, _path: &Path) {}
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
